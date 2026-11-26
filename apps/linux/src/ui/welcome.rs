//! Accueil et compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow » ; maquettes
//! https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru), avec les éléments de l'app : l'accueil en trois écrans (au
//! salon, l'écran 2 montre le code de la connexion par code dans une carte à droite), le bouton « Connecter un compte »
//! en haut de l'accueil, la carte du compte en fin de rangée des ordinateurs, le bandeau après un appairage et la
//! fenêtre « Hors de chez vous ? ».

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, primary, ring};
use super::{launch, title_bar, unit, Frame};
use crate::app::{App, Layout, Rect, Target};
use crate::i18n::tr;
use crate::welcome::{self as w, connect_label, create_label};

/// Le violet clair des étiquettes sur les cartes du compte (maquettes : #D4CEFF).
const LILAC: Rgba = Rgba::hex(0xD4CEFF);
/// Le fond des cartes du compte (maquettes : #1B1640 vers #0E1020).
const VIOLET_INK: Rgba = Rgba::hex(0x15122F);

// MARK: - Accueil en trois écrans

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (wd, ht) = (cv.w, cv.h);
    let console = app.layout.console();
    let tv = app.layout == Layout::Tv;
    let u = unit(app.layout);
    let step = app.welcome.step.clamp(1, 3);
    cv.clear(INK_BG);
    match step {
        1 => cv.glow(wd * 0.78, ht * 0.3, wd * 0.7, ht * 0.7, Rgba::hex(0x5B48D8).alpha(0.32)),
        2 => cv.glow(wd * 0.85, ht * 0.1, wd * 0.7, ht * 0.6, Rgba::hex(0x5B48D8).alpha(0.26)),
        _ => cv.glow(wd * 0.2, ht, wd * 0.6, ht * 0.6, Rgba::hex(0x5B48D8).alpha(0.24)),
    }
    let x0 = if tv { 96.0 } else if console { 40.0 } else { (wd * 0.05).clamp(40.0, 72.0) };
    let top = if console {
        let (lx, ly, lh) = launch::header_logo(app.layout);
        cv.logo_link(lx, ly, lh, 1.0);
        56.0 * u + if tv { 12.0 } else { 0.0 }
    } else {
        title_bar(app, cv, f)
    };
    // « Passer » (écran 3 : « Plus tard »), en haut à droite.
    let skip = if step == 3 { tr("Plus tard", "Later") } else { tr("Passer", "Skip") };
    let (sh, ss) = if console { (40.0 * u, 15.0 * u) } else { (34.0, 14.0) };
    let sw = ghost_w(cv, ss, skip);
    let sy = if console { (top - sh) / 2.0 } else { top + 16.0 };
    widgets::ghost(cv, f, Target::WelcomeSkip, wd - x0 - sw, sy, sh, ss, None, skip, MUTED);
    let footer = if console { 56.0 * u + if tv { 4.0 } else { 0.0 } } else { 0.0 };
    let bottom = ht - footer - if console { 28.0 * u } else { 36.0 };
    // Les trois traits d'avancement, en bas à gauche.
    let mut tx = x0;
    for i in 1..=3u8 {
        let tw = if i == step { 26.0 } else { 10.0 } * u;
        cv.rrect(tx, bottom - 4.0 * u, tw, 4.0 * u, 2.0 * u, if i == step { SPECTRE } else { LINE_2 });
        tx += tw + 8.0 * u;
    }
    let y0 = if console { top + 24.0 * u } else { top + 64.0 };
    let area = Rect::new(x0, y0, wd - 2.0 * x0, bottom - 24.0 * u - y0);
    match step {
        1 => one(app, cv, f, area, u),
        2 if app.sign_in_by_code() => two_by_code(app, cv, f, area, u),
        2 => two(app, cv, f, area),
        _ => three(app, cv, f, area, u),
    }
    if console {
        cv.rect(0.0, ht - footer, wd, footer, Rgba(5, 7, 10, 178));
        cv.rect(0.0, ht - footer, wd, 1.0, LINE);
        let gy = ht - footer / 2.0;
        let g = 26.0 * u;
        let x = x0 + legend(cv, x0, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
        if step > 1 {
            legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
        }
    }
}

fn ghost_w(cv: &mut Canvas, size: f32, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, size, MUTED), s) + size * 2.0
}

/// L'étiquette « BIENVENUE · n / 3 » et le grand titre en capitales ; rend la hauteur prise.
fn heading(cv: &mut Canvas, x: f32, y: f32, size: f32, step: u8, max: f32, u: f32) -> f32 {
    eyebrow(cv, x, y + 12.0 * u, 12.0 * u, SPECTRE, &w::step_label(step));
    let style = Text::new(Font::Display, size, FROST).tracking(-size * 0.015);
    let lh = size * 1.02;
    let h = cv.paragraph(x - 2.0, y + 30.0 * u + size * 0.82, style, &w::title(step).to_uppercase(), max, lh);
    30.0 * u + h.max(lh) + 4.0 * u
}

/// Écran 1 : le titre, le texte, « Continuer » ; à droite, un ordinateur relié à d'autres appareils.
fn one(app: &App, cv: &mut Canvas, f: &mut Frame, a: Rect, u: f32) {
    let console = app.layout.console();
    let wide = a.w >= 960.0 * u;
    let max = if wide { (a.w * 0.55).min(640.0 * u) } else { a.w };
    let size = if console { 56.0 * u } else if a.h < 420.0 { 44.0 } else { 56.0 };
    let mut y = a.y + if a.h > 460.0 * u { 40.0 * u } else { 0.0 };
    y += heading(cv, a.x, y, size, 1, max, u) + 16.0 * u;
    let body = Text::new(Font::Regular, 18.0 * u, MUTED);
    y += cv.paragraph(a.x, y + 18.0 * u, body, w::text(1), max.min(560.0 * u), 27.0 * u) + 28.0 * u;
    let bh = if console { 60.0 * u } else { 52.0 };
    primary(cv, f, Target::WelcomeNext, a.x, y, bh, tr("Continuer", "Continue"), false);
    if wide {
        devices(cv, a.x + a.w - 380.0 * u, a.y + 30.0 * u, u);
    }
}

/// Le dessin de l'écran 1 (maquette) : un ordinateur, un portable, un téléphone, une tablette, reliés en pointillés.
fn devices(cv: &mut Canvas, x: f32, y: f32, u: f32) {
    let s = u;
    cv.rrect_stroke(x + 100.0 * s, y + 20.0 * s, 200.0 * s, 130.0 * s, 14.0 * s, 2.5 * s, SPECTRE);
    cv.line(x + 165.0 * s, y + 190.0 * s, x + 235.0 * s, y + 190.0 * s, 2.5 * s, SPECTRE);
    cv.line(x + 200.0 * s, y + 150.0 * s, x + 200.0 * s, y + 190.0 * s, 2.5 * s, SPECTRE);
    cv.rrect_stroke(x, y + 230.0 * s, 140.0 * s, 88.0 * s, 12.0 * s, 2.0 * s, GHOST);
    cv.rrect_stroke(x + 280.0 * s, y + 214.0 * s, 58.0 * s, 104.0 * s, 12.0 * s, 2.0 * s, GHOST);
    cv.rrect_stroke(x + 348.0 * s, y + 246.0 * s, 32.0 * s, 72.0 * s, 8.0 * s, 2.0 * s, GHOST);
    // Les liens en pointillés : des points le long de deux courbes.
    let dots = |cv: &mut Canvas, p: [(f32, f32); 3]| {
        for i in 0..=12 {
            let t = i as f32 / 12.0;
            let (a, b, c) = (p[0], p[1], p[2]);
            let px = (1.0 - t).powi(2) * a.0 + 2.0 * (1.0 - t) * t * b.0 + t * t * c.0;
            let py = (1.0 - t).powi(2) * a.1 + 2.0 * (1.0 - t) * t * b.1 + t * t * c.1;
            cv.circle(x + px * s, y + py * s, 1.6 * s, SPECTRE.alpha(0.8));
        }
    };
    dots(cv, [(100.0, 90.0), (40.0, 130.0), (70.0, 226.0)]);
    dots(cv, [(300.0, 90.0), (360.0, 140.0), (316.0, 210.0)]);
}

/// Écran 2, au bureau : le titre, le texte, les quatre avantages, « Connecter un compte », « Créer un compte »,
/// « Continuer sans compte », l'aide.
fn two(_app: &App, cv: &mut Canvas, f: &mut Frame, a: Rect) {
    let compact = a.h < 470.0;
    let size = if compact { 38.0 } else { 50.0 };
    let mut y = a.y;
    y += heading(cv, a.x, y, size, 2, a.w.min(820.0), 1.0) + 6.0;
    y += cv.paragraph(a.x, y + 17.0, Text::new(Font::Regular, 17.0, MUTED), w::text(2), a.w.min(700.0), 24.0) + if compact { 16.0 } else { 26.0 };
    // Les avantages : quatre cartes sur une ligne (deux par ligne dans une fenêtre étroite).
    let cols = if a.w >= 860.0 { 4 } else { 2 };
    let gap = 14.0;
    let cw = (a.w - gap * (cols - 1) as f32) / cols as f32;
    let icons = [Icon::Globe, Icon::Devices, Icon::People, Icon::Sliders];
    let body = Text::new(Font::Regular, 14.0, MUTED);
    let pad = 16.0;
    let ch = w::perks().iter().map(|(_, t)| cv.line_count(body, t, cw - 2.0 * pad) as f32 * 20.0).fold(0.0, f32::max) + if compact { 72.0 } else { 84.0 };
    for (i, ((label, text), icon)) in w::perks().iter().zip(icons).enumerate() {
        let cx = a.x + (i % cols) as f32 * (cw + gap);
        let cy = y + (i / cols) as f32 * (ch + gap);
        cv.rrect(cx, cy, cw, ch, 12.0, INK_2);
        cv.rrect_stroke(cx, cy, cw, ch, 12.0, 1.0, LINE);
        let mut yy = cy + pad;
        if !compact {
            cv.icon(&icon, cx + pad, yy, 22.0, 1.7, SPECTRE);
            yy += 34.0;
        } else {
            cv.icon(&icon, cx + cw - pad - 18.0, yy - 2.0, 18.0, 1.7, SPECTRE);
        }
        eyebrow(cv, cx + pad, yy + 9.0, 10.5, FAINT, label);
        yy += 22.0;
        cv.paragraph(cx + pad, yy + 12.0, body, text, cw - 2.0 * pad, 20.0);
    }
    let rows = (4 / cols) as f32;
    y += rows * ch + (rows - 1.0) * gap + if compact { 18.0 } else { 28.0 };
    let bw = primary(cv, f, Target::WelcomeSignIn, a.x, y, 52.0, connect_label(), false);
    let gx = a.x + bw + 14.0;
    let gw = widgets::ghost(cv, f, Target::WelcomeSignUp, gx, y + 4.0, 44.0, 15.0, None, create_label(), FROST);
    link(cv, f, Target::WelcomeNoAccount, gx + gw + 22.0, y + 4.0, 44.0, 15.0, tr("Continuer sans compte", "Continue without an account"));
    y += 52.0 + 22.0;
    cv.text_fit(a.x, y, Text::new(Font::Regular, 13.0, FAINT), &w::relay_help(crate::model::DEFAULT_RELAY), a.w);
}

/// Un lien (texte Spectre, sans cadre) ; rend sa largeur.
fn link(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, size: f32, s: &str) -> f32 {
    let style = Text::new(Font::Semibold, size, SPECTRE);
    let tw = cv.text_width(style, s);
    let r = Rect::new(x - 8.0, y, tw + 16.0, h);
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 8.0);
    }
    cv.text(x, y + h / 2.0 + size * 0.36, style.color(if hover { LILAC } else { SPECTRE }), s);
    if hover {
        cv.rect(x, y + h / 2.0 + size * 0.6, tw, 1.0, LILAC);
    }
    tw + 16.0
}

/// Écran 2 au salon (Deck, téléviseur, Link OS) : à gauche le titre, les avantages et « Continuer sans compte » ; à
/// droite la carte « CONNECTER UN COMPTE » : le QR code, le code, les étapes, l'attente (ou le refus et
/// « Nouveau code »).
fn two_by_code(app: &App, cv: &mut Canvas, f: &mut Frame, a: Rect, u: f32) {
    let card_w = (a.w * 0.36).clamp(300.0 * u, 440.0 * u);
    let left = a.w - card_w - 48.0 * u;
    let mut y = a.y;
    y += heading(cv, a.x, y, 44.0 * u, 2, left, u) + 4.0 * u;
    y += cv.paragraph(a.x, y + 17.0 * u, Text::new(Font::Regular, 17.0 * u, MUTED), w::text(2), left, 24.0 * u) + 18.0 * u;
    let icons = [Icon::Globe, Icon::Devices, Icon::People, Icon::Sliders];
    for ((label, text), icon) in w::perks().iter().zip(icons) {
        cv.icon(&icon, a.x, y + 2.0 * u, 24.0 * u, 1.7, SPECTRE);
        let tx = a.x + 40.0 * u;
        eyebrow(cv, tx, y + 11.0 * u, 11.0 * u, FAINT, label);
        cv.text_shrink(tx, y + 33.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), text, left - 40.0 * u);
        y += 52.0 * u;
    }
    y += 14.0 * u;
    primary(cv, f, Target::WelcomeNoAccount, a.x, y, 60.0 * u, tr("Continuer sans compte", "Continue without an account"), false);
    code_card(app, cv, f, Rect::new(a.x + a.w - card_w, a.y, card_w, a.h), u);
}

/// La carte du code (écran 2 au salon).
fn code_card(app: &App, cv: &mut Canvas, f: &mut Frame, r: Rect, u: f32) {
    let fl = &app.accounts.flow;
    let pad = 24.0 * u;
    let cx = r.x + r.w / 2.0;
    let small = Text::new(Font::Regular, 14.0 * u, MUTED).align(Align::Center);
    let lh = 20.0 * u;
    let inner = r.w - 2.0 * pad;
    // La hauteur d'abord, pour centrer le contenu.
    let failed = fl.denied || fl.message.is_some();
    let url_label = fl.code.as_ref().map(|c| c.url_label.clone()).unwrap_or_else(|| format!("{}/tv", crate::model::relay_label(&fl.relay)));
    let steps = [tr("Scannez le QR code avec votre téléphone,", "Scan the QR code with your phone,").to_string(), crate::accounts::open_step(&url_label), tr("puis touchez « Connecter ».", "then tap “Connect”.").to_string()];
    let side = (r.h * 0.36).min(inner * 0.62).min(220.0 * u);
    let h = (pad + 18.0 * u + 16.0 * u + side + 18.0 * u + 46.0 * u + 3.0 * lh + 8.0 * u + lh + 10.0 * u + lh + pad).min(r.h);
    let y0 = r.y;
    widgets::panel(cv, Rect::new(r.x, y0, r.w, h), 18.0 * u);
    cv.rrect(r.x, y0, r.w, h, 18.0 * u, INK_2.alpha(0.9));
    cv.rrect_stroke(r.x, y0, r.w, h, 18.0 * u, 1.0, LINE_2);
    let mut y = y0 + pad;
    let label = tr("Connecter un compte", "Connect an account").to_uppercase();
    let lw = cv.text_width(Text::new(Font::Mono, 12.0 * u, MUTED).tracking(12.0 * u * 0.08), &label);
    eyebrow(cv, cx - lw / 2.0, y + 12.0 * u, 12.0 * u, MUTED, &label);
    y += 18.0 * u + 16.0 * u;
    match (&fl.code, failed) {
        (Some(c), false) => {
            if let Some(card) = widgets::qr_card(cv, &c.url_complete, cx - side / 2.0, y, side) {
                y += card;
            }
            y += 18.0 * u;
            cv.text(cx, y + 34.0 * u, Text::new(Font::MonoMedium, 34.0 * u, FROST).tracking(3.0 * u).align(Align::Center), &c.code);
            y += 46.0 * u;
        }
        (_, false) => {
            // Le code se demande.
            cv.rrect(cx - side / 2.0, y, side, side, 8.0, INK_3);
            y += side + 18.0 * u;
            cv.text(cx, y + 34.0 * u, Text::new(Font::MonoMedium, 34.0 * u, GHOST).tracking(3.0 * u).align(Align::Center), "····-····");
            y += 46.0 * u;
        }
        (_, true) => {
            let m = if fl.denied { tr("Connexion refusée sur le téléphone.", "Sign-in declined on the phone.").to_string() } else { fl.message.clone().unwrap_or_default() };
            y += cv.paragraph(cx, y + 18.0 * u, Text::new(Font::Regular, 16.0 * u, WARN).align(Align::Center), &m, inner, 22.0 * u) + 16.0 * u;
            let label = tr("Nouveau code", "New code");
            let bw = cv.text_width(Text::new(Font::Semibold, 52.0 * u * 0.32, FROST), label) + 2.0 * 52.0 * u * 0.58;
            primary(cv, f, Target::AccountNewCode, cx - bw / 2.0, y, 52.0 * u, label, false);
            return;
        }
    }
    for s in &steps {
        cv.text_fit(cx, y + 14.0 * u, small, s, inner);
        y += lh;
    }
    y += 8.0 * u;
    cv.text_fit(cx, y + 14.0 * u, small.color(FROST), w::no_account_yet(), inner);
    y += lh + 10.0 * u;
    if let Some(c) = &fl.code {
        let n = c.minutes_left(crate::model::now_secs());
        cv.text_fit(cx, y + 14.0 * u, small.color(FAINT), &crate::accounts::waiting_line(n.max(1)), inner);
    }
}

/// Écran 3 : le titre, le texte, « ORDINATEURS À PORTÉE » : les ordinateurs du réseau (« Ajouter »), « UN CODE »
/// (« Rejoindre avec un code »), « PAS ENCORE INSTALLÉ » (le lien de la Station).
fn three(app: &App, cv: &mut Canvas, f: &mut Frame, a: Rect, u: f32) {
    let console = app.layout.console();
    let compact = a.h < 470.0 * u;
    let size = if compact { 40.0 * u } else { 52.0 * u };
    let mut y = a.y;
    let max = a.w.min(760.0 * u);
    y += heading(cv, a.x, y, size, 3, max, u) + 8.0 * u;
    cv.paragraph(a.x, y + 18.0 * u, Text::new(Font::Regular, 17.0 * u, MUTED), w::text(3), max.min(680.0 * u), 25.0 * u);
    // La rangée, calée en bas de la zone.
    let ch = if compact { 140.0 * u } else { 164.0 * u };
    let cy = a.y + a.h - ch;
    eyebrow(cv, a.x, cy - 16.0 * u, 12.0 * u, MUTED, tr("Ordinateurs à portée", "Computers in reach"));
    let found = app.new_found();
    let shown = found.len().min(if a.w >= 1100.0 * u { 2 } else { 1 });
    let n = shown + 2;
    let gap = 16.0 * u;
    let cw = (a.w - gap * (n - 1) as f32) / n as f32;
    let pad = 18.0 * u;
    let body = Text::new(Font::Regular, 15.0 * u, MUTED);
    let bh = 38.0 * u;
    let bsize = 14.0 * u;
    let mut x = a.x;
    for (i, fd) in found.iter().take(shown).enumerate() {
        glow_card(cv, Rect::new(x, cy, cw, ch), u);
        cv.icon(&Icon::Wifi, x + pad, cy + pad - 1.0 * u, 15.0 * u, 1.8, MUTED);
        eyebrow(cv, x + pad + 22.0 * u, cy + pad + 11.0 * u, 11.0 * u, MUTED, tr("Sur le réseau", "On the network"));
        cv.text_fit(x + pad, cy + pad + 44.0 * u, Text::new(Font::Title, 22.0 * u, FROST), &fd.name, cw - 2.0 * pad);
        widgets::ghost(cv, f, Target::WelcomeFound(i), x + pad, cy + ch - pad - bh, bh, bsize, Some(Icon::Plus), tr("Ajouter", "Add"), FROST);
        x += cw + gap;
    }
    // « UN CODE ».
    cv.rrect(x, cy, cw, ch, 14.0 * u, INK_2);
    cv.rrect_stroke(x, cy, cw, ch, 14.0 * u, 1.0, LINE);
    eyebrow(cv, x + pad, cy + pad + 11.0 * u, 11.0 * u, FAINT, tr("Un code", "A code"));
    cv.paragraph(x + pad, cy + pad + 38.0 * u, body, tr("Un ami vous ouvre son ordinateur : entrez le code qu'il vous a donné.", "A friend opens their computer to you: enter the code they gave you."), cw - 2.0 * pad, 21.0 * u);
    let jl = tr("Rejoindre avec un code", "Join with a code");
    widgets::ghost(cv, f, Target::WelcomeJoin, x + pad, cy + ch - pad - bh, bh, bsize, Some(Icon::Share), jl, FROST);
    x += cw + gap;
    // « PAS ENCORE INSTALLÉ ».
    cv.rrect(x, cy, cw, ch, 14.0 * u, INK_2);
    cv.rrect_dashed(x, cy, cw, ch, 14.0 * u, 1.0, LINE_2);
    eyebrow(cv, x + pad, cy + pad + 11.0 * u, 11.0 * u, FAINT, tr("Pas encore installé", "Not installed yet"));
    cv.paragraph(x + pad, cy + pad + 38.0 * u, body, tr("Téléchargez KyberNow Station sur l'ordinateur à rejoindre.", "Download KyberNow Station on the computer you want to reach."), cw - 2.0 * pad, 21.0 * u);
    let site = "www.kybernow.app";
    if console {
        // Pas de navigateur sous la main au salon : l'adresse, à taper ailleurs.
        cv.text(x + pad, cy + ch - pad - bh / 2.0 + 5.0 * u, Text::new(Font::Semibold, 15.0 * u, SPECTRE), site);
    } else {
        link(cv, f, Target::WelcomeDownload, x + pad, cy + ch - pad - bh, bh, 15.0, &format!("{site} →"));
    }
}

/// Une carte à la lueur de « SUR LE RÉSEAU » et du compte : fond violet, contour Spectre, halo.
fn glow_card(cv: &mut Canvas, r: Rect, u: f32) {
    cv.glow(r.x + r.w / 2.0, r.y + r.h / 2.0, r.w * 0.75, r.h * 0.95, HALO.alpha(0.16));
    cv.rrect(r.x, r.y, r.w, r.h, 14.0 * u, VIOLET_INK);
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0 * u, 1.2, FLAT);
    cv.rrect_stroke(r.x - 3.0, r.y - 3.0, r.w + 6.0, r.h + 6.0, 16.0 * u, 3.0, HALO.alpha(0.18));
}

// MARK: - Sans compte, ensuite

/// Un petit bouton plein (aplat Spectre), sans coin coupé : rend sa largeur.
#[allow(clippy::too_many_arguments)]
fn small_primary(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, size: f32, icon: Option<Icon>, s: &str) -> f32 {
    let style = Text::new(Font::Semibold, size, Rgba(255, 255, 255, 255));
    let iw = if icon.is_some() { size * 1.15 + size * 0.5 } else { 0.0 };
    let w = cv.text_width(style, s) + size * 1.8 + iw;
    let r = Rect::new(x, y, w, h);
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 8.0);
    }
    cv.rrect(x, y, w, h, 8.0, if hover { Rgba::hex(0x7E6EFF) } else { FLAT });
    let mut tx = x + size * 0.9;
    if let Some(i) = icon {
        cv.icon(&i, tx, y + (h - size * 1.15) / 2.0, size * 1.15, 1.9, Rgba(255, 255, 255, 255));
        tx += iw;
    }
    cv.text(tx, y + h / 2.0 + size * 0.36, style, s);
    w
}

fn small_primary_w(cv: &mut Canvas, size: f32, icon: bool, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Semibold, size, FROST), s) + size * 1.8 + if icon { size * 1.65 } else { 0.0 }
}

/// « Connecter un compte » en haut de l'accueil (icône d'une personne), aligné à droite sur `right` ; rend sa largeur
/// (0 : pas de bouton, un compte est connecté).
pub fn account_button(app: &App, cv: &mut Canvas, f: &mut Frame, right: f32, y: f32, h: f32, size: f32) -> f32 {
    if !app.account_button_visible() {
        return 0.0;
    }
    let bw = small_primary_w(cv, size, true, connect_label());
    small_primary(cv, f, Target::AccountTop, right - bw, y, h, size, Some(Icon::Person), connect_label());
    bw
}

/// Combien de places de la rangée prend la carte du compte (0, 1 ou 2), d'après les places libres après les
/// ordinateurs, les consoles et ceux du réseau : elle ne pousse jamais un ordinateur hors de la rangée.
pub fn card_slots(app: &App, free: usize) -> usize {
    if !app.account_card_visible() {
        return 0;
    }
    free.min(2)
}

/// La carte du compte, en fin de rangée : « COMPTE KYBERNOW », « Retrouvez vos ordinateurs partout », le texte (si la
/// place le permet), « Connecter un compte », « Créer un compte » (bureau, sur deux places), la croix « Fermer » (ou
/// « Ne plus afficher », revenue après 30 jours).
pub fn account_card(app: &App, cv: &mut Canvas, f: &mut Frame, r: Rect, u: f32, wide: bool) {
    glow_card(cv, r, 0.72 * u);
    let pad = 14.0 * u;
    // La croix, en haut à droite ; revenue, elle dit « Ne plus afficher ».
    let xs = 22.0 * u;
    let final_close = app.store.app.welcome.card_close_is_final();
    let close_label = if final_close { tr("Ne plus afficher", "Don't show again") } else { tr("Fermer", "Close") };
    let lst = Text::new(Font::Regular, 11.0 * u, LILAC.alpha(0.8));
    let lw = if final_close { cv.text_width(lst, close_label) + 6.0 * u } else { 0.0 };
    let cr = Rect::new(r.x + r.w - 6.0 * u - xs - lw, r.y + 6.0 * u, xs + lw, xs);
    let (focused, hover) = f.state(Target::CardClose, cr);
    if focused {
        ring(cv, cr, 6.0 * u);
    }
    if hover {
        cv.rrect(cr.x, cr.y, cr.w, cr.h, 6.0 * u, Rgba(227, 237, 240, 20));
    }
    if final_close {
        cv.text(cr.x + 4.0 * u, cr.y + cr.h / 2.0 + 4.0 * u, lst, close_label);
    }
    cv.icon(&Icon::Close, cr.x + cr.w - xs + 5.0 * u, cr.y + 5.0 * u, 12.0 * u, 2.0, LILAC.alpha(0.8));
    // Le texte.
    let max = r.w - 2.0 * pad - cr.w - 4.0 * u;
    cv.icon(&Icon::Globe, r.x + pad, r.y + pad - 1.0 * u, 13.0 * u, 2.0, LILAC);
    eyebrow(cv, r.x + pad + 19.0 * u, r.y + pad + 10.0 * u, 10.0 * u, LILAC, "Compte KyberNow");
    let title = tr("Retrouvez vos ordinateurs partout", "Find your computers everywhere");
    let bh = 28.0 * u;
    let by = r.y + r.h - pad - bh + 2.0 * u;
    let title_y = r.y + pad + 33.0 * u;
    cv.text_shrink(r.x + pad, title_y, Text::new(Font::Title, 15.5 * u, FROST), title, if wide { r.w - 2.0 * pad } else { max + cr.w });
    if by - title_y > 30.0 * u {
        cv.text_fit(r.x + pad, title_y + 20.0 * u, Text::new(Font::Regular, 12.5 * u, Rgba::hex(0xC9C5E2)), tr("Sur tous vos appareils, même hors de chez vous.", "On all your devices, even away from home."), r.w - 2.0 * pad);
    }
    let bw = small_primary(cv, f, Target::CardSignIn, r.x + pad, by, bh, 13.0 * u, None, connect_label());
    // « Créer un compte » : au bureau (le navigateur), sur une carte assez large.
    if wide && !app.sign_in_by_code() {
        account_ghost(cv, f, Target::CardSignUp, r.x + pad + bw + 8.0 * u, by, bh, 13.0 * u, create_label());
    }
}

/// Le bouton secondaire des cartes du compte (fond violet sombre, contour lilas).
#[allow(clippy::too_many_arguments)]
fn account_ghost(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, size: f32, s: &str) -> f32 {
    let style = Text::new(Font::Semibold, size, FROST);
    let w = cv.text_width(style, s) + size * 1.8;
    let r = Rect::new(x, y, w, h);
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 8.0);
    }
    cv.rrect(x, y, w, h, 8.0, if hover { Rgba::hex(0x2E2660) } else { Rgba::hex(0x241E4A) });
    cv.rrect_stroke(x, y, w, h, 8.0, 1.0, Rgba::hex(0x4A4380));
    cv.text(x + size * 0.9, y + h / 2.0 + size * 0.36, style, s);
    w
}

/// Le bandeau après un appairage, sous les boutons de l'ordinateur ajouté ; rend sa hauteur (0 : pas de bandeau, ou
/// pas la place au-dessus de `limit`).
#[allow(clippy::too_many_arguments)]
pub fn paired_banner(app: &App, cv: &mut Canvas, f: &mut Frame, id: &str, x: f32, y: f32, width: f32, limit: f32) -> f32 {
    if !app.paired_banner(id) {
        return 0.0;
    }
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let bh = if desktop { 32.0 } else { 40.0 * u };
    let size = if desktop { 13.5 } else { 15.0 * u };
    let pad = 16.0 * u;
    let later = tr("Plus tard", "Later");
    let buttons = small_primary_w(cv, size, false, connect_label()) + 8.0 * u + ghost_w(cv, size, later);
    let width = width.min(820.0 * u);
    let text_w = (width - pad - 26.0 * u - 14.0 * u - buttons - 2.0 * pad).max(160.0 * u);
    let style = Text::new(Font::Regular, size, Rgba::hex(0xDAD6F2));
    let lh = size * 1.4;
    let lines = cv.line_count(style, w::paired_text(), text_w) as f32;
    let h = (lines * lh + 2.0 * 12.0 * u).max(bh + 24.0 * u);
    if y + h > limit {
        return 0.0;
    }
    let r = Rect::new(x, y, width, h);
    glow_card(cv, r, 0.85 * u);
    let cy = y + h / 2.0;
    cv.icon(&Icon::Globe, x + pad, cy - 12.0 * u, 24.0 * u, 1.8, LILAC);
    let tx = x + pad + 26.0 * u + 14.0 * u;
    cv.paragraph(tx, cy - lines * lh / 2.0 + size * 1.0, style, w::paired_text(), text_w, lh);
    let mut bx = x + width - pad;
    let by = cy - bh / 2.0;
    bx -= ghost_w(cv, size, later);
    widgets::ghost(cv, f, Target::PairedLater, bx, by, bh, size, None, later, Rgba::hex(0xC9C5E2));
    bx -= 8.0 * u + small_primary_w(cv, size, false, connect_label());
    small_primary(cv, f, Target::PairedSignIn, bx, by, bh, size, None, connect_label());
    h
}

/// « Hors de chez vous ? » par-dessus l'accueil : l'icône du globe, le titre, le texte ; « Pas maintenant »,
/// « Créer un compte » (bureau), « Connecter un compte ».
pub fn away(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(a) = &app.welcome.away else { return };
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let (wd, ht) = (cv.w, cv.h);
    f.hits.clear();
    cv.rect(0.0, 0.0, wd, ht, Rgba(2, 3, 5, 170));
    let cw = (640.0 * u).min(wd - 32.0 * u);
    let pad = 32.0 * u;
    let icon = 60.0 * u;
    let tw = cw - 2.0 * pad - icon - 18.0 * u;
    let title = Text::new(Font::Display, 30.0 * u, FROST).tracking(-0.3 * u);
    let body = Text::new(Font::Regular, 16.5 * u, MUTED);
    let lh = 24.0 * u;
    let text = w::away_text(&a.name);
    let text_h = cv.line_count(body, &text, tw) as f32 * lh;
    let bh = if desktop { 46.0 } else { 56.0 * u };
    let ch = pad + 40.0 * u + 12.0 * u + text_h + 26.0 * u + bh + pad;
    let (x, y) = ((wd - cw) / 2.0, ((ht - ch) / 2.0).max(12.0));
    cv.rrect(x, y, cw, ch, 20.0 * u, INK);
    cv.rrect_stroke(x, y, cw, ch, 20.0 * u, 1.0, LINE_2);
    cv.rrect(x + pad, y + pad, icon, icon, 14.0 * u, Rgba::hex(0x171431));
    cv.icon(&Icon::Globe, x + pad + icon * 0.25, y + pad + icon * 0.25, icon * 0.5, 1.8, SPECTRE);
    let tx = x + pad + icon + 18.0 * u;
    cv.text_shrink(tx, y + pad + 30.0 * u, title, tr("Hors de chez vous ?", "Away from home?"), tw);
    cv.paragraph(tx, y + pad + 40.0 * u + 12.0 * u + 16.0 * u, body, &text, tw, lh);
    // Les boutons, alignés à droite : « Pas maintenant », « Créer un compte », « Connecter un compte ».
    let by = y + ch - pad - bh;
    let size = if desktop { 15.0 } else { 17.0 * u };
    let mut bx = x + cw - pad;
    let main_w = cv.text_width(Text::new(Font::Semibold, bh * 0.32, FROST), connect_label()) + 2.0 * bh * 0.58;
    bx -= main_w;
    primary(cv, f, Target::AwaySignIn, bx, by, bh, connect_label(), false);
    if !app.sign_in_by_code() {
        bx -= 12.0 * u + ghost_w(cv, size, create_label());
        widgets::ghost(cv, f, Target::AwaySignUp, bx, by + (bh - 44.0 * u.min(1.2)) / 2.0, 44.0 * u.min(1.2), size, None, create_label(), FROST);
    }
    let later = tr("Pas maintenant", "Not now");
    bx -= 12.0 * u + ghost_w(cv, size, later);
    widgets::ghost(cv, f, Target::AwayLater, bx, by + (bh - 44.0 * u.min(1.2)) / 2.0, 44.0 * u.min(1.2), size, None, later, MUTED);
}
