//! Key custody and disclosure-bound `LayerX` signing.

pub mod ct;
pub mod ed25519;
pub mod secp256k1;

use layerx_types::result::{KnownResult, ResultCode};
use layerx_wire::hash::Domain;
use sha2::{Digest as _, Sha256};

/// Canonical bytes coupled to the protocol and network they are valid on.
#[derive(Clone, Copy)]
pub struct SignatureMessage<'a> {
    domain: Domain,
    protocol_version: u16,
    network_id: u32,
    canonical: &'a [u8],
}

impl<'a> SignatureMessage<'a> {
    /// Binds canonical bytes to one supported protocol version and non-zero
    /// network identifier.
    ///
    /// # Errors
    ///
    /// Rejects unsupported versions and the reserved zero network.
    pub const fn new(
        domain: Domain,
        protocol_version: u16,
        network_id: u32,
        canonical: &'a [u8],
    ) -> Result<Self, VerifyError> {
        if protocol_version != 1 {
            return Err(VerifyError::VersionUnsupported);
        }
        if network_id == 0 {
            return Err(VerifyError::WrongNetwork);
        }
        Ok(Self {
            domain,
            protocol_version,
            network_id,
            canonical,
        })
    }

    /// Computes the exact core-compatible scoped domain digest.
    #[must_use]
    pub fn digest(self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.domain.tag());
        hasher.update(self.protocol_version.to_be_bytes());
        hasher.update(self.network_id.to_be_bytes());
        hasher.update(self.canonical);
        hasher.finalize().into()
    }
}

impl std::fmt::Debug for SignatureMessage<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignatureMessage")
            .field("domain", &self.domain)
            .field("protocol_version", &self.protocol_version)
            .field("network_id", &self.network_id)
            .field("canonical_length", &self.canonical.len())
            .finish()
    }
}

/// Exact protocol classification for signature verification failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyError {
    /// The protocol version is not supported by this build.
    VersionUnsupported,
    /// Network zero is reserved and never a valid signing scope.
    WrongNetwork,
    /// The key, signature encoding, scalar form, or verification equation failed.
    BadSignature,
}

impl VerifyError {
    /// Returns the core result code corresponding to this failure.
    #[must_use]
    pub const fn result_code(self) -> ResultCode {
        match self {
            Self::VersionUnsupported => ResultCode::from_raw(KnownResult::VersionUnsupported.raw()),
            Self::WrongNetwork => ResultCode::from_raw(KnownResult::WrongNetwork.raw()),
            Self::BadSignature => ResultCode::from_raw(KnownResult::BadSignature.raw()),
        }
    }
}
