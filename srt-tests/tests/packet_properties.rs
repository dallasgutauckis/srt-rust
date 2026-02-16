//! Property-based tests for SRT packet serialization
//!
//! These tests use proptest to generate random packets and verify that
//! serialization/deserialization roundtrips correctly for all valid inputs.

use bytes::Bytes;
use proptest::prelude::*;
use srt_protocol::packet::{
    ControlPacket, ControlType, DataPacket, EncryptionKeySpec, MsgNumber, Packet, PacketBoundary,
    MAX_PAYLOAD_SIZE,
};
use srt_protocol::sequence::{SeqNumber, MAX_SEQ_NUMBER};

// Property test strategies

fn seq_number_strategy() -> impl Strategy<Value = SeqNumber> {
    (0..=MAX_SEQ_NUMBER).prop_map(SeqNumber::new_unchecked)
}

fn packet_boundary_strategy() -> impl Strategy<Value = PacketBoundary> {
    prop_oneof![
        Just(PacketBoundary::Subsequent),
        Just(PacketBoundary::Last),
        Just(PacketBoundary::First),
        Just(PacketBoundary::Solo),
    ]
}

fn encryption_key_strategy() -> impl Strategy<Value = EncryptionKeySpec> {
    prop_oneof![
        Just(EncryptionKeySpec::None),
        Just(EncryptionKeySpec::Even),
        Just(EncryptionKeySpec::Odd),
    ]
}

fn msg_number_strategy() -> impl Strategy<Value = MsgNumber> {
    (
        packet_boundary_strategy(),
        any::<bool>(), // in_order
        encryption_key_strategy(),
        any::<bool>(),      // retransmitted
        0u32..=0x03FF_FFFF, // seq (26 bits)
    )
        .prop_map(
            |(boundary, in_order, encryption_key, retransmitted, seq)| MsgNumber {
                boundary,
                in_order,
                encryption_key,
                retransmitted,
                seq,
            },
        )
}

fn control_type_strategy() -> impl Strategy<Value = ControlType> {
    prop_oneof![
        Just(ControlType::Handshake),
        Just(ControlType::KeepAlive),
        Just(ControlType::Ack),
        Just(ControlType::Nak),
        Just(ControlType::CongestionWarning),
        Just(ControlType::Shutdown),
        Just(ControlType::AckAck),
        Just(ControlType::DropReq),
        Just(ControlType::PeerError),
        Just(ControlType::UserDefined),
    ]
}

#[allow(dead_code)]
fn payload_strategy() -> impl Strategy<Value = Bytes> {
    prop::collection::vec(any::<u8>(), 0..=MAX_PAYLOAD_SIZE).prop_map(Bytes::from)
}

fn small_payload_strategy() -> impl Strategy<Value = Bytes> {
    prop::collection::vec(any::<u8>(), 0..=256).prop_map(Bytes::from)
}

// Property tests

proptest! {
    #[test]
    fn prop_data_packet_roundtrip(
        seq in seq_number_strategy(),
        msg in msg_number_strategy(),
        timestamp in any::<u32>(),
        socket_id in any::<u32>(),
        payload in small_payload_strategy(),
    ) {
        let packet = DataPacket::new(seq, msg, timestamp, socket_id, payload.clone());
        let serialized = packet.to_bytes();
        let deserialized = DataPacket::from_bytes(&serialized).unwrap();

        prop_assert_eq!(deserialized.seq_number(), seq);
        prop_assert_eq!(deserialized.header.timestamp, timestamp);
        prop_assert_eq!(deserialized.header.dest_socket_id, socket_id);

        // Verify message number fields (do this before moving payload)
        let decoded_msg = deserialized.msg_number();
        prop_assert_eq!(deserialized.payload, payload);
        prop_assert_eq!(decoded_msg.boundary, msg.boundary);
        prop_assert_eq!(decoded_msg.in_order, msg.in_order);
        prop_assert_eq!(decoded_msg.encryption_key, msg.encryption_key);
        prop_assert_eq!(decoded_msg.retransmitted, msg.retransmitted);
        prop_assert_eq!(decoded_msg.seq, msg.seq);
    }

    #[test]
    fn prop_control_packet_roundtrip(
        control_type in control_type_strategy(),
        type_specific_info in any::<u16>(),
        additional_info in any::<u32>(),
        timestamp in any::<u32>(),
        socket_id in any::<u32>(),
        control_info in small_payload_strategy(),
    ) {
        let packet = ControlPacket::new(
            control_type,
            type_specific_info,
            additional_info,
            timestamp,
            socket_id,
            control_info.clone(),
        );

        let serialized = packet.to_bytes();
        let deserialized = ControlPacket::from_bytes(&serialized).unwrap();

        prop_assert_eq!(deserialized.control_type(), control_type);
        prop_assert_eq!(deserialized.header.type_specific_info().unwrap(), type_specific_info);
        prop_assert_eq!(deserialized.header.additional_info().unwrap(), additional_info);
        prop_assert_eq!(deserialized.header.timestamp, timestamp);
        prop_assert_eq!(deserialized.header.dest_socket_id, socket_id);
        prop_assert_eq!(deserialized.control_info, control_info);
    }

    #[test]
    fn prop_packet_unified_roundtrip(
        is_data in any::<bool>(),
        seq in seq_number_strategy(),
        msg in msg_number_strategy(),
        timestamp in any::<u32>(),
        socket_id in any::<u32>(),
        payload in small_payload_strategy(),
    ) {
        let packet: Packet = if is_data {
            Packet::Data(DataPacket::new(seq, msg, timestamp, socket_id, payload))
        } else {
            Packet::Control(ControlPacket::new(
                ControlType::KeepAlive,
                0,
                0,
                timestamp,
                socket_id,
                payload,
            ))
        };

        let serialized = packet.to_bytes();
        let deserialized = Packet::from_bytes(&serialized).unwrap();

        prop_assert_eq!(packet.is_data(), deserialized.is_data());
        prop_assert_eq!(packet.is_control(), deserialized.is_control());
        prop_assert_eq!(packet.timestamp(), deserialized.timestamp());
        prop_assert_eq!(packet.dest_socket_id(), deserialized.dest_socket_id());
    }

    #[test]
    fn prop_msg_number_encode_decode(
        boundary in packet_boundary_strategy(),
        in_order in any::<bool>(),
        encryption_key in encryption_key_strategy(),
        retransmitted in any::<bool>(),
        seq in 0u32..=0x03FF_FFFF,
    ) {
        let msg = MsgNumber {
            boundary,
            in_order,
            encryption_key,
            retransmitted,
            seq,
        };

        let raw = msg.to_raw();
        let decoded = MsgNumber::from_raw(raw);

        prop_assert_eq!(decoded.boundary, boundary);
        prop_assert_eq!(decoded.in_order, in_order);
        prop_assert_eq!(decoded.encryption_key, encryption_key);
        prop_assert_eq!(decoded.retransmitted, retransmitted);
        prop_assert_eq!(decoded.seq, seq);
    }

    #[test]
    fn prop_sequence_number_arithmetic(
        a in 0u32..=MAX_SEQ_NUMBER,
        b in 0u32..=MAX_SEQ_NUMBER,
    ) {
        let seq_a = SeqNumber::new(a);
        let seq_b = SeqNumber::new(b);

        // Test distance calculation
        let dist_ab = seq_a.distance_to(seq_b);
        let dist_ba = seq_b.distance_to(seq_a);

        // Distance should be symmetric
        prop_assert_eq!(dist_ab, -dist_ba);

        // Test addition and subtraction
        let added = seq_a + 100;
        let subtracted = added - 100;

        // Should be close to original (accounting for potential wraparound)
        let distance = seq_a.distance_to(subtracted);
        prop_assert!(distance.abs() <= 1, "Distance: {}", distance);
    }

    #[test]
    fn prop_sequence_number_comparison(
        a in 0u32..=MAX_SEQ_NUMBER,
        offset in 1u32..=1000,
    ) {
        let seq_a = SeqNumber::new(a);
        let seq_b = seq_a + offset;

        // b should be ahead of a
        prop_assert!(seq_a.lt(seq_b));
        prop_assert!(seq_b.gt(seq_a));
        prop_assert!(seq_a.le(seq_b));
        prop_assert!(seq_b.ge(seq_a));

        // Reflexive property
        prop_assert!(seq_a.le(seq_a));
        prop_assert!(seq_a.ge(seq_a));
    }

    #[test]
    fn prop_packet_header_flags(
        seq in seq_number_strategy(),
        msg in msg_number_strategy(),
    ) {
        // Create a data packet
        let data_packet = DataPacket::new(
            seq,
            msg,
            1000,
            9999,
            Bytes::from_static(b"test"),
        );

        // Verify the header is correctly identified as data
        prop_assert!(data_packet.header.is_data());
        prop_assert!(!data_packet.header.is_control());

        // Create a control packet
        let control_packet = ControlPacket::new(
            ControlType::Ack,
            0,
            0,
            1000,
            9999,
            Bytes::new(),
        );

        // Verify the header is correctly identified as control
        prop_assert!(control_packet.header.is_control());
        prop_assert!(!control_packet.header.is_data());
    }
}
