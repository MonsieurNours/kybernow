//! Journal de l'app : `kybernow.log` dans `Store::dir` (le précédent gardé en
//! `.1`), et la sortie d'erreur (visible par `ares-launch` et ssh). Tourné au
//! lancement et dès qu'il dépasse `MAX_BYTES` (WEBOS-19) : une app laissée des
//! jours ouverte ne remplit plus le stockage du téléviseur.

use std::io::Write;
use std::sync::Mutex;
use std::time::SystemTime;

/// Taille au-delà de laquelle le journal est tourné.
const MAX_BYTES: u64 = 5 * 1024 * 1024;

struct Sink {
    file: Option<std::fs::File>,
    written: u64,
    path: std::path::PathBuf,
}

impl Sink {
    fn open(path: std::path::PathBuf) -> Self {
        let _ = std::fs::rename(&path, path.with_extension("log.1"));
        Self { file: std::fs::File::create(&path).ok(), written: 0, path }
    }

    fn write(&mut self, line: &[u8]) {
        if self.written + line.len() as u64 > MAX_BYTES {
            *self = Sink::open(std::mem::take(&mut self.path));
        }
        if let Some(f) = self.file.as_mut()
            && f.write_all(line).is_ok()
        {
            self.written += line.len() as u64;
        }
    }
}

struct FileLog(Mutex<Sink>);

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
        self.0.lock().unwrap().write(line.as_bytes());
    }

    fn flush(&self) {}
}

pub fn init() {
    let path = crate::model::Store::dir().join("kybernow.log");
    let logger = Box::leak(Box::new(FileLog(Mutex::new(Sink::open(path)))));
    if log::set_logger(logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}
