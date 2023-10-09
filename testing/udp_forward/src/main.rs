use std::{env, net::*, str};
mod err;

fn main() -> err::Result<()> {
    let mut arg_it = env::args();
    let from_addr = arg_it.next().expect("First arg=<IP:port> to listen to.");
    let to_addr = arg_it
        .next()
        .expect("Second arg=<IP:port> to forward packets to.");
    let rate: usize = arg_it
        .next()
        .map(|s| str::parse(&s))
        .unwrap_or(Ok(usize::MAX))?;
    let mut buf = [0u8; u16::MAX as usize];
    let socket = UdpSocket::bind(from_addr)?;
    let mut streak = 0;
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, _sender)) => {
                if streak >= rate {
                    println!("Dropping {rate}th packet");
                    streak -= rate;
                } else {
                    match socket.send_to(&buf[0..len], to_addr.clone()) {
                        Ok(_) => streak += 1,
                        Err(e) => println!("Error sending: {e:?}"),
                    }
                }
            }
            Err(e) => println!("Error receiving: {e:?}"),
        }
    }
    // Ok(())
}
