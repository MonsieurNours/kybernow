//! KyberNow Link OS : le premier démarrage, d'après les maquettes
//! (https://claude.ai/artifact/6crVeg3QyrA5vs8TRhWRft, « Premier démarrage ») :
//! Manette (Bluetooth à l'écoute, sans code ; USB, télécommande CEC ou clavier
//! marchent aussi), Langue, Réseau (Ethernet, ou un Wi-Fi et son mot de passe
//! au pavé), Usage (salon ou bureau), puis l'ordinateur (l'écran d'ajout par
//! code). Tant que la langue n'est pas choisie, les écrans sont bilingues :
//! l'anglais d'abord, le français dessous. Dessiné en 1920 × 1080, comme le salon.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, legend_width, primary};
use super::{launch, Frame};
use crate::app::{App, NetKind, Rect, SetupStep, Target};
use crate::i18n::tr;

/// Les étiquettes des étapes : (anglais, français).
const STEPS: [(&str, &str); 5] = [("Controller", "Manette"), ("Language", "Langue"), ("Network", "Réseau"), ("Use", "Usage"), ("Computer", "Ordinateur")];

/// Avant le choix de la langue : « English · Français ».
fn bi(app: &App, en: &str, fr: &str) -> String {
    if bilingual(app) {
        format!("{en} · {fr}")
    } else if crate::i18n::english() {
        en.to_string()
    } else {
        fr.to_string()
    }
}

fn bilingual(app: &App) -> bool {
    app.setup.active && matches!(app.setup.step, SetupStep::Controller | SetupStep::Language)
}

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    cv.glow(w * 0.72, h * 0.18, w * 0.68, h * 0.83, Rgba::hex(0x7A5CFF).alpha(0.28));
    let s = 1100.0;
    cv.mark(w - s * 0.76, h - s * 0.67, s, Rgba::hex(0x141B23).alpha(0.5), Rgba::hex(0x141B23).alpha(0.5), None);
    header(app, cv);
    steps(app, cv, 96.0 + 28.0);
    match app.setup.step {
        SetupStep::Controller => controller(app, cv, f),
        SetupStep::Language => language(app, cv, f),
        SetupStep::Network => network(app, cv, f),
        SetupStep::Usage => usage(app, cv, f),
        SetupStep::Computer => {}
    }
}

/// L'en-tête : le logo, la version de l'OS, le modèle, le Bluetooth à l'écoute.
fn header(app: &App, cv: &mut Canvas) {
    let (lx, ly, lh) = launch::header_logo(crate::app::Layout::Tv);
    cv.logo_link(lx, ly, lh, 1.0);
    let mono = Text::new(Font::Mono, 18.0, MUTED);
    let mut parts: Vec<(bool, String)> = Vec::new();
    parts.push((false, format!("KyberNow Link OS {}", app.os.as_ref().map(|o| o.version.as_str()).unwrap_or(app.version))));
    if let Some(p) = &app.machine.pi {
        parts.push((false, format!("{} · {} {}", p.model, p.ram_gb, if bilingual(app) { "GB" } else { tr("Go", "GB") })));
    }
    if app.setup.step == SetupStep::Controller && app.setup.bt_listening {
        parts.push((true, bi(app, "Bluetooth listening", "à l'écoute")));
    }
    let gap = 22.0;
    let widths: Vec<f32> = parts.iter().map(|(dot, s)| cv.text_width(mono, s) + if *dot { 24.0 } else { 0.0 }).collect();
    let total: f32 = widths.iter().sum::<f32>() + gap * (parts.len().saturating_sub(1)) as f32;
    let mut x = cv.w - 96.0 - total;
    for ((dot, s), w) in parts.iter().zip(widths) {
        if *dot {
            // Le point qui respire : le Bluetooth cherche.
            let t = app.screen_since.elapsed().as_secs_f32();
            let a = 0.6 + 0.4 * (t * 2.2).sin();
            cv.circle(x + 7.0, 48.0 + 6.0 - 6.0, 7.0, SPECTRE.alpha(a));
            cv.circle(x + 7.0, 48.0, 13.0, SPECTRE.alpha(0.25 * a));
            x += 24.0;
        }
        cv.text(x, 48.0 + 6.0, mono, s);
        x += w - if *dot { 24.0 } else { 0.0 } + gap;
    }
}

/// Les cinq étapes : faites (✓), en cours, à venir.
fn steps(app: &App, cv: &mut Canvas, y: f32) {
    let current = SetupStep::ALL.iter().position(|s| *s == app.setup.step).unwrap_or(0);
    let mut x = 96.0;
    for (i, (en, fr)) in STEPS.iter().enumerate() {
        let (color, ring, fill) = if i < current {
            (FAINT, OK, None)
        } else if i == current {
            (FROST, SPECTRE, Some(FLAT))
        } else {
            (GHOST, LINE_2, None)
        };
        let cy = y + 15.0;
        if let Some(c) = fill {
            cv.circle(x + 15.0, cy, 15.0, c);
        }
        cv.circle_stroke(x + 15.0, cy, 15.0, 2.0, ring);
        let num = Text::new(Font::Mono, 14.0, if i < current { OK } else if i == current { Rgba(255, 255, 255, 255) } else { GHOST }).align(Align::Center);
        let n = format!("{}", i + 1);
        cv.text(x + 15.0, cy + 5.0, num, if i < current { "✓" } else { &n });
        let label = if bilingual(app) || i == 3 && bilingual(app) { format!("{en} · {fr}") } else { tr(fr, en).to_string() };
        let tw = cv.text(x + 42.0, cy + 6.0, Text::new(Font::Mono, 17.0, color), &label);
        x += 42.0 + tw + 40.0;
    }
}

/// Le titre et sa phrase ; rend le bas du bloc.
fn title(app: &App, cv: &mut Canvas, y: f32, main: &str, sub: &str) -> f32 {
    let _ = app;
    cv.text_shrink(95.0, y + 54.0, Text::new(Font::Display, 68.0, FROST).tracking(-1.7), main, cv.w - 192.0);
    cv.text_fit(96.0, y + 54.0 + 10.0 + 26.0, Text::new(Font::Regular, 26.0, FAINT), sub, cv.w - 192.0);
    y + 54.0 + 10.0 + 26.0 + 10.0
}

fn footer(app: &App, cv: &mut Canvas, entries: &[(&[&str], String)], note: Option<&str>) {
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, h - 80.0, w, 80.0, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - 80.0, w, 1.0, LINE);
    let gy = h - 40.0;
    let mut x = 96.0;
    for (keys, s) in entries {
        x += legend(cv, x, gy, 34.0, keys, s) + 40.0;
    }
    if let Some(n) = note {
        let st = Text::new(Font::Regular, 17.0, FAINT).align(Align::Right);
        cv.text_fit(w - 96.0, gy + 6.0, st, n, w - 96.0 - x);
    }
    let _ = app;
}

// MARK: - Manette

fn controller(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    // Le titre dans une langue, la seconde dessous (maquette : l'anglais d'abord).
    let (main, second) = if bilingual(app) { ("Plug in or pair a controller", "Branchez ou associez une manette") } else { (tr("Branchez ou associez une manette", "Plug in or pair a controller"), "") };
    let y = title(app, cv, 124.0 + 30.0 + 40.0, main, second);
    let y0 = y + 36.0;
    // À gauche : les familles de manettes et leur mode association.
    let x0 = 96.0;
    let left_w = 1000.0f32.min(w - 192.0 - 560.0 - 56.0);
    let wireless = if bilingual(app) { "Wireless · put the controller in pairing mode, it shows up here".to_string() } else { tr("Sans fil · mettez la manette en mode association, elle apparaît ici", "Wireless · put the controller in pairing mode, it shows up here").to_string() };
    eyebrow(cv, x0, y0 + 11.0, 15.0, FAINT, &wireless);
    let pads: [(&str, &str, &str, &str); 4] = [
        ("XBOX", "Xbox controller", "Hold the small pairing button near the USB port for three seconds, until the logo flashes fast.", "Tenir le petit bouton d'association, près du port USB, trois secondes, jusqu'à ce que le logo clignote vite."),
        ("PS", "DualSense, DualShock 4", "Hold Create (or Share) and PS together until the light bar flashes.", "Tenir Create (ou Share) et PS ensemble jusqu'à ce que la barre lumineuse clignote."),
        ("PRO", "Nintendo Switch Pro", "Hold SYNC on the top edge until the lights sweep.", "Tenir le bouton SYNC, sur la tranche du haut, jusqu'aux voyants qui défilent."),
        ("8BIT", "8BitDo and others", "X (Xbox) or S (Switch) mode, then Pair. Any PIN-less Bluetooth controller is accepted.", "Mode X (Xbox) ou S (Switch), puis le bouton Pair. Toute manette Bluetooth sans code est acceptée."),
    ];
    let mut py = y0 + 20.0 + 4.0;
    let text_w = left_w - 56.0 - 22.0;
    for (i, (badge, name, en, fr)) in pads.iter().enumerate() {
        let top = py + 18.0;
        cv.rrect(x0, top, 56.0, 56.0, 14.0, INK_3);
        cv.text(x0 + 28.0, top + 28.0 + 5.0, Text::new(Font::MonoMedium, 15.0, MUTED).align(Align::Center), badge);
        let tx = x0 + 56.0 + 22.0;
        cv.text(tx, top + 23.0, Text::new(Font::Semibold, 23.0, FROST), name);
        let (first, second) = if bilingual(app) { (en.to_string(), Some(fr.to_string())) } else { (tr(fr, en).to_string(), None) };
        let h1 = cv.paragraph(tx, top + 23.0 + 4.0 + 17.0, Text::new(Font::Regular, 17.0, FAINT), &first, text_w, 24.5);
        let mut row_h = 23.0 + 4.0 + h1;
        if let Some(s) = second {
            row_h += 2.0 + cv.paragraph(tx, top + row_h + 15.0, Text::new(Font::Regular, 15.0, GHOST), &s, text_w, 21.0);
        }
        let row_h = row_h.max(56.0);
        py = top + row_h + 18.0;
        if i < 3 {
            cv.rect(x0, py, left_w, 1.0, LINE);
        }
    }
    // À droite : USB, la télécommande, un clavier.
    let ax = w - 96.0 - 560.0;
    let alts: [(Icon, &str, &str, &str, &str); 3] = [
        (Icon::Usb, "A USB cable", "Un câble USB", "Any wired controller works at once, no pairing.", "Toute manette branchée marche tout de suite."),
        (Icon::Remote, "The TV remote", "La télécommande du téléviseur", "Over HDMI-CEC, arrows and OK are enough for these screens.", "Par HDMI-CEC, flèches et OK suffisent ici."),
        (Icon::Keyboard, "A USB keyboard", "Un clavier USB", "Arrows and Enter.", "Flèches et Entrée."),
    ];
    let mut ay = y0 + 24.0;
    let panel_top = y0;
    widgets::panel(cv, Rect::new(ax, panel_top, 560.0, 1.0), 18.0);
    eyebrow(cv, ax + 28.0, ay + 11.0, 15.0, FAINT, &bi(app, "Or", "ou bien"));
    ay += 20.0 + 6.0;
    let atw = 560.0 - 56.0 - 56.0;
    for (i, (icon, en, fr, sen, sfr)) in alts.iter().enumerate() {
        let top = ay + 16.0;
        cv.rrect(ax + 28.0, top, 40.0, 40.0, 10.0, INK_3);
        cv.icon(icon, ax + 28.0 + 9.0, top + 9.0, 22.0, 1.8, SPECTRE);
        let tx = ax + 28.0 + 56.0;
        cv.text(tx, top + 20.0, Text::new(Font::Semibold, 20.0, FROST), if bilingual(app) { en } else { tr(fr, en) });
        let body = if bilingual(app) { format!("{sen} {sfr}") } else { tr(sfr, sen).to_string() };
        let hh = cv.paragraph(tx, top + 20.0 + 4.0 + 16.0, Text::new(Font::Regular, 16.0, FAINT), &body, atw, 23.0);
        ay = top + (20.0 + 4.0 + hh).max(40.0) + 16.0;
        if i < 2 {
            cv.rect(ax + 28.0, ay, 560.0 - 56.0, 1.0, LINE);
        }
    }
    let panel_h = ay + 10.0 - panel_top;
    widgets::panel(cv, Rect::new(ax, panel_top, 560.0, panel_h), 18.0);
    // Redessiné par-dessus le panneau (le fond est translucide).
    let mut ay = y0 + 24.0;
    eyebrow(cv, ax + 28.0, ay + 11.0, 15.0, FAINT, &bi(app, "Or", "ou bien"));
    ay += 20.0 + 6.0;
    for (i, (icon, en, fr, sen, sfr)) in alts.iter().enumerate() {
        let top = ay + 16.0;
        cv.rrect(ax + 28.0, top, 40.0, 40.0, 10.0, INK_3);
        cv.icon(icon, ax + 28.0 + 9.0, top + 9.0, 22.0, 1.8, SPECTRE);
        let tx = ax + 28.0 + 56.0;
        cv.text(tx, top + 20.0, Text::new(Font::Semibold, 20.0, FROST), if bilingual(app) { en } else { tr(fr, en) });
        let body = if bilingual(app) { format!("{sen} {sfr}") } else { tr(sfr, sen).to_string() };
        let hh = cv.paragraph(tx, top + 20.0 + 4.0 + 16.0, Text::new(Font::Regular, 16.0, FAINT), &body, atw, 23.0);
        ay = top + (20.0 + 4.0 + hh).max(40.0) + 16.0;
        if i < 2 {
            cv.rect(ax + 28.0, ay, 560.0 - 56.0, 1.0, LINE);
        }
    }

    // En bas : la manette trouvée (Bluetooth ou USB), et A pour continuer.
    let fy = h - 118.0 - 84.0;
    eyebrow(cv, 96.0, fy - 14.0 - 3.0, 15.0, FAINT, &bi(app, "Found", "trouvée"));
    let r = Rect::new(96.0, fy, w - 192.0, 84.0);
    let found = app.setup.bt.iter().find(|d| d.connected).or_else(|| app.setup.bt.iter().find(|d| d.paired)).map(|d| (d.name.clone(), d.connected));
    let usb = app.pads.first().map(|p| (p.name.clone(), true));
    let (focused, _) = f.state(Target::SetupContinue, r);
    match found.or(usb) {
        Some((name, connected)) if !name.is_empty() => {
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(14, 20, 26, 217));
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.5, SPECTRE);
            cv.rrect_stroke(r.x - 4.0, r.y - 4.0, r.w + 8.0, r.h + 8.0, 17.0, 3.0, HALO.alpha(0.22));
            cv.icon(&Icon::Gamepad, r.x + 24.0, r.y + 27.0, 30.0, 1.8, SPECTRE);
            let nx = r.x + 24.0 + 30.0 + 18.0;
            let nw = cv.text(nx, r.y + 42.0 + 8.0, Text::new(Font::Semibold, 24.0, FROST), &name);
            let state = if connected { bi(app, "paired · player 1", "associée · joueur 1") } else { bi(app, "pairing…", "association…") };
            cv.text(nx + nw + 18.0, r.y + 42.0 + 6.0, Text::new(Font::Mono, 17.0, OK), &state);
            cv.text(r.x + r.w - 24.0, r.y + 42.0 + 6.0, Text::new(Font::Mono, 17.0, MUTED).align(Align::Right), &bi(app, "Press A to continue", "Appuyez sur A pour continuer"));
        }
        _ => {
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(14, 20, 26, 160));
            cv.rrect_dashed(r.x, r.y, r.w, r.h, 14.0, 1.0, LINE_2);
            let t = app.screen_since.elapsed().as_secs_f32();
            let dots = ".".repeat(1 + (t * 1.5) as usize % 3);
            cv.text(r.x + 24.0, r.y + 42.0 + 6.0, Text::new(Font::Mono, 17.0, FAINT), &format!("{}{dots}", bi(app, "Nothing yet", "rien encore")));
            cv.text(r.x + r.w - 24.0, r.y + 42.0 + 6.0, Text::new(Font::Mono, 17.0, GHOST).align(Align::Right), &bi(app, "A keyboard or the remote can continue", "un clavier ou la télécommande peuvent continuer"));
        }
    }
    if focused {
        widgets::ring(cv, r, 14.0);
    }
    footer(app, cv, &[(&["A"], bi(app, "Continue", "Continuer"))], Some(&bi(app, "No code to type: pairing is accepted during this screen and in Settings › Controllers.", "Aucun code : l'association est acceptée pendant cet écran et dans Réglages › Manettes.")));
}

// MARK: - Langue

fn language(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let y = title(app, cv, 124.0 + 30.0 + 40.0, "Language · Langue", "Choose the language of the screens. · Choisissez la langue des écrans.");
    let cards: [(&str, &str, &str); 2] = [("English", "Menus, settings and help in English.", "EN"), ("Français", "Menus, réglages et aide en français.", "FR")];
    let mut x = 96.0;
    for (i, (name, desc, badge)) in cards.iter().enumerate() {
        let r = Rect::new(x, y + 48.0, 560.0, 300.0);
        choice_card(cv, f, Target::SetupLang(i), r, badge, name, desc, "", i == app.setup.lang);
        x += 560.0 + 40.0;
    }
    footer(app, cv, &[(&["A"], "Choose · Choisir".into()), (&["B"], "Back · Retour".into())], None);
}

/// Une grande carte à choisir (langue, usage) : icône ou sigle, titre, texte, note.
#[allow(clippy::too_many_arguments)]
fn choice_card(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, badge: &str, name: &str, desc: &str, note: &str, on: bool) {
    let (focused, hover) = f.state(t, r);
    cv.rrect(r.x, r.y, r.w, r.h, 22.0, if hover { Rgba(20, 27, 35, 235) } else { Rgba(14, 20, 26, 217) });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 22.0, 1.0, if on { SPECTRE } else { LINE });
    if focused {
        widgets::ring(cv, r, 22.0);
    }
    let (px, py) = (r.x + 44.0, r.y + 40.0);
    cv.rrect(px, py, 72.0, 72.0, 18.0, INK_3);
    match badge {
        "GAMEPAD" => cv.icon(&Icon::Gamepad, px + 17.0, py + 17.0, 38.0, 1.8, SPECTRE),
        "MONITOR" => cv.icon(&Icon::Monitor, px + 17.0, py + 17.0, 38.0, 1.8, SPECTRE),
        b => {
            cv.text(px + 36.0, py + 36.0 + 8.0, Text::new(Font::MonoMedium, 22.0, SPECTRE).align(Align::Center), b);
        }
    }
    let ty = py + 72.0 + 18.0;
    cv.text_shrink(px - 1.0, ty + 32.0, Text::new(Font::Display, 40.0, FROST).tracking(-0.8), name, r.w - 88.0);
    cv.paragraph(px, ty + 32.0 + 18.0 + 21.0, Text::new(Font::Regular, 21.0, MUTED), desc, r.w - 88.0, 30.0);
    if !note.is_empty() {
        let st = Text::new(Font::Mono, 15.0, FAINT);
        let lines = cv.line_count(st, note, r.w - 88.0) as f32;
        cv.paragraph(px, r.y + r.h - 40.0 - (lines - 1.0) * 22.0, st, note, r.w - 88.0, 22.0);
    }
}

// MARK: - Réseau

fn network(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    let s = &app.setup;
    if let Some(i) = s.wifi_pick {
        let ssid = s.wifi.get(i).map(|n| n.ssid.clone()).unwrap_or_default();
        let y = title(app, cv, 124.0 + 30.0 + 40.0, &format!("{} {ssid}", tr("Mot de passe de", "Password for")), tr("Au pavé, à la manette ; un clavier USB marche aussi.", "On the keypad, with the controller; a USB keyboard works too."));
        password(app, cv, f, 96.0, y + 24.0);
        footer(app, cv, &[(&["A"], tr("Touche", "Key").into()), (&["B"], tr("Retour", "Back").into()), (&["Y"], tr("Majuscule", "Shift").into())], None);
        return;
    }
    let y = title(app, cv, 124.0 + 30.0 + 40.0, tr("Réseau", "Network"), tr("L'ordinateur et ce Raspberry Pi doivent être sur le même réseau ; Ethernet est le plus sûr.", "The computer and this Raspberry Pi must be on the same network; Ethernet is the safest."));
    let x0 = 96.0;
    let mut cy = y + 36.0;
    if app.net.kind == NetKind::Ethernet {
        let r = Rect::new(x0, cy, w - 192.0, 84.0);
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(14, 20, 26, 217));
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.5, OK);
        cv.icon(&Icon::Ethernet, r.x + 24.0, r.y + 27.0, 30.0, 1.8, OK);
        let link = app.net.link_mbps.map(|m| format!(" · {}", super::gbit_label(m))).unwrap_or_default();
        cv.text(r.x + 72.0, r.y + 50.0, Text::new(Font::Semibold, 24.0, FROST), &format!("{}{link}", tr("Connecté par Ethernet", "Connected over Ethernet")));
        cv.text(r.x + r.w - 24.0, r.y + 48.0, Text::new(Font::Mono, 17.0, MUTED).align(Align::Right), tr("Appuyez sur A pour continuer", "Press A to continue"));
        cy += 84.0 + 24.0;
        primary(cv, f, Target::SetupContinue, x0, cy, 72.0, tr("Continuer", "Continue"), false);
        footer(app, cv, &[(&["A"], tr("Continuer", "Continue").into()), (&["B"], tr("Retour", "Back").into())], None);
        return;
    }
    eyebrow(cv, x0, cy + 11.0, 15.0, FAINT, if s.wifi_scanning { tr("Réseaux Wi-Fi · recherche…", "Wi-Fi networks · searching…") } else { tr("Réseaux Wi-Fi", "Wi-Fi networks") });
    cy += 20.0 + 10.0;
    let list_w = (w - 192.0).min(1100.0);
    let max_rows = ((h - 80.0 - 40.0 - cy) / 74.0).floor().max(1.0) as usize;
    for (i, n) in s.wifi.iter().take(max_rows).enumerate() {
        let r = Rect::new(x0, cy, list_w, 66.0);
        let (focused, hover) = f.state(Target::SetupWifi(i), r);
        cv.rrect(r.x, r.y, r.w, r.h, 12.0, if hover { Rgba(20, 27, 35, 235) } else { Rgba(14, 20, 26, 200) });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 12.0, 1.0, if n.active { OK } else { LINE });
        if focused {
            widgets::ring(cv, r, 12.0);
        }
        // Le signal : quatre barres.
        let bars = (n.signal as f32 / 25.0).ceil().clamp(0.0, 4.0) as i32;
        for b in 0..4 {
            let bh = 8.0 + b as f32 * 5.0;
            cv.rrect(r.x + 24.0 + b as f32 * 9.0, r.y + 45.0 - bh, 6.0, bh, 2.0, if b < bars { SPECTRE } else { LINE_2 });
        }
        cv.text_fit(r.x + 80.0, r.y + 33.0 + 8.0, Text::new(Font::Semibold, 22.0, FROST), &n.ssid, r.w - 260.0);
        let right = Text::new(Font::Mono, 15.0, FAINT).align(Align::Right);
        let state = if n.active { tr("connecté", "connected") } else if n.secured { tr("mot de passe", "password") } else { tr("ouvert", "open") };
        cv.text(r.x + r.w - 24.0, r.y + 33.0 + 6.0, right.color(if n.active { OK } else { FAINT }), state);
        cy += 74.0;
    }
    if s.wifi.is_empty() && !s.wifi_scanning {
        cv.text(x0, cy + 24.0, Text::new(Font::Regular, 20.0, FAINT), tr("Aucun réseau vu. Rapprochez la box, ou branchez un câble Ethernet.", "No network seen. Move closer to the router, or plug in an Ethernet cable."));
        cy += 48.0;
    }
    if let Some(m) = &s.net_message {
        cv.text_fit(x0, cy + 24.0, Text::new(Font::Regular, 19.0, if s.net_busy { MUTED } else { WARN }), m, list_w);
        cy += 44.0;
    }
    cy += 12.0;
    let mut bx = x0;
    bx += widgets::ghost(cv, f, Target::SetupRescan, bx, cy, 56.0, 19.0, Some(Icon::Refresh), tr("Chercher à nouveau", "Search again"), MUTED) + 16.0;
    if app.setup.active {
        widgets::ghost(cv, f, Target::SetupSkip, bx, cy, 56.0, 19.0, None, tr("Passer, je brancherai un câble", "Skip, I will plug in a cable"), MUTED);
    }
    footer(app, cv, &[(&["A"], tr("Choisir", "Choose").into()), (&["B"], tr("Retour", "Back").into())], Some(tr("Le mot de passe se tape au pavé, à la manette.", "The password is typed on the keypad, with the controller.")));
}

/// Le champ du mot de passe et le pavé : lettres, chiffres, signes, majuscule, effacer, continuer.
fn password(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y: f32) {
    let s = &app.setup;
    let fw = 900.0;
    let fh = 64.0;
    cv.rrect(x0, y, fw, fh, 12.0, INK_2);
    cv.rrect_stroke(x0, y, fw, fh, 12.0, 1.5, SPECTRE);
    let shown: String = s.password.chars().map(|_| '•').collect();
    let shown = if s.password.is_empty() { tr("mot de passe", "password").to_string() } else { shown };
    let tw = cv.text(x0 + 22.0, y + fh / 2.0 + 9.0, Text::new(Font::Mono, 24.0, if s.password.is_empty() { GHOST } else { FROST }), &shown);
    if (app.screen_since.elapsed().as_millis() / 530) % 2 == 0 && !s.password.is_empty() {
        cv.rect(x0 + 24.0 + tw, y + 16.0, 2.0, fh - 32.0, SPECTRE);
    }
    let mut ky = y + fh + 18.0;
    if let Some(m) = &s.net_message {
        cv.text_fit(x0, ky + 8.0, Text::new(Font::Regular, 19.0, if s.net_busy { MUTED } else { WARN }), m, fw);
        ky += 36.0;
    }
    let rows: [&str; 5] = ["1234567890", "qwertyuiop", "asdfghjkl", "zxcvbnm", "-_.@!?#&+:/"];
    let (kw, kh, gap) = (64.0, 52.0, 8.0);
    for (ri, row) in rows.iter().enumerate() {
        let mut kx = x0 + ri.min(3) as f32 * 18.0 * if ri == 4 { 0.0 } else { 1.0 };
        for c in row.chars() {
            let label = if s.shift { c.to_uppercase().to_string() } else { c.to_string() };
            super::pair::key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label, 1.0);
            kx += kw + gap;
        }
        if ri == 3 {
            // Majuscule et effacer au bout de la rangée des lettres.
            let r = Rect::new(kx, ky, kw * 1.4, kh);
            let (focused, hover) = f.state(Target::KeyShift, r);
            if focused {
                widgets::ring(cv, r, 8.0);
            }
            cv.rrect(r.x, r.y, r.w, r.h, 8.0, if s.shift { INK_3 } else if hover { INK_3 } else { INK_2 });
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 8.0, 1.0, if s.shift { SPECTRE } else { LINE_2 });
            // Majuscule : une flèche vers le haut, au trait (le glyphe ⇧ manque aux polices).
            let (cx, cy, c) = (r.x + r.w / 2.0, r.y + r.h / 2.0, if s.shift { SPECTRE } else { FROST });
            cv.polyline(&[(cx - 9.0, cy + 1.0), (cx, cy - 9.0), (cx + 9.0, cy + 1.0)], 1.8, c);
            cv.line(cx, cy - 8.0, cx, cy + 9.0, 1.8, c);
            kx += r.w + gap;
            super::pair::key(cv, f, Target::KeyErase, Rect::new(kx, ky, kw * 1.6, kh), "⌫", 1.0);
        }
        ky += kh + gap;
    }
    ky += 10.0;
    primary(cv, f, Target::KeyOk, x0, ky, 56.0, tr("Se connecter", "Connect"), false);
}

// MARK: - Usage

fn usage(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let y = title(
        app,
        cv,
        124.0 + 30.0 + 40.0,
        tr("Comment utilisez-vous ce Raspberry Pi ?", "How do you use this Raspberry Pi?"),
        tr("Link s'ouvre dans l'interface qui va avec. Vous changerez d'avis quand vous voulez : Réglages de l'appareil, ou le bouton Start tenu trois secondes.", "Link opens in the matching interface. Change your mind any time: Device settings, or the Start button held for three seconds."),
    );
    let cards: [(&str, &str, &str, &str); 2] = [
        ("GAMEPAD", tr("Au salon", "In the living room"), tr("Branché au téléviseur, piloté à la manette. Tout au focus, texte lisible à trois mètres, colonne « Prêt à jouer ».", "Plugged into the TV, driven with a controller. Everything by focus, text readable from ten feet, “Ready to play” column."), tr("Manettes Bluetooth ou USB · HDMI-CEC : la télécommande allume et réveille", "Bluetooth or USB controllers · HDMI-CEC: the remote turns on and wakes")),
        ("MONITOR", tr("Au bureau", "At the desk"), tr("Sur un écran de bureau, au clavier et à la souris. L'interface Bureau de Link, en plein écran : votre ordinateur Windows comme si vous y étiez.", "On a desktop screen, with keyboard and mouse. Link's Desktop interface, full screen: your Windows computer as if you were there."), tr("Profil Bureau d'office · presse-papiers · souris du bureau", "Desktop profile by default · clipboard · desktop mouse")),
    ];
    let mut x = 96.0;
    for (i, (badge, name, desc, note)) in cards.iter().enumerate() {
        choice_card(cv, f, Target::SetupUsage(i), Rect::new(x, y + 48.0, 560.0, 380.0), badge, name, desc, note, i == app.setup.usage);
        x += 560.0 + 40.0;
    }
    let bw = legend_width(cv, 34.0, &["A"], tr("Choisir", "Choose"));
    let _ = bw;
    footer(app, cv, &[(&["A"], tr("Choisir", "Choose").into()), (&["B"], tr("Retour", "Back").into())], Some(tr("Un clavier marche aussi : flèches et Entrée", "A keyboard works too: arrows and Enter")));
    // A sur la carte choisie continue.
    let _ = Target::SetupContinue;
}
