use clap::Parser;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio_serial::SerialPortBuilderExt;

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

    let uplink_addr: SocketAddr = args
        .uplink_address
        .parse()
        .expect("Failed to parse uplink address");
    let downlink_addr: SocketAddr = args
        .downlink_address
        .parse()
        .expect("Failed to parse downlink address");

    let socket = UdpSocket::bind(&uplink_addr).await?;
    println!("UDP Uplink on:  {}", args.uplink_address);
    println!("UPD Downlink on: {}", args.downlink_address);
    println!("Serial radio on: {}", args.serial_device);

    let (serial_queue_writer, serial_queue_reader): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel();

    let mut serial_stream = tokio_serial::new(args.serial_device, 115200).open_native_async()?;
    serial_stream
        .set_exclusive(false)
        .expect("Failed to set serial to exclusive");

    let wrapped_serial = Arc::new(Mutex::new(serial_stream));

    let mut buf = vec![0; 1024];

    let thread_serial = Arc::clone(&wrapped_serial);

    thread::spawn(move || loop {
        if let Ok(data) = serial_queue_reader.recv() {
            println!("Found {} bytes to send over serial", data.len());
            let mut ser = thread_serial.lock().unwrap();
            ser.write(&data).unwrap();
        }
        thread::sleep(Duration::from_millis(250));
    });

    let main_serial = Arc::clone(&wrapped_serial);

    loop {
        if let Ok(len) = socket.try_recv(&mut buf) {
            if len > 0 {
                println!("Received {} bytes over udp, queueing for serial", len);
                serial_queue_writer
                    .send(buf[..len].to_vec())
                    .expect("Failed to send??");
            }
        }

        let len = {
            let mut ser = main_serial.lock().unwrap();
            ser.read(&mut buf)
        };
        if let Ok(serial_len) = len {
            if serial_len > 0 {
                println!(
                    "Received {} bytes over serial, sending over udp",
                    serial_len
                );
                socket.send_to(&buf[..serial_len], downlink_addr).await?;
            }
        }

        thread::sleep(Duration::from_millis(1));
    }
}
