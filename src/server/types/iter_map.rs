use serde::{
    ser::{Error, SerializeMap},
    Serialize,
};
use std::cell::RefCell;

pub struct IterMap<I>(RefCell<Option<I>>);

impl<I> IterMap<I> {
    pub fn new(iter: I) -> Self {
        IterMap(RefCell::new(Some(iter)))
    }
}

impl<I, K, V> Serialize for IterMap<I>
where
    I: IntoIterator<Item = (K, V)>,
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let iter = self
            .0
            .take()
            .ok_or_else(|| Error::custom("IterMap should only be serialized __once__"))?
            .into_iter();

        let mut map = serializer.serialize_map(iter.size_hint().1)?;
        for (key, value) in iter.into_iter() {
            map.serialize_entry(&key, &value)?;
        }

        map.end()
    }
}
