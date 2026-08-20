//! Caller-owned byte buffers.
//!
//! The guest bindings never allocate. Every host call that returns bytes
//! writes into storage the program declared itself, so a program's memory
//! ceiling is visible in its own source and stays inside the resource budget
//! the runtime meters.

use crate::error::{Field, ProgramError, Reason};

/// A fixed-capacity byte buffer sized by the program that owns it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bytes<const N: usize> {
    buffer: [u8; N],
    length: usize,
}

impl<const N: usize> Bytes<N> {
    /// Declared capacity of this buffer.
    pub const CAPACITY: usize = N;

    /// Creates an empty buffer of the declared capacity.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            buffer: [0; N],
            length: 0,
        }
    }

    /// Creates a buffer holding a copy of the given bytes.
    ///
    /// # Errors
    ///
    /// Refuses input longer than the declared capacity.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, ProgramError> {
        let mut buffer = Self::empty();
        buffer.extend(bytes)?;
        Ok(buffer)
    }

    /// Returns the number of bytes currently held.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Reports whether the buffer holds no bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Borrows the bytes currently held.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer[..self.length]
    }

    /// Borrows the whole declared capacity for a host function to fill.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Discards every held byte without touching the declared capacity.
    pub fn clear(&mut self) {
        self.length = 0;
    }

    /// Declares how many leading bytes of the capacity are now held.
    ///
    /// # Errors
    ///
    /// Refuses a length past the declared capacity.
    pub fn set_length(&mut self, length: usize) -> Result<(), ProgramError> {
        if length > N {
            return Err(ProgramError::value(Field::Buffer, Reason::TooLarge));
        }
        self.length = length;
        Ok(())
    }

    /// Appends one byte.
    ///
    /// # Errors
    ///
    /// Refuses a write past the declared capacity.
    pub fn push(&mut self, byte: u8) -> Result<(), ProgramError> {
        let slot = self
            .buffer
            .get_mut(self.length)
            .ok_or_else(|| ProgramError::value(Field::Buffer, Reason::TooSmall))?;
        *slot = byte;
        self.length = self.length.saturating_add(1);
        Ok(())
    }

    /// Appends a run of bytes.
    ///
    /// # Errors
    ///
    /// Refuses a write past the declared capacity.
    pub fn extend(&mut self, bytes: &[u8]) -> Result<(), ProgramError> {
        let end = self
            .length
            .checked_add(bytes.len())
            .ok_or_else(|| ProgramError::value(Field::Buffer, Reason::TooLarge))?;
        let target = self
            .buffer
            .get_mut(self.length..end)
            .ok_or_else(|| ProgramError::value(Field::Buffer, Reason::TooSmall))?;
        target.copy_from_slice(bytes);
        self.length = end;
        Ok(())
    }
}

impl<const N: usize> Default for Bytes<N> {
    fn default() -> Self {
        Self::empty()
    }
}
