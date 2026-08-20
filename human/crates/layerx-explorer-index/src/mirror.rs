//! Explorer-facing mirror verification with mandatory provenance and freshness.

use layerx_mirror::{
    MirrorVerificationFreshness, MirrorVerifier, MirrorVerifyError, SignedHeaderTrust,
};
use layerx_types::verify::VerificationLevel;

/// Display-safe result. A mirror fact cannot be returned without naming the
/// signed header and the exact mirrored head from which it was established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirrorReceiptReport {
    pub verification_level: VerificationLevel,
    pub receipt_digest: [u8; 32],
    pub batch_number: u64,
    pub signed_header_digest: [u8; 32],
    pub freshness: MirrorVerificationFreshness,
}

/// Verifies a receipt with the LayerX service and explorer index absent.
/// `trust` is deployment configuration, not archive-provided data.
pub fn verify_mirrored_receipt(
    archive_bytes: &[u8],
    canonical_receipt: &[u8],
    trust: SignedHeaderTrust,
    freshness: MirrorVerificationFreshness,
) -> Result<MirrorReceiptReport, MirrorVerifyError> {
    let verified =
        MirrorVerifier::new(archive_bytes, trust, freshness)?.receipt(canonical_receipt)?;
    let receipt_digest = verified
        .value
        .evidence()
        .receipt_digest()
        .ok_or(MirrorVerifyError::ReceiptDecode)?;
    Ok(MirrorReceiptReport {
        verification_level: verified.value.level(),
        receipt_digest,
        batch_number: verified.batch_number,
        signed_header_digest: verified.signed_header_digest,
        freshness: verified.freshness,
    })
}
