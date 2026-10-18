//! Borrowed views of `map<K><V>`.

use crate::{
    read::Reader,
    wire::{Key, Wire},
};
use std::{cmp::Ordering, collections::BTreeMap, fmt, iter::FusedIterator};

/// A verified map read in place. Pairs are visited in their encoded order.
pub struct MapRef<'a, K: Key, V: Wire> {
    pairs: Reader<'a>,
    len: u64,
    _marker: std::marker::PhantomData<(K, V)>,
}

impl<'a, K: Key, V: Wire> MapRef<'a, K, V> {
    pub(crate) fn new(pairs: Reader<'a>, len: u64) -> Self {
        MapRef {
            pairs,
            len,
            _marker: std::marker::PhantomData,
        }
    }

    /// The number of pairs, without walking them.
    pub fn len(&self) -> usize {
        usize::try_from(self.len).unwrap_or(usize::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> MapIter<'a, K, V> {
        MapIter {
            pairs: self.pairs,
            remaining: self.len,
            _marker: std::marker::PhantomData,
        }
    }

    /// Looks up `key` by a linear scan.
    pub fn get(&self, key: K::Ref<'_>) -> Option<V::Ref<'a>> {
        self.iter()
            .find(|(candidate, _)| K::cmp_ref(*candidate, key) == Ordering::Equal)
            .map(|(_, value)| value)
    }

    /// The encoded pairs, without the count prefix.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.pairs.as_slice()
    }

    /// Converts the pairs to an owned map, sharing `str` and `data` bytes with the input.
    pub fn to_map(&self) -> BTreeMap<K::Owned, V::Owned>
    where
        K::Owned: Ord,
    {
        let mut pairs = self.pairs;
        let mut out = BTreeMap::new();
        for _ in 0..self.len {
            let key = K::read_owned(&mut pairs);
            out.insert(key, V::read_owned(&mut pairs));
        }
        out
    }
}

impl<K: Key, V: Wire> Clone for MapRef<'_, K, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Key, V: Wire> Copy for MapRef<'_, K, V> {}

/// Compares encoded pairs, so maps holding the same pairs in a different order are unequal.
impl<K: Key, V: Wire> PartialEq for MapRef<'_, K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_bytes() == other.as_bytes()
    }
}

impl<'a, K: Key, V: Wire> IntoIterator for MapRef<'a, K, V> {
    type Item = (K::Ref<'a>, V::Ref<'a>);
    type IntoIter = MapIter<'a, K, V>;

    fn into_iter(self) -> MapIter<'a, K, V> {
        self.iter()
    }
}

impl<'a, K: Key, V: Wire> fmt::Debug for MapRef<'a, K, V>
where
    K::Ref<'a>: fmt::Debug,
    V::Ref<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// Iterator over a [`MapRef`].
pub struct MapIter<'a, K: Key, V: Wire> {
    pairs: Reader<'a>,
    remaining: u64,
    _marker: std::marker::PhantomData<(K, V)>,
}

impl<K: Key, V: Wire> Clone for MapIter<'_, K, V> {
    fn clone(&self) -> Self {
        MapIter {
            pairs: self.pairs,
            remaining: self.remaining,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'a, K: Key, V: Wire> Iterator for MapIter<'a, K, V> {
    type Item = (K::Ref<'a>, V::Ref<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let key = K::read(&mut self.pairs);
        Some((key, V::read(&mut self.pairs)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl<K: Key, V: Wire> ExactSizeIterator for MapIter<'_, K, V> {}

impl<K: Key, V: Wire> FusedIterator for MapIter<'_, K, V> {}
