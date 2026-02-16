//! SRT Packet Structures and Serialization
//!
//! This module implements the SRT packet format, which consists of a 128-bit (16-byte)
//! header followed by optional payload data. Packets are either data packets or control
//! packets, distinguished by bit 31 of the sequence number field.

use crate::sequence::SeqNumber;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt;
use thiserror::Error;

/// Size of the SRT packet header in bytes (4 fields × 4 bytes each)
pub const HEADER_SIZE: usize = 16;

/// Maximum payload size for SRT packet (MTU 1500 - IP/UDP headers - SRT header)
pub const MAX_PAYLOAD_SIZE: usize = 1456; // 1500 - 28 (IP+UDP) - 16 (SRT header)

/// Maximum timestamp value (32-bit)
pub const MAX_TIMESTAMP: u32 = 0xFFFF_FFFF;

/// Control packet flag (bit 31 of sequence number field)
const CONTROL_FLAG: u32 = 0x8000_0000;

/// Mask for sequence number value (bits 0-30)
const SEQ_MASK: u32 = 0x7FFF_FFFF;

/// Packet header fields (each field is 32 bits / 4 bytes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HeaderField {
    /// Sequence number (data) or control type (control)
    SeqNo = 0,
    /// Message number and flags (data) or additional info (control)
    MsgNo = 1,
    /// Timestamp
    Timestamp = 2,
    /// Socket ID
    SocketId = 3,
}

/// Control packet types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ControlType {
    /// Connection handshake
    Handshake = 0,
    /// Keep-alive
    KeepAlive = 1,
    /// Acknowledgement
    Ack = 2,
    /// Negative acknowledgement (loss report)
    Nak = 3,
    /// Congestion warning
    CongestionWarning = 4,
    /// Shutdown
    Shutdown = 5,
    /// Acknowledgement of acknowledgement
    AckAck = 6,
    /// Drop request
    DropReq = 7,
    /// Peer error
    PeerError = 8,
    /// User-defined control packet
    UserDefined = 0x7FFF,
}

impl ControlType {
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(ControlType::Handshake),
            1 => Some(ControlType::KeepAlive),
            2 => Some(ControlType::Ack),
            3 => Some(ControlType::Nak),
            4 => Some(ControlType::CongestionWarning),
            5 => Some(ControlType::Shutdown),
            6 => Some(ControlType::AckAck),
            7 => Some(ControlType::DropReq),
            8 => Some(ControlType::PeerError),
            0x7FFF => Some(ControlType::UserDefined),
            _ => None,
        }
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Message boundary flags (bits 30-31 of message number field)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketBoundary {
    /// Packet in the middle of a message
    Subsequent = 0b00,
    /// Last packet of a message
    Last = 0b01,
    /// First packet of a message
    First = 0b10,
    /// Solo packet (complete message)
    Solo = 0b11,
}

impl PacketBoundary {
    pub fn from_bits(value: u8) -> Self {
        match value & 0b11 {
            0b00 => PacketBoundary::Subsequent,
            0b01 => PacketBoundary::Last,
            0b10 => PacketBoundary::First,
            0b11 => PacketBoundary::Solo,
            _ => unreachable!(),
        }
    }

    pub fn as_bits(self) -> u8 {
        self as u8
    }
}

/// Encryption key specification (bits 28-29 of message number field)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncryptionKeySpec {
    /// No encryption
    None = 0,
    /// Even key
    Even = 1,
    /// Odd key
    Odd = 2,
}

impl EncryptionKeySpec {
    pub fn from_bits(value: u8) -> Self {
        match value & 0b11 {
            0 => EncryptionKeySpec::None,
            1 => EncryptionKeySpec::Even,
            2 => EncryptionKeySpec::Odd,
            _ => EncryptionKeySpec::None,
        }
    }

    pub fn as_bits(self) -> u8 {
        self as u8
    }
}

/// Message number and flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgNumber {
    /// Message boundary (bits 30-31)
    pub boundary: PacketBoundary,
    /// In-order delivery flag (bit 29)
    pub in_order: bool,
    /// Encryption key spec (bits 27-28)
    pub encryption_key: EncryptionKeySpec,
    /// Retransmission flag (bit 26)
    pub retransmitted: bool,
    /// Message sequence number (bits 0-25)
    pub seq: u32,
}

impl MsgNumber {
    /// Create a new message number
    pub fn new(seq: u32) -> Self {
        MsgNumber {
            boundary: PacketBoundary::Solo,
            in_order: false,
            encryption_key: EncryptionKeySpec::None,
            retransmitted: false,
            seq: seq & 0x03FF_FFFF, // 26 bits
        }
    }

    /// Parse message number from raw 32-bit value
    pub fn from_raw(raw: u32) -> Self {
        MsgNumber {
            boundary: PacketBoundary::from_bits(((raw >> 30) & 0b11) as u8),
            in_order: (raw & (1 << 29)) != 0,
            encryption_key: EncryptionKeySpec::from_bits(((raw >> 27) & 0b11) as u8),
            retransmitted: (raw & (1 << 26)) != 0,
            seq: raw & 0x03FF_FFFF,
        }
    }

    /// Convert to raw 32-bit value
    pub fn to_raw(self) -> u32 {
        let mut raw = self.seq & 0x03FF_FFFF;
        raw |= (self.boundary.as_bits() as u32) << 30;
        if self.in_order {
            raw |= 1 << 29;
        }
        raw |= (self.encryption_key.as_bits() as u32) << 27;
        if self.retransmitted {
            raw |= 1 << 26;
        }
        raw
    }
}

/// Common packet header (128 bits = 16 bytes)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    /// Field 0: Sequence number or control information
    pub seq_or_control: u32,
    /// Field 1: Message number or additional info
    pub msg_or_info: u32,
    /// Field 2: Timestamp (microseconds)
    pub timestamp: u32,
    /// Field 3: Destination socket ID
    pub dest_socket_id: u32,
}

impl PacketHeader {
    /// Create a new data packet header
    pub fn new_data(
        seq: SeqNumber,
        msg_number: MsgNumber,
        timestamp: u32,
        dest_socket_id: u32,
    ) -> Self {
        PacketHeader {
            seq_or_control: seq.as_raw() & SEQ_MASK, // Ensure bit 31 is 0
            msg_or_info: msg_number.to_raw(),
            timestamp,
            dest_socket_id,
        }
    }

    /// Create a new control packet header
    pub fn new_control(
        control_type: ControlType,
        type_specific_info: u16,
        additional_info: u32,
        timestamp: u32,
        dest_socket_id: u32,
    ) -> Self {
        let seq_or_control =
            CONTROL_FLAG | ((control_type.as_u16() as u32) << 16) | (type_specific_info as u32);

        PacketHeader {
            seq_or_control,
            msg_or_info: additional_info,
            timestamp,
            dest_socket_id,
        }
    }

    /// Check if this is a control packet
    #[inline]
    pub fn is_control(&self) -> bool {
        (self.seq_or_control & CONTROL_FLAG) != 0
    }

    /// Check if this is a data packet
    #[inline]
    pub fn is_data(&self) -> bool {
        !self.is_control()
    }

    /// Get the sequence number (for data packets only)
    pub fn seq_number(&self) -> Option<SeqNumber> {
        if self.is_data() {
            Some(SeqNumber::new_unchecked(self.seq_or_control & SEQ_MASK))
        } else {
            None
        }
    }

    /// Get the control type (for control packets only)
    pub fn control_type(&self) -> Option<ControlType> {
        if self.is_control() {
            let type_value = ((self.seq_or_control >> 16) & 0x7FFF) as u16;
            ControlType::from_u16(type_value)
        } else {
            None
        }
    }

    /// Get the type-specific information field (for control packets only)
    pub fn type_specific_info(&self) -> Option<u16> {
        if self.is_control() {
            Some((self.seq_or_control & 0xFFFF) as u16)
        } else {
            None
        }
    }

    /// Get the message number (for data packets only)
    pub fn msg_number(&self) -> Option<MsgNumber> {
        if self.is_data() {
            Some(MsgNumber::from_raw(self.msg_or_info))
        } else {
            None
        }
    }

    /// Get the additional info field (for control packets only)
    pub fn additional_info(&self) -> Option<u32> {
        if self.is_control() {
            Some(self.msg_or_info)
        } else {
            None
        }
    }

    /// Parse header from bytes (network byte order)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        if bytes.len() < HEADER_SIZE {
            return Err(PacketError::InsufficientData {
                expected: HEADER_SIZE,
                actual: bytes.len(),
            });
        }

        let mut buf = &bytes[..HEADER_SIZE];
        Ok(PacketHeader {
            seq_or_control: buf.get_u32(),
            msg_or_info: buf.get_u32(),
            timestamp: buf.get_u32(),
            dest_socket_id: buf.get_u32(),
        })
    }

    /// Serialize header to bytes (network byte order)
    pub fn to_bytes(&self, buf: &mut BytesMut) {
        buf.put_u32(self.seq_or_control);
        buf.put_u32(self.msg_or_info);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.dest_socket_id);
    }
}

/// Data packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPacket {
    /// Packet header
    pub header: PacketHeader,
    /// Payload data
    pub payload: Bytes,
}

impl DataPacket {
    /// Create a new data packet
    pub fn new(
        seq: SeqNumber,
        msg_number: MsgNumber,
        timestamp: u32,
        dest_socket_id: u32,
        payload: Bytes,
    ) -> Self {
        DataPacket {
            header: PacketHeader::new_data(seq, msg_number, timestamp, dest_socket_id),
            payload,
        }
    }

    /// Get the sequence number
    pub fn seq_number(&self) -> SeqNumber {
        self.header.seq_number().expect("Data packet has seq number")
    }

    /// Get the message number
    pub fn msg_number(&self) -> MsgNumber {
        self.header.msg_number().expect("Data packet has msg number")
    }

    /// Total size of the packet (header + payload)
    pub fn size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }

    /// Serialize the packet to bytes
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.size());
        self.header.to_bytes(&mut buf);
        buf.put_slice(&self.payload);
        buf
    }

    /// Parse a data packet from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        let header = PacketHeader::from_bytes(bytes)?;

        if !header.is_data() {
            return Err(PacketError::WrongPacketType {
                expected: "data",
                actual: "control",
            });
        }

        let payload = if bytes.len() > HEADER_SIZE {
            Bytes::copy_from_slice(&bytes[HEADER_SIZE..])
        } else {
            Bytes::new()
        };

        Ok(DataPacket { header, payload })
    }
}

/// Control packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPacket {
    /// Packet header
    pub header: PacketHeader,
    /// Control information data
    pub control_info: Bytes,
}

impl ControlPacket {
    /// Create a new control packet
    pub fn new(
        control_type: ControlType,
        type_specific_info: u16,
        additional_info: u32,
        timestamp: u32,
        dest_socket_id: u32,
        control_info: Bytes,
    ) -> Self {
        ControlPacket {
            header: PacketHeader::new_control(
                control_type,
                type_specific_info,
                additional_info,
                timestamp,
                dest_socket_id,
            ),
            control_info,
        }
    }

    /// Get the control type
    pub fn control_type(&self) -> ControlType {
        self.header
            .control_type()
            .expect("Control packet has control type")
    }

    /// Total size of the packet (header + control info)
    pub fn size(&self) -> usize {
        HEADER_SIZE + self.control_info.len()
    }

    /// Serialize the packet to bytes
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(self.size());
        self.header.to_bytes(&mut buf);
        buf.put_slice(&self.control_info);
        buf
    }

    /// Parse a control packet from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        let header = PacketHeader::from_bytes(bytes)?;

        if !header.is_control() {
            return Err(PacketError::WrongPacketType {
                expected: "control",
                actual: "data",
            });
        }

        let control_info = if bytes.len() > HEADER_SIZE {
            Bytes::copy_from_slice(&bytes[HEADER_SIZE..])
        } else {
            Bytes::new()
        };

        Ok(ControlPacket {
            header,
            control_info,
        })
    }
}

/// Unified packet type (either data or control)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Data(DataPacket),
    Control(ControlPacket),
}

impl Packet {
    /// Check if this is a data packet
    pub fn is_data(&self) -> bool {
        matches!(self, Packet::Data(_))
    }

    /// Check if this is a control packet
    pub fn is_control(&self) -> bool {
        matches!(self, Packet::Control(_))
    }

    /// Get the packet header
    pub fn header(&self) -> &PacketHeader {
        match self {
            Packet::Data(p) => &p.header,
            Packet::Control(p) => &p.header,
        }
    }

    /// Get the destination socket ID
    pub fn dest_socket_id(&self) -> u32 {
        self.header().dest_socket_id
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u32 {
        self.header().timestamp
    }

    /// Total size of the packet
    pub fn size(&self) -> usize {
        match self {
            Packet::Data(p) => p.size(),
            Packet::Control(p) => p.size(),
        }
    }

    /// Serialize the packet to bytes
    pub fn to_bytes(&self) -> BytesMut {
        match self {
            Packet::Data(p) => p.to_bytes(),
            Packet::Control(p) => p.to_bytes(),
        }
    }

    /// Parse a packet from bytes (automatically determines type)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PacketError> {
        let header = PacketHeader::from_bytes(bytes)?;

        if header.is_data() {
            Ok(Packet::Data(DataPacket::from_bytes(bytes)?))
        } else {
            Ok(Packet::Control(ControlPacket::from_bytes(bytes)?))
        }
    }
}

/// Packet type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Data,
    Control(ControlType),
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketType::Data => write!(f, "Data"),
            PacketType::Control(ct) => write!(f, "Control({:?})", ct),
        }
    }
}

/// Packet parsing and validation errors
#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Insufficient data: expected {expected} bytes, got {actual}")]
    InsufficientData { expected: usize, actual: usize },

    #[error("Wrong packet type: expected {expected}, got {actual}")]
    WrongPacketType {
        expected: &'static str,
        actual: &'static str,
    },

    #[error("Invalid control type: {0}")]
    InvalidControlType(u16),

    #[error("Payload too large: {size} bytes (max {max})")]
    PayloadTooLarge { size: usize, max: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_boundary() {
        assert_eq!(PacketBoundary::from_bits(0b00), PacketBoundary::Subsequent);
        assert_eq!(PacketBoundary::from_bits(0b01), PacketBoundary::Last);
        assert_eq!(PacketBoundary::from_bits(0b10), PacketBoundary::First);
        assert_eq!(PacketBoundary::from_bits(0b11), PacketBoundary::Solo);
    }

    #[test]
    fn test_msg_number_roundtrip() {
        let msg = MsgNumber {
            boundary: PacketBoundary::First,
            in_order: true,
            encryption_key: EncryptionKeySpec::Even,
            retransmitted: false,
            seq: 12345,
        };

        let raw = msg.to_raw();
        let decoded = MsgNumber::from_raw(raw);

        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_data_packet_header() {
        let seq = SeqNumber::new(1000);
        let msg = MsgNumber::new(100);
        let header = PacketHeader::new_data(seq, msg, 5000, 9999);

        assert!(header.is_data());
        assert!(!header.is_control());
        assert_eq!(header.seq_number().unwrap(), seq);
        assert_eq!(header.dest_socket_id, 9999);
    }

    #[test]
    fn test_control_packet_header() {
        let header = PacketHeader::new_control(ControlType::Ack, 0x1234, 5000, 10000, 9999);

        assert!(header.is_control());
        assert!(!header.is_data());
        assert_eq!(header.control_type().unwrap(), ControlType::Ack);
        assert_eq!(header.type_specific_info().unwrap(), 0x1234);
        assert_eq!(header.additional_info().unwrap(), 5000);
    }

    #[test]
    fn test_data_packet_serialization() {
        let seq = SeqNumber::new(1000);
        let msg = MsgNumber::new(100);
        let payload = Bytes::from_static(b"Hello, SRT!");

        let packet = DataPacket::new(seq, msg, 5000, 9999, payload.clone());
        let bytes = packet.to_bytes();

        let decoded = DataPacket::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.seq_number(), seq);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_control_packet_serialization() {
        let control_info = Bytes::from_static(&[1, 2, 3, 4, 5]);

        let packet = ControlPacket::new(
            ControlType::KeepAlive,
            0x5678,
            1234,
            5000,
            9999,
            control_info.clone(),
        );
        let bytes = packet.to_bytes();

        let decoded = ControlPacket::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.control_type(), ControlType::KeepAlive);
        assert_eq!(decoded.control_info, control_info);
    }

    #[test]
    fn test_packet_auto_detect() {
        // Test data packet auto-detection
        let data_packet = DataPacket::new(
            SeqNumber::new(100),
            MsgNumber::new(10),
            1000,
            9999,
            Bytes::from_static(b"test"),
        );
        let bytes = data_packet.to_bytes();
        let packet = Packet::from_bytes(&bytes).unwrap();
        assert!(packet.is_data());

        // Test control packet auto-detection
        let control_packet =
            ControlPacket::new(ControlType::Ack, 0, 0, 1000, 9999, Bytes::new());
        let bytes = control_packet.to_bytes();
        let packet = Packet::from_bytes(&bytes).unwrap();
        assert!(packet.is_control());
    }
}
