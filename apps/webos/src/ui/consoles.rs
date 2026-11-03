//! Consoles PlayStation (docs/link-ui.md, « Consoles PlayStation ») : la console choisie sur l'accueil et ses cartes
//! dans la rangée, l'écran « PlayStation » de l'ajout (comptes, consoles sur ce réseau, recevoir d'un autre appareil,
//! saisie à la main, code de la console, envoyer à un appareil), les réglages d'une console, et dans la partie le
//! clavier de la console et les mesures. La connexion et le menu de la partie sont ceux d'un ordinateur
//! (`session.rs`).

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::pair::key;
use super::theme::*;
use super::widgets::{self, Key, Value};
use super::Frame;
use crate::app::{App, Rect, Target};
use crate::i18n::{english, tr};
use crate::model::PsConsole;
use crate::ps::{consoles_label, ManualField, PsSection, PsSetting, PsState, PsStep, PsTarget, PS_SECTIONS};

fn t(p: PsTarget) -> Target {
    Target::Ps(p)
}

// MARK: - Accueil

/// La console choisie : son état, son nom, son modèle, le jeu en cours ; le bouton principal, la puce du compte, la
/// puce HDR, la puce « Auto » ou « Personnalisé » et l'engrenage. Rend ce que fait OK, pour l'aide du bas.
pub fn hero(app: &App, cv: &mut Canvas, f: &mut Frame) -> String {
    let Some(c) = app.selected_console() else { return String::new() };
    let state = app.ps_state(&c.host_id);
    let paired = !c.pairings.is_empty() && c.play_account(&app.store.playstation.accounts).is_some();
    let y0 = 240.0;
    if state.lit() {
        cv.circle(101.0, y0 - 6.0, 13.0, HALO.alpha(0.35));
        cv.circle(101.0, y0 - 6.0, 5.0, SPECTRE);
    } else {
        cv.circle(101.0, y0 - 6.0, 5.0, FROST_3);
    }
    let status = if paired { state.label() } else { tr("Sur le réseau", "On your network") };
    cv.text(122.0, y0, Text::new(Font::Mono, 18.0, FROST_2).tracking(2.0), &status.to_uppercase());
    let name = if c.name.is_empty() { c.model.as_str() } else { c.name.as_str() };
    cv.text_fit(90.0, y0 + 170.0, Text::new(Font::Display, 168.0, FROST).tracking(-5.0), name, 1700.0);

    let line = Text::new(Font::Regular, 25.0, FROST_2);
    let mut yy = y0 + 232.0;
    cv.text_fit(96.0, yy, line, &c.model, 1000.0);
    yy += 38.0;
    if let Some(game) = app.ps.live.get(&c.host_id).and_then(|l| l.running_app.as_ref()).filter(|g| !g.is_empty()) {
        cv.text_fit(96.0, yy, line, game, 1000.0);
        yy += 38.0;
    }

    let by = yy + 44.0;
    let waking = app.ps.waking.as_deref() == Some(c.host_id.as_str());
    let label = if !paired {
        tr("Associer", "Pair")
    } else if waking {
        tr("Réveil de la console…", "Waking the console…")
    } else {
        match state {
            PsState::On | PsState::Away => tr("Jouer", "Play"),
            PsState::Standby => tr("Réveiller et jouer", "Wake and play"),
            PsState::Offline | PsState::Unknown => tr("Réessayer", "Try again"),
        }
    };
    let mut x = 96.0 + widgets::primary(cv, f, t(PsTarget::Primary), 96.0, by, label) + 28.0;
    let chips_x = x;
    // Plusieurs comptes associés : la puce « {pseudo} » ouvre le choix du compte.
    let accounts = c.paired_accounts(&app.store.playstation.accounts);
    if accounts.len() > 1
        && let Some(a) = c.play_account(&app.store.playstation.accounts)
    {
        x += widgets::chip(cv, f, t(PsTarget::AccountChip), x, by + 14.0, &a.label(), app.ps.account_menu) + 10.0;
    }
    let hdr_ok = app.ps_hdr_available(c);
    if c.ps5() && app.tv.hdr {
        x += widgets::chip_state(cv, f, t(PsTarget::HdrChip), x, by + 14.0, "HDR", c.hdr && hdr_ok, hdr_ok) + 10.0;
    }
    if paired {
        let custom = c.settings.is_custom();
        x += widgets::chip(cv, f, t(PsTarget::CustomChip), x, by + 14.0, if custom { tr("Personnalisé", "Custom") } else { "Auto" }, true) + 28.0;
        widgets::icon_chip(cv, f, t(PsTarget::Gear), x, by + 14.0, Icon::Gear, tr("Réglages", "Settings"));
    }
    // Le choix du compte, sous les puces.
    if app.ps.account_menu && accounts.len() > 1 {
        let mut ax = chips_x;
        let current = c.play_account(&app.store.playstation.accounts).map(|a| a.id.clone());
        for (i, a) in accounts.iter().enumerate() {
            ax += widgets::chip(cv, f, t(PsTarget::AccountPick(i)), ax, by + 96.0, &a.label(), current.as_deref() == Some(a.id.as_str())) + 10.0;
        }
    } else {
        let phrase = match f.focus {
            Target::Ps(PsTarget::HdrChip) if !hdr_ok && c.settings.codec == "h264" => tr("Le H.264 n'a pas de HDR : réglez le format sur Auto ou H.265.", "H.264 has no HDR: set the format to Auto or H.265."),
            Target::Ps(PsTarget::HdrChip) if !hdr_ok => crate::app::HdrReason::Decoder.text(),
            Target::Ps(PsTarget::HdrChip) => crate::app::hdr_help(),
            Target::Ps(PsTarget::AccountChip) => tr("Le profil avec lequel la console ouvre la partie.", "The profile the console opens the session with."),
            _ if !paired => tr("Le code s'affiche sur la console : Réglages › Système › Lecture à distance › Associer un appareil.", "The code shows on the console: Settings › System › Remote Play › Link Device."),
            _ if c.settings.is_custom() => tr("Vos propres réglages.", "Your own settings."),
            _ => tr("Le mieux que la console et ce téléviseur permettent.", "The best the console and this TV allow."),
        };
        cv.text_fit(chips_x + 4.0, by + 128.0, Text::new(Font::Regular, 22.0, FROST_3), phrase, W - chips_x - 200.0);
    }
    label.to_string()
}

/// Les cartes des consoles associées dans la rangée (le nom et l'état). `x` avance d'une carte à chaque fois.
pub fn cards(app: &App, cv: &mut Canvas, f: &mut Frame, x: &mut f32, y: f32, pitch: f32) {
    let selected = app.store.selected.clone();
    for (i, c) in app.store.playstation.consoles.iter().enumerate().filter(|(_, c)| !c.pairings.is_empty()) {
        let r = Rect::new(*x, y, 300.0, 168.0);
        let focused = f.hit(t(PsTarget::Card(i)), r);
        if r.x + r.w > 0.0 && r.x < W {
            let state = app.ps_state(&c.host_id);
            let dim = if state == PsState::Offline { 0.6 } else { 1.0 };
            if focused {
                cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
            }
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(26, 34, 50, (255.0 * dim) as u8));
            model_mark(cv, r, &c.model, dim);
            let border = if focused { CORE } else if selected.as_deref() == Some(c.key().as_str()) { Rgba(227, 237, 240, 128) } else { BORDER };
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, if focused { 3.0 } else { 2.0 }, border);
            cv.text_fit(r.x, y + 168.0 + 34.0, Text::new(Font::Semibold, 21.0, FROST.alpha(dim)), &c.name, 300.0);
            let s = if state == PsState::Unknown { "" } else { state.label() };
            cv.text(r.x, y + 168.0 + 62.0, Text::new(Font::Regular, 17.0, FROST_3), s);
        }
        *x += pitch;
    }
}

/// Les consoles trouvées sur le réseau et pas encore associées : « Sur le réseau ».
pub fn found_cards(app: &App, cv: &mut Canvas, f: &mut Frame, x: &mut f32, y: f32, pitch: f32, max: usize) {
    for (i, found) in app.ps_new_found().iter().enumerate().take(max) {
        let r = Rect::new(*x, y, 300.0, 168.0);
        let focused = f.hit(t(PsTarget::FoundCard(i)), r);
        if r.x + r.w > 0.0 && r.x < W {
            if focused {
                cv.neon(r.x, r.y, r.w, r.h, 14.0, 1.0);
                cv.rrect(r.x, r.y, r.w, r.h, 14.0, Rgba(30, 28, 56, 255));
            }
            cv.rrect_dashed(r.x, r.y, r.w, r.h, 14.0, 1.5, SPECTRE.alpha(0.6));
            cv.text(r.x + 150.0, r.y + 90.0, Text::new(Font::Mono, 15.0, SPECTRE).tracking(2.0).align(Align::Center), &tr("Sur le réseau", "On your network").to_uppercase());
            cv.text_fit(r.x, y + 168.0 + 34.0, Text::new(Font::Regular, 21.0, FROST), &format!("{} · {}", found.name, found.model), 300.0);
        }
        *x += pitch;
    }
}

/// Le modèle en toutes lettres au centre de la carte (aucun logo de Sony).
fn model_mark(cv: &mut Canvas, r: Rect, model: &str, dim: f32) {
    cv.text(r.x + r.w / 2.0, r.y + r.h / 2.0 + 22.0, Text::new(Font::Display, 64.0, FROST_3.alpha(dim)).tracking(-1.0).align(Align::Center), model);
}

/// Le nombre de cartes de consoles dans la rangée (associées, puis trouvées).
pub fn card_count(app: &App, found_max: usize) -> (usize, usize) {
    let paired = app.store.playstation.consoles.iter().filter(|c| !c.pairings.is_empty()).count();
    (paired, app.ps_new_found().len().min(found_max))
}

/// La place d'une carte de console dans la rangée (parmi les consoles associées).
pub fn card_slot(app: &App, i: usize) -> usize {
    app.store.playstation.consoles.iter().take(i).filter(|c| !c.pairings.is_empty()).count()
}

// MARK: - Écran « PlayStation » de l'ajout

pub fn add_screen(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    widgets::label(cv, 96.0, 104.0, tr("Ajouter un ordinateur ou une console", "Add a computer or a console"));
    match app.ps.add.step {
        PsStep::Main => main(app, cv, f),
        PsStep::Receive => receive(app, cv, f),
        PsStep::Manual => manual(app, cv, f),
        PsStep::Choose => choose(app, cv, f),
        PsStep::Code => code(app, cv, f),
        PsStep::Send => send(app, cv, f),
    }
    if app.ps.dialog.is_some() {
        f.hits.clear();
        dialog(app, cv, f);
    }
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Choisir", "Select")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

fn title(cv: &mut Canvas, s: &str) {
    cv.text_fit(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), s, W - 180.0);
}

/// L'avis ou l'erreur de l'étape.
fn message(app: &App, cv: &mut Canvas, x: f32, y: f32, w: f32) -> f32 {
    let Some((m, error)) = &app.ps.add.message else { return 0.0 };
    let (bg, line, color) = if *error { (DANGER_BG, DANGER_LINE, DANGER) } else { (SPECTRE.alpha(0.08), SPECTRE.alpha(0.3), FROST) };
    cv.rrect(x, y, w, 64.0, 14.0, bg);
    cv.rrect_stroke(x, y, w, 64.0, 14.0, 1.0, line);
    cv.icon(&Icon::Info, x + 20.0, y + 18.0, 28.0, 1.6, if *error { DANGER } else { SPECTRE });
    cv.text_fit(x + 64.0, y + 41.0, Text::new(Font::Regular, 21.0, color), m, w - 84.0);
    84.0
}

/// Une ligne de liste (compte, console, appareil) : titre, sous-titre, et à droite un texte.
fn list_row(cv: &mut Canvas, focused: bool, r: Rect, title: &str, sub: &str, right: &str) {
    if focused {
        cv.neon(r.x, r.y, r.w, r.h, 18.0, 0.9);
        cv.rrect(r.x, r.y, r.w, r.h, 18.0, Rgba(30, 28, 56, 255));
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 2.0, SPECTRE);
    } else {
        cv.rrect(r.x, r.y, r.w, r.h, 18.0, PANEL);
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, BORDER);
    }
    let rw = if right.is_empty() { 0.0 } else { cv.text(r.x + r.w - 26.0, r.y + r.h / 2.0 + 7.0, Text::new(Font::Mono, 16.0, if focused { SPECTRE } else { FROST_3 }).align(Align::Right), right) + 30.0 };
    if sub.is_empty() {
        cv.text_fit(r.x + 28.0, r.y + r.h / 2.0 + 9.0, Text::new(Font::Semibold, 26.0, FROST), title, r.w - 56.0 - rw);
    } else {
        cv.text_fit(r.x + 28.0, r.y + 42.0, Text::new(Font::Semibold, 26.0, FROST), title, r.w - 56.0 - rw);
        cv.text_fit(r.x + 28.0, r.y + 74.0, Text::new(Font::Regular, 19.0, FROST_2), sub, r.w - 56.0 - rw);
    }
}

/// Comptes PlayStation Network à gauche (avec « Renommer », « Retirer », « Ajouter un compte », « Recevoir d'un
/// autre appareil » et « Envoyer à un appareil »), consoles sur ce réseau à droite.
fn main(app: &App, cv: &mut Canvas, f: &mut Frame) {
    title(cv, "PlayStation");
    let ps = &app.store.playstation;
    let (lx, lw) = (96.0, 820.0);
    widgets::label(cv, lx, 290.0, tr("Comptes PlayStation Network", "PlayStation Network accounts"));
    let mut y = 312.0;
    if ps.accounts.is_empty() {
        cv.rrect_dashed(lx, y, lw, 96.0, 18.0, 1.5, BORDER);
        cv.text_fit(lx + 28.0, y + 56.0, Text::new(Font::Regular, 21.0, FROST_2), tr("Aucun compte : ajoutez-en un pour associer une console.", "No account: add one to pair a console."), lw - 56.0);
        y += 112.0;
    }
    for (i, a) in ps.accounts.iter().enumerate().take(4) {
        let r = Rect::new(lx, y, lw - 360.0, 96.0);
        // Le compte lui-même ne se choisit pas : ses deux boutons, à droite.
        cv.rrect(r.x, r.y, r.w, r.h, 18.0, PANEL);
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, BORDER);
        cv.text_fit(r.x + 28.0, r.y + 42.0, Text::new(Font::Semibold, 26.0, FROST), &a.label(), r.w - 56.0);
        cv.text(r.x + 28.0, r.y + 74.0, Text::new(Font::Regular, 19.0, FROST_2), &consoles_label(ps.consoles_of(&a.id)));
        let bx = r.x + r.w + 16.0;
        let bw = widgets::secondary(cv, f, t(PsTarget::Rename(i)), bx, y + 18.0, tr("Renommer", "Rename"), 21.0);
        widgets::secondary(cv, f, t(PsTarget::RemoveAccount(i)), bx + bw + 12.0, y + 18.0, tr("Retirer", "Remove"), 21.0);
        y += 108.0;
    }
    let bw = widgets::secondary(cv, f, t(PsTarget::AddAccount), lx, y + 8.0, tr("Ajouter un compte", "Add an account"), 22.0);
    widgets::secondary(cv, f, t(PsTarget::Receive), lx + bw + 16.0, y + 8.0, tr("Recevoir d'un autre appareil", "Receive from another device"), 22.0);
    y += 96.0;
    // Envoyer à un appareil : seulement avec des comptes.
    if !ps.accounts.is_empty() {
        widgets::label(cv, lx, y + 20.0, tr("Envoyer à un appareil", "Send to a device"));
        y += 40.0;
        if app.ps.add.waiting.is_empty() {
            cv.paragraph(
                lx,
                y + 26.0,
                Text::new(Font::Regular, 20.0, FROST_3),
                tr(
                    "Aucun appareil en attente. Sur la télé ou l'autre appareil : Ajouter une console › Recevoir d'un autre appareil.",
                    "No device waiting. On the TV or the other device: Add a console › Receive from another device.",
                ),
                lw,
                28.0,
            );
        }
        for (i, w) in app.ps.add.waiting.iter().enumerate().take(2) {
            let r = Rect::new(lx, y, lw, 76.0);
            let focused = f.hit(t(PsTarget::SendTo(i)), r);
            list_row(cv, focused, r, &w.line(), "", tr("envoyer", "send"));
            y += 88.0;
        }
    }
    message(app, cv, lx, H - 190.0, lw);

    // Les consoles du réseau.
    let rx = 1010.0;
    let rw = W - 96.0 - rx;
    widgets::label(cv, rx, 290.0, tr("Consoles sur ce réseau", "Consoles on this network"));
    let mut yy = 312.0;
    if app.ps.found.is_empty() {
        cv.rrect_dashed(rx, yy, rw, 130.0, 18.0, 1.5, BORDER);
        cv.paragraph(
            rx + 28.0,
            yy + 48.0,
            Text::new(Font::Regular, 21.0, FROST_2),
            tr(
                "Aucune console trouvée. Allumez-la ou mettez-la en veille, ou saisissez son adresse dans « Par adresse ».",
                "No console found. Turn it on or put it in rest mode, or enter its address in “By address”.",
            ),
            rw - 56.0,
            30.0,
        );
    }
    for (i, fd) in app.ps.found.iter().enumerate().take(4) {
        let console = ps.console(&fd.host_id);
        let paired: Vec<String> = console.map(|c| c.paired_accounts(&ps.accounts).iter().map(|a| a.label()).collect()).unwrap_or_default();
        let r = Rect::new(rx, yy, rw, 104.0);
        let sub = format!("{} · {}", fd.model, fd.state.label());
        if paired.is_empty() {
            let focused = f.hit(t(PsTarget::Pair(i)), r);
            list_row(cv, focused, r, &fd.name, &sub, tr("Associer", "Pair"));
            yy += 120.0;
        } else {
            // Associée : la ligne le dit ; « Associer avec un autre compte » s'il en reste un.
            cv.rrect(r.x, r.y, r.w, r.h, 18.0, PANEL);
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 1.0, BORDER);
            cv.text_fit(r.x + 28.0, r.y + 42.0, Text::new(Font::Semibold, 26.0, FROST), &fd.name, rw - 56.0);
            let who = if english() { format!("{sub} · Paired: {}", paired.join(", ")) } else { format!("{sub} · Associée : {}", paired.join(", ")) };
            cv.text_fit(r.x + 28.0, r.y + 74.0, Text::new(Font::Regular, 19.0, FROST_2), &who, rw - 56.0);
            yy += 116.0;
            if !app.ps_candidates(&fd.host_id).is_empty() {
                widgets::secondary(cv, f, t(PsTarget::PairOther(i)), rx, yy, tr("Associer avec un autre compte", "Pair with another account"), 20.0);
                yy += 76.0;
            }
            yy += 8.0;
        }
    }
}

/// « Recevoir d'un autre appareil » : le code en grand, ce qu'on fait sur l'autre appareil ; « Saisir l'identifiant à
/// la main » ; après 5 min : « Code expiré. » et « Nouveau code ».
fn receive(app: &App, cv: &mut Canvas, f: &mut Frame) {
    title(cv, tr("Recevoir d'un autre appareil", "Receive from another device"));
    let (x, y, w, h) = (96.0, 260.0, 860.0, 300.0);
    cv.neon(x, y, w, h, 26.0, if app.ps.add.receive_expired { 0.2 } else { 0.55 });
    cv.rrect(x, y, w, h, 26.0, PANEL_SOLID);
    widgets::label(cv, x + 44.0, y + 64.0, tr("Code de ce téléviseur", "This TV's code"));
    let shown = match (&app.ps.add.receive_code, app.ps.add.receive_expired) {
        (_, true) => tr("Code expiré.", "Code expired.").to_string(),
        (Some(c), false) if c.len() == 6 => format!("{} {}", &c[..3], &c[3..]),
        (Some(c), false) => c.clone(),
        (None, false) => "··· ···".into(),
    };
    let size = if app.ps.add.receive_expired { 64.0 } else { 120.0 };
    cv.text(x + w / 2.0, y + 200.0, Text::new(Font::MonoMedium, size, if app.ps.add.receive_expired { FROST_3 } else { FROST }).tracking(10.0).align(Align::Center), &shown);
    let used = cv.paragraph(
        96.0,
        y + h + 64.0,
        Text::new(Font::Regular, 24.0, FROST_2),
        tr(
            "Sur votre Mac, votre PC ou votre téléphone : Ajouter un ordinateur ou une console › PlayStation › Envoyer à un appareil, puis saisissez ce code.",
            "On your Mac, PC or phone: Add a computer or a console › PlayStation › Send to a device, then enter this code.",
        ),
        860.0,
        36.0,
    );
    message(app, cv, 96.0, y + h + 90.0 + used, 860.0);

    // À droite : la saisie à la main (une télé n'a pas de navigateur pour la connexion Sony), un nouveau code.
    let rx = 1060.0;
    let rw = W - 96.0 - rx;
    if let Some(p) = &app.ps.add.pairing {
        widgets::label(cv, rx, 300.0, tr("Pour associer", "To pair"));
        cv.text_fit(rx, 352.0, Text::new(Font::Semibold, 30.0, FROST), &format!("{} · {}", p.name, p.model), rw);
    }
    widgets::label(cv, rx, 430.0, tr("Ou bien", "Or"));
    let mut by = 454.0;
    widgets::secondary(cv, f, t(PsTarget::Manual), rx, by, tr("Saisir l'identifiant à la main", "Enter the ID by hand"), 24.0);
    by += 90.0;
    if app.ps.add.receive_expired {
        widgets::secondary(cv, f, t(PsTarget::NewCode), rx, by, tr("Nouveau code", "New code"), 24.0);
    }
}

/// « Saisir l'identifiant à la main » : « Identifiant du compte » et « Nom », « Continuer » ; le pavé des lettres.
/// « Renommer » : le nom seul.
fn manual(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let add = &app.ps.add;
    let renaming = add.rename.is_some();
    title(cv, if renaming { tr("Renommer", "Rename") } else { tr("Saisir l'identifiant à la main", "Enter the ID by hand") });
    let (x, w) = (96.0, 860.0);
    let mut y = 250.0;
    let field = |cv: &mut Canvas, f: &mut Frame, tg: Target, label: &str, help: &str, value: &str, active: bool, y: f32| -> f32 {
        widgets::label(cv, x, y + 20.0, label);
        let r = Rect::new(x, y + 34.0, w, 76.0);
        let focused = f.hit(tg, r);
        cv.rrect(r.x, r.y, r.w, r.h, 16.0, PANEL);
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 16.0, if active || focused { 2.0 } else { 1.0 }, if active || focused { SPECTRE } else { BORDER });
        let tw = cv.text_fit(r.x + 26.0, r.y + 50.0, Text::new(Font::Regular, 30.0, FROST), value, r.w - 52.0);
        if active {
            cv.rect(r.x + 30.0 + tw, r.y + 18.0, 3.0, 40.0, SPECTRE);
        }
        if help.is_empty() {
            34.0 + 76.0 + 24.0
        } else {
            cv.text_fit(x, y + 34.0 + 76.0 + 30.0, Text::new(Font::Regular, 19.0, FROST_3), help, w);
            34.0 + 76.0 + 56.0
        }
    };
    if !renaming {
        y += field(
            cv,
            f,
            t(PsTarget::ManualId),
            tr("Identifiant du compte", "Account ID"),
            tr("Le numéro du compte, ou son identifiant en base64.", "The account number, or its ID in base64."),
            &add.manual_id,
            add.manual_field == ManualField::Id,
            y,
        );
    }
    y += field(cv, f, t(PsTarget::ManualName), tr("Nom", "Name"), tr("Pour le reconnaître dans la liste.", "To recognise it in the list."), &add.manual_name, add.manual_field == ManualField::Name, y);
    let label = if renaming { tr("Renommer", "Rename") } else { tr("Continuer", "Continue") };
    let bw = widgets::primary_state(cv, f, t(PsTarget::ManualGo), x, y + 10.0, label, !add.busy);
    if add.busy {
        cv.circle(x + bw + 50.0, y + 50.0, 14.0, HALO.alpha(0.35));
        cv.circle(x + bw + 50.0, y + 50.0, 6.0, SPECTRE);
    }
    message(app, cv, x, y + 120.0, w);
    super::join::letter_pad(add.shift, cv, f);
    // Ce que le base64 demande en plus du pavé : + / =, et OK.
    let (kx, ky, kw, kh, gap) = (1030.0, 820.0, 68.0, 92.0, 10.0);
    for (i, c) in ['+', '/', '='].iter().enumerate() {
        key(cv, f, Target::Key(*c), Rect::new(kx + i as f32 * (kw + gap), ky, kw, kh), &c.to_string(), false);
    }
    key(cv, f, Target::KeyOk, Rect::new(kx + 3.0 * (kw + gap), ky, kw * 2.0 + gap, kh), "OK", true);
}

/// « Avec quel compte ? »
fn choose(app: &App, cv: &mut Canvas, f: &mut Frame) {
    title(cv, tr("Avec quel compte ?", "Which account?"));
    let Some(p) = &app.ps.add.pairing else { return };
    cv.text_fit(96.0, 262.0, Text::new(Font::Regular, 24.0, FROST_2), &format!("{} · {}", p.name, p.model), 1200.0);
    let mut y = 320.0;
    let ids = app.ps_candidates(&p.host_id);
    for (i, id) in ids.iter().enumerate().take(5) {
        let Some(a) = app.store.playstation.account(id) else { continue };
        let r = Rect::new(96.0, y, 860.0, 88.0);
        let focused = f.hit(t(PsTarget::Choose(i)), r);
        list_row(cv, focused, r, &a.label(), "", "");
        y += 102.0;
    }
    widgets::secondary(cv, f, t(PsTarget::Choose(ids.len())), 96.0, y + 10.0, tr("Ajouter un compte", "Add an account"), 22.0);
}

/// Le code à 8 chiffres de la console, au pavé des chiffres.
fn code(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let add = &app.ps.add;
    let Some(p) = &add.pairing else { return };
    title(cv, &format!("{} · {}", p.name, p.model));
    let account = p.account.as_ref().and_then(|id| app.store.playstation.account(id)).map(|a| a.label()).unwrap_or_default();
    widgets::label(cv, 96.0, 290.0, &if english() { format!("Account: {account}") } else { format!("Compte : {account}") });
    let used = cv.paragraph(
        96.0,
        340.0,
        Text::new(Font::Regular, 24.0, FROST_2),
        tr(
            "Sur la console : Réglages › Système › Lecture à distance › Associer un appareil. Saisissez le code affiché.",
            "On the console: Settings › System › Remote Play › Link Device. Enter the code shown.",
        ),
        860.0,
        36.0,
    );
    let y = 340.0 + used + 30.0;
    digits_field(cv, 96.0, y, &add.code, 8, tr("Code à 8 chiffres", "8-digit code"));
    let by = y + 190.0;
    let bw = widgets::primary_state(cv, f, t(PsTarget::PairGo), 96.0, by, tr("Associer", "Pair"), add.code.len() == 8 && !add.busy);
    if add.busy {
        cv.circle(96.0 + bw + 50.0, by + 40.0, 14.0, HALO.alpha(0.35));
        cv.circle(96.0 + bw + 50.0, by + 40.0, 6.0, SPECTRE);
    }
    message(app, cv, 96.0, by + 110.0, 860.0);
    digit_pad(cv, f);
}

/// « Envoyer à un appareil » : le code que l'autre appareil affiche.
fn send(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let add = &app.ps.add;
    let name = add.send_to.as_ref().map(|w| w.name.clone()).unwrap_or_default();
    title(cv, tr("Envoyer à un appareil", "Send to a device"));
    cv.text_fit(96.0, 300.0, Text::new(Font::Regular, 24.0, FROST_2), &if english() { format!("Enter the code shown on {name}.") } else { format!("Saisissez le code affiché sur {name}.") }, 860.0);
    digits_field(cv, 96.0, 360.0, &add.send_code, 6, tr("Code à 6 chiffres", "6-digit code"));
    let by = 550.0;
    let bw = widgets::primary_state(cv, f, t(PsTarget::SendGo), 96.0, by, tr("Envoyer", "Send"), add.send_code.len() == 6 && !add.busy);
    if add.busy {
        cv.circle(96.0 + bw + 50.0, by + 40.0, 14.0, HALO.alpha(0.35));
        cv.circle(96.0 + bw + 50.0, by + 40.0, 6.0, SPECTRE);
    }
    message(app, cv, 96.0, by + 110.0, 860.0);
    digit_pad(cv, f);
}

/// Un champ de chiffres en cases (le code de la console, celui d'un envoi).
fn digits_field(cv: &mut Canvas, x: f32, y: f32, value: &str, n: usize, label: &str) {
    widgets::label(cv, x, y, label);
    let (dw, dh, gap) = (if n > 6 { 92.0 } else { 116.0 }, 120.0, 12.0);
    let digits: Vec<char> = value.chars().collect();
    let mut dx = x;
    for i in 0..n {
        if n == 8 && i == 4 || n == 6 && i == 3 {
            dx += 18.0;
        }
        let current = i == digits.len();
        cv.rrect(dx, y + 20.0, dw, dh, 18.0, PANEL_SOLID);
        cv.rrect_stroke(dx, y + 20.0, dw, dh, 18.0, if current { 2.5 } else { 1.0 }, if current { SPECTRE } else { BORDER });
        if let Some(c) = digits.get(i) {
            cv.text(dx + dw / 2.0, y + 20.0 + dh / 2.0 + 28.0, Text::new(Font::MonoMedium, 76.0, FROST).align(Align::Center), &c.to_string());
        }
        dx += dw + gap;
    }
}

/// Le pavé des chiffres à droite (la télécommande a aussi ses touches chiffrées).
fn digit_pad(cv: &mut Canvas, f: &mut Frame) {
    let (kx, ky, kw, kh, gap) = (1160.0, 260.0, 170.0, 120.0, 18.0);
    let keys = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];
    for (i, c) in keys.iter().enumerate() {
        let (col, row) = (i % 3, i / 3);
        key(cv, f, Target::Key(*c), Rect::new(kx + col as f32 * (kw + gap), ky + row as f32 * (kh + gap), kw, kh), &c.to_string(), false);
    }
    let y = ky + 3.0 * (kh + gap);
    key(cv, f, Target::KeyErase, Rect::new(kx, y, kw, kh), "⌫", false);
    key(cv, f, Target::Key('0'), Rect::new(kx + kw + gap, y, kw, kh), "0", false);
    key(cv, f, Target::KeyOk, Rect::new(kx + 2.0 * (kw + gap), y, kw, kh), "OK", true);
}

// MARK: - Réglages d'une console

/// « Réglages de {nom} » : la même présentation que ceux d'un ordinateur (rubriques à gauche, lignes à droite).
pub fn settings(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    let Some(c) = app.ps_settings_console() else { return };
    cv.rect(520.0, 0.0, 1.0, H, LINE);
    widgets::label(cv, 96.0, 104.0, if english() { "Settings for" } else if crate::i18n::de(&c.name) == "d'" { "Réglages d'" } else { "Réglages de" });
    cv.text_fit(92.0, 172.0, Text::new(Font::Title, 56.0, FROST).tracking(-1.5), &c.name, 410.0);
    cv.text(96.0, 222.0, Text::new(Font::Regular, 22.0, FROST_3), &c.model);
    let mut y = 300.0;
    for (i, sec) in PS_SECTIONS.iter().enumerate() {
        let r = Rect::new(96.0, y, 384.0, 72.0);
        let focused = f.hit(t(PsTarget::Section(i)), r);
        let current = app.ps.section == i;
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 0.8);
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, CORE);
            cv.text_fit(r.x + 26.0, r.y + 46.0, Text::new(Font::Semibold, 25.0, INK), sec.label(), r.w - 40.0);
        } else {
            if current {
                cv.rrect(r.x, r.y, r.w, r.h, 14.0, SPECTRE.alpha(0.14));
            }
            cv.text_fit(r.x + 26.0, r.y + 46.0, Text::new(if current { Font::Semibold } else { Font::Regular }, 25.0, FROST), sec.label(), r.w - 40.0);
        }
        y += 84.0;
    }
    rows(app, cv, f, c);
    if app.ps.dialog.is_some() {
        f.hits.clear();
        dialog(app, cv, f);
    }
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Changer", "Change")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

fn rows(app: &App, cv: &mut Canvas, f: &mut Frame, c: &PsConsole) {
    let s = |s: PsSetting| Some(t(PsTarget::Setting(s)));
    let (x, w, gap) = (600.0, W - 110.0 - 600.0, 14.0);
    let mut y = 88.0;
    let st = &c.settings;
    match PS_SECTIONS[app.ps.section.min(PS_SECTIONS.len() - 1)] {
        PsSection::Picture => {
            let res: Vec<String> = vec!["Auto".into(), "1080p".into(), "720p".into(), "540p".into(), "360p".into()];
            let ri = ["auto", "p1080", "p720", "p540", "p360"].iter().position(|v| *v == st.resolution).unwrap_or(0);
            let help = if ri == 0 { tr("La plus haute que la console accepte : 1080p.", "The highest the console accepts: 1080p.") } else { tr("Plus bas : moins de débit.", "Lower: less bitrate.") };
            y += widgets::row(cv, f, s(PsSetting::Resolution), x, y, w, tr("Définition", "Resolution"), help, Value::Choices(&res, ri)) + gap;
            let fps: Vec<String> = if english() { vec!["60 fps".into(), "30 fps".into()] } else { vec!["60 i/s".into(), "30 i/s".into()] };
            y += widgets::row(cv, f, s(PsSetting::Fps), x, y, w, tr("Images par seconde", "Frame rate"), "", Value::Choices(&fps, usize::from(st.fps == 30))) + gap;
            let rate = match st.bitrate_kbps {
                None => "Auto".to_string(),
                Some(k) => format!("{} Mbit/s", k / 1000),
            };
            let help = if st.bitrate_kbps.is_none() { tr("Suit la définition.", "Follows the resolution.") } else { "" };
            y += widgets::row(cv, f, s(PsSetting::Bitrate), x, y, w, tr("Débit", "Bitrate"), help, Value::Text(rate)) + gap;
            if c.ps5() {
                let codecs: Vec<String> = vec!["Auto".into(), "H.264".into(), "H.265".into()];
                let ci = ["auto", "h264", "h265"].iter().position(|v| *v == st.codec).unwrap_or(0);
                let help = match ci {
                    0 => tr("H.265 si cet appareil le lit.", "H.265 if this device can play it."),
                    1 => tr("Sans HDR.", "No HDR."),
                    _ => "",
                };
                y += widgets::row(cv, f, s(PsSetting::Codec), x, y, w, tr("Format", "Format"), help, Value::Choices(&codecs, ci)) + gap;
            } else {
                y += widgets::row(cv, f, None, x, y, w, tr("Format", "Format"), tr("La PS4 n'envoie que du H.264.", "The PS4 only sends H.264."), Value::Unavailable("H.264")) + gap;
            }
            // « HDR » : la puce de l'accueil (PS5, écran HDR).
            if c.ps5() && app.tv.hdr {
                let ok = app.ps_hdr_available(c);
                let (help, value) = if ok {
                    (crate::app::hdr_setting_help(), Value::Toggle(c.hdr))
                } else if st.codec == "h264" {
                    (tr("Le H.264 n'a pas de HDR.", "H.264 has no HDR."), Value::Unavailable(tr("indisponible", "unavailable")))
                } else {
                    (crate::app::HdrReason::Decoder.text(), Value::Unavailable(tr("indisponible", "unavailable")))
                };
                widgets::row(cv, f, s(PsSetting::Hdr), x, y, w, "HDR", help, value);
            }
        }
        PsSection::Controls => {
            let pct = |v: u8| if v == 0 { tr("coupé", "off").to_string() } else { format!("{v} %") };
            y += widgets::row(cv, f, s(PsSetting::Rumble), x, y, w, tr("Vibrations", "Rumble"), "", Value::Text(pct(st.rumble))) + gap;
            y += widgets::row(cv, f, s(PsSetting::Triggers), x, y, w, tr("Gâchettes adaptatives", "Adaptive triggers"), "", Value::Text(pct(st.triggers))) + gap;
            cv.paragraph(
                x + 4.0,
                y + 30.0,
                Text::new(Font::Regular, 20.0, FROST_3),
                tr(
                    "Pour la DualSense, quand le téléviseur donne ses moteurs et ses gâchettes à l'app. Sans manette, la Magic Remote tient la croix (flèches), Croix (OK) et Options (Retour ouvre la barre de la partie) ; un clavier branché suit la disposition par défaut.",
                    "For the DualSense, when the TV gives its motors and triggers to the app. With no controller, the Magic Remote holds the D-pad (arrows), Cross (OK) and Options (Back opens the game bar); a plugged-in keyboard follows the default layout.",
                ),
                w - 8.0,
                28.0,
            );
        }
        PsSection::Console => {
            let accounts: Vec<String> = c.paired_accounts(&app.store.playstation.accounts).iter().map(|a| a.label()).collect();
            let who = if accounts.is_empty() { tr("Pas encore associée", "Not paired yet").to_string() } else { accounts.join(", ") };
            y += widgets::row(cv, f, None, x, y, w, tr("Associée avec", "Paired with"), "", Value::Text(who)) + gap;
            if let Some(a) = c.addresses.first() {
                y += widgets::row(cv, f, None, x, y, w, tr("Adresse", "Address"), "", Value::Text(a.clone())) + gap;
            }
            widgets::row(cv, f, s(PsSetting::Remove), x, y, w, tr("Retirer la console", "Remove the console"), "", Value::Danger(tr("Retirer", "Remove")));
        }
    }
}

/// Une confirmation des écrans des consoles.
pub fn dialog(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some((title, body, ok)) = app.ps_dialog_texts() else { return };
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    let (w, h) = (1000.0, 360.0);
    let (x, y) = ((W - w) / 2.0, (H - h) / 2.0);
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    cv.text_fit(x + 60.0, y + 100.0, Text::new(Font::Title, 40.0, FROST), &title, w - 120.0);
    cv.paragraph(x + 60.0, y + 156.0, Text::new(Font::Regular, 24.0, FROST_2), &body, w - 120.0, 34.0);
    let by = y + h - 110.0;
    let bw = widgets::primary(cv, f, t(PsTarget::DialogOk), x + 60.0, by, ok);
    widgets::secondary(cv, f, t(PsTarget::DialogCancel), x + 60.0 + bw + 28.0, by + 10.0, tr("Annuler", "Cancel"), 24.0);
}

// MARK: - Partie

/// Les mesures d'une partie (docs/link-ui.md, « Barre de la partie », « Statistiques ») : trois lignes en capsule.
pub fn stats(cv: &mut Canvas, s: &crate::app::SessionUi, y: f32) {
    let Some(ps) = &s.ps else { return };
    let lines: Vec<String> = match &ps.stats {
        Some(st) => {
            let mut l = vec![st.line1(), st.line2(), st.line3()];
            // Par le relais : la ligne « Chemin », comme un ordinateur.
            if s.via_relay() {
                let ms = s.route.and_then(|r| r.total_ms()).map(|v| format!(" · {} ms", crate::i18n::decimal(v, 0))).unwrap_or_default();
                l.push(format!("{}{} {}{ms}", tr("Chemin", "Path"), if english() { ":" } else { " :" }, s.path().label()));
            }
            l
        }
        None => vec![tr("Mesures…", "Measuring…").to_string()],
    };
    let style = Text::new(Font::Mono, 19.0, FROST);
    let wmax = lines.iter().map(|l| cv.text_width(style, l)).fold(0.0, f32::max);
    let (w, h) = (wmax + 96.0, 28.0 + lines.len() as f32 * 34.0);
    let x = (W - w) / 2.0;
    cv.rrect(x, y, w, h, 24.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, w, h, 24.0, 1.0, SPECTRE.alpha(0.5));
    cv.circle(x + 30.0, y + 36.0, 5.0, SPECTRE);
    for (i, l) in lines.iter().enumerate() {
        cv.text(x + 52.0, y + 44.0 + i as f32 * 34.0, if i == 0 { style } else { style.color(FROST_2) }, l);
    }
}

/// « La console ne peut pas diffuser ce qu'elle affiche. », sur l'image tant que dure le blocage.
pub fn cant_display(cv: &mut Canvas) {
    let msg = tr("La console ne peut pas diffuser ce qu'elle affiche.", "The console can't stream what it's showing.");
    let style = Text::new(Font::Regular, 26.0, FROST);
    let w = cv.text_width(style, msg) + 96.0;
    let (x, y) = ((W - w) / 2.0, H / 2.0 - 40.0);
    cv.rrect(x, y, w, 80.0, 24.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, w, 80.0, 24.0, 1.0, BORDER);
    cv.text(x + 48.0, y + 50.0, style, msg);
}

/// Le clavier de la console (elle a ouvert son clavier à l'écran) : en bas de l'image, le bandeau, le texte, le pavé
/// des lettres ; OK valide, Retour annule.
pub fn keyboard(cv: &mut Canvas, f: &mut Frame, ps: &crate::ps::PsSessionUi) {
    let Some(text) = &ps.keyboard else { return };
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 150));
    let (x, y, w) = (96.0, 300.0, 860.0);
    widgets::label(cv, x, y - 20.0, tr("Clavier de la console", "Console keyboard"));
    let r = Rect::new(x, y, w, 96.0);
    cv.rrect(r.x, r.y, r.w, r.h, 18.0, PANEL_SOLID);
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 18.0, 2.0, SPECTRE);
    let tw = cv.text_fit(r.x + 30.0, r.y + 62.0, Text::new(Font::Regular, 34.0, FROST), text, r.w - 60.0);
    cv.rect(r.x + 34.0 + tw, r.y + 22.0, 3.0, 52.0, SPECTRE);
    let by = y + 140.0;
    let bw = widgets::primary(cv, f, t(PsTarget::KeyboardAccept), x, by, tr("Valider", "Accept"));
    widgets::secondary(cv, f, t(PsTarget::KeyboardCancel), x + bw + 28.0, by + 10.0, tr("Annuler", "Cancel"), 24.0);
    super::join::letter_pad(ps.keyboard_shift, cv, f);
    // Le bandeau, comme celui des aides de la souris.
    let msg = tr("Tapez votre texte · OK pour valider · Retour pour annuler", "Type your text · OK to accept · Back to cancel");
    let style = Text::new(Font::Regular, 22.0, FROST);
    let mw = cv.text_width(style, msg) + 72.0;
    let mx = (W - mw) / 2.0;
    cv.rrect(mx, H - 120.0, mw, 60.0, 30.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(mx, H - 120.0, mw, 60.0, 30.0, 1.0, SPECTRE.alpha(0.4));
    cv.text(mx + 36.0, H - 81.0, style, msg);
}
