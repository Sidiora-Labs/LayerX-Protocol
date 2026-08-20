//! A read-only projection of the hosted gateway's durable store.
//!
//! The dashboard never writes to the gateway store and never loads the fields
//! that authenticate a key: the mirrored records below carry the principal, the
//! quota and the disabled flag, and the salt and secret digest beside them are
//! simply not read. Audit records are matched by the same principal digest the
//! gateway itself records, so one developer's request log can never contain
//! another's line.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use layerx_platform_gateway::Quota;
use layerx_platform_webhooks::encoding::hex_encode;
use layerx_platform_webhooks::events::{Principal, Verification};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::DashboardError;
use crate::model::{
    per_mille, KeyView, ReceiptView, RequestOutcome, RequestRecord, RequestSummary, UsageSummary,
};

/// Name of the durable gateway store inside the configured root.
pub const STATE_FILE: &str = "gateway-state.json";

const KNOWN_OPERATIONS: [&str; 2] = ["POST /v1/activities", "GET /v1/state"];
const MAXIMUM_IDEMPOTENCY_KEY: usize = 128;

#[derive(Deserialize)]
struct KeyRecord {
    principal: String,
    quota: Quota,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct UsageRecord {
    window_started: u64,
    used: u64,
}

#[derive(Deserialize)]
struct OperationRecord {
    #[serde(default)]
    response: Vec<u8>,
    #[serde(default)]
    receipt: Vec<u8>,
    #[serde(default)]
    verification_level: String,
}

#[derive(Deserialize)]
struct IdempotencyRecord {
    request_digest: [u8; 32],
    result: OperationRecord,
}

#[derive(Clone, Copy, Deserialize)]
enum AuditOutcome {
    Completed,
    RateLimited,
    Refused,
}

impl AuditOutcome {
    const fn presented(self) -> RequestOutcome {
        match self {
            Self::Completed => RequestOutcome::Completed,
            Self::RateLimited => RequestOutcome::RateLimited,
            Self::Refused => RequestOutcome::Refused,
        }
    }
}

#[derive(Deserialize)]
struct AuditRecord {
    at: u64,
    principal_digest: [u8; 32],
    operation_digest: [u8; 32],
    outcome: AuditOutcome,
}

#[derive(Default, Deserialize)]
struct Mirror {
    #[serde(default)]
    keys: BTreeMap<String, KeyRecord>,
    #[serde(default)]
    usage: BTreeMap<String, UsageRecord>,
    #[serde(default)]
    idempotency: BTreeMap<String, IdempotencyRecord>,
    #[serde(default)]
    audit: Vec<AuditRecord>,
}

/// Everything the developer's gateway account looks like at one instant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    /// The issued keys and the quota window each is spending.
    pub keys: Vec<KeyView>,
    /// Quota and usage across those keys.
    pub usage: UsageSummary,
    /// The shape of the whole request log.
    pub requests: RequestSummary,
    /// The most recent request lines, newest first.
    pub recent_requests: Vec<RequestRecord>,
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn byte_count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn operation_name(digest: &[u8; 32]) -> Option<&'static str> {
    KNOWN_OPERATIONS
        .into_iter()
        .find(|operation| &hash(operation.as_bytes()) == digest)
}

fn scoped_idempotency(principal: &Principal, key: &str) -> String {
    hex_encode(&hash(
        &[principal.as_str().as_bytes(), b"\0", key.as_bytes()].concat(),
    ))
}

fn key_view(id: &str, record: &KeyRecord, usage: Option<&UsageRecord>, now: u64) -> KeyView {
    let recorded = usage.map_or(now, |value| value.window_started);
    let lapsed = usage.is_none() || now.saturating_sub(recorded) >= record.quota.window_seconds;
    let used = if lapsed {
        0
    } else {
        usage.map_or(0, |value| value.used)
    };
    let started = if lapsed { now } else { recorded };
    KeyView {
        key_id: id.to_owned(),
        principal: record.principal.clone(),
        disabled: record.disabled,
        requests_per_window: record.quota.requests,
        window_seconds: record.quota.window_seconds,
        used_in_window: used,
        remaining_in_window: record.quota.requests.saturating_sub(used),
        window_started_at: started,
        window_resets_at: started.saturating_add(record.quota.window_seconds),
        window_lapsed: lapsed,
        utilisation_per_mille: per_mille(used, record.quota.requests),
    }
}

fn key_views(state: &Mirror, principal: &Principal, now: u64) -> Vec<KeyView> {
    state
        .keys
        .iter()
        .filter(|(_, record)| record.principal == principal.as_str())
        .map(|(id, record)| key_view(id, record, state.usage.get(id), now))
        .collect()
}

fn usage_summary(keys: &[KeyView]) -> UsageSummary {
    let mut summary = UsageSummary::default();
    for key in keys {
        summary.keys = summary.keys.saturating_add(1);
        if key.disabled {
            summary.disabled_keys = summary.disabled_keys.saturating_add(1);
            continue;
        }
        summary.live_keys = summary.live_keys.saturating_add(1);
        summary.requests_allowed = summary
            .requests_allowed
            .saturating_add(key.requests_per_window);
        summary.requests_used = summary.requests_used.saturating_add(key.used_in_window);
        summary.requests_remaining = summary
            .requests_remaining
            .saturating_add(key.remaining_in_window);
    }
    summary.utilisation_per_mille = per_mille(summary.requests_used, summary.requests_allowed);
    summary
}

fn request_records(state: &Mirror, principal: &Principal, limit: usize) -> Vec<RequestRecord> {
    let owned = principal.audit_digest();
    state
        .audit
        .iter()
        .rev()
        .filter(|record| record.principal_digest == owned)
        .take(limit)
        .map(|record| RequestRecord {
            at: record.at,
            operation: operation_name(&record.operation_digest).map(str::to_owned),
            operation_digest: hex_encode(&record.operation_digest),
            outcome: record.outcome.presented(),
            verification: Verification::Unverified,
        })
        .collect()
}

fn request_summary(state: &Mirror, principal: &Principal) -> RequestSummary {
    let owned = principal.audit_digest();
    let mut summary = RequestSummary::default();
    for record in state
        .audit
        .iter()
        .filter(|record| record.principal_digest == owned)
    {
        summary.records = summary.records.saturating_add(1);
        match record.outcome {
            AuditOutcome::Completed => summary.completed = summary.completed.saturating_add(1),
            AuditOutcome::RateLimited => {
                summary.rate_limited = summary.rate_limited.saturating_add(1);
            }
            AuditOutcome::Refused => summary.refused = summary.refused.saturating_add(1),
        }
        summary.first_at = Some(summary.first_at.map_or(record.at, |at| at.min(record.at)));
        summary.last_at = Some(summary.last_at.map_or(record.at, |at| at.max(record.at)));
    }
    summary
}

fn receipt_view(idempotency_key: &str, record: &IdempotencyRecord) -> ReceiptView {
    let verification =
        Verification::parse(&record.result.verification_level).unwrap_or(Verification::Unverified);
    let evidence = !record.result.receipt.is_empty();
    ReceiptView {
        idempotency_key: idempotency_key.to_owned(),
        request_digest: hex_encode(&record.request_digest),
        receipt_digest: evidence.then(|| hex_encode(&hash(&record.result.receipt))),
        receipt_bytes: byte_count(record.result.receipt.len()),
        response_bytes: byte_count(record.result.response.len()),
        recorded_level: record.result.verification_level.clone(),
        verification,
        settled: evidence && verification.at_least(Verification::ReceiptVerified),
    }
}

/// The gateway store, opened for reading only.
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// Opens the durable gateway store for reading.
    ///
    /// # Errors
    /// Returns [`DashboardError::UnknownRoot`] when the root is not an existing
    /// directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, DashboardError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(DashboardError::UnknownRoot);
        }
        Ok(Self {
            path: root.join(STATE_FILE),
        })
    }

    fn read(&self) -> Result<Mirror, DashboardError> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| DashboardError::CorruptStore),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Mirror::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Reads the keys the principal holds and the window each is spending.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] for an undecodable store and
    /// [`DashboardError::Io`] when it cannot be read.
    pub fn keys(&self, principal: &Principal, now: u64) -> Result<Vec<KeyView>, DashboardError> {
        Ok(key_views(&self.read()?, principal, now))
    }

    /// Reads quota and usage across every key the principal holds.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] for an undecodable store and
    /// [`DashboardError::Io`] when it cannot be read.
    pub fn usage(&self, principal: &Principal, now: u64) -> Result<UsageSummary, DashboardError> {
        Ok(usage_summary(&key_views(&self.read()?, principal, now)))
    }

    /// Reads the principal's own request log, newest first.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] for an undecodable store and
    /// [`DashboardError::Io`] when it cannot be read.
    pub fn requests(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<RequestRecord>, DashboardError> {
        Ok(request_records(&self.read()?, principal, limit))
    }

    /// Reads keys, usage and the request log in one pass over the store.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] for an undecodable store and
    /// [`DashboardError::Io`] when it cannot be read.
    pub fn snapshot(
        &self,
        principal: &Principal,
        now: u64,
        limit: usize,
    ) -> Result<Snapshot, DashboardError> {
        let state = self.read()?;
        let keys = key_views(&state, principal, now);
        Ok(Snapshot {
            usage: usage_summary(&keys),
            keys,
            requests: request_summary(&state, principal),
            recent_requests: request_records(&state, principal, limit),
        })
    }

    /// Reads the receipt the gateway retained under the developer's own
    /// idempotency key, addressable only by the principal that made the request.
    ///
    /// # Errors
    /// Returns [`DashboardError::InvalidRequest`] for a key outside the bounds
    /// the gateway accepts, [`DashboardError::UnknownReceipt`] when no receipt
    /// is recorded under it, [`DashboardError::CorruptStore`] for an undecodable
    /// store and [`DashboardError::Io`] when it cannot be read.
    pub fn receipt(
        &self,
        principal: &Principal,
        idempotency_key: &str,
    ) -> Result<ReceiptView, DashboardError> {
        if idempotency_key.is_empty()
            || idempotency_key.len() > MAXIMUM_IDEMPOTENCY_KEY
            || !idempotency_key.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(DashboardError::InvalidRequest);
        }
        let state = self.read()?;
        let record = state
            .idempotency
            .get(&scoped_idempotency(principal, idempotency_key))
            .ok_or(DashboardError::UnknownReceipt)?;
        Ok(receipt_view(idempotency_key, record))
    }
}
