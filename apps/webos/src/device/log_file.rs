//! Journal de l'app : `kybernow.log` dans `Store::dir` (le précédent gardé en
//! `.1`), et la sortie d'erreur (visible par `ares-launch` et ssh).

use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

struct FileLog(Mutex<Option<std::fs::File>>);

impl log::Log for FileLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let line = format!("{t:.3} {} {}: {}\n", r.level(), r.target(), r.args());
        eprint!("{line}");
        if let Some(f) = self.0.lock().unwrap().as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    let dir = crate::model::Store::dir();
    let path = dir.join("kybernow.log");
    let _ = std::fs::rename(&path, dir.join("kybernow.log.1"));
    let file = std::fs::File::create(&path).ok();
    let logger = Box::leak(Box::new(FileLog(Mutex::new(file))));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
