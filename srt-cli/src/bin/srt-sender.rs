//! SRT Sender - Multi-path stream sender
//!
//! Reads from stdin, file, or UDP/SRT input and sends over multiple SRT paths with bonding.

use clap::Parser;
use srt_bonding::*;
use srt_protocol::{Connection, SeqNumber, DataPacket, MsgNumber, SrtHandshake};
use srt_io::SrtSocket;
use bytes::Bytes;
use std::fs::File;
use std::io::{self, Read, BufReader, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

#[derive(Parser, Debug)]
#[command(name = "srt-sender")]
#[command(about = "SRT multi-path sender", long_about = None)]
struct Args {
    /// Input source: file path, '-' for stdin, 'udp://host:port' for UDP input
    #[arg(short, long, default_value = "-")]
    input: String,

    /// Bonding mode (broadcast, backup, balancing)
    #[arg(short = 'g', long, default_value = "broadcast")]
    group: String,

    /// Output paths (format: host:port)
    #[arg(short, long)]
    path: Vec<String>,

    /// FEC overhead percentage
    #[arg(long, default_value = "0")]
    fec_overhead: u8,

    /// Statistics interval in seconds
    #[arg(long, default_value = "1")]
    stats: u64,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Input source types
enum InputSource {
    Stdin,
    File(String),
    Udp(SocketAddr),
}

fn parse_input(input: &str) -> anyhow::Result<InputSource> {
    if input == "-" {
        Ok(InputSource::Stdin)
    } else if input.starts_with("udp://") {
        let addr_str = &input[6..];
        let addr_str = if addr_str.starts_with(':') {
            format!("0.0.0.0{}", addr_str)
        } else {
            addr_str.to_string()
        };
        let addr: SocketAddr = addr_str.parse()?;
        Ok(InputSource::Udp(addr))
    } else {
        Ok(InputSource::File(input.to_string()))
    }
}

fn create_input_reader(source: InputSource) -> anyhow::Result<Box<dyn Read + Send>> {
    match source {
        InputSource::Stdin => {
            tracing::info!("Creating stdin reader");
            Ok(Box::new(io::stdin()))
        }
        InputSource::File(path) => {
            tracing::info!("Creating file reader for {}", path);
            Ok(Box::new(BufReader::new(File::open(path)?)))
        }
        InputSource::Udp(addr) => {
            tracing::info!("Creating UDP reader for {}", addr);
            let socket = SrtSocket::bind(addr)?;
            Ok(Box::new(UdpReader::new(socket)))
        }
    }
}

struct UdpReader {
    socket: SrtSocket,
    buffer: Vec<u8>,
    buffer_pos: usize,
    buffer_len: usize,
}

impl UdpReader {
    fn new(socket: SrtSocket) -> Self {
        UdpReader {
            socket,
            buffer: vec![0u8; 65536],
            buffer_pos: 0,
            buffer_len: 0,
        }
    }
}

impl Read for UdpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.buffer_pos < self.buffer_len {
            let available = self.buffer_len - self.buffer_pos;
            let to_copy = available.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + to_copy]);
            self.buffer_pos += to_copy;
            return Ok(to_copy);
        }
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((n, _addr)) => {
                    self.buffer_len = n;
                    self.buffer_pos = 0;
                    let to_copy = n.min(buf.len());
                    buf[..to_copy].copy_from_slice(&self.buffer[..to_copy]);
                    self.buffer_pos = to_copy;
                    return Ok(to_copy);
                }
                Err(e) => {
                    if let srt_io::SocketError::Io(ref io_err) = e {
                        if io_err.kind() == io::ErrorKind::WouldBlock {
                            thread::sleep(Duration::from_micros(100));
                            continue;
                        }
                    }
                    return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    tracing::info!("SRT Sender starting...");

    if args.path.is_empty() {
        anyhow::bail!("At least one output path is required");
    }

    let group_type = match args.group.as_str() {
        "broadcast" => GroupType::Broadcast,
        "backup" => GroupType::Backup,
        _ => GroupType::Broadcast,
    };

    let group = Arc::new(SocketGroup::new(1, group_type, args.path.len()));
    let mut sockets = Vec::new();

    for (idx, path_str) in args.path.iter().enumerate() {
        let remote_addr: SocketAddr = path_str.parse()?;
        let local_addr: SocketAddr = if remote_addr.ip().is_loopback() {
            "127.0.0.1:0".parse()?
        } else {
            "0.0.0.0:0".parse()?
        };

        let socket = SrtSocket::bind(local_addr)?;
        let actual_local = socket.local_addr()?;
        tracing::info!("Sender bound to {} for path {}", actual_local, remote_addr);
        let member_id = (idx + 1) as u32;
        
        let mut conn = Connection::new(member_id, actual_local, remote_addr, SeqNumber::new(0), 120);
        
        // Handshake
        tracing::info!("Initiating handshake with {}...", remote_addr);
        let handshake = conn.create_handshake();
        let hs_body = handshake.to_bytes();
        let hs_packet = srt_protocol::ControlPacket::new(
            srt_protocol::packet::ControlType::Handshake,
            0, 0, 0, member_id,
            bytes::Bytes::copy_from_slice(&hs_body),
        );
        let _ = socket.send_to(&hs_packet.to_bytes(), remote_addr);

        let mut hs_buf = vec![0u8; 2048];
        let mut handshake_done = false;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            match socket.recv_from(&mut hs_buf) {
                Ok((n, addr)) => {
                    tracing::info!("Received {} bytes in handshake loop from {}", n, addr);
                    if n >= 16 && (hs_buf[0] & 0x80) != 0 {
                        if let Ok(resp_hs) = SrtHandshake::from_bytes(&hs_buf[16..n]) {
                            match conn.process_handshake(resp_hs.clone()) {
                                Ok(()) => {
                                    tracing::info!("Handshake successful with {}, remote_socket_id={:?}",
                                        remote_addr, conn.remote_socket_id());
                                    handshake_done = true;
                                    break;
                                }
                                Err(e) => {
                                    tracing::error!("Handshake processing failed: {}", e);
                                }
                            }
                        } else {
                            tracing::debug!("Failed to parse SRT handshake from {}", addr);
                        }
                    } else {
                        tracing::debug!("Received non-control packet during handshake from {}", addr);
                    }
                }
                Err(_) => {}
            }
            thread::sleep(Duration::from_millis(50));
        }

        if !handshake_done {
            tracing::warn!("Handshake with {} timed out, continuing anyway...", remote_addr);
            conn = Connection::new_connected(member_id, actual_local, remote_addr, SeqNumber::new(0), 120);
        }

        let conn_arc = Arc::new(conn);
        let _ = group.add_member(conn_arc.clone(), remote_addr);
        let _ = group.update_member_status(member_id, MemberStatus::Active);
        sockets.push((socket, remote_addr, conn_arc));
    }

    let input_source = parse_input(&args.input)?;
    let mut reader = create_input_reader(input_source)?;

    let mut buffer = vec![0u8; 1316];
    let mut total_bytes = 0u64;
    let mut packet_count = 0u64;
    let mut seq_num = SeqNumber::new(0);
    let start_time = Instant::now();

    tracing::info!("Entering main send loop...");
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) => {
                tracing::info!("End of input reached");
                break;
            }
            Ok(n) => n,
            Err(e) => {
                tracing::error!("Read error: {}", e);
                thread::sleep(Duration::from_millis(10));
                continue;
            }
        };

        let data = Bytes::copy_from_slice(&buffer[..n]);
        for (socket, remote_addr, conn) in &sockets {
            let remote_id = conn.remote_socket_id().unwrap_or(0);
            if remote_id == 0 {
                tracing::warn!("Sending data packet with dest_socket_id=0 (handshake may have failed)");
            }
            let packet = DataPacket::new(seq_num, MsgNumber::new(seq_num.as_raw()), 0, remote_id, data.clone());
            if packet_count == 0 {
                tracing::info!("Sending first data packet: seq={}, dest_socket_id={}, size={}",
                    seq_num.as_raw(), remote_id, data.len());
            }
            let _ = socket.send_to(&packet.to_bytes(), *remote_addr);
        }

        total_bytes += n as u64;
        packet_count += 1;
        seq_num = seq_num.next();

        if packet_count % 100 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
            tracing::info!("Sent {} packets, {:.2} Mbps", packet_count, mbps);
            let _ = io::stderr().flush();
        }
    }

    Ok(())
}
