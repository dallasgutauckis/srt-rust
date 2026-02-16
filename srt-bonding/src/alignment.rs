//! Packet Alignment for Multi-Path Reception
//!
//! Aligns sequence numbers across multiple paths, detects and eliminates
//! duplicates, and reorders packets for in-order delivery.

use srt_protocol::{DataPacket, SeqNumber};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Alignment errors
#[derive(Error, Debug)]
pub enum AlignmentError {
    #[error("Buffer is full")]
    BufferFull,

    #[error("Packet is too old")]
    TooOld,

    #[error("Invalid sequence number")]
    InvalidSequence,
}

/// Packet source information
#[derive(Debug, Clone)]
pub struct PacketSource {
    /// Member ID that delivered this packet
    pub member_id: u32,
    /// Reception timestamp
    pub received_at: Instant,
    /// RTT estimate for this path (microseconds)
    pub rtt_us: u32,
}

/// Aligned packet (with source tracking)
#[derive(Debug, Clone)]
pub struct AlignedPacket {
    /// The packet data
    pub packet: DataPacket,
    /// Source that delivered this packet first
    pub source: PacketSource,
    /// All sources that delivered this packet (for duplicate detection)
    pub duplicate_sources: Vec<PacketSource>,
}

/// Packet alignment buffer
///
/// Receives packets from multiple paths, detects duplicates,
/// and delivers packets in sequence order.
pub struct AlignmentBuffer {
    /// Buffered packets indexed by sequence number
    buffer: BTreeMap<SeqNumber, AlignedPacket>,
    /// Next expected sequence number for delivery
    next_expected: SeqNumber,
    /// Maximum buffer size
    max_buffer_size: usize,
    /// Maximum age for buffered packets
    max_packet_age: Duration,
    /// Statistics
    stats: AlignmentStats,
}

impl AlignmentBuffer {
    /// Create a new alignment buffer
    pub fn new(max_buffer_size: usize, max_packet_age: Duration) -> Self {
        AlignmentBuffer {
            buffer: BTreeMap::new(),
            next_expected: SeqNumber::new(0),
            max_buffer_size,
            max_packet_age,
            stats: AlignmentStats::default(),
        }
    }

    /// Add a packet from a specific path
    ///
    /// Returns true if this is a new packet (not a duplicate).
    pub fn add_packet(
        &mut self,
        packet: DataPacket,
        member_id: u32,
        rtt_us: u32,
    ) -> Result<bool, AlignmentError> {
        let seq = packet.seq_number();

        // Check if packet is too old
        if seq.lt(self.next_expected) {
            self.stats.packets_too_old += 1;
            return Err(AlignmentError::TooOld);
        }

        // Check buffer size
        if self.buffer.len() >= self.max_buffer_size {
            // Try to clean up old packets first
            self.cleanup_old_packets();

            if self.buffer.len() >= self.max_buffer_size {
                self.stats.buffer_full_events += 1;
                return Err(AlignmentError::BufferFull);
            }
        }

        let source = PacketSource {
            member_id,
            received_at: Instant::now(),
            rtt_us,
        };

        // Check if we already have this packet
        if let Some(existing) = self.buffer.get_mut(&seq) {
            // Duplicate packet
            existing.duplicate_sources.push(source);
            self.stats.duplicates_detected += 1;
            Ok(false)
        } else {
            // New packet
            let aligned = AlignedPacket {
                packet,
                source,
                duplicate_sources: Vec::new(),
            };

            self.buffer.insert(seq, aligned);
            self.stats.packets_received += 1;
            Ok(true)
        }
    }

    /// Get next packet in sequence order
    ///
    /// Returns None if the next packet is not yet available.
    pub fn pop_next(&mut self) -> Option<AlignedPacket> {
        if let Some(aligned) = self.buffer.remove(&self.next_expected) {
            self.next_expected = self.next_expected.next();
            self.stats.packets_delivered += 1;
            Some(aligned)
        } else {
            None
        }
    }

    /// Get all packets that are ready for delivery (in order)
    pub fn pop_ready_packets(&mut self) -> Vec<AlignedPacket> {
        let mut ready = Vec::new();

        while let Some(aligned) = self.buffer.remove(&self.next_expected) {
            self.next_expected = self.next_expected.next();
            self.stats.packets_delivered += 1;
            ready.push(aligned);
        }

        ready
    }

    /// Clean up packets that are too old
    fn cleanup_old_packets(&mut self) {
        let now = Instant::now();
        let max_age = self.max_packet_age;

        self.buffer.retain(|_, aligned| {
            let age = now.duration_since(aligned.source.received_at);
            if age > max_age {
                self.stats.packets_expired += 1;
                false
            } else {
                true
            }
        });
    }

    /// Get missing sequence numbers (gaps in received packets)
    pub fn get_missing_sequences(&self) -> Vec<SeqNumber> {
        if self.buffer.is_empty() {
            return Vec::new();
        }

        let mut missing = Vec::new();
        let mut current = self.next_expected;

        // Find gaps up to the highest received packet
        if let Some((&highest, _)) = self.buffer.iter().next_back() {
            while current.lt(highest) {
                if !self.buffer.contains_key(&current) {
                    missing.push(current);
                }
                current = current.next();
            }
        }

        missing
    }

    /// Get buffer statistics
    pub fn stats(&self) -> &AlignmentStats {
        &self.stats
    }

    /// Get current buffer utilization
    pub fn utilization(&self) -> f32 {
        self.buffer.len() as f32 / self.max_buffer_size as f32
    }

    /// Get buffered packet count
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Get next expected sequence number
    pub fn next_expected(&self) -> SeqNumber {
        self.next_expected
    }

    /// Set next expected sequence number (for synchronization)
    pub fn set_next_expected(&mut self, seq: SeqNumber) {
        self.next_expected = seq;
    }
}

/// Alignment statistics
#[derive(Debug, Clone, Default)]
pub struct AlignmentStats {
    /// Total packets received
    pub packets_received: u64,
    /// Total packets delivered in order
    pub packets_delivered: u64,
    /// Duplicate packets detected
    pub duplicates_detected: u64,
    /// Packets that were too old
    pub packets_too_old: u64,
    /// Packets that expired before delivery
    pub packets_expired: u64,
    /// Buffer full events
    pub buffer_full_events: u64,
}

impl AlignmentStats {
    /// Get duplication rate (0.0 to 1.0)
    pub fn duplication_rate(&self) -> f64 {
        if self.packets_received == 0 {
            0.0
        } else {
            self.duplicates_detected as f64 / self.packets_received as f64
        }
    }

    /// Get delivery efficiency (delivered / received)
    pub fn delivery_efficiency(&self) -> f64 {
        if self.packets_received == 0 {
            0.0
        } else {
            self.packets_delivered as f64 / self.packets_received as f64
        }
    }
}

/// Path statistics for alignment
#[derive(Debug, Clone)]
pub struct PathStats {
    /// Path identifier (member ID)
    pub path_id: u32,
    /// Packets received from this path
    pub packets_received: u64,
    /// Packets that were first from this path
    pub packets_first: u64,
    /// Average RTT (microseconds)
    pub avg_rtt_us: u32,
}

/// Multi-path alignment tracker
///
/// Tracks which paths are delivering packets and their performance.
pub struct PathTracker {
    /// Statistics per path
    paths: HashMap<u32, PathStats>,
}

impl PathTracker {
    /// Create a new path tracker
    pub fn new() -> Self {
        PathTracker {
            paths: HashMap::new(),
        }
    }

    /// Record packet reception from a path
    pub fn record_packet(&mut self, path_id: u32, was_first: bool, rtt_us: u32) {
        let stats = self.paths.entry(path_id).or_insert_with(|| PathStats {
            path_id,
            packets_received: 0,
            packets_first: 0,
            avg_rtt_us: 0,
        });

        stats.packets_received += 1;
        if was_first {
            stats.packets_first += 1;
        }

        // Update average RTT (exponential moving average)
        if stats.avg_rtt_us == 0 {
            stats.avg_rtt_us = rtt_us;
        } else {
            stats.avg_rtt_us = ((stats.avg_rtt_us as u64 * 7 + rtt_us as u64) / 8) as u32;
        }
    }

    /// Get statistics for a path
    pub fn get_stats(&self, path_id: u32) -> Option<&PathStats> {
        self.paths.get(&path_id)
    }

    /// Get all path statistics
    pub fn all_stats(&self) -> Vec<&PathStats> {
        self.paths.values().collect()
    }

    /// Get fastest path (by average RTT)
    pub fn fastest_path(&self) -> Option<u32> {
        self.paths
            .values()
            .min_by_key(|s| s.avg_rtt_us)
            .map(|s| s.path_id)
    }

    /// Get path with most first deliveries
    pub fn most_reliable_path(&self) -> Option<u32> {
        self.paths
            .values()
            .max_by_key(|s| s.packets_first)
            .map(|s| s.path_id)
    }
}

impl Default for PathTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use srt_protocol::MsgNumber;

    fn create_test_packet(seq: u32) -> DataPacket {
        DataPacket::new(
            SeqNumber::new(seq),
            MsgNumber::new(seq),
            0,
            0,
            bytes::Bytes::from(format!("Packet {}", seq)),
        )
    }

    #[test]
    fn test_alignment_in_order() {
        let mut buffer = AlignmentBuffer::new(1024, Duration::from_secs(10));

        // Add packets in order
        for i in 0..5 {
            let packet = create_test_packet(i);
            let is_new = buffer.add_packet(packet, 1, 50_000).unwrap();
            assert!(is_new);
        }

        // All should be deliverable
        let ready = buffer.pop_ready_packets();
        assert_eq!(ready.len(), 5);
    }

    #[test]
    fn test_alignment_out_of_order() {
        let mut buffer = AlignmentBuffer::new(1024, Duration::from_secs(10));

        // Add packets out of order: 0, 2, 1, 3
        buffer.add_packet(create_test_packet(0), 1, 50_000).unwrap();
        buffer.add_packet(create_test_packet(2), 1, 50_000).unwrap();

        // Only packet 0 should be deliverable
        let ready = buffer.pop_ready_packets();
        assert_eq!(ready.len(), 1);

        // Add packet 1
        buffer.add_packet(create_test_packet(1), 1, 50_000).unwrap();

        // Now packets 1 and 2 should be deliverable
        let ready = buffer.pop_ready_packets();
        assert_eq!(ready.len(), 2);
    }

    #[test]
    fn test_duplicate_detection() {
        let mut buffer = AlignmentBuffer::new(1024, Duration::from_secs(10));

        let packet = create_test_packet(0);

        // First reception - should be new
        let is_new1 = buffer.add_packet(packet.clone(), 1, 50_000).unwrap();
        assert!(is_new1);

        // Second reception from different path - should be duplicate
        let is_new2 = buffer.add_packet(packet, 2, 60_000).unwrap();
        assert!(!is_new2);

        assert_eq!(buffer.stats().duplicates_detected, 1);
    }

    #[test]
    fn test_missing_sequences() {
        let mut buffer = AlignmentBuffer::new(1024, Duration::from_secs(10));

        // Add packets 0, 2, 3 (missing 1)
        buffer.add_packet(create_test_packet(0), 1, 50_000).unwrap();
        buffer.add_packet(create_test_packet(2), 1, 50_000).unwrap();
        buffer.add_packet(create_test_packet(3), 1, 50_000).unwrap();

        buffer.pop_next(); // Pop packet 0

        let missing = buffer.get_missing_sequences();
        assert_eq!(missing, vec![SeqNumber::new(1)]);
    }

    #[test]
    fn test_path_tracker() {
        let mut tracker = PathTracker::new();

        // Path 1 delivers first
        tracker.record_packet(1, true, 50_000);
        tracker.record_packet(2, false, 60_000);

        // Path 2 delivers first
        tracker.record_packet(2, true, 55_000);
        tracker.record_packet(1, false, 52_000);

        let stats1 = tracker.get_stats(1).unwrap();
        assert_eq!(stats1.packets_received, 2);
        assert_eq!(stats1.packets_first, 1);

        // Path 1 should be fastest (lower average RTT)
        assert_eq!(tracker.fastest_path(), Some(1));
    }

    #[test]
    fn test_buffer_full() {
        let mut buffer = AlignmentBuffer::new(2, Duration::from_secs(10));

        // Fill buffer
        buffer.add_packet(create_test_packet(0), 1, 50_000).unwrap();
        buffer.add_packet(create_test_packet(1), 1, 50_000).unwrap();

        // Should fail - buffer full
        let result = buffer.add_packet(create_test_packet(2), 1, 50_000);
        assert!(matches!(result, Err(AlignmentError::BufferFull)));
    }

    #[test]
    fn test_statistics() {
        let mut buffer = AlignmentBuffer::new(1024, Duration::from_secs(10));

        buffer.add_packet(create_test_packet(0), 1, 50_000).unwrap();
        buffer.add_packet(create_test_packet(0), 2, 60_000).unwrap(); // Duplicate

        let stats = buffer.stats();
        assert_eq!(stats.packets_received, 1);
        assert_eq!(stats.duplicates_detected, 1);
        assert_eq!(stats.duplication_rate(), 1.0);
    }
}
