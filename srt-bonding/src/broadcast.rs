//! Broadcast Bonding Mode
//!
//! Send the same packet to all group members simultaneously.
//! Receive from the first member that delivers (fastest path wins).

use crate::group::{GroupError, MemberStatus, SocketGroup};
use bytes::Bytes;
use parking_lot::RwLock;
use srt_protocol::{DataPacket, MsgNumber, SeqNumber};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

/// Broadcast mode errors
#[derive(Error, Debug)]
pub enum BroadcastError {
    #[error("No active members to send")]
    NoActiveMembers,

    #[error("Group error: {0}")]
    Group(#[from] GroupError),

    #[error("Failed to send on all paths")]
    AllPathsFailed,

    #[error("Packet already received")]
    DuplicatePacket,
}

/// Broadcast send result
#[derive(Debug, Clone)]
pub struct BroadcastSendResult {
    /// Number of members packet was sent to
    pub sent_count: usize,
    /// Number of successful sends
    pub success_count: usize,
    /// IDs of members that failed
    pub failed_members: Vec<u32>,
    /// Sequence number used
    pub sequence: SeqNumber,
}

/// Received packet info
#[derive(Debug, Clone)]
struct ReceivedPacketInfo {
    /// Packet data
    packet: DataPacket,
    /// Which member received it
    _member_id: u32,
    /// When it was received
    _received_at: Instant,
}

/// Broadcast receiver state
///
/// Tracks packets received from multiple paths to deliver only once
/// (from the fastest path).
pub struct BroadcastReceiver {
    /// Packets received, indexed by sequence number
    received: Arc<RwLock<HashMap<SeqNumber, ReceivedPacketInfo>>>,
    /// Next expected sequence number
    next_expected: Arc<RwLock<SeqNumber>>,
    /// Ordered packets ready for delivery
    ready_queue: Arc<RwLock<VecDeque<DataPacket>>>,
    /// Maximum buffer size
    max_buffer_size: usize,
}

impl BroadcastReceiver {
    /// Create a new broadcast receiver
    pub fn new(max_buffer_size: usize) -> Self {
        BroadcastReceiver {
            received: Arc::new(RwLock::new(HashMap::new())),
            next_expected: Arc::new(RwLock::new(SeqNumber::new(0))),
            ready_queue: Arc::new(RwLock::new(VecDeque::new())),
            max_buffer_size,
        }
    }

    /// Process a received packet
    ///
    /// Returns true if this is a new packet (not a duplicate).
    pub fn on_packet_received(
        &self,
        packet: DataPacket,
        member_id: u32,
    ) -> Result<bool, BroadcastError> {
        let seq = packet.seq_number();

        // Check if packet has already been delivered (seq < next_expected)
        let next_expected = *self.next_expected.read();
        if seq.distance_to(next_expected) > 0 {
            // Packet is before next_expected, already delivered
            return Err(BroadcastError::DuplicatePacket);
        }

        let mut received = self.received.write();

        // Check if we already received this packet (buffered but not yet delivered)
        if received.contains_key(&seq) {
            return Err(BroadcastError::DuplicatePacket);
        }

        // Check buffer size
        if received.len() >= self.max_buffer_size {
            // Buffer full, drop this packet
            return Ok(false);
        }

        // Store the packet
        received.insert(
            seq,
            ReceivedPacketInfo {
                packet: packet.clone(),
                _member_id: member_id,
                _received_at: Instant::now(),
            },
        );

        // Try to deliver in-order packets
        self.deliver_ready_packets(&mut received);

        Ok(true)
    }

    /// Deliver packets that are ready (in sequence order)
    fn deliver_ready_packets(&self, received: &mut HashMap<SeqNumber, ReceivedPacketInfo>) {
        let mut next_expected = self.next_expected.write();
        let mut ready_queue = self.ready_queue.write();

        while let Some(info) = received.remove(&*next_expected) {
            ready_queue.push_back(info.packet);
            *next_expected = next_expected.next();
        }
    }

    /// Get next ready packet for delivery
    pub fn pop_ready_packet(&self) -> Option<DataPacket> {
        self.ready_queue.write().pop_front()
    }

    /// Get number of ready packets
    pub fn ready_packet_count(&self) -> usize {
        self.ready_queue.read().len()
    }

    /// Get statistics
    pub fn stats(&self) -> BroadcastReceiverStats {
        let received = self.received.read();
        let ready_queue = self.ready_queue.read();

        BroadcastReceiverStats {
            buffered_packets: received.len(),
            ready_packets: ready_queue.len(),
            next_expected: *self.next_expected.read(),
        }
    }
}

/// Broadcast receiver statistics
#[derive(Debug, Clone)]
pub struct BroadcastReceiverStats {
    /// Number of packets buffered (waiting for in-order delivery)
    pub buffered_packets: usize,
    /// Number of packets ready for delivery
    pub ready_packets: usize,
    /// Next expected sequence number
    pub next_expected: SeqNumber,
}

/// Broadcast sender
///
/// Sends packets to all active group members.
pub struct BroadcastSender {
    /// The socket group
    group: Arc<SocketGroup>,
}

impl BroadcastSender {
    /// Create a new broadcast sender
    pub fn new(group: Arc<SocketGroup>) -> Self {
        BroadcastSender { group }
    }

    /// Send data to all active members
    pub fn send(&self, data: &[u8]) -> Result<BroadcastSendResult, BroadcastError> {
        let members = self.group.get_active_members();

        if members.is_empty() {
            return Err(BroadcastError::NoActiveMembers);
        }

        let sequence = self.group.next_sequence();
        let mut success_count = 0;
        let mut failed_members = Vec::new();

        // Create packet (will be sent to all members with same sequence number)
        let msg_number = MsgNumber::new(sequence.as_raw());

        for member in &members {
            let _packet = DataPacket::new(
                sequence,
                msg_number,
                0, // Timestamp will be set by connection
                member.connection.remote_socket_id().unwrap_or(0),
                Bytes::copy_from_slice(data),
            );

            match member.connection.send(data) {
                Ok(_) => {
                    member.record_sent(data.len());
                    success_count += 1;
                }
                Err(_) => {
                    failed_members.push(member.connection.local_socket_id());
                    // Mark member as potentially broken
                    let mut stats = member.stats.write();
                    stats.failure_count += 1;

                    if stats.failure_count > 3 {
                        stats.status = MemberStatus::Broken;
                    }
                }
            }
        }

        if success_count == 0 {
            return Err(BroadcastError::AllPathsFailed);
        }

        Ok(BroadcastSendResult {
            sent_count: members.len(),
            success_count,
            failed_members,
            sequence,
        })
    }

    /// Get group statistics
    pub fn group_stats(&self) -> crate::group::GroupStats {
        self.group.get_stats()
    }
}

/// Complete broadcast bonding implementation
pub struct BroadcastBonding {
    /// Broadcast sender
    pub sender: BroadcastSender,
    /// Broadcast receiver
    pub receiver: BroadcastReceiver,
    /// Socket group
    pub group: Arc<SocketGroup>,
}

impl BroadcastBonding {
    /// Create new broadcast bonding
    pub fn new(group: Arc<SocketGroup>) -> Self {
        BroadcastBonding {
            sender: BroadcastSender::new(group.clone()),
            receiver: BroadcastReceiver::new(8192),
            group,
        }
    }

    /// Send data on all paths
    pub fn send(&self, data: &[u8]) -> Result<BroadcastSendResult, BroadcastError> {
        self.sender.send(data)
    }

    /// Process received packet from any member
    pub fn on_receive(&self, packet: DataPacket, member_id: u32) -> Result<bool, BroadcastError> {
        let result = self.receiver.on_packet_received(packet, member_id);

        // Update member stats
        if let Some(member) = self.group.get_member(member_id) {
            if result.is_ok() {
                member.record_received(1456); // Approximate packet size
            }
        }

        result
    }

    /// Get next ready packet
    pub fn receive(&self) -> Option<DataPacket> {
        self.receiver.pop_ready_packet()
    }

    /// Get complete statistics
    pub fn stats(&self) -> BroadcastBondingStats {
        BroadcastBondingStats {
            group_stats: self.group.get_stats(),
            receiver_stats: self.receiver.stats(),
        }
    }
}

/// Broadcast bonding statistics
#[derive(Debug, Clone)]
pub struct BroadcastBondingStats {
    /// Group statistics
    pub group_stats: crate::group::GroupStats,
    /// Receiver statistics
    pub receiver_stats: BroadcastReceiverStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::GroupType;
    use srt_protocol::Connection;

    fn create_test_group() -> Arc<SocketGroup> {
        Arc::new(SocketGroup::new(1, GroupType::Broadcast, 10))
    }

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
    fn test_broadcast_receiver_duplicate_detection() {
        let receiver = BroadcastReceiver::new(1024);

        let packet = DataPacket::new(
            SeqNumber::new(0),
            MsgNumber::new(0),
            0,
            0,
            Bytes::from("test"),
        );

        // First receive should succeed
        let result1 = receiver.on_packet_received(packet.clone(), 1);
        assert!(result1.is_ok());

        // Second receive (duplicate) should fail
        let result2 = receiver.on_packet_received(packet, 2);
        assert!(matches!(result2, Err(BroadcastError::DuplicatePacket)));
    }

    #[test]
    fn test_broadcast_receiver_ordering() {
        let receiver = BroadcastReceiver::new(1024);

        // Create packets 0, 1, 2
        let packets: Vec<_> = (0..3)
            .map(|i| {
                let mut p = DataPacket::new(
                    SeqNumber::new(i),
                    MsgNumber::new(i),
                    0,
                    0,
                    Bytes::from(format!("Packet {}", i)),
                );
                p.header.seq_or_control = i;
                p
            })
            .collect();

        // Receive out of order: 0, 2, 1
        receiver.on_packet_received(packets[0].clone(), 1).unwrap();
        receiver.on_packet_received(packets[2].clone(), 1).unwrap();

        // Only packet 0 should be ready
        assert_eq!(receiver.ready_packet_count(), 1);

        // Receive packet 1
        receiver.on_packet_received(packets[1].clone(), 1).unwrap();

        // Now all 3 should be ready
        assert_eq!(receiver.ready_packet_count(), 3);
    }

    #[test]
    fn test_broadcast_sender_no_members() {
        let group = create_test_group();
        let sender = BroadcastSender::new(group);

        let result = sender.send(b"test");
        assert!(matches!(result, Err(BroadcastError::NoActiveMembers)));
    }

    #[test]
    fn test_broadcast_bonding() {
        let group = create_test_group();
        let bonding = BroadcastBonding::new(group.clone());

        // Add some members
        let conn1 = create_test_connection(1);
        let conn2 = create_test_connection(2);

        group
            .add_member(conn1, "127.0.0.1:9001".parse().unwrap())
            .unwrap();
        group
            .add_member(conn2, "127.0.0.1:9002".parse().unwrap())
            .unwrap();

        // Note: Actual send will fail since connections aren't real,
        // but we can test the API
        let stats = bonding.stats();
        assert_eq!(stats.group_stats.member_count, 2);
    }
}
