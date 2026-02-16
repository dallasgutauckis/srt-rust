//! Circular buffers for send and receive packet storage
//!
//! SRT uses circular buffers indexed by sequence numbers for efficient
//! packet storage and retrieval.

use crate::packet::DataPacket;
use crate::sequence::SeqNumber;
use bytes::Bytes;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Buffer errors
#[derive(Error, Debug)]
pub enum BufferError {
    #[error("Buffer is full")]
    Full,

    #[error("Packet not found: {0}")]
    NotFound(SeqNumber),

    #[error("Sequence number out of range")]
    OutOfRange,

    #[error("Invalid message number")]
    InvalidMessage,
}

/// Stored packet with metadata
#[derive(Clone)]
struct StoredPacket {
    /// The packet data
    packet: DataPacket,
    /// Time when packet was first sent
    first_sent: Instant,
    /// Time when packet was last sent (for retransmission)
    last_sent: Instant,
    /// Number of times this packet has been sent
    send_count: u32,
    /// Whether this packet has been acknowledged
    acknowledged: bool,
}

/// Circular send buffer
///
/// Stores sent packets for potential retransmission. Indexed by sequence number.
pub struct SendBuffer {
    /// Buffer storage (circular)
    buffer: Vec<Option<StoredPacket>>,
    /// Buffer capacity (power of 2 for efficient modulo)
    capacity: usize,
    /// Mask for fast modulo operation (capacity - 1)
    mask: usize,
    /// Next sequence number to send
    next_seq: SeqNumber,
    /// Oldest unacknowledged sequence number
    oldest_unacked: SeqNumber,
    /// Oldest packet in buffer (acknowledged or not)
    oldest_in_buffer: SeqNumber,
    /// Time-to-live for packets (packets older than this are dropped)
    ttl: Duration,
}

impl SendBuffer {
    /// Create a new send buffer
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of packets to store (will be rounded up to power of 2)
    /// * `ttl` - Time-to-live for packets
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        // Round up to next power of 2 for efficient modulo
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;

        SendBuffer {
            buffer: vec![None; capacity],
            capacity,
            mask,
            next_seq: SeqNumber::new(0),
            oldest_unacked: SeqNumber::new(0),
            oldest_in_buffer: SeqNumber::new(0),
            ttl,
        }
    }

    /// Get the index in the buffer for a given sequence number
    #[inline]
    fn index(&self, seq: SeqNumber) -> usize {
        (seq.as_raw() as usize) & self.mask
    }

    /// Add a packet to the buffer
    ///
    /// Returns the sequence number assigned to the packet.
    pub fn push(&mut self, mut packet: DataPacket) -> Result<SeqNumber, BufferError> {
        // Check if buffer is full
        let available = self.available_space();
        if available == 0 {
            // Try to drop old packets
            self.drop_expired();
            if self.available_space() == 0 {
                return Err(BufferError::Full);
            }
        }

        // Assign sequence number
        let seq = self.next_seq;
        packet.header.seq_or_control = seq.as_raw();

        let idx = self.index(seq);
        let now = Instant::now();

        self.buffer[idx] = Some(StoredPacket {
            packet,
            first_sent: now,
            last_sent: now,
            send_count: 1,
            acknowledged: false,
        });

        self.next_seq = seq.next();

        Ok(seq)
    }

    /// Get a packet for transmission
    ///
    /// Returns a reference to the packet and updates send statistics.
    pub fn get_for_send(&mut self, seq: SeqNumber) -> Result<DataPacket, BufferError> {
        let idx = self.index(seq);

        match &mut self.buffer[idx] {
            Some(stored) if stored.packet.seq_number() == seq => {
                stored.last_sent = Instant::now();
                stored.send_count += 1;

                // Mark as retransmitted if sent more than once
                if stored.send_count > 1 {
                    let mut msg = stored.packet.msg_number();
                    msg.retransmitted = true;
                    stored.packet.header.msg_or_info = msg.to_raw();
                }

                Ok(stored.packet.clone())
            }
            _ => Err(BufferError::NotFound(seq)),
        }
    }

    /// Get a packet by sequence number (read-only)
    pub fn get(&self, seq: SeqNumber) -> Result<&DataPacket, BufferError> {
        let idx = self.index(seq);

        match &self.buffer[idx] {
            Some(stored) if stored.packet.seq_number() == seq => Ok(&stored.packet),
            _ => Err(BufferError::NotFound(seq)),
        }
    }

    /// Mark a packet as acknowledged
    pub fn acknowledge(&mut self, seq: SeqNumber) -> Result<(), BufferError> {
        let idx = self.index(seq);

        match &mut self.buffer[idx] {
            Some(stored) if stored.packet.seq_number() == seq => {
                stored.acknowledged = true;
                Ok(())
            }
            _ => Err(BufferError::NotFound(seq)),
        }
    }

    /// Acknowledge all packets up to and including `seq`
    pub fn acknowledge_up_to(&mut self, seq: SeqNumber) {
        let mut current = self.oldest_unacked;

        while current.le(seq) && current.lt(self.next_seq) {
            let _ = self.acknowledge(current);
            current = current.next();
        }

        // Update oldest unacked
        self.oldest_unacked = seq.next();
    }

    /// Remove acknowledged packets from the buffer
    pub fn flush_acknowledged(&mut self) -> usize {
        let mut count = 0;
        let mut current = self.oldest_in_buffer;

        while current.lt(self.next_seq) {
            let idx = self.index(current);

            if let Some(stored) = &self.buffer[idx] {
                if stored.acknowledged {
                    self.buffer[idx] = None;
                    count += 1;
                    current = current.next();
                } else {
                    break;
                }
            } else {
                // Slot is empty, skip to next
                current = current.next();
            }
        }

        // Update oldest_in_buffer to the first non-flushed packet
        self.oldest_in_buffer = current;
        self.oldest_unacked = current;
        count
    }

    /// Drop packets that have exceeded TTL
    pub fn drop_expired(&mut self) -> usize {
        let mut count = 0;
        let now = Instant::now();

        for slot in &mut self.buffer {
            if let Some(stored) = slot {
                if now.duration_since(stored.first_sent) > self.ttl {
                    *slot = None;
                    count += 1;
                }
            }
        }

        count
    }

    /// Get the number of packets currently in the buffer
    pub fn len(&self) -> usize {
        self.next_seq.as_raw().wrapping_sub(self.oldest_unacked.as_raw()) as usize
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get available space in the buffer
    pub fn available_space(&self) -> usize {
        self.capacity.saturating_sub(self.len())
    }

    /// Get the next sequence number to be used
    pub fn next_seq(&self) -> SeqNumber {
        self.next_seq
    }

    /// Get the oldest unacknowledged sequence number
    pub fn oldest_unacked(&self) -> SeqNumber {
        self.oldest_unacked
    }

    /// Check if a sequence number is in the valid range
    pub fn contains(&self, seq: SeqNumber) -> bool {
        seq.ge(self.oldest_unacked) && seq.lt(self.next_seq)
    }
}

/// Received packet entry
#[derive(Clone)]
struct ReceivedPacket {
    packet: DataPacket,
    _received_at: Instant,
}

/// Circular receive buffer
///
/// Handles out-of-order packet reception and message reassembly.
pub struct ReceiveBuffer {
    /// Buffer storage (circular)
    buffer: Vec<Option<ReceivedPacket>>,
    /// Buffer capacity
    capacity: usize,
    /// Mask for fast modulo
    mask: usize,
    /// Next expected sequence number
    next_expected: SeqNumber,
    /// Highest received sequence number
    highest_received: SeqNumber,
    /// Queue for reassembled messages ready for delivery
    ready_messages: VecDeque<Bytes>,
}

impl ReceiveBuffer {
    /// Create a new receive buffer
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let mask = capacity - 1;

        ReceiveBuffer {
            buffer: vec![None; capacity],
            capacity,
            mask,
            next_expected: SeqNumber::new(0),
            highest_received: SeqNumber::new(0),
            ready_messages: VecDeque::new(),
        }
    }

    /// Get the index for a sequence number
    #[inline]
    fn index(&self, seq: SeqNumber) -> usize {
        (seq.as_raw() as usize) & self.mask
    }

    /// Add a received packet to the buffer
    pub fn push(&mut self, packet: DataPacket) -> Result<(), BufferError> {
        let seq = packet.seq_number();

        // Check if this is a duplicate or too old
        if seq.lt(self.next_expected) {
            // Packet is too old, ignore it
            return Ok(());
        }

        // Check if packet is too far ahead
        let distance = self.next_expected.distance_to(seq);
        if distance >= self.capacity as i32 {
            return Err(BufferError::OutOfRange);
        }

        let idx = self.index(seq);

        // Store the packet
        self.buffer[idx] = Some(ReceivedPacket {
            packet,
            _received_at: Instant::now(),
        });

        // Update highest received
        if seq.gt(self.highest_received) {
            self.highest_received = seq;
        }

        // Try to reassemble messages
        self.reassemble_messages();

        Ok(())
    }

    /// Reassemble complete messages from received packets
    fn reassemble_messages(&mut self) {
        while let Some(received) = &self.buffer[self.index(self.next_expected)] {
            let packet = &received.packet;
            let msg_num = packet.msg_number();

            // Check message boundary
            match msg_num.boundary {
                crate::packet::PacketBoundary::Solo => {
                    // Complete message in single packet
                    self.ready_messages.push_back(packet.payload.clone());
                    let idx = self.index(self.next_expected);
                    self.buffer[idx] = None;
                    self.next_expected = self.next_expected.next();
                }
                crate::packet::PacketBoundary::First => {
                    // Start of multi-packet message
                    if let Some(message) = self.reassemble_multi_packet_message() {
                        self.ready_messages.push_back(message);
                    } else {
                        break; // Not all packets available yet
                    }
                }
                _ => {
                    // Invalid: message should start with First or Solo
                    // Skip this packet
                    let idx = self.index(self.next_expected);
                    self.buffer[idx] = None;
                    self.next_expected = self.next_expected.next();
                }
            }
        }
    }

    /// Reassemble a multi-packet message starting at next_expected
    fn reassemble_multi_packet_message(&mut self) -> Option<Bytes> {
        let mut packets = Vec::new();
        let mut current_seq = self.next_expected;
        let first_msg_num = self.buffer[self.index(current_seq)]
            .as_ref()?
            .packet
            .msg_number()
            .seq;

        loop {
            let idx = self.index(current_seq);
            let received = self.buffer[idx].as_ref()?;
            let packet = &received.packet;
            let msg_num = packet.msg_number();

            // Check if this packet belongs to the same message
            if msg_num.seq != first_msg_num {
                return None; // Not part of this message
            }

            packets.push(packet.payload.clone());

            match msg_num.boundary {
                crate::packet::PacketBoundary::Last => {
                    // End of message
                    // Concatenate all packets
                    let mut message = bytes::BytesMut::new();
                    for payload in &packets {
                        message.extend_from_slice(payload);
                    }

                    // Clear the packets from buffer
                    for i in 0..packets.len() {
                        let seq = self.next_expected + (i as u32);
                        let idx = self.index(seq);
                        self.buffer[idx] = None;
                    }

                    self.next_expected = current_seq.next();
                    return Some(message.freeze());
                }
                crate::packet::PacketBoundary::First | crate::packet::PacketBoundary::Subsequent => {
                    // First or middle packet, continue
                    current_seq = current_seq.next();
                }
                _ => {
                    // Invalid boundary (Solo shouldn't appear in multi-packet message)
                    return None;
                }
            }
        }
    }

    /// Get the next ready message
    pub fn pop_message(&mut self) -> Option<Bytes> {
        self.ready_messages.pop_front()
    }

    /// Get number of ready messages
    pub fn ready_message_count(&self) -> usize {
        self.ready_messages.len()
    }

    /// Get missing sequence numbers (gaps) for NAK generation
    pub fn get_loss_list(&self) -> Vec<SeqNumber> {
        let mut losses = Vec::new();
        let mut current = self.next_expected;

        while current.le(self.highest_received) {
            if self.buffer[self.index(current)].is_none() {
                losses.push(current);
            }
            current = current.next();
        }

        losses
    }

    /// Get the next expected sequence number
    pub fn next_expected(&self) -> SeqNumber {
        self.next_expected
    }

    /// Get the highest received sequence number
    pub fn highest_received(&self) -> SeqNumber {
        self.highest_received
    }

    /// Get buffer utilization (0.0 to 1.0)
    pub fn utilization(&self) -> f32 {
        let filled = self
            .buffer
            .iter()
            .filter(|slot| slot.is_some())
            .count();
        filled as f32 / self.capacity as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{MsgNumber, PacketBoundary};

    fn create_test_packet(seq: u32, msg_seq: u32, payload: &[u8]) -> DataPacket {
        DataPacket::new(
            SeqNumber::new(seq),
            MsgNumber::new(msg_seq),
            0,
            0,
            Bytes::copy_from_slice(payload),
        )
    }

    #[test]
    fn test_send_buffer_push_pop() {
        let mut buffer = SendBuffer::new(16, Duration::from_secs(10));

        let packet = create_test_packet(0, 0, b"test");
        let seq = buffer.push(packet.clone()).unwrap();

        assert_eq!(seq, SeqNumber::new(0));
        assert_eq!(buffer.len(), 1);

        let retrieved = buffer.get(seq).unwrap();
        assert_eq!(retrieved.payload, packet.payload);
    }

    #[test]
    fn test_send_buffer_acknowledge() {
        let mut buffer = SendBuffer::new(16, Duration::from_secs(10));

        let seq1 = buffer.push(create_test_packet(0, 0, b"test1")).unwrap();
        let seq2 = buffer.push(create_test_packet(0, 1, b"test2")).unwrap();
        let seq3 = buffer.push(create_test_packet(0, 2, b"test3")).unwrap();

        buffer.acknowledge_up_to(seq2);
        let flushed = buffer.flush_acknowledged();

        assert_eq!(flushed, 2); // seq1 and seq2
        assert!(buffer.get(seq1).is_err());
        assert!(buffer.get(seq2).is_err());
        assert!(buffer.get(seq3).is_ok());
    }

    #[test]
    fn test_receive_buffer_in_order() {
        let mut buffer = ReceiveBuffer::new(16);

        let mut packet = create_test_packet(0, 0, b"message1");
        packet.header.msg_or_info = MsgNumber {
            boundary: PacketBoundary::Solo,
            in_order: false,
            encryption_key: crate::packet::EncryptionKeySpec::None,
            retransmitted: false,
            seq: 0,
        }
        .to_raw();

        buffer.push(packet).unwrap();

        assert_eq!(buffer.ready_message_count(), 1);
        let msg = buffer.pop_message().unwrap();
        assert_eq!(&msg[..], b"message1");
    }

    #[test]
    fn test_receive_buffer_out_of_order() {
        let mut buffer = ReceiveBuffer::new(16);

        // Receive packet 2 first
        let mut packet2 = create_test_packet(2, 0, b"pkt2");
        packet2.header.msg_or_info = MsgNumber {
            boundary: PacketBoundary::Solo,
            seq: 2,
            ..MsgNumber::new(0)
        }
        .to_raw();
        packet2.header.seq_or_control = 2;

        buffer.push(packet2).unwrap();
        assert_eq!(buffer.ready_message_count(), 0); // Not ready yet

        // Receive packet 1
        let mut packet1 = create_test_packet(1, 0, b"pkt1");
        packet1.header.msg_or_info = MsgNumber {
            boundary: PacketBoundary::Solo,
            seq: 1,
            ..MsgNumber::new(0)
        }
        .to_raw();
        packet1.header.seq_or_control = 1;

        buffer.push(packet1).unwrap();
        assert_eq!(buffer.ready_message_count(), 0); // Still waiting for packet 0

        // Receive packet 0
        let mut packet0 = create_test_packet(0, 0, b"pkt0");
        packet0.header.msg_or_info = MsgNumber {
            boundary: PacketBoundary::Solo,
            seq: 0,
            ..MsgNumber::new(0)
        }
        .to_raw();

        buffer.push(packet0).unwrap();
        assert_eq!(buffer.ready_message_count(), 3); // All three ready
    }

    #[test]
    fn test_receive_buffer_loss_detection() {
        let mut buffer = ReceiveBuffer::new(16);

        // Receive packets 0, 2, 3 (missing 1)
        for i in [0, 2, 3] {
            let mut packet = create_test_packet(i, i, b"test");
            packet.header.seq_or_control = i;
            packet.header.msg_or_info = MsgNumber {
                boundary: PacketBoundary::Solo,
                seq: i,
                ..MsgNumber::new(0)
            }
            .to_raw();
            buffer.push(packet).unwrap();
        }

        let losses = buffer.get_loss_list();
        assert_eq!(losses, vec![SeqNumber::new(1)]);
    }
}
