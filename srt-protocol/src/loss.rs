//! Loss list tracking for SRT
//!
//! Tracks lost packets for NAK (Negative Acknowledgment) generation and
//! retransmission scheduling.

use crate::sequence::SeqNumber;
use std::time::Instant;

/// Loss sequence range (inclusive)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossRange {
    /// First sequence number in range
    pub start: SeqNumber,
    /// Last sequence number in range (inclusive)
    pub end: SeqNumber,
}

impl LossRange {
    /// Create a new loss range
    pub fn new(start: SeqNumber, end: SeqNumber) -> Self {
        LossRange { start, end }
    }

    /// Create a single-packet loss range
    pub fn single(seq: SeqNumber) -> Self {
        LossRange { start: seq, end: seq }
    }

    /// Check if this range contains a sequence number
    pub fn contains(&self, seq: SeqNumber) -> bool {
        seq.ge(self.start) && seq.le(self.end)
    }

    /// Get the length of this range
    pub fn len(&self) -> usize {
        (self.end.as_raw().wrapping_sub(self.start.as_raw()) + 1) as usize
    }

    /// Check if this is a single packet
    pub fn is_single(&self) -> bool {
        self.start == self.end
    }

    /// Merge with another range if they overlap or are adjacent
    pub fn try_merge(&self, other: &LossRange) -> Option<LossRange> {
        // Check if ranges overlap or are adjacent
        if other.start.le(self.end.next()) && other.end.ge(self.start - 1) {
            Some(LossRange {
                start: if self.start.lt(other.start) {
                    self.start
                } else {
                    other.start
                },
                end: if self.end.gt(other.end) {
                    self.end
                } else {
                    other.end
                },
            })
        } else {
            None
        }
    }
}

/// Loss entry with timing information
#[derive(Debug, Clone)]
struct LossEntry {
    /// Sequence number range
    range: LossRange,
    /// Time when loss was first detected
    detected_at: Instant,
    /// Time when NAK was last sent
    last_nak_sent: Option<Instant>,
    /// Number of NAKs sent for this loss
    nak_count: u32,
}

/// Loss list for tracking packet losses
///
/// Used by both sender (for retransmission) and receiver (for NAK generation).
pub struct LossList {
    /// Loss entries sorted by sequence number
    losses: Vec<LossEntry>,
    /// Maximum number of NAKs to send for a single loss
    max_nak_count: u32,
    /// Minimum interval between NAKs for the same loss
    nak_interval: std::time::Duration,
}

impl LossList {
    /// Create a new loss list
    pub fn new(max_nak_count: u32, nak_interval: std::time::Duration) -> Self {
        LossList {
            losses: Vec::new(),
            max_nak_count,
            nak_interval,
        }
    }

    /// Add a lost packet
    pub fn add(&mut self, seq: SeqNumber) {
        self.add_range(LossRange::single(seq));
    }

    /// Add a range of lost packets
    pub fn add_range(&mut self, range: LossRange) {
        let entry = LossEntry {
            range,
            detected_at: Instant::now(),
            last_nak_sent: None,
            nak_count: 0,
        };

        // Insert in sorted order and try to merge with adjacent ranges
        let mut merged = entry;
        let mut new_losses = Vec::new();

        for existing in self.losses.drain(..) {
            if let Some(merged_range) = merged.range.try_merge(&existing.range) {
                // Merge the ranges
                merged.range = merged_range;
                // Keep the earlier detection time
                if existing.detected_at < merged.detected_at {
                    merged.detected_at = existing.detected_at;
                }
                // Sum NAK counts
                merged.nak_count = merged.nak_count.max(existing.nak_count);
            } else if existing.range.start.lt(merged.range.start) {
                // This existing range comes before the new one
                new_losses.push(existing);
            } else {
                // This existing range comes after, push merged and continue with existing
                new_losses.push(merged);
                merged = existing;
            }
        }

        new_losses.push(merged);
        self.losses = new_losses;
    }

    /// Remove a sequence number (packet recovered)
    pub fn remove(&mut self, seq: SeqNumber) {
        let mut new_losses = Vec::new();

        for entry in self.losses.drain(..) {
            if !entry.range.contains(seq) {
                // This range doesn't contain the sequence, keep it
                new_losses.push(entry);
            } else {
                // Split the range if needed
                if entry.range.is_single() {
                    // Single packet, remove entirely
                    continue;
                } else if seq == entry.range.start {
                    // Remove first packet of range
                    new_losses.push(LossEntry {
                        range: LossRange::new(entry.range.start.next(), entry.range.end),
                        ..entry
                    });
                } else if seq == entry.range.end {
                    // Remove last packet of range
                    new_losses.push(LossEntry {
                        range: LossRange::new(entry.range.start, entry.range.end - 1),
                        ..entry
                    });
                } else {
                    // Remove middle packet, split into two ranges
                    new_losses.push(LossEntry {
                        range: LossRange::new(entry.range.start, seq - 1),
                        detected_at: entry.detected_at,
                        last_nak_sent: entry.last_nak_sent,
                        nak_count: entry.nak_count,
                    });
                    new_losses.push(LossEntry {
                        range: LossRange::new(seq.next(), entry.range.end),
                        detected_at: entry.detected_at,
                        last_nak_sent: entry.last_nak_sent,
                        nak_count: entry.nak_count,
                    });
                }
            }
        }

        self.losses = new_losses;
    }

    /// Remove all losses up to and including a sequence number
    pub fn remove_up_to(&mut self, seq: SeqNumber) {
        self.losses.retain(|entry| entry.range.end.gt(seq));

        // Trim the first range if it starts before seq
        if let Some(first) = self.losses.first_mut() {
            if first.range.start.le(seq) {
                first.range.start = seq.next();
            }
        }
    }

    /// Get ranges that need NAK to be sent
    pub fn get_nak_ranges(&mut self) -> Vec<LossRange> {
        let now = Instant::now();
        let mut ranges = Vec::new();

        for entry in &mut self.losses {
            // Check if we should send NAK
            let should_send = match entry.last_nak_sent {
                None => true, // Never sent NAK for this loss
                Some(last_sent) => {
                    // Check if enough time has passed and we haven't exceeded max count
                    now.duration_since(last_sent) >= self.nak_interval
                        && entry.nak_count < self.max_nak_count
                }
            };

            if should_send {
                ranges.push(entry.range);
                entry.last_nak_sent = Some(now);
                entry.nak_count += 1;
            }
        }

        ranges
    }

    /// Get all loss ranges (for inspection)
    pub fn ranges(&self) -> Vec<LossRange> {
        self.losses.iter().map(|e| e.range).collect()
    }

    /// Get total number of lost packets
    pub fn len(&self) -> usize {
        self.losses.iter().map(|e| e.range.len()).sum()
    }

    /// Check if the loss list is empty
    pub fn is_empty(&self) -> bool {
        self.losses.is_empty()
    }

    /// Clear all losses
    pub fn clear(&mut self) {
        self.losses.clear();
    }

    /// Check if a sequence number is in the loss list
    pub fn contains(&self, seq: SeqNumber) -> bool {
        self.losses.iter().any(|e| e.range.contains(seq))
    }
}

/// Sender loss list
///
/// Tracks packets that need to be retransmitted based on receiver NAKs.
pub struct SenderLossList {
    inner: LossList,
}

impl SenderLossList {
    /// Create a new sender loss list
    pub fn new() -> Self {
        SenderLossList {
            inner: LossList::new(u32::MAX, std::time::Duration::from_millis(0)),
        }
    }

    /// Add a lost packet from NAK
    pub fn add(&mut self, seq: SeqNumber) {
        self.inner.add(seq);
    }

    /// Add a range of lost packets from NAK
    pub fn add_range(&mut self, range: LossRange) {
        self.inner.add_range(range);
    }

    /// Remove a packet (retransmitted)
    pub fn remove(&mut self, seq: SeqNumber) {
        self.inner.remove(seq);
    }

    /// Get next packet to retransmit
    pub fn pop_next(&mut self) -> Option<SeqNumber> {
        if let Some(entry) = self.inner.losses.first() {
            let seq = entry.range.start;
            self.remove(seq);
            Some(seq)
        } else {
            None
        }
    }

    /// Get all packets that need retransmission
    pub fn get_all(&self) -> Vec<SeqNumber> {
        let mut packets = Vec::new();
        for entry in &self.inner.losses {
            let mut seq = entry.range.start;
            while seq.le(entry.range.end) {
                packets.push(seq);
                seq = seq.next();
            }
        }
        packets
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get count of packets to retransmit
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for SenderLossList {
    fn default() -> Self {
        Self::new()
    }
}

/// Receiver loss list
///
/// Tracks detected packet losses for NAK generation.
pub struct ReceiverLossList {
    inner: LossList,
}

impl ReceiverLossList {
    /// Create a new receiver loss list
    ///
    /// # Arguments
    /// * `max_nak_count` - Maximum times to send NAK for a single loss
    /// * `nak_interval` - Minimum interval between NAKs
    pub fn new(max_nak_count: u32, nak_interval: std::time::Duration) -> Self {
        ReceiverLossList {
            inner: LossList::new(max_nak_count, nak_interval),
        }
    }

    /// Add a detected loss
    pub fn add(&mut self, seq: SeqNumber) {
        self.inner.add(seq);
    }

    /// Add a range of detected losses
    pub fn add_range(&mut self, range: LossRange) {
        self.inner.add_range(range);
    }

    /// Remove a recovered packet
    pub fn remove(&mut self, seq: SeqNumber) {
        self.inner.remove(seq);
    }

    /// Get ranges to include in NAK packet
    pub fn get_nak_ranges(&mut self) -> Vec<LossRange> {
        self.inner.get_nak_ranges()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get count of lost packets
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loss_range_merge() {
        let r1 = LossRange::new(SeqNumber::new(10), SeqNumber::new(15));
        let r2 = LossRange::new(SeqNumber::new(16), SeqNumber::new(20));

        // Adjacent ranges should merge
        let merged = r1.try_merge(&r2).unwrap();
        assert_eq!(merged.start, SeqNumber::new(10));
        assert_eq!(merged.end, SeqNumber::new(20));
    }

    #[test]
    fn test_loss_range_no_merge() {
        let r1 = LossRange::new(SeqNumber::new(10), SeqNumber::new(15));
        let r2 = LossRange::new(SeqNumber::new(20), SeqNumber::new(25));

        // Non-adjacent ranges should not merge
        assert!(r1.try_merge(&r2).is_none());
    }

    #[test]
    fn test_loss_list_add_remove() {
        let mut list = LossList::new(3, std::time::Duration::from_millis(100));

        list.add(SeqNumber::new(10));
        list.add(SeqNumber::new(11));
        list.add(SeqNumber::new(12));

        assert_eq!(list.len(), 3);

        list.remove(SeqNumber::new(11));
        assert_eq!(list.len(), 2);

        // Should have split into two ranges
        let ranges = list.ranges();
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_loss_list_merge() {
        let mut list = LossList::new(3, std::time::Duration::from_millis(100));

        list.add(SeqNumber::new(10));
        list.add(SeqNumber::new(12));
        list.add(SeqNumber::new(11)); // Should merge all three

        let ranges = list.ranges();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, SeqNumber::new(10));
        assert_eq!(ranges[0].end, SeqNumber::new(12));
    }

    #[test]
    fn test_sender_loss_list() {
        let mut list = SenderLossList::new();

        list.add(SeqNumber::new(5));
        list.add(SeqNumber::new(7));
        list.add(SeqNumber::new(6));

        assert_eq!(list.len(), 3);

        let next = list.pop_next().unwrap();
        assert_eq!(next, SeqNumber::new(5));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_receiver_loss_list_nak() {
        let mut list = ReceiverLossList::new(3, std::time::Duration::from_millis(10));

        list.add(SeqNumber::new(10));
        list.add(SeqNumber::new(11));

        // First NAK should return the range
        let ranges = list.get_nak_ranges();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, SeqNumber::new(10));
        assert_eq!(ranges[0].end, SeqNumber::new(11));

        // Immediate second NAK should return nothing (too soon)
        let ranges = list.get_nak_ranges();
        assert_eq!(ranges.len(), 0);

        // After waiting, should get NAK again
        std::thread::sleep(std::time::Duration::from_millis(15));
        let ranges = list.get_nak_ranges();
        assert_eq!(ranges.len(), 1);
    }
}
