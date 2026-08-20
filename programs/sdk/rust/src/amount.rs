//! The protocol monetary integer.
//!
//! [`Amount`] is an exact unsigned 128-bit integer. It has no floating-point
//! constructor, no floating-point conversion and no arithmetic operator that
//! could silently wrap, so a program cannot express money the deterministic
//! runtime is unable to reproduce. Widening constructors accept only the
//! sealed [`ProtocolInteger`] set, which no floating-point type can join.

use crate::error::{Field, ProgramError, Reason};

mod sealed {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for u128 {}
}

/// Unsigned protocol integers admitted into the monetary vocabulary.
///
/// The trait is sealed inside this crate and implemented for no signed or
/// floating-point type, so [`Amount::from_integer`] is closed against both by
/// construction rather than by convention.
pub trait ProtocolInteger: sealed::Sealed + Copy {
    /// Widens the value to the protocol's exact monetary width.
    #[must_use]
    fn into_units(self) -> u128;
}

impl ProtocolInteger for u8 {
    fn into_units(self) -> u128 {
        u128::from(self)
    }
}

impl ProtocolInteger for u16 {
    fn into_units(self) -> u128 {
        u128::from(self)
    }
}

impl ProtocolInteger for u32 {
    fn into_units(self) -> u128 {
        u128::from(self)
    }
}

impl ProtocolInteger for u64 {
    fn into_units(self) -> u128 {
        u128::from(self)
    }
}

impl ProtocolInteger for u128 {
    fn into_units(self) -> u128 {
        self
    }
}

/// A protocol amount represented by the exact unsigned 128-bit width.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Amount(u128);

impl Amount {
    /// Zero amount.
    pub const ZERO: Self = Self(0);

    /// The largest amount the protocol width holds.
    pub const MAX: Self = Self(u128::MAX);

    /// Constructs from an exact integer. No floating-point conversion exists.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(value)
    }

    /// Widens any sealed protocol integer into an amount.
    #[must_use]
    pub fn from_integer<T: ProtocolInteger>(value: T) -> Self {
        Self(value.into_units())
    }

    /// Decodes the canonical big-endian 16-byte representation.
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_be_bytes(bytes))
    }

    /// Rebuilds an amount from the network-order word pair `transfer_402`
    /// carries, the exact inverse of [`Amount::split`].
    #[must_use]
    pub const fn from_words(high: i64, low: i64) -> Self {
        let high = high.to_be_bytes();
        let low = low.to_be_bytes();
        Self(u128::from_be_bytes([
            high[0], high[1], high[2], high[3], high[4], high[5], high[6], high[7], low[0], low[1],
            low[2], low[3], low[4], low[5], low[6], low[7],
        ]))
    }

    /// Constructs from a signed entrypoint argument.
    ///
    /// # Errors
    ///
    /// Refuses a negative value, which has no monetary meaning.
    pub fn from_i64(value: i64) -> Result<Self, ProgramError> {
        u128::try_from(value)
            .map(Self)
            .map_err(|_| ProgramError::value(Field::Amount, Reason::Underflow))
    }

    /// Returns the exact integer.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }

    /// Returns the canonical big-endian bytes.
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// Reports whether this is the zero the monetary law refuses.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Splits into the network-order word pair carried by `transfer_402`.
    #[must_use]
    pub const fn split(self) -> (i64, i64) {
        let bytes = self.0.to_be_bytes();
        let high = [
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ];
        let low = [
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ];
        (i64::from_be_bytes(high), i64::from_be_bytes(low))
    }

    /// Adds without wrapping.
    ///
    /// # Errors
    ///
    /// Refuses a sum that does not fit the protocol width.
    pub const fn checked_add(self, right: Self) -> Result<Self, ProgramError> {
        match self.0.checked_add(right.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgramError::value(Field::Amount, Reason::Overflow)),
        }
    }

    /// Subtracts without wrapping or signed reinterpretation.
    ///
    /// # Errors
    ///
    /// Refuses a difference below zero.
    pub const fn checked_sub(self, right: Self) -> Result<Self, ProgramError> {
        match self.0.checked_sub(right.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgramError::value(Field::Amount, Reason::Underflow)),
        }
    }

    /// Multiplies without wrapping.
    ///
    /// # Errors
    ///
    /// Refuses a product that does not fit the protocol width.
    pub const fn checked_mul(self, right: Self) -> Result<Self, ProgramError> {
        match self.0.checked_mul(right.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgramError::value(Field::Amount, Reason::Overflow)),
        }
    }

    /// Divides with exact integer truncation toward zero.
    ///
    /// # Errors
    ///
    /// Refuses division by the zero amount.
    pub const fn checked_div(self, right: Self) -> Result<Self, ProgramError> {
        match self.0.checked_div(right.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgramError::value(Field::Amount, Reason::Zero)),
        }
    }

    /// Returns the exact integer remainder.
    ///
    /// # Errors
    ///
    /// Refuses a remainder against the zero amount.
    pub const fn checked_rem(self, right: Self) -> Result<Self, ProgramError> {
        match self.0.checked_rem(right.0) {
            Some(value) => Ok(Self(value)),
            None => Err(ProgramError::value(Field::Amount, Reason::Zero)),
        }
    }
}
