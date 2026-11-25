//! Le mode console d'un prêt sur une Station Linux (docs/pret.md, « Station
//! Linux ») : les mêmes fonctions que `system.rs` (Windows) et
//! `system_mac.rs`, avec GNOME ou KDE Plasma sous Wayland. L'agent
//! (`kybernow-console --console`, lancé par Admin dans une unité systemd de
//! l'utilisateur) tourne dans la session de l'hôte, avec son compte.
//!
//! - « Crochets » : l'entrée **physique** (claviers, souris, pavés tactiles
//!   et manettes branchés sur l'ordinateur) est prise en exclusivité
//!   (`EVIOCGRAB` sur `/dev/input/event*`, il faut la règle udev du paquet) :
//!   le compositeur ne la voit plus, l'agent n'y lit qu'Entrée et Échap pour
//!   le cache. L'entrée de l'ami arrive par les appareils uinput de Kyber
//!   (appareils virtuels), jamais pris ;
//! - raccourcis du bureau (« barres des tâches ») : coupés pendant le prêt,
//!   pour l'ami comme pour le clavier de l'ordinateur : sous GNOME, les
//!   raccourcis de Mutter, du Shell et de gnome-settings-daemon (touche Super,
//!   Alt+Tab, vue d'ensemble, terminal, fermeture de session…) mis à vide
//!   par `gsettings` ; sous KDE, `blockGlobalShortcuts` de KGlobalAccel et
//!   la touche Meta seule de KWin. Les valeurs d'avant sont gardées sur le
//!   disque (`shortcuts.json`) et remises à la fin, ou au départ suivant
//!   après une panne ;
//! - fenêtres de l'hôte : Wayland ne laisse pas un programme réduire celles
//!   des autres ; elles restent sur les écrans de l'ordinateur, sous les
//!   caches (l'écran de l'ami est l'écran virtuel, principal) ;
//! - premier plan : Wayland ne dit pas quelle fenêtre a le clavier ; les apps
//!   ouvertes hors des règles sont fermées par Admin (surveillance des
//!   processus), le lanceur revient par le bouton « Accueil » ;
//! - fenêtres « Ouvrir » : pas d'équivalent (limite connue, comme sur Mac).

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::{Value, json};

use super::store::Window;
use super::{CoverKey, Display};

/// Une fenêtre de l'agent : sous Linux, rien de plus qu'un numéro (Wayland
/// ne donne pas de poignée sur les fenêtres des autres).
pub type Hwnd = isize;

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

/// `~/.local/share/kybernow/lend` : l'état du prêt et la vue web.
pub fn dir() -> Option<PathBuf> {
    Some(xdg("XDG_DATA_HOME", ".local/share").join("kybernow/lend"))
}

/// Une ligne du journal de l'agent (`~/.local/state/kybernow/console.log`).
pub fn log(line: &str) {
    let dir = xdg("XDG_STATE_HOME", ".local/state").join("kybernow");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("console.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20) {
        let _ = std::fs::rename(&path, dir.join("console.old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {line}", unix_now());
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Démarrage de l'ordinateur (`btime` de `/proc/stat`), en secondes Unix.
pub fn boot_time() -> i64 {
    std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("btime ")).and_then(|v| v.trim().parse().ok()))
        .unwrap_or(0)
}

/// Heure et minute locales d'un instant (secondes Unix).
pub fn local_clock(unix: i64) -> Option<(u32, u32)> {
    let t: libc::time_t = unix;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return None;
    }
    Some((tm.tm_hour as u32, tm.tm_min as u32))
}

/// Un verrou de la session (fichier verrouillé par `flock`), attendu au plus `wait_ms`.
pub fn lock_named(name: &str, wait_ms: u32) -> Option<std::fs::File> {
    let dir = dir()?;
    let _ = std::fs::create_dir_all(&dir);
    let file_name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' }).collect();
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(format!("{file_name}.lock"))).ok()?;
    let deadline = std::time::Instant::now() + Duration::from_millis(wait_ms as u64);
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Some(file);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn release(file: std::fs::File) {
    drop(file);
}

fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Verrouille la session graphique de l'utilisateur (`loginctl lock-session`).
pub fn lock_workstation() {
    let uid = unsafe { libc::getuid() }.to_string();
    let session = std::env::var("XDG_SESSION_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| output("loginctl", &["show-user", &uid, "-p", "Display", "--value"]).filter(|s| !s.is_empty()));
    let done = match &session {
        Some(id) => run("loginctl", &["lock-session", id]),
        None => false,
    };
    if !done {
        log("verrouillage impossible : pas de session graphique");
    }
}

// --- L'entrée physique de l'ordinateur ----------------------------------------

/// `EVIOCGRAB` : `_IOW('E', 0x90, int)`.
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
const EV_KEY: u16 = 1;
const KEY_ESC: u16 = 1;
const KEY_ENTER: u16 = 28;
const KEY_KPENTER: u16 = 96;

/// `EVIOCGNAME(len)` : `_IOC(_IOC_READ, 'E', 0x06, len)`.
fn eviocgname(len: usize) -> libc::c_ulong {
    (2 << 30) | ((len as libc::c_ulong) << 16) | (0x45 << 8) | 0x06
}

/// Les appareils d'entrée branchés sur l'ordinateur : pas ceux de uinput
/// (Kyber, l'ami ; `devices/virtual`), pas les boutons et interrupteurs que
/// logind garde (alimentation, mise en veille, rabat de l'écran).
fn physical_devices() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir("/sys/class/input") else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
        .filter(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| !p.to_string_lossy().contains("/devices/virtual/")))
        .filter(|e| {
            let name = std::fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
            !["Power Button", "Sleep Button", "Lid Switch", "Video Bus"].iter().any(|n| name.contains(n))
        })
        .map(|e| Path::new("/dev/input").join(e.file_name()))
        .collect();
    out.sort();
    out
}

fn device_name(fd: &OwnedFd) -> String {
    let mut buf = [0u8; 128];
    let n = unsafe { libc::ioctl(fd.as_raw_fd(), eviocgname(buf.len()), buf.as_mut_ptr()) };
    if n <= 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buf[..(n as usize).min(buf.len())]).trim_end_matches('\0').to_string()
}

/// Ouvre et prend un appareil en exclusivité.
fn grab(path: &Path) -> std::io::Result<OwnedFd> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC).open(path)?;
    let fd: OwnedFd = file.into();
    if unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(fd)
}

/// Lit les évènements en attente : rend les touches du cache appuyées.
fn read_keys(fd: &OwnedFd) -> (Vec<CoverKey>, bool) {
    const SIZE: usize = std::mem::size_of::<libc::input_event>();
    let mut keys = Vec::new();
    let mut buf = [0u8; SIZE * 64];
    loop {
        let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            let gone = std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODEV);
            return (keys, gone);
        }
        if n == 0 {
            return (keys, true);
        }
        for chunk in buf[..n as usize].chunks_exact(SIZE) {
            let ev: libc::input_event = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
            if ev.type_ == EV_KEY && ev.value == 1 {
                match ev.code {
                    KEY_ENTER | KEY_KPENTER => keys.push(CoverKey::Enter),
                    KEY_ESC => keys.push(CoverKey::Escape),
                    _ => {}
                }
            }
        }
    }
}

pub struct Hooks {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Hooks {
    /// Prend l'entrée physique (relue toutes les 2 s : un clavier branché en
    /// cours de prêt est pris aussi) ; Entrée et Échap vont au cache. Sans
    /// droit sur `/dev/input` (règle udev absente), le prêt continue, l'entrée
    /// de l'ordinateur n'est pas bloquée, et le journal le dit.
    pub fn start(sink: impl Fn(CoverKey) + Send + Sync + 'static) -> Result<Hooks> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let join = std::thread::spawn(move || {
            let mut held: HashMap<PathBuf, OwnedFd> = HashMap::new();
            let mut refused: HashSet<PathBuf> = HashSet::new();
            let mut scan_at = std::time::Instant::now() - Duration::from_secs(10);
            while !stop2.load(Ordering::SeqCst) {
                if scan_at.elapsed() >= Duration::from_secs(2) {
                    scan_at = std::time::Instant::now();
                    for path in physical_devices() {
                        if held.contains_key(&path) || refused.contains(&path) {
                            continue;
                        }
                        match grab(&path) {
                            Ok(fd) => {
                                log(&format!("entrée de l'ordinateur prise : {} « {} »", path.display(), device_name(&fd)));
                                held.insert(path, fd);
                            }
                            Err(e) => {
                                log(&format!("entrée de l'ordinateur non bloquée : {} ({e})", path.display()));
                                refused.insert(path);
                            }
                        }
                    }
                }
                let mut fds: Vec<libc::pollfd> = held.values().map(|fd| libc::pollfd { fd: fd.as_raw_fd(), events: libc::POLLIN, revents: 0 }).collect();
                if fds.is_empty() {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
                let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 200) };
                if n <= 0 {
                    continue;
                }
                let ready: HashSet<i32> = fds.iter().filter(|p| p.revents != 0).map(|p| p.fd).collect();
                let mut gone = Vec::new();
                for (path, fd) in &held {
                    if !ready.contains(&fd.as_raw_fd()) {
                        continue;
                    }
                    let (keys, unplugged) = read_keys(fd);
                    for key in keys {
                        sink(key);
                    }
                    if unplugged {
                        gone.push(path.clone());
                    }
                }
                for path in gone {
                    held.remove(&path);
                }
            }
            // Fermer le descripteur rend l'appareil au compositeur.
            for (_, fd) in held.drain() {
                unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, 0 as libc::c_int) };
            }
        });
        Ok(Hooks { stop, join: Some(join) })
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

// --- Les raccourcis du bureau ---------------------------------------------------

/// GNOME : (schéma, clé) mis à vide pendant le prêt. `overlay-key` est une
/// chaîne, le reste des listes de raccourcis.
const GNOME_KEYS: &[(&str, &str)] = &[
    ("org.gnome.mutter", "overlay-key"),
    ("org.gnome.desktop.wm.keybindings", "switch-applications"),
    ("org.gnome.desktop.wm.keybindings", "switch-applications-backward"),
    ("org.gnome.desktop.wm.keybindings", "switch-windows"),
    ("org.gnome.desktop.wm.keybindings", "switch-windows-backward"),
    ("org.gnome.desktop.wm.keybindings", "switch-group"),
    ("org.gnome.desktop.wm.keybindings", "switch-group-backward"),
    ("org.gnome.desktop.wm.keybindings", "cycle-windows"),
    ("org.gnome.desktop.wm.keybindings", "panel-main-menu"),
    ("org.gnome.desktop.wm.keybindings", "panel-run-dialog"),
    ("org.gnome.desktop.wm.keybindings", "show-desktop"),
    ("org.gnome.desktop.wm.keybindings", "minimize"),
    ("org.gnome.desktop.wm.keybindings", "move-to-monitor-left"),
    ("org.gnome.desktop.wm.keybindings", "move-to-monitor-right"),
    ("org.gnome.desktop.wm.keybindings", "move-to-monitor-up"),
    ("org.gnome.desktop.wm.keybindings", "move-to-monitor-down"),
    ("org.gnome.shell.keybindings", "toggle-overview"),
    ("org.gnome.shell.keybindings", "toggle-application-view"),
    ("org.gnome.shell.keybindings", "toggle-message-tray"),
    ("org.gnome.shell.keybindings", "toggle-quick-settings"),
    ("org.gnome.shell.keybindings", "focus-active-notification"),
    ("org.gnome.shell.keybindings", "open-application-menu"),
    ("org.gnome.settings-daemon.plugins.media-keys", "logout"),
    ("org.gnome.settings-daemon.plugins.media-keys", "screensaver"),
    ("org.gnome.settings-daemon.plugins.media-keys", "terminal"),
    ("org.gnome.settings-daemon.plugins.media-keys", "home"),
    ("org.gnome.settings-daemon.plugins.media-keys", "www"),
    ("org.gnome.settings-daemon.plugins.media-keys", "email"),
    ("org.gnome.settings-daemon.plugins.media-keys", "search"),
    ("org.gnome.settings-daemon.plugins.media-keys", "control-center"),
    ("org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"),
];

fn shortcuts_file() -> Option<PathBuf> {
    dir().map(|d| d.join("shortcuts.json"))
}

fn gnome() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_ascii_uppercase().contains("GNOME"))
}

fn kde() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_ascii_uppercase().contains("KDE"))
}

/// La valeur vide d'une clé : `''` pour une chaîne, `@as []` pour une liste.
fn empty_value(current: &str) -> &'static str {
    if current.starts_with('[') || current.starts_with("@as") { "@as []" } else { "''" }
}

fn kde_config(tool: &str) -> String {
    for v in ["6", "5"] {
        let name = format!("{tool}{v}");
        if output("sh", &["-c", &format!("command -v {name}")]).is_some() {
            return name;
        }
    }
    format!("{tool}6")
}

/// Coupe les raccourcis du bureau ; les valeurs d'avant vont sur le disque
/// d'abord (une seule fois : une reprise ne les écrase pas).
fn block_shortcuts() {
    let Some(file) = shortcuts_file() else { return };
    if file.exists() {
        return;
    }
    let mut saved = json!({ "gnome": {}, "kde": null });
    if gnome() {
        for (schema, key) in GNOME_KEYS {
            if let Some(v) = output("gsettings", &["get", schema, key]) {
                saved["gnome"][format!("{schema} {key}")] = json!(v);
            }
        }
    }
    if kde() {
        let read = kde_config("kreadconfig");
        let meta = output(&read, &["--file", "kwinrc", "--group", "ModifierOnlyShortcuts", "--key", "Meta"]);
        saved["kde"] = json!({ "meta": meta });
    }
    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new("/")));
    if std::fs::write(&file, saved.to_string()).is_err() {
        log("raccourcis : état non écrit, laissés tels quels");
        return;
    }
    if let Some(keys) = saved["gnome"].as_object() {
        for (k, v) in keys {
            let (schema, key) = k.split_once(' ').unwrap_or((k, ""));
            run("gsettings", &["set", schema, key, empty_value(v.as_str().unwrap_or(""))]);
        }
    }
    if !saved["kde"].is_null() {
        let write = kde_config("kwriteconfig");
        run(&write, &["--file", "kwinrc", "--group", "ModifierOnlyShortcuts", "--key", "Meta", ""]);
        run("busctl", &["--user", "call", "org.kde.KWin", "/KWin", "org.kde.KWin", "reconfigure"]);
        run("busctl", &["--user", "call", "org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel", "blockGlobalShortcuts", "b", "true"]);
    }
    log("raccourcis du bureau coupés");
}

/// Remet les raccourcis notés par [`block_shortcuts`], s'il y en a.
fn restore_shortcuts() {
    let Some(file) = shortcuts_file() else { return };
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let saved: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if let Some(keys) = saved["gnome"].as_object() {
        for (k, v) in keys {
            let (schema, key) = k.split_once(' ').unwrap_or((k, ""));
            if let Some(v) = v.as_str() {
                run("gsettings", &["set", schema, key, v]);
            }
        }
    }
    if !saved["kde"].is_null() {
        run("busctl", &["--user", "call", "org.kde.kglobalaccel", "/kglobalaccel", "org.kde.KGlobalAccel", "blockGlobalShortcuts", "b", "false"]);
        let write = kde_config("kwriteconfig");
        match saved["kde"]["meta"].as_str() {
            Some(meta) => run(&write, &["--file", "kwinrc", "--group", "ModifierOnlyShortcuts", "--key", "Meta", meta]),
            None => run(&write, &["--file", "kwinrc", "--group", "ModifierOnlyShortcuts", "--key", "Meta", "--delete"]),
        };
        run("busctl", &["--user", "call", "org.kde.KWin", "/KWin", "org.kde.KWin", "reconfigure"]);
    }
    let _ = std::fs::remove_file(&file);
    log("raccourcis du bureau remis");
}

/// Le pendant des barres des tâches de Windows : les raccourcis du bureau,
/// coupés pendant le prêt et remis à la fin (ou à la remise en état).
pub fn taskbars(show: bool) {
    if show { restore_shortcuts() } else { block_shortcuts() }
}

// --- Fenêtres, pointeur, premier plan -------------------------------------------

/// Wayland ne laisse pas réduire les fenêtres des autres : rien à noter.
pub fn minimize_all() -> Vec<Window> {
    Vec::new()
}

pub fn minimize_again(_saved: &[Window]) {}

pub fn restore(_saved: &[Window]) {}

/// Le pointeur de l'ami est absolu (Kyber) : rien à retenir.
pub fn clip(_display: Option<&Display>) {}

pub fn keep_cursor(_d: &Display) {}

pub fn center_cursor(_d: &Display) {}

pub fn file_dialogs() -> HashSet<isize> {
    HashSet::new()
}

pub fn close_file_dialogs(_known: &HashSet<isize>) {}

/// Les processus nés pendant le prêt (heure de départ dans `/proc`).
pub struct Born {
    started: i64,
}

impl Born {
    pub fn new(started_unix: i64) -> Born {
        Born { started: started_unix }
    }

    pub fn is_new(&mut self, pid: u32) -> bool {
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1);
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|s| s.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(19).and_then(|v| v.parse::<i64>().ok())))
            .is_some_and(|start| boot_time() + start / ticks >= self.started - 1)
    }
}

/// Les fenêtres des autres ne sont pas visibles sous Wayland : les apps du
/// prêt qui restent, Admin les ferme (surveillance des processus).
pub fn loan_windows(_born: &mut Born) -> Vec<Hwnd> {
    Vec::new()
}

pub fn keep_on(_display: &Display, _born: &mut Born) {}

pub fn close_loan_windows(_born: &mut Born) -> usize {
    0
}

pub fn processes(name: &str) -> Vec<u32> {
    let uid = unsafe { libc::getuid() }.to_string();
    output("pgrep", &["-x", "-u", &uid, name]).map(|o| o.lines().filter_map(|l| l.trim().parse().ok()).collect()).unwrap_or_default()
}

pub fn kill(pid: u32) {
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
}

/// Wayland ne dit pas quelle fenêtre a le clavier.
pub fn foreground() -> Hwnd {
    0
}

pub fn acceptable_foreground(_fg: Hwnd, _display: &Display) -> bool {
    true
}

/// Le lanceur revient devant par `tao` (agent) ; rien de plus ici.
pub fn bring_to_front(_hwnd: Hwnd) {}

pub fn allow_foreground() {}

pub fn topmost(_hwnd: Hwnd) {}

pub fn read_pads() -> u16 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_valeur_vide_d_un_raccourci() {
        assert_eq!(empty_value("['<Super>a']"), "@as []");
        assert_eq!(empty_value("@as []"), "@as []");
        assert_eq!(empty_value("'Super_L'"), "''");
    }

    #[test]
    fn le_numero_de_eviocgname() {
        // EVIOCGNAME(256) de linux/input.h : 0x81004506.
        assert_eq!(eviocgname(256), 0x8100_4506);
    }
}
