//! HDR des sessions d'ordinateur (docs/hdr.md) : ce que le pont JNI tire du
//! cœur pour Kotlin (la puce « HDR », les valeurs d'un écran de l'appareil)
//! et pour MediaCodec (les métadonnées HDR10 de secours). Sans JNI ni NDK :
//! compilé et essayé sur toutes les cibles (`cargo test -p kybernow-android`).

use kybernow_core::console::Info;
use kybernow_core::profile::{self, Capabilities, HdrLuminance, StreamSettings};

/// La puce « HDR » (`profile::hdr_state`) en JSON pour Kotlin :
/// `{"shown", "available", "reason", "kind"}`. `info_json` : l'info de
/// l'ordinateur (`consoleInfo`, `hdr` : `{available, reason}` ; absent :
/// Station d'avant le HDR) ; `settings_json` : les réglages de la session
/// (seuls `codec` et `profile` comptent) ; `caps_json` :
/// `{hevc, hevc444, av1, hdr}`. `None` si un JSON est illisible.
pub fn state_json(info_json: &str, settings_json: &str, caps_json: &str, screen_hdr: bool) -> Option<String> {
    // Une info vide (ordinateur pas encore joint) : Station sans HDR connu.
    let info: Info = if info_json.trim().is_empty() { Info::default() } else { serde_json::from_str(info_json).ok()? };
    let settings = StreamSettings::from_json(settings_json).ok()?;
    let caps: Capabilities = serde_json::from_str(caps_json).ok()?;
    serde_json::to_string(&profile::hdr_state(&info, &settings, caps, screen_hdr)).ok()
}

/// Les valeurs HDR d'un écran de l'appareil, d'après le JSON de Kotlin
/// (`{max, full_frame, min, sdr_white}` en nits) ; vide ou illisible :
/// `None` (valeurs par défaut du cœur).
pub fn luminance(json: Option<&str>) -> Option<HdrLuminance> {
    let text = json?.trim();
    if text.is_empty() {
        return None;
    }
    serde_json::from_str::<HdrLuminance>(text).ok()
}

/// Taille de `MediaFormat.KEY_HDR_STATIC_INFO` : l'identifiant du type, puis
/// douze valeurs de 16 bits.
pub const STATIC_INFO_LEN: usize = 25;

/// `MediaFormat.KEY_HDR_STATIC_INFO` pour une image HDR10 (CTA-861.3, type 1,
/// petit-boutiste, comme `HDRStaticInfo` d'Android) : primaires BT.2020 et
/// blanc D65 (unités de 0,000 02), luminosité maximale de l'écran de
/// matriçage (nit), minimale (0,0001 nit), MaxCLL et MaxFALL (nit). Les
/// valeurs sont celles de l'écran de l'appareil, données à la Station : le
/// jeu fait son tone mapping pour elles. MediaCodec ne s'en sert que si le
/// flux n'a pas ses propres SEI (mastering display, content light level).
pub fn static_info(l: HdrLuminance) -> [u8; STATIC_INFO_LEN] {
    let l = l.clamped();
    let nits = |v: f64| v.round().clamp(0.0, u16::MAX as f64) as u16;
    let values: [u16; 12] = [
        35400,
        14600, // rouge 0,708 ; 0,292
        8500,
        39850, // vert 0,170 ; 0,797
        6550,
        2300, // bleu 0,131 ; 0,046
        15635,
        16450, // blanc D65 0,3127 ; 0,3290
        nits(l.max),
        (l.min * 10_000.0).round().clamp(0.0, u16::MAX as f64) as u16,
        nits(l.max),
        nits(l.full_frame),
    ];
    let mut out = [0u8; STATIC_INFO_LEN];
    for (i, v) in values.iter().enumerate() {
        out[1 + 2 * i..3 + 2 * i].copy_from_slice(&v.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &str = r#"{"hevc": true, "hevc444": false, "av1": false, "hdr": true}"#;
    const JEU: &str = r#"{"profile": "jeu", "codec": "hevc", "yuv444": false, "mbps": 60, "reliable": false}"#;

    fn state(info: &str, settings: &str, caps: &str, screen: bool) -> serde_json::Value {
        serde_json::from_str(&state_json(info, settings, caps, screen).expect("JSON")).unwrap()
    }

    #[test]
    fn puce_d_apres_l_info_les_reglages_et_le_decodeur() {
        kybernow_core::i18n::set_current(kybernow_core::i18n::Lang::Fr);
        let on = r#"{"hdr": {"available": true}}"#;
        let s = state(on, JEU, ALL, true);
        assert_eq!((s["shown"].as_bool(), s["available"].as_bool()), (Some(true), Some(true)));
        assert!(s["reason"].is_null());
        // Écran SDR : aucune trace du HDR.
        assert_eq!(state(on, JEU, ALL, false)["shown"].as_bool(), Some(false));
        // Station d'avant le HDR (ou ordinateur pas encore joint) : inactive, raison de la Station.
        for info in ["{}", ""] {
            let s = state(info, JEU, ALL, true);
            assert_eq!((s["available"].as_bool(), s["kind"].as_str()), (Some(false), Some("off")));
            assert_eq!(s["reason"].as_str(), Some("L'ordinateur ne propose pas le HDR."));
        }
        let s = state(r#"{"hdr": {"available": false, "reason": "windows"}}"#, JEU, ALL, true);
        assert_eq!(s["kind"].as_str(), Some("windows"));
        // H.264 choisi à la main (sans profil) ; un profil en H.264 passe lui-même en HEVC.
        let h264 = r#"{"codec": "h264", "mbps": 60}"#;
        assert_eq!(state(on, h264, ALL, true)["kind"].as_str(), Some("codec"));
        assert_eq!(state(on, r#"{"profile": "jeu", "codec": "h264", "mbps": 60}"#, ALL, true)["available"].as_bool(), Some(true));
        // Décodeur sans HEVC Main 10 ni AV1 10 bits.
        let sdr = r#"{"hevc": true, "hevc444": false, "av1": false, "hdr": false}"#;
        let s = state(on, JEU, sdr, true);
        assert_eq!(s["kind"].as_str(), Some("decoder"));
        assert_eq!(s["reason"].as_str(), Some("Cet appareil ne décode pas le HDR."));
        // JSON illisible : rien.
        assert!(state_json("{", JEU, ALL, true).is_none());
        assert!(state_json(on, "{}", ALL, true).is_none());
    }

    #[test]
    fn valeurs_d_un_ecran() {
        assert_eq!(luminance(None), None);
        assert_eq!(luminance(Some("")), None);
        assert_eq!(luminance(Some("pas du JSON")), None);
        let l = luminance(Some(r#"{"max": 800, "full_frame": 400, "min": 0.05, "sdr_white": 203}"#)).unwrap();
        assert_eq!(l, HdrLuminance { max: 800.0, full_frame: 400.0, min: 0.05, sdr_white: 203.0 });
    }

    #[test]
    fn metadonnees_hdr10_de_secours() {
        let b = static_info(HdrLuminance { max: 800.0, full_frame: 400.0, min: 0.05, sdr_white: 203.0 });
        let at = |i: usize| u16::from_le_bytes([b[1 + 2 * i], b[2 + 2 * i]]);
        assert_eq!(b[0], 0, "type 1 de CTA-861.3");
        assert_eq!((at(0), at(1)), (35400, 14600), "rouge BT.2020");
        assert_eq!((at(6), at(7)), (15635, 16450), "blanc D65");
        assert_eq!((at(8), at(9), at(10), at(11)), (800, 500, 800, 400));
        // Les valeurs sont bornées comme celles données à la Station.
        let b = static_info(HdrLuminance { max: 50_000.0, full_frame: 60_000.0, min: 3.0, sdr_white: 203.0 });
        let at = |i: usize| u16::from_le_bytes([b[1 + 2 * i], b[2 + 2 * i]]);
        assert_eq!((at(8), at(9), at(10), at(11)), (10_000, 10_000, 10_000, 10_000));
        let d = static_info(HdrLuminance::default());
        let at = |i: usize| u16::from_le_bytes([d[1 + 2 * i], d[2 + 2 * i]]);
        assert_eq!((at(8), at(9), at(10), at(11)), (1000, 0, 1000, 1000));
    }
}
