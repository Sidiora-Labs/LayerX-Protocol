//! Protocol-backed and explicitly local spending limits.

mod create;
#[path = "reconcile.rs"]
mod accounting;
#[path = "reserve.rs"]
mod reservations;

pub use create::{
    create_protocol_budget, BudgetCreationError, BudgetKind, BudgetPipeline, BudgetRequest,
    CoreBudgetReceipt, LocalLimit, ProtocolBudget,
};
pub use accounting::{
    LocalAccounting, ProtocolBudgetState, ReconcileError, ReconciliationState, VerifiedSpendReceipt,
};
pub use reservations::{
    BudgetLimiter, BudgetReservation, LimitConfig, LimitId, LimitRefusal, LimitScope,
    ReleaseKind, ReservationRequest,
};

/// Reconciles local budget cache state against verified protocol evidence.
pub fn reconcile(
    local: &mut LocalAccounting,
    protocol: ProtocolBudgetState,
    receipts: &[VerifiedSpendReceipt],
) -> Result<ReconciliationState, ReconcileError> {
    accounting::reconcile_state(local, protocol, receipts)
}

/// Atomically reserves against every applicable scope.
pub fn reserve(
    limiter: &BudgetLimiter,
    request: &ReservationRequest,
) -> Result<BudgetReservation, LimitRefusal> {
    reservations::reserve_all(limiter, request)
}

/// Deterministically releases or consumes one reservation.
pub fn release(
    limiter: &BudgetLimiter,
    reservation_id: [u8; 32],
    kind: ReleaseKind,
    current_sequence: u64,
) -> Result<bool, LimitRefusal> {
    reservations::release_all(limiter, reservation_id, kind, current_sequence)
}
