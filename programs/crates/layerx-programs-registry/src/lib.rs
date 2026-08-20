#![forbid(unsafe_code)]

use core::fmt::{self, Display};
use std::collections::BTreeMap;

use layerx_programs_runtime::{DeploymentReceipt, ProgramVersion};
pub use layerx_programs_runtime::{ProgramId, UpgradePolicy};
use sha2::{Digest, Sha256};

const TEXT_LIMIT: usize = 512;
const COMMAND_LIMIT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildEnvironment {
    pub builder_image_digest: [u8; 32],
    pub toolchain_digest: [u8; 32],
    pub dependency_lock_digest: [u8; 32],
    pub source_date_epoch: u64,
    pub command: Vec<String>,
}

impl BuildEnvironment {
    fn validate(&self) -> Result<(), RegistryError> {
        if self.builder_image_digest == [0; 32]
            || self.toolchain_digest == [0; 32]
            || self.dependency_lock_digest == [0; 32]
            || self.source_date_epoch == 0
            || self.command.is_empty()
            || self.command.len() > COMMAND_LIMIT
            || self.command.iter().any(|part| {
                part.is_empty()
                    || part.len() > TEXT_LIMIT
                    || part.bytes().any(|byte| byte.is_ascii_control())
            })
        {
            return Err(RegistryError::InvalidBuildEnvironment);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedSource {
    pub uri: String,
    pub canonical_archive: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReproducibleBuild {
    pub source_uri: String,
    pub source_digest: [u8; 32],
    pub environment: BuildEnvironment,
    pub environment_digest: [u8; 32],
    pub artifact_digest: [u8; 32],
}

impl ReproducibleBuild {
    /// Records the exact source, hermetic build environment and resulting
    /// artifact digest from a completed reproducible build.
    ///
    /// # Errors
    ///
    /// Refuses invalid source locations, empty inputs and unpinned build
    /// environments.
    pub fn from_output(
        source: &PublishedSource,
        environment: BuildEnvironment,
        wasm: &[u8],
    ) -> Result<Self, RegistryError> {
        validate_uri(&source.uri)?;
        if source.canonical_archive.is_empty() || wasm.is_empty() {
            return Err(RegistryError::EmptyArtifact);
        }
        environment.validate()?;
        let environment_digest = environment_hash(&environment);
        Ok(Self {
            source_uri: source.uri.clone(),
            source_digest: sha256(&source.canonical_archive),
            environment,
            environment_digest,
            artifact_digest: sha256(wasm),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceStatus {
    Unpublished,
    Verified {
        source_digest: [u8; 32],
        environment_digest: [u8; 32],
    },
    Mismatch {
        expected: [u8; 32],
        reproduced: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramLifecycle {
    Active,
    Deprecated,
    Tombstoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindDownStateAccess {
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindDownPolicy {
    pub exit_program: [u8; 32],
    pub deadline: u64,
    pub state_access: WindDownStateAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleTransition {
    pub program: ProgramId,
    pub expected: ProgramLifecycle,
    pub target: ProgramLifecycle,
    pub authority: [u8; 32],
    pub effective_sequence: u64,
    pub wind_down: WindDownPolicy,
    pub live_value_accounts: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleReceipt {
    pub program: ProgramId,
    pub prior: ProgramLifecycle,
    pub current: ProgramLifecycle,
    pub authority: [u8; 32],
    pub effective_sequence: u64,
    pub wind_down: WindDownPolicy,
    pub live_value_accounts: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryVersion {
    pub number: u32,
    pub code_hash: [u8; 32],
    pub abi_version: u16,
    pub deployment_receipt_digest: [u8; 32],
    pub source: SourceStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub program: ProgramId,
    pub upgrade_policy: UpgradePolicy,
    pub lifecycle: ProgramLifecycle,
    pub versions: Vec<RegistryVersion>,
    pub lifecycle_history: Vec<LifecycleReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadFreshness {
    pub observed_sequence: u64,
    pub observed_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRegistryRead {
    pub entry: RegistryEntry,
    pub receipt_digest: [u8; 32],
    pub freshness: ReadFreshness,
}

pub trait RegistryReadAuthority {
    /// Verifies the protocol receipt backing the latest registry projection.
    ///
    /// # Errors
    ///
    /// Refuses missing, stale, malformed or mismatched receipt evidence.
    fn verify_registry_read(
        &self,
        program: ProgramId,
        latest: &RegistryVersion,
    ) -> Result<([u8; 32], ReadFreshness), RegistryError>;
}

#[derive(Debug, Default)]
pub struct Registry {
    entries: BTreeMap<ProgramId, RegistryEntry>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one lifecycle-produced deployment or upgrade to contiguous
    /// registry history.
    ///
    /// # Errors
    ///
    /// Refuses mismatched receipts, policies, code hashes or version history.
    pub fn record_deployment(
        &mut self,
        receipt: &DeploymentReceipt,
        version: &ProgramVersion,
        policy: UpgradePolicy,
        receipt_digest: [u8; 32],
    ) -> Result<(), RegistryError> {
        if receipt_digest == [0; 32]
            || receipt.new_code_hash != version.code_hash
            || receipt.version == 0
        {
            return Err(RegistryError::DeploymentMismatch);
        }
        let entry = self
            .entries
            .entry(receipt.program)
            .or_insert_with(|| RegistryEntry {
                program: receipt.program,
                upgrade_policy: policy,
                lifecycle: ProgramLifecycle::Active,
                versions: Vec::new(),
                lifecycle_history: Vec::new(),
            });
        if entry.upgrade_policy != policy
            || usize::try_from(receipt.version).ok() != Some(entry.versions.len() + 1)
            || receipt.old_code_hash != entry.versions.last().map(|prior| prior.code_hash)
        {
            return Err(RegistryError::VersionHistoryMismatch);
        }
        entry.versions.push(RegistryVersion {
            number: receipt.version,
            code_hash: version.code_hash,
            abi_version: version.abi_version,
            deployment_receipt_digest: receipt_digest,
            source: SourceStatus::Unpublished,
        });
        Ok(())
    }

    /// Compares a hermetic rebuild with the registered on-chain code hash and
    /// stores either verified or visibly mismatched status.
    ///
    /// # Errors
    ///
    /// Refuses unknown programs and versions.
    pub fn verify_source(
        &mut self,
        program: ProgramId,
        version: u32,
        build: &ReproducibleBuild,
    ) -> Result<SourceStatus, RegistryError> {
        let target = self.version_mut(program, version)?;
        let status = if build.artifact_digest == target.code_hash {
            SourceStatus::Verified {
                source_digest: build.source_digest,
                environment_digest: build.environment_digest,
            }
        } else {
            SourceStatus::Mismatch {
                expected: target.code_hash,
                reproduced: build.artifact_digest,
            }
        };
        target.source = status;
        Ok(status)
    }

    /// Returns authority, lifecycle and complete version history only after
    /// the latest registry projection's receipt is independently verified.
    ///
    /// # Errors
    ///
    /// Refuses unknown programs or absent, stale and mismatched evidence.
    pub fn read(
        &self,
        program: ProgramId,
        authority: &impl RegistryReadAuthority,
    ) -> Result<VerifiedRegistryRead, RegistryError> {
        let entry = self
            .entries
            .get(&program)
            .ok_or(RegistryError::UnknownProgram)?;
        let latest = entry.versions.last().ok_or(RegistryError::UnknownProgram)?;
        let (receipt_digest, freshness) = authority.verify_registry_read(program, latest)?;
        if receipt_digest == [0; 32]
            || receipt_digest != latest.deployment_receipt_digest
            || freshness.observed_sequence == 0
            || freshness.observed_at == 0
        {
            return Err(RegistryError::UnverifiedRead);
        }
        Ok(VerifiedRegistryRead {
            entry: entry.clone(),
            receipt_digest,
            freshness,
        })
    }

    /// Applies one authority-bearing lifecycle record while retaining the
    /// complete append-only transition history for the wind-down subsystem.
    ///
    /// # Errors
    ///
    /// Refuses unknown programs, stale expected state, missing authority or
    /// malformed transition evidence.
    pub fn transition_lifecycle(
        &mut self,
        request: LifecycleTransition,
    ) -> Result<LifecycleReceipt, RegistryError> {
        if request.authority == [0; 32]
            || request.effective_sequence == 0
            || request.wind_down.exit_program == [0; 32]
            || request.wind_down.deadline == 0
            || request.target == ProgramLifecycle::Active
        {
            return Err(RegistryError::InvalidLifecycleTransition);
        }
        let entry = self
            .entries
            .get_mut(&request.program)
            .ok_or(RegistryError::UnknownProgram)?;
        if entry.lifecycle != request.expected {
            return Err(RegistryError::LifecycleConflict);
        }
        let receipt = LifecycleReceipt {
            program: request.program,
            prior: request.expected,
            current: request.target,
            authority: request.authority,
            effective_sequence: request.effective_sequence,
            wind_down: request.wind_down,
            live_value_accounts: request.live_value_accounts,
        };
        entry.lifecycle = request.target;
        entry.lifecycle_history.push(receipt);
        Ok(receipt)
    }

    /// Returns immutable registry state for the wind-down rules engine.
    ///
    /// # Errors
    ///
    /// Refuses an unknown program.
    pub fn entry_for_wind_down(&self, program: ProgramId) -> Result<&RegistryEntry, RegistryError> {
        self.entries
            .get(&program)
            .ok_or(RegistryError::UnknownProgram)
    }

    fn version_mut(
        &mut self,
        program: ProgramId,
        version: u32,
    ) -> Result<&mut RegistryVersion, RegistryError> {
        let index = version
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(RegistryError::UnknownVersion)?;
        self.entries
            .get_mut(&program)
            .ok_or(RegistryError::UnknownProgram)?
            .versions
            .get_mut(index)
            .ok_or(RegistryError::UnknownVersion)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryError {
    InvalidSourceUri,
    InvalidBuildEnvironment,
    EmptyArtifact,
    UnknownProgram,
    UnknownVersion,
    DeploymentMismatch,
    VersionHistoryMismatch,
    UnverifiedRead,
    InvalidLifecycleTransition,
    LifecycleConflict,
}

impl Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidSourceUri => "published source URI is invalid",
            Self::InvalidBuildEnvironment => "reproducible build environment is invalid",
            Self::EmptyArtifact => "source archive or build artifact is empty",
            Self::UnknownProgram => "program is not registered",
            Self::UnknownVersion => "program version is not registered",
            Self::DeploymentMismatch => "deployment receipt and program version do not match",
            Self::VersionHistoryMismatch => "program version history is not contiguous",
            Self::UnverifiedRead => "registry read lacks matching receipt evidence",
            Self::InvalidLifecycleTransition => "program lifecycle transition is invalid",
            Self::LifecycleConflict => "program lifecycle state changed before transition",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RegistryError {}

fn validate_uri(uri: &str) -> Result<(), RegistryError> {
    if uri.len() > TEXT_LIMIT
        || !(uri.starts_with("https://") || uri.starts_with("ipfs://"))
        || uri
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        Err(RegistryError::InvalidSourceUri)
    } else {
        Ok(())
    }
}

fn environment_hash(environment: &BuildEnvironment) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"LayerX/program-build-environment/v1\0");
    bytes.extend_from_slice(&environment.builder_image_digest);
    bytes.extend_from_slice(&environment.toolchain_digest);
    bytes.extend_from_slice(&environment.dependency_lock_digest);
    bytes.extend_from_slice(&environment.source_date_epoch.to_be_bytes());
    for part in &environment.command {
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part.as_bytes());
    }
    sha256(&bytes)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[must_use]
pub const fn programs_source_verification() -> &'static str {
    "sha256-source-artifact-reproducible-build-v1"
}
