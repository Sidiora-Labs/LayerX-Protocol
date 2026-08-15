//! Connection lifecycle for the sole core-boundary client.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use layerx_proof::receipt::AuthorizedBatch;
use layerx_types::payload::ModuleRegistry;

use crate::head::{Head, HeadError, HeadTracker};
use crate::lni::handshake::{perform, Handshake, HandshakeConfig, HandshakeError};
use crate::lni::report::capability_report;
use crate::lni::schema::Capability;
use crate::lni::transport::{ConnectionGate, Limits, TransportError, Uds};
use crate::receipt::{
    lookup, resolve_unknown, Lookup, LookupContext, ReceiptError, ReceiptSelector, Resolution,
};
use crate::submit::{submit_signed, Submission, SubmissionContext, SubmitError, UnknownCause};

/// Externally visible connection lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Connected,
    Degraded,
    Incompatible,
    Unreachable,
}

/// Bounded deterministic jitter schedule for reconnect attempts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconnectPolicy {
    pub maximum_attempts: u8,
    pub base_delay: Duration,
    pub maximum_delay: Duration,
    pub jitter_percent: u8,
}

impl ReconnectPolicy {
    /// Returns one bounded deterministic delay from the attempt and endpoint.
    #[must_use]
    pub fn delay(self, attempt: u8, endpoint: &str) -> Duration {
        let exponent = u32::from(attempt.min(31));
        let factor = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        let base = self
            .base_delay
            .checked_mul(factor)
            .unwrap_or(self.maximum_delay)
            .min(self.maximum_delay);
        let jitter_bound = base
            .as_millis()
            .saturating_mul(u128::from(self.jitter_percent))
            / 100;
        if jitter_bound == 0 {
            return base;
        }
        let seed = endpoint.bytes().fold(u128::from(attempt), |value, byte| {
            value.wrapping_mul(131).wrapping_add(u128::from(byte))
        });
        let jitter = seed % (jitter_bound + 1);
        base.saturating_add(Duration::from_millis(
            u64::try_from(jitter).unwrap_or(u64::MAX),
        ))
        .min(self.maximum_delay)
    }
}

/// Explicit client connection configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    pub endpoint: PathBuf,
    pub handshake: HandshakeConfig,
    pub limits: Limits,
    pub reconnect: ReconnectPolicy,
}

/// Failure to establish or safely refresh the boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionError {
    Transport(TransportError),
    Handshake(HandshakeError),
    Head(HeadError),
    AttemptsExhausted,
}

impl ConnectionError {
    /// Stable state corresponding to this connection failure.
    #[must_use]
    pub const fn state(self) -> ConnectionState {
        match self {
            Self::Handshake(
                HandshakeError::InterfaceIncompatible { .. }
                | HandshakeError::ProtocolVersion { .. }
                | HandshakeError::Network { .. }
                | HandshakeError::MalformedEnvelope
                | HandshakeError::MalformedNodeInfo,
            )
            | Self::Head(_) => ConnectionState::Incompatible,
            Self::Transport(_)
            | Self::Handshake(HandshakeError::Transport(_))
            | Self::AttemptsExhausted => ConnectionState::Unreachable,
        }
    }
}

/// Sole owner of a live core-boundary connection.
pub struct Client {
    config: ClientConfig,
    gate: ConnectionGate,
    transport: Option<Uds>,
    handshake: Handshake,
    head: HeadTracker,
    state: ConnectionState,
}

impl Client {
    /// Opens the configured Unix boundary and performs the mandatory handshake.
    ///
    /// # Errors
    ///
    /// Returns a typed transport or startup-refusal failure before exposing a
    /// client.
    pub fn connect(config: ClientConfig) -> Result<Self, ConnectionError> {
        let gate = ConnectionGate::new(config.limits.maximum_connections);
        let mut transport = Uds::connect(&config.endpoint, &gate, config.limits)
            .map_err(ConnectionError::Transport)?;
        let handshake =
            perform(&mut transport, &config.handshake, None).map_err(ConnectionError::Handshake)?;
        let head = HeadTracker::new(handshake.node());
        let state = state_for(&handshake);
        Ok(Self {
            config,
            gate,
            transport: Some(transport),
            handshake,
            head,
            state,
        })
    }

    /// Reopens and revalidates the boundary under bounded jittered backoff.
    ///
    /// # Errors
    ///
    /// Refuses incompatibility and head regression immediately; reports
    /// exhaustion after the configured number of transport attempts.
    pub fn reconnect(&mut self) -> Result<(), ConnectionError> {
        self.transport = None;
        let endpoint = self.config.endpoint.to_string_lossy();
        for attempt in 0..self.config.reconnect.maximum_attempts {
            if attempt != 0 {
                thread::sleep(self.config.reconnect.delay(attempt, &endpoint));
            }
            let Ok(mut transport) =
                Uds::connect(&self.config.endpoint, &self.gate, self.config.limits)
            else {
                self.state = ConnectionState::Unreachable;
                continue;
            };
            let handshake = perform(
                &mut transport,
                &self.config.handshake,
                Some(&self.handshake),
            )
            .map_err(ConnectionError::Handshake)?;
            self.head
                .update(handshake.node())
                .map_err(ConnectionError::Head)?;
            self.state = state_for(&handshake);
            self.handshake = handshake;
            self.transport = Some(transport);
            return Ok(());
        }
        Err(ConnectionError::AttemptsExhausted)
    }

    /// Marks a lost in-flight request without claiming its outcome.
    pub const fn mark_transport_lost(&mut self) {
        self.state = ConnectionState::Unreachable;
    }

    #[must_use]
    pub const fn state(&self) -> ConnectionState {
        self.state
    }

    #[must_use]
    pub const fn head(&self) -> Head {
        self.head.current()
    }

    #[must_use]
    pub const fn handshake(&self) -> &Handshake {
        &self.handshake
    }

    /// Requires a receipt key to have been advertised for its batch.
    ///
    /// # Errors
    ///
    /// Refuses an unadvertised key without attempting signature verification.
    pub fn require_receipt_key(&self, batch: u64, key: [u8; 32]) -> Result<(), HeadError> {
        self.head.require_sequencer_key(batch, key)
    }

    /// Verifies and transmits one signed activity through the sole boundary
    /// connection.
    ///
    /// # Errors
    ///
    /// Refuses unavailable submission capability, a disconnected client, or
    /// any pre-transmission canonical/signature failure.
    pub fn submit_signed(
        &mut self,
        registry: &ModuleRegistry,
        signer_public_key: [u8; 32],
        correlation_id: u64,
        attempt: u32,
        signed_bytes: &[u8],
    ) -> Result<Submission, SubmitError> {
        if !self.handshake.capabilities().contains(Capability::Submit) {
            return Err(SubmitError::UnavailableCapability);
        }
        let context = SubmissionContext {
            interface_version: self.handshake.node().interface_version,
            protocol_version: self.config.handshake.expected_protocol_version,
            network_id: self.config.handshake.expected_network_id,
            correlation_id,
            signer_public_key,
            attempt,
        };
        let transport = self.transport.as_mut().ok_or(SubmitError::Disconnected)?;
        let submission = submit_signed(transport, registry, context, signed_bytes)?;
        let transport_lost = matches!(
            &submission,
            Submission::Unknown(unknown)
                if matches!(unknown.cause(), UnknownCause::Transport(_))
        );
        if transport_lost {
            self.transport = None;
            self.state = ConnectionState::Unreachable;
        }
        Ok(submission)
    }

    /// Retrieves and verifies one receipt through the active boundary.
    ///
    /// # Errors
    ///
    /// Refuses a capability gap, disconnected boundary, malformed response,
    /// selector mismatch, or proof verification failure.
    pub fn lookup_receipt(
        &mut self,
        selector: ReceiptSelector,
        correlation_id: u64,
        authorised_batch: AuthorizedBatch,
    ) -> Result<Lookup, ReceiptError> {
        if !self
            .handshake
            .capabilities()
            .contains(Capability::ReceiptLookup)
        {
            return Err(ReceiptError::UnavailableCapability);
        }
        let transport = self.transport.as_mut().ok_or(ReceiptError::Disconnected)?;
        lookup(
            transport,
            selector,
            LookupContext {
                interface_version: self.handshake.node().interface_version,
                correlation_id,
                authorised_batch,
            },
        )
    }

    /// Runs bounded receipt-only resolution for an unknown submission.
    ///
    /// # Errors
    ///
    /// Refuses a capability gap, disconnected boundary, or invalid receipt
    /// evidence. Absence and transport loss remain `Resolution::Unknown`.
    pub fn resolve_unknown(
        &mut self,
        unknown: &crate::submit::Unknown,
        correlation_id: u64,
        authorised_batch: AuthorizedBatch,
        policy: ReconnectPolicy,
    ) -> Result<Resolution, ReceiptError> {
        if !self
            .handshake
            .capabilities()
            .contains(Capability::ReceiptLookup)
        {
            return Err(ReceiptError::UnavailableCapability);
        }
        let transport = self.transport.as_mut().ok_or(ReceiptError::Disconnected)?;
        resolve_unknown(
            transport,
            unknown,
            LookupContext {
                interface_version: self.handshake.node().interface_version,
                correlation_id,
                authorised_batch,
            },
            policy,
        )
    }
}

fn state_for(handshake: &Handshake) -> ConnectionState {
    if capability_report(handshake.capabilities())
        .gaps()
        .is_empty()
    {
        ConnectionState::Connected
    } else {
        ConnectionState::Degraded
    }
}
