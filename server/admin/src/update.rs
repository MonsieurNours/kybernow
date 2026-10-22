//! Mises à jour en un clic, depuis les versions publiées sur GitHub
//! (github.com/MonsieurNours/kybernow, dépôt public : aucun jeton).
//!
//! La console regarde la dernière version toutes les 6 heures (ou à la
//! demande), sur le canal choisi (`update_channel` : stable, ou bêta avec les
//! préversions). Une version ne s'installe que si son manifeste
//! (`kybernow-release.json`) est signé par la clé de KyberNow compilée ici,
//! annonce le même numéro que GitHub et un canal suivi, et accepte la version
//! installée (voir `manifest`). « Installer » télécharge `kybernow-update.zip`,
//! vérifie sa taille et son empreinte contre le manifeste, puis lance
//! l'applicateur (`kyber-admin update apply`, voir `apply`), détaché, depuis
//! une copie d'elle-même : il revérifie tout, extrait le paquet sous Program
//! Files, arrête les services (la console comprise), bascule les dossiers par
//! renommage et relance tout. Si la nouvelle console ne répond pas en 60 s, il
//! remet les dossiers précédents et l'écrit dans `update/resultat.json`, que
//! la console relit à son démarrage. L'icône de la zone de notification est
//! relancée au démarrage de la console (`update/relaunch-tray`). Le même
//! applicateur remet la version précédente à la demande (`update rollback`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::i18n::{self, Msg};

pub mod acl;
pub mod apply;
pub mod manifest;

pub use manifest::Channel;
use manifest::{Manifest, Refusal};

pub const REPO: &str = "MonsieurNours/kybernow";
pub const PACKAGE: &str = "kybernow-update.zip";
/// Résultat de la dernière installation, écrit par l'applicateur (ou `update.ps1`).
const RESULT: &str = "resultat.json";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub published: String,
    pub page: String,
    /// Préversion GitHub (canal bêta).
    pub prerelease: bool,
    package: Option<String>,
    manifest_url: Option<String>,
    signature_url: Option<String>,
    /// Manifeste vérifié (signature de KyberNow), si la version est plus récente.
    manifest: Option<Manifest>,
    /// Pourquoi cette version ne s'installe pas depuis la console.
    refusal: Option<Refusal>,
}

impl Release {
    fn installable(&self) -> bool {
        self.manifest.is_some() && self.refusal.is_none() && self.package.is_some()
    }
}

#[derive(Default)]
struct State {
    channel: Channel,
    checked: Option<Instant>,
    latest: Option<Release>,
    /// Échec de la dernière vérification (écrit dans la langue de qui lit).
    error: Option<anyhow::Error>,
    installing: Option<String>,
    /// Applicateur de l'installation en cours, suivi pour voir s'il s'arrête
    /// sans avoir remplacé la console.
    child: Option<std::process::Child>,
    /// Dernière installation qui n'a pas abouti (fin de son journal).
    install_error: Option<Msg>,
    /// Échec signalé par l'applicateur (`update/resultat.json`) : version,
    /// raison, retour à la version précédente, journal.
    last_failure: Option<Value>,
    /// Version gardée dans les dossiers `.prev` (retour en arrière possible).
    previous: Option<String>,
}

pub struct Updater {
    dir: PathBuf,
    layout: apply::Layout,
    state: Mutex<State>,
}

/// Dossier d'installation : celui de la console (`<installation>\admin\kyber-admin.exe`).
fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let admin = exe.parent()?;
            admin.file_name().is_some_and(|n| n.eq_ignore_ascii_case("admin")).then(|| admin.parent().map(Path::to_path_buf)).flatten()
        })
        .unwrap_or_else(apply::default_install_dir)
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Une mise à jour (ou un retour en arrière) coupe la session en cours :
/// message du refus (409) si un appareil est connecté et que `force` n'est pas
/// demandé. `live` : la session en cours d'`activity` (`null` sans session).
pub fn session_conflict(live: &Value, force: bool) -> Option<Msg> {
    if force || live.is_null() {
        return None;
    }
    Some(match live["name"].as_str() {
        Some(name) => Msg::new("update.sessionConflictNamed").with("name", name),
        None => Msg::new("update.sessionConflict"),
    })
}

/// Numéro de version publiable : trois nombres (« 0.2.6 »). Il sert de nom
/// de dossier et passe en argument à l'applicateur : rien d'autre n'est accepté.
pub fn valid_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// « 0.10.2 » > « 0.9.9 » : comparaison nombre par nombre.
pub fn newer(candidate: &str, than: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v').split(['.', '-']).map_while(|p| p.parse().ok()).collect()
    };
    parse(candidate) > parse(than)
}

impl Updater {
    pub fn new(data_dir: &Path) -> Self {
        let dir = data_dir.join("update");
        let mut state = State::default();
        if let Some(failure) = read_failure(&dir.join(RESULT)) {
            let version = failure["version"].as_str().unwrap_or("?");
            // La raison vient de l'applicateur (en français, comme son journal).
            let message = failure["message"].as_str().map_or_else(|| Msg::new("update.unknownReason").to_value(), |m| json!(m));
            let back = if failure["rolled_back"].as_bool() == Some(true) {
                Msg::new("update.rolledBack").with("version", current())
            } else if failure["undo_failed"].as_bool() == Some(true) {
                Msg::new("update.undoFailed").with("version", current()).with("dir", install_dir().display().to_string())
            } else {
                Msg::new("update.notReplaced")
            };
            let log = failure["log"].as_str().filter(|l| !l.is_empty()).map_or(json!(""), |l| Msg::new("update.logLine").with("path", l).to_value());
            let msg = Msg::new("update.failed").with("version", version).with("message", message).with_msg("back", back).with("log", log);
            tracing::error!("mise à jour {version} non installée : {}", failure["message"].as_str().unwrap_or("raison inconnue"));
            state.install_error = Some(msg);
            state.last_failure = Some(failure);
        }
        // Le canal est relu ici (le fichier vient d'être lu par la console) :
        // le démarrage n'a rien d'autre à transmettre.
        state.channel = crate::settings::Settings::load(data_dir).map(|s| s.update_channel).unwrap_or_default();
        let layout = apply::Layout { install: install_dir(), data: data_dir.to_path_buf() };
        state.previous = apply::previous_version(&layout).filter(|v| v != current());
        Self { dir, layout, state: Mutex::new(state) }
    }

    pub fn channel(&self) -> Channel {
        self.state.lock().unwrap().channel
    }

    /// Change le canal suivi ; la version connue est oubliée jusqu'à la
    /// prochaine vérification (elle venait peut-être de l'autre canal).
    pub fn set_channel(&self, channel: Channel) {
        let mut s = self.state.lock().unwrap();
        if s.channel != channel {
            s.channel = channel;
            s.latest = None;
            s.checked = None;
        }
    }

    /// État pour la console et l'icône (sans appel réseau), textes et notes de
    /// version dans la langue de la requête.
    pub fn status(&self) -> Value {
        let lang = i18n::current();
        let mut s = self.state.lock().unwrap();
        self.reap(&mut s);
        let latest = s.latest.as_ref();
        let refusal = latest.and_then(|r| r.refusal.as_ref());
        json!({
            "current": current(),
            "channel": s.channel,
            "latest": latest.map(|r| r.version.clone()),
            "available": latest.is_some_and(|r| newer(&r.version, current())),
            "prerelease": latest.is_some_and(|r| r.prerelease),
            // La partie des notes dans cette langue (marqueur `<!-- en -->`).
            "notes": latest.map(|r| i18n::notes_for(&r.notes, lang)),
            "published": latest.map(|r| r.published.clone()),
            "page": latest.map(|r| r.page.clone()),
            // Manifeste signé vérifié, compatible, paquet présent.
            "installable": latest.is_some_and(Release::installable),
            // Paquet refusé (signature, canal, version) ou à installer avec l'installateur.
            "verify_error": refusal.map(|r| r.message().render(lang)),
            "requires_setup": matches!(refusal, Some(Refusal::Setup(_))),
            // Nom de l'installateur à télécharger : celui que liste le manifeste
            // signé (l'ancien nom pour une version publiée avant le renommage).
            "setup": latest.and_then(|r| r.manifest.as_ref()).map_or(manifest::SETUP, Manifest::setup_installer),
            "checked_seconds_ago": s.checked.map(|t| t.elapsed().as_secs()),
            "error": s.error.as_ref().map(|e| i18n::error_text(lang, e)),
            "installing": s.installing,
            "install_error": s.install_error.as_ref().map(|m| m.render(lang)),
            // Échec écrit par l'applicateur : contrôle de santé raté, retour arrière.
            "last_failure": s.last_failure,
            // Version des dossiers .prev : « Revenir à la version précédente ».
            "rollback": s.previous,
            "repo": REPO,
        })
    }

    /// Applicateur terminé alors que la console tourne encore : l'installation
    /// s'est arrêtée en route (sinon il aurait arrêté la console).
    fn reap(&self, s: &mut State) {
        let Some(child) = s.child.as_mut() else { return };
        let Ok(Some(exit)) = child.try_wait() else { return };
        let version = s.installing.take().unwrap_or_default();
        s.child = None;
        let dir = self.dir.join(&version);
        // Le plus récent des deux : le journal, ou la sortie de l'applicateur
        // s'il s'est arrêté avant d'écrire (le journal peut dater d'avant).
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let (journal, output) = (dir.join("update.log"), dir.join("lancement.log"));
        let log = if modified(&journal) >= modified(&output) && journal.exists() { journal } else { output };
        let tail = std::fs::read_to_string(&log)
            .map(|t| t.lines().rev().filter(|l| !l.trim().is_empty()).take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))
            .ok()
            .filter(|t| !t.trim().is_empty())
            .map_or_else(|| Msg::new("update.noOutput").to_value(), Value::String);
        let msg = Msg::new("update.stopped")
            .with("version", version.as_str())
            .with("exit", exit.to_string())
            .with("log", log.display().to_string())
            .with("tail", tail);
        tracing::error!("mise à jour {version} interrompue ({exit}) :\n{}", msg.params["tail"].as_str().unwrap_or("aucune sortie"));
        s.install_error = Some(msg);
    }

    /// Version disponible, si plus récente et signée par KyberNow (pour
    /// l'icône) : une version falsifiée n'est signalée que sur la page.
    pub fn available(&self) -> Option<String> {
        let s = self.state.lock().unwrap();
        s.latest.as_ref().filter(|r| newer(&r.version, current()) && r.manifest.is_some()).map(|r| r.version.clone())
    }

    /// Demande à GitHub la dernière version du canal, et vérifie son
    /// manifeste si elle est plus récente (bloquant).
    pub fn check(&self) -> Value {
        let channel = self.channel();
        let result = fetch_latest(channel).map(|mut r| {
            if newer(&r.version, current()) {
                verify_release(&mut r, channel);
            }
            r
        });
        {
            let mut s = self.state.lock().unwrap();
            s.checked = Some(Instant::now());
            match result {
                // Le canal a changé pendant la vérification : résultat périmé.
                Ok(_) if s.channel != channel => {}
                Ok(r) => {
                    if let Some(refusal) = &r.refusal {
                        tracing::warn!("version {} non installable : {}", r.version, refusal.message());
                    }
                    s.latest = Some(r);
                    s.error = None;
                }
                Err(e) => s.error = Some(e),
            }
        }
        self.status()
    }

    /// Vérification de fond, toutes les 6 heures.
    pub fn supervise(self: std::sync::Arc<Self>) {
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(60));
            loop {
                let _ = self.check();
                if let Some(v) = self.available() {
                    tracing::info!("mise à jour disponible : {v}");
                }
                std::thread::sleep(CHECK_EVERY);
            }
        });
    }

    /// Télécharge, vérifie et lance l'installation (la console s'arrête ensuite).
    pub fn install(&self) -> Result<String> {
        let release = {
            let mut s = self.state.lock().unwrap();
            self.reap(&mut s);
            if s.installing.is_some() {
                bail!(Msg::new("update.alreadyInstalling"));
            }
            s.latest.clone().ok_or_else(|| Msg::new("update.noKnownVersion"))?
        };
        if !newer(&release.version, current()) {
            bail!(Msg::new("update.upToDate").with("version", current()));
        }
        if let Some(refusal) = &release.refusal {
            bail!(refusal.message().clone());
        }
        let (Some(package), Some(manifest_url), Some(signature_url)) = (&release.package, &release.manifest_url, &release.signature_url) else {
            bail!(Msg::new("update.noSignedPackage").with("version", release.version.as_str()));
        };
        if !valid_version(&release.version) {
            bail!(Msg::new("update.badVersion").with("version", release.version.as_str()));
        }
        let channel = self.channel();
        {
            let mut s = self.state.lock().unwrap();
            s.installing = Some(release.version.clone());
            s.install_error = None;
            s.last_failure = None;
        }
        let _ = std::fs::remove_file(self.dir.join(RESULT));
        let result = self.download_and_launch(&release.version, channel, package, manifest_url, signature_url);
        let mut s = self.state.lock().unwrap();
        match result {
            Ok(child) => {
                s.child = child;
                Ok(release.version)
            }
            Err(e) => {
                s.installing = None;
                Err(e)
            }
        }
    }

    /// Manifeste et signature retéléchargés et revérifiés (rien ne vient de
    /// la vérification précédente), puis le paquet, contre le manifeste.
    fn download_and_launch(
        &self,
        version: &str,
        channel: Channel,
        package_url: &str,
        manifest_url: &str,
        signature_url: &str,
    ) -> Result<Option<std::process::Child>> {
        if !valid_version(version) {
            bail!(Msg::new("update.badVersion").with("version", version));
        }
        let dir = self.dir.join(version);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let json = get(manifest_url, MANIFEST_LIMIT)?;
        let signature = get(signature_url, MANIFEST_LIMIT)?;
        let manifest = manifest::verify_release(&json, &String::from_utf8_lossy(&signature))?;
        manifest.check(version, channel, current()).map_err(|r| anyhow::Error::new(r.message().clone()))?;
        std::fs::write(dir.join(manifest::MANIFEST), &json)?;
        std::fs::write(dir.join(manifest::SIGNATURE), &signature)?;
        let zip = dir.join(PACKAGE);
        let expected = manifest.file(PACKAGE)?;
        tracing::info!("mise à jour {version} : téléchargement ({} octets)", expected.size);
        // Jamais plus que la taille annoncée par le manifeste signé.
        std::fs::write(&zip, get(package_url, expected.size + 1)?)?;
        manifest.check_file(PACKAGE, &zip)?;
        tracing::info!("mise à jour {version} : paquet vérifié (signature, taille, empreinte), installation");
        let args = vec!["update".into(), "apply".into(), zip.into_os_string(), "--version".into(), version.into()];
        self.launch(&dir, args).map(Some)
    }

    /// Remet la version précédente (dossiers `.prev`) ; la console s'arrête
    /// ensuite et revient dans cette version.
    pub fn rollback(&self) -> Result<String> {
        let previous = {
            let mut s = self.state.lock().unwrap();
            self.reap(&mut s);
            if s.installing.is_some() {
                bail!(Msg::new("update.alreadyInstalling"));
            }
            let previous = s.previous.clone().ok_or_else(|| Msg::new("update.noPrevious"))?;
            s.installing = Some(previous.clone());
            s.install_error = None;
            s.last_failure = None;
            previous
        };
        let _ = std::fs::remove_file(self.dir.join(RESULT));
        let dir = self.dir.join(&previous);
        let result = std::fs::create_dir_all(&dir).map_err(anyhow::Error::from).and_then(|_| self.launch(&dir, vec!["update".into(), "rollback".into()]));
        let mut s = self.state.lock().unwrap();
        match result {
            Ok(child) => {
                s.child = Some(child);
                Ok(previous)
            }
            Err(e) => {
                s.installing = None;
                Err(e)
            }
        }
    }

    /// Lance l'applicateur, détaché, avec les dossiers de cet ordinateur.
    fn launch(&self, log_dir: &Path, mut args: Vec<std::ffi::OsString>) -> Result<std::process::Child> {
        args.extend(["--install-dir".into(), self.layout.install.clone().into_os_string()]);
        args.extend(["--data-dir".into(), self.layout.data.clone().into_os_string()]);
        launch_applicator(&self.layout, log_dir, &args)
    }
}

/// Taille maximale du manifeste et de sa signature.
const MANIFEST_LIMIT: u64 = 256 * 1024;

/// Télécharge et vérifie le manifeste d'une version plus récente que celle
/// installée ; le refus, s'il y en a un, est gardé pour la page.
fn verify_release(release: &mut Release, channel: Channel) {
    let (Some(json_url), Some(sig_url)) = (&release.manifest_url, &release.signature_url) else {
        release.refusal = Some(Refusal::Rejected(
            Msg::new("update.unsigned").with("version", release.version.as_str()).with("file", manifest::MANIFEST),
        ));
        return;
    };
    let verified = get(json_url, MANIFEST_LIMIT).and_then(|json| {
        let signature = get(sig_url, MANIFEST_LIMIT)?;
        manifest::verify_release(&json, &String::from_utf8_lossy(&signature))
    });
    match verified {
        Ok(m) => {
            release.refusal = m.check(&release.version, channel, current()).err();
            if release.refusal.is_none() && release.package.is_none() {
                release.refusal =
                    Some(Refusal::Rejected(Msg::new("update.packageMissing").with("package", PACKAGE).with("version", release.version.as_str())));
            }
            release.manifest = Some(m);
        }
        Err(e) => {
            release.refusal =
                Some(Refusal::Rejected(Msg::new("update.rejected").with("version", release.version.as_str()).with("error", i18n::error_param(&e))))
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(format!("kyber-admin/{}", current()))
        .build()
        .into()
}

fn get(url: &str, limit: u64) -> Result<Vec<u8>> {
    let failed = |e: ureq::Error| i18n::because(Msg::new("update.download").with("url", url), &e.into());
    let mut resp = agent().get(url).call().map_err(failed)?;
    resp.body_mut().with_config().limit(limit).read_to_vec().map_err(failed)
}

/// Version la plus récente du canal parmi les dernières publiées : en stable,
/// sans les préversions ; jamais un brouillon ni un numéro hors format.
fn pick(releases: &[Value], channel: Channel) -> Option<&Value> {
    releases
        .iter()
        .filter(|r| r["draft"].as_bool() != Some(true))
        .filter(|r| channel == Channel::Beta || r["prerelease"].as_bool() != Some(true))
        .filter(|r| r["tag_name"].as_str().is_some_and(|t| valid_version(t.trim_start_matches('v'))))
        .reduce(|best, r| {
            let version = |x: &Value| x["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
            if newer(&version(r), &version(best)) { r } else { best }
        })
}

fn fetch_latest(channel: Channel) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| i18n::because(Msg::new("update.githubDown"), &e.into()))?;
    let list: Vec<Value> = serde_json::from_slice(&resp.body_mut().with_config().limit(16 * 1024 * 1024).read_to_vec()?)?;
    let v = pick(&list, channel).ok_or_else(|| Msg::new("update.noRelease").with("channel", channel.as_str()))?;
    let asset = |name: &str| {
        v["assets"].as_array().and_then(|a| {
            a.iter().find(|x| x["name"] == name).and_then(|x| x["browser_download_url"].as_str()).map(String::from)
        })
    };
    let version = v["tag_name"].as_str().ok_or_else(|| Msg::new("update.noTag"))?.trim_start_matches('v').to_string();
    if !valid_version(&version) {
        bail!(Msg::new("update.badGithubVersion").with("version", version));
    }
    Ok(Release {
        version,
        notes: v["body"].as_str().unwrap_or("").to_string(),
        published: v["published_at"].as_str().unwrap_or("").to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        prerelease: v["prerelease"].as_bool().unwrap_or(false),
        package: asset(PACKAGE),
        manifest_url: asset(manifest::MANIFEST),
        signature_url: asset(manifest::SIGNATURE),
        manifest: None,
        refusal: None,
    })
}


/// Échec signalé par l'applicateur ou `update.ps1`, s'il y en a un. Un échec de la version qui
/// tourne est périmé : elle a été installée depuis.
fn read_failure(path: &Path) -> Option<Value> {
    let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    // Sauf un retour arrière raté : la version qui tourne est alors celle qui a échoué.
    let stale = value["version"].as_str() == Some(current()) && value["undo_failed"].as_bool() != Some(true);
    (value["ok"].as_bool() == Some(false) && !stale).then_some(value)
}

/// L'applicateur, détaché de la console (elle va s'arrêter) : une copie de
/// cet exécutable, `<installation>\.staging\kyber-admin-maj.exe` (sous
/// Program Files, lecture seule pour les utilisateurs), pour que le dossier
/// `admin` puisse être renommé pendant qu'il tourne. Sa sortie va dans
/// `lancement.log`, son journal dans `update.log`.
#[cfg(windows)]
fn launch_applicator(layout: &apply::Layout, log_dir: &Path, args: &[std::ffi::OsString]) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    use anyhow::Context;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let staging = layout.staging();
    std::fs::create_dir_all(&staging).with_context(|| format!("création de {}", staging.display()))?;
    let applicator = staging.join(apply::APPLICATOR);
    let exe = std::env::current_exe().context("chemin de la console")?;
    std::fs::copy(&exe, &applicator).with_context(|| format!("copie de la console dans {}", applicator.display()))?;
    let output = std::fs::File::create(log_dir.join("lancement.log")).context("création de lancement.log")?;
    let spawn = |flags: u32| -> std::io::Result<std::process::Child> {
        std::process::Command::new(&applicator)
            .args(args)
            .current_dir(&staging)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output.try_clone()?)
            .creation_flags(flags)
            .spawn()
    };
    // Hors du « job » du service s'il y en a un : sinon l'arrêt de la console l'emporterait.
    let child = spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP))
        .context("lancement de l'applicateur")?;
    tracing::info!("mise à jour : applicateur lancé (processus {})", child.id());
    Ok(child)
}

#[cfg(not(windows))]
fn launch_applicator(_: &apply::Layout, log_dir: &Path, _: &[std::ffi::OsString]) -> Result<std::process::Child> {
    bail!("installation réservée à Windows (paquet vérifié dans {})", log_dir.display())
}

/// Au démarrage de la console : `C:\ProgramData\KyberNow` réservé à SYSTEM et
/// aux Administrateurs (voir `acl`), avant d'y écrire ou d'y lancer quoi que
/// ce soit, dont le paquet de `update\<version>\`. Un échec n'empêche pas
/// la console de démarrer : il est journalisé.
pub fn secure_data_dir(data_dir: &Path) {
    match acl::secure_dir(data_dir) {
        Ok(0) => tracing::info!("droits de {} : SYSTEM et Administrateurs seulement", data_dir.display()),
        Ok(n) => tracing::warn!("droits de {} posés, {n} lien(s) retiré(s)", data_dir.display()),
        Err(e) => tracing::error!("droits de {} non posés : {e:#}", data_dir.display()),
    }
}

/// Au démarrage de la console : relance l'icône si une mise à jour l'a arrêtée.
pub fn relaunch_tray_if_asked(data_dir: &Path) {
    let marker = data_dir.join("update").join("relaunch-tray");
    if !marker.exists() {
        return;
    }
    let _ = std::fs::remove_file(&marker);
    #[cfg(windows)]
    {
        let tray = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent()?.parent().map(|d| d.join("tray").join("kybernow-tray.exe")));
        if let Some(tray) = tray.filter(|t| t.exists()) {
            // Laisse le temps à la session de l'utilisateur d'être prête.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if let Err(e) = crate::display::agent::launch_as_user(&tray) {
                    tracing::warn!("relance de l'icône : {e:#}");
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_versions_are_accepted() {
        assert!(valid_version("0.2.6"));
        assert!(valid_version("10.20.300"));
        for bad in ["", "0.2", "0.2.6.1", "0.2.6-beta", "0..6", "../0.2.6", "0.2.6'; Remove-Item", "v0.2.6", "0.2.x"] {
            assert!(!valid_version(bad), "{bad}");
        }
    }

    #[test]
    fn failure_written_by_the_script_is_reported() {
        let dir = std::env::temp_dir().join(format!("kybernow-update-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("update")).unwrap();
        std::fs::write(
            dir.join("update").join(RESULT),
            r#"{"version":"9.9.9","ok":false,"rolled_back":true,"message":"la nouvelle console ne repond pas","log":"C:\\x\\update.log"}"#,
        )
        .unwrap();
        let status = Updater::new(&dir).status();
        assert_eq!(status["last_failure"]["version"], "9.9.9");
        let error = status["install_error"].as_str().unwrap();
        assert!(error.contains("9.9.9") && error.contains("remise en place"), "{error}");
        let english = i18n::with_lang(i18n::Lang::En, || Updater::new(&dir).status())["install_error"].as_str().unwrap().to_string();
        assert!(english.starts_with("Installing 9.9.9 failed: la nouvelle console ne repond pas. The previous version"), "{english}");
        assert!(english.ends_with("\nLog: C:\\x\\update.log"), "{english}");
        std::fs::write(dir.join("update").join(RESULT), r#"{"version":"9.9.9","ok":true}"#).unwrap();
        assert!(Updater::new(&dir).status()["last_failure"].is_null());
        // Échec de la version qui tourne : périmé, sauf si le retour arrière a raté.
        let failed = |undo: bool| format!(r#"{{"version":"{}","ok":false,"rolled_back":false,"undo_failed":{undo},"message":"x"}}"#, current());
        std::fs::write(dir.join("update").join(RESULT), failed(false)).unwrap();
        assert!(Updater::new(&dir).status()["last_failure"].is_null());
        std::fs::write(dir.join("update").join(RESULT), failed(true)).unwrap();
        let error = Updater::new(&dir).status()["install_error"].as_str().unwrap().to_string();
        assert!(error.contains("n'a pas abouti") && error.contains("KyberNow-Station-Setup.exe"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_update_during_a_session_needs_force() {
        assert_eq!(session_conflict(&Value::Null, false), None);
        let live = json!({ "device_id": "abc", "name": "Apple TV du salon" });
        let msg = session_conflict(&live, false).unwrap().to_string();
        assert!(msg.contains("Apple TV du salon") && msg.contains("coupera sa session"), "{msg}");
        assert_eq!(session_conflict(&live, true), None);
        assert!(session_conflict(&json!({ "device_id": "abc" }), false).unwrap().to_string().starts_with("Un appareil est connecté :"));
        let english = session_conflict(&live, false).unwrap().render(i18n::Lang::En);
        assert_eq!(english, "A device is connected (Apple TV du salon): installing will end its session");
    }

    #[test]
    fn channel_setting_defaults_to_stable() {
        let s: crate::settings::Settings = toml::from_str("listen = \"0.0.0.0:9443\"").unwrap();
        assert_eq!(s.update_channel, Channel::Stable);
        let s: crate::settings::Settings = toml::from_str("update_channel = \"beta\"").unwrap();
        assert_eq!(s.update_channel, Channel::Beta);
        assert!(toml::to_string(&s).unwrap().contains("update_channel = \"beta\""));
        assert!(toml::from_str::<crate::settings::Settings>("update_channel = \"nightly\"").is_err());
    }

    fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            notes: String::new(),
            published: String::new(),
            page: String::new(),
            prerelease: false,
            package: Some("https://example.invalid/kybernow-update.zip".into()),
            manifest_url: None,
            signature_url: None,
            manifest: None,
            refusal: None,
        }
    }

    #[test]
    fn an_unsigned_release_is_shown_but_never_installed() {
        let dir = std::env::temp_dir().join(format!("kybernow-update-unsigned-{}", std::process::id()));
        let updater = Updater::new(&dir);
        let mut r = release("99.0.0");
        // Pas de manifeste publié : refusé sans rien télécharger.
        verify_release(&mut r, Channel::Stable);
        updater.state.lock().unwrap().latest = Some(r);
        let status = updater.status();
        assert_eq!(status["available"], true);
        assert_eq!(status["installable"], false);
        assert_eq!(status["requires_setup"], false);
        assert_eq!(status["setup"], manifest::SETUP);
        assert!(status["verify_error"].as_str().unwrap().contains("pas signée"), "{status}");
        assert_eq!(status["channel"], "stable");
        assert!(status["rollback"].is_null());
        // L'icône n'en parle pas, et l'installation est refusée avant tout téléchargement.
        assert_eq!(updater.available(), None);
        assert!(updater.install().unwrap_err().to_string().contains("pas signée"));
        assert!(updater.state.lock().unwrap().installing.is_none());
        assert!(updater.rollback().is_err());
        // Changer de canal oublie la version connue.
        updater.set_channel(Channel::Beta);
        assert!(updater.status()["latest"].is_null());
        assert_eq!(updater.channel(), Channel::Beta);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_signed_release_that_needs_the_installer_says_so() {
        let dir = std::env::temp_dir().join(format!("kybernow-update-setup-{}", std::process::id()));
        let updater = Updater::new(&dir);
        let mut r = release("99.0.0");
        let mut m = manifest::tests::sample("99.0.0", Channel::Stable);
        m.requires_setup = true;
        r.refusal = m.check("99.0.0", Channel::Stable, current()).err();
        r.manifest = Some(m);
        updater.state.lock().unwrap().latest = Some(r);
        let status = updater.status();
        assert_eq!(status["installable"], false);
        assert_eq!(status["requires_setup"], true);
        assert!(status["verify_error"].as_str().unwrap().contains("KyberNow-Station-Setup.exe"));
        assert_eq!(status["setup"], "KyberNow-Station-Setup.exe");
        // Signée : l'icône la signale (pour réinstaller).
        assert_eq!(updater.available().as_deref(), Some("99.0.0"));
        assert!(updater.install().is_err());
        // Manifeste d'avant le renommage : l'ancien nom de l'installateur.
        let mut r = release("99.0.0");
        let mut m = manifest::tests::sample("99.0.0", Channel::Stable);
        m.files.remove(manifest::SETUP);
        m.requires_setup = true;
        r.refusal = m.check("99.0.0", Channel::Stable, current()).err();
        r.manifest = Some(m);
        updater.state.lock().unwrap().latest = Some(r);
        let status = updater.status();
        assert_eq!(status["setup"], "KyberNow-Server-Setup.exe");
        assert!(status["verify_error"].as_str().unwrap().contains("KyberNow-Server-Setup.exe"), "{status}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn channel_picks_the_newest_matching_release() {
        let list: Vec<Value> = serde_json::from_value(json!([
            { "tag_name": "v0.2.8", "prerelease": true, "draft": false },
            { "tag_name": "v0.3.0", "prerelease": false, "draft": true },
            { "tag_name": "v0.2.10", "prerelease": false, "draft": false },
            { "tag_name": "v0.2.9", "prerelease": false, "draft": false },
            { "tag_name": "v0.4.0-rc1", "prerelease": true, "draft": false },
            { "tag_name": "v0.2.11", "prerelease": true, "draft": false },
        ]))
        .unwrap();
        assert_eq!(pick(&list, Channel::Stable).unwrap()["tag_name"], "v0.2.10");
        assert_eq!(pick(&list, Channel::Beta).unwrap()["tag_name"], "v0.2.11");
        assert!(pick(&[], Channel::Stable).is_none());
    }

    #[test]
    fn versions_compare_number_by_number() {
        assert!(newer("0.10.0", "0.9.9"));
        assert!(newer("0.2.10", "0.2.9"));
        assert!(!newer("0.2.6", "0.2.6"));
        assert!(!newer("0.2.5", "0.2.6"));
        assert!(newer("v1.0.0", "0.99.99"));
    }

}
