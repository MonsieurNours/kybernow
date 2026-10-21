//! Lecture des journaux de Kyber et de la console.
//!
//! Les fichiers sont nommés « source/fichier » (ex. `kyber/kyavserver.log`,
//! `kybernow/kyber-admin.log`) pour distinguer leurs dossiers d'origine.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

/// Taille maximale lue à la fin d'un fichier.
const TAIL_BYTES: u64 = 512 * 1024;
/// Journal de la console : au-delà de cette taille, il est tourné.
pub const ROTATE_BYTES: u64 = 5 * 1024 * 1024;
/// Fichiers gardés en tout (`kyber-admin.log`, `.1` … `.4`).
pub const ROTATE_KEEP: usize = 5;

/// Écrivain du journal de la console (`kyber-admin.log`) : à 5 Mo, le fichier
/// devient `kyber-admin.log.1` (les précédents glissent en `.2`, `.3`, `.4`,
/// le plus ancien disparaît) et un fichier neuf est ouvert. Tous restent
/// visibles dans Système › Journaux (leur nom contient `.log`).
pub struct RotatingFile {
    path: PathBuf,
    file: File,
    size: u64,
}

impl RotatingFile {
    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let file = fs::OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self { path, file, size })
    }

    fn rotated(&self, n: usize) -> PathBuf {
        let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.path.with_file_name(format!("{name}.{n}"))
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let _ = fs::remove_file(self.rotated(ROTATE_KEEP - 1));
        for n in (1..ROTATE_KEEP - 1).rev() {
            let _ = fs::rename(self.rotated(n), self.rotated(n + 1));
        }
        let _ = fs::rename(&self.path, self.rotated(1));
        self.file = fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        self.size = 0;
        Ok(())
    }
}

impl std::io::Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.size > 0 && self.size + buf.len() as u64 > ROTATE_BYTES {
            // Une rotation impossible (fichier tenu par un autre programme)
            // n'arrête pas le journal : on continue dans le fichier en cours.
            let _ = self.rotate();
        }
        let n = self.file.write(buf)?;
        self.size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[derive(Debug, Serialize)]
pub struct LogFile {
    pub name: String,
    pub size: u64,
    /// Secondes depuis l'époque Unix.
    pub modified: u64,
}

/// Un dossier de journaux et le préfixe qui le désigne.
pub struct Source {
    pub prefix: &'static str,
    pub dir: PathBuf,
}

fn is_log(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().contains(".log"))
}

pub fn list(sources: &[Source]) -> Result<Vec<LogFile>> {
    let mut files: Vec<LogFile> = Vec::new();
    for source in sources {
        let Ok(entries) = fs::read_dir(&source.dir) else { continue };
        for e in entries.filter_map(|e| e.ok()) {
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() || !is_log(&e.path()) {
                continue;
            }
            let name = format!("{}/{}", source.prefix, e.file_name().to_string_lossy());
            if files.iter().any(|f| f.name == name) {
                continue;
            }
            files.push(LogFile {
                name,
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
            });
        }
    }
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}

/// Renvoie les `lines` dernières lignes d'un journal (« source/fichier »).
pub fn tail(sources: &[Source], name: &str, lines: usize) -> Result<String> {
    // Le nom vient du navigateur : on refuse tout ce qui sortirait des dossiers connus.
    let (prefix, file) = name.split_once('/').context("nom de fichier invalide")?;
    if file.is_empty() || file.contains(['/', '\\']) || file.contains("..") || !is_log(Path::new(file)) {
        bail!("nom de fichier invalide");
    }
    let path = sources
        .iter()
        .filter(|s| s.prefix == prefix)
        .map(|s| s.dir.join(file))
        .find(|p| p.is_file())
        .context("journal introuvable")?;
    let mut f = File::open(&path).with_context(|| format!("ouverture de {}", path.display()))?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut all: Vec<&str> = text.lines().collect();
    if start > 0 && !all.is_empty() {
        all.remove(0); // première ligne probablement tronquée
    }
    let from = all.len().saturating_sub(lines);
    Ok(all[from..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_du_journal() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("kybernow-logrotate-{}-{}", std::process::id(), rand::random::<u32>()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kyber-admin.log");
        let mut w = RotatingFile::open(path.clone()).unwrap();
        // Simule un fichier déjà plein : la prochaine écriture tourne.
        w.size = ROTATE_BYTES;
        w.write_all(b"ligne 1\n").unwrap();
        assert!(dir.join("kyber-admin.log.1").exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), "ligne 1\n");
        for i in 2..=6 {
            w.size = ROTATE_BYTES;
            w.write_all(format!("ligne {i}\n").as_bytes()).unwrap();
        }
        let names: Vec<String> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names.len(), ROTATE_KEEP, "{names:?}");
        assert!(!names.iter().any(|n| n.ends_with(".5")), "{names:?}");
        assert_eq!(fs::read_to_string(dir.join("kyber-admin.log.4")).unwrap(), "ligne 2\n");
        assert!(list(&[Source { prefix: "kybernow", dir: dir.clone() }]).unwrap().iter().any(|f| f.name == "kybernow/kyber-admin.log.3"));
        let _ = fs::remove_dir_all(dir);
    }
}
