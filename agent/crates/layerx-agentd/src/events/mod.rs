//! Ordered ingestion of exact core-produced protocol events.

#[path = "deliver.rs"]
mod delivery;
pub mod gap;
#[path = "ingest.rs"]
mod ingestion;
pub mod outbound;
pub mod subscription;

pub use delivery::{
    BackfillTransition, DeliveredEvent, DeliveryEngine, DeliveryError, DeliveryHealth,
    DeliveryItem, DeliveryPhase, PumpReport, RetryPlan, RetryPolicy,
    CONSUMER_DEDUPLICATION_OBLIGATION,
};
pub use ingestion::{CoreEvent, EventAttributes, EventIngestor, IngestError, Watermark};

/// Ingests one exact core-produced event through the bounded durable pipeline.
///
/// # Errors
///
/// Refuses an empty, malformed, or receipt-inconsistent event, a sequence that is
/// not the next expected one, and a buffer already at capacity; durable write
/// failures propagate.
pub fn ingest(ingestor: &mut EventIngestor, event: CoreEvent) -> Result<(), IngestError> {
    ingestion::ingest_event(ingestor, event)
}

/// Ingests a daemon-enforced restriction event into the same ordered stream
/// while retaining its local-only authority classification.
///
/// # Errors
///
/// Refuses an empty, malformed, or receipt-inconsistent event, a sequence that is
/// not the next expected one, and a buffer already at capacity; durable write
/// failures propagate.
pub fn ingest_local_restriction(
    ingestor: &mut EventIngestor,
    event: CoreEvent,
) -> Result<(), IngestError> {
    ingestion::ingest_local_event(ingestor, event)
}

/// Loads durable history into the engine's explicitly bounded delivery buffer.
///
/// # Errors
///
/// Returns `ContinuityBlocked` while a gap or truncation stands and `Backpressure`
/// when the buffer fills with source items still pending; durable read and
/// subscription failures propagate.
pub fn backfill(engine: &mut DeliveryEngine) -> Result<PumpReport, DeliveryError> {
    delivery::pump(engine)
}

/// Returns the current at-least-once delivery attempt without removing it.
///
/// # Errors
///
/// Returns `ContinuityBlocked` while a gap or truncation stands and propagates the
/// durable read and subscription failures of a refill; backpressure alone is not
/// an error.
pub fn deliver(engine: &mut DeliveryEngine) -> Result<Option<DeliveryItem>, DeliveryError> {
    delivery::delivery_attempt(engine)
}

/// Returns complete subscription delivery health including durable cursor and lag.
#[must_use]
pub fn health(engine: &DeliveryEngine) -> DeliveryHealth {
    engine.health_snapshot().clone()
}
