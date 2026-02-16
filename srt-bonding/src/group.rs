//! Socket Group Management
//!
//! Manages groups of SRT connections for bonding multiple network paths.

use parking_lot::RwLock;
use srt_protocol::{Connection, SeqNumber};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

/// Group errors
#[derive(Error, Debug)]
pub enum GroupError {
    #[error("Group is full (max {max} members)")]
    GroupFull { max: usize },

    #[error("Member not found: {0}")]
    MemberNotFound(u32),

    #[error("No active members available")]
    NoActiveMembers,

    #[error("Invalid group state")]
    InvalidState,

    #[error("Connection error: {0}")]
    Connection(String),
}

/// Group type/mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupType {
    /// Broadcast: send to all, receive from first
    Broadcast,
    /// Backup: primary/backup with failover
    Backup,
    /// Balancing: load balance across members
    Balancing,
}

/// Member status in group
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStatus {
    /// Member is pending connection
    Pending,
    /// Member is active and connected
    Active,
    /// Member is idle (backup mode)
    Idle,
    /// Member is broken/failed
    Broken,
}

/// Statistics for a group member
#[derive(Debug, Clone)]
pub struct MemberStats {
    /// Member ID (socket ID)
    pub member_id: u32,
    /// Member address
    pub address: SocketAddr,
    /// Current status
    pub status: MemberStatus,
    /// Packets sent on this member
    pub packets_sent: u64,
    /// Packets received on this member
    pub packets_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Estimated RTT (microseconds)
    pub rtt_us: u32,
    /// Estimated bandwidth (bytes per second)
    pub bandwidth_bps: u64,
    /// Last activity timestamp
    pub last_activity: Instant,
    /// Number of failures
    pub failure_count: u32,
}

impl MemberStats {
    fn new(member_id: u32, address: SocketAddr) -> Self {
        MemberStats {
            member_id,
            address,
            status: MemberStatus::Pending,
            packets_sent: 0,
            packets_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            rtt_us: 0,
            bandwidth_bps: 0,
            last_activity: Instant::now(),
            failure_count: 0,
        }
    }
}

/// Group member (a connection in the group)
pub struct GroupMember {
    /// Member connection
    pub connection: Arc<Connection>,
    /// Member statistics
    pub stats: Arc<RwLock<MemberStats>>,
    /// Weight for load balancing (0.0 to 1.0)
    pub weight: f64,
}

impl GroupMember {
    fn new(connection: Arc<Connection>, member_id: u32, address: SocketAddr) -> Self {
        GroupMember {
            connection,
            stats: Arc::new(RwLock::new(MemberStats::new(member_id, address))),
            weight: 1.0,
        }
    }

    /// Check if member is active
    pub fn is_active(&self) -> bool {
        self.stats.read().status == MemberStatus::Active && self.connection.is_connected()
    }

    /// Update member status
    pub fn set_status(&self, status: MemberStatus) {
        self.stats.write().status = status;
    }

    /// Record packet sent
    pub fn record_sent(&self, bytes: usize) {
        let mut stats = self.stats.write();
        stats.packets_sent += 1;
        stats.bytes_sent += bytes as u64;
        stats.last_activity = Instant::now();
    }

    /// Record packet received
    pub fn record_received(&self, bytes: usize) {
        let mut stats = self.stats.write();
        stats.packets_received += 1;
        stats.bytes_received += bytes as u64;
        stats.last_activity = Instant::now();
    }

    /// Update RTT estimate
    pub fn update_rtt(&self, rtt_us: u32) {
        self.stats.write().rtt_us = rtt_us;
    }

    /// Update bandwidth estimate
    pub fn update_bandwidth(&self, bps: u64) {
        self.stats.write().bandwidth_bps = bps;
    }

    /// Get member statistics
    pub fn get_stats(&self) -> MemberStats {
        self.stats.read().clone()
    }
}

/// Socket Group
///
/// Manages multiple SRT connections as a bonded group.
pub struct SocketGroup {
    /// Group ID
    group_id: u32,
    /// Group type/mode
    group_type: GroupType,
    /// Group members indexed by socket ID
    members: Arc<RwLock<HashMap<u32, Arc<GroupMember>>>>,
    /// Maximum number of members
    max_members: usize,
    /// Next sequence number for group send operations
    next_seq: Arc<RwLock<SeqNumber>>,
    /// Group creation time
    created_at: Instant,
}

impl SocketGroup {
    /// Create a new socket group
    pub fn new(group_id: u32, group_type: GroupType, max_members: usize) -> Self {
        SocketGroup {
            group_id,
            group_type,
            members: Arc::new(RwLock::new(HashMap::new())),
            max_members,
            next_seq: Arc::new(RwLock::new(SeqNumber::new(0))),
            created_at: Instant::now(),
        }
    }

    /// Get group ID
    pub fn group_id(&self) -> u32 {
        self.group_id
    }

    /// Get group type
    pub fn group_type(&self) -> GroupType {
        self.group_type
    }

    /// Add a member to the group
    pub fn add_member(
        &self,
        connection: Arc<Connection>,
        address: SocketAddr,
    ) -> Result<u32, GroupError> {
        let mut members = self.members.write();

        if members.len() >= self.max_members {
            return Err(GroupError::GroupFull {
                max: self.max_members,
            });
        }

        let member_id = connection.local_socket_id();
        let member = Arc::new(GroupMember::new(connection, member_id, address));

        members.insert(member_id, member);

        Ok(member_id)
    }

    /// Remove a member from the group
    pub fn remove_member(&self, member_id: u32) -> Result<(), GroupError> {
        let mut members = self.members.write();

        if members.remove(&member_id).is_none() {
            return Err(GroupError::MemberNotFound(member_id));
        }

        Ok(())
    }

    /// Get a member by ID
    pub fn get_member(&self, member_id: u32) -> Option<Arc<GroupMember>> {
        self.members.read().get(&member_id).cloned()
    }

    /// Get all members
    pub fn get_all_members(&self) -> Vec<Arc<GroupMember>> {
        self.members.read().values().cloned().collect()
    }

    /// Get active members only
    pub fn get_active_members(&self) -> Vec<Arc<GroupMember>> {
        self.members
            .read()
            .values()
            .filter(|m| m.is_active())
            .cloned()
            .collect()
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.members.read().len()
    }

    /// Get active member count
    pub fn active_member_count(&self) -> usize {
        self.members
            .read()
            .values()
            .filter(|m| m.is_active())
            .count()
    }

    /// Update member status
    pub fn update_member_status(
        &self,
        member_id: u32,
        status: MemberStatus,
    ) -> Result<(), GroupError> {
        let member = self
            .get_member(member_id)
            .ok_or(GroupError::MemberNotFound(member_id))?;

        member.set_status(status);
        Ok(())
    }

    /// Get next sequence number for group operations
    pub fn next_sequence(&self) -> SeqNumber {
        let mut seq = self.next_seq.write();
        let current = *seq;
        *seq = seq.next();
        current
    }

    /// Get group statistics
    pub fn get_stats(&self) -> GroupStats {
        let members = self.members.read();
        let member_stats: Vec<_> = members.values().map(|m| m.get_stats()).collect();

        let total_sent: u64 = member_stats.iter().map(|s| s.packets_sent).sum();
        let total_received: u64 = member_stats.iter().map(|s| s.packets_received).sum();
        let total_bytes_sent: u64 = member_stats.iter().map(|s| s.bytes_sent).sum();
        let total_bytes_received: u64 = member_stats.iter().map(|s| s.bytes_received).sum();

        let active_count = member_stats
            .iter()
            .filter(|s| s.status == MemberStatus::Active)
            .count();

        GroupStats {
            group_id: self.group_id,
            group_type: self.group_type,
            member_count: members.len(),
            active_member_count: active_count,
            total_packets_sent: total_sent,
            total_packets_received: total_received,
            total_bytes_sent,
            total_bytes_received,
            member_stats,
            uptime: self.created_at.elapsed(),
        }
    }

    /// Health check: remove broken members
    pub fn cleanup_broken_members(&self) {
        let mut members = self.members.write();
        let broken: Vec<_> = members
            .iter()
            .filter(|(_, m)| m.get_stats().status == MemberStatus::Broken)
            .map(|(id, _)| *id)
            .collect();

        for id in broken {
            members.remove(&id);
        }
    }

    /// Find best member based on criteria
    pub fn find_best_member<F>(&self, criteria: F) -> Option<Arc<GroupMember>>
    where
        F: Fn(&MemberStats) -> i64,
    {
        self.get_active_members()
            .into_iter()
            .max_by_key(|m| criteria(&m.get_stats()))
    }
}

/// Group statistics
#[derive(Debug, Clone)]
pub struct GroupStats {
    /// Group ID
    pub group_id: u32,
    /// Group type
    pub group_type: GroupType,
    /// Total member count
    pub member_count: usize,
    /// Active member count
    pub active_member_count: usize,
    /// Total packets sent across all members
    pub total_packets_sent: u64,
    /// Total packets received across all members
    pub total_packets_received: u64,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Individual member statistics
    pub member_stats: Vec<MemberStats>,
    /// Group uptime
    pub uptime: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_group_creation() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 10);

        assert_eq!(group.group_id(), 1);
        assert_eq!(group.group_type(), GroupType::Broadcast);
        assert_eq!(group.member_count(), 0);
    }

    #[test]
    fn test_add_member() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 10);
        let conn = create_test_connection(12345);

        let member_id = group
            .add_member(conn, "127.0.0.1:9001".parse().unwrap())
            .unwrap();

        assert_eq!(member_id, 12345);
        assert_eq!(group.member_count(), 1);
    }

    #[test]
    fn test_remove_member() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 10);
        let conn = create_test_connection(12345);

        let member_id = group
            .add_member(conn, "127.0.0.1:9001".parse().unwrap())
            .unwrap();

        group.remove_member(member_id).unwrap();
        assert_eq!(group.member_count(), 0);
    }

    #[test]
    fn test_max_members() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 2);

        let conn1 = create_test_connection(1);
        let conn2 = create_test_connection(2);
        let conn3 = create_test_connection(3);

        group
            .add_member(conn1, "127.0.0.1:9001".parse().unwrap())
            .unwrap();
        group
            .add_member(conn2, "127.0.0.1:9002".parse().unwrap())
            .unwrap();

        let result = group.add_member(conn3, "127.0.0.1:9003".parse().unwrap());
        assert!(matches!(result, Err(GroupError::GroupFull { max: 2 })));
    }

    #[test]
    fn test_member_stats() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 10);
        let conn = create_test_connection(12345);

        group
            .add_member(conn, "127.0.0.1:9001".parse().unwrap())
            .unwrap();

        let member = group.get_member(12345).unwrap();
        member.record_sent(1456);
        member.record_received(1456);

        let stats = member.get_stats();
        assert_eq!(stats.packets_sent, 1);
        assert_eq!(stats.packets_received, 1);
        assert_eq!(stats.bytes_sent, 1456);
        assert_eq!(stats.bytes_received, 1456);
    }

    #[test]
    fn test_group_stats() {
        let group = SocketGroup::new(1, GroupType::Broadcast, 10);

        let conn1 = create_test_connection(1);
        let conn2 = create_test_connection(2);

        group
            .add_member(conn1, "127.0.0.1:9001".parse().unwrap())
            .unwrap();
        group
            .add_member(conn2, "127.0.0.1:9002".parse().unwrap())
            .unwrap();

        let member1 = group.get_member(1).unwrap();
        let member2 = group.get_member(2).unwrap();

        member1.record_sent(1000);
        member2.record_sent(2000);

        let stats = group.get_stats();
        assert_eq!(stats.member_count, 2);
        assert_eq!(stats.total_bytes_sent, 3000);
    }
}
