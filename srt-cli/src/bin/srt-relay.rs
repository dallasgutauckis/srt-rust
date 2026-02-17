//! SRT Relay - Multi-format stream relay/restreamer
//!
//! Receives stream in one format and outputs to multiple destinations in various formats.
//!
//! Examples:
//!   • Receive SRT → Output UDP to 3 destinations
//!   • Receive UDP → Output to file + UDP + stdout
//!   • Receive bonded SRT → Output single stream to multiple servers

use clap::Parser;
use srt_bonding::*;
use srt_io::SrtSocket;
use srt_protocol::DataPacket;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "srt-relay")]
#[command(about = "Multi-format stream relay/restreamer", long_about = None)]
struct Args {
    /// Input source: 'srt://0.0.0.0:port', 'udp://0.0.0.0:port', or file path
    ///
    /// Examples:
    ///   --input srt://:9000           (receive bonded SRT on port 9000)
    ///   --input udp://:5000           (receive UDP on port 5000)
    ///   --input video.ts              (read from file)
    ///   --input -                     (read from stdin)
    #[arg(short, long)]
    input: String,

    /// Output destinations: 'udp://host:port', 'file:path', or '-' for stdout
    /// Can be specified multiple times for multiple outputs
    ///
    /// Examples:
    ///   --output udp://192.168.1.10:5000
    ///   --output udp://192.168.1.11:5000
    ///   --output file:/tmp/recorded.ts
    ///   --output -
    #[arg(short, long)]
    output: Vec<String>,

    /// Number of expected input paths (for SRT input)
    #[arg(long, default_value = "1")]
    num_paths: usize,

    /// Statistics interval in seconds
    #[arg(long, default_value = "2")]
    stats: u64,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Input source type
enum InputSource {
    Srt(u16),     // SRT listen port
    Udp(u16),     // UDP listen port
    File(String), // File path
    Stdin,        // Stdin
}

/// Output destination type
enum OutputDest {
    Udp(SocketAddr), // UDP destination
    File(String),    // File path
    Stdout,          // Stdout
}

/// Parse input string
fn parse_input(input: &str) -> anyhow::Result<InputSource> {
    if input == "-" {
        Ok(InputSource::Stdin)
    } else if input.starts_with("srt://") {
        let addr_str = input.strip_prefix("srt://").unwrap();
        let addr_str = if addr_str.starts_with(':') {
            format!("0.0.0.0{}", addr_str)
        } else {
            addr_str.to_string()
        };
        let addr: SocketAddr = addr_str.parse()?;
        Ok(InputSource::Srt(addr.port()))
    } else if input.starts_with("udp://") {
        let addr_str = input.strip_prefix("udp://").unwrap();
        let addr_str = if addr_str.starts_with(':') {
            format!("0.0.0.0{}", addr_str)
        } else {
            addr_str.to_string()
        };
        let addr: SocketAddr = addr_str.parse()?;
        Ok(InputSource::Udp(addr.port()))
    } else {
        Ok(InputSource::File(input.to_string()))
    }
}

/// Parse output string
fn parse_output(output: &str) -> anyhow::Result<OutputDest> {
    if output == "-" {
        Ok(OutputDest::Stdout)
    } else if output.starts_with("udp://") {
        let addr_str = output.strip_prefix("udp://").unwrap();
        let addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid UDP address '{}': {}", addr_str, e))?;
        Ok(OutputDest::Udp(addr))
    } else if output.starts_with("file:") {
        let path = output.strip_prefix("file:").unwrap();
        Ok(OutputDest::File(path.to_string()))
    } else {
        // Default to file path
        Ok(OutputDest::File(output.to_string()))
    }
}

/// Output writer that can write to multiple destinations
struct MultiWriter {
    udp_outputs: Vec<(UdpSocket, SocketAddr)>,
    file_outputs: Vec<BufWriter<File>>,
    stdout_output: Option<io::Stdout>,
}

impl MultiWriter {
    fn new(outputs: Vec<OutputDest>) -> anyhow::Result<Self> {
        let mut udp_outputs = Vec::new();
        let mut file_outputs = Vec::new();
        let mut stdout_output = None;

        for output in outputs {
            match output {
                OutputDest::Udp(addr) => {
                    tracing::info!("Adding UDP output: {}", addr);
                    let socket = UdpSocket::bind("0.0.0.0:0")?;
                    udp_outputs.push((socket, addr));
                }
                OutputDest::File(path) => {
                    tracing::info!("Adding file output: {}", path);
                    let file = File::create(&path)?;
                    file_outputs.push(BufWriter::new(file));
                }
                OutputDest::Stdout => {
                    tracing::info!("Adding stdout output");
                    stdout_output = Some(io::stdout());
                }
            }
        }

        Ok(MultiWriter {
            udp_outputs,
            file_outputs,
            stdout_output,
        })
    }

    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        // Write to all UDP outputs
        for (socket, addr) in &self.udp_outputs {
            socket.send_to(data, addr)?;
        }

        // Write to all file outputs
        for file in &mut self.file_outputs {
            file.write_all(data)?;
        }

        // Write to stdout if enabled
        if let Some(ref mut stdout) = self.stdout_output {
            stdout.write_all(data)?;
        }

        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        for file in &mut self.file_outputs {
            file.flush()?;
        }
        if let Some(ref mut stdout) = self.stdout_output {
            stdout.flush()?;
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    tracing::info!("SRT Relay starting...");
    tracing::info!("Input: {}", args.input);
    tracing::info!("Outputs: {:?}", args.output);

    if args.output.is_empty() {
        anyhow::bail!("At least one output is required (use --output)");
    }

    // Parse input
    let input_source = parse_input(&args.input)?;

    // Parse outputs
    let output_dests: Vec<OutputDest> = args
        .output
        .iter()
        .map(|s| parse_output(s))
        .collect::<Result<_, _>>()?;

    // Create multi-writer
    let mut writer = MultiWriter::new(output_dests)?;

    // Handle input based on type
    match input_source {
        InputSource::Srt(port) => {
            tracing::info!("Receiving bonded SRT on port {}", port);
            relay_srt_input(port, args.num_paths, &mut writer, args.stats)?;
        }
        InputSource::Udp(port) => {
            tracing::info!("Receiving UDP on port {}", port);
            relay_udp_input(port, &mut writer, args.stats)?;
        }
        InputSource::File(path) => {
            tracing::info!("Reading from file: {}", path);
            relay_file_input(&path, &mut writer)?;
        }
        InputSource::Stdin => {
            tracing::info!("Reading from stdin");
            relay_stdin_input(&mut writer)?;
        }
    }

    Ok(())
}

/// Relay SRT input to outputs
fn relay_srt_input(
    port: u16,
    num_paths: usize,
    writer: &mut MultiWriter,
    stats_interval: u64,
) -> anyhow::Result<()> {
    // Create SRT receiver
    let listen_addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    let socket = SrtSocket::bind(listen_addr)?;
    tracing::info!("Listening on: {}", socket.local_addr()?);

    // Create socket group and bonding
    let group = Arc::new(SocketGroup::new(1, GroupType::Broadcast, num_paths));
    let bonding = Arc::new(BroadcastBonding::new(group.clone()));

    // Track remote addresses to member IDs
    let addr_to_member: HashMap<SocketAddr, u32> = HashMap::new();

    // Statistics thread
    let bonding_stats = bonding.clone();
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

    tracing::info!("Ready to receive and relay packets...");

    loop {
        // Receive packet
        let (n, remote_addr) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(e) => {
                if let srt_io::SocketError::Io(ref io_err) = e {
                    if io_err.kind() == io::ErrorKind::WouldBlock {
                        thread::sleep(Duration::from_micros(100));

                        // Try to pop ready packets
                        while let Some(packet) = bonding.receiver.pop_ready_packet() {
                            writer.write_all(&packet.payload)?;
                            total_bytes += packet.payload.len() as u64;
                        }

                        continue;
                    }
                }
                tracing::error!("Receive error: {}", e);
                continue;
            }
        };

        // Get member ID for this remote address - reject if not handshaked
        let member_id = match addr_to_member.get(&remote_addr) {
            Some(id) => *id,
            None => {
                tracing::warn!(
                    "Received data from {} without handshake, ignoring packet",
                    remote_addr
                );
                continue;
            }
        };

        // Deserialize and process packet
        let packet = match DataPacket::from_bytes(&buffer[..n]) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to parse packet: {}", e);
                continue;
            }
        };

        match bonding.receiver.on_packet_received(packet, member_id) {
            Ok(_) => {
                packet_count += 1;

                // Pop all ready packets and write to outputs
                while let Some(ready_packet) = bonding.receiver.pop_ready_packet() {
                    writer.write_all(&ready_packet.payload)?;
                    total_bytes += ready_packet.payload.len() as u64;
                }

                if packet_count % 100 == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                    tracing::debug!(
                        "Relayed {} packets, {:.2} MB, {:.2} Mbps",
                        packet_count,
                        total_bytes as f64 / 1_000_000.0,
                        mbps
                    );
                }
            }
            Err(e) => {
                tracing::trace!("Packet processing: {}", e);
            }
        }

        // Flush periodically
        if packet_count % 50 == 0 {
            writer.flush()?;
        }
    }
}

/// Relay UDP input to outputs
fn relay_udp_input(port: u16, writer: &mut MultiWriter, stats_interval: u64) -> anyhow::Result<()> {
    let listen_addr = format!("0.0.0.0:{}", port);
    let socket = UdpSocket::bind(&listen_addr)?;
    socket.set_nonblocking(true)?;
    tracing::info!("UDP listening on: {}", listen_addr);

    let mut buffer = vec![0u8; 65536];
    let mut total_bytes = 0u64;
    let mut packet_count = 0u64;
    let start_time = Instant::now();
    let mut last_stats = Instant::now();

    loop {
        match socket.recv(&mut buffer) {
            Ok(n) => {
                // Write to all outputs
                writer.write_all(&buffer[..n])?;

                total_bytes += n as u64;
                packet_count += 1;

                if packet_count % 50 == 0 {
                    writer.flush()?;
                }

                // Print stats
                if stats_interval > 0 && last_stats.elapsed() >= Duration::from_secs(stats_interval)
                {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                    tracing::info!(
                        "Relayed {} packets, {:.2} MB, {:.2} Mbps",
                        packet_count,
                        total_bytes as f64 / 1_000_000.0,
                        mbps
                    );
                    last_stats = Instant::now();
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_micros(100));
            }
            Err(e) => {
                tracing::error!("Receive error: {}", e);
                return Err(e.into());
            }
        }
    }
}

/// Relay file input to outputs
fn relay_file_input(path: &str, writer: &mut MultiWriter) -> anyhow::Result<()> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 8192];

    loop {
        match file.read(&mut buffer) {
            Ok(0) => {
                tracing::info!("End of file reached");
                break;
            }
            Ok(n) => {
                writer.write_all(&buffer[..n])?;
            }
            Err(e) => {
                tracing::error!("Read error: {}", e);
                return Err(e.into());
            }
        }
    }

    writer.flush()?;
    Ok(())
}

/// Relay stdin to outputs
fn relay_stdin_input(writer: &mut MultiWriter) -> anyhow::Result<()> {
    use std::io::Read;

    let mut stdin = io::stdin();
    let mut buffer = vec![0u8; 8192];

    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => {
                tracing::info!("End of input reached");
                break;
            }
            Ok(n) => {
                writer.write_all(&buffer[..n])?;
            }
            Err(e) => {
                tracing::error!("Read error: {}", e);
                return Err(e.into());
            }
        }
    }

    writer.flush()?;
    Ok(())
}
