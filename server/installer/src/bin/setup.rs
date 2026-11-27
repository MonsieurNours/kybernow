//! `KyberNow-Station-Setup.exe` : lanceur de l'installation.
//!
//! Il porte le contenu à installer (accolé à la fin du fichier), dépose
//! l'interface et `WebView2Loader.dll` dans `C:\Program Files\KyberNow\.setup`,
//! puis lance l'interface. Avec `--uninstall`, il désinstalle KyberNow Station.
//! Avec `/quiet [/enroll=<jeton>] [/relay=<adresse>]`, il installe sans écran
//! (et sans WebView2), puis enrôle la Station (`kybernow::quiet`).
//!
//! Pas dans `%TEMP%` : l'interface y tournerait avec le jeton administrateur
//! depuis un dossier où tout programme de l'utilisateur peut écrire, et
//! charger une DLL déposée à côté d'elle (contournement de l'UAC). Sous
//! Program Files, seuls les administrateurs écrivent.
//!
//! Le dossier de données de WebView2 (profil, cache), lui, reste dans
//! `%TEMP%` : les processus que WebView2 isole (rendu, réseau) tournent avec
//! un jeton restreint qui ne peut pas écrire sous Program Files, et Edge
//! refuse alors de démarrer (« ne peut pas lire ni écrire dans son répertoire
//! de données »). Ce dossier ne contient aucun code exécuté par l'interface.
//! Son chemin est donné **en forme longue** : élevé avec le compte d'un autre
//! administrateur, `%TEMP%` arrive en forme courte (`C:\Users\USER_A~1\…`),
//! que les règles du bac à sable de WebView2 ne reconnaissent pas, et Edge
//! refuse de la même façon. Si `%TEMP%` ne se crée pas, `%LOCALAPPDATA%`
//! prend le relais (voir [`webview_data_dir`]).

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

use kybernow::i18n::t;

const UI_EXE: &[u8] = include_bytes!(env!("KYBERNOW_UI_EXE"));
const WEBVIEW2_LOADER: &[u8] = include_bytes!(env!("KYBERNOW_WEBVIEW2_LOADER"));

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--uninstall") {
        uninstall();
        return;
    }
    if let Some(quiet) = kybernow::quiet::parse_args(&args) {
        let code = match std::env::current_exe() {
            Ok(exe) if kybernow::payload::read_footer(&exe).is_ok() => kybernow::quiet::run_windows(&quiet, &exe),
            _ => kybernow::quiet::EXIT_INSTALL_FAILED,
        };
        std::process::exit(code);
    }
    if let Err(e) = launch() {
        message(kybernow::DISPLAY_NAME, &t("setup.startFailed", &[("error", &format!("{e:#}"))]), false);
    }
}

fn launch() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    if kybernow::payload::read_footer(&exe).is_err() {
        anyhow::bail!(t("setup.notAPayload", &[]));
    }
    if !webview2_installed() {
        message(kybernow::DISPLAY_NAME, &t("setup.webview2Missing", &[]), false);
        kybernow::sys::open_url("https://go.microsoft.com/fwlink/p/?LinkId=2124703");
        return Ok(());
    }

    let tmp: PathBuf = kybernow::sys::install_dir().join(".setup");
    // Reste d'une installation interrompue : effacé, jamais réutilisé.
    if std::fs::symlink_metadata(&tmp).is_ok() {
        std::fs::remove_dir_all(&tmp).or_else(|_| std::fs::remove_file(&tmp))?;
    }
    std::fs::create_dir_all(&tmp)?;
    let ui = tmp.join("kybernow-ui.exe");
    std::fs::write(&ui, UI_EXE)?;
    std::fs::write(tmp.join("WebView2Loader.dll"), WEBVIEW2_LOADER)?;

    let webview_data = webview_data_dir();
    let status = std::process::Command::new(&ui)
        .arg("--payload")
        .arg(&exe)
        .arg("--webview-data")
        .arg(&webview_data)
        .status();

    // WebView2 relâche ses fichiers un peu après la fermeture de l'interface.
    for dir in [&tmp, &webview_data] {
        for _ in 0..10 {
            if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
    }
    status?;
    Ok(())
}

/// Le dossier de données de WebView2 : dans `%TEMP%` du compte qui a élevé,
/// sinon dans `%LOCALAPPDATA%`, toujours en forme longue (sans `~1`), et créé
/// d'avance pour qu'un échec se voie ici plutôt que dans Edge.
fn webview_data_dir() -> PathBuf {
    let name = format!("KyberNow-Station-Setup-WebView-{}", std::process::id());
    let mut bases = vec![std::env::temp_dir()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        bases.push(local.join("Temp"));
        bases.push(local.join(kybernow::PRODUCT));
    }
    for base in &bases {
        let dir = long_path(base).join(&name);
        if std::fs::create_dir_all(&dir).is_ok() {
            return long_path(&dir);
        }
    }
    long_path(&std::env::temp_dir()).join(name)
}

/// Un chemin existant en forme longue (`GetLongPathNameW`) ; tel quel s'il
/// n'existe pas ou si Windows ne sait pas le développer.
#[cfg(windows)]
fn long_path(path: &std::path::Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; 1024];
    loop {
        let n = unsafe { GetLongPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            return path.to_path_buf();
        }
        if n < buf.len() {
            return PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]));
        }
        buf.resize(n + 1, 0);
    }
}

#[cfg(not(windows))]
fn long_path(path: &std::path::Path) -> PathBuf {
    path.to_path_buf()
}

fn uninstall() {
    let title = t("uninstall.title", &[]);
    if !message(&title, &t("uninstall.confirm", &[]), true) {
        return;
    }
    let keep_data = message(&title, &t("uninstall.keepData", &[]), true);
    let done = match kybernow::uninstall::run_uninstall(keep_data) {
        Ok(()) if keep_data => t("uninstall.doneKept", &[]),
        Ok(()) => t("uninstall.done", &[]),
        Err(e) => t("uninstall.failed", &[("error", &format!("{e:#}"))]),
    };
    message(kybernow::DISPLAY_NAME, &done, false);
    // Après la boîte : le dossier est effacé une fois ce programme fermé.
    kybernow::uninstall::schedule_removal();
}

fn webview2_installed() -> bool {
    if !cfg!(windows) {
        return true;
    }
    const KEY: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    [
        format!(r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{KEY}"),
        format!(r"HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{KEY}"),
        format!(r"HKCU\SOFTWARE\Microsoft\EdgeUpdate\Clients\{KEY}"),
    ]
    .iter()
    .any(|k| {
        kybernow::sys::run("reg", &["query", k, "/v", "pv"]).is_ok_and(|o| {
            o.status.success() && !String::from_utf8_lossy(&o.stdout).contains("0.0.0.0")
        })
    })
}

/// Boîte de dialogue Windows. Avec `ask`, renvoie `true` si l'utilisateur accepte.
#[cfg(windows)]
fn message(title: &str, text: &str, ask: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (t, m) = (wide(title), wide(text));
    let flags = if ask { MB_YESNO | MB_ICONQUESTION } else { MB_OK | MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), m.as_ptr(), t.as_ptr(), flags) == IDYES }
}

#[cfg(not(windows))]
fn message(title: &str, text: &str, ask: bool) -> bool {
    println!("[{title}] {text}");
    ask
}
