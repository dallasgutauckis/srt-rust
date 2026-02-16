//! Load Balancing for Multi-Path Transmission
//!
//! Distributes packets across multiple paths based on bandwidth,
//! RTT, and path health to maximize throughput.

use crate::group::{GroupError, MemberStatus, SocketGroup};
use parking_lot::RwLock;
use srt_protocol::SeqNumber;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Load balancing errors
#[derive(Error, Debug)]
pub enum BalancingError {
    #[error("No active members available")]
    NoActiveMembers,

    #[error("Group error: {0}")]
    Group(#[from] GroupError),

    #[error("All paths failed")]
    AllPathsFailed,
}

/// Path capacity estimate
#[derive(Debug, Clone)]
pub struct PathCapacity {
    /// Path identifier (member ID)
    pub path_id: u32,
    /// Estimated bandwidth (bytes per second)
    pub bandwidth_bps: u64,
    /// Average RTT (microseconds)
    pub rtt_us: u32,
    /// Packet loss rate (0.0 to 1.0)
    pub loss_rate: f64,
    /// Current load (packets in flight)
    pub packets_in_flight: u32,
    /// Last capacity update
    pub last_update: Instant,
}

impl PathCapacity {
    /// Create new path capacity estimate
    fn new(path_id: u32) -> Self {
        PathCapacity {
            path_id,
            bandwidth_bps: 1_000_000, // Initial estimate: 1 MB/s
            rtt_us: 100_000,           // Initial estimate: 100ms
            loss_rate: 0.0,
            packets_in_flight: 0,
            last_update: Instant::now(),
        }
    }

    /// Calculate path weight for load balancing
    ///
    /// Higher weight = more capacity
    fn calculate_weight(&self) -> f64 {
        if self.loss_rate >= 1.0 {
            return 0.0; // Path is completely broken
        }

        // Weight based on bandwidth and RTT
        let bandwidth_factor = self.bandwidth_bps as f64;
        let rtt_factor = 1.0 / (self.rtt_us as f64 + 1.0);
        let loss_factor = 1.0 - self.loss_rate;

        bandwidth_factor * rtt_factor * loss_factor
    }

    /// Check if path is available for sending
    fn _is_available(&self, max_in_flight: u32) -> bool {
        self.packets_in_flight < max_in_flight && self.loss_rate < 0.5
    }
}

/// Load balancer for multi-path transmission
pub struct LoadBalancer {
    /// Socket group
    group: Arc<SocketGroup>,
    /// Path capacity estimates
    capacities: Arc<RwLock<HashMap<u32, PathCapacity>>>,
    /// Balancing algorithm
    algorithm: BalancingAlgorithm,
    /// Maximum packets in flight per path
    _max_in_flight_per_path: u32,
    /// Capacity update interval
    _capacity_update_interval: Duration,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new(
        group: Arc<SocketGroup>,
        algorithm: BalancingAlgorithm,
        max_in_flight_per_path: u32,
    ) -> Self {
        LoadBalancer {
            group,
            capacities: Arc::new(RwLock::new(HashMap::new())),
            algorithm,
            _max_in_flight_per_path: max_in_flight_per_path,
            _capacity_update_interval: Duration::from_millis(100),
        }
    }

    /// Send data using load balancing
    pub fn send(&self, data: &[u8]) -> Result<BalancingSendResult, BalancingError> {
        let members = self.group.get_active_members();

        if members.is_empty() {
            return Err(BalancingError::NoActiveMembers);
        }

        // Update capacity estimates
        self.update_capacities();

        // Select path based on algorithm
        let selected_path = self.select_path(&members)?;

        // Send on selected path
        let member = self
            .group
            .get_member(selected_path)
            .ok_or(BalancingError::NoActiveMembers)?;

        let sequence = self.group.next_sequence();

        match member.connection.send(data) {
            Ok(_) => {
                member.record_sent(data.len());

                // Update in-flight count
                if let Some(capacity) = self.capacities.write().get_mut(&selected_path) {
                    capacity.packets_in_flight += 1;
                }

                Ok(BalancingSendResult {
                    path_id: selected_path,
                    sequence,
                    bytes_sent: data.len(),
                })
            }
            Err(_) => {
                // Path failed, try another
                self.mark_path_failed(selected_path);

                // Recursively try another path
                self.send(data)
            }
        }
    }

    /// Select a path based on the balancing algorithm
    fn select_path(&self, members: &[Arc<crate::group::GroupMember>]) -> Result<u32, BalancingError> {
        let capacities = self.capacities.read();

        match self.algorithm {
            BalancingAlgorithm::RoundRobin => {
                // Simple round-robin
                static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let index = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % members.len();
                Ok(members[index].connection.local_socket_id())
            }

            BalancingAlgorithm::WeightedRoundRobin => {
                // Select based on bandwidth weights
                let weights: Vec<_> = members
                    .iter()
                    .map(|m| {
                        let id = m.connection.local_socket_id();
                        capacities
                            .get(&id)
                            .map(|c| c.calculate_weight())
                            .unwrap_or(1.0)
                    })
                    .collect();

                let total_weight: f64 = weights.iter().sum();
                if total_weight == 0.0 {
                    return Ok(members[0].connection.local_socket_id());
                }

                // Random weighted selection
                use std::sync::atomic::{AtomicU64, Ordering};
                static RANDOM: AtomicU64 = AtomicU64::new(0);
                let r = RANDOM.fetch_add(12345, Ordering::Relaxed) as f64 / u64::MAX as f64;
                let mut threshold = r * total_weight;

                for (i, &weight) in weights.iter().enumerate() {
                    threshold -= weight;
                    if threshold <= 0.0 {
                        return Ok(members[i].connection.local_socket_id());
                    }
                }

                Ok(members[members.len() - 1].connection.local_socket_id())
            }

            BalancingAlgorithm::LeastLoaded => {
                // Select path with least packets in flight
                members
                    .iter()
                    .filter_map(|m| {
                        let id = m.connection.local_socket_id();
                        capacities.get(&id).map(|c| (id, c.packets_in_flight))
                    })
                    .min_by_key(|(_, in_flight)| *in_flight)
                    .map(|(id, _)| id)
                    .ok_or(BalancingError::NoActiveMembers)
            }

            BalancingAlgorithm::FastestPath => {
                // Select path with lowest RTT
                members
                    .iter()
                    .filter_map(|m| {
                        let id = m.connection.local_socket_id();
                        capacities.get(&id).map(|c| (id, c.rtt_us))
                    })
                    .min_by_key(|(_, rtt)| *rtt)
                    .map(|(id, _)| id)
                    .ok_or(BalancingError::NoActiveMembers)
            }

            BalancingAlgorithm::HighestBandwidth => {
                // Select path with highest bandwidth
                members
                    .iter()
                    .filter_map(|m| {
                        let id = m.connection.local_socket_id();
                        capacities.get(&id).map(|c| (id, c.bandwidth_bps))
                    })
                    .max_by_key(|(_, bw)| *bw)
                    .map(|(id, _)| id)
                    .ok_or(BalancingError::NoActiveMembers)
            }
        }
    }

    /// Update capacity estimates for all paths
    fn update_capacities(&self) {
        let members = self.group.get_active_members();
        let mut capacities = self.capacities.write();

        for member in members {
            let id = member.connection.local_socket_id();
            let stats = member.get_stats();

            let capacity = capacities
                .entry(id)
                .or_insert_with(|| PathCapacity::new(id));

            // Update bandwidth estimate (simplified)
            if stats.bandwidth_bps > 0 {
                capacity.bandwidth_bps = stats.bandwidth_bps;
            }

            // Update RTT
            if stats.rtt_us > 0 {
                capacity.rtt_us = stats.rtt_us;
            }

            capacity.last_update = Instant::now();
        }
    }

    /// Record packet ACK (reduce in-flight count)
    pub fn on_ack(&self, path_id: u32, packets: u32) {
        if let Some(capacity) = self.capacities.write().get_mut(&path_id) {
            capacity.packets_in_flight = capacity.packets_in_flight.saturating_sub(packets);
        }
    }

    /// Record packet loss
    pub fn on_loss(&self, path_id: u32, lost_packets: u32) {
        if let Some(capacity) = self.capacities.write().get_mut(&path_id) {
            // Update loss rate (exponential moving average)
            let loss_event = lost_packets as f64 / (capacity.packets_in_flight.max(1) as f64);
            capacity.loss_rate = 0.9 * capacity.loss_rate + 0.1 * loss_event;

            capacity.packets_in_flight = capacity.packets_in_flight.saturating_sub(lost_packets);
        }
    }

    /// Mark path as failed
    fn mark_path_failed(&self, path_id: u32) {
        if let Some(capacity) = self.capacities.write().get_mut(&path_id) {
            capacity.loss_rate = 1.0; // Mark as completely failed
        }

        // Update member status
        let _ = self.group.update_member_status(path_id, MemberStatus::Broken);
    }

    /// Get balancing statistics
    pub fn stats(&self) -> BalancingStats {
        let capacities = self.capacities.read();
        let path_capacities: Vec<_> = capacities.values().cloned().collect();

        BalancingStats {
            algorithm: self.algorithm,
            path_count: path_capacities.len(),
            path_capacities,
            total_bandwidth_bps: capacities.values().map(|c| c.bandwidth_bps).sum(),
        }
    }
}

/// Load balancing algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalancingAlgorithm {
    /// Simple round-robin (equal distribution)
    RoundRobin,
    /// Weighted round-robin (based on bandwidth)
    WeightedRoundRobin,
    /// Send on least loaded path (fewest packets in flight)
    LeastLoaded,
    /// Send on fastest path (lowest RTT)
    FastestPath,
    /// Send on highest bandwidth path
    HighestBandwidth,
}

/// Balancing send result
#[derive(Debug, Clone)]
pub struct BalancingSendResult {
    /// Path ID used
    pub path_id: u32,
    /// Sequence number
    pub sequence: SeqNumber,
    /// Bytes sent
    pub bytes_sent: usize,
}

/// Balancing statistics
#[derive(Debug, Clone)]
pub struct BalancingStats {
    /// Algorithm used
    pub algorithm: BalancingAlgorithm,
    /// Number of active paths
    pub path_count: usize,
    /// Per-path capacity estimates
    pub path_capacities: Vec<PathCapacity>,
    /// Total available bandwidth (sum of all paths)
    pub total_bandwidth_bps: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::GroupType;
    use srt_protocol::Connection;

    fn create_test_group() -> Arc<SocketGroup> {
        Arc::new(SocketGroup::new(1, GroupType::Balancing, 10))
    }

    #[allow(dead_code)]
    fn create_test_connection(id: u32) -> Arc<Connection> {
        Arc::new(Connection::new(
            id,
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            SeqNumber::new(1000),
            120,
        ))
    }

    #[test]
    fn test_load_balancer_creation() {
        let group = create_test_group();
        let balancer = LoadBalancer::new(group, BalancingAlgorithm::RoundRobin, 100);

        let stats = balancer.stats();
        assert_eq!(stats.algorithm, BalancingAlgorithm::RoundRobin);
    }

    #[test]
    fn test_path_capacity_weight() {
        let mut capacity = PathCapacity::new(1);
        capacity.bandwidth_bps = 10_000_000; // 10 MB/s
        capacity.rtt_us = 50_000;            // 50ms
        capacity.loss_rate = 0.01;           // 1% loss

        let weight = capacity.calculate_weight();
        assert!(weight > 0.0);

        // Broken path should have zero weight
        capacity.loss_rate = 1.0;
        assert_eq!(capacity.calculate_weight(), 0.0);
    }

    #[test]
    fn test_on_ack() {
        let group = create_test_group();
        let balancer = LoadBalancer::new(group, BalancingAlgorithm::RoundRobin, 100);

        // Set up capacity
        {
            let mut capacities = balancer.capacities.write();
            let mut cap = PathCapacity::new(1);
            cap.packets_in_flight = 10;
            capacities.insert(1, cap);
        }

        // ACK some packets
        balancer.on_ack(1, 5);

        let capacities = balancer.capacities.read();
        assert_eq!(capacities.get(&1).unwrap().packets_in_flight, 5);
    }

    #[test]
    fn test_on_loss() {
        let group = create_test_group();
        let balancer = LoadBalancer::new(group, BalancingAlgorithm::RoundRobin, 100);

        // Set up capacity
        {
            let mut capacities = balancer.capacities.write();
            let mut cap = PathCapacity::new(1);
            cap.packets_in_flight = 100;
            capacities.insert(1, cap);
        }

        // Report loss
        balancer.on_loss(1, 10);

        let capacities = balancer.capacities.read();
        let cap = capacities.get(&1).unwrap();
        assert!(cap.loss_rate > 0.0);
        assert_eq!(cap.packets_in_flight, 90);
    }
}
