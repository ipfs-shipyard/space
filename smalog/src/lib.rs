use log::{Level, Metadata, Record};

struct Smalog;

static LOGGER: Smalog = Smalog {};
pub fn init() {
    log::set_logger(&LOGGER).expect("Failed to set the logger implementation!");
}

impl log::Log for Smalog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
