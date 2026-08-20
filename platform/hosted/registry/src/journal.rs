//! Durable canonical deployment journal on local disk.
//!
//! Every entry is named by the digest of its own canonical encoding, so a
//! record that has been altered in place can no longer be found under the
//! digest a registry projection refers to.

use std::fs;
use std::path::PathBuf;

use layerx_programs::{hex, DeploymentJournal, DeploymentRecord, ObservedHead, RegistryError};

use crate::write_atomic;

const RECORD_SUFFIX: &str = "deployment";
const HEAD_FILE: &str = "head";

/// Journal of accepted deployments and upgrades observed at the node boundary.
#[derive(Clone, Debug)]
pub struct FileDeploymentJournal {
    root: PathBuf,
}

impl FileDeploymentJournal {
    /// Opens, creating the journal directory when it is absent.
    ///
    /// # Errors
    ///
    /// Returns the filesystem error that prevented the directory from opening.
    pub fn open(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root)
            .map_err(|error| format!("could not open the deployment journal: {error}"))?;
        Ok(Self { root })
    }

    /// Reads every canonical record, refusing entries whose contents no longer
    /// hash to the digest they are filed under.
    ///
    /// # Errors
    ///
    /// Returns unreadable directories, undecodable records and misfiled
    /// records.
    pub fn records(&self) -> Result<Vec<DeploymentRecord>, String> {
        let mut paths = Vec::new();
        let entries = fs::read_dir(&self.root)
            .map_err(|error| format!("could not read the deployment journal: {error}"))?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("could not read the deployment journal: {error}"))?
                .path();
            if path.extension().is_some_and(|value| value == RECORD_SUFFIX) {
                paths.push(path);
            }
        }
        paths.sort();
        let mut records = Vec::with_capacity(paths.len());
        for path in paths {
            let bytes = fs::read(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let record = DeploymentRecord::decode(&bytes)
                .map_err(|error| format!("{} is corrupt: {error}", path.display()))?;
            record
                .validate()
                .map_err(|error| format!("{} is corrupt: {error}", path.display()))?;
            let named = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if named != hex::encode(&record.digest()) {
                return Err(format!(
                    "{} is filed under a digest it does not hash to",
                    path.display()
                ));
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Appends one accepted deployment and returns the digest it is filed
    /// under.
    ///
    /// # Errors
    ///
    /// Refuses records that fail their own validation and returns the
    /// filesystem error that prevented durable persistence.
    pub fn append(&self, record: &DeploymentRecord) -> Result<[u8; 32], String> {
        record
            .validate()
            .map_err(|error| format!("deployment record is not admissible: {error}"))?;
        let digest = record.digest();
        let path = self
            .root
            .join(format!("{}.{RECORD_SUFFIX}", hex::encode(&digest)));
        write_atomic(&path, &record.canonical_encoding())
            .map_err(|error| format!("could not persist {}: {error}", path.display()))?;
        Ok(digest)
    }

    /// Removes one record that the registry projection refused, so the journal
    /// and the projection never disagree.
    ///
    /// # Errors
    ///
    /// Returns the filesystem error that prevented removal.
    pub fn discard(&self, digest: [u8; 32]) -> Result<(), String> {
        let path = self
            .root
            .join(format!("{}.{RECORD_SUFFIX}", hex::encode(&digest)));
        fs::remove_file(&path)
            .map_err(|error| format!("could not discard {}: {error}", path.display()))
    }

    /// Records the protocol head last observed by the node boundary.
    ///
    /// # Errors
    ///
    /// Refuses absent observations and returns the filesystem error that
    /// prevented durable persistence.
    pub fn refresh_head(&self, head: ObservedHead) -> Result<(), String> {
        if head.sequence == 0 || head.observed_at == 0 {
            return Err("an observed head needs a sequence and an observation time".to_owned());
        }
        let path = self.root.join(HEAD_FILE);
        write_atomic(
            &path,
            format!("{}\t{}\n", head.sequence, head.observed_at).as_bytes(),
        )
        .map_err(|error| format!("could not persist {}: {error}", path.display()))
    }
}

impl DeploymentJournal for FileDeploymentJournal {
    fn canonical_record(&self, receipt_digest: [u8; 32]) -> Result<Vec<u8>, RegistryError> {
        let path = self
            .root
            .join(format!("{}.{RECORD_SUFFIX}", hex::encode(&receipt_digest)));
        fs::read(path).map_err(|_| RegistryError::JournalUnavailable)
    }

    fn observed_head(&self) -> Result<ObservedHead, RegistryError> {
        let text = fs::read_to_string(self.root.join(HEAD_FILE))
            .map_err(|_| RegistryError::JournalUnavailable)?;
        let (sequence, observed_at) = text
            .trim()
            .split_once('\t')
            .ok_or(RegistryError::JournalUnavailable)?;
        Ok(ObservedHead {
            sequence: sequence
                .parse()
                .map_err(|_| RegistryError::JournalUnavailable)?,
            observed_at: observed_at
                .parse()
                .map_err(|_| RegistryError::JournalUnavailable)?,
        })
    }
}
