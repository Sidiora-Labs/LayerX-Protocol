//! Compact low-S secp256k1 verification for Paxeer-facing certificates.

use k256::ecdsa::{signature::hazmat::PrehashVerifier as _, Signature, VerifyingKey};

use crate::{SignatureMessage, VerifyError};

/// Verifies a canonical compact secp256k1 signature over the scoped core digest.
///
/// # Errors
///
/// Returns `BadSignature` for invalid SEC1 keys, zero or non-reduced scalars,
/// high-S malleable signatures, and failed verification equations.
pub fn verify(
    public_key: &[u8],
    signature: &[u8; 64],
    message: SignatureMessage<'_>,
) -> Result<(), VerifyError> {
    verify_digest(public_key, signature, &message.digest())
}

/// Verifies a canonical compact secp256k1 signature over an already
/// domain-separated 32-byte core digest.
///
/// # Errors
///
/// Returns `BadSignature` for invalid SEC1 keys, zero or non-reduced scalars,
/// high-S malleable signatures, and failed verification equations.
pub fn verify_digest(
    public_key: &[u8],
    signature: &[u8; 64],
    digest: &[u8; 32],
) -> Result<(), VerifyError> {
    if public_key.len() != 33 && public_key.len() != 65 {
        return Err(VerifyError::BadSignature);
    }
    let signature = Signature::from_slice(signature).map_err(|_| VerifyError::BadSignature)?;
    if signature.normalize_s().is_some() {
        return Err(VerifyError::BadSignature);
    }
    let verifying_key =
        VerifyingKey::from_sec1_bytes(public_key).map_err(|_| VerifyError::BadSignature)?;
    verifying_key
        .verify_prehash(digest, &signature)
        .map_err(|_| VerifyError::BadSignature)
}
