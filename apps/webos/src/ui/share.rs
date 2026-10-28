//! Le panneau de l'hôte de KyberNow Share (docs/link-ui.md, « Partage ») : « Partager {nom} », le code
//! (« 123 456 »), le QR code du lien des apps (à scanner au téléphone), l'adresse du client web en clair,
//! l'aide, les demandes venues par le relais (« Accepter », « Refuser »), les invités (leur app, depuis quand,
//! leur rôle, ce que l'hôte peut en faire, « Retirer »), « Nouveau code », « Arrêter le partage ». Il s'ouvre
//! depuis la tuile « Partager » du menu de session et prend l'image ; Retour le referme. Pas de « Copier le
//! lien » : un téléviseur n'a pas de presse-papiers utile.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, Rect, SessionUi, ShareMove, Target, MAX_GUESTS};
use crate::i18n::tr;

const PAD: f32 = 48.0;

/// Ce que le panneau mesure. Avec des demandes en plus des invités, il se resserre (jamais au point de rogner un
/// bouton : l'hôte doit atteindre chaque « Accepter ») : d'abord des mesures plus serrées, puis, tout à fait en
/// dernier, le code seul (ni QR code, ni adresse, ni aide : l'hôte les a déjà vus).
#[derive(Clone, Copy)]
struct Metrics {
    header: f32,
    /// Le QR code (et la hauteur du bloc du code) ; nul : le code seul.
    qr: f32,
    /// Une ligne d'invité, une ligne de demande, le bas du panneau.
    row: f32,
    request: f32,
    foot: f32,
    help: bool,
}

const LEVELS: [Metrics; 3] = [
    Metrics { header: 92.0, qr: 200.0, row: 88.0, request: 96.0, foot: 96.0, help: true },
    Metrics { header: 84.0, qr: 150.0, row: 80.0, request: 84.0, foot: 84.0, help: true },
    Metrics { header: 84.0, qr: 0.0, row: 80.0, request: 78.0, foot: 84.0, help: false },
];

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

/// La hauteur du bloc du haut : le code, avec le QR code et l'adresse ; le code seul au dernier niveau.
fn top_height(m: &Metrics) -> f32 {
    if m.qr > 0.0 { m.qr + 24.0 } else { 104.0 }
}

/// La hauteur du panneau : l'en-tête, le code et le QR code, l'aide, les demandes, les invités, les boutons du bas.
fn height(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, m: &Metrics) -> f32 {
    let inner = w - 2.0 * PAD;
    let Some(v) = &s.share else {
        let msg = closed_text(app, s).map(|(e, _)| lines(cv, Text::new(Font::Regular, 25.0, DANGER), &e, inner) * 36.0 + 20.0).unwrap_or(0.0);
        return m.header + 24.0 + msg.max(60.0);
    };
    let help_h = if m.help {
        // Relais réglé mais injoignable : une ligne sous l'aide.
        let down = if v.relay_down_text().is_some() { 31.0 } else { 0.0 };
        lines(cv, Text::new(Font::Regular, 22.0, FROST_2), v.help(), inner) * 31.0 + down + 24.0 + 16.0
    } else {
        16.0
    };
    let (requests, guests) = (v.requests().len(), v.accepted().len());
    let asking = if requests > 0 { 52.0 + requests as f32 * m.request + 12.0 } else { 0.0 };
    let people = 52.0 + if guests == 0 { 64.0 } else { guests as f32 * m.row };
    let err = s.share_error.as_ref().map(|e| lines(cv, Text::new(Font::Regular, 22.0, DANGER), e, inner) * 31.0 + 12.0).unwrap_or(0.0);
    m.header + top_height(m) + help_h + asking + people + err + m.foot
}

/// Les mesures du premier niveau où le panneau tient dans l'écran.
fn fit(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, avail: f32) -> Metrics {
    for m in LEVELS {
        if height(cv, app, s, w, &m) <= avail {
            return m;
        }
    }
    LEVELS[LEVELS.len() - 1]
}

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    let w = 1240.0;
    let avail = H - 110.0;
    let m = fit(cv, app, s, w, avail);
    let h = height(cv, app, s, w, &m).min(H - 100.0);
    let (x, y) = ((W - w) / 2.0, ((H - 80.0 - h) / 2.0).max(30.0));
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    card(app, cv, f, s, Rect::new(x, y, w, h), &m);
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

/// Un bouton du panneau inactif (« Accepter » quand les quatre places sont prises) : estompé, rien n'est inscrit au
/// cadre, il ne prend ni le focus ni le clic. Rend sa largeur (celle de `button`).
fn button_inactive(cv: &mut Canvas, x: f32, y: f32, label: &str, size: f32) -> f32 {
    let style = Text::new(Font::Semibold, size, FROST_4);
    let w = cv.text_width(style, label) + size * 1.6;
    let h = size * 2.2;
    cv.rrect(x, y, w, h, 12.0, Rgba(227, 237, 240, 6));
    cv.rrect_stroke(x, y, w, h, 12.0, 1.5, LINE);
    cv.text(x + size * 0.8, y + h / 2.0 + size * 0.36, style, label);
    w
}

/// Le contenu du panneau, dans `r`.
fn card(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, r: Rect, m: &Metrics) {
    let (x, mut y) = (r.x + PAD, r.y);
    let inner = r.w - 2.0 * PAD;
    // L'en-tête : « Partager {nom} », une demande en cours.
    let name = app.store.host(&s.host).map(|h| h.name.as_str()).unwrap_or(&s.name);
    let title = format!("{} {name}", tr("Partager", "Share"));
    let mut right = r.x + r.w - PAD;
    if s.share_busy {
        let t = s.started.elapsed().as_secs_f32();
        let (cx, cy) = (right - 14.0, y + m.header / 2.0 + 4.0);
        cv.circle(cx, cy, 16.0, HALO.alpha(0.2 + 0.3 * (t * 4.0).sin().abs()));
        cv.circle(cx, cy, 7.0, SPECTRE);
        right -= 48.0;
    }
    cv.text_fit(x, y + m.header - 30.0, Text::new(Font::Title, 40.0, FROST), &title, (right - x).max(80.0));
    y += m.header;

    let Some(v) = &s.share else {
        // Pas encore ouvert : l'attente (le cercle ci-dessus), ce qu'Admin a refusé, ou pourquoi le partage est
        // impossible.
        if let Some((text, error)) = closed_text(app, s) {
            cv.paragraph(x, y + 36.0, Text::new(Font::Regular, 25.0, if error { DANGER } else { FROST }), &text, inner, 36.0);
        }
        return;
    };

    // Le code, l'adresse du client web en clair (celle du relais quand il tient le code) ; le QR code du lien des
    // apps à droite (celui du relais, avec `relay` et `sid`, quand il est joint).
    let fs = if m.qr > 0.0 { 96.0 * m.qr / 200.0 } else { 64.0 };
    widgets::label(cv, x, y + 22.0, tr("Code", "Code"));
    cv.text(x - 2.0, y + 38.0 + 76.0 / 96.0 * fs, Text::new(Font::MonoMedium, fs, FROST).tracking(6.0 * fs / 96.0), &crate::app::format_code(&v.code));
    if m.qr > 0.0 {
        let lw = inner - m.qr - 40.0;
        // L'adresse sous le code, au bas du QR code (un peu plus bas quand le bloc est resserré).
        let ay = if m.qr >= 200.0 { m.qr - 32.0 } else { m.qr - 12.0 };
        cv.text_fit(x, y + ay, Text::new(Font::Mono, 20.0, FROST_3), &v.web, lw);
        widgets::qr(cv, &v.link, r.x + r.w - PAD - m.qr, y + 4.0, m.qr);
    }
    y += top_height(m);
    if m.help {
        y += cv.paragraph(x, y + 24.0, Text::new(Font::Regular, 22.0, FROST_2), v.help(), inner, 31.0);
        // Le relais est réglé mais ne répond pas : sous l'aide du même réseau.
        if let Some(t) = v.relay_down_text() {
            cv.text_fit(x, y + 40.0, Text::new(Font::Regular, 22.0, DANGER), t, inner);
            y += 31.0;
        }
        y += 16.0;
    } else {
        y += 16.0;
    }

    // Les demandes (invités venus par le relais), au-dessus des invités, seulement s'il y en a : « {prénom} demande
    // à rejoindre », dessous « {app} · hors de chez vous », « Accepter » et « Refuser ».
    let requests = v.requests();
    if !requests.is_empty() {
        cv.rect(x, y, inner, 1.0, LINE);
        widgets::label(cv, x, y + 34.0, tr("Demandes", "Requests"));
        y += 52.0;
        for (i, g) in requests {
            let (decline, accept) = (tr("Refuser", "Decline"), tr("Accepter", "Accept"));
            let size = 22.0;
            let dw = cv.text_width(Text::new(Font::Semibold, size, DANGER), decline) + size * 1.6;
            let aw = cv.text_width(Text::new(Font::Semibold, size, FROST), accept) + size * 1.6;
            let accept_x = x + inner - aw;
            let decline_x = accept_x - 14.0 - dw;
            button(cv, f, Target::ShareRefuse(i), decline_x, y + 6.0, decline, size, true);
            // Les quatre places sont prises : « Accepter » est inactif (« Quatre invités au plus. » dit pourquoi).
            if v.full() {
                button_inactive(cv, accept_x, y + 6.0, accept, size);
            } else {
                button(cv, f, Target::ShareAccept(i), accept_x, y + 6.0, accept, size, false);
            }
            cv.text_fit(x, y + 34.0, Text::new(Font::Semibold, 27.0, FROST), &g.request_line(), (decline_x - x - 24.0).max(80.0));
            cv.text_fit(x, y + 66.0, Text::new(Font::Regular, 20.0, FROST_3), &g.request_meta(), inner);
            y += m.request;
        }
        y += 12.0;
    }

    // Les invités (acceptés).
    let accepted = v.accepted();
    cv.rect(x, y, inner, 1.0, LINE);
    widgets::label(cv, x, y + 34.0, tr("Invités", "Guests"));
    // La limite se dit quand la liste est pleine, comme dans les apps Apple.
    if accepted.len() >= MAX_GUESTS {
        cv.text(x + inner, y + 34.0, Text::new(Font::Regular, 20.0, FROST_3).align(Align::Right), tr("Quatre invités au plus.", "Four guests at most."));
    }
    y += 52.0;
    if accepted.is_empty() {
        cv.text(x, y + 36.0, Text::new(Font::Regular, 26.0, FROST_2), tr("Personne pour l'instant.", "Nobody yet."));
        y += 64.0;
    }
    let now = crate::model::now_secs() as i64;
    for (i, g) in accepted.into_iter().take(MAX_GUESTS) {
        let meta = g.meta(now);
        let mw = cv.text_width(Text::new(Font::Regular, 20.0, FROST_3), &meta);
        let base = if m.row >= 88.0 { 30.0 } else { 24.0 };
        cv.text_fit(x, y + base, Text::new(Font::Semibold, 27.0, FROST), &g.line(), (inner - mw - 24.0).max(80.0));
        cv.text(x + inner, y + base, Text::new(Font::Regular, 20.0, FROST_3).align(Align::Right), &meta);
        // Ses deux droits, chacun donné ou repris sans toucher à l'autre, puis « Retirer ».
        let mut bx = x;
        let by = y + m.row - 48.0;
        for mv in ShareMove::ALL {
            bx += button(cv, f, mv.target(i), bx, by, mv.label(g.role), 21.0, false) + 12.0;
        }
        button(cv, f, Target::ShareKick(i), bx, by, tr("Retirer", "Remove"), 21.0, true);
        y += m.row;
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
