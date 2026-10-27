//! La « Qualité de l'image » d'un invité de KyberNow Share en session (docs/link-ui.md, « Partage ») : les cinq
//! choix (« Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz »), le choisi coché ; sur la ligne choisie,
//! la qualité reçue quand elle diffère du libellé (« Auto » : « 1080p · 60 Hz »). Au bureau, c'est le menu de la
//! puce de la barre (`session::bar`) ; à la manette (Steam Deck, salon, KyberNow Link OS), un panneau par-dessus
//! l'image, ouvert par la tuile « Qualité de l'image » du menu (B revient au menu). La changer ne relance que
//! l'image de l'invité.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{App, Layout, Rect, SessionUi, ShareQuality, Target};
use crate::i18n::tr;

/// Les cinq choix d'une session d'invité : (cible, libellé, la qualité reçue sur la ligne choisie, choisi).
pub fn items(s: &SessionUi) -> Vec<(Target, String, String, bool)> {
    let Some(g) = &s.guest else { return Vec::new() };
    ShareQuality::ALL
        .iter()
        .map(|q| {
            let on = g.quality == *q;
            // La qualité reçue, seulement si elle dit autre chose que le libellé (« Auto » : « 720p · 30 Hz »).
            let received = if on { g.image.clone().filter(|i| i != q.label()).unwrap_or_default() } else { String::new() };
            (Target::QualityPick(*q), q.label().to_string(), received, on)
        })
        .collect()
}

/// À la manette : le panneau au centre de l'image assombrie, et la légende des boutons.
pub fn overlay(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
    let x0 = if app.layout == Layout::Tv { 96.0 } else { 48.0 };
    let mut y = 50.0 * u;
    eyebrow(cv, x0, y, 12.0 * u, FAINT, tr("Qualité de l'image", "Picture quality"));
    y += 26.0 * u;
    let (rw, rh, gap) = ((w - 2.0 * x0).min(620.0 * u), 64.0 * u, 12.0 * u);
    for (t, label, received, on) in items(s) {
        let r = Rect::new(x0, y, rw, rh);
        let (focused, hover) = f.state(t, r);
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { Rgba(14, 20, 26, 230) });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, if on { SPECTRE } else { LINE_2 });
        let mut tx = r.x + 18.0 * u;
        if on {
            cv.icon(&Icon::Check, tx, r.y + rh / 2.0 - 9.0 * u, 18.0 * u, 2.0, SPECTRE);
            tx += 32.0 * u;
        }
        cv.text(tx, r.y + rh / 2.0 + 6.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), &label);
        if !received.is_empty() {
            cv.text_fit(r.x + r.w - 18.0 * u, r.y + rh / 2.0 + 5.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT).align(Align::Right), &received, r.w * 0.5);
        }
        y += rh + gap;
    }
    let footer = 56.0 * u;
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx = x0;
    lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
    legend(cv, lx, gy, g, &["B"], tr("Retour", "Back"));
}
