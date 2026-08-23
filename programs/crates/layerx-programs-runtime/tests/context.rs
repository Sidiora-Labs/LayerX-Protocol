//! Execution-context ABI: golden freeze of the field identifiers and their
//! encodings, and end-to-end proof that every field is derived from host-fixed
//! protocol state. The caller field is proven honest: at the activity's entry
//! frame it is absent, and the field-addressed ABI offers guest code no channel
//! through which to supply or spoof an identity. Frame-to-frame caller honesty
//! at depth and across re-entry is proven by the crate-internal call-graph
//! tests in `calls.rs`, which exercise the exact derivation this host function
//! reads.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::expect_used,
    clippy::unwrap_used
)]

use layerx_programs_runtime::abi::context::canonical_field_manifest;
use layerx_programs_runtime::abi::response::CANDIDATE_ABI_MODULE;
use layerx_programs_runtime::test_support::{
    code_section, func_body, function_section, import_section, module, type_section, unsigned_leb,
    TYPE_I32,
};
use layerx_programs_runtime::{
    AbiError, AuthorizationContext, AuthorizedExecutionRequest, CapabilitySet, CompositionContext,
    ContextField, ExecutionContext, Executor, PrincipalId, ProgramId, ReceiptOracle, ReceiptView,
    Storage, WasmEngine, ABI_VERSION, CALL_ENTRY_EXPORT, RUNTIME_VERSION,
};

const CONTEXT_V1_GOLDEN: &str = include_str!("../vectors/context-v1.hex");

/// The status the host returns for an unknown field identifier. Frozen here so
/// a refusal can never silently become a zero-length success.
const STATUS_INVALID: i32 = -2;

struct NoReceipts;
impl ReceiptOracle for NoReceipts {
    fn verified_receipt(&self, _: [u8; 32]) -> Result<ReceiptView, AbiError> {
        Err(AbiError::ReceiptMismatch)
    }
}

fn decode_hex(encoded: &str) -> Vec<u8> {
    let compact: Vec<u8> = encoded
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    assert_eq!(compact.len() % 2, 0, "golden vector has an odd hex length");
    compact
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
}

fn nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        other => panic!("golden vector contains non-hex byte {other}"),
    }
}

fn program(byte: u8) -> ProgramId {
    ProgramId::new([byte; 32]).unwrap_or_else(|error| panic!("program id: {error}"))
}

fn principal(byte: u8) -> PrincipalId {
    PrincipalId::new([byte; 32]).unwrap_or_else(|error| panic!("principal id: {error}"))
}

fn memory_section() -> Vec<u8> {
    let mut payload = vec![1];
    payload.extend_from_slice(&[1, 1, 1]);
    let mut encoded = vec![5];
    encoded.extend(unsigned_leb(payload.len() as u64));
    encoded.extend_from_slice(&payload);
    encoded
}

fn export_section_with_memory(functions: &[(&str, u32)]) -> Vec<u8> {
    let mut payload = unsigned_leb((functions.len() + 1) as u64);
    for (name, index) in functions {
        payload.extend(unsigned_leb(name.len() as u64));
        payload.extend_from_slice(name.as_bytes());
        payload.extend_from_slice(&[0, *index as u8]);
    }
    payload.extend(unsigned_leb("memory".len() as u64));
    payload.extend_from_slice(b"memory");
    payload.extend_from_slice(&[2, 0]);
    let mut encoded = vec![7];
    encoded.extend(unsigned_leb(payload.len() as u64));
    encoded.extend_from_slice(&payload);
    encoded
}

fn small_const(value: i32) -> [u8; 2] {
    assert!((0..64).contains(&value), "helper only encodes small consts");
    [0x41, value as u8]
}

/// Builds a candidate module whose `layerx_call` reads one context field into
/// linear memory and publishes exactly its bytes as the activity response.
fn context_reader(field: ContextField) -> Vec<u8> {
    let byte_len = field.encoded_len() as i32;
    let mut call_body = Vec::new();
    call_body.extend_from_slice(&small_const(field.id() as i32));
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(byte_len));
    call_body.extend_from_slice(&[0x10, 0, 0x1a]);
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(byte_len));
    call_body.extend_from_slice(&[0x10, 1, 0x1a]);
    call_body.extend_from_slice(&[0x41, 0, 0x0b]);
    module(&[
        type_section(&[
            (&[TYPE_I32, TYPE_I32, TYPE_I32], &[TYPE_I32]),
            (&[TYPE_I32], &[TYPE_I32]),
            (&[TYPE_I32, TYPE_I32], &[TYPE_I32]),
        ]),
        import_section(&[
            (CANDIDATE_ABI_MODULE, "context_read", 0),
            (CANDIDATE_ABI_MODULE, "response_write", 0),
        ]),
        function_section(&[1, 2]),
        memory_section(),
        export_section_with_memory(&[("layerx_reserve", 2), (CALL_ENTRY_EXPORT, 3)]),
        code_section(&[func_body(&[], &[0x41, 0, 0x0b]), func_body(&[], &call_body)]),
    ])
}

/// Builds a candidate module whose `layerx_call` reads an unknown field, stores
/// the returned status word into linear memory and publishes it, so a refusal
/// can be observed rather than mistaken for data.
fn unknown_field_probe(field_id: i32) -> Vec<u8> {
    let mut call_body = Vec::new();
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(field_id));
    call_body.extend_from_slice(&small_const(4));
    call_body.extend_from_slice(&small_const(64));
    call_body.extend_from_slice(&[0x10, 0]);
    call_body.extend_from_slice(&[0x36, 0x02, 0x00]);
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(0));
    call_body.extend_from_slice(&small_const(4));
    call_body.extend_from_slice(&[0x10, 1, 0x1a]);
    call_body.extend_from_slice(&[0x41, 0, 0x0b]);
    module(&[
        type_section(&[
            (&[TYPE_I32, TYPE_I32, TYPE_I32], &[TYPE_I32]),
            (&[TYPE_I32], &[TYPE_I32]),
            (&[TYPE_I32, TYPE_I32], &[TYPE_I32]),
        ]),
        import_section(&[
            (CANDIDATE_ABI_MODULE, "context_read", 0),
            (CANDIDATE_ABI_MODULE, "response_write", 0),
        ]),
        function_section(&[1, 2]),
        memory_section(),
        export_section_with_memory(&[("layerx_reserve", 2), (CALL_ENTRY_EXPORT, 3)]),
        code_section(&[func_body(&[], &[0x41, 0, 0x0b]), func_body(&[], &call_body)]),
    ])
}

fn read_field(
    executing: ProgramId,
    invoker: PrincipalId,
    context: ExecutionContext,
    field: ContextField,
) -> Vec<u8> {
    let wasm = context_reader(field);
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    let module = engine
        .validate_candidate_v2(&wasm)
        .unwrap_or_else(|error| panic!("candidate validation for {}: {error}", field.name()));
    let capabilities = CapabilitySet::new([]).unwrap_or_else(|error| panic!("capabilities: {error}"));
    let record = Executor::declared()
        .execute_authorized_candidate(
            &mut Storage::new(),
            AuthorizedExecutionRequest {
                module: &module,
                program: executing,
                authorization: AuthorizationContext::new(invoker, capabilities),
                receipts: &NoReceipts,
                entrypoint: CALL_ENTRY_EXPORT,
                calldata: &[],
                composition: CompositionContext::isolated().with_execution_context(context),
                response_capacity: field.encoded_len(),
            },
        )
        .unwrap_or_else(|error| panic!("candidate execution for {}: {error}", field.name()));
    record
        .response()
        .unwrap_or_else(|| panic!("no response for {}", field.name()))
        .bytes
        .clone()
}

#[test]
fn context_field_manifest_matches_golden_vector() {
    assert_eq!(canonical_field_manifest(), decode_hex(CONTEXT_V1_GOLDEN));
}

#[test]
fn field_identifiers_and_encoded_lengths_are_frozen() {
    let frozen: [(ContextField, u32, &str, usize); 9] = [
        (ContextField::ExecutingProgram, 1, "executing_program", 32),
        (ContextField::CallingProgram, 2, "calling_program", 33),
        (ContextField::InvokingPrincipal, 3, "invoking_principal", 32),
        (ContextField::ActivitySequence, 4, "activity_sequence", 8),
        (ContextField::BatchHeight, 5, "batch_height", 8),
        (ContextField::RuntimeVersion, 6, "runtime_version", 2),
        (ContextField::AbiVersion, 7, "abi_version", 2),
        (ContextField::RemainingFuel, 8, "remaining_fuel", 8),
        (ContextField::FeeScheduleVersion, 9, "fee_schedule_version", 4),
    ];
    for (field, id, name, len) in frozen {
        assert_eq!(field.id(), id, "{name} id");
        assert_eq!(field.name(), name, "{name} name");
        assert_eq!(field.encoded_len(), len, "{name} length");
        assert_eq!(ContextField::from_id(id), Some(field), "{name} round trip");
    }
}

#[test]
fn sample_field_encodings_are_frozen() {
    let executing = program(0x11);
    let caller = program(0x22);
    let invoker = principal(0x33);
    let context = ExecutionContext::new(0x0102_0304_0506_0708, 0x1112_1314_1516_1718, 1, 1, 1);

    assert_eq!(
        context.encode_field(ContextField::ExecutingProgram, executing, Some(caller), invoker, 5),
        [0x11; 32].to_vec()
    );
    let mut expected_caller = vec![1u8];
    expected_caller.extend_from_slice(&[0x22; 32]);
    assert_eq!(
        context.encode_field(ContextField::CallingProgram, executing, Some(caller), invoker, 5),
        expected_caller
    );
    assert_eq!(
        context.encode_field(ContextField::InvokingPrincipal, executing, Some(caller), invoker, 5),
        [0x33; 32].to_vec()
    );
    assert_eq!(
        context.encode_field(ContextField::ActivitySequence, executing, Some(caller), invoker, 5),
        vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
    );
    assert_eq!(
        context.encode_field(ContextField::BatchHeight, executing, Some(caller), invoker, 5),
        vec![0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]
    );
    assert_eq!(
        context.encode_field(ContextField::RuntimeVersion, executing, Some(caller), invoker, 5),
        vec![0x00, 0x01]
    );
    assert_eq!(
        context.encode_field(ContextField::AbiVersion, executing, Some(caller), invoker, 5),
        vec![0x00, 0x01]
    );
    assert_eq!(
        context.encode_field(ContextField::RemainingFuel, executing, Some(caller), invoker, 5),
        vec![0, 0, 0, 0, 0, 0, 0, 5]
    );
    assert_eq!(
        context.encode_field(ContextField::FeeScheduleVersion, executing, Some(caller), invoker, 5),
        vec![0x00, 0x00, 0x00, 0x01]
    );
}

#[test]
fn executing_program_field_reports_the_running_program() {
    let executing = program(0x42);
    let bytes = read_field(
        executing,
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::ExecutingProgram,
    );
    assert_eq!(bytes, executing.bytes().to_vec());
}

#[test]
fn calling_program_field_is_absent_at_the_entry_frame() {
    let bytes = read_field(
        program(0x42),
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::CallingProgram,
    );
    assert_eq!(bytes.len(), 33);
    assert_eq!(bytes[0], 0, "entry frame has no calling program");
    assert_eq!(&bytes[1..], &[0u8; 32]);
}

#[test]
fn invoking_principal_field_reports_the_activity_principal() {
    let invoker = principal(0x9a);
    let bytes = read_field(
        program(0x42),
        invoker,
        ExecutionContext::declared(),
        ContextField::InvokingPrincipal,
    );
    assert_eq!(bytes, invoker.bytes().to_vec());
}

#[test]
fn sequence_and_height_reflect_supplied_protocol_state() {
    let context = ExecutionContext::at(4242, 7777);
    let sequence = read_field(
        program(0x42),
        principal(0x9a),
        context,
        ContextField::ActivitySequence,
    );
    assert_eq!(sequence, 4242u64.to_be_bytes().to_vec());
    let height = read_field(
        program(0x42),
        principal(0x9a),
        context,
        ContextField::BatchHeight,
    );
    assert_eq!(height, 7777u64.to_be_bytes().to_vec());
}

#[test]
fn versions_reflect_the_runtime_abi_and_fee_schedule() {
    let runtime = read_field(
        program(0x42),
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::RuntimeVersion,
    );
    assert_eq!(runtime, RUNTIME_VERSION.to_be_bytes().to_vec());
    let abi = read_field(
        program(0x42),
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::AbiVersion,
    );
    assert_eq!(abi, ABI_VERSION.to_be_bytes().to_vec());
    let schedule = read_field(
        program(0x42),
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::FeeScheduleVersion,
    );
    assert_eq!(schedule, 1u32.to_be_bytes().to_vec());
}

#[test]
fn remaining_fuel_is_bounded_by_the_declared_budget() {
    let bytes = read_field(
        program(0x42),
        principal(0x9a),
        ExecutionContext::declared(),
        ContextField::RemainingFuel,
    );
    assert_eq!(bytes.len(), 8);
    let remaining = u64::from_be_bytes(bytes.try_into().expect("eight fuel bytes"));
    assert!(remaining > 0, "some fuel must remain mid-execution");
    assert!(
        remaining <= 1_000_000,
        "remaining fuel cannot exceed the declared budget"
    );
}

#[test]
fn unknown_field_identifier_is_refused_not_zeroed() {
    let wasm = unknown_field_probe(0);
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    let module = engine
        .validate_candidate_v2(&wasm)
        .unwrap_or_else(|error| panic!("candidate validation: {error}"));
    let capabilities = CapabilitySet::new([]).unwrap_or_else(|error| panic!("capabilities: {error}"));
    let record = Executor::declared()
        .execute_authorized_candidate(
            &mut Storage::new(),
            AuthorizedExecutionRequest {
                module: &module,
                program: program(0x42),
                authorization: AuthorizationContext::new(principal(0x9a), capabilities),
                receipts: &NoReceipts,
                entrypoint: CALL_ENTRY_EXPORT,
                calldata: &[],
                composition: CompositionContext::isolated(),
                response_capacity: 4,
            },
        )
        .unwrap_or_else(|error| panic!("candidate execution: {error}"));
    let bytes = record.response().expect("response").bytes.clone();
    assert_eq!(bytes.len(), 4);
    let status = i32::from_le_bytes(bytes.try_into().expect("four status bytes"));
    assert_eq!(status, STATUS_INVALID, "unknown field must be refused");
}
