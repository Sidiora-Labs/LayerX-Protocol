//! Solana addresses, program-derived addresses and the namespaced-storage key
//! a ported account occupies.
//!
//! Solana has one flat 32-byte address space, so a program that needs many
//! accounts derives their addresses by hashing seeds. `LayerX` gives each
//! program a byte-keyed map inside a namespace already fixed to
//! `(program, principal)`, so the seeds themselves can be the key and the hash
//! is unnecessary. Both views are kept here: the exact Solana derivation, so a
//! snapshot of live accounts can be located, and the collapsed key the ported
//! program actually uses.

use layerx_programs_runtime::storage::MAX_STORAGE_KEY_BYTES;

use crate::error::PortRefusal;
use crate::hash::sha256;

/// Byte width of a Solana public key.
pub const PUBKEY_BYTES: usize = 32;
/// Maximum byte length of one seed, as the runtime enforces it.
pub const MAX_SEED_BYTES: usize = 32;
/// Maximum number of seeds in one derivation.
pub const MAX_SEEDS: usize = 16;
/// The domain marker appended to every program-derived address preimage.
pub const PDA_MARKER: &[u8] = b"ProgramDerivedAddress";

/// One Solana public key.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Pubkey([u8; PUBKEY_BYTES]);

impl Pubkey {
    /// Constructs a public key that is not the all-zero key.
    ///
    /// # Errors
    ///
    /// Refuses the all-zero key, which is the System Program's own identifier
    /// and never an account a ported program may credit or address.
    pub fn new(bytes: [u8; PUBKEY_BYTES]) -> Result<Self, PortRefusal> {
        if bytes == [0u8; PUBKEY_BYTES] {
            return Err(PortRefusal::ZeroPubkey);
        }
        Ok(Self(bytes))
    }

    /// Returns the raw key bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; PUBKEY_BYTES] {
        self.0
    }
}

/// The seeds one program-derived address is built from, in derivation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedPath {
    seeds: Vec<Vec<u8>>,
}

impl SeedPath {
    /// Collects a seed path.
    ///
    /// # Errors
    ///
    /// Refuses an empty path, more seeds than the runtime admits and any seed
    /// longer than the runtime's seed bound.
    pub fn new(seeds: Vec<Vec<u8>>) -> Result<Self, PortRefusal> {
        if seeds.is_empty()
            || seeds.len() > MAX_SEEDS
            || seeds.iter().any(|seed| seed.len() > MAX_SEED_BYTES)
        {
            return Err(PortRefusal::InvalidSeeds);
        }
        Ok(Self { seeds })
    }

    /// Borrows the seeds in derivation order.
    #[must_use]
    pub fn seeds(&self) -> &[Vec<u8>] {
        &self.seeds
    }

    /// Returns the address Solana derives from these seeds, the published bump
    /// and the owning program.
    ///
    /// This is the exact `create_program_address` preimage:
    /// `seeds . bump . program . "ProgramDerivedAddress"`. Solana additionally
    /// requires the result to lie off the `ed25519` curve, which is a property
    /// of the address the chain already published rather than something a port
    /// re-decides; [`Self::verify`] therefore checks the derivation against the
    /// published address, which is the check that can actually be wrong.
    #[must_use]
    pub fn address(&self, bump: u8, program: Pubkey) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(PDA_MARKER.len() + PUBKEY_BYTES + 64);
        for seed in &self.seeds {
            preimage.extend_from_slice(seed);
        }
        preimage.push(bump);
        preimage.extend_from_slice(&program.bytes());
        preimage.extend_from_slice(PDA_MARKER);
        sha256(&preimage)
    }

    /// Checks that the published bump really derives the published address.
    ///
    /// # Errors
    ///
    /// Refuses a bump that derives a different address.
    pub fn verify(
        &self,
        bump: u8,
        program: Pubkey,
        published: [u8; 32],
    ) -> Result<(), PortRefusal> {
        if self.address(bump, program) == published {
            Ok(())
        } else {
            Err(PortRefusal::DerivationMismatch)
        }
    }

    /// Returns the namespaced-storage key for the whole seed path, framed so
    /// that no two distinct paths can collide: each seed is written as its
    /// one-byte length followed by its bytes.
    ///
    /// # Errors
    ///
    /// Refuses a framed key beyond the storage key bound.
    pub fn storage_key(&self) -> Result<Vec<u8>, PortRefusal> {
        let mut key = Vec::with_capacity(self.seeds.len().saturating_mul(2));
        for seed in &self.seeds {
            let length = u8::try_from(seed.len()).map_err(|_| PortRefusal::InvalidSeeds)?;
            key.push(length);
            key.extend_from_slice(seed);
        }
        if key.len() > MAX_STORAGE_KEY_BYTES {
            return Err(PortRefusal::InvalidSeeds);
        }
        Ok(key)
    }

    /// Returns the seed path with the envelope-supplied seeds removed.
    ///
    /// A seed that carries the signer's public key or the program's own
    /// identifier tells the port nothing the runtime has not already fixed: the
    /// namespace is `(program, principal)` before guest code runs. Dropping
    /// those seeds is what collapses a per-user account onto a single key.
    ///
    /// # Errors
    ///
    /// Refuses an index outside the path and a path with nothing left.
    pub fn collapse(&self, envelope: &[usize]) -> Result<Self, PortRefusal> {
        if envelope.iter().any(|index| *index >= self.seeds.len()) {
            return Err(PortRefusal::InvalidSeeds);
        }
        let kept: Vec<Vec<u8>> = self
            .seeds
            .iter()
            .enumerate()
            .filter(|(index, _)| !envelope.contains(index))
            .map(|(_, seed)| seed.clone())
            .collect();
        Self::new(kept)
    }

    /// Returns the path with one seed replaced, which is how a per-signer
    /// derivation is rebuilt for each holder in a migration plan.
    ///
    /// # Errors
    ///
    /// Refuses an index outside the path and an oversized seed.
    pub fn with_seed(&self, index: usize, seed: &[u8]) -> Result<Self, PortRefusal> {
        if index >= self.seeds.len() {
            return Err(PortRefusal::InvalidSeeds);
        }
        let mut seeds = self.seeds.clone();
        let slot = seeds.get_mut(index).ok_or(PortRefusal::InvalidSeeds)?;
        *slot = seed.to_vec();
        Self::new(seeds)
    }
}

/// One holder of a per-signer account in a migration plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountHolder {
    /// The signer whose public key the derivation carries as a seed.
    pub signer: Pubkey,
    /// The published bump the account's address was derived with.
    pub bump: u8,
    /// The address the account occupies on Solana.
    pub address: [u8; 32],
    /// The principal whose namespace holds the ported account.
    pub principal: [u8; 32],
}

/// One live account located on Solana and the namespaced cell it becomes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationCell {
    /// The program-derived address the account occupies on Solana.
    pub address: [u8; 32],
    /// The key the ported account occupies in `LayerX` namespaced storage.
    pub layerx_key: Vec<u8>,
    /// The principal whose namespace holds the ported account.
    pub principal: [u8; 32],
}

/// Builds the import plan for a per-signer account: one cell per holder,
/// naming the address to read from a Solana snapshot and the collapsed key to
/// write, in that holder's own namespace.
///
/// Every holder's account collapses onto the *same* key, because the seed that
/// distinguished them is the signer's public key and the namespace already
/// carries the principal. The cells do not collide: each one is written in a
/// different namespace.
///
/// # Errors
///
/// Refuses a signer index outside the path, a bump that does not derive the
/// published address, the reserved zero principal, and any seed path or key
/// the declared bounds reject.
pub fn per_signer_import(
    base: &SeedPath,
    signer_index: usize,
    program: Pubkey,
    envelope: &[usize],
    holders: &[AccountHolder],
) -> Result<Vec<MigrationCell>, PortRefusal> {
    let layerx_key = base.collapse(envelope)?.storage_key()?;
    let mut plan = Vec::with_capacity(holders.len());
    for holder in holders {
        if holder.principal == [0u8; 32] {
            return Err(PortRefusal::ZeroPubkey);
        }
        let derived = base.with_seed(signer_index, &holder.signer.bytes())?;
        derived.verify(holder.bump, program, holder.address)?;
        plan.push(MigrationCell {
            address: holder.address,
            layerx_key: layerx_key.clone(),
            principal: holder.principal,
        });
    }
    Ok(plan)
}
