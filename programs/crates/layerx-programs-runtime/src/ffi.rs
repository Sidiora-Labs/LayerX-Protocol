//! Scalar-only C ingress for the deterministic runtime. Avoiding raw pointers
//! keeps the consensus bridge inside the workspace's `unsafe_code = deny` gate.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::{Executor, WasmEngine};

const RESULT_OK: i32 = 0;
const RESULT_NON_CANONICAL: i32 = -3;
const RESULT_LENGTH_LIMIT: i32 = -5;
const RESULT_UNKNOWN_ACTIVITY: i32 = -106;
const RESULT_GAS_EXHAUSTED: i32 = -601;
const RESULT_FATAL_INVARIANT: i32 = -1001;
const MAX_MODULE_BYTES: usize = 1_048_576;
const MAX_HOOK_BYTES: usize = 1_024;

#[derive(Debug)]
struct PendingMigration {
    wasm: Vec<u8>,
    hook: Vec<u8>,
    expected_wasm: usize,
    expected_hook: usize,
}

static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);
static PENDING: OnceLock<Mutex<BTreeMap<u64, PendingMigration>>> = OnceLock::new();

fn pending() -> &'static Mutex<BTreeMap<u64, PendingMigration>> {
    PENDING.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Starts one bounded migration request. Zero means refusal.
#[no_mangle]
pub extern "C" fn layerx_programs_migration_begin(wasm_length: u32, hook_length: u16) -> u64 {
    let wasm_length = wasm_length as usize;
    let hook_length = hook_length as usize;
    if wasm_length == 0
        || wasm_length > MAX_MODULE_BYTES
        || hook_length == 0
        || hook_length > MAX_HOOK_BYTES
    {
        return 0;
    }
    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    let Ok(mut requests) = pending().lock() else {
        return 0;
    };
    requests.insert(
        handle,
        PendingMigration {
            wasm: Vec::with_capacity(wasm_length),
            hook: Vec::with_capacity(hook_length),
            expected_wasm: wasm_length,
            expected_hook: hook_length,
        },
    );
    handle
}

/// Appends one byte to the staged WASM module.
#[no_mangle]
pub extern "C" fn layerx_programs_migration_wasm_byte(handle: u64, byte: u8) -> i32 {
    append(handle, byte, false)
}

/// Appends one byte to the staged migration export name.
#[no_mangle]
pub extern "C" fn layerx_programs_migration_hook_byte(handle: u64, byte: u8) -> i32 {
    append(handle, byte, true)
}

fn append(handle: u64, byte: u8, hook: bool) -> i32 {
    let Ok(mut requests) = pending().lock() else {
        return RESULT_FATAL_INVARIANT;
    };
    let Some(request) = requests.get_mut(&handle) else {
        return RESULT_NON_CANONICAL;
    };
    let (bytes, expected) = if hook {
        (&mut request.hook, request.expected_hook)
    } else {
        (&mut request.wasm, request.expected_wasm)
    };
    if bytes.len() == expected {
        return RESULT_LENGTH_LIMIT;
    }
    bytes.push(byte);
    RESULT_OK
}

/// Validates and executes the staged migration under declared metering.
#[no_mangle]
pub extern "C" fn layerx_programs_migration_execute(handle: u64) -> i32 {
    let request = {
        let Ok(mut requests) = pending().lock() else {
            return RESULT_FATAL_INVARIANT;
        };
        let Some(request) = requests.remove(&handle) else {
            return RESULT_NON_CANONICAL;
        };
        request
    };
    if request.wasm.len() != request.expected_wasm || request.hook.len() != request.expected_hook {
        return RESULT_NON_CANONICAL;
    }
    let Ok(hook) = String::from_utf8(request.hook) else {
        return RESULT_NON_CANONICAL;
    };
    let Ok(engine) = WasmEngine::declared() else {
        return RESULT_FATAL_INVARIANT;
    };
    let Ok(module) = engine.validate(&request.wasm) else {
        return RESULT_NON_CANONICAL;
    };
    match Executor::declared().execute(&module, &hook, &[]) {
        Ok(_) => RESULT_OK,
        Err(crate::ExecutionError::Resource(_)) => RESULT_GAS_EXHAUSTED,
        Err(crate::ExecutionError::Fault(crate::ExecutionFault::UnknownExport { .. })) => {
            RESULT_UNKNOWN_ACTIVITY
        }
        Err(_) => RESULT_NON_CANONICAL,
    }
}

/// Discards a partial request after a C-side transfer failure.
#[no_mangle]
pub extern "C" fn layerx_programs_migration_abort(handle: u64) {
    if let Ok(mut requests) = pending().lock() {
        requests.remove(&handle);
    }
}
