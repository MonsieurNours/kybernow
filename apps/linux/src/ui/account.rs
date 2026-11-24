//! Comptes KyberNow (docs/comptes.md ; textes : docs/link-ui.md, « Comptes KyberNow ») : l'écran du code (titre, trois
//! étapes, le code en chasse fixe, son QR code, l'attente, « Relais : {relais} », « Un autre relais… », « Retour » ;
//! refusé : « Connexion refusée sur le téléphone. » et « Nouveau code »), « Connexion au compte… » pendant que la page
//! du relais est ouverte dans le navigateur (bureau), le relais d'abord (« Ajouter un compte » au bureau), « Un autre
//! relais » (le champ, au clavier ou au pavé), et « Gérer le compte » au salon (le QR code de la page du compte).

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::pair::key;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, primary};
use super::{launch, title_bar, unit, Frame};
use crate::accounts::{open_step, relay_line, sign_in_help, waiting_line, AccountStep};
use crate::app::{App, Layout, Rect, Target};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let console = app.layout.console();
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
    let mut y = top + 28.0 * u;
    if app.layout == Layout::Desktop {
        widgets::ghost(cv, f, Target::AccountBack, x0, y, 40.0, 14.0, Some(Icon::Back), tr("Retour", "Back"), MUTED);
        y += 56.0;
    }
    match app.accounts.flow.step.clone() {
        AccountStep::Code => code(app, cv, f, x0, y, u),
        AccountStep::Browser => browser(app, cv, f, x0, y, u),
        AccountStep::Relay => relay_first(cv, f, x0, y, u),
        AccountStep::OtherRelay => other_relay(app, cv, f, x0, y, u),
        AccountStep::Manage(_) => manage(app, cv, f, x0, y, u),
    }
    if console {
        // La légende des boutons.
        let footer = 56.0 * u + if tv { 4.0 } else { 0.0 };
        cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
        cv.rect(0.0, h - footer, w, 1.0, LINE);
        let gy = h - footer / 2.0;
        let g = 26.0 * u;
        let x = x0 + legend(cv, x0, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
        legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
    }
}

/// Le titre de l'écran, et son surtitre.
fn heading(cv: &mut Canvas, x0: f32, y: f32, u: f32, eyebrow_text: &str, title: &str, max: f32) -> f32 {
    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, eyebrow_text);
    cv.text_shrink(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), title, max);
    80.0 * u
}

/// Le cercle d'attente, à gauche de `x`, centré sur `cy`.
fn spinner(app: &App, cv: &mut Canvas, x: f32, cy: f32, u: f32) {
    let t = app.screen_since.elapsed().as_secs_f32();
    let a0 = if app.reduce_motion { 0.0 } else { t * 4.0 };
    let pts: Vec<(f32, f32)> = (0..=16)
        .map(|i| {
            let a = a0 + i as f32 * 0.28;
            (x + 9.0 * u + 8.0 * u * a.cos(), cy + 8.0 * u * a.sin())
        })
        .collect();
    cv.polyline(&pts, 2.0 * u, SPECTRE);
}

/// Le code de téléviseur : les trois étapes, le code, l'attente, le relais ; le QR code à droite.
fn code(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let fl = &app.accounts.flow;
    let left_w = (w * 0.5).min(760.0 * u);
    let mut y = y0;
    y += heading(cv, x0, y, u, "KyberNow", tr("Se connecter à un compte KyberNow", "Sign in to a KyberNow account"), left_w + 120.0 * u);
    let url_label = fl.code.as_ref().map(|c| c.url_label.clone()).unwrap_or_else(|| format!("{}/tv", crate::model::relay_label(&fl.relay)));
    let steps = [tr("Scannez le QR code avec votre téléphone,", "Scan the QR code with your phone,").to_string(), open_step(&url_label), tr("puis touchez « Connecter ».", "then tap “Connect”.").to_string()];
    let body = Text::new(Font::Regular, 17.0 * u, MUTED);
    for (i, s) in steps.iter().enumerate() {
        cv.text(x0, y + 20.0 * u, Text::new(Font::Mono, 14.0 * u, SPECTRE), &format!("{}", i + 1));
        y += cv.paragraph(x0 + 28.0 * u, y + 20.0 * u, body, s, left_w - 28.0 * u, 26.0 * u) + 6.0 * u;
    }
    y += 24.0 * u;
    // Le code, en chasse fixe, gros.
    let code_style = Text::new(Font::MonoMedium, 64.0 * u, FROST).tracking(4.0 * u);
    match (&fl.code, fl.denied || fl.message.is_some()) {
        (Some(c), false) => {
            cv.text(x0 - 2.0, y + 60.0 * u, code_style, &c.code);
            y += 84.0 * u;
            spinner(app, cv, x0, y + 10.0 * u, u);
            let n = c.minutes_left(crate::model::now_secs());
            cv.text_fit(x0 + 30.0 * u, y + 16.0 * u, Text::new(Font::Regular, 16.0 * u, MUTED), &waiting_line(n.max(1)), left_w - 30.0 * u);
            y += 40.0 * u;
        }
        (_, false) => {
            // Le code se demande.
            cv.text(x0 - 2.0, y + 60.0 * u, code_style.color(GHOST), "····-····");
            y += 84.0 * u;
            spinner(app, cv, x0, y + 10.0 * u, u);
            y += 40.0 * u;
        }
        (_, true) => {
            let m = if fl.denied { tr("Connexion refusée sur le téléphone.", "Sign-in declined on the phone.").to_string() } else { fl.message.clone().unwrap_or_default() };
            y += cv.paragraph(x0, y + 22.0 * u, Text::new(Font::Regular, 17.0 * u, WARN), &m, left_w, 24.0 * u) + 16.0 * u;
            primary(cv, f, Target::AccountNewCode, x0, y, 52.0 * u, tr("Nouveau code", "New code"), false);
            y += 52.0 * u + 18.0 * u;
        }
    }
    // Le relais, « Un autre relais… », « Retour ».
    cv.text_fit(x0, y + 18.0 * u, Text::new(Font::Mono, 14.0 * u, FAINT), &relay_line(&fl.relay), left_w);
    y += 34.0 * u;
    let gw = widgets::ghost(cv, f, Target::AccountOtherRelay, x0, y, 44.0 * u, 15.0 * u, None, tr("Un autre relais…", "Another relay…"), MUTED);
    if app.layout.console() {
        widgets::ghost(cv, f, Target::AccountBack, x0 + gw + 14.0 * u, y, 44.0 * u, 15.0 * u, None, tr("Retour", "Back"), MUTED);
    }
    // Le QR code de la page avec le code, à droite.
    if let Some(c) = fl.code.as_ref().filter(|_| !fl.denied && fl.message.is_none()) {
        let side = (w - x0 - (x0 + left_w + 40.0 * u)).clamp(120.0 * u, 300.0 * u);
        let qx = w - x0 - side;
        if let Some(card) = widgets::qr_card(cv, &c.url_complete, qx, y0 + 90.0 * u, side) {
            cv.text_fit(qx + card / 2.0, y0 + 90.0 * u + card + 28.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT).align(Align::Center), tr("QR code de connexion", "QR code to sign in"), side + 40.0 * u);
        }
    }
}

/// Bureau : la page du relais est ouverte dans le navigateur.
fn browser(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let fl = &app.accounts.flow;
    let max = (w - 2.0 * x0).min(680.0);
    let mut y = y0;
    y += heading(cv, x0, y, u, "KyberNow", tr("Se connecter à un compte KyberNow", "Sign in to a KyberNow account"), max);
    match &fl.message {
        Some(m) => {
            y += cv.paragraph(x0, y + 22.0 * u, Text::new(Font::Regular, 16.0 * u, WARN), m, max, 24.0 * u) + 12.0 * u;
        }
        None => {
            spinner(app, cv, x0, y + 18.0 * u, u);
            cv.text(x0 + 30.0 * u, y + 24.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), tr("Connexion au compte…", "Signing in…"));
            y += 48.0 * u;
        }
    }
    y += cv.paragraph(x0, y + 16.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), sign_in_help(false), max, 22.0 * u) + 18.0 * u;
    cv.text_fit(x0, y + 14.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT), &relay_line(&fl.relay), max);
    let _ = f;
}

/// « Ajouter un compte » au bureau : le relais d'abord.
fn relay_first(cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let max = (w - 2.0 * x0).min(680.0);
    let mut y = y0;
    y += heading(cv, x0, y, u, "KyberNow", tr("Ajouter un compte", "Add an account"), max);
    eyebrow(cv, x0, y + 18.0 * u, 11.5 * u, FAINT, tr("Relais", "Relay"));
    y += 34.0 * u;
    let bw = primary(cv, f, Target::AccountRelayDefault, x0, y, 52.0 * u, crate::model::DEFAULT_RELAY, false);
    widgets::ghost(cv, f, Target::AccountOtherRelay, x0 + bw + 18.0 * u, y + 4.0 * u, 44.0 * u, 15.0 * u, None, tr("Un autre relais…", "Another relay…"), MUTED);
    y += 52.0 * u + 22.0 * u;
    cv.paragraph(x0, y + 14.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), sign_in_help(false), max, 22.0 * u);
}

/// « Un autre relais » : le champ « Relais », son aide, le refus, « Continuer » et « Annuler » ; au pavé, les touches
/// d'une adresse à droite.
fn other_relay(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let console = app.layout.console();
    let fl = &app.accounts.flow;
    let left_w = if console { (w * 0.46).min(560.0 * u) } else { (w - 2.0 * x0).min(640.0) };
    let mut y = y0;
    y += heading(cv, x0, y, u, "KyberNow", tr("Un autre relais", "Another relay"), left_w + 80.0 * u);
    eyebrow(cv, x0, y + 8.0 * u, 11.5 * u, FAINT, tr("Relais", "Relay"));
    let r = Rect::new(x0, y + 20.0 * u, left_w, 56.0 * u);
    let (focused, hover) = f.state(Target::AccountRelayField, r);
    if focused {
        widgets::ring(cv, r, 10.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.5, SPECTRE);
    let tw = if fl.field.is_empty() {
        cv.text(r.x + 18.0 * u, r.y + r.h / 2.0 + 7.0 * u, Text::new(Font::Mono, 18.0 * u, GHOST), "relais.exemple.org:9443");
        0.0
    } else {
        cv.text_fit(r.x + 18.0 * u, r.y + r.h / 2.0 + 7.0 * u, Text::new(Font::Mono, 18.0 * u, FROST), &fl.field, r.w - 40.0 * u)
    };
    if !app.reduce_motion && (app.screen_since.elapsed().as_millis() / 530) % 2 == 0 {
        cv.rect(r.x + 20.0 * u + tw, r.y + 14.0 * u, 2.0, r.h - 28.0 * u, SPECTRE);
    }
    y += 20.0 * u + 56.0 * u + 10.0 * u;
    y += cv.paragraph(x0, y + 14.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT), tr("L'adresse du relais de votre compte, si ce n'est pas share.kybernow.app.", "Your account's relay address, if it isn't share.kybernow.app."), left_w, 20.0 * u) + 6.0 * u;
    if let Some(m) = &fl.field_message {
        y += cv.paragraph(x0, y + 14.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), m, left_w, 22.0 * u) + 4.0 * u;
    }
    y += 18.0 * u;
    let bw = widgets::primary_state(cv, f, Target::AccountRelayGo, x0, y, 52.0 * u, tr("Continuer", "Continue"), false, !fl.field.trim().is_empty());
    widgets::ghost(cv, f, Target::AccountRelayCancel, x0 + bw + 18.0 * u, y + 6.0 * u, 40.0 * u, 14.0 * u, None, tr("Annuler", "Cancel"), MUTED);
    if console {
        let px = x0 + left_w + 40.0 * u;
        address_pad(cv, f, px, y0 + 80.0 * u, w - px - x0, u);
    }
}

/// Le pavé d'une adresse de relais : chiffres, lettres, `.`, `:`, `-`, effacer.
fn address_pad(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, width: f32, u: f32) {
    let gap = 8.0 * u;
    let kw = ((width - 9.0 * gap) / 10.0).min(64.0 * u);
    let kh = 52.0 * u;
    let rows: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl.", "zxcvbnm:-"];
    let mut ky = y;
    for row in rows {
        let mut kx = x;
        for c in row.chars() {
            key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &c.to_string(), u);
            kx += kw + gap;
        }
        if row.starts_with('z') {
            key(cv, f, Target::KeyErase, Rect::new(kx, ky, kw, kh), "⌫", u);
        }
        ky += kh + gap;
    }
}

/// « Gérer le compte » au salon : la page du compte en QR code, son adresse dessous.
fn manage(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = cv.w;
    let Some(a) = app.managed_account() else { return };
    let max = (w * 0.5).min(760.0 * u);
    let mut y = y0;
    y += heading(cv, x0, y, u, tr("Comptes", "Accounts"), tr("Gérer le compte", "Manage account"), max);
    cv.text_fit(x0, y + 24.0 * u, Text::new(Font::Semibold, 20.0 * u, FROST), &a.label(), max);
    y += 34.0 * u;
    if !a.account.name.trim().is_empty() && !a.account.email.is_empty() {
        cv.text_fit(x0, y + 18.0 * u, Text::new(Font::Regular, 16.0 * u, MUTED), &a.account.email, max);
        y += 28.0 * u;
    }
    cv.text_fit(x0, y + 18.0 * u, Text::new(Font::Mono, 14.0 * u, FAINT), &a.relay_label(), max);
    y += 40.0 * u;
    y += cv.paragraph(x0, y + 14.0 * u, Text::new(Font::Regular, 16.0 * u, MUTED), crate::accounts::accounts_help(), max, 24.0 * u) + 18.0 * u;
    widgets::ghost(cv, f, Target::AccountBack, x0, y, 44.0 * u, 15.0 * u, None, tr("Retour", "Back"), MUTED);
    let url = a.console();
    let side = (w - x0 - (x0 + max + 40.0 * u)).clamp(120.0 * u, 300.0 * u);
    let qx = w - x0 - side;
    if let Some(card) = widgets::qr_card(cv, &url, qx, y0 + 90.0 * u, side) {
        let mut yy = y0 + 90.0 * u + card + 28.0 * u;
        let center = Text::new(Font::Regular, 14.0 * u, MUTED).align(Align::Center);
        yy += cv.paragraph(qx + card / 2.0, yy, center, tr("Scannez ce code avec votre téléphone pour ouvrir la page du compte.", "Scan this code with your phone to open the account's page."), side + 40.0 * u, 20.0 * u);
        cv.text_fit(qx + card / 2.0, yy + 4.0 * u, Text::new(Font::Mono, 12.0 * u, FAINT).align(Align::Center), &url, side + 60.0 * u);
    }
}
