use std::collections::VecDeque;
use std::fmt::{Display, Formatter, Write};

use layerx_agent_api::identity::ContractError;
use layerx_agent_api::subscription::{
    Cursor as ApiCursor, CursorAcknowledgement, EventDelivery, EventIdentity, ReceiptReference,
    SubscriptionFilter, SubscriptionRecord, SubscriptionTarget,
};
use layerx_agent_api::track::ReceiptRef;
use layerx_agent_api::Sequence;
use sha2::{Digest, Sha256};

use super::ingestion::{durable_event, durable_sequences, IngestError};
use super::subscription::{Continuity, Cursor, Store as SubscriptionStore, SubscriptionError};
use crate::store::TenantId;

/// Public consumer contract for the at-least-once delivery interface.
pub const CONSUMER_DEDUPLICATION_OBLIGATION: &str =
    "Consumers must deduplicate every delivery by deduplication_id before applying side effects.";

/// Whether an event came from durable history or the live side of the seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryPhase {
    Backfill,
    Live,
}

/// Exact core event delivery with its immutable global position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveredEvent {
    pub global_sequence: u64,
    pub phase: DeliveryPhase,
    pub delivery: EventDelivery,
}

/// Observable boundary between historical and live delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackfillTransition {
    pub live_starts_at: u64,
    pub resume_cursor: ApiCursor,
}

/// One delivery attempt. Items remain at the front until explicitly accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryItem {
    Event(DeliveredEvent),
    BackfillComplete(BackfillTransition),
}

/// Bounded deterministic retry settings for an unreachable consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub base_delay_ms: u64,
    pub maximum_delay_ms: u64,
    pub jitter_percent: u8,
    pub maximum_attempts: u32,
}

impl RetryPolicy {
    fn validate(self) -> Result<Self, DeliveryError> {
        if self.base_delay_ms == 0
            || self.maximum_delay_ms < self.base_delay_ms
            || self.jitter_percent > 100
            || self.maximum_attempts == 0
        {
            Err(DeliveryError::InvalidRetryPolicy)
        } else {
            Ok(self)
        }
    }
}

/// Retry instruction after one failed endpoint delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPlan {
    pub attempt: u32,
    pub retry_at_ms: u64,
    pub delay_ms: u64,
}

/// Subscription delivery health derived from durable cursor positions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryHealth {
    pub lagging: bool,
    pub lag_sequences: u64,
    pub last_delivery_at_ms: Option<u64>,
    pub failure_count: u64,
    pub last_error: Option<String>,
}

/// Result of filling the bounded buffer from durable event history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PumpReport {
    pub buffered: usize,
    pub backfill_events: usize,
    pub live_events: usize,
    pub seam_queued: bool,
    pub lag_sequences: u64,
}

/// Delivery failures preserve the front item and its durable cursor.
#[derive(Debug)]
pub enum DeliveryError {
    InvalidCapacity,
    InvalidRetryPolicy,
    InvalidTenant,
    InvalidEvent,
    SequenceExhausted,
    ContinuityBlocked,
    Backpressure { capacity: usize, lag_sequences: u64 },
    NoPendingDelivery,
    RetryExhausted { attempts: u32 },
    Ingest(IngestError),
    Subscription(SubscriptionError),
}

impl Display for DeliveryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCapacity => formatter.write_str("delivery capacity must be non-zero"),
            Self::InvalidRetryPolicy => formatter.write_str("delivery retry policy is invalid"),
            Self::InvalidTenant => formatter.write_str("subscription tenant is invalid"),
            Self::InvalidEvent => formatter.write_str("durable event cannot form a delivery"),
            Self::SequenceExhausted => formatter.write_str("delivery cursor is exhausted"),
            Self::ContinuityBlocked => formatter.write_str("subscription continuity is blocked"),
            Self::Backpressure {
                capacity,
                lag_sequences,
            } => write!(
                formatter,
                "delivery buffer reached capacity {capacity} with sequence lag {lag_sequences}"
            ),
            Self::NoPendingDelivery => formatter.write_str("no delivery is pending"),
            Self::RetryExhausted { attempts } => {
                write!(
                    formatter,
                    "delivery retry limit reached after {attempts} attempts"
                )
            }
            Self::Ingest(error) => Display::fmt(error, formatter),
            Self::Subscription(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for DeliveryError {}

impl From<IngestError> for DeliveryError {
    fn from(value: IngestError) -> Self {
        Self::Ingest(value)
    }
}

impl From<SubscriptionError> for DeliveryError {
    fn from(value: SubscriptionError) -> Self {
        Self::Subscription(value)
    }
}

/// Bounded at-least-once delivery state for one exact-scope subscription.
#[derive(Debug)]
pub struct DeliveryEngine {
    subscriptions: SubscriptionStore,
    target: SubscriptionTarget,
    tenant: TenantId,
    live_start: u64,
    next_sequence: u64,
    capacity: usize,
    buffer: VecDeque<DeliveryItem>,
    seam_queued: bool,
    seam_confirmed: bool,
    retry_policy: RetryPolicy,
    consecutive_failures: u32,
    health: DeliveryHealth,
}

impl DeliveryEngine {
    /// Opens delivery from the last acknowledged durable cursor. `live_start`
    /// is the boundary watermark observed when backfill begins.
    pub fn open(
        subscriptions: SubscriptionStore,
        target: SubscriptionTarget,
        live_start: u64,
        capacity: usize,
        retry_policy: RetryPolicy,
    ) -> Result<Self, DeliveryError> {
        if capacity == 0 {
            return Err(DeliveryError::InvalidCapacity);
        }
        let retry_policy = retry_policy.validate()?;
        let record = subscriptions.get(&target)?;
        let tenant = TenantId::new(record.scope.tenant.as_str())
            .map_err(|_| DeliveryError::InvalidTenant)?;
        let next_sequence = subscriptions.resume_cursor(&target)?.0;
        Ok(Self {
            subscriptions,
            target,
            tenant,
            live_start,
            next_sequence,
            capacity,
            buffer: VecDeque::with_capacity(capacity),
            seam_queued: false,
            seam_confirmed: false,
            retry_policy,
            consecutive_failures: 0,
            health: DeliveryHealth {
                lagging: false,
                lag_sequences: 0,
                last_delivery_at_ms: None,
                failure_count: 0,
                last_error: None,
            },
        })
    }

    /// Returns the current health without exposing another subscription.
    #[must_use]
    pub const fn health(&self) -> &DeliveryHealth {
        &self.health
    }

    /// Returns the number of events and seam markers held in memory.
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns whether the backfill-to-live marker has been accepted.
    #[must_use]
    pub const fn seam_confirmed(&self) -> bool {
        self.seam_confirmed
    }

    /// Records successful transport acceptance and removes exactly the front
    /// item. Event delivery state is persisted before removal.
    pub fn accept_front(&mut self, delivered_at_ms: u64) -> Result<DeliveryItem, DeliveryError> {
        let item = self
            .buffer
            .front()
            .cloned()
            .ok_or(DeliveryError::NoPendingDelivery)?;
        match &item {
            DeliveryItem::Event(event) => {
                self.subscriptions
                    .mark_delivered(&self.target, Cursor::from(event.delivery.cursor))?;
                self.health.last_delivery_at_ms = Some(delivered_at_ms);
            }
            DeliveryItem::BackfillComplete(_) => {
                self.seam_confirmed = true;
            }
        }
        self.buffer.pop_front();
        self.consecutive_failures = 0;
        self.refresh_health()?;
        Ok(item)
    }

    /// Leaves the current front item intact and returns a bounded jittered
    /// retry instruction.
    pub fn fail_front(
        &mut self,
        failed_at_ms: u64,
        error: &str,
    ) -> Result<RetryPlan, DeliveryError> {
        if self.buffer.is_empty() {
            return Err(DeliveryError::NoPendingDelivery);
        }
        let attempt =
            self.consecutive_failures
                .checked_add(1)
                .ok_or(DeliveryError::RetryExhausted {
                    attempts: self.retry_policy.maximum_attempts,
                })?;
        if attempt > self.retry_policy.maximum_attempts {
            self.health.lagging = true;
            return Err(DeliveryError::RetryExhausted {
                attempts: self.retry_policy.maximum_attempts,
            });
        }
        self.consecutive_failures = attempt;
        self.health.failure_count = self.health.failure_count.saturating_add(1);
        self.health.last_error = Some(error.chars().take(256).collect());
        self.health.lagging = true;
        let delay_ms = retry_delay(
            self.retry_policy,
            self.target.subscription_id.as_str(),
            attempt,
        );
        Ok(RetryPlan {
            attempt,
            retry_at_ms: failed_at_ms.saturating_add(delay_ms),
            delay_ms,
        })
    }

    /// Durably acknowledges a cursor that this engine delivered previously.
    pub fn acknowledge(
        &mut self,
        acknowledgement: &CursorAcknowledgement,
    ) -> Result<SubscriptionRecord, DeliveryError> {
        let record = self.subscriptions.acknowledge(acknowledgement)?;
        self.refresh_health()?;
        Ok(record)
    }

    /// Consumes delivery state and returns the subscription store.
    #[must_use]
    pub fn into_subscriptions(self) -> SubscriptionStore {
        self.subscriptions
    }

    fn pump(&mut self) -> Result<PumpReport, DeliveryError> {
        let sequences = self.eligible_sequences()?;
        let mut backfill_events = 0;
        let mut live_events = 0;
        while self.buffer.len() < self.capacity {
            let Some(item) = self.next_source_item(&sequences)? else {
                break;
            };
            match &item {
                DeliveryItem::Event(event) => match event.phase {
                    DeliveryPhase::Backfill => backfill_events += 1,
                    DeliveryPhase::Live => live_events += 1,
                },
                DeliveryItem::BackfillComplete(_) => {}
            }
            self.buffer.push_back(item);
        }
        self.refresh_health()?;
        let report = PumpReport {
            buffered: self.buffer.len(),
            backfill_events,
            live_events,
            seam_queued: self.seam_queued,
            lag_sequences: self.health.lag_sequences,
        };
        if self.buffer.len() == self.capacity && self.source_pending(&sequences) {
            self.health.lagging = true;
            return Err(DeliveryError::Backpressure {
                capacity: self.capacity,
                lag_sequences: self.health.lag_sequences,
            });
        }
        Ok(report)
    }

    fn next_source_item(
        &mut self,
        sequences: &[u64],
    ) -> Result<Option<DeliveryItem>, DeliveryError> {
        let next = sequences
            .iter()
            .copied()
            .find(|sequence| *sequence >= self.next_sequence);
        if !self.seam_queued && next.is_none_or(|sequence| sequence >= self.live_start) {
            self.seam_queued = true;
            return Ok(Some(DeliveryItem::BackfillComplete(BackfillTransition {
                live_starts_at: self.live_start,
                resume_cursor: ApiCursor(Sequence(self.next_sequence)),
            })));
        }
        let Some(sequence) = next else {
            return Ok(None);
        };
        let core_event = durable_event(self.subscriptions.durable(), &self.tenant, sequence)?;
        let cursor_sequence = sequence
            .checked_add(1)
            .ok_or(DeliveryError::SequenceExhausted)?;
        let identity = event_identity(sequence, &core_event.canonical_bytes);
        let receipt_reference = match core_event.receipt_reference {
            Some(reference) => ReceiptReference::Verified {
                receipt_ref: ReceiptRef::new(hex_reference(reference))
                    .map_err(|_: ContractError| DeliveryError::InvalidEvent)?,
                verification_level: core_event.receipt_verification_level.into(),
            },
            None => ReceiptReference::None,
        };
        let delivery = EventDelivery::new(
            identity,
            core_event.canonical_bytes,
            ApiCursor(Sequence(cursor_sequence)),
            receipt_reference,
        )
        .map_err(|_: ContractError| DeliveryError::InvalidEvent)?;
        self.next_sequence = cursor_sequence;
        Ok(Some(DeliveryItem::Event(DeliveredEvent {
            global_sequence: sequence,
            phase: if sequence < self.live_start {
                DeliveryPhase::Backfill
            } else {
                DeliveryPhase::Live
            },
            delivery,
        })))
    }

    fn source_pending(&self, sequences: &[u64]) -> bool {
        !self.seam_queued
            || sequences
                .iter()
                .any(|sequence| *sequence >= self.next_sequence)
    }

    fn refresh_health(&mut self) -> Result<(), DeliveryError> {
        let sequences = self.eligible_sequences()?;
        let head = sequences
            .last()
            .copied()
            .map(|sequence| sequence.saturating_add(1))
            .unwrap_or(self.next_sequence);
        let acknowledged = self.subscriptions.resume_cursor(&self.target)?.0;
        self.health.lag_sequences = head.saturating_sub(acknowledged);
        self.health.lagging = self.health.lag_sequences > 0
            || self.consecutive_failures > 0
            || self.buffer.len() == self.capacity;
        Ok(())
    }

    fn eligible_sequences(&self) -> Result<Vec<u64>, DeliveryError> {
        if self.subscriptions.continuity(&self.target)? != Continuity::Healthy {
            return Err(DeliveryError::ContinuityBlocked);
        }
        // The tenant is fixed by the structurally scoped durable store and the
        // exact subscription target was resolved before its narrowing filter.
        let filter = &self.subscriptions.get(&self.target)?.filter;
        let mut eligible = Vec::new();
        for sequence in durable_sequences(self.subscriptions.durable(), &self.tenant)? {
            let event = durable_event(self.subscriptions.durable(), &self.tenant, sequence)?;
            if matches_filter(filter, &event.attributes) {
                eligible.push(sequence);
            }
        }
        Ok(eligible)
    }
}

pub(super) fn pump(engine: &mut DeliveryEngine) -> Result<PumpReport, DeliveryError> {
    engine.pump()
}

pub(super) fn delivery_attempt(
    engine: &mut DeliveryEngine,
) -> Result<Option<DeliveryItem>, DeliveryError> {
    if engine.buffer.is_empty() {
        match engine.pump() {
            Ok(_) | Err(DeliveryError::Backpressure { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(engine.buffer.front().cloned())
}

fn event_identity(sequence: u64, canonical_bytes: &[u8]) -> EventIdentity {
    let mut hasher = Sha256::new();
    hasher.update(sequence.to_be_bytes());
    hasher.update(canonical_bytes);
    EventIdentity::new(hasher.finalize().into())
}

fn matches_filter(filter: &SubscriptionFilter, event: &super::ingestion::EventAttributes) -> bool {
    filter
        .agents
        .values()
        .iter()
        .any(|value| value.value.as_str() == event.agent)
        && filter
            .accounts
            .values()
            .iter()
            .any(|value| value.value.as_str() == event.account)
        && filter
            .activity_types
            .values()
            .iter()
            .any(|value| value.0 == event.activity_type)
        && filter
            .modules
            .values()
            .iter()
            .any(|value| value.value.as_str() == event.module)
        && filter
            .assets
            .values()
            .iter()
            .any(|value| value.value.as_str() == event.asset)
        && filter
            .counterparties
            .values()
            .iter()
            .any(|value| value.value.as_str() == event.counterparty)
        && filter
            .result_classes
            .values()
            .iter()
            .any(|value| value.raw() == event.result_code)
}

fn hex_reference(reference: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in reference {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn retry_delay(policy: RetryPolicy, subscription_id: &str, attempt: u32) -> u64 {
    let exponent = attempt.saturating_sub(1).min(63);
    let base = policy
        .base_delay_ms
        .saturating_mul(1_u64.checked_shl(exponent).unwrap_or(u64::MAX))
        .min(policy.maximum_delay_ms);
    let jitter_bound = base.saturating_mul(u64::from(policy.jitter_percent)) / 100;
    if jitter_bound == 0 {
        return base;
    }
    let mut hasher = Sha256::new();
    hasher.update(subscription_id.as_bytes());
    hasher.update(attempt.to_be_bytes());
    let digest = hasher.finalize();
    let mut seed = [0_u8; 8];
    seed.copy_from_slice(&digest[..8]);
    let jitter = u64::from_be_bytes(seed) % (jitter_bound + 1);
    base.saturating_add(jitter).min(policy.maximum_delay_ms)
}
