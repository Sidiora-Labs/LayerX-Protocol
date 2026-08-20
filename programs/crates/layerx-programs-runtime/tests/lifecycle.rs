use layerx_programs_runtime::test_support::{
    add_module, code_section, export_section, func_body, function_section, module, type_section,
    OP_END, OP_I32_CONST, TYPE_I32,
};
use layerx_programs_runtime::{
    Deploy, Lifecycle, LifecycleRefusal, Migration, ProgramId, Upgrade, UpgradePolicy, ABI_VERSION,
};

fn program(byte: u8) -> ProgramId {
    ProgramId::new([byte; 32]).unwrap_or_else(|error| panic!("program id refused: {error}"))
}

fn migration_module() -> Vec<u8> {
    module(&[
        type_section(&[(&[], &[TYPE_I32])]),
        function_section(&[0]),
        export_section(&[("migrate", 0)]),
        code_section(&[func_body(&[], &[OP_I32_CONST, 0, OP_END])]),
    ])
}

#[test]
fn deploy_becomes_callable_only_with_verified_receipt() {
    let mut lifecycle = Lifecycle::declared()
        .unwrap_or_else(|error| panic!("lifecycle construction refused: {error}"));
    let receipt = lifecycle
        .deploy(Deploy {
            program: program(1),
            code_hash: [2; 32],
            wasm: add_module(),
            abi_version: ABI_VERSION,
            upgrade_policy: UpgradePolicy::default(),
        })
        .unwrap_or_else(|error| panic!("deployment refused: {error}"));
    assert_eq!(
        lifecycle.callable(&receipt, false),
        Err(LifecycleRefusal::UnverifiedReceipt)
    );
    let callable = lifecycle
        .callable(&receipt, true)
        .unwrap_or_else(|error| panic!("verified deployment not callable: {error}"));
    assert_eq!(callable.code_hash, [2; 32]);
}

#[test]
fn immutable_is_default_and_upgrade_records_hash_history() {
    let mut immutable = Lifecycle::declared()
        .unwrap_or_else(|error| panic!("lifecycle construction refused: {error}"));
    immutable
        .deploy(Deploy {
            program: program(3),
            code_hash: [4; 32],
            wasm: add_module(),
            abi_version: ABI_VERSION,
            upgrade_policy: UpgradePolicy::default(),
        })
        .unwrap_or_else(|error| panic!("deployment refused: {error}"));
    let request = Upgrade {
        program: program(3),
        authority: [9; 32],
        code_hash: [5; 32],
        wasm: migration_module(),
        abi_version: ABI_VERSION,
        migration: Some(Migration {
            export: "migrate".to_string(),
        }),
    };
    assert_eq!(immutable.upgrade(request), Err(LifecycleRefusal::Immutable));

    let mut upgradeable = Lifecycle::declared()
        .unwrap_or_else(|error| panic!("lifecycle construction refused: {error}"));
    upgradeable
        .deploy(Deploy {
            program: program(6),
            code_hash: [7; 32],
            wasm: add_module(),
            abi_version: ABI_VERSION,
            upgrade_policy: UpgradePolicy::Authority([9; 32]),
        })
        .unwrap_or_else(|error| panic!("deployment refused: {error}"));
    let receipt = upgradeable
        .upgrade(Upgrade {
            program: program(6),
            authority: [9; 32],
            code_hash: [8; 32],
            wasm: migration_module(),
            abi_version: ABI_VERSION,
            migration: Some(Migration {
                export: "migrate".to_string(),
            }),
        })
        .unwrap_or_else(|error| panic!("upgrade refused: {error}"));
    assert_eq!(receipt.version, 2);
    assert_eq!(receipt.old_code_hash, Some([7; 32]));
    assert!(receipt.migration.is_some());
}

#[test]
fn failed_validation_is_preserved_but_never_executable() {
    let mut lifecycle = Lifecycle::declared()
        .unwrap_or_else(|error| panic!("lifecycle construction refused: {error}"));
    let result = lifecycle.deploy(Deploy {
        program: program(10),
        code_hash: [11; 32],
        wasm: vec![1, 2, 3],
        abi_version: ABI_VERSION,
        upgrade_policy: UpgradePolicy::default(),
    });
    assert!(matches!(result, Err(LifecycleRefusal::Validation(_))));
    assert_eq!(lifecycle.diagnostics().len(), 1);
}

#[test]
fn unknown_program_and_incompatible_abi_are_typed() {
    let mut lifecycle = Lifecycle::declared()
        .unwrap_or_else(|error| panic!("lifecycle construction refused: {error}"));
    let unknown = lifecycle.upgrade(Upgrade {
        program: program(12),
        authority: [13; 32],
        code_hash: [14; 32],
        wasm: migration_module(),
        abi_version: ABI_VERSION,
        migration: None,
    });
    assert_eq!(unknown, Err(LifecycleRefusal::UnknownProgram));
    let incompatible = lifecycle.deploy(Deploy {
        program: program(15),
        code_hash: [16; 32],
        wasm: add_module(),
        abi_version: ABI_VERSION + 1,
        upgrade_policy: UpgradePolicy::default(),
    });
    assert_eq!(
        incompatible,
        Err(LifecycleRefusal::IncompatibleAbi {
            requested: ABI_VERSION + 1,
            supported: ABI_VERSION,
        })
    );
}
