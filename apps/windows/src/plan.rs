//! Ce qu'on demande à l'ordinateur pour une session : réglages d'image et
//! mode de son écran, à partir des choix de l'utilisateur, de ce que la carte
//! graphique décode et de l'écran de ce PC. Partagé par l'app et la ligne de
//! commande.

use kybernow_core::console::Mode;
use kybernow_core::profile::{Capabilities, Codec, Profile, StreamSettings};

#[derive(Debug, Clone, Default)]
pub struct Choices {
    pub profile: Option<Profile>,
    pub codec: Option<Codec>,
    pub yuv444: Option<bool>,
    pub mbps: Option<u32>,
    /// Transport fiable (`Some(true)`) ou rapide ; sinon celui du profil.
    pub reliable: Option<bool>,
    /// Écran de l'ordinateur à 3840 × 2160 au plus.
    pub max_4k: bool,
    /// Mode imposé ; sinon celui de l'écran de ce PC.
    pub size: Option<Mode>,
    /// Sur batterie : 30 Hz au plus (le débit d'un profil suit).
    pub lighten: bool,
}

/// Réduit un mode pour tenir dans `max_w` × `max_h`, en gardant les
/// proportions et des dimensions paires.
fn fit(mut m: Mode, max_w: u32, max_h: u32) -> Mode {
    if m.width > max_w || m.height > max_h {
        let scale = (max_w as f64 / m.width as f64).min(max_h as f64 / m.height as f64);
        m.width = ((m.width as f64 * scale) as u32) & !1;
        m.height = ((m.height as f64 * scale) as u32) & !1;
    }
    m
}

pub fn plan(c: &Choices, caps: Capabilities, screen: Mode) -> (StreamSettings, Mode) {
    let mut mode = c.size.unwrap_or(screen);
    if c.max_4k {
        mode = fit(mode, 3840, 2160);
    }
    if c.lighten {
        mode.refresh = mode.refresh.min(30);
    }
    let mut settings = StreamSettings::for_profile(c.profile.unwrap_or(Profile::Jeu), caps);
    let mut custom = c.profile.is_none();
    if let Some(codec) = c.codec {
        if codec != settings.codec {
            settings.codec = codec;
            custom = true;
        }
    }
    if let Some(y) = c.yuv444 {
        let y = y && settings.codec == Codec::Hevc;
        if y != settings.yuv444 {
            settings.yuv444 = y;
            custom = true;
        }
    }
    if let Some(r) = c.reliable {
        if r != settings.reliable {
            settings.reliable = r;
            custom = true;
        }
    }
    // Le 4:4:4 n'existe qu'en HEVC, et seulement si la carte le décode.
    if settings.yuv444 && (settings.codec != Codec::Hevc || !caps.hevc444) {
        settings.yuv444 = false;
    }
    if c.mbps.is_some() {
        custom = true;
    }
    // NVENC n'encode pas l'H.264 au-delà de 4096 de large.
    if settings.codec == Codec::H264 {
        mode = fit(mode, 4096, 4096);
    }
    // Un réglage retouché rend le profil « Personnalisé » : son débit est
    // celui que le profil donnait pour ce mode, le mode demandé (après la
    // réduction de l'H.264), comme pour un profil. L'interface refait ce
    // calcul (`sessionPlan`, ui/src/lib/api.ts).
    if custom {
        let profile_mbps = settings.bitrate(mode.width, mode.height, mode.refresh) / 1_000_000;
        settings.profile = None;
        settings.mbps = c.mbps.unwrap_or(profile_mbps);
    }
    (settings, mode)
}
