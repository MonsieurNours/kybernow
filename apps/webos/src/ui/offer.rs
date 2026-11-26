//! Mise à jour des Stations (docs/link-ui.md, « Mise à jour des Stations » ; maquettes MajStationBandeau et
//! MajStationAvis) : le bandeau de l'accueil, sous la rangée « Se connecter » et profils, sur toute sa largeur (la
//! flèche bas depuis « Se connecter » y mène) ; l'avis après une mise à jour de l'app, en plein écran, le focus sur
//! son bouton principal. Seuls ses boutons se choisissent.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, Target};
use crate::station_offer::{row_state_text, version_line, HomeBanner, RowState};

/// La hauteur du bandeau : l'accueil remonte d'autant quand il est là.
pub const BANNER_H: f32 = 92.0;

/// La flèche vers le haut, dans un cercle.
fn arrow_up(cv: &mut Canvas, cx: f32, cy: f32, r: f32, color: Rgba) {
    cv.circle(cx, cy, r, color.alpha(0.16));
    let a = r * 0.45;
    cv.line(cx, cy + a, cx, cy - a, 3.0, color);
    cv.line(cx - a * 0.75, cy - a * 0.2, cx, cy - a, 3.0, color);
    cv.line(cx + a * 0.75, cy - a * 0.2, cx, cy - a, 3.0, color);
}

/// L'attente : un cercle de points qui tourne.
fn spinner(cv: &mut Canvas, cx: f32, cy: f32) {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f32() % 60.0).unwrap_or(0.0);
    for i in 0..12 {
        let a = t * 4.0 + i as f32 * 0.4;
        cv.circle(cx + 12.0 * a.cos(), cy + 12.0 * a.sin(), 2.4, SPECTRE.alpha(0.3 + 0.06 * i as f32));
    }
}

fn secondary_w(cv: &mut Canvas, s: &str, size: f32) -> f32 {
    cv.text_width(Text::new(Font::Semibold, size, FROST), s) + size * 2.0
}

/// Le bandeau de l'ordinateur choisi, en `y` (haut du bandeau) ; rien sans bandeau.
pub fn banner(app: &App, cv: &mut Canvas, f: &mut Frame, id: &str, y: f32) {
    let Some(b) = app.home_banner(id) else { return };
    let (x, w, h) = (96.0, W - 192.0, BANNER_H);
    let failed = matches!(b, HomeBanner::Failed(_));
    let accent = if failed { WARN } else { SPECTRE };
    cv.rrect(x, y, w, h, 18.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, w, h, 18.0, 1.5, if failed { WARN.alpha(0.55) } else { BORDER });
    cv.rrect(x + 1.0, y + 18.0, 4.0, h - 36.0, 2.0, accent);
    let cy = y + h / 2.0;
    if b == HomeBanner::Busy {
        spinner(cv, x + 52.0, cy);
    } else {
        arrow_up(cv, x + 52.0, cy, 26.0, accent);
    }
    // Les boutons, à droite : « Mettre à jour » (le principal), « Plus tard ».
    let size = 22.0;
    let bh = size * 2.4;
    let mut bx = x + w - 24.0;
    let by = cy - bh / 2.0;
    if b.later() {
        let later = crate::i18n::tr("Plus tard", "Later");
        bx -= secondary_w(cv, later, size);
        widgets::secondary(cv, f, Target::OfferLater, bx, by, later, size);
        bx -= 14.0;
    }
    if let Some(l) = b.main_label() {
        let bw = secondary_w(cv, l, size);
        bx -= bw;
        // Le principal : à l'aplat Spectre ; au focus, le néon.
        let focused = f.hit(Target::OfferUpdate, crate::app::Rect::new(bx, by, bw, bh));
        if focused {
            cv.neon(bx, by, bw, bh, 12.0, 0.9);
        }
        cv.rrect(bx, by, bw, bh, 12.0, if focused { CORE } else { FLAT });
        let style = Text::new(Font::Semibold, size, CORE);
        cv.text(bx + size, by + bh / 2.0 + size * 0.36, style.color(if focused { INK } else { CORE }), l);
        bx -= 14.0;
    }
    // Le texte, sur une ligne ou deux.
    let tx = x + 100.0;
    let style = Text::new(Font::Regular, 22.0, if failed { WARN } else { FROST });
    let text = b.text();
    let max = bx - tx - 12.0;
    let lines = if cv.text_width(style, &text) <= max { 1.0 } else { 2.0 };
    cv.paragraph(tx, cy - lines * 30.0 / 2.0 + 22.0, style, &text, max, 30.0);
}

/// L'avis après une mise à jour de l'app, en plein écran : le titre, le texte ; une Station : son état ; plusieurs :
/// une ligne par ordinateur (le nom, « Version {v} », « Mettre à jour » ou son état) ; le bouton principal et
/// « Plus tard » (« Fermer » quand tout est à jour).
pub fn notice(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(n) = &app.station_notice else { return };
    f.hits.clear();
    cv.clear(INK);
    let x = 160.0;
    let iw = W - 2.0 * x;
    let mut y = if n.single() { 260.0 } else { 170.0 };
    arrow_up(cv, x + 30.0, y, 30.0, SPECTRE);
    y += 110.0;
    y += cv.paragraph(x, y, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), &n.title(), iw, 76.0);
    y += 10.0;
    y += cv.paragraph(x, y, Text::new(Font::Regular, 28.0, FROST_2), &n.text(), iw.min(1300.0), 40.0);
    if n.single() {
        let r = &n.rows[0];
        if let Some(s) = row_state_text(&r.state) {
            let color = row_color(&r.state);
            if r.state == RowState::Busy {
                spinner(cv, x + 14.0, y + 18.0);
                cv.text_fit(x + 44.0, y + 28.0, Text::new(Font::Regular, 26.0, color), &s, iw - 44.0);
            } else {
                cv.text_fit(x, y + 28.0, Text::new(Font::Regular, 26.0, color), &s, iw);
            }
        }
        y += 60.0;
    } else {
        y += 20.0;
        let row_h = 88.0;
        for (i, r) in n.rows.iter().enumerate() {
            cv.rect(x, y, iw, 1.0, LINE);
            cv.text_fit(x, y + 40.0, Text::new(Font::Semibold, 28.0, FROST), &r.name, iw * 0.5);
            cv.text(x, y + 72.0, Text::new(Font::Mono, 18.0, FROST_3), &version_line(&r.version));
            let size = 22.0;
            let bh = size * 2.4;
            let mut sx = x + iw;
            let label = match &r.state {
                RowState::Ready | RowState::Failed(_) => Some(crate::i18n::tr("Mettre à jour", "Update")),
                RowState::Refused(_) => Some(crate::station_offer::update_anyway()),
                _ => None,
            };
            if let Some(l) = label {
                sx -= secondary_w(cv, l, size);
                widgets::secondary(cv, f, Target::NoticeRow(i), sx, y + (row_h - bh) / 2.0, l, size);
                sx -= 20.0;
            } else if r.state == RowState::Busy {
                spinner(cv, sx - 14.0, y + row_h / 2.0);
                sx -= 44.0;
            }
            if let Some(s) = row_state_text(&r.state) {
                let st = Text::new(Font::Regular, 24.0, row_color(&r.state)).align(Align::Right);
                cv.text_fit(sx, y + row_h / 2.0 + 9.0, st, &s, (sx - (x + iw * 0.4)).max(120.0));
            }
            y += row_h;
        }
        cv.rect(x, y, iw, 1.0, LINE);
        y += 20.0;
    }
    // En bas : le bouton principal, puis « Plus tard » (« Fermer »).
    let by = (y + 30.0).min(H - 120.0 - 84.0);
    let mut bx = x;
    if let Some(l) = n.main_label() {
        bx += widgets::primary(cv, f, Target::NoticeMain, bx, by, l) + 24.0;
    }
    widgets::secondary(cv, f, Target::NoticeLater, bx, by + 12.0, n.later_label(), 25.0);
    // Aides.
    let hy = H - 48.0;
    let hx = 96.0 + widgets::hint(cv, 96.0, hy, Key::Ok, crate::i18n::tr("Choisir", "Choose")) + 40.0;
    widgets::hint(cv, hx, hy, Key::Back, n.later_label());
}

fn row_color(s: &RowState) -> Rgba {
    match s {
        RowState::Done => SPECTRE,
        RowState::Refused(_) | RowState::Failed(_) => WARN,
        _ => FROST_2,
    }
}
