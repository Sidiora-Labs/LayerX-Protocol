//! Ordered ingestion of exact core-produced protocol events.

#[path = "ingest.rs"]
mod ingestion;

pub use ingestion::{CoreEvent, EventIngestor, IngestError, Watermark};

/// Ingests one exact core-produced event through the bounded durable pipeline.
pub fn ingest(ingestor: &mut EventIngestor, event: CoreEvent) -> Result<(), IngestError> {
    ingestion::ingest_event(ingestor, event)
}
