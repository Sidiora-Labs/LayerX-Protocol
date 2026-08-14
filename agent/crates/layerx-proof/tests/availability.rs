use layerx_proof::availability::{
    verify_chunk, verify_reassembled, AvailabilityCheck, AvailabilityClass, Chunk,
    ReassembledRecords, RootCommitments,
};
use layerx_proof::merkle::{build_leaf_hash_proof, root};
use layerx_wire::hash::availability_chunk_digest;

fn chunks() -> (Vec<Chunk>, Vec<layerx_proof::merkle::Proof>, [u8; 32]) {
    let mut chunks = Vec::new();
    for (index, class) in [
        AvailabilityClass::Activities,
        AvailabilityClass::Receipts,
        AvailabilityClass::Oracle,
        AvailabilityClass::StateDiff,
        AvailabilityClass::Recovery,
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = vec![b'a' + u8::try_from(index).unwrap_or(0)];
        let hash =
            availability_chunk_digest(7, u32::try_from(index).unwrap_or(0), class as u8, 0, &bytes)
                .unwrap_or_else(|error| panic!("chunk hash failed: {error:?}"));
        chunks.push(Chunk {
            batch_number: 7,
            index: u32::try_from(index).unwrap_or(0),
            class,
            class_offset: 0,
            bytes,
            claimed_hash: hash,
        });
    }
    let hashes: Vec<_> = chunks.iter().map(|chunk| chunk.claimed_hash).collect();
    let mut proofs = Vec::new();
    let mut root_value = [0; 32];
    for index in 0..hashes.len() {
        let (proof, computed) = build_leaf_hash_proof(&hashes, index)
            .unwrap_or_else(|error| panic!("proof build failed: {error:?}"));
        proofs.push(proof);
        root_value = computed;
    }
    (chunks, proofs, root_value)
}

fn records<'a>() -> (ReassembledRecords<'a>, RootCommitments) {
    static ACTIVITIES: [&[u8]; 1] = [b"activity"];
    static RECEIPTS: [&[u8]; 1] = [b"receipt"];
    static EVENTS: [&[u8]; 1] = [b"event"];
    static ORACLE: [&[u8]; 1] = [b"oracle"];
    let commitments = RootCommitments {
        activity: root(&ACTIVITIES)
            .unwrap_or_else(|error| panic!("activity root failed: {error:?}")),
        receipt: root(&RECEIPTS).unwrap_or_else(|error| panic!("receipt root failed: {error:?}")),
        event: root(&EVENTS).unwrap_or_else(|error| panic!("event root failed: {error:?}")),
        oracle: root(&ORACLE).unwrap_or_else(|error| panic!("oracle root failed: {error:?}")),
    };
    (
        ReassembledRecords {
            activities: &ACTIVITIES,
            receipts: &RECEIPTS,
            events: &EVENTS,
            oracle_inputs: &ORACLE,
        },
        commitments,
    )
}

#[test]
fn verifies_chunks_all_classes_and_reassembled_roots() {
    let (chunks, proofs, root_value) = chunks();
    let verified: Vec<_> = chunks
        .into_iter()
        .zip(&proofs)
        .map(|(chunk, proof)| {
            verify_chunk(chunk, proof, 7, &root_value)
                .unwrap_or_else(|error| panic!("valid chunk failed: {error:?}"))
        })
        .collect();
    let (records, commitments) = records();
    let report = verify_reassembled(&verified, &records, commitments)
        .unwrap_or_else(|error| panic!("valid reassembly failed: {error:?}"));
    assert_eq!(report.classes.obtained.len(), 5);
    assert!(report.classes.missing.is_empty());
    assert_eq!(report.total_bytes, 5);
}

#[test]
fn retains_evidence_for_altered_and_cross_batch_chunks() {
    let (mut chunks, proofs, root_value) = chunks();
    chunks[0].bytes[0] ^= 1;
    let altered = verify_chunk(chunks[0].clone(), &proofs[0], 7, &root_value)
        .err()
        .unwrap_or_else(|| panic!("altered chunk verified"));
    assert_eq!(altered.check, AvailabilityCheck::ChunkHash);
    assert_eq!(altered.served_bytes, chunks[0].bytes);
    assert_eq!(altered.commitment, chunks[0].claimed_hash);

    chunks[1].batch_number = 8;
    let wrong_batch = verify_chunk(chunks[1].clone(), &proofs[1], 7, &root_value)
        .err()
        .unwrap_or_else(|| panic!("cross-batch chunk verified"));
    assert_eq!(wrong_batch.check, AvailabilityCheck::BatchNumber);
    assert_eq!(wrong_batch.served_bytes, chunks[1].bytes);
}

#[test]
fn rejects_reordered_chunks_withheld_classes_and_root_mismatch() {
    let (chunks, proofs, root_value) = chunks();
    let mut verified: Vec<_> = chunks
        .into_iter()
        .zip(&proofs)
        .map(|(chunk, proof)| {
            verify_chunk(chunk, proof, 7, &root_value)
                .unwrap_or_else(|error| panic!("valid chunk failed: {error:?}"))
        })
        .collect();
    let (records, commitments) = records();

    verified.swap(0, 1);
    let reordered = verify_reassembled(&verified, &records, commitments)
        .err()
        .unwrap_or_else(|| panic!("reordered chunks verified"));
    assert_eq!(reordered.check, AvailabilityCheck::ChunkOrder);
    verified.swap(0, 1);

    let withheld = verify_reassembled(&verified[..4], &records, commitments)
        .err()
        .unwrap_or_else(|| panic!("withheld class verified"));
    assert_eq!(withheld.check, AvailabilityCheck::MissingClass);
    assert_eq!(withheld.classes.missing, vec![AvailabilityClass::Recovery]);

    let mut wrong = commitments;
    wrong.event[0] ^= 1;
    let mismatch = verify_reassembled(&verified, &records, wrong)
        .err()
        .unwrap_or_else(|| panic!("mismatching event root verified"));
    assert_eq!(mismatch.check, AvailabilityCheck::EventRoot);
    assert_eq!(mismatch.commitment, wrong.event);
    assert_eq!(mismatch.served_bytes.len(), 5);
}
