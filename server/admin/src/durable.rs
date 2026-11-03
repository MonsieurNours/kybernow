//! Écritures qui survivent à un gel ou à une coupure du PC.
//!
//! NTFS garde un temps en mémoire ce qu'on lui écrit : un fichier écrit puis
//! renommé juste avant un gel peut revenir rempli de zéros au redémarrage
//! (le 03/11, `kyber_config.toml` et la clé de Kyber, juste après une mise à
//! jour). Ici, chaque fichier est forcé sur le disque (`File::sync_all`, soit
//! `FlushFileBuffers`) avant de prendre sa place, le renommage est écrit tout
//! de suite (`MOVEFILE_WRITE_THROUGH`), puis le dossier est forcé à son tour
//! quand Windows le permet. L'applicateur des mises à jour force en plus tout
//! le volume après la bascule (`flush_volume`).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Fichier temporaire à côté de `path` (même dossier : même volume pour le renommage).
fn temp_of(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{name}.kybernow-tmp"))
}

/// Écrit `bytes` dans `path` d'un bloc : fichier temporaire forcé sur le
/// disque, puis renommage.
pub fn write(path: &Path, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    write_then(path, bytes, |_| {})
}

/// Comme `write`, avec `before` appelé sur le fichier temporaire juste avant
/// le renommage (droits à poser, par exemple).
pub fn write_then(path: &Path, bytes: impl AsRef<[u8]>, before: impl FnOnce(&Path)) -> io::Result<()> {
    let tmp = temp_of(path);
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes.as_ref())?;
        f.sync_all()?;
        drop(f);
        before(&tmp);
        rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Force un fichier déjà écrit sur le disque.
pub fn sync_file(path: &Path) -> io::Result<()> {
    // `FlushFileBuffers` demande un accès en écriture ; rien n'est tronqué.
    fs::OpenOptions::new().write(true).open(path)?.sync_all()
}

/// Force un dossier (ses entrées, donc les renommages) sur le disque, si le
/// système le permet ; sans effet sinon.
pub fn sync_dir(dir: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Un dossier ne s'ouvre qu'avec FILE_FLAG_BACKUP_SEMANTICS (droit de SYSTEM et des administrateurs).
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        if let Ok(d) = fs::OpenOptions::new().write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(dir) {
            let _ = d.sync_all();
        }
    }
    #[cfg(not(windows))]
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
}

/// Renomme (en remplaçant `to`) et force le renommage sur le disque.
pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let wide = |p: &Path| p.as_os_str().encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
        if unsafe { MoveFileExW(wide(from).as_ptr(), wide(to).as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(not(windows))]
    fs::rename(from, to)?;
    if let Some(dir) = to.parent() {
        sync_dir(dir);
    }
    Ok(())
}

/// Copie `from` vers `to` et force la copie sur le disque.
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let n = fs::copy(from, to)?;
    sync_file(to)?;
    Ok(n)
}

/// Force sur le disque tout ce qui est en attente sur le volume de `path`
/// (`FlushFileBuffers` sur `\\.\C:`, réservé à SYSTEM et aux
/// administrateurs : l'applicateur des mises à jour). Rend une erreur si
/// Windows le refuse ; sans effet hors Windows.
pub fn flush_volume(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        let letter = text.chars().next().filter(|c| c.is_ascii_alphabetic() && text[1..].starts_with(':'));
        let Some(letter) = letter else {
            return Err(io::Error::other(format!("lecteur introuvable pour {text}")));
        };
        fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\{letter}:"))?.sync_all()
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecrit_d_un_bloc_sans_laisser_de_temporaire() {
        let dir = std::env::temp_dir().join(format!("kybernow-durable-{}", rand::random::<u32>()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("etat.json");
        write(&path, b"un").unwrap();
        write(&path, b"deux").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"deux");
        let mut seen = None;
        write_then(&path, b"trois", |tmp| seen = Some(fs::read(tmp).unwrap())).unwrap();
        assert_eq!(seen.as_deref(), Some(&b"trois"[..]), "le temporaire est complet avant le renommage");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "aucun temporaire laissé");
        let copy_to = dir.join("copie.json");
        copy(&path, &copy_to).unwrap();
        assert_eq!(fs::read(&copy_to).unwrap(), b"trois");
        // Un dossier absent : erreur, rien d'écrit.
        assert!(write(&dir.join("absent").join("x"), b"x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
