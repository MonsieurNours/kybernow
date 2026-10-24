//! Migrations de `kyber-admin.toml`.
//!
//! Le fichier porte la version de son format (`schema`, absent = 0). Chaque
//! migration fait passer le fichier d'une version à la suivante, dans l'ordre,
//! jusqu'à [`CURRENT`]. Une migration est idempotente (la rejouer ne change
//! plus rien : une console plus ancienne remise par un retour arrière peut
//! effacer `schema` en réécrivant le fichier, et la suivante repasse alors par
//! les mêmes étapes). Avant d'écrire, l'original est copié dans
//! `<données>/backups/kyber-admin.toml.schema-<N>-<horodatage>`, et chaque
//! changement est journalisé.
//!
//! Lancée par [`crate::settings::Settings::load`] (démarrage de la console,
//! `init`, `reset-password`) et à la main par `kyber-admin migrate`.
//!
//! Ajouter une migration : écrire `fn to_N(doc) -> Vec<String>` (les
//! changements faits, en français), l'ajouter à [`MIGRATIONS`], passer
//! [`CURRENT`] à N, et la tester ici sur un fichier de la version N-1.

use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, value};

use crate::settings::{DEFAULT_LISTEN, FILE_NAME};

/// Version du format écrite par cette console.
pub const CURRENT: i64 = 1;

struct Migration {
    /// Version atteinte (depuis `to - 1`).
    to: i64,
    /// Ce que fait la migration, pour le journal.
    what: &'static str,
    /// Transforme le document et rend la liste des changements faits.
    apply: fn(&mut DocumentMut) -> Vec<String>,
}

/// Dans l'ordre : la n-ième mène à la version n.
const MIGRATIONS: &[Migration] = &[Migration {
    to: 1,
    what: "adresse d'écoute normalisée",
    apply: to_1,
}];

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Pas encore de réglages (première installation).
    NoFile,
    /// Déjà au format de cette console.
    UpToDate,
    /// Écrit par une console plus récente (retour arrière) : laissé tel quel.
    /// Les champs inconnus sont gardés à la réécriture (`Settings::extra`).
    Newer(i64),
    Migrated {
        from: i64,
        to: i64,
        /// Copie de l'original (`None` en essai).
        backup: Option<PathBuf>,
        changes: Vec<String>,
    },
}

/// Version du format d'un document (`schema` absent ou illisible = 0).
fn schema_of(doc: &DocumentMut) -> i64 {
    doc.get("schema").and_then(Item::as_integer).unwrap_or(0)
}

/// Amène `<données>/kyber-admin.toml` au format [`CURRENT`]. `dry_run` :
/// rend ce qui serait fait sans rien écrire.
pub fn migrate(data_dir: &Path, dry_run: bool) -> Result<Outcome> {
    let path = data_dir.join(FILE_NAME);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Outcome::NoFile),
        Err(e) => return Err(e).with_context(|| format!("lecture de {}", path.display())),
    };
    let mut doc: DocumentMut = text.parse().with_context(|| format!("{} illisible", path.display()))?;
    let from = schema_of(&doc);
    if from > CURRENT {
        return Ok(Outcome::Newer(from));
    }
    if from == CURRENT {
        return Ok(Outcome::UpToDate);
    }

    let mut changes = Vec::new();
    for m in MIGRATIONS.iter().filter(|m| m.to > from) {
        for change in (m.apply)(&mut doc) {
            changes.push(format!("schéma {} → {} ({}) : {change}", m.to - 1, m.to, m.what));
        }
        doc["schema"] = value(m.to);
    }
    let backup = if dry_run {
        None
    } else {
        let dir = data_dir.join("backups");
        fs::create_dir_all(&dir).with_context(|| format!("création de {}", dir.display()))?;
        let backup = dir.join(format!("{FILE_NAME}.schema-{from}-{}", crate::display::now_secs()));
        fs::copy(&path, &backup).with_context(|| format!("sauvegarde dans {}", backup.display()))?;
        // Même écriture d'un bloc que Settings::save.
        let tmp = data_dir.join(format!("{FILE_NAME}.tmp"));
        fs::write(&tmp, doc.to_string()).with_context(|| format!("écriture de {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("remplacement de {}", path.display()))?;
        Some(backup)
    };
    Ok(Outcome::Migrated { from, to: CURRENT, backup, changes })
}

/// Migration au chargement des réglages : journalise, et n'empêche jamais la
/// console de démarrer (un fichier illisible est traité par `Settings::load`).
pub fn before_load(data_dir: &Path) {
    match migrate(data_dir, false) {
        Ok(Outcome::Migrated { from, to, backup, changes }) => {
            tracing::info!(
                "réglages migrés du schéma {from} au schéma {to} (original : {})",
                backup.as_deref().map_or_else(String::new, |b| b.display().to_string())
            );
            for change in changes {
                tracing::info!("  {change}");
            }
        }
        Ok(Outcome::Newer(n)) => tracing::warn!(
            "réglages au schéma {n}, plus récent que cette console ({CURRENT}) : laissés tels quels"
        ),
        Ok(Outcome::NoFile | Outcome::UpToDate) => {}
        Err(e) => tracing::error!("migration des réglages impossible : {e:#}"),
    }
}

/// `kyber-admin migrate [--dry-run]`.
pub fn command(data_dir: &Path, dry_run: bool) -> Result<()> {
    let path = data_dir.join(FILE_NAME);
    match migrate(data_dir, dry_run)? {
        Outcome::NoFile => println!("{} n'existe pas : rien à migrer.", path.display()),
        Outcome::UpToDate => println!("{} est déjà au schéma {CURRENT}.", path.display()),
        Outcome::Newer(n) => println!(
            "{} est au schéma {n}, plus récent que cette console ({CURRENT}) : laissé tel quel.",
            path.display()
        ),
        Outcome::Migrated { from, to, backup, changes } => {
            let verb = if dry_run { "serait migré" } else { "migré" };
            println!("{} {verb} du schéma {from} au schéma {to}.", path.display());
            for change in &changes {
                println!("  {change}");
            }
            if let Some(backup) = backup {
                println!("Original : {}", backup.display());
            }
        }
    }
    Ok(())
}

/// 0 → 1 : `listen` devient une adresse « IP:port » valide. Sans cela, une
/// adresse sans port (`listen = "0.0.0.0"`) empêche la console de démarrer
/// (« adresse d'écoute invalide »), et un port seul (`listen = 9443`) rend
/// tout le fichier illisible : il est mis de côté, mot de passe compris.
fn to_1(doc: &mut DocumentMut) -> Vec<String> {
    enum Fix {
        Keep,
        Set(String),
        Remove,
    }
    let Some(item) = doc.get("listen") else { return Vec::new() };
    let default_port: u16 = DEFAULT_LISTEN.rsplit(':').next().and_then(|p| p.parse().ok()).unwrap_or(9443);
    let fix = if let Some(s) = item.as_str() {
        let t = s.trim();
        if t.parse::<SocketAddr>().is_ok() {
            if t == s { Fix::Keep } else { Fix::Set(t.to_string()) }
        } else if let Ok(ip) = t.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
            Fix::Set(SocketAddr::new(ip, default_port).to_string())
        } else {
            Fix::Remove
        }
    } else {
        match item.as_integer().and_then(|p| u16::try_from(p).ok()) {
            Some(port) if port > 0 => Fix::Set(format!("0.0.0.0:{port}")),
            _ => Fix::Remove,
        }
    };
    let before = item.to_string().trim().to_string();
    match fix {
        Fix::Keep => Vec::new(),
        Fix::Set(new) => {
            doc["listen"] = value(new.as_str());
            vec![format!("listen {before} → \"{new}\"")]
        }
        Fix::Remove => {
            doc.remove("listen");
            vec![format!("listen {before} illisible : retiré (adresse par défaut {DEFAULT_LISTEN})")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn data_dir(name: &str, content: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kyber-admin-migrate-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        if let Some(c) = content {
            fs::write(dir.join(FILE_NAME), c).unwrap();
        }
        dir
    }

    fn migrated(content: &str) -> (String, Vec<String>) {
        let mut doc: DocumentMut = content.parse().unwrap();
        let changes = to_1(&mut doc);
        (doc.to_string(), changes)
    }

    #[test]
    fn listen_normalise() {
        let (out, changes) = migrated("listen = \"0.0.0.0\"\n");
        assert_eq!(out, "listen = \"0.0.0.0:9443\"\n");
        assert_eq!(changes.len(), 1);
        assert!(migrated("listen = \" 192.168.1.2:9443 \"\n").0.contains("\"192.168.1.2:9443\""));
        assert!(migrated("listen = \"[::]\"\n").0.contains("\"[::]:9443\""));
        assert!(migrated("listen = 8443\n").0.contains("\"0.0.0.0:8443\""));
        let (out, changes) = migrated("listen = \"n'importe quoi\"\nkyber_dir = \"C:/k\"\n");
        assert_eq!(out, "kyber_dir = \"C:/k\"\n");
        assert!(changes[0].contains("retiré"));
        // Déjà valide, ou absent : rien.
        assert!(migrated("listen = \"0.0.0.0:9443\"\n").1.is_empty());
        assert!(migrated("kyber_dir = \"C:/k\"\n").1.is_empty());
    }

    #[test]
    fn idempotente() {
        for content in ["listen = \"0.0.0.0\"\n", "listen = 8443\n", "listen = \"?\"\n", ""] {
            let (once, _) = migrated(content);
            let (twice, changes) = migrated(&once);
            assert_eq!(once, twice, "{content}");
            assert!(changes.is_empty(), "{content}");
        }
    }

    #[test]
    fn fichier_migre_sauvegarde_et_relu() {
        let original = "# réglé à la main\nlisten = 9443\nadmin_password_hash = \"$argon2id$x\"\n\n[virtual_display]\ntrigger = \"all\"\n";
        let dir = data_dir("fichier", Some(original));
        // Sans migration, ce fichier est mis de côté (mot de passe perdu).
        assert!(toml::from_str::<Settings>(original).is_err());

        let essai = migrate(&dir, true).unwrap();
        assert!(matches!(essai, Outcome::Migrated { from: 0, to: 1, backup: None, .. }));
        assert_eq!(fs::read_to_string(dir.join(FILE_NAME)).unwrap(), original, "l'essai n'écrit rien");

        let Outcome::Migrated { backup: Some(backup), .. } = migrate(&dir, false).unwrap() else { panic!() };
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        let text = fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(text.starts_with("# réglé à la main\n"), "commentaires gardés : {text}");
        assert!(text.contains("schema = 1"));
        let s: Settings = toml::from_str(&text).unwrap();
        assert_eq!(s.listen.as_deref(), Some("0.0.0.0:9443"));
        assert_eq!(s.admin_password_hash.as_deref(), Some("$argon2id$x"));
        assert_eq!(s.schema, 1);

        assert_eq!(migrate(&dir, false).unwrap(), Outcome::UpToDate);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn absent_recent_et_illisible() {
        let dir = data_dir("absent", None);
        assert_eq!(migrate(&dir, false).unwrap(), Outcome::NoFile);
        fs::write(dir.join(FILE_NAME), "schema = 7\nlisten = \"0.0.0.0\"\n").unwrap();
        assert_eq!(migrate(&dir, false).unwrap(), Outcome::Newer(7));
        assert!(fs::read_to_string(dir.join(FILE_NAME)).unwrap().contains("\"0.0.0.0\""));
        fs::write(dir.join(FILE_NAME), "listen = [").unwrap();
        assert!(migrate(&dir, false).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nouveaux_reglages_au_format_courant() {
        let dir = data_dir("neufs", None);
        let s = Settings::load(&dir).unwrap();
        s.save(&dir).unwrap();
        assert_eq!(migrate(&dir, false).unwrap(), Outcome::UpToDate);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn champs_inconnus_gardes() {
        // Réglages d'une console plus récente relus par celle-ci (retour arrière).
        let dir = data_dir("inconnus", Some("schema = 2\nfutur = \"x\"\n\n[virtual_audio]\nchannels = \"auto\"\n\n[nouveau]\na = 1\n"));
        let s = Settings::load(&dir).unwrap();
        s.save(&dir).unwrap();
        let text = fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(text.contains("schema = 2") && text.contains("futur = \"x\"") && text.contains("[nouveau]"), "{text}");
        let _ = fs::remove_dir_all(&dir);
    }
}
