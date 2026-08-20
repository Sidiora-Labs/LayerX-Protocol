//! Deterministic WASM runtime foundation for `LayerX` guest programs.

#[deny(unsafe_code)]
pub mod abi;
#[deny(unsafe_code)]
pub mod engine;
#[deny(unsafe_code)]
pub mod execute;
mod ffi;
#[deny(unsafe_code)]
mod host;
#[deny(unsafe_code)]
pub mod lifecycle;
#[deny(unsafe_code)]
pub mod limits;
#[deny(unsafe_code)]
pub mod meter;
#[deny(unsafe_code)]
pub mod storage;
#[deny(unsafe_code)]
pub mod test_support;
#[deny(unsafe_code)]
pub mod validate;

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
pub use storage::{PrincipalId, ProgramId, Storage, StorageError, StorageNamespace};
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
