//! SRT Receiver - Multi-path stream receiver
//!
//! Receives bonded SRT streams and writes to stdout or file.

use clap::Parser;
use srt_bonding::*;
use srt_io::SrtSocket;
use srt_protocol::{Connection, DataPacket, SeqNumber, SrtHandshake};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "srt-receiver")]
#[command(about = "SRT multi-path receiver", long_about = None)]
struct Args {
    /// Output file (use '-' for stdout, or 'udp://host:port')
    #[arg(short, long, default_value = "-")]
    output: String,

    /// Bonding mode (broadcast, backup, balancing)
    #[arg(short = 'g', long, default_value = "broadcast")]
    group: String,

    /// Listen port
    #[arg(short, long)]
    listen: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// Expected number of paths
    #[arg(long, default_value = "1")]
    num_paths: usize,

    /// Statistics interval in seconds
    #[arg(long, default_value = "1")]
    stats: u64,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt::init();

    tracing::info!("SRT Receiver starting...");
    tracing::info!("Output target: {}", args.output);

    // Parse group mode
    let group_type = match args.group.as_str() {
        "broadcast" => GroupType::Broadcast,
        "backup" => GroupType::Backup,
        "balancing" => GroupType::Broadcast,
        _ => anyhow::bail!("Invalid group mode: {}", args.group),
    };

    // Create socket
    let listen_addr: SocketAddr = format!("{}:{}", args.bind, args.listen).parse()?;
    let socket = SrtSocket::bind(listen_addr)?;
    tracing::info!("Listening on: {}", socket.local_addr()?);

    // Create socket group
    let group = Arc::new(SocketGroup::new(1, group_type, args.num_paths));

    // Create bonding
    let bonding = Arc::new(BroadcastBonding::new(group.clone()));

    // Track remote addresses to member IDs
    let mut addr_to_member: HashMap<SocketAddr, u32> = HashMap::new();
    let mut next_member_id = 1u32;

    // Open output
    let mut writer: Box<dyn Write> = if args.output == "-" {
        tracing::info!("Writing to stdout");
        Box::new(io::stdout())
    } else if args.output.starts_with("udp://") {
        let addr_str = &args.output[6..];
        let target_addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid UDP output address '{}': {}", addr_str, e))?;

        tracing::info!("Relaying to UDP: {}", target_addr);
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(target_addr)?;
        Box::new(UdpWriter::new(socket))
    } else {
        tracing::info!("Writing to file: {}", args.output);
        let file = File::create(&args.output)
            .map_err(|e| anyhow::anyhow!("Failed to create file '{}': {}", args.output, e))?;
        Box::new(BufWriter::new(file))
    };

    // Statistics thread
    let bonding_stats = bonding.clone();
    let stats_interval = args.stats;
    if stats_interval > 0 {
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(stats_interval));
            let stats = bonding_stats.stats();
            tracing::info!(
                "Stats: {} members, buffered={}, ready={}",
                stats.group_stats.member_count,
                stats.receiver_stats.buffered_packets,
                stats.receiver_stats.ready_packets
            );
        });
    }

    // Main receive loop
    let mut buffer = vec![0u8; 2048];
    let mut total_bytes = 0u64;
    let mut packet_count = 0u64;
    let start_time = Instant::now();

    tracing::info!("Ready to receive packets...");

    loop {
        // Receive packet
        let (n, remote_addr) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(e) => {
                if let srt_io::SocketError::Io(ref io_err) = e {
                    if io_err.kind() == io::ErrorKind::WouldBlock {
                        // No data available, sleep briefly
                        thread::sleep(Duration::from_millis(10));

                        // Try to pop ready packets from bonding
                        while let Some(packet) = bonding.receiver.pop_ready_packet() {
                            let _ = writer.write_all(&packet.payload);
                            total_bytes += packet.payload.len() as u64;
                        }

                        continue;
                    }
                }
                tracing::error!("Receive error: {}", e);
                continue;
            }
        };

        // Deserialize SRT packet
        if n >= 16 && (buffer[0] & 0x80) != 0 {
            tracing::info!("Received control packet ({} bytes) from {}", n, remote_addr);
            // Control packet - skip 16-byte header for handshake body
            if let Ok(hs) = SrtHandshake::from_bytes(&buffer[16..n]) {
                tracing::info!(
                    "Received handshake request from {}, sender_socket_id={}",
                    remote_addr,
                    hs.udt.socket_id
                );

                // Get or create member ID for this remote address
                let member_id = *addr_to_member.entry(remote_addr).or_insert_with(|| {
                    let id = next_member_id;
                    next_member_id += 1;
                    tracing::info!(
                        "New path detected (handshake): {} (member {})",
                        remote_addr,
                        id
                    );
                    id
                });

                // Store sender's socket_id for later use
                let _sender_socket_id = hs.udt.socket_id;

                let mut resp_hs = hs.clone();
                resp_hs.udt.handshake_type = -2; // Agreement
                resp_hs.udt.socket_id = 999;

                let hs_body = resp_hs.to_bytes();
                let resp_packet = srt_protocol::ControlPacket::new(
                    srt_protocol::packet::ControlType::Handshake,
                    0,
                    0,
                    0,
                    0,
                    bytes::Bytes::copy_from_slice(&hs_body),
                );

                let resp_bytes = resp_packet.to_bytes();
                match socket.send_to(&resp_bytes, remote_addr) {
                    Ok(n) => {
                        tracing::info!("Sent {} bytes of handshake agreement to {}", n, remote_addr)
                    }
                    Err(e) => tracing::error!(
                        "Failed to send handshake agreement to {}: {}",
                        remote_addr,
                        e
                    ),
                }

                // Ensure member is in group and active
                if group.get_member(member_id).is_none() {
                    let mut conn = Connection::new(
                        999, // Our socket ID
                        socket.local_addr().unwrap(),
                        remote_addr,
                        SeqNumber::new(0),
                        120,
                    );
                    // Set remote socket ID to sender's socket ID
                    let _ = conn.process_handshake(hs.clone());
                    tracing::info!(
                        "Created connection for member {}, remote_socket_id={:?}",
                        member_id,
                        conn.remote_socket_id()
                    );

                    let conn_arc = Arc::new(conn);
                    let _ = group.add_member(conn_arc, remote_addr);
                    let _ = group.update_member_status(member_id, MemberStatus::Active);
                }
                continue;
            }
        }

        // Get or create member ID for this remote address
        let member_id = *addr_to_member.entry(remote_addr).or_insert_with(|| {
            let id = next_member_id;
            next_member_id += 1;
            tracing::info!("New path detected (data): {} (member {})", remote_addr, id);
            let conn = Arc::new(Connection::new_connected(
                id,
                socket.local_addr().unwrap(),
                remote_addr,
                SeqNumber::new(0),
                120,
            ));
            let _ = group.add_member(conn, remote_addr);
            let _ = group.update_member_status(id, MemberStatus::Active);
            id
        });

        // Deserialize Data packet
        if let Ok(packet) = DataPacket::from_bytes(&buffer[..n]) {
            if packet_count == 0 {
                tracing::info!(
                    "Received first data packet: seq={}, dest_socket_id={}, size={}",
                    packet.seq_number().as_raw(),
                    packet.header.dest_socket_id,
                    packet.payload.len()
                );
            }
            match bonding.receiver.on_packet_received(packet, member_id) {
                Ok(_) => {}
                Err(e) => tracing::error!("Error processing data packet: {}", e),
            }
            packet_count += 1;

            while let Some(ready_packet) = bonding.receiver.pop_ready_packet() {
                let _ = writer.write_all(&ready_packet.payload);
                total_bytes += ready_packet.payload.len() as u64;
            }

            if packet_count % 100 == 0 {
                let elapsed = start_time.elapsed().as_secs_f64();
                let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                tracing::debug!("Received {} packets, {:.2} Mbps", packet_count, mbps);
            }
        }

        if packet_count % 50 == 0 {
            let _ = writer.flush();
        }
    }
}

struct UdpWriter {
    socket: std::net::UdpSocket,
}

impl UdpWriter {
    fn new(socket: std::net::UdpSocket) -> Self {
        Self { socket }
    }
}

impl Write for UdpWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.socket.send(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
