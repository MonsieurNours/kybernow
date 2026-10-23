//! Ajouter un ordinateur, d'après la maquette « Steam Deck · Ajouter un
//! ordinateur » : à gauche les Stations trouvées sur le réseau et les autres
//! façons ; à droite le code à six chiffres à accepter dans Admin. Puis, par
//! adresse : l'adresse d'Admin (clavier, ou pavé à la manette), et les
//! quatre premiers groupes de l'empreinte à comparer.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, primary};
use super::{launch, title_bar, unit, Frame};
use crate::app::{short_fp, AddStep, App, Layout, Rect, Target};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    cv.glow(w * 0.85, h * 0.15, w * 0.6, h * 0.65, Rgba::hex(0x7A5CFF).alpha(0.2));
    let tv = app.layout == Layout::Tv;
    let x0 = if tv { 96.0 } else if app.layout == Layout::Desktop { 48.0 } else { 40.0 };
    let top = if app.layout == Layout::Desktop {
        title_bar(app, cv, f)
    } else {
        let (lx, ly, lh) = launch::header_logo(app.layout);
        cv.logo_link(lx, ly, lh, 1.0);
        56.0 * u + if tv { 12.0 } else { 0.0 }
    };
    let footer = if app.layout.console() { 56.0 * u } else { 0.0 };
    let y0 = top + 28.0 * u;
    match app.add.step.clone() {
        AddStep::Code => code(app, cv, f, x0, y0, u, footer),
        AddStep::Address => address(app, cv, f, x0, y0, u),
        AddStep::Verify { address, port, fingerprint } => verify(app, cv, f, x0, y0, u, &address, port, &fingerprint),
    }
    if app.layout.console() {
        cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
        cv.rect(0.0, h - footer, w, 1.0, LINE);
        let gy = h - footer / 2.0;
        let g = 26.0 * u;
        let mut x = x0;
        match app.add.step {
            AddStep::Code => {
                x += legend(cv, x, gy, g, &["A"], tr("Demander l'accès", "Ask for access")) + 28.0 * u;
                x += legend(cv, x, gy, g, &["B"], tr("Retour", "Back")) + 28.0 * u;
                legend(cv, x, gy, g, &["Y"], tr("Entrer une adresse", "Enter an address"));
            }
            AddStep::Address => {
                x += legend(cv, x, gy, g, &["A"], tr("Touche", "Key")) + 28.0 * u;
                legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
                let label = tr("Clavier", "Keyboard");
                let lw = widgets::legend_width(cv, g, &["Steam + X"], label);
                legend(cv, w - x0 - lw, gy, g, &["Steam + X"], label);
            }
            AddStep::Verify { .. } => {
                x += legend(cv, x, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
                legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
            }
        }
    }
}

fn back_button(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, u: f32) -> f32 {
    widgets::ghost(cv, f, crate::app::Target::Back, x, y, 40.0 * u, 14.0 * u, Some(Icon::Back), tr("Retour", "Back"), MUTED)
}

fn code(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32, footer: f32) {
    let w = cv.w;
    let h = cv.h;
    let mut y = y0;
    if app.layout == Layout::Desktop {
        back_button(cv, f, x0, y, u);
        y += 56.0;
    }
    let left_w = (560.0 * u).min(w * 0.46);
    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, tr("Ajouter un ordinateur", "Add a computer"));
    cv.text(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), tr("Sur le réseau", "On the network"));
    y += 80.0 * u;
    let found = app.new_found();
    let item_h = 76.0 * u;
    for (i, fd) in found.iter().enumerate().take(4) {
        let r = Rect::new(x0, y, left_w, item_h);
        let (focused, hover) = f.state(Target::AddFound(i), r);
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { Rgba(20, 27, 35, 240) } else { CARD });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, if focused { SPECTRE } else { LINE });
        widgets::dot(cv, r.x + 22.0 * u, r.y + r.h / 2.0, 5.0 * u, SPECTRE, true);
        cv.text_fit(r.x + 44.0 * u, r.y + 32.0 * u, Text::new(Font::Semibold, 18.0 * u, FROST), &fd.name, r.w - 220.0 * u);
        let fp = fd.fingerprint.as_deref().map(short_fp).unwrap_or_default();
        cv.text_fit(r.x + 44.0 * u, r.y + 56.0 * u, Text::new(Font::Mono, 12.0 * u, FAINT), &format!("{} · Admin {} · fp {fp}", fd.address, fd.port), r.w - 60.0 * u);
        cv.text(r.x + r.w - 18.0 * u, r.y + 31.0 * u, Text::new(Font::Mono, 10.5 * u, SPECTRE).tracking(u).align(Align::Right), "KYBERNOW STATION");
        y += item_h + 12.0 * u;
    }
    // Les ordinateurs déjà ajoutés, en rappel.
    for hst in app.store.hosts.iter().take(if found.is_empty() { 2 } else { 1 }) {
        let r = Rect::new(x0, y, left_w, item_h);
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, Rgba(14, 20, 26, 140));
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE);
        widgets::dot(cv, r.x + 22.0 * u, r.y + r.h / 2.0, 5.0 * u, GHOST, false);
        cv.text_fit(r.x + 44.0 * u, r.y + 32.0 * u, Text::new(Font::Semibold, 18.0 * u, MUTED), &hst.name, r.w - 60.0 * u);
        cv.text(r.x + 44.0 * u, r.y + 56.0 * u, Text::new(Font::Mono, 12.0 * u, GHOST), tr("Déjà dans vos ordinateurs", "Already in your computers"));
        y += item_h + 12.0 * u;
    }
    // Recherche en cours.
    let t = app.add.started.elapsed().as_secs_f32();
    let cx = x0 + 9.0 * u;
    let cy = y + 10.0 * u;
    let a0 = t * 4.0;
    let pts: Vec<(f32, f32)> = (0..=16).map(|i| {
        let a = a0 + i as f32 * 0.28;
        (cx + 8.0 * u * a.cos(), cy + 8.0 * u * a.sin())
    }).collect();
    cv.polyline(&pts, 2.0, SPECTRE);
    cv.text(x0 + 28.0 * u, y + 15.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT), tr("Recherche en cours, annonce Bonjour d'Admin", "Searching, Admin's Bonjour announcement"));
    y += 44.0 * u;
    widgets::ghost(cv, f, Target::AddByAddress, x0, y, 44.0 * u, 15.0 * u, Some(Icon::Ethernet), tr("Avec une adresse", "With an address"), MUTED);

    // Le code, à droite.
    let px = x0 + left_w + 40.0 * u;
    let pw = w - px - x0;
    let py = y0 + if app.layout == Layout::Desktop { 56.0 } else { 0.0 };
    let ph = (h - footer - 28.0 * u - py).min(470.0 * u);
    if pw < 300.0 {
        return;
    }
    cv.rrect(px, py, pw, ph, 14.0 * u, Rgba(9, 13, 18, 205));
    cv.rrect_stroke(px, py, pw, ph, 14.0 * u, 1.0, LINE);
    let ix = px + 30.0 * u;
    let mut yy = py + 28.0 * u + 12.0 * u;
    eyebrow(cv, ix, yy, 12.0 * u, FAINT, tr("Avec un code", "With a code"));
    yy += 30.0 * u;
    let target = app.add.asked.last().map(|(n, _)| n.clone()).or_else(|| found.first().map(|f| f.name.clone()));
    let lead = match &target {
        Some(n) => format!("{} {n}. {}", tr("Cet appareil demande l'accès à", "This device asks for access to"), tr("Acceptez ce code dans Admin, Appareils Link.", "Accept this code in Admin, Link devices.")),
        None => tr("Le code sera présenté à la Station dès qu'elle apparaît sur le réseau. Acceptez-le dans Admin, Appareils Link.", "The code will be shown to the Station as soon as it appears on the network. Accept it in Admin, Link devices.").into(),
    };
    yy += cv.paragraph(ix, yy, Text::new(Font::Regular, 18.0 * u, FROST), &lead, pw - 60.0 * u, 26.0 * u);
    yy += 18.0 * u;
    // Les six chiffres.
    let dw = 64.0 * u;
    let dh = 84.0 * u;
    let mut dx = ix;
    for (i, c) in app.add.code.chars().enumerate() {
        if i == 3 {
            dx += 14.0 * u;
        }
        cv.rrect(dx, yy, dw, dh, 10.0 * u, INK_2);
        cv.rrect_stroke(dx, yy, dw, dh, 10.0 * u, 1.0, LINE_2);
        cv.text(dx + dw / 2.0, yy + dh / 2.0 + 16.0 * u, Text::new(Font::MonoMedium, 44.0 * u, CORE).align(Align::Center), &c.to_string());
        dx += dw + 10.0 * u;
    }
    yy += dh + 30.0 * u;
    let waiting = if app.add.asked.is_empty() { tr("En attente d'une Station", "Waiting for a Station") } else { tr("En attente d'Admin", "Waiting for Admin") };
    widgets::dot(cv, ix + 4.0 * u, yy - 4.0 * u, 3.5 * u, SPECTRE, true);
    cv.text(ix + 16.0 * u, yy, Text::new(Font::Mono, 12.0 * u, FAINT), waiting);
    if let Some(m) = &app.add.message {
        yy += 26.0 * u;
        cv.paragraph(ix, yy, Text::new(Font::Regular, 14.0 * u, MUTED), m, pw - 60.0 * u, 20.0 * u);
    }
    let foot = format!(
        "{} {}.",
        tr("L'appareil reçoit son propre compte Kyber : rien à taper ici. Nom de cet appareil pour Admin :", "The device gets its own Kyber account: nothing to type here. This device's name for Admin:"),
        app.device_name
    );
    cv.paragraph(ix, py + ph - 52.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT), &foot, pw - 60.0 * u, 20.0 * u);
}

fn address(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let mut y = y0;
    if app.layout == Layout::Desktop {
        back_button(cv, f, x0, y, u);
        y += 56.0;
    }
    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, tr("Ajouter un ordinateur", "Add a computer"));
    cv.text(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), tr("Par adresse", "By address"));
    y += 82.0 * u;
    cv.paragraph(
        x0,
        y,
        Text::new(Font::Regular, 16.0 * u, MUTED),
        tr("L'adresse et le port d'Admin, tels qu'Admin les affiche (Appareils Link), par exemple 192.168.1.20:24811. Tailscale et WireGuard marchent aussi.", "Admin's address and port, as Admin shows them (Link devices), for example 192.168.1.20:24811. Tailscale and WireGuard work too."),
        (w - 2.0 * x0).min(760.0 * u),
        24.0 * u,
    );
    y += 70.0 * u;
    // Le champ.
    let fw = (w - 2.0 * x0).min(640.0 * u);
    let fh = 56.0 * u;
    cv.rrect(x0, y, fw, fh, 10.0 * u, INK_2);
    cv.rrect_stroke(x0, y, fw, fh, 10.0 * u, 1.5, SPECTRE);
    let text = if app.add.address.is_empty() { "192.168.1.20:24811".to_string() } else { app.add.address.clone() };
    let tw = cv.text(x0 + 18.0 * u, y + fh / 2.0 + 7.0 * u, Text::new(Font::Mono, 20.0 * u, if app.add.address.is_empty() { GHOST } else { FROST }), &text);
    if !app.add.address.is_empty() && (app.screen_since.elapsed().as_millis() / 530) % 2 == 0 {
        cv.rect(x0 + 20.0 * u + tw, y + 14.0 * u, 2.0, fh - 28.0 * u, SPECTRE);
    }
    y += fh + 20.0 * u;
    if let Some(m) = &app.add.message {
        cv.text_fit(x0, y + 6.0 * u, Text::new(Font::Regular, 14.0 * u, if app.add.busy { MUTED } else { WARN }), m, fw);
        y += 30.0 * u;
    }
    // Le pavé, à la manette (et à la souris).
    let keys = ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", ":"];
    let kw = 68.0 * u;
    let kh = 52.0 * u;
    let gap = 10.0 * u;
    for (i, k) in keys.iter().enumerate() {
        let (col, row) = (i % 3, i / 3);
        let r = Rect::new(x0 + col as f32 * (kw + gap), y + row as f32 * (kh + gap), kw, kh);
        let c = k.chars().next().unwrap();
        key(cv, f, Target::Key(c), r, k, u);
    }
    let kx = x0 + 3.0 * (kw + gap) + gap;
    key(cv, f, Target::KeyErase, Rect::new(kx, y, kw * 1.6, kh), "⌫", u);
    primary(cv, f, Target::KeyOk, kx, y + 3.0 * (kh + gap), kh, tr("Continuer", "Continue"), false);
}

fn key(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, label: &str, u: f32) {
    let (focused, hover) = f.state(t, r);
    if focused {
        widgets::ring(cv, r, 8.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 8.0 * u, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 8.0 * u, 1.0, LINE_2);
    cv.text(r.x + r.w / 2.0, r.y + r.h / 2.0 + 7.0 * u, Text::new(Font::MonoMedium, 20.0 * u, FROST).align(Align::Center), label);
}

#[allow(clippy::too_many_arguments)]
fn verify(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32, address: &str, port: u16, fingerprint: &str) {
    let w = cv.w;
    let mut y = y0;
    if app.layout == Layout::Desktop {
        back_button(cv, f, x0, y, u);
        y += 56.0;
    }
    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, &format!("{address}:{port}"));
    cv.text(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), tr("Est-ce bien lui ?", "Is it the right one?"));
    y += 82.0 * u;
    cv.paragraph(
        x0,
        y,
        Text::new(Font::Regular, 16.0 * u, MUTED),
        tr("Comparez ces quatre groupes avec ceux qu'Admin affiche (Appareils Link). S'ils diffèrent, ce n'est pas votre ordinateur.", "Compare these four groups with those Admin shows (Link devices). If they differ, it is not your computer."),
        (w - 2.0 * x0).min(760.0 * u),
        24.0 * u,
    );
    y += 74.0 * u;
    let groups = short_fp(fingerprint);
    let mut gx = x0;
    for g in groups.split(" · ") {
        let gw = 120.0 * u;
        cv.rrect(gx, y, gw, 64.0 * u, 10.0 * u, INK_2);
        cv.rrect_stroke(gx, y, gw, 64.0 * u, 10.0 * u, 1.0, LINE_2);
        cv.text(gx + gw / 2.0, y + 42.0 * u, Text::new(Font::MonoMedium, 26.0 * u, CORE).align(Align::Center), g);
        gx += gw + 12.0 * u;
    }
    y += 96.0 * u;
    let bw = primary(cv, f, Target::VerifyOk, x0, y, 56.0 * u, tr("Oui, demander l'accès", "Yes, ask for access"), false);
    widgets::ghost(cv, f, Target::VerifyNo, x0 + bw + 18.0 * u, y + 6.0 * u, 44.0 * u, 15.0 * u, None, tr("Non, une autre adresse", "No, another address"), MUTED);
}
