//! Concurrent, digest-bound approval holds with deterministic expiry.

use std::collections::BTreeMap;
use std::sync::Mutex;

use layerx_agent_api::prepare::{Disclosure, Prepared};
use layerx_types::ids::Did;
use layerx_types::verify::VerificationLevel;
use sha2::{Digest as _, Sha256};

use crate::capability::CapabilityId;
use crate::session::SessionId;
use crate::store::TenantId;

/// Complete audit context captured before an approval hold is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalContext {
    pub tenant: TenantId,
    pub agent: Did,
    pub session: SessionId,
    pub capability: CapabilityId,
    pub policy_version: String,
    pub request_id: [u8; 32],
}

/// Identity of the human or external approver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApproverId(String);

impl ApproverId {
    pub fn new(value: impl Into<String>) -> Result<Self, ApprovalError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ApprovalError::InvalidApprover);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Terminal choice submitted by an approver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalChoice {
    Approve,
    Reject,
}

/// Explicit lifecycle for a held request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalState {
    AwaitingApproval,
    Approved,
    Rejected,
    Expired,
    Defective,
}

/// Approval audit entry carrying every required request dimension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalAuditEntry {
    pub tenant: TenantId,
    pub agent: Did,
    pub session: SessionId,
    pub capability: CapabilityId,
    pub policy_version: String,
    pub request_id: [u8; 32],
    pub idempotency_key: String,
    pub decision: ApprovalState,
    pub reason: &'static str,
    pub resulting_activity_id: Option<[u8; 32]>,
    pub verification_level: VerificationLevel,
    pub approver: Option<ApproverId>,
    pub disclosure_digest: [u8; 32],
}

/// Disclosure-only view presented to an approver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalTicket {
    pub hold_id: [u8; 32],
    pub disclosure: Disclosure,
    pub disclosure_digest: [u8; 32],
    pub expires_at_sequence: u64,
    pub state: ApprovalState,
}

#[derive(Clone, Debug)]
struct HeldApproval {
    context: ApprovalContext,
    prepared: Prepared,
    created_at_sequence: u64,
    expires_at_sequence: u64,
    state: ApprovalState,
    audit: Option<ApprovalAuditEntry>,
    decision_claimed: bool,
}

/// Complete immutable snapshot used by the authenticated approval service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApprovalSnapshot {
    pub context: ApprovalContext,
    pub prepared: Prepared,
    pub created_at_sequence: u64,
    pub expires_at_sequence: u64,
    pub state: ApprovalState,
}

/// Thread-safe approval registry; state transitions serialize per hold.
#[derive(Default)]
pub struct ApprovalRegistry {
    holds: Mutex<BTreeMap<[u8; 32], HeldApproval>>,
}

impl ApprovalRegistry {
    pub fn ticket(&self, hold_id: [u8; 32]) -> Result<Option<ApprovalTicket>, ApprovalError> {
        let holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        Ok(holds.get(&hold_id).map(ticket_from_hold))
    }

    pub fn audit_entry(
        &self,
        hold_id: [u8; 32],
    ) -> Result<Option<ApprovalAuditEntry>, ApprovalError> {
        let holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        Ok(holds.get(&hold_id).and_then(|held| held.audit.clone()))
    }

    pub(crate) fn list_scoped(
        &self,
        tenant: &TenantId,
        current_sequence: u64,
    ) -> Result<Vec<ApprovalSnapshot>, ApprovalError> {
        let mut holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        let mut records = Vec::new();
        for held in holds.values_mut() {
            if &held.context.tenant == tenant {
                validate_for_read(held, current_sequence);
                records.push(snapshot(held));
            }
        }
        Ok(records)
    }

    pub(crate) fn get_scoped(
        &self,
        tenant: &TenantId,
        hold_id: [u8; 32],
        current_sequence: u64,
    ) -> Result<ApprovalSnapshot, ApprovalError> {
        let mut holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        let held = holds.get_mut(&hold_id).ok_or(ApprovalError::NotFound)?;
        if &held.context.tenant != tenant {
            return Err(ApprovalError::NotFound);
        }
        validate_for_read(held, current_sequence);
        Ok(snapshot(held))
    }

    pub(crate) fn claim_scoped(
        &self,
        tenant: &TenantId,
        hold_id: [u8; 32],
        current_sequence: u64,
    ) -> Result<ApprovalSnapshot, ApprovalError> {
        let mut holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        let held = holds.get_mut(&hold_id).ok_or(ApprovalError::NotFound)?;
        if &held.context.tenant != tenant {
            return Err(ApprovalError::NotFound);
        }
        validate_for_read(held, current_sequence);
        if held.state == ApprovalState::Defective {
            return Err(ApprovalError::Defective);
        }
        if held.state != ApprovalState::AwaitingApproval {
            return Err(ApprovalError::AlreadyDecided(held.state));
        }
        if held.decision_claimed {
            return Err(ApprovalError::DecisionConflict);
        }
        held.decision_claimed = true;
        Ok(snapshot(held))
    }

    pub(crate) fn complete_claim(
        &self,
        hold_id: [u8; 32],
        state: ApprovalState,
        approver: ApproverId,
        reason: &'static str,
        resulting_activity_id: Option<[u8; 32]>,
    ) -> Result<ApprovalAuditEntry, ApprovalError> {
        let mut holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        let held = holds.get_mut(&hold_id).ok_or(ApprovalError::NotFound)?;
        if !held.decision_claimed || held.state != ApprovalState::AwaitingApproval {
            return Err(ApprovalError::DecisionConflict);
        }
        let mut audit = terminal_audit(held, state, Some(approver), reason);
        audit.resulting_activity_id = resulting_activity_id;
        held.state = state;
        held.decision_claimed = false;
        held.audit = Some(audit.clone());
        Ok(audit)
    }

    pub(crate) fn abort_claim(&self, hold_id: [u8; 32]) -> Result<(), ApprovalError> {
        let mut holds = self.holds.lock().map_err(|_| ApprovalError::Unavailable)?;
        let held = holds.get_mut(&hold_id).ok_or(ApprovalError::NotFound)?;
        held.decision_claimed = false;
        Ok(())
    }
}

/// Approval hold refusal taxonomy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApprovalError {
    InvalidApprover,
    InvalidWindow,
    InvalidDisclosureDigest,
    ActorMismatch,
    DuplicateHold,
    NotFound,
    AlreadyDecided(ApprovalState),
    DisclosureChanged,
    Defective,
    DecisionConflict,
    Unavailable,
}

/// Holds only prepared bytes and their decoded disclosure, never the caller request.
pub fn hold(
    registry: &ApprovalRegistry,
    context: ApprovalContext,
    prepared: Prepared,
    current_sequence: u64,
    expires_at_sequence: u64,
) -> Result<ApprovalTicket, ApprovalError> {
    if expires_at_sequence <= current_sequence {
        return Err(ApprovalError::InvalidWindow);
    }
    let digest = canonical_digest(prepared.unsigned_canonical_bytes.as_bytes());
    if digest != prepared.disclosure.canonical_digest {
        return Err(ApprovalError::InvalidDisclosureDigest);
    }
    if context.agent.as_bytes() != prepared.disclosure.actor.as_str().as_bytes() {
        return Err(ApprovalError::ActorMismatch);
    }
    let hold_id = context.request_id;
    let mut holds = registry
        .holds
        .lock()
        .map_err(|_| ApprovalError::Unavailable)?;
    if holds.contains_key(&hold_id) {
        return Err(ApprovalError::DuplicateHold);
    }
    let held = HeldApproval {
        context,
        prepared,
        created_at_sequence: current_sequence,
        expires_at_sequence,
        state: ApprovalState::AwaitingApproval,
        audit: None,
        decision_claimed: false,
    };
    let ticket = ticket_from_hold(&held);
    holds.insert(hold_id, held);
    Ok(ticket)
}

/// Applies exactly one decision to the disclosure digest the approver saw.
pub fn decide(
    registry: &ApprovalRegistry,
    hold_id: [u8; 32],
    approver: ApproverId,
    choice: ApprovalChoice,
    presented_disclosure_digest: [u8; 32],
    current_sequence: u64,
) -> Result<ApprovalAuditEntry, ApprovalError> {
    let mut holds = registry
        .holds
        .lock()
        .map_err(|_| ApprovalError::Unavailable)?;
    let held = holds.get_mut(&hold_id).ok_or(ApprovalError::NotFound)?;
    if held.state != ApprovalState::AwaitingApproval {
        return Err(ApprovalError::AlreadyDecided(held.state));
    }
    if current_sequence >= held.expires_at_sequence {
        let audit = terminal_audit(
            held,
            ApprovalState::Expired,
            None,
            "approval_window_expired",
        );
        held.state = ApprovalState::Expired;
        held.audit = Some(audit);
        return Err(ApprovalError::AlreadyDecided(ApprovalState::Expired));
    }
    if presented_disclosure_digest != held.prepared.disclosure.canonical_digest {
        return Err(ApprovalError::DisclosureChanged);
    }
    let (state, reason) = match choice {
        ApprovalChoice::Approve => (ApprovalState::Approved, "approver_approved_disclosure"),
        ApprovalChoice::Reject => (ApprovalState::Rejected, "approver_rejected_disclosure"),
    };
    let audit = terminal_audit(held, state, Some(approver), reason);
    held.state = state;
    held.audit = Some(audit.clone());
    Ok(audit)
}

fn validate_for_read(held: &mut HeldApproval, current_sequence: u64) {
    if held.state != ApprovalState::AwaitingApproval {
        return;
    }
    if current_sequence >= held.expires_at_sequence {
        let audit = terminal_audit(
            held,
            ApprovalState::Expired,
            None,
            "approval_window_expired",
        );
        held.state = ApprovalState::Expired;
        held.audit = Some(audit);
        return;
    }
    let observed = canonical_digest(held.prepared.unsigned_canonical_bytes.as_bytes());
    if observed != held.prepared.disclosure.canonical_digest {
        let audit = terminal_audit(
            held,
            ApprovalState::Defective,
            None,
            "held_disclosure_digest_mismatch",
        );
        held.state = ApprovalState::Defective;
        held.audit = Some(audit);
    }
}

fn snapshot(held: &HeldApproval) -> ApprovalSnapshot {
    ApprovalSnapshot {
        context: held.context.clone(),
        prepared: held.prepared.clone(),
        created_at_sequence: held.created_at_sequence,
        expires_at_sequence: held.expires_at_sequence,
        state: held.state,
    }
}

/// Expires every elapsed hold against an explicit protocol-relative sequence.
pub fn expire(
    registry: &ApprovalRegistry,
    current_sequence: u64,
) -> Result<Vec<ApprovalAuditEntry>, ApprovalError> {
    let mut holds = registry
        .holds
        .lock()
        .map_err(|_| ApprovalError::Unavailable)?;
    let mut expired = Vec::new();
    for held in holds.values_mut() {
        if held.state == ApprovalState::AwaitingApproval
            && current_sequence >= held.expires_at_sequence
        {
            let audit = terminal_audit(
                held,
                ApprovalState::Expired,
                None,
                "approval_window_expired",
            );
            held.state = ApprovalState::Expired;
            held.audit = Some(audit.clone());
            expired.push(audit);
        }
    }
    Ok(expired)
}

fn ticket_from_hold(held: &HeldApproval) -> ApprovalTicket {
    ApprovalTicket {
        hold_id: held.context.request_id,
        disclosure: held.prepared.disclosure.clone(),
        disclosure_digest: held.prepared.disclosure.canonical_digest,
        expires_at_sequence: held.expires_at_sequence,
        state: held.state,
    }
}

fn terminal_audit(
    held: &HeldApproval,
    decision: ApprovalState,
    approver: Option<ApproverId>,
    reason: &'static str,
) -> ApprovalAuditEntry {
    ApprovalAuditEntry {
        tenant: held.context.tenant.clone(),
        agent: held.context.agent.clone(),
        session: held.context.session,
        capability: held.context.capability,
        policy_version: held.context.policy_version.clone(),
        request_id: held.context.request_id,
        idempotency_key: held.prepared.disclosure.idempotency_key.as_str().to_owned(),
        decision,
        reason,
        resulting_activity_id: None,
        verification_level: VerificationLevel::UNVERIFIED,
        approver,
        disclosure_digest: held.prepared.disclosure.canonical_digest,
    }
}

fn canonical_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
