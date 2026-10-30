//! Le panneau de l'hôte de KyberNow Share (docs/link-ui.md, « Partage ») : « Partager {nom} », le code
//! (« 123 456 »), « Copier le lien » (l'adresse du client web, au bureau seulement : ailleurs il n'y a pas de
//! presse-papiers utile), le QR code du lien des apps, l'aide, les invités (leur app, depuis quand, leur
//! rôle, ce que l'hôte peut en faire, « Retirer »), « Nouveau code », « Arrêter le partage ». Au bureau, un
//! menu de la barre sous le bouton « Partager » ; à la manette, un panneau par-dessus l'image (B le ferme).

use std::time::Duration;

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{App, Layout, Rect, SessionUi, ShareMove, ShareView, Target, MAX_GUESTS};
use crate::i18n::tr;

/// Ce que le panneau mesure, en points de l'interface (`u` : l'échelle de l'interface).
const HEADER: f32 = 46.0;
const QR: f32 = 128.0;
const PAD: f32 = 20.0;
const ROW: f32 = 58.0;
/// Une demande : « {prénom} demande à rejoindre » avec ses deux boutons, et dessous son app.
const REQUEST: f32 = 54.0;
const FOOT: f32 = 56.0;

/// Le nom de l'ordinateur qui partage.
fn computer<'a>(app: &'a App, s: &'a SessionUi) -> &'a str {
    app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name)
}

/// Ce que dit le panneau tant que le partage n'est pas ouvert : ce qu'Admin a refusé (`true` : une erreur), ou,
/// sinon, pourquoi le partage est impossible (Station trop ancienne, partage coupé) : le panneau est alors réduit
/// à cette raison.
fn closed_text(app: &App, s: &SessionUi) -> Option<(String, bool)> {
    match &s.share_error {
        Some(e) => Some((e.clone(), true)),
        None => app.share_avail(s).and_then(|a| a.text()).map(|t| (t.to_string(), false)),
    }
}

/// La hauteur du panneau : l'en-tête, le code et le QR code, l'aide, les invités, les boutons du bas.
fn height(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, u: f32) -> f32 {
    let Some(v) = &s.share else {
        let msg = closed_text(app, s).map(|(e, _)| cv.line_count(Text::new(Font::Regular, 14.0 * u, WARN), &e, w - 2.0 * PAD * u) as f32 * 20.0 * u + 12.0 * u).unwrap_or(0.0);
        return (HEADER + 20.0) * u + msg.max(36.0 * u);
    };
    let help_lines = cv.line_count(Text::new(Font::Regular, 12.5 * u, FAINT), v.help(), w - 2.0 * PAD * u) as f32;
    // Lien du relais en préparation, ou relais réglé mais injoignable : une ligne sous l'aide.
    let down = if v.relay_wait_text().is_some() || v.relay_down_text().is_some() { 17.0 * u } else { 0.0 };
    let (requests, guests) = (v.requests().len(), v.accepted().len());
    let asking = if requests > 0 { 26.0 * u + requests as f32 * REQUEST * u + 8.0 * u } else { 0.0 };
    let people = 26.0 * u + if guests == 0 { 36.0 * u } else { guests as f32 * ROW * u };
    let err = s.share_error.as_ref().map(|e| cv.line_count(Text::new(Font::Regular, 12.5 * u, WARN), e, w - 2.0 * PAD * u) as f32 * 17.0 * u + 8.0 * u).unwrap_or(0.0);
    HEADER * u + QR * u + 14.0 * u + 18.0 * u + help_lines * 17.0 * u + down + 12.0 * u + asking + people + err + FOOT * u
}

/// Au bureau : le menu de la barre, sous le bouton « Partager » (centré sur `anchor`).
pub fn dropdown(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, anchor: f32, y: f32) {
    let w = 520.0f32.min(cv.w - 16.0);
    let x = (anchor - w / 2.0).clamp(8.0, (cv.w - w - 8.0).max(8.0));
    // Quatre invités dans la fenêtre la plus basse (600 points) : le panneau se resserre un peu plutôt que de sortir.
    let full = height(cv, app, s, w, 1.0);
    let u = ((cv.h - y - 8.0) / full).clamp(0.66, 1.0);
    let h = height(cv, app, s, w, u);
    cv.rrect(x, y, w, h, 12.0, Rgba(9, 13, 18, 247));
    cv.rrect_stroke(x, y, w, h, 12.0, 1.0, LINE_2);
    card(cv, f, app, s, Rect::new(x, y, w, h), u);
}

/// À la manette : le panneau au centre de l'image assombrie, et la légende des boutons.
pub fn overlay(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    // Le Deck, tenu à la main : un peu plus grand que les maquettes du menu.
    let u = unit(app.layout) * if app.layout == Layout::Handheld { 1.15 } else { 1.0 };
    let (cw, ch) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, cw, ch, Rgba(5, 7, 10, 184));
    let footer = 56.0 * unit(app.layout);
    let w = (600.0 * u).min(cw - 80.0);
    // Des demandes en plus des invités : le panneau se resserre pour tenir dans l'écran, boutons compris.
    let avail = ch - footer - 40.0;
    let mut u = u;
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
    // Réduit à une raison ou à l'attente, il n'y a rien à choisir.
    if s.share.is_some() {
        lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * unit(app.layout);
    }
    legend(cv, lx, gy, g, &["B"], tr("Retour", "Back"));
}

/// Le cercle d'attente du panneau (une demande à Admin, le lien du relais en préparation), centré sur (`cx`, `cy`).
/// Celui du panneau du prêt aussi.
pub(super) fn wait_ring(cv: &mut Canvas, app: &App, cx: f32, cy: f32, radius: f32, stroke: f32) {
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

/// Le QR code du lien des apps en (`x`, `y`), de côté `side` ; le lien du relais en préparation, le cercle d'attente à
/// sa place (le partage et le code de prêt).
#[allow(clippy::too_many_arguments)]
pub(super) fn qr_or_wait(cv: &mut Canvas, app: &App, link: &str, preparing: bool, x: f32, y: f32, side: f32, u: f32) {
    if preparing {
        cv.rrect_stroke(x, y, side, side, 8.0, 1.0, LINE_2);
        wait_ring(cv, app, x + side / 2.0, y + side / 2.0, 12.0 * u, 2.2 * u);
    } else {
        widgets::qr_card(cv, link, x, y, side);
    }
}

/// Sous l'aide : « Préparation du lien hors de chez vous… » avec le cercle d'attente, ou « Pas encore joignable hors
/// de chez vous. » (le partage et le code de prêt) ; rend la hauteur dessinée (17 points par ligne).
#[allow(clippy::too_many_arguments)]
pub(super) fn relay_lines(cv: &mut Canvas, app: &App, wait: Option<&str>, down: Option<&str>, x: f32, y: f32, inner: f32, u: f32) -> f32 {
    let mut h = 0.0;
    if let Some(t) = wait {
        wait_ring(cv, app, x + 6.0 * u, y + 11.5 * u, 5.0 * u, 1.6 * u);
        cv.text_fit(x + 18.0 * u, y + 16.0 * u, Text::new(Font::Regular, 12.5 * u, MUTED), t, inner - 18.0 * u);
        h += 17.0 * u;
    }
    if let Some(t) = down {
        cv.text_fit(x, y + h + 16.0 * u, Text::new(Font::Regular, 12.5 * u, WARN), t, inner);
        h += 17.0 * u;
    }
    h
}

/// Le contenu du panneau, dans `r` ; rend la hauteur dessinée (celle de `height`).
fn card(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, r: Rect, u: f32) -> f32 {
    let desktop = app.layout == Layout::Desktop;
    let pad = PAD * u;
    let (x, mut y) = (r.x + pad, r.y);
    let inner = r.w - 2.0 * pad;
    // L'en-tête : « Partager {nom} », une demande en cours, fermer (au bureau).
    let title = format!("{} {}", tr("Partager", "Share"), computer(app, s));
    let mut right = r.x + r.w - pad;
    if desktop {
        let b = Rect::new(right - 28.0, y + 9.0, 28.0, 28.0);
        let (focused, hover) = f.state(Target::ShareClose, b);
        if focused {
            widgets::ring(cv, b, 7.0);
        }
        if hover {
            cv.rrect(b.x, b.y, b.w, b.h, 7.0, Rgba(227, 237, 240, 16));
        }
        cv.icon(&Icon::Close, b.x + 7.0, b.y + 7.0, 14.0, 1.8, MUTED);
        right -= 36.0;
    }
    if s.share_busy {
        wait_ring(cv, app, right - 8.0 * u, y + HEADER * u / 2.0, 7.0 * u, 1.8 * u);
        right -= 24.0 * u;
    }
    cv.text_fit(x, y + 29.0 * u, Text::new(Font::Semibold, 17.0 * u, FROST), &title, (right - x).max(40.0));
    y += HEADER * u;

    let Some(v) = &s.share else {
        // Pas encore ouvert : l'attente (le cercle ci-dessus), ce qu'Admin a refusé, ou pourquoi le partage est
        // impossible.
        if let Some((text, error)) = closed_text(app, s) {
            y += cv.paragraph(x, y + 8.0 * u, Text::new(Font::Regular, 14.0 * u, if error { WARN } else { FROST }), &text, inner, 20.0 * u) + 12.0 * u;
        }
        return y - r.y;
    };

    // Le code, « Copier le lien » (bureau), l'adresse du client web ; le QR code du lien des apps à droite.
    let qr = QR * u;
    let lw = inner - qr - 22.0 * u;
    eyebrow(cv, x, y + 12.0 * u, 10.5 * u, FAINT, tr("Code", "Code"));
    cv.text(x - 1.0, y + 50.0 * u, Text::new(Font::MonoMedium, 36.0 * u, CORE), &crate::app::format_code(&v.code));
    // Le lien du relais en préparation : « Copier le lien » inactif, le cercle d'attente à la place du QR code, et
    // l'adresse du réseau pas encore montrée (`RELAY_LINK_WAIT` au plus).
    let preparing = v.relay_preparing();
    if desktop && preparing {
        widgets::ghost_inactive(cv, x, y + 62.0 * u, 28.0 * u, 12.5 * u, tr("Copier le lien", "Copy link"));
    } else if desktop {
        let bw = widgets::ghost(cv, f, Target::ShareCopy, x, y + 62.0 * u, 28.0 * u, 12.5 * u, None, tr("Copier le lien", "Copy link"), MUTED);
        // Le lien copié, ou l'échec : l'adresse reste dessous, à sélectionner à la main.
        if let Some((at, ok)) = s.share_copied
            && at.elapsed() < Duration::from_secs(4)
            && ok
        {
            cv.icon(&Icon::Check, x + bw + 10.0 * u, y + 66.0 * u, 18.0 * u, 2.0, OK);
            cv.text(x + bw + 34.0 * u, y + 81.0 * u, Text::new(Font::Regular, 12.5 * u, OK), tr("Lien copié.", "Link copied."));
        }
    }
    if let Some((at, ok)) = s.share_copied
        && at.elapsed() < Duration::from_secs(4)
        && !ok
    {
        cv.text_fit(x, y + 112.0 * u, Text::new(Font::Regular, 11.5 * u, WARN), tr("Copie impossible : sélectionnez le lien à la main.", "Could not copy: select the link by hand."), lw);
    }
    qr_or_wait(cv, app, &v.link, preparing, r.x + r.w - pad - qr, y + 4.0 * u, qr - 8.0 * u, u);
    y += qr + 14.0 * u;
    // L'adresse du client web, en clair : à taper dans un navigateur, ou à sélectionner si la copie a échoué.
    if !preparing {
        cv.text_fit(x, y + 4.0 * u, Text::new(Font::Mono, 11.0 * u, FAINT), &v.web, inner);
    }
    y += 18.0 * u;
    y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, FAINT), v.help(), inner, 17.0 * u);
    // Le lien du relais se prépare (le cercle d'attente et la ligne), ou le relais est réglé mais ne répond pas : sous
    // l'aide du même réseau.
    y += relay_lines(cv, app, v.relay_wait_text(), v.relay_down_text(), x, y, inner, u);
    y += 12.0 * u;

    // Les demandes (invités venus par le relais), au-dessus des invités, seulement s'il y en a : « {prénom} demande à
    // rejoindre », dessous « {app} · hors de chez vous », « Accepter » et « Refuser ».
    let requests = v.requests();
    if !requests.is_empty() {
        cv.rect(x, y, inner, 1.0, LINE);
        eyebrow(cv, x, y + 18.0 * u, 10.5 * u, FAINT, tr("Demandes", "Requests"));
        y += 26.0 * u;
        for (i, g) in requests {
            let (decline, accept) = (tr("Refuser", "Decline"), tr("Accepter", "Accept"));
            // Les deux boutons à droite de la ligne, « Accepter » en dernier (le plus proche du bord, celui du pouce).
            let size = 12.0 * u;
            let dw = cv.text_width(Text::new(Font::Regular, size, MUTED), decline) + 2.0 * size;
            let aw = cv.text_width(Text::new(Font::Regular, size, FROST), accept) + 2.0 * size;
            let accept_x = x + inner - aw;
            let decline_x = accept_x - 8.0 * u - dw;
            widgets::ghost(cv, f, Target::ShareRefuse(i), decline_x, y + 4.0 * u, 28.0 * u, size, None, decline, BAD_TEXT);
            // Les quatre places sont prises : « Accepter » est inactif (« Quatre invités au plus. » dit pourquoi).
            if v.full() {
                widgets::ghost_inactive(cv, accept_x, y + 4.0 * u, 28.0 * u, size, accept);
            } else {
                widgets::ghost(cv, f, Target::ShareAccept(i), accept_x, y + 4.0 * u, 28.0 * u, size, None, accept, OK);
            }
            cv.text_fit(x, y + 22.0 * u, Text::new(Font::Regular, 13.5 * u, FROST), &g.request_line(), (decline_x - x - 12.0 * u).max(60.0));
            cv.text_fit(x, y + 40.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT), &g.request_meta(), inner);
            y += REQUEST * u;
        }
        y += 8.0 * u;
    }

    // Les invités (acceptés).
    let accepted = v.accepted();
    cv.rect(x, y, inner, 1.0, LINE);
    eyebrow(cv, x, y + 18.0 * u, 10.5 * u, FAINT, tr("Invités", "Guests"));
    // La limite se dit quand la liste est pleine, comme dans les apps Apple.
    if accepted.len() >= MAX_GUESTS {
        cv.text(x + inner, y + 18.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT).align(Align::Right), tr("Quatre invités au plus.", "Four guests at most."));
    }
    y += 26.0 * u;
    if accepted.is_empty() {
        cv.text(x, y + 20.0 * u, Text::new(Font::Regular, 13.5 * u, MUTED), tr("Personne pour l'instant.", "Nobody yet."));
        y += 36.0 * u;
    }
    let now = crate::model::now_secs() as i64;
    for (i, g) in accepted.into_iter().take(MAX_GUESTS) {
        let meta = g.meta(now);
        let mw = cv.text_width(Text::new(Font::Regular, 11.5 * u, FAINT), &meta);
        cv.text_fit(x, y + 16.0 * u, Text::new(Font::Regular, 13.5 * u, FROST), &g.line(), (inner - mw - 16.0 * u).max(60.0));
        cv.text(x + inner, y + 16.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT).align(Align::Right), &meta);
        // Ses deux droits, chacun donné ou repris sans toucher à l'autre, puis « Retirer ».
        let mut bx = x;
        for m in ShareMove::ALL {
            bx += widgets::ghost(cv, f, m.target(i), bx, y + 24.0 * u, 26.0 * u, 12.0 * u, None, m.label(g.role), MUTED) + 8.0 * u;
        }
        widgets::ghost(cv, f, Target::ShareKick(i), bx, y + 24.0 * u, 26.0 * u, 12.0 * u, None, tr("Retirer", "Remove"), BAD_TEXT);
        y += ROW * u;
    }
    if let Some(e) = &s.share_error {
        y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, WARN), e, inner, 17.0 * u) + 8.0 * u;
    }

    // « Nouveau code » (les invités déjà là restent) et « Arrêter le partage » (tout le monde sort).
    let by = y + 14.0 * u;
    let bw = widgets::ghost(cv, f, Target::ShareNewCode, x, by, 32.0 * u, 13.0 * u, Some(Icon::Refresh), tr("Nouveau code", "New code"), MUTED);
    let _ = bw;
    let stop = tr("Arrêter le partage", "Stop sharing");
    let sw = cv.text_width(Text::new(Font::Regular, 13.0 * u, BAD_TEXT), stop) + 26.0 * u;
    widgets::ghost(cv, f, Target::ShareStop, x + inner - sw, by, 32.0 * u, 13.0 * u, None, stop, BAD_TEXT);
    y += FOOT * u;
    y - r.y
}

/// La pastille de la barre : « 2 invités », à partir d'un invité accepté (jamais « 0 invité » : le bouton « Partager »
/// est allumé tant que le partage est ouvert, la pastille dit seulement qui est là). Une demande n'y compte pas.
pub fn pill_label(s: &SessionUi) -> Option<String> {
    s.share.as_ref().and_then(ShareView::guests_pill)
}

/// La pastille des demandes en attente : « 1 demande », « 2 demandes », seulement quand il y en a.
pub fn requests_pill_label(s: &SessionUi) -> Option<String> {
    s.share.as_ref().and_then(ShareView::requests_pill)
}
