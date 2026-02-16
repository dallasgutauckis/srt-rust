//! SRT Protocol Core Implementation
//!
//! This crate implements the core SRT (Secure Reliable Transport) protocol,
//! including packet structures, handshake, connection state machine, buffers,
//! loss tracking, ACK/NAK generation, and congestion control.

pub mod packet;
pub mod sequence;
pub mod buffer;
pub mod loss;
pub mod handshake;
pub mod connection;
pub mod ack;
pub mod congestion;

pub use packet::{ControlPacket, DataPacket, Packet, PacketType, MsgNumber, PacketBoundary};
pub use sequence::SeqNumber;
pub use buffer::{ReceiveBuffer, SendBuffer, BufferError};
pub use loss::{LossRange, ReceiverLossList, SenderLossList};
pub use handshake::{SrtHandshake, SrtOptions, HandshakeError};
pub use connection::{Connection, ConnectionState, ConnectionStats, ConnectionError};
pub use ack::{AckGenerator, AckInfo, NakGenerator, NakInfo, RttEstimator};
pub use congestion::{BandwidthEstimator, CongestionController, CongestionStats};
