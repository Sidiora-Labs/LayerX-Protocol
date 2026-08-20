//! Registered developer endpoints, their retry contract and the delivery health
//! the developer dashboards render.

use serde::{Deserialize, Serialize};

use crate::error::WebhookError;
use crate::events::{EndpointId, EventKind, Principal, Verification};
use crate::scheme::EndpointKey;

const MAXIMUM_URL: usize = 512;
const MAXIMUM_REASON: usize = 256;

/// Bounded deterministic retry contract for one unreachable endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Delay applied before the second attempt, in whole seconds.
    pub base_delay_seconds: u64,
    /// Upper bound on any computed delay, in whole seconds.
    pub maximum_delay_seconds: u64,
    /// Total attempts before the delivery is dead-lettered.
    pub maximum_attempts: u32,
    /// Deterministic spread applied around the doubled delay, in percent.
    pub spread_percent: u8,
    /// Consecutive dead letters that suspend the endpoint.
    pub suspend_after_dead_letters: u32,
    /// Seconds after which an attempt left in flight by a crash is retried.
    pub in_flight_timeout_seconds: u64,
}

impl RetryPolicy {
    /// Returns the hosted default: eight attempts spread from ten seconds to one
    /// hour, suspending an endpoint after twenty consecutive dead letters.
    #[must_use]
    pub const fn hosted() -> Self {
        Self {
            base_delay_seconds: 10,
            maximum_delay_seconds: 3_600,
            maximum_attempts: 8,
            spread_percent: 20,
            suspend_after_dead_letters: 20,
            in_flight_timeout_seconds: 120,
        }
    }

    /// Checks that every bound is usable.
    ///
    /// # Errors
    /// Returns [`WebhookError::InvalidRequest`] when a bound is zero, the
    /// maximum delay is below the base delay, or the spread exceeds a half.
    pub fn validate(self) -> Result<Self, WebhookError> {
        if self.base_delay_seconds == 0
            || self.maximum_delay_seconds < self.base_delay_seconds
            || self.maximum_attempts == 0
            || self.spread_percent > 50
            || self.in_flight_timeout_seconds == 0
        {
            return Err(WebhookError::InvalidRequest);
        }
        Ok(self)
    }

    /// Returns the delay before the given attempt number, doubling from the base
    /// delay and spread deterministically by the delivery's own digest so a
    /// replayed schedule is reproducible.
    #[must_use]
    pub fn backoff_seconds(&self, attempt: u32, spread_source: &[u8; 32]) -> u64 {
        let exponent = attempt.saturating_sub(1).min(16);
        let doubled = self
            .base_delay_seconds
            .saturating_mul(1_u64 << exponent)
            .min(self.maximum_delay_seconds);
        let swing = doubled.saturating_mul(u64::from(self.spread_percent)) / 100;
        let span = swing.saturating_mul(2).saturating_add(1);
        let index = usize::try_from(attempt % 32).unwrap_or(0);
        let offset = u64::from(spread_source[index]) % span;
        doubled
            .saturating_sub(swing)
            .saturating_add(offset)
            .clamp(1, self.maximum_delay_seconds)
    }
}

/// Accepts only a transport-secure destination, or a loopback destination for
/// local development, mirroring the rule the platform client applies.
///
/// # Errors
/// Returns [`WebhookError::InvalidRequest`] when the destination is oversized,
/// carries credentials or control characters, or is plaintext and not loopback.
pub fn validate_url(url: &str) -> Result<(), WebhookError> {
    if url.is_empty() || url.len() > MAXIMUM_URL {
        return Err(WebhookError::InvalidRequest);
    }
    if url.bytes().any(|byte| byte <= b' ' || byte == b'\x7f') {
        return Err(WebhookError::InvalidRequest);
    }
    let authority = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        let host = rest.split('/').next().unwrap_or_default();
        let loopback = host == "localhost"
            || host.starts_with("localhost:")
            || host == "127.0.0.1"
            || host.starts_with("127.0.0.1:")
            || host == "[::1]"
            || host.starts_with("[::1]:");
        if !loopback {
            return Err(WebhookError::InvalidRequest);
        }
        rest
    } else {
        return Err(WebhookError::InvalidRequest);
    };
    let host = authority.split('/').next().unwrap_or_default();
    if host.is_empty() || host.contains('@') {
        return Err(WebhookError::InvalidRequest);
    }
    Ok(())
}

/// One registered developer endpoint and its durable delivery counters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub(crate) id: EndpointId,
    pub(crate) principal: Principal,
    pub(crate) url: String,
    pub(crate) kinds: Vec<EventKind>,
    pub(crate) minimum_verification: Verification,
    pub(crate) key: EndpointKey,
    pub(crate) pending_key: Option<EndpointKey>,
    pub(crate) pending_key_activates_at: u64,
    pub(crate) created_at: u64,
    pub(crate) key_rotated_at: u64,
    pub(crate) suspended: bool,
    pub(crate) suspended_reason: Option<String>,
    pub(crate) consecutive_dead_letters: u32,
    pub(crate) delivered_total: u64,
    pub(crate) dead_lettered_total: u64,
    pub(crate) last_delivery_at: Option<u64>,
    pub(crate) last_failure: Option<String>,
    pub(crate) last_failure_at: Option<u64>,
}

impl Endpoint {
    /// Borrows the endpoint identifier.
    #[must_use]
    pub const fn id(&self) -> &EndpointId {
        &self.id
    }

    /// Borrows the owning principal.
    #[must_use]
    pub const fn principal(&self) -> &Principal {
        &self.principal
    }

    /// Borrows the destination.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Borrows the subscribed families. An empty list subscribes to all.
    #[must_use]
    pub fn kinds(&self) -> &[EventKind] {
        &self.kinds
    }

    /// Returns the weakest level this endpoint accepts.
    #[must_use]
    pub const fn minimum_verification(&self) -> Verification {
        self.minimum_verification
    }

    /// Returns whether the endpoint is currently suspended.
    #[must_use]
    pub const fn suspended(&self) -> bool {
        self.suspended
    }

    /// Returns true when this endpoint has asked for the family and accepts the
    /// level the event actually established.
    #[must_use]
    pub fn accepts(&self, kind: EventKind, verification: Verification) -> bool {
        let subscribed = self.kinds.is_empty() || self.kinds.contains(&kind);
        subscribed && verification.at_least(self.minimum_verification)
    }

    /// Borrows the key a delivery made now is signed under.
    ///
    /// A rotation keeps signing under the superseded key until the announced
    /// activation time, so a receiver can install the new public key before any
    /// delivery is signed with it. Only one signature is ever presented, so
    /// there is never a window in which a receiver must accept two keys at once.
    #[must_use]
    pub fn signing_key(&self, now: u64) -> &EndpointKey {
        match &self.pending_key {
            Some(pending) if now >= self.pending_key_activates_at => pending,
            _ => &self.key,
        }
    }

    /// Promotes a pending key once its activation time has passed, returning
    /// true when the durable record changed.
    pub fn promote_due_key(&mut self, now: u64) -> bool {
        if now < self.pending_key_activates_at {
            return false;
        }
        let Some(pending) = self.pending_key.take() else {
            return false;
        };
        self.key = pending;
        self.pending_key_activates_at = 0;
        true
    }

    /// Borrows the pending key, when a rotation is announced but not yet active.
    #[must_use]
    pub fn pending_key(&self) -> Option<&EndpointKey> {
        self.pending_key.as_ref()
    }

    /// Returns when the announced key starts signing.
    #[must_use]
    pub const fn pending_key_activates_at(&self) -> u64 {
        self.pending_key_activates_at
    }

    pub(crate) fn set_reason(&mut self, reason: &str) -> Result<(), WebhookError> {
        if reason.is_empty() || reason.len() > MAXIMUM_REASON || reason.contains(['\n', '\r', '\0'])
        {
            return Err(WebhookError::InvalidRequest);
        }
        self.suspended_reason = Some(reason.to_owned());
        Ok(())
    }
}

/// Delivery health for one endpoint, rendered by the developer dashboards.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EndpointHealth {
    /// Endpoint identifier.
    pub endpoint: String,
    /// Destination the deliveries are posted to.
    pub url: String,
    /// Subscribed families, empty meaning every family.
    pub kinds: Vec<EventKind>,
    /// Weakest level the endpoint accepts.
    pub minimum_verification: Verification,
    /// Whether delivery is currently suspended.
    pub suspended: bool,
    /// Why delivery was suspended, when it was.
    pub suspended_reason: Option<String>,
    /// Deliveries queued and never attempted.
    pub pending: u64,
    /// Deliveries whose attempt has not yet been resolved.
    pub in_flight: u64,
    /// Deliveries waiting for their next scheduled attempt.
    pub retrying: u64,
    /// Deliveries accepted by the endpoint.
    pub delivered_total: u64,
    /// Deliveries that exhausted their attempts.
    pub dead_lettered_total: u64,
    /// Consecutive dead letters since the last acceptance.
    pub consecutive_dead_letters: u32,
    /// Age in seconds of the oldest delivery that has not been accepted.
    pub oldest_undelivered_seconds: u64,
    /// When the next scheduled attempt is due.
    pub next_attempt_at: Option<u64>,
    /// When the endpoint last accepted a delivery.
    pub last_delivery_at: Option<u64>,
    /// The last recorded refusal or transport failure.
    pub last_failure: Option<String>,
    /// When the last refusal or transport failure was recorded.
    pub last_failure_at: Option<u64>,
    /// Identifier of the key deliveries are signed under right now.
    pub key_id: String,
    /// Public half of that key, base64 as the shipped consumers accept it.
    pub public_key: String,
    /// When the signing key was last rotated.
    pub key_rotated_at: u64,
    /// Identifier of an announced key that has not started signing yet.
    pub pending_key_id: Option<String>,
    /// Public half of the announced key.
    pub pending_public_key: Option<String>,
    /// When the announced key starts signing.
    pub pending_key_activates_at: Option<u64>,
}
