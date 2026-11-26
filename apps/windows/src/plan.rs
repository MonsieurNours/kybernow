//! Ce qu'on demande à l'ordinateur pour une session : réglages d'image et
//! mode de son écran, à partir des choix de l'utilisateur, de ce que la carte
//! graphique décode et de l'écran de ce PC. Partagé par l'app et la ligne de
//! commande.

use kybernow_core::autotest::AutoResult;
use kybernow_core::console::Mode;
use kybernow_core::profile::{Capabilities, Codec, PYROWAVE_MBPS, PYROWAVE_MBPS_444, Profile, StreamSettings};

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
    /// Profil Auto : le résultat du test gardé pour cet ordinateur
    /// (`Computer::auto`), qui donne les réglages du profil
    /// (`AutoResult::settings`). Auto sans résultat : ceux de Jeu.
    pub auto: Option<AutoResult>,
    /// « Débit adaptatif » coupé (carte « Débit ») : le débit reste fixe.
    /// Hors du profil, il ne rend pas « Personnalisé ».
    pub fixed: bool,
}

/// Les réglages d'un profil sur ce PC : ceux du test pour Auto
/// (`AutoResult::settings` ; HDR et câble posés ensuite par la session),
/// `StreamSettings::for_profile` pour les autres.
pub fn profile_settings(profile: Profile, auto: Option<&AutoResult>, caps: Capabilities) -> StreamSettings {
    match (profile, auto) {
        (Profile::Auto, Some(r)) => r.settings(false, false),
        (Profile::Auto, None) => StreamSettings::for_profile(Profile::Jeu, caps),
        (p, _) => StreamSettings::for_profile(p, caps),
    }
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
    let mut settings = profile_settings(c.profile.unwrap_or(Profile::Jeu), c.auto.as_ref(), caps);
    let mut custom = c.profile.is_none();
    if let Some(codec) = c.codec {
        if codec != settings.codec {
            settings.codec = codec;
            custom = true;
        }
    }
    if let Some(y) = c.yuv444 {
        let y = y && matches!(settings.codec, Codec::Hevc | Codec::PyroWave);
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
    // Le 4:4:4 n'existe qu'en HEVC, et seulement si la carte le décode ; et
    // en PyroWave (presque sans coût, docs/link-ui.md « PyroWave »).
    let can444 = match settings.codec {
        Codec::Hevc => caps.hevc444,
        Codec::PyroWave => caps.pyrowave,
        _ => false,
    };
    if settings.yuv444 && !can444 {
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
        // PyroWave choisi à la main : le débit de PyroWave pour ce mode (celui
        // qu'« Auto » prendrait par câble), pas celui du HEVC.
        let base = if settings.codec == Codec::PyroWave {
            StreamSettings { mbps: if settings.yuv444 { PYROWAVE_MBPS_444 } else { PYROWAVE_MBPS }, ..settings.clone() }
        } else {
            settings.clone()
        };
        let profile_mbps = base.bitrate(mode.width, mode.height, mode.refresh) / 1_000_000;
        settings.profile = None;
        settings.mbps = c.mbps.unwrap_or(profile_mbps);
    }
    settings.adaptive = !c.fixed;
    (settings, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::autotest::{Link, Measures, Reason, Reasons};

    fn caps() -> Capabilities {
        Capabilities { hevc: true, hevc444: true, av1: true, hdr: true, pyrowave: true }
    }

    fn screen() -> Mode {
        Mode { width: 3840, height: 2160, refresh: 60 }
    }

    fn result(codec: Codec) -> AutoResult {
        AutoResult {
            at: 1_790_000_000,
            network: Some("Ethernet".into()),
            measures: Measures { link: Link::Cable, latency: None, bandwidth: None, decode: Vec::new() },
            codec,
            yuv444: true,
            width: 2560,
            height: 1440,
            refresh: 120,
            mbps: 180,
            mode_mbps: 150,
            reliable: true,
            reasons: Reasons {
                codec: Reason::BestFormat,
                mode: Reason::DecodeLimited,
                yuv444: Reason::Yuv444Allowed,
                bitrate: Reason::Measured,
                transport: Reason::Unsteady,
                decode_fps: Some(90.0),
            },
            station: "Atelier".into(),
        }
    }

    #[test]
    fn profil_auto_reglages_du_test() {
        let c = Choices { profile: Some(Profile::Auto), auto: Some(result(Codec::Hevc)), ..Default::default() };
        let (s, mode) = plan(&c, caps(), screen());
        assert_eq!(s.profile, Some(Profile::Auto));
        assert_eq!((s.codec, s.yuv444, s.mbps, s.reliable, s.adaptive), (Codec::Hevc, true, 180, true, true));
        // La définition du test passe par les réglages de définition (`size`), pas par le plan.
        assert_eq!(mode, screen());
        // PyroWave choisi par le test : gardé tel quel (la session le prend par câble seulement).
        let c = Choices { profile: Some(Profile::Auto), auto: Some(result(Codec::PyroWave)), ..Default::default() };
        let (s, _) = plan(&c, caps(), screen());
        assert_eq!((s.profile, s.codec, s.mbps), (Some(Profile::Auto), Codec::PyroWave, 180));
    }

    #[test]
    fn profil_auto_sans_test_comme_jeu() {
        let (s, _) = plan(&Choices { profile: Some(Profile::Auto), ..Default::default() }, caps(), screen());
        assert_eq!(s.profile, Some(Profile::Jeu));
    }

    #[test]
    fn auto_retouche_devient_personnalise() {
        // Le format du test n'est pas un écart ; un autre l'est.
        let same = Choices { profile: Some(Profile::Auto), auto: Some(result(Codec::Av1)), codec: Some(Codec::Av1), ..Default::default() };
        assert_eq!(plan(&same, caps(), screen()).0.profile, Some(Profile::Auto));
        let other = Choices { codec: Some(Codec::Hevc), ..same };
        assert_eq!(plan(&other, caps(), screen()).0.profile, None);
    }

    #[test]
    fn debit_adaptatif_hors_du_profil() {
        let (s, _) = plan(&Choices { profile: Some(Profile::Bureau), fixed: true, ..Default::default() }, caps(), screen());
        assert_eq!(s.profile, Some(Profile::Bureau));
        assert!(!s.adaptive);
        let (s, _) = plan(&Choices { profile: Some(Profile::Jeu), mbps: Some(80), fixed: true, ..Default::default() }, caps(), screen());
        assert_eq!((s.profile, s.mbps, s.adaptive), (None, 80, false));
        let (s, _) = plan(&Choices { profile: Some(Profile::Jeu), mbps: Some(80), ..Default::default() }, caps(), screen());
        assert!(s.adaptive);
    }
}
