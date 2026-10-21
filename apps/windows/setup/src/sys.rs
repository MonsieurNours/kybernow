//! Windows : chemins de l'app, registre de l'utilisateur, processeur, journal.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW, RegSetValueExW,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, IsWow64Process2, OpenMutexW};

use crate::payload::Arch;

pub const PRODUCT: &str = "KyberNow";
/// Mutex de l'app ouverte (apps/windows/src/instance.rs).
const APP_MUTEX: &str = r"Local\KyberNow.App";
/// Tenu pendant l'installation : l'app rouverte entre-temps attend sa fin.
const SETUP_MUTEX: &str = r"Local\KyberNow.Setup";
pub const APP_EXE: &str = "kybernow.exe";
pub const UNINSTALLER: &str = "uninstall.exe";
pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\KyberNow";
const SYNCHRONIZE: u32 = 0x0010_0000;
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

/// `%LOCALAPPDATA%\Programs\KyberNow` : par utilisateur, sans droits d'administrateur.
pub fn install_dir() -> PathBuf {
    env_dir("LOCALAPPDATA").join("Programs").join(PRODUCT)
}

/// Données de l'app : ordinateurs et réglages, journaux, profil de WebView2.
pub fn data_dirs() -> [PathBuf; 3] {
    [
        env_dir("APPDATA").join(PRODUCT),
        env_dir("LOCALAPPDATA").join(PRODUCT),
        env_dir("LOCALAPPDATA").join("fr.leslignesclaires.kybernow"),
    ]
}

/// Raccourci du menu Démarrer (posé par l'app, `--setup-shell`).
pub fn start_menu_shortcut() -> PathBuf {
    env_dir("APPDATA").join(r"Microsoft\Windows\Start Menu\Programs\KyberNow.lnk")
}

/// Raccourci du Bureau (le Bureau peut être déplacé, dans OneDrive par exemple).
pub fn desktop_shortcut() -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Desktop, SHGetKnownFolderPath};
    unsafe {
        let mut p: *mut u16 = std::ptr::null_mut();
        if SHGetKnownFolderPath(&FOLDERID_Desktop, 0, std::ptr::null_mut(), &mut p) != 0 || p.is_null() {
            return None;
        }
        let len = (0..).take_while(|&i| *p.add(i) != 0).count();
        let dir = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p.cast());
        Some(PathBuf::from(dir).join("KyberNow.lnk"))
    }
}

/// Processeur de Windows, même quand cet installateur (x64) tourne émulé sur ARM.
pub fn native_arch() -> Arch {
    const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;
    let (mut process, mut native) = (0u16, 0u16);
    if unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut native) } != 0 && native != 0 {
        return if native == IMAGE_FILE_MACHINE_ARM64 { Arch::Arm64 } else { Arch::X64 };
    }
    // Avant Windows 10 1709 : la variable du système, pas celle du processus.
    match reg_get(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment", "PROCESSOR_ARCHITECTURE") {
        Some(a) if a.eq_ignore_ascii_case("ARM64") => Arch::Arm64,
        _ => Arch::X64,
    }
}

/// Programme de Windows par son chemin complet : `Command::new("cmd")`
/// chercherait d'abord à côté de l'installateur (dans Téléchargements).
pub fn system32(exe: &str) -> String {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32")
        .join(exe)
        .display()
        .to_string()
}

/// Signale une installation en cours jusqu'à la fin de ce processus.
pub fn hold_setup_mutex() {
    use windows_sys::Win32::System::Threading::CreateMutexW;
    // Le descripteur reste ouvert : Windows le ferme avec le processus.
    unsafe {
        CreateMutexW(std::ptr::null(), 0, wide(SETUP_MUTEX).as_ptr());
    }
}

/// L'app est-elle ouverte (pour cet utilisateur, dans cette session) ?
pub fn app_running() -> bool {
    unsafe {
        let h = OpenMutexW(SYNCHRONIZE, 0, wide(APP_MUTEX).as_ptr());
        if h.is_null() {
            return false;
        }
        CloseHandle(h);
        true
    }
}

/// Attend que l'app se ferme, puis la ferme si elle est encore là.
pub fn close_app(wait: Duration) -> Result<()> {
    let started = Instant::now();
    while app_running() && started.elapsed() < wait {
        std::thread::sleep(Duration::from_millis(200));
    }
    if !app_running() {
        return Ok(());
    }
    log("l'app est encore ouverte : fermeture forcée");
    let _ = run(&system32("taskkill.exe"), &["/IM", APP_EXE, "/F"]);
    let started = Instant::now();
    while app_running() && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(200));
    }
    if app_running() {
        bail!("KyberNow est encore ouverte et refuse de se fermer. Fermez-la, puis réessayez.");
    }
    // Les fichiers se libèrent un peu après la fin du processus.
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

/// Lance une commande sans fenêtre et renvoie sa sortie.
pub fn run(program: &str, args: &[&str]) -> Result<Output> {
    use std::os::windows::process::CommandExt;
    Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .with_context(|| format!("lancement de {program}"))
}

/// Ouvre une page web dans le navigateur.
pub fn open_url(url: &str) {
    if url.starts_with("https://") {
        let _ = Command::new(system32("cmd.exe")).args(["/C", "start", "", url]).spawn();
    }
}

/// Lance l'app installée, détachée de l'installateur.
pub fn launch_app(args: &[&str]) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let dir = install_dir();
    Command::new(dir.join(APP_EXE))
        .args(args)
        .current_dir(&dir)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .context("ouverture de KyberNow")?;
    Ok(())
}

// --- Registre -----------------------------------------------------------------

/// Valeur texte, ou `None`.
pub fn reg_get(root: HKEY, path: &str, name: &str) -> Option<String> {
    unsafe {
        let (p, n) = (wide(path), wide(name));
        let mut len = 0u32;
        if RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut len) != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u16; (len as usize).div_ceil(2) + 1];
        let mut len = (buf.len() * 2) as u32;
        if RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) != ERROR_SUCCESS {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

pub fn reg_get_dword(root: HKEY, path: &str, name: &str) -> Option<u32> {
    unsafe {
        let (p, n) = (wide(path), wide(name));
        let (mut v, mut len) = (0u32, 4u32);
        (RegGetValueW(root, p.as_ptr(), n.as_ptr(), RRF_RT_REG_DWORD, std::ptr::null_mut(), (&mut v as *mut u32).cast(), &mut len) == ERROR_SUCCESS)
            .then_some(v)
    }
}

fn reg_set_raw(path: &str, name: Option<&str>, kind: u32, data: &[u8]) -> Result<()> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let r = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if r != ERROR_SUCCESS {
            bail!("registre : HKCU\\{path} refusé ({r})");
        }
        let n = name.map(wide);
        let r = RegSetValueExW(key, n.as_ref().map_or(std::ptr::null(), |n| n.as_ptr()), 0, kind, data.as_ptr(), data.len() as u32);
        RegCloseKey(key);
        if r != ERROR_SUCCESS {
            bail!("registre : HKCU\\{path} {} refusé ({r})", name.unwrap_or("(défaut)"));
        }
        Ok(())
    }
}

/// Écrit une valeur texte sous HKCU.
pub fn reg_set(path: &str, name: Option<&str>, value: &str) -> Result<()> {
    let w = wide(value);
    reg_set_raw(path, name, REG_SZ, unsafe { std::slice::from_raw_parts(w.as_ptr().cast(), w.len() * 2) })
}

pub fn reg_set_dword(path: &str, name: &str, value: u32) -> Result<()> {
    reg_set_raw(path, Some(name), REG_DWORD, &value.to_le_bytes())
}

/// Efface une clé de HKCU et tout ce qu'elle contient.
pub fn reg_delete(path: &str) {
    unsafe {
        RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr());
    }
}

/// Noms des sous-clés.
pub fn reg_subkeys(root: HKEY, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) != ERROR_SUCCESS {
            return out;
        }
        for i in 0.. {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            let r = RegEnumKeyExW(key, i, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            if r != ERROR_SUCCESS {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        RegCloseKey(key);
    }
    out
}

/// Version de KyberNow déjà installée pour cet utilisateur.
pub fn installed_version() -> Option<String> {
    reg_get(HKEY_CURRENT_USER, UNINSTALL_KEY, "DisplayVersion")
}

// --- Journal ------------------------------------------------------------------

static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// `%LOCALAPPDATA%\KyberNow\<name>.log` (la précédente dans `<name>.prev.log`) :
/// `setup` pour le lanceur (mise à jour, désinstallation), `setup-ui` pour l'interface.
pub fn init_log(name: &str) {
    let dir = env_dir("LOCALAPPDATA").join(PRODUCT);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{name}.log"));
    let _ = std::fs::rename(&path, dir.join(format!("{name}.prev.log")));
    *LOG.lock().unwrap() = std::fs::File::create(path).ok();
    log(&format!("installateur de KyberNow {}, Windows {}", env!("CARGO_PKG_VERSION"), native_arch().label()));
}

/// Ferme le journal (avant d'effacer son dossier).
pub fn close_log() {
    *LOG.lock().unwrap() = None;
}

pub fn log(line: &str) {
    if let Some(f) = LOG.lock().unwrap().as_mut() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{t} {line}");
        let _ = f.flush();
    }
}

/// Boîte de dialogue Windows. Avec `ask`, renvoie `true` si l'utilisateur accepte.
pub fn message(title: &str, text: &str, ask: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_SETFOREGROUND, MB_YESNO, MessageBoxW};
    let flags = if ask { MB_YESNO | MB_ICONQUESTION } else { MB_OK | MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide(title).as_ptr(), flags | MB_SETFOREGROUND) == IDYES }
}

/// Efface un dossier une fois ce programme fermé (le désinstalleur est dedans).
pub fn schedule_removal(dir: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    // Ligne passée telle quelle : les arguments de Rust échapperaient les
    // guillemets en \", que cmd ne comprend pas.
    let script = format!("/C ping 127.0.0.1 -n 4 > nul & rmdir /s /q \"{}\"", dir.display());
    let _ = Command::new(system32("cmd.exe"))
        .raw_arg(script)
        .current_dir(std::env::temp_dir())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
}
