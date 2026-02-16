//! ACK and NAK packet generation
//!
//! Implements the generation of ACK (acknowledgment) and NAK (negative acknowledgment)
//! control packets for reliable data transfer.

use crate::loss::LossRange;
use crate::packet::{ControlPacket, ControlType};
use crate::sequence::SeqNumber;
use bytes::{BufMut, Bytes, BytesMut};
use std::time::{Duration, Instant};

/// ACK packet information
#[derive(Debug, Clone)]
pub struct AckInfo {
    /// Sequence number being acknowledged (up to and including this)
    pub ack_seq: SeqNumber,
    /// Round-trip time in microseconds
    pub rtt_us: u32,
    /// RTT variance in microseconds
    pub rtt_var_us: u32,
    /// Available buffer size (packets)
    pub buffer_available: u32,
    /// Packet arrival rate (packets per second)
    pub packet_arrival_rate: u32,
    /// Estimated link capacity (packets per second)
    pub estimated_link_capacity: u32,
    /// Receive rate (bytes per second)
    pub receive_rate_bps: u32,
}

impl AckInfo {
    /// Create a new ACK info
    pub fn new(ack_seq: SeqNumber) -> Self {
        AckInfo {
            ack_seq,
            rtt_us: 0,
            rtt_var_us: 0,
            buffer_available: 8192,
            packet_arrival_rate: 0,
            estimated_link_capacity: 0,
            receive_rate_bps: 0,
        }
    }

    /// Serialize ACK info to control packet data
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(32);

        // ACK sequence number
        buf.put_u32(self.ack_seq.as_raw());

        // RTT (microseconds)
        buf.put_u32(self.rtt_us);

        // RTT variance
        buf.put_u32(self.rtt_var_us);

        // Available buffer size
        buf.put_u32(self.buffer_available);

        // Packet arrival rate
        buf.put_u32(self.packet_arrival_rate);

        // Estimated link capacity
        buf.put_u32(self.estimated_link_capacity);

        // Receive rate
        buf.put_u32(self.receive_rate_bps);

        buf.freeze()
    }

    /// Parse ACK info from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 28 {
            return None;
        }

        let mut buf = bytes;
        use bytes::Buf;

        Some(AckInfo {
            ack_seq: SeqNumber::new_unchecked(buf.get_u32()),
            rtt_us: buf.get_u32(),
            rtt_var_us: buf.get_u32(),
            buffer_available: buf.get_u32(),
            packet_arrival_rate: buf.get_u32(),
            estimated_link_capacity: buf.get_u32(),
            receive_rate_bps: buf.get_u32(),
        })
    }
}

/// NAK packet information
#[derive(Debug, Clone)]
pub struct NakInfo {
    /// Lost packet ranges
    pub loss_ranges: Vec<LossRange>,
}

impl NakInfo {
    /// Create a new NAK info
    pub fn new(loss_ranges: Vec<LossRange>) -> Self {
        NakInfo { loss_ranges }
    }

    /// Serialize NAK info to control packet data
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();

        for range in &self.loss_ranges {
            if range.is_single() {
                // Single lost packet
                buf.put_u32(range.start.as_raw());
            } else {
                // Range of lost packets
                // First packet has bit 31 set to indicate range start
                buf.put_u32(range.start.as_raw() | 0x8000_0000);
                // Last packet
                buf.put_u32(range.end.as_raw());
            }
        }

        buf.freeze()
    }

    /// Parse NAK info from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut loss_ranges = Vec::new();
        let mut buf = bytes;

        use bytes::Buf;

        while buf.remaining() >= 4 {
            let first = buf.get_u32();

            if (first & 0x8000_0000) != 0 {
                // Range indicator
                if buf.remaining() < 4 {
                    return None;
                }
                let start = SeqNumber::new_unchecked(first & 0x7FFF_FFFF);
                let end = SeqNumber::new_unchecked(buf.get_u32());
                loss_ranges.push(LossRange::new(start, end));
            } else {
                // Single packet
                let seq = SeqNumber::new_unchecked(first);
                loss_ranges.push(LossRange::single(seq));
            }
        }

        Some(NakInfo { loss_ranges })
    }
}

/// ACK generator
///
/// Generates periodic ACK packets based on received data.
pub struct AckGenerator {
    /// Last ACK sequence number sent
    last_ack_seq: SeqNumber,
    /// Last ACK send time
    last_ack_time: Instant,
    /// ACK interval
    ack_interval: Duration,
    /// ACK sequence number (increments with each ACK sent)
    ack_number: u32,
}

impl AckGenerator {
    /// Create a new ACK generator
    pub fn new(ack_interval: Duration) -> Self {
        AckGenerator {
            last_ack_seq: SeqNumber::new(0),
            last_ack_time: Instant::now(),
            ack_interval,
            ack_number: 0,
        }
    }

    /// Check if ACK should be sent
    pub fn should_send_ack(&self, current_seq: SeqNumber) -> bool {
        // Send ACK if:
        // 1. Enough time has passed since last ACK
        // 2. OR sequence number has advanced significantly
        let time_elapsed = self.last_ack_time.elapsed() >= self.ack_interval;
        let seq_advanced = current_seq.distance_to(self.last_ack_seq).abs() >= 64;

        time_elapsed || seq_advanced
    }

    /// Generate an ACK packet
    pub fn generate_ack(&mut self, ack_info: AckInfo, dest_socket_id: u32) -> ControlPacket {
        self.last_ack_seq = ack_info.ack_seq;
        self.last_ack_time = Instant::now();

        let ack_data = ack_info.to_bytes();

        // Increment ACK number
        self.ack_number = self.ack_number.wrapping_add(1);

        ControlPacket::new(
            ControlType::Ack,
            (self.ack_number & 0xFFFF) as u16, // ACK sequence number in type-specific field
            ack_info.ack_seq.as_raw(),         // Last acknowledged packet
            0,                                 // Timestamp
            dest_socket_id,
            ack_data,
        )
    }

    /// Get last ACK sequence number
    pub fn last_ack_seq(&self) -> SeqNumber {
        self.last_ack_seq
    }
}

/// NAK generator
///
/// Generates NAK packets for lost packets.
pub struct NakGenerator {
    /// Last NAK send time
    last_nak_time: Instant,
    /// Minimum NAK interval
    min_nak_interval: Duration,
}

impl NakGenerator {
    /// Create a new NAK generator
    pub fn new(min_nak_interval: Duration) -> Self {
        NakGenerator {
            // Initialize to past time so first NAK can be sent immediately
            last_nak_time: Instant::now() - min_nak_interval,
            min_nak_interval,
        }
    }

    /// Check if NAK can be sent
    pub fn can_send_nak(&self) -> bool {
        self.last_nak_time.elapsed() >= self.min_nak_interval
    }

    /// Generate a NAK packet
    pub fn generate_nak(
        &mut self,
        nak_info: NakInfo,
        dest_socket_id: u32,
    ) -> Option<ControlPacket> {
        if !self.can_send_nak() || nak_info.loss_ranges.is_empty() {
            return None;
        }

        self.last_nak_time = Instant::now();

        let nak_data = nak_info.to_bytes();

        Some(ControlPacket::new(
            ControlType::Nak,
            0,
            0,
            0, // Timestamp
            dest_socket_id,
            nak_data,
        ))
    }
}

/// RTT (Round-Trip Time) estimator
///
/// Tracks RTT measurements and calculates smoothed RTT and variance.
pub struct RttEstimator {
    /// Smoothed RTT (microseconds)
    srtt: f64,
    /// RTT variance (microseconds)
    rtt_var: f64,
    /// Number of samples
    sample_count: u32,
}

impl RttEstimator {
    /// Create a new RTT estimator
    pub fn new() -> Self {
        RttEstimator {
            srtt: 100_000.0, // Initial estimate: 100ms
            rtt_var: 50_000.0,
            sample_count: 0,
        }
    }

    /// Update with a new RTT sample
    pub fn update(&mut self, rtt_sample_us: u32) {
        let sample = rtt_sample_us as f64;

        if self.sample_count == 0 {
            // First sample
            self.srtt = sample;
            self.rtt_var = sample / 2.0;
        } else {
            // Exponential moving average
            let alpha = 0.125; // Smoothing factor for SRTT
            let beta = 0.25; // Smoothing factor for variance

            let error = sample - self.srtt;
            self.srtt += alpha * error;
            self.rtt_var = (1.0 - beta) * self.rtt_var + beta * error.abs();
        }

        self.sample_count += 1;
    }

    /// Get smoothed RTT in microseconds
    pub fn srtt(&self) -> u32 {
        self.srtt as u32
    }

    /// Get RTT variance in microseconds
    pub fn rtt_var(&self) -> u32 {
        self.rtt_var as u32
    }

    /// Get retransmission timeout (RTO)
    ///
    /// RTO = SRTT + 4 * RTT_VAR
    pub fn rto(&self) -> Duration {
        let rto_us = self.srtt + 4.0 * self.rtt_var;
        Duration::from_micros(rto_us as u64)
    }
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_info_serialization() {
        let mut ack = AckInfo::new(SeqNumber::new(1000));
        ack.rtt_us = 50_000;
        ack.buffer_available = 4096;

        let bytes = ack.to_bytes();
        let decoded = AckInfo::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.ack_seq, ack.ack_seq);
        assert_eq!(decoded.rtt_us, ack.rtt_us);
        assert_eq!(decoded.buffer_available, ack.buffer_available);
    }

    #[test]
    fn test_nak_info_single() {
        let nak = NakInfo::new(vec![LossRange::single(SeqNumber::new(100))]);

        let bytes = nak.to_bytes();
        let decoded = NakInfo::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.loss_ranges.len(), 1);
        assert_eq!(decoded.loss_ranges[0].start, SeqNumber::new(100));
        assert!(decoded.loss_ranges[0].is_single());
    }

    #[test]
    fn test_nak_info_range() {
        let nak = NakInfo::new(vec![LossRange::new(
            SeqNumber::new(100),
            SeqNumber::new(105),
        )]);

        let bytes = nak.to_bytes();
        let decoded = NakInfo::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.loss_ranges.len(), 1);
        assert_eq!(decoded.loss_ranges[0].start, SeqNumber::new(100));
        assert_eq!(decoded.loss_ranges[0].end, SeqNumber::new(105));
    }

    #[test]
    fn test_ack_generator() {
        let mut gen = AckGenerator::new(Duration::from_millis(10));

        assert!(gen.should_send_ack(SeqNumber::new(100)));

        let ack = gen.generate_ack(AckInfo::new(SeqNumber::new(100)), 9999);
        assert_eq!(ack.control_type(), ControlType::Ack);

        // Should not send immediately after
        assert!(!gen.should_send_ack(SeqNumber::new(101)));
    }

    #[test]
    fn test_nak_generator() {
        let mut gen = NakGenerator::new(Duration::from_millis(10));

        let nak_info = NakInfo::new(vec![LossRange::single(SeqNumber::new(100))]);
        let nak = gen.generate_nak(nak_info.clone(), 9999);

        assert!(nak.is_some());
        assert_eq!(nak.unwrap().control_type(), ControlType::Nak);

        // Should not send immediately after
        let nak2 = gen.generate_nak(nak_info, 9999);
        assert!(nak2.is_none());
    }

    #[test]
    fn test_rtt_estimator() {
        let mut estimator = RttEstimator::new();

        // Add some samples
        estimator.update(100_000); // 100ms
        estimator.update(120_000); // 120ms
        estimator.update(90_000); // 90ms

        let srtt = estimator.srtt();
        assert!(srtt > 90_000 && srtt < 120_000);

        let rto = estimator.rto();
        assert!(rto > Duration::from_millis(100));
    }
}
