//! Mises à jour en un clic, depuis les versions publiées sur GitHub
//! (github.com/MonsieurNours/kybernow, dépôt public : aucun jeton).
//!
//! La console regarde la dernière version toutes les 6 heures (ou à la
//! demande). « Installer » télécharge `kybernow-update.zip`, vérifie son
//! empreinte dans `SHA256SUMS`, puis lance PowerShell, détaché : il
//! décompresse le paquet et exécute son `update.ps1`, qui arrête les
//! services (la console comprise), remplace les fichiers et relance tout.
//! L'icône de la zone de notification est relancée au démarrage de la
//! nouvelle console (`update/relaunch-tray`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const REPO: &str = "MonsieurNours/kybernow";
const PACKAGE: &str = "kybernow-update.zip";
const SUMS: &str = "SHA256SUMS";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub published: String,
    pub page: String,
    package: Option<String>,
    sums: Option<String>,
}

#[derive(Default)]
struct State {
    checked: Option<Instant>,
    latest: Option<Release>,
    error: Option<String>,
    installing: Option<String>,
}

pub struct Updater {
    dir: PathBuf,
    state: Mutex<State>,
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
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
        Self { dir: data_dir.join("update"), state: Mutex::new(State::default()) }
    }

    /// État pour la console et l'icône (sans appel réseau).
    pub fn status(&self) -> Value {
        let s = self.state.lock().unwrap();
        let latest = s.latest.as_ref();
        json!({
            "current": current(),
            "latest": latest.map(|r| r.version.clone()),
            "available": latest.is_some_and(|r| newer(&r.version, current())),
            "notes": latest.map(|r| r.notes.clone()),
            "published": latest.map(|r| r.published.clone()),
            "page": latest.map(|r| r.page.clone()),
            "installable": latest.is_some_and(|r| r.package.is_some() && r.sums.is_some()),
            "checked_seconds_ago": s.checked.map(|t| t.elapsed().as_secs()),
            "error": s.error,
            "installing": s.installing,
            "repo": REPO,
        })
    }

    /// Version disponible, si plus récente (pour l'icône).
    pub fn available(&self) -> Option<String> {
        let s = self.state.lock().unwrap();
        s.latest.as_ref().filter(|r| newer(&r.version, current())).map(|r| r.version.clone())
    }

    /// Demande à GitHub la dernière version publiée (bloquant).
    pub fn check(&self) -> Value {
        let result = fetch_latest();
        {
            let mut s = self.state.lock().unwrap();
            s.checked = Some(Instant::now());
            match result {
                Ok(r) => {
                    s.latest = Some(r);
                    s.error = None;
                }
                Err(e) => s.error = Some(format!("{e:#}")),
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
            let s = self.state.lock().unwrap();
            if s.installing.is_some() {
                bail!("une installation est déjà en cours");
            }
            s.latest.clone().context("aucune version connue : vérifiez d'abord")?
        };
        if !newer(&release.version, current()) {
            bail!("déjà à jour ({})", current());
        }
        let (Some(package), Some(sums)) = (&release.package, &release.sums) else {
            bail!("la version {} n'a pas de paquet de mise à jour", release.version);
        };
        self.state.lock().unwrap().installing = Some(release.version.clone());
        let result = self.download_and_launch(&release.version, package, sums);
        if result.is_err() {
            self.state.lock().unwrap().installing = None;
        }
        result.map(|_| release.version)
    }

    fn download_and_launch(&self, version: &str, package_url: &str, sums_url: &str) -> Result<()> {
        let dir = self.dir.join(version);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let zip = dir.join(PACKAGE);
        tracing::info!("mise à jour {version} : téléchargement");
        download(package_url, &zip)?;
        let sums = String::from_utf8(get(sums_url)?).context("SHA256SUMS illisible")?;
        let expected = sums
            .lines()
            .find_map(|l| {
                let (hash, name) = l.split_once(char::is_whitespace)?;
                (name.trim().trim_start_matches('*') == PACKAGE).then(|| hash.to_lowercase())
            })
            .context("empreinte du paquet absente de SHA256SUMS")?;
        let actual = hex::encode(Sha256::digest(std::fs::read(&zip)?));
        if actual != expected {
            bail!("empreinte du paquet incorrecte : téléchargement abîmé ou falsifié");
        }
        tracing::info!("mise à jour {version} : paquet vérifié, installation");
        launch_installer(&dir, &zip)
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(format!("kyber-admin/{}", current()))
        .build()
        .into()
}

fn get(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent().get(url).call().with_context(|| format!("téléchargement de {url}"))?;
    Ok(resp.body_mut().with_config().limit(512 * 1024 * 1024).read_to_vec()?)
}

fn download(url: &str, to: &Path) -> Result<()> {
    std::fs::write(to, get(url)?)?;
    Ok(())
}

fn fetch_latest() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .context("GitHub ne répond pas")?;
    let v: Value = serde_json::from_slice(&resp.body_mut().read_to_vec()?)?;
    let asset = |name: &str| {
        v["assets"].as_array().and_then(|a| {
            a.iter().find(|x| x["name"] == name).and_then(|x| x["browser_download_url"].as_str()).map(String::from)
        })
    };
    Ok(Release {
        version: v["tag_name"].as_str().context("version sans numéro")?.trim_start_matches('v').to_string(),
        notes: v["body"].as_str().unwrap_or("").to_string(),
        published: v["published_at"].as_str().unwrap_or("").to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        package: asset(PACKAGE),
        sums: asset(SUMS),
    })
}

/// PowerShell, détaché de la console (elle va s'arrêter) : décompresse et
/// exécute le `update.ps1` du paquet.
#[cfg(windows)]
fn launch_installer(dir: &Path, zip: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let out = dir.join("contenu");
    let log = dir.join("update.log");
    let script = format!(
        "Start-Transcript -Path '{log}' -Force; Expand-Archive -Path '{zip}' -DestinationPath '{out}' -Force; & '{out}\\update.ps1' -Source '{out}'; Stop-Transcript",
        log = log.display(),
        zip = zip.display(),
        out = out.display(),
    );
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    // Hors du « job » du service s'il y en a un : sinon l'arrêt de la console l'emporterait.
    if cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB).spawn().is_err() {
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()
            .context("lancement de PowerShell")?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_installer(dir: &Path, _zip: &Path) -> Result<()> {
    bail!("installation réservée à Windows (paquet vérifié dans {})", dir.display())
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
