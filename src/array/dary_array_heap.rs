use core::cmp::Ordering;
use core::fmt;

use crate::Heap;
use crate::array::{Comparator, NaturalOrder};

const DEFAULT_HEAP_CAPACITY: usize = 16;

/// Error returned when a d-ary heap has fewer than two children per node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidDegree(pub usize);

impl fmt::Display for InvalidDegree {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "d-ary heaps must have at least 2 children per node; got {}",
            self.0
        )
    }
}

impl std::error::Error for InvalidDegree {}

/// An array-backed d-ary min-heap.
///
/// Larger degrees reduce the height of the heap, making insertion cheaper, but
/// require more comparisons while removing a minimum value.
#[derive(Clone, Debug)]
pub struct DaryArrayHeap<T, C = NaturalOrder> {
    values: Vec<T>,
    degree: usize,
    compare: C,
}

impl<T: Ord> DaryArrayHeap<T> {
    /// Creates an empty heap with `degree` children per node.
    pub fn new(degree: usize) -> Result<Self, InvalidDegree> {
        Self::with_capacity(degree, DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    pub fn with_capacity(degree: usize, capacity: usize) -> Result<Self, InvalidDegree> {
        Self::with_capacity_and_comparator(degree, capacity, NaturalOrder)
    }

    /// Builds a heap from `values` in linear time.
    pub fn from_vec(degree: usize, values: Vec<T>) -> Result<Self, InvalidDegree> {
        Self::from_vec_by(degree, values, NaturalOrder)
    }
}

impl<T: Ord> Default for DaryArrayHeap<T> {
    fn default() -> Self {
        Self::new(2).expect("binary degree is valid")
    }
}

impl<T, C> DaryArrayHeap<T, C>
where
    C: Comparator<T>,
{
    /// Creates an empty heap ordered by `compare`.
    pub fn with_comparator(degree: usize, compare: C) -> Result<Self, InvalidDegree> {
        Self::with_capacity_and_comparator(degree, DEFAULT_HEAP_CAPACITY, compare)
    }

    /// Creates an empty heap with at least `capacity` value slots, ordered by
    /// `compare`.
    pub fn with_capacity_and_comparator(
        degree: usize,
        capacity: usize,
        compare: C,
    ) -> Result<Self, InvalidDegree> {
        Self::validate_degree(degree)?;
        Ok(Self {
            values: Vec::with_capacity(capacity),
            degree,
            compare,
        })
    }

    /// Builds a heap from `values` in linear time using `compare`.
    pub fn from_vec_by(degree: usize, values: Vec<T>, compare: C) -> Result<Self, InvalidDegree> {
        Self::validate_degree(degree)?;
        let mut heap = Self {
            values,
            degree,
            compare,
        };
        heap.heapify();
        Ok(heap)
    }

    /// Returns the number of children per node.
    #[must_use]
    pub const fn degree(&self) -> usize {
        self.degree
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

    fn validate_degree(degree: usize) -> Result<(), InvalidDegree> {
        if degree < 2 {
            return Err(InvalidDegree(degree));
        }
        Ok(())
    }

    fn heapify(&mut self) {
        let last_parent = self.values.len().saturating_sub(2) / self.degree;
        for index in (0..=last_parent).rev() {
            self.sift_down(index);
        }
    }

    fn sift_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) / self.degree;
            if self
                .compare
                .compare(&self.values[parent], &self.values[index])
                != Ordering::Greater
            {
                return;
            }
            self.values.swap(parent, index);
            index = parent;
        }
    }

    fn sift_down(&mut self, mut index: usize) {
        loop {
            let first_child = index
                .checked_mul(self.degree)
                .and_then(|value| value.checked_add(1))
                .unwrap_or(self.values.len());
            if first_child >= self.values.len() {
                return;
            }

            let end = first_child
                .saturating_add(self.degree)
                .min(self.values.len());
            let mut smallest = first_child;
            for child in first_child + 1..end {
                if self
                    .compare
                    .compare(&self.values[child], &self.values[smallest])
                    == Ordering::Less
                {
                    smallest = child;
                }
            }

            if self
                .compare
                .compare(&self.values[index], &self.values[smallest])
                != Ordering::Greater
            {
                return;
            }
            self.values.swap(index, smallest);
            index = smallest;
        }
    }
}

impl<T, C> Heap<T> for DaryArrayHeap<T, C>
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
