//! SRT Protocol Core Implementation
//!
//! This crate implements the core SRT (Secure Reliable Transport) protocol,
//! including packet structures, handshake, connection state machine, buffers,
//! loss tracking, ACK/NAK generation, and congestion control.

pub mod ack;
pub mod buffer;
pub mod congestion;
pub mod connection;
pub mod handshake;
pub mod loss;
pub mod packet;
pub mod sequence;

pub use ack::{AckGenerator, AckInfo, NakGenerator, NakInfo, RttEstimator};
pub use buffer::{BufferError, ReceiveBuffer, SendBuffer};
pub use congestion::{BandwidthEstimator, CongestionController, CongestionStats};
pub use connection::{Connection, ConnectionError, ConnectionState, ConnectionStats};
pub use handshake::{HandshakeError, SrtHandshake, SrtOptions};
pub use loss::{LossRange, ReceiverLossList, SenderLossList};
pub use packet::{ControlPacket, DataPacket, MsgNumber, Packet, PacketBoundary, PacketType};
pub use sequence::SeqNumber;
