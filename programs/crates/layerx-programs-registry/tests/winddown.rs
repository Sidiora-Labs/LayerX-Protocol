use layerx_programs::{
    Deprecation, DeprecationRefusal, DeprecationRequest, ProgramLifecycle, Registry, ValueAccount,
    WindDownPolicy, WindDownStateAccess,
};
use layerx_programs_runtime::{DeploymentReceipt, ProgramId, ProgramVersion, UpgradePolicy};

fn program(byte: u8) -> ProgramId {
    ProgramId::new([byte; 32]).unwrap_or_else(|error| panic!("program id: {error}"))
}

fn deployed_registry(id: ProgramId) -> Registry {
    let mut registry = Registry::new();
    registry
        .record_deployment(
            &DeploymentReceipt {
                program: id,
                version: 1,
                old_code_hash: None,
                new_code_hash: [2; 32],
                migration: None,
            },
            &ProgramVersion {
                code_hash: [2; 32],
                wasm: vec![0, 97, 115, 109, 1, 0, 0, 0],
                abi_version: 1,
            },
            UpgradePolicy::Authority([4; 32]),
            [3; 32],
        )
        .unwrap_or_else(|error| panic!("registry deployment: {error}"));
    registry
}

fn request(id: ProgramId, account: ValueAccount) -> DeprecationRequest {
    DeprecationRequest {
        program: id,
        expected: ProgramLifecycle::Active,
        target: ProgramLifecycle::Deprecated,
        authority: [4; 32],
        effective_sequence: 100,
        wind_down: WindDownPolicy {
            exit_program: [5; 32],
            deadline: 200,
            state_access: WindDownStateAccess::ReadOnly,
        },
        value_accounts: vec![account],
    }
}

#[test]
fn live_balance_remains_reachable_through_authorized_exit_after_deprecation() {
    let id = program(1);
    let mut registry = deployed_registry(id);
    let mut deprecation = Deprecation::new();
    let account = ValueAccount {
        account_id: [6; 32],
        balance: 500,
        exit_destination: [7; 32],
        exit_authority: [8; 32],
        exit_receipt_digest: [9; 32],
        exit_limit: 500,
        exit_not_after: 200,
    };
    let receipt = deprecation
        .transition(&mut registry, &request(id, account))
        .unwrap_or_else(|error| panic!("deprecation: {error}"));
    assert_eq!(receipt.current, ProgramLifecycle::Deprecated);
    assert_eq!(receipt.live_value_accounts, 1);
    let view = deprecation
        .read(&registry, id)
        .unwrap_or_else(|error| panic!("wind-down read: {error}"));
    assert_eq!(view.lifecycle, ProgramLifecycle::Deprecated);
    assert_eq!(view.live_value_accounts, vec![account]);
    assert_eq!(view.transition_history, vec![receipt]);
}

#[test]
fn deprecation_refuses_to_strand_live_value() {
    let id = program(8);
    let mut registry = deployed_registry(id);
    let mut deprecation = Deprecation::new();
    let account = ValueAccount {
        account_id: [9; 32],
        balance: 1,
        exit_destination: [0; 32],
        exit_authority: [0; 32],
        exit_receipt_digest: [0; 32],
        exit_limit: 0,
        exit_not_after: 0,
    };
    assert_eq!(
        deprecation.transition(&mut registry, &request(id, account)),
        Err(DeprecationRefusal::ValueWouldBeStranded {
            account_id: account.account_id,
        })
    );
}

#[test]
fn deprecation_refuses_non_deployment_authority() {
    let id = program(9);
    let mut registry = deployed_registry(id);
    let mut deprecation = Deprecation::new();
    let account = ValueAccount {
        account_id: [10; 32],
        balance: 0,
        exit_destination: [0; 32],
        exit_authority: [0; 32],
        exit_receipt_digest: [0; 32],
        exit_limit: 0,
        exit_not_after: 0,
    };
    let mut transition = request(id, account);
    transition.authority = [99; 32];
    assert!(matches!(
        deprecation.transition(&mut registry, &transition),
        Err(DeprecationRefusal::Registry(
            layerx_programs::RegistryError::InvalidLifecycleTransition
        ))
    ));
}

#[test]
fn tombstone_preserves_history_and_exit_only_state() {
    let id = program(10);
    let mut registry = deployed_registry(id);
    let mut deprecation = Deprecation::new();
    let account = ValueAccount {
        account_id: [11; 32],
        balance: 75,
        exit_destination: [12; 32],
        exit_authority: [13; 32],
        exit_receipt_digest: [14; 32],
        exit_limit: 75,
        exit_not_after: 200,
    };
    deprecation
        .transition(&mut registry, &request(id, account))
        .unwrap_or_else(|error| panic!("deprecation: {error}"));
    let mut tombstone = request(id, account);
    tombstone.expected = ProgramLifecycle::Deprecated;
    tombstone.target = ProgramLifecycle::Tombstoned;
    tombstone.effective_sequence = 150;
    deprecation
        .transition(&mut registry, &tombstone)
        .unwrap_or_else(|error| panic!("tombstone: {error}"));
    let view = deprecation
        .read(&registry, id)
        .unwrap_or_else(|error| panic!("wind-down read: {error}"));
    assert_eq!(view.lifecycle, ProgramLifecycle::Tombstoned);
    assert_eq!(view.transition_history.len(), 2);
    assert_eq!(view.live_value_accounts[0].balance, 75);
}
