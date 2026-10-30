//! Le panneau de l'hôte du prêt de l'ordinateur (docs/pret.md ; textes : docs/link-ui.md, « Prêt de l'ordinateur ») :
//! « Prêter {nom} », la durée (« 1 heure », « 2 heures » proposée, « 4 heures », « Jusqu'à ce que je reprenne »), le
//! compte Steam (« Son compte Steam » proposé, « Votre compte Steam »), « Créer un code » ; puis le code, « Copier le
//! lien » (au bureau seulement), le QR code et l'adresse du client web comme le partage, « Pour une séance, valable
//! 24 h. », l'aide, « Votre session se fermera quand votre ami arrivera. », « Annuler le code » ; les demandes
//! (« {prénom} veut emprunter l'ordinateur. », « Accepter », « Refuser », « Votre session va se fermer. ») ;
//! l'avertissement de confiance jusqu'au premier code. Au bureau, un menu de la barre sous le bouton « Prêter
//! l'ordinateur… » ; à la manette, un panneau par-dessus l'image (B le ferme). Le prêt impossible (Station trop
//! ancienne, prêt coupé dans Admin) : le panneau se réduit à la raison.

use std::time::Duration;

use super::canvas::{Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{format_code, lend_duration_label, App, Layout, LendSteam, Rect, SessionUi, Target, LEND_DURATIONS};
use crate::i18n::tr;

const HEADER: f32 = 46.0;
const PAD: f32 = 20.0;
const QR: f32 = 128.0;
const REQUEST: f32 = 54.0;
const FOOT: f32 = 56.0;

const WARNING: (&str, &str) = (
    "Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance.",
    "Your friend plays in your Windows session. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.",
);
const HELP: (&str, &str) = (
    "Votre ami entre ce code avec « Rejoindre avec un code » dans KyberNow Link, ou ouvre ce lien dans un navigateur.",
    "Your friend enters this code with “Join with a code” in KyberNow Link, or opens this link in a browser.",
);
const CLOSES: (&str, &str) = ("Votre session se fermera quand votre ami arrivera.", "Your session will close when your friend arrives.");
const ONE_SESSION: (&str, &str) = ("Pour une séance, valable 24 h.", "For one session, valid for 24 h.");

/// « Prêter {nom} », “Lend {name}”.
fn title(app: &App, s: &SessionUi) -> String {
    let name = app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name);
    format!("{} {name}", tr("Prêter", "Lend"))
}

/// Pourquoi le prêt est impossible (Station trop ancienne, prêt coupé), ou le refus d'Admin sans vue du prêt.
fn closed_text(app: &App, s: &SessionUi) -> Option<(String, bool)> {
    if let Some(t) = app.lend_avail(s).and_then(|a| a.text()) {
        return Some((t.to_string(), false));
    }
    // Admin dit le prêt coupé dans sa vue : la même raison.
    if s.lend.as_ref().is_some_and(|v| !v.enabled) {
        return Some((crate::app::LendAvail::Off.text().unwrap_or_default().to_string(), false));
    }
    None
}

fn lines(cv: &mut Canvas, text: &str, size: f32, w: f32) -> f32 {
    cv.line_count(Text::new(Font::Regular, size, FAINT), text, w) as f32
}

/// La hauteur du panneau, au même calcul que `card`.
fn height(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, u: f32) -> f32 {
    let inner = w - 2.0 * PAD * u;
    if let Some((t, _)) = closed_text(app, s) {
        return (HEADER + 20.0) * u + lines(cv, &t, 14.0 * u, inner) * 20.0 * u + 12.0 * u;
    }
    let v = s.lend.as_ref();
    let mut h = HEADER * u;
    match v.and_then(|v| v.code.as_ref()) {
        None => {
            if !app.store.app.lend_warned {
                h += lines(cv, tr(WARNING.0, WARNING.1), 12.5 * u, inner) * 17.0 * u + 14.0 * u;
            }
            h += 2.0 * (22.0 * u + 36.0 * u + 14.0 * u) + 48.0 * u + 12.0 * u;
        }
        Some(_) => {
            h += QR * u + 14.0 * u + 18.0 * u + 18.0 * u;
            h += lines(cv, tr(HELP.0, HELP.1), 12.5 * u, inner) * 17.0 * u;
            h += lines(cv, tr(CLOSES.0, CLOSES.1), 12.5 * u, inner) * 17.0 * u + 12.0 * u;
            h += FOOT * u;
        }
    }
    let requests = v.map_or(0, |v| v.requests.len());
    if requests > 0 {
        h += 26.0 * u + requests as f32 * REQUEST * u + 17.0 * u + 8.0 * u;
    }
    if let Some(e) = &s.lend_error {
        h += lines(cv, e, 12.5 * u, inner) * 17.0 * u + 8.0 * u;
    }
    h + 8.0 * u
}

/// Au bureau : le menu de la barre, sous le bouton « Prêter l'ordinateur… » (centré sur `anchor`).
pub fn dropdown(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, anchor: f32, y: f32) {
    let w = 520.0f32.min(cv.w - 16.0);
    let x = (anchor - w / 2.0).clamp(8.0, (cv.w - w - 8.0).max(8.0));
    let full = height(cv, app, s, w, 1.0);
    let u = ((cv.h - y - 8.0) / full).clamp(0.66, 1.0);
    let h = height(cv, app, s, w, u);
    cv.rrect(x, y, w, h, 12.0, Rgba(9, 13, 18, 247));
    cv.rrect_stroke(x, y, w, h, 12.0, 1.0, LINE_2);
    card(cv, f, app, s, Rect::new(x, y, w, h), u);
}

/// À la manette : le panneau au centre de l'image assombrie, et la légende des boutons.
pub fn overlay(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let u0 = unit(app.layout) * if app.layout == Layout::Handheld { 1.15 } else { 1.0 };
    let (cw, ch) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, cw, ch, Rgba(5, 7, 10, 184));
    let footer = 56.0 * unit(app.layout);
    let w = (600.0 * u0).min(cw - 80.0);
    let avail = ch - footer - 40.0;
    let mut u = u0;
    for _ in 0..2 {
        let full = height(cv, app, s, w, u);
        if full > avail {
            u = (u * avail / full).max(0.55);
        }
    }
    let h = height(cv, app, s, w, u).min(avail);
    let (x, y) = ((cw - w) / 2.0, ((ch - footer - h) / 2.0).max(20.0));
    cv.rrect(x, y, w, h, 14.0 * u, Rgba(9, 13, 18, 247));
    cv.rrect_stroke(x, y, w, h, 14.0 * u, 1.0, LINE_2);
    card(cv, f, app, s, Rect::new(x, y, w, h), u);
    cv.rect(0.0, ch - footer, cw, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, ch - footer, cw, 1.0, LINE);
    let gy = ch - footer / 2.0;
    let g = 26.0 * unit(app.layout);
    let x0 = if app.layout == Layout::Tv { 96.0 } else { 48.0 };
    let mut lx = x0;
    if closed_text(app, s).is_none() {
        lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * unit(app.layout);
    }
    legend(cv, lx, gy, g, &["B"], tr("Retour", "Back"));
}

/// Le cercle d'attente d'une demande à Admin.
fn wait_ring(cv: &mut Canvas, app: &App, cx: f32, cy: f32, radius: f32, stroke: f32) {
    let t = app.screen_since.elapsed().as_secs_f32();
    let a0 = if app.reduce_motion { 0.0 } else { t * 5.0 };
    let pts: Vec<(f32, f32)> = (0..=14)
        .map(|i| {
            let a = a0 + i as f32 * 0.32;
            (cx + radius * a.cos(), cy + radius * a.sin())
        })
        .collect();
    cv.polyline(&pts, stroke, SPECTRE);
}

/// Le contenu du panneau, dans `r`.
fn card(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, r: Rect, u: f32) {
    let desktop = app.layout == Layout::Desktop;
    let pad = PAD * u;
    let (x, mut y) = (r.x + pad, r.y);
    let inner = r.w - 2.0 * pad;
    let mut right = r.x + r.w - pad;
    if desktop {
        let b = Rect::new(right - 28.0, y + 9.0, 28.0, 28.0);
        let (focused, hover) = f.state(Target::LendClose, b);
        if focused {
            widgets::ring(cv, b, 7.0);
        }
        if hover {
            cv.rrect(b.x, b.y, b.w, b.h, 7.0, Rgba(227, 237, 240, 16));
        }
        cv.icon(&Icon::Close, b.x + 7.0, b.y + 7.0, 14.0, 1.8, MUTED);
        right -= 36.0;
    }
    if s.lend_busy {
        wait_ring(cv, app, right - 8.0 * u, y + HEADER * u / 2.0, 7.0 * u, 1.8 * u);
        right -= 24.0 * u;
    }
    cv.text_fit(x, y + 29.0 * u, Text::new(Font::Semibold, 17.0 * u, FROST), &title(app, s), (right - x).max(40.0));
    y += HEADER * u;

    if let Some((text, _)) = closed_text(app, s) {
        // La manette n'a rien d'autre à choisir : « Retour » (B) ferme le panneau ; au bureau, la croix.
        if !desktop {
            f.state(Target::LendClose, Rect::new(r.x, r.y, 1.0, 1.0));
        }
        cv.paragraph(x, y + 8.0 * u, Text::new(Font::Regular, 14.0 * u, FROST), &text, inner, 20.0 * u);
        return;
    }

    let v = s.lend.as_ref();
    match v.and_then(|v| v.code.as_ref()) {
        None => {
            // L'avertissement de confiance, jusqu'au premier code créé.
            if !app.store.app.lend_warned {
                y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, WARN), tr(WARNING.0, WARNING.1), inner, 17.0 * u) + 14.0 * u;
            }
            eyebrow(cv, x, y + 12.0 * u, 10.5 * u, FAINT, tr("Durée", "Duration"));
            y += 22.0 * u;
            let items: Vec<(Target, String, bool, bool)> = LEND_DURATIONS.iter().enumerate().map(|(i, d)| (Target::LendDuration(i), lend_duration_label(*d), s.lend_duration == i, false)).collect();
            widgets::segments(cv, f, x, y, 36.0 * u, 12.5 * u, &items);
            y += 36.0 * u + 14.0 * u;
            eyebrow(cv, x, y + 12.0 * u, 10.5 * u, FAINT, tr("Compte Steam", "Steam account"));
            y += 22.0 * u;
            let items: Vec<(Target, String, bool, bool)> = LendSteam::ALL.iter().map(|st| (Target::LendSteamPick(*st), st.label().to_string(), s.lend_steam == *st, false)).collect();
            widgets::segments(cv, f, x, y, 36.0 * u, 12.5 * u, &items);
            y += 36.0 * u + 14.0 * u;
            widgets::primary_state(cv, f, Target::LendCreate, x, y, 44.0 * u, tr("Créer un code", "Create a code"), false, !s.lend_busy);
            y += 48.0 * u + 12.0 * u;
        }
        Some(c) => {
            // Le code, « Copier le lien » (bureau), le QR code du lien des apps à droite ; dessous l'adresse du client web.
            let qr = QR * u;
            let lw = inner - qr - 22.0 * u;
            eyebrow(cv, x, y + 12.0 * u, 10.5 * u, FAINT, tr("Code", "Code"));
            cv.text(x - 1.0, y + 50.0 * u, Text::new(Font::MonoMedium, 36.0 * u, CORE), &format_code(&c.code));
            if desktop {
                let bw = widgets::ghost(cv, f, Target::LendCopy, x, y + 62.0 * u, 28.0 * u, 12.5 * u, None, tr("Copier le lien", "Copy link"), MUTED);
                if let Some((at, ok)) = s.lend_copied
                    && at.elapsed() < Duration::from_secs(4)
                {
                    if ok {
                        cv.icon(&Icon::Check, x + bw + 10.0 * u, y + 66.0 * u, 18.0 * u, 2.0, OK);
                        cv.text(x + bw + 34.0 * u, y + 81.0 * u, Text::new(Font::Regular, 12.5 * u, OK), tr("Lien copié.", "Link copied."));
                    } else {
                        cv.text_fit(x, y + 112.0 * u, Text::new(Font::Regular, 11.5 * u, WARN), tr("Copie impossible : sélectionnez le lien à la main.", "Could not copy: select the link by hand."), lw);
                    }
                }
            }
            // La durée et le compte Steam du code, sous le code.
            let meta = format!("{} · {}", lend_duration_label(c.duration), c.steam.label());
            cv.text_fit(x, y + if desktop { 112.0 } else { 80.0 } * u, Text::new(Font::Regular, 12.0 * u, FAINT), &meta, lw);
            widgets::qr_card(cv, &c.link, r.x + r.w - pad - qr, y + 4.0 * u, qr - 8.0 * u);
            y += qr + 14.0 * u;
            cv.text_fit(x, y + 4.0 * u, Text::new(Font::Mono, 11.0 * u, FAINT), &c.web, inner);
            y += 18.0 * u;
            cv.text_fit(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, MUTED), tr(ONE_SESSION.0, ONE_SESSION.1), inner);
            y += 18.0 * u;
            y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, FAINT), tr(HELP.0, HELP.1), inner, 17.0 * u);
            y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, FROST), tr(CLOSES.0, CLOSES.1), inner, 17.0 * u) + 12.0 * u;
        }
    }

    // Les demandes (ordinateur utilisé) : « {prénom} veut emprunter l'ordinateur. », son app, « Refuser », « Accepter ».
    if let Some(v) = v.filter(|v| !v.requests.is_empty()) {
        cv.rect(x, y, inner, 1.0, LINE);
        eyebrow(cv, x, y + 18.0 * u, 10.5 * u, FAINT, tr("Demandes", "Requests"));
        y += 26.0 * u;
        for (i, rq) in v.requests.iter().enumerate() {
            let (decline, accept) = (tr("Refuser", "Decline"), tr("Accepter", "Accept"));
            let size = 12.0 * u;
            let dw = cv.text_width(Text::new(Font::Regular, size, MUTED), decline) + 2.0 * size;
            let aw = cv.text_width(Text::new(Font::Regular, size, FROST), accept) + 2.0 * size;
            let accept_x = x + inner - aw;
            let decline_x = accept_x - 8.0 * u - dw;
            widgets::ghost(cv, f, Target::LendRefuse(i), decline_x, y + 4.0 * u, 28.0 * u, size, None, decline, BAD_TEXT);
            widgets::ghost(cv, f, Target::LendAccept(i), accept_x, y + 4.0 * u, 28.0 * u, size, None, accept, OK);
            cv.text_fit(x, y + 22.0 * u, Text::new(Font::Regular, 13.5 * u, FROST), &rq.line(), (decline_x - x - 12.0 * u).max(60.0));
            cv.text_fit(x, y + 40.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT), &rq.meta(), inner);
            y += REQUEST * u;
        }
        // Accepter depuis sa session la ferme.
        cv.text_fit(x, y + 12.0 * u, Text::new(Font::Regular, 12.0 * u, MUTED), tr("Votre session va se fermer.", "Your session will close."), inner);
        y += 17.0 * u + 8.0 * u;
    }
    if let Some(e) = &s.lend_error {
        y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, WARN), e, inner, 17.0 * u) + 8.0 * u;
    }
    // « Annuler le code », en bas, quand un code est en cours.
    if v.is_some_and(|v| v.code.is_some()) {
        let label = tr("Annuler le code", "Cancel code");
        widgets::ghost(cv, f, Target::LendCancelCode, x, y + 12.0 * u, 32.0 * u, 13.0 * u, None, label, BAD_TEXT);
    }
}

/// Le sous-titre de la tuile « Prêter l'ordinateur… » du menu, et si elle est allumée : le code en cours
/// (« 123 456 »), « 1 demande » quand une demande attend ; sinon « Code et QR code ».
pub fn tile(s: &SessionUi) -> (String, bool) {
    match &s.lend {
        Some(v) if !v.requests.is_empty() => (crate::app::requests_label(v.requests.len()), true),
        Some(v) if v.code.is_some() => (format_code(&v.code.as_ref().map(|c| c.code.clone()).unwrap_or_default()), true),
        _ => (tr("Code et QR code", "Code and QR code").to_string(), false),
    }
}
