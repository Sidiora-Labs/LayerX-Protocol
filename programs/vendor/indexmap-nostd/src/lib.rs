//! ### Disclaimer
//!
//! This is a reimplementation of the [`indexmap` crate] and meant
//! to be a drop-in replacement for its [`indexmap::IndexMap`] and
//! [`indexmap::IndexSet`] types in embedded `no_std` environments that cannot
//! afford to use hash tables that require random seed initialization in order
//! to _not_ be attackable by users controlling their inputs.
//!
//! This crate was originally and primarily conceived as building block for
//! a similar [`wasmparser-nostd` crate] fork for embedded `no_std` environments.
//! Therefore this crate currently only supports a subset of the features of
//! the original [`indexmap` crate] that are required by the
//! [`wasmparser-nostd` crate] initiative.
//!
//! ### Types
//!
//! [`IndexMap`] is a hash table where the iteration order of the key-value
//! pairs is independent of the hash values of the keys.
//!
//! [`IndexSet`] is a corresponding hash set using the same implementation and
//! with similar properties.
//!
//! [`wasmparser-nostd` crate]: https://crates.io/crates/wasmparser-nostd
//! [`indexmap::IndexMap`]: https://docs.rs/indexmap/latest/indexmap/map/struct.IndexMap.html
//! [`indexmap::IndexSet`]: https://docs.rs/indexmap/latest/indexmap/set/struct.IndexSet.html
//! [`indexmap` crate]: https://crates.io/crates/indexmap
//! [`IndexMap`]: map/struct.IndexMap.html
//! [`IndexSet`]: set/struct.IndexSet.html
//!
//! ### Feature Highlights
//!
//! [`IndexMap`] and [`IndexSet`] are mostly drop-in compatible with the
//! standard library's `HashMap` and `HashSet`.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std as alloc;

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod map;
pub mod set;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "serde")]
pub mod serde_seq;

pub use self::map::IndexMap;
pub use self::set::IndexSet;

/// A slot index referencing a slot in an [`IndexMap`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SlotIndex(usize);

impl SlotIndex {
    /// Returns the raw `usize` index of the [`SlotIndex`].
    pub fn index(self) -> usize {
        self.0
    }
}
