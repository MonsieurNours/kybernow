//! Applicateur des mises à jour sur une Station Mac (docs/station-macos.md,
//! jalon 6).
//!
//! Admin a téléchargé `KyberNow-Station-Mac-update.zip` et l'a vérifié contre
//! le manifeste signé (taille, empreinte). Ce programme (une copie
//! d'Admin, lancée par launchd en service à part : launchd arrêterait avec
//! Admin tout ce qu'Admin lance lui-même) :
//!
//! 1. extrait l'app (`ditto`) et revérifie sa signature : paquet intact,
//!    identifiant `fr.leslignesclaires.kybernow.station`, même équipe que
//!    l'app installée, version attendue ;
//! 2. arrête l'icône, Admin et Kyber (LaunchAgents de la session) ;
//! 3. met l'app en place par renommage, l'ancienne gardée dans
//!    `update/previous` (« Revenir à la version précédente ») ;
//! 4. relance tout, et attend qu'Admin réponde dans la nouvelle version
//!    (60 s), sinon remet l'ancienne.
//!
//! Même signature d'une version à l'autre : macOS garde les autorisations,
//! et rien ne demande le mot de passe du Mac. La sortie son n'est pas
//! remplacée (il faudrait ce mot de passe) : une sortie plus récente dans
//! l'app est notée au journal et posée à la prochaine réinstallation.
//! Résultat dans `update/resultat.json`, au même format que sous Windows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::apply::{Action, HEALTH_TIMEOUT, Journal, Layout};
use super::{RESULT, valid_version};

pub const BUNDLE_ID: &str = "fr.leslignesclaires.kybernow.station";
pub const APP_NAME: &str = "KyberNow Station.app";
/// Service launchd de l'applicateur (dans la session, comme la Station).
pub const LABEL: &str = "fr.leslignesclaires.kybernow.station.update";
const LABELS: [&str; 3] = [
    "fr.leslignesclaires.kybernow.station.menubar",
    "fr.leslignesclaires.kybernow.station.admin",
    "fr.leslignesclaires.kybernow.station.kyber",
];

/// L'app qui contient cet Admin (`…/KyberNow Station.app/Contents/MacOS/kyber-admin`).
pub fn installed_app() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let contents = exe.parent()?.parent()?;
            (contents.file_name()? == "Contents").then(|| contents.parent().map(Path::to_path_buf)).flatten()
        })
        .unwrap_or_else(|| PathBuf::from("/Applications").join(APP_NAME))
}

fn previous_app(layout: &Layout) -> PathBuf {
    layout.updates().join("previous").join(APP_NAME)
}

/// Version d'une app (`CFBundleShortVersionString`).
pub fn app_version(app: &Path) -> Option<String> {
    let out = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    valid_version(&v).then_some(v)
}

/// Version gardée pour « Revenir à la version précédente ».
pub fn previous_version(layout: &Layout) -> Option<String> {
    app_version(&previous_app(layout))
}

fn domain() -> String {
    use std::os::unix::fs::MetadataExt;
    format!("gui/{}", fs::metadata("/dev/console").map(|m| m.uid()).unwrap_or(0))
}

fn launchctl(args: &[&str]) -> std::process::Output {
    Command::new("/bin/launchctl").args(args).output().unwrap_or_else(|_| std::process::Output {
        status: std::process::ExitStatus::default(),
        stdout: Vec::new(),
        stderr: Vec::new(),
    })
}

fn loaded(label: &str) -> bool {
    launchctl(&["print", &format!("{}/{label}", domain())]).status.success()
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn plist(label: &str) -> PathBuf {
    home().join("Library/LaunchAgents").join(format!("{label}.plist"))
}

fn stop_all(log: &mut Journal) {
    for label in LABELS {
        if loaded(label) {
            log.line(format!("    arrêt de {label}"));
            launchctl(&["bootout", &format!("{}/{label}", domain())]);
            let deadline = Instant::now() + Duration::from_secs(15);
            while loaded(label) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
}

fn start_all(log: &mut Journal) {
    for label in LABELS.iter().rev() {
        let path = plist(label);
        if !path.is_file() {
            continue;
        }
        if !loaded(label) {
            let out = launchctl(&["bootstrap", &domain(), &path.to_string_lossy()]);
            if !out.status.success() {
                log.line(format!("    {label} : {}", String::from_utf8_lossy(&out.stderr).trim()));
            }
        }
        launchctl(&["kickstart", &format!("{}/{label}", domain())]);
        log.line(format!("    {label} lancé"));
    }
}

/// `TeamIdentifier` et `Identifier` de la signature d'un paquet, après
/// vérification complète (`codesign --verify --deep --strict`).
fn signature(app: &Path) -> Result<(String, String)> {
    let verify = Command::new("/usr/bin/codesign").args(["--verify", "--deep", "--strict"]).arg(app).output()?;
    if !verify.status.success() {
        bail!("signature invalide : {}", String::from_utf8_lossy(&verify.stderr).trim());
    }
    let info = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(app).output()?;
    let text = String::from_utf8_lossy(&info.stderr).to_string();
    let field = |name: &str| text.lines().find_map(|l| l.strip_prefix(name).map(str::to_string)).unwrap_or_default();
    Ok((field("TeamIdentifier="), field("Identifier=")))
}

/// Admin répond-il, dans la version attendue ?
fn health(address: &str, version: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        // Certificat d'Admin, auto-signé : seule la version compte ici.
        .tls_config(ureq::tls::TlsConfig::builder().disable_verification(true).build())
        .http_status_as_error(false)
        .proxy(None)
        .build()
        .into();
    let url = format!("https://{address}/api/session");
    let mut last = String::from("aucune réponse");
    while Instant::now() < deadline {
        match agent.get(&url).call() {
            Ok(mut resp) => {
                let body: Value = resp.body_mut().read_to_vec().ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
                match body["version"].as_str() {
                    Some(v) if v == version => return Ok(()),
                    Some(v) => last = format!("Admin répond en version {v}"),
                    None => last = "réponse sans version".into(),
                }
            }
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    bail!("KyberNow Admin ne répond pas en version {version} au bout de {} s ({last})", timeout.as_secs())
}

fn write_result(layout: &Layout, value: Value) {
    let _ = fs::create_dir_all(layout.updates());
    let _ = fs::write(layout.updates().join(RESULT), serde_json::to_vec_pretty(&value).unwrap_or_default());
}

/// Version dans `kybernow.json` de la Station (dossier parent des données d'Admin).
fn record_version(layout: &Layout, version: &str) {
    let Some(path) = layout.data.parent().map(|d| d.join("kybernow.json")) else { return };
    let Ok(bytes) = fs::read(&path) else { return };
    let Ok(mut record) = serde_json::from_slice::<Value>(&bytes) else { return };
    record["version"] = json!(version);
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, serde_json::to_vec_pretty(&record).unwrap_or_default()).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

fn remove(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).with_context(|| format!("suppression de {}", path.display())),
        _ => Ok(()),
    }
}

/// Met `incoming` à la place de l'app installée, l'ancienne dans `previous`,
/// relance la Station et vérifie qu'Admin répond en `version` ; sinon, remet
/// l'ancienne. Rend l'erreur et si le retour arrière a réussi.
fn swap_and_check(layout: &Layout, incoming: &Path, version: &str, log: &mut Journal) -> Result<()> {
    let app = &layout.install;
    let previous = previous_app(layout);
    let parent = app.parent().context("dossier de l'app")?;
    // /Applications appartient à root, groupe admin : un compte standard ne peut pas y renommer.
    let probe = parent.join(format!(".kybernow-ecriture-{}", std::process::id()));
    fs::write(&probe, b"").with_context(|| format!("{} n'accepte pas d'écriture pour ce compte", parent.display()))?;
    let _ = fs::remove_file(&probe);

    log.line("==> Arrêt de la Station");
    stop_all(log);

    log.line("==> Bascule");
    fs::create_dir_all(previous.parent().unwrap())?;
    remove(&previous)?;
    fs::rename(app, &previous).with_context(|| format!("{} → {}", app.display(), previous.display()))?;
    if let Err(e) = fs::rename(incoming, app) {
        let _ = fs::rename(&previous, app);
        start_all(log);
        return Err(e).with_context(|| format!("{} → {}", incoming.display(), app.display()));
    }
    log.line(format!("    {} en place, l'ancienne dans {}", app.display(), previous.display()));

    log.line("==> Démarrage de la Station");
    start_all(log);
    let address = super::apply::console_address(layout);
    log.line(format!("==> Contrôle : Admin en version {version} sur {address}"));
    match health(&address, version, HEALTH_TIMEOUT) {
        Ok(()) => Ok(()),
        Err(e) => {
            log.line(format!("ERREUR : {e:#}"));
            log.line("==> Retour à la version qui tournait");
            stop_all(log);
            let failed = layout.updates().join("failed").join(APP_NAME);
            let _ = remove(&failed);
            let _ = fs::create_dir_all(failed.parent().unwrap());
            fs::rename(app, &failed).and_then(|_| fs::rename(&previous, app)).context("retour arrière")?;
            start_all(log);
            Err(e.context("version remise en place"))
        }
    }
}

fn apply(layout: &Layout, zip: &Path, version: &str) -> Result<()> {
    let mut log = Journal::open(layout.updates().join(version).join("update.log"));
    log.line(format!("==> Mise à jour de KyberNow Station vers {version} (Mac)"));
    let result = (|| -> Result<()> {
        if !valid_version(version) {
            bail!("version inattendue : {version}");
        }
        let staging = layout.updates().join(version).join("staging");
        remove(&staging)?;
        fs::create_dir_all(&staging)?;
        log.line(format!("==> Extraction de {}", zip.display()));
        let out = Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(zip).arg(&staging).output()?;
        if !out.status.success() {
            bail!("extraction : {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        let incoming = staging.join(APP_NAME);
        log.line("==> Signature");
        let (team, id) = signature(&incoming)?;
        let (installed_team, _) = signature(&layout.install).unwrap_or_default();
        if id != BUNDLE_ID {
            bail!("identifiant inattendu : {id}");
        }
        if team.is_empty() || (!installed_team.is_empty() && team != installed_team) {
            bail!("équipe de signature inattendue : « {team} » (app installée : « {installed_team} »)");
        }
        let found = app_version(&incoming).unwrap_or_default();
        if found != version {
            bail!("l'app du paquet est en version « {found} », pas {version}");
        }
        log.line(format!("    {BUNDLE_ID}, équipe {team}, version {version}"));
        // Sortie son : la poser demanderait le mot de passe du Mac.
        let driver = incoming.join("Contents/Resources/KyberNow.driver");
        let installed_driver = Path::new("/Library/Audio/Plug-Ins/HAL/KyberNow.driver");
        if installed_driver.exists()
            && !Command::new("/usr/bin/diff").args(["-rq"]).arg(driver.join("Contents/MacOS")).arg(installed_driver.join("Contents/MacOS")).output().is_ok_and(|o| o.status.success())
        {
            log.line("    sortie son plus récente dans l'app : elle sera posée à la prochaine réinstallation");
        }
        swap_and_check(layout, &incoming, version, &mut log)
    })();
    finish(layout, version, result, &mut log)
}

fn rollback(layout: &Layout) -> Result<()> {
    let previous = previous_app(layout);
    let version = app_version(&previous).context("aucune version précédente gardée")?;
    let mut log = Journal::open(layout.updates().join(&version).join("update.log"));
    log.line(format!("==> Retour à KyberNow Station {version} (Mac)"));
    let staging = layout.updates().join(&version).join("staging");
    let result = (|| -> Result<()> {
        remove(&staging)?;
        fs::create_dir_all(&staging)?;
        let incoming = staging.join(APP_NAME);
        fs::rename(&previous, &incoming).context("version précédente")?;
        swap_and_check(layout, &incoming, &version, &mut log)
    })();
    finish(layout, &version, result, &mut log)
}

fn finish(layout: &Layout, version: &str, result: Result<()>, log: &mut Journal) -> Result<()> {
    match &result {
        Ok(()) => {
            record_version(layout, version);
            write_result(layout, json!({ "version": version, "ok": true, "log": log.path }));
            log.line(format!("Terminé : KyberNow Station {version}."));
        }
        Err(e) => {
            let message = format!("{e:#}");
            let (rolled_back, undo_failed) = (message.contains("version remise en place"), message.contains("retour arrière"));
            log.line(format!("ERREUR : {e:#}"));
            // Arrêtée en route avant la bascule : la Station n'a pas bougé, ou a été relancée.
            if !LABELS.iter().any(|l| loaded(l)) {
                start_all(log);
            }
            write_result(
                layout,
                json!({ "version": version, "ok": false, "rolled_back": rolled_back, "undo_failed": undo_failed, "message": message, "log": log.path }),
            );
        }
    }
    result
}

pub fn run(action: Action, data_dir: &Path) -> Result<()> {
    let install = match &action {
        Action::Apply { install_dir, .. } | Action::Rollback { install_dir } => install_dir.clone().unwrap_or_else(installed_app),
    };
    let layout = Layout { install, data: data_dir.to_path_buf() };
    match action {
        Action::Apply { zip, version, .. } => apply(&layout, &zip, &version),
        Action::Rollback { .. } => rollback(&layout),
    }
}

/// Lance l'applicateur en service launchd à part (`LABEL`), depuis une copie
/// d'Admin : launchd arrêterait avec Admin un processus qu'Admin lance.
pub fn launch(layout: &Layout, log_dir: &Path, args: &[std::ffi::OsString]) -> Result<()> {
    let dir = layout.updates();
    fs::create_dir_all(&dir)?;
    let applicator = dir.join("kyber-admin-maj");
    let exe = std::env::current_exe().context("chemin d'Admin")?;
    let _ = fs::remove_file(&applicator);
    fs::copy(&exe, &applicator).with_context(|| format!("copie d'Admin dans {}", applicator.display()))?;
    let xml = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut program = vec![applicator.display().to_string()];
    program.extend(args.iter().map(|a| a.to_string_lossy().into_owned()));
    let arguments: String = program.iter().map(|a| format!("<string>{}</string>", xml(a))).collect();
    let output = xml(&log_dir.join("lancement.log").display().to_string());
    let plist = dir.join(format!("{LABEL}.plist"));
    fs::write(
        &plist,
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key><string>{LABEL}</string>\n    <key>ProgramArguments</key><array>{arguments}</array>\n    <key>StandardOutPath</key><string>{output}</string>\n    <key>StandardErrorPath</key><string>{output}</string>\n    <key>RunAtLoad</key><true/>\n    <key>LaunchOnlyOnce</key><true/>\n    <key>AssociatedBundleIdentifiers</key><array><string>{BUNDLE_ID}</string></array>\n</dict>\n</plist>\n"
        ),
    )?;
    if loaded(LABEL) {
        launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
    }
    let out = launchctl(&["bootstrap", &domain(), &plist.to_string_lossy()]);
    if !out.status.success() {
        bail!("launchctl bootstrap : {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    tracing::info!("mise à jour : applicateur lancé ({LABEL})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_of_the_built_app() {
        // Un Info.plist minimal : seule la version est lue.
        let dir = std::env::temp_dir().join(format!("kn-mac-app-{}", std::process::id()));
        let app = dir.join(APP_NAME);
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleShortVersionString</key><string>1.2.3</string></dict></plist>",
        )
        .unwrap();
        assert_eq!(app_version(&app).as_deref(), Some("1.2.3"));
        let layout = Layout { install: app.clone(), data: dir.join("admin") };
        assert_eq!(previous_version(&layout), None);
        let _ = fs::remove_dir_all(&dir);
    }
}
