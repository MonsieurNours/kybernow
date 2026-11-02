//! Les mires de l'écran « Réglage du HDR » (docs/hdr.md, « Réglage du HDR ») :
//! des images fixes HEVC Main 10 PQ, une par palier de luminosité de chaque
//! étape, fabriquées par `scripts/hdr-mires.py` dans `assets/hdr-mires.bin`.
//! Le téléviseur les affiche par sa chaîne vidéo (NDL), en mode HDR : son plan
//! graphique, celui de l'interface, est en SDR.

use std::sync::OnceLock;

use crate::i18n::{english, tr};
use crate::model::Luminance;

static PACK: &[u8] = include_bytes!("../assets/hdr-mires.bin");

/// Les quatre étapes, dans l'ordre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Luminosité maximale (petite zone).
    Peak,
    /// Luminosité sur tout l'écran.
    Full,
    Black,
    /// Blanc du bureau.
    White,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Peak, Step::Full, Step::Black, Step::White];

    pub fn title(self) -> &'static str {
        match self {
            Step::Peak => tr("Luminosité maximale", "Maximum brightness"),
            Step::Full => tr("Luminosité sur tout l'écran", "Full-screen brightness"),
            Step::Black => tr("Noir", "Black"),
            Step::White => tr("Blanc du bureau", "Desktop white"),
        }
    }

    pub fn sentence(self) -> &'static str {
        match self {
            Step::Peak | Step::Full => tr("Montez jusqu'à ce que le symbole du centre disparaisse à peine.", "Raise it until the symbol in the middle just disappears."),
            Step::Black => tr("Baissez jusqu'à ce que le symbole disparaisse à peine.", "Lower it until the symbol just disappears."),
            Step::White => tr("Choisissez la luminosité des fenêtres et du texte de l'ordinateur.", "Pick how bright the computer's windows and text look."),
        }
    }

    /// La valeur de cette étape dans des valeurs de l'écran.
    pub fn of(self, l: &Luminance) -> f32 {
        match self {
            Step::Peak => l.max,
            Step::Full => l.full_frame,
            Step::Black => l.min,
            Step::White => l.sdr_white,
        }
    }

    fn set(self, l: &mut Luminance, v: f32) {
        match self {
            Step::Peak => l.max = v,
            Step::Full => l.full_frame = v,
            Step::Black => l.min = v,
            Step::White => l.sdr_white = v,
        }
    }
}

/// Une mire : son étape, sa valeur (nits), l'image (Annex B).
struct Mire {
    step: usize,
    nits: f32,
    image: &'static [u8],
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn parse(b: &'static [u8]) -> Vec<Mire> {
    if b.get(0..4) != Some(b"KNHM") || u32_at(b, 4) != Some(1) {
        return Vec::new();
    }
    let n = u32_at(b, 8).unwrap_or(0) as usize;
    (0..n)
        .filter_map(|i| {
            let e = 12 + 16 * i;
            let (step, milli, off, len) = (u32_at(b, e)?, u32_at(b, e + 4)?, u32_at(b, e + 8)? as usize, u32_at(b, e + 12)? as usize);
            Some(Mire { step: step as usize, nits: milli as f32 / 1000.0, image: b.get(off..off + len)? })
        })
        .collect()
}

fn pack() -> &'static [Mire] {
    static P: OnceLock<Vec<Mire>> = OnceLock::new();
    P.get_or_init(|| parse(PACK))
}

fn index(step: Step) -> usize {
    Step::ALL.iter().position(|s| *s == step).unwrap_or(0)
}

/// Les paliers d'une étape (nits, croissants).
pub fn values(step: Step) -> Vec<f32> {
    pack().iter().filter(|m| m.step == index(step)).map(|m| m.nits).collect()
}

/// L'image d'un palier (`i` : rang dans `values`).
pub fn image(step: Step, i: usize) -> Option<&'static [u8]> {
    pack().iter().filter(|m| m.step == index(step)).nth(i).map(|m| m.image)
}

/// Le rang du palier le plus proche de `v`.
pub fn nearest(step: Step, v: f32) -> usize {
    values(step).iter().enumerate().min_by(|a, b| (a.1 - v).abs().total_cmp(&(b.1 - v).abs())).map(|(i, _)| i).unwrap_or(0)
}

/// Les rangs des paliers de ces valeurs, étape par étape.
pub fn indices(l: &Luminance) -> [usize; 4] {
    Step::ALL.map(|s| nearest(s, s.of(l)))
}

/// Les valeurs de ces paliers ; la luminosité sur tout l'écran ne dépasse pas la maximale.
pub fn luminance(idx: &[usize; 4]) -> Luminance {
    let mut l = Luminance::default();
    for (s, i) in Step::ALL.iter().zip(idx) {
        if let Some(v) = values(*s).get(*i) {
            s.set(&mut l, *v);
        }
    }
    l.full_frame = l.full_frame.min(l.max);
    l
}

/// « 1 000 nits », « 0,05 nit » (« 1,000 nits », « 0.05 nits »).
pub fn nits_label(v: f32) -> String {
    let number = if v >= 1.0 && v.fract() == 0.0 {
        let n = v as u32;
        if n >= 1000 {
            let sep = if english() { "," } else { "\u{a0}" };
            format!("{}{sep}{:03}", n / 1000, n % 1000)
        } else {
            n.to_string()
        }
    } else {
        let s = format!("{v:.3}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if english() { s } else { s.replace('.', ",") }
    };
    // En français, « nit » sous 2.
    let unit = if !english() && v < 2.0 { "nit" } else { "nits" };
    format!("{number} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_and_labels() {
        crate::i18n::force_english(false);
        assert_eq!(values(Step::Peak).len(), 27);
        assert_eq!(values(Step::Black)[0], 0.0);
        assert_eq!(values(Step::White).first().copied(), Some(80.0));
        // Les valeurs par défaut tombent sur un palier (le blanc de 203 nits sur 200).
        let auto = luminance(&indices(&Luminance::default()));
        assert_eq!((auto.max, auto.full_frame, auto.min, auto.sdr_white), (1000.0, 1000.0, 0.0, 200.0));
        // Chaque mire est une image HEVC Main 10 avec ses métadonnées HDR10.
        for s in Step::ALL {
            let seen = crate::hdr::scan_hevc(image(s, 0).unwrap());
            assert!(seen.main10 && seen.has_metadata(), "{s:?}");
        }
        assert_eq!(nits_label(1000.0), "1\u{a0}000 nits");
        assert_eq!(nits_label(0.05), "0,05 nit");
        assert_eq!(nits_label(0.0), "0 nit");
        assert_eq!(nits_label(200.0), "200 nits");
        crate::i18n::force_english(true);
        assert_eq!(nits_label(1000.0), "1,000 nits");
        assert_eq!(nits_label(0.05), "0.05 nits");
        // Plein écran plus clair que le maximum : ramené au maximum.
        let l = luminance(&[0, 26, 0, 0]);
        assert_eq!(l.full_frame, l.max);
    }
}
