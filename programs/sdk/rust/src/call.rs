//! Program-to-program call bindings.
//!
//! A call carries an explicitly narrowed capability list. The host refuses any
//! grant this program does not already hold and any transfer ceiling above the
//! one it was given, so authority can only shrink as a call graph deepens.

use crate::abi::{MAX_CALL_INPUT_BYTES, MAX_CAPABILITY_ENCODING_BYTES};
use crate::error::{Field, ProgramError, Reason};

#[cfg(target_arch = "wasm32")]
use crate::capability::CapabilitySet;
#[cfg(target_arch = "wasm32")]
use crate::host;
#[cfg(target_arch = "wasm32")]
use crate::ids::ProgramId;

const MINIMUM_CAPABILITY_ENCODING_BYTES: usize = 2;

/// The input bytes handed to a called program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallInput<'a>(&'a [u8]);

impl<'a> CallInput<'a> {
    /// Constructs call input inside the version-one bound.
    ///
    /// # Errors
    ///
    /// Refuses input past the declared bound.
    pub const fn new(bytes: &'a [u8]) -> Result<Self, ProgramError> {
        if bytes.len() > MAX_CALL_INPUT_BYTES {
            return Err(ProgramError::value(Field::CallInput, Reason::TooLarge));
        }
        Ok(Self(bytes))
    }

    /// Borrows the canonical input bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.0
    }
}

/// A non-negative result code a called program returns.
///
/// The runtime treats a negative code from a callee as a refusal that aborts
/// the whole call graph, so this type holds no negative value at all.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct CallResult(i32);

impl CallResult {
    /// The code a callee returns for an unqualified success.
    pub const OK: Self = Self(0);

    /// Builds a non-negative result code.
    ///
    /// # Errors
    ///
    /// Refuses a code wider than the boundary's signed integer.
    pub fn new(code: u32) -> Result<Self, ProgramError> {
        i32::try_from(code)
            .map(Self)
            .map_err(|_| ProgramError::value(Field::CallResult, Reason::TooLarge))
    }

    /// Returns the code as it crosses the boundary.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// An encoded capability list narrowed for one call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantedCapabilities<'a>(&'a [u8]);

impl<'a> GrantedCapabilities<'a> {
    /// Wraps bytes already produced by
    /// [`CapabilitySet::encode_into`](crate::capability::CapabilitySet::encode_into).
    ///
    /// # Errors
    ///
    /// Refuses an encoding shorter than the grant count and an encoding past
    /// the declared bound.
    pub const fn new(encoded: &'a [u8]) -> Result<Self, ProgramError> {
        if encoded.len() < MINIMUM_CAPABILITY_ENCODING_BYTES {
            return Err(ProgramError::value(
                Field::CapabilityEncoding,
                Reason::Malformed,
            ));
        }
        if encoded.len() > MAX_CAPABILITY_ENCODING_BYTES {
            return Err(ProgramError::value(
                Field::CapabilityEncoding,
                Reason::TooLarge,
            ));
        }
        Ok(Self(encoded))
    }

    /// Borrows the canonical encoded grant list.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.0
    }
}

/// Calls another program with an already-encoded grant list, returning the
/// callee's non-negative result code.
///
/// # Errors
///
/// Refuses missing call authority, an oversized input, any escalation of the
/// held grants, and every meter refusal.
#[cfg(target_arch = "wasm32")]
pub fn invoke(
    callee: ProgramId,
    input: CallInput<'_>,
    capabilities: GrantedCapabilities<'_>,
) -> Result<i32, ProgramError> {
    let program = callee.bytes();
    host::program_call(&program, input.bytes(), capabilities.bytes())
}

/// Encodes a capability set into a caller-owned scratch buffer and calls
/// another program with it, returning the callee's non-negative result code.
///
/// # Errors
///
/// Refuses a scratch buffer shorter than the encoding, missing call
/// authority, an oversized input, any escalation of the held grants, and
/// every meter refusal.
#[cfg(target_arch = "wasm32")]
pub fn invoke_with<const N: usize>(
    callee: ProgramId,
    input: CallInput<'_>,
    capabilities: &CapabilitySet<N>,
    scratch: &mut [u8],
) -> Result<i32, ProgramError> {
    let written = capabilities.encode_into(scratch)?;
    let encoded = scratch
        .get(..written)
        .ok_or_else(|| ProgramError::value(Field::Buffer, Reason::TooSmall))?;
    invoke(callee, input, GrantedCapabilities::new(encoded)?)
}
