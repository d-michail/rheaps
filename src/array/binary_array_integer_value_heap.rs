use crate::ValueHeap;

const DEFAULT_HEAP_CAPACITY: usize = 16;

/// An array-backed min-heap with `i32` keys and associated values.
///
/// Keys are stored directly. Insertion
/// and removal are `O(log n)` and construction from a vector is `O(n)`.
#[derive(Clone, Debug)]
pub struct BinaryArrayIntegerValueHeap<V> {
    entries: Vec<(i32, V)>,
}

impl<V> BinaryArrayIntegerValueHeap<V> {
    /// Creates an empty heap.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HEAP_CAPACITY)
    }

    /// Creates an empty heap with storage for at least `capacity` entries.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Builds a heap from key-value pairs in linear time.
    #[must_use]
    pub fn from_vec(entries: Vec<(i32, V)>) -> Self {
        let mut heap = Self { entries };
        heap.heapify();
        heap
    }

    /// Inserts `key` and its associated `value`.
    pub fn push(&mut self, key: i32, value: V) {
        self.entries.push((key, value));
        self.sift_up(self.entries.len() - 1);
    }

    /// Alias for [`Self::push`], matching JHeaps terminology.
    pub fn insert(&mut self, key: i32, value: V) {
        self.push(key, value);
    }

    /// Returns the minimum key and its associated value.
    #[must_use]
    pub fn peek(&self) -> Option<(&i32, &V)> {
        self.entries.first().map(|(key, value)| (key, value))
    }

    /// Returns the minimum key.
    #[must_use]
    pub fn peek_key(&self) -> Option<&i32> {
        self.entries.first().map(|(key, _)| key)
    }

    /// Returns the value associated with the minimum key.
    #[must_use]
    pub fn peek_value(&self) -> Option<&V> {
        self.entries.first().map(|(_, value)| value)
    }

    /// Removes and returns the minimum key-value pair.
    pub fn pop(&mut self) -> Option<(i32, V)> {
        if self.entries.is_empty() {
            return None;
        }
        let entry = self.entries.swap_remove(0);
        if !self.entries.is_empty() {
            self.sift_down(0);
        }
        Some(entry)
    }

    /// Returns the number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the heap contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes every entry from the heap.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Consumes the heap and returns its backing entries in heap order.
    #[must_use]
    pub fn into_vec(self) -> Vec<(i32, V)> {
        self.entries
    }

    /// Iterates over the heap's entries in internal heap order, not priority
    /// order.
    pub fn iter(&self) -> impl Iterator<Item = &(i32, V)> {
        self.entries.iter()
    }

    fn heapify(&mut self) {
        for index in (0..self.entries.len() / 2).rev() {
            self.sift_down(index);
        }
    }

    fn sift_up(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) / 2;
            if self.entries[parent].0 <= self.entries[index].0 {
                break;
            }
            self.entries.swap(parent, index);
            index = parent;
        }
    }

    fn sift_down(&mut self, mut index: usize) {
        loop {
            let left = index * 2 + 1;
            if left >= self.entries.len() {
                return;
            }
            let right = left + 1;
            let child =
                if right < self.entries.len() && self.entries[right].0 < self.entries[left].0 {
                    right
                } else {
                    left
                };
            if self.entries[index].0 <= self.entries[child].0 {
                return;
            }
            self.entries.swap(index, child);
            index = child;
        }
    }
}

impl<V> Default for BinaryArrayIntegerValueHeap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> FromIterator<(i32, V)> for BinaryArrayIntegerValueHeap<V> {
    fn from_iter<I: IntoIterator<Item = (i32, V)>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

impl<V> Extend<(i32, V)> for BinaryArrayIntegerValueHeap<V> {
    fn extend<I: IntoIterator<Item = (i32, V)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.push(key, value);
        }
    }
}

impl<V> IntoIterator for BinaryArrayIntegerValueHeap<V> {
    type Item = (i32, V);
    type IntoIter = std::vec::IntoIter<(i32, V)>;

    /// Iterates over the heap's entries in internal heap order, not priority
    /// order.
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<'a, V> IntoIterator for &'a BinaryArrayIntegerValueHeap<V> {
    type Item = &'a (i32, V);
    type IntoIter = std::slice::Iter<'a, (i32, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<V> ValueHeap<i32, V> for BinaryArrayIntegerValueHeap<V> {
    fn push(&mut self, key: i32, value: V) {
        Self::push(self, key, value);
    }

    fn peek(&self) -> Option<(&i32, &V)> {
        Self::peek(self)
    }

    fn pop(&mut self) -> Option<(i32, V)> {
        Self::pop(self)
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn clear(&mut self) {
        Self::clear(self);
    }
}
