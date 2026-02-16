//! Integration tests for SRT protocol packet handling

use bytes::Bytes;
use srt_protocol::packet::{
    ControlPacket, ControlType, DataPacket, EncryptionKeySpec, MsgNumber, Packet, PacketBoundary,
    HEADER_SIZE, MAX_PAYLOAD_SIZE,
};
use srt_protocol::sequence::SeqNumber;

#[test]
fn test_simple_data_packet_roundtrip() {
    let seq = SeqNumber::new(1000);
    let msg = MsgNumber::new(500);
    let payload = Bytes::from_static(b"Hello, SRT!");

    let packet = DataPacket::new(seq, msg, 12345, 9999, payload.clone());
    let serialized = packet.to_bytes();
    let deserialized = DataPacket::from_bytes(&serialized).unwrap();

    assert_eq!(deserialized.seq_number(), seq);
    assert_eq!(deserialized.header.timestamp, 12345);
    assert_eq!(deserialized.header.dest_socket_id, 9999);
    assert_eq!(deserialized.payload, payload);
}

#[test]
fn test_control_packet_roundtrip() {
    let control_info = Bytes::from(vec![1, 2, 3, 4, 5, 6, 7, 8]);

    let packet = ControlPacket::new(
        ControlType::Ack,
        0xABCD,
        0x12345678,
        98765,
        8888,
        control_info.clone(),
    );

    let serialized = packet.to_bytes();
    let deserialized = ControlPacket::from_bytes(&serialized).unwrap();

    assert_eq!(deserialized.control_type(), ControlType::Ack);
    assert_eq!(deserialized.header.type_specific_info().unwrap(), 0xABCD);
    assert_eq!(deserialized.header.additional_info().unwrap(), 0x12345678);
    assert_eq!(deserialized.header.timestamp, 98765);
    assert_eq!(deserialized.header.dest_socket_id, 8888);
    assert_eq!(deserialized.control_info, control_info);
}

#[test]
fn test_packet_auto_detection() {
    // Create a data packet
    let data_packet = DataPacket::new(
        SeqNumber::new(100),
        MsgNumber::new(50),
        1000,
        9999,
        Bytes::from_static(b"test data"),
    );
    let data_bytes = data_packet.to_bytes();

    // Parse using the unified Packet enum
    let parsed = Packet::from_bytes(&data_bytes).unwrap();
    assert!(parsed.is_data());
    match parsed {
        Packet::Data(p) => {
            assert_eq!(p.seq_number(), SeqNumber::new(100));
            assert_eq!(p.payload, Bytes::from_static(b"test data"));
        }
        _ => panic!("Expected data packet"),
    }

    // Create a control packet
    let control_packet = ControlPacket::new(ControlType::KeepAlive, 0, 0, 2000, 9999, Bytes::new());
    let control_bytes = control_packet.to_bytes();

    // Parse using the unified Packet enum
    let parsed = Packet::from_bytes(&control_bytes).unwrap();
    assert!(parsed.is_control());
    match parsed {
        Packet::Control(p) => {
            assert_eq!(p.control_type(), ControlType::KeepAlive);
        }
        _ => panic!("Expected control packet"),
    }
}

#[test]
fn test_message_boundary_flags() {
    for boundary in [
        PacketBoundary::Subsequent,
        PacketBoundary::Last,
        PacketBoundary::First,
        PacketBoundary::Solo,
    ] {
        let mut msg = MsgNumber::new(100);
        msg.boundary = boundary;

        let packet = DataPacket::new(
            SeqNumber::new(1),
            msg,
            1000,
            9999,
            Bytes::from_static(b"test"),
        );

        let serialized = packet.to_bytes();
        let deserialized = DataPacket::from_bytes(&serialized).unwrap();

        assert_eq!(deserialized.msg_number().boundary, boundary);
    }
}

#[test]
fn test_encryption_key_flags() {
    for key_spec in [
        EncryptionKeySpec::None,
        EncryptionKeySpec::Even,
        EncryptionKeySpec::Odd,
    ] {
        let mut msg = MsgNumber::new(100);
        msg.encryption_key = key_spec;

        let packet = DataPacket::new(
            SeqNumber::new(1),
            msg,
            1000,
            9999,
            Bytes::from_static(b"encrypted"),
        );

        let serialized = packet.to_bytes();
        let deserialized = DataPacket::from_bytes(&serialized).unwrap();

        assert_eq!(deserialized.msg_number().encryption_key, key_spec);
    }
}

#[test]
fn test_retransmit_flag() {
    let mut msg = MsgNumber::new(100);
    msg.retransmitted = true;

    let packet = DataPacket::new(
        SeqNumber::new(1),
        msg,
        1000,
        9999,
        Bytes::from_static(b"rexmit"),
    );

    let serialized = packet.to_bytes();
    let deserialized = DataPacket::from_bytes(&serialized).unwrap();

    assert!(deserialized.msg_number().retransmitted);
}

#[test]
fn test_in_order_flag() {
    let mut msg = MsgNumber::new(100);
    msg.in_order = true;

    let packet = DataPacket::new(
        SeqNumber::new(1),
        msg,
        1000,
        9999,
        Bytes::from_static(b"ordered"),
    );

    let serialized = packet.to_bytes();
    let deserialized = DataPacket::from_bytes(&serialized).unwrap();

    assert!(deserialized.msg_number().in_order);
}

#[test]
fn test_all_control_types() {
    let control_types = [
        ControlType::Handshake,
        ControlType::KeepAlive,
        ControlType::Ack,
        ControlType::Nak,
        ControlType::CongestionWarning,
        ControlType::Shutdown,
        ControlType::AckAck,
        ControlType::DropReq,
        ControlType::PeerError,
        ControlType::UserDefined,
    ];

    for control_type in control_types {
        let packet = ControlPacket::new(control_type, 0, 0, 1000, 9999, Bytes::new());

        let serialized = packet.to_bytes();
        let deserialized = ControlPacket::from_bytes(&serialized).unwrap();

        assert_eq!(deserialized.control_type(), control_type);
    }
}

#[test]
fn test_large_payload() {
    let payload = Bytes::from(vec![0xAB; MAX_PAYLOAD_SIZE]);
    let packet = DataPacket::new(
        SeqNumber::new(1000),
        MsgNumber::new(100),
        5000,
        9999,
        payload.clone(),
    );

    let serialized = packet.to_bytes();
    assert_eq!(serialized.len(), HEADER_SIZE + MAX_PAYLOAD_SIZE);

    let deserialized = DataPacket::from_bytes(&serialized).unwrap();
    assert_eq!(deserialized.payload.len(), MAX_PAYLOAD_SIZE);
    assert_eq!(deserialized.payload, payload);
}

#[test]
fn test_empty_payload() {
    let packet = DataPacket::new(
        SeqNumber::new(100),
        MsgNumber::new(50),
        1000,
        9999,
        Bytes::new(),
    );

    let serialized = packet.to_bytes();
    assert_eq!(serialized.len(), HEADER_SIZE);

    let deserialized = DataPacket::from_bytes(&serialized).unwrap();
    assert_eq!(deserialized.payload.len(), 0);
}

#[test]
fn test_sequence_number_wraparound() {
    // Test packet with sequence number near max
    let seq = SeqNumber::new(0x7FFF_FFFF);
    let packet = DataPacket::new(
        seq,
        MsgNumber::new(100),
        1000,
        9999,
        Bytes::from_static(b"wrap"),
    );

    let serialized = packet.to_bytes();
    let deserialized = DataPacket::from_bytes(&serialized).unwrap();

    assert_eq!(deserialized.seq_number(), seq);
}

#[test]
fn test_message_number_fields_combined() {
    let mut msg = MsgNumber::new(0x03FF_FFFF); // Maximum message seq (26 bits)
    msg.boundary = PacketBoundary::First;
    msg.in_order = true;
    msg.encryption_key = EncryptionKeySpec::Odd;
    msg.retransmitted = true;

    let packet = DataPacket::new(
        SeqNumber::new(5000),
        msg,
        10000,
        9999,
        Bytes::from_static(b"complex"),
    );

    let serialized = packet.to_bytes();
    let deserialized = DataPacket::from_bytes(&serialized).unwrap();
    let decoded_msg = deserialized.msg_number();

    assert_eq!(decoded_msg.boundary, PacketBoundary::First);
    assert!(decoded_msg.in_order);
    assert_eq!(decoded_msg.encryption_key, EncryptionKeySpec::Odd);
    assert!(decoded_msg.retransmitted);
    assert_eq!(decoded_msg.seq, 0x03FF_FFFF);
}

#[test]
fn test_control_type_specific_info() {
    let packet = ControlPacket::new(
        ControlType::Ack,
        0xFFFF,      // Max type-specific info
        0xDEAD_BEEF, // Additional info
        12345,
        9999,
        Bytes::new(),
    );

    let serialized = packet.to_bytes();
    let deserialized = ControlPacket::from_bytes(&serialized).unwrap();

    assert_eq!(deserialized.header.type_specific_info().unwrap(), 0xFFFF);
    assert_eq!(deserialized.header.additional_info().unwrap(), 0xDEAD_BEEF);
}
