//! Authenticated approval operations over the policy hold registry.

use std::collections::BTreeMap;

use layerx_agent_api::prepare::{Disclosure, Prepared};
use sha2::{Digest as _, Sha256};

use crate::budget::{self, BudgetLimiter, ReleaseKind};
use crate::policy::approval::{
    ApprovalError as RegistryError, ApprovalRegistry, ApprovalSnapshot, ApprovalState, ApproverId,
};
use crate::store::TenantId;

/// Human-readable reason attached to the policy hold.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoldReason {
    pub code: &'static str,
    pub message: &'static str,
}

/// Tenant-scoped record returned by list and get.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecord {
    pub approval_id: [u8; 32],
    pub tenant: TenantId,
    pub held_activity: Disclosure,
    pub canonical_bytes_digest: [u8; 32],
    pub hold_reason: HoldReason,
    pub created_at_sequence: u64,
    pub expires_at_sequence: u64,
    pub state: ApprovalState,
}

/// Bounded deterministic approval page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalPage {
    pub approvals: Vec<ApprovalRecord>,
    pub next_cursor: Option<[u8; 32]>,
}

/// Total result vocabulary for approval decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalOutcome {
    Granted,
    Rejected,
    Expired,
    Defective,
    AlreadyDecided,
    Conflict,
}

/// Result returned to the requesting agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalDecision {
    pub outcome: ApprovalOutcome,
    pub submission_ref: Option<[u8; 32]>,
}

/// A real pre-signing submission queue containing the exact approved preparation.
#[derive(Default)]
pub struct ApprovalSubmissionQueue {
    queued: BTreeMap<[u8; 32], (TenantId, [u8; 32], Prepared)>,
}

impl ApprovalSubmissionQueue {
    fn release(
        &mut self,
        tenant: TenantId,
        approval_id: [u8; 32],
        prepared: Prepared,
    ) -> Result<[u8; 32], ApprovalOutcome> {
        let mut hasher = Sha256::new();
        hasher.update(b"layerx-approved-preparation-v1");
        hasher.update(tenant.as_str().as_bytes());
        hasher.update(approval_id);
        hasher.update(prepared.unsigned_canonical_bytes.as_bytes());
        let submission_ref: [u8; 32] = hasher.finalize().into();
        match self.queued.get(&submission_ref) {
            Some((stored_tenant, stored_approval, stored))
                if stored_tenant == &tenant
                    && stored_approval == &approval_id
                    && stored == &prepared =>
            {
                Ok(submission_ref)
            }
            Some(_) => Err(ApprovalOutcome::Conflict),
            None => {
                self.queued
                    .insert(submission_ref, (tenant, approval_id, prepared));
                Ok(submission_ref)
            }
        }
    }

    /// Returns the exact preparation released by approval.
    #[must_use]
    pub fn prepared(&self, submission_ref: [u8; 32]) -> Option<&Prepared> {
        self.queued.get(&submission_ref).map(|(_, _, value)| value)
    }

    /// Number of preparations waiting for the ordinary sign-and-submit path.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queued.len()
    }

    /// Whether no approved preparation is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queued.is_empty()
    }
}

/// Tenant-authenticated list/get/approve/reject service.
pub struct ApprovalService<'a> {
    registry: &'a ApprovalRegistry,
    limiter: &'a BudgetLimiter,
}

impl<'a> ApprovalService<'a> {
    #[must_use]
    pub const fn new(registry: &'a ApprovalRegistry, limiter: &'a BudgetLimiter) -> Self {
        Self { registry, limiter }
    }

    /// Lists only holds belonging to the authenticated tenant.
    ///
    /// # Errors
    ///
    /// Refuses an invalid page bound or an unavailable approval registry.
    pub fn list(
        &self,
        tenant: &TenantId,
        cursor: Option<[u8; 32]>,
        page_limit: usize,
        current_sequence: u64,
    ) -> Result<ApprovalPage, ApprovalOperationError> {
        if page_limit == 0 || page_limit > 100 {
            return Err(ApprovalOperationError::InvalidPageLimit);
        }
        let mut snapshots = self
            .registry
            .list_scoped(tenant, current_sequence)
            .map_err(ApprovalOperationError::Registry)?;
        snapshots.sort_by_key(|snapshot| snapshot.context.request_id);
        let mut records = snapshots
            .into_iter()
            .filter(|snapshot| cursor.is_none_or(|value| snapshot.context.request_id > value))
            .map(record)
            .collect::<Vec<_>>();
        let next_cursor = (records.len() > page_limit).then(|| records[page_limit - 1].approval_id);
        records.truncate(page_limit);
        Ok(ApprovalPage {
            approvals: records,
            next_cursor,
        })
    }

    /// Gets one hold without revealing whether another tenant owns its identifier.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` for absent and cross-tenant identifiers, or the registry failure.
    pub fn get(
        &self,
        tenant: &TenantId,
        approval_id: [u8; 32],
        current_sequence: u64,
    ) -> Result<ApprovalRecord, ApprovalOperationError> {
        self.registry
            .get_scoped(tenant, approval_id, current_sequence)
            .map(record)
            .map_err(ApprovalOperationError::Registry)
    }

    /// Releases exactly the held prepared activity into the ordinary submission queue.
    ///
    /// # Errors
    ///
    /// Returns registry failures that cannot be represented by a typed decision outcome.
    pub fn approve(
        &self,
        tenant: &TenantId,
        approval_id: [u8; 32],
        approver: ApproverId,
        current_sequence: u64,
        current_prepared: &Prepared,
        submissions: &mut ApprovalSubmissionQueue,
    ) -> Result<ApprovalDecision, ApprovalOperationError> {
        let claimed = match self
            .registry
            .claim_scoped(tenant, approval_id, current_sequence)
        {
            Ok(claimed) => claimed,
            Err(error) => return outcome_or_error(error),
        };
        let digest = canonical_digest(claimed.prepared.unsigned_canonical_bytes.as_bytes());
        if current_prepared != &claimed.prepared
            || digest != claimed.prepared.disclosure.canonical_digest
        {
            self.registry
                .complete_claim(
                    approval_id,
                    ApprovalState::Defective,
                    approver,
                    "held_preparation_changed_after_approval_hold",
                    None,
                )
                .map_err(ApprovalOperationError::Registry)?;
            return Ok(decision(ApprovalOutcome::Defective, None));
        }
        let submission_ref =
            match submissions.release(tenant.clone(), approval_id, claimed.prepared.clone()) {
                Ok(reference) => reference,
                Err(outcome) => {
                    self.registry
                        .abort_claim(approval_id)
                        .map_err(ApprovalOperationError::Registry)?;
                    return Ok(decision(outcome, None));
                }
            };
        self.registry
            .complete_claim(
                approval_id,
                ApprovalState::Approved,
                approver,
                "approver_released_exact_preparation",
                Some(submission_ref),
            )
            .map_err(ApprovalOperationError::Registry)?;
        Ok(decision(ApprovalOutcome::Granted, Some(submission_ref)))
    }

    /// Finalizes rejection and releases the hold's reservation deterministically.
    ///
    /// # Errors
    ///
    /// Returns registry or reservation failures that prevent a final rejection.
    pub fn reject(
        &self,
        tenant: &TenantId,
        approval_id: [u8; 32],
        approver: ApproverId,
        current_sequence: u64,
    ) -> Result<ApprovalDecision, ApprovalOperationError> {
        match self
            .registry
            .claim_scoped(tenant, approval_id, current_sequence)
        {
            Ok(_) => {}
            Err(error) => return outcome_or_error(error),
        }
        if let Err(error) = budget::release(
            self.limiter,
            approval_id,
            ReleaseKind::Failed,
            current_sequence,
        ) {
            self.registry
                .abort_claim(approval_id)
                .map_err(ApprovalOperationError::Registry)?;
            return Err(ApprovalOperationError::Reservation(error));
        }
        self.registry
            .complete_claim(
                approval_id,
                ApprovalState::Rejected,
                approver,
                "approver_rejected_and_released_reservation",
                None,
            )
            .map_err(ApprovalOperationError::Registry)?;
        Ok(decision(ApprovalOutcome::Rejected, None))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApprovalOperationError {
    InvalidPageLimit,
    Registry(RegistryError),
    Reservation(crate::budget::LimitRefusal),
}

fn record(snapshot: ApprovalSnapshot) -> ApprovalRecord {
    ApprovalRecord {
        approval_id: snapshot.context.request_id,
        tenant: snapshot.context.tenant,
        held_activity: snapshot.prepared.disclosure.clone(),
        canonical_bytes_digest: canonical_digest(
            snapshot.prepared.unsigned_canonical_bytes.as_bytes(),
        ),
        hold_reason: HoldReason {
            code: "policy_approval_required",
            message: "Policy requires human approval before submission",
        },
        created_at_sequence: snapshot.created_at_sequence,
        expires_at_sequence: snapshot.expires_at_sequence,
        state: snapshot.state,
    }
}

fn outcome_or_error(error: RegistryError) -> Result<ApprovalDecision, ApprovalOperationError> {
    match error {
        RegistryError::AlreadyDecided(ApprovalState::Expired) => {
            Ok(decision(ApprovalOutcome::Expired, None))
        }
        RegistryError::AlreadyDecided(_) => Ok(decision(ApprovalOutcome::AlreadyDecided, None)),
        RegistryError::Defective => Ok(decision(ApprovalOutcome::Defective, None)),
        RegistryError::DecisionConflict | RegistryError::DisclosureChanged => {
            Ok(decision(ApprovalOutcome::Conflict, None))
        }
        other => Err(ApprovalOperationError::Registry(other)),
    }
}

const fn decision(outcome: ApprovalOutcome, submission_ref: Option<[u8; 32]>) -> ApprovalDecision {
    ApprovalDecision {
        outcome,
        submission_ref,
    }
}

fn canonical_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
