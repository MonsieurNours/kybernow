//! Accueil (docs/link-ui.md) : l'ordinateur choisi en plein cadre (son
//! image), son état, son nom, son système, l'écran que la session créera ;
//! « Se connecter », les profils et ses réglages ; en haut, les réglages de
//! l'app et les manettes branchées ; puis les ordinateurs en rangée.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::{Assets, Frame};
use crate::app::{App, Online, Problem, Rect, Target};
use crate::i18n::{english, tr};
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

    // En haut à droite : les réglages de l'app (atteignables à la croix, sans
    // touche particulière de la télécommande), les manettes branchées, et la
    // mise à jour de l'app quand il y en a une.
    let label = tr("Réglages de l'app", "App settings");
    let gw = widgets::icon_chip_width(cv, label);
    let mut x = W - 96.0 - gw;
    widgets::icon_chip(cv, f, Target::AppSettings, x, 64.0, Icon::Gear, label);
    x -= 36.0;
    let pads = pads_label(app.pads.len());
    let style = Text::new(Font::Regular, 22.0, FROST_2);
    let pw = cv.text_width(style, &pads);
    cv.text(x - pw, 100.0, style, &pads);
    cv.icon(&Icon::Gamepad, x - pw - 42.0, 77.0, 30.0, 1.6, FROST_2);
    x -= pw + 42.0 + 36.0;
    if let crate::app::UpdateState::Available { version, .. } = &app.update {
        let s = if english() { format!("Version {version} available") } else { format!("Version {version} disponible") };
        let style = Text::new(Font::Mono, 16.0, SPECTRE).tracking(1.0).align(Align::Right);
        cv.text(x, 99.0, style, &s.to_uppercase());
    }

    let mut primary = tr("Se connecter", "Connect");
    match host {
        Some(h) => {
            let live = app.live.get(&h.id).cloned().unwrap_or_default();
            let (status, lit) = match live.online {
                Online::Online => (tr("En ligne", "Online"), true),
                Online::Offline => (tr("Hors ligne", "Offline"), false),
                Online::Unknown => (tr("Recherche…", "Searching…"), false),
            };
            // Une erreur prend une ligne de plus : le bloc remonte d'autant.
            let y0 = if live.problem.is_some() { 202.0 } else { 240.0 };
            if lit {
                cv.circle(101.0, y0 - 6.0, 13.0, HALO.alpha(0.35));
                cv.circle(101.0, y0 - 6.0, 5.0, SPECTRE);
            } else {
                cv.circle(101.0, y0 - 6.0, 5.0, FROST_3);
            }
            cv.text(122.0, y0, Text::new(Font::Mono, 18.0, FROST_2).tracking(2.0), &status.to_uppercase());
            let name_style = Text::new(Font::Display, 168.0, FROST).tracking(-5.0);
            cv.text_fit(90.0, y0 + 170.0, name_style, &h.name, 1000.0);

            // Sous le nom : l'erreur s'il y en a une, le système, l'écran.
            let mut yy = y0 + 232.0;
            let line = Text::new(Font::Regular, 25.0, FROST_2);
            if let Some(p) = live.problem {
                cv.text_fit(96.0, yy, line.color(DANGER), problem_text(p), 1100.0);
                yy += 38.0;
            }
            if let Some(os) = &h.os {
                cv.text_fit(96.0, yy, line, os, 1000.0);
                yy += 38.0;
            }
            cv.text_fit(96.0, yy, line, &app.screen_line(h), 1000.0);

            let by = yy + 44.0;
            let mut x = 96.0;
            if live.online == Online::Offline {
                primary = if app.waking() == Some(h.id.as_str()) {
                    tr("Réveil de l'ordinateur…", "Waking the computer…")
                } else if h.can_wake() {
                    tr("Réveiller l'ordinateur", "Wake the computer")
                } else {
                    tr("Réessayer", "Try again")
                };
            }
            x += widgets::primary(cv, f, Target::Connect, x, by, primary) + 28.0;
            let custom = h.settings.is_custom();
            let chips_x = x;
            for p in Profile::ALL {
                x += widgets::chip(cv, f, Target::Chip(p), x, by + 14.0, profile_label(p), !custom && h.settings.profile == p) + 10.0;
            }
            if custom {
                x += widgets::chip_static(cv, x, by + 14.0, tr("Personnalisé", "Custom")) + 10.0;
            }
            x += 18.0;
            widgets::icon_chip(cv, f, Target::HostSettings, x, by + 14.0, Icon::Gear, tr("Réglages", "Settings"));
            let phrase = if custom { tr("Vos propres réglages.", "Your own settings.") } else { profile_phrase(h.settings.profile) };
            cv.text_fit(chips_x + 4.0, by + 84.0 + 44.0, Text::new(Font::Regular, 22.0, FROST_3), phrase, W - chips_x - 200.0);
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

    cards(app, cv, f);

    // Aides.
    let y = H - 48.0;
    let mut x = 96.0;
    if let Some(h) = host {
        x += widgets::hint(cv, x, y, Key::Ok, primary) + 40.0;
        x += widgets::hint(cv, x, y, Key::Dots, &settings_for(&h.name)) + 40.0;
    }
    x += widgets::hint(cv, x, y, Key::Back, tr("Quitter", "Quit")) + 40.0;
    x += widgets::hint(cv, x, y, Key::Green, tr("Rejoindre avec un code", "Join with a code")) + 40.0;
    widgets::hint(cv, x, y, Key::Pointer, tr("Pointeur de la Magic Remote ou croix", "Magic Remote pointer or arrows"));
    cv.text(W - 96.0, y, Text::new(Font::Mono, 15.0, FROST_4).align(Align::Right), &format!("KyberNow Link {}", app.version));
}

/// « 2 manettes », « 1 manette », « Aucune manette ».
pub fn pads_label(n: usize) -> String {
    match n {
        0 => tr("Aucune manette", "No controller").into(),
        1 => tr("1 manette", "1 controller").into(),
        n => format!("{n} {}", tr("manettes", "controllers")),
    }
}

/// « Réglages de {nom} ».
pub fn settings_for(name: &str) -> String {
    if english() { format!("Settings for {name}") } else { format!("Réglages {}{name}", crate::i18n::de(name)) }
}

/// La phrase de chaque profil, sous les pastilles.
pub fn profile_phrase(p: Profile) -> &'static str {
    match p {
        Profile::Jeu => tr("La réactivité d'abord, pour jouer.", "Responsiveness first, for gaming."),
        Profile::Bureau => tr("Un texte net, pour travailler.", "Sharp text, for work."),
        Profile::Max => tr("La meilleure image que cet appareil affiche.", "The best picture this device can show."),
    }
}

/// Ce qu'Admin refuse, dit sous le nom. Sur ce téléviseur, le compte vient
/// toujours du code : on le retire, puis on ajoute l'ordinateur de nouveau.
fn problem_text(p: Problem) -> &'static str {
    match p {
        Problem::Refused => tr(
            "Accès refusé : retirez cet ordinateur dans ses réglages, puis ajoutez-le à nouveau.",
            "Access refused: remove this computer in its settings, then add it again.",
        ),
        Problem::Reinstalled => tr(
            "La Station a été réinstallée : retirez cet ordinateur dans ses réglages, puis ajoutez-le à nouveau.",
            "The Station was reinstalled: remove this computer in its settings, then add it again.",
        ),
    }
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

/// La rangée des ordinateurs : enregistrés, trouvés sur le réseau, ajouter.
fn cards(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let y = H - 120.0 - 168.0 - 44.0;
    let mut x = 96.0;
    let selected = app.selected().map(|h| h.id.clone());
    // Les ordinateurs d'abord, puis ceux du réseau, et la paire « Ajouter » (168 de large) et « Rejoindre »
    // (KyberNow Share) : quatre cartes de 300 au plus avant elles.
    let fit = ((W - 192.0 - 364.0) / 328.0).floor() as usize;
    for (i, h) in app.store.hosts.iter().enumerate().take(fit) {
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
        cv.text_fit(r.x, y + 168.0 + 34.0, name, &h.name, 300.0);
        let state = match live {
            Online::Online => tr("En ligne", "Online"),
            Online::Offline => tr("Hors ligne", "Offline"),
            Online::Unknown => "",
        };
        cv.text(r.x, y + 168.0 + 62.0, Text::new(Font::Regular, 17.0, FROST_3), state);
        x += 328.0;
    }
    for (i, found) in app.new_found().iter().enumerate().take(fit.saturating_sub(app.store.hosts.len()).min(3)) {
        let r = Rect::new(x, y, 300.0, 168.0);
        let focused = f.hit(Target::Found(i), r);
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(30, 28, 56, 255));
        }
        cv.rrect_dashed(r.x, r.y, r.w, r.h, 14.0, 1.5, SPECTRE.alpha(0.6));
        cv.text(r.x + 150.0, r.y + 90.0, Text::new(Font::Mono, 15.0, SPECTRE).tracking(2.0).align(Align::Center), &tr("Sur le réseau", "On your network").to_uppercase());
        cv.text_fit(r.x, y + 168.0 + 34.0, Text::new(Font::Regular, 21.0, FROST), &found.name, 300.0);
        x += 328.0;
    }
    action_card(cv, f, Target::AddCard, Rect::new(x, y, 168.0, 168.0), Icon::Plus, tr("Ajouter un ordinateur", "Add a computer"));
    // KyberNow Share : rejoindre le partage de quelqu'un, à côté de « Ajouter un ordinateur ».
    action_card(cv, f, Target::JoinCard, Rect::new(x + 196.0, y, 168.0, 168.0), Icon::Share, tr("Rejoindre avec un code", "Join with a code"));
}

/// Une carte carrée, une action : son icône, et son libellé dessous sur deux lignes au plus.
fn action_card(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, icon: Icon, label: &str) {
    let focused = f.hit(t, r);
    if focused {
        cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, CORE);
        cv.icon(&icon, r.x + 62.0, r.y + 62.0, 44.0, 1.8, INK);
    } else {
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.0, BORDER);
        cv.icon(&icon, r.x + 62.0, r.y + 62.0, 44.0, 1.5, FROST);
    }
    cv.paragraph(r.x, r.y + 168.0 + 34.0, Text::new(Font::Regular, 21.0, FROST), label, 184.0, 27.0);
}
