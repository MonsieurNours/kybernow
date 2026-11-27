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
    /// Prêt de l'ordinateur : interrupteur, apps permises.
    #[serde(default)]
    pub lend: crate::lend::LendSettings,
    /// Compte KyberNow rattaché (docs/comptes.md) : relais, propriétaire,
    /// clé du relais épinglée, « Accepter les comptes KyberNow ».
    #[serde(default)]
    pub account: crate::accounts::AccountSettings,
    /// Enregistrement des sessions (docs/enregistrement.md) : permis, dossier, durée.
    #[serde(default)]
    pub recording: crate::recording::RecordingSettings,
    /// Réglages inconnus de cette version (écrits par une console plus
    /// récente, avant un retour arrière) : gardés à la réécriture.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Settings {
    /// Lit les réglages. Un fichier illisible (coupure pendant une écriture,
    /// édition à la main) est mis de côté à côté de l'original et la console
    /// reprend la dernière copie lisible (`kyber-admin.toml.bak` : mot de
    /// passe d'Admin, relais, compte), sinon les valeurs par défaut, plutôt
    /// que de ne plus démarrer du tout (voir `store`).
    pub fn load(data_dir: &Path) -> Result<Self> {
        crate::migrate::before_load(data_dir);
        let path = data_dir.join(FILE_NAME);
        let parse = |b: &[u8]| std::str::from_utf8(b).map_err(|e| e.to_string()).and_then(|t| toml::from_str::<Self>(t).map_err(|e| e.to_string()));
        Ok(crate::store::load_with(&path, parse).0.unwrap_or_else(Self::fresh))
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
        // Forcé sur le disque avant de prendre la place (`durable`).
        crate::durable::write(&path, toml::to_string_pretty(self)?).with_context(|| format!("remplacement de {}", path.display()))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `kyber-admin.toml` abîmé : le mot de passe d'Admin et le relais
    /// reviennent de la dernière copie lisible, pas le code d'initialisation.
    #[test]
    fn damaged_settings_come_back_from_the_last_good_copy() {
        let dir = std::env::temp_dir().join(format!("kybernow-settings-bak-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut s = Settings::load(&dir).unwrap();
        s.admin_password_hash = Some("$argon2id$empreinte".into());
        s.save(&dir).unwrap();
        // Relu une fois : la copie est faite.
        assert_eq!(Settings::load(&dir).unwrap().admin_password_hash.as_deref(), Some("$argon2id$empreinte"));
        fs::write(dir.join(FILE_NAME), [0u8; 64]).unwrap();
        assert_eq!(Settings::load(&dir).unwrap().admin_password_hash.as_deref(), Some("$argon2id$empreinte"));
        let _ = fs::remove_dir_all(&dir);
    }
}
