//! Applicateur des mises à jour : `kyber-admin update apply <zip> --version X`
//! et `kyber-admin update rollback`.
//!
//! La console le lance elle-même, en SYSTEM, détaché, depuis une copie
//! d'elle-même (`<installation>\.staging\kyber-admin-maj.exe`) : il arrête la
//! console puis renomme son dossier, ce qu'il ne pourrait pas faire en
//! tournant depuis `admin\`.
//!
//! `apply` revérifie le manifeste signé et le paquet (rien ne vient de la
//! console qui l'a lancé), l'extrait dans `<installation>\.staging\<version>\`
//! (sous Program Files : lecture seule pour les utilisateurs), arrête l'icône,
//! le serveur puis la console, prépare chaque dossier (copie du dossier actuel,
//! réglages et certificat de Kyber compris, puis le paquet par-dessus), bascule
//! par renommage (`kyber` → `kyber.prev`, `.staging\<version>\kyber` →
//! `kyber`, de même pour `admin`, `tray` et `webclient` s'il y en a un), remet
//! les droits des services, relance et contrôle la santé : la console doit
//! répondre sur son port en 60 s avec la nouvelle version. Sinon tout est remis
//! en place. Le résultat va dans `update\resultat.json` et le journal dans
//! `update\<version>\update.log`.
//!
//! `rollback` fait la même bascule en sens inverse (`kyber.prev` → `kyber`),
//! avec le même contrôle de santé.
//!
//! Les services, les processus et le contrôle de santé passent par le trait
//! [`System`] : la bascule se teste sur un faux dossier d'installation.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::manifest::{self, Channel, Manifest};
use super::{PACKAGE, RESULT, valid_version};

/// Dossiers remplacés, dans l'ordre de la bascule (les trois premiers sont obligatoires).
pub const PARTS: [&str; 4] = ["kyber", "admin", "tray", "webclient"];
const REQUIRED: [&str; 3] = ["kyber", "admin", "tray"];
/// Nom de la copie de la console qui applique la mise à jour (jamais arrêtée
/// avec les processus `kyber-admin.exe`).
#[cfg_attr(not(windows), allow(dead_code))]
pub const APPLICATOR: &str = "kyber-admin-maj.exe";
pub const STAGING: &str = ".staging";
/// Journaux de Kyber et ancienne sauvegarde, jamais recopiés dans la nouvelle version.
const KYBER_SKIP: [&str; 2] = ["log", "avant-maj"];
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(clap::Subcommand)]
pub enum Action {
    /// Installe un paquet téléchargé et vérifié par la console
    Apply {
        /// Paquet `kybernow-update.zip` (son manifeste signé est à côté)
        zip: PathBuf,
        /// Version attendue (X.Y.Z)
        #[arg(long)]
        version: String,
        /// Dossier d'installation (par défaut %ProgramFiles%\KyberNow)
        #[arg(long)]
        install_dir: Option<PathBuf>,
    },
    /// Remet la version précédente (dossiers `.prev`)
    Rollback {
        #[arg(long)]
        install_dir: Option<PathBuf>,
    },
}

/// Où est installé KyberNow.
#[derive(Clone, Debug)]
pub struct Layout {
    /// `C:\Program Files\KyberNow`
    pub install: PathBuf,
    /// `C:\ProgramData\KyberNow`
    pub data: PathBuf,
}

impl Layout {
    pub fn staging(&self) -> PathBuf {
        self.install.join(STAGING)
    }
    pub fn updates(&self) -> PathBuf {
        self.data.join("update")
    }
    fn part(&self, name: &str) -> PathBuf {
        self.install.join(name)
    }
    fn with_suffix(&self, name: &str, suffix: &str) -> PathBuf {
        self.install.join(format!("{name}.{suffix}"))
    }
    /// Version précédente gardée par la dernière mise à jour.
    pub fn has_previous(&self) -> bool {
        REQUIRED.iter().all(|p| self.with_suffix(p, "prev").is_dir())
    }
}

pub fn default_install_dir() -> PathBuf {
    std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files")).join("KyberNow")
}

/// Ce que la bascule demande au système : services, processus, droits, santé.
pub trait System {
    /// Arrête l'icône ; vrai si elle tournait.
    fn stop_tray(&mut self) -> bool;
    /// Arrête le serveur Kyber et ses processus.
    fn stop_server(&mut self) -> Result<()>;
    /// Arrête la console et ses agents (pas l'applicateur).
    fn stop_console(&mut self) -> Result<()>;
    /// Droits des services et règle de pare-feu.
    fn service_rights(&mut self, layout: &Layout) -> Result<()>;
    /// Démarre la console puis le serveur.
    fn start_services(&mut self) -> Result<()>;
    /// La console répond à `address` (hôte:port) avec `version`, et le serveur tourne.
    fn health(&mut self, address: &str, version: &str, timeout: Duration) -> Result<()>;
    /// « Applications installées » (`DisplayVersion`).
    fn register_version(&mut self, version: &str) -> Result<()>;
    /// Version d'une console, lue sur l'exécutable (`--version`).
    fn exe_version(&mut self, exe: &Path) -> Result<String>;
}

/// Journal de l'applicateur (`update\<version>\update.log`), recopié sur la sortie.
pub struct Journal {
    file: Option<fs::File>,
    pub path: PathBuf,
}

impl Journal {
    pub fn open(path: PathBuf) -> Self {
        let file = path.parent().and_then(|d| fs::create_dir_all(d).ok()).and_then(|_| fs::OpenOptions::new().create(true).append(true).open(&path).ok());
        Self { file, path }
    }

    pub fn line(&mut self, text: impl AsRef<str>) {
        let now = time::OffsetDateTime::now_utc();
        let line = format!("{:02}:{:02}:{:02} {}", now.hour(), now.minute(), now.second(), text.as_ref());
        println!("{line}");
        if let Some(f) = self.file.as_mut() {
            let _ = write!(f, "{line}\r\n");
        }
    }
}

/// Chemin relatif d'une entrée du paquet, refusé s'il pourrait sortir du
/// dossier d'extraction (`..`, chemin absolu, lecteur, flux NTFS `:`) ou ne
/// pas désigner un fichier ordinaire sous Windows (périphériques `CON`,
/// `NUL`…, nom finissant par un point ou une espace, caractères interdits).
pub fn safe_entry_path(name: &str) -> Result<PathBuf> {
    if name.is_empty() || name.chars().any(|c| c.is_control() || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*')) {
        bail!("chemin refusé dans le paquet : {name:?}");
    }
    if name.starts_with('/') || name.starts_with('\\') {
        bail!("chemin absolu refusé dans le paquet : {name:?}");
    }
    let mut out = PathBuf::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => bail!("chemin sortant refusé dans le paquet : {name:?}"),
            p if p.ends_with('.') || p.ends_with(' ') || reserved_name(p) => bail!("nom refusé sous Windows dans le paquet : {name:?}"),
            p => out.push(p),
        }
    }
    if out.components().any(|c| !matches!(c, Component::Normal(_))) || out.as_os_str().is_empty() {
        bail!("chemin refusé dans le paquet : {name:?}");
    }
    Ok(out)
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`, avec ou sans extension.
fn reserved_name(part: &str) -> bool {
    let stem = part.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit() && stem.as_bytes()[3] != b'0')
}

/// Extrait le paquet dans `dest` (créé, doit être vide). Refuse les liens,
/// les chemins sortants et les doublons. Renvoie le nombre de fichiers.
pub fn extract(zip: &Path, dest: &Path) -> Result<usize> {
    let file = fs::File::open(zip).with_context(|| format!("ouverture de {}", zip.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("paquet illisible (zip)")?;
    fs::create_dir_all(dest)?;
    let mut files = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).context("entrée du paquet illisible")?;
        let name = entry.name().context("nom d'entrée illisible")?.into_owned();
        let rel = safe_entry_path(&name)?;
        if entry.enclosed_name().is_none() {
            bail!("chemin refusé dans le paquet : {name:?}");
        }
        if entry.is_symlink() {
            bail!("lien refusé dans le paquet : {name:?}");
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)
            .with_context(|| format!("entrée en double ou impossible à créer : {name:?}"))?;
        std::io::copy(&mut entry, &mut target).with_context(|| format!("extraction de {name:?}"))?;
        files += 1;
    }
    Ok(files)
}

/// Copie `from` dans `to` (fusion, les fichiers existants sont remplacés),
/// sans suivre de lien ; `skip` : noms ignorés au premier niveau.
pub fn copy_tree(from: &Path, to: &Path, skip: &[&str]) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("création de {}", to.display()))?;
    for entry in fs::read_dir(from).with_context(|| format!("lecture de {}", from.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
            continue;
        }
        let kind = entry.file_type()?;
        let (src, dst) = (entry.path(), to.join(&name));
        if kind.is_symlink() {
            continue;
        } else if kind.is_dir() {
            copy_tree(&src, &dst, &[])?;
        } else {
            // Un fichier en lecture seule ne se remplace pas (Copy-Item -Force le faisait).
            if let Ok(meta) = fs::symlink_metadata(&dst)
                && meta.permissions().readonly()
            {
                let mut perms = meta.permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                perms.set_readonly(false);
                let _ = fs::set_permissions(&dst, perms);
            }
            fs::copy(&src, &dst).with_context(|| format!("copie de {}", src.display()))?;
        }
    }
    Ok(())
}

/// Renomme en réessayant : un antivirus ou l'Explorateur peut garder un
/// fichier ouvert quelques secondes.
fn rename_retry(from: &Path, to: &Path) -> Result<()> {
    let mut tries = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if tries >= 9 => {
                return Err(e).with_context(|| format!("renommage de {} en {} impossible", from.display(), to.display()));
            }
            Err(_) => {
                tries += 1;
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn remove_dir(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("suppression de {}", path.display())),
    }
}

/// Adresse où joindre la console depuis l'ordinateur : `listen` de
/// kyber-admin.toml (toutes les adresses → 127.0.0.1 ou [::1], une adresse
/// précise → elle-même), sinon le `console_port` de kybernow.json, sinon 9443.
pub fn console_address(layout: &Layout) -> String {
    let from_settings = crate::settings::Settings::load(&layout.data).ok().and_then(|s| s.listen).and_then(|l| {
        let (host, port) = l.rsplit_once(':')?;
        let port: u16 = port.parse().ok()?;
        let host = match host {
            "" | "0.0.0.0" => "127.0.0.1",
            "[::]" => "[::1]",
            h => h,
        };
        Some(format!("{host}:{port}"))
    });
    let from_record = || {
        let v: Value = serde_json::from_slice(&fs::read(layout.install.join("kybernow.json")).ok()?).ok()?;
        v["console_port"].as_u64().and_then(|p| u16::try_from(p).ok()).map(|p| format!("127.0.0.1:{p}"))
    };
    from_settings.or_else(from_record).unwrap_or_else(|| "127.0.0.1:9443".into())
}

/// Version dans `kybernow.json` (écrite d'un bloc : fichier temporaire puis renommage).
fn record_version(layout: &Layout, version: &str) -> Result<()> {
    let path = layout.install.join("kybernow.json");
    let Ok(bytes) = fs::read(&path) else { return Ok(()) };
    let mut record: Value = serde_json::from_slice(&bytes).context("kybernow.json illisible")?;
    record["version"] = json!(version);
    let tmp = layout.install.join("kybernow.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn write_result(layout: &Layout, value: Value) {
    let dir = layout.updates();
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(RESULT), serde_json::to_vec_pretty(&value).unwrap_or_default());
}

/// Demande à la console qui démarre de relancer l'icône dans la session de l'utilisateur.
fn request_tray_relaunch(layout: &Layout) {
    let _ = fs::create_dir_all(layout.updates());
    let _ = fs::write(layout.updates().join("relaunch-tray"), b"");
}

/// Paquets téléchargés : les deux versions les plus récentes restent.
fn prune_updates(layout: &Layout, keep: &str) {
    let Ok(entries) = fs::read_dir(layout.updates()) else { return };
    let mut versions: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| valid_version(n))
        .collect();
    versions.sort_by(|a, b| {
        if super::newer(a, b) {
            std::cmp::Ordering::Less
        } else if super::newer(b, a) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    for old in versions.iter().skip(2).filter(|v| v.as_str() != keep) {
        let _ = fs::remove_dir_all(layout.updates().join(old));
    }
}

/// Un dossier basculé : `part` a pris la place de l'ancien, venu de `incoming`.
struct Swap {
    part: &'static str,
    /// D'où vient le nouveau dossier (`.staging\<version>\kyber`, ou
    /// `kyber.prev`) ; rien si le dossier est seulement retiré (`webclient`
    /// apporté par la version qu'on quitte).
    incoming: Option<PathBuf>,
    /// Où est parti l'ancien (`kyber.prev`, ou `kyber.bad`), s'il existait.
    old: Option<PathBuf>,
}

/// Plan d'une bascule : chaque dossier et son remplaçant (rien : retiré).
type Plan = Vec<(&'static str, Option<PathBuf>)>;

/// Bascule : chaque dossier actuel part dans `old_suffix`, son remplaçant prend sa place.
fn switch(layout: &Layout, plan: &Plan, old_suffix: &str, swaps: &mut Vec<Swap>, log: &mut Journal) -> Result<()> {
    for (part, incoming) in plan {
        let current = layout.part(part);
        let old = layout.with_suffix(part, old_suffix);
        let had = current.exists();
        if had {
            remove_dir(&old)?;
            rename_retry(&current, &old)?;
        }
        swaps.push(Swap { part, incoming: incoming.clone(), old: had.then_some(old) });
        match incoming {
            Some(incoming) => {
                rename_retry(incoming, &current)?;
                log.line(format!("    {part} remplacé"));
            }
            None => log.line(format!("    {part} retiré")),
        }
    }
    Ok(())
}

/// Remet chaque dossier où il était avant la bascule (dans l'ordre inverse).
/// Va jusqu'au bout même si un dossier résiste, et le dit.
fn undo(layout: &Layout, swaps: &mut Vec<Swap>, log: &mut Journal) -> Result<()> {
    let mut failed = Vec::new();
    while let Some(s) = swaps.pop() {
        let current = layout.part(s.part);
        if let (true, Some(old)) = (s.part == "kyber", &s.old) {
            move_kyber_log(&current, old);
        }
        let step = (|| -> Result<()> {
            if current.exists() {
                match &s.incoming {
                    Some(incoming) => {
                        remove_dir(incoming)?;
                        rename_retry(&current, incoming)?;
                    }
                    None => bail!("{} réapparu pendant la mise à jour", current.display()),
                }
            }
            if let Some(old) = &s.old {
                rename_retry(old, &current)?;
            }
            Ok(())
        })();
        match step {
            Ok(()) => log.line(format!("    {} remis", s.part)),
            Err(e) => {
                log.line(format!("    {} NON remis : {e:#}", s.part));
                failed.push(s.part);
            }
        }
    }
    if failed.is_empty() { Ok(()) } else { bail!("dossiers non remis : {}", failed.join(", ")) }
}

/// Journaux de Kyber : déplacés de l'ancien dossier vers le nouveau, jamais copiés.
fn move_kyber_log(from: &Path, to: &Path) {
    let (src, dst) = (from.join("log"), to.join("log"));
    if src.is_dir() && !dst.exists() {
        let _ = fs::rename(&src, &dst);
    }
}

/// Ce qu'on vérifie avant de toucher à quoi que ce soit.
pub struct Checks<'a> {
    /// Clé publique de publication (format minisign).
    pub public_key: &'a str,
    pub channel: Channel,
    /// Version installée (celle de cet exécutable).
    pub installed: &'a str,
}

/// Revérifie le manifeste signé (à côté du paquet, dans `update\<version>\`) et le paquet.
pub fn verify_package(layout: &Layout, zip: &Path, version: &str, checks: &Checks) -> Result<Manifest> {
    if !valid_version(version) {
        bail!("numéro de version inattendu : {version}");
    }
    let dir = layout.updates().join(version);
    let json = fs::read(dir.join(manifest::MANIFEST)).with_context(|| format!("manifeste absent de {}", dir.display()))?;
    let signature = fs::read_to_string(dir.join(manifest::SIGNATURE)).context("signature du manifeste absente")?;
    if !super::newer(version, checks.installed) {
        bail!("la version {version} n'est pas plus récente que celle installée ({}) : utilisez le retour en arrière", checks.installed);
    }
    let manifest = manifest::verify(&json, &signature, checks.public_key)?;
    manifest.check(version, checks.channel, checks.installed).map_err(|r| anyhow::anyhow!("{}", r.message()))?;
    manifest.check_file(PACKAGE, zip)?;
    Ok(manifest)
}

/// Installe le paquet `zip` (version `version`). Renvoie une erreur si la
/// mise à jour n'a pas abouti ; le résultat est aussi écrit pour la console.
pub fn apply(layout: &Layout, sys: &mut dyn System, zip: &Path, version: &str, checks: &Checks) -> Result<()> {
    let mut log = Journal::open(layout.updates().join(if valid_version(version) { version } else { "invalide" }).join("update.log"));
    log.line(format!("==> KyberNow {version} : vérification du manifeste signé et du paquet"));
    if let Err(e) = verify_package(layout, zip, version, checks) {
        log.line(format!("ERREUR : {e:#}"));
        return Err(e);
    }
    let staged = layout.staging().join(version);
    let package = staged.join("paquet");
    log.line(format!("==> Extraction dans {}", package.display()));
    let extracted = remove_dir(&staged).and_then(|_| extract(zip, &package)).and_then(|n| {
        for p in REQUIRED {
            if !package.join(p).is_dir() {
                bail!("paquet incomplet : dossier {p} absent");
            }
        }
        let inside = fs::read_to_string(package.join("version.txt")).context("version.txt absent du paquet")?;
        if inside.trim() != version {
            bail!("le paquet contient la version {} au lieu de {version}", inside.trim());
        }
        Ok(n)
    });
    match extracted {
        Ok(n) => log.line(format!("    {n} fichiers")),
        Err(e) => {
            log.line(format!("ERREUR : {e:#}"));
            let _ = remove_dir(&staged);
            return Err(e);
        }
    }
    // Tout dossier présent ici ou dans le paquet passe par la bascule : chacun
    // a ainsi son .prev, et un dossier sans .prev a été apporté par la mise à
    // jour (le retour en arrière le retire).
    let parts: Vec<&'static str> = PARTS.iter().copied().filter(|p| package.join(p).is_dir() || layout.part(p).is_dir()).collect();
    let (result, undo_failed) = run_switch(layout, sys, &mut log, version, || {
        let mut plan = Vec::new();
        for part in PARTS.iter().filter(|p| !parts.contains(p)) {
            remove_dir(&layout.with_suffix(part, "prev"))?;
        }
        // Copie du dossier actuel (réglages, certificat), puis le paquet par-dessus.
        for part in &parts {
            let target = staged.join(part);
            let current = layout.part(part);
            if current.is_dir() {
                copy_tree(&current, &target, if *part == "kyber" { &KYBER_SKIP } else { &[] })?;
            }
            if package.join(part).is_dir() {
                copy_tree(&package.join(part), &target, &[])?;
            }
            plan.push((*part, Some(target)));
        }
        Ok(plan)
    }, "prev", |layout| {
        move_kyber_log(&layout.with_suffix("kyber", "prev"), &layout.part("kyber"));
    });
    if undo_failed {
        // Les dossiers de la nouvelle version y sont peut-être encore : gardés.
        log.line(format!("    {} gardé pour remettre les dossiers à la main", staged.display()));
    } else {
        let _ = remove_dir(&staged);
    }
    prune_updates(layout, version);
    result
}

/// Remet la version précédente (dossiers `.prev`).
pub fn rollback(layout: &Layout, sys: &mut dyn System) -> Result<String> {
    if !layout.has_previous() {
        bail!("aucune version précédente à remettre (dossiers .prev absents)");
    }
    let previous = sys.exe_version(&layout.with_suffix("admin", "prev").join("kyber-admin.exe"))?;
    if !valid_version(&previous) {
        bail!("version précédente illisible : {previous}");
    }
    let mut log = Journal::open(layout.updates().join(&previous).join("update.log"));
    log.line(format!("==> Retour à KyberNow {previous} (version précédente)"));
    // Un dossier sans .prev a été apporté par la version qu'on quitte : retiré.
    let plan: Plan = PARTS
        .iter()
        .filter_map(|p| {
            let prev = layout.with_suffix(p, "prev");
            if prev.is_dir() {
                Some((*p, Some(prev)))
            } else {
                layout.part(p).is_dir().then_some((*p, None))
            }
        })
        .collect();
    let (result, _) = run_switch(layout, sys, &mut log, &previous, || Ok(plan), "bad", |layout| {
        // Réglages de Kyber changés depuis la mise à jour : gardés.
        let (bad, kyber) = (layout.with_suffix("kyber", "bad"), layout.part("kyber"));
        let _ = fs::copy(bad.join("kyber_config.toml"), kyber.join("kyber_config.toml"));
        move_kyber_log(&bad, &kyber);
    });
    if result.is_ok() {
        // La version retirée ne sert plus : elle se réinstalle depuis la page.
        for p in PARTS {
            let _ = remove_dir(&layout.with_suffix(p, "bad"));
        }
    }
    result.map(|()| previous)
}

/// Arrêt, bascule, droits, redémarrage, contrôle de santé ; retour arrière
/// si l'une de ces étapes échoue. Les services sont relancés quoi qu'il arrive.
fn run_switch(
    layout: &Layout,
    sys: &mut dyn System,
    log: &mut Journal,
    version: &str,
    prepare: impl FnOnce() -> Result<Plan>,
    old_suffix: &str,
    after_switch: impl Fn(&Layout),
) -> (Result<()>, bool) {
    log.line("==> Arrêt de l'icône, du serveur puis de la console");
    let tray_was_running = sys.stop_tray();
    let mut swaps = Vec::new();
    let steps = (|| -> Result<()> {
        sys.stop_server()?;
        sys.stop_console()?;
        log.line("==> Préparation des nouveaux dossiers");
        let plan = prepare()?;
        log.line(format!("==> Bascule (les dossiers remplacés partent en .{old_suffix})"));
        switch(layout, &plan, old_suffix, &mut swaps, log)?;
        after_switch(layout);
        log.line("==> Droits des services et pare-feu");
        sys.service_rights(layout)?;
        if tray_was_running {
            request_tray_relaunch(layout);
        }
        let _ = fs::remove_file(layout.updates().join(RESULT));
        log.line(format!("==> Redémarrage et contrôle de santé (la console doit répondre en {} s)", HEALTH_TIMEOUT.as_secs()));
        sys.start_services()?;
        sys.health(&console_address(layout), version, HEALTH_TIMEOUT)?;
        Ok(())
    })();
    match steps {
        Ok(()) => {
            log.line(format!("    KyberNow {version} en ligne"));
            if let Err(e) = record_version(layout, version) {
                log.line(format!("    kybernow.json non mis à jour : {e:#}"));
            }
            if let Err(e) = sys.register_version(version) {
                log.line(format!("    DisplayVersion non mise à jour : {e:#}"));
            }
            write_result(layout, json!({ "version": version, "ok": true, "log": log.path }));
            log.line(format!("Terminé : KyberNow {version}."));
            (Ok(()), false)
        }
        Err(e) => {
            log.line(format!("ERREUR : {e:#}"));
            let (mut rolled_back, mut undo_failed) = (false, false);
            if !swaps.is_empty() {
                log.line("==> Retour à la version qui tournait");
                // L'icône a pu être relancée par la nouvelle console : elle
                // tiendrait le dossier tray.
                sys.stop_tray();
                let _ = sys.stop_server();
                let _ = sys.stop_console();
                match undo(layout, &mut swaps, log) {
                    Ok(()) => rolled_back = true,
                    Err(u) => {
                        undo_failed = true;
                        log.line(format!("ERREUR pendant le retour arrière : {u:#} ; dossiers à vérifier dans {}", layout.install.display()));
                    }
                }
                if rolled_back {
                    let _ = sys.service_rights(layout);
                }
            }
            if tray_was_running {
                request_tray_relaunch(layout);
            }
            log.line("==> Démarrage des services");
            if let Err(s) = sys.start_services() {
                log.line(format!("    {s:#}"));
            }
            write_result(
                layout,
                json!({ "version": version, "ok": false, "rolled_back": rolled_back, "undo_failed": undo_failed, "message": format!("{e:#}"), "log": log.path }),
            );
            (Err(e), undo_failed)
        }
    }
}

/// Point d'entrée de `kyber-admin update …`.
pub fn run(action: Action, data_dir: &Path) -> Result<()> {
    let install_dir = match &action {
        Action::Apply { install_dir, .. } | Action::Rollback { install_dir } => install_dir.clone().unwrap_or_else(default_install_dir),
    };
    let layout = Layout { install: install_dir, data: data_dir.to_path_buf() };
    let mut sys = platform()?;
    match action {
        Action::Apply { zip, version, .. } => {
            let channel = crate::settings::Settings::load(data_dir).map(|s| s.update_channel).unwrap_or_default();
            let checks = Checks { public_key: manifest::RELEASE_KEY, channel, installed: super::current() };
            apply(&layout, sys.as_mut(), &zip, &version, &checks)
        }
        Action::Rollback { .. } => rollback(&layout, sys.as_mut()).map(|v| println!("Version {v} remise en place.")),
    }
}

#[cfg(windows)]
fn platform() -> Result<Box<dyn System>> {
    Ok(Box::new(windows::WindowsSystem))
}

#[cfg(not(windows))]
fn platform() -> Result<Box<dyn System>> {
    bail!("mise à jour de l'ordinateur réservée à Windows")
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail};
    use windows_service::service::{ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use super::{Layout, System};

    /// Droits Windows par défaut, plus démarrer et arrêter (RP, WP) pour les
    /// utilisateurs interactifs (IU) : l'icône pilote les services sans
    /// droits administrateur. Même chaîne que l'installateur et `update.ps1`.
    const SERVICE_SDDL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)";
    const KYBER_PROCESSES: [&str; 5] = ["kyavserver.exe", "kycontroller.exe", "kynputserver.exe", "kyclient.exe", "kyservice.exe"];
    const UNINSTALL_KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\KyberNow";
    /// Relance automatique de Kyber après une panne, posée par l'installateur
    /// (`server/installer/src/install.rs`) : suspendue pendant la bascule, pour
    /// que Windows ne relance pas `kyber` au milieu des renommages.
    const KYBER_RECOVERY: [&str; 6] = ["failure", "kyber", "reset=", "86400", "actions=", "restart/5000/restart/5000/restart/30000"];
    const NO_RECOVERY: [&str; 6] = ["failure", "kyber", "reset=", "0", "actions=", ""];
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub struct WindowsSystem;

    /// Outil de Windows par son chemin complet (jamais par le PATH).
    fn system32(tool: &str) -> PathBuf {
        let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        root.join("System32").join(tool)
    }

    fn run(tool: &str, args: &[&str]) -> Result<std::process::Output> {
        use std::os::windows::process::CommandExt;
        Command::new(system32(tool)).args(args).creation_flags(CREATE_NO_WINDOW).output().with_context(|| format!("lancement de {tool}"))
    }

    /// Sans `/T` : l'applicateur est un enfant de la console, il ne doit pas
    /// partir avec elle.
    fn kill(image: &str) -> bool {
        run("taskkill.exe", &["/F", "/IM", image]).is_ok_and(|o| o.status.success())
    }

    fn manager() -> Result<ServiceManager> {
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).context("connexion au gestionnaire de services")
    }

    fn stop_service(name: &str) -> Result<()> {
        let Ok(service) = manager()?.open_service(name, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP) else {
            return Ok(());
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match service.query_status()?.current_state {
                ServiceState::Stopped => return Ok(()),
                // En marche ou en train de démarrer (relancé entre-temps) : l'arrêt est redemandé.
                ServiceState::Running | ServiceState::StartPending | ServiceState::Paused => {
                    let _ = service.stop();
                }
                _ => {}
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        // Les processus sont arrêtés ensuite de toute façon.
        Ok(())
    }

    fn start_service(name: &str) -> Result<()> {
        let service = manager()?
            .open_service(name, ServiceAccess::QUERY_STATUS | ServiceAccess::START)
            .with_context(|| format!("service {name} introuvable"))?;
        if matches!(service.query_status()?.current_state, ServiceState::Running | ServiceState::StartPending) {
            return Ok(());
        }
        service.start::<&OsStr>(&[]).with_context(|| format!("démarrage du service {name}"))
    }

    fn service_running(name: &str) -> bool {
        manager()
            .and_then(|m| m.open_service(name, ServiceAccess::QUERY_STATUS).context("ouverture"))
            .and_then(|s| Ok(s.query_status()?.current_state))
            .is_ok_and(|s| s == ServiceState::Running)
    }

    impl System for WindowsSystem {
        fn stop_tray(&mut self) -> bool {
            let was = kill("kybernow-tray.exe");
            std::thread::sleep(Duration::from_secs(1));
            was
        }

        fn stop_server(&mut self) -> Result<()> {
            let _ = run("sc.exe", &NO_RECOVERY);
            stop_service("kyber")?;
            for p in KYBER_PROCESSES {
                kill(p);
            }
            Ok(())
        }

        fn stop_console(&mut self) -> Result<()> {
            stop_service("kyber-admin")?;
            // Agents de la console dans la session de l'utilisateur (l'applicateur
            // s'appelle kyber-admin-maj.exe : il n'est pas concerné).
            kill("kyber-admin.exe");
            // La console a pu relancer Kyber entre-temps (comptes, bouton Démarrer).
            stop_service("kyber")?;
            for p in KYBER_PROCESSES {
                kill(p);
            }
            std::thread::sleep(Duration::from_secs(2));
            Ok(())
        }

        fn service_rights(&mut self, layout: &Layout) -> Result<()> {
            for s in ["kyber", "kyber-admin"] {
                let out = run("sc.exe", &["sdset", s, SERVICE_SDDL])?;
                if !out.status.success() {
                    bail!("sc sdset {s} a échoué ({})", out.status);
                }
            }
            let program = format!("program={}", layout.install.join("admin").join("kyber-admin.exe").display());
            let _ = run("netsh.exe", &["advfirewall", "firewall", "delete", "rule", "name=KyberNow-bonjour-UDP"]);
            let _ = run(
                "netsh.exe",
                &["advfirewall", "firewall", "add", "rule", "name=KyberNow-bonjour-UDP", "dir=in", "action=allow", "protocol=UDP", "localport=5353", &program, "profile=private,domain"],
            );
            Ok(())
        }

        fn start_services(&mut self) -> Result<()> {
            let _ = run("sc.exe", &KYBER_RECOVERY);
            let admin = start_service("kyber-admin");
            let kyber = start_service("kyber");
            admin.and(kyber)
        }

        fn health(&mut self, address: &str, version: &str, timeout: Duration) -> Result<()> {
            let deadline = Instant::now() + timeout;
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(3)))
                // Certificat de la console, auto-signé : seule la version compte ici.
                .tls_config(ureq::tls::TlsConfig::builder().disable_verification(true).build())
                .http_status_as_error(false)
                // Jamais par un proxy (HTTPS_PROXY du système) : c'est l'ordinateur lui-même.
                .proxy(None)
                .build()
                .into();
            let url = format!("https://{address}/api/session");
            let mut last = String::from("aucune réponse");
            while Instant::now() < deadline {
                match agent.get(&url).call() {
                    Ok(mut resp) => {
                        let body: serde_json::Value =
                            resp.body_mut().read_to_vec().ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
                        match body["version"].as_str() {
                            Some(v) if v == version => {
                                if service_running("kyber") {
                                    return Ok(());
                                }
                                last = "le serveur Kyber ne démarre pas".into();
                            }
                            Some(v) => last = format!("la console répond en version {v} au lieu de {version}"),
                            // Console d'avant la 0.2.6 (retour arrière) : sa réponse suffit.
                            None if resp.status().is_success() && service_running("kyber") => return Ok(()),
                            None => last = format!("réponse inattendue de la console ({})", resp.status()),
                        }
                    }
                    Err(e) => last = format!("la console ne répond pas ({e})"),
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            bail!("{last} ({address}) après {} s", timeout.as_secs())
        }

        fn register_version(&mut self, version: &str) -> Result<()> {
            if !run("reg.exe", &["query", UNINSTALL_KEY])?.status.success() {
                return Ok(());
            }
            let out = run("reg.exe", &["add", UNINSTALL_KEY, "/v", "DisplayVersion", "/t", "REG_SZ", "/d", version, "/f"])?;
            if !out.status.success() {
                bail!("reg add a échoué ({})", out.status);
            }
            Ok(())
        }

        fn exe_version(&mut self, exe: &Path) -> Result<String> {
            exe_version(exe)
        }
    }

    /// « kyber-admin 0.2.5 » → « 0.2.5 ».
    pub fn exe_version(exe: &Path) -> Result<String> {
        use std::os::windows::process::CommandExt;
        let out = Command::new(exe).arg("--version").creation_flags(CREATE_NO_WINDOW).output().with_context(|| format!("lancement de {}", exe.display()))?;
        let text = String::from_utf8_lossy(&out.stdout);
        text.split_whitespace().last().map(str::to_string).with_context(|| format!("version de {} illisible", exe.display()))
    }
}

/// Version gardée dans les dossiers `.prev`, s'il y en a (bouton « Revenir à
/// la version précédente »).
#[cfg(windows)]
pub fn previous_version(layout: &Layout) -> Option<String> {
    if !layout.has_previous() {
        return None;
    }
    windows::exe_version(&layout.with_suffix("admin", "prev").join("kyber-admin.exe")).ok().filter(|v| valid_version(v))
}

#[cfg(not(windows))]
pub fn previous_version(_: &Layout) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Write as _;

    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::update::manifest::FileEntry;
    use crate::update::manifest::tests::{TestKey, sample, signed};

    /// Faux système : note les appels, contrôle de santé réglable.
    #[derive(Default)]
    struct Fake {
        calls: Vec<String>,
        /// Versions refusées au contrôle de santé.
        unhealthy: Vec<String>,
        tray_running: bool,
        registered: Option<String>,
    }

    impl System for Fake {
        fn stop_tray(&mut self) -> bool {
            self.calls.push("stop_tray".into());
            self.tray_running
        }
        fn stop_server(&mut self) -> Result<()> {
            self.calls.push("stop_server".into());
            Ok(())
        }
        fn stop_console(&mut self) -> Result<()> {
            self.calls.push("stop_console".into());
            Ok(())
        }
        fn service_rights(&mut self, _: &Layout) -> Result<()> {
            self.calls.push("rights".into());
            Ok(())
        }
        fn start_services(&mut self) -> Result<()> {
            self.calls.push("start".into());
            Ok(())
        }
        fn health(&mut self, address: &str, version: &str, _: Duration) -> Result<()> {
            self.calls.push(format!("health {address} {version}"));
            if self.unhealthy.iter().any(|v| v == version) {
                bail!("la nouvelle console ne répond pas");
            }
            Ok(())
        }
        fn register_version(&mut self, version: &str) -> Result<()> {
            self.registered = Some(version.into());
            Ok(())
        }
        fn exe_version(&mut self, exe: &Path) -> Result<String> {
            // Les faux exécutables contiennent « kyber-admin X.Y.Z ».
            Ok(fs::read_to_string(exe)?.split_whitespace().last().unwrap_or_default().to_string())
        }
    }

    struct Bench {
        root: PathBuf,
        layout: Layout,
        key: TestKey,
    }

    impl Bench {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("kybernow-apply-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let layout = Layout { install: root.join("Program Files/KyberNow"), data: root.join("ProgramData/KyberNow") };
            let w = |p: &str, text: &str| {
                let path = layout.install.join(p);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, text).unwrap();
            };
            w("kyber/kyavserver.exe", "kyavserver 0.2.5");
            w("kyber/kyber_config.toml", "port = 30000");
            w("kyber/log/kyber.log", "ancien journal");
            w("admin/kyber-admin.exe", "kyber-admin 0.2.5");
            w("tray/kybernow-tray.exe", "tray 0.2.5");
            w("kybernow.json", r#"{"version":"0.2.5","console_port":9555}"#);
            Self { root, layout, key: TestKey::new() }
        }

        fn read(&self, p: &str) -> String {
            fs::read_to_string(self.layout.install.join(p)).unwrap_or_else(|e| panic!("{p} : {e}"))
        }

        /// Paquet `version`, signé (manifeste et signature dans update\<version>\).
        fn package(&self, version: &str, extra: &[(&str, &str)]) -> PathBuf {
            let dir = self.layout.updates().join(version);
            fs::create_dir_all(&dir).unwrap();
            let zip_path = dir.join(PACKAGE);
            let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
            let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
            let mut entries = vec![
                ("kyber/kyavserver.exe".to_string(), format!("kyavserver {version}")),
                ("admin/kyber-admin.exe".to_string(), format!("kyber-admin {version}")),
                ("tray/kybernow-tray.exe".to_string(), format!("tray {version}")),
                ("update.ps1".to_string(), "# script".to_string()),
                ("version.txt".to_string(), format!("{version}\r\n")),
            ];
            entries.extend(extra.iter().map(|(n, t)| (n.to_string(), t.to_string())));
            for (name, text) in entries {
                zip.start_file(name, opts).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
            self.sign(version, &zip_path);
            zip_path
        }

        fn sign(&self, version: &str, zip_path: &Path) {
            let mut m = sample(version, Channel::Stable);
            m.files = BTreeMap::from([(
                PACKAGE.to_string(),
                FileEntry { sha256: manifest::sha256_file(zip_path).unwrap(), size: fs::metadata(zip_path).unwrap().len() },
            )]);
            let (json, sig) = signed(&self.key, &m);
            let dir = self.layout.updates().join(version);
            fs::write(dir.join(manifest::MANIFEST), json).unwrap();
            fs::write(dir.join(manifest::SIGNATURE), sig).unwrap();
        }

        fn apply(&self, sys: &mut Fake, zip: &Path, version: &str) -> Result<()> {
            let public = self.key.public();
            let checks = Checks { public_key: &public, channel: Channel::Stable, installed: "0.2.5" };
            apply(&self.layout, sys, zip, version, &checks)
        }

        fn result(&self) -> Value {
            serde_json::from_slice(&fs::read(self.layout.updates().join(RESULT)).unwrap()).unwrap()
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn entry_paths_never_leave_the_folder() {
        assert_eq!(safe_entry_path("kyber/webclient/index.html").unwrap(), PathBuf::from("kyber/webclient/index.html"));
        assert_eq!(safe_entry_path("admin\\kyber-admin.exe").unwrap(), PathBuf::from("admin/kyber-admin.exe"));
        assert!(safe_entry_path("tray/console.dll").is_ok());
        assert!(safe_entry_path("kyber/COM10.txt").is_ok());
        for bad in [
            "../x", "kyber/../../x", "kyber/..", "/etc/passwd", "\\Windows\\x", "C:/x", "kyber/a:flux", "", "./", "a\0b",
            "kyber/CON", "kyber/nul.txt", "admin/com1.log", "LPT9", "kyber/fin.", "kyber/fin ", "kyber/a?b", "kyber/a|b", "a<b", "a\"b",
        ] {
            assert!(safe_entry_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn extraction_refuses_escapes_and_links() {
        let bench = Bench::new("extract");
        let make = |name: &str, build: &dyn Fn(&mut zip::ZipWriter<fs::File>)| {
            let path = bench.root.join(name);
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            build(&mut zip);
            zip.finish().unwrap();
            path
        };
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let escape = make("escape.zip", &|z| {
            z.start_file("kyber/ok.txt", opts).unwrap();
            z.write_all(b"ok").unwrap();
            z.start_file("../dehors.txt", opts).unwrap();
            z.write_all(b"x").unwrap();
        });
        let e = extract(&escape, &bench.root.join("out1")).unwrap_err().to_string();
        assert!(e.contains("sortant"), "{e}");
        assert!(!bench.root.join("dehors.txt").exists());

        let link = make("link.zip", &|z| {
            z.add_symlink("kyber/lien", "../../Windows", opts).unwrap();
        });
        let e = extract(&link, &bench.root.join("out2")).unwrap_err().to_string();
        assert!(e.contains("lien"), "{e}");

        let double = make("double.zip", &|z| {
            z.start_file("a.txt", opts).unwrap();
            z.start_file("dir/../a.txt", opts).unwrap();
        });
        assert!(extract(&double, &bench.root.join("out3")).is_err());

        let fine = make("fine.zip", &|z| {
            z.add_directory("kyber/webclient/", opts).unwrap();
            z.start_file("kyber/webclient/index.html", opts).unwrap();
            z.write_all(b"<html>").unwrap();
        });
        assert_eq!(extract(&fine, &bench.root.join("out4")).unwrap(), 1);
        assert_eq!(fs::read_to_string(bench.root.join("out4/kyber/webclient/index.html")).unwrap(), "<html>");
    }

    #[test]
    fn successful_update_switches_folders_and_records_the_version() {
        let bench = Bench::new("ok");
        let zip = bench.package("0.2.6", &[("webclient/index.html", "web")]);
        let mut sys = Fake { tray_running: true, ..Fake::default() };
        bench.apply(&mut sys, &zip, "0.2.6").unwrap();

        assert_eq!(sys.calls, ["stop_tray", "stop_server", "stop_console", "rights", "start", "health 127.0.0.1:9555 0.2.6"]);
        // Nouveaux fichiers, réglages de Kyber gardés, journaux déplacés.
        assert_eq!(bench.read("kyber/kyavserver.exe"), "kyavserver 0.2.6");
        assert_eq!(bench.read("kyber/kyber_config.toml"), "port = 30000");
        assert_eq!(bench.read("kyber/log/kyber.log"), "ancien journal");
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.6");
        assert_eq!(bench.read("webclient/index.html"), "web");
        // Version précédente gardée.
        assert_eq!(bench.read("kyber.prev/kyavserver.exe"), "kyavserver 0.2.5");
        assert_eq!(bench.read("admin.prev/kyber-admin.exe"), "kyber-admin 0.2.5");
        assert!(!bench.layout.with_suffix("webclient", "prev").exists());
        assert!(bench.layout.has_previous());
        // Version inscrite, résultat écrit, icône à relancer, staging nettoyé.
        let record: Value = serde_json::from_str(&bench.read("kybernow.json")).unwrap();
        assert_eq!(record["version"], "0.2.6");
        assert_eq!(record["console_port"], 9555);
        assert_eq!(sys.registered.as_deref(), Some("0.2.6"));
        assert_eq!(bench.result()["ok"], true);
        assert!(bench.layout.updates().join("relaunch-tray").exists());
        assert!(!bench.layout.staging().join("0.2.6").exists());
        let log = fs::read_to_string(bench.layout.updates().join("0.2.6/update.log")).unwrap();
        assert!(log.contains("Terminé"), "{log}");
    }

    #[test]
    fn console_address_follows_the_listen_setting() {
        let bench = Bench::new("address");
        assert_eq!(console_address(&bench.layout), "127.0.0.1:9555");
        fs::create_dir_all(&bench.layout.data).unwrap();
        for (listen, expected) in [("0.0.0.0:9443", "127.0.0.1:9443"), ("192.168.1.5:9444", "192.168.1.5:9444"), ("[::]:9445", "[::1]:9445")] {
            fs::write(bench.layout.data.join(crate::settings::FILE_NAME), format!("listen = \"{listen}\"")).unwrap();
            assert_eq!(console_address(&bench.layout), expected);
        }
    }

    #[test]
    fn older_or_same_versions_are_not_applied() {
        let bench = Bench::new("downgrade");
        let zip = bench.package("0.2.5", &[]);
        let e = bench.apply(&mut Fake::default(), &zip, "0.2.5").unwrap_err().to_string();
        assert!(e.contains("pas plus récente"), "{e}");
    }

    #[test]
    fn webclient_added_by_an_update_leaves_with_the_rollback() {
        let bench = Bench::new("webclient");
        let zip = bench.package("0.2.6", &[("webclient/index.html", "web 0.2.6")]);
        bench.apply(&mut Fake::default(), &zip, "0.2.6").unwrap();
        assert_eq!(bench.read("webclient/index.html"), "web 0.2.6");
        assert!(!bench.layout.with_suffix("webclient", "prev").exists());
        rollback(&bench.layout, &mut Fake::default()).unwrap();
        assert!(!bench.layout.part("webclient").exists());
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.5");
    }

    #[test]
    fn folders_missing_from_the_package_still_get_a_previous_copy() {
        let bench = Bench::new("keep");
        fs::create_dir_all(bench.layout.part("webclient")).unwrap();
        fs::write(bench.layout.part("webclient").join("index.html"), "web 0.2.5").unwrap();
        // Ancien .prev d'une version plus ancienne : ne doit pas survivre.
        fs::create_dir_all(bench.layout.with_suffix("webclient", "prev")).unwrap();
        fs::write(bench.layout.with_suffix("webclient", "prev").join("index.html"), "web 0.2.3").unwrap();
        let zip = bench.package("0.2.6", &[]);
        bench.apply(&mut Fake::default(), &zip, "0.2.6").unwrap();
        assert_eq!(bench.read("webclient/index.html"), "web 0.2.5");
        assert_eq!(bench.read("webclient.prev/index.html"), "web 0.2.5");
        rollback(&bench.layout, &mut Fake::default()).unwrap();
        assert_eq!(bench.read("webclient/index.html"), "web 0.2.5");
    }

    #[test]
    fn failed_health_check_puts_everything_back() {
        let bench = Bench::new("rollback");
        let zip = bench.package("0.2.6", &[]);
        let mut sys = Fake { unhealthy: vec!["0.2.6".into()], ..Fake::default() };
        let e = bench.apply(&mut sys, &zip, "0.2.6").unwrap_err().to_string();
        assert!(e.contains("ne répond pas"), "{e}");

        assert_eq!(bench.read("kyber/kyavserver.exe"), "kyavserver 0.2.5");
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.5");
        assert_eq!(bench.read("tray/kybernow-tray.exe"), "tray 0.2.5");
        assert_eq!(bench.read("kyber/log/kyber.log"), "ancien journal");
        assert!(!bench.layout.has_previous());
        let record: Value = serde_json::from_str(&bench.read("kybernow.json")).unwrap();
        assert_eq!(record["version"], "0.2.5");
        let result = bench.result();
        assert_eq!(result["ok"], false);
        assert_eq!(result["rolled_back"], true);
        // Icône relancée par la nouvelle console arrêtée avant de remettre les
        // dossiers ; services relancés ensuite ; l'icône ne tournait pas avant.
        assert_eq!(sys.calls.iter().filter(|c| *c == "stop_tray").count(), 2);
        assert_eq!(sys.calls.last().map(String::as_str), Some("start"));
        assert_eq!(result["undo_failed"], false);
        assert!(!bench.layout.updates().join("relaunch-tray").exists());
        assert!(!bench.layout.staging().join("0.2.6").exists());
    }

    #[test]
    fn tampered_or_unsigned_packages_change_nothing() {
        let bench = Bench::new("tampered");
        let zip = bench.package("0.2.6", &[]);
        // Paquet modifié après la signature.
        let mut bytes = fs::read(&zip).unwrap();
        let last = bytes.len() - 30;
        bytes[last] ^= 0xff;
        fs::write(&zip, bytes).unwrap();
        let mut sys = Fake::default();
        assert!(bench.apply(&mut sys, &zip, "0.2.6").unwrap_err().to_string().contains("falsifié"));
        assert!(sys.calls.is_empty(), "{:?}", sys.calls);
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.5");

        // Version demandée différente de celle du manifeste.
        let zip = bench.package("0.2.7", &[]);
        assert!(bench.apply(&mut sys, &zip, "0.2.8").is_err());
        // Signature absente.
        fs::remove_file(bench.layout.updates().join("0.2.7").join(manifest::SIGNATURE)).unwrap();
        assert!(bench.apply(&mut sys, &zip, "0.2.7").is_err());
        assert!(sys.calls.is_empty());
        assert!(!bench.layout.staging().exists() || fs::read_dir(bench.layout.staging()).unwrap().next().is_none());
    }

    #[test]
    fn package_with_a_wrong_inner_version_is_refused_before_stopping() {
        let bench = Bench::new("inner");
        let dir = bench.layout.updates().join("0.2.6");
        let zip = bench.package("0.2.6", &[]);
        // Même paquet, mais version.txt annonce autre chose : resigné, il reste refusé.
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip).unwrap()).unwrap();
        let other = dir.join("autre.zip");
        let mut out = zip::ZipWriter::new(fs::File::create(&other).unwrap());
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let name = f.name().unwrap().into_owned();
            let mut text = String::new();
            std::io::Read::read_to_string(&mut f, &mut text).unwrap();
            out.start_file(name.clone(), opts).unwrap();
            out.write_all(if name == "version.txt" { b"0.2.9" } else { text.as_bytes() }).unwrap();
        }
        out.finish().unwrap();
        fs::rename(&other, &zip).unwrap();
        bench.sign("0.2.6", &zip);
        let mut sys = Fake::default();
        let e = bench.apply(&mut sys, &zip, "0.2.6").unwrap_err().to_string();
        assert!(e.contains("0.2.9"), "{e}");
        assert!(sys.calls.is_empty());
    }

    #[test]
    fn rollback_on_request_brings_the_previous_version_back() {
        let bench = Bench::new("back");
        assert!(rollback(&bench.layout, &mut Fake::default()).is_err());
        let zip = bench.package("0.2.6", &[]);
        bench.apply(&mut Fake::default(), &zip, "0.2.6").unwrap();
        // Réglage de Kyber changé après la mise à jour : il reste.
        fs::write(bench.layout.install.join("kyber/kyber_config.toml"), "port = 31000").unwrap();

        let mut sys = Fake::default();
        assert_eq!(rollback(&bench.layout, &mut sys).unwrap(), "0.2.5");
        assert_eq!(sys.calls, ["stop_tray", "stop_server", "stop_console", "rights", "start", "health 127.0.0.1:9555 0.2.5"]);
        assert_eq!(bench.read("kyber/kyavserver.exe"), "kyavserver 0.2.5");
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.5");
        assert_eq!(bench.read("kyber/kyber_config.toml"), "port = 31000");
        assert_eq!(bench.read("kyber/log/kyber.log"), "ancien journal");
        assert!(!bench.layout.has_previous());
        assert!(!bench.layout.with_suffix("kyber", "bad").exists());
        let record: Value = serde_json::from_str(&bench.read("kybernow.json")).unwrap();
        assert_eq!(record["version"], "0.2.5");
        assert_eq!(sys.registered.as_deref(), Some("0.2.5"));
    }

    #[test]
    fn failed_rollback_keeps_the_running_version() {
        let bench = Bench::new("back-fail");
        let zip = bench.package("0.2.6", &[]);
        bench.apply(&mut Fake::default(), &zip, "0.2.6").unwrap();
        let mut sys = Fake { unhealthy: vec!["0.2.5".into()], ..Fake::default() };
        assert!(rollback(&bench.layout, &mut sys).is_err());
        assert_eq!(bench.read("admin/kyber-admin.exe"), "kyber-admin 0.2.6");
        assert_eq!(bench.read("admin.prev/kyber-admin.exe"), "kyber-admin 0.2.5");
        assert!(bench.layout.has_previous());
        assert_eq!(bench.result()["rolled_back"], true);
    }

    #[test]
    fn only_the_two_newest_downloads_are_kept() {
        let bench = Bench::new("prune");
        for v in ["0.2.3", "0.2.4", "0.2.10", "0.2.9"] {
            fs::create_dir_all(bench.layout.updates().join(v)).unwrap();
        }
        prune_updates(&bench.layout, "0.2.3");
        let mut left: Vec<String> = fs::read_dir(bench.layout.updates()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        assert_eq!(left, ["0.2.10", "0.2.3", "0.2.9"]);
    }
}
