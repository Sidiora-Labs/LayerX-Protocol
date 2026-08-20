//! Functions to serialize and deserialize an [`IndexMap`] as an ordered sequence.
//!
//! The default `serde` implementation serializes `IndexMap` as a normal map,
//! but there is no guarantee that serialization formats will preserve the order
//! of the key-value pairs. This module serializes `IndexMap` as a sequence of
//! `(key, value)` elements instead, in order.
//!
//! This module may be used in a field attribute for derived implementations:
//!
//! ```
//! # use indexmap_nostd::IndexMap;
//! # use serde_derive::{Deserialize, Serialize};
//! #[derive(Deserialize, Serialize)]
//! struct Data {
//!     #[serde(with = "indexmap_nostd::serde_seq")]
//!     map: IndexMap<i32, u64>,
//!     // ...
//! }
//! ```
//!
//! Requires crate feature `"serde"`.

use crate::IndexMap;
use core::fmt::{self, Formatter};
use core::marker::PhantomData;
use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use serde::ser::{Serialize, Serializer};

/// Serializes an [`IndexMap`] as an ordered sequence.
///
/// This function may be used in a field attribute for deriving `Serialize`:
///
/// ```
/// # use indexmap_nostd::IndexMap;
/// # use serde_derive::Serialize;
/// #[derive(Serialize)]
/// struct Data {
///     #[serde(serialize_with = "indexmap_nostd::serde_seq::serialize")]
///     map: IndexMap<i32, u64>,
///     // ...
/// }
/// ```
///
/// Requires crate feature `"serde"`.
pub fn serialize<K, V, T>(map: &IndexMap<K, V>, serializer: T) -> Result<T::Ok, T::Error>
where
    K: Serialize + Ord,
    V: Serialize,
    T: Serializer,
{
    serializer.collect_seq(map)
}

/// Visitor to deserialize a *sequenced* [`IndexMap`].
struct SeqVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for SeqVisitor<K, V>
where
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    type Value = IndexMap<K, V>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequenced map")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = seq.size_hint().unwrap_or(0);
        let mut map = IndexMap::with_capacity(capacity);
        while let Some((key, value)) = seq.next_element()? {
            map.insert(key, value);
        }
        Ok(map)
    }
}

/// Deserializes an [`IndexMap`] from an ordered sequence.
///
/// This function may be used in a field attribute for deriving `Deserialize`:
///
/// ```
/// # use indexmap_nostd::IndexMap;
/// # use serde_derive::Deserialize;
/// #[derive(Deserialize)]
/// struct Data {
///     #[serde(deserialize_with = "indexmap_nostd::serde_seq::deserialize")]
///     map: IndexMap<i32, u64>,
///     // ...
/// }
/// ```
///
/// Requires crate feature `"serde"`.
pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<IndexMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    deserializer.deserialize_seq(SeqVisitor(PhantomData))
}
