#[path = "../../layerx-programs-registry/tests/support/mod.rs"]
mod protocol_support;

use layerx_programs_runtime::{AuthorizationContext, CapabilitySet, FeeSchedule, Meter,
    PrincipalId, ProgramId, ResourceBudget, RuntimeGlobal, ValidationLimits, WasmEngine,
    WasmValue};
use layerx_programs_runtime::test_support::{code_section, func_body, function_section, module,
    raw_section, type_section, unsigned_leb, OP_END, OP_I32_ADD, OP_I32_CONST, TYPE_I32};
use layerx_programs_sandbox::{restore, ContinuationPoint, Lease,
    LeaseActivity, LeaseId, LeaseLimits, LeaseState, LeaseTransition, NamespaceCell,
    SandboxState, Snapshot, SnapshotRefusal, TransitionEvidence};

const ENTRYPOINT: &[u8] = b"sandbox_transition";
const CALL_PREFIX: usize = 106;

fn candidate(id: u8, tenant: u8, opened: u64, expiry: u64) -> Lease {
    Lease::request(
        LeaseId::new([id; 32]).unwrap_or_else(|error| panic!("lease id: {error}")),
        PrincipalId::new([tenant; 32]).unwrap_or_else(|error| panic!("tenant: {error}")),
        ProgramId::new([3; 32]).unwrap_or_else(|error| panic!("host: {error}")),
        [4; 32], [5; 32], 10_000,
        LeaseLimits { cpu_fuel: 10_000, memory_bytes: 1 << 20,
            storage_read_bytes: 1 << 20, storage_write_bytes: 1 << 20,
            output_values: 1_000, output_bytes: 1 << 20, table_elements: 1_000,
            namespace_bytes: 1 << 20 }, opened, expiry,
    ).unwrap_or_else(|error| panic!("lease: {error}"))
}

fn call_payload(host: ProgramId, transition: LeaseTransition) -> Vec<u8> {
    let mut calldata = vec![0; 101];
    calldata[..2].copy_from_slice(&1u16.to_be_bytes());
    calldata[2..34].copy_from_slice(&transition.lease.bytes());
    calldata[34..66].copy_from_slice(&transition.tenant.bytes());
    calldata[66] = transition.activity as u8;
    calldata[67] = transition.from as u8;
    calldata[68] = transition.to as u8;
    calldata[69..].copy_from_slice(&transition.usage_observation_digest);
    let mut payload = vec![0; CALL_PREFIX];
    payload[..32].copy_from_slice(&host.bytes());
    payload[32..34].copy_from_slice(&2u16.to_be_bytes());
    payload[34..36].copy_from_slice(&(ENTRYPOINT.len() as u16).to_be_bytes());
    payload[36..40].copy_from_slice(&(calldata.len() as u32).to_be_bytes());
    payload[46..50].copy_from_slice(&1u32.to_be_bytes());
    payload.extend_from_slice(ENTRYPOINT);
    payload.extend_from_slice(&calldata);
    payload
}

fn evidence(lease: &Lease, mut transition: LeaseTransition, batch: u64)
    -> (LeaseTransition, TransitionEvidence) {
    let fixture = protocol_support::programs_call_fixture(
        call_payload(lease.host_program(), transition), batch, protocol_support::NOW);
    let verifier = protocol_support::verifier_for_fixture(&fixture, batch, batch, None, 1_000);
    let proof = &fixture.proof.state;
    let head = verifier.verify_current_protocol_head(&proof.receipt, &proof.receipt_proof,
        &proof.header, &proof.header_signature, protocol_support::NOW)
        .unwrap_or_else(|error| panic!("head: {error}"));
    transition.activity_id = head.activity_id();
    let verified = TransitionEvidence::verify_call(&head, lease, transition,
        &AuthorizationContext::new(lease.tenant(), CapabilitySet::empty()),
        &fixture.proof.activity, &fixture.proof.activity_proof, &proof.header)
        .unwrap_or_else(|error| panic!("evidence: {error}"));
    (transition, verified)
}

fn advance(mut lease: Lease, edges: &[(LeaseActivity, LeaseState, LeaseState, u64)]) -> Lease {
    let request = LeaseTransition { lease: lease.id(), tenant: lease.tenant(),
        activity: LeaseActivity::Request, from: LeaseState::Requested,
        to: LeaseState::Requested, activity_id: [0; 32],
        usage_observation_digest: lease.request_binding_digest()
            .unwrap_or_else(|error| panic!("request digest: {error}")) };
    let (request, proof) = evidence(&lease, request, lease.opened_at());
    let mut book = layerx_programs_sandbox::LeaseBook::new();
    book.insert_requested(lease, request, proof)
        .unwrap_or_else(|error| panic!("request: {error}"));
    for &(activity, from, to, batch) in edges {
        lease = book.get(request.lease).unwrap_or_else(|| panic!("lease")).clone();
        let transition = LeaseTransition { lease: lease.id(), tenant: lease.tenant(),
            activity, from, to, activity_id: [0; 32], usage_observation_digest: [0; 32] };
        let (transition, proof) = evidence(&lease, transition, batch);
        book.transition(lease.id(), transition, proof)
            .unwrap_or_else(|error| panic!("advance: {error}"));
    }
    book.get(request.lease).unwrap_or_else(|| panic!("advanced lease")).clone()
}

fn advance_existing(mut lease: Lease, edges: &[(LeaseActivity, LeaseState, LeaseState, u64)]) -> Lease {
    for &(activity, from, to, batch) in edges {
        let transition = LeaseTransition { lease: lease.id(), tenant: lease.tenant(),
            activity, from, to, activity_id: [0; 32], usage_observation_digest: [0; 32] };
        let (transition, proof) = evidence(&lease, transition, batch);
        lease.transition(transition, proof)
            .unwrap_or_else(|error| panic!("advance existing: {error}"));
    }
    lease
}

fn active(id: u8, tenant: u8) -> Lease {
    advance(candidate(id, tenant, 10, 30), &[
        (LeaseActivity::Fund, LeaseState::Requested, LeaseState::Funded, 11),
        (LeaseActivity::Activate, LeaseState::Funded, LeaseState::Active, 12),
    ])
}

fn funded(id: u8, tenant: u8) -> Lease {
    advance(candidate(id, tenant, 20, 40), &[
        (LeaseActivity::Fund, LeaseState::Requested, LeaseState::Funded, 21),
    ])
}

fn activation(lease: &Lease, batch: u64) -> (LeaseTransition, TransitionEvidence) {
    evidence(lease, LeaseTransition { lease: lease.id(), tenant: lease.tenant(),
        activity: LeaseActivity::Activate, from: LeaseState::Funded, to: LeaseState::Active,
        activity_id: [0; 32], usage_observation_digest: [0; 32] }, batch)
}

fn state(lease: &Lease) -> SandboxState {
    SandboxState::new(lease, vec![0, 1, 2, 3, 5, 8],
        vec![RuntimeGlobal { name: "counter".to_string(), value: WasmValue::I64(13) },
            RuntimeGlobal { name: "phase".to_string(), value: WasmValue::I32(-1) }],
        ContinuationPoint { entrypoint: "continue".to_string(),
            arguments: vec![WasmValue::I32(21), WasmValue::I64(-34)] },
        vec![NamespaceCell { key: b"counter".to_vec(), value: 42u64.to_be_bytes().to_vec() },
            NamespaceCell { key: b"result".to_vec(), value: b"continued".to_vec() }])
        .unwrap_or_else(|error| panic!("state: {error}"))
}

fn meter() -> Meter {
    Meter::new(ResourceBudget::new_complete(1_000, 1 << 20, 1 << 20, 1 << 20,
        100, 1 << 20, 100), FeeSchedule::declared())
}

fn continuation_module() -> Vec<u8> {
    let global_section = raw_section(6, &[1, TYPE_I32, 1, OP_I32_CONST, 0, OP_END]);
    let memory_section = raw_section(5, &[1, 0, 1]);
    let mut exports = unsigned_leb(4);
    for (name, kind, index) in [("seed", 0, 0), ("continue", 0, 1),
        ("memory", 2, 0), ("counter", 3, 0)] {
        exports.extend(unsigned_leb(name.len() as u64));
        exports.extend_from_slice(name.as_bytes());
        exports.push(kind);
        exports.extend(unsigned_leb(index));
    }
    module(&[
        type_section(&[(&[], &[TYPE_I32])]),
        function_section(&[0, 0]),
        memory_section,
        global_section,
        raw_section(7, &exports),
        code_section(&[
            func_body(&[], &[OP_I32_CONST, 41, 0x24, 0, OP_I32_CONST, 41, OP_END]),
            func_body(&[], &[0x23, 0, OP_I32_CONST, 1, OP_I32_ADD, 0x24, 0,
                0x23, 0, OP_END]),
        ]),
    ])
}

fn validated_continuation() -> layerx_programs_runtime::ValidatedModule {
    WasmEngine::new(ValidationLimits::declared())
        .unwrap_or_else(|error| panic!("engine: {error}"))
        .validate(&continuation_module())
        .unwrap_or_else(|error| panic!("validation: {error}"))
}

#[test]
fn snapshot_destroy_restore_and_continue_preserves_exact_execution_state() {
    let mut source = active(1, 9);
    let module = validated_continuation();
    let mut uninterrupted_instance = module.instantiate()
        .unwrap_or_else(|error| panic!("uninterrupted instance: {error}"));
    assert_eq!(uninterrupted_instance.call("seed", &[]), Ok(vec![WasmValue::I32(41)]));
    let runtime = uninterrupted_instance.capture_continuation("continue", &[])
        .unwrap_or_else(|error| panic!("capture: {error}"));
    let uninterrupted = SandboxState::from_runtime(&source, runtime,
        vec![NamespaceCell { key: b"counter".to_vec(), value: 41u64.to_be_bytes().to_vec() }])
        .unwrap_or_else(|error| panic!("sandbox state: {error}"));
    let mut snapshot_meter = meter();
    let source_tenant = source.tenant();
    let snapshot = Snapshot::commit(&mut source, &uninterrupted, source_tenant, &mut snapshot_meter)
        .unwrap_or_else(|error| panic!("snapshot: {error}"));
    let snapshot_usage = snapshot_meter.finish().unwrap_or_else(|error| panic!("usage: {error}"));
    assert_eq!(snapshot_usage.storage_write_bytes, snapshot.byte_length());
    assert_eq!(source.snapshot_records().len(), 1);
    assert_eq!(source.snapshot_records()[0].digest(), snapshot.digest());
    assert_eq!(source.snapshot_records()[0].namespace(), source.namespace());
    assert_eq!(source.snapshot_records()[0].canonical_state(),
        uninterrupted.canonical_bytes().unwrap_or_else(|error| panic!("canonical: {error}")));

    let destroyed = advance_existing(source, &[
        (LeaseActivity::BeginSettlement, LeaseState::Active, LeaseState::Settling, 13),
        (LeaseActivity::Expire, LeaseState::Settling, LeaseState::Expired, 14),
        (LeaseActivity::Destroy, LeaseState::Expired, LeaseState::Destroyed, 15),
    ]);
    assert_eq!(destroyed.state(), LeaseState::Destroyed);
    assert_eq!(destroyed.snapshot_records().len(), 1);
    assert_eq!(destroyed.snapshot_records()[0].digest(), snapshot.digest());
    assert_eq!(destroyed.snapshot_records()[0].namespace(), destroyed.namespace());
    assert!(destroyed.snapshot_records()[0].canonical_state().is_empty());
    let mut target = funded(2, 9);
    let (activate, activation_evidence) = activation(&target, 22);
    let mut restore_meter = meter();
    let target_tenant = target.tenant();
    let prepared = restore(&mut target, &snapshot, uninterrupted.clone(), target_tenant,
        &mut restore_meter, activate, activation_evidence)
        .unwrap_or_else(|error| panic!("restore: {error}"));
    let restored = prepared.state();
    assert_eq!(restored.linear_memory(), uninterrupted.linear_memory());
    assert_eq!(restored.globals(), uninterrupted.globals());
    assert_eq!(restored.continuation(), uninterrupted.continuation());
    assert_eq!(restored.namespace_cells(), uninterrupted.namespace_cells());
    assert_eq!(restored.source_lease(), target.id());
    assert_eq!(restored.source_namespace(), target.namespace());
    assert_eq!(target.state(), LeaseState::Active);
    assert_eq!(target.restored_from(), Some(snapshot.digest()));
    let restore_usage = restore_meter.finish().unwrap_or_else(|error| panic!("usage: {error}"));
    assert_eq!(restore_usage.storage_write_bytes, snapshot.byte_length());

    let uninterrupted_before = uninterrupted_instance.meter().finish()
        .unwrap_or_else(|error| panic!("uninterrupted before: {error}"));
    let uninterrupted_output = uninterrupted_instance.call("continue", &[])
        .unwrap_or_else(|error| panic!("uninterrupted continuation: {error}"));
    let uninterrupted_after = uninterrupted_instance.meter().finish()
        .unwrap_or_else(|error| panic!("uninterrupted after: {error}"));
    let mut restored_instance = module.instantiate()
        .unwrap_or_else(|error| panic!("restored instance: {error}"));
    let restored_before = restored_instance.meter().finish()
        .unwrap_or_else(|error| panic!("restored before: {error}"));
    let restored_output = restored_instance.restore_continuation(&restored.runtime_continuation())
        .unwrap_or_else(|error| panic!("restored continuation: {error}"));
    let restored_usage = restored_instance.meter().finish()
        .unwrap_or_else(|error| panic!("restored runtime usage: {error}"));
    assert_eq!(restored_output, uninterrupted_output);
    assert_eq!(restored_output, vec![WasmValue::I32(42)]);
    let uninterrupted_delta = (
        uninterrupted_after.cpu_fuel - uninterrupted_before.cpu_fuel,
        uninterrupted_after.memory_bytes - uninterrupted_before.memory_bytes,
        uninterrupted_after.storage_read_bytes - uninterrupted_before.storage_read_bytes,
        uninterrupted_after.storage_write_bytes - uninterrupted_before.storage_write_bytes,
        uninterrupted_after.output_values - uninterrupted_before.output_values,
        uninterrupted_after.output_bytes - uninterrupted_before.output_bytes,
    );
    let restored_delta = (
        restored_usage.cpu_fuel - restored_before.cpu_fuel,
        restored_usage.memory_bytes - restored_before.memory_bytes,
        restored_usage.storage_read_bytes - restored_before.storage_read_bytes,
        restored_usage.storage_write_bytes - restored_before.storage_write_bytes,
        restored_usage.output_values - restored_before.output_values,
        restored_usage.output_bytes - restored_before.output_bytes,
    );
    assert_eq!(restored_delta, uninterrupted_delta);
}

#[test]
fn ownership_digest_and_target_bindings_are_refused_before_metering() {
    let mut source = active(3, 9);
    let canonical = state(&source);
    let mut commit_meter = meter();
    let source_tenant = source.tenant();
    let snapshot = Snapshot::commit(&mut source, &canonical, source_tenant, &mut commit_meter)
        .unwrap_or_else(|error| panic!("snapshot: {error}"));
    let mut target = funded(4, 9);
    let intruder = PrincipalId::new([8; 32]).unwrap_or_else(|error| panic!("intruder: {error}"));
    let mut intruder_meter = meter();
    let (activate, activation_evidence) = activation(&target, 22);
    assert_eq!(restore(&mut target, &snapshot, canonical.clone(), intruder,
        &mut intruder_meter, activate, activation_evidence),
        Err(SnapshotRefusal::NotSnapshotOwner));
    assert_eq!(intruder_meter.finish().map(|usage| usage.storage_write_bytes), Ok(0));

    let mut altered = canonical.clone();
    let replacement = SandboxState::new(&source, altered.linear_memory().to_vec(),
        altered.globals().to_vec(), altered.continuation().clone(),
        vec![NamespaceCell { key: b"counter".to_vec(), value: 43u64.to_be_bytes().to_vec() }])
        .unwrap_or_else(|error| panic!("altered: {error}"));
    altered = replacement;
    let mut mismatch_meter = meter();
    let (activate, activation_evidence) = activation(&target, 22);
    let target_tenant = target.tenant();
    assert_eq!(restore(&mut target, &snapshot, altered, target_tenant, &mut mismatch_meter,
        activate, activation_evidence),
        Err(SnapshotRefusal::DigestMismatch));
    assert_eq!(mismatch_meter.finish().map(|usage| usage.storage_write_bytes), Ok(0));

    let mut foreign = funded(5, 8);
    let mut foreign_meter = meter();
    let (activate, activation_evidence) = activation(&foreign, 22);
    let foreign_tenant = foreign.tenant();
    assert_eq!(restore(&mut foreign, &snapshot, canonical, foreign_tenant, &mut foreign_meter,
        activate, activation_evidence),
        Err(SnapshotRefusal::NotSnapshotOwner));
}

#[test]
fn canonical_namespace_order_is_part_of_snapshot_admission() {
    let source = active(6, 9);
    assert_eq!(SandboxState::new(&source, Vec::new(), Vec::new(),
        ContinuationPoint { entrypoint: "continue".to_string(), arguments: Vec::new() },
        vec![NamespaceCell { key: b"z".to_vec(), value: vec![1] },
            NamespaceCell { key: b"a".to_vec(), value: vec![2] }]),
        Err(SnapshotRefusal::NonCanonicalNamespace));
}

#[test]
fn canonical_integer_discriminants_distinguish_runtime_value_widths() {
    let source = active(7, 9);
    let i32_state = SandboxState::new(&source, Vec::new(),
        vec![RuntimeGlobal { name: "value".to_string(), value: WasmValue::I32(-1) }],
        ContinuationPoint { entrypoint: "continue".to_string(),
            arguments: vec![WasmValue::I64(1)] }, Vec::new())
        .unwrap_or_else(|error| panic!("i32 state: {error}"));
    let i64_state = SandboxState::new(&source, Vec::new(),
        vec![RuntimeGlobal { name: "value".to_string(), value: WasmValue::I64(-1) }],
        ContinuationPoint { entrypoint: "continue".to_string(),
            arguments: vec![WasmValue::I32(1)] }, Vec::new())
        .unwrap_or_else(|error| panic!("i64 state: {error}"));
    assert_ne!(i32_state.canonical_bytes(), i64_state.canonical_bytes());
    assert_ne!(i32_state.digest(), i64_state.digest());
}
