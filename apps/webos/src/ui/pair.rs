//! Ajouter un ordinateur : le code à six chiffres présenté aux Stations du
//! réseau (accepté dans Admin › Appareils Link), ou l'adresse d'Admin tapée
//! au pavé, avec son empreinte à comparer.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{AddStep, App, Rect, Target};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    widgets::label(cv, 96.0, 104.0, tr("Ajouter un ordinateur", "Add a computer"));
    match &app.add.step {
        AddStep::Code => code(app, cv, f),
        AddStep::Address => address(app, cv, f),
        AddStep::Verify { address, port, fingerprint } => verify(cv, f, address, *port, fingerprint),
    }
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Choisir", "Select")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

fn title(cv: &mut Canvas, s: &str) {
    cv.text(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), s);
}

fn message(app: &App, cv: &mut Canvas, x: f32, y: f32, w: f32) {
    if let Some(m) = &app.add.message {
        cv.rrect(x, y, w, 64.0, 14.0, SPECTRE.alpha(0.08));
        cv.rrect_stroke(x, y, w, 64.0, 14.0, 1.0, SPECTRE.alpha(0.3));
        cv.icon(&Icon::Info, x + 20.0, y + 18.0, 28.0, 1.6, SPECTRE);
        cv.text_fit(x + 64.0, y + 41.0, Text::new(Font::Regular, 21.0, FROST), m, w - 84.0);
    }
}

fn code(app: &App, cv: &mut Canvas, f: &mut Frame) {
    title(cv, tr("Par code", "With a code"));
    // Le code, en grand.
    let (x, y, w, h) = (96.0, 260.0, 860.0, 300.0);
    cv.neon(x, y, w, h, 26.0, 0.55);
    cv.rrect(x, y, w, h, 26.0, PANEL_SOLID);
    widgets::label(cv, x + 44.0, y + 64.0, tr("Code de ce téléviseur", "This TV's code"));
    let code = &app.add.code;
    let spaced = if code.len() == 6 { format!("{} {}", &code[..3], &code[3..]) } else { code.clone() };
    cv.text(x + w / 2.0, y + 210.0, Text::new(Font::MonoMedium, 120.0, FROST).tracking(10.0).align(Align::Center), &spaced);
    cv.paragraph(
        96.0,
        y + h + 64.0,
        Text::new(Font::Regular, 24.0, FROST_2),
        tr(
            "Sur l'ordinateur, ouvrez KyberNow Admin › Appareils Link (ou l'icône près de l'horloge) et acceptez ce code. Le téléviseur reçoit un compte à lui, que vous pourrez retirer dans Admin.",
            "On the computer, open KyberNow Admin › Link devices (or the icon by the clock) and accept this code. The TV gets an account of its own, which you can remove in Admin.",
        ),
        860.0,
        36.0,
    );
    message(app, cv, 96.0, y + h + 190.0, 860.0);

    // À droite : les Stations trouvées et l'ajout par adresse.
    let rx = 1060.0;
    let rw = W - 96.0 - rx;
    widgets::label(cv, rx, 300.0, tr("Stations sur le réseau", "Stations on the network"));
    let mut yy = 330.0;
    let found = app.new_found();
    if found.is_empty() {
        cv.rrect_dashed(rx, yy, rw, 120.0, 18.0, 1.5, BORDER);
        cv.paragraph(
            rx + 28.0,
            yy + 50.0,
            Text::new(Font::Regular, 21.0, FROST_2),
            tr("Aucune pour l'instant. L'ordinateur est-il allumé, sur le même réseau ?", "None yet. Is the computer on, on the same network?"),
            rw - 56.0,
            30.0,
        );
        yy += 140.0;
    }
    for (i, fd) in found.iter().enumerate().take(4) {
        let r = Rect::new(rx, yy, rw, 104.0);
        let focused = f.hit(Target::AddFound(i), r);
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 18.0, 0.9);
            cv.rrect(r.x, r.y, r.w, r.h, 18.0, Rgba(30, 28, 56, 255));
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 2.0, SPECTRE);
        } else {
            cv.rrect(r.x, r.y, r.w, r.h, 18.0, PANEL);
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, BORDER);
        }
        cv.icon(&Icon::Monitor, r.x + 26.0, r.y + 32.0, 40.0, 1.6, FROST);
        cv.text_fit(r.x + 88.0, r.y + 46.0, Text::new(Font::Semibold, 26.0, FROST), &fd.name, rw - 300.0);
        cv.text(r.x + 88.0, r.y + 78.0, Text::new(Font::Mono, 15.0, FROST_2), &format!("{}:{}", fd.address, fd.port));
        let asked = app.add.asked.iter().any(|(_, a)| a == &fd.address);
        let state = if asked { tr("en attente d'Admin", "waiting for Admin") } else { tr("présenter le code", "show the code") };
        cv.text(r.x + rw - 26.0, r.y + 60.0, Text::new(Font::Mono, 15.0, if asked { SPECTRE } else { FROST_3 }).align(Align::Right), state);
        yy += 120.0;
    }
    yy += 20.0;
    widgets::label(cv, rx, yy + 20.0, tr("Ou bien", "Or"));
    widgets::secondary(cv, f, Target::AddByAddress, rx, yy + 44.0, tr("Par adresse et port", "With address and port"), 24.0);
}

const KEYS: [char; 12] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '0', ':'];

fn address(app: &App, cv: &mut Canvas, f: &mut Frame) {
    title(cv, tr("Par adresse", "With an address"));
    cv.paragraph(
        96.0,
        262.0,
        Text::new(Font::Regular, 24.0, FROST_2),
        tr(
            "L'adresse et le port d'Admin sont dans KyberNow Admin › Appareils Link, sous le QR code. Un clavier branché à la télé marche aussi.",
            "Admin's address and port are in KyberNow Admin › Link devices, under the QR code. A keyboard plugged into the TV works too.",
        ),
        820.0,
        36.0,
    );
    // Le champ.
    let (x, y, w) = (96.0, 380.0, 820.0);
    cv.rrect(x, y, w, 110.0, 18.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, 110.0, 18.0, 2.0, SPECTRE);
    let shown = if app.add.address.is_empty() { "192.168.1.20:24811" } else { app.add.address.as_str() };
    let color = if app.add.address.is_empty() { FROST_4 } else { FROST };
    let tw = cv.text(x + 36.0, y + 72.0, Text::new(Font::MonoMedium, 44.0, color), shown);
    if !app.add.address.is_empty() {
        cv.rect(x + 40.0 + tw, y + 28.0, 4.0, 56.0, SPECTRE);
    }
    message(app, cv, 96.0, 530.0, 820.0);

    // Le pavé.
    let (kx, ky, kw, kh, gap) = (1060.0, 260.0, 170.0, 120.0, 18.0);
    for (i, c) in KEYS.iter().enumerate() {
        let (col, rowi) = (i % 3, i / 3);
        let r = Rect::new(kx + col as f32 * (kw + gap), ky + rowi as f32 * (kh + gap), kw, kh);
        key(cv, f, Target::Key(*c), r, &c.to_string(), false);
    }
    let r = Rect::new(kx + 3.0 * (kw + gap), ky, kw, kh * 2.0 + gap);
    key(cv, f, Target::KeyErase, r, "⌫", false);
    let r = Rect::new(kx + 3.0 * (kw + gap), ky + 2.0 * (kh + gap), kw, kh * 2.0 + gap);
    key(cv, f, Target::KeyOk, r, "OK", true);
}

fn key(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, s: &str, primary: bool) {
    let focused = f.hit(t, r);
    let (bg, fg) = if focused {
        cv.neon(r.x, r.y, r.w, r.h, 18.0, 0.9);
        (CORE, INK)
    } else if primary {
        (FLAT, CORE)
    } else {
        (Rgba(14, 19, 28, 230), FROST)
    };
    cv.rrect(r.x, r.y, r.w, r.h, 18.0, bg);
    if !focused && !primary {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, BORDER);
    }
    if s == "⌫" {
        cv.icon(&Icon::Back, r.x + r.w / 2.0 - 20.0, r.y + r.h / 2.0 - 20.0, 40.0, 2.0, fg);
    } else {
        cv.text(r.x + r.w / 2.0, r.y + r.h / 2.0 + 16.0, Text::new(Font::MonoMedium, 44.0, fg).align(Align::Center), s);
    }
}

fn verify(cv: &mut Canvas, f: &mut Frame, address: &str, port: u16, fingerprint: &str) {
    title(cv, tr("Vérifier l'ordinateur", "Check the computer"));
    cv.paragraph(
        96.0,
        262.0,
        Text::new(Font::Regular, 24.0, FROST_2),
        tr(
            "Comparez cette empreinte avec celle de KyberNow Admin › Appareils Link. Si elles sont identiques, c'est bien votre ordinateur.",
            "Compare this fingerprint with the one in KyberNow Admin › Link devices. If they match, it really is your computer.",
        ),
        1200.0,
        36.0,
    );
    let (x, y, w, h) = (96.0, 380.0, 1200.0, 220.0);
    cv.rrect(x, y, w, h, 24.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 24.0, 1.0, BORDER);
    widgets::label(cv, x + 40.0, y + 58.0, &format!("{address}:{port}"));
    let groups: Vec<&str> = fingerprint.trim_start_matches("sha256:").split(':').take(4).collect();
    let shown = if groups.len() >= 4 { groups.join(" ").to_uppercase() } else { fingerprint.chars().take(24).collect::<String>().to_uppercase() };
    cv.text(x + 40.0, y + 160.0, Text::new(Font::MonoMedium, 64.0, FROST).tracking(4.0), &shown);
    let by = y + h + 60.0;
    let bw = widgets::primary(cv, f, Target::VerifyOk, 96.0, by, tr("C'est la même", "It matches"));
    widgets::secondary(cv, f, Target::VerifyNo, 96.0 + bw + 28.0, by + 10.0, tr("Annuler", "Cancel"), 26.0);
}
