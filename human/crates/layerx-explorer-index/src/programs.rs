//! Public program-registry projection built only from receipt-verified reads.

use layerx_programs::{ProgramLifecycle, SourceStatus, UpgradePolicy, VerifiedRegistryRead};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerProgramVersion {
    pub number: u32,
    pub code_hash: [u8; 32],
    pub abi_version: u16,
    pub source: SourceStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplorerProgram {
    pub identifier: [u8; 32],
    pub upgrade_policy: UpgradePolicy,
    pub lifecycle: ProgramLifecycle,
    pub versions: Vec<ExplorerProgramVersion>,
    pub observed_sequence: u64,
    pub observed_at: u64,
    pub receipt_digest: [u8; 32],
}

impl From<VerifiedRegistryRead> for ExplorerProgram {
    fn from(read: VerifiedRegistryRead) -> Self {
        Self {
            identifier: read.entry.program.bytes(),
            upgrade_policy: read.entry.upgrade_policy,
            lifecycle: read.entry.lifecycle,
            versions: read
                .entry
                .versions
                .into_iter()
                .map(|version| ExplorerProgramVersion {
                    number: version.number,
                    code_hash: version.code_hash,
                    abi_version: version.abi_version,
                    source: version.source,
                })
                .collect(),
            observed_sequence: read.freshness.observed_sequence,
            observed_at: read.freshness.observed_at,
            receipt_digest: read.receipt_digest,
        }
    }
}
