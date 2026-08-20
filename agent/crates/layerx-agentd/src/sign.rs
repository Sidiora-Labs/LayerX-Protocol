//! External-by-default signing and bounded protocol session-key self-signing.

use std::collections::BTreeSet;

use layerx_crypto::local::LocalSigner;
use layerx_crypto::session::IssuedSessionKey;
use layerx_crypto::signer::{sign_disclosed, SignError, Signer};
use layerx_types::activity::{ActivityBuildError, Authority};
use layerx_types::payload::{ActivityType, ModuleRegistry};
use layerx_wire::WireError;

use crate::prepare::{
    verify_disclosure_binding, DisclosureBindingError, DisclosureDigest, Prepared,
};

#[path = "sign_verify.rs"]
mod verification;

pub use verification::{SubmissionBindingAudit, VerifiedSubmission};

/// Signing location made explicit in every response and audit record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SigningMode {
    External,
    ProtocolSessionKey,
}

/// Key-free package returned for signing outside the daemon.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalSigningPackage {
    pub canonical_bytes: Vec<u8>,
    pub signing_preimage: [u8; 32],
    pub disclosure_digest: DisclosureDigest,
    pub mode: SigningMode,
}

/// Distinct audit evidence for one self-signed preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfSigningAudit {
    pub mode: SigningMode,
    pub session_public_key: [u8; 32],
    pub activity_type: ActivityType,
    pub disclosure_digest: DisclosureDigest,
    pub authority_bytes: Vec<u8>,
    pub expires_at: u64,
    pub revocation_sequence: u64,
}

/// Output from a bounded in-daemon protocol session key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedPreparation {
    pub canonical_bytes: Vec<u8>,
    pub signed_canonical_bytes: Vec<u8>,
    pub signature: [u8; 64],
    pub signer_public_key: [u8; 32],
    pub mode: SigningMode,
    pub audit: SelfSigningAudit,
}

/// Provisioned signer whose scope is the exact issued protocol authority.
pub struct ProvisionedSessionKey {
    signer: LocalSigner,
    authority: Authority,
    permitted_activity_types: BTreeSet<ActivityType>,
    expires_at: u64,
    revocation_sequence: u64,
}

impl std::fmt::Debug for ProvisionedSessionKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProvisionedSessionKey")
            .field("signer", &"[redacted]")
            .field("expires_at", &self.expires_at)
            .field("revocation_sequence", &self.revocation_sequence)
            .finish_non_exhaustive()
    }
}

impl ProvisionedSessionKey {
    /// Imports key material only when it matches an issued protocol session authority.
    ///
    /// # Errors
    ///
    /// Returns `KeyMismatch` when the seed derives a different public key, and
    /// `InvalidProvisioning` for an empty permitted activity set or a zero expiry or revocation
    /// sequence.
    pub fn new(seed: [u8; 32], issued: IssuedSessionKey) -> Result<Self, SigningError> {
        let signer = LocalSigner::new(seed);
        if signer.public_key() != issued.session_public_key {
            return Err(SigningError::KeyMismatch);
        }
        if issued.permitted_activity_types.is_empty()
            || issued.expires_at == 0
            || issued.revocation_sequence == 0
        {
            return Err(SigningError::InvalidProvisioning);
        }
        Ok(Self {
            signer,
            authority: issued.authority,
            permitted_activity_types: issued.permitted_activity_types.into_iter().collect(),
            expires_at: issued.expires_at,
            revocation_sequence: issued.revocation_sequence,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SigningError {
    Disclosure(DisclosureBindingError),
    NotProvisioned,
    InvalidProvisioning,
    KeyMismatch,
    ScopeDenied,
    Expired,
    Revoked,
    AuthorityMismatch,
    Crypto(SignError),
    SignatureEncoding(ActivityBuildError),
    Wire(WireError),
    PreparedBytesChanged,
    SignatureInvalid,
}

/// Returns the default key-free package for an external signer.
///
/// # Errors
///
/// Returns the disclosure binding failure when the preparation no longer re-encodes to its exact
/// canonical bytes and digest.
pub fn external(prepared: &Prepared) -> Result<ExternalSigningPackage, SigningError> {
    verify_disclosure_binding(prepared).map_err(SigningError::Disclosure)?;
    Ok(ExternalSigningPackage {
        canonical_bytes: prepared.canonical_bytes.clone(),
        signing_preimage: prepared.signing_preimage,
        disclosure_digest: prepared.disclosure_digest,
        mode: SigningMode::External,
    })
}

/// Signs only through an explicitly provisioned, unexpired protocol session key.
///
/// # Errors
///
/// Refuses an absent, expired, or revoked session key, an activity type outside its permitted set,
/// and an authority other than the issued one. Also returns the disclosure, signing, encoding, and
/// wire failures raised while producing the signed bytes.
pub async fn self_sign(
    provisioned: Option<&ProvisionedSessionKey>,
    prepared: &Prepared,
    registry: &ModuleRegistry,
    protocol_time: u64,
    current_revocation_sequence: u64,
) -> Result<SignedPreparation, SigningError> {
    let provisioned = provisioned.ok_or(SigningError::NotProvisioned)?;
    verify_disclosure_binding(prepared).map_err(SigningError::Disclosure)?;
    if protocol_time >= provisioned.expires_at {
        return Err(SigningError::Expired);
    }
    if current_revocation_sequence != provisioned.revocation_sequence {
        return Err(SigningError::Revoked);
    }
    let activity_type = prepared.envelope.activity_type();
    if !provisioned
        .permitted_activity_types
        .contains(&activity_type)
    {
        return Err(SigningError::ScopeDenied);
    }
    if prepared.envelope.authority().as_bytes() != provisioned.authority.as_bytes() {
        return Err(SigningError::AuthorityMismatch);
    }
    let signature = sign_disclosed(
        &provisioned.signer,
        &prepared.canonical_bytes,
        &prepared.disclosure,
        registry,
    )
    .await
    .map_err(SigningError::Crypto)?;
    let public_key = provisioned.signer.public_key();
    let authority_bytes = provisioned.authority.as_bytes().to_vec();
    let signature_bytes = *signature.as_bytes();
    let signed_canonical_bytes = verification::attach_signature(prepared, signature_bytes)?;
    Ok(SignedPreparation {
        canonical_bytes: prepared.canonical_bytes.clone(),
        signed_canonical_bytes,
        signature: signature_bytes,
        signer_public_key: public_key,
        mode: SigningMode::ProtocolSessionKey,
        audit: SelfSigningAudit {
            mode: SigningMode::ProtocolSessionKey,
            session_public_key: public_key,
            activity_type,
            disclosure_digest: prepared.disclosure_digest,
            authority_bytes,
            expires_at: provisioned.expires_at,
            revocation_sequence: provisioned.revocation_sequence,
        },
    })
}

/// Attaches a returned external signature through the canonical wire encoder.
///
/// # Errors
///
/// Returns the disclosure binding failure, a signature the envelope rejects, or the wire failure
/// raised while encoding the signed envelope.
pub fn attach_external_signature(
    prepared: &Prepared,
    signature: [u8; 64],
) -> Result<Vec<u8>, SigningError> {
    verify_disclosure_binding(prepared).map_err(SigningError::Disclosure)?;
    verification::attach_signature(prepared, signature)
}

/// Verifies the exact signed bytes and returns the only submit-capable wrapper.
///
/// # Errors
///
/// Refuses bytes that do not decode, re-encode, or reduce to the exact prepared canonical form,
/// and an absent, malformed, or invalid signature. Also returns the disclosure binding failure.
pub fn verify_before_submit(
    signed_canonical_bytes: &[u8],
    prepared: &Prepared,
    signer_public_key: &[u8; 32],
    registry: &ModuleRegistry,
) -> Result<VerifiedSubmission, SigningError> {
    verification::verify_exact(
        signed_canonical_bytes,
        prepared,
        signer_public_key,
        registry,
    )
}
