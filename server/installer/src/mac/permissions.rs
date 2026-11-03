//! Les trois autorisations de macOS : enregistrement de l'écran,
//! accessibilité, micro (lecture du son de la session).
//!
//! macOS les rattache au processus « responsable » et au paquet qui contient
//! son exécutable. L'installateur ne peut donc ni les lire ni les demander
//! lui-même : il lance l'exécutable de l'app installée (`KyberNow Station
//! --permissions …`) en se désignant non responsable
//! (`responsibility_spawnattrs_setdisclaim`, comme launchd le fait pour
//! Kyber). Vérifié le 03/11/2026 : un exécutable du paquet de Kyber, lancé
//! ainsi, voit les autorisations du paquet ; lancé simplement, il voit
//! celles de l'installateur.

use std::ffi::CString;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};

use super::commands::{Event, Mic, Which};

unsafe extern "C" {
    fn responsibility_spawnattrs_setdisclaim(attr: *mut libc::posix_spawnattr_t, disclaim: libc::c_int) -> libc::c_int;
}

/// Lance `exe args` comme son propre responsable ; rend son pid.
fn spawn_disclaimed(exe: &Path, args: &[&str]) -> Result<libc::pid_t> {
    let program = CString::new(exe.to_string_lossy().as_bytes())?;
    let mut owned = vec![program.clone()];
    for a in args {
        owned.push(CString::new(*a)?);
    }
    let mut argv: Vec<*mut libc::c_char> = owned.iter().map(|a| a.as_ptr() as *mut libc::c_char).collect();
    argv.push(std::ptr::null_mut());
    unsafe extern "C" {
        static environ: *const *mut libc::c_char;
    }
    unsafe {
        let mut attr: libc::posix_spawnattr_t = std::ptr::null_mut();
        if libc::posix_spawnattr_init(&mut attr) != 0 {
            bail!("posix_spawnattr_init");
        }
        responsibility_spawnattrs_setdisclaim(&mut attr, 1);
        let mut pid: libc::pid_t = 0;
        let rc = libc::posix_spawn(&mut pid, program.as_ptr(), std::ptr::null(), &attr, argv.as_ptr(), environ);
        libc::posix_spawnattr_destroy(&mut attr);
        if rc != 0 {
            bail!("posix_spawn {} : {}", exe.display(), std::io::Error::from_raw_os_error(rc));
        }
        Ok(pid)
    }
}

fn wait(pid: libc::pid_t) {
    let mut status = 0;
    unsafe {
        libc::waitpid(pid, &mut status, 0);
    }
}

fn helper(app: &Path) -> std::path::PathBuf {
    super::app_exe(app, super::MENUBAR_EXE)
}

/// État des trois autorisations pour l'app installée.
pub fn check(app: &Path) -> Result<Event> {
    let out = std::env::temp_dir().join(format!("kybernow-permissions-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let pid = spawn_disclaimed(&helper(app), &["--permissions", "check", "--out", &out.to_string_lossy()])?;
    wait(pid);
    let text = std::fs::read(&out)?;
    let _ = std::fs::remove_file(&out);
    let v: serde_json::Value = serde_json::from_slice(&text)?;
    let microphone = match v["microphone"].as_str() {
        Some("granted") => Mic::Granted,
        Some("denied") => Mic::Denied,
        _ => Mic::Undetermined,
    };
    Ok(Event::Permissions { screen: v["screen"].as_bool().unwrap_or(false), accessibility: v["accessibility"].as_bool().unwrap_or(false), microphone })
}

/// Demande une autorisation : la fenêtre de macOS (ou le réglage s'il a déjà
/// été refusé). Le processus d'aide reste en vie le temps de la réponse.
pub fn request(app: &Path, which: Which) -> Result<()> {
    let pid = spawn_disclaimed(&helper(app), &["--permissions", "request", which.arg()])?;
    // Ramassé quand il se termine (réponse donnée, ou au plus 10 minutes).
    std::thread::spawn(move || wait(pid));
    Ok(())
}

/// Kyber et Admin relancés : ils prennent les autorisations tout juste données.
pub fn apply() {
    for label in [super::LABEL_KYBER, super::LABEL_ADMIN] {
        if super::loaded(label) {
            let _ = super::launchctl(&["kickstart", "-k", &format!("{}/{label}", super::domain())]);
        }
    }
    std::thread::sleep(Duration::from_millis(300));
}
