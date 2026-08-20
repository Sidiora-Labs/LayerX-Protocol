//! Data-availability chunk, class, and committed-root verification.

use std::collections::BTreeSet;

use layerx_wire::hash::availability_chunk_digest;

use crate::merkle::{root, verify_leaf_hash, MerkleError, Proof};

/// The five and only five protocol availability classes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AvailabilityClass {
    /// Canonical accepted activities.
    Activities = 1,
    /// Canonical activity receipts.
    Receipts = 2,
    /// Canonical signed oracle inputs.
    Oracle = 3,
    /// Canonical state-diff material.
    StateDiff = 4,
    /// Canonical recovery metadata.
    Recovery = 5,
}

impl AvailabilityClass {
    const ALL: [Self; 5] = [
        Self::Activities,
        Self::Receipts,
        Self::Oracle,
        Self::StateDiff,
        Self::Recovery,
    ];
}

/// One exact chunk response and its claimed digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    /// Batch that produced the chunk.
    pub batch_number: u64,
    /// Flattened bundle index committed by the Merkle proof.
    pub index: u32,
    /// Availability section containing the bytes.
    pub class: AvailabilityClass,
    /// Byte offset inside the section.
    pub class_offset: u64,
    /// Exact served bytes.
    pub bytes: Vec<u8>,
    /// Digest claimed by the provider.
    pub claimed_hash: [u8; 32],
}

/// A chunk whose metadata digest and inclusion path both passed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedChunk {
    chunk: Chunk,
    data_availability_root: [u8; 32],
}

impl VerifiedChunk {
    /// Borrows the exact verified provider response.
    #[must_use]
    pub const fn chunk(&self) -> &Chunk {
        &self.chunk
    }

    /// Returns the availability root against which this exact chunk passed.
    #[must_use]
    pub const fn data_availability_root(&self) -> [u8; 32] {
        self.data_availability_root
    }
}

/// Exact availability verification stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AvailabilityCheck {
    /// Chunk batch differs from the requested batch.
    BatchNumber,
    /// Chunk and proof indices differ.
    ChunkIndex,
    /// Provider's claimed digest differs from recomputation.
    ChunkHash,
    /// Inclusion under the availability root failed.
    ChunkInclusion,
    /// Verified chunks were not in strict flattened-index order.
    ChunkOrder,
    /// Section offsets were not contiguous.
    ClassOffset,
    /// At least one required class was withheld.
    MissingClass,
    /// Reassembled activity bytes did not match their root.
    ActivityRoot,
    /// Reassembled receipt bytes did not match their root.
    ReceiptRoot,
    /// Reassembled event bytes did not match their root.
    EventRoot,
    /// Reassembled oracle bytes did not match their root.
    OracleRoot,
}

/// Availability failure evidence retained for audit and provider attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailabilityFailure {
    /// Exact failed stage.
    pub check: AvailabilityCheck,
    /// Exact bytes served by the failing provider.
    pub served_bytes: Vec<u8>,
    /// Commitment the served bytes failed against.
    pub commitment: [u8; 32],
    /// Classes obtained before the failure was established.
    pub classes: ClassReport,
}

/// Explicit obtained and missing class sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassReport {
    /// Classes present in the verified response.
    pub obtained: Vec<AvailabilityClass>,
    /// Classes absent from the verified response.
    pub missing: Vec<AvailabilityClass>,
}

fn class_report(chunks: &[VerifiedChunk]) -> ClassReport {
    let obtained_set: BTreeSet<_> = chunks.iter().map(|chunk| chunk.chunk.class).collect();
    let obtained = AvailabilityClass::ALL
        .into_iter()
        .filter(|class| obtained_set.contains(class))
        .collect();
    let missing = AvailabilityClass::ALL
        .into_iter()
        .filter(|class| !obtained_set.contains(class))
        .collect();
    ClassReport { obtained, missing }
}

/// The canonical record streams decoded from the reassembled sections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReassembledRecords<'a> {
    /// Canonical activity records, in global-sequence order.
    pub activities: &'a [&'a [u8]],
    /// Canonical receipt records, in global-sequence order.
    pub receipts: &'a [&'a [u8]],
    /// Canonical event records recovered from the receipt/event material.
    pub events: &'a [&'a [u8]],
    /// Canonical signed oracle inputs, in committed order.
    pub oracle_inputs: &'a [&'a [u8]],
}

/// Four signed batch commitments checked after section reassembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootCommitments {
    /// Header activity root.
    pub activity: [u8; 32],
    /// Header receipt root.
    pub receipt: [u8; 32],
    /// Header event root.
    pub event: [u8; 32],
    /// Header oracle root.
    pub oracle: [u8; 32],
}

/// Successful five-class reassembly and root-verification report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReassemblyReport {
    /// Explicitly complete availability-class report.
    pub classes: ClassReport,
    /// Total exact provider bytes retained across all chunks.
    pub total_bytes: usize,
}

fn no_classes() -> ClassReport {
    ClassReport {
        obtained: Vec::new(),
        missing: AvailabilityClass::ALL.to_vec(),
    }
}

/// Recomputes the chunk digest and verifies its index-aware Merkle path.
///
/// # Errors
///
/// Returns evidence retaining the exact served bytes and failed commitment.
pub fn verify_chunk(
    chunk: Chunk,
    proof: &Proof,
    expected_batch_number: u64,
    data_availability_root: &[u8; 32],
) -> Result<VerifiedChunk, AvailabilityFailure> {
    if chunk.batch_number != expected_batch_number {
        return Err(AvailabilityFailure {
            check: AvailabilityCheck::BatchNumber,
            served_bytes: chunk.bytes,
            commitment: *data_availability_root,
            classes: no_classes(),
        });
    }
    if chunk.index != proof.leaf_index() {
        return Err(AvailabilityFailure {
            check: AvailabilityCheck::ChunkIndex,
            served_bytes: chunk.bytes,
            commitment: *data_availability_root,
            classes: no_classes(),
        });
    }
    let computed = availability_chunk_digest(
        chunk.batch_number,
        chunk.index,
        chunk.class as u8,
        chunk.class_offset,
        &chunk.bytes,
    )
    .map_err(|_| AvailabilityFailure {
        check: AvailabilityCheck::ChunkHash,
        served_bytes: chunk.bytes.clone(),
        commitment: chunk.claimed_hash,
        classes: no_classes(),
    })?;
    if computed != chunk.claimed_hash {
        return Err(AvailabilityFailure {
            check: AvailabilityCheck::ChunkHash,
            served_bytes: chunk.bytes,
            commitment: chunk.claimed_hash,
            classes: no_classes(),
        });
    }
    verify_leaf_hash(&computed, proof, data_availability_root).map_err(|_error: MerkleError| {
        AvailabilityFailure {
            check: AvailabilityCheck::ChunkInclusion,
            served_bytes: chunk.bytes.clone(),
            commitment: *data_availability_root,
            classes: no_classes(),
        }
    })?;
    Ok(VerifiedChunk {
        chunk,
        data_availability_root: *data_availability_root,
    })
}

fn all_served_bytes(chunks: &[VerifiedChunk]) -> Vec<u8> {
    let capacity = chunks.iter().fold(0_usize, |total, chunk| {
        total.saturating_add(chunk.chunk.bytes.len())
    });
    let mut bytes = Vec::with_capacity(capacity);
    for chunk in chunks {
        bytes.extend_from_slice(&chunk.chunk.bytes);
    }
    bytes
}

fn compare_root(
    records: &[&[u8]],
    expected: [u8; 32],
    check: AvailabilityCheck,
    chunks: &[VerifiedChunk],
    classes: &ClassReport,
) -> Result<(), AvailabilityFailure> {
    let computed = root(records).map_err(|_| AvailabilityFailure {
        check,
        served_bytes: all_served_bytes(chunks),
        commitment: expected,
        classes: classes.clone(),
    })?;
    if computed == expected {
        Ok(())
    } else {
        Err(AvailabilityFailure {
            check,
            served_bytes: all_served_bytes(chunks),
            commitment: expected,
            classes: classes.clone(),
        })
    }
}

/// Verifies ordering, section contiguity, class completeness, and all four
/// signed batch record roots after reassembly.
///
/// # Errors
///
/// Returns retained provider bytes, the mismatching commitment, and the class
/// report at the exact point of failure.
pub fn verify_reassembled(
    chunks: &[VerifiedChunk],
    records: &ReassembledRecords<'_>,
    commitments: RootCommitments,
) -> Result<ReassemblyReport, AvailabilityFailure> {
    let classes = class_report(chunks);
    let served_bytes = all_served_bytes(chunks);
    for pair in chunks.windows(2) {
        if pair[0].chunk.index >= pair[1].chunk.index
            || pair[0].chunk.batch_number != pair[1].chunk.batch_number
            || pair[0].data_availability_root != pair[1].data_availability_root
        {
            return Err(AvailabilityFailure {
                check: AvailabilityCheck::ChunkOrder,
                served_bytes,
                commitment: [0; 32],
                classes,
            });
        }
    }
    for class in AvailabilityClass::ALL {
        let mut expected_offset = 0_u64;
        for chunk in chunks.iter().filter(|chunk| chunk.chunk.class == class) {
            if chunk.chunk.class_offset != expected_offset {
                return Err(AvailabilityFailure {
                    check: AvailabilityCheck::ClassOffset,
                    served_bytes,
                    commitment: [0; 32],
                    classes,
                });
            }
            expected_offset = expected_offset
                .saturating_add(u64::try_from(chunk.chunk.bytes.len()).unwrap_or(u64::MAX));
        }
    }
    if !classes.missing.is_empty() {
        return Err(AvailabilityFailure {
            check: AvailabilityCheck::MissingClass,
            served_bytes,
            commitment: [0; 32],
            classes,
        });
    }
    compare_root(
        records.activities,
        commitments.activity,
        AvailabilityCheck::ActivityRoot,
        chunks,
        &classes,
    )?;
    compare_root(
        records.receipts,
        commitments.receipt,
        AvailabilityCheck::ReceiptRoot,
        chunks,
        &classes,
    )?;
    compare_root(
        records.events,
        commitments.event,
        AvailabilityCheck::EventRoot,
        chunks,
        &classes,
    )?;
    compare_root(
        records.oracle_inputs,
        commitments.oracle,
        AvailabilityCheck::OracleRoot,
        chunks,
        &classes,
    )?;
    Ok(ReassemblyReport {
        classes,
        total_bytes: served_bytes.len(),
    })
}
