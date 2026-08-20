//! Budget creation through the ordinary canonical write pipeline.

use crate::store::{ObjectKind, Store, StoreError, TenantId, TenantKey};

const LOCAL_BYPASS_STATEMENT: &str =
    "daemon-enforced only; bypassing layerx-agentd bypasses this limit";

/// Protocol object offered for a spending limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetKind {
    ProtocolBudget,
    CapabilityGrant,
}

/// Complete input to a protocol-enforced limit creation activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetRequest {
    pub tenant: TenantId,
    pub request_id: [u8; 32],
    pub kind: BudgetKind,
    pub asset: [u8; 32],
    pub ceiling: u128,
    pub expiry_sequence: u64,
    pub canonical_activity: Vec<u8>,
    pub signature: [u8; 64],
}

/// Core receipt proving creation of a protocol object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreBudgetReceipt {
    pub object_id: [u8; 32],
    pub canonical_receipt: Vec<u8>,
    pub verified: bool,
    pub executed: bool,
}

/// The mandatory prepare, sign, submit and verify seam.
pub trait BudgetPipeline {
    /// Prepares, signs, submits and verifies one budget creation activity.
    ///
    /// # Errors
    ///
    /// Returns `Submission` when the signed canonical activity does not reach core and produce a
    /// receipt.
    fn submit_budget(
        &mut self,
        request: &BudgetRequest,
    ) -> Result<CoreBudgetReceipt, BudgetCreationError>;
}

/// Successfully created protocol-backed budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolBudget {
    pub object_id: [u8; 32],
    pub kind: BudgetKind,
    pub receipt_bytes: Vec<u8>,
    pub enforcement: &'static str,
}

/// Honest daemon-only limit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalLimit {
    pub tenant: TenantId,
    pub id: [u8; 32],
    pub asset: [u8; 32],
    pub ceiling: u128,
    pub enforcement: &'static str,
    pub bypass_statement: &'static str,
}

impl LocalLimit {
    #[must_use]
    pub fn new(tenant: TenantId, id: [u8; 32], asset: [u8; 32], ceiling: u128) -> Self {
        Self {
            tenant,
            id,
            asset,
            ceiling,
            enforcement: "daemon-enforced",
            bypass_statement: LOCAL_BYPASS_STATEMENT,
        }
    }
}

#[derive(Debug)]
pub enum BudgetCreationError {
    EmptyActivity,
    InvalidLimit,
    Submission,
    UnverifiedReceipt,
    CoreRejected,
    Store(StoreError),
}

impl PartialEq for BudgetCreationError {
    fn eq(&self, other: &Self) -> bool {
        matches!((self, other), (Self::Store(_), Self::Store(_)))
            || std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl Eq for BudgetCreationError {}

impl From<StoreError> for BudgetCreationError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

/// Creates a protocol-enforced limit only through signed canonical bytes.
///
/// # Errors
///
/// Refuses a zero ceiling or expiry sequence and empty canonical activity bytes, returns
/// `UnverifiedReceipt` or `CoreRejected` when the core receipt is unverified or unexecuted, and
/// returns the store failure raised while caching the receipt.
pub fn create_protocol_budget(
    store: &mut Store,
    request: &BudgetRequest,
    pipeline: &mut dyn BudgetPipeline,
) -> Result<ProtocolBudget, BudgetCreationError> {
    if request.ceiling == 0 || request.expiry_sequence == 0 {
        return Err(BudgetCreationError::InvalidLimit);
    }
    if request.canonical_activity.is_empty() {
        return Err(BudgetCreationError::EmptyActivity);
    }
    let receipt = pipeline.submit_budget(request)?;
    if !receipt.verified {
        return Err(BudgetCreationError::UnverifiedReceipt);
    }
    if !receipt.executed {
        return Err(BudgetCreationError::CoreRejected);
    }
    let key = TenantKey::new(
        request.tenant.clone(),
        ObjectKind::Budget,
        receipt.object_id.to_vec(),
    )?;
    store.put_core_cache(key, receipt.canonical_receipt.clone())?;
    Ok(ProtocolBudget {
        object_id: receipt.object_id,
        kind: request.kind,
        receipt_bytes: receipt.canonical_receipt,
        enforcement: "protocol-enforced",
    })
}
