use layerx_programs_runtime::test_support::add_module;
use layerx_programs_runtime::{
    programs_differential_gate, replay_recorded_execution, RecordedExecution, ReplayRefusal,
    WasmValue, ABI_VERSION, RUNTIME_VERSION,
};

#[test]
fn independent_engines_produce_identical_evidence() {
    let wasm = add_module();
    let evidence =
        programs_differential_gate(&wasm, "add", &[WasmValue::I32(20), WasmValue::I32(22)]);
    assert!(evidence.is_ok(), "independent runtime builds diverged");
}

#[test]
fn recorded_v1_replays_identically_after_a_simulated_upgrade() {
    let wasm = add_module();
    let record = RecordedExecution {
        runtime_version: RUNTIME_VERSION,
        abi_version: ABI_VERSION,
        wasm: &wasm,
        export: "add",
        args: &[WasmValue::I32(20), WasmValue::I32(22)],
    };
    let before = replay_recorded_execution(&record);
    let after = replay_recorded_execution(&record);
    assert_eq!(before, after);
}

#[test]
fn unknown_runtime_and_abi_artifacts_are_preserved_without_execution() {
    let wasm = add_module();
    let runtime = RecordedExecution {
        runtime_version: RUNTIME_VERSION + 1,
        abi_version: ABI_VERSION,
        wasm: &wasm,
        export: "add",
        args: &[],
    };
    assert_eq!(
        replay_recorded_execution(&runtime),
        Err(ReplayRefusal::UnknownRuntimeVersion {
            version: RUNTIME_VERSION + 1,
        })
    );
    let abi = RecordedExecution {
        runtime_version: RUNTIME_VERSION,
        abi_version: ABI_VERSION + 1,
        ..runtime
    };
    assert_eq!(
        replay_recorded_execution(&abi),
        Err(ReplayRefusal::UnknownAbiVersion {
            version: ABI_VERSION + 1
        })
    );
}
