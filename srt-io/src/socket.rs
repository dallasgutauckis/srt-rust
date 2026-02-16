//! UDP socket wrapper for SRT
//!
//! Provides cross-platform UDP socket abstraction with SRT-specific options.

use socket2::{Domain, Protocol, Socket, Type};
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, UdpSocket};
use thiserror::Error;

/// Socket configuration errors
#[derive(Error, Debug)]
pub enum SocketError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid socket address")]
    InvalidAddress,

    #[error("Socket option not supported on this platform")]
    UnsupportedOption,
}

/// SRT socket wrapper
///
/// Wraps a UDP socket with SRT-specific configuration.
pub struct SrtSocket {
    inner: Socket,
}

impl SrtSocket {
    /// Create a new SRT socket bound to the given address
    pub fn bind(addr: SocketAddr) -> Result<Self, SocketError> {
        let domain = if addr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };

        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

        // Set socket options
        socket.set_reuse_address(true)?;
        // #[cfg(unix)]
        // socket.set_reuse_port(true)?;

        // Bind the socket
        socket.bind(&addr.into())?;

        // Set non-blocking mode
        socket.set_nonblocking(true)?;

        Ok(SrtSocket { inner: socket })
    }

    /// Create a new unbound SRT socket
    pub fn new(ipv6: bool) -> Result<Self, SocketError> {
        let domain = if ipv6 { Domain::IPV6 } else { Domain::IPV4 };
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

        socket.set_nonblocking(true)?;

        Ok(SrtSocket { inner: socket })
    }

    /// Set the send buffer size
    pub fn set_send_buffer_size(&self, size: usize) -> Result<(), SocketError> {
        self.inner.set_send_buffer_size(size)?;
        Ok(())
    }

    /// Set the receive buffer size
    pub fn set_recv_buffer_size(&self, size: usize) -> Result<(), SocketError> {
        self.inner.set_recv_buffer_size(size)?;
        Ok(())
    }

    /// Get the send buffer size
    pub fn send_buffer_size(&self) -> Result<usize, SocketError> {
        Ok(self.inner.send_buffer_size()?)
    }

    /// Get the receive buffer size
    pub fn recv_buffer_size(&self) -> Result<usize, SocketError> {
        Ok(self.inner.recv_buffer_size()?)
    }

    /// Get the local address this socket is bound to
    pub fn local_addr(&self) -> Result<SocketAddr, SocketError> {
        self.inner
            .local_addr()?
            .as_socket()
            .ok_or(SocketError::InvalidAddress)
    }

    /// Send data to the given address
    ///
    /// Returns the number of bytes sent, or WouldBlock if the socket is not ready.
    pub fn send_to(&self, buf: &[u8], target: SocketAddr) -> Result<usize, SocketError> {
        match self.inner.send_to(buf, &target.into()) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Err(SocketError::Io(e)),
            Err(e) => Err(SocketError::Io(e)),
        }
    }

    /// Receive data from the socket
    ///
    /// Returns the number of bytes received and the source address,
    /// or WouldBlock if the socket is not ready.
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), SocketError> {
        // socket2 recv_from needs MaybeUninit, but we can use recv_from directly on the UdpSocket
        // For now, use unsafe to transmute the buffer
        use std::mem::MaybeUninit;
        let uninit_buf = unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut MaybeUninit<u8>, buf.len())
        };

        match self.inner.recv_from(uninit_buf) {
            Ok((n, addr)) => Ok((n, addr.as_socket().ok_or(SocketError::InvalidAddress)?)),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Err(SocketError::Io(e)),
            Err(e) => Err(SocketError::Io(e)),
        }
    }

    /// Try to clone the socket
    pub fn try_clone(&self) -> Result<Self, SocketError> {
        Ok(SrtSocket {
            inner: self.inner.try_clone()?,
        })
    }

    /// Get a reference to the underlying socket
    pub fn as_socket(&self) -> &Socket {
        &self.inner
    }

    /// Convert to a standard UDP socket
    pub fn into_udp_socket(self) -> UdpSocket {
        self.inner.into()
    }
}

/// Socket poll result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollEvent {
    /// Socket is readable
    Readable,
    /// Socket is writable
    Writable,
    /// Socket has both read and write ready
    ReadWrite,
    /// No events ready
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_creation() {
        let socket = SrtSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = socket.local_addr().unwrap();
        assert!(addr.port() > 0);
    }

    #[test]
    fn test_socket_buffer_sizes() {
        let socket = SrtSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();

        // Set buffer sizes
        socket.set_send_buffer_size(262144).unwrap();
        socket.set_recv_buffer_size(262144).unwrap();

        // Get buffer sizes (may not match exactly due to OS limits)
        let send_size = socket.send_buffer_size().unwrap();
        let recv_size = socket.recv_buffer_size().unwrap();

        assert!(send_size > 0);
        assert!(recv_size > 0);
    }

    #[test]
    fn test_socket_send_recv() {
        let sender = SrtSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let receiver = SrtSocket::bind("127.0.0.1:0".parse().unwrap()).unwrap();

        let receiver_addr = receiver.local_addr().unwrap();

        // Send data
        let data = b"Hello, SRT!";
        sender.send_to(data, receiver_addr).unwrap();

        // Receive data (may need to retry due to non-blocking)
        let mut buf = [0u8; 1024];
        for _ in 0..10 {
            match receiver.recv_from(&mut buf) {
                Ok((n, _addr)) => {
                    assert_eq!(&buf[..n], data);
                    return;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        panic!("Failed to receive data");
    }

    #[test]
    fn test_socket_ipv6() {
        // May fail on systems without IPv6
        if let Ok(socket) = SrtSocket::bind("[::1]:0".parse().unwrap()) {
            let addr = socket.local_addr().unwrap();
            assert!(addr.is_ipv6());
        }
    }
}
