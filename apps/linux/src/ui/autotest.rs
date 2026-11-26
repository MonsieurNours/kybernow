//! Le panneau du test du profil Auto (docs/link-ui.md, « Profil Auto et débit adaptatif »), par-dessus l'accueil ou
//! les réglages, au bureau comme à la manette : le titre et son sous-titre ; pendant le test, les quatre étapes (en
//! attente, la roue, cochée avec sa mesure, passée avec sa raison) et « Annuler » ; à la fin, « Réglages proposés »
//! (une ligne par choix : réglage, valeur, raison), « Annuler », « Appliquer » et « Refaire le test » ; l'échec et
//! « Réessayer ». Seuls ses boutons se choisissent.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow};
use super::{unit, Frame};
use crate::app::{App, Layout, Target};
use crate::autotest::{self as auto, State, StepState};

pub fn overlay(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(p) = &app.autotest else { return };
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let (w, h) = (cv.w, cv.h);
    f.hits.clear();
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
    let cw = (640.0 * u).min(w - 48.0);
    let pad = 32.0 * u;
    let iw = cw - 2.0 * pad;
    let title = Text::new(Font::Display, 26.0 * u, FROST).tracking(-0.3 * u);
    let sub = Text::new(Font::Regular, 15.0 * u, MUTED);
    let bh = if desktop { 44.0 } else { 52.0 * u };
    let size = if desktop { 14.0 } else { 17.0 * u };

    // La hauteur d'abord (le contenu dépend de l'état), puis le dessin.
    let sub_h = cv.line_count(sub, auto::subtitle(), iw) as f32 * 22.0 * u;
    let body_h = match &p.state {
        State::Running(steps) => steps.len() as f32 * 52.0 * u,
        State::Done { choices, .. } => 34.0 * u + choices.len() as f32 * 50.0 * u,
        State::Failed(e) => cv.line_count(sub, e, iw) as f32 * 22.0 * u + 8.0 * u,
    };
    let ch = (pad + 30.0 * u + 12.0 * u + sub_h + 20.0 * u + body_h + 20.0 * u + bh + pad).min(h - 24.0);
    let (x, y) = ((w - cw) / 2.0, ((h - ch) / 2.0).max(12.0));
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, LINE_2);
    let ix = x + pad;
    let mut yy = y + pad + 26.0 * u;
    // Le titre, avec les étincelles d'Auto.
    cv.icon(&Icon::Sparkles, ix, yy - 22.0 * u, 24.0 * u, 1.8 * u, SPECTRE);
    cv.text(ix + 32.0 * u, yy, title, auto::title());
    yy += 12.0 * u;
    yy += cv.paragraph(ix, yy + 16.0 * u, sub, auto::subtitle(), iw, 22.0 * u) + 20.0 * u;

    let right = x + cw - pad;
    let by = y + ch - pad - bh;
    match &p.state {
        State::Running(steps) => {
            for s in steps {
                step_row(app, cv, s, ix, yy, iw, u);
                yy += 52.0 * u;
            }
            // « Annuler » : le test s'arrête, le profil d'avant reste.
            let cancel = auto::cancel();
            let cw_ = button_w(cv, size, cancel);
            widgets::ghost(cv, f, Target::AutoCancel, right - cw_, by, bh, size, None, cancel, MUTED);
        }
        State::Done { choices, .. } => {
            eyebrow(cv, ix, yy + 12.0 * u, 11.0 * u, FAINT, auto::summary());
            yy += 34.0 * u;
            let col = (iw * 0.34).min(220.0 * u);
            for c in choices {
                cv.text_fit(ix, yy + 16.0 * u, Text::new(Font::Regular, 13.5 * u, FAINT), &c.title, col - 12.0 * u);
                cv.text_fit(ix + col, yy + 16.0 * u, Text::new(Font::Semibold, 15.0 * u, FROST), &c.value, iw - col);
                cv.text_fit(ix + col, yy + 36.0 * u, Text::new(Font::Regular, 13.0 * u, MUTED), &c.reason, iw - col);
                cv.rect(ix, yy + 46.0 * u, iw, 1.0, LINE.alpha(0.7));
                yy += 50.0 * u;
            }
            // « Refaire le test » à gauche ; « Annuler » puis « Appliquer » à droite.
            widgets::ghost(cv, f, Target::AutoAgain, ix, by, bh, size, Some(Icon::Refresh), auto::again(), MUTED);
            let apply_w = cv.text_width(Text::new(Font::Semibold, bh * 0.32, FROST), auto::apply()) + 2.0 * bh * 0.58;
            widgets::primary(cv, f, Target::AutoApply, right - apply_w, by, bh, auto::apply(), false);
            let cancel = auto::cancel();
            let cw_ = button_w(cv, size, cancel);
            widgets::ghost(cv, f, Target::AutoCancel, right - apply_w - 12.0 * u - cw_, by, bh, size, None, cancel, MUTED);
        }
        State::Failed(e) => {
            cv.paragraph(ix, yy + 16.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), e, iw, 22.0 * u);
            let retry_w = cv.text_width(Text::new(Font::Semibold, bh * 0.32, FROST), auto::retry()) + 2.0 * bh * 0.58;
            widgets::primary(cv, f, Target::AutoRetry, right - retry_w, by, bh, auto::retry(), false);
            let cancel = auto::cancel();
            let cw_ = button_w(cv, size, cancel);
            widgets::ghost(cv, f, Target::AutoCancel, right - retry_w - 12.0 * u - cw_, by, bh, size, None, cancel, MUTED);
        }
    }
    // À la manette : la légende des deux boutons.
    if !desktop {
        // Par-dessus la légende de l'écran du dessous.
        cv.rect(0.0, h - 56.0 * u, w, 56.0 * u, Rgba(5, 7, 10, 250));
        cv.rect(0.0, h - 56.0 * u, w, 1.0, LINE);
        let gy = h - 28.0 * u;
        let mut lx = 40.0 * u;
        lx += widgets::legend(cv, lx, gy, 26.0 * u, &["A"], crate::i18n::tr("Choisir", "Choose")) + 28.0 * u;
        widgets::legend(cv, lx, gy, 26.0 * u, &["B"], auto::cancel());
    }
}

fn button_w(cv: &mut Canvas, size: f32, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, size, FROST), s) + size * 2.0
}

/// Une étape : son état à gauche (cercle vide, roue, coche, tiret), son titre, sa mesure ou sa raison à droite.
fn step_row(app: &App, cv: &mut Canvas, s: &auto::Step, x: f32, y: f32, w: f32, u: f32) {
    let cy = y + 22.0 * u;
    let r = 9.0 * u;
    let cx = x + r;
    let (title_color, detail, detail_color) = match &s.state {
        StepState::Waiting => {
            cv.circle_stroke(cx, cy, r, 1.5 * u, GHOST);
            (MUTED, None, FAINT)
        }
        StepState::Running => {
            let t = app.screen_since.elapsed().as_secs_f32();
            let a0 = if app.reduce_motion { 0.0 } else { t * 4.0 };
            cv.circle_stroke(cx, cy, r, 1.5 * u, LINE_2);
            let pts: Vec<(f32, f32)> = (0..=16).map(|i| a0 + i as f32 * 0.28).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect();
            cv.polyline(&pts, 2.0 * u, SPECTRE);
            (FROST, None, FAINT)
        }
        StepState::Done(m) => {
            cv.circle(cx, cy, r + 1.0 * u, OK.alpha(0.18));
            cv.icon(&Icon::Check, cx - 7.0 * u, cy - 7.0 * u, 14.0 * u, 2.0 * u, OK);
            (FROST, Some(m.as_str()), MUTED)
        }
        StepState::Skipped(why) => {
            cv.circle_stroke(cx, cy, r, 1.5 * u, WARN.alpha(0.7));
            cv.icon(&Icon::Minus, cx - 6.0 * u, cy - 6.5 * u, 12.0 * u, 2.0 * u, WARN);
            (MUTED, Some(why.as_str()), WARN)
        }
    };
    let tx = x + 2.0 * r + 14.0 * u;
    let title = Text::new(Font::Regular, 15.5 * u, title_color);
    let tw = cv.text_width(title, &s.title).min(w * 0.45);
    cv.text_fit(tx, cy + 5.5 * u, title, &s.title, w * 0.45);
    if let Some(d) = detail {
        let style = Text::new(Font::Mono, 12.5 * u, detail_color).align(Align::Right);
        cv.text_fit(x + w, cy + 5.0 * u, style, d, (x + w) - (tx + tw + 16.0 * u));
    }
    cv.rect(x, y + 46.0 * u, w, 1.0, LINE.alpha(0.5));
}

/// La hauteur du bandeau « Autre réseau que celui du test », en points.
pub const BANNER_H: f32 = 62.0;
