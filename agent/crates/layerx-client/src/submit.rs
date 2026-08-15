//! Byte-exact signed activity submission and indeterminate outcome handling.

use layerx_crypto::{ed25519, SignatureMessage, VerifyError};
use layerx_types::payload::ModuleRegistry;
use layerx_wire::activity::{decode_signed, encode_signed, encode_unsigned};
use layerx_wire::hash::Domain;
use layerx_wire::WireError;

use crate::lni::schema::{decode_envelope, encode_envelope, Envelope, SchemaError, Version};
use crate::lni::transport::{FrameTransport, TransportError};

const SUBMIT_REQUEST_TAG: u16 = 3;
const SUBMIT_RESPONSE_TAG: u16 = 4;

/// Immutable scope and identity for one transmission attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubmissionContext {
    pub interface_version: Version,
    pub protocol_version: u16,
    pub network_id: u32,
    pub correlation_id: u64,
    pub signer_public_key: [u8; 32],
    pub attempt: u32,
}

/// Core-produced evidence that the bytes reached admission processing.
///
/// This type intentionally contains no execution status or receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Acknowledgement {
    correlation_id: u64,
    idempotency_key: [u8; 32],
    admission_bytes: Vec<u8>,
    core_evidence: Vec<u8>,
}

impl Acknowledgement {
    #[must_use]
    pub const fn correlation_id(&self) -> u64 {
        self.correlation_id
    }

    #[must_use]
    pub const fn idempotency_key(&self) -> [u8; 32] {
        self.idempotency_key
    }

    #[must_use]
    pub fn admission_bytes(&self) -> &[u8] {
        &self.admission_bytes
    }

    #[must_use]
    pub fn core_evidence(&self) -> &[u8] {
        &self.core_evidence
    }
}

/// Why an attempted submission has no determinate outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCause {
    Transport(TransportError),
    IndeterminateResponse,
}

/// A first-class indeterminate state that can only be resolved by receipt
/// lookup under its idempotency key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unknown {
    idempotency_key: [u8; 32],
    correlation_id: u64,
    attempt: u32,
    cause: UnknownCause,
}

impl Unknown {
    #[must_use]
    pub const fn idempotency_key(self) -> [u8; 32] {
        self.idempotency_key
    }

    #[must_use]
    pub const fn correlation_id(self) -> u64 {
        self.correlation_id
    }

    #[must_use]
    pub const fn attempt(self) -> u32 {
        self.attempt
    }

    #[must_use]
    pub const fn cause(self) -> UnknownCause {
        self.cause
    }
}

/// The only two states a submission exchange itself can establish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Submission {
    Acknowledged(Acknowledgement),
    Unknown(Unknown),
}

/// A refusal detected before any bytes are transmitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    Wire(WireError),
    SignatureLength(usize),
    Signature(VerifyError),
    ProtocolVersion { expected: u16, actual: u16 },
    Network { expected: u32, actual: u32 },
    Envelope(SchemaError),
    UnavailableCapability,
    Disconnected,
}

impl From<WireError> for SubmitError {
    fn from(value: WireError) -> Self {
        Self::Wire(value)
    }
}

impl From<SchemaError> for SubmitError {
    fn from(value: SchemaError) -> Self {
        Self::Envelope(value)
    }
}

/// Verifies and sends exactly the supplied signed canonical bytes once.
///
/// Transport loss and malformed or mismatched responses are returned as
/// [`Submission::Unknown`], never as success or failure. Callers must resolve
/// that state by receipt lookup under the returned idempotency key.
///
/// # Errors
///
/// Refuses malformed, non-canonical, incorrectly scoped, or incorrectly signed
/// bytes before opening the transmission path.
pub fn submit_signed(
    transport: &mut dyn FrameTransport,
    registry: &ModuleRegistry,
    context: SubmissionContext,
    signed_bytes: &[u8],
) -> Result<Submission, SubmitError> {
    let activity = decode_signed(signed_bytes, registry)?;
    if activity.protocol_version() != context.protocol_version {
        return Err(SubmitError::ProtocolVersion {
            expected: context.protocol_version,
            actual: activity.protocol_version(),
        });
    }
    if activity.network_id() != context.network_id {
        return Err(SubmitError::Network {
            expected: context.network_id,
            actual: activity.network_id(),
        });
    }
    if encode_signed(&activity)? != signed_bytes {
        return Err(SubmitError::Wire(WireError {
            result: layerx_types::result::KnownResult::NonCanonical.into(),
            offset: 0,
        }));
    }
    let signature_bytes = activity
        .signature()
        .ok_or(SubmitError::SignatureLength(0))?;
    let signature: [u8; 64] = signature_bytes
        .try_into()
        .map_err(|_| SubmitError::SignatureLength(signature_bytes.len()))?;
    let unsigned = encode_unsigned(&activity)?;
    let message = SignatureMessage::new(
        Domain::SignaturePreimage,
        activity.protocol_version(),
        activity.network_id(),
        &unsigned,
    )
    .map_err(SubmitError::Signature)?;
    ed25519::verify(&context.signer_public_key, &signature, message)
        .map_err(SubmitError::Signature)?;

    let idempotency_key = activity.idempotency_key();
    let request = encode_envelope(Envelope {
        version: context.interface_version,
        message_tag: SUBMIT_REQUEST_TAG,
        correlation_id: context.correlation_id,
        canonical_payload: signed_bytes,
        proof_material: &[],
    })?;
    if let Err(error) = transport.send(&request) {
        return Ok(Submission::Unknown(unknown(
            context,
            idempotency_key,
            UnknownCause::Transport(error),
        )));
    }
    let response = match transport.receive() {
        Ok(response) => response,
        Err(error) => {
            return Ok(Submission::Unknown(unknown(
                context,
                idempotency_key,
                UnknownCause::Transport(error),
            )));
        }
    };
    let Ok(response) = decode_envelope(&response) else {
        return Ok(Submission::Unknown(unknown(
            context,
            idempotency_key,
            UnknownCause::IndeterminateResponse,
        )));
    };
    if response.version.major != context.interface_version.major
        || response.message_tag != SUBMIT_RESPONSE_TAG
        || response.correlation_id != context.correlation_id
    {
        return Ok(Submission::Unknown(unknown(
            context,
            idempotency_key,
            UnknownCause::IndeterminateResponse,
        )));
    }
    Ok(Submission::Acknowledged(Acknowledgement {
        correlation_id: context.correlation_id,
        idempotency_key,
        admission_bytes: response.canonical_payload.to_vec(),
        core_evidence: response.proof_material.to_vec(),
    }))
}

const fn unknown(
    context: SubmissionContext,
    idempotency_key: [u8; 32],
    cause: UnknownCause,
) -> Unknown {
    Unknown {
        idempotency_key,
        correlation_id: context.correlation_id,
        attempt: context.attempt,
        cause,
    }
}
