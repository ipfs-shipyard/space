use chrono::{offset::Utc, DateTime};
use rand::Rng;
use std::{env, net::*, str, time::SystemTime};

mod err;

fn main() -> err::Result<()> {
    let mut arg_it = env::args();
    arg_it.next();
    let listen = arg_it.next().expect("First arg=<IP:port> to listen to.");
    let dest_a = arg_it
        .next()
        .expect("Second arg=<IP:port> to forward packets to.");
    let dest_b = arg_it
        .next()
        .expect("Third arg=<IP:port> to forward packets to.");
    let rate: usize = arg_it
        .next()
        .map(|s| str::parse(&s))
        .unwrap_or(Ok(usize::MAX))?;
    let mut buf = [0u8; u16::MAX as usize];
    let socket = UdpSocket::bind(listen.clone())?;
    let mut good = 0;
    let mut bad = 0;
    let mut rng = rand::thread_rng();
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, sender)) => {
                let to = if format!("{sender:?}") == dest_a {
                    dest_b.clone()
                } else {
                    dest_a.clone()
                };
                if bad >= 1 && good >= rate {
                    bad -= 1;
                    good -= rate;
                }
                let bad_odds = good / rate / 2 + 1;
                let good_odds = bad / 2 + rate;
                let n = bad_odds + good_odds;
                let i = rng.gen_range(0..n);
                if i < bad_odds {
                    let now: DateTime<Utc> = SystemTime::now().into();
                    println!(
                        "Dropping {}th packet (from {sender:?}). Excess: good={good} bad={bad} @ {now}", 
                        rate +1
                    );
                    bad += 1;
                } else {
                    match socket.send_to(&buf[0..len], to) {
                        Ok(_) => {
                            good += 1;
                            print!(".");
                        }
                        Err(e) => println!("Error sending: {e:?}"),
                    }
                }
            }
            Err(e) => println!("Error receiving: {e:?}"),
        }
    }
    // Ok(())
}
