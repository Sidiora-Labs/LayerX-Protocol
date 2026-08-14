use layerx_crypto::keystore::{Keystore, KeystoreEntropy, KeystoreError};
use layerx_crypto::session::{issue_session_key, SessionIssueError, SessionKeyRequest};
use layerx_types::payload::{ActivityType, ModuleId};
use layerx_wire::decode::Decoder;

fn activity_type(module: ModuleId, ordinal: u16) -> ActivityType {
    let Ok(activity_type) = ActivityType::new(module, ordinal) else {
        panic!("valid activity type rejected");
    };
    activity_type
}

fn entropy() -> KeystoreEntropy {
    let Ok(entropy) = KeystoreEntropy::new([0x31; 16], [0x42; 24]) else {
        panic!("valid keystore entropy rejected");
    };
    entropy
}

#[test]
fn encrypted_keystore_round_trips_only_under_its_bound_context() {
    let key = [0x5a; 32];
    let secret = b"operator supplied high entropy secret";
    let identity = b"did:layerx:alice";
    let Ok(keystore) = Keystore::seal(&key, secret, identity, 17, entropy()) else {
        panic!("valid keystore input rejected");
    };
    let Ok(persisted) = keystore.to_bytes() else {
        panic!("encrypted keystore could not be persisted");
    };
    assert!(!persisted.windows(key.len()).any(|window| window == key));
    let Ok(restored) = Keystore::from_bytes(&persisted) else {
        panic!("persisted keystore rejected");
    };
    assert_eq!(restored.identity(), identity);
    assert_eq!(restored.network_id(), 17);
    let Ok(opened) = restored.open(secret, identity, 17) else {
        panic!("correct keystore context rejected");
    };
    assert_eq!(*opened, key);
    let rendered = format!("{restored:?}");
    assert!(!rendered.contains("5a5a5a5a"));
    assert!(!rendered.contains("operator supplied"));
}

#[test]
fn wrong_secret_network_and_identity_have_typed_refusals() {
    let key = [0x5a; 32];
    let secret = b"operator supplied high entropy secret";
    let identity = b"did:layerx:alice";
    let Ok(keystore) = Keystore::seal(&key, secret, identity, 17, entropy()) else {
        panic!("valid keystore input rejected");
    };
    assert_eq!(
        keystore.open(b"wrong secret", identity, 17).err(),
        Some(KeystoreError::AuthenticationFailed)
    );
    assert_eq!(
        keystore.open(secret, identity, 18).err(),
        Some(KeystoreError::NetworkMismatch)
    );
    assert_eq!(
        keystore.open(secret, b"did:layerx:mallory", 17).err(),
        Some(KeystoreError::IdentityMismatch)
    );
}

#[test]
fn any_persisted_ciphertext_tamper_is_refused() {
    let key = [0x5a; 32];
    let secret = b"operator supplied high entropy secret";
    let identity = b"did:layerx:alice";
    let Ok(keystore) = Keystore::seal(&key, secret, identity, 17, entropy()) else {
        panic!("valid keystore input rejected");
    };
    let Ok(mut persisted) = keystore.to_bytes() else {
        panic!("encrypted keystore could not be persisted");
    };
    let last = persisted.len().saturating_sub(1);
    persisted[last] ^= 1;
    let Ok(tampered) = Keystore::from_bytes(&persisted) else {
        panic!("structurally valid encrypted container rejected before authentication");
    };
    assert_eq!(
        tampered.open(secret, identity, 17).err(),
        Some(KeystoreError::AuthenticationFailed)
    );
}

fn bounded_request() -> SessionKeyRequest {
    SessionKeyRequest {
        grantor: [0x11; 32],
        session_public_key: [0x22; 32],
        not_before: 10,
        expires_at: Some(100),
        permitted_activity_types: vec![
            activity_type(ModuleId::Asset, 5),
            activity_type(ModuleId::Asset, 6),
        ],
        revocation_sequence: Some(9),
    }
}

#[test]
fn session_issuance_emits_the_exact_protocol_session_authority_grant() {
    let request = bounded_request();
    let Ok(issued) = issue_session_key(&request) else {
        panic!("fully bounded session key refused");
    };
    assert_eq!(issued.authority.as_bytes(), issued.registration_payload);
    assert_eq!(
        issued.permitted_activity_types,
        request.permitted_activity_types
    );
    assert_eq!(issued.expires_at, 100);
    assert_eq!(issued.revocation_sequence, 9);
    assert_ne!(issued.grant_id, [0_u8; 32]);

    let mut decoder = Decoder::new(&issued.registration_payload, 0);
    assert_eq!(decoder.structure_header(0x2001), Ok(()));
    assert_eq!(decoder.u8(), Ok(1));
    assert_eq!(decoder.bytes(32), Ok([0x11; 32].as_slice()));
    assert_eq!(decoder.bytes(32), Ok([0x11; 32].as_slice()));
    assert_eq!(decoder.u8(), Ok(2));
    assert_eq!(decoder.bytes(32), Ok([0x22; 32].as_slice()));
    assert_eq!(decoder.u64(), Ok(1_u64 << 1));
    assert_eq!(decoder.u16(), Ok(5));
    assert_eq!(decoder.u16(), Ok(6));
    assert_eq!(decoder.bytes(32), Ok([0_u8; 32].as_slice()));
    assert_eq!(decoder.u128(), Ok(0));
    assert_eq!(decoder.u128(), Ok(0));
    assert_eq!(decoder.u128(), Ok(0));
    assert_eq!(decoder.u64(), Ok(0));
    assert_eq!(decoder.u128(), Ok(0));
    assert_eq!(decoder.u128(), Ok(0));
    assert_eq!(decoder.u64(), Ok(0));
    assert_eq!(decoder.bytes(32), Ok([0_u8; 32].as_slice()));
    assert_eq!(decoder.u64(), Ok(10));
    assert_eq!(decoder.u64(), Ok(100));
    assert_eq!(decoder.u64(), Ok(9));
    assert_eq!(decoder.u8(), Ok(0));
    assert_eq!(decoder.u64(), Ok(0));
    assert_eq!(decoder.bytes(64), Ok([0_u8; 64].as_slice()));
    assert_eq!(decoder.finish(), Ok(()));
}

#[test]
fn session_issuance_refuses_every_unbounded_or_widened_scope() {
    let mut request = bounded_request();
    request.expires_at = None;
    assert_eq!(
        issue_session_key(&request).err(),
        Some(SessionIssueError::MissingExpiry)
    );

    let mut request = bounded_request();
    request.revocation_sequence = None;
    assert_eq!(
        issue_session_key(&request).err(),
        Some(SessionIssueError::MissingRevocationSequence)
    );

    let mut request = bounded_request();
    request.permitted_activity_types.clear();
    assert_eq!(
        issue_session_key(&request).err(),
        Some(SessionIssueError::EmptyActivitySet)
    );

    let mut request = bounded_request();
    request.permitted_activity_types = vec![
        activity_type(ModuleId::Asset, 5),
        activity_type(ModuleId::Asset, 7),
    ];
    assert_eq!(
        issue_session_key(&request).err(),
        Some(SessionIssueError::NonRepresentableActivitySet)
    );
}
