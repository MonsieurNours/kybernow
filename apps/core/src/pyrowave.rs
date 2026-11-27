//! PyroWave (docs/pyrowave.md, « Définition ramenée ») : le débit dont une
//! image PyroWave a besoin, et la définition (puis la fréquence) ramenée pour
//! tenir dans le plafond de la session. Un profil demande le débit de la
//! netteté pleine ([`needed_mbps`], 1,5 bit par pixel) ; sous un plafond plus
//! bas, la définition de l'écran reste jusqu'à [`MIN_BITS_PER_PIXEL`] (0,5) :
//! mesuré, elle fait mieux que la définition ramenée puis agrandie. Au-dessous
//! seulement, la définition baisse, puis la fréquence (docs/link-ui.md,
//! « PyroWave »), et l'app le dit.
//!
//! Tout ici est du calcul pur ; la session ([`crate::session`]) s'en sert
//! pour la définition demandée à la Station (`prepare`) et pour le débit.

use serde::{Deserialize, Serialize};

use crate::console::Mode;

/// Bits par pixel d'une image PyroWave en 4:2:0 (SDR 8 bits ou HDR10) :
/// le haut de ce que donne PyroWave à netteté pleine (~1,2 à 1,5 bit par
/// pixel), soit ≈ 187 Mbit/s en 1080p60 et ≈ 746 Mbit/s en 4K60.
pub const BITS_PER_PIXEL: f64 = 1.5;

/// Bits par pixel en 4:4:4 (SDR seulement) : moitié plus qu'en 4:2:0, comme
/// les débits de base ([`crate::profile::PYROWAVE_MBPS_444`]) ; deux fois plus
/// d'échantillons de couleur, mais la couleur pèse moins que la luminance.
pub const BITS_PER_PIXEL_444: f64 = 2.25;

/// Bits par pixel au-dessous desquels la définition est ramenée (4:2:0) :
/// jusque-là, PyroWave garde la définition de l'écran avec moins de bits par
/// pixel. Mesuré le 26/11/2026 (encodeur Metal et décodeur de l'app, trois
/// images 4K réelles, VMAF, SSIM et PSNR) : à débit égal, la définition
/// gardée fait mieux que la définition ramenée puis agrandie à chaque palier,
/// de 1,5 jusqu'à 0,5 bit par pixel (4K60 : VMAF 93,5 à 250 Mbit/s contre
/// 92,3 en 1248p, 95,2 à 500 contre 94,2 en 1764p). Docs : pyrowave.md,
/// « Définition ramenée ».
pub const MIN_BITS_PER_PIXEL: f64 = 0.5;

/// Le même plancher en 4:4:4 (moitié plus, comme [`BITS_PER_PIXEL_444`]).
pub const MIN_BITS_PER_PIXEL_444: f64 = 0.75;

/// Définition la plus basse à laquelle la session descend avant de toucher
/// à la fréquence (hauteur de l'écran principal ; celle de l'écran si elle
/// est plus petite).
pub const MIN_HEIGHT: u32 = 720;

/// Fréquence la plus basse ; au-dessous, le plafond n'est plus tenu et
/// l'image perd de la netteté.
pub const MIN_REFRESH: u32 = 30;

/// Fréquences essayées, dans l'ordre, une fois la définition à
/// [`MIN_HEIGHT`] (celles sous la fréquence de l'écran seulement).
const REFRESH_STEPS: [u32; 5] = [120, 100, 60, 50, 30];

/// Bits par pixel de PyroWave pour ce sous-échantillonnage.
pub fn bits_per_pixel(yuv444: bool) -> f64 {
    if yuv444 { BITS_PER_PIXEL_444 } else { BITS_PER_PIXEL }
}

/// Débit au-dessous duquel ces écrans ne tiennent plus à leur définition
/// ([`MIN_BITS_PER_PIXEL`]), en bit/s.
fn floor_bits_per_second(modes: &[Mode], yuv444: bool) -> f64 {
    let bpp = if yuv444 { MIN_BITS_PER_PIXEL_444 } else { MIN_BITS_PER_PIXEL };
    modes.iter().map(|m| f64::from(m.width) * f64::from(m.height) * f64::from(m.refresh.max(1))).sum::<f64>() * bpp
}

/// Le plancher en Mbit/s ([`MIN_BITS_PER_PIXEL`]) : au-dessous, la définition baisse.
#[cfg(test)]
pub fn floor_mbps(modes: &[Mode], yuv444: bool) -> u32 {
    (floor_bits_per_second(modes, yuv444) / 1e6).ceil() as u32
}

fn bits_per_second(modes: &[Mode], yuv444: bool) -> f64 {
    modes.iter().map(|m| f64::from(m.width) * f64::from(m.height) * f64::from(m.refresh.max(1))).sum::<f64>() * bits_per_pixel(yuv444)
}

/// Débit nécessaire à PyroWave pour ces écrans (un seul, ou ceux d'une
/// session à plusieurs écrans), en Mbit/s arrondis au-dessus.
pub fn needed_mbps(modes: &[Mode], yuv444: bool) -> u32 {
    (bits_per_second(modes, yuv444) / 1e6).ceil() as u32
}

/// Le plafond de la session, en Mbit/s : le plus petit de ceux qui sont
/// connus (`None` ou 0 : inconnu). `station` : le « Débit maximal » annoncé
/// par l'ordinateur (`pyrowave.max_mbps` de `GET /api/stream/info`) ;
/// `measured` : le débit mesuré ou estimé par l'app (vitesse du lien) ;
/// `app` : le débit réglé à la main dans l'app (format sans profil).
/// `None` : aucun plafond, la définition reste.
pub fn ceiling(station: Option<u32>, measured: Option<u32>, app: Option<u32>) -> Option<u32> {
    [station, measured, app].into_iter().flatten().filter(|v| *v > 0).min()
}

/// La définition ramenée, telle que les apps la montrent (bandeau et
/// Statistiques) : celle de l'écran principal, sa fréquence, et le plafond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lowered {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// Le plafond tenu, en Mbit/s.
    pub mbps: u32,
}

impl Lowered {
    /// « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. »
    pub fn text(&self, lang: crate::i18n::Lang) -> String {
        let (w, h, n) = (self.width.to_string(), self.height.to_string(), self.mbps.to_string());
        crate::i18n::tr(lang, "pyrowave.lowered", &[("width", &w), ("height", &h), ("mbps", &n)])
    }
}

// MARK: - Paquet de configuration (WINAPP-50)

/// Le paquet de configuration d'un flux PyroWave (`KNPW`, docs/pyrowave.md),
/// lu une fois pour toutes les apps (avant : Windows, Linux et Android, avec
/// trois contrôles différents) : `"KNPW"` | version | chroma (0 : 4:2:0,
/// 1 : 4:4:4) | couleur (0 : SDR BT.709, 1 : HDR10 BT.2020 PQ) | plage
/// (0 : pleine, 1 : réduite) | largeur u32 LE | hauteur u32 LE. Une version
/// plus récente garde ces 16 octets en tête (la version 2 ajoute les
/// métadonnées HDR à la fin) : les octets en trop sont ignorés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub chroma444: bool,
    pub hdr: bool,
    pub full_range: bool,
    pub width: u32,
    pub height: u32,
}

impl Config {
    /// Taille de la tête du paquet.
    pub const SIZE: usize = 16;
    /// Côté le plus grand qu'un décodeur accepte.
    pub const MAX_SIDE: u32 = 16384;

    /// Lit un paquet de configuration ; `None` s'il n'en est pas un, ou s'il
    /// décrit une image impossible (côté nul ou trop grand, 4:2:0 de taille
    /// impaire).
    pub fn parse(d: &[u8]) -> Option<Self> {
        if !Self::is_config(d) || d[4] < 1 || d[5] > 1 || d[6] > 1 || d[7] > 1 {
            return None;
        }
        let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let c = Self { chroma444: d[5] == 1, hdr: d[6] == 1, full_range: d[7] == 0, width: u32le(8), height: u32le(12) };
        c.valid().then_some(c)
    }

    /// Un paquet de configuration, pas une image : il commence par `KNPW`
    /// (une image commence par son en-tête de séquence, au bit de poids fort
    /// levé) et a au moins [`Self::SIZE`] octets. Il peut arriver dans le
    /// flux des images.
    pub fn is_config(d: &[u8]) -> bool {
        d.len() >= Self::SIZE && d.starts_with(b"KNPW")
    }

    /// Sans paquet de configuration : la définition lue dans l'en-tête de
    /// séquence d'une image (SDR, plage pleine).
    pub fn from_sequence_header(d: &[u8]) -> Option<Self> {
        if d.len() < 8 {
            return None;
        }
        let w0 = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
        let w1 = u32::from_le_bytes([d[4], d[5], d[6], d[7]]);
        if w0 >> 31 != 1 || (w1 >> 24) & 3 != 0 {
            return None;
        }
        let c = Self { chroma444: (w1 >> 26) & 1 == 1, hdr: false, full_range: true, width: (w0 & 0x3FFF) + 1, height: ((w0 >> 14) & 0x3FFF) + 1 };
        c.valid().then_some(c)
    }

    /// Faute de mieux (aucun paquet, en-tête illisible) : 4:2:0 SDR plage
    /// pleine, à la taille donnée ramenée au pair.
    pub fn fallback(width: u32, height: u32) -> Self {
        Self { chroma444: false, hdr: false, full_range: true, width: (width & !1).max(2), height: (height & !1).max(2) }
    }

    fn valid(&self) -> bool {
        (1..=Self::MAX_SIDE).contains(&self.width) && (1..=Self::MAX_SIDE).contains(&self.height) && (self.chroma444 || (self.width % 2 == 0 && self.height % 2 == 0))
    }

    /// La taille des plans de chrominance.
    pub fn chroma_size(&self) -> (u32, u32) {
        if self.chroma444 { (self.width, self.height) } else { (self.width / 2, self.height / 2) }
    }

    /// Pour le journal : « 3840×2160 4:2:0 SDR pleine ».
    pub fn label(&self) -> String {
        format!(
            "{}×{} {} {} {}",
            self.width,
            self.height,
            if self.chroma444 { "4:4:4" } else { "4:2:0" },
            if self.hdr { "HDR10" } else { "SDR" },
            if self.full_range { "pleine" } else { "réduite" }
        )
    }

    /// Le paquet (version 1), pour les essais et l'encodeur de la Station.
    pub fn bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[..4].copy_from_slice(b"KNPW");
        b[4] = 1;
        b[5] = self.chroma444 as u8;
        b[6] = self.hdr as u8;
        b[7] = (!self.full_range) as u8;
        b[8..12].copy_from_slice(&self.width.to_le_bytes());
        b[12..16].copy_from_slice(&self.height.to_le_bytes());
        b
    }
}

/// L'avis bref quand PyroWave n'a pas pu démarrer sur l'appareil (docs/link-ui.md,
/// « PyroWave ») : « Image en {format} : PyroWave n'a pas pu démarrer sur
/// cet appareil. », `codec` le format de repli.
pub fn fallback_text(codec: crate::profile::Codec, lang: crate::i18n::Lang) -> String {
    crate::i18n::tr(lang, "pyrowave.fallback_notice", &[("format", codec.label())])
}

fn even(v: f64) -> u32 {
    (((v / 2.0).round() as u32) * 2).max(2)
}

/// Les écrans à l'échelle `k` (largeur et hauteur paires), chacun à
/// `refresh` au plus.
fn scaled(modes: &[Mode], k: f64, refresh: u32) -> Vec<Mode> {
    modes
        .iter()
        .map(|m| {
            if k >= 1.0 {
                Mode { refresh: m.refresh.min(refresh), ..*m }
            } else {
                Mode {
                    width: even(f64::from(m.width) * k).min(m.width),
                    height: even(f64::from(m.height) * k).min(m.height),
                    refresh: m.refresh.min(refresh),
                }
            }
        })
        .collect()
}

/// Les écrans ramenés pour tenir dans `ceiling_mbps` au plancher de
/// [`MIN_BITS_PER_PIXEL`] ; `None` s'ils y tiennent déjà (ou s'il n'y en a
/// pas). `primary` : l'index de l'écran
/// principal, qui donne l'échelle (les autres suivent la même).
///
/// D'abord la plus grande définition paire qui tient (proportions gardées),
/// à la fréquence de l'écran, jusqu'à [`MIN_HEIGHT`] ; puis, à défaut, une
/// fréquence plus basse (120, 100, 60, 50, 30 Hz), avec de nouveau la plus
/// grande définition qui tient. Rien ne tient même à [`MIN_HEIGHT`] et
/// [`MIN_REFRESH`] : ce plancher (le débit sera alors plafonné).
pub fn fit(modes: &[Mode], primary: usize, yuv444: bool, ceiling_mbps: u32) -> Option<Vec<Mode>> {
    let reference = *modes.get(primary).or(modes.first())?;
    if reference.width == 0 || reference.height == 0 {
        return None;
    }
    let budget = f64::from(ceiling_mbps) * 1e6;
    // La définition reste tant que le plafond donne au moins le plancher de
    // bits par pixel ([`MIN_BITS_PER_PIXEL`]) ; le profil demande, lui, le
    // débit de la netteté pleine ([`needed_mbps`]) quand il le peut.
    let fits = |ms: &[Mode]| floor_bits_per_second(ms, yuv444) <= budget;
    if fits(modes) {
        return None;
    }
    let native = reference.refresh.max(1);
    let min_h = MIN_HEIGHT.min(reference.height);
    let mut rates = vec![native];
    rates.extend(REFRESH_STEPS.iter().copied().filter(|r| *r < native && *r >= MIN_REFRESH));
    for &refresh in &rates {
        let full = scaled(modes, 1.0, refresh);
        if fits(&full) {
            return Some(full);
        }
        // Échelle qui tient à peu près (le débit suit le carré de l'échelle),
        // puis la hauteur paire au-dessous, deux pixels à la fois.
        let guess = (budget / floor_bits_per_second(&full, yuv444)).sqrt();
        let mut h = (even(f64::from(reference.height) * guess) + 4).min(reference.height & !1);
        while h >= min_h {
            let candidate = scaled(modes, f64::from(h) / f64::from(reference.height), refresh);
            if fits(&candidate) {
                return Some(candidate);
            }
            h -= 2;
        }
    }
    let last = *rates.last().unwrap_or(&native);
    Some(scaled(modes, f64::from(min_h) / f64::from(reference.height), last))
}

/// [`fit`], avec ce que les apps montrent ([`Lowered`], de l'écran
/// principal).
pub fn lowered(modes: &[Mode], primary: usize, yuv444: bool, ceiling_mbps: u32) -> Option<(Vec<Mode>, Lowered)> {
    let fitted = fit(modes, primary, yuv444, ceiling_mbps)?;
    let p = fitted.get(primary).or(fitted.first())?;
    Some((fitted.clone(), Lowered { width: p.width, height: p.height, refresh: p.refresh, mbps: ceiling_mbps }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WINAPP-50 : le paquet de configuration lu d'une seule façon.
    #[test]
    fn paquet_de_configuration() {
        let c = Config { chroma444: false, hdr: true, full_range: false, width: 3840, height: 2160 };
        let b = c.bytes();
        assert_eq!(Config::parse(&b), Some(c));
        assert!(Config::is_config(&b));
        assert_eq!(c.chroma_size(), (1920, 1080));
        assert_eq!(c.label(), "3840×2160 4:2:0 HDR10 réduite");
        // Version 2 : métadonnées HDR à la fin, ignorées ici.
        let mut v2 = b.to_vec();
        v2[4] = 2;
        v2.extend_from_slice(&[0u8; 28]);
        assert!(Config::is_config(&v2));
        assert_eq!(Config::parse(&v2), Some(c));
        // Refusés : trop court, autre en-tête, version 0, octet inconnu, 4:2:0 impair, trop grand.
        assert_eq!(Config::parse(&b[..15]), None);
        let bad = |i: usize, v: u8| {
            let mut x = b;
            x[i] = v;
            Config::parse(&x)
        };
        assert_eq!(bad(0, b'X'), None);
        assert_eq!(bad(4, 0), None);
        assert_eq!((bad(5, 2), bad(6, 2), bad(7, 2)), (None, None, None));
        assert_eq!(Config::parse(&Config { width: 1921, ..c }.bytes()), None);
        assert!(Config::parse(&Config { chroma444: true, width: 1921, ..c }.bytes()).is_some(), "4:4:4 impair possible");
        assert_eq!(Config::parse(&Config { width: 16385, ..c }.bytes()), None);
        // Une image n'est pas un paquet de configuration.
        let header = [(1919u32 | (1079 << 14) | (1 << 31)).to_le_bytes(), 0u32.to_le_bytes()].concat();
        assert!(!Config::is_config(&header));
        assert_eq!(Config::from_sequence_header(&header), Some(Config { chroma444: false, hdr: false, full_range: true, width: 1920, height: 1080 }));
        assert_eq!(Config::fallback(1921, 1081), Config { chroma444: false, hdr: false, full_range: true, width: 1920, height: 1080 });
    }

    fn m(width: u32, height: u32, refresh: u32) -> Mode {
        Mode { width, height, refresh }
    }

    #[test]
    fn needed_bitrate() {
        assert_eq!(needed_mbps(&[m(1920, 1080, 60)], false), 187);
        assert_eq!(needed_mbps(&[m(3840, 2160, 60)], false), 747);
        assert_eq!(needed_mbps(&[m(3840, 2160, 60)], true), 1120);
        assert_eq!(needed_mbps(&[m(1920, 1080, 60), m(1920, 1080, 60)], false), 374);
    }

    #[test]
    fn the_smallest_known_ceiling() {
        assert_eq!(ceiling(None, None, None), None);
        assert_eq!(ceiling(Some(800), None, None), Some(800));
        assert_eq!(ceiling(Some(800), Some(450), Some(500)), Some(450));
        assert_eq!(ceiling(Some(0), None, Some(500)), Some(500));
    }

    #[test]
    fn fits_already() {
        assert_eq!(fit(&[m(3840, 2160, 60)], 0, false, 800), None);
        assert_eq!(fit(&[m(1920, 1080, 60)], 0, true, 300), None);
        assert_eq!(fit(&[], 0, false, 10), None);
    }

    #[test]
    fn largest_even_definition_that_fits() {
        // 4K60 4:2:0 : la définition reste jusqu'au plancher (≈ 249 Mbit/s,
        // 0,5 bit par pixel), même sous le débit de la netteté pleine (747).
        assert_eq!(fit(&[m(3840, 2160, 60)], 0, false, 500), None);
        assert_eq!(fit(&[m(3840, 2160, 60)], 0, false, 250), None);
        // Dans 160 Mbit/s : ≈ 0,8 de l'échelle.
        let (modes, l) = lowered(&[m(3840, 2160, 60)], 0, false, 160).unwrap();
        let r = modes[0];
        assert_eq!((r.width % 2, r.height % 2, r.refresh), (0, 0, 60));
        assert!(floor_mbps(&modes, false) <= 160, "{r:?}");
        // La hauteur paire au-dessus ne tient plus.
        let above = scaled(&[m(3840, 2160, 60)], f64::from(r.height + 2) / 2160.0, 60);
        assert!(floor_mbps(&above, false) > 160, "{above:?}");
        assert_eq!(l, Lowered { width: r.width, height: r.height, refresh: 60, mbps: 160 });
        // Proportions gardées.
        assert!((f64::from(r.width) / f64::from(r.height) - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn four_four_four_costs_more() {
        // 300 Mbit/s : 4K60 tient en 4:2:0 (plancher 249), pas en 4:4:4 (373).
        let a = fit(&[m(3840, 2160, 60)], 0, false, 300);
        let b = fit(&[m(3840, 2160, 60)], 0, true, 300).unwrap();
        assert_eq!(a, None);
        assert!(b[0].height < 2160 && floor_mbps(&b, true) <= 300);
    }

    #[test]
    fn then_the_refresh_rate() {
        // 4K120 dans 50 Mbit/s : 720p120 demande 56 Mbit/s au plancher, donc
        // 100 Hz, avec la plus grande définition qui tient à 100 Hz.
        let modes = fit(&[m(3840, 2160, 120)], 0, false, 50).unwrap();
        assert_eq!(modes[0].refresh, 100);
        assert!(modes[0].height >= MIN_HEIGHT && floor_mbps(&modes, false) <= 50);
        // Rien ne tient : 720p à 30 Hz.
        let floor = fit(&[m(3840, 2160, 120)], 0, false, 10).unwrap();
        assert_eq!(floor[0], m(1280, 720, 30));
        // Un écran plus petit que 720p descend en fréquence tout de suite.
        let small = fit(&[m(1280, 600, 60)], 0, false, 17).unwrap();
        assert_eq!((small[0].width, small[0].height, small[0].refresh), (1280, 600, 30));
    }

    #[test]
    fn several_screens_share_the_scale() {
        let screens = [m(2560, 1440, 60), m(3840, 2160, 60)];
        let (modes, l) = lowered(&screens, 1, false, 200).unwrap();
        assert!(floor_mbps(&modes, false) <= 200);
        let k0 = f64::from(modes[0].height) / 1440.0;
        let k1 = f64::from(modes[1].height) / 2160.0;
        assert!((k0 - k1).abs() < 0.01, "{modes:?}");
        // Ce que l'app montre : l'écran principal.
        assert_eq!((l.width, l.height), (modes[1].width, modes[1].height));
    }
}
