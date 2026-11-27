//! Le journal de l'app (AND-01) : ce que le cœur, Kyber et le pont écrivent
//! (`log`) part dans logcat (étiquette `KyberNow`) et, en copie, dans un
//! fichier tournant de l'app (`files/journal/natif.log`, puis `natif.log.1`,
//! 1 Mo chacun au plus). Une panique Rust, où qu'elle arrive (fil de session,
//! fil de l'image, export JNI), y est écrite avec son fil, son emplacement et
//! sa pile, avant que le fil ou l'app ne s'arrête : sur Android, stderr ne va
//! nulle part. « Envoyer le journal » (Réglages › À propos) joint ces fichiers.
//!
//! Hors d'Android, seul le fichier tournant existe (essayé par `cargo test`).

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Taille d'un fichier du journal avant de passer au suivant.
pub const MAX_BYTES: u64 = 1024 * 1024;

/// Un fichier journal qui tourne : `nom`, puis `nom.1` (l'ancien, écrasé).
pub struct Rotating {
    path: PathBuf,
    max: u64,
    file: Option<File>,
    written: u64,
}

impl Rotating {
    pub fn open(path: impl Into<PathBuf>, max: u64) -> Self {
        let path = path.into();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let file = OpenOptions::new().create(true).append(true).open(&path).ok();
        let written = file.as_ref().and_then(|f| f.metadata().ok()).map(|m| m.len()).unwrap_or(0);
        Self { path, max, file, written }
    }

    /// Une ligne (terminée par `\n`), écrite tout de suite : rien n'attend
    /// dans un tampon si l'app s'arrête juste après.
    pub fn line(&mut self, text: &str) {
        if self.written >= self.max {
            self.rotate();
        }
        let Some(f) = self.file.as_mut() else { return };
        let mut buf = String::with_capacity(text.len() + 32);
        buf.push_str(&stamp());
        buf.push(' ');
        buf.push_str(text);
        if !buf.ends_with('\n') {
            buf.push('\n');
        }
        if f.write_all(buf.as_bytes()).is_ok() {
            self.written += buf.len() as u64;
        }
    }

    fn rotate(&mut self) {
        self.file = None;
        let _ = fs::rename(&self.path, old(&self.path));
        self.file = OpenOptions::new().create(true).append(true).open(&self.path).ok();
        self.written = 0;
    }
}

/// `natif.log` → `natif.log.1`.
pub fn old(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".1");
    PathBuf::from(s)
}

/// Heure UTC « 2026-11-27 14:03:07.123 », sans dépendance.
pub fn stamp() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format_utc(d.as_secs() as i64, d.subsec_millis())
}

pub fn format_utc(secs: i64, millis: u32) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Jours depuis 1970 → date civile (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}.{millis:03}", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Le texte d'une panique : fil, message et emplacement, puis la pile.
pub fn panic_text(thread: &str, info: &std::panic::PanicHookInfo, backtrace: &str) -> String {
    format!("panique dans le fil « {thread} » : {info}\n{backtrace}")
}

/// Le fichier du journal natif, ouvert par [`install`].
static FILE: Mutex<Option<Rotating>> = Mutex::new(None);

/// Écrit dans le fichier du journal (s'il est ouvert). Jamais de panique ici :
/// un Mutex empoisonné est repris.
pub fn write_file(text: &str) {
    let mut g = FILE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = g.as_mut() {
        r.line(text);
    }
}

/// Ouvre le fichier du journal dans `dir` et pose le crochet de panique.
/// Une seule fois par processus (les appels suivants ne changent rien).
pub fn install(dir: &Path) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Rotating::open(dir.join("natif.log"), MAX_BYTES));
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("?").to_string();
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let text = panic_text(&name, info, &bt);
            // Le fichier d'abord : logcat peut avoir tourné avant qu'on le lise.
            write_file(&text);
            log::error!(target: "panique", "{text}");
            previous(info);
        }));
    });
}

/// logcat (`KyberNow`) et, en copie, le fichier du journal.
#[cfg(target_os = "android")]
pub struct Tee {
    android: android_logger::AndroidLogger,
}

#[cfg(target_os = "android")]
impl log::Log for Tee {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        self.android.log(r);
        // La panique est déjà dans le fichier (écrite par le crochet).
        if r.target() != "panique" {
            write_file(&format!("{} {}: {}", r.level(), r.target(), r.args()));
        }
    }

    fn flush(&self) {}
}

/// Le journal du processus : logcat et le fichier, au niveau Info.
#[cfg(target_os = "android")]
pub fn init_logger() {
    let config = android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("KyberNow");
    if log::set_boxed_logger(Box::new(Tee { android: android_logger::AndroidLogger::new(config) })).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates() {
        assert_eq!(format_utc(0, 0), "1970-01-01 00:00:00.000");
        assert_eq!(format_utc(1_795_788_187, 42), "2026-11-27 14:03:07.042");
        assert_eq!(format_utc(951_782_400, 0), "2000-02-29 00:00:00.000");
    }

    #[test]
    fn rotates_at_the_limit() {
        let dir = std::env::temp_dir().join(format!("kn-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("natif.log");
        let mut r = Rotating::open(&path, 100);
        for i in 0..10 {
            r.line(&format!("ligne {i} ........................"));
        }
        let now = fs::read_to_string(&path).unwrap();
        let before = fs::read_to_string(old(&path)).unwrap();
        assert!(now.contains("ligne 9"));
        assert!(!now.contains("ligne 0 "));
        assert!(fs::metadata(&path).unwrap().len() <= 100 + 80);
        assert!(!before.is_empty());
        // Rouvert : il reprend à la suite, sans tout effacer.
        drop(r);
        let mut r = Rotating::open(&path, 10_000);
        r.line("reprise");
        let again = fs::read_to_string(&path).unwrap();
        assert!(again.contains("ligne 9") && again.ends_with("reprise\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn panic_is_written_to_the_file() {
        let dir = std::env::temp_dir().join(format!("kn-panique-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        install(&dir);
        let r = std::thread::Builder::new().name("kybernow-essai".into()).spawn(|| panic!("essai de panique")).unwrap().join();
        assert!(r.is_err());
        let text = fs::read_to_string(dir.join("natif.log")).unwrap();
        assert!(text.contains("panique dans le fil « kybernow-essai » : "), "{text}");
        assert!(text.contains("essai de panique"));
        assert!(text.contains("journal.rs"), "emplacement : {text}");
        let _ = fs::remove_dir_all(&dir);
    }
}
