use crate::Heap;

const DEFAULT_HEAP_CAPACITY: usize = 16;
const INSERTION_BUFFER_CAPACITY: usize = 34;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct WeakHeapCore<T> {
    values: Vec<T>,
    reverse: Vec<bool>,
}

impl<T: Ord> WeakHeapCore<T> {
    fn new(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            reverse: Vec::with_capacity(capacity),
        }
    }

    fn from_vec(values: Vec<T>) -> Self {
        let mut heap = Self {
            reverse: vec![false; values.len()],
            values,
        };
        for index in (1..heap.values.len()).rev() {
            let ancestor = heap.distinguished_ancestor(index);
            heap.join(ancestor, index);
        }
        heap
    }

    fn push(&mut self, value: T) {
        let index = self.append_unfixed(value);
        self.fix_up(index);
    }

    fn append_unfixed(&mut self, value: T) -> usize {
        let index = self.values.len();
        self.values.push(value);
        if self.reverse.len() <= index {
            self.reverse.resize(index + 1, false);
        }
        self.reverse[index] = false;
        if index.is_multiple_of(2) {
            self.reverse[index / 2] = false;
        }
        index
    }

    fn append_bulk_unfixed(&mut self, value: T) {
        let index = self.values.len();
        self.values.push(value);
        if self.reverse.len() <= index {
            self.reverse.resize(index + 1, false);
        }
        self.reverse[index] = false;
    }

    fn pop(&mut self) -> Option<T> {
        if self.values.is_empty() {
            return None;
        }
        let result = self.values.swap_remove(0);
        if self.values.len() > 1 {
            self.fix_down(0);
        }
        Some(result)
    }

    fn clear(&mut self) {
        self.values.clear();
        self.reverse.clear();
    }

    fn distinguished_ancestor(&self, mut index: usize) -> usize {
        while index > 0 && (index % 2 == 1) == self.reverse[index / 2] {
            index /= 2;
        }
        index / 2
    }

    fn join(&mut self, first: usize, second: usize) -> bool {
        if self.values[second] < self.values[first] {
            self.values.swap(first, second);
            self.reverse[second] = !self.reverse[second];
            false
        } else {
            true
        }
    }

    fn fix_up(&mut self, mut index: usize) {
        while index > 0 {
            let ancestor = self.distinguished_ancestor(index);
            if self.join(ancestor, index) {
                break;
            }
            index = ancestor;
        }
    }

    fn fix_down(&mut self, root: usize) {
        let mut index = 2 * root + usize::from(!self.reverse[root]);
        if index >= self.values.len() {
            return;
        }
        loop {
            let child = 2 * index + usize::from(self.reverse[index]);
            if child >= self.values.len() {
                break;
            }
            index = child;
        }
        while index != root {
            self.join(root, index);
            index /= 2;
        }
    }
}

/// An array-backed binary weak min-heap.
///
/// A weak heap stores one reverse bit per position and uses at most one
/// comparison per level during insertion. `push` and `pop` are `O(log n)`;
/// construction from a vector is `O(n)`.
///
/// ```
/// use rheaps::Heap;
/// use rheaps::array::BinaryArrayWeakHeap;
///
/// let mut heap = BinaryArrayWeakHeap::new();
/// heap.push(4);
/// heap.push(1);
/// heap.push(3);
///
/// assert_eq!(heap.pop(), Some(1));
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinaryArrayWeakHeap<T> {
    inner: WeakHeapCore<T>,
}

impl<T: Ord> BinaryArrayWeakHeap<T> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: WeakHeapCore::new(capacity),
        }
    }

    /// Builds a weak heap from `values` in linear time.
    #[must_use]
    pub fn from_vec(values: Vec<T>) -> Self {
        Self {
            inner: WeakHeapCore::from_vec(values),
        }
    }
}

impl<T: Ord> Default for BinaryArrayWeakHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> FromIterator<T> for BinaryArrayWeakHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl<T: Ord> Extend<T> for BinaryArrayWeakHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<T: Ord> BinaryArrayWeakHeap<T> {
    /// Consumes the heap and returns its backing values in heap order.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.inner.values
    }

    /// Iterates over the heap's values in internal heap order, not priority
    /// order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.values.iter()
    }
}

impl<T: Ord> IntoIterator for BinaryArrayWeakHeap<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    /// Iterates over the heap's values in internal heap order, not priority
    /// order.
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<'a, T: Ord> IntoIterator for &'a BinaryArrayWeakHeap<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.values.iter()
    }
}

impl<T: Ord> Heap<T> for BinaryArrayWeakHeap<T> {
    fn push(&mut self, value: T) {
        self.inner.push(value);
    }

    fn peek(&self) -> Option<&T> {
        self.inner.values.first()
    }

    fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }

    fn len(&self) -> usize {
        self.inner.values.len()
    }

    fn clear(&mut self) {
        self.inner.clear();
    }
}

/// A binary weak heap that batches insertions before integrating them.
///
/// Insertions first enter a small buffer, giving amortized `O(1)` insertion
/// work. The buffer's minimum is tracked, so [`Heap::peek`] remains `O(1)`.
/// A full buffer is integrated using the weak-heap bulk insertion algorithm.
///
/// ```
/// use rheaps::Heap;
/// use rheaps::array::BinaryArrayBulkInsertWeakHeap;
///
/// let mut heap = BinaryArrayBulkInsertWeakHeap::new();
/// heap.push(4);
/// heap.push(1);
/// heap.push(3);
///
/// assert_eq!(heap.pop(), Some(1));
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BinaryArrayBulkInsertWeakHeap<T> {
    inner: WeakHeapCore<T>,
    insertion_buffer: Vec<T>,
    insertion_buffer_min: usize,
}

impl<T: Ord> BinaryArrayBulkInsertWeakHeap<T> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with at least `capacity` value slots.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: WeakHeapCore::new(capacity),
            insertion_buffer: Vec::with_capacity(INSERTION_BUFFER_CAPACITY),
            insertion_buffer_min: 0,
        }
    }

    /// Builds a heap from `values` in linear time.
    #[must_use]
    pub fn from_vec(values: Vec<T>) -> Self {
        Self {
            inner: WeakHeapCore::from_vec(values),
            insertion_buffer: Vec::with_capacity(INSERTION_BUFFER_CAPACITY),
            insertion_buffer_min: 0,
        }
    }
}

impl<T: Ord> Default for BinaryArrayBulkInsertWeakHeap<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord> FromIterator<T> for BinaryArrayBulkInsertWeakHeap<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl<T: Ord> Extend<T> for BinaryArrayBulkInsertWeakHeap<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<T: Ord> BinaryArrayBulkInsertWeakHeap<T> {
    /// Consumes the heap and returns values in heap order.
    ///
    /// Pending buffered values are integrated before the backing vector is
    /// returned.
    #[must_use]
    pub fn into_vec(mut self) -> Vec<T> {
        self.bulk_insert();
        self.inner.values
    }

    /// Iterates over the heap's values in internal heap order, not priority
    /// order.
    ///
    /// Buffered values that have not yet been integrated are included, but
    /// appear after the integrated values rather than in heap order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.values.iter().chain(self.insertion_buffer.iter())
    }

    fn buffer_is_full(&self) -> bool {
        if self.insertion_buffer.len() >= INSERTION_BUFFER_CAPACITY {
            return true;
        }
        let total = self.inner.values.len() + self.insertion_buffer.len();
        total > 0 && total.ilog2() as usize + 3 >= INSERTION_BUFFER_CAPACITY
    }

    fn recompute_buffer_minimum(&mut self) {
        self.insertion_buffer_min = 0;
        for index in 1..self.insertion_buffer.len() {
            if self.insertion_buffer[index] < self.insertion_buffer[self.insertion_buffer_min] {
                self.insertion_buffer_min = index;
            }
        }
    }

    fn bulk_insert(&mut self) {
        if self.insertion_buffer.is_empty() {
            return;
        }
        let old_size = self.inner.values.len();
        let count = self.insertion_buffer.len();
        if old_size + count == 1 {
            let value = self
                .insertion_buffer
                .pop()
                .expect("a non-empty insertion buffer has one value");
            self.inner.append_unfixed(value);
            self.insertion_buffer_min = 0;
            return;
        }
        let mut right = old_size + count - 2;
        let mut left = old_size.max(right / 2);

        while let Some(value) = self.insertion_buffer.pop() {
            self.inner.append_bulk_unfixed(value);
        }

        while right > left + 1 {
            left /= 2;
            right /= 2;
            for index in left..=right {
                self.inner.fix_down(index);
            }
        }
        if left != 0 {
            let index = self.inner.distinguished_ancestor(left);
            self.inner.fix_down(index);
            self.inner.fix_up(index);
        }
        if right != 0 {
            let index = self.inner.distinguished_ancestor(right);
            self.inner.fix_down(index);
            self.inner.fix_up(index);
        }
        self.insertion_buffer_min = 0;
    }
}

impl<T: Ord> IntoIterator for BinaryArrayBulkInsertWeakHeap<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    /// Iterates over the heap's values in internal heap order, not priority
    /// order. Pending buffered values are integrated first.
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<'a, T: Ord> IntoIterator for &'a BinaryArrayBulkInsertWeakHeap<T> {
    type Item = &'a T;
    type IntoIter = core::iter::Chain<std::slice::Iter<'a, T>, std::slice::Iter<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.values.iter().chain(self.insertion_buffer.iter())
    }
}

impl<T: Ord> Heap<T> for BinaryArrayBulkInsertWeakHeap<T> {
    fn push(&mut self, value: T) {
        let index = self.insertion_buffer.len();
        self.insertion_buffer.push(value);
        if self.buffer_is_full() {
            self.bulk_insert();
        } else if index > 0
            && self.insertion_buffer[index] < self.insertion_buffer[self.insertion_buffer_min]
        {
            self.insertion_buffer_min = index;
        }
    }

    fn peek(&self) -> Option<&T> {
        match (
            self.inner.values.first(),
            self.insertion_buffer.get(self.insertion_buffer_min),
        ) {
            (None, None) => None,
            (Some(value), None) => Some(value),
            (None, Some(value)) => Some(value),
            (Some(heap_minimum), Some(buffer_minimum)) => {
                if heap_minimum > buffer_minimum {
                    Some(buffer_minimum)
                } else {
                    Some(heap_minimum)
                }
            }
        }
    }

    fn pop(&mut self) -> Option<T> {
        let remove_from_buffer = match (
            self.inner.values.first(),
            self.insertion_buffer.get(self.insertion_buffer_min),
        ) {
            (None, None) => return None,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(heap_minimum), Some(buffer_minimum)) => buffer_minimum < heap_minimum,
        };
        if remove_from_buffer {
            let value = self.insertion_buffer.swap_remove(self.insertion_buffer_min);
            if !self.insertion_buffer.is_empty() {
                self.recompute_buffer_minimum();
            } else {
                self.insertion_buffer_min = 0;
            }
            Some(value)
        } else {
            self.inner.pop()
        }
    }

    fn len(&self) -> usize {
        self.inner.values.len() + self.insertion_buffer.len()
    }

    fn clear(&mut self) {
        self.inner.clear();
        self.insertion_buffer.clear();
        self.insertion_buffer_min = 0;
    }
}
