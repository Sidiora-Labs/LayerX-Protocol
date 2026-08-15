use std::collections::VecDeque;
use std::fmt::{Display, Formatter};

use layerx_types::verify::VerificationLevel;

use crate::store::{ObjectKind, Store, StoreError, TenantId, TenantKey};

const WATERMARK_ID: &[u8] = b"event-ingestion-watermark";
const WATERMARK_MAGIC: &[u8; 4] = b"LXEW";
const METADATA_PREFIX: &[u8] = b"event-evidence:";
const METADATA_MAGIC: &[u8; 4] = b"LXEM";

/// Durable position of the strict global event stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Watermark {
    pub last_ingested: Option<u64>,
    pub next_expected: u64,
}

/// Exact event bytes and the receipt evidence supplied by the core boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreEvent {
    pub global_sequence: u64,
    pub canonical_bytes: Vec<u8>,
    pub receipt_reference: Option<[u8; 32]>,
    pub receipt_verification_level: VerificationLevel,
}

/// Failures that prevent an event from entering the durable ordered stream.
#[derive(Debug)]
pub enum IngestError {
    InvalidCapacity,
    EmptyCoreEvent,
    ReceiptEvidenceMismatch,
    Repeated { sequence: u64 },
    OutOfOrder { expected: u64, received: u64 },
    Backpressure { capacity: usize },
    SequenceExhausted,
    CorruptWatermark,
    Store(StoreError),
}

impl Display for IngestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCapacity => {
                formatter.write_str("event ingestion capacity must be non-zero")
            }
            Self::EmptyCoreEvent => formatter.write_str("core event bytes must not be empty"),
            Self::ReceiptEvidenceMismatch => {
                formatter.write_str("receipt reference and verification level disagree")
            }
            Self::Repeated { sequence } => {
                write!(formatter, "event sequence {sequence} was repeated")
            }
            Self::OutOfOrder { expected, received } => write!(
                formatter,
                "event sequence {received} arrived out of order; expected {expected}"
            ),
            Self::Backpressure { capacity } => {
                write!(
                    formatter,
                    "event ingestion buffer reached capacity {capacity}"
                )
            }
            Self::SequenceExhausted => formatter.write_str("event sequence space is exhausted"),
            Self::CorruptWatermark => formatter.write_str("durable event watermark is corrupt"),
            Self::Store(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for IngestError {}

impl From<StoreError> for IngestError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

/// Tenant-scoped bounded event ingestion state.
#[derive(Debug)]
pub struct EventIngestor {
    store: Store,
    tenant: TenantId,
    capacity: usize,
    buffered: VecDeque<CoreEvent>,
    watermark: Watermark,
}

impl EventIngestor {
    /// Opens an ingestor at the durable watermark, or at `initial_sequence`
    /// when the tenant has never ingested an event.
    pub fn open(
        store: Store,
        tenant: TenantId,
        capacity: usize,
        initial_sequence: u64,
    ) -> Result<Self, IngestError> {
        if capacity == 0 {
            return Err(IngestError::InvalidCapacity);
        }
        let key = watermark_key(tenant.clone())?;
        let watermark = store
            .get(&key)
            .map(|value| decode_watermark(value.bytes()))
            .transpose()?
            .unwrap_or(Watermark {
                last_ingested: None,
                next_expected: initial_sequence,
            });
        Ok(Self {
            store,
            tenant,
            capacity,
            buffered: VecDeque::with_capacity(capacity),
            watermark,
        })
    }

    /// Returns the configured maximum number of undelivered in-memory events.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the current number of buffered events.
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    /// Returns the last durable position and next required sequence.
    #[must_use]
    pub const fn watermark(&self) -> Watermark {
        self.watermark
    }

    /// Removes the oldest buffered event without changing the durable watermark.
    pub fn take_next(&mut self) -> Option<CoreEvent> {
        self.buffered.pop_front()
    }

    /// Borrows the durable store for exact-byte inspection and downstream reads.
    #[must_use]
    pub const fn store(&self) -> &Store {
        &self.store
    }

    /// Consumes the ingestor and returns its durable store.
    #[must_use]
    pub fn into_store(self) -> Store {
        self.store
    }
}

pub(super) fn ingest_event(
    ingestor: &mut EventIngestor,
    event: CoreEvent,
) -> Result<(), IngestError> {
    validate_event(&event)?;
    let expected = ingestor.watermark.next_expected;
    if event.global_sequence < expected {
        return Err(IngestError::Repeated {
            sequence: event.global_sequence,
        });
    }
    if event.global_sequence > expected {
        return Err(IngestError::OutOfOrder {
            expected,
            received: event.global_sequence,
        });
    }
    if ingestor.buffered.len() == ingestor.capacity {
        return Err(IngestError::Backpressure {
            capacity: ingestor.capacity,
        });
    }
    let next_expected = expected
        .checked_add(1)
        .ok_or(IngestError::SequenceExhausted)?;
    let next_watermark = Watermark {
        last_ingested: Some(expected),
        next_expected,
    };
    let event_key = event_key(ingestor.tenant.clone(), expected)?;
    let metadata_key = metadata_key(ingestor.tenant.clone(), expected)?;
    let watermark_key = watermark_key(ingestor.tenant.clone())?;
    ingestor.store.record_event(
        event_key,
        event.canonical_bytes.clone(),
        metadata_key,
        encode_metadata(&event),
        watermark_key,
        encode_watermark(next_watermark),
    )?;
    ingestor.watermark = next_watermark;
    ingestor.buffered.push_back(event);
    Ok(())
}

fn validate_event(event: &CoreEvent) -> Result<(), IngestError> {
    if event.canonical_bytes.is_empty() {
        return Err(IngestError::EmptyCoreEvent);
    }
    let has_receipt = event.receipt_reference.is_some();
    let is_verified = event.receipt_verification_level != VerificationLevel::UNVERIFIED;
    if has_receipt != is_verified {
        return Err(IngestError::ReceiptEvidenceMismatch);
    }
    Ok(())
}

fn event_key(tenant: TenantId, sequence: u64) -> Result<TenantKey, StoreError> {
    TenantKey::new(tenant, ObjectKind::Event, sequence.to_be_bytes().to_vec())
}

fn metadata_key(tenant: TenantId, sequence: u64) -> Result<TenantKey, StoreError> {
    let mut object_id = METADATA_PREFIX.to_vec();
    object_id.extend_from_slice(&sequence.to_be_bytes());
    TenantKey::new(tenant, ObjectKind::Configuration, object_id)
}

fn watermark_key(tenant: TenantId) -> Result<TenantKey, StoreError> {
    TenantKey::new(tenant, ObjectKind::Configuration, WATERMARK_ID.to_vec())
}

fn encode_watermark(watermark: Watermark) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(21);
    bytes.extend_from_slice(WATERMARK_MAGIC);
    bytes.push(u8::from(watermark.last_ingested.is_some()));
    bytes.extend_from_slice(&watermark.last_ingested.unwrap_or(0).to_be_bytes());
    bytes.extend_from_slice(&watermark.next_expected.to_be_bytes());
    bytes
}

fn decode_watermark(bytes: &[u8]) -> Result<Watermark, IngestError> {
    if bytes.len() != 21 || &bytes[..4] != WATERMARK_MAGIC || bytes[4] > 1 {
        return Err(IngestError::CorruptWatermark);
    }
    let mut last = [0_u8; 8];
    last.copy_from_slice(&bytes[5..13]);
    let mut next = [0_u8; 8];
    next.copy_from_slice(&bytes[13..21]);
    let last_ingested = (bytes[4] == 1).then(|| u64::from_be_bytes(last));
    let next_expected = u64::from_be_bytes(next);
    if last_ingested.and_then(|last| last.checked_add(1)) != Some(next_expected) {
        return Err(IngestError::CorruptWatermark);
    }
    Ok(Watermark {
        last_ingested,
        next_expected,
    })
}

fn encode_metadata(event: &CoreEvent) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(38);
    bytes.extend_from_slice(METADATA_MAGIC);
    bytes.push(event.receipt_verification_level.wire_rank());
    bytes.push(u8::from(event.receipt_reference.is_some()));
    bytes.extend_from_slice(&event.receipt_reference.unwrap_or([0_u8; 32]));
    bytes
}
