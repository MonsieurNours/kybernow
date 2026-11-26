//! Accueil et compte KyberNow (docs/link-ui.md ; maquette « Téléviseur : le compte par code ») : les trois écrans de
//! l'accueil à la télécommande, le bouton « Connecter un compte » du haut de l'accueil, la carte du compte de la
//! rangée et le bandeau après un appairage. Les règles sont dans `crate::welcome`.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key};
use super::{Assets, Frame};
use crate::accounts::{open_step, waiting_line};
use crate::app::{App, Rect, Target};
use crate::i18n::tr;
use crate::welcome::{self as wl, FOUND_MAX};

/// La largeur de la carte du compte, dans la rangée de l'accueil (168 de haut, comme les autres).
pub const CARD_W: f32 = 600.0;

/// Le violet clair des étiquettes sur le fond Spectre (#D4CEFF).
const LILAC: Rgba = Rgba::hex(0xD4CEFF);

pub fn draw(app: &App, cv: &mut Canvas, assets: &Assets, f: &mut Frame) {
    cv.clear(INK);
    // La lueur Spectre, à droite (le fond de la maquette).
    for i in 0..14 {
        let r = 1000.0 - i as f32 * 62.0;
        cv.circle(1500.0, 560.0, r, Rgba(31, 26, 76, 16));
    }
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    cv.image(&assets.logo, 96.0, 60.0, 1.0);
    let step = app.welcome.step.clamp(1, 3);

    // « Passer » en haut à droite ; « Plus tard » au dernier écran.
    let skip = if step == 3 { wl::later() } else { wl::skip() };
    let sw = cv.text_width(Text::new(Font::Semibold, 22.0, FROST), skip) + 44.0;
    widgets::secondary(cv, f, Target::WelcomeSkip, W - 96.0 - sw, 56.0, skip, 22.0);

    let label = Text::new(Font::Mono, 18.0, SPECTRE).tracking(3.0);
    cv.text(96.0, 220.0, label, &wl::step_label(step));
    match step {
        1 => first(cv, f),
        2 => second(app, cv, f),
        _ => third(app, cv, f),
    }

    // Les trois traits d'avancement, en bas au milieu.
    let (dw, gap) = (64.0, 14.0);
    let x0 = (W - (3.0 * dw + 2.0 * gap)) / 2.0;
    for i in 0..3u8 {
        let c = if i + 1 == step { SPECTRE } else if i + 1 < step { FROST_3 } else { BORDER };
        cv.rrect(x0 + i as f32 * (dw + gap), H - 62.0, dw, 6.0, 3.0, c);
    }
    let y = H - 48.0;
    let x = 96.0 + widgets::hint(cv, 96.0, y, Key::Ok, tr("Choisir", "Choose")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

/// Un grand titre en capitales (Archivo étendu), sur plusieurs lignes ; rend la hauteur.
fn title(cv: &mut Canvas, y: f32, s: &str, size: f32, max: f32) -> f32 {
    cv.paragraph(90.0, y, Text::new(Font::Title, size, FROST).tracking(-1.5), &s.to_uppercase(), max, size * 1.04)
}

/// Écran 1 : ce que fait KyberNow.
fn first(cv: &mut Canvas, f: &mut Frame) {
    let mut y = 340.0;
    y += title(cv, y, tr("Votre ordinateur, sur tous vos appareils", "Your computer, on all your devices"), 84.0, 1250.0);
    y += 30.0;
    y += cv.paragraph(96.0, y, Text::new(Font::Regular, 30.0, FROST_2), tr("Jouez, travaillez et retrouvez votre bureau depuis un autre écran, comme si vous étiez devant.", "Play, work and get back to your desktop from another screen, as if you were sitting in front of it."), 1000.0, 42.0);
    widgets::primary(cv, f, Target::WelcomeNext, 96.0, y + 30.0, tr("Continuer", "Continue"));
    // Les appareils, au trait Spectre.
    cv.icon(&Icon::Devices, 1320.0, 300.0, 420.0, 1.2, SPECTRE.alpha(0.75));
}

/// Écran 2 : les avantages d'un compte, et la carte du code à droite.
fn second(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let max = 1000.0;
    let mut y = 300.0;
    y += title(cv, y, tr("Avec un compte, tout vous suit", "With an account, everything follows you"), 60.0, 1080.0);
    y += 10.0;
    y += cv.paragraph(96.0, y, Text::new(Font::Regular, 28.0, FROST_2), tr("Un compte KyberNow, et vos ordinateurs vous attendent partout.", "A KyberNow account, and your computers wait for you everywhere."), max, 38.0);
    y += 34.0;
    let perks: [(Icon, &str, &str); 4] = [
        (Icon::Globe, tr("HORS DE CHEZ VOUS", "AWAY FROM HOME"), tr("Rejoignez votre ordinateur de n'importe où, sans rien régler sur votre box.", "Reach your computer from anywhere, with nothing to set up on your router.")),
        (Icon::Devices, tr("TOUS VOS APPAREILS", "ALL YOUR DEVICES"), tr("Vos ordinateurs apparaissent d'eux-mêmes partout où vous vous connectez.", "Your computers show up by themselves wherever you sign in.")),
        (Icon::People, tr("PARTAGER ET PRÊTER", "SHARE AND LEND"), tr("Invitez un ami à jouer, prêtez votre PC, ouvrez-le à la famille ou à votre équipe.", "Invite a friend to play, lend your PC, open it to your family or your team.")),
        (Icon::Sliders, tr("VOS RÉGLAGES", "YOUR SETTINGS"), tr("Profil, format et débit vous suivent d'un appareil à l'autre.", "Profile, format and bitrate follow you from one device to another.")),
    ];
    for (icon, head, text) in perks {
        cv.icon(&icon, 96.0, y - 4.0, 34.0, 1.6, SPECTRE);
        cv.text(150.0, y + 14.0, Text::new(Font::Mono, 16.0, LILAC).tracking(2.0), head);
        y += 24.0 + cv.paragraph(150.0, y + 48.0, Text::new(Font::Regular, 23.0, FROST_2), text, max - 54.0, 31.0) + 26.0;
    }
    widgets::primary(cv, f, Target::WelcomeNext, 96.0, y + 4.0, tr("Continuer sans compte", "Continue without an account"));
    code_card(app, cv, f, W - 96.0 - 620.0, 150.0);
}

/// La carte du code (« CONNECTER UN COMPTE ») : le QR code, le code, les étapes, « Pas encore de compte ? », l'attente ;
/// refusé, le texte et « Nouveau code ».
fn code_card(app: &App, cv: &mut Canvas, f: &mut Frame, x: f32, y: f32) {
    let (w, h) = (620.0, 790.0);
    let cx = x + w / 2.0;
    cv.rrect(x, y, w, h, 26.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 26.0, 1.0, BORDER);
    let center = |size: f32, color: Rgba| Text::new(Font::Regular, size, color).align(Align::Center);
    cv.text(cx, y + 62.0, Text::new(Font::Mono, 16.0, FROST_2).tracking(2.5).align(Align::Center), tr("CONNECTER UN COMPTE", "CONNECT AN ACCOUNT"));
    let fl = &app.accounts.flow;
    if fl.denied || fl.message.is_some() {
        let m = if fl.denied { tr("Connexion refusée sur le téléphone.", "Sign-in declined on the phone.").to_string() } else { fl.message.clone().unwrap_or_default() };
        let used = cv.paragraph(cx, y + 220.0, center(26.0, WARN), &m, w - 80.0, 36.0);
        let label = tr("Nouveau code", "New code");
        let bw = cv.text_width(Text::new(Font::Semibold, 24.0, FROST), label) + 48.0;
        widgets::secondary(cv, f, Target::AccountNewCode, cx - bw / 2.0, y + 260.0 + used, label, 24.0);
        return;
    }
    let side = 300.0;
    match &fl.code {
        Some(c) => {
            if widgets::qr(cv, &c.url_complete, cx - side / 2.0, y + 96.0, side).is_none() {
                cv.rrect(cx - side / 2.0, y + 96.0, side, side, 12.0, FROST_4);
            }
            cv.text(cx, y + 486.0, Text::new(Font::MonoMedium, 64.0, FROST).tracking(4.0).align(Align::Center), &c.code);
        }
        None => {
            cv.rrect_dashed(cx - side / 2.0, y + 96.0, side, side, 12.0, 1.5, BORDER);
            cv.text(cx, y + 486.0, Text::new(Font::MonoMedium, 64.0, FROST_4).tracking(4.0).align(Align::Center), "····-····");
        }
    }
    let url_label = fl.code.as_ref().map(|c| c.url_label.clone()).unwrap_or_else(|| format!("{}/tv", crate::model::relay_label(&fl.relay)));
    let steps = [tr("Scannez le QR code avec votre téléphone,", "Scan the QR code with your phone,").to_string(), open_step(&url_label), tr("puis touchez « Connecter ».", "then tap “Connect”.").to_string()];
    let mut ly = y + 548.0;
    for s in &steps {
        cv.text_fit(cx, ly, center(21.0, FROST_2), s, w - 40.0);
        ly += 32.0;
    }
    ly += 12.0;
    cv.text_fit(cx, ly, Text::new(Font::Semibold, 21.0, FROST).align(Align::Center), wl::no_account_yet(), w - 40.0);
    ly += 48.0;
    if let Some(c) = &fl.code {
        cv.paragraph(cx, ly, center(19.0, FROST_3), &waiting_line(c.minutes_left(crate::model::now_secs()).max(1)), w - 60.0, 26.0);
    }
}

/// Écran 3 : les ordinateurs à portée, « Un code », « Pas encore installé ».
fn third(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let mut y = 340.0;
    y += title(cv, y, tr("Ajoutez votre ordinateur", "Add your computer"), 84.0, 1740.0);
    y += 24.0;
    cv.paragraph(
        96.0,
        y,
        Text::new(Font::Regular, 28.0, FROST_2),
        tr(
            "Installez KyberNow Station sur l'ordinateur à rejoindre : sur le même réseau, il apparaît ici tout seul. Un ami vous a donné un code ? Entrez-le.",
            "Install KyberNow Station on the computer you want to reach: on the same network, it shows up here by itself. A friend gave you a code? Enter it.",
        ),
        1300.0,
        38.0,
    );
    let ry = 640.0;
    widgets::label(cv, 96.0, ry - 26.0, tr("Ordinateurs à portée", "Computers in reach"));
    let ch = 260.0;
    let mut x = 96.0;
    for (i, found) in app.new_found().iter().enumerate().take(FOUND_MAX) {
        let r = Rect::new(x, ry, 300.0, ch);
        let focused = f.hit(Target::Found(i), r);
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(30, 28, 56, 255));
        }
        cv.rrect_dashed(r.x, r.y, r.w, r.h, 14.0, 1.5, SPECTRE.alpha(0.6));
        cv.text(r.x + 150.0, r.y + 64.0, Text::new(Font::Mono, 15.0, SPECTRE).tracking(2.0).align(Align::Center), &tr("Sur le réseau", "On your network").to_uppercase());
        cv.text_fit(r.x + 150.0, r.y + 128.0, Text::new(Font::Semibold, 28.0, FROST).align(Align::Center), &found.name, 260.0);
        let add = tr("Ajouter", "Add");
        let style = Text::new(Font::Semibold, 22.0, if focused { INK } else { FROST });
        let bw = cv.text_width(style, add) + 44.0;
        let (bx, by) = (r.x + 150.0 - bw / 2.0, r.y + 172.0);
        cv.rrect(bx, by, bw, 52.0, 12.0, if focused { CORE } else { FLAT });
        cv.text(bx + 22.0, by + 34.0, style.color(if focused { INK } else { CORE }), add);
        x += 328.0;
    }
    // « UN CODE » : rejoindre l'ordinateur d'un ami.
    let r = Rect::new(x, ry, 460.0, ch);
    cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(227, 237, 240, 12));
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.0, BORDER);
    cv.text(r.x + 32.0, r.y + 52.0, Text::new(Font::Mono, 15.0, FROST_2).tracking(2.0), tr("UN CODE", "A CODE"));
    cv.paragraph(r.x + 32.0, r.y + 96.0, Text::new(Font::Regular, 22.0, FROST_2), tr("Un ami vous ouvre son ordinateur : entrez le code qu'il vous a donné.", "A friend opens their computer to you: enter the code they gave you."), r.w - 64.0, 30.0);
    widgets::secondary(cv, f, Target::JoinCard, r.x + 32.0, r.y + ch - 32.0 - 52.8, tr("Rejoindre avec un code", "Join with a code"), 22.0);
    x += 460.0 + 28.0;
    // « PAS ENCORE INSTALLÉ » : la page de téléchargement, son adresse et son QR code (pas de navigateur ici).
    let r = Rect::new(x, ry, 560.0, ch);
    cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(227, 237, 240, 12));
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.0, BORDER);
    cv.text(r.x + 32.0, r.y + 52.0, Text::new(Font::Mono, 15.0, FROST_2).tracking(2.0), tr("PAS ENCORE INSTALLÉ", "NOT INSTALLED YET"));
    let side = 196.0;
    cv.paragraph(r.x + 32.0, r.y + 96.0, Text::new(Font::Regular, 22.0, FROST_2), tr("Téléchargez KyberNow Station sur l'ordinateur à rejoindre.", "Download KyberNow Station on the computer you want to reach."), r.w - 64.0 - side - 24.0, 30.0);
    cv.text(r.x + 32.0, r.y + ch - 40.0, Text::new(Font::Mono, 20.0, SPECTRE), wl::download_label());
    widgets::qr(cv, wl::STATION_DOWNLOAD_URL, r.x + r.w - 32.0 - side, r.y + (ch - side) / 2.0, side);
}

/// Un petit bouton (44 de haut) : plein (Spectre) ou en contour, avec une icône ; rend sa largeur.
#[allow(clippy::too_many_arguments)]
fn small(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, label: &str, filled: bool, icon: Option<Icon>) -> f32 {
    let style = Text::new(Font::Semibold, 20.0, FROST);
    let iw = if icon.is_some() { 28.0 } else { 0.0 };
    let w = cv.text_width(style, label) + 40.0 + iw;
    let h = 46.0;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    let fg = if focused { INK } else if filled { CORE } else { LILAC };
    if focused {
        cv.neon(x, y, w, h, 10.0, 0.9);
        cv.rrect(x, y, w, h, 10.0, CORE);
    } else if filled {
        cv.rrect(x, y, w, h, 10.0, FLAT);
    } else {
        cv.rrect(x, y, w, h, 10.0, Rgba(36, 30, 74, 255));
        cv.rrect_stroke(x, y, w, h, 10.0, 1.0, Rgba(74, 67, 128, 255));
    }
    if let Some(i) = icon {
        cv.icon(&i, x + 16.0, y + 12.0, 22.0, 2.0, fg);
    }
    cv.text(x + 20.0 + iw, y + 30.0, style.color(fg), label);
    w
}

/// « Connecter un compte » en haut de l'accueil, son bord droit en `right` : à l'aplat Spectre, l'icône d'une
/// personne ; rend sa largeur.
pub fn account_button(cv: &mut Canvas, f: &mut Frame, right: f32, y: f32) -> f32 {
    let s = wl::connect_account();
    let style = Text::new(Font::Semibold, 22.0, CORE);
    let w = cv.text_width(style, s) + 26.0 + 30.0 + 12.0 + 28.0;
    let (x, h) = (right - w, 56.0);
    let focused = f.hit(Target::AccountSignIn, Rect::new(x, y, w, h));
    let fg = if focused { INK } else { CORE };
    if focused {
        cv.neon(x, y, w, h, 28.0, 0.8);
    }
    cv.rrect(x, y, w, h, 28.0, if focused { CORE } else { FLAT });
    cv.icon(&Icon::Person, x + 26.0, y + 13.0, 30.0, 1.8, fg);
    cv.text(x + 26.0 + 30.0 + 12.0, y + 36.0, style.color(fg), s);
    w
}

/// La carte du compte, dans la rangée (`CARD_W` × 168), avec la lueur de « Sur le réseau ».
pub fn account_card(app: &App, cv: &mut Canvas, f: &mut Frame, x: f32, y: f32) {
    let (w, h) = (CARD_W, 168.0);
    if x > W || x + w < 0.0 {
        // Hors de l'écran : choisissable quand même (le focus ramène la rangée).
        f.hit(Target::WelcomeCardConnect, Rect::new(x + 28.0, y + h - 62.0, 200.0, 46.0));
        f.hit(Target::WelcomeCardClose, Rect::new(x + w - 220.0, y + h - 62.0, 200.0, 46.0));
        return;
    }
    let lit = matches!(f.focus, Target::WelcomeCardConnect | Target::WelcomeCardClose);
    cv.neon(x, y, w, h, 14.0, if lit { 0.9 } else { 0.4 });
    cv.rrect(x, y, w, h, 14.0, Rgba(27, 22, 64, 255));
    cv.rrect_stroke(x, y, w, h, 14.0, 1.5, FLAT);
    cv.icon(&Icon::Globe, x + 26.0, y + 16.0, 22.0, 2.0, LILAC);
    cv.text(x + 58.0, y + 34.0, Text::new(Font::Mono, 14.0, LILAC).tracking(2.0), tr("COMPTE KYBERNOW", "KYBERNOW ACCOUNT"));
    cv.text_fit(x + 26.0, y + 66.0, Text::new(Font::Semibold, 24.0, FROST), tr("Retrouvez vos ordinateurs partout", "Find your computers everywhere"), w - 52.0);
    cv.text_fit(x + 26.0, y + 92.0, Text::new(Font::Regular, 19.0, Rgba::hex(0xC9C5E2)), tr("Sur tous vos appareils, même hors de chez vous.", "On all your devices, even away from home."), w - 52.0);
    let by = y + h - 60.0;
    small(cv, f, Target::WelcomeCardConnect, x + 26.0, by, wl::connect_account(), true, None);
    // La croix : « Fermer », puis, revenue, « Ne plus afficher ».
    let label = wl::card_close_label(app.store.welcome.card_close_is_final());
    let cw = cv.text_width(Text::new(Font::Semibold, 20.0, FROST), label) + 68.0;
    small(cv, f, Target::WelcomeCardClose, x + w - 26.0 - cw, by, label, false, Some(Icon::Close));
}

/// Le bandeau après un appairage, sans compte : à la place de celui de la mise à jour de la Station, même hauteur
/// (`super::offer::BANNER_H`), en `y` (haut du bandeau).
pub fn paired_banner(cv: &mut Canvas, f: &mut Frame, y: f32) {
    let (x, w, h) = (96.0, W - 192.0, super::offer::BANNER_H);
    cv.rrect(x, y, w, h, 18.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, w, h, 18.0, 1.5, BORDER);
    cv.rrect(x + 1.0, y + 18.0, 4.0, h - 36.0, 2.0, SPECTRE);
    let cy = y + h / 2.0;
    cv.circle(x + 52.0, cy, 26.0, SPECTRE.alpha(0.16));
    cv.icon(&Icon::Person, x + 52.0 - 14.0, cy - 14.0, 28.0, 1.8, SPECTRE);
    // Les boutons, à droite : « Connecter un compte » (le principal), « Plus tard ».
    let size = 22.0;
    let bh = size * 2.4;
    let by = cy - bh / 2.0;
    let later = wl::later();
    let lw = cv.text_width(Text::new(Font::Semibold, size, FROST), later) + size * 2.0;
    let mut bx = x + w - 24.0 - lw;
    widgets::secondary(cv, f, Target::WelcomePairedLater, bx, by, later, size);
    let main = wl::connect_account();
    let style = Text::new(Font::Semibold, size, CORE);
    let mw = cv.text_width(style, main) + size * 2.0;
    bx -= 14.0 + mw;
    let focused = f.hit(Target::WelcomePairedConnect, Rect::new(bx, by, mw, bh));
    if focused {
        cv.neon(bx, by, mw, bh, 12.0, 0.9);
    }
    cv.rrect(bx, by, mw, bh, 12.0, if focused { CORE } else { FLAT });
    cv.text(bx + size, by + bh / 2.0 + size * 0.36, style.color(if focused { INK } else { CORE }), main);
    // Le texte, sur une ligne ou deux.
    let tx = x + 100.0;
    let max = bx - 24.0 - tx;
    let style = Text::new(Font::Regular, 22.0, FROST);
    let text = wl::paired_text();
    if cv.text_width(style, text) <= max {
        cv.text(tx, cy + 8.0, style, text);
    } else {
        cv.paragraph(tx, cy - 7.0, style, text, max, 30.0);
    }
}
