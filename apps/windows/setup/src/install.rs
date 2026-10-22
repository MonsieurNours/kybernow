//! Installation, pour l'utilisateur seul (rien dans HKLM ni Program Files) :
//! fermer l'app, poser ses fichiers, WebView2, raccourcis et lien
//! `kybernow://` (par l'app elle-même), inscription dans « Applications
//! installées ».
//!
//! Les fichiers se posent d'abord à côté (`KyberNow.new`), puis prennent la
//! place de l'ancienne version d'un coup : une mise à jour qui échoue laisse
//! l'ancienne app intacte.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::events::{Emit, Event, TaskStatus};
use crate::i18n::{self, Lang, TrContext, lang, tr};
use crate::{payload, sys, webview2};

pub const TASKS: [&str; 5] = ["close", "files", "webview", "shortcuts", "register"];

struct Run<'a> {
    emit: &'a Emit,
}

impl Run<'_> {
    /// Une ligne du déroulé : en français au journal, dans la langue de
    /// Windows à l'écran.
    fn say(&self, key: &str, params: &[(&str, &dyn std::fmt::Display)]) {
        sys::log(&tr(Lang::Fr, key, params));
        (self.emit)(Event::Log { line: tr(lang(), key, params) });
    }

    fn task(&self, id: &str, status: TaskStatus) {
        (self.emit)(Event::Task { id: id.into(), status });
    }

    fn progress(&self, percent: f32) {
        (self.emit)(Event::Progress { percent });
    }
}

/// Installe (ou met à jour) l'app. `desktop` : raccourci sur le Bureau.
/// `wait_app` : temps laissé à l'app pour se fermer d'elle-même (mise à jour).
pub fn run_install(setup: &Path, desktop: bool, wait_app: Duration, emit: &Emit) -> Result<()> {
    let run = Run { emit };
    sys::hold_setup_mutex();
    let arch = sys::native_arch();
    let dir = sys::install_dir();
    run.say("install.log.start", &[("version", &env!("CARGO_PKG_VERSION")), ("arch", &arch.label()), ("dir", &dir.display())]);

    let step = |id: &str, f: &dyn Fn() -> Result<TaskStatus>| -> Result<()> {
        run.task(id, TaskStatus::Running);
        match f() {
            Ok(status) => {
                run.task(id, status);
                Ok(())
            }
            Err(e) => {
                sys::log(&format!("échec ({id}) : {e:#}"));
                let message = i18n::describe(lang(), &e);
                (emit)(Event::Log { line: tr(lang(), "install.log.failed", &[("task", &id), ("error", &message)]) });
                run.task(id, TaskStatus::Failed);
                (emit)(Event::Failed { task: id.into(), message });
                Err(e)
            }
        }
    };

    step("close", &|| {
        if !sys::app_running() {
            return Ok(TaskStatus::Skipped);
        }
        run.say("install.log.closing", &[]);
        sys::close_app(wait_app)?;
        Ok(TaskStatus::Done)
    })?;
    run.progress(4.0);

    step("files", &|| {
        let count = place_files(setup, arch, &dir, &|p, name| {
            run.progress(4.0 + p * 80.0);
            if !name.is_empty() && name.ends_with(".exe") {
                (emit)(Event::Log { line: format!("  {name}") });
            }
        })?;
        run.say("install.log.files", &[("count", &count)]);
        Ok(TaskStatus::Done)
    })?;
    run.progress(84.0);

    step("webview", &|| {
        if let Some(v) = webview2::version() {
            run.say("install.log.webviewPresent", &[("version", &v)]);
            return Ok(TaskStatus::Skipped);
        }
        let v = webview2::install()?;
        run.say("install.log.webviewInstalled", &[("version", &v)]);
        Ok(TaskStatus::Done)
    })?;
    run.progress(92.0);

    // L'app pose elle-même lien kybernow://, notifications et raccourcis (COM).
    step("shortcuts", &|| {
        let mut args = vec!["--setup-shell"];
        if desktop {
            args.push("--desktop");
        }
        match run_app(&dir, &args, Duration::from_secs(20)) {
            Ok(()) => {
                // L'app a posé `KyberNow Link.lnk` : plus d'ancien `KyberNow.lnk`
                // (0.3.1 et avant), ni de raccourci sur le Bureau s'il est décoché.
                let mut gone = sys::old_shortcuts();
                if !desktop {
                    gone.extend(sys::desktop_shortcuts());
                }
                for link in gone {
                    let _ = std::fs::remove_file(link);
                }
                Ok(TaskStatus::Done)
            }
            Err(e) => {
                sys::log(&format!("raccourcis : {e:#}"));
                (emit)(Event::Log { line: tr(lang(), "install.log.shortcuts", &[("error", &i18n::describe(lang(), &e))]) });
                Ok(TaskStatus::Warn)
            }
        }
    })?;
    run.progress(96.0);

    step("register", &|| {
        register(&dir)?;
        Ok(TaskStatus::Done)
    })?;
    run.progress(100.0);
    run.say("install.log.done", &[]);
    (emit)(Event::Done);
    Ok(())
}

/// Pose les fichiers de la version `arch` dans `dir`, en remplaçant d'un coup
/// ceux d'une version précédente.
fn place_files(setup: &Path, arch: payload::Arch, dir: &Path, progress: &dyn Fn(f32, &str)) -> Result<u64> {
    let parent = dir.parent().context("dossier d'installation")?;
    std::fs::create_dir_all(parent)?;
    let new = parent.join("KyberNow.new");
    let old = parent.join("KyberNow.old");
    remove_dir(&new)?;
    let count = payload::extract(setup, arch, &new, |p, name| progress(p, name))?;
    payload::copy_stub(setup, &new.join(sys::UNINSTALLER)).context("copie du désinstalleur")?;

    // L'app relancée pendant l'installation tiendrait ses fichiers ouverts.
    sys::close_app(Duration::from_secs(3))?;
    let _ = remove_dir(&old);
    if dir.exists() && !retry(|| std::fs::rename(dir, &old)) {
        // Dossier verrouillé (explorateur ouvert dessus, dossier courant d'un
        // programme) : on échange fichier par fichier, avec retour arrière.
        sys::log("ancien dossier verrouillé : échange fichier par fichier");
        swap_files(&new, dir).tr_context("errors.replaceFiles", &[])?;
        // Désinstalleur de l'ancien installateur (NSIS), remplacé par uninstall.exe.
        let _ = std::fs::remove_file(dir.join("Desinstaller.exe"));
        let _ = remove_dir(&new);
        return Ok(count);
    }
    if !retry(|| std::fs::rename(&new, dir)) {
        // Remet l'ancienne version en place avant d'abandonner.
        if !dir.exists() && std::fs::rename(&old, dir).is_err() {
            return Err(i18n::err("errors.placeFilesOld", &[("dir", &old.display())]));
        }
        return Err(i18n::err("errors.placeFiles", &[("dir", &dir.display())]));
    }
    if old.exists() && remove_dir(&old).is_err() {
        sys::schedule_removal(&old);
    }
    Ok(count)
}

/// Échange les fichiers de `new` avec ceux de `dir`, un par un : chaque
/// ancien fichier est d'abord renommé (`.ancien`), puis le nouveau prend sa
/// place. Au moindre échec, tout revient comme avant.
fn swap_files(new: &Path, dir: &Path) -> Result<()> {
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let result = (|| -> Result<()> {
        for e in std::fs::read_dir(new)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                bail!("dossier inattendu dans l'app : {}", e.path().display());
            }
            let target = dir.join(e.file_name());
            let backup = target.exists().then(|| dir.join(format!("{}.ancien", e.file_name().to_string_lossy())));
            if let Some(b) = &backup {
                let _ = std::fs::remove_file(b);
                std::fs::rename(&target, b).tr_context("errors.locked", &[("path", &target.display())])?;
            }
            done.push((target.clone(), backup.clone()));
            std::fs::rename(e.path(), &target).with_context(|| format!("pose de {}", target.display()))?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        for (target, backup) in done.into_iter().rev() {
            let _ = std::fs::remove_file(&target);
            if let Some(b) = backup {
                let _ = std::fs::rename(&b, &target);
            }
        }
        return Err(e);
    }
    for (_, backup) in done {
        if let Some(b) = backup {
            // Encore ouvert : il partira avec la prochaine mise à jour ou la désinstallation.
            let _ = std::fs::remove_file(b);
        }
    }
    Ok(())
}

fn retry(mut f: impl FnMut() -> std::io::Result<()>) -> bool {
    // Jusqu'à 6 s : les processus WebView2 de l'app se ferment après elle.
    for i in 0..20 {
        match f() {
            Ok(()) => return true,
            Err(e) if i == 19 => sys::log(&format!("renommage refusé : {e}")),
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    false
}

fn remove_dir(dir: &Path) -> Result<()> {
    if std::fs::symlink_metadata(dir).is_ok() {
        std::fs::remove_dir_all(dir).with_context(|| format!("effacement de {}", dir.display()))?;
    }
    Ok(())
}

/// Lance `kybernow.exe` avec des options d'installation et attend sa fin.
pub fn run_app(dir: &Path, args: &[&str], timeout: Duration) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let mut child = std::process::Command::new(dir.join(sys::APP_EXE))
        .args(args)
        .current_dir(dir)
        .creation_flags(sys::CREATE_NO_WINDOW)
        .spawn()
        .with_context(|| format!("lancement de {} {}", sys::APP_EXE, args.join(" ")))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("{} {} : code {}", sys::APP_EXE, args.join(" "), status.code().unwrap_or(-1));
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            bail!("{} {} : pas de réponse", sys::APP_EXE, args.join(" "));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// « Applications installées » de Windows.
fn register(dir: &Path) -> Result<()> {
    let key = sys::UNINSTALL_KEY;
    let exe = dir.join(sys::APP_EXE).display().to_string();
    let uninstall = dir.join(sys::UNINSTALLER).display().to_string();
    sys::reg_set(key, Some("DisplayName"), sys::DISPLAY_NAME)?;
    sys::reg_set(key, Some("DisplayVersion"), env!("CARGO_PKG_VERSION"))?;
    sys::reg_set(key, Some("Publisher"), "Les Lignes Claires")?;
    sys::reg_set(key, Some("URLInfoAbout"), "https://www.kybernow.app")?;
    sys::reg_set(key, Some("DisplayIcon"), &format!("{exe},0"))?;
    sys::reg_set(key, Some("InstallLocation"), &dir.display().to_string())?;
    sys::reg_set(key, Some("UninstallString"), &format!("\"{uninstall}\" --uninstall"))?;
    sys::reg_set(key, Some("QuietUninstallString"), &format!("\"{uninstall}\" --uninstall /S"))?;
    sys::reg_set_dword(key, "NoModify", 1)?;
    sys::reg_set_dword(key, "NoRepair", 1)?;
    sys::reg_set_dword(key, "EstimatedSize", (dir_size(dir) / 1024) as u32)?;
    Ok(())
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| match e.file_type() {
                    Ok(t) if t.is_dir() => dir_size(&e.path()),
                    _ => e.metadata().map(|m| m.len()).unwrap_or(0),
                })
                .sum()
        })
        .unwrap_or(0)
}
