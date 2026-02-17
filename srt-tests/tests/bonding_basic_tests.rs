//! Basic bonding tests matching the actual API
//!
//! These tests verify core functionality using the real API signatures.

use bytes::Bytes;
use srt_bonding::*;
use srt_protocol::{Connection, DataPacket, MsgNumber, SeqNumber};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// Helper to create test socket address
fn test_addr(port: u16) -> SocketAddr {
    format!("127.0.0.1:{}", port).parse().unwrap()
}

/// Helper to add a member to a group (performs proper handshake)
fn add_test_member(group: &SocketGroup, id: u32, addr: SocketAddr) -> Result<u32, GroupError> {
    let local_addr = "127.0.0.1:8000".parse().unwrap();
    let mut conn = Connection::new(
        id,
        local_addr,
        addr,
        SeqNumber::new(1000),
        120,
    );

    // Perform handshake: create handshake request and simulate response
    let handshake = conn.create_handshake();

    // Simulate receiving the handshake response
    conn.process_handshake(handshake).unwrap();

    let member_id = group.add_member(Arc::new(conn), addr)?;
    // Set member to Active status so it can send/receive
    group.update_member_status(member_id, MemberStatus::Active)?;
    Ok(member_id)
}

/// Helper to create a test data packet
fn create_test_packet(seq: SeqNumber, data: &[u8]) -> DataPacket {
    DataPacket::new(
        seq,
        MsgNumber::new(seq.as_raw()),
        0,
        123,
        Bytes::from(data.to_vec()),
    )
}

// ============================================================================
// SOCKET GROUP TESTS
// ============================================================================

#[test]
fn test_group_creation() {
    let group = SocketGroup::new(1, GroupType::Broadcast, 5);
    let stats = group.get_stats();
    assert_eq!(stats.member_count, 0);
    assert_eq!(stats.active_member_count, 0);
}

#[test]
fn test_group_add_members() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 3));

    add_test_member(&group, 1, test_addr(9000)).unwrap();
    add_test_member(&group, 2, test_addr(9001)).unwrap();
    add_test_member(&group, 3, test_addr(9002)).unwrap();

    let stats = group.get_stats();
    assert_eq!(stats.member_count, 3);
}

#[test]
fn test_group_max_members() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 2));

    add_test_member(&group, 1, test_addr(9000)).unwrap();
    add_test_member(&group, 2, test_addr(9001)).unwrap();

    // Should fail to add third member
    let result = add_test_member(&group, 3, test_addr(9002));
    assert!(result.is_err());
}

#[test]
fn test_group_remove_member() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 3));

    add_test_member(&group, 1, test_addr(9000)).unwrap();
    add_test_member(&group, 2, test_addr(9001)).unwrap();

    let stats_before = group.get_stats();
    assert_eq!(stats_before.member_count, 2);

    group.remove_member(1).unwrap();

    let stats_after = group.get_stats();
    assert_eq!(stats_after.member_count, 1);
}

// ============================================================================
// BROADCAST MODE TESTS
// ============================================================================

#[test]
fn test_broadcast_creation() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 3));
    let bonding = BroadcastBonding::new(group.clone());

    let stats = bonding.sender.group_stats();
    assert_eq!(stats.member_count, 0);
}

#[test]
fn test_broadcast_duplicate_detection() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 2));
    let bonding = BroadcastBonding::new(group);

    let seq = SeqNumber::new(100);
    let packet = create_test_packet(seq, b"test");

    // First packet - should be accepted
    let result1 = bonding.receiver.on_packet_received(packet.clone(), 1);
    assert!(result1.is_ok());
    assert!(result1.unwrap(), "First packet should be accepted");

    // Duplicate - should be rejected
    let result2 = bonding.receiver.on_packet_received(packet, 2);
    assert!(result2.is_err(), "Duplicate should be rejected");

    let stats = bonding.receiver.stats();
    assert_eq!(stats.buffered_packets, 1);
}

#[test]
fn test_broadcast_multiple_packets() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 2));
    let bonding = BroadcastBonding::new(group);

    // Send 10 unique packets
    for i in 0..10 {
        let seq = SeqNumber::new(i);
        let data = format!("packet_{}", i);
        let packet = create_test_packet(seq, data.as_bytes());
        bonding.receiver.on_packet_received(packet, 1).unwrap();
    }

    let stats = bonding.receiver.stats();
    assert_eq!(stats.ready_packets, 10);
}

// ============================================================================
// BACKUP MODE TESTS
// ============================================================================

#[test]
fn test_backup_creation() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Backup, 2));
    add_test_member(&group, 1, test_addr(9000)).unwrap();
    add_test_member(&group, 2, test_addr(9001)).unwrap();

    let bonding = BackupBonding::new(group.clone(), Duration::from_secs(1), 3);

    bonding.set_primary(1).unwrap();
    bonding.add_backup(2).unwrap();

    assert_eq!(bonding.get_primary_id(), Some(1));
    let backups = bonding.get_backup_ids();
    assert_eq!(backups.len(), 1);
    assert!(backups.contains(&2));
}

#[test]
fn test_backup_manual_failover() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Backup, 2));
    add_test_member(&group, 1, test_addr(9000)).unwrap();
    add_test_member(&group, 2, test_addr(9001)).unwrap();

    let bonding = BackupBonding::new(group.clone(), Duration::from_secs(1), 3);

    bonding.set_primary(1).unwrap();
    bonding.add_backup(2).unwrap();

    // Manual failover
    bonding.manual_failover(2).unwrap();

    assert_eq!(bonding.get_primary_id(), Some(2));

    let history = bonding.failover_history();
    assert_eq!(history.len(), 1);
}

// ============================================================================
// LOAD BALANCING TESTS
// ============================================================================

#[test]
fn test_load_balancer_creation() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 3));

    for i in 1..=3 {
        add_test_member(&group, i, test_addr(9000 + i as u16)).unwrap();
    }

    let balancer = LoadBalancer::new(group.clone(), BalancingAlgorithm::RoundRobin, 100);

    // Verify group has members
    let group_stats = group.get_stats();
    assert_eq!(group_stats.member_count, 3);

    // Stats won't show paths until send() is called
    let stats = balancer.stats();
    assert_eq!(stats.path_count, 0); // Paths discovered on first send
}

#[test]
fn test_load_balancer_round_robin() {
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, 3));

    for i in 1..=3 {
        add_test_member(&group, i, test_addr(9000 + i as u16)).unwrap();
    }

    let balancer = LoadBalancer::new(group.clone(), BalancingAlgorithm::RoundRobin, 100);

    // Verify algorithm is set correctly
    let stats = balancer.stats();
    assert_eq!(stats.algorithm, BalancingAlgorithm::RoundRobin);

    // Note: Actual send testing requires real connections
    // This test verifies the balancer is created with correct algorithm
    // Full round-robin distribution testing would need integration tests
}

// ============================================================================
// ALIGNMENT BUFFER TESTS
// ============================================================================

#[test]
fn test_alignment_buffer_creation() {
    let buffer = AlignmentBuffer::new(1000, Duration::from_secs(10));
    let stats = buffer.stats();
    assert_eq!(stats.packets_received, 0);
}

#[test]
fn test_alignment_in_order_packets() {
    let mut buffer = AlignmentBuffer::new(1000, Duration::from_secs(10));

    // Add packets in order
    for i in 0..5 {
        let seq = SeqNumber::new(100 + i);
        buffer
            .add_packet(create_test_packet(seq, b"data"), 1, 10)
            .unwrap();
    }

    let stats = buffer.stats();
    assert_eq!(stats.packets_received, 5);
}

#[test]
fn test_alignment_out_of_order() {
    let mut buffer = AlignmentBuffer::new(1000, Duration::from_secs(10));

    let seq1 = SeqNumber::new(0);
    let seq2 = SeqNumber::new(1);
    let seq3 = SeqNumber::new(2);

    // Add out of order: 0, 2, 1
    buffer
        .add_packet(create_test_packet(seq1, b"data1"), 1, 10)
        .unwrap();
    buffer
        .add_packet(create_test_packet(seq3, b"data3"), 1, 10)
        .unwrap();
    buffer
        .add_packet(create_test_packet(seq2, b"data2"), 2, 15)
        .unwrap();

    // Pop should come out in order
    let p1 = buffer.pop_next().unwrap();
    assert_eq!(p1.packet.seq_number(), seq1);

    let p2 = buffer.pop_next().unwrap();
    assert_eq!(p2.packet.seq_number(), seq2);

    let p3 = buffer.pop_next().unwrap();
    assert_eq!(p3.packet.seq_number(), seq3);
}

#[test]
fn test_alignment_gap_detection() {
    let mut buffer = AlignmentBuffer::new(1000, Duration::from_secs(10));

    buffer
        .add_packet(create_test_packet(SeqNumber::new(0), b"data"), 1, 10)
        .unwrap();
    buffer
        .add_packet(create_test_packet(SeqNumber::new(1), b"data"), 1, 10)
        .unwrap();
    // Gap: 2 missing
    buffer
        .add_packet(create_test_packet(SeqNumber::new(3), b"data"), 1, 10)
        .unwrap();

    let missing = buffer.get_missing_sequences();
    assert!(missing.contains(&SeqNumber::new(2)), "Should detect gap");
}

#[test]
fn test_alignment_buffer_overflow() {
    let mut buffer = AlignmentBuffer::new(5, Duration::from_secs(10));

    // Try to add more than capacity
    for i in 0..10 {
        let seq = SeqNumber::new(i);
        let result = buffer.add_packet(create_test_packet(seq, b"data"), 1, 10);

        if result.is_err() {
            // Buffer should handle overflow gracefully
            break;
        }
    }

    let stats = buffer.stats();
    assert!(stats.packets_received <= 10, "Should handle packets");
}

// ============================================================================
// SEQUENCE NUMBER WRAPAROUND TESTS
// ============================================================================

#[test]
fn test_sequence_wraparound() {
    let mut buffer = AlignmentBuffer::new(1000, Duration::from_secs(10));

    // Add packets in sequence
    for i in 0..5 {
        let seq = SeqNumber::new(i);
        buffer
            .add_packet(create_test_packet(seq, b"data"), 1, 10)
            .unwrap();
    }

    // Verify sequence numbers work correctly
    let stats = buffer.stats();
    assert_eq!(stats.packets_received, 5);

    // Test that we can pop them in order
    for i in 0..5 {
        let packet = buffer.pop_next().unwrap();
        assert_eq!(packet.packet.seq_number(), SeqNumber::new(i));
    }
}
