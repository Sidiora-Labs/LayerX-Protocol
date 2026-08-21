use layerx_programs_runtime::test_support::{
    code_section, func_body, function_section, import_section, module, type_section, unsigned_leb,
    OP_CALL, OP_END, OP_I32_CONST, TYPE_I32, TYPE_I64,
};
use layerx_programs_runtime::{
    Abi, AbiError, AuthorizationContext, AuthorizedExecutionRequest, Capability, CapabilitySet,
    CompositionContext, Executor, PrincipalId, ProgramId, ReceiptOracle, ReceiptView, Storage,
    StorageNamespace, ValidationRefusal, WasmEngine, WasmValue, ABI_MODULE, ABI_VERSION,
    HOST_FUNCTIONS,
};

const ABI_V1_GOLDEN: &str = include_str!("../vectors/abi-v1.hex");
#[derive(Debug)]
struct NoReceipts;

impl ReceiptOracle for NoReceipts {
    fn verified_receipt(&self, _receipt_digest: [u8; 32]) -> Result<ReceiptView, AbiError> {
        Err(AbiError::ReceiptMismatch)
    }
}

fn ids(program: u8, principal: u8) -> (ProgramId, PrincipalId) {
    let program = ProgramId::new([program; 32]).unwrap_or_else(|error| panic!("program: {error}"));
    let principal =
        PrincipalId::new([principal; 32]).unwrap_or_else(|error| panic!("principal: {error}"));
    (program, principal)
}

fn section(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut encoded = vec![id];
    encoded.extend(unsigned_leb(payload.len() as u64));
    encoded.extend_from_slice(payload);
    encoded
}

fn memory_and_data(value: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let memory = vec![5, 3, 1, 0, 1];
    let mut contents = b"key".to_vec();
    contents.extend_from_slice(value);
    let mut data_payload = vec![1, 0, OP_I32_CONST, 0, OP_END];
    data_payload.extend(unsigned_leb(contents.len() as u64));
    data_payload.extend(contents);
    (memory, section(11, &data_payload))
}

fn storage_module(function: &str, pointer: i32, value: &[u8]) -> Vec<u8> {
    let (memory, data) = memory_and_data(value);
    let export_payload = vec![
        2, 3, b'r', b'u', b'n', 0, 1, 6, b'm', b'e', b'm', b'o', b'r', b'y', 2, 0,
    ];
    let mut instructions = vec![OP_I32_CONST];
    instructions.extend(signed_leb_i32(pointer));
    instructions.extend([OP_I32_CONST, 3]);
    if function == "storage_write" {
        instructions.extend([OP_I32_CONST, 3, OP_I32_CONST]);
        instructions.extend(signed_leb_i32(
            i32::try_from(value.len()).unwrap_or_else(|error| panic!("value length: {error}")),
        ));
    } else {
        instructions.extend([OP_I32_CONST, 16, OP_I32_CONST, 16]);
    }
    instructions.extend([OP_CALL, 0, OP_END]);
    module(&[
        type_section(&[
            (&[TYPE_I32, TYPE_I32, TYPE_I32, TYPE_I32], &[TYPE_I32]),
            (&[], &[TYPE_I32]),
        ]),
        import_section(&[(ABI_MODULE, function, 0)]),
        function_section(&[1]),
        memory,
        section(7, &export_payload),
        code_section(&[func_body(&[], &instructions)]),
        data,
    ])
}

fn signed_leb_i32(value: i32) -> Vec<u8> {
    let mut value = i64::from(value);
    let mut bytes = Vec::new();
    loop {
        let byte = match u8::try_from(value & 0x7f) {
            Ok(byte) => byte,
            Err(error) => panic!("signed LEB byte conversion failed: {error}"),
        };
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        bytes.push(if done { byte } else { byte | 0x80 });
        if done {
            return bytes;
        }
    }
}

fn execute(
    wasm: &[u8],
    storage: &mut Storage,
    program: ProgramId,
    principal: PrincipalId,
    grants: impl IntoIterator<Item = Capability>,
) -> layerx_programs_runtime::AuthorizedExecutionRecord {
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    let module = engine
        .validate(wasm)
        .unwrap_or_else(|error| panic!("validation: {error}"));
    let capabilities =
        CapabilitySet::new(grants).unwrap_or_else(|error| panic!("capabilities: {error}"));
    Executor::declared()
        .execute_authorized(
            storage,
            AuthorizedExecutionRequest {
                module: &module,
                program,
                authorization: AuthorizationContext::new(principal, capabilities),
                receipts: &NoReceipts,
                export: "run",
                args: &[],
                composition: CompositionContext::isolated(),
            },
        )
        .unwrap_or_else(|error| panic!("execution: {error}"))
}

fn decode_hex(encoded: &str) -> Vec<u8> {
    let bytes = encoded.trim().as_bytes();
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

fn validation_refusal(engine: &WasmEngine, wasm: &[u8]) -> ValidationRefusal {
    match engine.validate(wasm) {
        Ok(_) => panic!("module unexpectedly validated"),
        Err(refusal) => refusal,
    }
}

fn assert_namespace(storage: &mut Storage, namespace: StorageNamespace, value: &[u8]) {
    assert_eq!(storage.namespace_cell_count(namespace), 1);
    assert_eq!(
        storage.namespace_persistent_bytes(namespace),
        Ok(u64::try_from(3 + value.len())
            .unwrap_or_else(|error| panic!("namespace bytes: {error}")))
    );
    assert_eq!(
        storage.transaction(namespace).read(b"key"),
        Ok(Some(value.to_vec()))
    );
}

fn nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("non-hex golden byte"),
    }
}

#[test]
fn abi_v1_manifest_matches_typed_declarations_and_golden() {
    let mut regenerated = Vec::new();
    regenerated.extend_from_slice(ABI_MODULE.as_bytes());
    regenerated.push(0);
    for function in HOST_FUNCTIONS {
        regenerated.extend_from_slice(function.name.as_bytes());
        regenerated.extend_from_slice(function.signature.as_bytes());
        regenerated.push(0);
    }
    assert_eq!(regenerated, Abi::canonical_manifest());
    let mut versioned = ABI_VERSION.to_be_bytes().to_vec();
    versioned.extend_from_slice(&regenerated);
    assert_eq!(versioned, decode_hex(ABI_V1_GOLDEN));
}

#[test]
fn exact_seven_function_surface_validates_and_instantiates() {
    let i32x4 = &[TYPE_I32; 4];
    let i32x2 = &[TYPE_I32; 2];
    let i32x6 = &[TYPE_I32; 6];
    let transfer = &[TYPE_I64, TYPE_I64, TYPE_I32, TYPE_I32, TYPE_I32, TYPE_I32];
    let wasm = module(&[
        type_section(&[
            (i32x4, &[TYPE_I32]),
            (i32x2, &[TYPE_I32]),
            (i32x6, &[TYPE_I32]),
            (transfer, &[TYPE_I32]),
        ]),
        import_section(&[
            (ABI_MODULE, "storage_read", 0),
            (ABI_MODULE, "storage_write", 0),
            (ABI_MODULE, "storage_delete", 1),
            (ABI_MODULE, "event_emit", 0),
            (ABI_MODULE, "program_call", 2),
            (ABI_MODULE, "transfer_402", 3),
            (ABI_MODULE, "receipt_read", 0),
        ]),
    ]);
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    let validated = engine
        .validate(&wasm)
        .unwrap_or_else(|error| panic!("validation: {error}"));
    validated
        .instantiate()
        .unwrap_or_else(|error| panic!("instantiation: {error}"));
}

#[test]
fn wrong_signature_is_rejected_during_validation() {
    let wasm = module(&[
        type_section(&[(&[TYPE_I32], &[TYPE_I32])]),
        import_section(&[(ABI_MODULE, "storage_read", 0)]),
    ]);
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    assert_eq!(
        validation_refusal(&engine, &wasm),
        ValidationRefusal::WrongImportSignature {
            import_name: "storage_read".into()
        }
    );
}

#[test]
fn unknown_and_ambient_kernel_imports_are_rejected() {
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    for (module_name, import_name) in [(ABI_MODULE, "kernel_state"), ("env", "kernel_state")] {
        let wasm = module(&[
            type_section(&[(&[], &[TYPE_I32])]),
            import_section(&[(module_name, import_name, 0)]),
        ]);
        assert_eq!(
            validation_refusal(&engine, &wasm),
            ValidationRefusal::ForbiddenImport {
                import_module: module_name.into(),
                import_name: import_name.into(),
            }
        );
    }
}

#[test]
fn wrong_kind_is_rejected_during_validation() {
    let mut payload = vec![1, 9];
    payload.extend_from_slice(ABI_MODULE.as_bytes());
    payload.push(12);
    payload.extend_from_slice(b"storage_read");
    payload.extend([2, 0, 1]);
    let wasm = module(&[section(2, &payload)]);
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    assert_eq!(
        validation_refusal(&engine, &wasm),
        ValidationRefusal::WrongImportKind {
            import_name: "storage_read".into()
        }
    );
}

#[test]
fn denied_guest_storage_write_is_stable_and_has_no_effect() {
    let (program, principal) = ids(1, 2);
    let mut storage = Storage::new();
    let before = storage.clone();
    let record = execute(
        &storage_module("storage_write", 0, b"new"),
        &mut storage,
        program,
        principal,
        [],
    );
    assert_eq!(record.execution.outputs, vec![WasmValue::I32(-1)]);
    assert_eq!(record.execution.usage.storage_write_bytes, 0);
    assert!(record.effects.events.is_empty());
    assert_eq!(storage, before);
}

#[test]
fn real_guest_storage_is_scoped_by_program_and_principal() {
    let (program_a, principal_p) = ids(1, 7);
    let (program_b, _) = ids(2, 7);
    let (_, principal_q) = ids(1, 8);
    let mut storage = Storage::new();
    let written = execute(
        &storage_module("storage_write", 0, b"new"),
        &mut storage,
        program_a,
        principal_p,
        [Capability::StorageWrite],
    );
    assert_eq!(written.execution.outputs, vec![WasmValue::I32(0)]);
    assert_eq!(written.execution.usage.storage_write_bytes, 6);
    let foreign_program_write = execute(
        &storage_module("storage_write", 0, b"bee"),
        &mut storage,
        program_b,
        principal_p,
        [Capability::StorageWrite],
    );
    let foreign_principal_write = execute(
        &storage_module("storage_write", 0, b"queue"),
        &mut storage,
        program_a,
        principal_q,
        [Capability::StorageWrite],
    );
    assert_eq!(
        foreign_program_write.execution.outputs,
        vec![WasmValue::I32(0)]
    );
    assert_eq!(foreign_program_write.execution.usage.storage_write_bytes, 6);
    assert_eq!(
        foreign_principal_write.execution.outputs,
        vec![WasmValue::I32(0)]
    );
    assert_eq!(
        foreign_principal_write.execution.usage.storage_write_bytes,
        8
    );
    let owner = execute(
        &storage_module("storage_read", 0, b""),
        &mut storage,
        program_a,
        principal_p,
        [Capability::StorageRead],
    );
    let other_program = execute(
        &storage_module("storage_read", 0, b""),
        &mut storage,
        program_b,
        principal_p,
        [Capability::StorageRead],
    );
    let other_principal = execute(
        &storage_module("storage_read", 0, b""),
        &mut storage,
        program_a,
        principal_q,
        [Capability::StorageRead],
    );
    assert_eq!(owner.execution.outputs, vec![WasmValue::I32(4)]);
    assert_eq!(owner.execution.usage.storage_read_bytes, 6);
    assert_eq!(other_program.execution.outputs, vec![WasmValue::I32(4)]);
    assert_eq!(other_program.execution.usage.storage_read_bytes, 6);
    assert_eq!(other_principal.execution.outputs, vec![WasmValue::I32(6)]);
    assert_eq!(other_principal.execution.usage.storage_read_bytes, 8);
    let owner_namespace = StorageNamespace::new(program_a, principal_p);
    let program_namespace = StorageNamespace::new(program_b, principal_p);
    let principal_namespace = StorageNamespace::new(program_a, principal_q);
    assert_namespace(&mut storage, owner_namespace, b"new");
    assert_namespace(&mut storage, program_namespace, b"bee");
    assert_namespace(&mut storage, principal_namespace, b"queue");
    let mut replacement = storage.transaction(owner_namespace);
    replacement
        .write(b"key", b"value")
        .unwrap_or_else(|error| panic!("replacement: {error}"));
    assert_eq!(replacement.commit(), 1);
    assert_eq!(storage.namespace_persistent_bytes(owner_namespace), Ok(8));
}

#[test]
fn guest_memory_bounds_refusal_cannot_write_or_emit_effects() {
    let (program, principal) = ids(3, 4);
    let mut storage = Storage::new();
    let before = storage.clone();
    let record = execute(
        &storage_module("storage_write", 65_535, b"new"),
        &mut storage,
        program,
        principal,
        [Capability::StorageWrite],
    );
    assert_eq!(record.execution.outputs, vec![WasmValue::I32(-3)]);
    assert_eq!(record.execution.usage.storage_write_bytes, 0);
    assert_eq!(storage, before);
    assert!(record.effects.events.is_empty());
}

#[test]
fn capability_narrowing_rejects_missing_grants_and_limit_widening_without_effects() {
    let (program, principal) = ids(5, 6);
    let (callee, _) = ids(9, 6);
    let asset = [11; 32];
    let to = [12; 32];
    let parent = CapabilitySet::new([
        Capability::Call { program: callee },
        Capability::Transfer402 {
            asset,
            to,
            maximum_amount: 10,
        },
    ])
    .unwrap_or_else(|error| panic!("capabilities: {error}"));
    assert_eq!(
        parent.narrow([Capability::StorageRead]),
        Err(AbiError::CapabilityDenied)
    );
    assert_eq!(
        parent.narrow([Capability::Transfer402 {
            asset,
            to,
            maximum_amount: 11
        }]),
        Err(AbiError::CapabilityEscalation)
    );
    let mut abi = Abi::new(
        ABI_VERSION,
        program,
        AuthorizationContext::new(principal, parent),
        Storage::new(),
        &NoReceipts,
    )
    .unwrap_or_else(|error| panic!("abi: {error}"));
    assert_eq!(
        abi.call_program(callee, b"", [Capability::StorageRead]),
        Err(AbiError::CapabilityDenied)
    );
    assert!(abi.commit().effects.calls.is_empty());
}

#[test]
fn host_table_contains_seven_unique_names() {
    let mut names = std::collections::BTreeSet::new();
    assert_eq!(HOST_FUNCTIONS.len(), 7);
    for function in HOST_FUNCTIONS {
        assert!(names.insert(function.name));
    }
}
