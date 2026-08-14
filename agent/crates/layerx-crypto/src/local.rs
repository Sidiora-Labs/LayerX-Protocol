//! Zeroizing in-process Ed25519 signer.

use std::fmt;

use ed25519_dalek::{Signer as _, SigningKey};
use zeroize::Zeroizing;

use crate::signer::{AgentSignature, SignFuture, Signer, SigningRequest};

/// In-process signer whose seed is zeroized on drop and never rendered.
pub struct LocalSigner {
    seed: Zeroizing<[u8; 32]>,
    public_key: [u8; 32],
}

impl LocalSigner {
    /// Imports a private Ed25519 seed into zeroizing memory.
    #[must_use]
    pub fn new(seed: [u8; 32]) -> Self {
        let seed = Zeroizing::new(seed);
        let public_key = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        Self { seed, public_key }
    }
}

impl fmt::Debug for LocalSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LocalSigner([redacted key])")
    }
}

impl Signer for LocalSigner {
    fn public_key(&self) -> [u8; 32] {
        self.public_key
    }

    fn sign<'a>(&'a self, request: SigningRequest<'a>) -> SignFuture<'a> {
        Box::pin(async move {
            let signing_key = SigningKey::from_bytes(&self.seed);
            let signature = signing_key.sign(&request.message().digest());
            Ok(AgentSignature::new(signature.to_bytes()))
        })
    }
}
