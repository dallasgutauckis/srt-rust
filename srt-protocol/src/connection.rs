//! SRT Connection State Machine
//!
//! Manages the lifecycle of an SRT connection from handshake through data
//! transfer to disconnection.

use crate::buffer::{ReceiveBuffer, SendBuffer};
use crate::handshake::{SrtHandshake, SrtOptions};
use crate::loss::{ReceiverLossList, SenderLossList};
use crate::packet::{DataPacket, MsgNumber};
use crate::sequence::SeqNumber;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Initial state, not yet connected
    Init,
    /// Handshake sent, waiting for response
    Connecting,
    /// Handshake complete, connection established
    Connected,
    /// Connection is being closed
    Closing,
    /// Connection is closed
    Closed,
}

/// Connection errors
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Connection is not in the correct state")]
    InvalidState,

    #[error("Connection is closed")]
    Closed,

    #[error("Buffer error: {0}")]
    Buffer(#[from] crate::buffer::BufferError),

    #[error("Handshake error: {0}")]
    Handshake(#[from] crate::handshake::HandshakeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets received
    pub packets_received: u64,
    /// Total packets lost
    pub packets_lost: u64,
    /// Total packets retransmitted
    pub packets_retransmitted: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Round-trip time (microseconds)
    pub rtt_us: u32,
    /// Estimated bandwidth (bytes per second)
    pub bandwidth_bps: u64,
}

/// SRT Connection
///
/// Represents a single SRT connection with send/receive buffers,
/// loss tracking, and connection state.
pub struct Connection {
    /// Connection state
    state: Arc<RwLock<ConnectionState>>,
    /// Local socket ID
    local_socket_id: u32,
    /// Remote socket ID
    remote_socket_id: Option<u32>,
    /// Local address
    _local_addr: SocketAddr,
    /// Remote address
    remote_addr: SocketAddr,
    /// Initial sequence number
    initial_seq_num: SeqNumber,
    /// SRT options negotiated
    options: SrtOptions,
    /// Send buffer
    send_buffer: Arc<RwLock<SendBuffer>>,
    /// Receive buffer
    recv_buffer: Arc<RwLock<ReceiveBuffer>>,
    /// Sender loss list
    _sender_losses: Arc<RwLock<SenderLossList>>,
    /// Receiver loss list
    _receiver_losses: Arc<RwLock<ReceiverLossList>>,
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
    /// Latency (milliseconds)
    latency_ms: u16,
}

impl Connection {
    /// Create a new connection
    pub fn new(
        local_socket_id: u32,
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        initial_seq_num: SeqNumber,
        latency_ms: u16,
    ) -> Self {
        Connection {
            state: Arc::new(RwLock::new(ConnectionState::Init)),
            local_socket_id,
            remote_socket_id: None,
            _local_addr: local_addr,
            remote_addr,
            initial_seq_num,
            options: SrtOptions::default_capabilities(),
            send_buffer: Arc::new(RwLock::new(SendBuffer::new(8192, Duration::from_secs(10)))),
            recv_buffer: Arc::new(RwLock::new(ReceiveBuffer::new(8192))),
            _sender_losses: Arc::new(RwLock::new(SenderLossList::new())),
            _receiver_losses: Arc::new(RwLock::new(ReceiverLossList::new(
                3,
                Duration::from_millis(100),
            ))),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            latency_ms,
        }
    }

    /// Get current connection state
    pub fn state(&self) -> ConnectionState {
        *self.state.read()
    }

    /// Set connection state
    fn set_state(&self, new_state: ConnectionState) {
        *self.state.write() = new_state;
    }

    /// Get local socket ID
    pub fn local_socket_id(&self) -> u32 {
        self.local_socket_id
    }

    /// Get remote socket ID
    pub fn remote_socket_id(&self) -> Option<u32> {
        self.remote_socket_id
    }

    /// Get remote address
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    /// Create handshake packet for connection initiation
    pub fn create_handshake(&self) -> SrtHandshake {
        SrtHandshake::new_request(
            self.initial_seq_num.as_raw(),
            self.local_socket_id,
            self.remote_addr,
            self.options,
            self.latency_ms,
            self.latency_ms,
        )
    }

    /// Process received handshake packet
    pub fn process_handshake(&mut self, handshake: SrtHandshake) -> Result<(), ConnectionError> {
        match self.state() {
            ConnectionState::Init | ConnectionState::Connecting => {
                // Store remote socket ID
                self.remote_socket_id = Some(handshake.udt.socket_id);

                // Negotiate options (use minimum capabilities)
                if let Some(peer_caps) = handshake.peer_capabilities() {
                    self.options = self.negotiate_options(&peer_caps);
                }

                // Transition to connected
                self.set_state(ConnectionState::Connected);
                Ok(())
            }
            _ => Err(ConnectionError::InvalidState),
        }
    }

    /// Negotiate options with peer
    fn negotiate_options(&self, peer: &SrtOptions) -> SrtOptions {
        SrtOptions {
            tsbpd_sender: self.options.tsbpd_sender && peer.tsbpd_sender,
            tsbpd_receiver: self.options.tsbpd_receiver && peer.tsbpd_receiver,
            encryption: self.options.encryption && peer.encryption,
            too_late_packet_drop: self.options.too_late_packet_drop && peer.too_late_packet_drop,
            nak_report: self.options.nak_report && peer.nak_report,
            rexmit_flag: self.options.rexmit_flag && peer.rexmit_flag,
            stream_mode: self.options.stream_mode && peer.stream_mode,
            packet_filter: self.options.packet_filter && peer.packet_filter,
        }
    }

    /// Send data
    pub fn send(&self, data: &[u8]) -> Result<usize, ConnectionError> {
        if self.state() != ConnectionState::Connected {
            return Err(ConnectionError::InvalidState);
        }

        // Create data packet
        let mut send_buf = self.send_buffer.write();
        let packet = DataPacket::new(
            SeqNumber::new(0), // Will be assigned by buffer
            MsgNumber::new(0), // Simplified for now
            0,                 // Timestamp will be set later
            self.remote_socket_id.unwrap_or(0),
            bytes::Bytes::copy_from_slice(data),
        );

        send_buf.push(packet)?;

        // Update stats
        let mut stats = self.stats.write();
        stats.packets_sent += 1;
        stats.bytes_sent += data.len() as u64;

        Ok(data.len())
    }

    /// Receive data
    pub fn recv(&self) -> Result<Option<bytes::Bytes>, ConnectionError> {
        if self.state() != ConnectionState::Connected {
            return Err(ConnectionError::InvalidState);
        }

        let mut recv_buf = self.recv_buffer.write();
        if let Some(message) = recv_buf.pop_message() {
            let mut stats = self.stats.write();
            stats.packets_received += 1;
            stats.bytes_received += message.len() as u64;
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    /// Process received data packet
    pub fn process_data_packet(&self, packet: DataPacket) -> Result<(), ConnectionError> {
        if self.state() != ConnectionState::Connected {
            return Err(ConnectionError::InvalidState);
        }

        let mut recv_buf = self.recv_buffer.write();
        recv_buf.push(packet)?;

        Ok(())
    }

    /// Get connection statistics
    pub fn stats(&self) -> ConnectionStats {
        self.stats.read().clone()
    }

    /// Close the connection
    pub fn close(&self) {
        self.set_state(ConnectionState::Closing);
        // In a real implementation, send SHUTDOWN control packet
        self.set_state(ConnectionState::Closed);
    }

    /// Check if connection is established
    pub fn is_connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }

    /// Check if connection is closed
    pub fn is_closed(&self) -> bool {
        matches!(self.state(), ConnectionState::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_lifecycle() {
        let conn = Connection::new(
            12345,
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            SeqNumber::new(1000),
            120,
        );

        assert_eq!(conn.state(), ConnectionState::Init);
        assert!(!conn.is_connected());

        // In a real scenario, handshake would be exchanged
        // For now, just verify state transitions work
        conn.close();
        assert!(conn.is_closed());
    }

    #[test]
    fn test_option_negotiation() {
        let conn = Connection::new(
            12345,
            "127.0.0.1:9000".parse().unwrap(),
            "127.0.0.1:9001".parse().unwrap(),
            SeqNumber::new(1000),
            120,
        );

        let mut peer_opts = SrtOptions::default_capabilities();
        peer_opts.encryption = false; // Peer doesn't support encryption

        let negotiated = conn.negotiate_options(&peer_opts);
        assert!(!negotiated.encryption); // Should be disabled
    }
}
