//! The sole monetary exit from the programs runtime. Guest execution can only
//! produce typed 402LXP requests; this module binds those requests to the
//! invocation authority, submits one atomic set to the kernel transfer
//! primitive, and returns success only with a verified standard receipt.

use core::fmt::{self, Display};
use std::collections::{BTreeMap, BTreeSet};

use crate::abi::{AbiEffects, TransferRequest};
use crate::storage::{PrincipalId, ProgramId};

const SET_DOMAIN: &[u8] = b"LayerX/programs/402LXP/transfer-set/v1\0";
const MAX_TRANSFER_LEGS: usize = 256;

/// Authority fixed by the invoking protocol activity. No constructor accepts
/// a balance handle, account store, or mutation callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferCapability {
    program: ProgramId,
    principal: PrincipalId,
    invocation_authority: [u8; 32],
}

impl TransferCapability {
    /// Binds one programs invocation to its protocol-authenticated authority.
    ///
    /// # Errors
    ///
    /// Refuses the reserved zero authority digest.
    pub fn new(
        program: ProgramId,
        principal: PrincipalId,
        invocation_authority: [u8; 32],
    ) -> Result<Self, TransferLawError> {
        if invocation_authority == [0; 32] {
            return Err(TransferLawError::UnverifiedAuthority);
        }
        Ok(Self {
            program,
            principal,
            invocation_authority,
        })
    }

    /// Closes successful guest effects into the only form accepted by the
    /// kernel monetary boundary.
    ///
    /// # Errors
    ///
    /// Aborts on an empty or oversized set, invalid leg, authority mismatch,
    /// or arithmetic overflow.
    pub fn authorize(&self, effects: &AbiEffects) -> Result<AtomicTransferSet, TransferLawError> {
        if effects.transfers.is_empty() || effects.transfers.len() > MAX_TRANSFER_LEGS {
            return Err(TransferLawError::InvalidTransferSet);
        }
        let reachable = self.authorized_call_graph(effects)?;
        if effects.transfers.iter().any(|transfer| {
            transfer.principal != self.principal || !reachable.contains(&transfer.program)
        }) {
            return Err(TransferLawError::InvariantViolation);
        }
        let mut child_totals = BTreeMap::new();
        for transfer in &effects.transfers {
            if transfer.program == self.program {
                continue;
            }
            let key = (transfer.program, transfer.asset, transfer.to);
            let prior = child_totals.get(&key).copied().unwrap_or(0_u128);
            let amount = prior
                .checked_add(transfer.amount)
                .ok_or(TransferLawError::AmountOverflow)?;
            child_totals.insert(key, amount);
        }
        for ((program, asset, to), amount) in child_totals {
            if !effects.calls.iter().any(|call| {
                call.callee == program
                    && reachable.contains(&call.caller)
                    && call.principal == self.principal
                    && call.capabilities.permits_transfer(asset, to, amount)
            }) {
                return Err(TransferLawError::CapabilityEscalation);
            }
        }
        let mut total = 0u128;
        let mut canonical = Vec::with_capacity(
            SET_DOMAIN.len()
                + 112
                + effects.calls.len().saturating_mul(166)
                + effects.transfers.len().saturating_mul(112),
        );
        canonical.extend_from_slice(SET_DOMAIN);
        canonical.extend_from_slice(&self.program.bytes());
        canonical.extend_from_slice(&self.principal.bytes());
        canonical.extend_from_slice(&self.invocation_authority);
        canonical.extend_from_slice(&(effects.calls.len() as u64).to_be_bytes());
        for call in &effects.calls {
            canonical.extend_from_slice(&call.caller.bytes());
            canonical.extend_from_slice(&call.callee.bytes());
            canonical.extend_from_slice(&call.principal.bytes());
            let grants = call.capabilities.canonical_encoding();
            let grant_length =
                u32::try_from(grants.len()).map_err(|_| TransferLawError::InvariantViolation)?;
            canonical.extend_from_slice(&grant_length.to_be_bytes());
            canonical.extend_from_slice(&grants);
        }
        canonical.extend_from_slice(&(effects.transfers.len() as u64).to_be_bytes());
        for transfer in &effects.transfers {
            if transfer.principal != self.principal || !reachable.contains(&transfer.program) {
                return Err(TransferLawError::InvariantViolation);
            }
            if transfer.asset == [0; 32] || transfer.to == [0; 32] || transfer.amount == 0 {
                return Err(TransferLawError::InvalidTransfer);
            }
            total = total
                .checked_add(transfer.amount)
                .ok_or(TransferLawError::AmountOverflow)?;
            canonical.extend_from_slice(&transfer.asset);
            canonical.extend_from_slice(&transfer.to);
            canonical.extend_from_slice(&transfer.amount.to_be_bytes());
            canonical.extend_from_slice(&transfer.program.bytes());
        }
        Ok(AtomicTransferSet {
            program: self.program,
            principal: self.principal,
            invocation_authority: self.invocation_authority,
            canonical,
            total_amount: total,
            legs: effects.transfers.clone(),
        })
    }

    fn authorized_call_graph(
        &self,
        effects: &AbiEffects,
    ) -> Result<BTreeSet<ProgramId>, TransferLawError> {
        let mut reachable = BTreeSet::from([self.program]);
        let mut changed = true;
        while changed {
            changed = false;
            for call in &effects.calls {
                if call.principal != self.principal {
                    return Err(TransferLawError::InvariantViolation);
                }
                if reachable.contains(&call.caller) && reachable.insert(call.callee) {
                    changed = true;
                }
            }
        }
        if effects
            .calls
            .iter()
            .any(|call| !reachable.contains(&call.caller) || !reachable.contains(&call.callee))
        {
            return Err(TransferLawError::InvariantViolation);
        }
        Ok(reachable)
    }

    /// Applies all requested monetary effects atomically through the kernel's
    /// existing 402LXP primitive and verifies its single standard receipt.
    ///
    /// # Errors
    ///
    /// Returns a typed law, kernel, or receipt refusal. No partially applied
    /// state or unverified success can be returned through this API.
    pub fn settle(
        &self,
        effects: &AbiEffects,
        kernel: &mut impl KernelTransferPrimitive,
    ) -> Result<VerifiedProgramSettlement, TransferLawError> {
        let transfers = self.authorize(effects)?;
        kernel.apply_and_verify_402lxp_set(&transfers)
    }
}

/// Immutable atomic request passed to the core transfer module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicTransferSet {
    program: ProgramId,
    principal: PrincipalId,
    invocation_authority: [u8; 32],
    canonical: Vec<u8>,
    total_amount: u128,
    legs: Vec<TransferRequest>,
}

impl AtomicTransferSet {
    #[must_use]
    pub const fn program(&self) -> ProgramId {
        self.program
    }
    #[must_use]
    pub const fn principal(&self) -> PrincipalId {
        self.principal
    }
    #[must_use]
    pub const fn invocation_authority(&self) -> [u8; 32] {
        self.invocation_authority
    }
    #[must_use]
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
    #[must_use]
    pub const fn total_amount(&self) -> u128 {
        self.total_amount
    }
    #[must_use]
    pub fn legs(&self) -> &[TransferRequest] {
        &self.legs
    }
}

/// The existing kernel transfer-set primitive. It owns all balance mutation,
/// conservation enforcement, atomic rollback, standard receipt emission, and
/// receipt verification against the authorised batch.
pub trait KernelTransferPrimitive {
    /// Applies the exact set or none of it and returns only a receipt-verified
    /// result bound to the exact canonical set.
    ///
    /// # Errors
    ///
    /// Returns a typed core refusal without exposing partial mutation.
    fn apply_and_verify_402lxp_set(
        &mut self,
        transfers: &AtomicTransferSet,
    ) -> Result<VerifiedProgramSettlement, TransferLawError>;
}

/// Receipt-backed terminal result; there is no successful constructor that
/// bypasses the canonical verifier and core semantic policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedProgramSettlement {
    pub transfer_set_digest: [u8; 32],
    pub receipt_digest: [u8; 32],
    pub leg_count: usize,
    pub total_amount: u128,
}

/// Closed monetary-law refusal taxonomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferLawError {
    UnverifiedAuthority,
    InvalidTransfer,
    InvalidTransferSet,
    AmountOverflow,
    InvariantViolation,
    CapabilityEscalation,
    KernelRefused,
    ReceiptInvalid,
    ReceiptMismatch,
}

impl Display for TransferLawError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnverifiedAuthority => formatter.write_str("invocation authority is unverified"),
            Self::InvalidTransfer => formatter.write_str("402LXP transfer request is invalid"),
            Self::InvalidTransferSet => formatter.write_str("402LXP transfer set is invalid"),
            Self::AmountOverflow => formatter.write_str("402LXP transfer total overflowed"),
            Self::InvariantViolation => formatter.write_str("INVARIANT 1 monetary bypass detected"),
            Self::CapabilityEscalation => {
                formatter.write_str("child transfer exceeds narrowed call authority")
            }
            Self::KernelRefused => formatter.write_str("kernel transfer primitive refused the set"),
            Self::ReceiptInvalid => formatter.write_str("standard settlement receipt is invalid"),
            Self::ReceiptMismatch => formatter.write_str("receipt does not bind the transfer set"),
        }
    }
}

impl std::error::Error for TransferLawError {}
