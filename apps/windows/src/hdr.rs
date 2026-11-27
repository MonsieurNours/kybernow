//! HDR de l'app Windows (docs/hdr.md, « Deuxième temps »).
//!
//! - **L'écran** : un écran de ce PC est HDR quand Windows l'a passé en HDR
//!   (« Utiliser le HDR » dans Paramètres › Affichage) : `IDXGIOutput6::
//!   GetDesc1` le dit en espace `RGB_FULL_G2084_NONE_P2020`. Ses luminosités
//!   viennent de la même description (l'EDID de l'écran, ou le profil posé
//!   par l'outil d'étalonnage HDR de Windows), le blanc du bureau du curseur
//!   « Luminosité du contenu SDR » (`DISPLAYCONFIG_SDR_WHITE_LEVEL`), lues par
//!   `HdrLuminance::from_display` du cœur. Une fabrique DXGI neuve à chaque
//!   relevé : l'ancienne garde l'état d'avant.
//! - **Le rendu** (`render.rs`) : une image PQ / BT.2020 (étiquetée SMPTE
//!   2084 par la Station ou la console) part telle quelle dans une chaîne
//!   d'échange 10 bits en espace PQ BT.2020 sur un écran HDR, ramenée à ce
//!   que l'écran affiche (EETF de BT.2390, relèvement du noir compris) quand
//!   le flux le dépasse ; sur un écran SDR (fenêtre glissée, HDR coupé, le
//!   temps que l'image repasse en SDR), la courbe de la Station la ramène au
//!   SDR. Pas de courbe de plus sinon (le jeu a fait la sienne d'après la
//!   luminosité de cet écran).
//!
//! Les règles et les courbes sont celles du cœur (`kybernow_core::rules::hdr`,
//! les mêmes pour toutes les apps et la Station, WINAPP-49) ; le shader de
//! `render.rs` les recopie, et un essai croisé (ici, sur le Mac) le vérifie.

use kybernow_core::profile::HdrLuminance;
// `frame_is_pq` et `yuv_matrix` servent au rendu (Windows seulement).
#[cfg_attr(not(windows), allow(unused_imports))]
pub use kybernow_core::rules::hdr::{Hdr10, Output, content_peak, frame_is_pq, nits_to_pq, yuv_matrix};
use kybernow_core::rules::hdr as rules;

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

/// Les métadonnées HDR10 d'un flux : MaxCLL et MaxFALL, luminosités maximale
/// et minimale de l'écran de matriçage (nits). Celles d'une image HEVC ou AV1
/// (SEI, `decoder::frame_hdr`) ou du paquet de configuration de PyroWave
/// version 2 ([`pyrowave_hdr`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameHdr {
    pub max_cll: Option<u32>,
    pub max_fall: Option<u32>,
    pub mastering_max: Option<f64>,
    pub mastering_min: Option<f64>,
}

impl FrameHdr {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// La version du paquet de configuration de PyroWave que lit l'app (KYB-29) :
/// la Station envoie la version 2, avec les métadonnées HDR10 de la capture,
/// à une app qui le demande (`kyclient::set_pyrowave_header`, au lancement).
pub const PYROWAVE_HEADER: u8 = 2;

/// Les métadonnées HDR10 du paquet de configuration de PyroWave version 2
/// (`KNPW`, docs/kyber-changes.md, « En-tête KNPW version 2 ») : drapeaux à
/// l'octet 16 (bit 0 : écran de matriçage, bit 1 : niveaux de lumière),
/// luminances maximale et minimale (u32 aux octets 36 et 40, en 0,0001 nit),
/// MaxCLL et MaxFALL (u16 aux octets 44 et 46). Version 1, version 2 de moins
/// de 48 octets, ou champ sans son drapeau : rien (crête par défaut). Les
/// octets au-delà sont ignorés (une version plus récente garde ceux-ci). La
/// tête du paquet (taille, chroma, couleur) est lue par le cœur
/// (`pyrowave::Config::parse`), qui n'en lit pas les métadonnées.
pub fn pyrowave_hdr(d: &[u8]) -> FrameHdr {
    let mut out = FrameHdr::default();
    if d.len() < 48 || !d.starts_with(b"KNPW") || d[4] < 2 {
        return out;
    }
    let u16le = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let flags = d[16];
    if flags & 1 != 0 {
        out.mastering_max = Some(f64::from(u32le(36)) * 0.0001).filter(|v| *v > 0.0);
        out.mastering_min = Some(f64::from(u32le(40)) * 0.0001);
    }
    if flags & 2 != 0 {
        out.max_cll = Some(u32::from(u16le(44))).filter(|v| *v > 0);
        out.max_fall = Some(u32::from(u16le(46))).filter(|v| *v > 0);
    }
    out
}

/// Les constantes de la sortie pour le shader : celles du cœur
/// (`rules::hdr::tone`), et le blanc du trait du bord haut, propre à l'app.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    pub rules: rules::Tone,
    /// Le blanc du trait du bord haut, en valeur de sortie (le blanc du
    /// bureau en PQ sur un écran HDR, 1 sinon).
    pub hint_level: f32,
}

/// Les constantes pour `output`, d'après la crête du flux (`content_peak`)
/// et les luminosités de l'écran (`None` : par défaut).
pub fn tone(output: Output, peak: f64, screen: Option<HdrLuminance>) -> Tone {
    let lum = HdrLuminance::of(screen);
    let hint_level = if output == Output::Pq { nits_to_pq(lum.sdr_white) as f32 } else { 1.0 };
    Tone { rules: rules::tone(output, peak, lum.max, lum.min, lum.sdr_white), hint_level }
}

/// Les métadonnées à donner à l'écran pour cette image : celles du flux,
/// ramenées à l'écran quand la courbe s'applique (`rules::hdr::hdr10_for`).
pub fn hdr10_for(peak: f64, min: f64, max_fall: Option<u32>, t: &Tone, screen: Option<HdrLuminance>) -> Hdr10 {
    rules::hdr10_for(peak, min, max_fall, &t.rules, HdrLuminance::of(screen).max)
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
            let luminance = hdr.then(|| HdrLuminance::from_display(f64::from(d.MaxLuminance), f64::from(d.MaxFullFrameLuminance), f64::from(d.MinLuminance), white));
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
    fn luminosites_de_l_ecran() {
        // `HdrLuminance::from_display` du cœur, sur ce que DXGI donne (f32).
        let l = HdrLuminance::from_display(1499.0, 799.0, f64::from(0.01f32), Some(240.0));
        assert_eq!((l.max, l.full_frame, l.sdr_white), (1499.0, 799.0, 240.0));
        // Blanc SDR de Windows (1000 = 80 nits).
        assert_eq!(sdr_white_nits(2500), Some(200.0));
        assert_eq!(sdr_white_nits(0), None);
    }

    #[test]
    fn trait_du_bord_haut_au_blanc_du_bureau() {
        let screen = Some(HdrLuminance { max: 600.0, full_frame: 400.0, min: 0.05, sdr_white: 240.0 });
        let t = tone(Output::Pq, 1000.0, screen);
        assert!(t.rules.tone && t.rules.out_mode == 1);
        assert!((f64::from(t.hint_level) - nits_to_pq(240.0)).abs() < 1e-6);
        // Le noir de l'écran va au relèvement du noir de l'EETF.
        assert!((t.rules.dst_lo_pq - nits_to_pq(0.05)).abs() < 1e-12);
        assert_eq!(tone(Output::PqToSdr, 4000.0, None).hint_level, 1.0);
        let m = hdr10_for(1000.0, 0.005, Some(400), &t, screen);
        assert_eq!((m.max_mastering, m.max_cll, m.max_fall), (600.0, 600, 400));
    }

    /// Un paquet `KNPW` version 2 tel que l'écrit la Station
    /// (`pyrowave_common.h`, docs/kyber-changes.md).
    fn knpw_v2(flags: u8, max: u32, min: u32, cll: u16, fall: u16) -> Vec<u8> {
        let c = kybernow_core::pyrowave::Config { chroma444: false, hdr: true, full_range: false, width: 3840, height: 2160 };
        let mut d = c.bytes().to_vec();
        d[4] = 2;
        d.resize(48, 0);
        d[16] = flags;
        // Primaires BT.2020 et blanc D65 (unités de 0,00002), lus par personne ici.
        for (i, v) in [35400u16, 14600, 8500, 39850, 6550, 2300, 15635, 16450].iter().enumerate() {
            d[20 + 2 * i..22 + 2 * i].copy_from_slice(&v.to_le_bytes());
        }
        d[36..40].copy_from_slice(&max.to_le_bytes());
        d[40..44].copy_from_slice(&min.to_le_bytes());
        d[44..46].copy_from_slice(&cll.to_le_bytes());
        d[46..48].copy_from_slice(&fall.to_le_bytes());
        d
    }

    #[test]
    fn knpw_version_2_metadonnees_hdr() {
        let d = knpw_v2(3, 10_000_000, 50, 1600, 400);
        // La tête : le cœur la lit en version 2 comme en version 1.
        let c = kybernow_core::pyrowave::Config::parse(&d).expect("paquet v2 lu par le cœur");
        assert!(c.hdr && (c.width, c.height) == (3840, 2160));
        assert!(kybernow_core::pyrowave::Config::is_config(&d));
        let m = pyrowave_hdr(&d);
        assert_eq!(m, FrameHdr { max_cll: Some(1600), max_fall: Some(400), mastering_max: Some(1000.0), mastering_min: Some(0.005) });
        assert_eq!(content_peak(m.max_cll, m.mastering_max), 1600.0);
        // Écran de matriçage seul : sa crête fait celle du flux.
        let m = pyrowave_hdr(&knpw_v2(1, 6_000_000, 1, 1600, 400));
        assert_eq!((m.max_cll, m.mastering_max), (None, Some(600.0)));
        assert_eq!(content_peak(m.max_cll, m.mastering_max), 600.0);
        // Drapeaux à zéro (SDR, ou capture sans métadonnées) : rien.
        assert!(pyrowave_hdr(&knpw_v2(0, 10_000_000, 50, 1600, 400)).is_empty());
        // Octets en plus (version suivante) : ignorés.
        let mut longer = knpw_v2(2, 0, 0, 800, 0);
        longer.extend_from_slice(&[9; 16]);
        assert_eq!(pyrowave_hdr(&longer), FrameHdr { max_cll: Some(800), ..FrameHdr::default() });
    }

    /// Le shader (`render.hlsl`), tel qu'écrit.
    const SHADER: &str = include_str!("render.hlsl");

    /// Les lignes du shader, sans les espaces de début et de fin.
    fn shader_has(line: &str) -> bool {
        SHADER.lines().any(|l| l.trim() == line)
    }

    /// `eetf` et `sdr_curve` du shader, ligne pour ligne en f32 (la précision
    /// du GPU) ; `shader_comme_le_coeur` vérifie que le shader a ces lignes.
    fn shader_eetf(e: f32, src_pq: f32, dst_lo_pq: f32, dst_pq: f32) -> f32 {
        let e1 = (e / src_pq).clamp(0.0, 1.0);
        let maxl = (dst_pq / src_pq).min(1.0);
        let minl = (dst_lo_pq / src_pq).clamp(0.0, 1.0);
        let ks = (1.5 * maxl - 0.5).max(0.0);
        let mut e2 = e1;
        if e1 >= ks && ks < 1.0 {
            let t = (e1 - ks) / (1.0 - ks);
            let t2 = t * t;
            let t3 = t2 * t;
            e2 = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * maxl;
        }
        let b = 1.0 - e2;
        (e2 + minl * b * b * b * b) * src_pq
    }

    fn shader_sdr_curve(m: f32) -> f32 {
        if m <= 0.75 {
            return m;
        }
        let x = m - 0.75;
        (0.75 + x / (1.0 + 3.692_307_7 * x)).min(1.0)
    }

    /// Les nombres d'une ligne du shader (constantes, matrice).
    fn numbers(line: &str) -> Vec<f64> {
        line.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|t| t.parse().ok()).collect()
    }

    #[test]
    fn shader_comme_le_coeur() {
        // Les lignes qui font les courbes (WINAPP-49) : EETF avec relèvement du
        // noir, courbe de la Station vers le SDR (pente `SDR_SLOPE`).
        for line in [
            "float e1 = saturate(e / src_pq);",
            "float maxl = min(dst_pq / src_pq, 1.0);",
            "float minl = saturate(dst_lo_pq / src_pq);",
            "float ks = max(1.5 * maxl - 0.5, 0.0);",
            "if (e1 >= ks && ks < 1.0) {",
            "float t = (e1 - ks) / (1.0 - ks);",
            "e2 = (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * maxl;",
            "float b = 1.0 - e2;",
            "return (e2 + minl * b * b * b * b) * src_pq;",
            "if (m <= 0.75) return m;",
            "float x = m - 0.75;",
            "return min(0.75 + x / (1.0 + 3.6923077 * x), 1.0);",
        ] {
            assert!(shader_has(line), "render.hlsl n'a plus : {line}");
        }
        assert!((3.692_307_7 - rules::SDR_SLOPE).abs() < 1e-6 && rules::SDR_KNEE == 0.75);
        // Constantes PQ et matrice BT.2020 → BT.709 : celles du cœur.
        for (name, v) in [("PQ_M1", rules::PQ_M1), ("PQ_M2", rules::PQ_M2), ("PQ_C1", rules::PQ_C1), ("PQ_C2", rules::PQ_C2), ("PQ_C3", rules::PQ_C3)] {
            let line = SHADER.lines().find(|l| l.contains(&format!("float {name} ="))).unwrap_or_else(|| panic!("{name} absent"));
            assert!((numbers(line.split('=').nth(1).unwrap())[0] - v).abs() < 1e-9, "{name}");
        }
        let m = SHADER.split("BT2020_TO_709 = {").nth(1).and_then(|s| s.split("};").next()).expect("matrice");
        let m: Vec<f64> = m.lines().flat_map(numbers).collect();
        assert_eq!(m.len(), 9);
        for (i, row) in rules::BT2020_TO_709.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                assert!((m[i * 3 + j] - v).abs() < 1e-9, "matrice [{i}][{j}]");
            }
        }
        // Les valeurs, sur une grille, à 1e-4 près de celles du cœur.
        for (src, lo, hi) in [(4000.0, 0.05, 600.0), (1000.0, 0.0, 400.0), (10_000.0, 0.5, 1000.0), (1600.0, 0.005, 1499.0)] {
            let (s, l, d) = (nits_to_pq(src), nits_to_pq(lo), nits_to_pq(hi));
            for i in 0..=200 {
                let e = i as f64 / 200.0;
                let gpu = shader_eetf(e as f32, s as f32, l as f32, d as f32);
                let core = rules::eetf(e, s, l, d);
                assert!((f64::from(gpu) - core).abs() < 1e-4, "eetf({e}) {src}/{lo}/{hi} : {gpu} contre {core}");
            }
        }
        for i in 0..=800 {
            let m = i as f64 / 100.0;
            assert!((f64::from(shader_sdr_curve(m as f32)) - rules::sdr_curve(m)).abs() < 1e-4, "sdr_curve({m})");
        }
    }

    #[test]
    fn knpw_version_1_toujours_lu() {
        let c = kybernow_core::pyrowave::Config { chroma444: true, hdr: true, full_range: true, width: 2560, height: 1440 };
        let d = c.bytes();
        assert_eq!(kybernow_core::pyrowave::Config::parse(&d), Some(c));
        // Pas de métadonnées en version 1 : crête par défaut.
        assert!(pyrowave_hdr(&d).is_empty());
        // Version 2 tronquée à 16 octets : la tête seule.
        let mut short = d;
        short[4] = 2;
        assert!(kybernow_core::pyrowave::Config::parse(&short).is_some());
        assert!(pyrowave_hdr(&short).is_empty());
    }
}
