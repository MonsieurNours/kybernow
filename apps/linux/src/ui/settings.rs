//! Réglages, d'après les maquettes : sur le bureau, la colonne de gauche porte
//! le profil (une rangée de boutons) et les rubriques ; à droite, la rubrique
//! choisie, un champ par réglage (libellé, segments avec leurs étiquettes,
//! aide). À la manette (Deck, salon), les rubriques à gauche, les valeurs ‹ ›
//! à droite, la légende en bas. Les lignes viennent de `App::rows`, les mêmes
//! pour les trois interfaces ; Statistiques et Station ajoutent leur carte
//! (`ui/station.rs`). Réglages de l'app : la carte « Cet appareil » dit ce
//! que l'app a trouvé (décodeurs, écran, son ; le Raspberry Pi).

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::home::{gbit, profile_label};
use super::theme::*;
use super::widgets::{self, eyebrow, legend, nav_button, segments_tagged, tag, tag_width};
use super::{launch, station, title_bar, unit, Frame};
use crate::app::{App, ButtonStyle, Confirm, Layout, Machine, NetKind, Rect, Row, RowKind, Section, StationKind, Target};
use crate::i18n::tr;
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    if app.layout == Layout::Desktop { desktop(app, cv, f) } else { console(app, cv, f) }
    if app.confirm.is_some() {
        confirm(app, cv, f);
    }
}

/// « Cet appareil » : ce que l'app sait de la machine qui la fait tourner.
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
    v.push((decoders(m), true));
    let vrr = match m.vrr {
        Some(true) => tr(", fréquence variable", ", variable refresh"),
        _ => "",
    };
    v.push((format!("{} {} × {} · {} Hz{vrr}", tr("Écran", "Screen"), m.display.0, m.display.1, m.refresh), false));
    v.push((format!("{} {} · {} {}", tr("Son", "Sound"), m.audio_name, m.audio_channels, tr("voies", "channels")), false));
    if let Some((pct, ac)) = m.battery {
        v.push((format!("{} {pct} %{}", tr("Batterie", "Battery"), if ac { tr(", sur secteur", ", plugged in") } else { "" }), false));
    }
    v
}

/// Ce que cet appareil décode, et par quoi : « Décode HEVC, H.264 par la
/// carte graphique », « HEVC par le décodeur du Pi », « Décodage par le processeur ».
fn decoders(m: &Machine) -> String {
    let mut dec = Vec::new();
    if m.hw_hevc {
        dec.push("HEVC");
    }
    if m.hw_h264 {
        dec.push("H.264");
    }
    if m.hw_av1 {
        dec.push("AV1");
    }
    if m.hw_hevc444 {
        dec.push("4:4:4");
    }
    if (m.vaapi || m.pi.is_some()) && !dec.is_empty() {
        let by = if m.pi.is_some() { tr("par le décodeur du Pi", "by the Pi's decoder") } else { tr("par la carte graphique", "by the graphics card") };
        format!("{} {} {by}", tr("Décode", "Decodes"), dec.join(", "))
    } else {
        tr("Décodage par le processeur (H.264, HEVC)", "Decoding by the processor (H.264, HEVC)").into()
    }
}

// MARK: - Bureau

/// La carte « Cet appareil » de la colonne (Réglages de l'app) : un titre (le
/// modèle) et des lignes en Martian Mono. Sur le Pi, comme la maquette :
/// mémoire et système, décodeurs, sortie HDMI, réseau et température.
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
            decoders(m),
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
        (tr("Cet appareil", "This device"), m.os.clone(), rest)
    } else {
        (tr("Cet appareil", "This device"), m.model.clone(), lines.into_iter().map(|(l, _)| l).filter(|l| *l != m.model).collect())
    }
}

/// La phrase sous le titre de la rubrique : le profil et l'ordinateur, puis
/// ce que règle la rubrique.
fn subtitle(app: &App, s: Section) -> String {
    let lead = match app.selected() {
        Some(h) if !app.settings_global && matches!(s, Section::Picture | Section::Sound | Section::Controls | Section::Network) => {
            let p = if h.settings.custom() { tr("Personnalisé", "Custom") } else { profile_label(h.settings.profile) };
            format!("{} {p} {} {}. ", tr("Profil", "Profile"), tr("sur", "on"), h.name)
        }
        _ => String::new(),
    };
    let who = if app.machine.pi.is_some() { tr("ce Raspberry Pi", "this Raspberry Pi") } else { tr("cet appareil", "this device") };
    let rest: String = match s {
        Section::Picture => format!("{} {} {}", tr("Les choix grisés dépassent ce que", "Greyed-out choices go beyond what"), who, tr("sait décoder ou afficher.", "can decode or display.")),
        Section::Sound => tr("Le son de l'ordinateur sort sur cet appareil.", "The computer's sound plays on this device.").into(),
        Section::Controls => tr("Les manettes, la souris et le clavier de cet appareil, tels que l'ordinateur les voit.", "This device's controllers, mouse and keyboard, as the computer sees them.").into(),
        Section::Network => tr("Comment l'image voyage, et ce que fait l'app quand le réseau coupe.", "How the picture travels, and what the app does when the network drops.").into(),
        Section::Statistics => tr("La dernière session sur cet appareil, et ce que la Station dit de l'ordinateur.", "The last session on this device, and what the Station says about the computer.").into(),
        Section::Station => tr("KyberNow Station, sur l'ordinateur : Admin, redémarrage, mise à jour.", "KyberNow Station, on the computer: Admin, restart, update.").into(),
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
    let host = app.selected().filter(|_| !app.settings_global);
    match (section, host) {
        (Section::Statistics, Some(hst)) => {
            y += 22.0;
            let cw = mw.min(680.0);
            y += station::last_session(cv, hst, mx, y, cw, 1.0);
            y += 12.0;
            y += cv.paragraph(mx, y + 13.0, Text::new(Font::Regular, 13.0, FAINT), station::ruler_help(), cw, 19.0);
            y += 34.0;
            eyebrow(cv, mx, y + 11.0, 11.0, FAINT, tr("Cet ordinateur", "This computer"));
            y += 12.0;
        }
        (Section::Station, Some(hst)) => {
            y += 22.0;
            y += station::station_block(app, cv, hst, mx, y, mw.min(760.0), 1.0);
            // KyberNow Link OS au bureau : pas de navigateur, le QR code d'Admin.
            if app.admin_by_qr() {
                y += station::admin_qr(cv, hst, mx, y + 8.0, 300.0, 1.0) + 16.0;
            }
            cv.rect(mx, y, mw, 1.0, LINE);
            y += 1.0;
        }
        _ => {}
    }
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
    if let Some(hst) = host {
        eyebrow(cv, nx + 14.0, ny + 11.0, 11.0, FAINT, tr("Profil", "Profile"));
        ny += 21.0;
        let custom = hst.settings.custom();
        for p in Profile::ALL {
            nav_button(cv, f, Target::Chip(p), nx, ny, bw, profile_label(p), !custom && hst.settings.profile == p);
            ny += 42.0;
        }
        if custom {
            // « Personnalisé » : l'état des réglages, pas un choix.
            cv.rrect(nx, ny, bw, 40.0, 8.0, INK_3);
            cv.rrect_stroke(nx, ny, bw, 40.0, 8.0, 1.0, LINE_2);
            cv.text_fit(nx + 14.0, ny + 20.0 + 15.0 * 0.36, Text::new(Font::Regular, 15.0, FROST), tr("Personnalisé", "Custom"), bw - 28.0);
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
    // « Cet appareil », en bas de la colonne des réglages de l'app.
    if app.settings_global {
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
    }
    if top > 0.0 {
        title_bar(app, cv, f);
    }
}

/// Un champ de la rubrique (maquette) : le libellé, les segments et leurs
/// étiquettes, l'aide ; un bouton qui dit ce qu'il fait, puis son aide ; ou,
/// pour un interrupteur, une action et une information, le libellé et l'aide
/// à gauche et la commande à droite. Rend sa hauteur (le filet compris).
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
        RowKind::Button { text, style } => {
            let by = y + 18.0;
            let bh = 40.0;
            match (style, r.enabled) {
                (ButtonStyle::Primary, true) => {
                    widgets::primary(cv, f, Target::Setting(r.setting), x, by, bh, text, false);
                }
                (_, true) => {
                    widgets::ghost(cv, f, Target::Setting(r.setting), x, by, bh, 14.0, None, text, if *style == ButtonStyle::Danger { BAD_TEXT } else { FROST });
                }
                // Indisponible : le bouton se voit, sans cible.
                (_, false) => {
                    let st = Text::new(Font::Regular, 14.0, GHOST);
                    let bw = cv.text_width(st, text) + 28.0;
                    cv.rrect_stroke(x, by, bw, bh, 8.0, 1.0, LINE);
                    cv.text(x + 14.0, by + bh / 2.0 + 14.0 * 0.36, st, text);
                }
            }
            let hy = by + bh + 6.0;
            let hh = hint_block(cv, r, hint, x, hy, w.min(760.0));
            hy + hh + 18.0
        }
        other => {
            let cw = match other {
                RowKind::Toggle(_) => 44.0,
                RowKind::Action(s) => ghost_w(cv, s),
                RowKind::Info(s) => cv.text_width(Text::new(Font::Mono, 12.0, MUTED), s),
                _ => 0.0,
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
                RowKind::Info(s) => {
                    cv.text(right, y + fh / 2.0 + 4.0, Text::new(Font::Mono, 12.0, MUTED).align(Align::Right), s);
                }
                _ => {}
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
    if r.hint.is_empty() && r.tag.is_none() {
        return 0.0;
    }
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
    // En-tête : logo / Réglages de {nom} (ou de l'app).
    let (lx, ly, lh) = launch::header_logo(app.layout);
    let lw = cv.logo_link(lx, ly, lh, 1.0);
    let by = ly + lh * 0.62;
    let mut x = lx + lw + 18.0 * u;
    x += cv.text(x, by, Text::new(Font::Mono, 14.0 * u, GHOST), "/") + 14.0 * u;
    let title = match app.selected() {
        Some(hst) if !app.settings_global => crate::i18n::settings_for(&hst.name),
        _ => tr("Réglages de l'app", "App settings").into(),
    };
    cv.text_fit(x, by + 2.0 * u, Text::new(Font::Title, 20.0 * u, FROST), &title, w - x - x0);

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
    // « Cet appareil », sous les rubriques des réglages de l'app, s'il reste la place.
    if app.settings_global {
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
    }

    // La rubrique.
    let mx = x0 + nav_w + 44.0 * u;
    let mut mw = (w - mx - x0).min(1180.0);
    let section = app.current_section();
    let rh = if tv { 88.0 } else { 62.0 };
    let mut y = header + 8.0 * u;
    let host = app.selected().filter(|_| !app.settings_global);
    match (section, host) {
        (Section::Statistics, Some(hst)) => {
            y += 6.0 * u;
            y += station::last_session(cv, hst, mx, y, mw, u);
            y += 10.0 * u;
            y += cv.paragraph(mx, y + 13.0 * u, Text::new(Font::Regular, 13.0 * u, FAINT), station::ruler_help(), mw, 19.0 * u);
            y += 26.0 * u;
            eyebrow(cv, mx, y + 11.0 * u, 11.0 * u, FAINT, tr("Cet ordinateur", "This computer"));
            y += 16.0 * u;
        }
        (Section::Station, Some(hst)) => {
            // Le QR code d'Admin à droite, l'état et les boutons à gauche.
            let qw = if tv { 400.0 } else { 236.0 };
            let qx = w - x0 - qw;
            mw = mw.min(qx - 32.0 * u - mx);
            station::admin_qr(cv, hst, qx, y + 14.0 * u, qw, u);
            y += 6.0 * u;
            y += station::station_block(app, cv, hst, mx, y, mw, u);
        }
        _ => {}
    }
    for r in &app.rows(section) {
        y += row_console(cv, f, r, mx, y, mw, rh, u);
    }

    // La légende.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx2 = x0;
    if app.confirm.is_some() {
        lx2 += legend(cv, lx2, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
        legend(cv, lx2, gy, g, &["B"], tr("Annuler", "Cancel"));
        return;
    }
    lx2 += legend(cv, lx2, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
    lx2 += legend(cv, lx2, gy, g, &["B"], tr("Retour", "Back")) + 28.0 * u;
    legend(cv, lx2, gy, g, &["L1", "R1"], tr("Rubrique", "Section"));
}

/// Une ligne de réglage à la manette : libellé et aide à gauche, valeur à
/// droite. L'aide passe à la ligne plutôt que d'être coupée : la ligne
/// grandit d'autant. Rend sa hauteur (`h` au moins).
#[allow(clippy::too_many_arguments)]
pub(super) fn row_console(cv: &mut Canvas, f: &mut Frame, r: &Row, x: f32, y: f32, w: f32, h: f32, u: f32) -> f32 {
    let right = x + w;
    let value = Text::new(Font::Mono, 17.0 * u, if r.enabled { FROST } else { DIM });
    let info = Text::new(Font::Mono, 15.0 * u, MUTED);
    let hint = Text::new(Font::Mono, 11.5 * u, FAINT);
    // La largeur de la valeur d'abord : elle donne celle du libellé et de l'aide.
    let arrow_w = cv.text_width(value, "›");
    let cw = match &r.kind {
        RowKind::Choice { options, current, .. } => 2.0 * arrow_w + cv.text_width(value, &options[*current]) + 32.0 * u,
        RowKind::Toggle(_) => 30.0 * u * 1.7,
        RowKind::Action(s) => arrow_w + 14.0 * u + cv.text_width(value, s),
        RowKind::Button { .. } => arrow_w,
        RowKind::Info(s) => cv.text_width(info, s),
    };
    let tw = w - cw - 32.0 * u;
    let lines = if r.hint.is_empty() { 1 } else { cv.line_count(hint, &r.hint, tw) };
    let height = h + (lines.saturating_sub(1)) as f32 * 18.0 * u;
    let rect = Rect::new(x - 14.0 * u, y + 4.0, w + 28.0 * u, height - 8.0);
    let selectable = r.enabled && !matches!(r.kind, RowKind::Info(_));
    let (focused, hover) = if selectable { f.state(Target::Setting(r.setting), rect) } else { (false, false) };
    if focused {
        cv.rrect(rect.x, rect.y, rect.w, rect.h, 12.0, Rgba(30, 28, 56, 230));
        widgets::ring(cv, rect, 12.0);
    } else if hover {
        cv.rrect(rect.x, rect.y, rect.w, rect.h, 12.0, Rgba(227, 237, 240, 10));
    }
    let arrows = Text::new(Font::Mono, 17.0 * u, if focused { SPECTRE } else { GHOST });
    let vy = y + h / 2.0 + 6.0 * u;
    let mut label_color = if r.enabled { FROST } else { GHOST };
    match &r.kind {
        RowKind::Choice { options, current, .. } => {
            let w1 = cv.text(right, vy, arrows.align(Align::Right), "›");
            let w2 = cv.text(right - w1 - 16.0 * u, vy, value.align(Align::Right), &options[*current]);
            cv.text(right - w1 - w2 - 32.0 * u, vy, arrows.align(Align::Right), "‹");
        }
        RowKind::Toggle(on) => {
            let th = 30.0 * u;
            widgets::toggle(cv, right - th * 1.7, y + (h - th) / 2.0, th, *on, r.enabled);
        }
        RowKind::Action(s) => {
            let w1 = cv.text(right, vy, arrows.align(Align::Right), "›");
            cv.text(right - w1 - 14.0 * u, vy, value.align(Align::Right), s);
        }
        // Un bouton : son texte en libellé, « › » à droite ; Retirer en rouge.
        RowKind::Button { style, .. } => {
            if r.enabled {
                label_color = match style {
                    ButtonStyle::Danger => BAD_TEXT,
                    ButtonStyle::Primary => SPECTRE,
                    ButtonStyle::Ghost => FROST,
                };
                cv.text(right, vy, arrows.align(Align::Right), "›");
            }
        }
        RowKind::Info(s) => {
            cv.text(right, vy, info.align(Align::Right), s);
        }
    }
    // Sans aide, le libellé se centre sur la ligne de la valeur.
    let label_y = if r.hint.is_empty() { vy } else { y + h / 2.0 - 2.0 * u };
    cv.text_fit(x, label_y, Text::new(Font::Regular, 17.0 * u, label_color), &r.label, tw);
    cv.paragraph(x, y + h / 2.0 + 19.0 * u, hint, &r.hint, tw, 18.0 * u);
    cv.rect(x, y + height - 1.0, w, 1.0, LINE.alpha(0.7));
    height
}

// MARK: - Confirmation

/// La confirmation par-dessus les réglages (docs/link-ui.md) : retirer
/// l'ordinateur ; redémarrer ou mettre à jour quand un appareil est en
/// session. Seuls ses deux boutons se choisissent.
fn confirm(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(c) = &app.confirm else { return };
    let u = if app.layout == Layout::Desktop { 1.0 } else { unit(app.layout) };
    let (w, h) = (cv.w, cv.h);
    f.hits.clear();
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 190));
    let (title, message, yes, danger) = match c {
        Confirm::Remove { name, .. } => (
            format!("{} {name} {}", tr("Retirer", "Remove"), tr("de cet appareil ?", "from this device?")),
            tr("Vous pourrez l'ajouter à nouveau.", "You can add it again later.").to_string(),
            tr("Retirer", "Remove"),
            true,
        ),
        Confirm::Force { kind, device, .. } => (
            format!("{device} {}", tr("est connecté", "is connected")),
            if *kind == StationKind::Update { tr("La mise à jour coupera sa session.", "The update will end its session.") } else { tr("Redémarrer la Station coupera sa session.", "Restarting the Station will end its session.") }.to_string(),
            if *kind == StationKind::Update { tr("Mettre à jour quand même", "Update anyway") } else { tr("Redémarrer quand même", "Restart anyway") },
            false,
        ),
    };
    let cw = (560.0 * u).min(w - 48.0);
    let pad = 32.0 * u;
    let title_style = Text::new(Font::Display, 26.0 * u, FROST).tracking(-0.3 * u);
    let text = Text::new(Font::Regular, 16.0 * u, MUTED);
    let title_lines = cv.line_count(title_style, &title, cw - 2.0 * pad) as f32;
    let msg_lines = cv.line_count(text, &message, cw - 2.0 * pad) as f32;
    let bh = if app.layout == Layout::Desktop { 44.0 } else { 52.0 * u };
    let ch = pad + 60.0 * u + (title_lines - 1.0) * 34.0 * u + (msg_lines - 1.0) * 24.0 * u + 28.0 * u + bh + pad;
    let (x, y) = ((w - cw) / 2.0, (h - ch) / 2.0);
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, if danger { BAD_LINE } else { LINE_2 });
    let ix = x + pad;
    let mut yy = y + pad + 26.0 * u;
    yy += cv.paragraph(ix, yy, title_style, &title, cw - 2.0 * pad, 34.0 * u) - 34.0 * u + 12.0 * u + 22.0 * u;
    yy += cv.paragraph(ix, yy, text, &message, cw - 2.0 * pad, 24.0 * u) - 24.0 * u + 28.0 * u;
    let by = yy;
    let size = if app.layout == Layout::Desktop { 14.0 } else { 17.0 * u };
    // Les boutons à droite : Annuler, puis l'action.
    let no = tr("Annuler", "Cancel");
    let yes_w = cv.text_width(Text::new(Font::Regular, size, FROST), yes) + size * 2.0;
    let no_w = cv.text_width(Text::new(Font::Regular, size, FROST), no) + size * 2.0;
    let right = x + cw - pad;
    widgets::ghost(cv, f, Target::ConfirmYes, right - yes_w, by, bh, size, None, yes, if danger { BAD_TEXT } else { FROST });
    widgets::ghost(cv, f, Target::ConfirmNo, right - yes_w - 12.0 * u - no_w, by, bh, size, None, no, MUTED);
}
