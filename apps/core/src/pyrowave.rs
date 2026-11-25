//! PyroWave (docs/pyrowave.md, « Définition ramenée ») : le débit dont une
//! image PyroWave a besoin, et la définition (puis la fréquence) ramenée pour
//! tenir dans le plafond de la session. PyroWave ne compresse presque pas :
//! lui donner moins de débit que nécessaire rend l'image floue. La règle de
//! docs/link-ui.md (« PyroWave ») est donc de baisser la définition, puis la
//! fréquence, jamais la netteté, et de le dire.
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

/// Les écrans ramenés pour tenir dans `ceiling_mbps` ; `None` s'ils y
/// tiennent déjà (ou s'il n'y en a pas). `primary` : l'index de l'écran
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
    let fits = |ms: &[Mode]| bits_per_second(ms, yuv444) <= budget;
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
        let guess = (budget / bits_per_second(&full, yuv444)).sqrt();
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
        // 4K60 4:2:0 dans 500 Mbit/s : ≈ 0,82 de l'échelle.
        let (modes, l) = lowered(&[m(3840, 2160, 60)], 0, false, 500).unwrap();
        let r = modes[0];
        assert_eq!((r.width % 2, r.height % 2, r.refresh), (0, 0, 60));
        assert!(needed_mbps(&modes, false) <= 500, "{r:?}");
        // La hauteur paire au-dessus ne tient plus.
        let above = scaled(&[m(3840, 2160, 60)], f64::from(r.height + 2) / 2160.0, 60);
        assert!(needed_mbps(&above, false) > 500, "{above:?}");
        assert_eq!(l, Lowered { width: r.width, height: r.height, refresh: 60, mbps: 500 });
        // Proportions gardées.
        assert!((f64::from(r.width) / f64::from(r.height) - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn four_four_four_costs_more() {
        let a = fit(&[m(3840, 2160, 60)], 0, false, 800);
        let b = fit(&[m(3840, 2160, 60)], 0, true, 800).unwrap();
        assert_eq!(a, None);
        assert!(b[0].height < 2160 && needed_mbps(&b, true) <= 800);
    }

    #[test]
    fn then_the_refresh_rate() {
        // 4K120 dans 150 Mbit/s : 720p120 demande 166 Mbit/s, donc 100 Hz,
        // avec la plus grande définition qui tient à 100 Hz.
        let modes = fit(&[m(3840, 2160, 120)], 0, false, 150).unwrap();
        assert_eq!(modes[0].refresh, 100);
        assert!(modes[0].height >= MIN_HEIGHT && needed_mbps(&modes, false) <= 150);
        // Rien ne tient : 720p à 30 Hz.
        let floor = fit(&[m(3840, 2160, 120)], 0, false, 10).unwrap();
        assert_eq!(floor[0], m(1280, 720, 30));
        // Un écran plus petit que 720p descend en fréquence tout de suite.
        let small = fit(&[m(1280, 600, 60)], 0, false, 50).unwrap();
        assert_eq!((small[0].width, small[0].height, small[0].refresh), (1280, 600, 30));
    }

    #[test]
    fn several_screens_share_the_scale() {
        let screens = [m(2560, 1440, 60), m(3840, 2160, 60)];
        let (modes, l) = lowered(&screens, 1, false, 600).unwrap();
        assert!(needed_mbps(&modes, false) <= 600);
        let k0 = f64::from(modes[0].height) / 1440.0;
        let k1 = f64::from(modes[1].height) / 2160.0;
        assert!((k0 - k1).abs() < 0.01, "{modes:?}");
        // Ce que l'app montre : l'écran principal.
        assert_eq!((l.width, l.height), (modes[1].width, modes[1].height));
    }
}
