use std::fs;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ed25519_dalek::{Signer as _, SigningKey};
use layerx_agentd::read::balance;
use layerx_client::head::Head;
use layerx_client::lni::framing::{read_frame, write_frame};
use layerx_client::lni::schema::{decode_envelope, encode_envelope, Envelope, Version};
use layerx_client::lni::transport::{ConnectionGate, Limits, Uds};
use layerx_client::read::{ReadContext, ReadError, Requested};
use layerx_proof::inclusion::SequencerAuthorization;
use layerx_proof::merkle::{build_proof, Proof};
use layerx_types::verify::VerificationLevel;
use layerx_wire::encode::Encoder;
use layerx_wire::hash::batch_header_digest;

static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

struct SocketPath(PathBuf);

impl SocketPath {
    fn new(label: &str) -> Self {
        let sequence = NEXT_SOCKET.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "layerx-agentd-balance-{label}-{}-{sequence}.sock",
            std::process::id()
        )))
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn limits() -> Limits {
    Limits {
        maximum_frame_bytes: 1024 * 1024,
        maximum_connections: 1,
        maximum_streams: 4,
        maximum_queued_bytes: 2 * 1024 * 1024,
        deadline: Duration::from_secs(2),
    }
}

fn header_bytes(state_root: [u8; 32], sequencer_id: [u8; 32]) -> Vec<u8> {
    let mut encoder = Encoder::new(354);
    assert_eq!(encoder.structure_header(0x1701), Ok(()));
    assert_eq!(encoder.u8(15), Ok(()));
    for field in 1..=15 {
        assert_eq!(encoder.tag(field, 15), Ok(()));
        match field {
            1 => assert_eq!(encoder.u16(1), Ok(())),
            2 => assert_eq!(encoder.u32(77), Ok(())),
            3 => assert_eq!(encoder.u64(2), Ok(())),
            4 => assert_eq!(encoder.u64(7), Ok(())),
            5 => assert_eq!(encoder.u64(10), Ok(())),
            6 => assert_eq!(encoder.u64(12), Ok(())),
            7 => assert_eq!(encoder.bytes(&[1; 32], 32), Ok(())),
            8 => assert_eq!(encoder.bytes(&state_root, 32), Ok(())),
            9 => assert_eq!(encoder.bytes(&[0x51; 32], 32), Ok(())),
            10 => assert_eq!(encoder.bytes(&[2; 32], 32), Ok(())),
            11 => assert_eq!(encoder.bytes(&[3; 32], 32), Ok(())),
            12 => assert_eq!(encoder.bytes(&[4; 32], 32), Ok(())),
            13 => assert_eq!(encoder.bytes(&[5; 32], 32), Ok(())),
            14 => assert_eq!(encoder.u64(1_000), Ok(())),
            15 => assert_eq!(encoder.bytes(&sequencer_id, 32), Ok(())),
            _ => panic!("unreachable header field"),
        }
    }
    encoder.finish()
}

fn encode_proof(
    asserted_level: u8,
    root: [u8; 32],
    proof: &Proof,
    header: &[u8],
    signature: &[u8; 64],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(asserted_level);
    bytes.extend_from_slice(&root);
    bytes.extend_from_slice(&proof.leaf_index().to_be_bytes());
    bytes.extend_from_slice(&proof.leaf_count().to_be_bytes());
    bytes.push(
        u8::try_from(proof.siblings().len())
            .unwrap_or_else(|error| panic!("proof length: {error}")),
    );
    for sibling in proof.siblings() {
        bytes.extend_from_slice(sibling);
    }
    bytes.extend_from_slice(
        &u32::try_from(header.len())
            .unwrap_or_else(|error| panic!("header length: {error}"))
            .to_be_bytes(),
    );
    bytes.extend_from_slice(header);
    bytes.extend_from_slice(signature);
    bytes
}

fn fixture(asserted_level: u8) -> (Vec<u8>, Vec<u8>, SequencerAuthorization, [u8; 32], [u8; 32]) {
    let account = [0x21; 32];
    let asset = [0x31; 32];
    let mut leaf = Vec::with_capacity(80);
    leaf.extend_from_slice(&account);
    leaf.extend_from_slice(&asset);
    leaf.extend_from_slice(&750_u128.to_be_bytes());
    let other = [0x99; 80];
    let (proof, root) = build_proof(&[leaf.as_slice(), other.as_slice()], 0)
        .unwrap_or_else(|error| panic!("state proof: {error:?}"));
    let key = SigningKey::from_bytes(&[0x41; 32]);
    let sequencer_id = key.verifying_key().to_bytes();
    let header = header_bytes(root, sequencer_id);
    let digest =
        batch_header_digest(&header).unwrap_or_else(|error| panic!("header digest: {error:?}"));
    let signature = key.sign(&digest).to_bytes();
    (
        leaf,
        encode_proof(asserted_level, root, &proof, &header, &signature),
        SequencerAuthorization::new(sequencer_id, sequencer_id, 7, 7),
        account,
        asset,
    )
}

fn context(requested: VerificationLevel, authorization: SequencerAuthorization) -> ReadContext {
    ReadContext {
        interface_version: Version::V1_0,
        correlation_id: 44,
        requested: Requested::new(requested),
        head: Head {
            chain_sequence: 20,
            sealed_batch: 9,
            finalised_checkpoint: [0x71; 32],
        },
        sequencer_authorization: authorization,
    }
}

fn request(stream: &mut UnixStream) -> u64 {
    let frame =
        read_frame(stream, 1024 * 1024).unwrap_or_else(|error| panic!("request frame: {error:?}"));
    let envelope =
        decode_envelope(&frame).unwrap_or_else(|error| panic!("request envelope: {error:?}"));
    assert_eq!(envelope.message_tag, 7);
    envelope.correlation_id
}

fn respond(stream: &mut UnixStream, correlation_id: u64, payload: &[u8], proof: &[u8]) {
    let response = encode_envelope(Envelope {
        version: Version::V1_0,
        message_tag: 8,
        correlation_id,
        canonical_payload: payload,
        proof_material: proof,
    })
    .unwrap_or_else(|error| panic!("response envelope: {error:?}"));
    write_frame(stream, &response, 1024 * 1024)
        .unwrap_or_else(|error| panic!("response frame: {error:?}"));
}

#[test]
fn proven_balance_carries_identity_bytes_level_and_all_freshness_coordinates() {
    let socket = SocketPath::new("proven");
    let listener =
        UnixListener::bind(&socket.0).unwrap_or_else(|error| panic!("listener: {error}"));
    let (leaf, proof, authorization, account, asset) = fixture(3);
    let server_leaf = leaf.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener
            .accept()
            .unwrap_or_else(|error| panic!("accept: {error}"));
        let correlation = request(&mut stream);
        respond(&mut stream, correlation, &server_leaf, &proof);
    });
    let gate = ConnectionGate::new(1);
    let mut transport = Uds::connect(&socket.0, &gate, limits())
        .unwrap_or_else(|error| panic!("connect: {error:?}"));
    let value = balance(
        &mut transport,
        account,
        asset,
        context(VerificationLevel::STATE_PROVEN, authorization),
    )
    .unwrap_or_else(|error| panic!("balance: {error:?}"));
    assert_eq!(value.account, account);
    assert_eq!(value.asset, asset);
    assert_eq!(value.amount.value(), 750);
    assert_eq!(value.canonical_bytes, leaf);
    assert_eq!(value.achieved, VerificationLevel::STATE_PROVEN);
    assert_eq!(value.freshness.value_global_sequence, 12);
    assert_eq!(value.freshness.value_batch_number, 7);
    assert_eq!(value.freshness.observed_head_sequence, 20);
    assert_eq!(value.freshness.latest_sealed_batch, 9);
    assert_eq!(value.freshness.latest_finalised_checkpoint, [0x71; 32]);
    assert!(server.join().is_ok(), "node thread failed");
}

#[test]
fn hostile_asserted_level_cannot_raise_evidence_and_missing_proof_is_named() {
    let socket = SocketPath::new("hostile");
    let listener =
        UnixListener::bind(&socket.0).unwrap_or_else(|error| panic!("listener: {error}"));
    let (leaf, hostile_proof, authorization, account, asset) = fixture(5);
    let server = thread::spawn(move || {
        for proof in [&hostile_proof[..], &[][..]] {
            let (mut stream, _) = listener
                .accept()
                .unwrap_or_else(|error| panic!("accept: {error}"));
            let correlation = request(&mut stream);
            respond(&mut stream, correlation, &leaf, proof);
        }
    });
    let gate = ConnectionGate::new(1);
    {
        let mut transport = Uds::connect(&socket.0, &gate, limits())
            .unwrap_or_else(|error| panic!("connect hostile: {error:?}"));
        assert_eq!(
            balance(
                &mut transport,
                account,
                asset,
                context(VerificationLevel::CHECKPOINT_FINALISED, authorization),
            ),
            Err(ReadError::MissingEvidence {
                requested: VerificationLevel::CHECKPOINT_FINALISED,
                achieved: VerificationLevel::STATE_PROVEN,
            })
        );
    }
    {
        let mut transport = Uds::connect(&socket.0, &gate, limits())
            .unwrap_or_else(|error| panic!("connect missing: {error:?}"));
        assert_eq!(
            balance(
                &mut transport,
                account,
                asset,
                context(VerificationLevel::STATE_PROVEN, authorization),
            ),
            Err(ReadError::MissingEvidence {
                requested: VerificationLevel::STATE_PROVEN,
                achieved: VerificationLevel::UNVERIFIED,
            })
        );
    }
    assert!(server.join().is_ok(), "node thread failed");
}
