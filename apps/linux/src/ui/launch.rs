//! Lancement animé des interfaces console (Deck, salon), comme sur iPhone et
//! iPad (apps/ios/KyberNowIOS/LaunchView.swift) et sur les maquettes :
//!
//!   0 → 0,2 s    l'encre et le symbole éteint (l'image de la fenêtre qui arrive) ;
//!   0,2 → 0,6 s  le grésillement : trois éclairs, puis allumé ;
//!   0,8 → 1,1 s  le nom et la signature LINK sous le symbole ;
//!   → 1,8 s      la scène tient ;
//!   1,8 → 2,4 s  l'accueil prend la main : le symbole et le nom filent à leur
//!                place en haut à gauche, le décor arrive en fondu.
//!
//! « Réduire les animations » : le symbole s'allume sans grésiller, puis un
//! fondu à 0,7 s. Un appui sur A ou B saute à l'accueil.

use super::canvas::Canvas;
use super::theme::*;
use super::{home, unit, Frame};
use crate::app::{App, Layout};

/// Durée totale (jusqu'à la main à l'accueil).
pub const TOTAL: f32 = 2.4;
pub const TOTAL_REDUCED: f32 = 0.7;

/// Niveaux du néon du symbole : (instant, allumage, éclair), reliés en ligne droite.
const STEPS: [(f32, f32, f32); 8] = [(0.2, 0.0, 0.0), (0.25, 1.0, 1.0), (0.29, 0.0, 0.0), (0.34, 1.0, 0.0), (0.38, 0.0, 0.0), (0.44, 1.0, 1.0), (0.48, 0.5, 0.0), (0.57, 1.0, 0.0)];

fn neon_at(t: f32) -> (f32, f32) {
    if t <= STEPS[0].0 {
        return (0.0, 0.0);
    }
    for w in STEPS.windows(2) {
        let (t0, l0, f0) = w[0];
        let (t1, l1, f1) = w[1];
        if t <= t1 {
            let p = (t - t0) / (t1 - t0);
            return (l0 + (l1 - l0) * p, f0 + (f1 - f0) * p);
        }
    }
    (1.0, 0.0)
}

fn ease(p: f32) -> f32 {
    // cubic-bezier(.2, .9, .1, 1) approché : départ vif, arrivée douce.
    1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(4)
}

/// Où se pose le logo signé de l'en-tête : (x, y, hauteur), en points.
pub fn header_logo(layout: Layout) -> (f32, f32, f32) {
    match layout {
        Layout::Tv => (96.0, 31.0, 34.0),
        Layout::Desktop => (16.0, 9.0, 22.0),
        Layout::Handheld => (40.0, 15.0, 26.0),
    }
}

pub fn draw(app: &App, cv: &mut Canvas) {
    let mut t = app.launched.elapsed().as_secs_f32();
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let reduced = app.reduce_motion;
    let handover_start = if reduced { 0.45 } else { 1.8 };
    let total = if reduced { TOTAL_REDUCED } else { TOTAL };
    // KyberNow Link OS : la scène tient tant que quelque chose se prépare.
    let loading = app.os_loading.clone();
    if loading.is_some() {
        t = t.min(handover_start - 0.01);
    }
    let p = ease((t - handover_start) / (total - handover_start));

    // L'accueil arrive dessous, en fondu.
    if t > handover_start {
        let mut f = Frame { hits: Vec::new(), focus: app.focus, show_focus: true, pointer: None };
        home::draw_body(app, cv, &mut f, false);
        cv.rect(0.0, 0.0, w, h, INK.alpha(1.0 - p));
    } else {
        cv.clear(INK);
    }

    // La scène : symbole au centre, nom et signature dessous.
    let m = 160.0 * u;
    let cap = 26.0 * u;
    let word_w = 599.5 * cap / 68.66;
    let cx = w / 2.0;
    let mark_y = h * 0.5 - m * 0.62;
    let base_y = mark_y + m + 36.0 * u + cap;
    let tag_h = 20.0 * u;
    let (lit, flash) = if reduced { ((t / 0.25).clamp(0.0, 1.0), 0.0) } else { neon_at(t) };
    let named = if reduced { (t / 0.25).clamp(0.0, 1.0) } else { ((t - 0.8) / 0.3).clamp(0.0, 1.0) };

    // Le relais : le symbole et le nom filent vers l'en-tête de l'accueil.
    let (hx, hy, hh) = header_logo(app.layout);
    let s = hh / super::brand::SIGNED_H;
    let (to_mark, to_mx, to_my) = (super::brand::MARK * s, hx, hy);
    let (to_cap, to_wx, to_wy) = (68.66 * s, hx + super::brand::WORD_X * s, hy + super::brand::BASELINE * s);
    let lerp = |a: f32, b: f32| a + (b - a) * p;
    let mark_size = lerp(m, to_mark);
    let mx = lerp(cx - m / 2.0, to_mx);
    let my = lerp(mark_y, to_my);
    let wcap = lerp(cap, to_cap);
    let wx = lerp(cx - word_w / 2.0, to_wx);
    let wy = lerp(base_y, to_wy);

    // Le halo s'éteint pendant le vol (jamais de néon sur l'accueil au repos).
    let glow = 1.0 - p;
    cv.mark_lit(mx, my, mark_size, lit * glow + (1.0 - glow), flash * glow);
    if t > handover_start {
        // Sous le néon éteint, le symbole aux couleurs du logo.
        cv.mark(mx, my, mark_size, FROST, SPECTRE, None);
    }
    if named > 0.0 {
        cv.wordmark(wx, wy, wcap, FROST.alpha(named));
        let tag_alpha = named * (1.0 - p);
        if tag_alpha > 0.0 {
            let tw = 126.8 * tag_h / 36.4;
            cv.link_tag(cx - tw / 2.0, base_y + 22.0 * u, tag_h, tag_alpha);
        }
    }
    // Le chargement (OS), seulement s'il y a à attendre : la couture se remplit sous le nom.
    if let Some((label, progress)) = loading
        && named >= 1.0
    {
        let bw = 320.0 * u / 1.5;
        let bx = cx - bw / 2.0;
        let by = base_y + 22.0 * u + tag_h + 36.0 * u;
        let since = (app.screen_since.elapsed().as_secs_f32() - 1.2).max(0.0);
        let a = (since / 0.4).clamp(0.0, 1.0);
        cv.rrect(bx, by, bw, 3.0, 1.5, INK_3.alpha(a));
        let fill = progress.clamp(0.04, 1.0);
        cv.gradient(bx, by, bw * fill, 3.0, true, &[(0.0, FLAT.alpha(a)), (1.0, SPECTRE.alpha(a))]);
        let style = super::canvas::Text::new(super::canvas::Font::Mono, 14.0 * u / 1.5, FAINT.alpha(a)).tracking(1.1).align(super::canvas::Align::Center);
        cv.text(cx, by + 16.0 + 14.0 * u / 1.5, style, &label.to_uppercase());
    }
}
