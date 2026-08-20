//! Receipt-derived managed-agent spend and protocol-budget reconciliation.

use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};

use layerx_proof::receipt::{verify, AuthorizedBatch, VerificationFailure};
use layerx_types::verify::VerificationLevel;
use sha2::{Digest as _, Sha256};

use crate::store::PrincipalId;

/// Copy-catalog key shown when the verified protocol budget corrected the
/// receipt-derived local figure.
pub const RECONCILIATION_COPY_KEY: &str = "agent.spend.protocol-adopted";
/// Plain-language reconciliation note shared by both shells.
pub const RECONCILIATION_EXPLANATION: &str =
    "Recent verified activity and the network budget differed. The network figure is shown.";

/// The two native renderers consuming the same Human API agent record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AgentShell {
    Mobile,
    Desktop,
}

/// Immutable identity and protocol-account facts for one managed agent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendProfile {
    pub principal: PrincipalId,
    pub agent_id: [u8; 32],
    pub agent_account: [u8; 32],
    pub budget_id: [u8; 32],
    pub asset: [u8; 32],
}

impl SpendProfile {
    fn validate(&self) -> Result<(), SpendError> {
        if self.agent_id == [0; 32]
            || self.agent_account == [0; 32]
            || self.budget_id == [0; 32]
            || self.asset == [0; 32]
        {
            return Err(SpendError::InvalidProfile);
        }
        Ok(())
    }
}

/// State-proven protocol budget state at one atomic agent-layer head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolBudgetEvidence {
    pub agent_id: [u8; 32],
    pub budget_id: [u8; 32],
    pub asset: [u8; 32],
    pub period_start: u64,
    pub period_end: u64,
    pub window_start_sequence: u64,
    pub window_end_sequence: u64,
    pub observed_head_sequence: u64,
    pub limit: u128,
    pub consumed: u128,
    pub remaining: u128,
    pub verification_level: VerificationLevel,
    pub evidence_digest: [u8; 32],
}

/// Exact core receipt bytes and independent batch authority supplied through
/// the agent contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendReceiptEvidence {
    pub canonical_receipt: Vec<u8>,
    pub authorized_batch: AuthorizedBatch,
}

/// One head-consistent agent-layer read. The Human plane does not join data
/// obtained at different heads and therefore cannot hide concurrent activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpendSnapshot {
    pub protocol_budget: ProtocolBudgetEvidence,
    pub receipts: Vec<SpendReceiptEvidence>,
}

/// Agent-layer failures preserve unavailability and refusal as distinct states.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpendBoundaryError {
    Unavailable,
    Refused(&'static str),
    CorruptResponse,
}

/// The only source of protocol facts used by spend reconciliation.
pub trait SpendAgentContract {
    /// Reads the current budget period and every available receipt at one
    /// observed protocol head, scoped to the authenticated human principal.
    ///
    /// # Errors
    ///
    /// Returns typed unavailability, refusal, or corrupt-response failures;
    /// callers must not construct a partial spend view from any failure.
    fn spend_snapshot(
        &self,
        principal: &PrincipalId,
        agent_id: [u8; 32],
    ) -> Result<SpendSnapshot, SpendBoundaryError>;
}

/// Direction of a visible receipt/protocol discrepancy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationDirection {
    ProtocolHigher,
    ProtocolLower,
}

/// Honest status of the local receipt calculation against protocol authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpendReconciliationStatus {
    InSync,
    ProtocolAdopted {
        direction: ReconciliationDirection,
        difference: u128,
    },
}

/// Contract-shaped current-period spend carried by an agent record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSpendView {
    pub agent_id: [u8; 32],
    pub period_start: u64,
    pub period_end: u64,
    /// Authoritative figure shown to the user. It always adopts verified
    /// protocol budget state after comparing it with receipt-only accounting.
    pub spent: u128,
    /// Independently recomputed total from verified current-period receipts.
    pub receipt_spent: u128,
    pub limit: u128,
    pub remaining: u128,
    pub receipt_count: usize,
    pub verification_level: VerificationLevel,
    pub observed_head_sequence: u64,
    pub reconciliation: SpendReconciliationStatus,
    pub reconciliation_copy_key: Option<&'static str>,
    pub reconciliation_explanation: Option<&'static str>,
    pub evidence_digests: Vec<[u8; 32]>,
}

/// One shell-tagged projection of the shared Human API agent-spend contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellAgentSpend {
    pub shell: AgentShell,
    pub spend: AgentSpendView,
}

/// Both native shell projections derived from one atomic snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentSpendSurfaces {
    pub mobile: ShellAgentSpend,
    pub desktop: ShellAgentSpend,
}

/// Principal-scoped spend service. It holds no authoritative ledger: every
/// call reconstructs the local figure from verified receipts and then adopts
/// the verified protocol budget figure.
pub struct SpendReconciliation<B: SpendAgentContract> {
    boundary: B,
    profile: SpendProfile,
}

impl<B: SpendAgentContract> SpendReconciliation<B> {
    /// Binds the service to one human principal and one managed-agent account.
    ///
    /// # Errors
    ///
    /// Rejects zero protocol identifiers before any boundary read can occur.
    pub fn new(boundary: B, profile: SpendProfile) -> Result<Self, SpendError> {
        profile.validate()?;
        Ok(Self { boundary, profile })
    }

    /// Returns one shell projection. The requesting principal must be the
    /// human owner bound when the service was constructed.
    ///
    /// # Errors
    ///
    /// Refuses cross-principal access, malformed or unverified protocol state,
    /// and any receipt that does not verify or name this exact agent account.
    pub fn for_shell(
        &self,
        principal: &PrincipalId,
        shell: AgentShell,
    ) -> Result<ShellAgentSpend, SpendError> {
        Ok(ShellAgentSpend {
            shell,
            spend: self.reconcile(principal)?,
        })
    }

    /// Returns both shell projections from one agent-layer snapshot, ensuring
    /// mobile and desktop cannot disagree even while the agent is active.
    ///
    /// # Errors
    ///
    /// Returns the same typed failures as [`Self::for_shell`].
    pub fn both_shells(&self, principal: &PrincipalId) -> Result<AgentSpendSurfaces, SpendError> {
        let spend = self.reconcile(principal)?;
        Ok(AgentSpendSurfaces {
            mobile: ShellAgentSpend {
                shell: AgentShell::Mobile,
                spend: spend.clone(),
            },
            desktop: ShellAgentSpend {
                shell: AgentShell::Desktop,
                spend,
            },
        })
    }

    fn reconcile(&self, principal: &PrincipalId) -> Result<AgentSpendView, SpendError> {
        if principal != &self.profile.principal {
            return Err(SpendError::WrongPrincipal);
        }
        let snapshot = self
            .boundary
            .spend_snapshot(principal, self.profile.agent_id)?;
        validate_protocol_budget(&self.profile, &snapshot.protocol_budget)?;

        let protocol = snapshot.protocol_budget;
        let mut receipt_spent = 0_u128;
        let mut receipt_ids = BTreeSet::new();
        let mut evidence_digests = BTreeSet::from([protocol.evidence_digest]);
        let mut weakest_receipt_level = None;

        for material in &snapshot.receipts {
            let verified = verify(&material.canonical_receipt, &material.authorized_batch)?;
            let receipt = verified
                .receipt()
                .protocol()
                .ok_or(SpendError::ReceiptConflict(
                    "receipt is not a full protocol receipt",
                ))?;
            if receipt.from() != self.profile.agent_account {
                return Err(SpendError::ReceiptConflict(
                    "receipt debit does not name the managed agent",
                ));
            }
            if receipt.asset() != self.profile.asset {
                return Err(SpendError::ReceiptConflict(
                    "receipt asset does not match the managed-agent budget",
                ));
            }
            if receipt.global_sequence() < protocol.window_start_sequence
                || receipt.global_sequence() > protocol.window_end_sequence
                || receipt.global_sequence() > protocol.observed_head_sequence
            {
                return Err(SpendError::ReceiptConflict(
                    "receipt is outside the current verified budget period",
                ));
            }
            if !receipt_ids.insert(receipt.activity_id()) {
                return Err(SpendError::ReceiptConflict(
                    "duplicate receipt activity identifier",
                ));
            }
            receipt_spent = receipt_spent
                .checked_add(receipt.amount())
                .ok_or(SpendError::Arithmetic)?;
            let digest: [u8; 32] = Sha256::digest(verified.canonical_bytes()).into();
            evidence_digests.insert(digest);
            weakest_receipt_level = Some(
                weakest_receipt_level.map_or(verified.level(), |current: VerificationLevel| {
                    current.min(verified.level())
                }),
            );
        }

        let reconciliation = match receipt_spent.cmp(&protocol.consumed) {
            std::cmp::Ordering::Equal => SpendReconciliationStatus::InSync,
            std::cmp::Ordering::Less => SpendReconciliationStatus::ProtocolAdopted {
                direction: ReconciliationDirection::ProtocolHigher,
                difference: protocol.consumed - receipt_spent,
            },
            std::cmp::Ordering::Greater => SpendReconciliationStatus::ProtocolAdopted {
                direction: ReconciliationDirection::ProtocolLower,
                difference: receipt_spent - protocol.consumed,
            },
        };
        let diverged = !matches!(reconciliation, SpendReconciliationStatus::InSync);
        let verification_level = weakest_receipt_level
            .map_or(protocol.verification_level, |receipt_level| {
                receipt_level.min(protocol.verification_level)
            });

        Ok(AgentSpendView {
            agent_id: self.profile.agent_id,
            period_start: protocol.period_start,
            period_end: protocol.period_end,
            spent: protocol.consumed,
            receipt_spent,
            limit: protocol.limit,
            remaining: protocol.remaining,
            receipt_count: receipt_ids.len(),
            verification_level,
            observed_head_sequence: protocol.observed_head_sequence,
            reconciliation,
            reconciliation_copy_key: diverged.then_some(RECONCILIATION_COPY_KEY),
            reconciliation_explanation: diverged.then_some(RECONCILIATION_EXPLANATION),
            evidence_digests: evidence_digests.into_iter().collect(),
        })
    }
}

fn validate_protocol_budget(
    profile: &SpendProfile,
    protocol: &ProtocolBudgetEvidence,
) -> Result<(), SpendError> {
    if protocol.agent_id != profile.agent_id
        || protocol.budget_id != profile.budget_id
        || protocol.asset != profile.asset
    {
        return Err(SpendError::ProtocolConflict(
            "budget state does not name the requested managed agent",
        ));
    }
    if protocol.period_start >= protocol.period_end
        || protocol.window_start_sequence > protocol.window_end_sequence
        || protocol.observed_head_sequence < protocol.window_start_sequence
        || protocol.observed_head_sequence > protocol.window_end_sequence
    {
        return Err(SpendError::ProtocolConflict(
            "budget period boundaries are inconsistent",
        ));
    }
    if protocol.limit == 0
        || protocol.consumed > protocol.limit
        || protocol.limit.checked_sub(protocol.consumed) != Some(protocol.remaining)
    {
        return Err(SpendError::ProtocolConflict(
            "budget amounts are internally inconsistent",
        ));
    }
    if protocol.verification_level < VerificationLevel::STATE_PROVEN
        || protocol.evidence_digest == [0; 32]
    {
        return Err(SpendError::UnverifiedProtocolBudget);
    }
    Ok(())
}

/// Typed spend read failure. No failure carries a partially trusted view.
#[derive(Debug)]
pub enum SpendError {
    InvalidProfile,
    WrongPrincipal,
    UnverifiedProtocolBudget,
    ProtocolConflict(&'static str),
    ReceiptConflict(&'static str),
    Arithmetic,
    Boundary(SpendBoundaryError),
    Receipt(VerificationFailure),
}

impl Display for SpendError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProfile => formatter.write_str("managed-agent spend profile is invalid"),
            Self::WrongPrincipal => formatter.write_str("agent spend belongs to another principal"),
            Self::UnverifiedProtocolBudget => {
                formatter.write_str("protocol budget state is not state-proven")
            }
            Self::ProtocolConflict(reason) => {
                write!(formatter, "protocol budget conflict: {reason}")
            }
            Self::ReceiptConflict(reason) => write!(formatter, "spend receipt conflict: {reason}"),
            Self::Arithmetic => formatter.write_str("agent spend arithmetic overflow"),
            Self::Boundary(SpendBoundaryError::Unavailable) => {
                formatter.write_str("agent spend is temporarily unavailable")
            }
            Self::Boundary(SpendBoundaryError::Refused(reason)) => {
                write!(formatter, "agent spend read was refused: {reason}")
            }
            Self::Boundary(SpendBoundaryError::CorruptResponse) => {
                formatter.write_str("agent spend response was corrupt")
            }
            Self::Receipt(error) => {
                write!(formatter, "spend receipt verification failed: {error:?}")
            }
        }
    }
}

impl std::error::Error for SpendError {}

impl From<SpendBoundaryError> for SpendError {
    fn from(value: SpendBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<VerificationFailure> for SpendError {
    fn from(value: VerificationFailure) -> Self {
        Self::Receipt(value)
    }
}
