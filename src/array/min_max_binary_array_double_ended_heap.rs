use core::cmp::Ordering;

use crate::array::{Comparator, NaturalOrder};
use crate::{DoubleEndedHeap, Heap};

const DEFAULT_HEAP_CAPACITY: usize = 16;

/// An array-backed min-max heap.
///
/// Values on alternating tree levels are ordered as minima and maxima,
/// respectively. This gives `O(1)` access to both extrema and `O(log n)`
/// insertion and removal.
#[derive(Clone, Debug)]
pub struct MinMaxBinaryArrayDoubleEndedHeap<T, C = NaturalOrder> {
    values: Vec<T>,
    compare: C,
}

impl<T: Ord> MinMaxBinaryArrayDoubleEndedHeap<T> {
    /// Creates an empty heap using the natural ordering of values.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            compare: NaturalOrder,
        }
    }

    /// Builds a min-max heap from `values` in linear time.
    #[must_use]
    pub fn from_vec(values: Vec<T>) -> Self {
        Self::from_vec_by(values, NaturalOrder)
    }
}

impl<T: Ord> Default for MinMaxBinaryArrayDoubleEndedHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, C> MinMaxBinaryArrayDoubleEndedHeap<T, C>
where
    C: Comparator<T>,
{
    /// Creates an empty heap ordered by `compare`.
    #[must_use]
    pub fn with_comparator(compare: C) -> Self {
        Self::with_capacity_and_comparator(DEFAULT_HEAP_CAPACITY, compare)
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

    /// Builds a min-max heap from `values` in linear time using `compare`.
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

    /// Returns a reference to a maximum value, if present.
    #[must_use]
    pub fn peek_max(&self) -> Option<&T> {
        self.max_index().map(|index| &self.values[index])
    }

    /// Removes and returns a maximum value, if present.
    pub fn pop_max(&mut self) -> Option<T> {
        let index = self.max_index()?;
        let value = self.values.swap_remove(index);
        if index < self.values.len() {
            self.fix_down_max(index);
        }
        Some(value)
    }

    /// Consumes the heap and returns its backing storage in heap order.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.values
    }

    fn heapify(&mut self) {
        for index in (0..self.values.len() / 2).rev() {
            self.fix_down(index);
        }
    }

    fn max_index(&self) -> Option<usize> {
        match self.values.len() {
            0 => None,
            1 => Some(0),
            2 => Some(1),
            _ => {
                if self.compare.compare(&self.values[2], &self.values[1]) == Ordering::Greater {
                    Some(2)
                } else {
                    Some(1)
                }
            }
        }
    }

    fn on_min_level(index: usize) -> bool {
        (index + 1).ilog2().is_multiple_of(2)
    }

    fn fix_up(&mut self, index: usize) {
        if index == 0 {
            return;
        }
        let parent = (index - 1) / 2;
        if Self::on_min_level(index) {
            if self
                .compare
                .compare(&self.values[index], &self.values[parent])
                == Ordering::Greater
            {
                self.values.swap(index, parent);
                self.bubble_up_max(parent);
            } else {
                self.bubble_up_min(index);
            }
        } else if self
            .compare
            .compare(&self.values[index], &self.values[parent])
            == Ordering::Less
        {
            self.values.swap(index, parent);
            self.bubble_up_min(parent);
        } else {
            self.bubble_up_max(index);
        }
    }

    fn bubble_up_min(&mut self, mut index: usize) {
        while index >= 3 {
            let grandparent = (index - 3) / 4;
            if self
                .compare
                .compare(&self.values[index], &self.values[grandparent])
                != Ordering::Less
            {
                return;
            }
            self.values.swap(index, grandparent);
            index = grandparent;
        }
    }

    fn bubble_up_max(&mut self, mut index: usize) {
        while index >= 3 {
            let grandparent = (index - 3) / 4;
            if self
                .compare
                .compare(&self.values[index], &self.values[grandparent])
                != Ordering::Greater
            {
                return;
            }
            self.values.swap(index, grandparent);
            index = grandparent;
        }
    }

    fn fix_down(&mut self, index: usize) {
        if Self::on_min_level(index) {
            self.fix_down_min(index);
        } else {
            self.fix_down_max(index);
        }
    }

    fn fix_down_min(&mut self, mut index: usize) {
        while let Some(candidate) = self.best_descendant(index, Ordering::Less) {
            let first_grandchild = index
                .checked_mul(4)
                .and_then(|value| value.checked_add(3))
                .unwrap_or(self.values.len());
            if candidate >= first_grandchild {
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[index])
                    != Ordering::Less
                {
                    return;
                }
                self.values.swap(index, candidate);
                let parent = (candidate - 1) / 2;
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[parent])
                    == Ordering::Greater
                {
                    self.values.swap(candidate, parent);
                }
                index = candidate;
            } else {
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[index])
                    == Ordering::Less
                {
                    self.values.swap(index, candidate);
                }
                return;
            }
        }
    }

    fn fix_down_max(&mut self, mut index: usize) {
        while let Some(candidate) = self.best_descendant(index, Ordering::Greater) {
            let first_grandchild = index
                .checked_mul(4)
                .and_then(|value| value.checked_add(3))
                .unwrap_or(self.values.len());
            if candidate >= first_grandchild {
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[index])
                    != Ordering::Greater
                {
                    return;
                }
                self.values.swap(index, candidate);
                let parent = (candidate - 1) / 2;
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[parent])
                    == Ordering::Less
                {
                    self.values.swap(candidate, parent);
                }
                index = candidate;
            } else {
                if self
                    .compare
                    .compare(&self.values[candidate], &self.values[index])
                    == Ordering::Greater
                {
                    self.values.swap(index, candidate);
                }
                return;
            }
        }
    }

    fn best_descendant(&self, index: usize, wanted: Ordering) -> Option<usize> {
        let first_child = index.checked_mul(2)?.checked_add(1)?;
        if first_child >= self.values.len() {
            return None;
        }
        let second_child = first_child + 1;
        let first_grandchild = index.checked_mul(4).and_then(|value| value.checked_add(3));
        let mut best = first_child;
        let candidates = [second_child].into_iter().chain(
            first_grandchild
                .into_iter()
                .flat_map(|first| first..first.saturating_add(4)),
        );
        for candidate in candidates.filter(|&candidate| candidate < self.values.len()) {
            if self
                .compare
                .compare(&self.values[candidate], &self.values[best])
                == wanted
            {
                best = candidate;
            }
        }
        Some(best)
    }
}

impl<T, C> Heap<T> for MinMaxBinaryArrayDoubleEndedHeap<T, C>
where
    C: Comparator<T>,
{
    fn push(&mut self, value: T) {
        self.values.push(value);
        self.fix_up(self.values.len() - 1);
    }

    fn peek(&self) -> Option<&T> {
        self.values.first()
    }

    fn pop(&mut self) -> Option<T> {
        if self.values.is_empty() {
            return None;
        }
        let value = self.values.swap_remove(0);
        if !self.values.is_empty() {
            self.fix_down(0);
        }
        Some(value)
    }

    fn len(&self) -> usize {
        self.values.len()
    }

    fn clear(&mut self) {
        self.values.clear();
    }
}

impl<T, C> DoubleEndedHeap<T> for MinMaxBinaryArrayDoubleEndedHeap<T, C>
where
    C: Comparator<T>,
{
    fn peek_max(&self) -> Option<&T> {
        Self::peek_max(self)
    }

    fn pop_max(&mut self) -> Option<T> {
        Self::pop_max(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heapify_and_alternating_removals_work_at_all_small_sizes() {
        for size in 0..128 {
            let mut heap =
                MinMaxBinaryArrayDoubleEndedHeap::from_vec((0..size).rev().collect::<Vec<_>>());
            let mut minimum = 0;
            let mut maximum = size;
            let mut remove_minimum = true;
            while minimum < maximum {
                if remove_minimum {
                    assert_eq!(heap.pop(), Some(minimum));
                    minimum += 1;
                } else {
                    maximum -= 1;
                    assert_eq!(heap.pop_max(), Some(maximum), "initial size {size}");
                }
                remove_minimum = !remove_minimum;
            }
            assert!(heap.is_empty());
        }
    }

    #[test]
    fn incremental_alternating_removals_work_at_all_small_sizes() {
        for size in 0..128 {
            let mut heap = MinMaxBinaryArrayDoubleEndedHeap::new();
            for value in (0..size).rev() {
                heap.push(value);
            }
            let mut minimum = 0;
            let mut maximum = size;
            while minimum < maximum {
                assert_eq!(heap.pop(), Some(minimum));
                minimum += 1;
                if minimum < maximum {
                    maximum -= 1;
                    assert_eq!(heap.pop_max(), Some(maximum));
                }
            }
            assert!(heap.is_empty());
        }
    }
}
