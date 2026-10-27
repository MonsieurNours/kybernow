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
use crate::app::{guests_label, App, Layout, Rect, SessionUi, ShareView, Target, MAX_GUESTS};
use crate::i18n::tr;

/// Ce que le panneau mesure, en points de l'interface (`u` : l'échelle de l'interface).
const HEADER: f32 = 46.0;
const QR: f32 = 128.0;
const PAD: f32 = 20.0;
const ROW: f32 = 58.0;
const FOOT: f32 = 56.0;

fn help() -> &'static str {
    tr(
        "Sur le même réseau : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur.",
        "On the same network: “Join with a code” in KyberNow Link, or this link in a browser.",
    )
}

/// Le nom de l'ordinateur qui partage.
fn computer<'a>(app: &'a App, s: &'a SessionUi) -> &'a str {
    app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name)
}

/// La hauteur du panneau : l'en-tête, le code et le QR code, l'aide, les invités, les boutons du bas.
fn height(cv: &mut Canvas, s: &SessionUi, w: f32, u: f32) -> f32 {
    let Some(v) = &s.share else {
        let msg = s.share_error.as_ref().map(|e| cv.line_count(Text::new(Font::Regular, 14.0 * u, WARN), e, w - 2.0 * PAD * u) as f32 * 20.0 * u + 12.0 * u).unwrap_or(0.0);
        return (HEADER + 20.0) * u + msg.max(36.0 * u);
    };
    let help_lines = cv.line_count(Text::new(Font::Regular, 12.5 * u, FAINT), help(), w - 2.0 * PAD * u) as f32;
    let people = 26.0 * u + if v.guests.is_empty() { 36.0 * u } else { v.guests.len() as f32 * ROW * u };
    let err = s.share_error.as_ref().map(|e| cv.line_count(Text::new(Font::Regular, 12.5 * u, WARN), e, w - 2.0 * PAD * u) as f32 * 17.0 * u + 8.0 * u).unwrap_or(0.0);
    HEADER * u + QR * u + 14.0 * u + 18.0 * u + help_lines * 17.0 * u + 12.0 * u + people + err + FOOT * u
}

/// Au bureau : le menu de la barre, sous le bouton « Partager » (centré sur `anchor`).
pub fn dropdown(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, anchor: f32, y: f32) {
    let w = 520.0f32.min(cv.w - 16.0);
    let x = (anchor - w / 2.0).clamp(8.0, (cv.w - w - 8.0).max(8.0));
    // Quatre invités dans la fenêtre la plus basse (600 points) : le panneau se resserre un peu plutôt que de sortir.
    let full = height(cv, s, w, 1.0);
    let u = ((cv.h - y - 8.0) / full).clamp(0.8, 1.0);
    let h = height(cv, s, w, u);
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
    let h = height(cv, s, w, u).min(ch - footer - 40.0);
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
    lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * unit(app.layout);
    legend(cv, lx, gy, g, &["B"], tr("Retour", "Back"));
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
        let t = app.screen_since.elapsed().as_secs_f32();
        let (cx, cy) = (right - 8.0 * u, y + HEADER * u / 2.0);
        let a0 = if app.reduce_motion { 0.0 } else { t * 5.0 };
        let pts: Vec<(f32, f32)> = (0..=14)
            .map(|i| {
                let a = a0 + i as f32 * 0.32;
                (cx + 7.0 * u * a.cos(), cy + 7.0 * u * a.sin())
            })
            .collect();
        cv.polyline(&pts, 1.8 * u, SPECTRE);
        right -= 24.0 * u;
    }
    cv.text_fit(x, y + 29.0 * u, Text::new(Font::Semibold, 17.0 * u, FROST), &title, (right - x).max(40.0));
    y += HEADER * u;

    let Some(v) = &s.share else {
        // Pas encore ouvert : l'attente (le cercle ci-dessus), ou ce qu'Admin a refusé.
        if let Some(e) = &s.share_error {
            y += cv.paragraph(x, y + 8.0 * u, Text::new(Font::Regular, 14.0 * u, WARN), e, inner, 20.0 * u) + 12.0 * u;
        }
        return y - r.y;
    };

    // Le code, « Copier le lien » (bureau), l'adresse du client web ; le QR code du lien des apps à droite.
    let qr = QR * u;
    let lw = inner - qr - 22.0 * u;
    eyebrow(cv, x, y + 12.0 * u, 10.5 * u, FAINT, tr("Code", "Code"));
    cv.text(x - 1.0, y + 50.0 * u, Text::new(Font::MonoMedium, 36.0 * u, CORE), &crate::app::format_code(&v.code));
    if desktop {
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
    widgets::qr_card(cv, &v.link, r.x + r.w - pad - qr, y + 4.0 * u, qr - 8.0 * u);
    y += qr + 14.0 * u;
    // L'adresse du client web, en clair : à taper dans un navigateur, ou à sélectionner si la copie a échoué.
    cv.text_fit(x, y + 4.0 * u, Text::new(Font::Mono, 11.0 * u, FAINT), &v.web, inner);
    y += 18.0 * u;
    y += cv.paragraph(x, y + 4.0 * u, Text::new(Font::Regular, 12.5 * u, FAINT), help(), inner, 17.0 * u) + 12.0 * u;

    // Les invités.
    cv.rect(x, y, inner, 1.0, LINE);
    eyebrow(cv, x, y + 18.0 * u, 10.5 * u, FAINT, tr("Invités", "Guests"));
    cv.text(x + inner, y + 18.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT).align(Align::Right), tr("Quatre invités au plus.", "Four guests at most."));
    y += 26.0 * u;
    if v.guests.is_empty() {
        cv.text(x, y + 20.0 * u, Text::new(Font::Regular, 13.5 * u, MUTED), tr("Personne pour l'instant.", "Nobody yet."));
        y += 36.0 * u;
    }
    let now = crate::model::now_secs() as i64;
    for (i, g) in v.guests.iter().take(MAX_GUESTS).enumerate() {
        let meta = g.meta(now);
        let mw = cv.text_width(Text::new(Font::Regular, 11.5 * u, FAINT), &meta);
        cv.text_fit(x, y + 16.0 * u, Text::new(Font::Regular, 13.5 * u, FROST), &g.line(), (inner - mw - 16.0 * u).max(60.0));
        cv.text(x + inner, y + 16.0 * u, Text::new(Font::Regular, 11.5 * u, FAINT).align(Align::Right), &meta);
        // Ce que l'hôte peut en faire, puis « Retirer ».
        let mut bx = x;
        for m in g.role.moves() {
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

/// La pastille de la barre : « 2 invités » (le partage est ouvert).
pub fn pill_label(s: &SessionUi) -> Option<String> {
    s.share.as_ref().map(|v: &ShareView| guests_label(v.guests.len()))
}
