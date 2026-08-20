#![forbid(unsafe_code)]

use layerx_fiat::FiatJourneyState;
use layerx_proof::receipt::{AuthorizedBatch, ReceiptCheck};
use layerx_sdk::production::verify_receipt;

pub const EXTERNAL_CUSTODY_LABEL: &str =
    "External custody: this independent market maker controls the off-platform funds and payout.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalAccount(pub [u8; 32]);

impl PrincipalAccount {
    /// Creates a non-zero ordinary account identifier.
    ///
    /// # Errors
    /// Refuses the zero identifier.
    pub fn new(value: [u8; 32]) -> Result<Self, RampError> {
        if value == [0; 32] {
            Err(RampError::InvalidPrincipal)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RampDirection {
    OnRamp,
    OffRamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RampOrder {
    pub direction: RampDirection,
    pub operator: PrincipalAccount,
    pub customer: PrincipalAccount,
    pub asset: [u8; 32],
    pub amount: u128,
    pub idempotency_key: [u8; 32],
    pub payer_grant: Option<[u8; 32]>,
}

impl RampOrder {
    /// Checks amount, asset, idempotency and optional payer-grant identity.
    ///
    /// # Errors
    /// Refuses zero or otherwise absent protocol identifiers.
    pub fn validate(&self) -> Result<(), RampError> {
        if self.asset == [0; 32] || self.amount == 0 || self.idempotency_key == [0; 32] {
            return Err(RampError::InvalidOrder);
        }
        if self.payer_grant == Some([0; 32]) {
            return Err(RampError::InvalidPayerGrant);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ReceiptEvidence {
    pub canonical_receipt: Vec<u8>,
    pub authorised_batch: AuthorizedBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalPayoutState {
    NotSubmitted,
    Pending,
    Settled { provider_evidence_digest: [u8; 32] },
    Reversed { provider_evidence_digest: [u8; 32] },
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaxeerRebalanceState {
    NotRequired,
    Pending,
    ReceiptVerified { receipt_digest: [u8; 32] },
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RampProgress {
    AwaitingExternalSettlement,
    AwaitingLayerXTransfer,
    LayerXLegVerified {
        receipt_digest: [u8; 32],
        external_payout: ExternalPayoutState,
    },
}

pub trait OrdinaryPrincipalPlane {
    type Error;
    /// Submits the ordinary 402LXP leg and returns only real receipt evidence.
    ///
    /// # Errors
    /// Returns the production boundary's error without translating it to success.
    fn transfer_402(&mut self, order: &RampOrder) -> Result<Option<ReceiptEvidence>, Self::Error>;
    /// Rebalances only the operator's account through the Paxeer boundary.
    ///
    /// # Errors
    /// Returns the production boundary's error without inventing a receipt.
    fn rebalance_through_paxeer(
        &mut self,
        operator: PrincipalAccount,
        asset: [u8; 32],
        amount: u128,
        idempotency_key: [u8; 32],
    ) -> Result<PaxeerRebalanceState, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RampError {
    InvalidPrincipal,
    InvalidOrder,
    InvalidPayerGrant,
    Receipt(ReceiptCheck),
    ReceiptMismatch,
    Plane,
}

pub struct RampToolkit;

impl RampToolkit {
    /// Advances an on-ramp only after provider credit and a verified 402LXP leg.
    ///
    /// # Errors
    /// Refuses invalid orders, plane failures and unverifiable receipts.
    pub fn on_ramp<P: OrdinaryPrincipalPlane>(
        order: &RampOrder,
        provider_credit: &FiatJourneyState,
        plane: &mut P,
    ) -> Result<RampProgress, RampError> {
        order.validate()?;
        if order.direction != RampDirection::OnRamp {
            return Err(RampError::InvalidOrder);
        }
        if !matches!(provider_credit, FiatJourneyState::Credited { .. }) {
            return Ok(RampProgress::AwaitingExternalSettlement);
        }
        let Some(evidence) = plane.transfer_402(order).map_err(|_| RampError::Plane)? else {
            return Ok(RampProgress::AwaitingLayerXTransfer);
        };
        verify_leg(order, &evidence, order.operator, order.customer).map(|receipt_digest| {
            RampProgress::LayerXLegVerified {
                receipt_digest,
                external_payout: ExternalPayoutState::NotSubmitted,
            }
        })
    }

    /// Verifies the customer-to-operator leg and preserves external payout state.
    ///
    /// # Errors
    /// Refuses invalid orders and unverifiable or mismatched receipts.
    pub fn off_ramp(
        order: &RampOrder,
        layerx_payment: Option<&ReceiptEvidence>,
        payout: ExternalPayoutState,
    ) -> Result<RampProgress, RampError> {
        order.validate()?;
        if order.direction != RampDirection::OffRamp {
            return Err(RampError::InvalidOrder);
        }
        let Some(evidence) = layerx_payment else {
            return Ok(RampProgress::AwaitingLayerXTransfer);
        };
        let receipt_digest = verify_leg(order, evidence, order.customer, order.operator)?;
        Ok(RampProgress::LayerXLegVerified {
            receipt_digest,
            external_payout: payout,
        })
    }

    /// Requests Paxeer rebalancing for the operator's own account.
    ///
    /// # Errors
    /// Refuses invalid orders and propagates the production plane failure.
    pub fn rebalance<P: OrdinaryPrincipalPlane>(
        order: &RampOrder,
        plane: &mut P,
    ) -> Result<PaxeerRebalanceState, RampError> {
        order.validate()?;
        plane
            .rebalance_through_paxeer(
                order.operator,
                order.asset,
                order.amount,
                order.idempotency_key,
            )
            .map_err(|_| RampError::Plane)
    }
}

fn verify_leg(
    order: &RampOrder,
    evidence: &ReceiptEvidence,
    from: PrincipalAccount,
    to: PrincipalAccount,
) -> Result<[u8; 32], RampError> {
    let verified = verify_receipt(&evidence.canonical_receipt, &evidence.authorised_batch)
        .map_err(|failure| RampError::Receipt(failure.check))?;
    let protocol = verified
        .receipt()
        .protocol()
        .ok_or(RampError::ReceiptMismatch)?;
    if protocol.from() != from.0
        || protocol.to() != to.0
        || protocol.asset() != order.asset
        || protocol.amount() != order.amount
    {
        return Err(RampError::ReceiptMismatch);
    }
    verified
        .evidence()
        .receipt_digest()
        .ok_or(RampError::ReceiptMismatch)
}

#[must_use]
pub const fn platform_ramp_toolkit() -> &'static str {
    "ordinary-principal-receipt-gated-market-maker-ramp"
}
