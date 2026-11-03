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
//!
//! « Personnalisé » : définition, images par seconde, débit ou format qui
//! s'écartent d'Auto. Le HDR (puce de l'accueil) et le plein écran n'en
//! font pas partie.

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
}

fn sixty() -> u32 {
    60
}
fn yes() -> bool {
    true
}

impl Default for PlaySettings {
    fn default() -> Self {
        PlaySettings { resolution: Resolution::Auto, fps: 60, bitrate_kbps: None, codec: CodecChoice::Auto, hdr: false, fullscreen: true }
    }
}

impl PlaySettings {
    /// Définition, images par seconde, débit ou format s'écartent d'Auto.
    pub fn is_custom(&self) -> bool {
        self.resolution != Resolution::Auto || self.fps != 60 || self.bitrate_kbps.is_some() || self.codec != CodecChoice::Auto
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
        let s = PlaySettings { resolution: Resolution::P720, fps: 30, bitrate_kbps: Some(250_000), codec: CodecChoice::H264, hdr: true, fullscreen: false };
        assert!(s.is_custom());
        let p = s.profile(true, true);
        assert_eq!((p.width, p.height, p.max_fps, p.bitrate), (1280, 720, 30, BITRATE_MAX));
        // Le HDR demande le H.265.
        assert!(!p.hevc && !p.hdr);
        // HDR et plein écran ne rendent pas « Personnalisé ».
        assert!(!PlaySettings { hdr: true, fullscreen: false, ..Default::default() }.is_custom());
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
    }
}
