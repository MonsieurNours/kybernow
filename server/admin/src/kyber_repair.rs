//! Réglages, certificat et clé de Kyber : lisibles ou abîmés, sauvegardes,
//! et réparation au démarrage d'Admin.
//!
//! Un gel du PC juste après une écriture peut laisser un fichier rempli de
//! zéros (le 03/11, après la mise à jour 0.9.3 : `kyber_config.toml` et
//! `kybertest_key.pem`, et Kyber ne démarrait plus). Au démarrage, Admin
//! remet des réglages illisibles depuis la dernière sauvegarde lisible
//! (`backups\`, puis `kyber.prev\`), et un certificat ou une clé illisibles
//! depuis la dernière paire sauvegardée, sinon `kyber.prev\`, sinon les
//! refait avec `cert-gen.exe`, comme l'installateur. L'applicateur des mises
//! à jour sauvegarde ces fichiers avant chaque mise à jour et chaque retour
//! arrière (`backup`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::durable;
use crate::i18n::Msg;

/// Paires certificat et clé gardées dans `backups\`.
const MAX_CERT_BACKUPS: usize = 10;
/// Sauvegardes de `kyber_config.toml` (comme `kyber_config::MAX_BACKUPS`).
const MAX_CONFIG_BACKUPS: usize = 30;

/// Les fichiers de Kyber qu'Admin surveille.
#[derive(Clone, Debug)]
pub struct KyberFiles {
    pub config: PathBuf,
    pub cert: PathBuf,
    pub key: PathBuf,
}

impl KyberFiles {
    pub fn of(kyber: &crate::kyber_config::KyberConfig) -> Self {
        KyberFiles { config: kyber.path().to_path_buf(), cert: kyber.tls_cert_path(), key: kyber.tls_key_path() }
    }

    /// Les mêmes noms dans le dossier `dir` (`kyber.prev`, `kyber.bad`).
    pub fn in_dir(&self, dir: &Path) -> Self {
        let at = |p: &Path| dir.join(p.file_name().unwrap_or_default());
        KyberFiles { config: at(&self.config), cert: at(&self.cert), key: at(&self.key) }
    }
}

/// Pourquoi `kyber_config.toml` est illisible pour Kyber (`None` : lisible).
pub fn config_problem(path: &Path) -> Option<String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return Some(format!("illisible ({e})")),
    };
    if bytes.is_empty() {
        return Some("vide".into());
    }
    if bytes.contains(&0) {
        return Some("octets nuls (fichier abîmé par une coupure)".into());
    }
    let Ok(text) = std::str::from_utf8(&bytes) else { return Some("pas en UTF-8".into()) };
    match text.parse::<toml::Table>() {
        Err(e) => Some(format!("TOML invalide : {}", e.message())),
        Ok(t) if !t.get("kycontroller").is_some_and(toml::Value::is_table) => Some("table [kycontroller] absente".into()),
        Ok(_) => None,
    }
}

/// Pourquoi un fichier PEM (certificat, ou clé si `private`) est illisible.
pub fn pem_problem(path: &Path, private: bool) -> Option<String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => return Some(format!("illisible ({e})")),
    };
    if bytes.contains(&0) {
        return Some("octets nuls (fichier abîmé par une coupure)".into());
    }
    let Ok(text) = std::str::from_utf8(&bytes) else { return Some("pas en texte".into()) };
    let begin = text.lines().map(str::trim).find(|l| l.starts_with("-----BEGIN ") && l.ends_with("-----"));
    let Some(label) = begin.map(|l| l.trim_start_matches("-----BEGIN ").trim_end_matches('-').to_string()) else {
        return Some("pas de bloc PEM".into());
    };
    let wanted = if private { label.ends_with("PRIVATE KEY") } else { label == "CERTIFICATE" };
    if !wanted {
        return Some(format!("bloc {label} inattendu"));
    }
    let end = format!("-----END {label}-----");
    let body: String = text
        .lines()
        .map(str::trim)
        .skip_while(|l| !l.starts_with("-----BEGIN "))
        .skip(1)
        .take_while(|l| *l != end)
        .collect();
    if !text.contains(&end) {
        return Some("bloc PEM tronqué".into());
    }
    let base64 = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=');
    if body.len() < 64 || body.len() % 4 != 0 || !body.chars().all(base64) {
        return Some("contenu PEM abîmé".into());
    }
    None
}

fn stamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(time::macros::format_description!("[year][month][day]-[hour][minute][second]-[subsecond digits:3]"))
        .unwrap_or_default()
}

/// `name-<stamp>.ext`, unique dans `dir`.
fn backup_name(dir: &Path, name: &str, ext: &str, stamp: &str) -> PathBuf {
    let mut dest = dir.join(format!("{name}-{stamp}.{ext}"));
    let mut n = 1;
    while dest.exists() {
        dest = dir.join(format!("{name}-{stamp}_{n:02}.{ext}"));
        n += 1;
    }
    dest
}

/// Garde les `max` plus récents des fichiers `prefix…ext` de `dir` (tri par nom, donc par date).
fn rotate(dir: &Path, prefix: &str, ext: &str, max: usize) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            n.starts_with(prefix) && n.ends_with(ext)
        })
        .collect();
    found.sort();
    for old in &found[..found.len().saturating_sub(max)] {
        let _ = fs::remove_file(old);
    }
}

/// Sauvegarde les réglages, le certificat et la clé de Kyber dans
/// `backup_dir` (`reason` : « avant la mise à jour 0.9.3 »…), chacun
/// seulement s'il est lisible : une copie abîmée chasserait une bonne
/// sauvegarde. Rend une ligne de journal par fichier.
pub fn backup(files: &KyberFiles, backup_dir: &Path, reason: &str) -> Vec<String> {
    let mut lines = Vec::new();
    if let Err(e) = fs::create_dir_all(backup_dir) {
        return vec![format!("sauvegarde des réglages de Kyber impossible ({}) : {e}", backup_dir.display())];
    }
    let stamp = stamp();
    if !files.config.exists() {
        lines.push(format!("{} absent : rien à sauvegarder", files.config.display()));
    } else if let Some(why) = config_problem(&files.config) {
        lines.push(format!("{} pas sauvegardé, illisible : {why}", files.config.display()));
    } else {
        let dest = backup_name(backup_dir, "kyber_config", "toml", &stamp);
        match durable::copy(&files.config, &dest) {
            Ok(_) => lines.push(format!("réglages de Kyber sauvegardés {reason} : {}", dest.display())),
            Err(e) => lines.push(format!("réglages de Kyber pas sauvegardés : {e}")),
        }
        rotate(backup_dir, "kyber_config-", ".toml", MAX_CONFIG_BACKUPS);
    }
    let cert_bad = pem_problem(&files.cert, false);
    let key_bad = pem_problem(&files.key, true);
    match (cert_bad, key_bad) {
        (None, None) => {
            let cert = backup_name(backup_dir, "kybertest_cert", "pem", &stamp);
            let key = cert.with_file_name(cert.file_name().unwrap().to_string_lossy().replace("kybertest_cert", "kybertest_key"));
            match durable::copy(&files.cert, &cert).and_then(|_| durable::copy(&files.key, &key)) {
                Ok(_) => lines.push(format!("certificat et clé de Kyber sauvegardés {reason}")),
                Err(e) => lines.push(format!("certificat et clé de Kyber pas sauvegardés : {e}")),
            }
            rotate(backup_dir, "kybertest_cert-", ".pem", MAX_CERT_BACKUPS);
            rotate(backup_dir, "kybertest_key-", ".pem", MAX_CERT_BACKUPS);
        }
        (c, k) => lines.push(format!(
            "certificat et clé de Kyber pas sauvegardés : {}",
            c.map(|w| format!("certificat {w}")).into_iter().chain(k.map(|w| format!("clé {w}"))).collect::<Vec<_>>().join(", ")
        )),
    }
    lines
}

/// Date d'un fichier, pour le dire à l'auteur (heure locale si connue).
fn file_date(path: &Path) -> String {
    let Ok(modified) = fs::metadata(path).and_then(|m| m.modified()) else { return "?".into() };
    let at = time::OffsetDateTime::from(modified);
    let at = time::UtcOffset::current_local_offset().map(|o| at.to_offset(o)).unwrap_or(at);
    at.format(time::macros::format_description!("[day]/[month]/[year] [hour]:[minute]")).unwrap_or_else(|_| "?".into())
}

/// Ce que la réparation a fait.
#[derive(Debug, Clone, PartialEq)]
pub enum Repair {
    /// Réglages remis depuis `from` (sauvegarde, ou `kyber.prev`), daté de `date`.
    Config { from: PathBuf, date: String, why: String },
    /// Réglages illisibles, aucune copie lisible.
    ConfigLost { why: String },
    /// Certificat et clé remis depuis une paire sauvegardée (ou `kyber.prev`).
    Cert { from: PathBuf, date: String, why: String },
    /// Certificat et clé refaits (`cert-gen.exe`).
    CertRegenerated { why: String },
    /// Certificat et clé illisibles, et pas refaits.
    CertLost { why: String, error: String },
}

impl Repair {
    /// Ligne de journal.
    pub fn log_line(&self) -> String {
        match self {
            Repair::Config { from, why, .. } => format!("kyber_config.toml illisible ({why}) : remis depuis {}", from.display()),
            Repair::ConfigLost { why } => format!("kyber_config.toml illisible ({why}) et aucune sauvegarde lisible : réinstaller la Station en gardant ses réglages"),
            Repair::Cert { from, why, .. } => format!("certificat ou clé de Kyber illisibles ({why}) : paire remise depuis {}", from.display()),
            Repair::CertRegenerated { why } => format!("certificat ou clé de Kyber illisibles ({why}) : refaits par cert-gen.exe"),
            Repair::CertLost { why, error } => format!("certificat ou clé de Kyber illisibles ({why}) et pas refaits : {error}"),
        }
    }

    /// Événement du fil d'Admin (`warn`).
    pub fn event(&self) -> Msg {
        match self {
            Repair::Config { date, .. } => Msg::new("events.kyberConfigRepaired").with("date", date.as_str()),
            Repair::ConfigLost { .. } => Msg::new("events.kyberConfigLost"),
            Repair::Cert { date, .. } => Msg::new("events.kyberCertRepaired").with("date", date.as_str()),
            Repair::CertRegenerated { .. } => Msg::new("events.kyberCertRegenerated"),
            Repair::CertLost { .. } => Msg::new("events.kyberCertLost"),
        }
    }

    /// Kyber est à relancer (un fichier a été remis).
    pub fn changed(&self) -> bool {
        matches!(self, Repair::Config { .. } | Repair::Cert { .. } | Repair::CertRegenerated { .. })
    }
}

/// Garde de côté le fichier abîmé (`<nom>.abime-<date>`), pour comprendre après coup.
fn set_aside(path: &Path) {
    let aside = path.with_file_name(format!("{}.abime-{}", path.file_name().unwrap_or_default().to_string_lossy(), stamp()));
    let _ = fs::rename(path, aside);
}

/// Les sauvegardes de `kyber_config.toml`, la plus récente d'abord
/// (`kyber_config-<date>.toml` d'Admin et de l'applicateur,
/// `kyber_config.toml.reinstallation-<date>` de l'installateur).
fn config_backups(backup_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(backup_dir) else { return Vec::new() };
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("kyber_config")))
        .filter_map(|p| Some((fs::metadata(&p).and_then(|m| m.modified()).ok()?, p)))
        .collect();
    found.sort_by(|a, b| b.cmp(a));
    found.into_iter().map(|(_, p)| p).collect()
}

/// Les paires certificat et clé sauvegardées, la plus récente d'abord.
fn cert_backups(backup_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let Ok(entries) = fs::read_dir(backup_dir) else { return Vec::new() };
    let mut certs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("kybertest_cert-")))
        .collect();
    certs.sort_by(|a, b| b.cmp(a));
    certs
        .into_iter()
        .map(|c| {
            let k = c.with_file_name(c.file_name().unwrap().to_string_lossy().replace("kybertest_cert-", "kybertest_key-"));
            (c, k)
        })
        .collect()
}

/// Répare les fichiers de Kyber abîmés (voir le haut du module). `prev` :
/// le dossier `kyber.prev` s'il existe ; `regenerate` refait le certificat
/// et la clé dans le dossier de Kyber (`cert_gen`). Ne touche à rien de lisible.
pub fn repair(files: &KyberFiles, prev: Option<&Path>, backup_dir: &Path, regenerate: &dyn Fn(&Path) -> Result<()>) -> Vec<Repair> {
    let mut done = Vec::new();
    if files.config.exists()
        && let Some(why) = config_problem(&files.config)
    {
        let prev_config = prev.map(|d| files.in_dir(d).config);
        let candidate = config_backups(backup_dir).into_iter().chain(prev_config).find(|p| config_problem(p).is_none());
        match candidate {
            Some(from) => {
                let date = file_date(&from);
                set_aside(&files.config);
                match durable::copy(&from, &files.config) {
                    Ok(_) => done.push(Repair::Config { from, date, why }),
                    Err(e) => done.push(Repair::ConfigLost { why: format!("{why} ; copie de {} : {e}", from.display()) }),
                }
            }
            None => done.push(Repair::ConfigLost { why }),
        }
    }
    let cert_bad = files.cert.exists().then(|| pem_problem(&files.cert, false)).flatten();
    let key_bad = files.key.exists().then(|| pem_problem(&files.key, true)).flatten();
    // Une moitié de paire absente compte aussi : Kyber ne démarre pas sans l'autre.
    let half = files.cert.exists() != files.key.exists();
    if cert_bad.is_some() || key_bad.is_some() || half {
        let why = [cert_bad.map(|w| format!("certificat {w}")), key_bad.map(|w| format!("clé {w}")), half.then(|| "paire incomplète".to_string())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
        let prev_pair = prev.map(|d| files.in_dir(d)).map(|f| (f.cert, f.key));
        let pair = cert_backups(backup_dir)
            .into_iter()
            .chain(prev_pair)
            .find(|(c, k)| pem_problem(c, false).is_none() && pem_problem(k, true).is_none());
        let restored = pair.and_then(|(c, k)| {
            let date = file_date(&c);
            for p in [&files.cert, &files.key] {
                if p.exists() {
                    set_aside(p);
                }
            }
            durable::copy(&c, &files.cert).and_then(|_| durable::copy(&k, &files.key)).ok().map(|_| (c, date))
        });
        match restored {
            Some((from, date)) => done.push(Repair::Cert { from, date, why }),
            None => {
                let dir = files.cert.parent().map(Path::to_path_buf).unwrap_or_default();
                for p in [&files.cert, &files.key] {
                    if p.exists() {
                        set_aside(p);
                    }
                }
                let ok = regenerate(&dir).and_then(|()| match (pem_problem(&files.cert, false), pem_problem(&files.key, true)) {
                    (None, None) => Ok(()),
                    (c, k) => bail!("cert-gen.exe n'a pas laissé une paire lisible ({})", c.or(k).unwrap_or_default()),
                });
                match ok {
                    Ok(()) => done.push(Repair::CertRegenerated { why }),
                    Err(e) => done.push(Repair::CertLost { why, error: format!("{e:#}") }),
                }
            }
        }
    }
    done
}

/// Refait le certificat et la clé de Kyber dans `dir` avec son `cert-gen.exe`,
/// comme l'installateur (`cert-gen.exe --output <kyber>`).
pub fn cert_gen(dir: &Path) -> Result<()> {
    let exe = dir.join("cert-gen.exe");
    if !exe.exists() {
        bail!("{} absent", exe.display());
    }
    let out = std::process::Command::new(&exe).arg("--output").arg(dir).output()?;
    if !out.status.success() {
        bail!("cert-gen.exe a échoué ({}) : {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
    }
    for f in ["kybertest_cert.pem", "kybertest_key.pem"] {
        let _ = durable::sync_file(&dir.join(f));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const CONFIG: &str = "[kycontroller]\nport = 8443\n";
    pub fn pem(label: &str) -> String {
        format!("-----BEGIN {label}-----\n{}\n-----END {label}-----\n", "QUJD".repeat(20))
    }

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn dir() -> Dir {
        let d = std::env::temp_dir().join(format!("kybernow-repair-{}", rand::random::<u32>()));
        fs::create_dir_all(d.join("kyber")).unwrap();
        Dir(d)
    }
    fn files(d: &Dir) -> KyberFiles {
        let k = d.0.join("kyber");
        KyberFiles { config: k.join("kyber_config.toml"), cert: k.join("kybertest_cert.pem"), key: k.join("kybertest_key.pem") }
    }
    fn good(f: &KyberFiles, config: &str) {
        fs::write(&f.config, config).unwrap();
        fs::write(&f.cert, pem("CERTIFICATE")).unwrap();
        fs::write(&f.key, pem("PRIVATE KEY")).unwrap();
    }
    fn no_regen(_: &Path) -> Result<()> {
        bail!("pas de cert-gen ici")
    }

    #[test]
    fn fichiers_lisibles_ou_abimes() {
        let d = dir();
        let f = files(&d);
        good(&f, CONFIG);
        assert_eq!(config_problem(&f.config), None);
        assert_eq!(pem_problem(&f.cert, false), None);
        assert_eq!(pem_problem(&f.key, true), None);
        assert!(pem_problem(&f.cert, true).is_some(), "un certificat n'est pas une clé");
        // Le cas du 03/11 : fichiers remplis de zéros.
        fs::write(&f.config, vec![0u8; 18_329]).unwrap();
        fs::write(&f.key, vec![0u8; 1_704]).unwrap();
        assert!(config_problem(&f.config).unwrap().contains("nuls"));
        assert!(pem_problem(&f.key, true).unwrap().contains("nuls"));
        fs::write(&f.config, "port = 1\n").unwrap();
        assert!(config_problem(&f.config).unwrap().contains("kycontroller"));
        fs::write(&f.config, "[kycontroller\n").unwrap();
        assert!(config_problem(&f.config).unwrap().contains("TOML"));
        fs::write(&f.key, pem("PRIVATE KEY").replace("-----END PRIVATE KEY-----\n", "")).unwrap();
        assert!(pem_problem(&f.key, true).is_some(), "tronqué");
    }

    #[test]
    fn reglages_remis_depuis_la_derniere_sauvegarde_lisible() {
        let d = dir();
        let f = files(&d);
        let backups = d.0.join("backups");
        good(&f, "[kycontroller]\nport = 1111\n");
        assert!(backup(&f, &backups, "avant la mise à jour").iter().any(|l| l.contains("sauvegardés")));
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Une sauvegarde plus récente mais abîmée ne doit pas servir.
        fs::write(backups.join("kyber_config-29990101-000000-000.toml"), vec![0u8; 100]).unwrap();
        fs::write(&f.config, vec![0u8; 500]).unwrap();
        let done = repair(&f, None, &backups, &no_regen);
        assert!(matches!(&done[..], [Repair::Config { .. }]), "{done:?}");
        assert!(done[0].changed());
        assert_eq!(fs::read_to_string(&f.config).unwrap(), "[kycontroller]\nport = 1111\n");
        // Le fichier abîmé est gardé de côté.
        assert!(fs::read_dir(f.config.parent().unwrap()).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains(".abime-")));
        // Plus rien à réparer.
        assert!(repair(&f, None, &backups, &no_regen).is_empty());
    }

    #[test]
    fn reglages_remis_depuis_kyber_prev_sans_sauvegarde() {
        let d = dir();
        let f = files(&d);
        let prev = d.0.join("kyber.prev");
        fs::create_dir_all(&prev).unwrap();
        good(&f.in_dir(&prev), "[kycontroller]\nport = 2222\n");
        good(&f, CONFIG);
        fs::write(&f.config, vec![0u8; 10]).unwrap();
        let done = repair(&f, Some(&prev), &d.0.join("backups"), &no_regen);
        assert!(matches!(&done[..], [Repair::Config { from, .. }] if from.starts_with(&prev)), "{done:?}");
        assert!(fs::read_to_string(&f.config).unwrap().contains("2222"));
        // Sans rien de lisible nulle part : dit, rien de remplacé.
        fs::write(&f.config, vec![0u8; 10]).unwrap();
        fs::write(f.in_dir(&prev).config, vec![0u8; 10]).unwrap();
        let done = repair(&f, Some(&prev), &d.0.join("backups"), &no_regen);
        assert!(matches!(&done[..], [Repair::ConfigLost { .. }]), "{done:?}");
        assert!(!done[0].changed());
    }

    #[test]
    fn cle_remise_depuis_la_paire_sauvegardee_sinon_refaite() {
        let d = dir();
        let f = files(&d);
        let backups = d.0.join("backups");
        good(&f, CONFIG);
        backup(&f, &backups, "avant la mise à jour");
        fs::write(&f.key, vec![0u8; 1704]).unwrap();
        let done = repair(&f, None, &backups, &no_regen);
        assert!(matches!(&done[..], [Repair::Cert { .. }]), "{done:?}");
        assert_eq!(pem_problem(&f.key, true), None);
        // Sans sauvegarde ni kyber.prev : cert-gen, comme l'installateur.
        let _ = fs::remove_dir_all(&backups);
        fs::write(&f.key, vec![0u8; 1704]).unwrap();
        let regen = |dir: &Path| -> Result<()> {
            fs::write(dir.join("kybertest_cert.pem"), pem("CERTIFICATE"))?;
            fs::write(dir.join("kybertest_key.pem"), pem("PRIVATE KEY"))?;
            Ok(())
        };
        let done = repair(&f, None, &backups, &regen);
        assert!(matches!(&done[..], [Repair::CertRegenerated { .. }]), "{done:?}");
        // cert-gen absent : dit, pour l'auteur.
        fs::write(&f.key, vec![0u8; 1704]).unwrap();
        let done = repair(&f, None, &backups, &no_regen);
        assert!(matches!(&done[..], [Repair::CertLost { .. }]), "{done:?}");
    }

    #[test]
    fn sauvegarde_seulement_des_fichiers_lisibles() {
        let d = dir();
        let f = files(&d);
        let backups = d.0.join("backups");
        good(&f, CONFIG);
        fs::write(&f.config, vec![0u8; 10]).unwrap();
        let lines = backup(&f, &backups, "avant le retour arrière");
        assert!(lines[0].contains("pas sauvegardé"), "{lines:?}");
        assert!(config_backups(&backups).is_empty());
        assert_eq!(cert_backups(&backups).len(), 1, "la paire, elle, est lisible");
    }
}
