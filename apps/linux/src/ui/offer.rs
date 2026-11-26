//! Mise à jour des Stations (docs/link-ui.md, « Mise à jour des Stations » ; maquettes MajStationBandeau et
//! MajStationAvis) : le bandeau de l'accueil, sous la rangée « Se connecter » et profils, sur toute sa largeur ; l'avis
//! après une mise à jour de l'app, par-dessus l'accueil (en fenêtre au bureau, en plein écran à la manette et au
//! salon). Seuls ses boutons se choisissent.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::theme::*;
use super::widgets;
use super::{unit, Frame};
use crate::app::{App, Layout, Target};
use crate::station_offer::{row_state_text, version_line, HomeBanner, RowState};

/// La flèche vers le haut du bandeau, dans un cercle.
fn arrow_up(cv: &mut Canvas, cx: f32, cy: f32, r: f32, u: f32, color: Rgba) {
    cv.circle(cx, cy, r, color.alpha(0.16));
    let a = r * 0.45;
    cv.polyline(&[(cx, cy + a), (cx, cy - a)], 1.9 * u, color);
    cv.polyline(&[(cx - a * 0.75, cy - a * 0.2), (cx, cy - a), (cx + a * 0.75, cy - a * 0.2)], 1.9 * u, color);
}

/// Le cercle d'attente.
fn spinner(app: &App, cv: &mut Canvas, cx: f32, cy: f32, r: f32, u: f32) {
    let t = app.screen_since.elapsed().as_secs_f32();
    let a0 = if app.reduce_motion { 0.0 } else { t * 4.0 };
    cv.circle_stroke(cx, cy, r, 1.5 * u, LINE_2);
    let pts: Vec<(f32, f32)> = (0..=16).map(|i| a0 + i as f32 * 0.28).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect();
    cv.polyline(&pts, 2.0 * u, SPECTRE);
}

fn primary_w(cv: &mut Canvas, h: f32, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Semibold, h * 0.32, FROST), s) + 2.0 * h * 0.58
}

fn ghost_w(cv: &mut Canvas, size: f32, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, size, FROST), s) + size * 2.0
}

/// Hauteur, hauteur des boutons, taille du texte, largeur du texte.
fn layout(app: &App, cv: &mut Canvas, b: &HomeBanner, width: f32) -> (f32, f32, f32, f32) {
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let bh = if desktop { 38.0 } else { 46.0 * u };
    let size = if desktop { 14.0 } else { 15.0 * u };
    let pad = 16.0 * u;
    let mut buttons = 0.0;
    if let Some(l) = b.main_label() {
        buttons += primary_w(cv, bh, l) + 10.0 * u;
    }
    if b.later() {
        buttons += ghost_w(cv, size, crate::i18n::tr("Plus tard", "Later")) + 10.0 * u;
    }
    let text_w = (width - pad - 36.0 * u - 14.0 * u - buttons - pad).max(160.0 * u);
    let style = Text::new(Font::Regular, size, FROST);
    let lines = cv.line_count(style, &b.text(), text_w) as f32;
    let h = (lines * size * 1.45 + 2.0 * pad).max(bh + 2.0 * 12.0 * u);
    (h, bh, size, text_w)
}

/// La hauteur du bandeau de l'ordinateur sur `width`, sans le dessiner (0 sans bandeau).
pub fn height(app: &App, cv: &mut Canvas, id: &str, width: f32) -> f32 {
    app.home_banner(id).map(|b| layout(app, cv, &b, width).0).unwrap_or(0.0)
}

/// Le bandeau de l'ordinateur choisi, en `(x, y)` sur `width` ; rend sa hauteur (0 sans bandeau).
#[allow(clippy::too_many_arguments)]
pub fn banner(app: &App, cv: &mut Canvas, f: &mut Frame, id: &str, x: f32, y: f32, width: f32) -> f32 {
    let Some(b) = app.home_banner(id) else { return 0.0 };
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let (h, bh, size, text_w) = layout(app, cv, &b, width);
    let pad = 16.0 * u;
    let failed = matches!(b, HomeBanner::Failed(_));
    let accent = if failed { WARN } else { SPECTRE };
    cv.rrect(x, y, width, h, 12.0 * u, Rgba(9, 13, 18, 230));
    cv.rrect_stroke(x, y, width, h, 12.0 * u, 1.0, if failed { WARN.alpha(0.55) } else { LINE_2 });
    cv.rrect(x + 1.0, y + 12.0 * u, 3.0 * u, h - 24.0 * u, 1.5 * u, accent);
    let cy = y + h / 2.0;
    let ix = x + pad + 18.0 * u;
    if b == HomeBanner::Busy {
        spinner(app, cv, ix, cy, 10.0 * u, u);
    } else {
        arrow_up(cv, ix, cy, 18.0 * u, u, accent);
    }
    let tx = x + pad + 36.0 * u + 14.0 * u;
    let style = Text::new(Font::Regular, size, if failed { WARN } else { FROST });
    let text = b.text();
    let lines = cv.line_count(style, &text, text_w) as f32;
    let lh = size * 1.45;
    cv.paragraph(tx, cy - lines * lh / 2.0 + size * 1.02, style, &text, text_w, lh);
    // Les boutons, à droite : « Mettre à jour » (le principal), « Plus tard ».
    let mut bx = x + width - pad;
    let by = cy - bh / 2.0;
    if b.later() {
        let later = crate::i18n::tr("Plus tard", "Later");
        bx -= ghost_w(cv, size, later);
        widgets::ghost(cv, f, Target::OfferLater, bx, by, bh, size, None, later, MUTED);
        bx -= 10.0 * u;
    }
    if let Some(l) = b.main_label() {
        bx -= primary_w(cv, bh, l);
        widgets::primary(cv, f, Target::OfferUpdate, bx, by, bh, l, false);
    }
    h
}

/// L'avis après une mise à jour de l'app : au bureau une fenêtre au milieu, à la manette et au salon tout l'écran ;
/// le titre, le texte ; une Station : son état sous le texte ; plusieurs : une ligne par ordinateur (le nom,
/// « Version {v} », « Mettre à jour » ou son état) ; en bas « Plus tard » (« Fermer » quand tout est à jour) et le
/// bouton principal.
pub fn notice(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(n) = &app.station_notice else { return };
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let (w, h) = (cv.w, cv.h);
    f.hits.clear();
    let bh = if desktop { 44.0 } else { 56.0 * u };
    let size = if desktop { 14.0 } else { 17.0 * u };
    let title = Text::new(Font::Display, if desktop { 26.0 } else { 34.0 * u }, FROST).tracking(-0.3 * u);
    let body = Text::new(Font::Regular, if desktop { 15.0 } else { 18.0 * u }, MUTED);
    let lh = if desktop { 22.0 } else { 27.0 * u };
    let row_h = if desktop { 58.0 } else { 72.0 * u };
    let (x, y, cw, ch, pad);
    if desktop {
        cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
        cw = 600.0f32.min(w - 48.0);
        pad = 32.0;
        let iw = cw - 2.0 * pad;
        let rows = if n.single() { 34.0 } else { 16.0 + n.rows.len() as f32 * row_h };
        let text_h = cv.line_count(body, &n.text(), iw) as f32 * lh;
        let title_h = cv.line_count(title, &n.title(), iw) as f32 * title.size * 1.2;
        ch = (pad + 34.0 + 18.0 + title_h + 14.0 + text_h + 10.0 + rows + 28.0 + bh + pad).min(h - 24.0);
        x = (w - cw) / 2.0;
        y = ((h - ch) / 2.0).max(12.0);
        cv.rrect(x, y, cw, ch, 16.0, Rgba(9, 13, 18, 245));
        cv.rrect_stroke(x, y, cw, ch, 16.0, 1.0, LINE_2);
    } else {
        cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 246));
        cw = (w - 2.0 * if app.layout == Layout::Tv { 96.0 } else { 40.0 }).min(1100.0 * u);
        pad = 0.0;
        x = (w - cw) / 2.0;
        y = 0.0;
        ch = h - 56.0 * u;
    }
    let ix = x + pad;
    let iw = cw - 2.0 * pad;
    let mut yy = if desktop { y + pad } else { (h * 0.16).max(48.0 * u) };
    arrow_up(cv, ix + 17.0 * u, yy + 17.0 * u, 17.0 * u, u, SPECTRE);
    yy += 34.0 * u + 18.0 * u;
    yy += cv.paragraph(ix, yy + title.size * 0.8, title, &n.title(), iw, title.size * 1.2);
    yy += 14.0 * u;
    yy += cv.paragraph(ix, yy + body.size, body, &n.text(), iw, lh) + 10.0 * u;
    if n.single() {
        // Une Station : son état sous le texte (« Mise à jour… », « À jour », un refus, un échec).
        let r = &n.rows[0];
        if let Some(s) = row_state_text(&r.state) {
            let color = row_color(&r.state);
            if r.state == RowState::Busy {
                spinner(app, cv, ix + 9.0 * u, yy + 12.0 * u, 9.0 * u, u);
                cv.text_fit(ix + 28.0 * u, yy + 18.0 * u, Text::new(Font::Regular, body.size, color), &s, iw - 28.0 * u);
            } else {
                cv.text_fit(ix, yy + 18.0 * u, Text::new(Font::Regular, body.size, color), &s, iw);
            }
        }
        yy += 34.0 * u;
    } else {
        yy += 6.0 * u;
        for (i, r) in n.rows.iter().enumerate() {
            cv.rect(ix, yy, iw, 1.0, LINE.alpha(0.8));
            let name = Text::new(Font::Semibold, if desktop { 15.5 } else { 19.0 * u }, FROST);
            cv.text_fit(ix, yy + row_h * 0.45, name, &r.name, iw * 0.5);
            cv.text(ix, yy + row_h * 0.78, Text::new(Font::Mono, if desktop { 12.0 } else { 14.0 * u }, FAINT), &version_line(&r.version));
            let right = ix + iw;
            let rb = if desktop { 36.0 } else { 46.0 * u };
            let by = yy + (row_h - rb) / 2.0;
            let rsize = if desktop { 13.5 } else { 16.0 * u };
            let state = row_state_text(&r.state);
            let mut sx = right;
            match &r.state {
                RowState::Ready | RowState::Failed(_) => {
                    let l = crate::i18n::tr("Mettre à jour", "Update");
                    sx -= ghost_w(cv, rsize, l);
                    widgets::ghost(cv, f, Target::NoticeRow(i), sx, by, rb, rsize, None, l, FROST);
                    sx -= 14.0 * u;
                }
                RowState::Refused(_) => {
                    let l = crate::station_offer::update_anyway();
                    sx -= ghost_w(cv, rsize, l);
                    widgets::ghost(cv, f, Target::NoticeRow(i), sx, by, rb, rsize, None, l, FROST);
                    sx -= 14.0 * u;
                }
                RowState::Busy => {
                    spinner(app, cv, sx - 9.0 * u, yy + row_h / 2.0, 9.0 * u, u);
                    sx -= 28.0 * u;
                }
                RowState::Done => {}
            }
            if let Some(s) = state {
                let st = Text::new(Font::Regular, rsize, row_color(&r.state)).align(Align::Right);
                cv.text_fit(sx, yy + row_h / 2.0 + rsize * 0.36, st, &s, (sx - (ix + iw * 0.42)).max(60.0 * u));
            }
            yy += row_h;
        }
        cv.rect(ix, yy, iw, 1.0, LINE.alpha(0.8));
    }
    // En bas : « Plus tard » (« Fermer ») puis le bouton principal, à droite.
    let by = if desktop { y + ch - pad - bh } else { (yy + 28.0 * u).min(ch - bh - 24.0 * u) };
    let mut bx = ix + iw;
    if let Some(l) = n.main_label() {
        bx -= primary_w(cv, bh, l);
        widgets::primary(cv, f, Target::NoticeMain, bx, by, bh, l, false);
        bx -= 12.0 * u;
    }
    let later = n.later_label();
    bx -= ghost_w(cv, size, later);
    widgets::ghost(cv, f, Target::NoticeLater, bx, by, bh, size, None, later, MUTED);
    // À la manette : la légende des deux boutons.
    if !desktop {
        cv.rect(0.0, h - 56.0 * u, w, 56.0 * u, Rgba(5, 7, 10, 250));
        cv.rect(0.0, h - 56.0 * u, w, 1.0, LINE);
        let gy = h - 28.0 * u;
        let mut lx = 40.0 * u;
        lx += widgets::legend(cv, lx, gy, 26.0 * u, &["A"], crate::i18n::tr("Choisir", "Choose")) + 28.0 * u;
        widgets::legend(cv, lx, gy, 26.0 * u, &["B"], later);
    }
}

fn row_color(s: &RowState) -> Rgba {
    match s {
        RowState::Done => OK,
        RowState::Refused(_) | RowState::Failed(_) => WARN,
        _ => MUTED,
    }
}
