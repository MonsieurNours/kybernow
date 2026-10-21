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

#[cfg(not(windows))]
fn probe() -> Value {
    json!({ "fullscreen": false, "game": false, "exe": null, "d3d": false })
}
