//! Fichiers de données d'Admin (JSON, réglages) relus sans jamais repartir
//! de zéro en silence.
//!
//! Chaque lecture réussie garde une copie, `<fichier>.bak`. Un fichier
//! illisible (gel du PC pendant une écriture, entrée au format d'une version
//! plus récente après un retour arrière, édition à la main) est journalisé
//! et mis de côté (`<fichier>.invalide-<horodatage>`, jamais écrasé par la
//! sauvegarde suivante), puis la dernière copie lisible est reprise ; les
//! valeurs par défaut ne viennent qu'en dernier.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// Copie de la dernière version lue sans erreur.
pub fn backup_of(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{name}.bak"))
}

/// Ce que la lecture a donné.
#[derive(Debug, PartialEq, Eq)]
pub enum Loaded {
    /// Le fichier lui-même.
    File,
    /// Absent : valeurs par défaut.
    Missing,
    /// Illisible, mis de côté ; la copie `.bak` a été reprise.
    Backup,
    /// Illisible, mis de côté, sans copie lisible : valeurs par défaut.
    Default,
}

/// Lit `path` avec `parse` ; voir le module.
pub fn load_with<T>(path: &Path, parse: impl Fn(&[u8]) -> Result<T, String>) -> (Option<T>, Loaded) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (None, Loaded::Missing),
        Err(e) => {
            // Illisible sans être absent (droits, disque) : rien n'est déplacé.
            tracing::error!("{} illisible ({e}) : dernière copie lisible (ou valeurs par défaut) pour cette fois", path.display());
            let value = parse_backup(path, &parse);
            let how = if value.is_some() { Loaded::Backup } else { Loaded::Default };
            return (value, how);
        }
    };
    match parse(&bytes) {
        Ok(value) => {
            let backup = backup_of(path);
            if fs::read(&backup).ok().as_deref() != Some(&bytes[..]) {
                let _ = crate::durable::write(&backup, &bytes);
            }
            (Some(value), Loaded::File)
        }
        Err(e) => {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let aside = path.with_file_name(format!("{name}.invalide-{}", crate::display::now_secs()));
            let _ = fs::rename(path, &aside);
            match parse_backup(path, &parse) {
                Some(value) => {
                    tracing::error!("{} illisible ({e}) : mis de côté dans {}, dernière copie lisible reprise", path.display(), aside.display());
                    (Some(value), Loaded::Backup)
                }
                None => {
                    tracing::error!("{} illisible ({e}) : mis de côté dans {}, valeurs par défaut", path.display(), aside.display());
                    (None, Loaded::Default)
                }
            }
        }
    }
}

fn parse_backup<T>(path: &Path, parse: &impl Fn(&[u8]) -> Result<T, String>) -> Option<T> {
    parse(&fs::read(backup_of(path)).ok()?).ok()
}

/// Un fichier JSON ; valeurs par défaut s'il est absent, ou illisible sans copie.
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    load_with(path, |b| serde_json::from_slice(b).map_err(|e| e.to_string())).0.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_damaged_file_falls_back_to_the_last_good_copy() {
        let dir = std::env::temp_dir().join(format!("kybernow-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");
        let parse = |b: &[u8]| serde_json::from_slice::<Vec<String>>(b).map_err(|e| e.to_string());
        assert_eq!(load_with(&path, parse).1, Loaded::Missing);
        fs::write(&path, r#"["lea","bloque"]"#).unwrap();
        assert_eq!(load_with(&path, parse), (Some(vec!["lea".to_string(), "bloque".to_string()]), Loaded::File));
        // Rempli de zéros par un gel : la copie revient, l'original est gardé de côté.
        fs::write(&path, [0u8; 32]).unwrap();
        let (value, how) = load_with(&path, parse);
        assert_eq!((value.unwrap().len(), how), (2, Loaded::Backup));
        let aside = fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".invalide-")).count();
        assert_eq!(aside, 1);
        // Sans copie lisible : les valeurs par défaut.
        fs::write(&path, "pas du json").unwrap();
        fs::write(backup_of(&path), "pas non plus").unwrap();
        assert_eq!(load_json::<Vec<String>>(&path), Vec::<String>::new());
        let _ = fs::remove_dir_all(&dir);
    }
}
