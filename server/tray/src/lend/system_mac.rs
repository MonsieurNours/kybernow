//! Le mode console d'un prêt sur une Station Mac (docs/pret.md) : les mêmes
//! fonctions que `system.rs` (Windows), avec macOS. L'agent
//! (`kybernow-console --console`, dans l'app KyberNow Station) tourne dans la
//! session de l'utilisateur, avec son compte et l'autorisation Accessibilité
//! de l'app.
//!
//! - « Fenêtres réduites » : les apps de l'utilisateur sont masquées
//!   (`NSRunningApplication.hide`), rendues à la fin ;
//! - « crochets » : un `CGEventTap` de la session avale l'entrée physique du
//!   Mac (sauf Entrée et Échap pour le cache) et laisse passer celle de l'ami
//!   (postée par kynputserver : un processus source), sauf ⌘⇥, ⌘ Espace,
//!   ⌘⌥Échap, ⌃ flèches et les touches de Mission Control et du Launchpad ;
//!   le pointeur de l'ami reste sur son écran (l'écran principal) ;
//! - barre des tâches : rien (le lanceur en plein écran cache le Dock et la
//!   barre des menus ; les caches couvrent les écrans du Mac) ;
//! - fenêtres « Ouvrir » : pas d'équivalent fiable, laissées (limite connue).

use std::collections::HashSet;
use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use objc2_app_kit::{NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication, NSWindow, NSWorkspace};
use objc2_foundation::MainThreadMarker;

use super::store::Window;
use super::{CoverKey, Display};

/// Une fenêtre de l'agent : son `NSWindow` ; pour une autre app au premier
/// plan, l'opposé de son numéro de processus (`foreground`).
pub type Hwnd = isize;

/// Identifiant de l'app KyberNow Station (jamais masquée ni fermée).
const OWN_BUNDLE: &str = "fr.leslignesclaires.kybernow.station";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `~/Library/Application Support/KyberNow/lend` : l'état du prêt et la vue web.
pub fn dir() -> Option<PathBuf> {
    Some(home().join("Library/Application Support/KyberNow/lend"))
}

/// Une ligne du journal de l'agent (`~/Library/Logs/KyberNow/console.log`).
pub fn log(line: &str) {
    let dir = home().join("Library/Logs/KyberNow");
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

/// Démarrage du Mac (`kern.boottime`), en secondes Unix.
pub fn boot_time() -> i64 {
    let mut tv = libc::timeval { tv_sec: 0, tv_usec: 0 };
    let mut size = std::mem::size_of::<libc::timeval>();
    let mut mib = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    let ok = unsafe { libc::sysctl(mib.as_mut_ptr(), 2, (&mut tv as *mut libc::timeval).cast(), &mut size, std::ptr::null_mut(), 0) } == 0;
    if ok { tv.tv_sec } else { 0 }
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
    use std::os::fd::AsRawFd;
    let dir = dir()?;
    let _ = std::fs::create_dir_all(&dir);
    let file_name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' }).collect();
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(format!("{file_name}.lock"))).ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(wait_ms as u64);
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Some(file);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn release(file: std::fs::File) {
    drop(file);
}

/// Verrouille la session : `SACLockScreenImmediate` (Pomme › Verrouiller l'écran).
pub fn lock_workstation() {
    unsafe {
        let handle = libc::dlopen(c"/System/Library/PrivateFrameworks/login.framework/Versions/Current/login".as_ptr(), libc::RTLD_NOW);
        if handle.is_null() {
            log("verrouillage impossible : login.framework");
            return;
        }
        let symbol = libc::dlsym(handle, c"SACLockScreenImmediate".as_ptr());
        if symbol.is_null() {
            log("verrouillage impossible : SACLockScreenImmediate");
            return;
        }
        let lock: extern "C" fn() -> i32 = std::mem::transmute(symbol);
        lock();
    }
}

// --- Le filtre du clavier et de la souris (CGEventTap) -------------------------

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type TapCallback = extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, mask: u64, callback: TapCallback, user: *mut c_void) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventSetLocation(event: *mut c_void, point: CGPoint);
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: *mut c_void, order: isize) -> *mut c_void;
    fn CFMachPortInvalidate(port: *mut c_void);
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: *mut c_void);
    fn CFRelease(p: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

const SESSION_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER: u32 = 0xFFFF_FFFF;
/// `kCGKeyboardEventKeycode`, `kCGEventSourceUnixProcessID`.
const FIELD_KEYCODE: u32 = 9;
const FIELD_SOURCE_PID: u32 = 41;
const FLAG_CONTROL: u64 = 1 << 18;
const FLAG_OPTION: u64 = 1 << 19;
const FLAG_COMMAND: u64 = 1 << 20;
const RETURN: i64 = 36;
const KEYPAD_ENTER: i64 = 76;
const ESCAPE: i64 = 53;
const TAB: i64 = 48;
const SPACE: i64 = 49;
const GRAVE: i64 = 50;
const ARROWS: [i64; 4] = [123, 124, 125, 126];
/// Mission Control (F3), Launchpad (F4), Dictée/Spotlight et le Centre de notifications.
const SYSTEM_KEYS: [i64; 4] = [160, 131, 176, 177];

static KEY_SINK: OnceLock<Box<dyn Fn(CoverKey) + Send + Sync>> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ENTER_DOWN: AtomicBool = AtomicBool::new(false);
static ESCAPE_DOWN: AtomicBool = AtomicBool::new(false);
/// L'écran où reste le pointeur de l'ami (points de macOS), une fois le lanceur placé.
static CLIP: Mutex<Option<CGRect>> = Mutex::new(None);

/// L'entrée de l'ami (injectée par kynputserver) laisse-t-elle passer cette touche ?
fn injected_key_allowed(keycode: i64, flags: u64) -> bool {
    let command = flags & FLAG_COMMAND != 0;
    if SYSTEM_KEYS.contains(&keycode) {
        return false;
    }
    match keycode {
        // ⌘⇥, ⌘` (changer d'app ou de fenêtre), ⌘ Espace (Spotlight).
        TAB | GRAVE | SPACE if command => false,
        // ⌘⌥Échap (Forcer à quitter).
        ESCAPE if command && flags & FLAG_OPTION != 0 => false,
        // ⌃ flèches : Mission Control et les Spaces.
        k if ARROWS.contains(&k) && flags & FLAG_CONTROL != 0 => false,
        _ => true,
    }
}

extern "C" fn tap_callback(_proxy: *mut c_void, kind: u32, event: *mut c_void, _user: *mut c_void) -> *mut c_void {
    if kind == TAP_DISABLED_BY_TIMEOUT || kind == TAP_DISABLED_BY_USER {
        let tap = TAP.load(Ordering::SeqCst);
        if !tap.is_null() {
            unsafe { CGEventTapEnable(tap, true) };
        }
        return event;
    }
    let source = unsafe { CGEventGetIntegerValueField(event, FIELD_SOURCE_PID) };
    let injected = source != 0 && source != std::process::id() as i64;
    match kind {
        KEY_DOWN | KEY_UP | FLAGS_CHANGED => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) };
            if !injected {
                // Entrée et Échap du clavier du Mac vont au cache, une fois par appui.
                let key = match keycode {
                    RETURN | KEYPAD_ENTER => Some(CoverKey::Enter),
                    ESCAPE => Some(CoverKey::Escape),
                    _ => None,
                };
                if let (Some(key), true) = (key, kind != FLAGS_CHANGED) {
                    let flag = if key == CoverKey::Enter { &ENTER_DOWN } else { &ESCAPE_DOWN };
                    if kind == KEY_DOWN && !flag.swap(true, Ordering::SeqCst) {
                        if let Some(sink) = KEY_SINK.get() {
                            sink(key);
                        }
                    } else if kind == KEY_UP {
                        flag.store(false, Ordering::SeqCst);
                    }
                }
                return std::ptr::null_mut();
            }
            if kind == KEY_DOWN && !injected_key_allowed(keycode, unsafe { CGEventGetFlags(event) }) {
                return std::ptr::null_mut();
            }
            event
        }
        _ => {
            // La souris et le trackpad du Mac ne font rien ; ceux de l'ami restent sur son écran.
            if !injected {
                return std::ptr::null_mut();
            }
            if let Some(r) = *CLIP.lock().unwrap() {
                let p = unsafe { CGEventGetLocation(event) };
                let x = p.x.clamp(r.origin.x, r.origin.x + r.size.width - 1.0);
                let y = p.y.clamp(r.origin.y, r.origin.y + r.size.height - 1.0);
                if (x, y) != (p.x, p.y) {
                    unsafe { CGEventSetLocation(event, CGPoint { x, y }) };
                }
            }
            event
        }
    }
}

/// Le filtre, sur son propre fil (avec sa boucle d'évènements) : un filtre
/// trop lent est coupé par macOS, il est alors rallumé (`tap_callback`).
pub struct Hooks {
    run_loop: usize,
    join: Option<JoinHandle<()>>,
}

impl Hooks {
    /// `sink` reçoit Entrée et Échap du clavier du Mac ; le premier appel le fixe.
    pub fn start(sink: impl Fn(CoverKey) + Send + Sync + 'static) -> Result<Hooks> {
        let _ = KEY_SINK.set(Box::new(sink));
        let (tx, rx) = mpsc::channel();
        let join = std::thread::spawn(move || unsafe {
            // Clavier, souris, trackpad, molette : tous les évènements d'entrée.
            let mask: u64 = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED) | (0..=7u64).fold(0, |m, t| m | (1 << t)) | (1 << 22) | (1 << 25) | (1 << 26) | (1 << 27) | (1 << 29) | (1 << 30) | (1 << 31);
            let tap = CGEventTapCreate(SESSION_TAP, HEAD_INSERT, 0, mask, tap_callback, std::ptr::null_mut());
            if tap.is_null() {
                let _ = tx.send(None);
                return;
            }
            TAP.store(tap, Ordering::SeqCst);
            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            let _ = tx.send(Some(run_loop as usize));
            CFRunLoopRun();
            CGEventTapEnable(tap, false);
            TAP.store(std::ptr::null_mut(), Ordering::SeqCst);
            CFMachPortInvalidate(tap);
            CFRelease(source);
            CFRelease(tap);
        });
        match rx.recv()? {
            Some(run_loop) => Ok(Hooks { run_loop, join: Some(join) }),
            None => {
                let _ = join.join();
                bail!("filtre du clavier et de la souris refusé (autorisation Accessibilité de KyberNow Station ?)")
            }
        }
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe { CFRunLoopStop(self.run_loop as *mut c_void) };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        ENTER_DOWN.store(false, Ordering::SeqCst);
        ESCAPE_DOWN.store(false, Ordering::SeqCst);
        *CLIP.lock().unwrap() = None;
    }
}

// --- Apps de l'utilisateur ---------------------------------------------------------

fn visible_apps() -> Vec<objc2::rc::Retained<NSRunningApplication>> {
    let me = std::process::id() as i32;
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|a| {
            a.activationPolicy() == NSApplicationActivationPolicy::Regular
                && !a.isHidden()
                && a.processIdentifier() != me
                && a.bundleIdentifier().is_none_or(|id| id.to_string() != OWN_BUNDLE)
        })
        .collect()
}

fn app_of(pid: u32) -> Option<objc2::rc::Retained<NSRunningApplication>> {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
}

/// Début de la mise en place du prêt (secondes Unix) : une app ouverte
/// ensuite est une app de l'ami (`acceptable_foreground`).
static BEGUN: Mutex<Option<f64>> = Mutex::new(None);

/// Masque les apps de l'utilisateur ; rend de quoi les montrer à la fin.
pub fn minimize_all() -> Vec<Window> {
    BEGUN.lock().unwrap().get_or_insert(unix_now() as f64);
    visible_apps()
        .into_iter()
        .map(|a| {
            a.hide();
            let pid = a.processIdentifier() as u32;
            Window { hwnd: pid as i64, pid, placement: [0; 10] }
        })
        .collect()
}

/// Une app de l'utilisateur qui revient (rappel, notification) : masquée de nouveau.
pub fn minimize_again(saved: &[Window]) {
    BEGUN.lock().unwrap().get_or_insert(unix_now() as f64);
    for w in saved {
        if let Some(a) = app_of(w.pid)
            && !a.isHidden()
        {
            a.hide();
        }
    }
}

/// Fin du prêt : les apps de l'utilisateur montrées de nouveau.
pub fn restore(saved: &[Window]) {
    for w in saved {
        if let Some(a) = app_of(w.pid) {
            a.unhide();
        }
    }
}

/// Le Dock et la barre des menus : cachés par le lanceur en plein écran.
pub fn taskbars(_show: bool) {}

fn main_bounds() -> CGRect {
    unsafe { CGDisplayBounds(CGMainDisplayID()) }
}

/// Le pointeur de l'ami gardé sur son écran (l'écran principal), ou libéré.
pub fn clip(display: Option<&Display>) {
    *CLIP.lock().unwrap() = display.map(|_| main_bounds());
}

pub fn keep_cursor(d: &Display) {
    clip(Some(d));
}

pub fn center_cursor(_d: &Display) {
    let r = main_bounds();
    unsafe { CGWarpMouseCursorPosition(CGPoint { x: r.origin.x + r.size.width / 2.0, y: r.origin.y + r.size.height / 2.0 }) };
}

/// Les fenêtres « Ouvrir » des apps : pas d'équivalent sûr sur Mac.
pub fn file_dialogs() -> HashSet<isize> {
    HashSet::new()
}

pub fn close_file_dialogs(_known: &HashSet<isize>) {}

/// Les apps nées pendant le prêt (après `started`).
pub struct Born {
    started: f64,
}

impl Born {
    pub fn new(started_unix: i64) -> Born {
        Born { started: started_unix as f64 }
    }

    pub fn is_new(&mut self, pid: u32) -> bool {
        app_of(pid).and_then(|a| a.launchDate()).is_some_and(|d| d.timeIntervalSince1970() >= self.started - 1.0)
    }
}

/// Les apps du prêt (nées pendant le prêt, hors l'agent), en numéros de processus.
fn loan_apps(born: &mut Born) -> Vec<objc2::rc::Retained<NSRunningApplication>> {
    let me = std::process::id() as i32;
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular && a.processIdentifier() != me)
        .filter(|a| a.bundleIdentifier().is_none_or(|id| id.to_string() != OWN_BUNDLE))
        .filter(|a| born.is_new(a.processIdentifier() as u32))
        .collect()
}

pub fn loan_windows(born: &mut Born) -> Vec<Hwnd> {
    loan_apps(born).iter().map(|a| -(a.processIdentifier() as isize)).collect()
}

/// Les fenêtres du prêt restent sur l'écran de l'ami : l'écran principal, où
/// macOS ouvre les apps (rien à déplacer).
pub fn keep_on(_display: &Display, _born: &mut Born) {}

/// Fin du prêt : les apps du prêt sont priées de quitter ; rend combien restent.
pub fn close_loan_windows(born: &mut Born) -> usize {
    let apps = loan_apps(born);
    for a in &apps {
        a.terminate();
    }
    apps.len()
}

/// Les numéros des processus de ce nom (`pgrep -x`).
pub fn processes(name: &str) -> Vec<u32> {
    let Ok(out) = std::process::Command::new("/usr/bin/pgrep").args(["-x", name]).output() else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect()
}

pub fn kill(pid: u32) {
    unsafe { libc::kill(pid as i32, libc::SIGKILL) };
}

/// Ce qui est au premier plan : la fenêtre clé de l'agent s'il l'est, sinon
/// l'opposé du numéro de processus de l'app au premier plan.
pub fn foreground() -> Hwnd {
    let front = NSWorkspace::sharedWorkspace().frontmostApplication();
    let pid = front.as_ref().map_or(0, |a| a.processIdentifier());
    if pid == std::process::id() as i32 {
        if let Some(mtm) = MainThreadMarker::new()
            && let Some(key) = NSApplication::sharedApplication(mtm).keyWindow()
        {
            return objc2::rc::Retained::as_ptr(&key) as Hwnd;
        }
        return 0;
    }
    -(pid as isize)
}

/// Une app de l'ami au premier plan : née pendant le prêt (jeu, app permise),
/// ou Steam. Le reste (Finder, une app de l'hôte) rend la main au lanceur.
pub fn acceptable_foreground(fg: Hwnd, _display: &Display) -> bool {
    if fg >= 0 {
        return false;
    }
    let Some(a) = app_of((-fg) as u32) else { return false };
    if a.bundleIdentifier().is_some_and(|id| id.to_string() == "com.valvesoftware.steam") {
        return true;
    }
    let Some(begun) = *BEGUN.lock().unwrap() else { return false };
    a.activationPolicy() == NSApplicationActivationPolicy::Regular && a.launchDate().is_some_and(|d| d.timeIntervalSince1970() >= begun - 1.0)
}

/// Le lanceur devant : l'agent activé, sa fenêtre clé.
pub fn bring_to_front(hwnd: Hwnd) {
    NSRunningApplication::currentApplication().activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    if hwnd > 0 {
        let window = unsafe { &*(hwnd as *const NSWindow) };
        window.makeKeyAndOrderFront(None);
    }
}

pub fn allow_foreground() {}

/// Un cache au-dessus de tout (même du Dock et de la barre des menus).
pub fn topmost(hwnd: Hwnd) {
    if hwnd > 0 {
        let window = unsafe { &*(hwnd as *const NSWindow) };
        // CGShieldingWindowLevel : au-dessus de tout, comme les économiseurs d'écran.
        window.setLevel(2_147_483_628);
        window.orderFrontRegardless();
    }
}

/// Les manettes du lanceur : pas encore de manettes sur la Station Mac.
pub fn read_pads() -> u16 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_the_friend_cannot_use() {
        assert!(!injected_key_allowed(TAB, FLAG_COMMAND));
        assert!(injected_key_allowed(TAB, 0));
        assert!(!injected_key_allowed(SPACE, FLAG_COMMAND));
        assert!(!injected_key_allowed(ESCAPE, FLAG_COMMAND | FLAG_OPTION));
        assert!(injected_key_allowed(ESCAPE, 0));
        assert!(!injected_key_allowed(123, FLAG_CONTROL));
        assert!(injected_key_allowed(123, 0));
        assert!(!injected_key_allowed(160, 0));
        assert!(injected_key_allowed(12, FLAG_COMMAND)); // ⌘Q : quitter le jeu
    }
}
