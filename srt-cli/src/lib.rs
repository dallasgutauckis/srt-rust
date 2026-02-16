//! SRT CLI Library
//!
//! Shared functionality for SRT command-line tools.

pub mod config;
pub mod stats;

pub use config::{Config, SenderConfig, ReceiverConfig, BondingMode, PathConfig};
pub use stats::{display_group_stats, display_compact_stats, format_bytes, format_bandwidth};
