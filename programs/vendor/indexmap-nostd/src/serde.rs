use crate::IndexMap;
use core::fmt::{self, Formatter};
use core::marker::PhantomData;
use serde::de::value::{MapDeserializer, SeqDeserializer};
use serde::de::{
    Deserialize, Deserializer, Error, IntoDeserializer, MapAccess, SeqAccess, Visitor,
};
use serde::ser::{Serialize, Serializer};

/// Requires crate feature `"serde"`
impl<K, V> Serialize for IndexMap<K, V>
where
    K: Serialize + Ord,
    V: Serialize,
{
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: Serializer,
    {
        serializer.collect_map(self)
    }
}

struct IndexMapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for IndexMapVisitor<K, V>
where
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    type Value = IndexMap<K, V>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = IndexMap::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((key, value)) = map.next_entry()? {
            values.insert(key, value);
        }
        Ok(values)
    }
}

/// Requires crate feature `"serde"`
impl<'de, K, V> Deserialize<'de> for IndexMap<K, V>
where
    K: Deserialize<'de> + Ord + Clone,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(IndexMapVisitor(PhantomData))
    }
}

impl<'de, K, V, E> IntoDeserializer<'de, E> for IndexMap<K, V>
where
    K: IntoDeserializer<'de, E> + Ord,
    V: IntoDeserializer<'de, E>,
    E: Error,
{
    type Deserializer = MapDeserializer<'de, <Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        MapDeserializer::new(self.into_iter())
    }
}

use crate::IndexSet;

/// Requires crate feature `"serde"`
impl<T> Serialize for IndexSet<T>
where
    T: Serialize + Ord,
{
    fn serialize<Se>(&self, serializer: Se) -> Result<Se::Ok, Se::Error>
    where
        Se: Serializer,
    {
        serializer.collect_seq(self)
    }
}

struct IndexSetVisitor<T>(PhantomData<T>);

impl<'de, T> Visitor<'de> for IndexSetVisitor<T>
where
    T: Deserialize<'de> + Ord + Clone,
{
    type Value = IndexSet<T>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a set")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = IndexSet::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(value) = seq.next_element()? {
            values.insert(value);
        }
        Ok(values)
    }
}

/// Requires crate feature `"serde"`
impl<'de, T> Deserialize<'de> for IndexSet<T>
where
    T: Deserialize<'de> + Ord + Clone,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(IndexSetVisitor(PhantomData))
    }
}

impl<'de, T, E> IntoDeserializer<'de, E> for IndexSet<T>
where
    T: IntoDeserializer<'de, E> + Ord,
    E: Error,
{
    type Deserializer = SeqDeserializer<<Self as IntoIterator>::IntoIter, E>;

    fn into_deserializer(self) -> Self::Deserializer {
        SeqDeserializer::new(self.into_iter())
    }
}
