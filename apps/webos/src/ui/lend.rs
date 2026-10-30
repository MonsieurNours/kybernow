//! Le panneau de l'hôte du prêt de l'ordinateur (docs/pret.md ; textes : docs/link-ui.md, « Prêt de l'ordinateur ») :
//! « Prêter {nom} », l'avertissement de confiance jusqu'au premier code, la durée (« 2 heures » proposée), le compte
//! Steam (« Son compte Steam » proposé), « Créer un code » ; puis le code, le QR code (à scanner au téléphone),
//! l'adresse du client web en clair, « Pour une séance, valable 24 h. », l'aide, « Votre session se fermera quand votre
//! ami arrivera. », « Annuler le code » ; les demandes (« {prénom} veut emprunter l'ordinateur. », « Accepter »,
//! « Refuser », « Votre session va se fermer. » ; celle d'un créneau : son texte et « Accepter » seul). Il s'ouvre
//! depuis la tuile « Prêter l'ordinateur… » du menu ; Retour le referme. Pas de « Copier le lien » : un téléviseur n'a
//! pas de presse-papiers utile. Le lien du relais du code se prépare comme celui du partage (« Préparation du lien hors
//! de chez vous… », le QR code et l'adresse retenus `RELAY_LINK_WAIT` au plus, puis « Pas encore joignable hors de chez
//! vous. ») : mêmes dessins que le panneau du partage (`share::qr_or_wait`, `share::relay_lines`).

use super::canvas::{Canvas, Font, Rgba, Text, H, W};
use super::share::{button, qr_or_wait, relay_lines, wait_dot};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{format_code, lend_duration_label, App, LendAvail, LendSteam, Rect, SessionUi, Target, LEND_DURATIONS};
use crate::i18n::tr;

const PAD: f32 = 48.0;

const WARNING: (&str, &str) = (
    "Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance.",
    "Your friend plays in your Windows session. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.",
);
const HELP: (&str, &str) = (
    "Votre ami entre ce code avec « Rejoindre avec un code » dans KyberNow Link, ou ouvre ce lien dans un navigateur.",
    "Your friend enters this code with “Join with a code” in KyberNow Link, or opens this link in a browser.",
);

/// Pourquoi le prêt est impossible (Station trop ancienne, prêt coupé dans Admin, prêt pas possible sur cet ordinateur).
fn closed_text(app: &App, s: &SessionUi) -> Option<&'static str> {
    app.lend_avail().and_then(|a| a.text()).or_else(|| s.lend.as_ref().filter(|v| !v.enabled).and_then(|_| LendAvail::Off.text()))
}

fn lines(cv: &mut Canvas, size: f32, s: &str, w: f32) -> f32 {
    ((cv.text_width(Text::new(Font::Regular, size, FROST_2), s) / (w * 0.92)).ceil()).max(1.0)
}

fn height(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32) -> f32 {
    let inner = w - 2.0 * PAD;
    let mut h = 92.0;
    if let Some(t) = closed_text(app, s) {
        return h + 24.0 + lines(cv, 25.0, t, inner) * 36.0 + 40.0;
    }
    let v = s.lend.as_ref();
    match v.and_then(|v| v.code.as_ref()) {
        None => {
            if !app.store.app.lend_warned {
                h += lines(cv, 22.0, tr(WARNING.0, WARNING.1), inner) * 31.0 + 20.0;
            }
            h += 2.0 * (40.0 + 56.0 + 20.0) + 100.0;
        }
        Some(c) => {
            h += 224.0 + 36.0 + lines(cv, 22.0, tr(HELP.0, HELP.1), inner) * 31.0 + 40.0 + 20.0 + 96.0;
            // Lien du relais en préparation, ou relais réglé mais injoignable : une ligne sous l'aide.
            if c.relay_wait_text().is_some() || c.relay_down_text().is_some() {
                h += 31.0;
            }
        }
    }
    let requests = v.map_or(0, |v| v.requests.len());
    if requests > 0 {
        h += 52.0 + requests as f32 * 96.0 + 44.0;
    }
    if let Some(e) = &s.lend_error {
        h += lines(cv, 22.0, e, inner) * 31.0 + 12.0;
    }
    h + 12.0
}

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    let w = 1240.0;
    let h = height(cv, app, s, w).min(H - 100.0);
    let (x, y) = ((W - w) / 2.0, ((H - 80.0 - h) / 2.0).max(30.0));
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    card(app, cv, f, s, Rect::new(x, y, w, h));
    let hy = H - 48.0;
    let mut hx = 96.0;
    if closed_text(app, s).is_none() {
        hx += widgets::hint(cv, hx, hy, Key::Ok, tr("Choisir", "Select")) + 40.0;
    }
    widgets::hint(cv, hx, hy, Key::Back, tr("Retour", "Back"));
}

fn card(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, r: Rect) {
    let (x, mut y) = (r.x + PAD, r.y);
    let inner = r.w - 2.0 * PAD;
    let name = app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name);
    let title = format!("{} {name}", tr("Prêter", "Lend"));
    let mut right = r.x + r.w - PAD;
    if s.lend_busy {
        wait_dot(cv, s, right - 14.0, y + 50.0, 1.0);
        right -= 48.0;
    }
    cv.text_fit(x, y + 62.0, Text::new(Font::Title, 40.0, FROST), &title, (right - x).max(80.0));
    y += 92.0;
    if let Some(t) = closed_text(app, s) {
        // Rien à choisir : Retour ferme le panneau.
        f.hit(Target::LendClose, Rect::new(r.x, r.y, 1.0, 1.0));
        cv.paragraph(x, y + 36.0, Text::new(Font::Regular, 25.0, FROST), t, inner, 36.0);
        return;
    }
    let v = s.lend.as_ref();
    match v.and_then(|v| v.code.as_ref()) {
        None => {
            if !app.store.app.lend_warned {
                y += cv.paragraph(x, y + 8.0, Text::new(Font::Regular, 22.0, WARN), tr(WARNING.0, WARNING.1), inner, 31.0) + 20.0;
            }
            widgets::label(cv, x, y + 26.0, tr("Durée", "Duration"));
            y += 40.0;
            let mut cx = x;
            for (i, d) in LEND_DURATIONS.iter().enumerate() {
                cx += widgets::chip(cv, f, Target::LendDuration(i), cx, y, &lend_duration_label(*d), s.lend_duration == i) + 10.0;
            }
            y += 56.0 + 20.0;
            widgets::label(cv, x, y + 26.0, tr("Compte Steam", "Steam account"));
            y += 40.0;
            let mut cx = x;
            for st in LendSteam::ALL {
                cx += widgets::chip(cv, f, Target::LendSteamPick(st), cx, y, st.label(), s.lend_steam == st) + 10.0;
            }
            y += 56.0 + 20.0;
            widgets::primary_state(cv, f, Target::LendCreate, x, y + 4.0, tr("Créer un code", "Create a code"), !s.lend_busy);
            y += 100.0;
        }
        Some(c) => {
            let qr = 200.0;
            widgets::label(cv, x, y + 22.0, tr("Code", "Code"));
            cv.text(x - 2.0, y + 114.0, Text::new(Font::MonoMedium, 96.0, FROST).tracking(6.0), &format_code(&c.code));
            cv.text_fit(x, y + 160.0, Text::new(Font::Regular, 22.0, FROST_3), &format!("{} · {}", lend_duration_label(c.duration), c.steam.label()), inner - qr - 40.0);
            // Le lien du relais en préparation : l'indicateur d'attente à la place du QR code, l'adresse du réseau pas
            // encore montrée (`RELAY_LINK_WAIT` au plus), comme le partage.
            qr_or_wait(cv, s, &c.link, &c.web, c.relay_preparing(), r.x + r.w - PAD - qr, y + 4.0, qr, (x, y + qr - 12.0), inner - qr - 40.0);
            y += 224.0;
            cv.text_fit(x, y + 26.0, Text::new(Font::Regular, 22.0, FROST), tr("Pour une séance, valable 24 h.", "For one session, valid for 24 h."), inner);
            y += 36.0;
            y += cv.paragraph(x, y + 24.0, Text::new(Font::Regular, 22.0, FROST_2), tr(HELP.0, HELP.1), inner, 31.0);
            y += relay_lines(cv, s, c.relay_wait_text(), c.relay_down_text(), x, y, inner);
            cv.text_fit(x, y + 34.0, Text::new(Font::Regular, 22.0, FROST), tr("Votre session se fermera quand votre ami arrivera.", "Your session will close when your friend arrives."), inner);
            y += 40.0 + 20.0;
        }
    }
    // Les demandes : « Léa veut emprunter l'ordinateur. », son app, « Refuser », « Accepter » ; celle d'un créneau :
    // le texte du créneau et « Accepter » seul.
    if let Some(v) = v.filter(|v| !v.requests.is_empty()) {
        cv.rect(x, y, inner, 1.0, LINE);
        widgets::label(cv, x, y + 34.0, tr("Demandes", "Requests"));
        y += 52.0;
        for (i, rq) in v.requests.iter().enumerate() {
            let (decline, accept) = (tr("Refuser", "Decline"), tr("Accepter", "Accept"));
            let size = 22.0;
            let dw = cv.text_width(Text::new(Font::Semibold, size, DANGER), decline) + size * 1.6;
            let aw = cv.text_width(Text::new(Font::Semibold, size, FROST), accept) + size * 1.6;
            let accept_x = x + inner - aw;
            // La demande d'un créneau : « Accepter » seul (l'ordinateur lui est prêté dans 2 min de toute façon).
            let decline_x = if rq.slot { accept_x } else { accept_x - 14.0 - dw };
            if !rq.slot {
                button(cv, f, Target::LendRefuse(i), decline_x, y + 6.0, decline, size, true);
            }
            button(cv, f, Target::LendAccept(i), accept_x, y + 6.0, accept, size, false);
            cv.text_fit(x, y + 34.0, Text::new(Font::Semibold, 27.0, FROST), &rq.line(), (decline_x - x - 24.0).max(80.0));
            cv.text_fit(x, y + 66.0, Text::new(Font::Regular, 20.0, FROST_3), &rq.meta(), inner);
            y += 96.0;
        }
        cv.text_fit(x, y + 24.0, Text::new(Font::Regular, 22.0, FROST_2), tr("Votre session va se fermer.", "Your session will close."), inner);
        y += 44.0;
    }
    if let Some(e) = &s.lend_error {
        y += cv.paragraph(x, y + 6.0, Text::new(Font::Regular, 22.0, DANGER), e, inner, 31.0) + 12.0;
    }
    if v.is_some_and(|v| v.code.is_some()) {
        button(cv, f, Target::LendCancelCode, x, y + 20.0, tr("Annuler le code", "Cancel code"), 24.0, true);
    }
}

/// Le sous-titre de la tuile « Prêter l'ordinateur… » du menu, et si elle est allumée : « 1 demande », le code en
/// cours, sinon « Code et QR code ».
pub fn tile(s: &SessionUi) -> (String, bool) {
    match &s.lend {
        Some(v) if !v.requests.is_empty() => (crate::app::requests_label(v.requests.len()), true),
        Some(v) if v.code.is_some() => (format_code(&v.code.as_ref().map(|c| c.code.clone()).unwrap_or_default()), true),
        _ => (tr("Code et QR code", "Code and QR code").to_string(), false),
    }
}
