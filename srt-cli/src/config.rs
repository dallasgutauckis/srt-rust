//! Configuration file support for SRT CLI tools

use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

/// Path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    /// Path name/label
    pub name: String,
    /// Remote address
    pub address: SocketAddr,
    /// Optional local bind address
    pub bind: Option<SocketAddr>,
    /// Weight for load balancing (0.0 to 1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,
}

fn default_weight() -> f64 {
    1.0
}

/// Bonding mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BondingMode {
    /// Broadcast to all paths
    Broadcast,
    /// Primary/backup with failover
    Backup,
    /// Load balance across paths
    Balancing,
}

/// Load balancing algorithm
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadBalancingAlgorithm {
    RoundRobin,
    WeightedRoundRobin,
    LeastLoaded,
    FastestPath,
    HighestBandwidth,
}

/// Sender configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderConfig {
    /// Input source (file path or "-" for stdin)
    pub input: String,
    /// Bonding mode
    pub mode: BondingMode,
    /// Paths to send on
    pub paths: Vec<PathConfig>,
    /// Maximum packet size
    #[serde(default = "default_mtu")]
    pub mtu: usize,
    /// Latency in milliseconds
    #[serde(default = "default_latency")]
    pub latency_ms: u16,
    /// Statistics interval in seconds
    #[serde(default = "default_stats_interval")]
    pub stats_interval_secs: u64,
    /// Load balancing algorithm (for balancing mode)
    pub balancing_algorithm: Option<LoadBalancingAlgorithm>,
}

fn default_mtu() -> usize {
    1456
}

fn default_latency() -> u16 {
    120
}

fn default_stats_interval() -> u64 {
    1
}

/// Receiver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiverConfig {
    /// Output destination (file path or "-" for stdout)
    pub output: String,
    /// Bonding mode
    pub mode: BondingMode,
    /// Listen addresses
    pub listen: Vec<SocketAddr>,
    /// Buffer size
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    /// Latency in milliseconds
    #[serde(default = "default_latency")]
    pub latency_ms: u16,
    /// Statistics interval in seconds
    #[serde(default = "default_stats_interval")]
    pub stats_interval_secs: u64,
}

fn default_buffer_size() -> usize {
    8192
}

/// Combined configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Sender configuration
    pub sender: Option<SenderConfig>,
    /// Receiver configuration
    pub receiver: Option<ReceiverConfig>,
}

impl Config {
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to TOML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let contents = toml::to_string_pretty(self)?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Create example sender configuration
    pub fn example_sender() -> Self {
        Config {
            sender: Some(SenderConfig {
                input: "-".to_string(),
                mode: BondingMode::Broadcast,
                paths: vec![
                    PathConfig {
                        name: "cellular1".to_string(),
                        address: "192.168.1.10:9000".parse().unwrap(),
                        bind: None,
                        weight: 1.0,
                    },
                    PathConfig {
                        name: "wifi1".to_string(),
                        address: "192.168.2.10:9000".parse().unwrap(),
                        bind: None,
                        weight: 1.0,
                    },
                ],
                mtu: 1456,
                latency_ms: 120,
                stats_interval_secs: 1,
                balancing_algorithm: None,
            }),
            receiver: None,
        }
    }

    /// Create example receiver configuration
    pub fn example_receiver() -> Self {
        Config {
            sender: None,
            receiver: Some(ReceiverConfig {
                output: "-".to_string(),
                mode: BondingMode::Broadcast,
                listen: vec![
                    "0.0.0.0:9000".parse().unwrap(),
                    "0.0.0.0:9001".parse().unwrap(),
                ],
                buffer_size: 8192,
                latency_ms: 120,
                stats_interval_secs: 1,
            }),
        }
    }
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

impl SenderConfig {
    /// Get statistics interval as Duration
    pub fn stats_interval(&self) -> Duration {
        Duration::from_secs(self.stats_interval_secs)
    }
}

impl ReceiverConfig {
    /// Get statistics interval as Duration
    pub fn stats_interval(&self) -> Duration {
        Duration::from_secs(self.stats_interval_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_configs() {
        let sender_config = Config::example_sender();
        assert!(sender_config.sender.is_some());

        let receiver_config = Config::example_receiver();
        assert!(receiver_config.receiver.is_some());
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::example_sender();
        let toml = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();

        assert!(parsed.sender.is_some());
    }
}
