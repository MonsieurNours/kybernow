//! Réglages d'une console (docs/playstation.md, « Réglages ») : ce que
//! l'app garde pour chaque console (JSON stable) et ce qui en sort pour la
//! console ([`super::stream::Profile`]).
//!
//! - **Définition** : Auto (1080p), 1080p, 720p, 540p, 360p ; la PS5 Pro
//!   refuse 1440p et 4K (essai du 03/11/2026).
//! - **Images par seconde** : 60 ou 30 (la lecture à distance ne monte pas
//!   plus haut).
//! - **Débit** : Auto (préréglages de chiaki-ng selon la définition) ou au
//!   choix, de 2 à 100 Mbit/s (100 accepté en 1080p à l'essai).
//! - **Format** : Auto (H.265 si l'appareil le décode, sinon H.264), H.264,
//!   H.265. Le HDR demande le H.265 ; la PS4 ne connaît que le H.264.
//! - **Plein écran** : retenu par console, plein écran la première fois
//!   (côté app seulement).
//! - **Agrandissement** : l'image de la console (1080p au plus) agrandie
//!   aux pixels de l'écran par MetalFX (côté app), allumé par défaut, avec
//!   une netteté de 0 à 100 (40 par défaut, 0 la coupe).
//! - **Manette** : intensité des vibrations et des gâchettes adaptatives de
//!   la DualSense, de 0 (coupées, gâchettes libres) à 100 % (défaut), par
//!   pas de 10 ; changées tout de suite pendant une partie
//!   ([`super::dualsense`]).
//!
//! « Personnalisé » : définition, images par seconde, débit ou format qui
//! s'écartent d'Auto. Le HDR (puce de l'accueil), le plein écran,
//! l'agrandissement et les intensités de la manette n'en font pas partie.

use serde::{Deserialize, Serialize};

use super::stream::Profile;

/// Définition demandée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    #[default]
    Auto,
    P1080,
    P720,
    P540,
    P360,
}

impl Resolution {
    /// Toutes, dans l'ordre des réglages.
    pub const ALL: [Resolution; 5] = [Resolution::Auto, Resolution::P1080, Resolution::P720, Resolution::P540, Resolution::P360];

    /// Largeur et hauteur (Auto : 1080p, la plus haute que la console accepte).
    pub fn size(self) -> (u32, u32) {
        match self {
            Resolution::Auto | Resolution::P1080 => (1920, 1080),
            Resolution::P720 => (1280, 720),
            Resolution::P540 => (960, 540),
            Resolution::P360 => (640, 360),
        }
    }

    /// Débit d'Auto, en kbit/s (préréglages de chiaki-ng).
    pub fn auto_bitrate(self) -> u32 {
        match self {
            Resolution::Auto | Resolution::P1080 => 15_000,
            Resolution::P720 => 10_000,
            Resolution::P540 => 6_000,
            Resolution::P360 => 2_000,
        }
    }
}

/// Format demandé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecChoice {
    #[default]
    Auto,
    H264,
    H265,
}

/// Débit choisi à la main : bornes, en kbit/s.
pub const BITRATE_MIN: u32 = 2_000;
pub const BITRATE_MAX: u32 = 100_000;

/// Les réglages d'une console.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaySettings {
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default = "sixty")]
    pub fps: u32,
    /// kbit/s ; absent : Auto.
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
    #[serde(default)]
    pub codec: CodecChoice,
    /// Demandé par la puce HDR de l'accueil (écran HDR).
    #[serde(default)]
    pub hdr: bool,
    /// Côté app : la partie s'ouvre en plein écran.
    #[serde(default = "yes")]
    pub fullscreen: bool,
    /// Côté app : l'image est agrandie aux pixels de l'écran (MetalFX).
    #[serde(default = "yes")]
    pub upscale: bool,
    /// Côté app : netteté après l'agrandissement, de 0 (coupée) à 100.
    #[serde(default = "default_sharpness")]
    pub sharpness: u8,
    /// Intensité des vibrations de la DualSense (HD et simples), de 0
    /// (coupées) à 100 %.
    #[serde(default = "full")]
    pub rumble: u8,
    /// Intensité des gâchettes adaptatives, de 0 (libres) à 100 %.
    #[serde(default = "full")]
    pub triggers: u8,
}

/// Intensités de la manette par défaut : entières.
pub const FULL_INTENSITY: u8 = 100;

fn full() -> u8 {
    FULL_INTENSITY
}

/// Netteté par défaut : modérée.
pub const DEFAULT_SHARPNESS: u8 = 40;

fn default_sharpness() -> u8 {
    DEFAULT_SHARPNESS
}

fn sixty() -> u32 {
    60
}
fn yes() -> bool {
    true
}

impl Default for PlaySettings {
    fn default() -> Self {
        PlaySettings {
            resolution: Resolution::Auto,
            fps: 60,
            bitrate_kbps: None,
            codec: CodecChoice::Auto,
            hdr: false,
            fullscreen: true,
            upscale: true,
            sharpness: DEFAULT_SHARPNESS,
            rumble: FULL_INTENSITY,
            triggers: FULL_INTENSITY,
        }
    }
}

impl PlaySettings {
    /// Définition, images par seconde, débit ou format s'écartent d'Auto.
    pub fn is_custom(&self) -> bool {
        self.resolution != Resolution::Auto || self.fps != 60 || self.bitrate_kbps.is_some() || self.codec != CodecChoice::Auto
    }

    /// Intensités de la manette (vibrations, gâchettes), bornées à 100 %.
    pub fn intensities(&self) -> (u8, u8) {
        (self.rumble.min(FULL_INTENSITY), self.triggers.min(FULL_INTENSITY))
    }

    /// Débit effectif, en kbit/s.
    pub fn bitrate(&self) -> u32 {
        self.bitrate_kbps.map(|b| b.clamp(BITRATE_MIN, BITRATE_MAX)).unwrap_or_else(|| self.resolution.auto_bitrate())
    }

    /// Ce que la console reçoit. `hevc_decoder` : l'appareil décode le
    /// H.265 ; `ps5` : la console est une PS5 (une PS4 ne connaît que le
    /// H.264, sans HDR).
    pub fn profile(&self, hevc_decoder: bool, ps5: bool) -> Profile {
        let (width, height) = self.resolution.size();
        let hevc = ps5
            && match self.codec {
                CodecChoice::Auto => hevc_decoder,
                CodecChoice::H264 => false,
                CodecChoice::H265 => true,
            };
        Profile {
            width,
            height,
            max_fps: if self.fps == 30 { 30 } else { 60 },
            bitrate: self.bitrate(),
            hevc,
            hdr: hevc && self.hdr,
            dualsense: true,
            // Sur le réseau ; par le relais, la partie la réduit (`Endpoint::mtu`).
            mtu: super::endpoint::MTU_LAN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_is_1080p60_hevc_15mbit() {
        let s = PlaySettings::default();
        assert!(!s.is_custom());
        let p = s.profile(true, true);
        assert_eq!((p.width, p.height, p.max_fps, p.bitrate, p.hevc, p.hdr), (1920, 1080, 60, 15_000, true, false));
        assert!(!s.profile(false, true).hevc);
    }

    #[test]
    fn custom_settings() {
        let s = PlaySettings { resolution: Resolution::P720, fps: 30, bitrate_kbps: Some(250_000), codec: CodecChoice::H264, hdr: true, fullscreen: false, upscale: false, sharpness: 0, rumble: 30, triggers: 0 };
        assert!(s.is_custom());
        let p = s.profile(true, true);
        assert_eq!((p.width, p.height, p.max_fps, p.bitrate), (1280, 720, 30, BITRATE_MAX));
        // Le HDR demande le H.265.
        assert!(!p.hevc && !p.hdr);
        // HDR, plein écran et agrandissement ne rendent pas « Personnalisé ».
        assert!(!PlaySettings { hdr: true, fullscreen: false, upscale: false, sharpness: 90, ..Default::default() }.is_custom());
        // Les intensités de la manette non plus.
        assert!(!PlaySettings { rumble: 0, triggers: 50, ..Default::default() }.is_custom());
    }

    #[test]
    fn ps4_is_h264_without_hdr() {
        let s = PlaySettings { codec: CodecChoice::H265, hdr: true, ..Default::default() };
        let p = s.profile(true, false);
        assert!(!p.hevc && !p.hdr);
    }

    #[test]
    fn json_is_stable_and_tolerant() {
        let s = PlaySettings { resolution: Resolution::P540, bitrate_kbps: Some(40_000), ..Default::default() };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"resolution\":\"p540\"") && json.contains("\"codec\":\"auto\""));
        assert_eq!(serde_json::from_str::<PlaySettings>(&json).unwrap(), s);
        assert_eq!(serde_json::from_str::<PlaySettings>("{}").unwrap(), PlaySettings::default());
        // Réglages d'avant l'agrandissement : allumé, netteté par défaut.
        let old: PlaySettings = serde_json::from_str("{\"resolution\":\"p720\",\"fullscreen\":false}").unwrap();
        assert!(old.upscale && old.sharpness == DEFAULT_SHARPNESS && !old.fullscreen);
        // Réglages d'avant les intensités : entières.
        assert_eq!(old.intensities(), (100, 100));
        let s: PlaySettings = serde_json::from_str("{\"rumble\":40,\"triggers\":0}").unwrap();
        assert_eq!(s.intensities(), (40, 0));
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"rumble\":40") && json.contains("\"triggers\":0"));
        // Au-delà de 100 : bornées.
        let s: PlaySettings = serde_json::from_str("{\"rumble\":250}").unwrap();
        assert_eq!(s.intensities(), (100, 100));
        // Valeur illisible : le JSON entier est refusé, l'app retombe sur Auto.
        assert!(serde_json::from_str::<PlaySettings>("{\"rumble\":-1}").is_err());
    }
}
