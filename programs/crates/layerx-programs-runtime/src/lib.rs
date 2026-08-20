//! Deterministic WASM runtime foundation for `LayerX` guest programs.

#[deny(unsafe_code)]
pub mod abi;
#[deny(unsafe_code)]
pub mod calls;
#[deny(unsafe_code)]
pub mod engine;
#[deny(unsafe_code)]
pub mod execute;
mod ffi;
mod ffi_transfer;
#[deny(unsafe_code)]
mod host;
#[deny(unsafe_code)]
pub mod lifecycle;
#[deny(unsafe_code)]
pub mod limits;
#[deny(unsafe_code)]
pub mod meter;
#[deny(unsafe_code)]
pub mod qualification;
#[deny(unsafe_code)]
pub mod storage;
#[deny(unsafe_code)]
pub mod test_support;
#[deny(unsafe_code)]
pub mod transfer;
#[deny(unsafe_code)]
pub mod validate;

pub use calls::{
    call_admission_fuel, CallEdge, CallFrame, CallGraph, CompositionContext, CompositionRefusal,
    CompositionRules, ProgramCatalog, ProgramResolver, CALL_ADMISSION_FUEL, CALL_ENTRY_EXPORT,
    CALL_INPUT_FUEL_PER_BYTE, CALL_RESERVE_EXPORT, DEFAULT_MAX_CALL_FANOUT,
    DEFAULT_MAX_CALL_GRAPH_EDGES, DEFAULT_MAX_COMPOSITION_DEPTH, DEFAULT_MAX_PROGRAM_VISITS,
};
pub use engine::{EngineRefusal, WasmEngine};
pub use execute::{
    AuthorizedExecutionRecord, AuthorizedExecutionRequest, ExecutionError, ExecutionFault,
    ExecutionRecord, Executor, ProgramInstance, WasmValue, ABI_VERSION, RUNTIME_VERSION,
};
pub use lifecycle::{
    CodeHash, Deploy, DeploymentReceipt, DiagnosticArtifact, Lifecycle, LifecycleRefusal,
    Migration, ProgramVersion, Upgrade, UpgradePolicy,
};
pub use limits::{DeclaredLimit, LimitsRefusal, ValidationLimits};
pub use meter::{FeeSchedule, Meter, MeterRefusal, MeteredUsage, ResourceBudget, ResourceKind};
pub use qualification::{
    programs_differential_gate, programs_fuzz_targets, replay_recorded_execution,
    DifferentialMismatch, FuzzTarget, RecordedExecution, ReplayRefusal,
};
pub use storage::{PrincipalId, ProgramId, Storage, StorageError, StorageNamespace};
pub use transfer::{
    AtomicTransferSet, KernelTransferPrimitive, TransferCapability, TransferLawError,
    VerifiedProgramSettlement,
};
pub use validate::{ValidatedModule, ValidationRefusal};

/// Identifies the workspace manifest that governs every programs-plane crate.
#[must_use]
pub const fn programs_workspace_manifest() -> &'static str {
    "programs/Cargo.toml"
}

/// Identifies the vendored deterministic WASM engine and its pinned revision.
#[must_use]
pub const fn programs_wasm_engine() -> &'static str {
    "wasmi 0.31.2 vendored at programs/vendor/wasmi"
}
pub use abi::{
    Abi, AbiCommit, AbiEffects, AbiError, AuthorizationContext, Capability, CapabilitySet,
    HostFunction, ProgramCall, ProgramEvent, ReceiptOracle, ReceiptView, TransferRequest,
    ABI_MANIFEST, ABI_MODULE, HOST_FUNCTIONS,
};
