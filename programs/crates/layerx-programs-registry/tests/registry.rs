use layerx_programs::{
    BuildEnvironment, ProgramLifecycle, PublishedSource, ReadFreshness, Registry, RegistryError,
    RegistryReadAuthority, ReproducibleBuild, SourceStatus,
};
use layerx_programs_runtime::{DeploymentReceipt, ProgramId, ProgramVersion, UpgradePolicy};

const PROGRAM: [u8; 32] = [0x31; 32];
const RECEIPT: [u8; 32] = [0x41; 32];
const WASM_HASH: [u8; 32] = [
    0x33, 0x61, 0x54, 0xbf, 0x67, 0xf7, 0x65, 0xf8, 0xf7, 0x5d, 0x16, 0xa0, 0xac, 0xce, 0xe6, 0x1b,
    0x5e, 0xe5, 0xf6, 0xa7, 0x5b, 0x2a, 0x29, 0x05, 0x70, 0x3d, 0xf9, 0x13, 0xbd, 0x55, 0x0f, 0x3e,
];

struct ReceiptAuthority;
struct WrongAuthority;

impl RegistryReadAuthority for ReceiptAuthority {
    fn verify_registry_read(
        &self,
        _program: ProgramId,
        latest: &layerx_programs::RegistryVersion,
    ) -> Result<([u8; 32], ReadFreshness), RegistryError> {
        Ok((
            latest.deployment_receipt_digest,
            ReadFreshness {
                observed_sequence: 77,
                observed_at: 1_700_000_000,
            },
        ))
    }
}

impl RegistryReadAuthority for WrongAuthority {
    fn verify_registry_read(
        &self,
        _program: ProgramId,
        _latest: &layerx_programs::RegistryVersion,
    ) -> Result<([u8; 32], ReadFreshness), RegistryError> {
        Ok((
            [0x99; 32],
            ReadFreshness {
                observed_sequence: 1,
                observed_at: 1,
            },
        ))
    }
}

#[test]
fn registry_records_authority_version_history_and_receipt_verified_reads() {
    let program = program();
    let policy = UpgradePolicy::Authority([0x51; 32]);
    let mut registry = Registry::new();
    registry
        .record_deployment(
            &deployment(program, 1, None, WASM_HASH),
            &version(WASM_HASH),
            policy,
            RECEIPT,
        )
        .unwrap_or_else(|error| panic!("deployment: {error}"));
    let second_hash = [0x61; 32];
    registry
        .record_deployment(
            &deployment(program, 2, Some(WASM_HASH), second_hash),
            &version(second_hash),
            policy,
            [0x42; 32],
        )
        .unwrap_or_else(|error| panic!("upgrade: {error}"));

    let read = registry
        .read(program, &ReceiptAuthority)
        .unwrap_or_else(|error| panic!("registry read: {error}"));
    assert_eq!(read.entry.program, program);
    assert_eq!(read.entry.upgrade_policy, policy);
    assert_eq!(read.entry.lifecycle, ProgramLifecycle::Active);
    assert_eq!(read.entry.versions.len(), 2);
    assert_eq!(read.entry.versions[0].code_hash, WASM_HASH);
    assert_eq!(read.entry.versions[1].code_hash, second_hash);
    assert_eq!(read.freshness.observed_sequence, 77);
}

#[test]
fn reproducible_build_verification_records_success_and_visible_mismatch() {
    let program = program();
    let mut registry = Registry::new();
    registry
        .record_deployment(
            &deployment(program, 1, None, WASM_HASH),
            &version(WASM_HASH),
            UpgradePolicy::Immutable,
            RECEIPT,
        )
        .unwrap_or_else(|error| panic!("deployment: {error}"));
    let verified = ReproducibleBuild::from_output(&source(), environment(), b"wasm")
        .unwrap_or_else(|error| panic!("build evidence: {error}"));
    assert_eq!(verified.artifact_digest, WASM_HASH);
    assert!(matches!(
        registry.verify_source(program, 1, &verified),
        Ok(SourceStatus::Verified { .. })
    ));

    let altered = ReproducibleBuild::from_output(&source(), environment(), b"wasm-altered")
        .unwrap_or_else(|error| panic!("altered evidence: {error}"));
    let mismatch = registry
        .verify_source(program, 1, &altered)
        .unwrap_or_else(|error| panic!("mismatch status: {error}"));
    assert_eq!(
        mismatch,
        SourceStatus::Mismatch {
            expected: WASM_HASH,
            reproduced: altered.artifact_digest,
        }
    );
    let read = registry
        .read(program, &ReceiptAuthority)
        .unwrap_or_else(|error| panic!("registry read: {error}"));
    assert_eq!(read.entry.versions[0].source, mismatch);
}

#[test]
fn noncontiguous_or_unverified_registry_history_is_refused() {
    let program = program();
    let mut registry = Registry::new();
    assert_eq!(
        registry.record_deployment(
            &deployment(program, 2, Some(WASM_HASH), [0x61; 32]),
            &version([0x61; 32]),
            UpgradePolicy::Immutable,
            RECEIPT,
        ),
        Err(RegistryError::VersionHistoryMismatch)
    );

    registry
        .record_deployment(
            &deployment(program, 1, None, WASM_HASH),
            &version(WASM_HASH),
            UpgradePolicy::Immutable,
            RECEIPT,
        )
        .unwrap_or_else(|error| panic!("deployment: {error}"));
    assert_eq!(
        registry.read(program, &WrongAuthority),
        Err(RegistryError::UnverifiedRead)
    );
}

fn program() -> ProgramId {
    ProgramId::new(PROGRAM).unwrap_or_else(|error| panic!("program: {error}"))
}

fn version(code_hash: [u8; 32]) -> ProgramVersion {
    ProgramVersion {
        code_hash,
        wasm: b"wasm".to_vec(),
        abi_version: 1,
    }
}

fn deployment(
    program: ProgramId,
    number: u32,
    old_code_hash: Option<[u8; 32]>,
    new_code_hash: [u8; 32],
) -> DeploymentReceipt {
    DeploymentReceipt {
        program,
        version: number,
        old_code_hash,
        new_code_hash,
        migration: None,
    }
}

fn source() -> PublishedSource {
    PublishedSource {
        uri: "https://source.example/program.tar".to_owned(),
        canonical_archive: b"canonical-source-archive".to_vec(),
    }
}

fn environment() -> BuildEnvironment {
    BuildEnvironment {
        builder_image_digest: [0x71; 32],
        toolchain_digest: [0x72; 32],
        dependency_lock_digest: [0x73; 32],
        source_date_epoch: 1_700_000_000,
        command: vec![
            "cargo".to_owned(),
            "build".to_owned(),
            "--locked".to_owned(),
        ],
    }
}
