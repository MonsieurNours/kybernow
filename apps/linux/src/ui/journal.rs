//! Le panneau « Envoyer le journal » par QR code (docs/link-ui.md, « Réglages de l'app › À propos ») : Steam Deck en
//! mode Jeu, salon, KyberNow Link OS. Par-dessus les réglages : le titre, l'étape (préparation, envoi, QR code et sa
//! phrase, ouvert sur le téléphone, expiré, relais injoignable, relais d'avant, trop d'envois), sous le QR code la
//! phrase du chiffrement, et un seul bouton (« Terminé », « Nouveau QR code », « Réessayer »).

use super::canvas::{Canvas, Font, Rgba, Text};
use super::theme::*;
use super::widgets;
use super::{unit, Frame};
use crate::app::{App, Layout, Target};
use crate::journal::{self, JournalStep};

pub fn overlay(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(step) = &app.journal else { return };
    let desktop = app.layout == Layout::Desktop;
    let u = if desktop { 1.0 } else { unit(app.layout) };
    let (w, h) = (cv.w, cv.h);
    // Seul son bouton se choisit.
    f.hits.clear();
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
    let pad = 32.0 * u;
    let cw = (560.0 * u).min(w - 48.0);
    let iw = cw - 2.0 * pad;
    let text = Text::new(Font::Regular, 16.0 * u, MUTED);
    let note = Text::new(Font::Regular, 13.0 * u, FAINT);
    let bh = if desktop { 44.0 } else { 52.0 * u };
    let qr_side = (260.0 * u).min(h - 300.0 * u).max(120.0);
    let qr = matches!(step, JournalStep::Qr { .. });
    // La hauteur d'abord, puis le dessin.
    let text_h = cv.line_count(text, step.text(), iw) as f32 * 23.0 * u;
    let note_h = if qr { cv.line_count(note, journal::encrypted_note(), iw) as f32 * 19.0 * u + 12.0 * u } else { 0.0 };
    let ch = (pad + 34.0 * u + 18.0 * u + text_h + if qr { 20.0 * u + qr_side + 16.0 * u } else { 0.0 } + note_h + 24.0 * u + bh + pad).min(h - 24.0);
    let (x, y) = ((w - cw) / 2.0, ((h - ch) / 2.0).max(12.0));
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, LINE_2);
    let ix = x + pad;
    let mut yy = y + pad + 28.0 * u;
    cv.text_shrink(ix, yy, Text::new(Font::Display, 26.0 * u, FROST).tracking(-0.3 * u), journal::title(), iw);
    yy += 24.0 * u;
    let color = match step {
        JournalStep::RelayDown | JournalStep::OldRelay | JournalStep::TooMany | JournalStep::Expired => WARN,
        JournalStep::Opened => OK,
        _ => MUTED,
    };
    yy += cv.paragraph(ix, yy + 16.0 * u, text.color(color), step.text(), iw, 23.0 * u);
    if let JournalStep::Qr { url } = step {
        yy += 20.0 * u;
        if let Some(side) = widgets::qr_card(cv, url, x + (cw - qr_side) / 2.0, yy, qr_side) {
            yy += side + 16.0 * u;
        }
        yy += cv.paragraph(ix, yy + 12.0 * u, note, journal::encrypted_note(), iw, 19.0 * u);
    }
    // Pendant la préparation et l'envoi, « Terminé » arrête tout.
    let by = (yy + 24.0 * u).min(y + ch - pad - bh);
    widgets::primary(cv, f, Target::JournalButton, ix, by, bh, step.button(), false);
}
