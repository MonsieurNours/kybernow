//! Lecture des journaux de Kyber (`<dossier Kyber>/log`).

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

/// Taille maximale lue à la fin d'un fichier.
const TAIL_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct LogFile {
    pub name: String,
    pub size: u64,
    /// Secondes depuis l'époque Unix.
    pub modified: u64,
}

pub fn list(dir: &Path) -> Result<Vec<LogFile>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut files: Vec<LogFile> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| LogFile {
                name: e.file_name().to_string_lossy().into_owned(),
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
            })
        })
        .collect();
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}

/// Renvoie les `lines` dernières lignes d'un fichier du dossier de journaux.
pub fn tail(dir: &Path, name: &str, lines: usize) -> Result<String> {
    // Le nom vient du navigateur : on refuse tout ce qui sortirait du dossier.
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        bail!("nom de fichier invalide");
    }
    let path = dir.join(name);
    let mut file = File::open(&path).with_context(|| format!("ouverture de {}", path.display()))?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let mut all: Vec<&str> = text.lines().collect();
    if start > 0 && !all.is_empty() {
        all.remove(0); // première ligne probablement tronquée
    }
    let from = all.len().saturating_sub(lines);
    Ok(all[from..].join("\n"))
}
