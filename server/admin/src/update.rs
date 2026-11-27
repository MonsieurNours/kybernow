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
// Station Linux ; ses parties sans système se testent aussi sur Mac.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod mac;
pub mod manifest;
/// Listes partagées avec l'installateur de la Station Windows.
pub mod windows_common;

pub use manifest::Channel;
use manifest::{Manifest, Refusal};

pub const REPO: &str = "MonsieurNours/kybernow";
/// Paquet de mise à jour de cette Station : `kybernow-update.zip` sous
/// Windows, l'app entière sur Mac (`server/macos/scripts/build-station-app.sh`),
/// le paquet .deb ou .rpm de son architecture sous Linux (`linux::package`).
pub fn package() -> &'static str {
    #[cfg(target_os = "linux")]
    let name = linux::package();
    #[cfg(target_os = "macos")]
    let name = "KyberNow-Station-Mac-update.zip";
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let name = "kybernow-update.zip";
    name
}
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

/// État résumé de la mise à jour (voir `Updater::brief`).
#[derive(Debug, Clone)]
pub struct Brief {
    /// Version plus récente et signée par KyberNow, s'il y en a une.
    pub available: Option<String>,
    /// Elle s'installe depuis Admin (manifeste vérifié, compatible, paquet présent).
    pub installable: bool,
    /// Elle s'installe avec l'installateur (`setup`), sur l'ordinateur.
    pub requires_setup: bool,
    /// Pourquoi elle ne s'installe pas depuis Admin.
    pub refusal: Option<Msg>,
    /// Installateur de la Station listé par son manifeste.
    pub setup: &'static str,
    pub installing: bool,
}

impl Brief {
    /// `station.update` de `GET /api/stream/info`.
    pub fn json(&self) -> Value {
        json!({
            "available": self.available.is_some(),
            "version": self.available,
            "installable": self.installable,
            "requires_setup": self.requires_setup,
            "installing": self.installing,
        })
    }
}

pub struct Updater {
    dir: PathBuf,
    /// `resultat.json` de l'applicateur : dans `dir`, sauf sous Linux, où
    /// l'écrit le service root, dans son propre dossier.
    results: PathBuf,
    layout: apply::Layout,
    state: Mutex<State>,
}

/// Dossier d'installation : celui de la console (`<installation>\admin\kyber-admin.exe`).
fn install_dir() -> PathBuf {
    // Mac : l'app KyberNow Station qui contient cet Admin.
    #[cfg(target_os = "macos")]
    return mac::installed_app();
    // Linux : le paquet, à sa place fixe.
    #[cfg(target_os = "linux")]
    return PathBuf::from("/usr/lib/kybernow-station");
    #[cfg_attr(any(target_os = "macos", target_os = "linux"), allow(unreachable_code))]
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let admin = exe.parent()?;
            admin.file_name().is_some_and(|n| n.eq_ignore_ascii_case("admin")).then(|| admin.parent().map(Path::to_path_buf)).flatten()
        })
        .unwrap_or_else(apply::default_install_dir)
}

/// Au démarrage d'Admin (Windows) : relance automatique des services reposée,
/// bascule interrompue reprise, restes de `.staging` effacés.
pub fn at_start(data_dir: &Path) {
    #[cfg(windows)]
    {
        let layout = apply::Layout { install: install_dir(), data: data_dir.to_path_buf() };
        if let Some(r) = apply::at_start(&layout) {
            if r.failed.is_empty() {
                tracing::warn!("bascule vers {} interrompue, {} au démarrage", r.version, if r.forward { "terminée" } else { "défaite" });
            } else {
                tracing::error!("bascule vers {} interrompue : dossiers non remis ({})", r.version, r.failed.join(", "));
            }
        }
    }
    #[cfg(not(windows))]
    let _ = data_dir;
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
        let results = results_path(&dir);
        let mut state = State::default();
        if let Some(failure) = read_failure(&results) {
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
        Self { dir, results, layout, state: Mutex::new(state) }
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
            "setup": latest.and_then(|r| r.manifest.as_ref()).map_or(manifest::default_setup(), Manifest::setup_installer),
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

    /// Ce que les apps KyberNow Link montrent de la mise à jour de la Station
    /// (`station.update` de `GET /api/stream/info`, toutes les 8 s par
    /// appareil) : lu en mémoire, sans appel réseau. Une version n'y est
    /// « disponible » que plus récente et signée par KyberNow, comme pour l'icône.
    pub fn brief(&self) -> Brief {
        let mut s = self.state.lock().unwrap();
        self.reap(&mut s);
        let latest = s.latest.as_ref().filter(|r| newer(&r.version, current()) && r.manifest.is_some());
        let refusal = latest.and_then(|r| r.refusal.as_ref());
        Brief {
            available: latest.map(|r| r.version.clone()),
            installable: latest.is_some_and(Release::installable),
            requires_setup: matches!(refusal, Some(Refusal::Setup(_))),
            refusal: refusal.map(|r| r.message().clone()),
            setup: latest.and_then(|r| r.manifest.as_ref()).map_or(manifest::default_setup(), Manifest::setup_installer),
            installing: s.installing.is_some(),
        }
    }

    /// Une installation (ou un retour en arrière) est en cours.
    pub fn installing(&self) -> bool {
        let mut s = self.state.lock().unwrap();
        self.reap(&mut s);
        s.installing.is_some()
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
        // Manifeste déjà vérifié pour cette version et ce canal : gardé (pas
        // retéléchargé à chaque vérification ; l'installation le revérifie).
        let known = self.state.lock().unwrap().latest.clone().filter(|r| r.manifest.is_some());
        let result = fetch_latest(channel).map(|mut r| {
            if newer(&r.version, current()) {
                match known.as_ref().filter(|k| k.version == r.version && k.package == r.package && k.manifest_url == r.manifest_url) {
                    Some(k) => {
                        r.manifest = k.manifest.clone();
                        r.refusal = k.refusal.clone();
                    }
                    None => verify_release(&mut r, channel),
                }
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

    /// Vérification demandée par une app KyberNow Link
    /// (`POST /api/stream/station/check`) : la même que `check`, sauf si la
    /// dernière date de moins de `fresh` (le résultat gardé suffit). Rend
    /// `true` quand le flux a été interrogé.
    pub fn check_unless_fresh(&self, fresh: Duration) -> bool {
        let recent = self.state.lock().unwrap().checked.is_some_and(|t| t.elapsed() < fresh);
        if recent {
            return false;
        }
        let _ = self.check();
        true
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
        // Vérifiée et réservée dans la même prise du verrou : deux « Installer »
        // presque simultanés (Admin, icône, app) ne partent jamais tous les deux.
        let (release, channel) = {
            let mut s = self.state.lock().unwrap();
            self.reap(&mut s);
            if s.installing.is_some() {
                bail!(Msg::new("update.alreadyInstalling"));
            }
            let release = s.latest.clone().ok_or_else(|| Msg::new("update.noKnownVersion"))?;
            if !newer(&release.version, current()) {
                bail!(Msg::new("update.upToDate").with("version", current()));
            }
            if let Some(refusal) = &release.refusal {
                bail!(refusal.message().clone());
            }
            if release.package.is_none() || release.manifest_url.is_none() || release.signature_url.is_none() {
                bail!(Msg::new("update.noSignedPackage").with("version", release.version.as_str()));
            }
            if !valid_version(&release.version) {
                bail!(Msg::new("update.badVersion").with("version", release.version.as_str()));
            }
            s.installing = Some(release.version.clone());
            s.install_error = None;
            s.last_failure = None;
            (release, s.channel)
        };
        let (Some(package), Some(manifest_url), Some(signature_url)) = (&release.package, &release.manifest_url, &release.signature_url) else {
            unreachable!("adresses vérifiées sous le verrou")
        };
        let _ = std::fs::remove_file(&self.results);
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
        crate::durable::write(&dir.join(manifest::MANIFEST), &json)?;
        crate::durable::write(&dir.join(manifest::SIGNATURE), &signature)?;
        let zip = dir.join(package());
        let expected = manifest.file(package())?;
        tracing::info!("mise à jour {version} : téléchargement ({} octets)", expected.size);
        // Jamais plus que la taille annoncée par le manifeste signé ; écrit au
        // fil de l'eau (pas tout en mémoire), avec un délai à la mesure du paquet.
        download_to(package_url, &zip, expected.size)?;
        manifest.check_file(package(), &zip)?;
        tracing::info!("mise à jour {version} : paquet vérifié (signature, taille, empreinte), installation");
        // Linux : le paquet de la version qui tourne, pour le retour arrière,
        // puis la demande au service root (qui revérifie tout).
        #[cfg(target_os = "linux")]
        {
            // Installation dans le dossier de l'utilisateur (systèmes immuables) :
            // son script bascule lui-même, sans root.
            if let Some(bin) = linux::home_bin() {
                let args = vec![
                    "update".into(),
                    zip.into_os_string(),
                    version.into(),
                    "--result".into(),
                    self.results.clone().into_os_string(),
                    "--log".into(),
                    dir.join("update.log").into_os_string(),
                ];
                return linux::run_home(&bin, &args, &dir.join("lancement.log")).map(|()| None);
            }
            if let Err(e) = self.fetch_running_package() {
                tracing::warn!("paquet de la version {} non téléchargé, pas de retour arrière possible : {e:#}", current());
            }
            return linux::request(&linux::Request::Apply { version: version.into() }).map(|()| None);
        }
        #[cfg_attr(target_os = "linux", allow(unreachable_code))]
        let args = vec!["update".into(), "apply".into(), zip.into_os_string(), "--version".into(), version.into()];
        self.launch(&dir, args)
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
        let _ = std::fs::remove_file(&self.results);
        let dir = self.dir.join(&previous);
        let result = std::fs::create_dir_all(&dir).map_err(anyhow::Error::from).and_then(|_| {
            // Linux : le service root remet la version qu'il a gardée.
            #[cfg(target_os = "linux")]
            return match linux::home_bin() {
                Some(bin) => {
                    let args = vec![
                        "rollback".into(),
                        "--result".into(),
                        self.results.clone().into_os_string(),
                        "--log".into(),
                        dir.join("update.log").into_os_string(),
                    ];
                    linux::run_home(&bin, &args, &dir.join("lancement.log")).map(|()| None)
                }
                None => linux::request(&linux::Request::Rollback).map(|()| None),
            };
            #[cfg_attr(target_os = "linux", allow(unreachable_code))]
            self.launch(&dir, vec!["update".into(), "rollback".into()])
        });
        let mut s = self.state.lock().unwrap();
        match result {
            Ok(child) => {
                s.child = child;
                Ok(previous)
            }
            Err(e) => {
                s.installing = None;
                Err(e)
            }
        }
    }

    /// Paquet publié de la version qui tourne, pour que le service root puisse
    /// y revenir : téléchargé et vérifié contre son manifeste signé, sauf
    /// s'il le garde déjà (installé par une mise à jour).
    #[cfg(target_os = "linux")]
    fn fetch_running_package(&self) -> Result<()> {
        let version = current();
        if linux::Store::system().kept(version, package()).is_some() {
            return Ok(());
        }
        let release = fetch_tag(version)?;
        let (Some(package_url), Some(json_url), Some(sig_url)) = (&release.package, &release.manifest_url, &release.signature_url) else {
            bail!("la version {version} publiée n'a pas de paquet {} signé", package());
        };
        let dir = self.dir.join(version);
        std::fs::create_dir_all(&dir)?;
        let json = get(json_url, MANIFEST_LIMIT)?;
        let signature = get(sig_url, MANIFEST_LIMIT)?;
        let manifest = manifest::verify_release(&json, &String::from_utf8_lossy(&signature))?;
        if manifest.version != version {
            bail!("le manifeste signé annonce {}, pas {version}", manifest.version);
        }
        let expected = manifest.file(package())?;
        let path = dir.join(package());
        download_to(package_url, &path, expected.size)?;
        manifest.check_file(package(), &path)?;
        crate::durable::write(&dir.join(manifest::MANIFEST), &json)?;
        crate::durable::write(&dir.join(manifest::SIGNATURE), &signature)?;
        tracing::info!("paquet de la version {version} téléchargé et vérifié (retour arrière)");
        Ok(())
    }

    /// Lance l'applicateur, détaché, avec les dossiers de cet ordinateur. Sur
    /// Mac, launchd le suit (service à part) : pas de processus à surveiller ici.
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    fn launch(&self, log_dir: &Path, mut args: Vec<std::ffi::OsString>) -> Result<Option<std::process::Child>> {
        args.extend(["--install-dir".into(), self.layout.install.clone().into_os_string()]);
        args.extend(["--data-dir".into(), self.layout.data.clone().into_os_string()]);
        #[cfg(target_os = "macos")]
        return mac::launch(&self.layout, log_dir, &args).map(|()| None);
        #[cfg_attr(target_os = "macos", allow(unreachable_code))]
        launch_applicator(&self.layout, log_dir, &args).map(Some)
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
            // Station en couche sur un système ostree : elle s'installe par rpm-ostree.
            #[cfg(target_os = "linux")]
            if release.refusal.is_none() && linux::ostree_layered() {
                release.refusal = Some(Refusal::Setup(Msg::new("update.rpmOstree").with("version", release.version.as_str())));
            }
            if release.refusal.is_none() && release.package.is_none() {
                release.refusal =
                    Some(Refusal::Rejected(Msg::new("update.packageMissing").with("package", package()).with("version", release.version.as_str())));
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

/// Délai accordé au téléchargement d'un paquet de `size` octets : 10 min au
/// moins, et de quoi le recevoir à 32 Kio/s (environ 256 kbit/s). L'ancien
/// délai de 120 s au total faisait échouer toute connexion sous 4 Mbit/s.
pub fn package_timeout(size: u64) -> Duration {
    Duration::from_secs((size / (32 * 1024)).max(600))
}

/// Télécharge `url` dans `path` (fichier temporaire forcé sur le disque, puis
/// renommé), sans jamais lire plus de `size` octets.
fn download_to(url: &str, path: &Path, size: u64) -> Result<()> {
    use std::io::Write as _;
    let failed = |e: ureq::Error| i18n::because(Msg::new("update.download").with("url", url), &e.into());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_recv_body(Some(package_timeout(size)))
        .user_agent(format!("kyber-admin/{}", current()))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(failed)?;
    let part = path.with_extension("part");
    let result = (|| -> Result<()> {
        let mut file = std::fs::File::create(&part)?;
        let mut reader = resp.body_mut().with_config().limit(size + 1).reader();
        std::io::copy(&mut reader, &mut file).map_err(|e| i18n::because(Msg::new("update.download").with("url", url), &e.into()))?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        crate::durable::rename(&part, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
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

/// Dernière liste des versions de GitHub et son ETag : une requête
/// conditionnelle (`If-None-Match`) qui répond 304 ne compte pas dans la
/// limite de 60 requêtes par heure, partagée par les Stations d'un même accès.
static RELEASES: Mutex<Option<(String, Vec<Value>)>> = Mutex::new(None);

fn fetch_latest(channel: Channel) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let etag = RELEASES.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|(e, _)| e.clone());
    let mut request = agent().get(&url).header("Accept", "application/vnd.github+json");
    if let Some(etag) = &etag {
        request = request.header("If-None-Match", etag);
    }
    let mut resp = request.call().map_err(|e| i18n::because(Msg::new("update.githubDown"), &e.into()))?;
    let list: Vec<Value> = if resp.status() == 304
        && let Some((_, list)) = RELEASES.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
    {
        list.clone()
    } else {
        let new_etag = resp.headers().get("etag").and_then(|v| v.to_str().ok()).map(str::to_string);
        let list: Vec<Value> = serde_json::from_slice(&resp.body_mut().with_config().limit(16 * 1024 * 1024).read_to_vec()?)?;
        if let Some(e) = new_etag {
            *RELEASES.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((e, list.clone()));
        }
        list
    };
    let v = pick(&list, channel).ok_or_else(|| Msg::new("update.noRelease").with("channel", channel.as_str()))?;
    release_from(v)
}

/// Une version publiée précise (`v<version>`), pour son paquet.
#[cfg(target_os = "linux")]
fn fetch_tag(version: &str) -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/tags/v{version}");
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| i18n::because(Msg::new("update.githubDown"), &e.into()))?;
    let v: Value = serde_json::from_slice(&resp.body_mut().with_config().limit(4 * 1024 * 1024).read_to_vec()?)?;
    release_from(&v)
}

/// Une version de l'API de GitHub, avec les adresses de ses fichiers.
fn release_from(v: &Value) -> Result<Release> {
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
        package: asset(package()),
        manifest_url: asset(manifest::MANIFEST),
        signature_url: asset(manifest::SIGNATURE),
        manifest: None,
        refusal: None,
    })
}


/// `resultat.json` de l'applicateur : dans `dir`, sauf sous Linux avec le
/// paquet du système, où l'écrit le service root dans son propre dossier.
fn results_path(dir: &Path) -> PathBuf {
    #[cfg(all(target_os = "linux", not(test)))]
    if linux::home_bin().is_none() {
        return linux::Store::system().result();
    }
    dir.join(RESULT)
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
    use anyhow::Context;
    let staging = layout.staging();
    std::fs::create_dir_all(&staging).with_context(|| format!("création de {}", staging.display()))?;
    let applicator = staging.join(apply::APPLICATOR);
    let exe = std::env::current_exe().context("chemin de la console")?;
    crate::durable::copy(&exe, &applicator).with_context(|| format!("copie de la console dans {}", applicator.display()))?;
    let child = spawn_detached(&applicator, args, &staging, &log_dir.join("lancement.log")).context("lancement de l'applicateur")?;
    tracing::info!("mise à jour : applicateur lancé (processus {})", child.id());
    Ok(child)
}

/// Lance `exe` sans fenêtre, détaché d'Admin, qui va s'arrêter : hors du
/// « job » du service s'il y en a un (sinon l'arrêt d'Admin l'emporterait),
/// sortie et erreurs dans `output`. Sert à l'applicateur des mises à jour et
/// à l'assistant de redémarrage de la Station (`station`).
#[cfg(windows)]
pub fn spawn_detached(exe: &Path, args: &[std::ffi::OsString], cwd: &Path, output: &Path) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    use anyhow::Context;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let output = std::fs::File::create(output).with_context(|| format!("création de {}", output.display()))?;
    let spawn = |flags: u32| -> std::io::Result<std::process::Child> {
        std::process::Command::new(exe)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output.try_clone()?)
            .creation_flags(flags)
            .spawn()
    };
    spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP))
        .with_context(|| format!("lancement de {}", exe.display()))
}

#[cfg(not(windows))]
#[cfg_attr(any(target_os = "macos", target_os = "linux"), allow(dead_code))]
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
        let setup = if cfg!(target_os = "linux") { "paquet kybernow-station" } else { "KyberNow-Station-Setup.exe" };
        assert!(error.contains("n'a pas abouti") && error.contains(setup), "{error}");
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
        assert_eq!(status["setup"], manifest::default_setup());
        assert!(status["verify_error"].as_str().unwrap().contains("pas signée"), "{status}");
        assert_eq!(status["channel"], "stable");
        assert!(status["rollback"].is_null());
        // L'icône et les apps n'en parlent pas, et l'installation est refusée avant tout téléchargement.
        assert_eq!(updater.available(), None);
        let brief = updater.brief();
        assert_eq!((brief.available, brief.installable, brief.requires_setup), (None, false, false));
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
        let setup = manifest::default_setup();
        assert!(status["verify_error"].as_str().unwrap().contains(setup));
        assert_eq!(status["setup"], setup);
        // Signée : l'icône la signale (pour réinstaller), les apps aussi.
        assert_eq!(updater.available().as_deref(), Some("99.0.0"));
        let brief = updater.brief();
        assert_eq!((brief.available.as_deref(), brief.installable, brief.requires_setup), (Some("99.0.0"), false, true));
        assert_eq!(brief.setup, setup);
        assert_eq!(
            brief.json(),
            json!({ "available": true, "version": "99.0.0", "installable": false, "requires_setup": true, "installing": false })
        );
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
        // (Sur Mac, toujours le .dmg, sous Linux le paquet : les anciens noms sont ceux de Windows.)
        let legacy = if cfg!(windows) { manifest::LEGACY_SETUP } else { manifest::default_setup() };
        assert_eq!(status["setup"], legacy);
        assert!(status["verify_error"].as_str().unwrap().contains(legacy), "{status}");
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
