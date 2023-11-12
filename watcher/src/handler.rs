use log::{debug, error, info, trace};
use messages::{ApplicationAPI, Message};
use notify::{event::ModifyKind, Event, EventKind};
use std::path::Path;
use std::time::{Duration, SystemTime};
use transports::{Transport, UdpTransport};

pub(crate) struct Handler {
    trx: UdpTransport,
    target_addr: String,
}

impl Handler {
    pub fn new(cfg: &config::Config) -> Result<Self, anyhow::Error> {
        let trx = UdpTransport::new("0.0.0.0:0", cfg.mtu, cfg.chunk_transmit_throttle)?;
        let target_addr = cfg.listen_address.clone();
        Ok(Self { trx, target_addr })
    }

    pub fn handle_event(&self, event: notify::Result<Event>) {
        trace!("handle_event({:?})", &event);
        match event {
            Err(err) => {
                error!("FileSystem error: {:?}", err);
            }
            Ok(ev) => match ev.kind {
                EventKind::Modify(ModifyKind::Data(_)) => {
                    for p in ev.paths {
                        //Some of these events can occur while the file is still being modified
                        self.wait_for_modification_to_stop(&p).ok();
                        info!("File modified, import: {:?}", &p);
                        self.send(&p);
                    }
                }
                _ => debug!("Ignoring FileSystem event: {:?}", &ev),
            },
        }
    }
    pub fn send(&self, path: &Path) {
        let path = if let Some(p) = path.as_os_str().to_str() {
            p.to_owned()
        } else {
            error!("Path {:?} can't be turned into string?!", &path);
            return;
        };
        let m = ApplicationAPI::ImportFile { path };
        let m = Message::ApplicationAPI(m);
        if let Err(e) = self.trx.send(m, &self.target_addr) {
            error!("Error sending: {:?}", &e);
        }
    }
    fn wait_for_modification_to_stop(&self, p: &Path) -> std::io::Result<()> {
        const MIN_AGE: Duration = Duration::from_secs(1);
        const MAX_SLEEP: Duration = Duration::from_millis(1234);
        loop {
            let mdt = p.metadata()?.modified()?;
            let now = SystemTime::now();
            if mdt + MIN_AGE < now {
                return Ok(());
            }
            let elapsed = now.duration_since(mdt).unwrap_or_default();
            std::thread::sleep(MAX_SLEEP - elapsed);
        }
    }
}
