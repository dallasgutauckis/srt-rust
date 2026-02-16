use std::net::UdpSocket;
use std::net::SocketAddr;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_addr: SocketAddr = "127.0.0.1:20002".parse()?;
    println!("Testing bind to 127.0.0.1:0");
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    println!("Bound to: {}", socket.local_addr()?);
    println!("Testing connect to {}", target_addr);
    socket.connect(target_addr)?;
    println!("Connected successfully");
    Ok(())
}
