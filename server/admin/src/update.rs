//! Mises à jour en un clic, depuis les versions publiées sur GitHub
//! (github.com/MonsieurNours/kybernow, dépôt public : aucun jeton).
//!
//! La console regarde la dernière version toutes les 6 heures (ou à la
//! demande). « Installer » télécharge `kybernow-update.zip`, vérifie son
//! empreinte dans `SHA256SUMS`, puis lance PowerShell, détaché : il
//! décompresse le paquet et exécute son `update.ps1`, qui arrête les
//! services (la console comprise), bascule les dossiers par renommage et
//! relance tout. Si la nouvelle console ne répond pas en 60 s, le script
//! remet les dossiers précédents et l'écrit dans `update/resultat.json`, que
//! la console relit à son démarrage. L'icône de la zone de notification est
//! relancée au démarrage de la console (`update/relaunch-tray`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const REPO: &str = "MonsieurNours/kybernow";
const PACKAGE: &str = "kybernow-update.zip";
const SUMS: &str = "SHA256SUMS";
/// Résultat de la dernière installation, écrit par `update.ps1` quand elle échoue.
const RESULT: &str = "resultat.json";
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
    /// PowerShell de l'installation en cours, suivi pour voir s'il s'arrête
    /// sans avoir remplacé la console.
    child: Option<std::process::Child>,
    /// Dernière installation qui n'a pas abouti (fin de son journal).
    install_error: Option<String>,
    /// Échec signalé par `update.ps1` (`update/resultat.json`) : version,
    /// raison, retour à la version précédente, journal.
    last_failure: Option<Value>,
}

pub struct Updater {
    dir: PathBuf,
    state: Mutex<State>,
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Numéro de version publiable : trois nombres (« 0.2.6 »). Il sert de nom
/// de dossier et passe dans un script PowerShell : rien d'autre n'est accepté.
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
            let message = failure["message"].as_str().unwrap_or("raison inconnue");
            let back = if failure["rolled_back"].as_bool() == Some(true) {
                format!("La version précédente ({}) a été remise en place.", current())
            } else {
                "Les fichiers n'ont pas été remplacés.".to_string()
            };
            let log = failure["log"].as_str().filter(|l| !l.is_empty()).map(|l| format!("\nJournal : {l}")).unwrap_or_default();
            tracing::error!("mise à jour {version} non installée : {message}");
            state.install_error = Some(format!("L'installation de {version} n'a pas abouti : {message}. {back}{log}"));
            state.last_failure = Some(failure);
        }
        Self { dir, state: Mutex::new(state) }
    }

    /// État pour la console et l'icône (sans appel réseau).
    pub fn status(&self) -> Value {
        let mut s = self.state.lock().unwrap();
        self.reap(&mut s);
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
            "install_error": s.install_error,
            // Échec écrit par update.ps1 : contrôle de santé raté, retour arrière.
            "last_failure": s.last_failure,
            "repo": REPO,
        })
    }

    /// PowerShell terminé alors que la console tourne encore : l'installation
    /// s'est arrêtée en route (sinon il aurait arrêté la console).
    fn reap(&self, s: &mut State) {
        let Some(child) = s.child.as_mut() else { return };
        let Ok(Some(exit)) = child.try_wait() else { return };
        let version = s.installing.take().unwrap_or_default();
        s.child = None;
        let dir = self.dir.join(&version);
        let log = dir.join("update.log");
        // Sans journal, la sortie de PowerShell dit pourquoi il n'a pas démarré.
        let log = if log.exists() { log } else { dir.join("lancement.log") };
        let tail = std::fs::read_to_string(&log)
            .map(|t| t.lines().rev().filter(|l| !l.trim().is_empty()).take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))
            .ok()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "aucune sortie : PowerShell s'est arrêté avant d'écrire".into());
        tracing::error!("mise à jour {version} interrompue ({exit}) :\n{tail}");
        s.install_error = Some(format!("L'installation de {version} s'est arrêtée ({exit}). Journal : {}\n{tail}", log.display()));
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
            let mut s = self.state.lock().unwrap();
            self.reap(&mut s);
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
        if !valid_version(&release.version) {
            bail!("numéro de version inattendu : {}", release.version);
        }
        {
            let mut s = self.state.lock().unwrap();
            s.installing = Some(release.version.clone());
            s.install_error = None;
            s.last_failure = None;
        }
        let _ = std::fs::remove_file(self.dir.join(RESULT));
        let result = self.download_and_launch(&release.version, package, sums);
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

    fn download_and_launch(&self, version: &str, package_url: &str, sums_url: &str) -> Result<Option<std::process::Child>> {
        if !valid_version(version) {
            bail!("numéro de version inattendu : {version}");
        }
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
        launch_installer(&dir, &zip).map(Some)
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
    let version = v["tag_name"].as_str().context("version sans numéro")?.trim_start_matches('v').to_string();
    if !valid_version(&version) {
        bail!("numéro de version inattendu sur GitHub : {version}");
    }
    Ok(Release {
        version,
        notes: v["body"].as_str().unwrap_or("").to_string(),
        published: v["published_at"].as_str().unwrap_or("").to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        package: asset(PACKAGE),
        sums: asset(SUMS),
    })
}

/// PowerShell, détaché de la console (elle va s'arrêter) : décompresse et
/// exécute le `update.ps1` du paquet.
///
/// Le script est écrit dans `lancement.ps1` (aucun guillemet à faire passer
/// par la ligne de commande). PowerShell reçoit une console cachée et une
/// sortie dans `lancement.log` : lancé depuis un service sans console ni
/// sortie valide (DETACHED_PROCESS, console 0.2.0), il pouvait quitter avant
/// d'écrire la moindre ligne.
/// Échec signalé par `update.ps1`, s'il y en a un.
fn read_failure(path: &Path) -> Option<Value> {
    let value: Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (value["ok"].as_bool() == Some(false)).then_some(value)
}

/// Chaîne entre apostrophes pour PowerShell (une apostrophe se double).
#[cfg_attr(not(windows), allow(dead_code))]
fn ps_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

#[cfg(windows)]
fn launch_installer(dir: &Path, zip: &Path) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let out = dir.join("contenu");
    let log = dir.join("update.log");
    let launcher = dir.join("lancement.ps1");
    // Sans barre de progression : Expand-Archive de PowerShell 5 en est
    // plusieurs fois plus lent. Toute erreur finit dans le journal.
    let script = format!(
        "$ProgressPreference = 'SilentlyContinue'\r\n\
         Start-Transcript -Path {log} -Force\r\n\
         try {{\r\n\
         \x20   Expand-Archive -Path {zip} -DestinationPath {out} -Force\r\n\
         \x20   & {script} -Source {out}\r\n\
         }} catch {{\r\n\
         \x20   Write-Output \"ERREUR : $($_.Exception.Message)\"\r\n\
         \x20   Write-Output $_.ScriptStackTrace\r\n\
         \x20   exit 1\r\n\
         }} finally {{\r\n\
         \x20   Stop-Transcript\r\n\
         }}\r\n",
        log = ps_quote(&log),
        zip = ps_quote(zip),
        script = ps_quote(&out.join("update.ps1")),
        out = ps_quote(&out),
    );
    std::fs::write(&launcher, script).context("écriture de lancement.ps1")?;
    let output = std::fs::File::create(dir.join("lancement.log")).context("création de lancement.log")?;
    let spawn = |flags: u32| -> std::io::Result<std::process::Child> {
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&launcher)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output.try_clone()?)
            .creation_flags(flags)
            .spawn()
    };
    // Hors du « job » du service s'il y en a un : sinon l'arrêt de la console l'emporterait.
    let child = spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP))
        .context("lancement de PowerShell")?;
    tracing::info!("mise à jour : PowerShell lancé (processus {})", child.id());
    Ok(child)
}

#[cfg(not(windows))]
fn launch_installer(dir: &Path, _zip: &Path) -> Result<std::process::Child> {
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
        std::fs::write(dir.join("update").join(RESULT), r#"{"version":"9.9.9","ok":true}"#).unwrap();
        assert!(Updater::new(&dir).status()["last_failure"].is_null());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn powershell_quotes_are_doubled() {
        assert_eq!(ps_quote(Path::new(r"C:\Program Files\K'N")), r"'C:\Program Files\K''N'");
    }
}
