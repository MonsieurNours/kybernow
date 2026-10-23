//! Réglages, d'après les maquettes : sur le bureau, l'ordinateur à gauche
//! (et « Cet ordinateur »), les réglages en lignes à droite avec leurs
//! segments ; à la manette (Deck, salon), les rubriques à gauche, les valeurs
//! ‹ › à droite, la légende en bas. Les lignes viennent de `App::rows`,
//! les mêmes pour les trois interfaces.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, segments};
use super::{launch, title_bar, unit, Frame};
use crate::app::{App, Layout, Machine, Rect, Row, RowKind, Section, Target};
use crate::i18n::tr;

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
    if m.vaapi && !dec.is_empty() {
        v.push((format!("{} {} {}", tr("Décode", "Decodes"), dec.join(", "), tr("par VA-API", "via VA-API")), true));
        if !m.hw_av1 {
            v.push((tr("AV1, 4:4:4 : non", "AV1, 4:4:4: no").into(), false));
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

fn desktop(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    let top = if app.fullscreen { 0.0 } else { 40.0 };
    let aside = 380.0f32.min(w * 0.34);
    // Les lignes d'abord (elles défilent sous la barre de titre).
    let mx = aside + 48.0;
    let mw = w - mx - 48.0;
    let sections = app.sections();
    let mut y = top + 44.0 - app.scroll;
    let start = y;
    for (i, s) in sections.iter().enumerate() {
        if *s == Section::Gamepads && app.settings_global {
            // Les manettes : une ligne dans la rubrique de l'app.
        }
        let rows = app.rows(*s);
        if rows.is_empty() {
            continue;
        }
        if i > 0 {
            y += 22.0;
        }
        eyebrow(cv, mx, y + 13.0, 13.0, SPECTRE, s.label());
        y += 20.0;
        let n = rows.len();
        for (j, r) in rows.iter().enumerate() {
            y += row_desktop(app, cv, f, r, mx, y, mw, j + 1 == n);
        }
    }
    let content = y - start + 40.0;
    let _ = content;
    // La colonne de gauche, par-dessus.
    cv.rect(0.0, top, aside, h - top, INK_BG);
    cv.glow(aside * 0.2, top + 40.0, aside * 1.2, 300.0, Rgba::hex(0x7A5CFF).alpha(0.22));
    cv.rect(aside - 1.0, top, 1.0, h - top, LINE);
    let ax = 40.0;
    let mut ay = top + 44.0;
    widgets::ghost(cv, f, Target::Back, ax, ay, 40.0, 14.0, Some(Icon::Back), if app.back_to == crate::app::Screen::Session { tr("Session", "Session") } else { tr("Accueil", "Home") }, MUTED);
    ay += 66.0;
    eyebrow(cv, ax, ay, 11.0, FAINT, if app.settings_global { tr("Réglages de l'app", "App settings") } else { tr("Réglages de l'ordinateur", "Computer settings") });
    ay += 46.0;
    cv.text_shrink(ax - 1.0, ay, Text::new(Font::Display, 40.0, FROST).tracking(-0.8), &title(app), aside - 2.0 * ax);
    ay += 26.0;
    let mono = Text::new(Font::Mono, 12.0, MUTED);
    if !app.settings_global
        && let Some(hst) = app.selected()
    {
        let mut lines = Vec::new();
        if let Some(os) = &hst.os {
            lines.push(os.clone());
        }
        if let Some(g) = &hst.gpu {
            lines.push(g.clone());
        }
        lines.push(format!("{} · Admin {}", hst.address, hst.port));
        lines.push(format!("{} {}", tr("compte", "account"), hst.user));
        for l in lines {
            cv.text_fit(ax, ay, mono, &l, aside - 2.0 * ax);
            ay += 21.0;
        }
    } else {
        cv.text_fit(ax, ay, mono, &format!("{} {}", tr("Version", "Version"), app.version), aside - 2.0 * ax);
    }
    // « Cet ordinateur ».
    let lines = machine_lines(&app.machine);
    let ch = 50.0 + lines.len() as f32 * 21.0;
    let cy = h - 40.0 - ch;
    let cw = aside - 2.0 * ax;
    cv.rrect(ax, cy, cw, ch, 10.0, Rgba(14, 20, 26, 205));
    cv.rrect_stroke(ax, cy, cw, ch, 10.0, 1.0, LINE);
    eyebrow(cv, ax + 16.0, cy + 26.0, 11.0, FAINT, tr("Cet ordinateur", "This computer"));
    let mut ly = cy + 50.0;
    for (l, strong) in lines {
        cv.text_fit(ax + 16.0, ly, Text::new(Font::Mono, 11.5, if strong { FROST } else { MUTED }), &l, cw - 32.0);
        ly += 21.0;
    }
    if top > 0.0 {
        title_bar(app, cv, f);
    }
}

/// Une ligne de réglage du bureau ; rend sa hauteur.
fn row_desktop(app: &App, cv: &mut Canvas, f: &mut Frame, r: &Row, x: f32, y: f32, w: f32, last: bool) -> f32 {
    let h = 62.0;
    let label = Text::new(Font::Regular, 15.0, if r.enabled { FROST } else { MUTED });
    let hint = Text::new(Font::Regular, 12.0, FAINT);
    let right = x + w;
    // La commande, à droite ; sa largeur limite le texte.
    let cw = match &r.kind {
        RowKind::Choice { options, current, disabled } => {
            let items: Vec<(Target, String, bool, bool)> = options.iter().enumerate().map(|(i, o)| (Target::Choice(r.setting, i), o.clone(), i == *current, disabled.get(i).copied().unwrap_or(false))).collect();
            let style = Text::new(Font::Regular, 13.0, MUTED);
            let est: f32 = options.iter().map(|o| cv.text_width(style, o) + 13.0 * 2.3 + 3.0).sum::<f32>() + 6.0;
            if est <= w * 0.62 {
                segments(cv, f, right - est, y + (h - 40.0) / 2.0, 40.0, 13.0, &items)
            } else {
                stepper(cv, f, r, right, y, h, &options[*current])
            }
        }
        RowKind::Toggle(on) => {
            let tr_ = Rect::new(right - 46.0, y + (h - 26.0) / 2.0, 46.0, 26.0);
            let (focused, _) = if r.enabled { f.state(Target::Setting(r.setting), tr_) } else { (false, false) };
            if focused {
                widgets::ring(cv, tr_, 13.0);
            }
            widgets::toggle(cv, tr_.x, tr_.y, 26.0, *on, r.enabled);
            46.0
        }
        RowKind::Action(s) => {
            let gw = ghost_w(cv, s);
            widgets::ghost(cv, f, Target::Setting(r.setting), right - gw, y + (h - 38.0) / 2.0, 38.0, 13.0, None, s, MUTED)
        }
        RowKind::Danger(s) => {
            let gw = ghost_w(cv, s);
            widgets::ghost(cv, f, Target::Setting(r.setting), right - gw, y + (h - 38.0) / 2.0, 38.0, 13.0, None, s, BAD_TEXT)
        }
        RowKind::Info(s) => cv.text(right, y + h / 2.0 + 5.0, Text::new(Font::Mono, 12.0, MUTED).align(Align::Right), s),
    };
    let tw = w - cw - 24.0;
    cv.text_fit(x, y + 26.0, label, &r.label, tw);
    cv.text_fit(x, y + 45.0, hint, &r.hint, tw);
    if !last {
        cv.rect(x, y + h - 1.0, w, 1.0, LINE);
    }
    let _ = app;
    h
}

fn ghost_w(cv: &mut Canvas, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, 13.0, MUTED), s) + 26.0
}

/// ‹ valeur › quand les segments ne tiennent pas ; rend la largeur.
fn stepper(cv: &mut Canvas, f: &mut Frame, r: &Row, right: f32, y: f32, h: f32, value: &str) -> f32 {
    let st = Text::new(Font::Mono, 13.0, FROST);
    let w = cv.text_width(st, value) + 64.0;
    let rect = Rect::new(right - w, y + (h - 40.0) / 2.0, w, 40.0);
    let (focused, hover) = f.state(Target::Setting(r.setting), rect);
    if focused {
        widgets::ring(cv, rect, 8.0);
    }
    cv.rrect(rect.x, rect.y, rect.w, rect.h, 8.0, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(rect.x, rect.y, rect.w, rect.h, 8.0, 1.0, LINE_2);
    cv.text(rect.x + 14.0, rect.y + 26.0, Text::new(Font::Mono, 13.0, GHOST), "‹");
    cv.text(rect.x + w / 2.0, rect.y + 25.0, st.align(Align::Center), value);
    cv.text(rect.x + w - 14.0, rect.y + 26.0, Text::new(Font::Mono, 13.0, GHOST).align(Align::Right), "›");
    w
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
fn row_console(cv: &mut Canvas, f: &mut Frame, r: &Row, x: f32, y: f32, w: f32, h: f32, u: f32) {
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
