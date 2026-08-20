use layerx_programs_runtime::{
    AbiEffects, Capability, CapabilitySet, PrincipalId, ProgramCall, ProgramId, TransferCapability,
    TransferLawError, TransferRequest,
};

fn program_id(byte: u8) -> ProgramId {
    ProgramId::new([byte; 32]).unwrap_or_else(|error| panic!("program: {error}"))
}

fn principal_id(byte: u8) -> PrincipalId {
    PrincipalId::new([byte; 32]).unwrap_or_else(|error| panic!("principal: {error}"))
}

fn request(program: ProgramId, principal: PrincipalId, amount: u128) -> TransferRequest {
    request_for(program, principal, [3; 32], [4; 32], amount)
}

fn request_for(
    program: ProgramId,
    principal: PrincipalId,
    asset: [u8; 32],
    to: [u8; 32],
    amount: u128,
) -> TransferRequest {
    TransferRequest {
        program,
        principal,
        asset,
        to,
        amount,
    }
}

fn child_call(
    parent_program: ProgramId,
    child_program: ProgramId,
    principal: PrincipalId,
    asset: [u8; 32],
    to: [u8; 32],
    maximum_amount: u128,
) -> ProgramCall {
    let capabilities = CapabilitySet::new([Capability::Transfer402 {
        asset,
        to,
        maximum_amount,
    }])
    .unwrap_or_else(|error| panic!("child capabilities: {error}"));
    ProgramCall {
        caller: parent_program,
        callee: child_program,
        principal,
        input: Vec::new(),
        capabilities,
    }
}

#[test]
fn transfer_set_is_bound_to_invocation_authority_and_exact_order() {
    let program = program_id(1);
    let principal = principal_id(2);
    let capability = TransferCapability::new(program, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    let effects = AbiEffects {
        transfers: vec![
            request(program, principal, 7),
            request(program, principal, 11),
        ],
        ..AbiEffects::default()
    };
    let set = capability
        .authorize(&effects)
        .unwrap_or_else(|error| panic!("set: {error}"));
    assert_eq!(set.program(), program);
    assert_eq!(set.principal(), principal);
    assert_eq!(set.invocation_authority(), [5; 32]);
    assert_eq!(set.total_amount(), 18);
    assert_eq!(set.legs(), effects.transfers);
    assert!(set
        .canonical()
        .starts_with(b"LayerX/programs/402LXP/transfer-set/v1\0"));
}

#[test]
fn empty_invalid_and_overflowing_sets_are_refused_before_core() {
    let program = program_id(1);
    let principal = principal_id(2);
    assert_eq!(
        TransferCapability::new(program, principal, [0; 32]),
        Err(TransferLawError::UnverifiedAuthority)
    );
    let capability = TransferCapability::new(program, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    assert_eq!(
        capability.authorize(&AbiEffects::default()),
        Err(TransferLawError::InvalidTransferSet)
    );
    let overflow = AbiEffects {
        transfers: vec![
            request(program, principal, u128::MAX),
            request(program, principal, 1),
        ],
        ..AbiEffects::default()
    };
    assert_eq!(
        capability.authorize(&overflow),
        Err(TransferLawError::AmountOverflow)
    );
}

#[test]
fn forged_program_or_principal_is_an_invariant_one_violation() {
    let program = program_id(1);
    let principal = principal_id(2);
    let capability = TransferCapability::new(program, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    for transfer in [
        request(program_id(9), principal, 1),
        request(program, principal_id(9), 1),
    ] {
        let effects = AbiEffects {
            transfers: vec![transfer],
            ..AbiEffects::default()
        };
        assert_eq!(
            capability.authorize(&effects),
            Err(TransferLawError::InvariantViolation)
        );
    }
}

#[test]
fn child_transfer_requires_a_reachable_call_graph_edge() {
    let root = program_id(1);
    let child = program_id(7);
    let principal = principal_id(2);
    let capability = TransferCapability::new(root, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    let disconnected = AbiEffects {
        transfers: vec![request(child, principal, 1)],
        ..AbiEffects::default()
    };
    assert_eq!(
        capability.authorize(&disconnected),
        Err(TransferLawError::InvariantViolation)
    );
}

#[test]
fn child_call_cannot_change_the_invoking_principal() {
    let root = program_id(1);
    let child = program_id(7);
    let principal = principal_id(2);
    let capability = TransferCapability::new(root, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    let effects = AbiEffects {
        calls: vec![child_call(
            root,
            child,
            principal_id(9),
            [3; 32],
            [4; 32],
            10,
        )],
        transfers: vec![request(child, principal, 1)],
        ..AbiEffects::default()
    };
    assert_eq!(
        capability.authorize(&effects),
        Err(TransferLawError::InvariantViolation)
    );
}

#[test]
fn child_transfer_must_fit_its_narrowed_asset_recipient_and_amount() {
    let root = program_id(1);
    let child = program_id(7);
    let principal = principal_id(2);
    let capability = TransferCapability::new(root, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    for transfer in [
        request_for(child, principal, [8; 32], [4; 32], 1),
        request_for(child, principal, [3; 32], [8; 32], 1),
        request_for(child, principal, [3; 32], [4; 32], 11),
    ] {
        let effects = AbiEffects {
            calls: vec![child_call(root, child, principal, [3; 32], [4; 32], 10)],
            transfers: vec![transfer],
            ..AbiEffects::default()
        };
        assert_eq!(
            capability.authorize(&effects),
            Err(TransferLawError::CapabilityEscalation)
        );
    }
    let cumulative = AbiEffects {
        calls: vec![child_call(root, child, principal, [3; 32], [4; 32], 10)],
        transfers: vec![request(child, principal, 6), request(child, principal, 6)],
        ..AbiEffects::default()
    };
    assert_eq!(
        capability.authorize(&cumulative),
        Err(TransferLawError::CapabilityEscalation)
    );
}

#[test]
fn canonical_transfer_set_commits_child_provenance_and_call_grants() {
    let root = program_id(1);
    let child = program_id(7);
    let principal = principal_id(2);
    let capability = TransferCapability::new(root, principal, [5; 32])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    let effects = |maximum_amount| AbiEffects {
        calls: vec![child_call(
            root,
            child,
            principal,
            [3; 32],
            [4; 32],
            maximum_amount,
        )],
        transfers: vec![request(child, principal, 7)],
        ..AbiEffects::default()
    };
    let narrow = capability
        .authorize(&effects(7))
        .unwrap_or_else(|error| panic!("narrow graph: {error}"));
    let broad = capability
        .authorize(&effects(8))
        .unwrap_or_else(|error| panic!("broad graph: {error}"));
    assert_eq!(narrow.legs()[0].program, child);
    assert_ne!(narrow.canonical(), broad.canonical());
}
