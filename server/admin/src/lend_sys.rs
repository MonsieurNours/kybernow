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
    "cmd.exe",
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
    "msiexec.exe",
    "rundll32.exe",
    "mshta.exe",
    "wscript.exe",
    "cscript.exe",
];

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
    /// et les programmes de Windows ; le reste est fermé.
    pub fn judge(&self, path: &str, parent_allowed: bool) -> Verdict {
        let path = path.to_ascii_lowercase().replace('/', "\\");
        let name = path.rsplit('\\').next().unwrap_or(&path);
        if DENIED.contains(&name) {
            return Verdict::Deny;
        }
        let allowed = parent_allowed
            || self.apps.contains(&path)
            || self.dirs.iter().any(|d| path.starts_with(d.as_str()))
            || path.contains("\\steamapps\\")
            || (!self.system.is_empty() && path.starts_with(self.system.as_str()));
        if allowed { Verdict::Allow } else { Verdict::Deny }
    }
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
    if let Some(steam) = steam {
        dirs.push(dir_key(&steam));
    }
    let system = std::env::var_os("SystemRoot").map(|r| dir_key(Path::new(&r))).unwrap_or_else(|| "c:\\windows\\".into());
    Rules { apps: apps.iter().map(|a| a.to_ascii_lowercase().replace('/', "\\")).collect(), dirs, system }
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

pub use platform::{host_name, idle_secs, keep_awake, lock_session, session_state, spawn_console, steam_dir, watch};

#[cfg(windows)]
mod platform {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Result, bail};

    use super::{Rules, SessionState, Verdict, Watch};
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
            let mut allowed: HashSet<u32> = HashSet::new();
            while !stop2.load(Ordering::SeqCst) {
                for (pid, ppid) in snapshot() {
                    if !seen.insert(pid) || session_of(pid) != Some(session) {
                        continue;
                    }
                    let Some(path) = image_path(pid) else { continue };
                    match rules.judge(&path, allowed.contains(&ppid)) {
                        Verdict::Allow => {
                            allowed.insert(pid);
                            started2.lock().unwrap().insert(pid);
                        }
                        Verdict::Deny => {
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

    /// Ferme ce qui a été ouvert pendant le prêt, hors KyberNow, Kyber et Windows.
    pub fn close_started(started: &[u32], rules: &Rules) -> usize {
        let keep = |path: &str| {
            let p = path.to_ascii_lowercase();
            (!rules.system.is_empty() && p.starts_with(rules.system.as_str())) || rules.dirs.first().is_some_and(|d| p.starts_with(d.as_str())) || rules.dirs.get(1).is_some_and(|d| p.starts_with(d.as_str()))
        };
        let mut closed = 0;
        for pid in started {
            if let Some(path) = image_path(*pid)
                && !keep(&path)
                && terminate(*pid)
            {
                closed += 1;
            }
        }
        closed
    }
}

#[cfg(not(windows))]
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
        }
    }

    #[test]
    fn ce_qui_est_permis_pendant_un_pret() {
        let rules = r();
        for (path, parent, verdict) in [
            ("D:\\Jeux\\Celeste\\Celeste.exe", false, Verdict::Allow),
            ("D:\\Jeux\\Celeste\\crash.exe", true, Verdict::Allow),
            ("C:\\Program Files (x86)\\Steam\\steam.exe", false, Verdict::Allow),
            ("E:\\SteamLibrary\\steamapps\\common\\Hades\\Hades.exe", false, Verdict::Allow),
            ("C:\\Program Files\\KyberNow\\tray\\kybernow-tray.exe", false, Verdict::Allow),
            ("C:\\Windows\\System32\\conhost.exe", true, Verdict::Allow),
            ("C:\\Windows\\System32\\dllhost.exe", false, Verdict::Allow),
            // Refusés, même enfants d'une app permise.
            ("C:\\Windows\\explorer.exe", true, Verdict::Deny),
            ("C:\\Windows\\System32\\Taskmgr.exe", false, Verdict::Deny),
            ("C:\\Windows\\System32\\cmd.exe", true, Verdict::Deny),
            ("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe", false, Verdict::Deny),
            ("C:\\Program Files\\PowerShell\\7\\pwsh.exe", false, Verdict::Deny),
            ("C:\\Program Files\\WindowsApps\\Microsoft.WindowsTerminal\\WindowsTerminal.exe", false, Verdict::Deny),
            ("C:\\Windows\\regedit.exe", false, Verdict::Deny),
            ("C:\\Windows\\System32\\mmc.exe", false, Verdict::Deny),
            ("C:\\Windows\\ImmersiveControlPanel\\SystemSettings.exe", false, Verdict::Deny),
            // Une app hors liste.
            ("C:\\Program Files\\Mozilla Firefox\\firefox.exe", false, Verdict::Deny),
            ("C:\\Users\\matthieu\\Downloads\\outil.exe", false, Verdict::Deny),
        ] {
            assert_eq!(rules.judge(path, parent), verdict, "{path}");
        }
    }
}
