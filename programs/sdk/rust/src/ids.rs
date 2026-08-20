//! Nonzero protocol identifiers.
//!
//! Every identifier the ABI carries is 32 bytes wide and reserves the all-zero
//! value for absence. Constructing one checks that bound, so a program cannot
//! hand the host an identifier the runtime would have to refuse.

use crate::error::{Field, ProgramError, Reason};

const IDENTIFIER_BYTES: usize = 32;

const fn is_reserved(bytes: &[u8; IDENTIFIER_BYTES]) -> bool {
    let mut index = 0;
    while index < IDENTIFIER_BYTES {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Stable identifier of a deployed program.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProgramId([u8; IDENTIFIER_BYTES]);

impl ProgramId {
    /// Constructs a nonzero program identifier.
    ///
    /// # Errors
    ///
    /// Refuses the all-zero identifier reserved for absence.
    pub const fn new(bytes: [u8; IDENTIFIER_BYTES]) -> Result<Self, ProgramError> {
        if is_reserved(&bytes) {
            return Err(ProgramError::value(Field::Program, Reason::Zero));
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical identifier bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; IDENTIFIER_BYTES] {
        self.0
    }
}

/// Stable identifier of a protocol asset.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssetId([u8; IDENTIFIER_BYTES]);

impl AssetId {
    /// Constructs a nonzero asset identifier.
    ///
    /// # Errors
    ///
    /// Refuses the all-zero identifier reserved for absence.
    pub const fn new(bytes: [u8; IDENTIFIER_BYTES]) -> Result<Self, ProgramError> {
        if is_reserved(&bytes) {
            return Err(ProgramError::value(Field::Asset, Reason::Zero));
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical identifier bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; IDENTIFIER_BYTES] {
        self.0
    }
}

/// Stable identifier of an account a 402LXP transfer may credit.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AccountId([u8; IDENTIFIER_BYTES]);

impl AccountId {
    /// Constructs a nonzero account identifier.
    ///
    /// # Errors
    ///
    /// Refuses the all-zero identifier reserved for absence.
    pub const fn new(bytes: [u8; IDENTIFIER_BYTES]) -> Result<Self, ProgramError> {
        if is_reserved(&bytes) {
            return Err(ProgramError::value(Field::Account, Reason::Zero));
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical identifier bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; IDENTIFIER_BYTES] {
        self.0
    }
}

/// Digest naming one verified protocol receipt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReceiptDigest([u8; IDENTIFIER_BYTES]);

impl ReceiptDigest {
    /// Constructs a nonzero receipt digest.
    ///
    /// # Errors
    ///
    /// Refuses the all-zero digest reserved for absence.
    pub const fn new(bytes: [u8; IDENTIFIER_BYTES]) -> Result<Self, ProgramError> {
        if is_reserved(&bytes) {
            return Err(ProgramError::value(Field::ReceiptDigest, Reason::Zero));
        }
        Ok(Self(bytes))
    }

    /// Returns the canonical digest bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; IDENTIFIER_BYTES] {
        self.0
    }
}
