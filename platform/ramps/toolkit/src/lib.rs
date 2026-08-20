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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RampStatusKey {
    Pending,
    Unknown,
    Done,
}

impl RampStatusKey {
    #[must_use]
    pub const fn copy_key(self) -> &'static str {
        match self {
            Self::Pending => "ramp.status.pending",
            Self::Unknown => "ramp.status.unknown",
            Self::Done => "ramp.status.done",
        }
    }
}

/// The only UI/API projection a ramp integration may expose. The external
/// custody label cannot be omitted, and `Done` cannot be constructed without
/// the receipt digest retained in [`RampProgress::LayerXLegVerified`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RampPresentation {
    external_custody_label_key: &'static str,
    status_key: RampStatusKey,
    layerx_receipt_digest: Option<[u8; 32]>,
    external_payout: ExternalPayoutState,
}

impl RampPresentation {
    #[must_use]
    pub const fn external_custody_label_key(&self) -> &'static str {
        self.external_custody_label_key
    }

    #[must_use]
    pub const fn status_key(&self) -> RampStatusKey {
        self.status_key
    }

    #[must_use]
    pub const fn layerx_receipt_digest(&self) -> Option<[u8; 32]> {
        self.layerx_receipt_digest
    }

    #[must_use]
    pub const fn external_payout(&self) -> ExternalPayoutState {
        self.external_payout
    }
}

impl From<RampProgress> for RampPresentation {
    fn from(progress: RampProgress) -> Self {
        match progress {
            RampProgress::AwaitingExternalSettlement | RampProgress::AwaitingLayerXTransfer => {
                Self {
                    external_custody_label_key: "ramp.external_custody.label",
                    status_key: RampStatusKey::Pending,
                    layerx_receipt_digest: None,
                    external_payout: ExternalPayoutState::NotSubmitted,
                }
            }
            RampProgress::LayerXLegVerified {
                receipt_digest,
                external_payout,
            } => Self {
                external_custody_label_key: "ramp.external_custody.label",
                status_key: RampStatusKey::Done,
                layerx_receipt_digest: Some(receipt_digest),
                external_payout,
            },
        }
    }
}

#[must_use]
pub fn ramp_labelling_gate(presentation: &RampPresentation) -> bool {
    if presentation.external_custody_label_key != "ramp.external_custody.label" {
        return false;
    }
    match presentation.status_key {
        RampStatusKey::Done => presentation.layerx_receipt_digest.is_some(),
        RampStatusKey::Pending | RampStatusKey::Unknown => {
            presentation.layerx_receipt_digest.is_none()
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        ramp_labelling_gate, ExternalPayoutState, RampPresentation, RampProgress, RampStatusKey,
    };

    #[test]
    fn pending_outcome_is_labelled_and_never_done() {
        let presentation = RampPresentation::from(RampProgress::AwaitingLayerXTransfer);
        assert_eq!(presentation.status_key, RampStatusKey::Pending);
        assert!(presentation.layerx_receipt_digest.is_none());
        assert!(ramp_labelling_gate(&presentation));
    }

    #[test]
    fn verified_layerx_leg_is_the_only_done_constructor() {
        let digest = [7; 32];
        let presentation = RampPresentation::from(RampProgress::LayerXLegVerified {
            receipt_digest: digest,
            external_payout: ExternalPayoutState::Unknown,
        });
        assert_eq!(presentation.status_key, RampStatusKey::Done);
        assert_eq!(presentation.layerx_receipt_digest, Some(digest));
        assert_eq!(presentation.external_payout, ExternalPayoutState::Unknown);
        assert!(ramp_labelling_gate(&presentation));
    }

    #[test]
    fn malformed_api_outcomes_fail_the_gate() {
        let missing_label = RampPresentation {
            external_custody_label_key: "status.done",
            status_key: RampStatusKey::Done,
            layerx_receipt_digest: Some([1; 32]),
            external_payout: ExternalPayoutState::Settled {
                provider_evidence_digest: [2; 32],
            },
        };
        assert!(!ramp_labelling_gate(&missing_label));
        let invented_done = RampPresentation {
            external_custody_label_key: "ramp.external_custody.label",
            status_key: RampStatusKey::Done,
            layerx_receipt_digest: None,
            external_payout: ExternalPayoutState::Settled {
                provider_evidence_digest: [2; 32],
            },
        };
        assert!(!ramp_labelling_gate(&invented_done));
    }
}
