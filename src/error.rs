//! Error types shared by every addressable-heap capability.
//!
//! These types describe outcomes common to a capability - handle validation,
//! key decreases, key increases - independent of which module implements the
//! heap. A specific heap family (for example, the radix heaps in
//! [`crate::monotone`]) may report a more detailed error from its own
//! inherent methods and convert it into one of these when implementing a
//! shared trait.

use core::fmt;

/// The reason an addressable heap rejected a handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InvalidHandle {
    /// The handle was created by a different heap.
    ForeignHeap,
    /// The entry was removed or the heap was cleared.
    Stale,
}

impl fmt::Display for InvalidHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignHeap => formatter.write_str("handle belongs to another heap"),
            Self::Stale => formatter.write_str("handle no longer identifies a live entry"),
        }
    }
}

impl std::error::Error for InvalidHandle {}

/// An error returned when decreasing an entry's key in a
/// [`DecreaseKeyHeap`](crate::DecreaseKeyHeap).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DecreaseKeyError {
    /// The handle was not valid for this heap.
    InvalidHandle(InvalidHandle),
    /// The proposed key has lower priority than the existing key.
    NotDecreased,
    /// The proposed key violates an implementation-specific key restriction.
    ///
    /// For example, radix heaps require keys to be no less than their last
    /// deleted key.
    InvalidKey,
}

impl fmt::Display for DecreaseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(error) => error.fmt(formatter),
            Self::NotDecreased => {
                formatter.write_str("new key must not be greater than the old key")
            }
            Self::InvalidKey => formatter.write_str("new key violates the heap's key restrictions"),
        }
    }
}

impl std::error::Error for DecreaseKeyError {}

/// An error returned when increasing an entry's key in a double-ended
/// addressable heap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IncreaseKeyError {
    /// The handle was not valid for this heap.
    InvalidHandle(InvalidHandle),
    /// The proposed key has higher priority than the existing key.
    NotIncreased,
}

impl fmt::Display for IncreaseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandle(error) => error.fmt(formatter),
            Self::NotIncreased => formatter.write_str("new key must not be less than the old key"),
        }
    }
}

impl std::error::Error for IncreaseKeyError {}
