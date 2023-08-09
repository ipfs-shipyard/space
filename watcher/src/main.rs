use notify::Watcher;
use std::{fs, path::PathBuf, time::Duration};

mod handler;

#[cfg(all(not(feature = "small"), not(feature = "big")))]
compile_error! {"Select either big or small feature"}

fn watched_dir(cfg: &config::Config) -> PathBuf {
    let mut result = PathBuf::new();
    result.push(
        cfg.clone()
            .watched_directory
            .clone()
            .expect("Must configure watched_directory before running watcher."),
    );
    result
}

fn main() {
    #[cfg(feature = "good_log")]
    env_logger::init();

    #[cfg(feature = "small_log")]
    smalog::init();

    let config_path = std::env::args().nth(1);
    let cfg = config::Config::parse(config_path).expect("Failed to parse config");
    let hndr = handler::Handler::new(&cfg).expect("Failed to configure transport & event handler");
    let dir = watched_dir(&cfg);
    let mut watcher = notify::recommended_watcher(move |e| hndr.handle_event(e))
        .expect("Unable to create directory watcher.");
    watcher
        .watch(&dir, notify::RecursiveMode::NonRecursive)
        .expect("Unable to watch directory.");
    let hndr =
        handler::Handler::new(&cfg).expect("Failed to configure second transport & event handler");
    let mut preexisting =
        fs::read_dir(&dir).expect("Can't list watched_directory - does it exist?");
    let mut t = 4;
    while dir.is_dir() {
        std::thread::sleep(Duration::from_secs(t));
        if let Some(Ok(f)) = preexisting.next() {
            if f.metadata().map(|d| d.is_file()).unwrap_or(false) {
                hndr.send(&f.path());
            }
        } else if let Ok(rd) = fs::read_dir(&dir) {
            preexisting = rd;
            t *= 2;
        }
    }
}
