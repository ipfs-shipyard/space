use log::{Metadata, Record};

struct Smalog {
    lev: log::LevelFilter,
}

static mut LOGGER: Smalog = Smalog {
    lev: log::LevelFilter::Info,
};
pub fn init() {
    set_level(log::LevelFilter::Info);
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
