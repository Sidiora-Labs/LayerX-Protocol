//! Deterministic WASM runtime foundation for `LayerX` guest programs.

pub mod engine;
pub mod execute;
pub mod limits;
pub mod test_support;
pub mod validate;

pub use engine::{EngineRefusal, WasmEngine};
pub use execute::{ExecutionFault, ProgramInstance, WasmValue};
pub use limits::{DeclaredLimit, LimitsRefusal, ValidationLimits};
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
