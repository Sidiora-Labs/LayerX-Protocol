//! Tenant-scoped, fail-closed audit evidence.

pub mod log;
#[path = "record.rs"]
mod records;
#[path = "redact.rs"]
mod redaction;

pub use log::{
    verify_chain, AppendReceipt, AuditError, ChainFailure, ChainIssue, Log, Verification,
};
pub use records::{
    read_entries, reconstruct_session, record, Coverage, Decision, DecisionEvidence, Entry,
    EventClass, Reconstruction, RecordError, StoredReceiptEvidence,
};
pub use redaction::{
    protect_payload, redact, DataClass, OutputSurface, PayloadEvidence, Redacted, RedactionError,
    RedactionRegistry, RenderedOutput,
};
