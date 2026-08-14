//! Canonical `LayerX` domain types shared by the interaction layer.

pub mod policy;

/// Identifies the workspace manifest used by all interaction-layer crates.
#[must_use]
pub const fn agent_workspace_manifest() -> &'static str {
    "agent/Cargo.toml"
}

/// Identifies the compiler version pinned for reproducible agent builds.
#[must_use]
pub const fn agent_toolchain_pin() -> &'static str {
    "1.91.1"
}
