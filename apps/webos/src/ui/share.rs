//! Le panneau de l'hôte de KyberNow Share (docs/link-ui.md, « Partage ») : « Partager {nom} », le code
//! (« 123 456 »), le QR code du lien des apps (à scanner au téléphone), l'adresse du client web en clair,
//! l'aide, les invités (leur app, depuis quand, leur rôle, ce que l'hôte peut en faire, « Retirer »),
//! « Nouveau code », « Arrêter le partage ». Il s'ouvre depuis la tuile « Partager » du menu de session et
//! prend l'image ; Retour le referme. Pas de « Copier le lien » : un téléviseur n'a pas de presse-papiers utile.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, Rect, SessionUi, Target, MAX_GUESTS};
use crate::i18n::tr;

const HEADER: f32 = 92.0;
const QR: f32 = 200.0;
const PAD: f32 = 48.0;
const ROW: f32 = 88.0;
const FOOT: f32 = 96.0;

fn help() -> &'static str {
    tr(
        "Sur le même réseau : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur.",
        "On the same network: “Join with a code” in KyberNow Link, or this link in a browser.",
    )
}

/// Les lignes que prend un texte (au plus un mot de marge).
fn lines(cv: &mut Canvas, style: Text, s: &str, w: f32) -> f32 {
    ((cv.text_width(style, s) / (w * 0.92)).ceil()).max(1.0)
}

/// Ce que dit le panneau tant que le partage n'est pas ouvert : ce qu'Admin a refusé (`true` : une erreur), ou,
/// sinon, pourquoi le partage est impossible (Station trop ancienne, partage coupé) : le panneau est alors réduit
/// à cette raison.
fn closed_text(app: &App, s: &SessionUi) -> Option<(String, bool)> {
    match &s.share_error {
        Some(e) => Some((e.clone(), true)),
        None => app.share_avail().and_then(|a| a.text()).map(|t| (t.to_string(), false)),
    }
}

/// La hauteur du panneau : l'en-tête, le code et le QR code, l'aide, les invités, les boutons du bas.
fn height(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32) -> f32 {
    let inner = w - 2.0 * PAD;
    let Some(v) = &s.share else {
        let msg = closed_text(app, s).map(|(e, _)| lines(cv, Text::new(Font::Regular, 25.0, DANGER), &e, inner) * 36.0 + 20.0).unwrap_or(0.0);
        return HEADER + 24.0 + msg.max(60.0);
    };
    let help_h = lines(cv, Text::new(Font::Regular, 22.0, FROST_2), help(), inner) * 31.0;
    let people = 52.0 + if v.guests.is_empty() { 64.0 } else { v.guests.len() as f32 * ROW };
    let err = s.share_error.as_ref().map(|e| lines(cv, Text::new(Font::Regular, 22.0, DANGER), e, inner) * 31.0 + 12.0).unwrap_or(0.0);
    HEADER + QR + 24.0 + 24.0 + help_h + 16.0 + people + err + FOOT
}

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    let w = 1240.0;
    let h = height(cv, app, s, w).min(H - 140.0);
    let (x, y) = ((W - w) / 2.0, ((H - 80.0 - h) / 2.0).max(30.0));
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    card(app, cv, f, s, Rect::new(x, y, w, h));
    let hy = H - 48.0;
    let mut hx = 96.0;
    // Réduit à une raison ou à l'attente, il n'y a rien à choisir.
    if s.share.is_some() {
        hx += widgets::hint(cv, hx, hy, Key::Ok, tr("Choisir", "Select")) + 40.0;
    }
    widgets::hint(cv, hx, hy, Key::Back, tr("Retour", "Back"));
}

/// Un bouton du panneau (contour) : son nom, sa cible ; `danger` : en rouge. Rend sa largeur.
#[allow(clippy::too_many_arguments)]
fn button(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, label: &str, size: f32, danger: bool) -> f32 {
    let style = Text::new(Font::Semibold, size, if danger { DANGER } else { FROST });
    let w = cv.text_width(style, label) + size * 1.6;
    let h = size * 2.2;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    if focused {
        cv.neon(x, y, w, h, 12.0, 0.9);
        cv.rrect(x, y, w, h, 12.0, CORE);
        cv.text(x + size * 0.8, y + h / 2.0 + size * 0.36, style.color(INK), label);
    } else {
        cv.rrect(x, y, w, h, 12.0, if danger { DANGER_BG } else { Rgba(227, 237, 240, 15) });
        cv.rrect_stroke(x, y, w, h, 12.0, 1.5, if danger { DANGER_LINE } else { BORDER });
        cv.text(x + size * 0.8, y + h / 2.0 + size * 0.36, style, label);
    }
    w
}

/// Le contenu du panneau, dans `r`.
fn card(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, r: Rect) {
    let (x, mut y) = (r.x + PAD, r.y);
    let inner = r.w - 2.0 * PAD;
    // L'en-tête : « Partager {nom} », une demande en cours.
    let name = app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name);
    let title = format!("{} {name}", tr("Partager", "Share"));
    let mut right = r.x + r.w - PAD;
    if s.share_busy {
        let t = s.started.elapsed().as_secs_f32();
        let (cx, cy) = (right - 14.0, y + HEADER / 2.0 + 4.0);
        cv.circle(cx, cy, 16.0, HALO.alpha(0.2 + 0.3 * (t * 4.0).sin().abs()));
        cv.circle(cx, cy, 7.0, SPECTRE);
        right -= 48.0;
    }
    cv.text_fit(x, y + 62.0, Text::new(Font::Title, 40.0, FROST), &title, (right - x).max(80.0));
    y += HEADER;

    let Some(v) = &s.share else {
        // Pas encore ouvert : l'attente (le cercle ci-dessus), ce qu'Admin a refusé, ou pourquoi le partage est
        // impossible.
        if let Some((text, error)) = closed_text(app, s) {
            cv.paragraph(x, y + 36.0, Text::new(Font::Regular, 25.0, if error { DANGER } else { FROST }), &text, inner, 36.0);
        }
        return;
    };

    // Le code, l'adresse du client web en clair ; le QR code du lien des apps à droite.
    let lw = inner - QR - 40.0;
    widgets::label(cv, x, y + 22.0, tr("Code", "Code"));
    cv.text(x - 2.0, y + 114.0, Text::new(Font::MonoMedium, 96.0, FROST).tracking(6.0), &crate::app::format_code(&v.code));
    cv.text_fit(x, y + 168.0, Text::new(Font::Mono, 20.0, FROST_3), &v.web, lw);
    widgets::qr(cv, &v.link, r.x + r.w - PAD - QR, y + 4.0, QR);
    y += QR + 24.0;
    y += cv.paragraph(x, y + 24.0, Text::new(Font::Regular, 22.0, FROST_2), help(), inner, 31.0) + 16.0;

    // Les invités.
    cv.rect(x, y, inner, 1.0, LINE);
    widgets::label(cv, x, y + 34.0, tr("Invités", "Guests"));
    // La limite se dit quand la liste est pleine, comme dans les apps Apple.
    if v.guests.len() >= MAX_GUESTS {
        cv.text(x + inner, y + 34.0, Text::new(Font::Regular, 20.0, FROST_3).align(Align::Right), tr("Quatre invités au plus.", "Four guests at most."));
    }
    y += 52.0;
    if v.guests.is_empty() {
        cv.text(x, y + 36.0, Text::new(Font::Regular, 26.0, FROST_2), tr("Personne pour l'instant.", "Nobody yet."));
        y += 64.0;
    }
    let now = crate::model::now_secs() as i64;
    for (i, g) in v.guests.iter().take(MAX_GUESTS).enumerate() {
        let meta = g.meta(now);
        let mw = cv.text_width(Text::new(Font::Regular, 20.0, FROST_3), &meta);
        cv.text_fit(x, y + 30.0, Text::new(Font::Semibold, 27.0, FROST), &g.line(), (inner - mw - 24.0).max(80.0));
        cv.text(x + inner, y + 30.0, Text::new(Font::Regular, 20.0, FROST_3).align(Align::Right), &meta);
        // Ce que l'hôte peut en faire, puis « Retirer ».
        let mut bx = x;
        for m in g.role.moves() {
            bx += button(cv, f, m.target(i), bx, y + 40.0, m.label(g.role), 21.0, false) + 12.0;
        }
        button(cv, f, Target::ShareKick(i), bx, y + 40.0, tr("Retirer", "Remove"), 21.0, true);
        y += ROW;
    }
    if let Some(e) = &s.share_error {
        y += cv.paragraph(x, y + 6.0, Text::new(Font::Regular, 22.0, DANGER), e, inner, 31.0) + 12.0;
    }

    // « Nouveau code » (les invités déjà là restent) et « Arrêter le partage » (tout le monde sort).
    let by = y + 20.0;
    button(cv, f, Target::ShareNewCode, x, by, tr("Nouveau code", "New code"), 24.0, false);
    let stop = tr("Arrêter le partage", "Stop sharing");
    let sw = cv.text_width(Text::new(Font::Semibold, 24.0, DANGER), stop) + 24.0 * 1.6;
    button(cv, f, Target::ShareStop, x + inner - sw, by, stop, 24.0, true);
}
