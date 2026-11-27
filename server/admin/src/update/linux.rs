//! Mises à jour d'une Station Linux (docs/station-linux.md, jalon 6).
//!
//! Admin tourne avec le compte de l'utilisateur (service systemd `--user`) et
//! ne peut pas installer un paquet du système. Il télécharge et vérifie le
//! paquet comme les autres Stations (manifeste signé minisign, taille,
//! empreinte), dans `~/.local/share/kybernow/station/admin/update/<version>/`,
//! puis le confie au service root `kybernow-station-update@.service`, lancé
//! par systemd à chaque connexion sur le socket
//! `/run/kybernow-station/update.sock` (`kybernow-station-update.socket`).
//! Admin n'est jamais root : il ne fait que déposer une demande.
//!
//! Le service root ne croit rien de ce que dit la demande :
//!
//! 1. il lit qui la fait (`SO_PEERCRED`) et ne prend les fichiers que dans le
//!    dossier de mises à jour de ce compte ; il les copie dans son propre
//!    dossier (`/var/lib/kybernow-station/update/<version>/`) et y revérifie
//!    tout : signature du manifeste par la clé compilée dans Admin, version
//!    plus récente que le paquet installé, canal suivi, taille et empreinte du
//!    paquet. Un retour arrière n'installe que la version que lui-même a
//!    gardée, revérifiée de même ;
//! 2. il garde aussi le paquet de la version qui tourne (Admin le télécharge
//!    de sa version publiée, vérifié de même) : c'est lui qui sert au retour
//!    arrière ;
//! 3. il arrête l'icône, Kyber et Admin de ce compte, installe le paquet
//!    (`apt-get` ou `dpkg` pour un .deb, `dnf` ou `rpm` pour un .rpm), relance
//!    les services et attend qu'Admin réponde dans la nouvelle version (60 s) ;
//!    sinon, il réinstalle la version d'avant.
//!
//! Résultat dans `/var/lib/kybernow-station/update/resultat.json` (lisible par
//! tous, au même format que sous Windows et sur Mac), qu'Admin relit à son
//! démarrage ; journal à côté, dans `<version>/update.log`.

// Compilé aussi pour les tests sur Mac (sans le service root).
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::fs;
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::apply::{HEALTH_TIMEOUT, Journal};
use super::manifest::{self, Channel, Manifest};
use super::{RESULT, newer, valid_version};

/// Socket du service root (`kybernow-station-update.socket`).
pub const SOCKET: &str = "/run/kybernow-station/update.sock";
/// Dossier du service root : paquets gardés, journaux, résultat.
pub const STATE_DIR: &str = "/var/lib/kybernow-station/update";
/// Nom du paquet dans dpkg et rpm.
pub const PACKAGE_NAME: &str = "kybernow-station";
/// Données d'Admin, relatives au dossier personnel (`kybernow-admin.service`).
pub const DATA_DIR: &str = ".local/share/kybernow/station/admin";
/// Services de l'utilisateur, dans l'ordre de l'arrêt (l'inverse au démarrage).
pub const USER_UNITS: [&str; 3] = ["kybernow-tray.service", "kybernow-kyber.service", "kybernow-admin.service"];
/// Paquets publiés avec chaque version (`server/linux/scripts/release-station.sh`).
pub const ASSETS: [&str; 4] = [
    "KyberNow-Station-Linux-x86_64.deb",
    "KyberNow-Station-Linux-arm64.deb",
    "KyberNow-Station-Linux-x86_64.rpm",
    "KyberNow-Station-Linux-arm64.rpm",
];
/// Archives des systèmes immuables (Bazzite, Fedora Atomic, SteamOS), installées
/// dans le dossier de l'utilisateur (`server/linux/immutable/make-archive.sh`).
pub const ARCHIVES: [&str; 2] = ["KyberNow-Station-Linux-x86_64.tar.xz", "KyberNow-Station-Linux-aarch64.tar.xz"];
/// Script d'installation dans le dossier de l'utilisateur, à côté d'Admin.
pub const HOME_SCRIPT: &str = "kybernow-station-home";

/// Fichier où le service root note la version gardée pour le retour arrière.
const PREVIOUS: &str = "previous";

/// Format du paquet installé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Deb,
    Rpm,
    /// Archive installée dans le dossier de l'utilisateur, sans root.
    Archive,
}

/// Paquet publié pour une architecture (`uname -m` ou celle de Rust) et un format.
pub fn asset_name(arch: &str, format: Format) -> Option<&'static str> {
    let arm = match arch {
        "x86_64" | "amd64" => false,
        "aarch64" | "arm64" => true,
        _ => return None,
    };
    Some(match format {
        Format::Deb => ASSETS[usize::from(arm)],
        Format::Rpm => ASSETS[usize::from(arm) + 2],
        Format::Archive => ARCHIVES[usize::from(arm)],
    })
}

/// `bin/` d'une Station installée dans le dossier de l'utilisateur
/// (`…/kybernow-station/versions/<version>/bin/kyber-admin`), si `exe` en est une.
pub fn home_bin_of(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    let version = bin.parent()?;
    let versions = version.parent()?;
    let ok = bin.file_name()? == "bin" && versions.file_name()? == "versions" && !exe.starts_with("/usr");
    ok.then(|| bin.to_path_buf())
}

/// Cet Admin tourne depuis une installation dans le dossier de l'utilisateur :
/// ses mises à jour passent par `kybernow-station-home`, sans service root.
pub fn home_bin() -> Option<PathBuf> {
    static BIN: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    BIN.get_or_init(|| std::env::current_exe().ok().and_then(|e| e.canonicalize().ok()).and_then(|e| home_bin_of(&e))).clone()
}

/// Version gardée par `kybernow-station-home` (lien `previous`, à côté de `versions`).
pub fn home_previous(bin: &Path) -> Option<String> {
    let base = bin.parent()?.parent()?.parent()?;
    let target = fs::read_link(base.join("previous")).ok()?;
    let v = target.file_name()?.to_string_lossy().to_string();
    (valid_version(&v) && base.join("versions").join(&v).is_dir()).then_some(v)
}

/// Système ostree (Bazzite, Fedora Atomic) avec la Station en couche sous
/// `/usr` (rpm-ostree) : rien ne s'y installe depuis Admin.
pub fn ostree_layered() -> bool {
    Path::new("/run/ostree-booted").exists() && home_bin().is_none()
}

/// Lance `kybernow-station-home update|rollback` (il se détache seul, par
/// `systemd-run --user`), sortie dans `output`.
pub fn run_home(bin: &Path, args: &[std::ffi::OsString], output: &Path) -> Result<()> {
    let out = fs::File::create(output).with_context(|| format!("création de {}", output.display()))?;
    let status = std::process::Command::new(bin.join(HOME_SCRIPT))
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(out.try_clone()?)
        .stderr(out)
        .status()
        .with_context(|| format!("lancement de {}", bin.join(HOME_SCRIPT).display()))?;
    if !status.success() {
        let tail = fs::read_to_string(output).unwrap_or_default();
        bail!("{HOME_SCRIPT} : {status}{}", tail.lines().last().map(|l| format!(" ({l})")).unwrap_or_default());
    }
    Ok(())
}

/// Format du paquet installé sur cet ordinateur : connu de dpkg, sinon .rpm
/// s'il y a rpm (Fedora), sinon .deb.
pub fn format() -> Format {
    static FORMAT: std::sync::OnceLock<Format> = std::sync::OnceLock::new();
    *FORMAT.get_or_init(|| {
        if home_bin().is_some() {
            Format::Archive
        } else if Path::new("/var/lib/dpkg/info").join(format!("{PACKAGE_NAME}.list")).exists() {
            Format::Deb
        } else if Path::new("/usr/bin/rpm").exists() && !Path::new("/usr/bin/dpkg").exists() {
            Format::Rpm
        } else if std::process::Command::new("rpm").args(["-q", PACKAGE_NAME]).output().is_ok_and(|o| o.status.success()) {
            Format::Rpm
        } else {
            Format::Deb
        }
    })
}

/// Paquet de mise à jour de cette Station.
pub fn package() -> &'static str {
    asset_name(std::env::consts::ARCH, format()).unwrap_or(ASSETS[0])
}

/// Version d'un paquet telle que la donnent dpkg ou rpm (« 0.12.0 »,
/// « 0.12.0~beta1 », « 0.12.0-1 ») : les trois nombres, sinon rien.
pub fn plain_version(raw: &str) -> Option<String> {
    let v = raw.trim().split(['~', '-', '+']).next().unwrap_or("").to_string();
    valid_version(&v).then_some(v)
}

/// Le dossier du service root, et ce qu'il garde.
#[derive(Clone, Debug)]
pub struct Store {
    pub root: PathBuf,
}

impl Store {
    pub fn system() -> Self {
        Self { root: PathBuf::from(STATE_DIR) }
    }
    pub fn dir(&self, version: &str) -> PathBuf {
        self.root.join(version)
    }
    pub fn result(&self) -> PathBuf {
        self.root.join(RESULT)
    }
    /// Paquet gardé pour une version (vérifié à son arrivée).
    pub fn kept(&self, version: &str, package: &str) -> Option<PathBuf> {
        let dir = self.dir(version);
        let ok = [package, manifest::MANIFEST, manifest::SIGNATURE].iter().all(|f| dir.join(f).is_file());
        ok.then(|| dir.join(package))
    }
    /// Version gardée pour « Revenir à la version précédente », si son paquet est là.
    pub fn previous(&self, package: &str) -> Option<String> {
        let v = fs::read_to_string(self.root.join(PREVIOUS)).ok()?.trim().to_string();
        (valid_version(&v) && self.kept(&v, package).is_some()).then_some(v)
    }
    fn set_previous(&self, version: &str) {
        let _ = crate::durable::write(&self.root.join(PREVIOUS), version);
    }
    fn write_result(&self, value: Value) {
        let _ = fs::create_dir_all(&self.root);
        let _ = crate::durable::write(&self.result(), serde_json::to_vec_pretty(&value).unwrap_or_default());
    }
    /// Garde les dossiers des versions `keep`, retire les autres.
    fn prune(&self, keep: &[&str]) {
        let Ok(entries) = fs::read_dir(&self.root) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if valid_version(&name) && !keep.contains(&name.as_str()) && e.file_type().is_ok_and(|t| t.is_dir()) {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
}

/// Version gardée pour le retour arrière, vue d'Admin.
pub fn previous_version() -> Option<String> {
    if let Some(bin) = home_bin() {
        return home_previous(&bin);
    }
    Store::system().previous(package())
}

/// Demande d'Admin au service root, une ligne de JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Request {
    /// Installer `version`, téléchargée dans `update/<version>/` du compte.
    Apply { version: String },
    /// Remettre la version gardée par le service root.
    Rollback,
}

/// Réponse du service root, une fois la demande vérifiée (avant d'arrêter quoi que ce soit).
pub const ACCEPTED: &str = "ok";

/// Côté Admin : dépose la demande et attend que le service root l'ait
/// acceptée (fichiers copiés et vérifiés). La suite arrête Admin.
pub fn request(req: &Request) -> Result<()> {
    request_at(Path::new(SOCKET), req)
}

fn request_at(socket: &Path, req: &Request) -> Result<()> {
    use std::os::unix::net::UnixStream;
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        // Socket réservé au compte de la Station (groupe kybernow-station).
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            bail!("ce compte n'a pas le droit de mettre à jour KyberNow Station ({}) : seul le compte qui l'utilise le peut", socket.display())
        }
        Err(e) => {
            return Err(e).with_context(|| format!("service des mises à jour injoignable ({}) : réinstallez le paquet kybernow-station", socket.display()));
        }
    };
    // La vérification copie le paquet (jusqu'à quelques centaines de Mo).
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    writeln!(stream, "{}", serde_json::to_string(req)?)?;
    let mut reply = String::new();
    std::io::BufReader::new(Read::take(stream, 64 * 1024)).read_line(&mut reply)?;
    let reply = reply.trim();
    if reply == ACCEPTED {
        return Ok(());
    }
    bail!("{}", reply.strip_prefix("erreur : ").unwrap_or(if reply.is_empty() { "le service des mises à jour n'a pas répondu" } else { reply }))
}

/// Compte qui a fait la demande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub uid: u32,
    pub name: String,
    pub home: PathBuf,
}

impl User {
    /// Données d'Admin de ce compte.
    pub fn data_dir(&self) -> PathBuf {
        self.home.join(DATA_DIR)
    }
}

/// Ligne de `/etc/passwd` (ou de `getent passwd`) : nom, uid, dossier personnel.
pub fn parse_passwd(line: &str) -> Option<User> {
    let f: Vec<&str> = line.trim_end().split(':').collect();
    if f.len() < 7 {
        return None;
    }
    let (name, uid, home) = (f[0], f[2].parse().ok()?, f[5]);
    if name.is_empty() || !home.starts_with('/') {
        return None;
    }
    Some(User { uid, name: name.to_string(), home: PathBuf::from(home) })
}

/// Ce que le service root demande au système : paquets, services, santé.
pub trait Host {
    /// Version du paquet installé.
    fn installed_version(&mut self) -> Option<String>;
    /// Installe un paquet (`downgrade` : version plus ancienne que l'installée).
    fn install(&mut self, package: &Path, downgrade: bool, log: &mut Journal) -> Result<()>;
    fn stop_units(&mut self, user: &User, log: &mut Journal);
    fn start_units(&mut self, user: &User, log: &mut Journal);
    /// Admin répond-il à `address` en `version` ?
    fn health(&mut self, address: &str, version: &str, timeout: Duration) -> Result<()>;
}

/// Ce qui est vérifié, comme dans `apply::Checks`.
pub struct Checks<'a> {
    pub public_key: &'a str,
    pub package: &'a str,
    /// Compte du demandeur : ses fichiers sont ouverts avec ses droits
    /// (`setfsuid`), jamais avec ceux de root (un lien vers `/etc/shadow`
    /// échoue comme pour lui). `None` dans les essais.
    pub reader: Option<u32>,
}

/// Ouvre `from` sans suivre de lien ni attendre (une FIFO ne bloque pas),
/// avec les droits de `reader` s'il est donné (Linux).
fn open_plain(from: &Path, reader: Option<u32>) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let open = || fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(from);
    #[cfg(target_os = "linux")]
    if let Some(uid) = reader {
        // Les droits de fichiers du demandeur pour cette ouverture seulement
        // (setfsuid ne vaut que pour ce fil), puis ceux de root.
        let before = unsafe { libc::setfsuid(uid) };
        let file = open();
        unsafe { libc::setfsuid(before as libc::uid_t) };
        return file;
    }
    let _ = reader;
    open()
}

/// Copie `from` dans `to` (réservé à root : 0600), sans suivre de lien, au
/// plus `limit` octets. Le type et la taille sont lus sur le fichier ouvert
/// (`fstat`), pas sur le chemin, qui peut changer entre-temps.
fn copy_limited(from: &Path, to: &Path, limit: u64, reader: Option<u32>) -> Result<()> {
    let mut src = open_plain(from, reader).with_context(|| format!("{} introuvable, ou pas un fichier ordinaire", from.display()))?;
    let meta = src.metadata().with_context(|| format!("lecture de {}", from.display()))?;
    if !meta.file_type().is_file() {
        bail!("{} n'est pas un fichier ordinaire", from.display());
    }
    if meta.len() > limit {
        bail!("{} : {} octets, {limit} attendus au plus", from.display(), meta.len());
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    Read::take(&mut src, limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("{} grossit pendant la copie", from.display());
    }
    crate::durable::try_write_then(to, bytes, |tmp| {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(tmp, fs::Permissions::from_mode(0o600))
    })
    .with_context(|| format!("écriture de {}", to.display()))
}

/// Dossier du service root : réservé à root (0700).
fn private_dir(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).with_context(|| format!("droits de {}", dir.display()))
}

/// Prend une version dans `source` (dossier d'Admin) : manifeste, signature
/// puis paquet copiés dans le dossier du service root et vérifiés là.
/// `accept` : contrôle du manifeste vérifié (canal, version installée),
/// fait avant de copier le paquet (des dizaines de Mo).
pub fn stage(store: &Store, source: &Path, version: &str, checks: &Checks, accept: &dyn Fn(&Manifest) -> Result<()>) -> Result<Manifest> {
    if !valid_version(version) {
        bail!("version inattendue : {version}");
    }
    private_dir(&store.root)?;
    let dir = store.dir(version);
    private_dir(&dir)?;
    let files = [manifest::MANIFEST, manifest::SIGNATURE, checks.package];
    // Rien d'une demande précédente (le journal reste).
    for f in files {
        let _ = fs::remove_file(dir.join(f));
    }
    let limit = 256 * 1024;
    let staged = (|| -> Result<Manifest> {
        copy_limited(&source.join(manifest::MANIFEST), &dir.join(manifest::MANIFEST), limit, checks.reader)?;
        copy_limited(&source.join(manifest::SIGNATURE), &dir.join(manifest::SIGNATURE), limit, checks.reader)?;
        let m = verify_kept(&dir, version, checks.public_key)?;
        accept(&m)?;
        let expected = m.file(checks.package)?;
        copy_limited(&source.join(checks.package), &dir.join(checks.package), expected.size, checks.reader)?;
        m.check_file(checks.package, &dir.join(checks.package))?;
        Ok(m)
    })();
    if staged.is_err() {
        // Refusé : rien de ce qui a été recopié ne reste (manifeste et
        // signature compris), même réservé à root.
        for f in files {
            let _ = fs::remove_file(dir.join(f));
        }
    }
    staged
}

/// Relit et revérifie le manifeste gardé d'une version.
fn verify_kept(dir: &Path, version: &str, public_key: &str) -> Result<Manifest> {
    let json = fs::read(dir.join(manifest::MANIFEST))?;
    let sig = fs::read_to_string(dir.join(manifest::SIGNATURE))?;
    let m = manifest::verify(&json, &sig, public_key)?;
    if m.version != version {
        bail!("le manifeste signé annonce {}, pas {version}", m.version);
    }
    Ok(m)
}

/// Une version prête à installer : son paquet, gardé et vérifié.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub version: String,
    pub package: PathBuf,
}

/// Vérifie une demande d'installation (avant de répondre à Admin) : la
/// nouvelle version, et la version qui tourne pour le retour arrière.
pub fn prepare_apply(
    store: &Store,
    host: &mut dyn Host,
    user: &User,
    version: &str,
    channel: Channel,
    checks: &Checks,
    log: &mut Journal,
) -> Result<(Prepared, Option<Prepared>)> {
    let installed = host.installed_version().context("paquet kybernow-station introuvable (dpkg, rpm)")?;
    if !newer(version, &installed) {
        bail!("{version} n'est pas plus récente que la version installée ({installed})");
    }
    let source = user.data_dir().join("update");
    let accept = |m: &Manifest| m.check_for(version, channel, &installed, checks.package).map_err(|r| anyhow::anyhow!("{}", r.message()));
    stage(store, &source.join(version), version, checks, &accept)?;
    log.line(format!("    {version} : manifeste signé, paquet {} vérifié (taille, empreinte)", checks.package));
    let target = Prepared { version: version.to_string(), package: store.dir(version).join(checks.package) };
    // Le paquet de la version qui tourne, pour le retour arrière : déjà gardé,
    // ou téléchargé par Admin de sa version publiée.
    let fallback = match store.kept(&installed, checks.package).filter(|_| verify_kept(&store.dir(&installed), &installed, checks.public_key).is_ok()) {
        Some(package) => Some(Prepared { version: installed.clone(), package }),
        None if source.join(&installed).join(checks.package).is_file() => match stage(store, &source.join(&installed), &installed, checks, &|_| Ok(())) {
            Ok(_) => Some(Prepared { version: installed.clone(), package: store.dir(&installed).join(checks.package) }),
            Err(e) => {
                log.line(format!("    paquet de {installed} refusé ({e:#}) : pas de retour arrière possible"));
                None
            }
        },
        None => {
            log.line(format!("    paquet de {installed} absent : pas de retour arrière possible"));
            None
        }
    };
    Ok((target, fallback))
}

/// Vérifie une demande de retour arrière : la version gardée, et celle qui tourne.
pub fn prepare_rollback(store: &Store, host: &mut dyn Host, checks: &Checks) -> Result<(Prepared, Option<Prepared>)> {
    let previous = store.previous(checks.package).context("aucune version précédente gardée")?;
    verify_kept(&store.dir(&previous), &previous, checks.public_key)?.check_file(checks.package, &store.dir(&previous).join(checks.package))?;
    let target = Prepared { package: store.dir(&previous).join(checks.package), version: previous };
    let fallback = host.installed_version().and_then(|v| store.kept(&v, checks.package).map(|package| Prepared { version: v, package }));
    Ok((target, fallback))
}

/// Adresse d'Admin pour le contrôle de santé, lue dans ses réglages sans les
/// modifier (le service root ne fait que lire chez l'utilisateur).
pub fn admin_address(data_dir: &Path) -> String {
    let listen = fs::read_to_string(data_dir.join(crate::settings::FILE_NAME))
        .ok()
        .and_then(|t| toml::from_str::<crate::settings::Settings>(&t).ok())
        .and_then(|s| s.listen);
    listen.as_deref().and_then(super::apply::address_from_listen).unwrap_or_else(|| "127.0.0.1:9443".into())
}

/// Canal suivi par ce compte (lu de même).
pub fn user_channel(data_dir: &Path) -> Channel {
    fs::read_to_string(data_dir.join(crate::settings::FILE_NAME))
        .ok()
        .and_then(|t| toml::from_str::<crate::settings::Settings>(&t).ok())
        .map(|s| s.update_channel)
        .unwrap_or_default()
}

/// Installe `target`, relance la Station et contrôle sa santé ; sinon remet
/// `fallback` (la version qui tournait). Écrit le résultat.
pub fn switch(store: &Store, host: &mut dyn Host, user: &User, target: &Prepared, fallback: Option<&Prepared>, downgrade: bool, log: &mut Journal) -> Result<()> {
    let result = (|| -> Result<()> {
        log.line("==> Arrêt de la Station");
        host.stop_units(user, log);
        log.line(format!("==> Installation de {}", target.package.display()));
        if let Err(e) = host.install(&target.package, downgrade, log) {
            host.start_units(user, log);
            return Err(e.context("installation du paquet"));
        }
        log.line("==> Démarrage de la Station");
        host.start_units(user, log);
        let address = admin_address(&user.data_dir());
        log.line(format!("==> Contrôle : Admin en version {} sur {address}", target.version));
        let Err(e) = host.health(&address, &target.version, HEALTH_TIMEOUT) else { return Ok(()) };
        log.line(format!("ERREUR : {e:#}"));
        let Some(back) = fallback else {
            return Err(e.context("aucun paquet de la version d'avant pour revenir en arrière"));
        };
        log.line(format!("==> Retour à {}", back.version));
        host.stop_units(user, log);
        let undo = host.install(&back.package, !downgrade, log);
        host.start_units(user, log);
        match undo {
            Ok(()) => Err(e.context("version remise en place")),
            Err(u) => Err(e.context(format!("retour arrière impossible : {u:#}"))),
        }
    })();
    match &result {
        Ok(()) => {
            if let Some(back) = fallback {
                store.set_previous(&back.version);
            }
            let keep: Vec<&str> = [Some(target.version.as_str()), fallback.map(|b| b.version.as_str())].into_iter().flatten().collect();
            store.prune(&keep);
            store.write_result(json!({ "version": target.version, "ok": true, "log": log.path }));
            log.line(format!("Terminé : KyberNow Station {}.", target.version));
        }
        Err(e) => {
            let message = format!("{e:#}");
            let (rolled_back, undo_failed) = (message.contains("version remise en place"), message.contains("retour arrière impossible"));
            log.line(format!("ERREUR : {message}"));
            store.write_result(json!({
                "version": target.version, "ok": false, "rolled_back": rolled_back, "undo_failed": undo_failed,
                "message": message, "log": log.path,
            }));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Le service root, sur un vrai Linux.

#[cfg(target_os = "linux")]
mod system {
    use std::os::fd::AsRawFd;
    use std::process::Command;

    use super::*;

    /// Compte de l'autre bout du socket.
    pub fn peer_uid(stream: &std::os::unix::net::UnixStream) -> Result<u32> {
        let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let rc = unsafe {
            libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, (&raw mut cred).cast(), &mut len)
        };
        if rc != 0 {
            bail!("SO_PEERCRED : {}", std::io::Error::last_os_error());
        }
        Ok(cred.uid)
    }

    pub fn user(uid: u32) -> Result<User> {
        let out = Command::new("getent").args(["passwd", &uid.to_string()]).output().context("getent passwd")?;
        parse_passwd(&String::from_utf8_lossy(&out.stdout)).with_context(|| format!("compte {uid} introuvable"))
    }

    pub struct LinuxHost;

    fn run(cmd: &mut Command, log: &mut Journal) -> Result<()> {
        let out = cmd.output().with_context(|| format!("lancement de {cmd:?}"))?;
        for line in String::from_utf8_lossy(&out.stdout).lines().chain(String::from_utf8_lossy(&out.stderr).lines()) {
            if !line.trim().is_empty() {
                log.line(format!("    {line}"));
            }
        }
        if !out.status.success() {
            bail!("{:?} : {}", cmd.get_program(), out.status);
        }
        Ok(())
    }

    fn has(program: &str) -> bool {
        ["/usr/bin", "/bin", "/usr/sbin", "/sbin"].iter().any(|d| Path::new(d).join(program).exists())
    }

    /// `systemctl --user` dans la session de `user`.
    fn user_systemctl(user: &User, args: &[&str]) -> Command {
        let mut cmd = Command::new("runuser");
        cmd.args(["-u", &user.name, "--", "env", &format!("XDG_RUNTIME_DIR=/run/user/{}", user.uid)])
            .arg(format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus", user.uid))
            .args(["systemctl", "--user"])
            .args(args);
        cmd
    }

    impl Host for LinuxHost {
        fn installed_version(&mut self) -> Option<String> {
            let out = match format() {
                Format::Deb => Command::new("dpkg-query").args(["-W", "-f=${Version}", PACKAGE_NAME]).output(),
                Format::Rpm => Command::new("rpm").args(["-q", "--qf", "%{VERSION}", PACKAGE_NAME]).output(),
                // Installation dans le dossier de l'utilisateur : jamais le service root.
                Format::Archive => return None,
            }
            .ok()?;
            out.status.success().then(|| plain_version(&String::from_utf8_lossy(&out.stdout))).flatten()
        }

        fn install(&mut self, package: &Path, downgrade: bool, log: &mut Journal) -> Result<()> {
            match format() {
                // apt-get prend les dépendances nouvelles ; dpkg seul sinon.
                Format::Deb if has("apt-get") => run(
                    Command::new("apt-get")
                        .env("DEBIAN_FRONTEND", "noninteractive")
                        .args(["install", "-y", "--allow-downgrades", "-o", "Dpkg::Options::=--force-confold"])
                        // Attendre que les mises à jour automatiques (unattended-upgrades) aient fini.
                        .args(["-o", "DPkg::Lock::Timeout=300"])
                        .arg(package),
                    log,
                ),
                Format::Deb => run(Command::new("dpkg").arg("-i").arg(package), log),
                Format::Rpm if downgrade => run(Command::new("rpm").args(["-Uvh", "--oldpackage"]).arg(package), log),
                Format::Rpm if has("dnf") => run(Command::new("dnf").args(["install", "-y"]).arg(package), log),
                Format::Rpm => run(Command::new("rpm").arg("-Uvh").arg(package), log),
                Format::Archive => bail!("installation dans le dossier de l'utilisateur : kybernow-station-home, pas le service root"),
            }
        }

        fn stop_units(&mut self, user: &User, log: &mut Journal) {
            for unit in USER_UNITS {
                log.line(format!("    arrêt de {unit}"));
                let _ = run(&mut user_systemctl(user, &["stop", unit]), log);
            }
        }

        fn start_units(&mut self, user: &User, log: &mut Journal) {
            // Les services du paquet ont pu changer.
            let _ = run(&mut user_systemctl(user, &["daemon-reload"]), log);
            for unit in USER_UNITS.iter().rev() {
                match run(&mut user_systemctl(user, &["start", "--no-block", unit]), log) {
                    Ok(()) => log.line(format!("    {unit} lancé")),
                    Err(e) => log.line(format!("    {unit} : {e:#}")),
                }
            }
        }

        fn health(&mut self, address: &str, version: &str, timeout: Duration) -> Result<()> {
            super::super::apply::admin_answers(address, version, timeout)
        }
    }
}

/// Fichier du service root qui garde le compte propriétaire de la Station.
pub const OWNER_FILE: &str = "proprietaire";

/// Seul le compte de la Station (le premier à avoir demandé une mise à jour,
/// noté par root) pilote le service root : un autre compte de l'ordinateur
/// ne lance ni mise à jour, ni retour arrière, ni version bêta pour tous.
/// root est toujours accepté.
pub fn check_owner(store: &Store, uid: u32) -> Result<()> {
    if uid == 0 {
        return Ok(());
    }
    let path = store.root.join(OWNER_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => match text.trim().parse::<u32>() {
            Ok(owner) if owner == uid => Ok(()),
            Ok(owner) => bail!("seul le compte qui utilise KyberNow Station sur cet ordinateur (uid {owner}) peut la mettre à jour"),
            Err(_) => bail!("{} illisible : supprimez-le (root) pour le recréer", path.display()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            private_dir(&store.root)?;
            crate::durable::try_write_then(&path, uid.to_string(), |tmp| {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(tmp, fs::Permissions::from_mode(0o600))
            })
            .with_context(|| format!("écriture de {}", path.display()))
        }
        Err(e) => Err(e).with_context(|| format!("lecture de {}", path.display())),
    }
}

/// Une demande à la fois : verrou (`flock`) sur `<dossier>/.lock`, gardé
/// jusqu'à la fin de la bascule. `None` si une autre demande le tient.
pub fn lock_store(store: &Store) -> Result<Option<fs::File>> {
    use std::os::fd::AsRawFd;
    private_dir(&store.root)?;
    let file = fs::OpenOptions::new().create(true).truncate(false).write(true).open(store.root.join(".lock"))?;
    let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    Ok(taken.then_some(file))
}

/// Le service root (`kyber-admin update serve`), lancé par systemd pour une
/// connexion sur le socket (`Accept=yes` : la connexion est l'entrée standard).
#[cfg(target_os = "linux")]
pub fn serve() -> Result<()> {
    use std::os::fd::FromRawFd;
    use std::os::unix::net::UnixStream;

    let mut stream = unsafe { UnixStream::from_raw_fd(0) };
    let uid = system::peer_uid(&stream)?;
    // Une connexion muette ne garde pas le service (le socket n'en accepte que quelques-unes).
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut line = String::new();
    std::io::BufReader::new(Read::take(&stream, 4096)).read_line(&mut line)?;
    let store = Store::system();
    private_dir(&store.root)?;
    let Some(_lock) = lock_store(&store)? else {
        let _ = writeln!(stream, "erreur : une mise à jour de KyberNow Station est déjà en cours");
        bail!("une mise à jour est déjà en cours");
    };
    let mut host = system::LinuxHost;
    let package = package();
    let checks = Checks { public_key: manifest::RELEASE_KEY, package, reader: Some(uid) };
    let answer = |stream: &mut UnixStream, r: &Result<()>| {
        let text = match r {
            Ok(()) => ACCEPTED.to_string(),
            Err(e) => format!("erreur : {e:#}").replace('\n', " "),
        };
        let _ = writeln!(stream, "{text}");
        let _ = stream.flush();
    };
    let prepared = (|| -> Result<(User, Request, Prepared, Option<Prepared>, Journal)> {
        check_owner(&store, uid)?;
        let user = system::user(uid)?;
        let req: Request = serde_json::from_str(line.trim()).context("demande illisible")?;
        let (target, fallback, log) = match &req {
            Request::Apply { version } => {
                if !valid_version(version) {
                    bail!("version inattendue : {version}");
                }
                let mut log = Journal::open(store.dir(version).join("update.log"));
                log.line(format!("==> Mise à jour de KyberNow Station vers {version} (Linux, demandée par {})", user.name));
                let channel = user_channel(&user.data_dir());
                let (t, f) = prepare_apply(&store, &mut host, &user, version, channel, &checks, &mut log)?;
                (t, f, log)
            }
            Request::Rollback => {
                let (t, f) = prepare_rollback(&store, &mut host, &checks)?;
                let mut log = Journal::open(store.dir(&t.version).join("update.log"));
                log.line(format!("==> Retour à KyberNow Station {} (Linux, demandé par {})", t.version, user.name));
                (t, f, log)
            }
        };
        Ok((user, req, target, fallback, log))
    })();
    let (user, req, target, fallback, mut log) = match prepared {
        Ok(p) => p,
        Err(e) => {
            eprintln!("demande refusée : {e:#}");
            answer(&mut stream, &Err(anyhow::anyhow!("{e:#}")));
            // Rien ne reste d'une demande refusée (journal compris) dans le
            // dossier du service root, sauf une version déjà gardée.
            if let Ok(Request::Apply { version }) = serde_json::from_str::<Request>(line.trim())
                && valid_version(&version)
                && store.kept(&version, package).is_none()
            {
                let _ = fs::remove_dir_all(store.dir(&version));
            }
            return Err(e);
        }
    };
    answer(&mut stream, &Ok(()));
    drop(stream);
    let downgrade = matches!(req, Request::Rollback);
    switch(&store, &mut host, &user, &target, fallback.as_ref(), downgrade, &mut log)
}

#[cfg(test)]
mod tests {

    /// Un lien ou une FIFO à la place du manifeste : refusés sans les suivre
    /// ni bloquer, et rien ne reste dans le dossier du service root.
    #[test]
    fn links_and_fifos_are_never_copied() {
        let root = std::env::temp_dir().join(format!("kybernow-linux-links-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (source, out) = (root.join("source"), root.join("store"));
        fs::create_dir_all(&source).unwrap();
        fs::write(root.join("secret"), "root:x:0:0").unwrap();
        std::os::unix::fs::symlink(root.join("secret"), source.join("lien")).unwrap();
        assert!(copy_limited(&source.join("lien"), &out.join("copie"), 1024, None).is_err());
        let fifo = source.join("fifo");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        assert!(copy_limited(&fifo, &out.join("copie"), 1024, None).is_err());
        fs::write(source.join("vrai"), "{}").unwrap();
        private_dir(&out).unwrap();
        copy_limited(&source.join("vrai"), &out.join("copie"), 1024, None).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(out.join("copie")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&out).unwrap().permissions().mode() & 0o777, 0o700);
        let _ = fs::remove_dir_all(&root);
    }

    /// Le premier compte qui demande est noté ; un autre est refusé, root non.
    #[test]
    fn only_the_station_account_drives_the_root_service() {
        let root = std::env::temp_dir().join(format!("kybernow-linux-owner-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let store = Store { root: root.clone() };
        check_owner(&store, 1000).unwrap();
        check_owner(&store, 1000).unwrap();
        assert!(check_owner(&store, 1001).is_err());
        check_owner(&store, 0).unwrap();
        // Une demande à la fois.
        let first = lock_store(&store).unwrap();
        assert!(first.is_some());
        assert!(lock_store(&store).unwrap().is_none());
        drop(first);
        assert!(lock_store(&store).unwrap().is_some());
        let _ = fs::remove_dir_all(&root);
    }
    use std::collections::BTreeMap;

    use super::*;
    use crate::update::manifest::tests::{TestKey, sample, signed};
    use crate::update::manifest::FileEntry;

    const PKG: &str = "KyberNow-Station-Linux-arm64.deb";

    #[test]
    fn package_names_follow_arch_and_format() {
        assert_eq!(asset_name("x86_64", Format::Deb), Some("KyberNow-Station-Linux-x86_64.deb"));
        assert_eq!(asset_name("aarch64", Format::Deb), Some(PKG));
        assert_eq!(asset_name("arm64", Format::Rpm), Some("KyberNow-Station-Linux-arm64.rpm"));
        assert_eq!(asset_name("amd64", Format::Rpm), Some("KyberNow-Station-Linux-x86_64.rpm"));
        assert_eq!(asset_name("riscv64", Format::Deb), None);
        assert_eq!(asset_name("arm64", Format::Archive), Some("KyberNow-Station-Linux-aarch64.tar.xz"));
        assert_eq!(asset_name("x86_64", Format::Archive), Some("KyberNow-Station-Linux-x86_64.tar.xz"));
    }

    #[test]
    fn a_home_install_is_recognised_with_its_previous_version() {
        let dir = tmp("home");
        let base = dir.join(".local/lib/kybernow-station");
        for v in ["0.12.0", "0.13.0"] {
            fs::create_dir_all(base.join("versions").join(v).join("bin")).unwrap();
        }
        let bin = home_bin_of(&base.join("versions/0.13.0/bin/kyber-admin")).unwrap();
        assert_eq!(bin, base.join("versions/0.13.0/bin"));
        assert_eq!(home_previous(&bin), None);
        std::os::unix::fs::symlink(base.join("versions/0.12.0"), base.join("previous")).unwrap();
        assert_eq!(home_previous(&bin).as_deref(), Some("0.12.0"));
        assert!(home_bin_of(Path::new("/usr/lib/kybernow-station/bin/kyber-admin")).is_none());
        assert!(home_bin_of(&dir.join("bin/kyber-admin")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_versions_are_read_plainly() {
        assert_eq!(plain_version("0.12.0").as_deref(), Some("0.12.0"));
        assert_eq!(plain_version("0.12.0~beta1\n").as_deref(), Some("0.12.0"));
        assert_eq!(plain_version("0.12.0-1").as_deref(), Some("0.12.0"));
        assert_eq!(plain_version("1:0.12.0"), None);
        assert_eq!(plain_version(""), None);
    }

    #[test]
    fn passwd_lines() {
        let u = parse_passwd("lea:x:1000:1000:Léa,,,:/home/lea:/bin/bash\n").unwrap();
        assert_eq!((u.uid, u.name.as_str()), (1000, "lea"));
        assert_eq!(u.data_dir(), PathBuf::from("/home/lea/.local/share/kybernow/station/admin"));
        assert!(parse_passwd("lea:x:abc:1000::/home/lea:/bin/bash").is_none());
        assert!(parse_passwd("lea:x:1000:1000::relative:/bin/bash").is_none());
        assert!(parse_passwd("").is_none());
    }

    #[test]
    fn requests_are_one_json_line() {
        let r = Request::Apply { version: "0.12.0".into() };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"action":"apply","version":"0.12.0"}"#);
        assert_eq!(serde_json::from_str::<Request>(r#"{"action":"rollback"}"#).unwrap(), Request::Rollback);
        assert!(serde_json::from_str::<Request>(r#"{"action":"rm","path":"/"}"#).is_err());
    }

    #[test]
    fn the_admin_address_is_read_without_writing() {
        let dir = tmp("address");
        assert_eq!(admin_address(&dir), "127.0.0.1:9443");
        fs::write(dir.join(crate::settings::FILE_NAME), "listen = \"0.0.0.0:9555\"\nupdate_channel = \"beta\"\n").unwrap();
        assert_eq!(admin_address(&dir), "127.0.0.1:9555");
        assert_eq!(user_channel(&dir), Channel::Beta);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Le service root parle à Admin par une ligne, et rend son refus.
    #[test]
    fn admin_gets_the_answer_of_the_service() {
        use std::os::unix::net::UnixListener;
        let dir = tmp("socket");
        let socket = dir.join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            for reply in ["ok", "erreur : signature invalide"] {
                let (mut s, _) = listener.accept().unwrap();
                let mut line = String::new();
                std::io::BufReader::new(&s).read_line(&mut line).unwrap();
                assert!(line.contains("\"action\""));
                writeln!(s, "{reply}").unwrap();
            }
        });
        request_at(&socket, &Request::Rollback).unwrap();
        let e = request_at(&socket, &Request::Apply { version: "0.12.0".into() }).unwrap_err().to_string();
        assert_eq!(e, "signature invalide");
        server.join().unwrap();
        assert!(request_at(&dir.join("absent"), &Request::Rollback).unwrap_err().to_string().contains("injoignable"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kn-linux-maj-{name}-{}-{}", std::process::id(), rand::random::<u32>()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Faux système : paquets « installés » par leur contenu (la version).
    struct Fake {
        installed: Option<String>,
        /// Version qui répond au contrôle de santé (sinon, celle installée).
        broken: Option<String>,
        fail_install: Option<String>,
        calls: Vec<String>,
    }

    impl Host for Fake {
        fn installed_version(&mut self) -> Option<String> {
            self.installed.clone()
        }
        fn install(&mut self, package: &Path, downgrade: bool, _: &mut Journal) -> Result<()> {
            let v = fs::read_to_string(package)?;
            self.calls.push(format!("install {v}{}", if downgrade { " (retour)" } else { "" }));
            if self.fail_install.as_deref() == Some(v.as_str()) {
                bail!("dpkg : échec");
            }
            self.installed = Some(v);
            Ok(())
        }
        fn stop_units(&mut self, _: &User, _: &mut Journal) {
            self.calls.push("stop".into());
        }
        fn start_units(&mut self, _: &User, _: &mut Journal) {
            self.calls.push("start".into());
        }
        fn health(&mut self, _: &str, version: &str, _: Duration) -> Result<()> {
            if self.broken.as_deref() == Some(version) || self.installed.as_deref() != Some(version) {
                bail!("Admin ne répond pas en version {version}");
            }
            Ok(())
        }
    }

    struct Bench {
        dir: PathBuf,
        key: TestKey,
        store: Store,
        user: User,
    }

    impl Bench {
        fn new(name: &str) -> Self {
            let dir = tmp(name);
            let user = User { uid: 1000, name: "lea".into(), home: dir.join("home") };
            fs::create_dir_all(user.data_dir().join("update")).unwrap();
            Self { store: Store { root: dir.join("var") }, key: TestKey::new(), user, dir }
        }
        /// Version publiée, téléchargée par Admin dans son dossier.
        fn publish(&self, version: &str, channel: Channel) {
            let d = self.user.data_dir().join("update").join(version);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(PKG), version).unwrap();
            let mut m = sample(version, channel);
            m.files = BTreeMap::from([(PKG.to_string(), FileEntry { sha256: manifest::sha256_file(&d.join(PKG)).unwrap(), size: version.len() as u64 })]);
            let (json, sig) = signed(&self.key, &m);
            fs::write(d.join(manifest::MANIFEST), json).unwrap();
            fs::write(d.join(manifest::SIGNATURE), sig).unwrap();
        }
        fn checks(&self) -> (String, &'static str) {
            (self.key.public(), PKG)
        }
        fn log(&self) -> Journal {
            Journal::open(self.dir.join("log"))
        }
        fn result(&self) -> Value {
            serde_json::from_slice(&fs::read(self.store.result()).unwrap()).unwrap()
        }
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn fake(installed: &str) -> Fake {
        Fake { installed: Some(installed.into()), broken: None, fail_install: None, calls: vec![] }
    }

    /// Une bêta demandée sur le canal stable : refusée avant de copier le
    /// paquet, et rien ne reste dans le dossier du service root.
    #[test]
    fn a_refused_channel_copies_nothing() {
        let b = Bench::new("canal");
        b.publish("0.12.0", Channel::Beta);
        let (pk, package) = b.checks();
        let checks = Checks { public_key: &pk, package, reader: None };
        let mut log = b.log();
        assert!(prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.12.0", Channel::Stable, &checks, &mut log).is_err());
        let dir = b.store.dir("0.12.0");
        assert!(!dir.join(PKG).exists() && !dir.join(manifest::MANIFEST).exists());
    }

    #[test]
    fn an_update_installs_checks_health_and_keeps_the_previous_version() {
        let b = Bench::new("ok");
        b.publish("0.12.0", Channel::Stable);
        b.publish("0.11.0", Channel::Stable);
        let (pk, package) = b.checks();
        let checks = Checks { public_key: &pk, package, reader: None };
        let mut host = fake("0.11.0");
        let mut log = b.log();
        let (target, fallback) = prepare_apply(&b.store, &mut host, &b.user, "0.12.0", Channel::Stable, &checks, &mut log).unwrap();
        assert_eq!(fallback.as_ref().unwrap().version, "0.11.0");
        switch(&b.store, &mut host, &b.user, &target, fallback.as_ref(), false, &mut log).unwrap();
        assert_eq!(host.calls, ["stop", "install 0.12.0", "start"]);
        assert_eq!(b.result()["ok"], true);
        assert_eq!(b.store.previous(PKG).as_deref(), Some("0.11.0"));
        // Puis le retour arrière, vers la version gardée.
        host.calls.clear();
        let (target, fallback) = prepare_rollback(&b.store, &mut host, &checks).unwrap();
        assert_eq!((target.version.as_str(), fallback.as_ref().unwrap().version.as_str()), ("0.11.0", "0.12.0"));
        switch(&b.store, &mut host, &b.user, &target, fallback.as_ref(), true, &mut log).unwrap();
        assert_eq!(host.calls, ["stop", "install 0.11.0 (retour)", "start"]);
        assert_eq!(b.store.previous(PKG).as_deref(), Some("0.12.0"));
    }

    #[test]
    fn a_silent_admin_brings_back_the_previous_version() {
        let b = Bench::new("sante");
        b.publish("0.12.0", Channel::Stable);
        b.publish("0.11.0", Channel::Stable);
        let (pk, package) = b.checks();
        let checks = Checks { public_key: &pk, package, reader: None };
        let mut host = fake("0.11.0");
        host.broken = Some("0.12.0".into());
        let mut log = b.log();
        let (target, fallback) = prepare_apply(&b.store, &mut host, &b.user, "0.12.0", Channel::Stable, &checks, &mut log).unwrap();
        assert!(switch(&b.store, &mut host, &b.user, &target, fallback.as_ref(), false, &mut log).is_err());
        assert_eq!(host.calls, ["stop", "install 0.12.0", "start", "stop", "install 0.11.0 (retour)", "start"]);
        let r = b.result();
        assert_eq!((r["ok"].as_bool(), r["rolled_back"].as_bool(), r["undo_failed"].as_bool()), (Some(false), Some(true), Some(false)));
        assert_eq!(host.installed.as_deref(), Some("0.11.0"));
        // Le retour raté aussi : il est dit.
        host.installed = Some("0.11.0".into());
        host.fail_install = Some("0.11.0".into());
        host.calls.clear();
        let mut log = b.log();
        let (target, fallback) = prepare_apply(&b.store, &mut host, &b.user, "0.12.0", Channel::Stable, &checks, &mut log).unwrap();
        assert!(switch(&b.store, &mut host, &b.user, &target, fallback.as_ref(), false, &mut log).is_err());
        assert_eq!(b.result()["undo_failed"], true);
    }

    #[test]
    fn without_the_running_package_there_is_no_way_back() {
        let b = Bench::new("sans-retour");
        b.publish("0.12.0", Channel::Stable);
        let (pk, package) = b.checks();
        let checks = Checks { public_key: &pk, package, reader: None };
        let mut host = fake("0.11.0");
        host.broken = Some("0.12.0".into());
        let mut log = b.log();
        let (target, fallback) = prepare_apply(&b.store, &mut host, &b.user, "0.12.0", Channel::Stable, &checks, &mut log).unwrap();
        assert!(fallback.is_none());
        let e = switch(&b.store, &mut host, &b.user, &target, None, false, &mut log).unwrap_err().to_string();
        assert!(e.contains("aucun paquet"), "{e}");
        assert_eq!(b.result()["rolled_back"], false);
        assert!(prepare_rollback(&b.store, &mut host, &checks).is_err());
    }

    #[test]
    fn the_service_refuses_what_admin_cannot_vouch_for() {
        let b = Bench::new("refus");
        let (pk, package) = b.checks();
        let checks = Checks { public_key: &pk, package, reader: None };
        let mut log = b.log();
        // Pas plus récente que l'installée.
        b.publish("0.11.0", Channel::Stable);
        let e = prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.11.0", Channel::Stable, &checks, &mut log).unwrap_err();
        assert!(e.to_string().contains("pas plus récente"), "{e}");
        // Bêta sur un ordinateur qui suit le canal stable.
        b.publish("0.12.0", Channel::Beta);
        assert!(prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.12.0", Channel::Stable, &checks, &mut log).is_err());
        assert!(prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.12.0", Channel::Beta, &checks, &mut log).is_ok());
        // Paquet changé après la signature.
        b.publish("0.13.0", Channel::Stable);
        fs::write(b.user.data_dir().join("update/0.13.0").join(PKG), "0.13.1").unwrap();
        let e = prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.13.0", Channel::Stable, &checks, &mut log).unwrap_err();
        assert!(e.to_string().contains("empreinte"), "{e}");
        assert!(!b.store.dir("0.13.0").join(PKG).exists());
        // Signé par une autre clé.
        let other = TestKey::new();
        let opk = other.public();
        let e = prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.12.0", Channel::Beta, &Checks { public_key: &opk, package, reader: None }, &mut log);
        assert!(e.is_err());
        // Un lien à la place du paquet n'est pas suivi.
        b.publish("0.14.0", Channel::Stable);
        let p = b.user.data_dir().join("update/0.14.0").join(PKG);
        fs::remove_file(&p).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", &p).unwrap();
        let e = prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "0.14.0", Channel::Stable, &checks, &mut log).unwrap_err();
        assert!(e.to_string().contains("fichier ordinaire"), "{e}");
        // Une version hors format.
        assert!(prepare_apply(&b.store, &mut fake("0.11.0"), &b.user, "../0.12.0", Channel::Stable, &checks, &mut log).is_err());
    }
}
