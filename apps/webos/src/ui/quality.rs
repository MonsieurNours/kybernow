//! La « Qualité de l'image » d'un invité de KyberNow Share en session (docs/link-ui.md, « Partage ») : les
//! quatre choix (« Auto », « Comme l'hôte », « 1080p · 60 Hz », « 720p · 30 Hz »), le choisi coché ; sur la
//! ligne choisie, la qualité reçue quand elle diffère du libellé (« Auto » : « 1080p · 60 Hz »). Un panneau
//! par-dessus l'image, ouvert par la tuile « Qualité de l'image » du menu de session (Retour revient au
//! menu). La changer ne relance que l'image de l'invité.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{Rect, SessionUi, ShareQuality, Target};
use crate::i18n::tr;

/// Les quatre choix d'une session d'invité : (cible, libellé, la qualité reçue sur la ligne choisie, choisi).
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

pub fn draw(cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    cv.text(W / 2.0, 290.0, Text::new(Font::Title, 40.0, FROST).align(Align::Center), tr("Qualité de l'image", "Picture quality"));
    let (x0, y0, rw, rh, gap) = (500.0, 340.0, W - 1000.0, 96.0, 22.0);
    for (i, (t, label, received, on)) in items(s).into_iter().enumerate() {
        let rect = Rect::new(x0, y0 + i as f32 * (rh + gap), rw, rh);
        let focused = f.hit(t, rect);
        let (fg, sub) = if focused {
            cv.neon(rect.x, rect.y, rect.w, rect.h, 16.0, 0.9);
            cv.rrect(rect.x, rect.y, rect.w, rect.h, 16.0, CORE);
            (INK, INK)
        } else {
            cv.rrect(rect.x, rect.y, rect.w, rect.h, 16.0, Rgba(14, 19, 28, 235));
            cv.rrect_stroke(rect.x, rect.y, rect.w, rect.h, 16.0, if on { 2.0 } else { 1.0 }, if on { SPECTRE } else { BORDER });
            (FROST, FROST_3)
        };
        let mut tx = rect.x + 36.0;
        if on {
            cv.icon(&Icon::Check, tx, rect.y + rh / 2.0 - 16.0, 32.0, 2.0, if focused { INK } else { SPECTRE });
            tx += 56.0;
        }
        cv.text(tx, rect.y + 60.0, Text::new(Font::Semibold, 30.0, fg), &label);
        if !received.is_empty() {
            cv.text_fit(rect.x + rect.w - 36.0, rect.y + 58.0, Text::new(Font::Mono, 24.0, sub).align(Align::Right), &received, rect.w * 0.5);
        }
    }
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Choisir", "Choose")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}
