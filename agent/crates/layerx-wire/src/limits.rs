//! Codec-wide hard bounds.

/// Maximum canonical activity or receipt allocation.
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
/// Protocol version emitted and accepted by this crate revision.
pub const PROTOCOL_VERSION: u16 = 1;
