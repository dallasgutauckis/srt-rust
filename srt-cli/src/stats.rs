//! Statistics display and formatting

use srt_bonding::{GroupStats, MemberStats};
use std::time::Duration;

/// Format bytes in human-readable form
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format bandwidth in human-readable form
pub fn format_bandwidth(bps: u64) -> String {
    const KBPS: u64 = 1000;
    const MBPS: u64 = KBPS * 1000;
    const GBPS: u64 = MBPS * 1000;

    if bps >= GBPS {
        format!("{:.2} Gbps", bps as f64 / GBPS as f64)
    } else if bps >= MBPS {
        format!("{:.2} Mbps", bps as f64 / MBPS as f64)
    } else if bps >= KBPS {
        format!("{:.2} Kbps", bps as f64 / KBPS as f64)
    } else {
        format!("{} bps", bps)
    }
}

/// Format RTT in human-readable form
pub fn format_rtt(rtt_us: u32) -> String {
    if rtt_us >= 1_000_000 {
        format!("{:.2}s", rtt_us as f64 / 1_000_000.0)
    } else if rtt_us >= 1_000 {
        format!("{:.2}ms", rtt_us as f64 / 1_000.0)
    } else {
        format!("{}µs", rtt_us)
    }
}

/// Format duration in human-readable form
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {:02}m {:02}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {:02}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Display group statistics
pub fn display_group_stats(stats: &GroupStats) {
    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!("│ GROUP STATISTICS                                            │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!(
        "│ Mode: {:?}                                          ",
        stats.group_type
    );
    println!(
        "│ Members: {} active / {} total                               ",
        stats.active_member_count, stats.member_count
    );
    println!(
        "│ Uptime: {}                                              ",
        format_duration(stats.uptime)
    );
    println!("├─────────────────────────────────────────────────────────────┤");
    println!("│ AGGREGATE STATISTICS                                        │");
    println!("├─────────────────────────────────────────────────────────────┤");
    println!(
        "│ Packets:  {} sent / {} received                       ",
        stats.total_packets_sent, stats.total_packets_received
    );
    println!(
        "│ Bytes:    {} sent / {} received                 ",
        format_bytes(stats.total_bytes_sent),
        format_bytes(stats.total_bytes_received)
    );
    println!("└─────────────────────────────────────────────────────────────┘");

    if !stats.member_stats.is_empty() {
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│ PER-PATH STATISTICS                                         │");
        println!("├──────┬─────────┬──────────┬──────────┬──────────┬──────────┤");
        println!("│ Path │ Status  │ Sent     │ Received │ RTT      │ Bandwidth│");
        println!("├──────┼─────────┼──────────┼──────────┼──────────┼──────────┤");

        for member in &stats.member_stats {
            display_member_stats_row(member);
        }

        println!("└──────┴─────────┴──────────┴──────────┴──────────┴──────────┘");
    }
}

/// Display member statistics as a table row
fn display_member_stats_row(stats: &MemberStats) {
    let status = format!("{:?}", stats.status);
    let sent = format_bytes(stats.bytes_sent);
    let received = format_bytes(stats.bytes_received);
    let rtt = if stats.rtt_us > 0 {
        format_rtt(stats.rtt_us)
    } else {
        "N/A".to_string()
    };
    let bandwidth = if stats.bandwidth_bps > 0 {
        format_bandwidth(stats.bandwidth_bps)
    } else {
        "N/A".to_string()
    };

    println!(
        "│ {:4} │ {:7} │ {:8} │ {:8} │ {:8} │ {:8} │",
        stats.member_id, status, sent, received, rtt, bandwidth
    );
}

/// Display compact stats on one line (for continuous updates)
pub fn display_compact_stats(stats: &GroupStats, elapsed: Duration) {
    let throughput_bps = if elapsed.as_secs() > 0 {
        (stats.total_bytes_sent * 8) / elapsed.as_secs()
    } else {
        0
    };

    print!(
        "\r[{:8}] Active: {}/{} | Sent: {} | Rate: {} | Packets: {}         ",
        format_duration(stats.uptime),
        stats.active_member_count,
        stats.member_count,
        format_bytes(stats.total_bytes_sent),
        format_bandwidth(throughput_bps),
        stats.total_packets_sent
    );

    use std::io::Write;
    std::io::stdout().flush().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.00 MB");
    }

    #[test]
    fn test_format_bandwidth() {
        assert_eq!(format_bandwidth(500), "500 bps");
        assert_eq!(format_bandwidth(10_000), "10.00 Kbps");
        assert_eq!(format_bandwidth(10_000_000), "10.00 Mbps");
    }

    #[test]
    fn test_format_rtt() {
        assert_eq!(format_rtt(500), "500µs");
        assert_eq!(format_rtt(50_000), "50.00ms");
        assert_eq!(format_rtt(2_000_000), "2.00s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 01m 01s");
    }
}
