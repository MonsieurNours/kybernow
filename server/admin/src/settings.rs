//! Réglages propres à kyber-admin, stockés dans `<données>/kyber-admin.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "kyber-admin.toml";
pub const DEFAULT_LISTEN: &str = "0.0.0.0:9443";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Adresse d'écoute de l'interface (ex. « 0.0.0.0:9443 »).
    pub listen: Option<String>,
    /// Dossier d'installation de Kyber (contient kyber_config.toml et log/).
    pub kyber_dir: Option<PathBuf>,
    /// Empreinte argon2 du mot de passe administrateur.
    pub admin_password_hash: Option<String>,
    /// Écran virtuel (déclenchement et mode par défaut).
    #[serde(default)]
    pub virtual_display: crate::display::VdSettings,
}

impl Settings {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join(FILE_NAME);
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).with_context(|| format!("lecture de {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("lecture de {}", path.display())),
        }
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)?;
        let path = data_dir.join(FILE_NAME);
        fs::write(&path, toml::to_string_pretty(self)?)
            .with_context(|| format!("écriture de {}", path.display()))
    }
}

pub fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("KyberNow")
    } else {
        PathBuf::from("dev-data")
    }
}

pub fn default_kyber_dir() -> Option<PathBuf> {
    cfg!(windows).then(|| {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
            .join("KyberNow")
            .join("kyber")
    })
}
