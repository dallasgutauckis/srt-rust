//! Congestion Control for SRT
//!
//! Implements rate-based congestion control with bandwidth estimation
//! and adaptive window management.

use std::time::{Duration, Instant};

/// Congestion control state
#[derive(Debug, Clone)]
pub struct CongestionController {
    /// Maximum sending rate (bytes per second)
    max_bandwidth_bps: u64,
    /// Current sending rate (bytes per second)
    current_bandwidth_bps: u64,
    /// Flow window size (packets)
    flow_window: u32,
    /// Congestion window size (packets)
    congestion_window: u32,
    /// Maximum packet size (bytes)
    max_packet_size: usize,
    /// Slow start threshold
    ssthresh: u32,
    /// In slow start phase
    slow_start: bool,
    /// Number of packets in flight
    packets_in_flight: u32,
    /// Last congestion event time
    last_congestion_event: Option<Instant>,
    /// Minimum congestion event interval
    min_congestion_interval: Duration,
    /// Packet delivery rate (packets per second)
    packet_delivery_rate: f64,
    /// Last update time
    last_update: Instant,
}

impl CongestionController {
    /// Create a new congestion controller
    ///
    /// # Arguments
    /// * `max_bandwidth_bps` - Maximum bandwidth in bits per second
    /// * `max_packet_size` - Maximum packet size in bytes
    /// * `flow_window` - Flow window size in packets
    pub fn new(max_bandwidth_bps: u64, max_packet_size: usize, flow_window: u32) -> Self {
        let initial_cwnd = 16; // Initial congestion window

        CongestionController {
            max_bandwidth_bps,
            current_bandwidth_bps: max_bandwidth_bps / 2, // Start conservative
            flow_window,
            congestion_window: initial_cwnd,
            max_packet_size,
            ssthresh: flow_window / 2,
            slow_start: true,
            packets_in_flight: 0,
            last_congestion_event: None,
            min_congestion_interval: Duration::from_secs(1),
            packet_delivery_rate: 0.0,
            last_update: Instant::now(),
        }
    }

    /// Get current sending rate in bytes per second
    pub fn sending_rate_bps(&self) -> u64 {
        self.current_bandwidth_bps
    }

    /// Get current congestion window size
    pub fn congestion_window(&self) -> u32 {
        self.congestion_window
    }

    /// Get effective window (minimum of flow window and congestion window)
    pub fn effective_window(&self) -> u32 {
        self.flow_window.min(self.congestion_window)
    }

    /// Check if we can send a packet
    pub fn can_send(&self) -> bool {
        self.packets_in_flight < self.effective_window()
    }

    /// Get number of packets that can be sent
    pub fn packets_allowed(&self) -> u32 {
        self.effective_window().saturating_sub(self.packets_in_flight)
    }

    /// Record packet sent
    pub fn on_packet_sent(&mut self) {
        self.packets_in_flight += 1;
    }

    /// Record packet acknowledged
    pub fn on_ack(&mut self, acked_packets: u32, rtt_us: u32) {
        self.packets_in_flight = self.packets_in_flight.saturating_sub(acked_packets);

        // Update congestion window
        if self.slow_start {
            // Slow start: increase cwnd by number of acked packets
            self.congestion_window += acked_packets;

            // Exit slow start if we reach ssthresh
            if self.congestion_window >= self.ssthresh {
                self.slow_start = false;
            }
        } else {
            // Congestion avoidance: increase cwnd by 1/cwnd for each ACK
            let increment = (acked_packets as f64 / self.congestion_window as f64).ceil() as u32;
            self.congestion_window += increment.max(1);
        }

        // Cap at flow window
        self.congestion_window = self.congestion_window.min(self.flow_window);

        // Update bandwidth estimate
        self.update_bandwidth_estimate(rtt_us);
    }

    /// Record packet loss (NAK received)
    pub fn on_loss(&mut self, lost_packets: u32) {
        // Check if enough time has passed since last congestion event
        let should_reduce = match self.last_congestion_event {
            None => true,
            Some(last) => last.elapsed() >= self.min_congestion_interval,
        };

        if should_reduce {
            // Multiplicative decrease
            self.ssthresh = self.congestion_window / 2;
            self.congestion_window = self.ssthresh.max(2);
            self.slow_start = false;

            // Reduce bandwidth estimate
            self.current_bandwidth_bps = (self.current_bandwidth_bps * 3) / 4;

            self.last_congestion_event = Some(Instant::now());
        }

        // Remove lost packets from in-flight count
        self.packets_in_flight = self.packets_in_flight.saturating_sub(lost_packets);
    }

    /// Update bandwidth estimate based on RTT
    fn update_bandwidth_estimate(&mut self, rtt_us: u32) {
        if rtt_us == 0 {
            return;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        self.last_update = now;

        if elapsed.as_secs() == 0 {
            return;
        }

        // Calculate delivery rate (packets per second)
        let rtt_sec = rtt_us as f64 / 1_000_000.0;
        let delivery_rate = self.congestion_window as f64 / rtt_sec;

        // Exponential moving average
        let alpha = 0.125;
        self.packet_delivery_rate = if self.packet_delivery_rate == 0.0 {
            delivery_rate
        } else {
            alpha * delivery_rate + (1.0 - alpha) * self.packet_delivery_rate
        };

        // Convert to bandwidth (bytes per second)
        let estimated_bps = (self.packet_delivery_rate * self.max_packet_size as f64) as u64;

        // Update current bandwidth with some headroom
        self.current_bandwidth_bps = estimated_bps.min(self.max_bandwidth_bps);
    }

    /// Update flow window (from peer's available buffer)
    pub fn update_flow_window(&mut self, new_flow_window: u32) {
        self.flow_window = new_flow_window;
        // Adjust congestion window if needed
        self.congestion_window = self.congestion_window.min(self.flow_window);
    }

    /// Get inter-packet interval for pacing
    pub fn inter_packet_interval(&self) -> Duration {
        if self.current_bandwidth_bps == 0 {
            return Duration::from_micros(1000); // 1ms default
        }

        // Calculate interval between packets
        let bytes_per_sec = self.current_bandwidth_bps;
        let packets_per_sec = bytes_per_sec / self.max_packet_size as u64;

        if packets_per_sec == 0 {
            return Duration::from_micros(1000);
        }

        let interval_us = 1_000_000 / packets_per_sec;
        Duration::from_micros(interval_us)
    }

    /// Reset to initial state
    pub fn reset(&mut self) {
        self.congestion_window = 16;
        self.ssthresh = self.flow_window / 2;
        self.slow_start = true;
        self.packets_in_flight = 0;
        self.current_bandwidth_bps = self.max_bandwidth_bps / 2;
        self.packet_delivery_rate = 0.0;
        self.last_congestion_event = None;
    }

    /// Get statistics
    pub fn stats(&self) -> CongestionStats {
        CongestionStats {
            congestion_window: self.congestion_window,
            flow_window: self.flow_window,
            packets_in_flight: self.packets_in_flight,
            current_bandwidth_bps: self.current_bandwidth_bps,
            slow_start: self.slow_start,
            ssthresh: self.ssthresh,
        }
    }
}

/// Congestion control statistics
#[derive(Debug, Clone, Copy)]
pub struct CongestionStats {
    /// Current congestion window
    pub congestion_window: u32,
    /// Current flow window
    pub flow_window: u32,
    /// Packets currently in flight
    pub packets_in_flight: u32,
    /// Current bandwidth estimate (bytes per second)
    pub current_bandwidth_bps: u64,
    /// Whether in slow start phase
    pub slow_start: bool,
    /// Slow start threshold
    pub ssthresh: u32,
}

/// Bandwidth estimator
///
/// Estimates available bandwidth based on packet delivery.
pub struct BandwidthEstimator {
    /// Samples of delivered packets
    samples: Vec<BandwidthSample>,
    /// Maximum samples to keep
    max_samples: usize,
    /// Estimated bandwidth (bytes per second)
    estimated_bps: u64,
}

#[derive(Debug, Clone)]
struct BandwidthSample {
    delivered_bytes: u64,
    timestamp: Instant,
    _rtt_us: u32,
}

impl BandwidthEstimator {
    /// Create a new bandwidth estimator
    pub fn new() -> Self {
        BandwidthEstimator {
            samples: Vec::new(),
            max_samples: 10,
            estimated_bps: 0,
        }
    }

    /// Add a bandwidth sample
    pub fn add_sample(&mut self, delivered_bytes: u64, rtt_us: u32) {
        let sample = BandwidthSample {
            delivered_bytes,
            timestamp: Instant::now(),
            _rtt_us: rtt_us,
        };

        self.samples.push(sample);

        // Keep only recent samples
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }

        self.update_estimate();
    }

    /// Update bandwidth estimate
    fn update_estimate(&mut self) {
        if self.samples.len() < 2 {
            return;
        }

        // Calculate delivery rate from recent samples
        let mut total_bytes = 0u64;
        let mut total_time_us = 0u64;

        for i in 1..self.samples.len() {
            total_bytes += self.samples[i].delivered_bytes;
            let time_diff = self.samples[i]
                .timestamp
                .duration_since(self.samples[i - 1].timestamp);
            total_time_us += time_diff.as_micros() as u64;
        }

        if total_time_us > 0 {
            // bytes / (time_us / 1_000_000) = bytes per second
            self.estimated_bps = (total_bytes * 1_000_000) / total_time_us;
        }
    }

    /// Get estimated bandwidth
    pub fn estimated_bandwidth_bps(&self) -> u64 {
        self.estimated_bps
    }
}

impl Default for BandwidthEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_congestion_controller_creation() {
        let cc = CongestionController::new(10_000_000, 1456, 8192);

        assert_eq!(cc.congestion_window(), 16); // Initial cwnd
        assert!(cc.can_send());
    }

    #[test]
    fn test_slow_start() {
        let mut cc = CongestionController::new(10_000_000, 1456, 8192);

        // Send some packets
        for _ in 0..10 {
            cc.on_packet_sent();
        }

        assert_eq!(cc.packets_in_flight, 10);

        // ACK all packets
        cc.on_ack(10, 50_000);

        // Congestion window should have grown
        assert!(cc.congestion_window() > 16);
    }

    #[test]
    fn test_congestion_avoidance() {
        let mut cc = CongestionController::new(10_000_000, 1456, 8192);

        // Force exit slow start
        cc.slow_start = false;
        cc.congestion_window = 100;

        let initial_cwnd = cc.congestion_window();

        // ACK some packets
        cc.on_ack(10, 50_000);

        // Window should grow, but slower than slow start
        assert!(cc.congestion_window() > initial_cwnd);
        assert!(cc.congestion_window() < initial_cwnd + 10);
    }

    #[test]
    fn test_loss_handling() {
        let mut cc = CongestionController::new(10_000_000, 1456, 8192);

        cc.congestion_window = 100;
        cc.packets_in_flight = 50;

        let initial_cwnd = cc.congestion_window();

        // Report loss
        cc.on_loss(5);

        // Congestion window should decrease
        assert!(cc.congestion_window() < initial_cwnd);
        assert_eq!(cc.packets_in_flight, 45); // Lost packets removed from flight
    }

    #[test]
    fn test_pacing() {
        let cc = CongestionController::new(10_000_000, 1456, 8192);

        let interval = cc.inter_packet_interval();
        assert!(interval > Duration::ZERO);
        assert!(interval < Duration::from_millis(100));
    }

    #[test]
    fn test_bandwidth_estimator() {
        let mut estimator = BandwidthEstimator::new();

        // Add samples
        estimator.add_sample(1456, 50_000);
        std::thread::sleep(Duration::from_millis(10));
        estimator.add_sample(1456, 50_000);

        let bw = estimator.estimated_bandwidth_bps();
        assert!(bw > 0);
    }

    #[test]
    fn test_flow_window_update() {
        let mut cc = CongestionController::new(10_000_000, 1456, 8192);

        cc.congestion_window = 5000;
        cc.update_flow_window(1000);

        // Congestion window should be capped at flow window
        assert_eq!(cc.congestion_window(), 1000);
    }
}
