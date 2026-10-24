//! Accueil : l'ordinateur choisi en plein cadre (son image, son état), « Se
//! connecter » et les profils, la colonne « Prêt à jouer » (téléviseur,
//! réseau, manettes, dernière session), puis les ordinateurs en rangée.

use super::canvas::{Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::{Assets, Frame};
use crate::app::{App, NetKind, Online, Rect, Target};
use crate::i18n::{decimal, tr};
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, assets: &Assets, f: &mut Frame) {
    let host = app.selected();
    cv.clear(INK);

    // Fond : l'image de l'ordinateur, assombrie (plus à gauche, sous le texte).
    match host.and_then(|h| app.images.get(&h.id)) {
        Some(img) => {
            let offline = host.and_then(|h| app.live.get(&h.id)).is_some_and(|l| l.online == Online::Offline);
            cv.image_cover(img, 0.0, 0.0, W, H, if offline { 0.35 } else { 0.9 });
        }
        None => backdrop(cv),
    }
    cv.rect(0.0, 0.0, W, H, Rgba(5, 7, 10, 158));
    cv.rect(0.0, 0.0, 1100.0, H, Rgba(5, 7, 10, 140));
    cv.rect(0.0, H - 420.0, W, 420.0, Rgba(5, 7, 10, 90));

    cv.image(&assets.logo, 96.0, 60.0, 1.0);

    // Réglages globaux, en haut à droite : atteignables à la croix, sans
    // touche particulière de la télécommande.
    let label = tr("Réglages", "Settings");
    let gw = widgets::icon_chip_width(cv, label);
    widgets::icon_chip(cv, f, Target::AppSettings, W - 96.0 - gw, 64.0, Icon::Gear, label);
    // Une mise à jour signée est publiée : rappel discret (détails dans Réglages › Ce téléviseur).
    if let crate::app::UpdateState::Available { version, .. } = &app.update {
        let s = format!("{} {version} {}", tr("Version", "Version"), tr("disponible", "available"));
        let style = Text::new(Font::Mono, 16.0, SPECTRE).align(super::canvas::Align::Right);
        cv.text(W - 96.0 - gw - 24.0, 100.0, style, &s.to_uppercase());
    }

    match host {
        Some(h) => {
            let live = app.live.get(&h.id).cloned().unwrap_or_default();
            let (status, lit) = match live.online {
                Online::Online => (tr("En ligne · réseau local", "Online · local network"), true),
                Online::Offline => (tr("Hors ligne", "Offline"), false),
                Online::Unknown => (tr("Recherche sur le réseau…", "Looking on the network…"), false),
            };
            let y0 = 262.0;
            if lit {
                cv.circle(101.0, y0 - 6.0, 13.0, HALO.alpha(0.35));
                cv.circle(101.0, y0 - 6.0, 5.0, SPECTRE);
            } else {
                cv.circle(101.0, y0 - 6.0, 5.0, FROST_3);
            }
            cv.text(122.0, y0, Text::new(Font::Mono, 18.0, FROST_2).tracking(2.0), &status.to_uppercase());
            let name_style = Text::new(Font::Display, 168.0, FROST).tracking(-5.0);
            cv.text_fit(90.0, y0 + 170.0, name_style, &h.name, 1000.0);
            let mut info = vec![];
            if let Some(os) = &h.os {
                info.push(os.clone());
            }
            if let Some(gpu) = &h.gpu {
                info.push(gpu.clone());
            }
            info.push(h.address.clone());
            let mono = Text::new(Font::Mono, 19.0, FROST_2);
            cv.text_fit(96.0, y0 + 232.0, mono, &info.join(" · "), 1000.0);
            let (w, hh, fps) = app.session_mode(h);
            // Le mode pris par l'écran virtuel à la dernière session, tant que la
            // demande n'a pas changé ; sinon celui que la session demandera.
            let ready = live.ready.filter(|(asked, _)| *asked == (w, hh, fps)).map(|(_, s)| s).unwrap_or_else(|| format!("{w} × {hh} {} {fps} Hz", tr("à", "at")));
            cv.text_fit(96.0, y0 + 264.0, mono, &format!("{}{ready}", tr("Écran virtuel prêt : ", "Virtual display ready: ")), 1000.0);

            let by = y0 + 312.0;
            let mut x = 96.0;
            if live.online == Online::Offline && h.mac.is_some() {
                x += widgets::primary(cv, f, Target::Wake, x, by, tr("Réveiller l'ordinateur", "Wake the computer")) + 28.0;
            } else {
                x += widgets::primary(cv, f, Target::Connect, x, by, tr("Se connecter", "Connect")) + 28.0;
            }
            for p in Profile::ALL {
                let label = profile_label(p);
                x += widgets::chip(cv, f, Target::Chip(p), x, by + 14.0, label, h.settings.profile == p) + 10.0;
            }
            x += 18.0;
            widgets::icon_chip(cv, f, Target::HostSettings, x, by + 14.0, Icon::Gear, tr("Réglages de l'ordinateur", "Computer settings"));
        }
        None => {
            let y0 = 300.0;
            cv.text(96.0, y0, Text::new(Font::Mono, 18.0, FROST_2).tracking(2.0), &tr("Bienvenue", "Welcome").to_uppercase());
            cv.text(90.0, y0 + 120.0, Text::new(Font::Display, 104.0, FROST).tracking(-3.0), tr("Votre ordinateur,", "Your computer,"));
            cv.text(90.0, y0 + 230.0, Text::new(Font::Display, 104.0, FROST).tracking(-3.0), tr("sur ce téléviseur.", "on this TV."));
            cv.paragraph(
                96.0,
                y0 + 300.0,
                Text::new(Font::Regular, 24.0, FROST_2),
                tr(
                    "Installez KyberNow Station sur l'ordinateur Windows, puis ajoutez-le ici : il s'affiche en dessous dès qu'il est sur le réseau.",
                    "Install KyberNow Station on the Windows computer, then add it here: it shows up below as soon as it is on the network.",
                ),
                940.0,
                36.0,
            );
        }
    }

    ready_panel(app, cv);
    cards(app, cv, f);

    // Aides.
    let y = H - 48.0;
    let mut x = 96.0;
    if host.is_some() {
        x += widgets::hint(cv, x, y, Key::Ok, tr("Se connecter", "Connect")) + 40.0;
        x += widgets::hint(cv, x, y, Key::Dots, tr("Réglages de l'ordinateur", "Computer settings")) + 40.0;
    }
    x += widgets::hint(cv, x, y, Key::Back, tr("Quitter", "Quit")) + 40.0;
    widgets::hint(cv, x, y, Key::Pointer, tr("Pointeur de la Magic Remote ou croix", "Magic Remote pointer or arrows"));
    cv.text(W - 96.0, y, Text::new(Font::Mono, 15.0, FROST_4).align(super::canvas::Align::Right), &format!("KyberNow Link {}", app.version));

}

pub fn profile_label(p: Profile) -> &'static str {
    match p {
        Profile::Jeu => tr("Jeu", "Game"),
        Profile::Bureau => tr("Bureau", "Desktop"),
        Profile::Max => "Max",
    }
}

/// Fond sans image : les écrans de l'ordinateur, en filigrane.
fn backdrop(cv: &mut Canvas) {
    cv.rect(0.0, 0.0, W, H, INK_2);
    cv.rrect(820.0, 120.0, 900.0, 560.0, 10.0, Rgba::hex(0x111723));
    cv.rrect_stroke(820.0, 120.0, 900.0, 560.0, 10.0, 1.0, LINE);
    cv.rrect(1040.0, 220.0, 760.0, 440.0, 10.0, Rgba::hex(0x141B29));
    cv.rrect_stroke(1040.0, 220.0, 760.0, 440.0, 10.0, 1.0, LINE);
    // La couture à 45°, en filigrane.
    cv.line(1180.0, 980.0, 1900.0, 260.0, 3.0, SPECTRE.alpha(0.12));
}

/// La colonne « Prêt à jouer ».
fn ready_panel(app: &App, cv: &mut Canvas) {
    let (x, y, w) = (W - 96.0 - 520.0, 150.0, 520.0);
    let pad = 34.0;
    // Hauteur selon le contenu.
    let pads = app.pads.len().max(1) as f32;
    let has_last = app.selected().and_then(|h| h.last.as_ref()).is_some();
    let h = 82.0 + 2.0 * 104.0 + 30.0 + pads * 40.0 + 22.0 + if has_last { 104.0 } else { 0.0 } + 24.0;
    cv.rrect(x, y, w, h, 22.0, Rgba(14, 19, 28, 214));
    cv.rrect_stroke(x, y, w, h, 22.0, 1.0, BORDER);
    let ix = x + pad;
    let iw = w - 2.0 * pad;
    let mut yy = y + pad + 14.0;
    widgets::label(cv, ix, yy, tr("Prêt à jouer", "Ready to play"));
    yy += 34.0;

    let small = Text::new(Font::Regular, 17.0, FROST_3);
    let big = Text::new(Font::Regular, 22.0, FROST);
    let mono = Text::new(Font::Mono, 16.0, FROST_2);

    // Téléviseur.
    cv.text(ix, yy + 16.0, small, tr("Téléviseur", "TV"));
    let (pw, ph) = app.tv.panel;
    let res = if ph >= 2160 { "4K".to_string() } else { format!("{ph}p") };
    cv.text_fit(ix, yy + 46.0, big, &format!("{} · {res} · {} Hz", app.tv.model, app.tv.refresh), iw);
    let mut caps = vec!["HEVC"];
    if app.tv.av1 {
        caps.push("AV1");
    }
    let webos = if app.tv.webos.is_empty() { "webOS".to_string() } else { format!("webOS {}", app.tv.webos) };
    let _ = pw;
    cv.text_fit(ix, yy + 74.0, mono, &format!("{webos} · {}", caps.join(", ")), iw);
    yy += 104.0;
    cv.rect(ix, yy - 8.0, iw, 1.0, LINE);

    // Réseau.
    cv.text(ix, yy + 16.0, small, tr("Réseau", "Network"));
    let (kind, detail) = match app.net.kind {
        NetKind::Ethernet => (
            "Ethernet".to_string(),
            format!("{} {} Mbit/s · {} {} Mbit/s", tr("lien", "link"), app.net.link_mbps.unwrap_or(100), tr("plafond", "cap"), app.net.cap_mbps()),
        ),
        NetKind::Wifi => ("Wi-Fi".to_string(), format!("{} {} Mbit/s", tr("plafond", "cap"), app.net.cap_mbps())),
        NetKind::Unknown => (tr("Inconnu", "Unknown").to_string(), String::new()),
    };
    cv.text_fit(ix, yy + 46.0, big, &kind, iw);
    cv.text_fit(ix, yy + 74.0, mono, &detail, iw);
    yy += 104.0;
    cv.rect(ix, yy - 8.0, iw, 1.0, LINE);

    // Manettes.
    cv.text(ix, yy + 16.0, small, tr("Manettes", "Controllers"));
    yy += 30.0;
    if app.pads.is_empty() {
        cv.text_fit(ix, yy + 26.0, Text::new(Font::Regular, 20.0, FROST_2), tr("Aucune : associez-la en Bluetooth à la télé", "None: pair it with the TV over Bluetooth"), iw);
        yy += 40.0;
    } else {
        for p in &app.pads {
            let j = format!("{}{}", tr("J", "P"), p.slot + 1);
            cv.text(ix, yy + 26.0, Text::new(Font::Mono, 20.0, SPECTRE), &j);
            cv.text_fit(ix + 50.0, yy + 26.0, big, &p.name, iw - 230.0);
            let r = if p.rumble { tr("vibrations", "rumble") } else { tr("sans vibration", "no rumble") };
            cv.text(ix + iw, yy + 26.0, mono.align(super::canvas::Align::Right), r);
            yy += 40.0;
        }
    }
    yy += 22.0;

    // Dernière session.
    if let Some(last) = app.selected().and_then(|h| h.last.clone()) {
        cv.rect(ix, yy - 8.0, iw, 1.0, LINE);
        cv.text(ix, yy + 16.0, small, tr("Dernière session", "Last session"));
        cv.text(ix, yy + 58.0, Text::new(Font::MonoMedium, 30.0, FROST), &format!("{} ms", decimal(last.median, 1)));
        let mins = last.seconds / 60;
        let dur = if mins >= 60 { format!("{} h {:02}", mins / 60, mins % 60) } else { format!("{mins} min") };
        let mut parts = vec![tr("médiane", "median").to_string()];
        if !last.mode.is_empty() {
            parts.push(last.mode.clone());
        }
        parts.push(dur);
        cv.text_fit(ix, yy + 88.0, mono, &parts.join(" · "), iw);
    }
}

/// La rangée des ordinateurs : enregistrés, trouvés sur le réseau, ajouter.
fn cards(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let y = H - 120.0 - 168.0 - 44.0;
    let mut x = 96.0;
    let selected = app.selected().map(|h| h.id.clone());
    for (i, h) in app.store.hosts.iter().enumerate().take(5) {
        let r = Rect::new(x, y, 300.0, 168.0);
        let focused = f.hit(Target::Host(i), r);
        let live = app.live.get(&h.id).map(|l| l.online).unwrap_or_default();
        let dim = if live == Online::Offline { 0.6 } else { 1.0 };
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(26, 34, 50, (255.0 * dim) as u8));
        if let Some(img) = app.images.get(&h.id) {
            cv.image_cover_rounded(img, r.x, r.y, r.w, r.h, 14.0, dim);
        }
        let border = if focused { CORE } else if selected.as_deref() == Some(&h.id) { Rgba(227, 237, 240, 128) } else { BORDER };
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, if focused { 3.0 } else { 2.0 }, border);
        let name = Text::new(Font::Semibold, 21.0, FROST.alpha(dim));
        let w = cv.text_fit(r.x, y + 168.0 + 34.0, name, &h.name, 300.0);
        if live == Online::Offline {
            cv.text(r.x + w + 12.0, y + 168.0 + 34.0, Text::new(Font::Mono, 15.0, FROST_3), tr("hors ligne", "offline"));
        }
        x += 328.0;
    }
    for (i, found) in app.new_found().iter().enumerate().take(3) {
        let r = Rect::new(x, y, 300.0, 168.0);
        let focused = f.hit(Target::Found(i), r);
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(30, 28, 56, 255));
        }
        cv.rrect_dashed(r.x, r.y, r.w, r.h, 14.0, 1.5, SPECTRE.alpha(0.6));
        cv.text(r.x + 150.0, r.y + 90.0, Text::new(Font::Mono, 15.0, SPECTRE).tracking(2.0).align(super::canvas::Align::Center), &tr("Sur le réseau", "On the network").to_uppercase());
        cv.text_fit(r.x, y + 168.0 + 34.0, Text::new(Font::Regular, 21.0, FROST), &found.name, 300.0);
        x += 328.0;
    }
    let r = Rect::new(x, y, 168.0, 168.0);
    let focused = f.hit(Target::AddCard, r);
    if focused {
        cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, CORE);
        cv.icon(&Icon::Plus, r.x + 62.0, r.y + 62.0, 44.0, 1.8, INK);
    } else {
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.0, BORDER);
        cv.icon(&Icon::Plus, r.x + 62.0, r.y + 62.0, 44.0, 1.5, FROST);
    }
    cv.text(r.x, y + 168.0 + 34.0, Text::new(Font::Regular, 21.0, FROST), tr("Ajouter", "Add"));
}
