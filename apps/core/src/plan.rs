//! Ce qu'on demande à l'ordinateur pour une session (WINAPP-52, TRANS-42) :
//! réglages d'image et mode de son écran, à partir des choix de
//! l'utilisateur, de ce que l'appareil décode et de son écran. Une seule
//! règle pour toutes les apps (avant : l'app Windows en Rust et en
//! TypeScript, les autres à leur façon) ; les ponts la donnent en JSON
//! ([`plan_json`]).

use serde::{Deserialize, Deserializer};

use crate::autotest::AutoResult;
use crate::console::Mode;
use crate::profile::{Capabilities, Codec, PYROWAVE_MBPS, PYROWAVE_MBPS_444, Profile, StreamSettings};

/// Les choix de l'utilisateur pour un ordinateur (JSON des ponts : mêmes
/// noms, tous facultatifs).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Choices {
    /// `jeu`, `bureau`, `max`, `auto` ; absent ou inconnu : Jeu.
    #[serde(deserialize_with = "profile_key")]
    pub profile: Option<Profile>,
    /// `h264`, `hevc`, `av1`, `pyrowave` ; absent : celui du profil.
    #[serde(deserialize_with = "codec_key")]
    pub codec: Option<Codec>,
    pub yuv444: Option<bool>,
    pub mbps: Option<u32>,
    /// Transport fiable (`Some(true)`) ou rapide ; sinon celui du profil.
    pub reliable: Option<bool>,
    /// Écran de l'ordinateur à 3840 × 2160 au plus (sans effet avec une
    /// définition sur mesure, `size`).
    pub max_4k: bool,
    /// Mode imposé (définition sur mesure, fenêtre) ; sinon celui de l'écran
    /// de l'appareil.
    pub size: Option<Mode>,
    /// Sur batterie, « Alléger » : 30 Hz au plus (le débit d'un profil suit).
    pub lighten: bool,
    /// Profil Auto : le résultat du test gardé pour cet ordinateur, qui donne
    /// les réglages du profil (`AutoResult::settings`). Auto sans résultat
    /// (le choisir ouvre le test) : ceux de Jeu.
    pub auto: Option<AutoResult>,
    /// « Débit adaptatif » coupé : le débit reste fixe. Hors du profil, il ne
    /// rend pas « Personnalisé ».
    pub fixed: bool,
    /// Plafond de l'encodeur gardé pour cet ordinateur, en Mbit/s par écran,
    /// quel que soit le profil ; sans lui, aucun (même pour Auto : le
    /// plafond gardé passe avant celui du résultat).
    pub encoder_max: Option<u32>,
}

fn profile_key<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Profile>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.and_then(|k| Profile::parse(&k)))
}

fn codec_key<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Codec>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.and_then(|k| Codec::parse(&k)))
}

/// Les réglages d'un profil sur cet appareil : ceux du test pour Auto
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
    if let Some(codec) = c.codec
        && codec != settings.codec
    {
        settings.codec = codec;
        custom = true;
    }
    if let Some(y) = c.yuv444 {
        let y = y && matches!(settings.codec, Codec::Hevc | Codec::PyroWave);
        if y != settings.yuv444 {
            settings.yuv444 = y;
            custom = true;
        }
    }
    if let Some(r) = c.reliable
        && r != settings.reliable
    {
        settings.reliable = r;
        custom = true;
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
    // réduction de l'H.264), comme pour un profil.
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
    // Hors du profil : il ne rend pas « Personnalisé ».
    settings.encoder_max = c.encoder_max.filter(|&n| n > 0);
    (settings, mode)
}

/// Les valeurs communes que les interfaces montrent ou appliquent (TRANS-40
/// à 42), pour les ponts : les profils sur cet appareil (`profiles` :
/// `jeu`, `bureau`, `max`, [`StreamSettings::to_json`]), PyroWave, les
/// bornes du débit à la main, le débit en arrière-plan de chaque appareil
/// (bit/s : `computer`, `apple_mobile`, `apple_tv`, `android`, `lg_tv`), les maxima,
/// les délais, le port et l'exemple d'adresse d'Admin, le relais public.
pub fn values_json(caps: Capabilities) -> serde_json::Value {
    use kybernow_rules::{address, limits, relay};
    let profile = |p: Profile| StreamSettings::for_profile(p, caps).to_json();
    serde_json::json!({
        "profiles": { "jeu": profile(Profile::Jeu), "bureau": profile(Profile::Bureau), "max": profile(Profile::Max) },
        "pyrowave_mbps": limits::PYROWAVE_MBPS,
        "pyrowave_mbps_444": limits::PYROWAVE_MBPS_444,
        "manual_mbps": { "min": limits::MANUAL_MBPS_MIN, "step": limits::MANUAL_MBPS_STEP, "max": limits::MANUAL_MBPS_MAX, "max_pyrowave": limits::MANUAL_MBPS_MAX_PYROWAVE, "lg_tv_max": limits::LG_TV_MBPS_MAX },
        "background_bitrate": {
            "computer": limits::background_bitrate(limits::Device::Computer),
            "apple_mobile": limits::background_bitrate(limits::Device::AppleMobile),
            "apple_tv": limits::background_bitrate(limits::Device::AppleTv),
            "android": limits::background_bitrate(limits::Device::Android),
            "lg_tv": limits::background_bitrate(limits::Device::LgTv),
        },
        "max_guests": limits::MAX_GUESTS,
        "max_sessions": limits::MAX_SESSIONS,
        "relay_link_wait_secs": limits::RELAY_LINK_WAIT_SECS,
        "station_notice_wait_secs": limits::STATION_NOTICE_WAIT_SECS,
        "station_notice_before": limits::STATION_NOTICE_BEFORE,
        "admin_port": address::ADMIN_PORT,
        "address_example": address::ADDRESS_EXAMPLE,
        "default_relay": relay::DEFAULT_RELAY,
    })
}

/// Le nom des réglages : « Jeu · HEVC », « Personnalisé · HEVC 4:4:4 ».
pub fn label(settings: &StreamSettings) -> String {
    format!("{} · {}{}", settings.label(), settings.codec.label(), if settings.yuv444 { " 4:4:4" } else { "" })
}

/// [`plan`] pour les ponts : `choices` en JSON ([`Choices`]), rend
/// `{"settings": {…StreamSettings::to_json}, "mode": {width, height,
/// refresh}, "mbps": <débit de la session pour ce mode, Mbit/s>, "label"}`,
/// ou `None` si le JSON ne se lit pas.
pub fn plan_json(choices: &str, caps: Capabilities, screen: Mode) -> Option<serde_json::Value> {
    let c: Choices = serde_json::from_str(choices).ok()?;
    let (settings, mode) = plan(&c, caps, screen);
    let mbps = settings.bitrate(mode.width, mode.height, mode.refresh) / 1_000_000;
    Some(serde_json::json!({ "settings": settings.to_json(), "mode": mode, "mbps": mbps, "label": label(&settings) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autotest::{EncoderMeasure, Link, Measures, Reason, Reasons};

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
            measures: Measures { link: Link::Cable, latency: None, bandwidth: None, decode: Vec::new(), encoder: None },
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

    #[test]
    fn plafond_de_l_encodeur_garde() {
        // Quel que soit le profil, sans le rendre « Personnalisé ».
        let (s, _) = plan(&Choices { profile: Some(Profile::Jeu), encoder_max: Some(150), ..Default::default() }, caps(), screen());
        assert_eq!((s.profile, s.encoder_max), (Some(Profile::Jeu), Some(150)));
        assert_eq!(s.to_json()["encoder_max"], 150);
        // Sans plafond gardé : aucun ; zéro compte comme aucun.
        let (s, _) = plan(&Choices { profile: Some(Profile::Bureau), ..Default::default() }, caps(), screen());
        assert_eq!(s.encoder_max, None);
        let (s, _) = plan(&Choices { profile: Some(Profile::Bureau), encoder_max: Some(0), ..Default::default() }, caps(), screen());
        assert_eq!(s.encoder_max, None);
        // Auto : le plafond gardé passe avant celui du résultat (une session
        // a pu redescendre depuis le test).
        let mut r = result(Codec::Hevc);
        r.measures.encoder = Some(EncoderMeasure { codec: Codec::Hevc, yuv444: true, width: 2560, height: 1440, refresh: 120, mbps: 200, limited: true });
        let c = Choices { profile: Some(Profile::Auto), auto: Some(r.clone()), encoder_max: Some(r.next_encoder_max(None).unwrap() - 50), ..Default::default() };
        let (s, _) = plan(&c, caps(), screen());
        assert_eq!((s.profile, s.encoder_max), (Some(Profile::Auto), Some(150)));
    }

    /// Les valeurs communes des ponts (TRANS-40 à 42), et le débit de
    /// PyroWave et les bornes à la main du cœur.
    #[test]
    fn valeurs_communes() {
        let v = values_json(caps());
        assert_eq!((v["profiles"]["jeu"]["mbps"].as_u64(), v["profiles"]["bureau"]["mbps"].as_u64(), v["profiles"]["max"]["mbps"].as_u64()), (Some(60), Some(90), Some(300)));
        let h264 = values_json(Capabilities::default());
        assert_eq!((h264["profiles"]["jeu"]["mbps"].as_u64(), h264["profiles"]["bureau"]["mbps"].as_u64()), (Some(90), Some(180)), "+1/2 en H.264, +1/3 sans 4:4:4");
        assert_eq!((v["manual_mbps"]["max"].as_u64(), v["manual_mbps"]["max_pyrowave"].as_u64()), (Some(500), Some(2500)));
        assert_eq!(
            (v["background_bitrate"]["computer"].as_u64(), v["background_bitrate"]["apple_mobile"].as_u64(), v["background_bitrate"]["android"].as_u64()),
            (Some(20_000_000), Some(8_000_000), Some(2_000_000))
        );
        assert_eq!((v["max_guests"].as_u64(), v["max_sessions"].as_u64(), v["relay_link_wait_secs"].as_u64()), (Some(4), Some(4), Some(10)));
        assert_eq!((v["admin_port"].as_u64(), v["default_relay"].as_str()), (Some(9443), Some("share.kybernow.app")));
        assert_eq!(v["station_notice_before"], crate::station_offer::BEFORE_NOTICE);
        assert_eq!(crate::profile::pyrowave_mbps(3840, 2160, 60, false), 800);
        assert_eq!(crate::profile::pyrowave_mbps(1920, 1080, 60, true), 480, "0,4 fois au moins");
        assert_eq!(crate::profile::manual_mbps(crate::profile::Device::Computer, Codec::PyroWave), (5, 2500));
        assert_eq!(crate::profile::manual_mbps(crate::profile::Device::LgTv, Codec::Hevc), (5, 95));
    }

    /// Pour les ponts : le même plan en JSON, avec le débit de la session
    /// pour ce mode (comme `sessionPlan` de l'app Windows le recalculait).
    #[test]
    fn plan_en_json() {
        let v = plan_json(r#"{"profile":"jeu"}"#, caps(), screen()).unwrap();
        assert_eq!(v["settings"]["profile"], "jeu");
        assert_eq!(v["mode"]["width"], 3840);
        assert_eq!(v["mbps"], 60);
        assert_eq!(v["label"], "Jeu · HEVC");
        let v = plan_json(r#"{"profile":"max","max_4k":true,"size":{"width":5120,"height":2880,"refresh":60}}"#, caps(), screen()).unwrap();
        assert_eq!((v["mode"]["width"].as_u64(), v["label"].as_str()), (Some(3840), Some("Max · HEVC 4:4:4")), "4K au plus");
        let v = plan_json(r#"{"profile":"jeu","mbps":80}"#, caps(), screen()).unwrap();
        assert_eq!((v["settings"]["profile"].is_null(), v["mbps"].as_u64()), (true, Some(80)));
        assert!(plan_json("pas du json", caps(), screen()).is_none());
    }
}
