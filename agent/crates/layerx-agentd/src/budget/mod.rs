//! Protocol-backed and explicitly local spending limits.

mod create;
#[path = "reconcile.rs"]
mod accounting;
#[path = "reserve.rs"]
mod reservations;
#[path = "hold.rs"]
mod recovery;

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
pub use recovery::{
    PersistedReceipt, RestartAccounting, RestartError, UnknownOutcome, UnknownReservation,
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

/// Persists a reservation whose submission outcome is unknown.
pub fn hold_unknown(
    store: &mut crate::store::Store,
    reservation: &UnknownReservation,
) -> Result<(), RestartError> {
    recovery::persist_unknown(store, reservation)
}

/// Rebuilds held and consumed accounting before writes are admitted.
pub fn rebuild(
    store: &crate::store::Store,
    tenant: crate::store::TenantId,
    unknown_ids: &[[u8; 32]],
    receipts: &[PersistedReceipt],
    protocol: ProtocolBudgetState,
) -> Result<RestartAccounting, RestartError> {
    recovery::rebuild_accounting(store, tenant, unknown_ids, receipts, protocol)
}
