//! Sequence Number Handling
//!
//! SRT uses 31-bit sequence numbers (bit 31 is reserved for control/data flag).
//! This module provides a wrapped sequence number type that handles arithmetic
//! with proper wraparound semantics.

use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Maximum sequence number value (31-bit: 0x7FFFFFFF)
pub const MAX_SEQ_NUMBER: u32 = 0x7FFF_FFFF;

/// Sequence number with 31-bit wraparound semantics
///
/// SRT sequence numbers are 31-bit values that wrap around. The comparison
/// and arithmetic operations account for this wraparound to properly handle
/// sequence number ordering even across the wrap boundary.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SeqNumber(u32);

impl SeqNumber {
    /// Create a new sequence number
    ///
    /// # Panics
    /// Panics if value exceeds MAX_SEQ_NUMBER
    pub fn new(value: u32) -> Self {
        assert!(
            value <= MAX_SEQ_NUMBER,
            "Sequence number {} exceeds maximum {}",
            value,
            MAX_SEQ_NUMBER
        );
        SeqNumber(value)
    }

    /// Create a sequence number without bounds checking (unsafe)
    ///
    /// The value will be masked to 31 bits
    #[inline]
    pub fn new_unchecked(value: u32) -> Self {
        SeqNumber(value & MAX_SEQ_NUMBER)
    }

    /// Get the raw sequence number value
    #[inline]
    pub fn as_raw(self) -> u32 {
        self.0
    }

    /// Increment the sequence number by 1
    #[inline]
    pub fn increment(&mut self) {
        self.0 = (self.0 + 1) & MAX_SEQ_NUMBER;
    }

    /// Get the next sequence number
    #[inline]
    pub fn next(self) -> Self {
        SeqNumber((self.0 + 1) & MAX_SEQ_NUMBER)
    }

    /// Calculate the distance from this sequence number to another
    ///
    /// Returns a signed distance that accounts for wraparound. Positive values
    /// mean `other` is ahead of `self`, negative means `other` is behind.
    pub fn distance_to(self, other: SeqNumber) -> i32 {
        let diff = other.0.wrapping_sub(self.0) as i32;

        // Handle wraparound: if the difference is > half the sequence space,
        // it's actually a negative distance in the other direction
        // Note: MAX_SEQ_NUMBER + 1 = 0x80000000, which fits in i64
        const WRAP_OFFSET: i64 = (MAX_SEQ_NUMBER as i64) + 1;
        let half_space = MAX_SEQ_NUMBER as i32 / 2;

        if diff > half_space {
            (diff as i64 - WRAP_OFFSET) as i32
        } else if diff < -half_space {
            (diff as i64 + WRAP_OFFSET) as i32
        } else {
            diff
        }
    }

    /// Check if this sequence number is less than another (accounting for wraparound)
    #[inline]
    pub fn lt(self, other: SeqNumber) -> bool {
        self.distance_to(other) > 0
    }

    /// Check if this sequence number is less than or equal to another
    #[inline]
    pub fn le(self, other: SeqNumber) -> bool {
        self == other || self.lt(other)
    }

    /// Check if this sequence number is greater than another
    #[inline]
    pub fn gt(self, other: SeqNumber) -> bool {
        self.distance_to(other) < 0
    }

    /// Check if this sequence number is greater than or equal to another
    #[inline]
    pub fn ge(self, other: SeqNumber) -> bool {
        self == other || self.gt(other)
    }
}

impl fmt::Debug for SeqNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqNumber({})", self.0)
    }
}

impl fmt::Display for SeqNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u32> for SeqNumber {
    fn from(value: u32) -> Self {
        SeqNumber::new_unchecked(value)
    }
}

impl From<SeqNumber> for u32 {
    fn from(seq: SeqNumber) -> u32 {
        seq.0
    }
}

impl Add<u32> for SeqNumber {
    type Output = SeqNumber;

    fn add(self, rhs: u32) -> SeqNumber {
        SeqNumber::new_unchecked(self.0.wrapping_add(rhs))
    }
}

impl AddAssign<u32> for SeqNumber {
    fn add_assign(&mut self, rhs: u32) {
        *self = SeqNumber::new_unchecked(self.0.wrapping_add(rhs));
    }
}

impl Sub<u32> for SeqNumber {
    type Output = SeqNumber;

    fn sub(self, rhs: u32) -> SeqNumber {
        SeqNumber::new_unchecked(self.0.wrapping_sub(rhs))
    }
}

impl SubAssign<u32> for SeqNumber {
    fn sub_assign(&mut self, rhs: u32) {
        *self = SeqNumber::new_unchecked(self.0.wrapping_sub(rhs));
    }
}

impl Sub for SeqNumber {
    type Output = i32;

    /// Calculate the signed distance between two sequence numbers
    fn sub(self, rhs: SeqNumber) -> i32 {
        rhs.distance_to(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let seq = SeqNumber::new(100);
        assert_eq!(seq.as_raw(), 100);
    }

    #[test]
    #[should_panic]
    fn test_new_overflow() {
        SeqNumber::new(MAX_SEQ_NUMBER + 1);
    }

    #[test]
    fn test_new_unchecked() {
        let seq = SeqNumber::new_unchecked(MAX_SEQ_NUMBER + 100);
        assert_eq!(seq.as_raw(), 99); // Wrapped around
    }

    #[test]
    fn test_increment() {
        let mut seq = SeqNumber::new(100);
        seq.increment();
        assert_eq!(seq.as_raw(), 101);
    }

    #[test]
    fn test_increment_wraparound() {
        let mut seq = SeqNumber::new(MAX_SEQ_NUMBER);
        seq.increment();
        assert_eq!(seq.as_raw(), 0);
    }

    #[test]
    fn test_next() {
        let seq = SeqNumber::new(100);
        assert_eq!(seq.next().as_raw(), 101);
    }

    #[test]
    fn test_distance_simple() {
        let a = SeqNumber::new(100);
        let b = SeqNumber::new(200);
        assert_eq!(a.distance_to(b), 100);
        assert_eq!(b.distance_to(a), -100);
    }

    #[test]
    fn test_distance_wraparound() {
        let a = SeqNumber::new(MAX_SEQ_NUMBER - 10);
        let b = SeqNumber::new(10);
        // b is 21 ahead of a (wrapping around)
        assert_eq!(a.distance_to(b), 21);
        assert_eq!(b.distance_to(a), -21);
    }

    #[test]
    fn test_comparison() {
        let a = SeqNumber::new(100);
        let b = SeqNumber::new(200);

        assert!(a.lt(b));
        assert!(a.le(b));
        assert!(b.gt(a));
        assert!(b.ge(a));
        assert!(a.le(a));
        assert!(a.ge(a));
    }

    #[test]
    fn test_comparison_wraparound() {
        let a = SeqNumber::new(MAX_SEQ_NUMBER - 10);
        let b = SeqNumber::new(10);

        assert!(a.lt(b)); // a < b because b is ahead after wraparound
        assert!(b.gt(a));
    }

    #[test]
    fn test_add() {
        let seq = SeqNumber::new(100);
        let result = seq + 50;
        assert_eq!(result.as_raw(), 150);
    }

    #[test]
    fn test_add_wraparound() {
        let seq = SeqNumber::new(MAX_SEQ_NUMBER - 10);
        let result = seq + 20;
        assert_eq!(result.as_raw(), 9);
    }

    #[test]
    fn test_sub() {
        let seq = SeqNumber::new(100);
        let result = seq - 50;
        assert_eq!(result.as_raw(), 50);
    }

    #[test]
    fn test_sub_wraparound() {
        let seq = SeqNumber::new(10);
        let result = seq - 20;
        assert_eq!(result.as_raw(), MAX_SEQ_NUMBER - 9);
    }

    #[test]
    fn test_sub_seqnumbers() {
        let a = SeqNumber::new(200);
        let b = SeqNumber::new(100);
        assert_eq!(a - b, 100);
        assert_eq!(b - a, -100);
    }
}
