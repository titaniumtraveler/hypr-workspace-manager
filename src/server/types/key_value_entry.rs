use serde::{
    de::{Error, Visitor},
    ser::SerializeMap,
    Deserialize, Serialize,
};
use std::{
    fmt::{self, Debug},
    marker::PhantomData,
};

pub struct KeyValueEntry<K, V> {
    pub key: K,
    pub value: V,
}

impl<K, V> Serialize for KeyValueEntry<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_key(&self.key)?;
        map.serialize_value(&self.value)?;
        map.end()
    }
}

impl<'de, K, V> Deserialize<'de> for KeyValueEntry<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

struct MapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = KeyValueEntry<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map with exactly one entry")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let (key, value) = map
            .next_entry()?
            .ok_or_else(|| A::Error::invalid_length(0, &"a map with exactly one entry"))?;

        Ok(KeyValueEntry { key, value })
    }
}

impl<K: Debug, V: Debug> Debug for KeyValueEntry<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries([(&self.key, &self.value)]).finish()
    }
}
