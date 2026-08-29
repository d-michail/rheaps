use crate::Heap;

/// An array-backed binary min-heap.
#[derive(Clone, Debug)]
pub struct BinaryArrayHeap<T> {
    values: Vec<T>,
}

impl<T: Ord> BinaryArrayHeap<T> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(16)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
        }
    }

    /// Builds a heap from `values` in linear time.
    #[must_use]
    pub fn from_vec(values: Vec<T>) -> Self {
        let mut heap = Self { values };
        heap.heapify();
        heap
    }
}

impl<T: Ord> Default for BinaryArrayHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> FromIterator<T> for BinaryArrayHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl<T: Ord> Extend<T> for BinaryArrayHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<T: Ord> BinaryArrayHeap<T> {
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
            if self.values[parent] <= self.values[index] {
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
            let child = if right < len && self.values[right] < self.values[left] {
                right
            } else {
                left
            };

            if self.values[index] <= self.values[child] {
                return;
            }
            self.values.swap(index, child);
            index = child;
        }
    }
}

impl<T: Ord> Heap<T> for BinaryArrayHeap<T> {
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
