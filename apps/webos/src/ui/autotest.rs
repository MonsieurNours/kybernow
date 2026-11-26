//! Le panneau du test du profil Auto (docs/link-ui.md, « Panneau du test ») : « Profil Auto », sa phrase, les quatre
//! étapes (en attente, en cours avec une roue, cochée avec sa mesure, passée avec sa raison) et « Annuler » ; puis
//! « Réglages proposés » (une ligne par réglage : valeur, raison), « Appliquer », « Annuler », « Refaire le test » ;
//! ou l'échec et « Réessayer ». Tout se fait à la croix et OK ; Retour vaut « Annuler ».

use std::time::Instant;

use super::canvas::{Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, Target};
use crate::autotest::{self, AutoPhase, StepState};

/// L'horloge de la roue (elle tourne d'elle-même, l'écran se redessine pendant le test).
fn spin() -> f32 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f32()
}

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    let Some(a) = &app.auto else { return };
    // La couture de la charte, discrète, comme à la connexion.
    cv.line(1180.0, 1080.0, 1920.0, 340.0, 3.0, SPECTRE.alpha(0.25));

    let x0 = 120.0;
    cv.text(x0, 120.0, Text::new(Font::Mono, 16.0, FROST_3).tracking(3.0), &a.name.to_uppercase());
    let tw = cv.text(x0 - 4.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), autotest::title());
    cv.icon(&Icon::Sparkles, x0 + tw + 20.0, 146.0, 52.0, 2.0, SPECTRE);
    cv.text_fit(x0, 248.0, Text::new(Font::Regular, 26.0, FROST_2), autotest::subtitle(), W - 2.0 * x0);

    // Les étapes : en colonne à gauche (seule pendant le test).
    let done = matches!(a.phase, AutoPhase::Done { .. });
    let sw = if done { 700.0 } else { 1100.0 };
    let mut y = 316.0;
    for step in &a.steps {
        step_row(cv, x0, y, sw, step);
        y += 112.0;
    }

    match &a.phase {
        AutoPhase::Running => {
            widgets::secondary(cv, f, Target::AutoCancel, x0, y + 30.0, autotest::cancel_word(), 26.0);
        }
        AutoPhase::Failed(text) => {
            cv.paragraph(x0, y + 40.0, Text::new(Font::Regular, 26.0, DANGER), text, sw, 36.0);
            let bx = x0 + widgets::primary(cv, f, Target::AutoRetry, x0, y + 90.0, autotest::retry_word()) + 24.0;
            widgets::secondary(cv, f, Target::AutoCancel, bx, y + 102.0, autotest::cancel_word(), 24.0);
        }
        AutoPhase::Done { choices, .. } => {
            // « Réglages proposés », à droite : une ligne par réglage, la valeur puis la raison.
            let (cx, cw) = (x0 + sw + 60.0, W - x0 - (x0 + sw + 60.0));
            let top = 300.0;
            let row = 78.0;
            let ch = 76.0 + row * choices.len() as f32 + 20.0;
            cv.rrect(cx, top, cw, ch, 22.0, PANEL_SOLID);
            cv.rrect_stroke(cx, top, cw, ch, 22.0, 1.0, LINE);
            widgets::label(cv, cx + 40.0, top + 54.0, autotest::summary_title());
            let mut yy = top + 76.0;
            for c in choices {
                cv.line(cx + 40.0, yy, cx + cw - 40.0, yy, 1.0, LINE);
                cv.text_fit(cx + 40.0, yy + 34.0, Text::new(Font::Regular, 20.0, FROST_3), &c.title, 300.0);
                cv.text_fit(cx + 360.0, yy + 34.0, Text::new(Font::Semibold, 26.0, FROST), &c.value, cw - 400.0);
                cv.text_fit(cx + 360.0, yy + 63.0, Text::new(Font::Regular, 20.0, FROST_2), &c.reason, cw - 400.0);
                yy += row;
            }
            // Les boutons, sous le résumé.
            let by = (top + ch + 36.0).min(H - 190.0);
            let mut bx = cx;
            bx += widgets::primary(cv, f, Target::AutoApply, bx, by, autotest::apply_word()) + 24.0;
            bx += widgets::secondary(cv, f, Target::AutoCancel, bx, by + 12.0, autotest::cancel_word(), 24.0) + 24.0;
            widgets::secondary(cv, f, Target::AutoAgain, bx, by + 12.0, autotest::again_word(), 24.0);
        }
    }

    // Aides.
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, crate::i18n::tr("Choisir", "Select")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, autotest::cancel_word());
}

/// Une étape : son état à gauche (cercle vide, roue, coche, flèche passée), son titre, sa mesure ou sa raison dessous.
fn step_row(cv: &mut Canvas, x: f32, y: f32, w: f32, step: &autotest::Step) {
    let h = 96.0;
    let running = step.state == StepState::Running;
    cv.rrect(x, y, w, h, 18.0, if running { Rgba(30, 28, 56, 255) } else { PANEL });
    cv.rrect_stroke(x, y, w, h, 18.0, if running { 1.5 } else { 1.0 }, if running { SPECTRE } else { LINE });
    let (cx, cy) = (x + 52.0, y + h / 2.0);
    match &step.state {
        StepState::Waiting => cv.circle_stroke(cx, cy, 16.0, 2.0, FROST_4),
        StepState::Running => {
            // La roue : un arc qui tourne, fait de points (tiny-skia n'a pas d'arc simple ici).
            cv.circle_stroke(cx, cy, 16.0, 2.0, Rgba(227, 237, 240, 30));
            let t = spin() * 5.0;
            for i in 0..10 {
                let ang = t + i as f32 * 0.16;
                cv.circle(cx + 16.0 * ang.cos(), cy + 16.0 * ang.sin(), 2.4, SPECTRE.alpha(0.3 + 0.07 * i as f32));
            }
        }
        StepState::Done(_) => {
            cv.circle(cx, cy, 18.0, SPECTRE.alpha(0.22));
            cv.icon(&Icon::Check, cx - 13.0, cy - 13.0, 26.0, 2.2, SPECTRE);
        }
        StepState::Skipped(_) => {
            cv.circle_stroke(cx, cy, 16.0, 2.0, FROST_3);
            cv.line(cx - 8.0, cy, cx + 8.0, cy, 2.2, FROST_3);
        }
    }
    let tx = x + 96.0;
    let (sub, sub_color) = match &step.state {
        StepState::Done(t) => (Some(t.as_str()), FROST_2),
        StepState::Skipped(t) => (Some(t.as_str()), FROST_3),
        _ => (None, FROST_3),
    };
    let title_color = if step.state == StepState::Waiting { FROST_3 } else { FROST };
    match sub {
        Some(s) => {
            cv.text_fit(tx, y + 42.0, Text::new(Font::Semibold, 26.0, title_color), &step.title, w - 130.0);
            cv.text_fit(tx, y + 76.0, Text::new(Font::Mono, 19.0, sub_color), s, w - 130.0);
        }
        None => {
            cv.text_fit(tx, y + 58.0, Text::new(Font::Semibold, 26.0, title_color), &step.title, w - 130.0);
        }
    }
}
