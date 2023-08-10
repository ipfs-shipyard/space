use log::{Level, Metadata, Record};
use std::env;

struct Smalog {
    lev: log::LevelFilter,
}

static mut LOGGER: Smalog = Smalog {
    lev: log::LevelFilter::Info,
};
pub fn init() {
    let lev = match env::var("RUST_LOG") {
        Ok(lev_s) => level_from_str(&lev_s),
        Err(_) => Level::Info,
    };
    set_level(lev.to_level_filter());
}
pub fn set_level(lev: log::LevelFilter) {
    unsafe {
        LOGGER.lev = lev;
        log::set_logger(&LOGGER).expect("Failed to set the logger implementation!");
    }
    log::set_max_level(lev);
}

impl log::Log for Smalog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.lev
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

fn level_from_str(s: &str) -> Level {
    use std::str::FromStr;
    if let Ok(l) = Level::from_str(s) {
        return l;
    }
    println!("ERROR! RUST_LOG set to {s} which is not recognized by smalog which only accepts a simple level name, i.e. one of: OFF; ERROR; WARN; INFO; DEBUG; TRACE. Will use INFO instead.");
    Level::Info
}
