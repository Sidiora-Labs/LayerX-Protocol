//! Determinism, replay and bounded fuzz qualification entry points.

use core::fmt::{self, Display};

use crate::{ExecutionError, Executor, ValidationLimits, ValidationRefusal, WasmEngine, WasmValue};

/// Maximum input accepted by the in-process fuzz targets.
pub const MAX_FUZZ_INPUT_BYTES: usize = 1_048_576;

/// One independently selected fuzz surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzTarget {
    /// Parser and deterministic-subset validation.
    Validation,
    /// Validated-module instantiation.
    Instantiation,
    /// Metered invocation of an exported function.
    Execution,
}

/// A replayable execution pinned to its consensus runtime and ABI versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedExecution<'a> {
    pub runtime_version: u16,
    pub abi_version: u16,
    pub wasm: &'a [u8],
    pub export: &'a str,
    pub args: &'a [WasmValue],
}

/// Typed replay refusal. Unknown versions are preserved but never executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayRefusal {
    UnknownRuntimeVersion { version: u16 },
    UnknownAbiVersion { version: u16 },
    Engine(String),
    Validation(ValidationRefusal),
    Execution(ExecutionError),
}

impl Display for ReplayRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRuntimeVersion { version } => {
                write!(f, "unknown runtime version {version}")
            }
            Self::UnknownAbiVersion { version } => write!(f, "unknown ABI version {version}"),
            Self::Engine(reason) => write!(f, "engine refusal: {reason}"),
            Self::Validation(reason) => write!(f, "validation refusal: {reason}"),
            Self::Execution(reason) => write!(f, "execution refusal: {reason}"),
        }
    }
}

impl std::error::Error for ReplayRefusal {}

/// Evidence mismatch between two independently constructed executions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferentialMismatch {
    pub first: Vec<u8>,
    pub second: Vec<u8>,
}

/// Runs one bounded fuzz input through a named real runtime surface.
///
/// Invalid input is an expected typed refusal. A panic, hang, or allocation
/// beyond the declared module bound is therefore visible to the fuzz runner.
pub fn programs_fuzz_targets(target: FuzzTarget, input: &[u8]) {
    if input.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }
    let Ok(engine) = WasmEngine::new(ValidationLimits::declared()) else {
        return;
    };
    let validated = engine.validate(input);
    if target == FuzzTarget::Validation {
        return;
    }
    let Ok(module) = validated else {
        return;
    };
    if target == FuzzTarget::Instantiation {
        let _ = module.instantiate_for_qualification();
        return;
    }
    let _ = Executor::declared().execute(&module, "add", &[WasmValue::I32(19), WasmValue::I32(23)]);
}

/// Executes the same immutable input using two separately constructed engines
/// and executors, rejecting any byte-level evidence divergence.
///
/// # Errors
///
/// Returns both evidence byte strings if the independent executions diverge.
pub fn programs_differential_gate(
    wasm: &[u8],
    export: &str,
    args: &[WasmValue],
) -> Result<Vec<u8>, DifferentialMismatch> {
    let run = || -> Result<Vec<u8>, Vec<u8>> {
        let engine = WasmEngine::declared().map_err(|error| error.to_string().into_bytes())?;
        let module = engine
            .validate(wasm)
            .map_err(|error| error.to_string().into_bytes())?;
        Executor::declared()
            .execute(&module, export, args)
            .map(|record| record.canonical_evidence())
            .map_err(|error| error.to_string().into_bytes())
    };
    let first = run().unwrap_or_else(|evidence| evidence);
    let second = run().unwrap_or_else(|evidence| evidence);
    if first == second {
        Ok(first)
    } else {
        Err(DifferentialMismatch { first, second })
    }
}

/// Replays only under the recorded version, never the newest implementation.
///
/// # Errors
///
/// Returns a typed refusal for unknown versions, invalid modules, or failed
/// execution.
pub fn replay_recorded_execution(record: &RecordedExecution<'_>) -> Result<Vec<u8>, ReplayRefusal> {
    if record.runtime_version != crate::RUNTIME_VERSION {
        return Err(ReplayRefusal::UnknownRuntimeVersion {
            version: record.runtime_version,
        });
    }
    if record.abi_version != crate::ABI_VERSION {
        return Err(ReplayRefusal::UnknownAbiVersion {
            version: record.abi_version,
        });
    }
    let engine =
        WasmEngine::declared().map_err(|error| ReplayRefusal::Engine(error.to_string()))?;
    let module = engine
        .validate(record.wasm)
        .map_err(ReplayRefusal::Validation)?;
    Executor::declared()
        .execute(&module, record.export, record.args)
        .map(|result| result.canonical_evidence())
        .map_err(ReplayRefusal::Execution)
}
