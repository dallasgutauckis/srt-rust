//! SRT Handshake Protocol
//!
//! Implements the SRT connection handshake for establishing connections
//! between peers with version negotiation and capability exchange.

use bytes::{Buf, BufMut, BytesMut};
use std::net::SocketAddr;
use thiserror::Error;

/// SRT protocol version
pub const SRT_VERSION: u32 = 0x00010500; // Version 1.5.0

/// SRT magic code for handshake
pub const SRT_MAGIC_CODE: u32 = 0x4A17;

/// Handshake errors
#[derive(Error, Debug)]
pub enum HandshakeError {
    #[error("Incompatible version: {0}")]
    IncompatibleVersion(u32),

    #[error("Invalid handshake packet")]
    InvalidPacket,

    #[error("Extension parse error")]
    ExtensionError,

    #[error("Handshake rejected by peer")]
    Rejected,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// SRT handshake options/capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtOptions {
    /// Timestamp-based packet delivery (sender)
    pub tsbpd_sender: bool,
    /// Timestamp-based packet delivery (receiver)
    pub tsbpd_receiver: bool,
    /// Encryption support
    pub encryption: bool,
    /// Too-late packet drop
    pub too_late_packet_drop: bool,
    /// Periodic NAK report
    pub nak_report: bool,
    /// Retransmit flag support
    pub rexmit_flag: bool,
    /// Stream mode (vs message mode)
    pub stream_mode: bool,
    /// Packet filter support
    pub packet_filter: bool,
}

impl SrtOptions {
    /// Default capabilities for this implementation
    pub fn default_capabilities() -> Self {
        SrtOptions {
            tsbpd_sender: true,
            tsbpd_receiver: true,
            encryption: true,
            too_late_packet_drop: true,
            nak_report: true,
            rexmit_flag: true,
            stream_mode: false, // Default to message mode
            packet_filter: false,
        }
    }

    /// Convert to bit flags
    pub fn to_flags(&self) -> u32 {
        let mut flags = 0u32;
        if self.tsbpd_sender {
            flags |= 1 << 0;
        }
        if self.tsbpd_receiver {
            flags |= 1 << 1;
        }
        if self.encryption {
            flags |= 1 << 2;
        }
        if self.too_late_packet_drop {
            flags |= 1 << 3;
        }
        if self.nak_report {
            flags |= 1 << 4;
        }
        if self.rexmit_flag {
            flags |= 1 << 5;
        }
        if self.stream_mode {
            flags |= 1 << 6;
        }
        if self.packet_filter {
            flags |= 1 << 7;
        }
        flags
    }

    /// Parse from bit flags
    pub fn from_flags(flags: u32) -> Self {
        SrtOptions {
            tsbpd_sender: (flags & (1 << 0)) != 0,
            tsbpd_receiver: (flags & (1 << 1)) != 0,
            encryption: (flags & (1 << 2)) != 0,
            too_late_packet_drop: (flags & (1 << 3)) != 0,
            nak_report: (flags & (1 << 4)) != 0,
            rexmit_flag: (flags & (1 << 5)) != 0,
            stream_mode: (flags & (1 << 6)) != 0,
            packet_filter: (flags & (1 << 7)) != 0,
        }
    }
}

/// Handshake type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeType {
    /// Initiation request
    Induction = 1,
    /// Conclusion request
    Conclusion = -1,
    /// Agreement response
    Agreement = -2,
}

/// UDT handshake packet structure
///
/// This is the base handshake packet format inherited from UDT.
#[derive(Debug, Clone)]
pub struct UdtHandshake {
    /// UDT version (always 4)
    pub version: u32,
    /// Socket type (1 = stream)
    pub socket_type: u32,
    /// Initial sequence number
    pub initial_seq_num: u32,
    /// Maximum packet size
    pub max_packet_size: u32,
    /// Maximum flow window size
    pub max_flow_window: u32,
    /// Handshake type
    pub handshake_type: i32,
    /// Socket ID
    pub socket_id: u32,
    /// SYN cookie (for rendezvous)
    pub syn_cookie: u32,
    /// Peer IP address
    pub peer_addr: SocketAddr,
}

impl UdtHandshake {
    /// Create a new handshake request
    pub fn new_request(
        initial_seq_num: u32,
        max_packet_size: u32,
        max_flow_window: u32,
        socket_id: u32,
        peer_addr: SocketAddr,
    ) -> Self {
        UdtHandshake {
            version: 4,
            socket_type: 1, // Stream
            initial_seq_num,
            max_packet_size,
            max_flow_window,
            handshake_type: HandshakeType::Induction as i32,
            socket_id,
            syn_cookie: 0,
            peer_addr,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(48);

        buf.put_u32(self.version);
        buf.put_u32(self.socket_type);
        buf.put_u32(self.initial_seq_num);
        buf.put_u32(self.max_packet_size);
        buf.put_u32(self.max_flow_window);
        buf.put_i32(self.handshake_type);
        buf.put_u32(self.socket_id);
        buf.put_u32(self.syn_cookie);

        // Peer IP address (16 bytes for IPv6, zeros for IPv4)
        match self.peer_addr {
            SocketAddr::V4(addr) => {
                buf.put_u32(u32::from(*addr.ip()));
                buf.put_u64(0);
                buf.put_u32(0);
            }
            SocketAddr::V6(addr) => {
                for &byte in addr.ip().octets().iter() {
                    buf.put_u8(byte);
                }
            }
        }

        buf
    }

    /// Parse from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HandshakeError> {
        if bytes.len() < 48 {
            return Err(HandshakeError::InvalidPacket);
        }

        let mut buf = &bytes[..48];

        let version = buf.get_u32();
        let socket_type = buf.get_u32();
        let initial_seq_num = buf.get_u32();
        let max_packet_size = buf.get_u32();
        let max_flow_window = buf.get_u32();
        let handshake_type = buf.get_i32();
        let socket_id = buf.get_u32();
        let syn_cookie = buf.get_u32();

        // Parse IP address
        let peer_addr = if buf[0..4] != [0, 0, 0, 0] || buf[4..16] == [0; 12] {
            // IPv4
            let ip = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
            SocketAddr::from(([
                ((ip >> 24) & 0xFF) as u8,
                ((ip >> 16) & 0xFF) as u8,
                ((ip >> 8) & 0xFF) as u8,
                (ip & 0xFF) as u8,
            ], 0))
        } else {
            // IPv6
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&buf[0..16]);
            SocketAddr::from((octets, 0))
        };

        Ok(UdtHandshake {
            version,
            socket_type,
            initial_seq_num,
            max_packet_size,
            max_flow_window,
            handshake_type,
            socket_id,
            syn_cookie,
            peer_addr,
        })
    }
}

/// SRT-specific handshake extension
#[derive(Debug, Clone)]
pub struct SrtHandshakeExtension {
    /// SRT version
    pub srt_version: u32,
    /// SRT flags/options
    pub srt_flags: u32,
    /// Latency (receiver in high 16 bits, sender in low 16 bits)
    pub latency: u32,
}

impl SrtHandshakeExtension {
    /// Create new SRT extension
    pub fn new(
        options: SrtOptions,
        recv_latency_ms: u16,
        send_latency_ms: u16,
    ) -> Self {
        let latency = ((recv_latency_ms as u32) << 16) | (send_latency_ms as u32);

        SrtHandshakeExtension {
            srt_version: SRT_VERSION,
            srt_flags: options.to_flags(),
            latency,
        }
    }

    /// Serialize as handshake extension
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(16);

        // Extension header: type (HSREQ=1) and size (3 words)
        buf.put_u16(1); // SRT_CMD_HSREQ
        buf.put_u16(3); // Size in 32-bit words

        // Extension data
        buf.put_u32(self.srt_version);
        buf.put_u32(self.srt_flags);
        buf.put_u32(self.latency);

        buf
    }

    /// Parse from extension bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HandshakeError> {
        if bytes.len() < 16 {
            return Err(HandshakeError::ExtensionError);
        }

        let mut buf = bytes;

        let ext_type = buf.get_u16();
        let ext_size = buf.get_u16();

        if ext_type != 1 || ext_size != 3 {
            return Err(HandshakeError::ExtensionError);
        }

        let srt_version = buf.get_u32();
        let srt_flags = buf.get_u32();
        let latency = buf.get_u32();

        Ok(SrtHandshakeExtension {
            srt_version,
            srt_flags,
            latency,
        })
    }

    /// Get receiver latency in milliseconds
    pub fn recv_latency_ms(&self) -> u16 {
        ((self.latency >> 16) & 0xFFFF) as u16
    }

    /// Get sender latency in milliseconds
    pub fn send_latency_ms(&self) -> u16 {
        (self.latency & 0xFFFF) as u16
    }

    /// Get SRT options
    pub fn options(&self) -> SrtOptions {
        SrtOptions::from_flags(self.srt_flags)
    }
}

/// Complete SRT handshake
#[derive(Debug, Clone)]
pub struct SrtHandshake {
    /// Base UDT handshake
    pub udt: UdtHandshake,
    /// SRT extension (if present)
    pub srt_ext: Option<SrtHandshakeExtension>,
}

impl SrtHandshake {
    /// Create a new SRT handshake request
    pub fn new_request(
        initial_seq_num: u32,
        socket_id: u32,
        peer_addr: SocketAddr,
        options: SrtOptions,
        recv_latency_ms: u16,
        send_latency_ms: u16,
    ) -> Self {
        let udt = UdtHandshake::new_request(
            initial_seq_num,
            1456, // Default MTU - headers
            8192, // Default flow window
            socket_id,
            peer_addr,
        );

        let srt_ext = Some(SrtHandshakeExtension::new(
            options,
            recv_latency_ms,
            send_latency_ms,
        ));

        SrtHandshake { udt, srt_ext }
    }

    /// Serialize complete handshake
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = self.udt.to_bytes();

        if let Some(ref ext) = self.srt_ext {
            buf.extend_from_slice(&ext.to_bytes());
        }

        buf
    }

    /// Parse complete handshake
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HandshakeError> {
        let udt = UdtHandshake::from_bytes(bytes)?;

        let srt_ext = if bytes.len() > 48 {
            Some(SrtHandshakeExtension::from_bytes(&bytes[48..])?)
        } else {
            None
        };

        Ok(SrtHandshake { udt, srt_ext })
    }

    /// Check if this is an SRT handshake (vs plain UDT)
    pub fn is_srt(&self) -> bool {
        self.srt_ext.is_some()
    }

    /// Get peer's SRT version
    pub fn peer_srt_version(&self) -> Option<u32> {
        self.srt_ext.as_ref().map(|ext| ext.srt_version)
    }

    /// Get peer's capabilities
    pub fn peer_capabilities(&self) -> Option<SrtOptions> {
        self.srt_ext.as_ref().map(|ext| ext.options())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_options_flags() {
        let options = SrtOptions::default_capabilities();
        let flags = options.to_flags();
        let decoded = SrtOptions::from_flags(flags);

        assert_eq!(decoded, options);
    }

    #[test]
    fn test_udt_handshake_roundtrip() {
        let hs = UdtHandshake::new_request(
            1000,
            1456,
            8192,
            12345,
            "127.0.0.1:9000".parse().unwrap(),
        );

        let bytes = hs.to_bytes();
        let decoded = UdtHandshake::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.version, hs.version);
        assert_eq!(decoded.initial_seq_num, hs.initial_seq_num);
        assert_eq!(decoded.socket_id, hs.socket_id);
    }

    #[test]
    fn test_srt_extension_roundtrip() {
        let ext = SrtHandshakeExtension::new(
            SrtOptions::default_capabilities(),
            120, // recv latency
            80,  // send latency
        );

        let bytes = ext.to_bytes();
        let decoded = SrtHandshakeExtension::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.srt_version, ext.srt_version);
        assert_eq!(decoded.srt_flags, ext.srt_flags);
        assert_eq!(decoded.recv_latency_ms(), 120);
        assert_eq!(decoded.send_latency_ms(), 80);
    }

    #[test]
    fn test_complete_handshake() {
        let hs = SrtHandshake::new_request(
            1000,
            12345,
            "127.0.0.1:9000".parse().unwrap(),
            SrtOptions::default_capabilities(),
            120,
            80,
        );

        assert!(hs.is_srt());
        assert_eq!(hs.peer_srt_version(), Some(SRT_VERSION));

        let bytes = hs.to_bytes();
        let decoded = SrtHandshake::from_bytes(&bytes).unwrap();

        assert!(decoded.is_srt());
        assert_eq!(decoded.udt.socket_id, hs.udt.socket_id);
    }
}
