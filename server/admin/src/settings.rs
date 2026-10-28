//! Réglages propres à kyber-admin, stockés dans `<données>/kyber-admin.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "kyber-admin.toml";
pub const DEFAULT_LISTEN: &str = "0.0.0.0:9443";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Version du format du fichier (absent = 0), voir migrate.rs.
    #[serde(default)]
    pub schema: i64,
    /// Adresse d'écoute de l'interface (ex. « 0.0.0.0:9443 »).
    pub listen: Option<String>,
    /// Dossier d'installation de Kyber (contient kyber_config.toml et log/).
    pub kyber_dir: Option<PathBuf>,
    /// Empreinte argon2 du mot de passe administrateur.
    pub admin_password_hash: Option<String>,
    /// Compte Kyber créé à l'installation (`init`) : celui sous lequel le
    /// client web se connecte. Absent (installation d'avant 0.2.6) : le
    /// premier compte de la liste, mémorisé au démarrage suivant.
    #[serde(default)]
    pub install_account: Option<String>,
    /// Écran virtuel (déclenchement et mode par défaut).
    #[serde(default)]
    pub virtual_display: crate::display::VdSettings,
    /// Sortie audio virtuelle (déclenchement et nombre de canaux).
    #[serde(default)]
    pub virtual_audio: crate::audio::AudioSettings,
    /// Canal des mises à jour : `stable` (défaut) ou `beta` (aussi les préversions).
    #[serde(default)]
    pub update_channel: crate::update::Channel,
    /// « Hors de chez vous » : adresse du relais, accès des appareils.
    #[serde(default)]
    pub relay: crate::relay::RelaySettings,
    /// Réglages inconnus de cette version (écrits par une console plus
    /// récente, avant un retour arrière) : gardés à la réécriture.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Settings {
    /// Lit les réglages. Un fichier illisible (coupure pendant une écriture,
    /// édition à la main) est mis de côté à côté de l'original et la console
    /// repart des valeurs par défaut, plutôt que de ne plus démarrer du tout.
    pub fn load(data_dir: &Path) -> Result<Self> {
        crate::migrate::before_load(data_dir);
        let path = data_dir.join(FILE_NAME);
        match fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(settings) => Ok(settings),
                Err(e) => {
                    let aside = data_dir.join(format!(
                        "{FILE_NAME}.invalide-{}",
                        crate::display::now_secs()
                    ));
                    tracing::error!(
                        "{} illisible ({e}) : mis de côté dans {}, réglages par défaut",
                        path.display(),
                        aside.display()
                    );
                    let _ = fs::rename(&path, &aside);
                    Ok(Self::fresh())
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::fresh()),
            Err(e) => Err(e).with_context(|| format!("lecture de {}", path.display())),
        }
    }

    /// Réglages neufs, au format de cette console.
    fn fresh() -> Self {
        Self { schema: crate::migrate::CURRENT, ..Self::default() }
    }

    /// Écrit les réglages d'un bloc : fichier temporaire puis renommage, pour
    /// qu'une coupure ne laisse jamais un fichier à moitié écrit.
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)?;
        let path = data_dir.join(FILE_NAME);
        let tmp = data_dir.join(format!("{FILE_NAME}.tmp"));
        fs::write(&tmp, toml::to_string_pretty(self)?)
            .with_context(|| format!("écriture de {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("remplacement de {}", path.display()))
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
