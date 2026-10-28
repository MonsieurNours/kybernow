//! Accueil (docs/link-ui.md, « Accueil ») : l'ordinateur choisi en plein
//! cadre (la dernière image de sa dernière session, sinon l'aperçu d'Admin,
//! sinon sa signature), son état, son nom, son système, l'écran que la session
//! demandera, « Se connecter », les profils et leur phrase, « Réglages » ; au
//! salon, les manettes. Rien de technique : les chiffres sont dans
//! Réglages › Statistiques. En bas les ordinateurs (nom et état) ; à la
//! manette, la légende des boutons.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, card, eyebrow, legend, legend_width, primary, segments};
use super::{launch, shade, signature, title_bar, unit, Frame};
use crate::app::{profile_phrase, App, Layout, NetKind, Online, Rect, Target};
use crate::i18n::{decimal, english, tr};
use crate::model::{Host, Profile};

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    draw_body(app, cv, f, true);
}

/// L'accueil ; `logo` : dessiner le logo de l'en-tête (le lancement le pose lui-même).
pub fn draw_body(app: &App, cv: &mut Canvas, f: &mut Frame, logo: bool) {
    let host = app.selected();
    backdrop(app, cv, host);
    match app.layout {
        Layout::Desktop => desktop(app, cv, f, host),
        Layout::Handheld | Layout::Tv => console(app, cv, f, host, logo),
    }
}

pub fn profile_label(p: Profile) -> &'static str {
    match p {
        Profile::Jeu => tr("Jeu", "Game"),
        Profile::Bureau => tr("Bureau", "Desktop"),
        Profile::Max => "Max",
    }
}

/// Le fond : l'image de l'ordinateur (assombrie hors ligne), sinon sa signature.
pub fn backdrop(app: &App, cv: &mut Canvas, host: Option<&Host>) {
    let offline = host.and_then(|h| app.live.get(&h.id)).is_some_and(|l| l.online == Online::Offline);
    match host.and_then(|h| app.images.get(&h.id)) {
        Some(img) => {
            cv.clear(INK_BG);
            let (w, h) = (cv.w, cv.h);
            cv.image_cover(img, 0.0, 0.0, w, h, if offline { 0.35 } else { 0.92 });
        }
        None => signature(cv, host.and_then(|h| h.fingerprint.as_deref()), offline),
    }
    shade(cv);
}

/// État de l'ordinateur : texte, couleur, lueur.
fn status(app: &App, h: &Host) -> (&'static str, Rgba, bool) {
    match app.live.get(&h.id).map(|l| l.online).unwrap_or_default() {
        Online::Online => (tr("En ligne", "Online"), OK, true),
        // Joignable seulement par le relais (docs/relais.md).
        Online::Away => (tr("Hors de chez vous", "Away from home"), OK, true),
        Online::Offline => (tr("Hors ligne", "Offline"), FAINT, false),
        Online::Unknown => (tr("Recherche…", "Searching…"), FAINT, false),
    }
}

/// L'écran que la session demandera (même calcul qu'elle) : l'écran virtuel
/// à la taille de cet appareil (ou plusieurs, au bureau : le nombre d'écrans
/// que la session prendra), sur mesure, ou ceux de l'ordinateur tels
/// quels (réglage coupé, ou Station sans écran virtuel). Les chiffres en
/// Martian Mono.
fn display_parts(app: &App, h: &Host) -> Vec<(String, bool)> {
    if !app.virtual_display_usable(h) {
        return vec![(tr("Les écrans de l'ordinateur, tels quels", "The computer's own displays, as they are").into(), false)];
    }
    // Plusieurs écrans (bureau) : « 3 écrans, à la taille de ceux de cet appareil », le même calcul que la session.
    if app.multi_screens() && app.screens_plan.count >= 2 {
        let n = app.screens_plan.count.to_string();
        return if english() {
            vec![(n, true), (" displays, sized for this device's screens".into(), false)]
        } else {
            vec![(n, true), (" écrans, à la taille de ceux de cet appareil".into(), false)]
        };
    }
    let (w, hh, fps) = app.session_mode(h);
    let custom = h.settings.resolution != 0 || h.settings.fps != 0;
    let (size, hz) = (format!("{w} × {hh}"), format!("{fps} Hz"));
    let mut v = if english() {
        vec![(size, true), (" display at ".into(), false), (hz, true)]
    } else {
        vec![("Écran de ".into(), false), (size, true), (" à ".into(), false), (hz, true)]
    };
    if !custom {
        v.push((tr(", à la taille de cet écran", ", sized for this screen").into(), false));
    }
    v
}

fn display_chip(app: &App, cv: &mut Canvas, h: &Host, x: f32, y: f32, ch: f32, size: f32) -> f32 {
    let parts = display_parts(app, h);
    let refs: Vec<(&str, bool)> = parts.iter().map(|(s, m)| (s.as_str(), *m)).collect();
    widgets::info_chip(cv, x, y, ch, size, Icon::Monitor, &refs)
}

/// « 2 manettes », « 1 manette », « Aucune manette ».
fn pads_label(n: usize) -> String {
    match n {
        0 => tr("Aucune manette", "No controller").into(),
        1 => tr("1 manette", "1 controller").into(),
        n => format!("{n} {}", tr("manettes", "controllers")),
    }
}

/// Le bouton principal : se connecter ; hors ligne, réveiller l'ordinateur
/// (l'app connaît une de ses cartes réseau) puis « Réveil de l'ordinateur… »
/// le temps qu'il revienne, ou réessayer. Rend sa largeur.
fn main_button(app: &App, cv: &mut Canvas, f: &mut Frame, h: &Host, x: f32, y: f32, height: f32) -> f32 {
    let (target, label, play) = main_action(app, h);
    primary(cv, f, target, x, y, height, label, play)
}

/// Ce que fait le bouton principal (et A sur l'accueil) : cible, libellé, symbole ▶.
pub(super) fn main_action(app: &App, h: &Host) -> (Target, &'static str, bool) {
    let offline = app.live.get(&h.id).is_some_and(|l| l.online == Online::Offline);
    match (offline, h.can_wake()) {
        (true, true) if app.is_waking(&h.id) => (Target::Wake, tr("Réveil de l'ordinateur…", "Waking the computer…"), false),
        (true, true) => (Target::Wake, tr("Réveiller l'ordinateur", "Wake the computer"), false),
        (true, false) => (Target::Retry, tr("Réessayer", "Try again"), false),
        _ => (Target::Connect, tr("Se connecter", "Connect"), true),
    }
}

/// Ce que fait A sur l'accueil, pour la légende.
fn main_label(app: &App, h: &Host) -> &'static str {
    main_action(app, h).1
}

/// Les profils, et « Personnalisé » quand les réglages s'en écartent (il
/// ouvre les réglages). Rend la largeur.
fn profiles(cv: &mut Canvas, f: &mut Frame, h: &Host, x: f32, y: f32, height: f32, size: f32) -> f32 {
    let custom = h.settings.custom();
    let mut items: Vec<(Target, String, bool, bool)> = Profile::ALL.iter().map(|p| (Target::Chip(*p), profile_label(*p).to_string(), !custom && h.settings.profile == *p, false)).collect();
    if custom {
        items.push((Target::CustomChip, tr("Personnalisé", "Custom").into(), true, false));
    }
    segments(cv, f, x, y, height, size, &items)
}

fn phrase(h: &Host) -> &'static str {
    profile_phrase((!h.settings.custom()).then_some(h.settings.profile))
}

/// Une heure locale « 19:42 ».
fn clock() -> String {
    crate::app_time::local_hhmm()
}

// MARK: - Bureau

fn desktop(app: &App, cv: &mut Canvas, f: &mut Frame, host: Option<&Host>) {
    let top = title_bar(app, cv, f);
    let (w, h) = (cv.w, cv.h);
    let x0 = 48.0;
    let hero_max = (w - 2.0 * x0).min(940.0);
    let mut y = top + 44.0;
    match host {
        Some(hst) => {
            let (st, color, glow) = status(app, hst);
            widgets::dot(cv, x0 + 4.0, y + 7.0, 4.0, color, glow);
            eyebrow(cv, x0 + 16.0, y + 11.0, 11.0, color, st);
            y += 26.0;
            cv.text_shrink(x0 - 2.0, y + 46.0, Text::new(Font::Display, 60.0, FROST).tracking(-1.2), &hst.name, hero_max);
            y += 68.0;
            if let Some(os) = &hst.os {
                cv.text_fit(x0, y + 12.0, Text::new(Font::Mono, 13.0, MUTED), os, hero_max);
                y += 28.0;
            }
            display_chip(app, cv, hst, x0, y, 36.0, 14.0);
            y += 36.0 + 24.0;
            let mut x = x0;
            x += main_button(app, cv, f, hst, x, y, 52.0) + 18.0;
            x += profiles(cv, f, hst, x, y + 3.0, 46.0, 14.0) + 12.0;
            widgets::ghost(cv, f, Target::HostSettings, x, y + 3.0, 46.0, 14.0, Some(Icon::Gear), tr("Réglages", "Settings"), MUTED);
            y += 52.0 + 30.0;
            cv.text_fit(x0, y, Text::new(Font::Regular, 14.0, FAINT), phrase(hst), hero_max);
        }
        None => welcome(cv, f, x0, y, 1.0, hero_max),
    }
    // Les ordinateurs.
    let ch = 96.0;
    let cy = h - 40.0 - ch;
    eyebrow(cv, x0, cy - 14.0, 11.0, FAINT, tr("Vos ordinateurs", "Your computers"));
    cards(app, cv, f, x0, cy, 244.0, ch, 14.0, w - 2.0 * x0, 1.0, true);
}

/// Sans ordinateur : bienvenue et ajout.
fn welcome(cv: &mut Canvas, f: &mut Frame, x0: f32, y: f32, u: f32, max: f32) {
    eyebrow(cv, x0, y + 11.0 * u, 11.0 * u, FAINT, tr("Bienvenue", "Welcome"));
    let style = Text::new(Font::Display, 52.0 * u, FROST).tracking(-1.0 * u);
    cv.text_shrink(x0 - 2.0, y + 72.0 * u, style, tr("Votre ordinateur,", "Your computer,"), max);
    cv.text_shrink(x0 - 2.0, y + 130.0 * u, style, tr("sur cet appareil.", "on this device."), max);
    cv.paragraph(
        x0,
        y + 170.0 * u,
        Text::new(Font::Regular, 15.0 * u, MUTED),
        tr(
            "Installez KyberNow Station sur l'ordinateur Windows, puis ajoutez-le ici : il s'affiche en bas dès qu'il est sur le réseau.",
            "Install KyberNow Station on the Windows computer, then add it here: it shows up below as soon as it is on the network.",
        ),
        max.min(620.0 * u),
        23.0 * u,
    );
    let bw = primary(cv, f, Target::AddCard, x0, y + 226.0 * u, 52.0 * u, tr("Ajouter un ordinateur", "Add a computer"), false);
    // KyberNow Share : rejoindre le partage de quelqu'un, à côté de « Ajouter un ordinateur ».
    widgets::ghost(cv, f, Target::JoinCard, x0 + bw + 18.0 * u, y + 226.0 * u + 3.0 * u, 46.0 * u, 15.0 * u, Some(Icon::Share), tr("Rejoindre avec un code", "Join with a code"), MUTED);
}

/// La rangée des ordinateurs : enregistrés (nom et état), trouvés sur le
/// réseau, ajouter.
#[allow(clippy::too_many_arguments)]
fn cards(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y: f32, cw: f32, ch: f32, gap: f32, width: f32, u: f32, add_card: bool) {
    let fit = (((width + gap) / (cw + gap)).floor() as usize).max(1);
    let selected = app.selected().map(|h| h.id.clone());
    let mut slots = fit;
    let found = app.new_found();
    if add_card {
        slots = slots.saturating_sub(1);
    }
    let mut x = x0;
    for (i, h) in app.store.hosts.iter().enumerate() {
        if slots == 0 {
            break;
        }
        let live = app.live.get(&h.id).map(|l| l.online).unwrap_or_default();
        let (dot, line) = match live {
            Online::Online => (OK, tr("En ligne", "Online")),
            Online::Away => (OK, tr("Hors de chez vous", "Away from home")),
            Online::Offline => (GHOST, tr("Hors ligne", "Offline")),
            Online::Unknown => (FAINT, tr("Recherche…", "Searching…")),
        };
        card(cv, f, Target::Host(i), Rect::new(x, y, cw, ch), u, &h.name, Some(dot), None, &[line], selected.as_deref() == Some(&h.id), false, live == Online::Offline);
        x += cw + gap;
        slots -= 1;
    }
    for (i, fd) in found.iter().enumerate() {
        if slots == 0 {
            break;
        }
        card(cv, f, Target::Found(i), Rect::new(x, y, cw, ch), u, &fd.name, None, None, &[tr("Sur le réseau", "On your network")], false, true, false);
        x += cw + gap;
        slots -= 1;
    }
    if add_card {
        // Deux cartes dans la place d'une : « Ajouter un ordinateur » et, dessous, « Rejoindre avec un
        // code » (KyberNow Share) ; la rangée ne perd pas un ordinateur de plus.
        let gap = 8.0 * u;
        let hh = (ch - gap) / 2.0;
        action_card(cv, f, Target::AddCard, Rect::new(x, y, cw, hh), u, Icon::Plus, tr("Ajouter un ordinateur", "Add a computer"));
        action_card(cv, f, Target::JoinCard, Rect::new(x, y + hh + gap, cw, hh), u, Icon::Share, tr("Rejoindre avec un code", "Join with a code"));
    }
}

/// Une carte en pointillés, une action : son icône et son libellé, centrés.
fn action_card(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, u: f32, icon: Icon, label: &str) {
    let (focused, hover) = f.state(t, r);
    if focused {
        widgets::ring(cv, r, 10.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { Rgba(20, 27, 35, 235) } else { CARD });
    cv.rrect_dashed(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE_2);
    let size = 15.0 * u;
    let st = Text::new(Font::Regular, size, if hover { FROST } else { MUTED });
    let isz = 18.0 * u;
    let tw = (cv.text_width(st, label) + isz + 10.0 * u).min(r.w - 24.0 * u);
    let tx = r.x + (r.w - tw) / 2.0;
    cv.icon(&icon, tx, r.y + (r.h - isz) / 2.0, isz, 1.8, MUTED);
    cv.text_fit(tx + isz + 10.0 * u, r.y + r.h / 2.0 + size * 0.36, st, label, tw - isz - 10.0 * u);
}

// MARK: - Console (Deck, salon)

fn console(app: &App, cv: &mut Canvas, f: &mut Frame, host: Option<&Host>, logo: bool) {
    let u = unit(app.layout);
    let tv = app.layout == Layout::Tv;
    let (w, h) = (cv.w, cv.h);
    let x0 = if tv { 96.0 } else { 40.0 };
    let header = 56.0 * u + if tv { 12.0 } else { 0.0 };
    // En-tête : logo, réseau, batterie, heure.
    if logo {
        let (lx, ly, lh) = launch::header_logo(app.layout);
        cv.logo_link(lx, ly, lh, 1.0);
    }
    status_line(app, cv, w - x0, header / 2.0 + 5.0 * u, u);
    let hero_max = (w - 2.0 * x0).min(if tv { 1300.0 } else { 900.0 });
    let mut y = header + 36.0 * u;
    match host {
        Some(hst) => {
            let (st, color, glow) = status(app, hst);
            widgets::dot(cv, x0 + 4.0 * u, y + 8.0 * u, 4.0 * u, color, glow);
            eyebrow(cv, x0 + 16.0 * u, y + 12.0 * u, 12.0 * u, color, st);
            y += 28.0 * u;
            // ‹ nom › : la croix change d'ordinateur quand le bouton principal a le focus.
            let many = app.store.hosts.len() > 1;
            let arrows = Text::new(Font::Mono, 22.0 * u, GHOST);
            let mut nx = x0;
            if many {
                nx += cv.text(x0, y + 44.0 * u, arrows, "‹") + 18.0 * u;
            }
            let name_w = cv.text_shrink(nx - 2.0, y + 50.0 * u, Text::new(Font::Display, 64.0 * u, FROST).tracking(-1.3 * u), &hst.name, hero_max - 140.0 * u);
            if many {
                let ax = nx + name_w + 18.0 * u;
                let aw = cv.text(ax, y + 44.0 * u, arrows, "›");
                let n = app.selected_index().unwrap_or(0) + 1;
                cv.text(ax + aw + 14.0 * u, y + 42.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT), &format!("{n} / {}", app.store.hosts.len()));
            }
            y += 72.0 * u;
            if let Some(os) = &hst.os {
                cv.text_fit(x0, y + 10.0 * u, Text::new(Font::Mono, 14.0 * u, MUTED), os, hero_max);
                y += 26.0 * u;
            }
            let cw = display_chip(app, cv, hst, x0, y, 40.0 * u, 16.0 * u);
            // Au salon (et sur KyberNow Link OS), les manettes branchées.
            if tv {
                widgets::info_chip(cv, x0 + cw + 14.0 * u, y, 40.0 * u, 16.0 * u, Icon::Gamepad, &[(&pads_label(app.pads.len()), false)]);
            }
            y += 40.0 * u + 28.0 * u;
            let mut x = x0;
            x += main_button(app, cv, f, hst, x, y, 60.0 * u) + 22.0 * u;
            x += profiles(cv, f, hst, x, y + 4.0 * u, 52.0 * u, 17.0 * u) + 14.0 * u;
            // « Réglages » se voit et se touche (écran tactile du Deck, souris) ; Y l'ouvre aussi.
            widgets::ghost(cv, f, Target::HostSettings, x, y + 4.0 * u, 52.0 * u, 17.0 * u, Some(Icon::Gear), tr("Réglages", "Settings"), MUTED);
            y += 60.0 * u + 30.0 * u;
            cv.text_fit(x0, y, Text::new(Font::Regular, 15.0 * u, FAINT), phrase(hst), hero_max);
        }
        None => welcome(cv, f, x0, y, u, hero_max),
    }

    // Les ordinateurs.
    let footer = 56.0 * u + if tv { 4.0 } else { 0.0 };
    let (cw, ch) = if tv { (320.0, 132.0) } else { (224.0, 96.0) };
    let cy = h - footer - 28.0 * u - ch;
    eyebrow(cv, x0, cy - 14.0 * u, 12.0 * u, FAINT, tr("Vos ordinateurs", "Your computers"));
    cards(app, cv, f, x0, cy, cw, ch, 14.0 * u, w - 2.0 * x0, u, true);

    // La légende des boutons.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut x = x0;
    if let Some(hst) = host {
        x += legend(cv, x, gy, g, &["A"], main_label(app, hst)) + 28.0 * u;
        x += legend(cv, x, gy, g, &["Y"], tr("Réglages", "Settings")) + 28.0 * u;
    }
    x += legend(cv, x, gy, g, &["X"], tr("Ajouter un ordinateur", "Add a computer")) + 28.0 * u;
    x += legend(cv, x, gy, g, &["⧉"], tr("Rejoindre avec un code", "Join with a code")) + 28.0 * u;
    if host.is_some() {
        legend(cv, x, gy, g, &["L1", "R1"], tr("Profil", "Profile"));
    }
    let label = if app.is_os() { tr("Réglages de l'appareil", "Device settings") } else { tr("Réglages de l'app", "App settings") };
    let lw = legend_width(cv, g, &["☰"], label);
    let mut rx = w - x0 - lw;
    if app.is_os() {
        // ⏻ Veille, à gauche de ☰ (le bouton Guide ou PS de la manette aussi).
        let sl = tr("Veille", "Standby");
        let sw = legend_width(cv, g, &["⏻"], sl);
        let sx = rx - 28.0 * u - sw;
        let sr = Rect::new(sx - 8.0, gy - g * 0.7, sw + 16.0, g * 1.4);
        let (sf, sh) = f.state(Target::Setting(crate::app::Setting::OsStandby), sr);
        if sf || sh {
            cv.rrect(sr.x, sr.y, sr.w, sr.h, 8.0, Rgba(227, 237, 240, 18));
        }
        if sf {
            widgets::ring(cv, sr, 8.0);
        }
        legend(cv, sx, gy, g, &["⏻"], sl);
        rx = w - x0 - lw;
    }
    let r = Rect::new(rx - 8.0, gy - g * 0.7, lw + 16.0, g * 1.4);
    let (focused, hover) = f.state(Target::AppSettings, r);
    if focused || hover {
        cv.rrect(r.x, r.y, r.w, r.h, 8.0, Rgba(227, 237, 240, 18));
    }
    if focused {
        widgets::ring(cv, r, 8.0);
    }
    legend(cv, rx, gy, g, &["☰"], label);
    // La mise à jour de l'app (sur l'OS, la pastille de l'en-tête le dit déjà).
    if let crate::app::UpdateState::Available { version, .. } = &app.update
        && !app.is_os()
    {
        let s = format!("{} {version} {}", tr("Version", "Version"), tr("disponible", "available"));
        cv.text(w - x0 - lw - 28.0 * u, gy + 5.0 * u, Text::new(Font::Mono, 12.0 * u, SPECTRE).align(Align::Right), &s.to_uppercase());
    }
}

/// Réseau, batterie et heure, alignés à droite en `right`.
/// Les pastilles de l'en-tête sur KyberNow Link OS (maquette) : texte en
/// Martian Mono 16 dans un contour arrondi de 40 pt, alignées à droite de `right`.
pub(super) fn pills(cv: &mut Canvas, right: f32, cy: f32, items: &[(String, Rgba)]) {
    let mono = Text::new(Font::Mono, 16.0, MUTED);
    let widths: Vec<f32> = items.iter().map(|(s, _)| cv.text_width(mono, s) + 32.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 16.0 * items.len().saturating_sub(1) as f32;
    let mut x = right - total;
    for ((s, c), w) in items.iter().zip(widths) {
        cv.rrect(x, cy - 20.0, w, 40.0, 20.0, Rgba(9, 13, 18, 191));
        cv.rrect_stroke(x, cy - 20.0, w, 40.0, 20.0, 1.0, if *c == WARN { WARN.alpha(0.4) } else { LINE_2 });
        cv.text(x + 16.0, cy + 6.0, mono.color(*c), s);
        x += w + 16.0;
    }
}

fn status_line(app: &App, cv: &mut Canvas, right: f32, y: f32, u: f32) {
    if app.is_os() && app.layout == Layout::Tv {
        let mut items: Vec<(String, Rgba)> = Vec::new();
        if let crate::app::UpdateState::Available { version, .. } = &app.update {
            items.push((format!("{version} {} · {}", tr("prête", "ready"), tr("redémarrer", "restart")), WARN));
        }
        match app.net.kind {
            NetKind::Wifi => items.push((match &app.net.band { Some(b) => format!("Wi-Fi · {b}"), None => "Wi-Fi".into() }, MUTED)),
            NetKind::Ethernet => items.push((match app.net.link_mbps { Some(m) => format!("Ethernet {}", gbit(m)), None => "Ethernet".into() }, MUTED)),
            NetKind::Unknown => {}
        }
        items.push((clock(), MUTED));
        pills(cv, right, y - 6.0 * u, &items);
        return;
    }
    let mono = Text::new(Font::Mono, 13.0 * u, MUTED);
    let mut parts: Vec<(Option<Icon>, String)> = Vec::new();
    match app.net.kind {
        NetKind::Wifi => parts.push((Some(Icon::Wifi), match &app.net.band { Some(b) => format!("Wi-Fi · {b}"), None => "Wi-Fi".into() })),
        NetKind::Ethernet => parts.push((Some(Icon::Ethernet), match app.net.link_mbps { Some(m) if m >= 1000 => format!("Ethernet {}", gbit(m)), Some(m) => format!("Ethernet {m} Mbit/s"), None => "Ethernet".into() })),
        NetKind::Unknown => {}
    }
    if let Some((pct, _)) = app.machine.battery {
        parts.push((Some(Icon::Battery), format!("{pct} %")));
    }
    parts.push((None, clock()));
    let gap = 16.0 * u;
    let isz = 16.0 * u;
    let mut total = 0.0;
    let mut widths = Vec::new();
    for (i, s) in &parts {
        let w = cv.text_width(mono, s) + if i.is_some() { isz + 6.0 * u } else { 0.0 };
        widths.push(w);
        total += w + gap;
    }
    let mut x = right - total + gap;
    for ((i, s), w) in parts.iter().zip(widths) {
        let mut tx = x;
        if let Some(icon) = i {
            cv.icon(icon, tx, y - isz + 3.0 * u, isz, 1.8, MUTED);
            tx += isz + 6.0 * u;
        }
        cv.text(tx, y, mono, s);
        x += w + gap;
    }
}

pub fn gbit(m: u32) -> String {
    if m % 1000 == 0 { format!("{} Gbit/s", m / 1000) } else { format!("{} Gbit/s", decimal(m as f64 / 1000.0, 1)) }
}
