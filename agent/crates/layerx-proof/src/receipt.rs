//! Offline verification of sequencer-signed canonical receipts.

use layerx_crypto::ed25519;
use layerx_wire::hash::receipt_digest;
use layerx_wire::receipt::{decode, encode, encode_unsigned, Receipt};

use crate::evidence::Evidence;
use crate::level::achieved;

/// Core-published batch facts needed to establish sequencer authority and the
/// state-root chain for one receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorizedBatch {
    batch_id: [u8; 32],
    asset: [u8; 32],
    previous_state_root: [u8; 32],
    resulting_state_root: [u8; 32],
    sequencer_public_key: [u8; 32],
}

impl AuthorizedBatch {
    /// Creates the independently supplied batch evidence for one receipt.
    #[must_use]
    pub const fn new(
        batch_id: [u8; 32],
        asset: [u8; 32],
        previous_state_root: [u8; 32],
        resulting_state_root: [u8; 32],
        sequencer_public_key: [u8; 32],
    ) -> Self {
        Self {
            batch_id,
            asset,
            previous_state_root,
            resulting_state_root,
            sequencer_public_key,
        }
    }

    #[must_use]
    pub const fn batch_id(&self) -> [u8; 32] {
        self.batch_id
    }

    #[must_use]
    pub const fn asset(&self) -> [u8; 32] {
        self.asset
    }

    #[must_use]
    pub const fn previous_state_root(&self) -> [u8; 32] {
        self.previous_state_root
    }

    #[must_use]
    pub const fn resulting_state_root(&self) -> [u8; 32] {
        self.resulting_state_root
    }

    #[must_use]
    pub const fn sequencer_public_key(&self) -> [u8; 32] {
        self.sequencer_public_key
    }
}

/// The exact verification stage that rejected a receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptCheck {
    /// Canonical decoding failed.
    Decode,
    /// Re-encoding did not reproduce the supplied bytes.
    CanonicalEncoding,
    /// The byte string was not a full signed protocol receipt.
    ReceiptShape,
    /// A required signature was absent.
    MissingSignature,
    /// The protocol version was not the supported version.
    ProtocolVersion,
    /// The core result was not successful.
    ResultCode,
    /// The operation tag was absent.
    Operation,
    /// The activity identifier was all zero.
    ActivityId,
    /// The batch identifier did not match the authorised batch.
    BatchId,
    /// The receipt asset did not match the independently supplied batch fact.
    Asset,
    /// The previous state root did not match the supplied chain predecessor.
    PreviousStateRoot,
    /// The resulting state root did not match the supplied chain successor.
    ResultingStateRoot,
    /// The debit balance did not decrease by exactly the amount.
    DebitBalance,
    /// The credit balance did not increase by exactly the amount.
    CreditBalance,
    /// The signature did not verify under the authorised sequencer key.
    SequencerSignature,
}

/// A typed receipt failure that never masquerades as a lower verified level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationFailure {
    /// Exact failed verification stage.
    pub check: ReceiptCheck,
}

impl VerificationFailure {
    const fn at(check: ReceiptCheck) -> Self {
        Self { check }
    }
}

/// A receipt whose canonical encoding, invariants, root chain, and authorised
/// sequencer signature all passed locally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedReceipt {
    receipt: Receipt,
    canonical_bytes: Vec<u8>,
    evidence: Evidence,
}

impl VerifiedReceipt {
    /// Borrows the decoded core receipt.
    #[must_use]
    pub const fn receipt(&self) -> &Receipt {
        &self.receipt
    }

    /// Borrows the exact core-produced bytes that were verified.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact level established by this verification routine.
    #[must_use]
    pub const fn level(&self) -> layerx_types::verify::VerificationLevel {
        achieved(&self.evidence)
    }

    /// Returns the receipt digest and achieved level as one immutable record.
    #[must_use]
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
}

/// Verifies one full receipt without network, clock, database, or ambient
/// process state.
///
/// # Errors
///
/// Returns a failure naming the exact canonical, invariant, root-chain, or
/// signature check that failed. No partial verified value is returned.
pub fn verify(
    receipt_bytes: &[u8],
    authorised: &AuthorizedBatch,
) -> Result<VerifiedReceipt, VerificationFailure> {
    let verified = verify_outcome(receipt_bytes, authorised)?;
    if verified
        .receipt
        .protocol()
        .is_some_and(|receipt| receipt.result_code() != 0)
    {
        return Err(VerificationFailure::at(ReceiptCheck::ResultCode));
    }
    Ok(verified)
}

/// Verifies a full sequencer-signed receipt while preserving either its
/// successful or rejected protocol result exactly.
///
/// # Errors
///
/// Returns the exact canonical, invariant, root-chain, or signature check that
/// failed. No partial verified value is returned.
pub fn verify_outcome(
    receipt_bytes: &[u8],
    authorised: &AuthorizedBatch,
) -> Result<VerifiedReceipt, VerificationFailure> {
    let receipt =
        decode(receipt_bytes).map_err(|_| VerificationFailure::at(ReceiptCheck::Decode))?;
    let reproduced =
        encode(&receipt).map_err(|_| VerificationFailure::at(ReceiptCheck::CanonicalEncoding))?;
    if reproduced != receipt_bytes {
        return Err(VerificationFailure::at(ReceiptCheck::CanonicalEncoding));
    }
    let protocol = receipt
        .protocol()
        .ok_or_else(|| VerificationFailure::at(ReceiptCheck::ReceiptShape))?;
    if protocol.protocol_version() != 1 {
        return Err(VerificationFailure::at(ReceiptCheck::ProtocolVersion));
    }
    if protocol.operation() == 0 {
        return Err(VerificationFailure::at(ReceiptCheck::Operation));
    }
    if protocol.activity_id() == [0; 32] {
        return Err(VerificationFailure::at(ReceiptCheck::ActivityId));
    }
    if protocol.batch_id() != authorised.batch_id {
        return Err(VerificationFailure::at(ReceiptCheck::BatchId));
    }
    if protocol.asset() != authorised.asset || protocol.asset() == [0; 32] {
        return Err(VerificationFailure::at(ReceiptCheck::Asset));
    }
    if protocol.previous_state_root() != authorised.previous_state_root {
        return Err(VerificationFailure::at(ReceiptCheck::PreviousStateRoot));
    }
    if protocol.resulting_state_root() != authorised.resulting_state_root {
        return Err(VerificationFailure::at(ReceiptCheck::ResultingStateRoot));
    }
    if protocol.result_code() == 0 {
        if protocol
            .debit_balance_before()
            .checked_sub(protocol.amount())
            != Some(protocol.debit_balance_after())
        {
            return Err(VerificationFailure::at(ReceiptCheck::DebitBalance));
        }
        if protocol
            .credit_balance_before()
            .checked_add(protocol.amount())
            != Some(protocol.credit_balance_after())
        {
            return Err(VerificationFailure::at(ReceiptCheck::CreditBalance));
        }
    }
    let signature = protocol
        .sequencer_signature()
        .ok_or_else(|| VerificationFailure::at(ReceiptCheck::MissingSignature))?;
    let unsigned = encode_unsigned(&receipt)
        .map_err(|_| VerificationFailure::at(ReceiptCheck::CanonicalEncoding))?;
    let digest = receipt_digest(&unsigned)
        .map_err(|_| VerificationFailure::at(ReceiptCheck::CanonicalEncoding))?;
    ed25519::verify_digest(&authorised.sequencer_public_key, &signature, &digest)
        .map_err(|_| VerificationFailure::at(ReceiptCheck::SequencerSignature))?;
    Ok(VerifiedReceipt {
        receipt,
        canonical_bytes: reproduced,
        evidence: Evidence::sequencer(digest),
    })
}
