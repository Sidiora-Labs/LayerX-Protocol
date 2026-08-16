//! Immutable vector sets keyed by intent wire version.

/// Intent version represented by [`V1_SOURCE`].
pub const V1_VERSION: u16 = 1;

/// Committed intent, payload, and payload-hash vectors for version 1.
pub const V1_SOURCE: &str = include_str!("../vectors/v1.kvx");
