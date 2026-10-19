//! Borrowed views of `list<T>` and `list<T>[N]`.

use crate::{read::Reader, wire::Wire};
use std::{fmt, iter::FusedIterator};

/// A verified list read in place. Elements are decoded as they are visited.
pub struct ListRef<'a, T: Wire> {
    elements: Reader<'a>,
    len: u64,
    _marker: std::marker::PhantomData<T>,
}

impl<'a, T: Wire> ListRef<'a, T> {
    pub(crate) fn new(elements: Reader<'a>, len: u64) -> Self {
        ListRef {
            elements,
            len,
            _marker: std::marker::PhantomData,
        }
    }

    /// The number of elements, without walking them.
    pub fn len(&self) -> usize {
        usize::try_from(self.len).unwrap_or(usize::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> ListIter<'a, T> {
        ListIter {
            elements: self.elements,
            remaining: self.len,
            _marker: std::marker::PhantomData,
        }
    }

    /// The element at `index`. Constant time when every element has the same encoded size,
    /// otherwise it walks the preceding elements.
    pub fn get(&self, index: usize) -> Option<T::Ref<'a>> {
        if index >= self.len() {
            return None;
        }
        match T::FIXED_SIZE {
            Some(size) => {
                let start = index.checked_mul(size)?;
                let end = start.checked_add(size)?;
                Some(T::read_span(self.elements.slice(start, end)))
            }
            None => self.iter().nth(index),
        }
    }

    /// The encoded elements, without the count prefix.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.elements.as_slice()
    }

    /// Converts the elements to owned values, sharing `str` and `data` bytes with the input.
    pub fn to_vec(&self) -> Vec<T::Owned> {
        let mut elements = self.elements;
        (0..self.len)
            .map(|_| T::read_owned(&mut elements))
            .collect()
    }
}

impl<T: Wire> Clone for ListRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Wire> Copy for ListRef<'_, T> {}

/// Verified encodings are canonical, so lists are equal when their encoded elements are.
impl<T: Wire> PartialEq for ListRef<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.as_bytes() == other.as_bytes()
    }
}

impl<'a, T: Wire> IntoIterator for ListRef<'a, T> {
    type Item = T::Ref<'a>;
    type IntoIter = ListIter<'a, T>;

    fn into_iter(self) -> ListIter<'a, T> {
        self.iter()
    }
}

impl<'a, T: Wire> fmt::Debug for ListRef<'a, T>
where
    T::Ref<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Iterator over a [`ListRef`].
pub struct ListIter<'a, T: Wire> {
    elements: Reader<'a>,
    remaining: u64,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Wire> Clone for ListIter<'_, T> {
    fn clone(&self) -> Self {
        ListIter {
            elements: self.elements,
            remaining: self.remaining,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<'a, T: Wire> Iterator for ListIter<'a, T> {
    type Item = T::Ref<'a>;

    fn next(&mut self) -> Option<T::Ref<'a>> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(T::read(&mut self.elements))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl<T: Wire> ExactSizeIterator for ListIter<'_, T> {}

impl<T: Wire> FusedIterator for ListIter<'_, T> {}
