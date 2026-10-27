//! Profils Jeu, Bureau et Max, et débit selon les pixels par seconde, comme
//! l'app Mac (`Model.swift`).

/// Format vidéo demandé à l'ordinateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

impl Codec {
    /// Code du format dans les paquets de Kyber (`media::FOURCC_*`).
    pub fn fourcc(self) -> u32 {
        match self {
            Self::H264 => crate::media::FOURCC_H264,
            Self::Hevc => crate::media::FOURCC_HEVC,
            Self::Av1 => crate::media::FOURCC_AV1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::Av1 => "AV1",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "h264" | "h.264" | "avc" => Some(Self::H264),
            "hevc" | "h265" | "h.265" => Some(Self::Hevc),
            "av1" => Some(Self::Av1),
            _ => None,
        }
    }

    pub(crate) fn kyber(self) -> kyclient::VideoCodec {
        match self {
            Self::H264 => kyclient::VideoCodec::H264,
            Self::Hevc => kyclient::VideoCodec::HEVC,
            Self::Av1 => kyclient::VideoCodec::AV1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Jeu,
    Bureau,
    Max,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jeu => "Jeu",
            Self::Bureau => "Bureau",
            Self::Max => "Max",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "jeu" | "game" => Some(Self::Jeu),
            "bureau" | "desktop" => Some(Self::Bureau),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// Ce que la console fait d'un changement de mode de l'ordinateur.
    pub fn mode_policy(self) -> &'static str {
        match self {
            Self::Jeu => "follow",
            Self::Bureau => "restore",
            Self::Max => "default",
        }
    }
}

/// Ce que l'appareil sait décoder par sa carte graphique.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Capabilities {
    pub hevc: bool,
    pub hevc444: bool,
    pub av1: bool,
}

/// Réglages d'une session en JSON ([`StreamSettings::from_json`]).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SettingsJson {
    pub profile: Option<String>,
    pub codec: String,
    #[serde(default)]
    pub yuv444: bool,
    pub mbps: u32,
    #[serde(default)]
    pub reliable: bool,
}

impl SettingsJson {
    pub fn settings(&self) -> Result<StreamSettings, String> {
        let codec = Codec::parse(&self.codec).ok_or_else(|| format!("format inconnu : {}", self.codec))?;
        Ok(StreamSettings {
            profile: self.profile.as_deref().and_then(Profile::parse),
            codec,
            yuv444: self.yuv444,
            mbps: self.mbps,
            reliable: self.reliable,
        })
    }
}

/// Réglages d'une session.
#[derive(Debug, Clone)]
pub struct StreamSettings {
    pub profile: Option<Profile>,
    pub codec: Codec,
    pub yuv444: bool,
    /// Débit de base, calé sur la 4K à 60 Hz pour un profil.
    pub mbps: u32,
    /// Transport fiable (Bureau) ou rapide avec correction d'erreurs.
    pub reliable: bool,
}

impl StreamSettings {
    /// Réglages d'un profil, ramenés à ce que l'appareil décode.
    pub fn for_profile(profile: Profile, caps: Capabilities) -> Self {
        let (yuv444, mut mbps, reliable) = match profile {
            Profile::Jeu => (false, 60, false),
            Profile::Bureau => (true, 90, true),
            Profile::Max => (true, 300, false),
        };
        let codec = if caps.hevc { Codec::Hevc } else { Codec::H264 };
        let mut yuv = yuv444;
        if yuv444 && !(codec == Codec::Hevc && caps.hevc444) {
            yuv = false;
            mbps += mbps / 3;
        }
        if codec == Codec::H264 {
            mbps += mbps / 2;
        }
        Self {
            profile: Some(profile),
            codec,
            yuv444: yuv,
            mbps,
            reliable,
        }
    }

    /// Réglages en JSON, comme les donnent les ponts des apps (C, JNI) :
    /// `profile` (`jeu`, `bureau`, `max` ; absent : personnalisé), `codec`
    /// (`h264`, `hevc`, `av1`), `yuv444`, `mbps`, `reliable`.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let wire: SettingsJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
        wire.settings()
    }

    pub fn label(&self) -> &'static str {
        self.profile.map(Profile::label).unwrap_or("Personnalisé")
    }

    pub fn mode_policy(&self) -> &'static str {
        self.profile.map(Profile::mode_policy).unwrap_or("default")
    }

    /// Débit en bit/s pour un mode : un profil suit les pixels par seconde
    /// (puissance 0,75, entre 0,4 et 3 fois), un débit réglé à la main reste.
    pub fn bitrate(&self, width: u32, height: u32, refresh: u32) -> u32 {
        self.bitrate_for_pixel_rate(width as f64 * height as f64 * refresh.max(1) as f64)
    }

    /// Débit pour des écrans dont les pixels par seconde (largeur × hauteur ×
    /// fréquence) font ensemble `pixel_rate` : la même règle, avec les mêmes
    /// plafonds (multi-écrans : la somme des écrans de la session).
    pub fn bitrate_for_pixel_rate(&self, pixel_rate: f64) -> u32 {
        let mbps = if self.profile.is_some() {
            let ratio = pixel_rate.max(1.0) / (3840.0 * 2160.0 * 60.0);
            let scale = ratio.powf(0.75).clamp(0.4, 3.0);
            (self.mbps as f64 * scale).round() as u32
        } else {
            self.mbps
        };
        mbps.clamp(5, 4294).saturating_mul(1_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debits_comme_sur_mac() {
        let caps = Capabilities { hevc: true, hevc444: true, av1: false };
        let jeu = StreamSettings::for_profile(Profile::Jeu, caps);
        assert_eq!(jeu.bitrate(3840, 2160, 60), 60_000_000);
        assert_eq!(jeu.bitrate(2560, 1440, 60), 33_000_000);
        assert_eq!(jeu.bitrate(1920, 1080, 60), 24_000_000);
        assert_eq!(jeu.bitrate(3840, 2160, 120), 101_000_000);
        let bureau = StreamSettings::for_profile(Profile::Bureau, Capabilities { hevc: true, hevc444: false, av1: false });
        assert!(!bureau.yuv444);
        assert_eq!(bureau.mbps, 120);
        let h264 = StreamSettings::for_profile(Profile::Jeu, Capabilities::default());
        assert_eq!(h264.codec, Codec::H264);
        assert_eq!(h264.mbps, 90);
        // Plusieurs écrans : la somme des pixels par seconde, mêmes plafonds.
        let sum = 3840.0 * 2160.0 * 160.0 + 5120.0 * 2880.0 * 60.0 + 3456.0 * 2234.0 * 60.0;
        let b = jeu.bitrate_for_pixel_rate(sum);
        assert!(b > jeu.bitrate(3840, 2160, 160) && b <= 180_000_000, "{b}");
        assert_eq!(jeu.bitrate_for_pixel_rate(3840.0 * 2160.0 * 60.0), jeu.bitrate(3840, 2160, 60));
    }
}
