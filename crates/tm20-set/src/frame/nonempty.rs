//! Nonempty sequence. Empty authoring containers use [`super::ItemBody::Blank`], not this.

use std::ops::Deref;
use std::slice;

/// Private nonempty storage. Equivalent to first-plus-tail; the vector is never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonEmpty<T> {
    items: Vec<T>,
}

impl<T> NonEmpty<T> {
    pub fn new(first: T, tail: Vec<T>) -> Self {
        let mut items = Vec::with_capacity(1 + tail.len());
        items.push(first);
        items.extend(tail);
        Self { items }
    }

    pub fn singleton(first: T) -> Self {
        Self { items: vec![first] }
    }

    pub fn from_vec(items: Vec<T>) -> Option<Self> {
        if items.is_empty() {
            None
        } else {
            Some(Self { items })
        }
    }

    pub fn first(&self) -> &T {
        &self.items[0]
    }

    pub fn tail(&self) -> &[T] {
        &self.items[1..]
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.items.iter()
    }
}

impl<T> Deref for NonEmpty<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T> IntoIterator for NonEmpty<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a NonEmpty<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_vec_is_not_nonempty() {
        assert!(NonEmpty::<u8>::from_vec(vec![]).is_none());
    }

    #[test]
    fn from_vec_keeps_order() {
        let n = NonEmpty::from_vec(vec![1, 2, 3]).unwrap();
        assert_eq!(n.first(), &1);
        assert_eq!(n.tail(), &[2, 3]);
        assert_eq!(n.as_slice(), &[1, 2, 3]);
        assert_eq!(n.len(), 3);
        assert_eq!(n.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn singleton_has_empty_tail() {
        let n = NonEmpty::singleton(7);
        assert_eq!(n.first(), &7);
        assert!(n.tail().is_empty());
        assert_eq!(n.iter().copied().collect::<Vec<_>>(), vec![7]);
    }

    #[test]
    fn new_joins_first_and_tail() {
        let n = NonEmpty::new(10, vec![11, 12]);
        assert_eq!(&*n, &[10, 11, 12]);
        let owned: Vec<_> = n.into_iter().collect();
        assert_eq!(owned, vec![10, 11, 12]);
    }
}
