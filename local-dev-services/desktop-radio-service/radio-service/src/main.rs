use tokio::net::{UdpSocket};
use std::net::SocketAddr;
use tokio_serial::SerialPortBuilderExt;
use std::io::{Read, Write};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Uplink Address (IP:Port)
    #[arg(short, long)]
    uplink_address: String,

    /// Downlink Address (IP:Port)
    #[arg(short, long)]
    downlink_address: String,

    /// Serial device
    #[arg(short, long)]
    serial_device: String,
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let args = Args::parse();

    let uplink_addr: SocketAddr = args.uplink_address.parse().expect("Failed to parse uplink address");
    let downlink_addr: SocketAddr = args.downlink_address.parse().expect("Failed to parse downlink address");
    
    let socket = UdpSocket::bind(&uplink_addr).await?;
    println!("UDP Uplink on:  {}", args.uplink_address);
    println!("UPD Downlink on: {}", args.downlink_address);
    println!("Serial radio on: {}", args.serial_device);


    let mut serial_stream = tokio_serial::new(args.serial_device, 115200).open_native_async()?;
    serial_stream.set_exclusive(false).expect("Failed to set serial to exclusive");

    let mut buf = vec![0; 1024];

    loop {
        if let Ok(len) = socket.try_recv(&mut buf) {
            if len > 0 {
                println!("Received {} bytes over udp, sending over serial", len);
                serial_stream.write(&buf[..len])?;
            }
        }
        
        if let Ok(serial_len) = serial_stream.read(&mut buf) {
            if serial_len > 0 {
                println!("Received {} bytes over serial, sending over udp", serial_len);
                socket.send_to(&buf[..serial_len], downlink_addr).await?;
            }
        }
    }
}
