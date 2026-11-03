//! Ce qui occupe l'écran de l'ordinateur pendant une session : une app en
//! plein écran, et si c'est un jeu. L'app du Mac s'en sert pour passer la
//! souris en mode jeu (déplacements bruts) toute seule.
//!
//! Le service vit dans la session 0, sans écran : un agent (`kyber-admin
//! display-agent foreground`) tourne dans la session de l'utilisateur le
//! temps de la session et écrit l'état toutes les 400 ms dans
//! `foreground/state.json`. Il s'arrête de lui-même quand `foreground/run`
//! disparaît ou n'est plus rafraîchi (le signal de vie de l'app le touche).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

/// Sans signal de vie depuis ce délai, l'agent s'arrête (le même que pour la
/// session de l'app et son écran virtuel).
pub const STALE: Duration = crate::activity::HEARTBEAT_GRACE;

pub struct Watcher {
    dir: PathBuf,
}

impl Watcher {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("foreground") }
    }

    fn run_file(&self) -> PathBuf {
        self.dir.join("run")
    }

    /// Début de session : lance l'agent (une seule fois). `run` n'est écrit
    /// que si un agent tourne : sinon `active()` dirait « en session » pendant
    /// 20 s sans personne pour écrire l'état.
    pub fn start(&self, data_dir: &Path) {
        let _ = std::fs::create_dir_all(&self.dir);
        if self.active() {
            let _ = std::fs::write(self.run_file(), b"");
            return; // un agent tourne déjà
        }
        let args = vec!["foreground".to_string(), "--dir".to_string(), self.dir.display().to_string()];
        #[cfg(windows)]
        let launched = crate::display::agent::launch(data_dir, &args);
        #[cfg(not(windows))]
        let launched: anyhow::Result<()> = {
            let _ = data_dir;
            std::env::current_exe()
                .and_then(|exe| std::process::Command::new(exe).arg("display-agent").args(&args).spawn())
                .map(|_| ())
                .map_err(Into::into)
        };
        match launched {
            Ok(()) => {
                let _ = std::fs::write(self.run_file(), b"");
            }
            Err(e) => tracing::warn!("veille du plein écran : {e:#}"),
        }
    }

    /// Une session d'app est en cours (signal de vie récent).
    pub fn active(&self) -> bool {
        self.run_file().metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < STALE)
    }

    /// Signal de vie de l'app : l'agent continue.
    pub fn touch(&self) {
        if self.run_file().exists() {
            let _ = std::fs::write(self.run_file(), b"");
        }
    }

    /// Fin de session : l'agent s'arrête à son prochain tour.
    pub fn stop(&self) {
        let _ = std::fs::remove_file(self.run_file());
        let _ = std::fs::remove_file(self.dir.join("state.json"));
    }

    /// Dernier état écrit par l'agent (inconnu s'il date de plus de 3 s).
    pub fn state(&self) -> Value {
        let path = self.dir.join("state.json");
        let recent = path.metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < Duration::from_secs(3));
        if !recent {
            return json!({ "known": false, "fullscreen": false, "game": false });
        }
        std::fs::read(&path).ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|mut v| {
                v["known"] = json!(true);
                v
            })
            .unwrap_or_else(|| json!({ "known": false, "fullscreen": false, "game": false }))
    }
}

/// Plein écran mais pas un jeu : navigateurs (vidéo), lecteurs, diaporamas,
/// visioconférence. La souris y reste celle du bureau.
#[cfg_attr(not(windows), allow(dead_code))]
const NOT_GAMES: &[&str] = &[
    "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe", "arc.exe", "iexplore.exe",
    "vlc.exe", "mpc-hc64.exe", "mpc-be64.exe", "mpv.exe", "potplayermini64.exe", "wmplayer.exe", "video.ui.exe",
    "microsoft.media.player.exe", "plex.exe", "kodi.exe", "spotify.exe", "netflix.exe",
    "powerpnt.exe", "acrord32.exe", "acrobat.exe", "teams.exe", "ms-teams.exe", "zoom.exe",
    "explorer.exe", "applicationframehost.exe", "lockapp.exe", "logonui.exe", "kybernow-tray.exe",
];

/// Agent : boucle de veille, dans la session de l'utilisateur.
pub fn run_agent(dir: &Path) -> anyhow::Result<()> {
    let run = dir.join("run");
    let state = dir.join("state.json");
    let tmp = dir.join("state.tmp");
    let started = SystemTime::now();
    loop {
        let alive = run.metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < STALE);
        // Au plus 24 h, quoi qu'il arrive.
        let too_old = SystemTime::now().duration_since(started).is_ok_and(|d| d > Duration::from_secs(86_400));
        if !alive || too_old {
            let _ = std::fs::remove_file(&state);
            return Ok(());
        }
        let value = probe();
        std::fs::write(&tmp, serde_json::to_vec(&value)?)?;
        std::fs::rename(&tmp, &state)?;
        std::thread::sleep(Duration::from_millis(400));
    }
}

#[cfg(windows)]
pub fn probe() -> Value {
    use windows_sys::Win32::Foundation::{CloseHandle, RECT};
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };

    unsafe {
        // Jeu en plein écran exclusif (DirectX) : Windows le sait directement.
        let mut quns = 0;
        let d3d = SHQueryUserNotificationState(&mut quns) == 0 && quns == QUNS_RUNNING_D3D_FULL_SCREEN;

        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return json!({ "fullscreen": d3d, "game": d3d, "exe": null, "d3d": d3d });
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        // Bureau et barre des tâches : jamais « plein écran ».
        let shell = matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd");

        let mut rect: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(monitor, &mut info);
        let m = info.rcMonitor;
        // Fenêtre sans bordure qui couvre tout l'écran (plein écran « fenêtré » des jeux et des vidéos).
        let covers = rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom;

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut exe = String::new();
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !process.is_null() {
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                exe = path.rsplit(['\\', '/']).next().unwrap_or("").to_lowercase();
            }
            CloseHandle(process);
        }

        let fullscreen = d3d || (covers && !shell);
        let game = fullscreen && !NOT_GAMES.contains(&exe.as_str());
        json!({ "fullscreen": fullscreen, "game": game, "exe": exe, "d3d": d3d })
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn probe() -> Value {
    json!({ "fullscreen": false, "game": false, "exe": null, "d3d": false })
}

/// Plein écran mais pas un jeu, sur Mac (nom de l'app, en minuscules).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const MAC_NOT_GAMES: &[&str] = &[
    "safari", "google chrome", "firefox", "microsoft edge", "arc", "brave browser", "opera", "vivaldi", "orion",
    "quicktime player", "vlc", "iina", "infuse", "mpv", "tv", "music", "podcasts", "photos", "preview", "spotify",
    "plex", "kodi", "keynote", "microsoft powerpoint", "zoom.us", "microsoft teams", "facetime", "finder",
    "loginwindow", "screensaverengine", "kybernow link",
];

/// Une fenêtre de la liste du serveur de fenêtres, de l'avant vers l'arrière.
#[derive(Debug, Clone)]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct MacWindow {
    layer: i32,
    /// x, y, largeur, hauteur, en points.
    bounds: (f64, f64, f64, f64),
    owner: String,
}

/// La fenêtre de premier plan (la première du niveau des apps) couvre-t-elle
/// un écran, et son app est-elle un jeu ?
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn classify_mac(windows: &[MacWindow], displays: &[(f64, f64, f64, f64)]) -> Value {
    let Some(front) = windows.iter().find(|w| w.layer == 0 && w.bounds.2 > 1.0 && w.bounds.3 > 1.0) else {
        return json!({ "fullscreen": false, "game": false, "exe": null, "d3d": false });
    };
    let (x, y, w, h) = front.bounds;
    let covers = displays.iter().any(|&(dx, dy, dw, dh)| x <= dx && y <= dy && x + w >= dx + dw && y + h >= dy + dh);
    let exe = front.owner.to_lowercase();
    let game = covers && !MAC_NOT_GAMES.contains(&exe.as_str());
    json!({ "fullscreen": covers, "game": game, "exe": exe, "d3d": false })
}

/// Station Mac : la fenêtre au premier plan d'après `CGWindowListCopyWindowInfo`
/// (place et app de chaque fenêtre, sans l'autorisation d'enregistrer l'écran).
#[cfg(target_os = "macos")]
pub fn probe() -> Value {
    classify_mac(&mac::windows(), &mac::displays())
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void};

    use super::MacWindow;

    type CFTypeRef = *const c_void;

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        static kCGWindowLayer: CFTypeRef;
        static kCGWindowBounds: CFTypeRef;
        static kCGWindowOwnerName: CFTypeRef;
        fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFTypeRef;
        fn CGRectMakeWithDictionaryRepresentation(dict: CFTypeRef, rect: *mut CGRect) -> bool;
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
        fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, kind: isize, value: *mut c_void) -> bool;
        fn CFStringGetCString(string: CFTypeRef, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
        fn CFRelease(object: CFTypeRef);
    }

    /// kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements.
    const ON_SCREEN: u32 = 1 | 16;
    const SINT32: isize = 3;
    const UTF8: u32 = 0x0800_0100;

    pub(super) fn windows() -> Vec<MacWindow> {
        let mut out = Vec::new();
        unsafe {
            let list = CGWindowListCopyWindowInfo(ON_SCREEN, 0);
            if list.is_null() {
                return out;
            }
            for i in 0..CFArrayGetCount(list) {
                let dict = CFArrayGetValueAtIndex(list, i);
                let mut layer = 0i32;
                let n = CFDictionaryGetValue(dict, kCGWindowLayer);
                if n.is_null() || !CFNumberGetValue(n, SINT32, (&mut layer as *mut i32).cast()) {
                    continue;
                }
                let mut r = CGRect::default();
                let b = CFDictionaryGetValue(dict, kCGWindowBounds);
                if b.is_null() || !CGRectMakeWithDictionaryRepresentation(b, &mut r) {
                    continue;
                }
                let mut owner = String::new();
                let s = CFDictionaryGetValue(dict, kCGWindowOwnerName);
                let mut buf = [0 as c_char; 256];
                if !s.is_null() && CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) {
                    owner = std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                }
                out.push(MacWindow { layer, bounds: (r.x, r.y, r.w, r.h), owner });
            }
            CFRelease(list);
        }
        out
    }

    pub(super) fn displays() -> Vec<(f64, f64, f64, f64)> {
        let mut ids = [0u32; 16];
        let mut count = 0u32;
        if unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) } != 0 {
            return Vec::new();
        }
        ids[..count as usize]
            .iter()
            .map(|&id| {
                let r = unsafe { CGDisplayBounds(id) };
                (r.x, r.y, r.w, r.h)
            })
            .collect()
    }
}

#[cfg(test)]
mod mac_tests {
    use super::{MacWindow, classify_mac};

    fn win(layer: i32, bounds: (f64, f64, f64, f64), owner: &str) -> MacWindow {
        MacWindow { layer, bounds, owner: owner.into() }
    }

    #[test]
    fn a_full_screen_game_on_a_virtual_display() {
        let displays = [(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 2560.0, 1440.0)];
        // Barre des menus (niveau 24) devant, jeu sans bordure sur le second écran.
        let game = [win(24, (0.0, 0.0, 1920.0, 24.0), "Window Server"), win(0, (1920.0, 0.0, 2560.0, 1440.0), "Baldur's Gate 3")];
        let v = classify_mac(&game, &displays);
        assert_eq!((v["fullscreen"].as_bool(), v["game"].as_bool()), (Some(true), Some(true)));
        assert_eq!(v["exe"], "baldur's gate 3");
        // Vidéo en plein écran dans Safari : pas un jeu.
        let video = [win(0, (0.0, 0.0, 1920.0, 1080.0), "Safari")];
        let v = classify_mac(&video, &displays);
        assert_eq!((v["fullscreen"].as_bool(), v["game"].as_bool()), (Some(true), Some(false)));
        // Fenêtre ordinaire : rien.
        let window = [win(0, (100.0, 100.0, 800.0, 600.0), "Baldur's Gate 3")];
        assert_eq!(classify_mac(&window, &displays)["fullscreen"], false);
        assert_eq!(classify_mac(&[], &displays)["game"], false);
    }
}
