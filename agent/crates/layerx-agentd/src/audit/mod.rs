//! Tenant-scoped, fail-closed audit evidence.

pub mod log;

pub use log::{
    verify_chain, AppendReceipt, AuditError, ChainFailure, ChainIssue, Log, Verification,
};
