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
