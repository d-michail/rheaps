use core::cmp::Ordering;

use crate::Heap;

/// Comparator that uses [`Ord`] to order values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NaturalOrder;

/// Defines the ordering used by a heap.
pub trait Comparator<T> {
    /// Returns the order of `left` relative to `right`.
    fn compare(&self, left: &T, right: &T) -> Ordering;
}

impl<T: Ord> Comparator<T> for NaturalOrder {
    fn compare(&self, left: &T, right: &T) -> Ordering {
        left.cmp(right)
    }
}

impl<T, F> Comparator<T> for F
where
    F: Fn(&T, &T) -> Ordering,
{
    fn compare(&self, left: &T, right: &T) -> Ordering {
        self(left, right)
    }
}

/// An array-backed binary min-heap.
///
/// `C` returns the order of its first argument relative to its second. Values
/// for which it returns [`Ordering::Less`] have higher priority.
#[derive(Clone, Debug)]
pub struct BinaryArrayHeap<T, C = NaturalOrder> {
    values: Vec<T>,
    compare: C,
}

impl<T: Ord> BinaryArrayHeap<T> {
    /// Creates an empty heap that uses the natural ordering of `T`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            compare: NaturalOrder,
        }
    }

    /// Builds a heap from `values` in linear time.
    #[must_use]
    pub fn from_vec(values: Vec<T>) -> Self {
        Self::from_vec_by(values, NaturalOrder)
    }
}

impl<T: Ord> Default for BinaryArrayHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, C> BinaryArrayHeap<T, C>
where
    C: Comparator<T>,
{
    /// Creates an empty heap ordered by `compare`.
    #[must_use]
    pub fn with_comparator(compare: C) -> Self {
        Self {
            values: Vec::with_capacity(16),
            compare,
        }
    }

    /// Creates an empty heap with at least `capacity` value slots, ordered by
    /// `compare`.
    #[must_use]
    pub fn with_capacity_and_comparator(capacity: usize, compare: C) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            compare,
        }
    }

    /// Builds a heap from `values` in linear time using `compare`.
    #[must_use]
    pub fn from_vec_by(values: Vec<T>, compare: C) -> Self {
        let mut heap = Self { values, compare };
        heap.heapify();
        heap
    }

    /// Returns the comparator used to order values.
    #[must_use]
    pub fn comparator(&self) -> &C {
        &self.compare
    }

    /// Consumes the heap and returns its backing storage in heap order.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    fn heapify(&mut self) {
        for index in (0..self.values.len() / 2).rev() {
            self.sift_down(index);
        }
    }

    fn sift_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) / 2;
            if self
                .compare
                .compare(&self.values[parent], &self.values[index])
                != Ordering::Greater
            {
                break;
            }
            self.values.swap(parent, index);
            index = parent;
        }
    }

    fn sift_down(&mut self, mut index: usize) {
        let len = self.values.len();
        loop {
            let left = 2 * index + 1;
            if left >= len {
                return;
            }

            let right = left + 1;
            let child = if right < len
                && self
                    .compare
                    .compare(&self.values[right], &self.values[left])
                    == Ordering::Less
            {
                right
            } else {
                left
            };

            if self
                .compare
                .compare(&self.values[index], &self.values[child])
                != Ordering::Greater
            {
                return;
            }
            self.values.swap(index, child);
            index = child;
        }
    }
}

impl<T, C> Heap<T> for BinaryArrayHeap<T, C>
where
    C: Comparator<T>,
{
    fn push(&mut self, value: T) {
        self.values.push(value);
        self.sift_up(self.values.len() - 1);
    }

    fn peek(&self) -> Option<&T> {
        self.values.first()
    }

    fn pop(&mut self) -> Option<T> {
        let result = self.values.pop()?;
        if self.values.is_empty() {
            return Some(result);
        }

        let minimum = core::mem::replace(&mut self.values[0], result);
        self.sift_down(0);
        Some(minimum)
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn clear(&mut self) {
        self.values.clear();
    }
}
