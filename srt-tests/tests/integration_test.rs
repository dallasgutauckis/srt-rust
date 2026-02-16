//! End-to-end integration tests
//!
//! Tests the complete SRT protocol stack with send/receive operations.

use bytes::Bytes;
use srt_protocol::{
    AckGenerator, AckInfo, Connection, ConnectionState, DataPacket, MsgNumber, NakGenerator,
    ReceiveBuffer, SendBuffer, SeqNumber, SrtHandshake, SrtOptions,
};
use std::time::Duration;

#[test]
fn test_handshake_exchange() {
    // Create two handshakes (simulating sender and receiver)
    let sender_hs = SrtHandshake::new_request(
        1000,
        12345,
        "127.0.0.1:9000".parse().unwrap(),
        SrtOptions::default_capabilities(),
        120,
        80,
    );

    let receiver_hs = SrtHandshake::new_request(
        2000,
        54321,
        "127.0.0.1:9001".parse().unwrap(),
        SrtOptions::default_capabilities(),
        120,
        80,
    );

    // Serialize and exchange
    let sender_bytes = sender_hs.to_bytes();
    let receiver_bytes = receiver_hs.to_bytes();

    // Parse received handshakes
    let sender_received = SrtHandshake::from_bytes(&receiver_bytes).unwrap();
    let receiver_received = SrtHandshake::from_bytes(&sender_bytes).unwrap();

    // Verify handshake data
    assert!(sender_received.is_srt());
    assert!(receiver_received.is_srt());
    assert_eq!(sender_received.udt.socket_id, 54321);
    assert_eq!(receiver_received.udt.socket_id, 12345);
}

#[test]
fn test_buffer_roundtrip() {
    let mut send_buffer = SendBuffer::new(1024, Duration::from_secs(10));
    let mut recv_buffer = ReceiveBuffer::new(1024);

    // Create and send packets
    let messages = vec![
        Bytes::from("Message 1"),
        Bytes::from("Message 2"),
        Bytes::from("Message 3"),
    ];

    let mut seqs = Vec::new();
    for (i, msg) in messages.iter().enumerate() {
        let mut packet = DataPacket::new(
            SeqNumber::new(i as u32),
            MsgNumber {
                boundary: srt_protocol::packet::PacketBoundary::Solo,
                in_order: false,
                encryption_key: srt_protocol::packet::EncryptionKeySpec::None,
                retransmitted: false,
                seq: i as u32,
            },
            0,
            0,
            msg.clone(),
        );

        let seq = send_buffer.push(packet.clone()).unwrap();
        seqs.push(seq);

        // Update packet with assigned sequence number for receiving
        packet.header.seq_or_control = seq.as_raw();
        recv_buffer.push(packet).unwrap();
    }

    // Verify messages received
    for expected_msg in &messages {
        let received = recv_buffer.pop_message().unwrap();
        assert_eq!(&received, expected_msg);
    }

    // ACK all packets
    send_buffer.acknowledge_up_to(seqs[seqs.len() - 1]);
    let flushed = send_buffer.flush_acknowledged();
    assert_eq!(flushed, messages.len());
}

#[test]
fn test_out_of_order_reception() {
    let mut recv_buffer = ReceiveBuffer::new(1024);

    // Create packets 0, 1, 2, 3
    let packets: Vec<_> = (0..4)
        .map(|i| {
            let mut packet = DataPacket::new(
                SeqNumber::new(i),
                MsgNumber::new(i),
                0,
                0,
                Bytes::from(format!("Packet {}", i)),
            );
            packet.header.seq_or_control = i;
            packet.header.msg_or_info = MsgNumber {
                boundary: srt_protocol::packet::PacketBoundary::Solo,
                seq: i,
                in_order: false,
                encryption_key: srt_protocol::packet::EncryptionKeySpec::None,
                retransmitted: false,
            }
            .to_raw();
            packet
        })
        .collect();

    // Receive out of order: 0, 2, 3, 1
    recv_buffer.push(packets[0].clone()).unwrap();
    recv_buffer.push(packets[2].clone()).unwrap();
    recv_buffer.push(packets[3].clone()).unwrap();

    // Should have one ready message (packet 0)
    assert_eq!(recv_buffer.ready_message_count(), 1);

    // Detect losses
    let losses = recv_buffer.get_loss_list();
    assert_eq!(losses.len(), 1);
    assert_eq!(losses[0], SeqNumber::new(1));

    // Receive missing packet
    recv_buffer.push(packets[1].clone()).unwrap();

    // Now all messages should be ready
    assert_eq!(recv_buffer.ready_message_count(), 4);

    // Verify order
    for i in 0..4 {
        let msg = recv_buffer.pop_message().unwrap();
        assert_eq!(&msg[..], format!("Packet {}", i).as_bytes());
    }
}

#[test]
fn test_ack_nak_generation() {
    let mut ack_gen = AckGenerator::new(Duration::from_millis(10));
    let mut nak_gen = NakGenerator::new(Duration::from_millis(10));

    // Generate ACK
    let ack_info = AckInfo::new(SeqNumber::new(100));
    let ack = ack_gen.generate_ack(ack_info, 9999);

    assert_eq!(
        ack.control_type(),
        srt_protocol::packet::ControlType::Ack
    );

    // Generate NAK
    let nak_info = srt_protocol::ack::NakInfo::new(vec![
        srt_protocol::loss::LossRange::single(SeqNumber::new(50)),
    ]);
    let nak = nak_gen.generate_nak(nak_info, 9999).unwrap();

    assert_eq!(
        nak.control_type(),
        srt_protocol::packet::ControlType::Nak
    );
}

#[test]
fn test_connection_lifecycle() {
    let mut sender = Connection::new(
        12345,
        "127.0.0.1:9000".parse().unwrap(),
        "127.0.0.1:9001".parse().unwrap(),
        SeqNumber::new(1000),
        120,
    );

    let mut receiver = Connection::new(
        54321,
        "127.0.0.1:9001".parse().unwrap(),
        "127.0.0.1:9000".parse().unwrap(),
        SeqNumber::new(2000),
        120,
    );

    // Initial state
    assert_eq!(sender.state(), ConnectionState::Init);
    assert_eq!(receiver.state(), ConnectionState::Init);

    // Exchange handshakes
    let sender_hs = sender.create_handshake();
    let receiver_hs = receiver.create_handshake();

    sender.process_handshake(receiver_hs).unwrap();
    receiver.process_handshake(sender_hs).unwrap();

    // Should be connected
    assert_eq!(sender.state(), ConnectionState::Connected);
    assert_eq!(receiver.state(), ConnectionState::Connected);

    // Close connections
    sender.close();
    receiver.close();

    assert!(sender.is_closed());
    assert!(receiver.is_closed());
}

#[test]
fn test_multi_packet_message() {
    let mut recv_buffer = ReceiveBuffer::new(1024);

    // Create a 3-packet message
    let packet1 = {
        let mut p = DataPacket::new(
            SeqNumber::new(0),
            MsgNumber::new(100),
            0,
            0,
            Bytes::from("Part1"),
        );
        p.header.seq_or_control = 0;
        p.header.msg_or_info = MsgNumber {
            boundary: srt_protocol::packet::PacketBoundary::First,
            seq: 100,
            in_order: false,
            encryption_key: srt_protocol::packet::EncryptionKeySpec::None,
            retransmitted: false,
        }
        .to_raw();
        p
    };

    let packet2 = {
        let mut p = DataPacket::new(
            SeqNumber::new(1),
            MsgNumber::new(100),
            0,
            0,
            Bytes::from("Part2"),
        );
        p.header.seq_or_control = 1;
        p.header.msg_or_info = MsgNumber {
            boundary: srt_protocol::packet::PacketBoundary::Subsequent,
            seq: 100,
            in_order: false,
            encryption_key: srt_protocol::packet::EncryptionKeySpec::None,
            retransmitted: false,
        }
        .to_raw();
        p
    };

    let packet3 = {
        let mut p = DataPacket::new(
            SeqNumber::new(2),
            MsgNumber::new(100),
            0,
            0,
            Bytes::from("Part3"),
        );
        p.header.seq_or_control = 2;
        p.header.msg_or_info = MsgNumber {
            boundary: srt_protocol::packet::PacketBoundary::Last,
            seq: 100,
            in_order: false,
            encryption_key: srt_protocol::packet::EncryptionKeySpec::None,
            retransmitted: false,
        }
        .to_raw();
        p
    };

    // Receive all packets
    recv_buffer.push(packet1).unwrap();
    recv_buffer.push(packet2).unwrap();
    recv_buffer.push(packet3).unwrap();

    // Should have one complete message
    assert_eq!(recv_buffer.ready_message_count(), 1);

    let message = recv_buffer.pop_message().unwrap();
    assert_eq!(&message[..], b"Part1Part2Part3");
}

#[test]
fn test_congestion_control() {
    use srt_protocol::CongestionController;

    let mut cc = CongestionController::new(10_000_000, 1456, 8192);

    // Initial state
    assert!(cc.can_send());

    // Send packets
    for _ in 0..10 {
        cc.on_packet_sent();
    }

    // ACK them
    cc.on_ack(10, 50_000);

    // Window should have grown
    let stats = cc.stats();
    assert!(stats.congestion_window > 16);

    // Simulate loss
    cc.on_loss(5);

    // Window should have shrunk
    let stats_after_loss = cc.stats();
    assert!(stats_after_loss.congestion_window < stats.congestion_window);
}

#[test]
fn test_rtt_estimation() {
    use srt_protocol::RttEstimator;

    let mut estimator = RttEstimator::new();

    // Add RTT samples
    estimator.update(100_000); // 100ms
    estimator.update(110_000); // 110ms
    estimator.update(90_000);  // 90ms

    let srtt = estimator.srtt();
    assert!(srtt > 80_000 && srtt < 120_000);

    let rto = estimator.rto();
    assert!(rto >= Duration::from_millis(100));
}

#[test]
fn test_bandwidth_estimation() {
    use srt_protocol::BandwidthEstimator;

    let mut estimator = BandwidthEstimator::new();

    // Simulate packet deliveries
    for _ in 0..5 {
        estimator.add_sample(1456, 50_000);
        std::thread::sleep(Duration::from_millis(10));
    }

    let bw = estimator.estimated_bandwidth_bps();
    assert!(bw > 0);
}
