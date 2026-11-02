//! « Réglage du HDR » (docs/hdr.md ; textes : docs/link-ui.md, « HDR ») : la
//! mire est sur le plan vidéo, en HDR (`crate::mires`, montrée par le
//! décodeur du téléviseur) ; l'interface, en SDR par-dessus, reste aux bords
//! pour ne pas la couvrir : l'étape en haut, la valeur et les boutons en bas.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::theme::*;
use super::widgets;
use super::Frame;
use crate::app::{App, Rect, Target};
use crate::i18n::tr;
use crate::mires;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear_transparent();
    let Some(c) = &app.calibration else { return };
    let step = c.step();

    // En haut : le titre de l'écran, l'introduction, l'étape et sa phrase.
    let (x, y, w) = (96.0, 56.0, 1180.0);
    cv.rrect(x - 36.0, y - 24.0, w + 72.0, 214.0, 22.0, PANEL_SOLID.alpha(0.92));
    let head = format!("{} · {}/4", tr("Réglage du HDR", "HDR calibration").to_uppercase(), c.step + 1);
    cv.text(x, y + 22.0, Text::new(Font::Mono, 16.0, FROST_3).tracking(3.0), &head);
    cv.text_fit(x, y + 78.0, Text::new(Font::Title, 40.0, FROST), step.title(), w);
    cv.text_fit(x, y + 122.0, Text::new(Font::Regular, 24.0, FROST), step.sentence(), w);
    cv.text_fit(x, y + 158.0, Text::new(Font::Regular, 19.0, FROST_3), tr("Quatre étapes pour que les jeux connaissent les vraies lumières de cet écran.", "Four steps so games know this screen's real brightness."), w);

    // En bas : moins, la valeur, plus ; les boutons ; le rappel du mode du téléviseur.
    let by = H - 250.0;
    cv.rrect(60.0, by - 24.0, W - 120.0, 230.0, 22.0, PANEL_SOLID.alpha(0.92));
    let values = mires::values(step);
    let i = c.idx[c.step].min(values.len().saturating_sub(1));
    let value = values.get(i).map(|v| mires::nits_label(*v)).unwrap_or_default();
    let mut xx = 96.0;
    xx += arrow(cv, f, Target::CalLess, xx, by, "‹", i > 0) + 20.0;
    let style = Text::new(Font::Mono, 34.0, FROST).align(Align::Center);
    cv.text(xx + 140.0, by + 46.0, style, &value);
    xx += 280.0 + 20.0;
    arrow(cv, f, Target::CalMore, xx, by, "›", i + 1 < values.len());
    // Une jauge de la position du palier.
    let (gx, gw) = (96.0, 480.0);
    cv.rrect(gx, by + 84.0, gw, 6.0, 3.0, Rgba(227, 237, 240, 30));
    let t = if values.len() > 1 { i as f32 / (values.len() - 1) as f32 } else { 0.0 };
    cv.rrect(gx, by + 84.0, (gw * t).max(6.0), 6.0, 3.0, SPECTRE);

    let last = c.step == 3;
    let mut bx = W - 96.0;
    let next = if last { tr("Terminer", "Done") } else { tr("Suivant", "Next") };
    let nw = cv.text_width(Text::new(Font::Semibold, 30.0, INK), next) + 104.0;
    bx -= nw;
    widgets::primary(cv, f, Target::CalNext, bx, by - 6.0, next);
    if c.step > 0 {
        let label = tr("Retour", "Back");
        bx -= cv.text_width(Text::new(Font::Semibold, 24.0, FROST), label) + 48.0 + 24.0;
        widgets::secondary(cv, f, Target::CalPrevious, bx, by + 6.0, label, 24.0);
    }
    let label = tr("Annuler", "Cancel");
    bx -= cv.text_width(Text::new(Font::Semibold, 24.0, FROST), label) + 48.0 + 24.0;
    widgets::secondary(cv, f, Target::CalCancel, bx, by + 6.0, label, 24.0);
    let auto = tr("Revenir au réglage automatique", "Back to automatic");
    let aw = cv.text_width(Text::new(Font::Semibold, 20.0, FROST), auto) + 40.0;
    widgets::secondary(cv, f, Target::CalAuto, W - 96.0 - aw, by + 104.0, auto, 20.0);
    // Le téléviseur doit écrêter à sa vraie luminosité, sans courbe à lui : Optimiseur de jeu et HGIG.
    let hint = tr(
        "Pour un réglage juste : Optimiseur de jeu, et Mappage dynamique des tons sur HGIG (bouton Réglages de la Magic Remote › Image).",
        "For an accurate calibration: Game Optimizer, and Dynamic Tone Mapping set to HGIG (Magic Remote Settings button › Picture).",
    );
    cv.text_fit(96.0, by + 168.0, Text::new(Font::Regular, 18.0, FROST_3), hint, W - 192.0 - aw - 40.0);
}

/// Un bouton rond, moins ou plus ; estompé au bout de la liste.
fn arrow(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, s: &str, enabled: bool) -> f32 {
    let d = 64.0;
    let focused = f.hit(t, Rect::new(x, y, d, d));
    if focused {
        cv.neon(x, y, d, d, d / 2.0, 0.8);
        cv.rrect(x, y, d, d, d / 2.0, CORE);
    } else {
        cv.rrect(x, y, d, d, d / 2.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(x, y, d, d, d / 2.0, 1.5, BORDER);
    }
    let color = if focused { INK } else if enabled { FROST } else { FROST_4 };
    cv.text(x + d / 2.0, y + 44.0, Text::new(Font::Semibold, 36.0, color).align(Align::Center), s);
    d
}
