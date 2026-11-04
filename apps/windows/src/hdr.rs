//! HDR de l'app Windows (docs/hdr.md, « Deuxième temps »).
//!
//! - **L'écran** : un écran de ce PC est HDR quand Windows l'a passé en HDR
//!   (« Utiliser le HDR » dans Paramètres › Affichage) : `IDXGIOutput6::
//!   GetDesc1` le dit en espace `RGB_FULL_G2084_NONE_P2020`. Ses luminosités
//!   viennent de la même description (l'EDID de l'écran, ou le profil posé
//!   par l'outil d'étalonnage HDR de Windows), le blanc du bureau du curseur
//!   « Luminosité du contenu SDR » (`DISPLAYCONFIG_SDR_WHITE_LEVEL`). Une
//!   fabrique DXGI neuve à chaque relevé : l'ancienne garde l'état d'avant.
//! - **Le rendu** (`render.rs`) : une image PQ / BT.2020 (étiquetée SMPTE
//!   2084 par la Station ou la console) part telle quelle dans une chaîne
//!   d'échange 10 bits en espace PQ BT.2020 sur un écran HDR, ramenée à ce
//!   que l'écran affiche (courbe BT.2390) quand le flux le dépasse ; sur un
//!   écran SDR (fenêtre glissée, HDR coupé, le temps que l'image repasse en
//!   SDR), une courbe la ramène au SDR. Pas de courbe de plus sinon (le jeu
//!   a fait la sienne d'après la luminosité de cet écran).
//!
//! Les règles et les courbes (les mêmes que le shader) se testent sur le Mac.

use kybernow_core::profile::HdrLuminance;

/// L'état HDR d'un écran de ce PC.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenHdr {
    /// Windows affiche cet écran en HDR.
    pub hdr: bool,
    /// Ses luminosités (écran HDR seulement) ; `None` : par défaut.
    pub luminance: Option<HdrLuminance>,
}

/// Le blanc du bureau d'après `DISPLAYCONFIG_SDR_WHITE_LEVEL` (en
/// millièmes de 80 nits : 1000 = 80 nits).
pub fn sdr_white_nits(level: u32) -> Option<f64> {
    (level > 0).then(|| f64::from(level) * 80.0 / 1000.0)
}

/// Les luminosités d'un écran HDR d'après `DXGI_OUTPUT_DESC1` et le blanc du
/// bureau : une valeur absente ou illisible (0, des pilotes en donnent)
/// prend celle par défaut ; le cœur borne le reste.
pub fn luminance_from(max: f32, full_frame: f32, min: f32, sdr_white: Option<f64>) -> HdrLuminance {
    let d = HdrLuminance::default();
    let max = if max.is_finite() && max >= 100.0 { f64::from(max) } else { d.max };
    let full_frame = if full_frame.is_finite() && full_frame > 0.0 { f64::from(full_frame).min(max) } else { max };
    let min = if min.is_finite() && (0.0..=1.0).contains(&min) { f64::from(min) } else { d.min };
    HdrLuminance { max, full_frame, min, sdr_white: sdr_white.unwrap_or(d.sdr_white) }.clamped()
}

// PQ (SMPTE ST 2084) : valeur codée de 0 à 1 ↔ 0 à 10 000 nits.
const PQ_M1: f64 = 2610.0 / 16384.0;
const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f64 = 3424.0 / 4096.0;
const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

#[allow(dead_code)] // le shader de render.rs fait le même calcul ; ici pour les essais
pub fn pq_to_nits(e: f64) -> f64 {
    let p = e.clamp(0.0, 1.0).powf(1.0 / PQ_M2);
    10_000.0 * ((p - PQ_C1).max(0.0) / (PQ_C2 - PQ_C3 * p)).powf(1.0 / PQ_M1)
}

pub fn nits_to_pq(nits: f64) -> f64 {
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

/// Ce que le rendu fait d'une image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Image SDR, chaîne d'échange 8 bits : telle quelle.
    Sdr,
    /// Image PQ sur un écran HDR : chaîne d'échange 10 bits en espace PQ
    /// BT.2020, valeurs gardées (ramenées à l'écran s'il le faut).
    Pq,
    /// Image PQ sur un écran SDR : ramenée au SDR, chaîne 8 bits.
    PqToSdr,
}

impl Output {
    /// `frame_pq` : l'image est en PQ ; `screen_hdr` : l'écran de la fenêtre
    /// est en HDR et sa chaîne d'échange accepte l'espace PQ.
    pub fn of(frame_pq: bool, screen_hdr: bool) -> Self {
        match (frame_pq, screen_hdr) {
            (false, _) => Self::Sdr,
            (true, true) => Self::Pq,
            (true, false) => Self::PqToSdr,
        }
    }

    /// La chaîne d'échange en 10 bits, espace PQ.
    pub fn hdr_chain(self) -> bool {
        self == Self::Pq
    }

    /// Le mode de sortie du shader (`out_mode`).
    pub fn shader(self) -> u32 {
        match self {
            Self::Sdr => 0,
            Self::Pq => 1,
            Self::PqToSdr => 2,
        }
    }
}

/// L'image PQ est-elle en PQ ? Étiquetée SMPTE 2084 ; sans étiquette, une
/// image 10 bits d'un flux annoncé HDR (`stream_hdr` : le cœur l'a dit pour
/// cet écran, ou la console pour sa partie).
pub fn frame_is_pq(trc: i32, ten_bit: bool, stream_hdr: bool) -> bool {
    const SMPTE2084: i32 = 16;
    const UNSPECIFIED: i32 = 2;
    trc == SMPTE2084 || (trc == UNSPECIFIED && ten_bit && stream_hdr)
}

/// La crête du flux, en nits : MaxCLL quand le flux le donne, sinon la
/// luminosité de l'écran de matriçage, sinon 1 000 nits (les valeurs par
/// défaut du réglage du HDR, celles que la Station donne alors à Windows).
pub fn content_peak(max_cll: Option<u32>, mastering_max: Option<f64>) -> f64 {
    let peak = match (max_cll.filter(|v| *v > 0), mastering_max.filter(|v| v.is_finite() && *v > 0.0)) {
        (Some(cll), _) => f64::from(cll),
        (None, Some(m)) => m,
        (None, None) => HdrLuminance::default().max,
    };
    peak.clamp(100.0, 10_000.0)
}

/// Au-delà de 2 % au-dessus de ce que l'écran affiche, la courbe s'applique.
const TONE_MARGIN: f64 = 1.02;

/// Les constantes de la sortie pour le shader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    pub out_mode: u32,
    /// Courbe BT.2390 (sortie PQ seulement) : le flux dépasse l'écran.
    pub tone: bool,
    /// Crêtes du flux et de l'écran, en valeurs PQ.
    pub src_pq: f32,
    pub dst_pq: f32,
    /// Blanc du bureau, en nits : la référence du SDR (sortie vers le SDR).
    pub sdr_white: f32,
    /// Le blanc du trait du bord haut, en valeur de sortie (le blanc du
    /// bureau en PQ sur un écran HDR, 1 sinon).
    pub hint_level: f32,
}

/// Les constantes pour `output`, d'après la crête du flux (`content_peak`)
/// et les luminosités de l'écran (`None` : par défaut).
pub fn tone(output: Output, peak: f64, screen: Option<HdrLuminance>) -> Tone {
    let lum = HdrLuminance::of(screen);
    let (tone, hint_level) = match output {
        Output::Pq => (peak > lum.max * TONE_MARGIN, nits_to_pq(lum.sdr_white) as f32),
        _ => (false, 1.0),
    };
    Tone {
        out_mode: output.shader(),
        tone,
        src_pq: nits_to_pq(peak) as f32,
        dst_pq: nits_to_pq(lum.max) as f32,
        sdr_white: lum.sdr_white as f32,
        hint_level,
    }
}

/// La courbe BT.2390 (EETF) sur une valeur PQ : `src_pq` (crête du flux)
/// ramené à `dst_pq` (crête de l'écran), droite jusqu'au genou. La même
/// que `eetf` du shader.
#[allow(dead_code)] // le shader de render.rs fait le même calcul ; ici pour les essais
pub fn eetf(e: f64, src_pq: f64, dst_pq: f64) -> f64 {
    if src_pq <= dst_pq {
        return e;
    }
    let x = (e / src_pq).min(1.0);
    let maxl = dst_pq / src_pq;
    let ks = 1.5 * maxl - 0.5;
    if x <= ks {
        return e;
    }
    let t = (x - ks) / (1.0 - ks);
    let (t2, t3) = (t * t, t * t * t);
    let p = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * maxl;
    p * src_pq
}

/// Le genou de la courbe vers le SDR (en part du blanc), et ce qu'elle
/// ramène au blanc : 4 fois le blanc (comme l'image à part des invités,
/// docs/hdr.md).
#[allow(dead_code)] // le shader de render.rs fait le même calcul ; ici pour les essais
pub const SDR_KNEE: f64 = 0.75;
#[allow(dead_code)] // le shader de render.rs fait le même calcul ; ici pour les essais
pub const SDR_RANGE: f64 = 4.0;

/// La courbe vers le SDR, sur une valeur relative au blanc du bureau :
/// droite jusqu'au genou, puis Reinhard étendu jusqu'à `SDR_RANGE` fois le
/// blanc, qui donne 1 (même pente au genou). La même que `sdr_curve` du shader.
#[allow(dead_code)] // le shader de render.rs fait le même calcul ; ici pour les essais
pub fn sdr_curve(r: f64) -> f64 {
    if r <= SDR_KNEE {
        return r;
    }
    let s = (r - SDR_KNEE) / (1.0 - SDR_KNEE);
    let smax = (SDR_RANGE - SDR_KNEE) / (1.0 - SDR_KNEE);
    (SDR_KNEE + (1.0 - SDR_KNEE) * s * (1.0 + s / (smax * smax)) / (1.0 + s)).min(1.0)
}

/// Matrice Y'CbCr → R'G'B' (lignes, constante en 4e colonne) selon l'espace
/// et la plage du flux, pour des échantillons lus en `unit` par code
/// (1/255 en 8 bits ; P010 : 64/65 535, le code est dans les 10 bits du
/// haut ; 10 bits sur 16 par le processeur : 1/65 535). BT.709 par défaut.
pub fn yuv_matrix(colorspace: i32, full_range: bool, bits: u32, unit: f32) -> [[f32; 4]; 3] {
    let (r_cr, g_cb, g_cr, b_cb) = match colorspace {
        5 | 6 => (1.402, -0.344_136, -0.714_136, 1.772),
        9 | 10 => (1.4746, -0.164_553, -0.571_353, 1.8814),
        _ => (1.5748, -0.187_324, -0.468_124, 1.8556),
    };
    let shift = bits.saturating_sub(8);
    let max_code = ((1u32 << bits) - 1) as f32;
    let (y_off, y_range, c_range) = if full_range {
        (0.0, max_code, max_code)
    } else {
        ((16u32 << shift) as f32, (219u32 << shift) as f32, (224u32 << shift) as f32)
    };
    let c_off = (1u32 << (bits - 1)) as f32;
    let (sy, oy, sc, oc) = (1.0 / (y_range * unit), y_off * unit, 1.0 / (c_range * unit), c_off * unit);
    let row = |a: f32, b: f32, c: f32| -> [f32; 4] { [a * sy, b * sc, c * sc, -(a * sy * oy + b * sc * oc + c * sc * oc)] };
    [row(1.0, 0.0, r_cr), row(1.0, g_cb, g_cr), row(1.0, b_cb, 0.0)]
}

/// Les métadonnées HDR10 données à la chaîne d'échange (`SetHDRMetaData`),
/// en nits ; primaires BT.2020 et blanc D65.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hdr10 {
    pub max_mastering: f64,
    pub min_mastering: f64,
    pub max_cll: u32,
    pub max_fall: u32,
}

/// Les métadonnées à donner à l'écran pour cette image : celles du flux,
/// ramenées à l'écran quand la courbe s'applique.
pub fn hdr10_for(peak: f64, min: f64, max_fall: Option<u32>, t: &Tone, screen: Option<HdrLuminance>) -> Hdr10 {
    let lum = HdrLuminance::of(screen);
    let top = if t.tone { lum.max } else { peak };
    let fall = max_fall.filter(|v| *v > 0).map_or(top, f64::from).min(top);
    Hdr10 { max_mastering: top, min_mastering: min.clamp(0.0, 1.0), max_cll: top.round() as u32, max_fall: fall.round() as u32 }
}

/// Les écrans de ce PC en HDR ou non, par nom de périphérique
/// (`\\.\DISPLAY1`, celui de `displays::Monitor::key`).
#[cfg(windows)]
pub fn outputs() -> Vec<(String, ScreenHdr)> {
    use windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6};
    use windows::core::Interface;

    let mut out: Vec<(String, ScreenHdr)> = Vec::new();
    let whites = sdr_whites();
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return out };
    for a in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(a) }) else { break };
        for o in 0.. {
            let Ok(output) = (unsafe { adapter.EnumOutputs(o) }) else { break };
            let Ok(o6) = output.cast::<IDXGIOutput6>() else { continue };
            let Ok(d) = (unsafe { o6.GetDesc1() }) else { continue };
            let key = wide(&d.DeviceName);
            if out.iter().any(|(k, _)| k.eq_ignore_ascii_case(&key)) {
                continue;
            }
            let hdr = d.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
            let white = whites.iter().find(|(k, _)| k.eq_ignore_ascii_case(&key)).and_then(|(_, w)| *w);
            let luminance = hdr.then(|| luminance_from(d.MaxLuminance, d.MaxFullFrameLuminance, d.MinLuminance, white));
            out.push((key, ScreenHdr { hdr, luminance }));
        }
    }
    out
}

#[cfg(windows)]
fn wide(s: &[u16]) -> String {
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..n])
}

/// Le blanc du bureau de chaque écran actif (« Luminosité du contenu SDR »),
/// par nom de périphérique.
#[cfg(windows)]
fn sdr_whites() -> Vec<(String, Option<f64>)> {
    use windows::Win32::Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
        DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
        DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
    };
    use windows::Win32::Foundation::ERROR_SUCCESS;

    let mut out = Vec::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != ERROR_SUCCESS {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None) != ERROR_SUCCESS {
            return out;
        }
        paths.truncate(np as usize);
        for p in &paths {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: p.sourceInfo.adapterId,
                    id: p.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
                    size: std::mem::size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
                    adapterId: p.targetInfo.adapterId,
                    id: p.targetInfo.id,
                },
                ..Default::default()
            };
            let level = if DisplayConfigGetDeviceInfo(&mut white.header) == 0 { sdr_white_nits(white.SDRWhiteLevel) } else { None };
            out.push((wide(&source.viewGdiDeviceName), level));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pq_aller_retour() {
        for nits in [0.0, 0.05, 1.0, 100.0, 203.0, 1000.0, 4000.0, 10_000.0] {
            let back = pq_to_nits(nits_to_pq(nits));
            assert!((back - nits).abs() <= nits * 1e-6 + 1e-6, "{nits} → {back}");
        }
        // Repères connus : 100 nits ≈ 0,508 ; 1 000 nits ≈ 0,752 ; 10 000 nits = 1.
        assert!((nits_to_pq(100.0) - 0.508).abs() < 0.001);
        assert!((nits_to_pq(1000.0) - 0.752).abs() < 0.001);
        assert!((nits_to_pq(10_000.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sortie_selon_image_et_ecran() {
        assert_eq!(Output::of(false, true), Output::Sdr);
        assert_eq!(Output::of(false, false), Output::Sdr);
        assert_eq!(Output::of(true, true), Output::Pq);
        assert_eq!(Output::of(true, false), Output::PqToSdr);
        assert!(Output::Pq.hdr_chain() && !Output::PqToSdr.hdr_chain() && !Output::Sdr.hdr_chain());
    }

    #[test]
    fn image_pq_d_apres_son_etiquette() {
        assert!(frame_is_pq(16, true, false));
        assert!(frame_is_pq(16, false, false));
        // Sans étiquette : seulement une image 10 bits d'un flux annoncé HDR.
        assert!(frame_is_pq(2, true, true));
        assert!(!frame_is_pq(2, false, true));
        assert!(!frame_is_pq(2, true, false));
        // BT.709 étiqueté : jamais.
        assert!(!frame_is_pq(1, true, true));
    }

    #[test]
    fn luminosites_de_l_ecran() {
        let l = luminance_from(1499.0, 799.0, 0.01, Some(240.0));
        assert_eq!((l.max, l.full_frame, l.min, l.sdr_white), (1499.0, 799.0, 0.009999999776482582, 240.0));
        // Pilote qui ne dit rien : valeurs par défaut.
        let d = luminance_from(0.0, 0.0, -1.0, None);
        assert_eq!(d, HdrLuminance::default());
        // Plein écran au plus la crête ; blanc SDR de Windows (1000 = 80 nits).
        assert_eq!(luminance_from(600.0, 900.0, 0.0, None).full_frame, 600.0);
        assert_eq!(sdr_white_nits(2500), Some(200.0));
        assert_eq!(sdr_white_nits(0), None);
    }

    #[test]
    fn crete_du_flux() {
        assert_eq!(content_peak(Some(1600), Some(4000.0)), 1600.0);
        assert_eq!(content_peak(Some(0), Some(4000.0)), 4000.0);
        assert_eq!(content_peak(None, None), 1000.0);
        assert_eq!(content_peak(Some(20_000), None), 10_000.0);
    }

    #[test]
    fn courbe_seulement_quand_le_flux_depasse_l_ecran() {
        let screen = Some(HdrLuminance { max: 600.0, full_frame: 400.0, min: 0.0, sdr_white: 240.0 });
        let t = tone(Output::Pq, 1000.0, screen);
        assert!(t.tone && t.out_mode == 1);
        assert!((t.hint_level as f64 - nits_to_pq(240.0)).abs() < 1e-6);
        // Le jeu a déjà fait sa courbe pour cet écran : rien de plus.
        assert!(!tone(Output::Pq, 600.0, screen).tone);
        assert!(!tone(Output::Pq, 610.0, screen).tone);
        // Vers le SDR : la courbe SDR (pas la BT.2390), blanc de l'écran par défaut.
        let s = tone(Output::PqToSdr, 4000.0, None);
        assert!(!s.tone && s.out_mode == 2 && s.sdr_white == 203.0 && s.hint_level == 1.0);
        assert_eq!(tone(Output::Sdr, 1000.0, None).out_mode, 0);
    }

    #[test]
    fn bt2390_garde_le_bas_et_ramene_la_crete() {
        let (src, dst) = (nits_to_pq(4000.0), nits_to_pq(600.0));
        // Les ombres et les tons moyens restent.
        for nits in [0.0, 1.0, 50.0, 100.0] {
            let e = nits_to_pq(nits);
            assert_eq!(eetf(e, src, dst), e);
        }
        // La crête du flux arrive à la crête de l'écran, jamais au-delà, et la courbe monte toujours.
        assert!((eetf(src, src, dst) - dst).abs() < 1e-9);
        assert!(eetf(1.0, src, dst) <= dst + 1e-9);
        let mut last = 0.0;
        for i in 0..=1000 {
            let v = eetf(i as f64 / 1000.0, src, dst);
            assert!(v + 1e-12 >= last, "monotone");
            last = v;
        }
        // Flux sous l'écran : identité.
        assert_eq!(eetf(0.7, nits_to_pq(500.0), dst), 0.7);
    }

    #[test]
    fn courbe_vers_le_sdr() {
        assert_eq!(sdr_curve(0.5), 0.5);
        assert_eq!(sdr_curve(SDR_KNEE), SDR_KNEE);
        assert!((sdr_curve(SDR_RANGE) - 1.0).abs() < 1e-9);
        assert_eq!(sdr_curve(50.0), 1.0);
        // Même pente au genou (pas de cassure), toujours croissante.
        let eps = 1e-6;
        assert!(((sdr_curve(SDR_KNEE + eps) - SDR_KNEE) / eps - 1.0).abs() < 1e-3);
        let mut last = 0.0;
        for i in 0..=4000 {
            let v = sdr_curve(i as f64 / 1000.0);
            assert!(v >= last);
            last = v;
        }
    }

    fn apply(m: &[[f32; 4]; 3], y: f32, u: f32, v: f32) -> [f32; 3] {
        let p = [y, u, v, 1.0];
        m.map(|r| r.iter().zip(p).map(|(a, b)| a * b).sum())
    }

    #[test]
    fn matrices_8_et_10_bits() {
        // 8 bits, plage limitée : 16 → noir, 235 → blanc.
        let m = yuv_matrix(1, false, 8, 1.0 / 255.0);
        let black = apply(&m, 16.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0);
        let white = apply(&m, 235.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0);
        assert!(black.iter().all(|c| c.abs() < 1e-5) && white.iter().all(|c| (c - 1.0).abs() < 1e-5));
        // P010 (code dans les 10 bits du haut d'un mot de 16 bits), BT.2020 : 64 → noir, 940 → blanc.
        let unit = 64.0 / 65535.0;
        let m = yuv_matrix(9, false, 10, unit);
        let black = apply(&m, 64.0 * unit, 512.0 * unit, 512.0 * unit);
        let white = apply(&m, 940.0 * unit, 512.0 * unit, 512.0 * unit);
        assert!(black.iter().all(|c| c.abs() < 1e-4), "{black:?}");
        assert!(white.iter().all(|c| (c - 1.0).abs() < 1e-4), "{white:?}");
        // Cr au haut de sa plage sur le noir : R' = 1,4746 × 0,5, B' = 0 (coefficients BT.2020).
        let red = apply(&m, 64.0 * unit, 512.0 * unit, 960.0 * unit);
        assert!((red[0] - 0.7373).abs() < 1e-3 && red[2].abs() < 1e-4, "{red:?}");
        // Par le processeur : même résultat avec des codes sur 16 bits (bas).
        let sw = yuv_matrix(9, false, 10, 1.0 / 65535.0);
        let w2 = apply(&sw, 940.0 / 65535.0, 512.0 / 65535.0, 512.0 / 65535.0);
        assert!(w2.iter().all(|c| (c - 1.0).abs() < 1e-4));
        // L'ancienne matrice 8 bits pleine plage.
        let f = yuv_matrix(1, true, 8, 1.0 / 255.0);
        assert!((f[0][0] - 1.0).abs() < 1e-6 && (f[0][3] + 1.5748 * 128.0 / 255.0).abs() < 1e-5);
    }

    #[test]
    fn metadonnees_ramenees_a_l_ecran() {
        let screen = Some(HdrLuminance { max: 600.0, full_frame: 400.0, min: 0.0, sdr_white: 240.0 });
        let t = tone(Output::Pq, 1000.0, screen);
        let m = hdr10_for(1000.0, 0.005, Some(400), &t, screen);
        assert_eq!((m.max_mastering, m.max_cll, m.max_fall), (600.0, 600, 400));
        let t = tone(Output::Pq, 500.0, screen);
        let m = hdr10_for(500.0, 0.005, None, &t, screen);
        assert_eq!((m.max_mastering, m.max_cll, m.max_fall), (500.0, 500, 500));
    }
}
