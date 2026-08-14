//! Constructor limits copied from the normative protocol headers.

/// Every consensus identifier and digest is 32 bytes.
pub const IDENTIFIER_BYTES: usize = 32;
/// Maximum canonical DID byte length (`LXP_MAX_DID_LENGTH`).
pub const MAX_DID_BYTES: usize = 255;
/// Maximum canonical account namespace length (`LX_ACCOUNT_NAME_MAX`).
pub const MAX_ACCOUNT_NAME_BYTES: usize = 512;
