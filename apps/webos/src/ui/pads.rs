//! Tester les manettes : la liste (joueur, nom, moteurs), et pour celle
//! choisie, ses touches, sticks et gâchettes en direct et une vibration
//! d'essai par moteur, aux deux, et aux gâchettes quand la manette en a.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, PadInfo, Rect, Target};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    widgets::label(cv, 96.0, 104.0, tr("Manettes", "Controllers"));
    cv.text(92.0, 176.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), tr("Tester les manettes", "Test controllers"));

    let mut y = 240.0;
    if app.pads.is_empty() {
        cv.rrect_dashed(96.0, y, 700.0, 200.0, 18.0, 1.5, BORDER);
        cv.paragraph(
            130.0,
            y + 64.0,
            Text::new(Font::Regular, 23.0, FROST_2),
            tr(
                "Aucune manette. Associez-la à la télé : Réglages › Général › Appareils › Manettes, ou en Bluetooth. Xbox, DualSense, DualShock 4 et Switch Pro marchent.",
                "No controller. Pair it with the TV: Settings › General › Devices › Gamepads, or over Bluetooth. Xbox, DualSense, DualShock 4 and Switch Pro work.",
            ),
            630.0,
            34.0,
        );
    }
    for (i, p) in app.pads.iter().enumerate().take(4) {
        let r = Rect::new(96.0, y, 700.0, 104.0);
        let focused = f.hit(Target::Pad(i), r);
        let selected = app.pad_selected == i;
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 18.0, 1.0);
            cv.rrect(r.x, r.y, r.w, r.h, 18.0, Rgba(36, 33, 68, 255));
        } else {
            cv.rrect(r.x, r.y, r.w, r.h, 18.0, if selected { SPECTRE.alpha(0.14) } else { PANEL });
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, if selected { SPECTRE } else { BORDER });
        }
        cv.rrect(r.x + 26.0, r.y + 25.0, 54.0, 54.0, 12.0, if selected || focused { FLAT } else { Rgba(227, 237, 240, 26) });
        cv.text(r.x + 53.0, r.y + 60.0, Text::new(Font::MonoMedium, 20.0, CORE).align(Align::Center), &format!("{}{}", tr("J", "P"), p.slot + 1));
        cv.text_fit(r.x + 104.0, r.y + 48.0, Text::new(Font::Semibold, 26.0, FROST), &p.name, 380.0);
        let mut detail = vec![];
        if p.trigger_rumble {
            detail.push(tr("gâchettes à vibration", "trigger rumble").to_string());
        }
        if let Some(b) = &p.battery {
            detail.push(b.clone());
        }
        cv.text_fit(r.x + 104.0, r.y + 78.0, Text::new(Font::Mono, 15.0, FROST_2), &detail.join(" · "), 380.0);
        let tag = if p.rumble { tr("2 moteurs", "2 motors") } else { tr("sans vibration", "no rumble") };
        let style = Text::new(Font::Mono, 15.0, if p.rumble { FROST } else { FROST_2 });
        let tw = cv.text_width(style, tag) + 28.0;
        let (tx, ty) = (r.x + r.w - 26.0 - tw, r.y + 34.0);
        if p.rumble {
            cv.rrect_stroke(tx, ty, tw, 36.0, 18.0, 1.0, SPECTRE);
        } else {
            cv.rrect_dashed(tx, ty, tw, 36.0, 18.0, 1.0, BORDER);
        }
        cv.text(tx + 14.0, ty + 24.0, style, tag);
        y += 120.0;
    }
    cv.paragraph(
        100.0,
        y.max(500.0) + 30.0,
        Text::new(Font::Regular, 19.0, FROST_3),
        tr(
            "Quatre joueurs au plus. Une manette Bluetooth vibre si la télé lui donne ses moteurs : sinon, la vibration d'essai ne fait rien.",
            "Four players at most. A Bluetooth controller rumbles if the TV exposes its motors: otherwise the test rumble does nothing.",
        ),
        690.0,
        28.0,
    );

    if let Some(p) = app.pads.get(app.pad_selected) {
        live(cv, f, p);
    }

    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Choisir", "Select")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

fn live(cv: &mut Canvas, f: &mut Frame, p: &PadInfo) {
    let (x, y, w, h) = (880.0, 240.0, 944.0, 740.0);
    cv.rrect(x, y, w, h, 26.0, PANEL);
    cv.rrect_stroke(x, y, w, h, 26.0, 1.0, BORDER);
    cv.text_fit(x + 44.0, y + 66.0, Text::new(Font::Title, 30.0, FROST), &format!("{}{} · {} {}", tr("J", "P"), p.slot + 1, p.name, tr("en direct", "live")), w - 340.0);
    cv.text(x + w - 44.0, y + 66.0, Text::new(Font::Mono, 15.0, FROST_3).align(Align::Right), tr("appuyez sur une touche", "press a button"));

    let held = |b: u32| p.buttons & (1 << b) != 0;
    // Sticks.
    let stick = |cv: &mut Canvas, cx: f32, cy: f32, ax: i16, ay: i16, pressed: bool, label: &str| {
        cv.circle_stroke(cx, cy, 75.0, 2.0, Rgba(227, 237, 240, 64));
        let (dx, dy) = (ax as f32 / 32768.0 * 60.0, ay as f32 / 32768.0 * 60.0);
        let active = pressed || dx.abs() > 6.0 || dy.abs() > 6.0;
        if active {
            cv.circle(cx + dx, cy + dy, 26.0, HALO.alpha(0.4));
        }
        cv.circle(cx + dx, cy + dy, 15.0, if active { SPECTRE } else { Rgba(227, 237, 240, 128) });
        cv.text(cx, cy + 112.0, Text::new(Font::Mono, 14.0, FROST_3).align(Align::Center), label);
    };
    let cy = y + 230.0;
    stick(cv, x + 180.0, cy, p.axes[0], p.axes[1], held(7), tr("stick gauche", "left stick"));
    stick(cv, x + w - 180.0, cy, p.axes[2], p.axes[3], held(8), tr("stick droit", "right stick"));
    // Boutons A B X Y.
    let face = [(3u32, 0.0f32, -52.0f32, "Y"), (2, -52.0, 0.0, "X"), (1, 52.0, 0.0, "B"), (0, 0.0, 52.0, "A")];
    let (fx, fy) = (x + w / 2.0 + 90.0, cy);
    for (b, dx, dy, l) in face {
        let on = held(b);
        if on {
            cv.circle(fx + dx, fy + dy, 26.0, SPECTRE);
        } else {
            cv.circle_stroke(fx + dx, fy + dy, 22.0, 2.0, Rgba(227, 237, 240, 77));
        }
        cv.text(fx + dx, fy + dy + 7.0, Text::new(Font::Semibold, 20.0, if on { INK } else { FROST }).align(Align::Center), l);
    }
    // Croix.
    let dpad = [(11u32, 0.0f32, -44.0f32), (12, 0.0, 44.0), (13, -44.0, 0.0), (14, 44.0, 0.0)];
    let (dx0, dy0) = (x + w / 2.0 - 90.0, cy);
    for (b, dx, dy) in dpad {
        let c = if held(b) { SPECTRE } else { Rgba(227, 237, 240, 51) };
        cv.rrect(dx0 + dx - 18.0, dy0 + dy - 18.0, 36.0, 36.0, 8.0, c);
    }
    // Épaules, Select, Start.
    let small = [(9u32, x + 180.0, "L1"), (10, x + w - 180.0, "R1"), (4, x + w / 2.0 - 60.0, "Select"), (6, x + w / 2.0 + 60.0, "Start")];
    for (b, sx, l) in small {
        let on = held(b);
        let style = Text::new(Font::Mono, 15.0, if on { INK } else { FROST_2 });
        let tw = cv.text_width(style, l) + 28.0;
        let yy = if b >= 9 { y + 100.0 } else { cy + 100.0 };
        cv.rrect(sx - tw / 2.0, yy, tw, 34.0, 17.0, if on { SPECTRE } else { Rgba(227, 237, 240, 26) });
        cv.text(sx, yy + 23.0, style.align(Align::Center), l);
    }
    // Gâchettes.
    let gy = y + 430.0;
    for (i, (l, v)) in [("L2", p.axes[4]), ("R2", p.axes[5])].iter().enumerate() {
        let gx = x + 44.0 + i as f32 * ((w - 88.0) / 2.0 + 14.0);
        let gw = (w - 88.0) / 2.0 - 14.0;
        let pct = (*v as f32 / 32767.0).clamp(0.0, 1.0);
        cv.text(gx, gy, Text::new(Font::Mono, 15.0, FROST_2), l);
        cv.text(gx + gw, gy, Text::new(Font::Mono, 15.0, FROST_2).align(Align::Right), &format!("{} %", (pct * 100.0).round()));
        cv.rrect(gx, gy + 14.0, gw, 12.0, 6.0, Rgba(227, 237, 240, 26));
        if pct > 0.01 {
            cv.rrect(gx, gy + 14.0, gw * pct, 12.0, 6.0, SPECTRE);
        }
    }
    cv.rect(x + 44.0, gy + 56.0, w - 88.0, 1.0, LINE);

    // Vibration d'essai.
    let vy = gy + 100.0;
    cv.text(x + 44.0, vy, Text::new(Font::Semibold, 22.0, FROST), tr("Vibration d'essai", "Test rumble"));
    let mut bx = x + 44.0;
    let labels = [tr("Gauche · grave", "Left · low"), tr("Droite · aiguë", "Right · high"), tr("Les deux", "Both"), tr("Gâchettes", "Triggers")];
    for (i, l) in labels.iter().enumerate() {
        if i == 3 && !p.trigger_rumble {
            continue;
        }
        bx += widgets::secondary(cv, f, Target::PadTest(i as u8), bx, vy + 20.0, l, 20.0) + 14.0;
    }
    let my = vy + 110.0;
    let gauge = |cv: &mut Canvas, gx: f32, l: &str, v: f32| {
        cv.text(gx, my + 6.0, Text::new(Font::Mono, 15.0, FROST_2), l);
        let lw = 150.0;
        cv.rrect(gx + lw, my - 4.0, 240.0, 8.0, 4.0, Rgba(227, 237, 240, 26));
        if v > 0.0 {
            cv.rrect(gx + lw, my - 4.0, 240.0 * v.clamp(0.0, 1.0), 8.0, 4.0, SPECTRE);
        }
    };
    gauge(cv, x + 44.0, tr("moteur gauche", "left motor"), p.motors.0);
    gauge(cv, x + 44.0 + 440.0, tr("moteur droit", "right motor"), p.motors.1);
    if !p.rumble {
        cv.text(x + 44.0, my + 50.0, Text::new(Font::Regular, 18.0, FROST_3), tr("La télé ne donne pas de moteur pour cette manette.", "The TV exposes no motor for this controller."));
    }
}
