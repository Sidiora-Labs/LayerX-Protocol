//! The EVM value vocabulary carried across the port: 20-byte addresses and
//! 32-byte big-endian words. Every conversion is checked at construction and
//! no value ever leaves the integer domain.

use crate::error::PortRefusal;

/// Byte width of an EVM storage word.
pub const WORD_BYTES: usize = 32;
/// Byte width of an EVM account address.
pub const ADDRESS_BYTES: usize = 20;

/// One 32-byte big-endian EVM word.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Word([u8; WORD_BYTES]);

impl Word {
    /// Wraps raw big-endian word bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; WORD_BYTES]) -> Self {
        Self(bytes)
    }

    /// Right-aligns a `uint64` into a word, exactly as `abi.encode` does.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        let mut bytes = [0u8; WORD_BYTES];
        bytes[24..].copy_from_slice(&value.to_be_bytes());
        Self(bytes)
    }

    /// Right-aligns a `uint128` into a word.
    #[must_use]
    pub fn from_u128(value: u128) -> Self {
        let mut bytes = [0u8; WORD_BYTES];
        bytes[16..].copy_from_slice(&value.to_be_bytes());
        Self(bytes)
    }

    /// Left-pads an address into a word, exactly as Solidity does when it uses
    /// an address as a mapping key or an indexed topic.
    #[must_use]
    pub fn from_address(address: Address) -> Self {
        let mut bytes = [0u8; WORD_BYTES];
        bytes[12..].copy_from_slice(&address.bytes());
        Self(bytes)
    }

    /// Returns the raw big-endian word bytes.
    #[must_use]
    pub const fn bytes(&self) -> [u8; WORD_BYTES] {
        self.0
    }

    /// Returns whether every byte of the word is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; WORD_BYTES]
    }

    /// Narrows a word to `uint64`.
    ///
    /// # Errors
    ///
    /// Refuses any word whose value does not fit in 64 bits, rather than
    /// truncating it the way a hand-written port silently would.
    pub fn to_u64(self) -> Result<u64, PortRefusal> {
        if self.0[..24] != [0u8; 24] {
            return Err(PortRefusal::WordTooWide);
        }
        let mut tail = [0u8; 8];
        tail.copy_from_slice(&self.0[24..]);
        Ok(u64::from_be_bytes(tail))
    }

    /// Narrows a word to `uint128`.
    ///
    /// # Errors
    ///
    /// Refuses any word wider than 128 bits.
    pub fn to_u128(self) -> Result<u128, PortRefusal> {
        if self.0[..16] != [0u8; 16] {
            return Err(PortRefusal::WordTooWide);
        }
        let mut tail = [0u8; 16];
        tail.copy_from_slice(&self.0[16..]);
        Ok(u128::from_be_bytes(tail))
    }

    /// Adds an unsigned scalar with EVM wrapping semantics at `2^256`.
    #[must_use]
    pub fn add_scalar(self, delta: u64) -> Self {
        let mut result = self.0;
        let mut carry = u128::from(delta);
        let mut index = WORD_BYTES;
        while index > 0 && carry != 0 {
            index -= 1;
            let sum = u128::from(result[index]) + (carry & 0xff);
            result[index] = u8::try_from(sum & 0xff).unwrap_or(0);
            carry = (carry >> 8) + (sum >> 8);
        }
        Self(result)
    }
}

/// One 20-byte EVM account address.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Address([u8; ADDRESS_BYTES]);

impl Address {
    /// Constructs a nonzero account address.
    ///
    /// # Errors
    ///
    /// Refuses the zero address, which in Solidity marks absence rather than
    /// an account and can therefore never be a port target.
    pub fn new(bytes: [u8; ADDRESS_BYTES]) -> Result<Self, PortRefusal> {
        if bytes == [0u8; ADDRESS_BYTES] {
            return Err(PortRefusal::ZeroAddress);
        }
        Ok(Self(bytes))
    }

    /// Returns the raw address bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; ADDRESS_BYTES] {
        self.0
    }

    /// Returns the address left-padded into a 32-byte word.
    #[must_use]
    pub fn word(self) -> Word {
        Word::from_address(self)
    }
}
