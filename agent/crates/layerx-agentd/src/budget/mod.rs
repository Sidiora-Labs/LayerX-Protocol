//! Protocol-backed and explicitly local spending limits.

mod create;
#[path = "reconcile.rs"]
mod accounting;

pub use create::{
    create_protocol_budget, BudgetCreationError, BudgetKind, BudgetPipeline, BudgetRequest,
    CoreBudgetReceipt, LocalLimit, ProtocolBudget,
};
pub use accounting::{
    LocalAccounting, ProtocolBudgetState, ReconcileError, ReconciliationState, VerifiedSpendReceipt,
};

/// Reconciles local budget cache state against verified protocol evidence.
pub fn reconcile(
    local: &mut LocalAccounting,
    protocol: ProtocolBudgetState,
    receipts: &[VerifiedSpendReceipt],
) -> Result<ReconciliationState, ReconcileError> {
    accounting::reconcile_state(local, protocol, receipts)
}
