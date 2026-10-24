//! Réglages, d'après les maquettes : sur le bureau, la colonne de gauche porte
//! le profil (une rangée de boutons), les rubriques et « Cet appareil » ; à
//! droite, la rubrique choisie, un champ par réglage (libellé, segments avec
//! leurs étiquettes, aide). À la manette (Deck, salon), les rubriques à
//! gauche, les valeurs ‹ › à droite, la légende en bas. Les lignes viennent de
//! `App::rows`, les mêmes pour les trois interfaces.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::home::{gbit, profile_label};
use super::theme::*;
use super::widgets::{self, eyebrow, legend, nav_button, segments_tagged, tag, tag_width};
use super::{launch, title_bar, unit, Frame};
use crate::app::{App, Layout, Machine, NetKind, Rect, Row, RowKind, Section, Target};
use crate::i18n::tr;
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    if app.layout == Layout::Desktop { desktop(app, cv, f) } else { console(app, cv, f) }
}

fn title(app: &App) -> String {
    if app.settings_global { "KyberNow Link".into() } else { app.selected().map(|h| h.name.clone()).unwrap_or_default() }
}

/// « Cet ordinateur » / « Cet appareil » : ce que l'app sait de la machine.
fn machine_lines(m: &Machine) -> Vec<(String, bool)> {
    let mut v = vec![(m.os.clone(), false)];
    if !m.desktop.is_empty() {
        v.last_mut().unwrap().0.push_str(&format!(" · {}", m.desktop));
    }
    if !m.model.is_empty() {
        v.push((m.model.clone(), false));
    }
    let gpu = m.gpu.split(" for ").nth(1).unwrap_or(&m.gpu).split(" (").next().unwrap_or("").trim().to_string();
    if !gpu.is_empty() {
        v.push((gpu, false));
    }
    let mut dec = Vec::new();
    if m.hw_h264 {
        dec.push("H.264");
    }
    if m.hw_hevc {
        dec.push("HEVC");
    }
    if m.hw_av1 {
        dec.push("AV1");
    }
    if m.hw_hevc444 {
        dec.push("4:4:4");
    }
    if (m.vaapi || m.pi.is_some()) && !dec.is_empty() {
        let by = if m.pi.is_some() { tr("par le décodeur du Pi", "by the Pi's decoder") } else { tr("par VA-API", "via VA-API") };
        v.push((format!("{} {} {}", tr("Décode", "Decodes"), dec.join(", "), by), true));
        let mut cpu = Vec::new();
        if !m.hw_av1 {
            cpu.push(tr("AV1 : non", "AV1: no"));
        }
        if !m.hw_hevc444 {
            cpu.push(tr("4:4:4 : processeur", "4:4:4: CPU"));
        }
        if !cpu.is_empty() {
            v.push((cpu.join(" · "), false));
        }
    } else {
        v.push((tr("Décodage par le processeur (H.264, HEVC)", "CPU decoding (H.264, HEVC)").into(), false));
    }
    let vrr = match m.vrr {
        Some(true) => " · VRR",
        _ => "",
    };
    v.push((format!("{} {} × {} · {} Hz{vrr}", tr("Écran", "Screen"), m.display.0, m.display.1, m.refresh), false));
    v.push((format!("{} {} · {} {}", tr("Son", "Sound"), m.audio_name, m.audio_channels, tr("voies", "channels")), false));
    if let Some((pct, ac)) = m.battery {
        v.push((format!("{} {pct} %{}", tr("Batterie", "Battery"), if ac { tr(", sur secteur", ", plugged in") } else { "" }), false));
    }
    v
}

// MARK: - Bureau

/// La carte « Cet appareil » de la colonne : un titre (le modèle) et des
/// lignes en Martian Mono. Sur le Pi, comme la maquette : mémoire et système,
/// sortie HDMI, réseau et température.
fn device_card(app: &App) -> (&'static str, String, Vec<String>) {
    let m = &app.machine;
    if let Some(p) = &m.pi {
        let net = match (app.net.kind, app.net.link_mbps) {
            (NetKind::Ethernet, Some(mb)) => format!("Ethernet {}", gbit(mb)),
            (NetKind::Ethernet, None) => "Ethernet".into(),
            (NetKind::Wifi, _) => match &app.net.band {
                Some(b) => format!("Wi-Fi {b}"),
                None => "Wi-Fi".into(),
            },
            _ => tr("Réseau", "Network").into(),
        };
        let temp = m.temp_c.map(|t| format!(" · {} °C", t.round() as i32)).unwrap_or_default();
        let lines = vec![
            format!("{} {} · {}", p.ram_gb, tr("Go", "GB"), m.os),
            format!("{} · {} {} Hz {}", p.hdmi(), Machine::height_label(p.max_height()), m.max_hz(p.max_height()), tr("au plus", "at most")),
            format!("{net}{temp}"),
        ];
        return (tr("Cet appareil", "This device"), p.model.clone(), lines);
    }
    let lines = machine_lines(m);
    if m.model.is_empty() {
        // Sans modèle, le système fait le titre ; le bureau prend la première ligne.
        let mut rest: Vec<String> = lines.into_iter().skip(1).map(|(l, _)| l).collect();
        if !m.desktop.is_empty() {
            rest.insert(0, m.desktop.clone());
        }
        (tr("Cet ordinateur", "This computer"), m.os.clone(), rest)
    } else {
        (tr("Cet ordinateur", "This computer"), m.model.clone(), lines.into_iter().map(|(l, _)| l).filter(|l| *l != m.model).collect())
    }
}

/// La phrase sous le titre de la rubrique : le profil et l'ordinateur, puis
/// ce que règle la rubrique.
fn subtitle(app: &App, s: Section) -> String {
    let lead = match app.selected() {
        Some(h) if !app.settings_global => format!("{} {} {} {}. ", tr("Profil", "Profile"), profile_label(h.settings.profile), tr("sur", "on"), h.name),
        _ => String::new(),
    };
    let who = if app.machine.pi.is_some() { tr("ce Raspberry Pi", "this Raspberry Pi") } else { tr("cet ordinateur", "this computer") };
    let rest: String = match s {
        Section::Picture => format!("{} {} {}", tr("Les choix grisés dépassent ce que", "Greyed-out choices go beyond what"), who, tr("sait décoder ou afficher.", "can decode or display.")),
        Section::Sound => tr("Le son de l'ordinateur sort sur cet appareil ; Auto suit la sortie en service.", "The computer's sound plays on this device; Auto follows the output in use.").into(),
        Section::Controllers => tr("Jusqu'à quatre manettes, vues par l'ordinateur comme vous le choisissez.", "Up to four controllers, seen by the computer as you choose.").into(),
        Section::Inputs => tr("La souris et le clavier de cet appareil, envoyés à l'ordinateur.", "This device's mouse and keyboard, sent to the computer.").into(),
        Section::Network => tr("Comment les paquets voyagent, et ce que fait l'app quand le réseau coupe.", "How packets travel, and what the app does when the network drops.").into(),
        Section::Account => tr("Le compte de cet appareil sur l'ordinateur, accordé par le code.", "This device's account on the computer, granted by the code.").into(),
        Section::App => tr("Langue, interface et mises à jour de l'app sur cet appareil.", "Language, interface and updates of the app on this device.").into(),
        Section::Gamepads => tr("Les manettes que voit cet appareil.", "The controllers this device sees.").into(),
        _ => String::new(),
    };
    format!("{lead}{rest}")
}

fn desktop(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    let top = if app.fullscreen { 0.0 } else { 40.0 };
    let nav_w = 232.0;
    let mx = nav_w + 40.0;
    let mw = w - mx - 40.0;
    let section = app.current_section();
    // La rubrique d'abord : elle défile sous l'en-tête et la colonne.
    let mut y = top + 28.0 - app.scroll;
    cv.text_shrink(mx - 1.0, y + 25.0, Text::new(Font::Display, 28.0, FROST).tracking(-0.3), section.label(), mw);
    y += 38.0;
    y += cv.paragraph(mx, y + 13.0, Text::new(Font::Regular, 13.0, FAINT), &subtitle(app, section), mw, 19.0) + 2.0;
    let rows = app.rows(section);
    let n = rows.len();
    for (i, r) in rows.iter().enumerate() {
        y += field(cv, f, r, mx, y, mw, i + 1 == n);
    }

    // La colonne, par-dessus.
    cv.rect(0.0, top, nav_w, h - top, INK_1);
    cv.rect(nav_w - 1.0, top, 1.0, h - top, LINE);
    let nx = 16.0;
    let bw = nav_w - 32.0;
    let mut ny = top + 24.0;
    if !app.settings_global
        && let Some(hst) = app.selected()
    {
        eyebrow(cv, nx + 14.0, ny + 11.0, 11.0, FAINT, tr("Profil", "Profile"));
        ny += 21.0;
        for p in Profile::ALL {
            nav_button(cv, f, Target::Chip(p), nx, ny, bw, profile_label(p), hst.settings.profile == p);
            ny += 42.0;
        }
        ny += 16.0;
    }
    eyebrow(cv, nx + 14.0, ny + 11.0, 11.0, FAINT, if app.settings_global { tr("Réglages de l'app", "App settings") } else { tr("Réglages", "Settings") });
    ny += 21.0;
    for (i, s) in app.sections().iter().enumerate().skip(app.first_desktop_section()) {
        nav_button(cv, f, Target::Section(i), nx, ny, bw, s.label(), i == app.section);
        ny += 42.0;
    }
    // « Cet appareil », en bas de la colonne.
    let (eyebrow_text, card_title, lines) = device_card(app);
    let mono = Text::new(Font::Mono, 11.0, FAINT);
    // Les lignes passent à la ligne, comme sur la maquette.
    let n_lines: usize = lines.iter().map(|l| cv.line_count(mono, l, bw - 28.0)).sum();
    let ch = 12.0 + 13.0 + 6.0 + 16.0 + 2.0 + n_lines as f32 * 17.6 + 12.0;
    let cy = h - 24.0 - ch;
    if cy > ny + 12.0 {
        cv.rrect(nx, cy, bw, ch, 10.0, Rgba(14, 20, 26, 204));
        cv.rrect_stroke(nx, cy, bw, ch, 10.0, 1.0, LINE);
        let cx = nx + 14.0;
        eyebrow(cv, cx, cy + 12.0 + 11.0, 11.0, FAINT, eyebrow_text);
        cv.text_fit(cx, cy + 12.0 + 13.0 + 6.0 + 13.0, Text::new(Font::Semibold, 13.0, FROST), &card_title, bw - 28.0);
        let mut ly = cy + 12.0 + 13.0 + 6.0 + 16.0 + 2.0 + 12.0;
        for l in &lines {
            ly += cv.paragraph(cx, ly, mono, l, bw - 28.0, 17.6);
        }
    }
    if top > 0.0 {
        title_bar(app, cv, f);
    }
}

/// Un champ de la rubrique (maquette) : le libellé, les segments et leurs
/// étiquettes, l'aide ; ou, pour un interrupteur, une action et une
/// information, le libellé et l'aide à gauche et la commande à droite.
/// Rend sa hauteur (le filet compris).
fn field(cv: &mut Canvas, f: &mut Frame, r: &Row, x: f32, y: f32, w: f32, last: bool) -> f32 {
    let label = Text::new(Font::Semibold, 15.0, if r.enabled { FROST } else { MUTED });
    let hint = Text::new(Font::Regular, 13.0, FAINT);
    let right = x + w;
    let bottom = match &r.kind {
        RowKind::Choice { options, current, disabled, tags } => {
            cv.text_fit(x, y + 18.0 + 14.0, label, &r.label, w);
            let items: Vec<(Target, String, bool, bool, Option<crate::app::Tag>)> =
                options.iter().enumerate().map(|(i, o)| (Target::Choice(r.setting, i), o.clone(), i == *current, disabled.get(i).copied().unwrap_or(false), tags.get(i).cloned().flatten())).collect();
            let (_, sh) = segments_tagged(cv, f, x, y + 46.0, w, &items);
            let hy = y + 46.0 + sh + 10.0;
            let hh = hint_block(cv, r, hint, x, hy, w);
            hy + hh + 18.0
        }
        other => {
            let cw = match other {
                RowKind::Toggle(_) => 44.0,
                RowKind::Action(s) | RowKind::Danger(s) => ghost_w(cv, s),
                RowKind::Info(s) => cv.text_width(Text::new(Font::Mono, 12.0, MUTED), s),
                RowKind::Choice { .. } => 0.0,
            };
            let tw = w - cw - 24.0;
            cv.text_fit(x, y + 18.0 + 14.0, label, &r.label, tw);
            let hy = y + 18.0 + 18.0;
            let hh = hint_block(cv, r, hint, x, hy, tw);
            let bottom = hy + hh + 18.0;
            let fh = bottom - y;
            match other {
                RowKind::Toggle(on) => {
                    let tr_ = Rect::new(right - 44.0, y + (fh - 26.0) / 2.0, 44.0, 26.0);
                    let (focused, _) = if r.enabled { f.state(Target::Setting(r.setting), tr_) } else { (false, false) };
                    if focused {
                        widgets::ring(cv, tr_, 13.0);
                    }
                    widgets::toggle(cv, tr_.x, tr_.y, 26.0, *on, r.enabled);
                }
                RowKind::Action(s) => {
                    widgets::ghost(cv, f, Target::Setting(r.setting), right - cw, y + (fh - 36.0) / 2.0, 36.0, 13.0, None, s, MUTED);
                }
                RowKind::Danger(s) => {
                    widgets::ghost(cv, f, Target::Setting(r.setting), right - cw, y + (fh - 36.0) / 2.0, 36.0, 13.0, None, s, BAD_TEXT);
                }
                RowKind::Info(s) => {
                    cv.text(right, y + fh / 2.0 + 4.0, Text::new(Font::Mono, 12.0, MUTED).align(Align::Right), s);
                }
                RowKind::Choice { .. } => {}
            }
            bottom
        }
    };
    if !last {
        cv.rect(x, bottom, w, 1.0, LINE);
    }
    bottom + 1.0 - y
}

/// L'aide d'un champ, avec son étiquette au bout ; rend sa hauteur.
fn hint_block(cv: &mut Canvas, r: &Row, style: Text, x: f32, y: f32, w: f32) -> f32 {
    let lh = 19.5;
    let Some(t) = &r.tag else {
        return cv.paragraph(x, y + 14.0, style, &r.hint, w, lh);
    };
    let tw = tag_width(cv, t);
    let text_w = cv.text_width(style, &r.hint);
    if text_w + 8.0 + tw <= w {
        cv.text(x, y + 14.0, style, &r.hint);
        tag(cv, x + text_w + 8.0, y + 10.0, t);
        lh.max(22.0)
    } else {
        let hh = cv.paragraph(x, y + 14.0, style, &r.hint, w, lh);
        tag(cv, x, y + hh + 4.0 + 11.0, t);
        hh + 4.0 + 22.0
    }
}

fn ghost_w(cv: &mut Canvas, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, 13.0, MUTED), s) + 26.0
}

// MARK: - Manette (Deck, salon)

fn console(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let u = unit(app.layout);
    let tv = app.layout == Layout::Tv;
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    cv.glow(w * 0.1, 0.0, w * 0.5, h * 0.55, Rgba::hex(0x7A5CFF).alpha(0.2));
    let x0 = if tv { 96.0 } else { 40.0 };
    let header = 56.0 * u + if tv { 12.0 } else { 0.0 };
    let footer = 56.0 * u;
    // En-tête : logo / ordinateur / Réglages.
    let (lx, ly, lh) = launch::header_logo(app.layout);
    let lw = cv.logo_link(lx, ly, lh, 1.0);
    let by = ly + lh * 0.62;
    let mut x = lx + lw + 18.0 * u;
    x += cv.text(x, by, Text::new(Font::Mono, 14.0 * u, GHOST), "/") + 14.0 * u;
    x += cv.text_fit(x, by + 2.0 * u, Text::new(Font::Title, 20.0 * u, FROST), &title(app), 420.0 * u) + 14.0 * u;
    cv.text(x, by, Text::new(Font::Mono, 13.0 * u, FAINT), tr("Réglages", "Settings"));

    // Les rubriques.
    let nav_w = if tv { 340.0 } else { 236.0 };
    let mut ny = header + 12.0 * u;
    let item_h = if tv { 64.0 } else { 46.0 };
    for (i, s) in app.sections().iter().enumerate() {
        let r = Rect::new(x0, ny, nav_w, item_h);
        let (focused, hover) = f.state(Target::Section(i), r);
        let on = i == app.section;
        if on {
            cv.rrect(r.x, r.y, r.w, r.h, 12.0, INK_3);
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 12.0, 1.0, LINE_2);
        } else if hover {
            cv.rrect(r.x, r.y, r.w, r.h, 12.0, Rgba(227, 237, 240, 12));
        }
        if focused {
            widgets::ring(cv, r, 12.0);
        }
        cv.text(r.x + 18.0 * u, r.y + r.h / 2.0 + 6.5 * u, Text::new(Font::Regular, 17.0 * u, if on { FROST } else { MUTED }), s.label());
        ny += item_h + 4.0 * u;
    }
    // « Cette machine », sous les rubriques, s'il reste la place.
    let lines = machine_lines(&app.machine);
    let card_h = 44.0 * u + lines.len() as f32 * 19.0 * u;
    let card_y = h - footer - 24.0 * u - card_h;
    if card_y > ny + 12.0 {
        cv.rrect(x0, card_y, nav_w, card_h, 12.0, Rgba(14, 20, 26, 205));
        cv.rrect_stroke(x0, card_y, nav_w, card_h, 12.0, 1.0, LINE);
        eyebrow(cv, x0 + 16.0 * u, card_y + 26.0 * u, 11.0 * u, FAINT, tr("Cet appareil", "This device"));
        let mut ly2 = card_y + 48.0 * u;
        for (l, strong) in lines {
            cv.text_fit(x0 + 16.0 * u, ly2, Text::new(Font::Mono, 10.5 * u, if strong { FROST } else { MUTED }), &l, nav_w - 32.0 * u);
            ly2 += 19.0 * u;
        }
    }

    // Les lignes de la rubrique.
    let mx = x0 + nav_w + 44.0 * u;
    let mw = (w - mx - x0).min(1180.0);
    let rows = app.rows(app.current_section());
    let rh = if tv { 88.0 } else { 62.0 };
    let mut y = header + 8.0 * u;
    for r in &rows {
        row_console(cv, f, r, mx, y, mw, rh, u);
        y += rh;
    }

    // La légende.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx2 = x0;
    lx2 += legend(cv, lx2, gy, g, &["A"], tr("Changer", "Change")) + 28.0 * u;
    lx2 += legend(cv, lx2, gy, g, &["B"], tr("Retour", "Back")) + 28.0 * u;
    legend(cv, lx2, gy, g, &["L1", "R1"], tr("Rubrique", "Section"));
}

/// Une ligne de réglage à la manette : libellé et aide à gauche, valeur à droite.
#[allow(clippy::too_many_arguments)]
pub(super) fn row_console(cv: &mut Canvas, f: &mut Frame, r: &Row, x: f32, y: f32, w: f32, h: f32, u: f32) {
    let rect = Rect::new(x - 14.0 * u, y + 4.0, w + 28.0 * u, h - 8.0);
    let selectable = r.enabled && !matches!(r.kind, RowKind::Info(_));
    let (focused, hover) = if selectable { f.state(Target::Setting(r.setting), rect) } else { (false, false) };
    if focused {
        cv.rrect(rect.x, rect.y, rect.w, rect.h, 12.0, Rgba(30, 28, 56, 230));
        widgets::ring(cv, rect, 12.0);
    } else if hover {
        cv.rrect(rect.x, rect.y, rect.w, rect.h, 12.0, Rgba(227, 237, 240, 10));
    }
    let right = x + w;
    let value = Text::new(Font::Mono, 17.0 * u, if r.enabled { FROST } else { DIM });
    let arrows = Text::new(Font::Mono, 17.0 * u, if focused { SPECTRE } else { GHOST });
    let vy = y + h / 2.0 + 6.0 * u;
    let cw = match &r.kind {
        RowKind::Choice { options, current, .. } => {
            let w1 = cv.text(right, vy, arrows.align(Align::Right), "›");
            let w2 = cv.text(right - w1 - 16.0 * u, vy, value.align(Align::Right), &options[*current]);
            let w3 = cv.text(right - w1 - w2 - 32.0 * u, vy, arrows.align(Align::Right), "‹");
            w1 + w2 + w3 + 32.0 * u
        }
        RowKind::Toggle(on) => {
            let th = 30.0 * u;
            widgets::toggle(cv, right - th * 1.7, y + (h - th) / 2.0, th, *on, r.enabled);
            th * 1.7
        }
        RowKind::Action(s) => {
            let w1 = cv.text(right, vy, arrows.align(Align::Right), "›");
            w1 + 14.0 * u + cv.text(right - w1 - 14.0 * u, vy, value.align(Align::Right), s)
        }
        RowKind::Danger(s) => cv.text(right, vy, Text::new(Font::Semibold, 17.0 * u, BAD_TEXT).align(Align::Right), s),
        RowKind::Info(s) => cv.text(right, vy, Text::new(Font::Mono, 15.0 * u, MUTED).align(Align::Right), s),
    };
    let tw = w - cw - 32.0 * u;
    cv.text_fit(x, y + h / 2.0 - 2.0 * u, Text::new(Font::Regular, 17.0 * u, if r.enabled { FROST } else { MUTED }), &r.label, tw);
    cv.text_fit(x, y + h / 2.0 + 19.0 * u, Text::new(Font::Mono, 11.5 * u, FAINT), &r.hint, tw);
    cv.rect(x, y + h - 1.0, w, 1.0, LINE.alpha(0.7));
}
