//! SRT I/O and Platform Abstraction
//!
//! This crate provides network I/O and platform-specific abstractions,
//! including UDP socket wrappers, event loops, and timing utilities.

pub mod socket;
pub mod time;

// Future modules
// pub mod epoll;

pub use socket::{SrtSocket, SocketError};
pub use time::{RateLimiter, Timer, Timestamp};
