//! Le prêt, côté Windows (docs/pret.md, « Station ») : l'état de la session
//! de l'utilisateur, la dernière entrée physique, l'agent de console
//! (`kybernow-tray.exe --console`, avec le compte de l'utilisateur), la
//! surveillance des processus de la session (par Admin, SYSTEM : il peut
//! fermer un Gestionnaire des tâches ou une invite élevés), le verrouillage
//! final et la veille empêchée. Hors de Windows (développement) : une session
//! toujours ouverte, personne devant, pas d'agent.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// La session de l'utilisateur sur l'écran de l'ordinateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Personne n'est connecté à Windows.
    None,
    /// Verrouillée (écran de verrouillage).
    Locked,
    Open,
}

/// Programmes de Windows refusés pendant un prêt, même lancés par une app permise.
#[cfg_attr(not(windows), allow(dead_code))]
const DENIED: &[&str] = &[
    "explorer.exe",
    "taskmgr.exe",
    "powershell.exe",
    "powershell_ise.exe",
    "pwsh.exe",
    "windowsterminal.exe",
    "wt.exe",
    "openconsole.exe",
    "regedit.exe",
    "regedt32.exe",
    "mmc.exe",
    "control.exe",
    "systemsettings.exe",
    "msconfig.exe",
    "resmon.exe",
    "perfmon.exe",
    "mshta.exe",
];

/// Permis seulement lancés (même de loin) par une app permise ou un jeu :
/// installateurs des bibliothèques d'un jeu, lanceurs en script. Refusés
/// autrement (depuis un programme de Windows, ou seuls).
#[cfg_attr(not(windows), allow(dead_code))]
const HELPERS: &[&str] = &["cmd.exe", "msiexec.exe", "rundll32.exe", "wscript.exe", "cscript.exe"];

/// Le parent d'un processus nouveau, tel que la surveillance l'a jugé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Parent {
    /// Pas permis pendant le prêt (ou d'avant le prêt).
    Other,
    /// Permis, hors des apps : un programme de Windows, KyberNow, Kyber.
    Allowed,
    /// Une app permise, Steam ou un jeu, ou un de leurs descendants.
    App,
}

/// Ce qui est permis pendant un prêt, à part les programmes de Windows.
#[derive(Debug, Clone, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Rules {
    /// Exécutables des apps permises (chemins complets, en minuscules).
    pub apps: Vec<String>,
    /// Dossiers dont tout est permis : KyberNow, Kyber, Steam (en minuscules, avec `\` final).
    pub dirs: Vec<String>,
    /// Dossier de Windows (`c:\windows\`).
    pub system: String,
    /// Dossier de Steam (vide s'il n'est pas connu) : ses programmes et ses jeux sont des apps.
    pub steam: String,
}

/// Le verdict sur un processus nouveau de la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Verdict {
    Allow,
    Deny,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Rules {
    /// Les règles de docs/pret.md (« Station », point 4) : refusés d'abord ;
    /// puis les apps permises et leurs enfants, Steam et ses jeux, KyberNow,
    /// et les programmes de Windows ; le reste est fermé. `cmd`, `msiexec`,
    /// `rundll32` et les scripts : seulement sous une app ou un jeu. Rend aussi
    /// si le processus est de la lignée d'une app (pour ses propres enfants).
    pub fn judge(&self, path: &str, parent: Parent) -> (Verdict, bool) {
        let path = path.to_ascii_lowercase().replace('/', "\\");
        let name = path.rsplit('\\').next().unwrap_or(&path);
        if DENIED.contains(&name) {
            return (Verdict::Deny, false);
        }
        let app = parent == Parent::App
            || self.apps.contains(&path)
            || (!self.steam.is_empty() && path.starts_with(self.steam.as_str()))
            || path.contains("\\steamapps\\");
        if HELPERS.contains(&name) {
            return if parent == Parent::App { (Verdict::Allow, true) } else { (Verdict::Deny, false) };
        }
        let allowed = app
            || parent == Parent::Allowed
            || self.dirs.iter().any(|d| path.starts_with(d.as_str()))
            || (!self.system.is_empty() && path.starts_with(self.system.as_str()));
        if allowed { (Verdict::Allow, app) } else { (Verdict::Deny, false) }
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Rules {
    /// À la fin du prêt, ce qui reste ouvert : Windows, KyberNow, Kyber et
    /// Steam, mais pas ses jeux (`steamapps`).
    pub fn kept_at_end(&self, path: &str) -> bool {
        let path = path.to_ascii_lowercase().replace('/', "\\");
        if path.contains("\\steamapps\\") {
            return false;
        }
        (!self.system.is_empty() && path.starts_with(self.system.as_str())) || self.dirs.iter().any(|d| path.starts_with(d.as_str()))
    }
}

/// Les processus nouveaux, chaque parent avant ses enfants.
#[cfg_attr(not(windows), allow(dead_code))]
fn parents_first(mut fresh: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    let mut ordered = Vec::with_capacity(fresh.len());
    while !fresh.is_empty() {
        let waiting: HashSet<u32> = fresh.iter().map(|(pid, _)| *pid).collect();
        let (ready, rest): (Vec<_>, Vec<_>) = fresh.into_iter().partition(|(pid, ppid)| ppid == pid || !waiting.contains(ppid));
        if ready.is_empty() {
            // Une boucle (identifiants réutilisés) : le reste tel quel.
            ordered.extend(rest);
            break;
        }
        ordered.extend(ready);
        fresh = rest;
    }
    ordered
}

fn dir_key(path: &Path) -> String {
    let mut s = path.display().to_string().to_ascii_lowercase().replace('/', "\\");
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

/// Les règles d'un prêt : `apps` les exécutables permis, `steam` le dossier de Steam s'il est connu.
pub fn rules(apps: &[String], steam: Option<PathBuf>, kyber_dir: &Path) -> Rules {
    let mut dirs = Vec::new();
    if let Some(install) = std::env::current_exe().ok().and_then(|e| e.parent().and_then(Path::parent).map(Path::to_path_buf)) {
        dirs.push(dir_key(&install));
    }
    dirs.push(dir_key(kyber_dir));
    let steam = steam.map(|s| dir_key(&s)).unwrap_or_default();
    if !steam.is_empty() {
        dirs.push(steam.clone());
    }
    let system = std::env::var_os("SystemRoot").map(|r| dir_key(Path::new(&r))).unwrap_or_else(|| "c:\\windows\\".into());
    Rules { apps: apps.iter().map(|a| a.to_ascii_lowercase().replace('/', "\\")).collect(), dirs, system, steam }
}

/// La surveillance en cours ; l'arrêter ferme ce qu'elle a laissé ouvrir.
pub struct Watch {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Processus nouveaux permis pendant le prêt (à fermer à la fin).
    started: std::sync::Arc<std::sync::Mutex<HashSet<u32>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watch {
    /// Arrête la surveillance et ferme les processus ouverts pendant le prêt
    /// (hors KyberNow, Kyber et Windows) : les apps permises, après le
    /// `WM_CLOSE` de l'agent.
    pub fn stop_and_close(mut self, rules: &Rules) -> usize {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let started: Vec<u32> = self.started.lock().unwrap().iter().copied().collect();
        platform::close_started(&started, rules)
    }
}

/// Une surveillance lâchée sans `stop_and_close` (prêt arrêté pendant sa
/// préparation) s'arrête quand même.
impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

pub use platform::{host_name, idle_secs, keep_awake, lock_session, session_state, spawn_console, steam_dir, watch};

/// Le mode console existe ici : Windows, ou une Station Mac installée (pas le
/// Mac de développement, où tout est simulé).
pub fn console_supported() -> bool {
    cfg!(windows) || crate::mac::is_station()
}

#[cfg(windows)]
mod platform {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Result, bail};

    use super::{Parent, Rules, SessionState, Verdict, Watch, parents_first};
    use crate::display::agent;

    fn console_session() -> Option<u32> {
        use windows_sys::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;
        let s = unsafe { WTSGetActiveConsoleSessionId() };
        (s != 0xFFFF_FFFF).then_some(s)
    }

    pub fn session_state() -> SessionState {
        use std::ffi::c_void;
        use windows_sys::Win32::System::RemoteDesktop::{
            WTS_CURRENT_SERVER_HANDLE, WTS_SESSIONSTATE_LOCK, WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
        };
        let Some(session) = console_session() else { return SessionState::None };
        unsafe {
            let mut buf: *mut u16 = std::ptr::null_mut();
            let mut len = 0u32;
            if WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, session, WTSSessionInfoEx, &mut buf, &mut len) == 0 || buf.is_null() {
                return SessionState::None;
            }
            let info = &*(buf as *const WTSINFOEXW);
            let (flags, user_empty) = if info.Level == 1 {
                let l1 = &info.Data.WTSInfoExLevel1;
                (l1.SessionFlags, l1.UserName[0] == 0)
            } else {
                (-1, true)
            };
            WTSFreeMemory(buf as *mut c_void);
            if user_empty {
                SessionState::None
            } else if flags == WTS_SESSIONSTATE_LOCK as i32 {
                SessionState::Locked
            } else {
                SessionState::Open
            }
        }
    }

    /// Secondes depuis la dernière entrée sur l'ordinateur (`display-agent idle`).
    pub fn idle_secs() -> Option<u64> {
        let v = agent::run_as_user(&["idle".to_string()], 10_000).ok()?;
        v["idle_ms"].as_u64().map(|ms| ms / 1000)
    }

    /// Verrouille la session (`display-agent lock`).
    pub fn lock_session() -> Result<()> {
        agent::run_as_user(&["lock".to_string()], 10_000).map(|_| ())
    }

    /// Lance l'agent de console dans la session de l'utilisateur, avec son compte.
    pub fn spawn_console() -> Result<()> {
        let tray = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent()?.parent().map(|d| d.join("tray").join("kybernow-tray.exe")))
            .filter(|t| t.exists());
        let Some(tray) = tray else { bail!("kybernow-tray.exe introuvable") };
        agent::launch_as_user_with(&tray, &["--console".to_string()])
    }

    /// Le prénom de l'hôte : le nom complet du compte de la session, sinon son nom.
    pub fn host_name() -> Option<String> {
        agent::run_as_user(&["whoami".to_string()], 10_000).ok().and_then(|v| v["name"].as_str().map(str::to_owned)).filter(|n| !n.is_empty())
    }

    /// Le dossier de Steam de l'utilisateur (`HKEY_USERS\<SID>\Software\Valve\Steam\SteamPath`).
    pub fn steam_dir() -> Option<PathBuf> {
        use windows_sys::Win32::System::Registry::{HKEY_USERS, RRF_RT_REG_SZ, RegGetValueW};
        let sid = agent::ConsoleUser::open().ok()?.sid().ok()?;
        let key: Vec<u16> = format!("{sid}\\Software\\Valve\\Steam").encode_utf16().chain([0]).collect();
        let name: Vec<u16> = "SteamPath".encode_utf16().chain([0]).collect();
        let mut buf = [0u16; 512];
        let mut len = (buf.len() * 2) as u32;
        let r = unsafe { RegGetValueW(HKEY_USERS, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        if r != 0 {
            return None;
        }
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..end]).replace('/', "\\")))
    }

    /// Empêche la veille et l'extinction de l'écran (qui verrouillerait la
    /// session) tant que le prêt est activé : un fil à part tient l'état.
    pub fn keep_awake(on: bool) {
        use windows_sys::Win32::System::Power::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState};
        static AWAKE: Mutex<Option<std::sync::mpsc::Sender<()>>> = Mutex::new(None);
        let mut awake = AWAKE.lock().unwrap();
        match (on, awake.is_some()) {
            (true, false) => {
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                std::thread::spawn(move || {
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) };
                    let _ = rx.recv();
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
                });
                *awake = Some(tx);
                tracing::info!("prêt activé : veille et extinction de l'écran empêchées");
            }
            (false, true) => {
                *awake = None;
                tracing::info!("prêt coupé : veille rendue à Windows");
            }
            _ => {}
        }
    }

    /// (pid, pid du parent) de tous les processus.
    fn snapshot() -> Vec<(u32, u32)> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
        };
        let mut out = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    out.push((entry.th32ProcessID, entry.th32ParentProcessID));
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
        out
    }

    fn session_of(pid: u32) -> Option<u32> {
        use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
        let mut s = 0u32;
        (unsafe { ProcessIdToSessionId(pid, &mut s) } != 0).then_some(s)
    }

    fn image_path(pid: u32) -> Option<String> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
            CloseHandle(h);
            ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    fn terminate(pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if h.is_null() {
                return false;
            }
            let ok = TerminateProcess(h, 1) != 0;
            CloseHandle(h);
            ok
        }
    }

    /// Surveille les processus de la session de l'utilisateur toutes les
    /// 500 ms : ceux d'avant le prêt ne sont pas touchés ; un nouveau est
    /// permis selon `rules`, sinon fermé.
    pub fn watch(rules: Rules) -> Option<Watch> {
        let session = console_session()?;
        let before: HashSet<u32> = snapshot().into_iter().map(|(pid, _)| pid).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let started = Arc::new(Mutex::new(HashSet::new()));
        let (stop2, started2) = (stop.clone(), started.clone());
        let thread = std::thread::spawn(move || {
            let mut seen = before;
            // Les processus permis pendant le prêt, et s'ils sont de la lignée d'une app.
            let mut allowed: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
            while !stop2.load(Ordering::SeqCst) {
                let fresh: Vec<(u32, u32)> = snapshot().into_iter().filter(|(pid, _)| !seen.contains(pid)).collect();
                // Les parents avant leurs enfants : les processus de WebView2
                // du lanceur, nés dans le même passage que lui, sont permis.
                for (pid, ppid) in parents_first(fresh) {
                    if session_of(pid) != Some(session) {
                        seen.insert(pid);
                        continue;
                    }
                    // Un processus encore illisible est revu au passage suivant.
                    let Some(path) = image_path(pid) else { continue };
                    seen.insert(pid);
                    let parent = match allowed.get(&ppid) {
                        None => Parent::Other,
                        Some(false) => Parent::Allowed,
                        Some(true) => Parent::App,
                    };
                    match rules.judge(&path, parent) {
                        (Verdict::Allow, app) => {
                            allowed.insert(pid, app);
                            started2.lock().unwrap().insert(pid);
                        }
                        (Verdict::Deny, _) => {
                            let done = terminate(pid);
                            tracing::info!("prêt : {path} fermé ({})", if done { "fait" } else { "refusé par Windows" });
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        Some(Watch { stop, started, thread: Some(thread) })
    }

    /// Ferme ce qui a été ouvert pendant le prêt et reste après la remise
    /// en état de l'agent, hors KyberNow, Kyber, Steam (remis par l'agent sur
    /// le compte de l'hôte) et Windows.
    pub fn close_started(started: &[u32], rules: &Rules) -> usize {
        let mut closed = 0;
        for pid in started {
            if let Some(path) = image_path(*pid)
                && !rules.kept_at_end(&path)
                && terminate(*pid)
            {
                closed += 1;
            }
        }
        closed
    }
}

/// Station Mac (docs/pret.md, « Station », équivalents Mac) : tout tourne dans
/// la session de l'utilisateur (Admin est un LaunchAgent), sans élévation.
/// Une app se reconnaît à l'identifiant de son paquet (`exe` des apps permises).
#[cfg(target_os = "macos")]
mod platform {
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Context, Result, bail};

    use super::{Rules, SessionState, Watch, parents_first};

    /// Apps refusées pendant un prêt, même ouvertes par une app permise.
    const DENIED: &[&str] = &[
        "com.apple.Terminal",
        "com.googlecode.iterm2",
        "com.apple.ActivityMonitor",
        "com.apple.systempreferences",
        "com.apple.Settings",
        "com.apple.ScriptEditor2",
        "com.apple.Automator",
        "com.apple.Console",
        "com.apple.DiskUtility",
        "com.apple.keychainaccess",
        "com.apple.shortcuts",
        "com.apple.exposelauncher",
        "com.apple.launchpad.launcher",
        "com.apple.finder.Open-iCloudDrive",
    ];
    const STEAM_ID: &str = "com.valvesoftware.steam";

    fn home() -> PathBuf {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
    }

    fn own_uid() -> u32 {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(home()).map(|m| m.uid()).unwrap_or(u32::MAX)
    }

    pub fn session_state() -> SessionState {
        use std::os::unix::fs::MetadataExt;
        // Une autre session (ou la fenêtre de connexion) devant l'écran : pas la nôtre.
        let console = std::fs::metadata("/dev/console").map(|m| m.uid()).unwrap_or(0);
        if console != own_uid() {
            return SessionState::None;
        }
        if crate::launch::platform_session_locked() { SessionState::Locked } else { SessionState::Open }
    }

    /// Secondes depuis la dernière entrée (`HIDIdleTime` d'IOHIDSystem, en ns).
    pub fn idle_secs() -> Option<u64> {
        let out = Command::new("/usr/sbin/ioreg").args(["-c", "IOHIDSystem", "-d", "4"]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let ns: u64 = text.lines().find_map(|l| l.split("\"HIDIdleTime\" = ").nth(1)).and_then(|v| v.trim().parse().ok())?;
        Some(ns / 1_000_000_000)
    }

    /// Verrouille la session (`kybernow-display lock`).
    pub fn lock_session() -> Result<()> {
        let agent = crate::display::mac_agent_path().context("agent des écrans introuvable")?;
        let out = Command::new(&agent).arg("lock").output().context("kybernow-display lock")?;
        if !out.status.success() {
            bail!("kybernow-display lock : {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }

    /// L'agent de console (`kybernow-console --console`, dans l'app) : dans son
    /// propre groupe de processus, pour que launchd ne l'arrête pas avec Admin
    /// (il se remet alors en état seul et verrouille la session).
    pub fn spawn_console() -> Result<()> {
        use std::os::unix::process::CommandExt;
        let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("kybernow-console"))).filter(|p| p.is_file());
        let Some(exe) = exe else { bail!("kybernow-console introuvable") };
        Command::new(&exe)
            .arg("--console")
            .process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("lancement de {}", exe.display()))?;
        Ok(())
    }

    /// Le prénom de l'hôte : le nom complet du compte (`id -F`).
    pub fn host_name() -> Option<String> {
        let out = Command::new("/usr/bin/id").arg("-F").output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|n| !n.is_empty())
    }

    /// Les données de Steam (`~/Library/Application Support/Steam`), s'il est installé.
    pub fn steam_dir() -> Option<PathBuf> {
        let dir = home().join("Library/Application Support/Steam");
        (dir.join("steamapps").is_dir() || dir.join("config").is_dir()).then_some(dir)
    }

    /// Pas de veille ni d'écran éteint (qui verrouillerait la session) tant
    /// que le prêt est activé : `caffeinate`, lié à Admin.
    pub fn keep_awake(on: bool) {
        static AWAKE: Mutex<Option<std::process::Child>> = Mutex::new(None);
        let mut awake = AWAKE.lock().unwrap();
        match (on, awake.is_some()) {
            (true, false) => {
                let pid = std::process::id().to_string();
                *awake = Command::new("/usr/bin/caffeinate").args(["-d", "-i", "-m", "-s", "-w", &pid]).spawn().ok();
            }
            (false, true) => {
                if let Some(mut child) = awake.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
            _ => {}
        }
    }

    /// Processus du compte : (pid, ppid, chemin de l'exécutable).
    fn snapshot() -> Vec<(u32, u32, String)> {
        let uid = own_uid().to_string();
        let Ok(out) = Command::new("/bin/ps").args(["-U", &uid, "-o", "pid=,ppid=,comm="]).output() else { return Vec::new() };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| {
                let mut it = l.trim_start().splitn(3, char::is_whitespace);
                let pid = it.next()?.trim().parse().ok()?;
                let ppid = it.next()?.trim().parse().ok()?;
                Some((pid, ppid, it.next()?.trim().to_string()))
            })
            .collect()
    }

    /// Le paquet `.app` le plus extérieur d'un chemin, et son identifiant.
    fn bundle_of(path: &str, cache: &mut HashMap<String, Option<String>>) -> Option<(String, String)> {
        let end = path.find(".app/")? + 4;
        let app = path[..end].to_string();
        let id = cache
            .entry(app.clone())
            .or_insert_with(|| {
                let out = Command::new("/usr/bin/plutil")
                    .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
                    .arg(Path::new(&app).join("Contents/Info.plist"))
                    .output()
                    .ok()?;
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
            })
            .clone()?;
        Some((app, id))
    }

    /// Verdict sur un processus nouveau (docs/pret.md, point 4, équivalent Mac) :
    /// seules les apps (`….app/Contents/…`) sont jugées. Refusées : la liste
    /// `DENIED`, et toute app qui n'est ni permise, ni Steam ou un jeu, ni
    /// KyberNow, ni un service de macOS (`/System/Library`), ni de la lignée
    /// d'une app permise. Rend aussi si le processus est de cette lignée.
    fn judge(rules: &Rules, path: &str, parent: super::Parent, cache: &mut HashMap<String, Option<String>>) -> (bool, bool) {
        use super::Parent;
        let lower = path.to_ascii_lowercase();
        let game = lower.contains("/steamapps/") || (!rules.steam.is_empty() && lower.starts_with(&rules.steam.replace('\\', "/")));
        let Some((app, id)) = bundle_of(path, cache) else {
            // Pas une app : outils lancés par les apps, services du système.
            return (true, parent == Parent::App || game);
        };
        // Seul l'exécutable principal d'une app est jugé : ses aides et ses
        // services XPC (lancés par launchd) suivent le sort de l'app.
        if path.get(app.len()..).is_none_or(|rest| !rest.starts_with("/Contents/MacOS/") || rest.contains(".app/")) {
            return (true, parent == Parent::App || game);
        }
        if DENIED.iter().any(|d| d.eq_ignore_ascii_case(&id)) {
            return (false, false);
        }
        let allowed_app = rules.apps.iter().any(|a| a.eq_ignore_ascii_case(&id) || a.eq_ignore_ascii_case(&app));
        let is_app = parent == Parent::App || allowed_app || game || id == STEAM_ID;
        let kept = rules.dirs.iter().any(|d| lower.starts_with(&d.replace('\\', "/"))) || lower.starts_with("/system/library/");
        (is_app || kept || parent == Parent::Allowed, is_app)
    }

    /// Surveille les processus de l'utilisateur toutes les 500 ms : ceux
    /// d'avant le prêt et ce que lancent les apps de l'hôte (les onglets d'un
    /// navigateur masqué…) ne sont pas touchés, sauf un jeu ; une app nouvelle
    /// (lancée par macOS, `launchd`, ou par une app du prêt) est permise selon
    /// `rules`, sinon fermée.
    pub fn watch(rules: Rules) -> Option<Watch> {
        let before: HashSet<u32> = snapshot().into_iter().map(|(pid, _, _)| pid).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let started = Arc::new(Mutex::new(HashSet::new()));
        let (stop2, started2) = (stop.clone(), started.clone());
        let thread = std::thread::spawn(move || {
            // La lignée des processus de l'hôte (pas launchd, qui lance tout).
            let mut host: HashSet<u32> = before.iter().copied().filter(|p| *p > 1).collect();
            let mut seen = before;
            let mut allowed: HashMap<u32, bool> = HashMap::new();
            let mut cache = HashMap::new();
            while !stop2.load(Ordering::SeqCst) {
                let all = snapshot();
                let paths: HashMap<u32, String> = all.iter().map(|(p, _, c)| (*p, c.clone())).collect();
                let fresh: Vec<(u32, u32)> = all.iter().filter(|(pid, _, _)| !seen.contains(pid)).map(|(p, pp, _)| (*p, *pp)).collect();
                for (pid, ppid) in parents_first(fresh) {
                    seen.insert(pid);
                    let Some(path) = paths.get(&pid) else { continue };
                    if host.contains(&ppid) && !path.to_ascii_lowercase().contains("/steamapps/") {
                        host.insert(pid);
                        continue;
                    }
                    let parent = match allowed.get(&ppid) {
                        None => super::Parent::Other,
                        Some(false) => super::Parent::Allowed,
                        Some(true) => super::Parent::App,
                    };
                    match judge(&rules, path, parent, &mut cache) {
                        (true, app) => {
                            allowed.insert(pid, app);
                            started2.lock().unwrap().insert(pid);
                        }
                        (false, _) => {
                            let done = unsafe { libc_kill(pid as i32, 9) } == 0;
                            tracing::info!("prêt : {path} fermé ({})", if done { "fait" } else { "refusé par macOS" });
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        Some(Watch { stop, started, thread: Some(thread) })
    }

    unsafe extern "C" {
        #[link_name = "kill"]
        fn libc_kill(pid: i32, sig: i32) -> i32;
    }

    /// Ferme les apps ouvertes pendant le prêt qui restent après la remise en
    /// état de l'agent, sauf KyberNow, Kyber, Steam (pas ses jeux) et macOS.
    pub fn close_started(started: &[u32], rules: &Rules) -> usize {
        let paths: HashMap<u32, String> = snapshot().into_iter().map(|(p, _, c)| (p, c)).collect();
        let mut closed = 0;
        for pid in started {
            let Some(path) = paths.get(pid) else { continue };
            let lower = path.to_ascii_lowercase();
            if !lower.contains(".app/") || lower.starts_with("/system/") || lower.contains("/steam.app/") {
                continue;
            }
            if !lower.contains("/steamapps/") && rules.dirs.iter().any(|d| lower.starts_with(&d.replace('\\', "/"))) {
                continue;
            }
            if unsafe { libc_kill(*pid as i32, 15) } == 0 {
                closed += 1;
            }
        }
        closed
    }

    #[cfg(test)]
    mod tests {
        use std::collections::HashMap;

        use super::super::{Parent, Rules};
        use super::judge;

        #[test]
        fn seul_l_executable_principal_d_une_app_est_juge() {
            let rules = Rules { apps: vec!["org.example.permise".into()], ..Rules::default() };
            let mut cache = HashMap::new();
            cache.insert("/Applications/Google Chrome.app".to_string(), Some("com.google.Chrome".to_string()));
            cache.insert("/Applications/Permise.app".to_string(), Some("org.example.permise".to_string()));
            cache.insert("/Applications/Utilities/Terminal.app".to_string(), Some("com.apple.Terminal".to_string()));
            let chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
            let helper = "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/1/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)";
            let xpc = "/Applications/Google Chrome.app/Contents/XPCServices/outil.xpc/Contents/MacOS/outil";
            // Une app hors liste lancée pendant le prêt : fermée ; ses aides suivent son sort.
            assert!(!judge(&rules, chrome, Parent::Other, &mut cache).0);
            assert!(judge(&rules, helper, Parent::Other, &mut cache).0);
            assert!(judge(&rules, xpc, Parent::Other, &mut cache).0);
            // Une app permise, et la lignée de ses enfants.
            assert_eq!(judge(&rules, "/Applications/Permise.app/Contents/MacOS/Permise", Parent::Other, &mut cache), (true, true));
            // Refusée même sous une app.
            assert!(!judge(&rules, "/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal", Parent::App, &mut cache).0);
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use std::path::PathBuf;

    use anyhow::Result;

    use super::{Rules, SessionState, Watch};

    pub fn session_state() -> SessionState {
        SessionState::Open
    }

    pub fn idle_secs() -> Option<u64> {
        None
    }

    pub fn lock_session() -> Result<()> {
        tracing::info!("(simulé) session verrouillée à la fin du prêt");
        Ok(())
    }

    pub fn spawn_console() -> Result<()> {
        Ok(())
    }

    pub fn host_name() -> Option<String> {
        None
    }

    pub fn steam_dir() -> Option<PathBuf> {
        None
    }

    pub fn keep_awake(_on: bool) {}

    pub fn watch(_rules: Rules) -> Option<Watch> {
        None
    }

    pub fn close_started(_started: &[u32], _rules: &Rules) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Rules {
        Rules {
            apps: vec!["d:\\jeux\\celeste\\celeste.exe".into()],
            dirs: vec!["c:\\program files\\kybernow\\".into(), "c:\\program files (x86)\\steam\\".into()],
            system: "c:\\windows\\".into(),
            steam: "c:\\program files (x86)\\steam\\".into(),
        }
    }

    #[test]
    fn ce_qui_est_permis_pendant_un_pret() {
        use Parent::{Allowed, App, Other};
        let rules = r();
        for (path, parent, verdict) in [
            ("D:\\Jeux\\Celeste\\Celeste.exe", Other, Verdict::Allow),
            ("D:\\Jeux\\Celeste\\crash.exe", App, Verdict::Allow),
            ("C:\\Program Files (x86)\\Steam\\steam.exe", Other, Verdict::Allow),
            ("E:\\SteamLibrary\\steamapps\\common\\Hades\\Hades.exe", Other, Verdict::Allow),
            ("C:\\Program Files\\KyberNow\\tray\\kybernow-tray.exe", Other, Verdict::Allow),
            ("C:\\Windows\\System32\\conhost.exe", App, Verdict::Allow),
            ("C:\\Windows\\System32\\dllhost.exe", Other, Verdict::Allow),
            // Refusés, même enfants d'une app permise.
            ("C:\\Windows\\explorer.exe", App, Verdict::Deny),
            ("C:\\Windows\\System32\\Taskmgr.exe", Other, Verdict::Deny),
            ("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", App, Verdict::Deny),
            ("C:\\Program Files\\PowerShell\\7\\pwsh.exe", Other, Verdict::Deny),
            ("C:\\Program Files\\WindowsApps\\Microsoft.WindowsTerminal\\WindowsTerminal.exe", Other, Verdict::Deny),
            ("C:\\Windows\\regedit.exe", Other, Verdict::Deny),
            ("C:\\Windows\\System32\\mmc.exe", Other, Verdict::Deny),
            ("C:\\Windows\\ImmersiveControlPanel\\SystemSettings.exe", Other, Verdict::Deny),
            // cmd, msiexec, rundll32 : sous un jeu (installateurs) oui, sinon non.
            ("C:\\Windows\\System32\\cmd.exe", App, Verdict::Allow),
            ("C:\\Windows\\System32\\msiexec.exe", App, Verdict::Allow),
            ("C:\\Windows\\System32\\rundll32.exe", App, Verdict::Allow),
            ("C:\\Windows\\System32\\cmd.exe", Allowed, Verdict::Deny),
            ("C:\\Windows\\System32\\cmd.exe", Other, Verdict::Deny),
            ("C:\\Windows\\System32\\rundll32.exe", Allowed, Verdict::Deny),
            // Une app hors liste.
            ("C:\\Program Files\\Mozilla Firefox\\firefox.exe", Other, Verdict::Deny),
            ("C:\\Users\\matthieu\\Downloads\\outil.exe", Other, Verdict::Deny),
        ] {
            assert_eq!(rules.judge(path, parent).0, verdict, "{path} ({parent:?})");
        }
        // La lignée : un jeu et ses enfants en sont, un programme de Windows lancé seul non.
        assert!(rules.judge("E:\\SteamLibrary\\steamapps\\common\\Hades\\Hades.exe", Other).1);
        assert!(rules.judge("C:\\Windows\\System32\\conhost.exe", App).1);
        assert!(!rules.judge("C:\\Windows\\System32\\dllhost.exe", Other).1);
        assert!(!rules.judge("C:\\Windows\\System32\\dllhost.exe", Allowed).1);
    }

    #[test]
    fn a_la_fin_steam_reste_ses_jeux_non() {
        let rules = r();
        assert!(rules.kept_at_end("C:\\Program Files (x86)\\Steam\\steam.exe"));
        assert!(rules.kept_at_end("C:\\Program Files (x86)\\Steam\\bin\\cef\\cef.win7x64\\steamwebhelper.exe"));
        assert!(rules.kept_at_end("C:\\Program Files\\KyberNow\\tray\\kybernow-tray.exe"));
        assert!(rules.kept_at_end("C:\\Windows\\System32\\conhost.exe"));
        assert!(!rules.kept_at_end("C:\\Program Files (x86)\\Steam\\steamapps\\common\\Hades\\Hades.exe"));
        assert!(!rules.kept_at_end("E:\\SteamLibrary\\steamapps\\common\\Hades\\Hades.exe"));
        assert!(!rules.kept_at_end("D:\\Jeux\\Celeste\\Celeste.exe"));
    }

    #[test]
    fn les_parents_avant_leurs_enfants() {
        // Le lanceur (10) et sa WebView2 (11, 12), vus dans le désordre ; 5 existait déjà.
        let order = parents_first(vec![(12, 11), (11, 10), (10, 5), (20, 20)]);
        let at = |pid: u32| order.iter().position(|(p, _)| *p == pid).unwrap();
        assert!(at(10) < at(11) && at(11) < at(12));
        assert_eq!(order.len(), 4);
        // Une boucle d'identifiants ne bloque pas.
        assert_eq!(parents_first(vec![(1, 2), (2, 1)]).len(), 2);
    }
}
