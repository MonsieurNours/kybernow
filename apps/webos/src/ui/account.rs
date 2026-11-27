//! Comptes KyberNow (docs/comptes.md ; textes : docs/link-ui.md, « Comptes KyberNow ») : l'écran du code (titre, trois
//! étapes, le code en chasse fixe, son QR code, l'attente, « Relais : {relais} », « Un autre relais… », « Retour » ;
//! refusé : « Connexion refusée sur le téléphone. » et « Nouveau code »), « Un autre relais » (le champ, au pavé de la
//! télécommande ou au clavier branché) et « Gérer le compte » (le QR code de la page du compte).

use super::canvas::{Align, Canvas, Font, Text, H, W};
use super::pair::key;
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::accounts::{accounts_help, open_step, relay_line, waiting_line, AccountStep};
use crate::app::{App, Rect, Target};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    match app.accounts.flow.step.clone() {
        AccountStep::Code => code(app, cv, f),
        AccountStep::OtherRelay => other_relay(app, cv, f),
        AccountStep::Manage(_) => manage(app, cv, f),
    }
    let y = H - 48.0;
    let x = 96.0 + widgets::hint(cv, 96.0, y, Key::Ok, tr("Choisir", "Choose")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

/// L'attente : un cercle ouvert, au trait.
fn spinner(cv: &mut Canvas, x: f32, cy: f32) {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f32() % 60.0).unwrap_or(0.0);
    for i in 0..12 {
        let a = t * 4.0 + i as f32 * 0.4;
        cv.circle(x + 14.0 + 12.0 * a.cos(), cy + 12.0 * a.sin(), 2.4, SPECTRE.alpha(0.3 + 0.06 * i as f32));
    }
}

/// Le code : les trois étapes, le code, l'attente, le relais ; le QR code à droite.
fn code(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let fl = &app.accounts.flow;
    widgets::label(cv, 96.0, 104.0, "KyberNow");
    cv.text_fit(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), tr("Connecter un compte KyberNow", "Connect a KyberNow account"), W - 190.0);
    let url_label = fl.code.as_ref().map(|c| c.url_label.clone()).unwrap_or_else(|| format!("{}/tv", crate::model::relay_label(&fl.relay)));
    let steps = [tr("Scannez le QR code avec votre téléphone,", "Scan the QR code with your phone,").to_string(), open_step(&url_label), tr("puis touchez « Connecter ».", "then tap “Connect”.").to_string()];
    let mut y = 272.0;
    for (i, s) in steps.iter().enumerate() {
        cv.text(96.0, y, Text::new(Font::Mono, 22.0, SPECTRE), &format!("{}", i + 1));
        cv.text_fit(140.0, y, Text::new(Font::Regular, 28.0, FROST_2), s, 1060.0);
        y += 50.0;
    }
    // Pas de navigateur sur le téléviseur : « Créer un compte », c'est cette ligne (docs/link-ui.md).
    cv.text_fit(140.0, y + 2.0, Text::new(Font::Regular, 24.0, FROST_3), crate::welcome::no_account_yet(), 1060.0);
    y += 36.0;
    let code_style = Text::new(Font::MonoMedium, 120.0, FROST).tracking(8.0);
    if fl.denied || fl.message.is_some() {
        let m = if fl.denied { tr("Connexion refusée sur le téléphone.", "Sign-in declined on the phone.").to_string() } else { fl.message.clone().unwrap_or_default() };
        y += cv.paragraph(96.0, y + 40.0, Text::new(Font::Regular, 28.0, WARN), &m, 1100.0, 38.0) + 30.0;
        widgets::primary(cv, f, Target::AccountNewCode, 96.0, y, tr("Nouveau code", "New code"));
        y += 84.0 + 40.0;
    } else {
        match &fl.code {
            Some(c) => {
                cv.text(92.0, y + 110.0, code_style, &c.code);
                y += 150.0;
                spinner(cv, 96.0, y + 4.0);
                cv.text_fit(140.0, y + 14.0, Text::new(Font::Regular, 26.0, FROST_2), &waiting_line(c.minutes_left(crate::model::now_secs()).max(1)), 1060.0);
            }
            None => {
                cv.text(92.0, y + 110.0, code_style.color(FROST_4), "····-····");
                y += 150.0;
                spinner(cv, 96.0, y + 4.0);
            }
        }
        y += 60.0;
    }
    cv.text_fit(96.0, y + 10.0, Text::new(Font::Mono, 22.0, FROST_3), &relay_line(&fl.relay), 1100.0);
    y += 40.0;
    let w = widgets::secondary(cv, f, Target::AccountOtherRelay, 96.0, y, tr("Un autre relais…", "Another relay…"), 24.0);
    widgets::secondary(cv, f, Target::AccountBack, 96.0 + w + 20.0, y, tr("Retour", "Back"), 24.0);
    // Le QR code de la page avec le code.
    if let Some(c) = fl.code.as_ref().filter(|_| !fl.denied && fl.message.is_none()) {
        let side = 420.0;
        let (qx, qy) = (W - 96.0 - side, 260.0);
        if widgets::qr(cv, &c.url_complete, qx, qy, side).is_some() {
            cv.text(qx + side / 2.0, qy + side + 46.0, Text::new(Font::Regular, 22.0, FROST_3).align(Align::Center), tr("QR code de connexion", "QR code to sign in"));
        }
    }
}

/// « Un autre relais » : le champ « Relais », son aide, le refus, « Continuer », « Annuler » ; le pavé à droite.
fn other_relay(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let fl = &app.accounts.flow;
    widgets::label(cv, 96.0, 104.0, "KyberNow");
    cv.text(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), tr("Un autre relais", "Another relay"));
    widgets::label(cv, 96.0, 290.0, tr("Relais", "Relay"));
    let field = Rect::new(96.0, 312.0, 860.0, 88.0);
    if f.hit(Target::AccountRelayField, field) {
        cv.neon(field.x, field.y, field.w, field.h, 18.0, 0.8);
    }
    cv.rrect(field.x, field.y, field.w, field.h, 18.0, PANEL_SOLID);
    cv.rrect_stroke(field.x, field.y, field.w, field.h, 18.0, 2.5, SPECTRE);
    let tw = if fl.field.is_empty() {
        cv.text(field.x + 32.0, field.y + 58.0, Text::new(Font::Mono, 32.0, FROST_4), tr("relais.exemple.org:9443", "relay.example.org:9443"));
        0.0
    } else {
        cv.text_fit(field.x + 32.0, field.y + 58.0, Text::new(Font::Mono, 32.0, FROST), &fl.field, field.w - 80.0)
    };
    cv.rect(field.x + 36.0 + tw, field.y + 20.0, 4.0, 48.0, SPECTRE);
    let mut y = 440.0;
    y += cv.paragraph(96.0, y, Text::new(Font::Regular, 22.0, FROST_3), tr("L'adresse du relais de votre compte, si ce n'est pas share.kybernow.app.", "Your account's relay address, if it isn't share.kybernow.app."), 860.0, 32.0);
    if let Some(m) = &fl.field_message {
        y += 10.0 + cv.paragraph(96.0, y + 10.0, Text::new(Font::Regular, 24.0, WARN), m, 860.0, 34.0);
    }
    y += 30.0;
    let bw = widgets::primary_state(cv, f, Target::AccountRelayGo, 96.0, y, tr("Continuer", "Continue"), !fl.field.trim().is_empty());
    widgets::secondary(cv, f, Target::AccountRelayCancel, 96.0 + bw + 28.0, y + 10.0, tr("Annuler", "Cancel"), 26.0);
    address_pad(cv, f);
}

/// Le pavé d'une adresse de relais : chiffres, lettres, `.`, `:`, `-`, effacer, OK.
fn address_pad(cv: &mut Canvas, f: &mut Frame) {
    let (x, y, gap) = (1030.0, 300.0, 10.0);
    let (kw, kh) = (68.0, 92.0);
    let mut ky = y;
    for row in ["1234567890", "qwertyuiop", "asdfghjkl.", "zxcvbnm:-"] {
        let mut kx = x;
        for c in row.chars() {
            key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &c.to_string(), false);
            kx += kw + gap;
        }
        if row.starts_with('z') {
            key(cv, f, Target::KeyErase, Rect::new(kx, ky, kw, kh), "⌫", false);
        }
        ky += kh + gap;
    }
    key(cv, f, Target::KeyOk, Rect::new(x, ky, 4.0 * kw + 3.0 * gap, kh), "OK", true);
}

/// « Gérer le compte » : le compte, l'aide, la page du compte en QR code et son adresse.
fn manage(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(a) = app.managed_account() else { return };
    widgets::label(cv, 96.0, 104.0, tr("Comptes", "Accounts"));
    cv.text(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), tr("Gérer le compte", "Manage account"));
    let mut y = 290.0;
    cv.text_fit(96.0, y, Text::new(Font::Semibold, 36.0, FROST), &a.label(), 1100.0);
    y += 50.0;
    if !a.account.name.trim().is_empty() && !a.account.email.is_empty() {
        cv.text_fit(96.0, y, Text::new(Font::Regular, 28.0, FROST_2), &a.account.email, 1100.0);
        y += 44.0;
    }
    cv.text_fit(96.0, y, Text::new(Font::Mono, 24.0, FROST_3), &a.relay_label(), 1100.0);
    y += 60.0;
    y += cv.paragraph(96.0, y, Text::new(Font::Regular, 26.0, FROST_2), accounts_help(), 1000.0, 36.0) + 30.0;
    widgets::secondary(cv, f, Target::AccountBack, 96.0, y, tr("Retour", "Back"), 24.0);
    let url = a.console();
    let side = 420.0;
    let (qx, qy) = (W - 96.0 - side, 260.0);
    if widgets::qr(cv, &url, qx, qy, side).is_some() {
        let center = Text::new(Font::Regular, 22.0, FROST_2).align(Align::Center);
        let used = cv.paragraph(qx + side / 2.0, qy + side + 46.0, center, tr("Scannez ce code avec votre téléphone pour ouvrir la page du compte.", "Scan this code with your phone to open the account's page."), side + 60.0, 32.0);
        cv.text(qx + side / 2.0, qy + side + 56.0 + used, Text::new(Font::Mono, 18.0, FROST_3).align(Align::Center), &url);
    }
}
