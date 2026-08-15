//! Receipt- and protocol-state-authoritative budget reconciliation.

/// Core-produced budget state for one deterministic window.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolBudgetState {
    pub consumed: u128,
    pub remaining: u128,
    pub window_start_sequence: u64,
    pub window_end_sequence: u64,
    pub observed_head_sequence: u64,
    pub verified: bool,
}

/// Persisted receipt applied to the local cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedSpendReceipt {
    pub receipt_id: [u8; 32],
    pub amount: u128,
    pub window_start_sequence: u64,
    pub verified: bool,
}

/// Rebuildable local cache, never authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalAccounting {
    pub consumed: u128,
    pub window_start_sequence: u64,
    pub last_receipt: Option<[u8; 32]>,
}

/// Observable reconciliation result including the corrected divergence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationState {
    pub last_verified_receipt: Option<[u8; 32]>,
    pub protocol_consumed: u128,
    pub local_before: u128,
    pub local_after: u128,
    pub divergence: Option<i128>,
    pub window_start_sequence: u64,
    pub window_end_sequence: u64,
    pub remaining: u128,
    pub observed_head_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconcileError {
    UnverifiedProtocolState,
    UnverifiedReceipt,
    ReceiptFromOtherWindow,
    Arithmetic,
}

/// Reconciles the cache to verified protocol state and records any correction.
pub(crate) fn reconcile_state(
    local: &mut LocalAccounting,
    protocol: ProtocolBudgetState,
    receipts: &[VerifiedSpendReceipt],
) -> Result<ReconciliationState, ReconcileError> {
    if !protocol.verified {
        return Err(ReconcileError::UnverifiedProtocolState);
    }
    let mut receipt_total = 0_u128;
    let mut last_receipt = None;
    for receipt in receipts {
        if !receipt.verified {
            return Err(ReconcileError::UnverifiedReceipt);
        }
        if receipt.window_start_sequence != protocol.window_start_sequence {
            return Err(ReconcileError::ReceiptFromOtherWindow);
        }
        receipt_total = receipt_total
            .checked_add(receipt.amount)
            .ok_or(ReconcileError::Arithmetic)?;
        last_receipt = Some(receipt.receipt_id);
    }
    if receipt_total > protocol.consumed {
        return Err(ReconcileError::Arithmetic);
    }
    let local_before = local.consumed;
    let divergence = if local_before == protocol.consumed {
        None
    } else {
        let protocol_value = i128::try_from(protocol.consumed).map_err(|_| ReconcileError::Arithmetic)?;
        let local_value = i128::try_from(local_before).map_err(|_| ReconcileError::Arithmetic)?;
        Some(protocol_value - local_value)
    };
    local.consumed = protocol.consumed;
    local.window_start_sequence = protocol.window_start_sequence;
    local.last_receipt = last_receipt;
    Ok(ReconciliationState {
        last_verified_receipt: last_receipt,
        protocol_consumed: protocol.consumed,
        local_before,
        local_after: local.consumed,
        divergence,
        window_start_sequence: protocol.window_start_sequence,
        window_end_sequence: protocol.window_end_sequence,
        remaining: protocol.remaining,
        observed_head_sequence: protocol.observed_head_sequence,
    })
}
