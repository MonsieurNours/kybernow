//! Fichiers de l'app : un seul dossier (`%APPDATA%\KyberNow`), une seule
//! façon d'écrire (fichier temporaire posé sur le disque, puis renommé : un
//! arrêt brutal laisse l'ancien ou le nouveau, jamais un fichier tronqué) et
//! une seule façon de relire (un fichier illisible est mis de côté et noté au
//! journal plutôt qu'écrasé en silence au prochain enregistrement).
//!
//! Rien de Windows ici : se teste sur le Mac.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// `%APPDATA%\KyberNow` (dossier temporaire sans `APPDATA`).
pub fn app_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow")
}

/// Un fichier de `%APPDATA%\KyberNow`.
pub fn app_file(name: &str) -> PathBuf {
    app_dir().join(name)
}

/// Écrit `data` dans `path` : dossier créé, fichier temporaire propre à cet
/// appel, posé sur le disque (`sync_all`), puis renommé par-dessus l'ancien
/// (`MoveFileExW` avec remplacement sous Windows).
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.{}-{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    let written = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(data)?;
        f.sync_all()
    });
    let done = written.and_then(|()| std::fs::rename(&tmp, path));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// `value` en JSON lisible dans `path` ; une erreur est notée au journal.
pub fn save_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let result = serde_json::to_vec_pretty(value).map_err(std::io::Error::other).and_then(|data| write_atomic(path, &data));
    match result {
        Ok(()) => true,
        Err(e) => {
            log::error!("{name} : enregistrement impossible ({e})");
            false
        }
    }
}

/// Relit `path` : `None` s'il n'existe pas ; illisible (JSON abîmé, coupure
/// pendant une écriture d'une ancienne version), mis de côté dans
/// `<nom>.<secondes>.illisible` et `None`.
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::error!("{} : lecture impossible ({e})", path.display());
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            let aside = set_aside(path);
            log::error!("{} illisible ({e}) : mis de côté dans {}", path.display(), aside.display());
            None
        }
    }
}

/// Met `path` de côté (renommé) et rend le nouveau chemin.
pub fn set_aside(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let aside = path.with_file_name(format!("{name}.{secs}.illisible"));
    let _ = std::fs::rename(path, &aside);
    aside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kybernow-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn ecrit_relit_et_remplace() {
        let d = dir("ecrit");
        let p = d.join("sous").join("app.json");
        assert!(save_json(&p, &vec![1, 2, 3]));
        assert_eq!(load_json::<Vec<i32>>(&p), Some(vec![1, 2, 3]));
        assert!(save_json(&p, &vec![4]));
        assert_eq!(load_json::<Vec<i32>>(&p), Some(vec![4]));
        // Aucun fichier temporaire ne reste.
        let left: Vec<_> = std::fs::read_dir(p.parent().unwrap()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "{left:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn absent_ou_illisible() {
        let d = dir("illisible");
        let p = d.join("app.json");
        assert_eq!(load_json::<Vec<i32>>(&p), None);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(&p, b"{\"open_at_login\": fal").unwrap();
        assert_eq!(load_json::<serde_json::Value>(&p), None);
        assert!(!p.exists(), "le fichier abîmé est mis de côté");
        let aside: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(aside.iter().any(|n| n.starts_with("app.json.") && n.ends_with(".illisible")), "{aside:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ecritures_en_parallele() {
        let d = dir("parallele");
        let p = d.join("x.json");
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let p = p.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        assert!(save_json(&p, &vec![i; 100]));
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let v: Vec<i32> = load_json(&p).unwrap();
        assert_eq!(v.len(), 100);
        assert!(v.iter().all(|x| *x == v[0]));
        let _ = std::fs::remove_dir_all(&d);
    }
}
