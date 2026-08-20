//! Event bindings.
//!
//! Events are emitted under the calling program's namespace and the invoking
//! principal. Topic and payload bounds are checked at construction, so an
//! oversized event never reaches the host.

use crate::abi::{MAX_EVENT_DATA_BYTES, MAX_EVENT_TOPIC_BYTES};
use crate::error::{Field, ProgramError, Reason};

#[cfg(target_arch = "wasm32")]
use crate::host;

/// The topic one emitted event is filed under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventTopic<'a>(&'a [u8]);

impl<'a> EventTopic<'a> {
    /// Constructs a topic inside the version-one event bound.
    ///
    /// # Errors
    ///
    /// Refuses an empty topic and a topic past the declared bound.
    pub const fn new(bytes: &'a [u8]) -> Result<Self, ProgramError> {
        if bytes.is_empty() {
            return Err(ProgramError::value(Field::EventTopic, Reason::Empty));
        }
        if bytes.len() > MAX_EVENT_TOPIC_BYTES {
            return Err(ProgramError::value(Field::EventTopic, Reason::TooLarge));
        }
        Ok(Self(bytes))
    }

    /// Borrows the canonical topic bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.0
    }
}

/// The payload one emitted event carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventData<'a>(&'a [u8]);

impl<'a> EventData<'a> {
    /// Constructs a payload inside the version-one event bound.
    ///
    /// # Errors
    ///
    /// Refuses a payload past the declared bound.
    pub const fn new(bytes: &'a [u8]) -> Result<Self, ProgramError> {
        if bytes.len() > MAX_EVENT_DATA_BYTES {
            return Err(ProgramError::value(Field::EventData, Reason::TooLarge));
        }
        Ok(Self(bytes))
    }

    /// Borrows the canonical payload bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.0
    }
}

/// Emits one event under this program's namespace.
///
/// # Errors
///
/// Refuses missing emit authority and every bound the host enforces.
#[cfg(target_arch = "wasm32")]
pub fn emit(topic: EventTopic<'_>, data: EventData<'_>) -> Result<(), ProgramError> {
    host::event_emit(topic.bytes(), data.bytes())?;
    Ok(())
}
