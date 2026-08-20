use layerx_programs_runtime::test_support::{
    add_module, code_section, export_section, func_body, function_section, module, type_section,
    OP_END,
};
use layerx_programs_runtime::{
    ExecutionError, Executor, FeeSchedule, Meter, MeterRefusal, ResourceBudget, ResourceKind,
    ValidationLimits, WasmEngine, WasmValue, ABI_VERSION, RUNTIME_VERSION,
};

const EXECUTION_V1_GOLDEN: &str = include_str!("../vectors/execution-v1.hex");

fn validated_add() -> layerx_programs_runtime::ValidatedModule {
    let engine = match WasmEngine::new(ValidationLimits::declared()) {
        Ok(engine) => engine,
        Err(refusal) => panic!("engine construction refused: {refusal}"),
    };
    match engine.validate(&add_module()) {
        Ok(module) => module,
        Err(refusal) => panic!("module validation refused: {refusal}"),
    }
}

fn execute_add(left: i32, right: i32) -> layerx_programs_runtime::ExecutionRecord {
    match Executor::declared().execute(
        &validated_add(),
        "add",
        &[WasmValue::I32(left), WasmValue::I32(right)],
    ) {
        Ok(record) => record,
        Err(error) => panic!("execution failed: {error}"),
    }
}

fn one_page_memory_module() -> Vec<u8> {
    let memory_section = vec![5, 3, 1, 0, 1];
    module(&[
        type_section(&[(&[], &[])]),
        function_section(&[0]),
        memory_section,
        export_section(&[("run", 0)]),
        code_section(&[func_body(&[], &[OP_END])]),
    ])
}

fn decode_hex(encoded: &str) -> Vec<u8> {
    let bytes = encoded.trim().as_bytes();
    assert_eq!(bytes.len() % 2, 0, "golden vector has an odd hex length");
    bytes
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("golden vector contains non-hex byte {byte}"),
    }
}

#[test]
fn execution_evidence_matches_the_architecture_independent_golden() {
    let record = execute_add(19, 23);
    assert_eq!(record.runtime_version, RUNTIME_VERSION);
    assert_eq!(record.abi_version, ABI_VERSION);
    assert_eq!(record.outputs, vec![WasmValue::I32(42)]);
    assert_eq!(record.canonical_evidence(), decode_hex(EXECUTION_V1_GOLDEN));
}

#[test]
fn equal_executions_consume_equal_budgets() {
    let values = [i32::MIN, -65_537, -1, 0, 1, 65_537, i32::MAX];
    for left in values {
        for right in values {
            let first = execute_add(left, right);
            let second = execute_add(left, right);
            assert_eq!(first, second, "metering diverged for {left} + {right}");
            assert_eq!(first.canonical_evidence(), second.canonical_evidence());
        }
    }
}

#[test]
fn cpu_exhaustion_is_a_typed_resource_result() {
    let executor = Executor::new(
        ResourceBudget::new(1, 65_536, 1_024, 1_024, 1, 1),
        FeeSchedule::declared(),
    );
    let result = executor.execute(
        &validated_add(),
        "add",
        &[WasmValue::I32(19), WasmValue::I32(23)],
    );
    assert!(matches!(
        result,
        Err(ExecutionError::Resource(MeterRefusal::BudgetExceeded {
            resource: ResourceKind::Cpu,
            limit: 1,
            ..
        }))
    ));
}

#[test]
fn initial_memory_exhaustion_is_a_typed_resource_result() {
    let engine = match WasmEngine::new(ValidationLimits::declared()) {
        Ok(engine) => engine,
        Err(refusal) => panic!("engine construction refused: {refusal}"),
    };
    let module = match engine.validate(&one_page_memory_module()) {
        Ok(module) => module,
        Err(refusal) => panic!("module validation refused: {refusal}"),
    };
    let executor = Executor::new(
        ResourceBudget::new(1_000, 65_535, 1_024, 1_024, 1, 1),
        FeeSchedule::declared(),
    );
    let result = executor.execute(&module, "run", &[]);
    assert_eq!(
        result,
        Err(ExecutionError::Resource(MeterRefusal::BudgetExceeded {
            resource: ResourceKind::Memory,
            limit: 65_535,
            attempted: 65_536,
        }))
    );
}

#[test]
fn storage_accounting_is_integer_exact_and_repeatable() {
    let budget = ResourceBudget::new(1_000, 65_536, 10, 10, 1, 1);
    let prices = FeeSchedule::new(1, 2, 3, 5, 7);
    let mut first = Meter::new(budget, prices);
    let mut second = Meter::new(budget, prices);
    for meter in [&mut first, &mut second] {
        assert_eq!(meter.charge_storage_read(4), Ok(()));
        assert_eq!(meter.charge_storage_write(6), Ok(()));
    }
    let first_usage = first.finish();
    let second_usage = second.finish();
    assert_eq!(first_usage, second_usage);
    let usage = match first_usage {
        Ok(usage) => usage,
        Err(refusal) => panic!("meter finalization refused: {refusal}"),
    };
    assert_eq!(usage.storage_read_bytes, 4);
    assert_eq!(usage.storage_write_bytes, 6);
    assert_eq!(usage.fee_units, 42);
}
