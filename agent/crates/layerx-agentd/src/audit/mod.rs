//! Tenant-scoped, fail-closed audit evidence.

pub mod log;
#[path = "record.rs"]
mod records;

pub use log::{
    verify_chain, AppendReceipt, AuditError, ChainFailure, ChainIssue, Log, Verification,
};
pub use records::{
    read_entries, reconstruct_session, record, Coverage, Decision, DecisionEvidence, Entry,
    EventClass, Reconstruction, RecordError,
};
