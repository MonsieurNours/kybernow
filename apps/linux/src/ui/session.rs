//! La session (docs/link-ui.md, « Connexion ») : la connexion montre ses
//! trois étapes (le fond de l'ordinateur, le néon dans la couture à 45°, la
//! capsule des étapes, Annuler), puis l'image seule dès la première image.
//! Par-dessus l'image, à la demande : la barre du bureau (en haut, à
//! l'approche du pointeur), le menu à la manette (santé du flux, tuiles,
//! légende), les raccourcis, les statistiques ; les capsules de ce qui change
//! (manette branchée, reconnexion) ; la fin sur erreur.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::home;
use super::icons::Icon;
use super::theme::*;
use super::lend;
use super::quality;
use super::share;
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{share_tile, shortcuts, App, BarMenu, CameraState, Layout, MenuItem, Online, Phase, Rect, Screen, SessionUi, ShareAvail, Target};
use crate::i18n::{decimal, tr};
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame, view: super::View) {
    let Some(s) = super::view_session(app, view) else {
        cv.clear(INK);
        return;
    };
    match &s.phase {
        // Plusieurs écrans : l'écran principal porte la connexion, les autres le même fond sans texte.
        Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages => {
            if view.main {
                passage(app, cv, f, s)
            } else {
                background(cv, unit(app.layout), view.missing)
            }
        }
        Phase::Ended { error } if view.main => ended(app, cv, f, s, error.as_deref()),
        Phase::Ending if view.main => ending(app, cv, s),
        Phase::Ended { .. } | Phase::Ending => cv.clear(INK),
        // Une fenêtre qui n'a pas encore son image garde le fond de la connexion.
        _ if !view.main && !view.has_frame => background(cv, unit(app.layout), view.missing),
        _ => {
            cv.clear_transparent();
            if s.keys {
                keys(app, cv, f, s);
            } else if s.menu {
                menu(app, cv, f, s);
            } else if s.lend_panel {
                lend::overlay(app, cv, f, s);
            } else if s.watch_panel {
                quality::screen_overlay(app, cv, f, s);
            } else if s.share_panel {
                // Un invité n'a pas de panneau d'hôte : le sien est la « Qualité de l'image ».
                if s.guest.is_some() {
                    quality::overlay(app, cv, f, s);
                } else {
                    share::overlay(app, cv, f, s);
                }
            } else {
                if s.show_stats && view.main && s.console.is_none() {
                    stats(app, cv, f, s);
                }
                if s.show_stats && view.main && s.console.is_some() {
                    super::ps::stats(app, cv, s);
                }
                if view.main {
                    super::ps::session_overlay(app, cv, s);
                }
                if s.bar && view.bar && app.layout == Layout::Desktop {
                    if s.console.is_some() {
                        super::ps::bar(app, cv, f, s);
                    } else {
                        bar(app, cv, f, s);
                    }
                } else if bar_hint(app, s) {
                    hint(cv);
                }
                if !view.main {
                    return;
                }
                // Le voyant de la caméra (docs/link-ui.md) : barre fermée, tant qu'elle part, une petite
                // capsule « ● Caméra » en haut à gauche.
                if !s.bar && s.camera == CameraState::Sending {
                    let style = Text::new(Font::Semibold, 11.0, Rgba(255, 201, 196, 255));
                    let label = tr("● Caméra", "● Camera");
                    let w = cv.text_width(style, label) + 16.0;
                    cv.rrect(10.0, 10.0, w, 20.0, 10.0, Rgba(0, 0, 0, 140));
                    cv.text(18.0, 24.0, style, label);
                }
                // PyroWave en Wi-Fi ou par le relais : le bandeau du début de session (8 s, ou la croix).
                // Au bureau, la capsule passe sous le bandeau (« Définition ramenée » peut arriver en même temps).
                let mut drop = 0.0;
                if s.pw_banner_visible() && s.console.is_none() {
                    pyrowave_banner(app, cv, f, s);
                    if app.layout == Layout::Desktop {
                        drop = PW_BANNER_H + 10.0;
                    }
                }
                // Profil Auto sur un autre réseau que celui du test : le bandeau discret du début (8 s, ou la croix).
                if s.auto_banner_visible() && s.console.is_none() {
                    auto_banner(app, cv, f);
                    if app.layout == Layout::Desktop {
                        drop = super::autotest::BANNER_H + 10.0;
                    }
                }
                if let Phase::Reconnecting { attempt, max } = s.phase {
                    capsule(app, cv, &format!("{} ({attempt}/{max})", tr("Reconnexion…", "Reconnecting…")), widgets::Mark::Dot(WARN), drop);
                } else if let Some((n, t)) = &s.notice
                    && t.elapsed().as_secs_f32() < 4.0
                {
                    // Les manettes indisponibles : avec l'icône de manette.
                    if n == crate::app::pads_unavailable_notice() {
                        capsule(app, cv, n, widgets::Mark::Icon(Icon::Gamepad), drop);
                    } else {
                        capsule(app, cv, n, widgets::Mark::Dot(SPECTRE), drop);
                    }
                }
            }
        }
    }
}

// MARK: - Passage

/// Durée de la fermeture, celle de l'app Mac (apps/macos/KyberNow/Passage.swift).
const CLOSE: f32 = 0.6;


/// La couture à 45° : son point haut (elle descend vers la gauche) et les deux moitiés.
fn halves(w: f32, h: f32) -> ([(f32, f32); 4], [(f32, f32); 4], f32) {
    let c = (w + h) / 2.0;
    ([(0.0, 0.0), (c, 0.0), (c - h, h), (0.0, h)], [(c, 0.0), (w, 0.0), (w, h), (c - h, h)], c)
}

/// Le tube néon dans la couture : `scale` (étirement depuis le centre),
/// `alpha` (grésillement, respiration), bouts en fondu.
fn tube(cv: &mut Canvas, u: f32, scale: f32, alpha: f32) {
    if alpha <= 0.01 || scale <= 0.0 {
        return;
    }
    let (w, h) = (cv.w, cv.h);
    let c = (w + h) / 2.0;
    let (cx, cy) = (c - h / 2.0, h / 2.0);
    let half = h / 2.0 * scale;
    let n = 28;
    for (width, color) in [(16.0 * u, FLAT.alpha(0.55)), (6.0 * u, SPECTRE), (3.0 * u, CORE)] {
        for i in 0..n {
            let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let mid = (t0 + t1) / 2.0;
            let fade = (mid / 0.14).min((1.0 - mid) / 0.14).clamp(0.0, 1.0);
            let p = |t: f32| (cx + half * (1.0 - 2.0 * t), cy - half * (1.0 - 2.0 * t));
            let (a, b) = (p(t0), p(t1));
            cv.line(a.0, a.1, b.0, b.1, width, color.alpha(alpha * fade));
        }
    }
}

/// La lueur Spectre le long de la couture.
fn band(cv: &mut Canvas, u: f32, alpha: f32) {
    if alpha <= 0.01 {
        return;
    }
    let (w, h) = (cv.w, cv.h);
    let c = (w + h) / 2.0;
    for i in 0..5 {
        let t = (i as f32 + 0.5) / 5.0;
        cv.glow(c - h * t, h * t, 170.0 * u, 170.0 * u, FLAT.alpha(0.42 * alpha));
    }
}

/// Niveau du tube pendant l'attente : grésillement sur une seconde, puis respiration.
fn ignite(t: f32, reduced: bool) -> (f32, f32) {
    if reduced {
        return (1.0, 1.0);
    }
    const KEYS: [(f32, f32); 8] = [(0.0, 0.0), (0.06, 0.9), (0.1, 0.1), (0.16, 0.95), (0.22, 0.05), (0.3, 1.0), (0.34, 0.3), (0.44, 1.0)];
    let alpha = if t < 1.0 {
        let mut a = 1.0;
        for w in KEYS.windows(2) {
            if t <= w[1].0 {
                let p = (t - w[0].0) / (w[1].0 - w[0].0);
                a = w[0].1 + (w[1].1 - w[0].1) * p;
                break;
            }
        }
        a
    } else {
        let phase = ((t - 1.0) / 1.6) % 2.0;
        let p = if phase < 1.0 { phase } else { 2.0 - phase };
        1.0 - 0.3 * p
    };
    (widgets::ease(t / 0.45), alpha)
}

/// La connexion : le fond de l'ordinateur (son image ou sa signature),
/// assombri ; le tube s'allume dans la couture ; la capsule des étapes.
fn passage(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    match s.console.as_ref().and_then(|c| app.store.console(&c.host_id)) {
        Some(c) => super::ps::backdrop(cv, c, false),
        None => home::backdrop(app, cv, app.store.host(&s.host)),
    }
    let t = s.started.elapsed().as_secs_f32();
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, (110.0 + 60.0 * widgets::ease(t / 0.4)) as u8));
    band(cv, u, 0.75 * widgets::ease(t / 0.6));
    let (scale, alpha) = ignite(t, app.reduce_motion);
    tube(cv, u, scale, alpha);
    capsule_steps(app, cv, f, s, t);
}

/// Le fond de la connexion sur les écrans qui ne portent pas la capsule des
/// étapes (plusieurs écrans) : sa couleur, sans la photo de l'accueil, sans
/// texte (docs/link-ui.md). Un écran que l'ordinateur n'a pas pu créer le garde
/// avec une seule ligne.
fn background(cv: &mut Canvas, u: f32, missing: bool) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(Rgba(5, 7, 10, 255));
    if missing {
        let style = Text::new(Font::Regular, 15.0 * u, MUTED).align(Align::Center);
        cv.text_fit(w / 2.0, h / 2.0 + 5.0 * u, style, tr("L'ordinateur n'a pas pu créer cet écran.", "The computer could not create this display."), w - 64.0 * u);
    }
}

/// La capsule : l'ordinateur, les trois étapes, Annuler.
fn capsule_steps(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, t: f32) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let cw = 520.0 * u;
    let ch = 236.0 * u;
    let (x, y) = ((w - cw) / 2.0, (h - ch) / 2.0);
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 236));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, LINE_2);
    let ix = x + 32.0 * u;
    let mut yy = y + 32.0 * u + 30.0 * u;
    cv.text_shrink(ix - 1.0, yy, Text::new(Font::Display, 36.0 * u, FROST).tracking(-0.7 * u), &s.name, cw - 64.0 * u);
    yy += 26.0 * u;
    // Le dernier recours : la Station redémarre à la place de l'étape « Connexion », puis les étapes reprennent.
    let restarting = s.phase == Phase::StationRestarting;
    let mut steps = vec![
        // Un emprunt : la Station a déjà mis l'ordinateur en mode console (« Préparation de l'ordinateur »).
        if s.borrow.is_some() {
            tr("Préparation de l'ordinateur", "Preparing the computer")
        } else if s.expected_screens >= 2 {
            tr("Préparation des écrans", "Preparing the displays")
        } else {
            tr("Préparation de l'écran", "Preparing the display")
        },
        if restarting { tr("La Station redémarre", "The Station is restarting") } else { tr("Connexion", "Connecting") },
        tr("Premières images", "First frames"),
    ];
    let mut current = match s.phase {
        Phase::Preparing => 0,
        Phase::Connecting | Phase::StationRestarting => 1,
        _ => 2,
    };
    // Une console : la même capsule, ses étapes à elle (« Réveil de la console », quand elle était en veille, puis
    // « Connexion à la console » et « Premières images »).
    if let Some(ps) = &s.console {
        steps = vec![tr("Connexion à la console", "Connecting to the console"), tr("Premières images", "First frames")];
        current = if matches!(s.phase, Phase::FirstImages | Phase::Streaming) { 1 } else { 0 };
        if ps.waking || s.phase == Phase::Preparing {
            steps.insert(0, tr("Réveil de la console", "Waking the console"));
            current = if s.phase == Phase::Preparing { 0 } else { current + 1 };
        }
    }
    for (i, label) in steps.iter().enumerate() {
        let sy = yy + 10.0 * u;
        let c = ix + 9.0 * u;
        if i < current {
            cv.circle(c, sy, 9.0 * u, OK);
            cv.icon(&Icon::Check, c - 6.0 * u, sy - 6.0 * u, 12.0 * u, 2.6, INK);
        } else if i == current {
            let a0 = if app.reduce_motion { 0.0 } else { t * 6.0 };
            let pts: Vec<(f32, f32)> = (0..=14)
                .map(|k| {
                    let a = a0 + k as f32 * 0.32;
                    (c + 8.0 * u * a.cos(), sy + 8.0 * u * a.sin())
                })
                .collect();
            cv.polyline(&pts, 2.0 * u, SPECTRE);
        } else {
            cv.rrect_stroke(c - 8.0 * u, sy - 8.0 * u, 16.0 * u, 16.0 * u, 8.0 * u, 2.0, LINE_2);
        }
        let color = if i <= current { FROST } else { GHOST };
        let label = if i == current { format!("{label}…") } else { label.to_string() };
        cv.text_fit(ix + 28.0 * u, sy + 5.0 * u, Text::new(Font::Regular, 16.0 * u, color), &label, cw - 90.0 * u);
        yy += 30.0 * u;
    }
    let by = y + ch - 30.0 * u;
    let label = tr("Annuler", "Cancel");
    if app.layout.console() {
        let lw = widgets::legend_width(cv, 26.0 * u, &["B"], label);
        legend(cv, x + cw - 32.0 * u - lw, by - 5.0 * u, 26.0 * u, &["B"], label);
        f.state(Target::CancelConnect, Rect::new(x + cw - 32.0 * u - lw, by - 20.0 * u, lw, 30.0 * u));
    } else {
        let bw = cv.text_width(Text::new(Font::Regular, 13.0, MUTED), label) + 26.0;
        widgets::ghost(cv, f, Target::CancelConnect, x + cw - 32.0 - bw, by - 24.0, 36.0, 13.0, None, label, MUTED);
    }
}

// MARK: - Déconnexion

/// Fin : la dernière image reste figée, les moitiés de l'accueil reviennent
/// et se joignent (0,6 s), un dernier éclair dans la couture ; puis
/// « Déconnexion… » le temps que l'ordinateur remette son écran et son son.
fn ending(app: &App, cv: &mut Canvas, s: &SessionUi) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let t = s.ending_since.map(|t| t.elapsed().as_secs_f32()).unwrap_or(10.0);
    let dur = if app.reduce_motion { 0.2 } else { CLOSE };
    let p = widgets::ease(t / dur);
    let mut under = Frame { hits: Vec::new(), focus: Target::Back, show_focus: false, pointer: None };
    home::draw_body(app, cv, &mut under, true);
    if p < 1.0 {
        let home = cv.pix.clone();
        cv.clear_transparent();
        let d = if app.reduce_motion { 0.0 } else { w.max(h) * 0.18 * (1.0 - p) };
        let (a, b, _) = halves(w, h);
        cv.draw_masked(&home, &a, -d, -d, p);
        cv.draw_masked(&home, &b, d, d, p);
    }
    // L'éclair : à 55 % éteint, 70 % allumé, 100 % éteint (sur la durée plus 0,15 s).
    let k = t / (dur + 0.15);
    let flash = if k < 0.55 { 0.0 } else if k < 0.7 { (k - 0.55) / 0.15 } else { (1.0 - (k - 0.7) / 0.3).max(0.0) };
    tube(cv, u, 1.0, flash);
    // « Déconnexion… », une fois les moitiés jointes.
    let a = ((t - dur) / 0.3).clamp(0.0, 1.0);
    if a > 0.0 {
        let msg = if s.console.is_some() { tr("Déconnexion…", "Disconnecting…") } else { tr("Déconnexion… l'ordinateur remet son écran", "Disconnecting… the computer restores its screen") };
        let style = Text::new(Font::Regular, 14.0 * u, MUTED.alpha(a));
        let ch = 38.0 * u;
        let cw = cv.text_width(style, msg) + 56.0 * u;
        let (x, y) = ((w - cw) / 2.0, h - 150.0 * u);
        cv.rrect(x, y, cw, ch, ch / 2.0, Rgba(9, 13, 18, (230.0 * a) as u8));
        cv.rrect_stroke(x, y, cw, ch, ch / 2.0, 1.0, LINE_2.alpha(a));
        let a0 = t * 6.0;
        let (cx, cy) = (x + 20.0 * u, y + ch / 2.0);
        let pts: Vec<(f32, f32)> = (0..=12).map(|k| {
            let ang = a0 + k as f32 * 0.35;
            (cx + 6.0 * u * ang.cos(), cy + 6.0 * u * ang.sin())
        }).collect();
        cv.polyline(&pts, 1.8 * u, SPECTRE.alpha(a));
        cv.text(x + 36.0 * u, y + ch / 2.0 + 5.0 * u, style, msg);
    }
}

// MARK: - Fin sur erreur

fn ended(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, error: Option<&str>) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let mut under = Frame { hits: Vec::new(), focus: Target::Back, show_focus: false, pointer: None };
    home::draw_body(app, cv, &mut under, true);
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
    // Une session d'emprunt : la fin dite par le prêt (« {hôte} a repris l'ordinateur. ») et « Fermer » ; sans le
    // prêt (fenêtre fermée, réseau perdu), « Vous pouvez revenir pendant 15 min. » et « Revenir ».
    if let Some(b) = &s.borrow {
        return borrow_ended(app, cv, f, s, b.ended_by_lend, error);
    }
    let cw = 620.0 * u;
    let ch = 260.0 * u;
    let (x, y) = ((w - cw) / 2.0, (h - ch) / 2.0);
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 240));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, BAD_LINE);
    let ix = x + 32.0 * u;
    // Sans première image, c'est la connexion qui a échoué.
    let title = if s.opened_at.is_none() { tr("La connexion a échoué", "The connection failed") } else { tr("Session terminée", "Session ended") };
    eyebrow(cv, ix, y + 40.0 * u, 12.0 * u, BAD_TEXT, title);
    cv.text_shrink(ix - 1.0, y + 86.0 * u, Text::new(Font::Display, 34.0 * u, FROST).tracking(-0.6 * u), &s.name, cw - 64.0 * u);
    cv.paragraph(ix, y + 122.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), error.unwrap_or(""), cw - 64.0 * u, 22.0 * u);
    // Avec d'autres sessions qui continuent, le bouton ne ramène pas à l'accueil : il ferme cette fenêtre (docs/link-ui.md).
    let back = if app.sessions.len() > 1 { tr("Fermer", "Close") } else { tr("Revenir à l'accueil", "Back to home") };
    widgets::primary(cv, f, Target::Back, ix, y + ch - 82.0 * u, 52.0 * u, back, false);
}

/// La carte de fin d'une session d'emprunt (docs/link-ui.md, « Prêt de l'ordinateur ») : la fin dite par le prêt et
/// « Fermer » ; ou, la session finie sans le prêt, sa raison, « Vous pouvez revenir pendant 15 min. », « Revenir » et
/// « Fermer ».
fn borrow_ended(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi, by_lend: bool, error: Option<&str>) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let cw = 620.0 * u;
    let ch = if by_lend { 250.0 } else { 290.0 } * u;
    let (x, y) = ((w - cw) / 2.0, (h - ch) / 2.0);
    cv.rrect(x, y, cw, ch, 16.0 * u, Rgba(9, 13, 18, 240));
    cv.rrect_stroke(x, y, cw, ch, 16.0 * u, 1.0, if by_lend { LINE_2 } else { BAD_LINE });
    let ix = x + 32.0 * u;
    eyebrow(cv, ix, y + 40.0 * u, 12.0 * u, if by_lend { FAINT } else { BAD_TEXT }, "KyberNow Share");
    cv.text_shrink(ix - 1.0, y + 86.0 * u, Text::new(Font::Display, 34.0 * u, FROST).tracking(-0.6 * u), &s.name, cw - 64.0 * u);
    let mut yy = y + 122.0 * u;
    if let Some(e) = error.filter(|e| !e.is_empty()) {
        yy += cv.paragraph(ix, yy, Text::new(Font::Regular, 15.0 * u, MUTED), e, cw - 64.0 * u, 22.0 * u);
    }
    let by = y + ch - 82.0 * u;
    if by_lend {
        widgets::primary(cv, f, Target::Back, ix, by, 52.0 * u, tr("Fermer", "Close"), false);
        return;
    }
    cv.text_fit(ix, yy + 4.0 * u, Text::new(Font::Regular, 15.0 * u, FROST), tr("Vous pouvez revenir pendant 15 min.", "You can come back for 15 min."), cw - 64.0 * u);
    let bw = widgets::primary(cv, f, Target::ComeBack, ix, by, 52.0 * u, tr("Revenir", "Come back"), false);
    widgets::ghost(cv, f, Target::Back, ix + bw + 16.0 * u, by + 4.0 * u, 44.0 * u, 15.0 * u, None, tr("Fermer", "Close"), MUTED);
}

// MARK: - Barre du bureau

/// Hauteur de la barre du bureau.
pub(super) const BAR_H: f32 = 46.0;
/// Le pointeur ouvre la barre à 2 points au plus du bord haut ; il la garde
/// ouverte à 8 points au plus de son rectangle.
const BAR_EDGE: f32 = 2.0;
/// La même bande, session dans une fenêtre.
const BAR_EDGE_WINDOW: f32 = 8.0;
const BAR_MARGIN: f32 = 8.0;
/// Le trait qui signale l'endroit (celui de l'app Mac : capsule blanche à 35 %,
/// 220 × 3 points) et la hauteur du bandeau qui le contient.
const HINT_W: f32 = 220.0;
const HINT_H: f32 = 3.0;
pub const HINT_STRIP: f32 = 4.0;

/// Un bouton de la barre : sa cible, son dessin, s'il est allumé, son nom au survol (le texte de
/// « Partager » quand il est inactif ; le raccourci entre parenthèses), s'il est éteint, et son libellé court
/// (sa ligne du menu « Plus »).
struct BarIcon {
    target: Target,
    icon: Icon,
    on: bool,
    name: String,
    dim: bool,
    label: String,
}

/// Les commandes qui passent dans « Plus », une à une et dans cet ordre, quand la barre ne tient pas (docs/link-ui.md,
/// « Menu « Plus » ») ; jamais l'ordinateur, la latence, le profil, « HDR », « Partager » et ses pastilles, « Prêter
/// l'ordinateur… », l'enregistrement, « Par le relais », « Se déconnecter ».
const MORE_ORDER: [Target; 8] = [Target::BarComputers, Target::BarKeys, Target::BarMouse, Target::BarSound, Target::BarMic, Target::BarCamera, Target::BarClipboard, Target::BarFullscreen];

/// Les boutons de la barre, de gauche à droite, dans l'ordre de link-ui : ordinateurs (plusieurs ordinateurs),
/// touches et souris (une session d'hôte, ou un joueur à qui le clavier et la souris sont donnés), haut-parleur
/// (« Garder le son »), micro, caméra, la pastille « Presse-papiers » (hôte), plein écran (une seule fenêtre),
/// « Partager » (une session d'hôte ; la pastille des invités le suit). La barre de Linux est en icônes seules : le
/// libellé est l'infobulle. Une session d'invité n'a ni profil, ni ordinateurs, ni « Partager ».
fn bar_icons(app: &App, s: &SessionUi, computers: bool, sound: bool) -> Vec<BarIcon> {
    let icon = |target, icon, on, name: &str| BarIcon { target, icon, on, name: name.into(), dim: false, label: name.into() };
    let mut v = Vec::new();
    if computers {
        v.push(icon(Target::BarComputers, Icon::Monitor, s.bar_menu == Some(BarMenu::Computers), tr("Ordinateurs", "Computers")));
    }
    if s.keyboard_mouse() {
        v.push(icon(Target::BarKeys, Icon::Keys, s.bar_menu == Some(BarMenu::Keys), tr("Touches", "Keys")));
        // La souris : le mode du moment (pointeur, viseur en souris des jeux), un menu « Auto », « Souris du bureau »,
        // « Souris des jeux ».
        let now = if s.relative { tr("Souris des jeux", "Game mouse") } else { tr("Souris du bureau", "Desktop mouse") };
        v.push(BarIcon { target: Target::BarMouse, icon: if s.relative { Icon::Crosshair } else { Icon::Mouse }, on: s.bar_menu == Some(BarMenu::Mouse), name: crate::app::mouse_tooltip(), dim: false, label: now.into() });
    }
    if sound {
        v.push(icon(Target::BarSound, Icon::Speaker, s.keep_sound, tr("Garder le son", "Keep sound")));
    }
    // « Micro », après le son (un micro sur l'appareil) : allumé tant qu'il est envoyé, estompé quand l'ordinateur
    // ne le prend pas (la raison en infobulle). Un invité ne l'a que si l'hôte laisse les invités parler.
    if let Some(m) = s.mic.as_ref().filter(|m| m.shown) {
        v.push(BarIcon { target: Target::BarMic, icon: if m.lit { Icon::Mic } else { Icon::MicOff }, on: m.lit, name: m.tooltip.clone(), dim: m.reason.is_some(), label: m.label.clone() });
    }
    // La caméra de l'appareil (docs/link-ui.md, « Caméra de l'appareil ») : seulement si l'ordinateur la propose.
    if s.camera_offered {
        let on = s.camera == CameraState::Sending || s.bar_menu == Some(BarMenu::Camera);
        let name = if s.camera == CameraState::Off { tr("Allumer la caméra", "Turn on camera") } else { tr("Éteindre la caméra", "Turn off camera") };
        let glyph = if s.camera == CameraState::Off { Icon::CameraOff } else { Icon::Camera };
        v.push(BarIcon { target: Target::BarCamera, icon: glyph, on, name: format!("{name} ({})", crate::app::shortcut_text('C')), dim: s.camera == CameraState::Off, label: tr("Caméra", "Camera").into() });
    }
    // La pastille « Presse-papiers » (hôte), sans action : estompée quand il n'est pas partagé.
    if !s.visitor() && s.borrow.is_none() {
        let shared = app.clipboard_shared(s);
        let name = if shared { tr("Presse-papiers partagé avec l'ordinateur (texte)", "Clipboard shared with the computer (text)") } else { tr("Presse-papiers non partagé (Réglages)", "Clipboard not shared (Settings)") };
        v.push(BarIcon { target: Target::BarClipboard, icon: Icon::Clipboard, on: false, name: name.into(), dim: !shared, label: tr("Presse-papiers", "Clipboard").into() });
    }
    if !s.multi() {
        let name = format!("{} ({})", tr("Plein écran", "Full screen"), crate::app::shortcut_text('F'));
        v.push(BarIcon { target: Target::BarFullscreen, icon: Icon::Fullscreen, on: s.fullscreen, name, dim: false, label: tr("Plein écran", "Full screen").into() });
    }
    if !s.visitor() {
        // Partage impossible (Station trop ancienne, partage coupé) : le bouton est estompé et inactif à la
        // souris, sa raison est son infobulle. Tant qu'Admin n'a pas répondu, il reste comme les autres : le
        // panneau attend la réponse (`App::share_known`).
        let blocked = app.share_avail(s).filter(|a| *a != ShareAvail::Available);
        let name = blocked.and_then(|a| a.text()).unwrap_or(tr("Partager", "Share"));
        v.push(BarIcon { target: Target::BarShare, icon: Icon::Share, on: s.share.is_some() || s.bar_menu == Some(BarMenu::Share), name: name.into(), dim: blocked.is_some(), label: tr("Partager", "Share").into() });
        // « Prêter l'ordinateur… » : estompé quand le prêt est impossible (la raison est son infobulle, et le panneau
        // qu'il ouvre la dit) ; allumé quand un code est en cours ou qu'une demande attend.
        let blocked = app.lend_avail(s).filter(|a| *a != crate::app::LendAvail::Available).is_some();
        let on = s.bar_menu == Some(BarMenu::Lend) || s.lend.as_ref().is_some_and(|v| v.code.is_some() || !v.requests.is_empty());
        let name = app.lend_avail(s).and_then(|a| a.text()).unwrap_or(tr("Prêter l'ordinateur…", "Lend the computer…"));
        v.push(BarIcon { target: Target::BarLend, icon: Icon::Lend, on, name: name.into(), dim: blocked, label: tr("Prêter l'ordinateur…", "Lend the computer…").into() });
        // « Enregistrer » (Station qui enregistre, rien en cours) : estompé quand c'est coupé dans Admin (la raison
        // est son infobulle) ; pendant l'enregistrement, la pastille rouge prend sa place (`rec_pill`).
        if let Some(r) = s.recording.as_ref().filter(|r| !r.active) {
            v.push(BarIcon { target: Target::BarRecord, icon: Icon::Record, on: false, name: r.tooltip().to_string(), dim: r.blocked(), label: r.label() });
        }
    }
    v
}

/// La pastille rouge de l'enregistrement en cours : « Enregistrement · 02:14 » chez l'hôte (un clic l'arrête),
/// « Enregistrement » chez un invité (sans action) ; `narrow` : la barre ne tient pas, « 02:14 » seul.
fn rec_pill(s: &SessionUi, narrow: bool) -> Option<String> {
    let r = s.recording.as_ref().filter(|r| r.active && s.borrow.is_none() && s.console.is_none())?;
    Some(if narrow && !r.guest { r.clock() } else { r.label() })
}

/// La largeur de la pastille : le rond, le texte, les marges.
fn rec_pill_width(cv: &mut Canvas, label: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, 12.5, FROST), label) + 22.0 + 14.0
}

/// Ce que la barre du bureau montre, mesuré : ses textes, leurs largeurs et sa
/// largeur totale (selon le contenu). Le dessin et le geste du pointeur
/// (zone d'ouverture, rectangle) partent de la même mesure.
struct BarLayout {
    lat: String,
    info: String,
    /// Le profil (menu) d'une session d'hôte ; le rôle d'un invité (« Spectateur », « Joueur »), « Prêté par {hôte} »
    /// d'un emprunt, sans menu.
    profile: String,
    quit: &'static str,
    /// Un emprunt : le temps qui reste (« Encore 1 h 20 », « Jusqu'à ce que {hôte} reprenne »), et sa largeur.
    left: Option<(String, f32)>,
    mono14: Text,
    mono12: Text,
    lat_w: f32,
    info_w: f32,
    prof_w: f32,
    quit_w: f32,
    /// Un emprunt : « Se déconnecter » avant « Rendre l'ordinateur » (la session se ferme, l'ordinateur reste prêté
    /// 15 min) ; sa largeur, 0 ailleurs.
    disc_w: f32,
    icons: Vec<BarIcon>,
    /// « 2 invités » : le partage est ouvert (il rouvre le panneau), et sa largeur.
    pill: Option<(String, f32)>,
    /// « 1 demande », « 2 demandes » : des invités attendent l'accord de l'hôte (elle rouvre le panneau), et sa largeur.
    requests: Option<(String, f32)>,
    /// La puce de « Qualité de l'image » d'un invité : la qualité reçue (le choix tant qu'on ne la connaît pas), et sa
    /// largeur.
    quality: Option<(String, f32)>,
    /// Regarder, avec plusieurs écrans : la puce « Écran » (son menu : un écran par ligne), et sa largeur.
    screen: Option<(String, f32)>,
    /// « Par le relais » (« Par le relais (TCP) » en repli) : la session passe par le relais ; sa largeur.
    route: Option<(String, f32)>,
    /// « HDR », cochable, après le profil (écran HDR seulement) : son état et sa largeur.
    hdr: Option<(crate::app::HdrUi, f32)>,
    /// PyroWave en Wi-Fi ou par le relais : la puce ambre (« PyroWave en Wi-Fi ») et sa largeur.
    pw: Option<(String, f32)>,
    /// L'enregistrement en cours : la pastille rouge (`rec_pill`) et sa largeur.
    rec: Option<(String, f32)>,
    /// Barre étroite : les commandes passées dans « Plus » (dans l'ordre de `MORE_ORDER`).
    more: Vec<BarIcon>,
    total: f32,
}

impl BarLayout {
    /// `w` : la largeur de la fenêtre, en points. Les mesures (définition, images par seconde, décodeur) sont la
    /// seule partie qui cède de la place : « Partager », sa pastille d'invités et « Se déconnecter » restent entiers.
    fn measure(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32) -> Self {
        let st = s.stats.clone().unwrap_or_default();
        let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "— ms".into());
        // Avec plusieurs écrans pris, leur nombre d'abord (le même compte que les statistiques de la dernière session).
        let mode = if s.screens >= 2 { format!("{} {} · {}", s.screens, tr("écrans", "displays"), st.mode) } else { st.mode.clone() };
        let info = if st.fps > 0.0 { format!("{} · {:.0} {} · {}", mode, st.fps, tr("i/s", "fps"), st.decoder) } else { mode };
        let mono14 = Text::new(Font::MonoMedium, 14.0, FROST);
        let mono12 = Text::new(Font::Mono, 12.0, MUTED);
        let lat_w = cv.text_width(mono14, &lat);
        let mut info_w = cv.text_width(mono12, &info);
        let profile = match (&s.guest, &s.borrow) {
            (Some(g), _) => g.role.label().to_string(),
            (None, Some(b)) => b.lent_by(),
            (None, None) => home::profile_label(s.profile).to_string(),
        };
        let prof_w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), &profile) + if s.visitor() { 24.0 } else { 34.0 };
        let quit = if s.guest.is_some() {
            s.leave_label()
        } else if s.borrow.is_some() {
            tr("Rendre l'ordinateur", "Give back the computer")
        } else {
            tr("Se déconnecter", "Disconnect")
        };
        let quit_w = cv.text_width(Text::new(Font::Semibold, 13.0, BAD_TEXT), quit) + 24.0;
        let disc_w = if s.borrow.is_some() { cv.text_width(Text::new(Font::Regular, 13.0, MUTED), tr("Se déconnecter", "Disconnect")) + 24.0 } else { 0.0 };
        let left = s.borrow.as_ref().map(|b| {
            let t = b.time_left(crate::model::now_secs());
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        // Plusieurs ordinateurs (bureau, hors KyberNow Link OS) : le menu « Ordinateurs » s'il y a un autre
        // ordinateur, le haut-parleur « Garder le son » avec au moins deux sessions. Un invité n'a pas le premier.
        let multi = app.multi_sessions();
        let computers = multi && !s.visitor() && app.store.hosts.iter().any(|h| h.id != s.host);
        let sound = multi && app.running_sessions() >= 2;
        let icons = bar_icons(app, s, computers, sound);
        // Un invité : la qualité de l'image qu'il reçoit, en menu (il n'a pas de profil).
        let quality = s.guest.as_ref().map(|g| {
            let t = g.shown().to_string();
            let w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), &t) + 34.0;
            (t, w)
        });
        // Qui regarde un ordinateur à plusieurs écrans : « Écran », en menu (docs/link-ui.md, « Regarder, barre »).
        let screen = s.watch_menu().then(|| {
            let t = crate::app::watch_screen_word().to_string();
            let w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), &t) + 34.0;
            (t, w)
        });
        let pill = share::pill_label(s).map(|t| {
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        let requests = share::requests_pill_label(s).map(|t| {
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        // Par le relais : la puce, dans la partie toujours visible de la barre (docs/link-ui.md, « Hors de chez vous »).
        let route = s.via_relay().then(|| {
            let t = s.path().label().to_string();
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        let hdr = s.hdr.map(|u| (u, cv.text_width(Text::new(Font::Semibold, 13.0, FROST), "HDR") + 40.0));
        let pw = s.pyrowave_risk().map(|r| {
            let t = r.chip().to_string();
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        let rec = rec_pill(s, false).map(|t| {
            let w = rec_pill_width(cv, &t);
            (t, w)
        });
        let rec_w = |rec: &Option<(String, f32)>| rec.as_ref().map_or(0.0, |(_, rw)| rw + 4.0);
        let others = rec_w(&rec) + pw.as_ref().map_or(0.0, |(_, pw)| pw + 8.0) + 16.0 + 16.0 + 12.0 + lat_w + 8.0 + 64.0 + 12.0 + 1.0 + 12.0 + 12.0 + 1.0 + 12.0 + prof_w + 12.0 + hdr.as_ref().map_or(0.0, |(_, hw)| hw + 8.0) + left.as_ref().map_or(0.0, |(_, lw)| lw + 8.0) + route.as_ref().map_or(0.0, |(_, rw)| rw + 8.0) + quality.as_ref().map_or(0.0, |(_, qw)| qw + 8.0) + screen.as_ref().map_or(0.0, |(_, sw)| sw + 8.0) + 34.0 * icons.len() as f32 + pill.as_ref().map_or(0.0, |(_, pw)| pw + 4.0) + requests.as_ref().map_or(0.0, |(_, pw)| pw + 4.0) + 24.0 + 1.0 + 12.0 + if disc_w > 0.0 { disc_w + 4.0 } else { 0.0 } + quit_w + 10.0;
        // Barre étroite : la pastille de l'enregistrement se réduit au temps (« 02:14 »).
        let (rec, others) = if w - 8.0 - others < 60.0 && rec.is_some() {
            let before = rec_w(&rec);
            let rec = rec_pill(s, true).map(|t| {
                let w = rec_pill_width(cv, &t);
                (t, w)
            });
            let others = others - before + rec_w(&rec);
            (rec, others)
        } else {
            (rec, others)
        };
        // Barre étroite (docs/link-ui.md) : d'abord les mesures se resserrent puis disparaissent (fenêtre d'un autre
        // ordinateur, 480 points) ; puis les pastilles se réduisent au nombre (« 2 ») ; puis les commandes passent une
        // à une dans « Plus » (8 points de marge de chaque côté).
        let avail = w - 16.0;
        let mut others = others;
        let (mut pill, mut requests) = (pill, requests);
        if others > avail {
            for p in [&mut pill, &mut requests] {
                if let Some((t, pw)) = p.as_mut() {
                    let n = t.split(' ').next().unwrap_or_default().to_string();
                    let nw = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &n) + 22.0;
                    others -= *pw - nw;
                    *t = n;
                    *pw = nw;
                }
            }
        }
        let mut icons = icons;
        let mut more: Vec<BarIcon> = Vec::new();
        for target in MORE_ORDER {
            if others <= avail {
                break;
            }
            let Some(i) = icons.iter().position(|ic| ic.target == target) else { continue };
            let moved = icons.remove(i);
            if more.is_empty() {
                // « Plus » prend la place de la première commande retirée.
                icons.insert(i, BarIcon { target: Target::BarMore, icon: Icon::More, on: s.bar_menu == Some(BarMenu::More), name: tr("Plus de commandes", "More controls").into(), dim: false, label: tr("Plus", "More").into() });
            } else {
                others -= 34.0;
            }
            more.push(moved);
        }
        info_w = info_w.min((w - 8.0 - others).max(0.0));
        let total = others + info_w;
        Self { lat, info, profile, quit, left, mono14, mono12, lat_w, info_w, prof_w, quit_w, disc_w, icons, pill, requests, quality, screen, route, hdr, pw, rec, more, total }
    }
}

/// Le rectangle de la barre ouverte : sa largeur réelle, centrée en haut. Plus large que la fenêtre (fenêtres
/// étroites de plusieurs ordinateurs), elle s'aligne sur le bord droit : « Se déconnecter » reste à l'écran,
/// le début (latence) sort à gauche.
fn bar_frame(w: f32, total: f32) -> Rect {
    let x = if total <= w - 16.0 { (w - total) / 2.0 } else { w - 8.0 - total };
    Rect::new(x, -1.0, total, BAR_H + 1.0)
}

/// Un menu déroulant dans la fenêtre : sa largeur (bornée à la fenêtre), sa gauche (jamais hors du bord) et
/// le nombre de lignes qui tiennent sous `y`.
fn dropdown_place(cv_w: f32, cv_h: f32, x: f32, y: f32, w: f32, head: f32, rows: usize) -> (f32, f32, usize) {
    let w = w.min(cv_w - 16.0).max(0.0);
    let x = x.min(cv_w - w - 8.0).max(8.0);
    let fit = ((cv_h - y - 8.0 - 12.0 - head) / 36.0).floor().max(1.0) as usize;
    (x, w, rows.min(fit))
}

/// `w` : la largeur de la fenêtre, en points (chaque écran a la sienne).
fn bar_rect(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32) -> Rect {
    // Appelé à chaque mouvement de la souris (bord haut, survol de la barre) : la largeur mesurée est gardée
    // 100 ms par session et largeur de fenêtre (une vingtaine de mesures de texte de moins par mouvement,
    // LINAPP-35) ; le dessin, lui, mesure toujours.
    thread_local! {
        static BAR_TOTAL: std::cell::Cell<Option<(crate::app::SessionId, u32, std::time::Instant, f32)>> = const { std::cell::Cell::new(None) };
    }
    let key = (s.id, w.to_bits());
    let cached = BAR_TOTAL.with(|c| c.get()).filter(|(id, wb, at, _)| (*id, *wb) == key && at.elapsed() < std::time::Duration::from_millis(100)).map(|c| c.3);
    let total = match cached {
        Some(t) => t,
        None => {
            let t = if s.console.is_some() { super::ps::bar_width(cv, s) } else { BarLayout::measure(cv, app, s, w).total };
            BAR_TOTAL.with(|c| c.set(Some((key.0, key.1, std::time::Instant::now(), t))));
            t
        }
    };
    bar_frame(w, total)
}

/// Le pointeur est contre le bord haut, dans la zone centrée de la largeur de
/// la barre : l'endroit où elle s'ouvre (après un quart de seconde).
pub fn at_bar_edge(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, ux: f32, uy: f32) -> bool {
    // Dans une fenêtre, le bord haut de l'image touche la barre de titre du système : le pointeur qui la
    // vise en sort vite. Une bande plus haute, comme le trait qui la signale.
    let edge = if s.fullscreen { BAR_EDGE } else { BAR_EDGE_WINDOW };
    if uy > edge {
        return false;
    }
    let r = bar_rect(cv, app, s, w);
    ux >= r.x && ux < r.x + r.w
}

/// Le pointeur est sur la barre ouverte (son rectangle avec une petite marge)
/// ou sur un de ses éléments (`hits`, menus compris).
pub fn over_bar(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, hits: &[(Target, Rect)], ux: f32, uy: f32) -> bool {
    let r = bar_rect(cv, app, s, w);
    (ux >= r.x - BAR_MARGIN && ux < r.x + r.w + BAR_MARGIN && uy < r.y + r.h + BAR_MARGIN) || hits.iter().any(|(_, h)| h.contains(ux, uy))
}

/// Le trait se voit : bureau, image en cours, barre fermée, souris du bureau
/// (en souris des jeux le pointeur est capturé, la barre ne vient pas ainsi).
/// `s` : la session de la fenêtre ; seule la session active a une barre (le geste est une
/// entrée : une session qui n'est pas active ne reçoit rien, pas même le pointeur).
pub fn bar_hint(app: &App, s: &SessionUi) -> bool {
    app.layout == Layout::Desktop
        && Some(s.id) == app.active
        && app.screen == Screen::Session
        && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. })
        && !s.menu
        && !s.keys
        && !s.bar
        && !s.relative
}

/// Le trait qui signale la barre, centré contre le bord haut.
fn hint(cv: &mut Canvas) {
    let x = (cv.w - HINT_W) / 2.0;
    cv.rrect(x, 0.0, HINT_W, HINT_H, HINT_H / 2.0, Rgba(255, 255, 255, 89));
}

fn bar(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let h = BAR_H;
    let st = s.stats.clone().unwrap_or_default();
    let width = cv.w;
    let BarLayout { lat, info, profile, quit, left, mono14, mono12, lat_w, info_w, prof_w, quit_w, disc_w, icons, pill, requests, quality, screen, route, hdr, pw, rec, more, total } = BarLayout::measure(cv, app, s, width);
    let mut tip: Option<(Rect, String)> = None;
    let r = bar_frame(cv.w, total);
    let x0 = r.x;
    cv.rrect(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, Rgba(9, 13, 18, 235));
    cv.rrect_stroke(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, 1.0, LINE_2);
    let cy = h / 2.0;
    let mut x = x0 + 16.0;
    // Les mesures (symbole, latence, courbe, image) : un clic ouvre « Statistiques » (docs/link-ui.md).
    {
        let mr = Rect::new(x - 8.0, cy - 15.0, 16.0 + 12.0 + lat_w + 8.0 + 64.0 + 12.0 + 13.0 + info_w + 12.0, 30.0);
        let (focused, hover) = f.state(Target::BarStats, mr);
        if hover || focused || s.show_stats {
            cv.rrect(mr.x, mr.y, mr.w, mr.h, 7.0, Rgba(227, 237, 240, if s.show_stats { 26 } else { 16 }));
        }
        if focused {
            cv.neon(mr.x, mr.y, mr.w, mr.h, 7.0, 1.0);
        }
    }
    // Le symbole allumé : un flux est en cours.
    cv.glow(x + 8.0, cy, 14.0, 14.0, HALO.alpha(0.5));
    cv.mark(x, cy - 8.0, 16.0, FROST, SPECTRE, Some(CORE));
    x += 16.0 + 12.0;
    cv.text(x, cy + 5.0, mono14, &lat);
    x += lat_w + 8.0;
    // La courbe des dernières secondes.
    let hist = &st.history;
    if hist.len() >= 2 {
        let max = hist.iter().cloned().fold(1.0f64, f64::max).max(16.7);
        let pts: Vec<(f32, f32)> = hist.iter().enumerate().map(|(i, v)| (x + i as f32 * 64.0 / (hist.len() - 1) as f32, cy + 8.0 - (*v / max) as f32 * 16.0)).collect();
        cv.polyline(&pts, 1.5, SPECTRE);
    }
    x += 64.0 + 12.0;
    cv.rect(x.round(), cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    cv.text_fit(x, cy + 4.0, mono12, &info, info_w);
    x += info_w + 12.0;
    cv.rect(x.round(), cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    // Profil (menu) ; un invité n'a pas de profil : son rôle, que l'hôte change, sans menu.
    let pr = Rect::new(x, cy - 15.0, prof_w, 30.0);
    if s.visitor() {
        cv.rrect_stroke(pr.x, pr.y, pr.w, pr.h, 7.0, 1.0, LINE_2);
        cv.text(pr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), &profile);
    } else {
        let (focused, hover) = f.state(Target::BarProfile, pr);
        if hover {
            tip = Some((pr, tr("Profil : Jeu, Bureau, Max ou Auto", "Profile: Game, Desktop, Max or Auto").into()));
        }
        cv.rrect(pr.x, pr.y, pr.w, pr.h, 7.0, if hover || s.bar_menu == Some(BarMenu::Profile) { INK_3 } else { Rgba(20, 27, 35, 255) });
        cv.rrect_stroke(pr.x, pr.y, pr.w, pr.h, 7.0, 1.0, LINE_2);
        if focused {
            widgets::ring(cv, pr, 7.0);
        }
        cv.text(pr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), &profile);
        cv.text(pr.x + pr.w - 12.0, cy + 4.0, Text::new(Font::Regular, 11.0, FAINT).align(Align::Right), "▾");
    }
    let profile_x = pr.x;
    x += prof_w + 12.0;
    // « HDR », cochable, près du profil : allumé (fond, voyant), éteint, ou inactif (estompé ; son clic dit la raison).
    if let Some((u, hw)) = hdr {
        let hr = Rect::new(x, cy - 15.0, hw, 30.0);
        let (focused, hover) = f.state(Target::BarHdr, hr);
        cv.rrect(hr.x, hr.y, hr.w, hr.h, 7.0, if u.lit() { FLAT.alpha(0.22) } else if hover { INK_3 } else { Rgba(20, 27, 35, 255) });
        cv.rrect_stroke(hr.x, hr.y, hr.w, hr.h, 7.0, 1.0, LINE_2);
        if focused {
            widgets::ring(cv, hr, 7.0);
        }
        if hover {
            tip = Some((hr, if u.available() { crate::app::hdr_state_label(u.on).to_string() } else { u.help().to_string() }));
        }
        let (dx, dot) = (hr.x + 12.0 + 3.0, 6.0);
        if u.lit() {
            cv.circle(dx, cy, dot / 2.0, SPECTRE);
        } else {
            cv.circle_stroke(dx, cy, dot / 2.0, 1.2, if u.available() { MUTED } else { DIM });
        }
        let color = if !u.available() { DIM } else if u.on { FROST } else { MUTED };
        cv.text(hr.x + 26.0, cy + 5.0, Text::new(if u.lit() { Font::Semibold } else { Font::Regular }, 13.0, color), "HDR");
        x += hw + 8.0;
    }
    // Un emprunt : le temps qui reste, en puce (ambrée pendant le préavis de 2 minutes).
    if let Some((label, lw)) = &left {
        let lr = Rect::new(x, cy - 12.0, *lw, 24.0);
        let notice = s.borrow.as_ref().is_some_and(|b| b.notice.is_some());
        cv.rrect(lr.x, lr.y, lr.w, lr.h, 12.0, if notice { WARN.alpha(0.24) } else { Rgba(227, 237, 240, 14) });
        cv.text(lr.x + lr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), label);
        x += lw + 8.0;
    }
    // Par le relais : une puce, sans menu (le chemin se lit, il ne se choisit pas).
    if let Some((label, rw)) = &route {
        let rr = Rect::new(x, cy - 12.0, *rw, 24.0);
        cv.rrect(rr.x, rr.y, rr.w, rr.h, 12.0, FLAT.alpha(0.22));
        cv.text(rr.x + rr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), label);
        x += rw + 8.0;
    }
    // PyroWave en Wi-Fi ou hors de chez soi : la puce ambre (docs/link-ui.md, « PyroWave »).
    if let Some((label, pww)) = &pw {
        let rr = Rect::new(x, cy - 12.0, *pww, 24.0);
        cv.rrect(rr.x, rr.y, rr.w, rr.h, 12.0, WARN.alpha(0.24));
        cv.text(rr.x + rr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), label);
        x += pww + 8.0;
    }
    // Un invité : « Qualité de l'image », une puce qui montre la qualité reçue (« 1080p · 60 Hz ») et ouvre les
    // cinq choix, comme le profil d'un hôte.
    let mut quality_x = x;
    if let Some((label, qw)) = &quality {
        let qr = Rect::new(x, cy - 15.0, *qw, 30.0);
        let (focused, hover) = f.state(Target::BarQuality, qr);
        cv.rrect(qr.x, qr.y, qr.w, qr.h, 7.0, if hover || s.bar_menu == Some(BarMenu::Quality) { INK_3 } else { Rgba(20, 27, 35, 255) });
        cv.rrect_stroke(qr.x, qr.y, qr.w, qr.h, 7.0, 1.0, LINE_2);
        if focused {
            widgets::ring(cv, qr, 7.0);
        }
        if hover {
            tip = Some((qr, tr("Qualité de l'image", "Picture quality").to_string()));
        }
        cv.text(qr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), label);
        cv.text(qr.x + qr.w - 12.0, cy + 4.0, Text::new(Font::Regular, 11.0, FAINT).align(Align::Right), "▾");
        quality_x = qr.x;
        x += qw + 8.0;
    }
    // Regarder, avec plusieurs écrans : « Écran », du même dessin ; son menu liste les écrans, celui regardé coché.
    let mut screen_x = x;
    if let Some((label, sw)) = &screen {
        let sr = Rect::new(x, cy - 15.0, *sw, 30.0);
        let (focused, hover) = f.state(Target::BarScreen, sr);
        cv.rrect(sr.x, sr.y, sr.w, sr.h, 7.0, if hover || s.bar_menu == Some(BarMenu::Screen) { INK_3 } else { Rgba(20, 27, 35, 255) });
        cv.rrect_stroke(sr.x, sr.y, sr.w, sr.h, 7.0, 1.0, LINE_2);
        if focused {
            widgets::ring(cv, sr, 7.0);
        }
        cv.text(sr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), label);
        cv.text(sr.x + sr.w - 12.0, cy + 4.0, Text::new(Font::Regular, 11.0, FAINT).align(Align::Right), "▾");
        screen_x = sr.x;
        x += sw + 8.0;
    }
    // Ordinateurs et touches (deux menus du même dessin), souris, haut-parleur (« Garder le son »), plein
    // écran, « Partager » ; chacun nomme son bouton au survol.
    let (mut keys_x, mut computers_x, mut share_x, mut lend_x, mut camera_x, mut mouse_x, mut more_x) = (x, x, x, x, x, x, x);
    for BarIcon { target: t, icon, on, name, dim, .. } in icons {
        let ir = Rect::new(x, cy - 17.0, 34.0, 34.0);
        // La pastille « Presse-papiers » n'a pas d'action : survolée, son infobulle, jamais choisie.
        let (focused, hover) = if t == Target::BarClipboard { (false, f.pointer.is_some_and(|(px, py)| ir.contains(px, py))) } else { f.state(t, ir) };
        if on {
            cv.rrect(ir.x, ir.y, ir.w, ir.h, 7.0, FLAT.alpha(0.22));
        } else if hover {
            cv.rrect(ir.x, ir.y, ir.w, ir.h, 7.0, Rgba(227, 237, 240, 16));
        }
        if focused {
            widgets::ring(cv, ir, 7.0);
        }
        if hover {
            tip = Some((ir, name));
        }
        match t {
            Target::BarComputers => computers_x = ir.x,
            Target::BarKeys => keys_x = ir.x,
            Target::BarShare => share_x = ir.x,
            Target::BarLend => lend_x = ir.x,
            Target::BarCamera => camera_x = ir.x,
            Target::BarMouse => mouse_x = ir.x,
            Target::BarMore => more_x = ir.x,
            _ => {}
        }
        cv.icon(&icon, x + 8.0, cy - 9.0, 18.0, 1.8, if on { FROST } else if dim { GHOST } else { MUTED });
        x += 34.0;
    }
    // L'enregistrement en cours : la pastille rouge, à la place de « Enregistrer » ; le rond bat une fois par seconde
    // (fixe avec moins d'animations). Chez l'hôte, un clic l'arrête ; chez un invité, elle ne fait rien.
    if let Some((label, rw)) = &rec {
        let rr = Rect::new(x + 2.0, cy - 12.0, *rw, 24.0);
        let host = s.recording.as_ref().is_some_and(|r| !r.guest);
        let (focused, hover) = if host { f.state(Target::BarRecord, rr) } else { (false, f.pointer.is_some_and(|(px, py)| rr.contains(px, py))) };
        cv.rrect(rr.x, rr.y, rr.w, rr.h, 12.0, BAD.alpha(if hover && host { 0.34 } else { 0.22 }));
        if focused {
            widgets::ring(cv, rr, 12.0);
        }
        let beat = if app.reduce_motion { 1.0 } else { rec_beat(s) };
        cv.circle(rr.x + 13.0, cy, 4.0, BAD.alpha(0.45 + 0.55 * beat));
        cv.text(rr.x + 22.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST), label);
        if hover {
            if let Some(r) = &s.recording {
                tip = Some((rr, r.tooltip().to_string()));
            }
        }
        x += rw + 4.0;
    }
    // La pastille « 2 invités » : le partage est ouvert ; elle rouvre le panneau.
    if let Some((label, pw)) = pill {
        let pr = Rect::new(x + 2.0, cy - 12.0, pw, 24.0);
        let (focused, hover) = f.state(Target::BarSharePill, pr);
        cv.rrect(pr.x, pr.y, pr.w, pr.h, 12.0, FLAT.alpha(if hover || s.bar_menu == Some(BarMenu::Share) { 0.34 } else { 0.22 }));
        if focused {
            widgets::ring(cv, pr, 12.0);
        }
        cv.text(pr.x + pr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), &label);
        x += pw + 4.0;
    }
    // « 1 demande » : un invité attend l'accord de l'hôte ; plus appuyée que les invités, elle rouvre aussi le panneau.
    if let Some((label, pw)) = requests {
        let pr = Rect::new(x + 2.0, cy - 12.0, pw, 24.0);
        let (focused, hover) = f.state(Target::BarShareRequests, pr);
        cv.rrect(pr.x, pr.y, pr.w, pr.h, 12.0, FLAT.alpha(if hover || s.bar_menu == Some(BarMenu::Share) { 0.7 } else { 0.55 }));
        if focused {
            widgets::ring(cv, pr, 12.0);
        }
        cv.text(pr.x + pr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), &label);
        x += pw + 4.0;
    }
    x += 12.0;
    cv.rect(x.round(), cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    // Un emprunt : « Se déconnecter » (la session se ferme, on peut revenir pendant 15 min), puis « Rendre l'ordinateur »
    // (sa confirmation) à la place du « Se déconnecter » des autres sessions.
    if disc_w > 0.0 {
        let dr = Rect::new(x, cy - 15.0, disc_w, 30.0);
        let (focused, hover) = f.state(Target::BarQuit, dr);
        if hover {
            cv.rrect(dr.x, dr.y, dr.w, dr.h, 7.0, Rgba(227, 237, 240, 16));
        }
        if focused {
            widgets::ring(cv, dr, 7.0);
        }
        cv.text(dr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, MUTED), tr("Se déconnecter", "Disconnect"));
        if hover {
            tip = Some((dr, disconnect_tooltip()));
        }
        x += disc_w + 4.0;
    }
    let qr = Rect::new(x, cy - 15.0, quit_w, 30.0);
    let (focused, hover) = f.state(if s.borrow.is_some() { Target::BarGiveBack } else { Target::BarQuit }, qr);
    if hover {
        cv.rrect(qr.x, qr.y, qr.w, qr.h, 7.0, BAD.alpha(0.18));
        // « Se déconnecter » avec son raccourci (le même pour « Quitter le partage » d'un invité).
        if s.borrow.is_none() {
            tip = Some((qr, if s.guest.is_some() { format!("{quit} ({})", crate::app::shortcut_text('D')) } else { disconnect_tooltip() }));
        }
    }
    if focused {
        widgets::ring(cv, qr, 7.0);
    }
    cv.text(qr.x + 12.0, cy + 5.0, Text::new(Font::Semibold, 13.0, BAD_TEXT), quit);

    // Les commandes passées dans « Plus » ouvrent leur menu sous lui.
    for ic in &more {
        match ic.target {
            Target::BarComputers => computers_x = more_x,
            Target::BarKeys => keys_x = more_x,
            Target::BarCamera => camera_x = more_x,
            Target::BarMouse => mouse_x = more_x,
            _ => {}
        }
    }
    // Menus déroulants.
    match s.bar_menu {
        Some(BarMenu::More) => {
            // Chaque commande avec son nom et son état (cochée quand elle est allumée) ; elle fait ce que fait son
            // icône (son menu, sa bascule) ; « Presse-papiers » sans action.
            let items: Vec<(Target, String, String, bool)> = more.iter().map(|ic| (ic.target, ic.label.clone(), String::new(), if ic.target == Target::BarClipboard { !ic.dim } else { ic.on })).collect();
            dropdown_titled(cv, f, more_x, h + 6.0, 260.0, Some(tr("Plus", "More")), &items);
        }
        Some(BarMenu::Mouse) => {
            // « Auto », « Souris du bureau », « Souris des jeux », le réglage de l'ordinateur coché.
            let current = app.mouse_setting(s);
            let items: Vec<(Target, String, String, bool)> = crate::app::MOUSE_MODES.iter().enumerate().map(|(i, (key, name))| (Target::BarMousePick(i), tr(name.0, name.1).to_string(), String::new(), *key == current)).collect();
            dropdown_titled(cv, f, mouse_x, h + 6.0, 260.0, Some(tr("Souris", "Mouse")), &items);
        }
        Some(BarMenu::Profile) => {
            let items: Vec<(Target, String, String, bool)> = Profile::ALL
                .iter()
                .map(|p| (Target::BarPick(*p), home::profile_label(*p).to_string(), crate::app::profile_phrase(Some(*p)).to_string(), *p == s.profile))
                .collect();
            dropdown(cv, f, profile_x, h + 6.0, 420.0, &items);
        }
        Some(BarMenu::Keys) => {
            let items: Vec<(Target, String, String, bool)> = shortcuts(app.session_mac(s)).iter().enumerate().map(|(i, k)| (Target::BarShortcut(i), k.name().to_string(), k.hint().to_string(), false)).collect();
            dropdown(cv, f, keys_x, h + 6.0, 300.0, &items);
        }
        Some(BarMenu::Computers) => computers_menu(cv, f, app, s, computers_x, h + 6.0),
        Some(BarMenu::Share) => share::dropdown(cv, f, app, s, share_x + 17.0, h + 6.0),
        Some(BarMenu::Lend) => lend::dropdown(cv, f, app, s, lend_x + 17.0, h + 6.0),
        Some(BarMenu::Quality) => dropdown_titled(cv, f, quality_x, h + 6.0, 300.0, Some(tr("Qualité de l'image", "Picture quality")), &quality::items(s)),
        Some(BarMenu::Screen) => dropdown_titled(cv, f, screen_x, h + 6.0, 300.0, Some(crate::app::watch_screen_word()), &quality::screen_items(s)),
        Some(BarMenu::Camera) => {
            // « Allumer la caméra » / « Éteindre la caméra », puis les webcams, celle choisie cochée.
            let toggle = if s.camera == CameraState::Off { tr("Allumer la caméra", "Turn on camera") } else { tr("Éteindre la caméra", "Turn off camera") };
            let mut items: Vec<(Target, String, String, bool)> = vec![(Target::BarCameraToggle, toggle.to_string(), String::new(), false)];
            items.extend(s.cameras.iter().enumerate().map(|(i, (path, name))| (Target::BarCameraPick(i), name.clone(), String::new(), s.camera_current.as_deref() == Some(path.as_str()))));
            dropdown_titled(cv, f, camera_x, h + 6.0, 300.0, Some(tr("Caméra", "Camera")), &items);
        }
        None => {
            // Le nom du bouton survolé, sous la barre (jamais par-dessus un menu).
            if let Some((r, name)) = tip {
                tooltip(cv, r, h + 4.0, &name);
            }
        }
    }
}

/// Le battement du rond de l'enregistrement, de 0 à 1, une pulsation par seconde du temps de l'enregistrement.
fn rec_beat(s: &SessionUi) -> f32 {
    let Some(r) = &s.recording else { return 1.0 };
    let t = (r.since.elapsed().as_secs_f32() + r.elapsed as f32).fract();
    0.5 + 0.5 * (t * std::f32::consts::TAU).cos()
}

/// L'infobulle de « Se déconnecter » : « Se déconnecter (Ctrl+Alt+Maj+D) ».
fn disconnect_tooltip() -> String {
    format!("{} ({})", tr("Se déconnecter", "Disconnect"), crate::app::shortcut_text('D'))
}

/// Le nom d'un bouton de la barre, en petite capsule sous lui.
pub(super) fn tooltip(cv: &mut Canvas, r: Rect, y: f32, name: &str) {
    let style = Text::new(Font::Regular, 12.5, FROST);
    let w = cv.text_width(style, name) + 24.0;
    let x = (r.x + r.w / 2.0 - w / 2.0).clamp(8.0, (cv.w - w - 8.0).max(8.0));
    cv.rrect(x, y, w, 26.0, 8.0, Rgba(9, 13, 18, 240));
    cv.rrect_stroke(x, y, w, 26.0, 8.0, 1.0, LINE_2);
    cv.text(x + 12.0, y + 17.5, style, name);
}

/// Une ligne du menu « Ordinateurs » : l'ordinateur, son état, ce que fait un clic.
struct ComputerRow {
    index: usize,
    label: String,
    hint: &'static str,
    dot: Rgba,
    in_session: bool,
    enabled: bool,
}

/// Vos autres ordinateurs, dans l'ordre de l'accueil (docs/link-ui.md, « Plusieurs ordinateurs ») :
/// « {nom} — En ligne », avec le point de couleur de l'état (vert en ligne, rouge hors ligne,
/// ambre pendant le réveil et pendant un redémarrage ou une mise à jour de la Station, gris
/// pendant la recherche). Ceux qui sont déjà en session sont cochés et proposent « Afficher » ;
/// les autres, ce que fait le bouton de l'accueil (se connecter, réveiller, réessayer) ; « Se
/// connecter » est inactif quand quatre ordinateurs sont déjà en session.
fn computer_rows(app: &App, s: &SessionUi) -> Vec<ComputerRow> {
    app.store
        .hosts
        .iter()
        .enumerate()
        .filter(|(_, h)| h.id != s.host)
        .map(|(index, h)| {
            let online = app.live.get(&h.id).map(|l| l.online).unwrap_or_default();
            let (state, color) = match online {
                Online::Online => (tr("En ligne", "Online"), OK),
                Online::Away => (tr("Hors de chez vous", "Away from home"), OK),
                Online::Offline => (tr("Hors ligne", "Offline"), BAD),
                Online::Unknown => (tr("Recherche…", "Searching…"), FAINT),
            };
            let busy = app.is_waking(&h.id) || app.station(&h.id).is_some_and(|st| st.restarting || st.installing);
            let in_session = app.session_of_host(&h.id).is_some();
            let (target, action, _) = home::main_action(app, h);
            let enabled = in_session || match target {
                Target::Connect => !app.sessions_full(),
                Target::Wake => !app.is_waking(&h.id),
                _ => true,
            };
            ComputerRow { index, label: format!("{} — {state}", h.name), hint: if in_session { tr("Afficher", "Show") } else { action }, dot: if busy { WARN } else { color }, in_session, enabled }
        })
        .collect()
}

/// Le menu « Ordinateurs » : une ligne par autre ordinateur ; en pied, l'aide de la limite de
/// quatre et le raccourci « Ordinateur suivant » (avec plusieurs sessions).
fn computers_menu(cv: &mut Canvas, f: &mut Frame, app: &App, s: &SessionUi, x: f32, y: f32) {
    let rows = computer_rows(app, s);
    let mut footer: Vec<String> = Vec::new();
    if app.sessions_full() {
        footer.push(tr("Quatre ordinateurs au plus en même temps.", "Four computers at most at the same time.").into());
    }
    if app.running_sessions() >= 2 {
        footer.push(format!("{} · Ctrl+Alt+{}+O", tr("Ordinateur suivant", "Next computer"), tr("Maj", "Shift")));
    }
    let (label, hint) = (Text::new(Font::Regular, 13.5, FROST), Text::new(Font::Regular, 12.0, FAINT));
    let note = Text::new(Font::Regular, 12.0, FAINT);
    let mut w = 320.0f32;
    for r in &rows {
        w = w.max(88.0 + cv.text_width(label, &r.label) + cv.text_width(hint, r.hint));
    }
    for l in &footer {
        w = w.max(cv.text_width(note, l) + 36.0);
    }
    let w = w.min(cv.w - 16.0);
    let x = x.min(cv.w - w - 8.0).max(8.0);
    let ih = 36.0;
    let foot_h = if footer.is_empty() { 0.0 } else { footer.len() as f32 * 22.0 + 8.0 };
    let h = rows.len() as f32 * ih + 12.0 + foot_h;
    cv.rrect(x, y, w, h, 10.0, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, w, h, 10.0, 1.0, LINE_2);
    let mut yy = y + 6.0;
    for r in &rows {
        let rr = Rect::new(x + 6.0, yy, w - 12.0, ih);
        let (focused, hover) = f.state(Target::BarComputer(r.index), rr);
        if (hover || focused) && r.enabled {
            cv.rrect(rr.x, rr.y, rr.w, rr.h, 7.0, Rgba(227, 237, 240, 16));
        }
        if r.in_session {
            cv.icon(&Icon::Check, rr.x + 8.0, rr.y + 10.0, 16.0, 2.0, SPECTRE);
        }
        widgets::dot(cv, rr.x + 36.0, rr.y + ih / 2.0, 3.5, r.dot, false);
        let fade = if r.enabled { 1.0 } else { 0.45 };
        let lw = cv.text_fit(rr.x + 48.0, rr.y + 23.0, label.color(FROST.alpha(fade)), &r.label, rr.w - 60.0);
        cv.text_fit(rr.x + 48.0 + lw + 14.0, rr.y + 23.0, hint.color(FAINT.alpha(fade)), r.hint, (rr.w - 60.0 - lw - 14.0).max(0.0));
        yy += ih;
    }
    for l in &footer {
        cv.text_fit(x + 18.0, yy + 16.0, note, l, w - 36.0);
        yy += 22.0;
    }
}

fn dropdown(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, w: f32, items: &[(Target, String, String, bool)]) {
    dropdown_titled(cv, f, x, y, w, None, items);
}

/// Un menu déroulant, avec son titre en capitales au-dessus de ses lignes s'il en a un.
fn dropdown_titled(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, w: f32, title: Option<&str>, items: &[(Target, String, String, bool)]) {
    let ih = 36.0;
    let head = if title.is_some() { 30.0 } else { 0.0 };
    // Borné à la fenêtre : à droite, en largeur, et en hauteur (les lignes qui ne tiennent pas ne sont pas montrées).
    let (x, w, shown) = dropdown_place(cv.w, cv.h, x, y, w, head, items.len());
    let items = &items[..shown];
    let h = items.len() as f32 * ih + 12.0 + head;
    cv.rrect(x, y, w, h, 10.0, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, w, h, 10.0, 1.0, LINE_2);
    if let Some(t) = title {
        eyebrow(cv, x + 16.0, y + 24.0, 10.5, FAINT, t);
    }
    let mut yy = y + 6.0 + head;
    for (t, label, hint, on) in items {
        let r = Rect::new(x + 6.0, yy, w - 12.0, ih);
        let (focused, hover) = f.state(*t, r);
        if hover || focused {
            cv.rrect(r.x, r.y, r.w, r.h, 7.0, Rgba(227, 237, 240, 16));
        }
        if *on {
            cv.icon(&Icon::Check, r.x + 8.0, r.y + 10.0, 16.0, 2.0, SPECTRE);
        }
        // Auto : les étincelles après le nom.
        let mut lw = cv.text_fit(r.x + 32.0, r.y + 23.0, Text::new(Font::Regular, 13.5, FROST), label, (r.w - 44.0).max(0.0));
        if let Some(icon) = widgets::profile_icon(*t) {
            cv.icon(&icon, r.x + 32.0 + lw + 6.0, r.y + 10.0, 15.0, 1.4, SPECTRE);
            lw += 22.0;
        }
        if !hint.is_empty() {
            cv.text_fit(r.x + 32.0 + lw + 10.0, r.y + 23.0, Text::new(Font::Regular, 12.0, FAINT), hint, (r.w - lw - 50.0).max(0.0));
        }
        yy += ih;
    }
}

/// Capsule en haut (bureau) ou en bas (manette) : un point (ambre pendant une attente) ou une icône avant le texte.
fn capsule(app: &App, cv: &mut Canvas, msg: &str, mark: widgets::Mark, drop: f32) {
    let u = unit(app.layout);
    let y = if app.layout == Layout::Desktop { 70.0 + drop } else { cv.h - 90.0 * u };
    widgets::capsule(cv, y, 34.0 * u, Text::new(Font::Regular, 13.0 * u, MUTED), msg, mark, 1.0);
}

// MARK: - PyroWave

/// La hauteur du bandeau PyroWave, en points.
const PW_BANNER_H: f32 = 62.0;

/// Le bandeau du début d'une session PyroWave en Wi-Fi ou par le relais (docs/link-ui.md, « PyroWave ») : en haut,
/// sous la barre ; titre (la puce, ou « Le réseau sature » sur l'avis du cœur), « L'image peut saccader. », « Passer en HEVC » et la croix. Au Deck et au salon
/// aussi (le toucher le prend ; à la manette, la tuile du menu fait de même) ; il part seul au bout de 8 s.
fn pyrowave_banner(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let Some(chip) = s.pw_banner_title() else { return };
    let u = unit(app.layout);
    let title = Text::new(Font::Semibold, 14.0 * u, FROST);
    let body = Text::new(Font::Regular, 13.0 * u, MUTED);
    let bh = 34.0 * u;
    let pad = 16.0 * u;
    let text_w = cv.text_width(title, chip).max(cv.text_width(body, crate::pyrowave::banner_text()));
    let button_w = cv.text_width(Text::new(Font::Semibold, bh * 0.32, FROST), crate::pyrowave::switch_to_hevc()) + 2.0 * bh * 0.58;
    let close = 28.0 * u;
    let w = (pad + 10.0 * u + text_w + 20.0 * u + button_w + 10.0 * u + close + pad).min(cv.w - 32.0 * u);
    let h = PW_BANNER_H * u;
    let x = (cv.w - w) / 2.0;
    let y = if app.layout == Layout::Desktop { BAR_H + 22.0 } else { 28.0 * u };
    cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 235));
    cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, WARN.alpha(0.55));
    // Le liseré ambre, à gauche.
    cv.rrect(x + 1.0, y + 12.0 * u, 3.0 * u, h - 24.0 * u, 1.5 * u, WARN);
    let tx = x + pad + 10.0 * u;
    cv.text(tx, y + 25.0 * u, title, chip);
    cv.text(tx, y + 45.0 * u, body, crate::pyrowave::banner_text());
    let bx = x + w - pad - close - 10.0 * u - button_w;
    widgets::primary(cv, f, Target::PwHevc, bx, y + (h - bh) / 2.0, bh, crate::pyrowave::switch_to_hevc(), false);
    let cr = Rect::new(x + w - pad - close, y + (h - close) / 2.0, close, close);
    let (focused, hover) = f.state(Target::PwClose, cr);
    if focused {
        widgets::ring(cv, cr, close / 2.0);
    }
    if hover {
        cv.rrect(cr.x, cr.y, cr.w, cr.h, close / 2.0, Rgba(227, 237, 240, 14));
    }
    cv.icon(&Icon::Close, cr.x + 6.0 * u, cr.y + 6.0 * u, close - 12.0 * u, 1.6, MUTED);
}

/// Le bandeau « Autre réseau que celui du test » (profil Auto, docs/link-ui.md) : en haut, sous la barre ; le titre,
/// « Le débit adaptatif s'en charge. », « Refaire le test » (le panneau s'ouvre après la session) et la croix. Discret :
/// liseré Spectre, bouton à contour. Il part seul au bout de 8 s.
fn auto_banner(app: &App, cv: &mut Canvas, f: &mut Frame) {
    use crate::autotest as auto;
    let u = unit(app.layout);
    let title = Text::new(Font::Semibold, 14.0 * u, FROST);
    let body = Text::new(Font::Regular, 13.0 * u, MUTED);
    let bh = 34.0 * u;
    let pad = 16.0 * u;
    let size = 13.0 * u;
    let text_w = cv.text_width(title, auto::other_network()).max(cv.text_width(body, auto::other_network_text()));
    let button_w = cv.text_width(Text::new(Font::Regular, size, FROST), auto::again()) + 2.0 * size;
    let close = 28.0 * u;
    let w = (pad + 10.0 * u + text_w + 20.0 * u + button_w + 10.0 * u + close + pad).min(cv.w - 32.0 * u);
    let h = super::autotest::BANNER_H * u;
    let x = (cv.w - w) / 2.0;
    let y = if app.layout == Layout::Desktop { BAR_H + 22.0 } else { 28.0 * u };
    cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 235));
    cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, LINE_2);
    cv.rrect(x + 1.0, y + 12.0 * u, 3.0 * u, h - 24.0 * u, 1.5 * u, SPECTRE);
    let tx = x + pad + 10.0 * u;
    cv.text(tx, y + 25.0 * u, title, auto::other_network());
    cv.text(tx, y + 45.0 * u, body, auto::other_network_text());
    let bx = x + w - pad - close - 10.0 * u - button_w;
    widgets::ghost(cv, f, Target::AutoBannerRetest, bx, y + (h - bh) / 2.0, bh, size, None, auto::again(), FROST);
    let cr = Rect::new(x + w - pad - close, y + (h - close) / 2.0, close, close);
    let (focused, hover) = f.state(Target::AutoBannerClose, cr);
    if focused {
        widgets::ring(cv, cr, close / 2.0);
    }
    if hover {
        cv.rrect(cr.x, cr.y, cr.w, cr.h, close / 2.0, Rgba(227, 237, 240, 14));
    }
    cv.icon(&Icon::Close, cr.x + 6.0 * u, cr.y + 6.0 * u, close - 12.0 * u, 1.6, MUTED);
}

// MARK: - Statistiques (par-dessus l'image, à la demande)

/// Le lieu d'une étape du trajet d'une image (docs/link-ui.md, « Statistiques »).
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Computer,
    Network,
    Device,
}

impl Place {
    fn label(self) -> &'static str {
        match self {
            Place::Computer => tr("Sur l'ordinateur", "On the computer"),
            Place::Network => tr("Réseau", "Network"),
            Place::Device => tr("Sur cet appareil", "On this device"),
        }
    }
}

/// Les six étapes : nom, lieu, aide (l'ordre de `Stats::stages`).
fn stages() -> [(&'static str, Place, &'static str); 6] {
    [
        (tr("Capture", "Capture"), Place::Computer, tr("De la capture de l'image au début de son encodage", "From capturing the frame to the start of its encoding")),
        (tr("Encodage", "Encoding"), Place::Computer, tr("Compression de l'image par la carte graphique de l'ordinateur", "Frame compression by the computer's graphics card")),
        (tr("Envoi", "Sending"), Place::Computer, tr("Mise en paquets et départ sur le réseau", "Packetising and sending over the network")),
        (tr("Réseau", "Network"), Place::Network, tr("Du départ de l'ordinateur à l'image complète reçue ici", "From leaving the computer to the full frame received here")),
        (tr("Décodage", "Decoding"), Place::Device, tr("Décompression de l'image par cet appareil", "Frame decompression by this device")),
        (tr("Affichage", "Display"), Place::Device, tr("Rendu et attente du prochain rafraîchissement de l'écran", "Rendering and waiting for the display's next refresh")),
    ]
}

/// Une durée du trajet, « — » sans mesure.
fn stage_ms(v: Option<f64>) -> String {
    v.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "—".into())
}

/// Le panneau des statistiques : son coin, sa largeur de base, et le haut du trajet d'une image.
struct StatsBox {
    x: f32,
    y: f32,
    w: f32,
    u: f32,
    /// Haut de la rubrique « Trajet d'une image ».
    path: f32,
}

/// La tête du panneau : le titre et sa croix, la latence en grand et « de la capture à votre écran ».
const STATS_HEAD: f32 = 104.0;

/// Cascade (bureau, Deck) : hauteur d'une étiquette de lieu et d'une étape, largeurs du nom et de la valeur.
const PLACE_H: f32 = 16.0;
const STAGE_H: f32 = 18.0;
const STAGE_NAME_W: f32 = 78.0;
const STAGE_VALUE_W: f32 = 58.0;

impl StatsBox {
    fn new(layout: Layout) -> Self {
        let u = unit(layout);
        let y = if layout == Layout::Desktop { 24.0 } else { 24.0 * u };
        // La cascade et les lignes du bas (« 2560 × 1440 · HEVC · matériel · 120 i/s ») demandent un peu de largeur.
        let w = if layout == Layout::Tv { 300.0 * u } else { 400.0 };
        Self { x: 24.0 * u, y, w, u, path: y + STATS_HEAD * u }
    }

    /// Le haut de la ligne de l'étape `i` dans la cascade (une étiquette de lieu avant chaque nouveau lieu).
    fn stage_top(&self, i: usize) -> f32 {
        let places = match i {
            0..=2 => 1.0,
            3 => 2.0,
            _ => 3.0,
        };
        self.path + 22.0 + STAGE_H * i as f32 + PLACE_H * places
    }

    /// Le bas de la cascade.
    fn path_end(&self) -> f32 {
        self.stage_top(5) + STAGE_H
    }
}

/// L'étape du trajet d'une image sous le pointeur (`ux`, `uy` en points), pour son aide : bureau et Deck, statistiques
/// ouvertes ; au salon, pas d'aide.
pub fn stats_stage_at(app: &App, s: &SessionUi, ux: f32, uy: f32) -> Option<usize> {
    if !s.show_stats || s.menu || s.keys || app.layout == Layout::Tv {
        return None;
    }
    let b = StatsBox::new(app.layout);
    if ux < b.x || ux >= b.x + b.w {
        return None;
    }
    (0..6).find(|&i| {
        let top = b.stage_top(i);
        uy >= top && uy < top + STAGE_H
    })
}

/// Le trajet d'une image en cascade (bureau, Deck) : chaque étape commence où la précédente finit, sur une échelle
/// commune ; le pointillé marque la durée d'une image ; les valeurs à droite.
fn frame_path_waterfall(cv: &mut Canvas, b: &StatsBox, st: &crate::app::Stats) {
    let stages = stages();
    let ix = b.x + 16.0;
    let right = b.x + b.w - 16.0;
    eyebrow(cv, ix, b.path + 10.0, 10.5, FAINT, tr("Trajet d'une image", "Path of a frame"));
    let budget = 1000.0 / st.fps.max(30.0);
    let total: f64 = st.stages.iter().flatten().sum();
    let scale = total.max(budget) * 1.08;
    let bar_x = ix + STAGE_NAME_W + 10.0;
    let bar_w = right - STAGE_VALUE_W - 10.0 - bar_x;
    let mut start = 0.0;
    for (i, (name, place, _)) in stages.iter().enumerate() {
        let top = b.stage_top(i);
        if i == 0 || stages[i - 1].1 != *place {
            cv.text(ix, top - PLACE_H + 11.0, Text::new(Font::MonoMedium, 9.0, GHOST).tracking(0.8), &place.label().to_uppercase());
        }
        let ms = st.stages[i];
        let cy = top + STAGE_H / 2.0;
        cv.text_fit(ix, top + 13.0, Text::new(Font::Regular, 12.0, if ms.is_some() { MUTED } else { FAINT }), name, STAGE_NAME_W);
        cv.rect(bar_x, cy.round(), bar_w, 1.0, Rgba(255, 255, 255, 13));
        if let Some(v) = ms {
            let x0 = bar_x + bar_w * (start / scale) as f32;
            let len = (bar_w * (v / scale) as f32).max(2.0);
            cv.rrect(x0, cy - 4.0, len, 8.0, 2.0, SPECTRE);
            start += v;
        }
        cv.text(right, top + 13.0, Text::new(Font::Mono, 11.5, if ms.is_some() { FROST } else { GHOST }).align(Align::Right), &stage_ms(ms));
    }
    // Le repère : une image au débit du moment, en pointillé à travers les étapes.
    let gx = (bar_x + bar_w * (budget / scale) as f32).round();
    let (top, bottom) = (b.stage_top(0), b.path_end());
    let mut yy = top;
    while yy < bottom {
        cv.rect(gx, yy, 1.0, 2.0f32.min(bottom - yy), Rgba(255, 255, 255, 102));
        yy += 5.0;
    }
}

/// L'aide d'une étape, en capsule à droite du panneau, à hauteur de sa ligne.
fn stage_help(cv: &mut Canvas, b: &StatsBox, i: usize) {
    let help = stages()[i].2;
    let style = Text::new(Font::Regular, 12.5, FROST);
    let w = cv.text_width(style, help) + 24.0;
    let x = (b.x + b.w + 8.0).min((cv.w - w - 8.0).max(8.0));
    let y = b.stage_top(i) - 4.0;
    cv.rrect(x, y, w, 26.0, 8.0, Rgba(9, 13, 18, 240));
    cv.rrect_stroke(x, y, w, 26.0, 8.0, 1.0, LINE_2);
    cv.text(x + 12.0, y + 17.5, style, help);
}

/// Le trajet d'une image au salon : les six valeurs en une rangée lisible de loin, regroupées par lieu, avec le
/// total de chaque lieu ; pas d'aide. Rend la largeur du panneau qu'il lui faut ; `draw` faux : mesure seulement.
fn frame_path_row(cv: &mut Canvas, b: &StatsBox, st: &crate::app::Stats, draw: bool) -> f32 {
    let u = b.u;
    let stages = stages();
    let ix = b.x + 16.0 * u;
    let put = |cv: &mut Canvas, x: f32, y: f32, style: Text, s: &str| if draw { cv.text(x, y, style, s) } else { cv.text_width(style, s) };
    if draw {
        eyebrow(cv, ix, b.path + 10.0 * u, 10.5 * u, FAINT, tr("Trajet d'une image", "Path of a frame"));
    }
    let place_style = Text::new(Font::Mono, 10.0 * u, FAINT).tracking(0.8 * u);
    let total_style = Text::new(Font::MonoMedium, 22.0 * u, FROST);
    let name_style = Text::new(Font::Regular, 12.0 * u, FAINT);
    let value_style = Text::new(Font::MonoMedium, 16.0 * u, FROST);
    let (col_gap, group_gap) = (18.0 * u, 28.0 * u);
    let (y_place, y_total, y_name, y_value) = (b.path + 34.0 * u, b.path + 64.0 * u, b.path + 88.0 * u, b.path + 112.0 * u);
    // Des colonnes à la largeur d'une valeur à deux chiffres : la rangée ne bouge pas d'une seconde à l'autre.
    let sample = format!("{} ms", decimal(88.8, 1));
    let (min_value, min_total) = (cv.text_width(value_style, &sample), cv.text_width(total_style, &sample));
    let mut x = ix;
    for (g, place) in [Place::Computer, Place::Network, Place::Device].into_iter().enumerate() {
        let idx: Vec<usize> = (0..6).filter(|&i| stages[i].1 == place).collect();
        let known: Vec<f64> = idx.iter().filter_map(|&i| st.stages[i]).collect();
        let total = (!known.is_empty()).then(|| known.iter().sum::<f64>());
        if g > 0 && draw {
            cv.rect((x - group_gap / 2.0).round(), y_place - 10.0 * u, 1.0, y_value - y_place + 16.0 * u, LINE_2);
        }
        let label = place.label().to_uppercase();
        let mut group_w = put(cv, x, y_place, place_style, &label);
        let total_text = stage_ms(total);
        group_w = group_w.max(put(cv, x, y_total, if total.is_some() { total_style } else { total_style.color(GHOST) }, &total_text)).max(min_total);
        let mut cx = x;
        for (n, &i) in idx.iter().enumerate() {
            let ms = st.stages[i];
            let value = stage_ms(ms);
            let nw = put(cv, cx, y_name, name_style, stages[i].0);
            let vw = put(cv, cx, y_value, if ms.is_some() { value_style } else { value_style.color(GHOST) }, &value);
            cx += nw.max(vw).max(min_value) + if n + 1 < idx.len() { col_gap } else { 0.0 };
        }
        group_w = group_w.max(cx - x);
        x += group_w + group_gap;
    }
    x - group_gap - b.x + 16.0 * u
}

/// Hauteur de la rangée du salon, depuis le haut de la rubrique.
const ROW_H: f32 = 124.0;

/// Place de la courbe du débit dans le panneau (courbe et marge).
const BITRATE_GRAPH_H: f32 = 40.0;

/// La courbe du débit sur les 30 dernières secondes, et le trait du débit visé seconde par seconde (rien là où la
/// seconde n'en a pas) ; l'échelle prend le plus haut des deux.
fn bitrate_graph(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, hist: &[(f64, Option<f64>)]) {
    let max = hist.iter().map(|(m, t)| m.max(t.unwrap_or(0.0))).fold(1.0f64, f64::max) * 1.1;
    let n = hist.len().max(2) - 1;
    let px = |i: usize| x + i as f32 * w / n as f32;
    let py = |v: f64| y + h - (v / max) as f32 * h;
    cv.rect(x, y + h, w, 1.0, LINE);
    // Le débit visé : un trait par suite de secondes qui en ont un.
    let mut run: Vec<(f32, f32)> = Vec::new();
    for (i, (_, t)) in hist.iter().enumerate() {
        match t {
            Some(t) => run.push((px(i), py(*t))),
            None => {
                if run.len() >= 2 {
                    cv.polyline(&run, 1.0, SPECTRE.alpha(0.7));
                }
                run.clear();
            }
        }
    }
    if run.len() >= 2 {
        cv.polyline(&run, 1.0, SPECTRE.alpha(0.7));
    }
    let pts: Vec<(f32, f32)> = hist.iter().enumerate().map(|(i, (m, _))| (px(i), py(*m))).collect();
    cv.polyline(&pts, 1.5, FROST);
}

/// Une ligne du bas du panneau : son libellé, sa valeur, sa couleur.
type StatsLine = (String, String, Rgba);

/// Les lignes du bas des Statistiques (docs/link-ui.md, « Statistiques ») : « Pertes · 30 s » (images perdues,
/// paquets perdus, images clés, la session), PyroWave, puis « Chemin », « Réseau », « Image », « Mesures », « Son »,
/// la caméra.
fn stats_lines(s: &SessionUi, st: &crate::app::Stats) -> (Vec<StatsLine>, Vec<StatsLine>) {
    let (frames, packets, keys) = st.losses.iter().fold((0u32, 0u32, 0u32), |a, l| (a.0 + l.0, a.1 + l.1, a.2 + l.2));
    let warn = |n: u32| if n > 0 { WARN } else { FROST };
    let mut losses = vec![
        (tr("Images perdues", "Lost frames").to_string(), frames.to_string(), warn(frames)),
        (tr("Paquets perdus", "Lost packets").to_string(), packets.to_string(), warn(packets)),
        (tr("Images clés", "Keyframes").to_string(), keys.to_string(), FROST),
    ];
    if st.decoder == "PyroWave" {
        let lost = crate::pyrowave::lost_value(st.lost_frames, st.fps.round() as u32);
        losses.push((crate::pyrowave::lost_label().to_string(), lost, if st.lost_frames > 0 { WARN } else { FROST }));
    }
    let dash = || "—".to_string();
    let ms = |v: f64| format!("{} ms", decimal(v, 1));
    let mut rest = Vec::new();
    // « Chemin » : Direct, Par le relais, Par le relais (TCP).
    rest.push((tr("Chemin", "Path").to_string(), s.path().label().to_string(), FROST));
    // « Réseau » : l'aller-retour ; par le relais, celui de l'app au relais et du relais à l'ordinateur.
    let rtt = s.route.and_then(|r| r.total_ms()).filter(|_| s.via_relay()).or(st.rtt_ms);
    rest.push((tr("Réseau", "Network").to_string(), rtt.map(|v| format!("{} {}", tr("aller-retour", "round trip"), ms(v))).unwrap_or_else(dash), FROST));
    rest.push((tr("Image", "Picture").to_string(), image_line(st), FROST));
    rest.push((tr("Mesures", "Measures").to_string(), st.measured.map(|p| format!("{:.0} {}", p, tr("% des images", "% of frames"))).map(|t| if crate::i18n::english() { t.replacen(" %", "%", 1) } else { t }).unwrap_or_else(dash), FROST));
    let sound = st.audio.map(|(buffer, output)| format!("{} {} · {} {}", tr("tampon", "buffer"), decimal(buffer, 0) + " ms", tr("sortie", "output"), decimal(output, 0) + " ms"));
    rest.push((tr("Son", "Sound").to_string(), sound.unwrap_or_else(dash), FROST));
    // La caméra de l'appareil, allumée : « 1280 × 720 · 30 i/s · H.264 · 1,4 Mbit/s ».
    if let Some(line) = &s.camera_line {
        rest.push((tr("Caméra", "Camera").to_string(), line.clone(), FROST));
    }
    (losses, rest)
}

/// « 2560 × 1440 · HEVC · matériel · 120 i/s » : la définition, le format tel que l'appareil le décode (matériel ou
/// processeur), les images par seconde ; « HDR » après la définition quand l'image l'est.
fn image_line(st: &crate::app::Stats) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !st.mode.is_empty() {
        parts.push(if st.hdr { format!("{} · HDR", st.mode) } else { st.mode.clone() });
    }
    let d = st.decoder.as_str();
    let (format, how) = match d.split_once(' ') {
        _ if d.is_empty() => (String::new(), ""),
        _ if d == "PyroWave" => ("PyroWave".to_string(), tr("matériel", "hardware")),
        Some(("CPU", codec)) => (codec.to_string(), tr("processeur", "processor")),
        Some((_, codec)) => (codec.to_string(), tr("matériel", "hardware")),
        None => (d.to_string(), ""),
    };
    if !format.is_empty() {
        parts.push(format);
    }
    if !how.is_empty() {
        parts.push(how.to_string());
    }
    if st.fps > 0.0 {
        parts.push(format!("{:.0} {}", st.fps, tr("i/s", "fps")));
    }
    if parts.is_empty() { "—".into() } else { parts.join(" · ") }
}

/// La courbe de la latence sur les 30 dernières secondes, avec le repère « 1 image » (la durée d'une image au rythme du
/// moment) en pointillé.
fn latency_graph(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, hist: &[f64], frame_ms: f64, u: f32) {
    let max = hist.iter().cloned().fold(frame_ms * 1.2, f64::max).max(1.0) * 1.1;
    // Le repère « 1 image » à droite de la courbe, à la hauteur de son pointillé.
    let style = Text::new(Font::Mono, 9.5 * u, GHOST).align(Align::Right);
    let label = tr("1 image", "1 frame");
    let label_w = cv.text_width(style, label);
    let w = w - label_w - 8.0 * u;
    cv.rect(x, y + h, w, 1.0, LINE);
    let fy = (y + h - (frame_ms / max) as f32 * h).round();
    let mut xx = x;
    while xx < x + w {
        cv.rect(xx, fy, 3.0f32.min(x + w - xx), 1.0, Rgba(255, 255, 255, 102));
        xx += 7.0;
    }
    cv.text(x + w + 8.0 * u + label_w, fy + 3.5 * u, style, label);
    if hist.len() >= 2 {
        let n = hist.len() - 1;
        let pts: Vec<(f32, f32)> = hist.iter().enumerate().map(|(i, v)| (x + i as f32 * w / n as f32, y + h - (*v / max) as f32 * h)).collect();
        cv.polyline(&pts, 1.5, SPECTRE);
    }
}

/// Le panneau « Statistiques » (docs/link-ui.md) : le titre, sa croix « Fermer les statistiques » et le raccourci ; la
/// latence en grand, « de la capture à votre écran » ; le trajet d'une image ; « Latence » et « Débit » sur 30 s (le
/// repère « 1 image », le trait et la ligne du débit visé) ; « Pertes · 30 s » et la session ; puis le chemin, le
/// réseau, l'image, les mesures, le son. Avant la première image, une phrase seulement.
fn stats(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let st = s.stats.clone().unwrap_or_default();
    let tv = app.layout == Layout::Tv;
    let b = StatsBox::new(app.layout);
    let (x, y, u) = (b.x, b.y, b.u);
    let mono = Text::new(Font::Mono, 11.5 * u, MUTED);
    let line_h = 19.0 * u;
    // Au salon, la rangée du trajet donne sa largeur au panneau : mesurée d'abord.
    let w = if tv { b.w.max(frame_path_row(cv, &b, &st, false)) } else { b.w };
    let ix = x + 16.0 * u;
    let inner = w - 32.0 * u;
    let head = |cv: &mut Canvas, f: &mut Frame, w: f32| {
        eyebrow(cv, ix, y + 26.0 * u, 11.0 * u, FAINT, tr("Statistiques", "Statistics"));
        cv.text(x + w - 46.0 * u, y + 26.0 * u, Text::new(Font::Mono, 10.0 * u, GHOST).align(Align::Right), &crate::app::shortcut_text('S'));
        let cr = Rect::new(x + w - 38.0 * u, y + 10.0 * u, 26.0 * u, 26.0 * u);
        let (focused, hover) = f.state(Target::StatsClose, cr);
        if hover || focused {
            cv.rrect(cr.x, cr.y, cr.w, cr.h, 6.0 * u, Rgba(227, 237, 240, 16));
        }
        if focused {
            widgets::ring(cv, cr, 6.0 * u);
        }
        cv.icon(&Icon::Close, cr.x + 6.0 * u, cr.y + 6.0 * u, cr.w - 12.0 * u, 1.6, MUTED);
        if hover {
            tooltip(cv, cr, cr.y + cr.h + 4.0 * u, tr("Fermer les statistiques", "Close statistics"));
        }
    };
    // Avant la première image : la phrase de link-ui.
    if s.stats.is_none() || (st.latency_ms.is_none() && st.fps <= 0.0) {
        let h = 104.0 * u;
        cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 214));
        cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, LINE);
        cv.paragraph(ix, y + 58.0 * u, Text::new(Font::Regular, 13.0 * u, MUTED), tr("Les statistiques s'affichent dès que l'image arrive.", "Statistics appear as soon as the picture arrives."), inner, 19.0 * u);
        head(cv, f, w);
        return;
    }
    let end = if tv { b.path + ROW_H * u } else { b.path_end() };
    let lowered_lines = st.lowered.as_deref().map_or(0, |t| cv.line_count(mono, t, inner));
    let (losses, rest) = stats_lines(s, &st);
    let graph_h = BITRATE_GRAPH_H * u;
    let graphs = (line_h + graph_h + 8.0 * u) * 2.0 + if st.target.is_some() { line_h } else { 0.0 };
    let col1 = end - y + 22.0 * u + graphs + 6.0 * u;
    let col2 = line_h * (2.0 + losses.len() as f32 + lowered_lines as f32) + 8.0 * u + line_h * rest.len() as f32;
    // Trop haut pour la fenêtre (bureau, 960 × 600) : les pertes et les lignes du bas passent dans une seconde colonne,
    // à droite du trajet.
    let two = !tv && y + col1 + col2 + 16.0 * u > cv.h - 16.0 * u;
    let (h, w_all) = if two { (col1.max(b.path - y + 10.0 * u + col2) + 16.0 * u, 2.0 * w) } else { (col1 + col2 + 16.0 * u, w) };
    let w = w_all;
    let inner = if two { w / 2.0 - 32.0 * u } else { inner };
    cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 214));
    cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, LINE);
    head(cv, f, w);
    // La latence en grand, et ce qu'elle mesure.
    let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "— ms".into());
    let lw = cv.text(ix, y + 68.0 * u, Text::new(Font::MonoMedium, 26.0 * u, SPECTRE), &lat);
    cv.text_fit(ix + lw + 12.0 * u, y + 66.0 * u, Text::new(Font::Regular, 12.5 * u, MUTED), tr("de la capture à votre écran", "from capture to your screen"), (inner - lw - 12.0 * u).max(0.0));
    if tv {
        frame_path_row(cv, &b, &st, true);
    } else {
        frame_path_waterfall(cv, &b, &st);
    }
    let mut yy = end + 22.0 * u;
    // « Latence » et « Débit », 30 s.
    eyebrow(cv, ix, yy, 10.5 * u, FAINT, tr("Latence", "Latency"));
    latency_graph(cv, ix, yy + 6.0 * u, inner, graph_h - 10.0 * u, &st.history, 1000.0 / st.fps.max(1.0), u);
    yy += line_h + graph_h + 8.0 * u;
    eyebrow(cv, ix, yy, 10.5 * u, FAINT, tr("Débit", "Bitrate"));
    cv.text(ix + inner, yy, mono.align(Align::Right), &format!("{:.0} Mbit/s", st.mbps));
    if st.bitrate_history.len() >= 2 {
        bitrate_graph(cv, ix, yy + 6.0 * u, inner, graph_h - 10.0 * u, &st.bitrate_history);
    }
    yy += line_h + graph_h + 8.0 * u;
    if let Some((label, text)) = &st.target {
        let colon = if crate::i18n::english() { ":" } else { " :" };
        cv.text_fit(ix, yy, mono, &format!("{label}{colon} {text}"), inner);
        yy += line_h;
    }
    yy += 6.0 * u;
    // La seconde colonne, à droite du trajet.
    let ix = if two { x + w / 2.0 + 16.0 * u } else { ix };
    if two {
        cv.rect((x + w / 2.0).round(), b.path, 1.0, h - (b.path - y) - 16.0 * u, LINE);
        yy = b.path + 10.0 * u;
    }
    // « Pertes · 30 s », puis la session.
    eyebrow(cv, ix, yy, 10.5 * u, FAINT, tr("Pertes · 30 s", "Losses · 30 s"));
    yy += line_h;
    // Dans chaque bloc, les valeurs commencent après le plus long de ses libellés.
    let label_w = |cv: &mut Canvas, lines: &[StatsLine]| lines.iter().map(|(l, _, _)| cv.text_width(mono, l)).fold(0.0f32, f32::max).min(inner * 0.45) + 14.0 * u;
    let row = |cv: &mut Canvas, yy: f32, label_w: f32, (label, value, color): &StatsLine| {
        cv.text_fit(ix, yy, mono, label, label_w - 14.0 * u);
        cv.text_shrink(ix + label_w, yy, mono.color(*color), value, inner - label_w);
    };
    let lw = label_w(cv, &losses);
    for l in &losses {
        row(cv, yy, lw, l);
        yy += line_h;
    }
    let (sf, sp) = st.session_lost;
    let session = if crate::i18n::english() { format!("Session: {sf} frames, {sp} packets") } else { format!("Session : {sf} images, {sp} paquets") };
    cv.text_fit(ix, yy, mono.color(FAINT), &session, inner);
    yy += line_h;
    if let Some(text) = &st.lowered {
        cv.paragraph(ix, yy, mono.color(WARN), text, inner, line_h);
        yy += lowered_lines as f32 * line_h;
    }
    yy += 8.0 * u;
    let lw = label_w(cv, &rest);
    for l in &rest {
        row(cv, yy, lw, l);
        yy += line_h;
    }
    // L'aide de l'étape survolée (bureau, Deck).
    if !tv && let Some(i) = s.stats_help.filter(|&i| i < 6) {
        stage_help(cv, &b, i);
    }
}

// MARK: - Menu à la manette

fn menu(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    if s.console.is_some() {
        return super::ps::menu(app, cv, f, s);
    }
    let u = unit(app.layout);
    let tv = app.layout == Layout::Tv;
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 184));
    let x0 = if tv { 96.0 } else { 48.0 };
    // Santé du flux.
    let head = 88.0 * u - if tv { 4.0 } else { 0.0 };
    cv.rect(0.0, 0.0, w, head, Rgba(5, 7, 10, 128));
    cv.rect(0.0, head - 1.0, w, 1.0, LINE);
    let st = s.stats.clone().unwrap_or_default();
    cv.glow(x0 + 14.0 * u, head / 2.0, 22.0 * u, 22.0 * u, HALO.alpha(0.5));
    cv.mark(x0, head / 2.0 - 14.0 * u, 28.0 * u, FROST, SPECTRE, Some(CORE));
    let mut x = x0 + 28.0 * u + 40.0 * u;
    let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "—".into());
    let lost_color = if st.lost == 0 { OK } else { WARN };
    let cells: Vec<(String, &str, Rgba)> = vec![
        (lat, tr("Latence", "Latency"), SPECTRE),
        (format!("{:.0} {}", st.fps, tr("i/s", "fps")), tr("Images", "Frames"), FROST),
        (format!("{:.0} Mbit/s", st.mbps), tr("Débit", "Bitrate"), FROST),
        (if st.decoder.is_empty() { "—".into() } else { st.decoder.clone() }, tr("Décodeur", "Decoder"), FROST),
        (home::profile_label(s.profile).into(), tr("Profil", "Profile"), FROST),
        (format!("{}", st.lost), tr("Perdues · 10 s", "Lost · 10 s"), lost_color),
    ];
    // Un invité n'a pas de profil : la cellule n'y est pas.
    let cells: Vec<_> = cells.into_iter().filter(|c| !s.visitor() || c.1 != tr("Profil", "Profile")).collect();
    for (v, k, c) in &cells {
        let vw = cv.text(x, head / 2.0 + 2.0 * u, Text::new(Font::MonoMedium, 20.0 * u, *c), v);
        let kw = eyebrow(cv, x, head / 2.0 + 22.0 * u, 11.0 * u, FAINT, k);
        x += vw.max(kw) + 40.0 * u;
        if x > w - 300.0 * u {
            break;
        }
    }
    let mode = if st.mode.is_empty() { "—".to_string() } else { st.mode.clone() };
    let vw = cv.text(w - x0, head / 2.0 + 2.0 * u, Text::new(Font::MonoMedium, 20.0 * u, FROST).align(Align::Right), &mode);
    let _ = vw;
    if !s.visitor() {
        let k = tr("Écran virtuel", "Virtual display");
        let kw = cv.text_width(Text::new(Font::Mono, 11.0 * u, FAINT).tracking(0.88 * u), &k.to_uppercase());
        eyebrow(cv, w - x0 - kw, head / 2.0 + 22.0 * u, 11.0 * u, FAINT, k);
    }

    // Les tuiles : cinq par rangée.
    let gap = 14.0 * u;
    let tw = ((w - 2.0 * x0 - 4.0 * gap) / 5.0).min(if tv { 330.0 } else { 216.0 * 1.2 });
    let th = if tv { 170.0 } else { 118.0 };
    let others: Vec<Profile> = Profile::ALL.into_iter().filter(|p| *p != s.profile).collect();
    let audio = match st.audio.map(|(buffer, output)| buffer + output) {
        Some(a) => format!("{} · {} ms", if s.muted { tr("coupé", "muted") } else { tr("actif", "on") }, decimal(a, 0)),
        None => if s.muted { tr("coupé", "muted").into() } else { tr("sur cet appareil", "on this device").into() },
    };
    let on_off = |b: bool| if b { tr("Actif", "On") } else { tr("Inactif", "Off") };
    let row2_keyboard = if app.layout == Layout::Tv { tr("Taper du texte", "Type text") } else { tr("Clavier", "Keyboard") };
    // Les tuiles de l'hôte ; un invité n'a ni profil, ni « Alléger », ni « Partager », et le clavier, la souris
    // et la manette en souris seulement quand ils lui sont donnés (KyberNow Share).
    // Un emprunt est un visiteur aussi : ni profil, ni « Alléger », ni « Partager », ni « Prêter l'ordinateur… ».
    let guest = s.visitor();
    let kbm = s.keyboard_mouse();
    let mut tiles: Vec<(MenuItem, String, String, bool, bool)> = vec![(MenuItem::Resume, tr("Reprendre", "Resume").into(), tr("Retour au jeu", "Back to the game").into(), false, false)];
    if !guest {
        for p in &others {
            tiles.push((MenuItem::Profile(*p), home::profile_label(*p).into(), tr("Changer de profil", "Change profile").into(), false, false));
        }
    }
    // « HDR », cochable, près du profil (écran HDR ; hôte et emprunt) : le changer relance l'image.
    if let Some(u) = s.hdr {
        tiles.push((MenuItem::Hdr, "HDR".into(), on_off(u.lit()).into(), u.lit(), false));
    }
    tiles.push((MenuItem::Sound, tr("Son", "Sound").into(), audio, s.muted, false));
    // « Micro » (un micro sur l'appareil) : « Envoyé », « Coupé », « Envoi… » ; estompé, il dit sa raison.
    let mic = s.mic.as_ref().filter(|m| m.shown);
    if let Some(m) = mic {
        tiles.push((MenuItem::Mic, crate::micro::label_on().into(), m.tile.clone(), m.lit, false));
    }
    if kbm {
        tiles.push((MenuItem::PadMouse, tr("Manette en souris", "Controller as mouse").into(), format!("{} · {}", on_off(s.pad_mouse), tr("stick gauche", "left stick")), s.pad_mouse, false));
        tiles.push((MenuItem::Keyboard, row2_keyboard.into(), tr("Clavier de Steam", "Steam keyboard").into(), false, false));
        // Face à une Station Mac, les touches du Mac (docs/link-ui.md, « Face à une Station Mac »).
        let keys_sub = if app.session_mac(s) { tr("⌘ ⇥, ⌥ ⌘ Échap", "⌘ ⇥, ⌥ ⌘ Esc") } else { tr("Ctrl+Alt+Suppr, Alt+Tab", "Ctrl+Alt+Del, Alt+Tab") };
        tiles.push((MenuItem::Keys, tr("Touches", "Keys").into(), keys_sub.into(), false, false));
    }
    tiles.push((MenuItem::Stats, tr("Statistiques", "Statistics").into(), tr("Latence, débit", "Latency, bitrate").into(), s.show_stats, false));
    // Un invité : « Qualité de l'image », qui montre la qualité reçue (le choix tant qu'on ne la connaît pas).
    if let Some(g) = &s.guest {
        tiles.push((MenuItem::Quality, tr("Qualité de l'image", "Picture quality").into(), g.shown().into(), false, false));
    }
    // Qui regarde un ordinateur à plusieurs écrans : « Écran », qui montre l'écran regardé (comme les autres choix).
    if let Some(w) = s.watch().filter(|w| w.menu) {
        tiles.push((MenuItem::Screen, crate::app::watch_screen_word().into(), w.shown(), false, false));
    }
    // Les manettes d'un invité ne servent qu'avec ce droit : sans lui, pas de tuile.
    let pads = s.role().is_none_or(|r| r.pads());
    if guest || tv || app.machine.battery.is_none() {
        if pads {
            tiles.push((MenuItem::Gamepads, tr("Manettes", "Controllers").into(), format!("{} · {}", app.pads.len(), tr("vibrations", "rumble")), false, false));
        }
    } else {
        tiles.push((MenuItem::Light, tr("Alléger", "Lighter").into(), tr("60 i/s, débit réduit", "60 fps, lower bitrate").into(), s.light, false));
    }
    if let Some(b) = &s.borrow {
        if s.via_relay() {
            tiles.push((MenuItem::Route, s.path().label().into(), route_sub(s), false, false));
        }
        // « Se déconnecter » : la session se ferme sans rendre l'ordinateur, on peut revenir pendant 15 min.
        tiles.push((MenuItem::Disconnect, tr("Se déconnecter", "Disconnect").into(), tr("Fin de la session", "Ends the session").into(), false, false));
        // « Rendre l'ordinateur » : sa confirmation (« Rendre l'ordinateur à {hôte} ? ») ; le sous-titre dit le temps qui reste.
        tiles.push((MenuItem::GiveBack, tr("Rendre l'ordinateur", "Give back the computer").into(), b.time_left(crate::model::now_secs()), false, true));
    } else if guest {
        // Par le relais : le chemin, en tuile (rien à choisir).
        if s.via_relay() {
            tiles.push((MenuItem::Route, s.path().label().into(), route_sub(s), false, false));
        }
        // L'hôte enregistre : la tuile « Enregistrement », inactive (« {nom} enregistre la session. »).
        if let Some(r) = s.recording.as_ref().filter(|r| r.active) {
            let (title, sub) = r.tile();
            tiles.push((MenuItem::Record, title, sub, false, true));
        }
        tiles.push((MenuItem::Disconnect, s.leave_label().into(), s.name.clone(), false, true));
    } else {
        // « Partager » : son sous-titre dit « 2 invités », le code ou « Code et QR code » (`share_tile`). Impossible,
        // elle est estompée mais se choisit : elle ouvre le panneau réduit à la raison.
        let (sub, on) = share_tile(s.share.as_ref());
        tiles.push((MenuItem::Share, tr("Partager", "Share").into(), sub, on, false));
        // « Prêter l'ordinateur… » : le code en cours, une demande, sinon « Code et QR code » ; estompée quand le prêt
        // est impossible (le panneau qu'elle ouvre dit pourquoi).
        let (sub, on) = lend::tile(s);
        tiles.push((MenuItem::Lend, tr("Prêter l'ordinateur…", "Lend the computer…").into(), sub, on, false));
        // « Enregistrer » / « Sur l'ordinateur » ; pendant : « Arrêter l'enregistrement » / « 02:14 », en rouge.
        if let Some(r) = &s.recording {
            let (title, sub) = r.tile();
            tiles.push((MenuItem::Record, title, sub, false, r.active));
        }
        if s.via_relay() {
            tiles.push((MenuItem::Route, s.path().label().into(), route_sub(s), false, false));
        }
        // PyroWave en Wi-Fi ou hors de chez soi : la puce de la barre, en tuile ; elle fait « Passer en HEVC ».
        // Le réseau sature (avis du cœur), par câble aussi : la même tuile, titrée « Le réseau sature ».
        if let Some(t) = s.pw_tile_title() {
            tiles.push((MenuItem::PyroWaveHevc, t.into(), crate::pyrowave::switch_to_hevc().into(), false, false));
        }
        tiles.push((MenuItem::Disconnect, tr("Se déconnecter", "Disconnect").into(), tr("Fin de la session", "Ends the session").into(), false, true));
    }
    let ty = head + 36.0 * u;
    let share_blocked = !guest && s.share.is_none() && app.share_avail(s).is_some_and(|a| a != ShareAvail::Available);
    let lend_blocked = !guest && app.lend_avail(s).is_some_and(|a| a != crate::app::LendAvail::Available);
    for (i, (item, title, sub, on, danger)) in tiles.iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        let r = Rect::new(x0 + col as f32 * (tw + gap), ty + row as f32 * (th + gap), tw, th);
        let hdr_blocked = s.hdr.is_some_and(|u| !u.available());
        let rec_blocked = s.recording.as_ref().is_some_and(|r| r.guest || r.blocked());
        let mic_blocked = mic.is_some_and(|m| m.reason.is_some());
        let dim = (*item == MenuItem::Share && share_blocked) || (*item == MenuItem::Lend && lend_blocked) || (*item == MenuItem::Hdr && hdr_blocked) || (*item == MenuItem::Record && rec_blocked) || (*item == MenuItem::Mic && mic_blocked);
        tile(cv, f, Target::Menu(*item), r, u, title, sub, i == 0, *on, *danger, dim);
    }
    // État, au-dessus de la légende.
    let footer = 56.0 * u;
    let sy = h - footer - 30.0 * u;
    widgets::dot(cv, x0 + 4.0 * u, sy - 4.0 * u, 3.5 * u, OK, false);
    // Le micro envoyé : une petite icône à côté de l'indicateur de la session.
    let mut line_x = x0 + 16.0 * u;
    if mic.is_some_and(|m| m.sent) {
        cv.icon(&Icon::Mic, line_x - 2.0 * u, sy - 12.0 * u, 14.0 * u, 1.6, SPECTRE);
        line_x += 18.0 * u;
    }
    let mut line = s.name.clone();
    if let Some(g) = &s.guest {
        line.push_str(&format!(" · {}", g.role.label()));
    }
    // Un emprunt : « Prêté par {hôte} · Encore 1 h 20 ».
    if let Some(b) = &s.borrow {
        line.push_str(&format!(" · {} · {}", b.lent_by(), b.time_left(crate::model::now_secs())));
    }
    if s.relative {
        line.push_str(&format!(" · {}", tr("souris des jeux", "game mouse")));
    }
    if app.machine.vrr == Some(true) {
        line.push_str(" · VRR");
    }
    if let Some(p) = app.pads.first().filter(|_| pads) {
        line.push_str(&format!(" · {} {}, {}1", tr("manette :", "controller:"), p.name, tr("joueur ", "player ")));
    }
    cv.text_fit(line_x, sy, Text::new(Font::Mono, 12.0 * u, FAINT), &line, w * 0.6);
    if s.role().is_none_or(|r| r.plays()) {
        cv.text(w - x0, sy, Text::new(Font::Mono, 12.0 * u, FAINT).align(Align::Right), tr("Pendant ce menu, l'ordinateur relâche tout", "While this menu is open, the computer releases everything"));
    }
    // Légende.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx = x0;
    lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
    legend(cv, lx, gy, g, &["B"], tr("Reprendre", "Resume"));
    let label = if app.layout == Layout::Handheld { tr("ouvre ce menu · ou un toucher en haut de l'écran", "opens this menu · or a touch at the top of the screen") } else { tr("ouvre ce menu", "opens this menu") };
    // Sans les manettes, un invité n'envoie aucune touche de manette : pas de combinaison (L1 + R1 + ☰ + ⧉), Start ou Select ouvrent le menu.
    let keys: &[&str] = if pads { &["L1", "R1", "☰", "⧉"] } else { &["☰", "⧉"] };
    let lw = widgets::legend_width(cv, g, keys, label);
    legend(cv, w - x0 - lw, gy, g, keys, label);
}

/// Le sous-titre de la tuile « Par le relais » : l'aller-retour mesuré, « 20 ms », sinon « Hors de chez vous ».
fn route_sub(s: &SessionUi) -> String {
    match s.route.and_then(|r| r.total_ms()) {
        Some(ms) => format!("{} ms", decimal(ms, 0)),
        None => tr("Hors de chez vous", "Away from home").into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tile(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, u: f32, title: &str, sub: &str, primary: bool, on: bool, danger: bool, dim: bool) {
    let (focused, hover) = f.state(t, r);
    let radius = 12.0 * u;
    if focused {
        widgets::ring(cv, r, radius);
    }
    if primary {
        let cut = 16.0 * u;
        cv.polygon(&[(r.x, r.y), (r.x + r.w, r.y), (r.x + r.w, r.y + r.h - cut), (r.x + r.w - cut, r.y + r.h), (r.x, r.y + r.h)], if hover { Rgba::hex(0x7E6EFF) } else { FLAT });
    } else {
        cv.rrect(r.x, r.y, r.w, r.h, radius, if hover { Rgba(20, 27, 35, 240) } else { Rgba(14, 20, 26, 230) });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, radius, 1.0, if danger { BAD_LINE } else if on { SPECTRE } else { LINE_2 });
    }
    let pad = 18.0 * u;
    let fg = if danger { BAD_TEXT } else if primary { Rgba(255, 255, 255, 255) } else { FROST };
    // Une tuile estompée (« Partager » impossible) se choisit quand même.
    let fg = if dim { fg.alpha(0.45) } else { fg };
    let mut tx = r.x + pad;
    if primary {
        let s = 16.0 * u;
        let cy = r.y + pad + 11.0 * u;
        cv.polygon(&[(tx, cy - s * 0.45), (tx, cy + s * 0.45), (tx + s * 0.75, cy)], fg);
        tx += s + 6.0 * u;
    }
    cv.text_fit(tx, r.y + pad + 17.0 * u, Text::new(Font::Semibold, 17.0 * u, fg), title, r.w - 2.0 * pad);
    let sub_c = if danger { Rgba::hex(0xC98C86) } else if primary { Rgba::hex(0xE9E4FF) } else { FAINT };
    let sub_c = if dim { sub_c.alpha(0.55) } else { sub_c };
    cv.text_fit(r.x + pad, r.y + r.h - pad, Text::new(Font::Mono, 11.5 * u, sub_c), sub, r.w - 2.0 * pad);
    if on && !primary {
        widgets::dot(cv, r.x + r.w - pad, r.y + pad + 10.0 * u, 4.0 * u, SPECTRE, true);
    }
}

// MARK: - Touches

fn keys(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 200));
    let x0 = if app.layout == Layout::Tv { 96.0 } else { 48.0 };
    let mut y = 50.0 * u;
    eyebrow(cv, x0, y, 12.0 * u, FAINT, tr("Touches", "Keys"));
    y += 26.0 * u;
    let gap = 12.0 * u;
    let tw = (w - 2.0 * x0 - gap) / 2.0;
    let th = 58.0 * u;
    for (i, k) in shortcuts(app.session_mac(s)).iter().enumerate() {
        let (col, row) = (i % 2, i / 2);
        let r = Rect::new(x0 + col as f32 * (tw + gap), y + row as f32 * (th + gap), tw, th);
        let (focused, hover) = f.state(Target::Shortcut(i), r);
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { Rgba(14, 20, 26, 230) });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE_2);
        // Le nom, l'indication en petit dessous.
        if k.hint().is_empty() {
            cv.text_fit(r.x + 18.0 * u, r.y + r.h / 2.0 + 6.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), k.name(), r.w - 36.0 * u);
        } else {
            cv.text_fit(r.x + 18.0 * u, r.y + 26.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), k.name(), r.w - 36.0 * u);
            cv.text_fit(r.x + 18.0 * u, r.y + 46.0 * u, Text::new(Font::Regular, 12.5 * u, FAINT), k.hint(), r.w - 36.0 * u);
        }
    }
    if app.layout.console() {
        let footer = 56.0 * u;
        cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
        let gy = h - footer / 2.0;
        let mut lx = x0;
        lx += legend(cv, lx, gy, 26.0 * u, &["A"], tr("Envoyer", "Send")) + 28.0 * u;
        legend(cv, lx, gy, 26.0 * u, &["B"], tr("Retour", "Back"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LINAPP-19 : une fenêtre de 480 points (un autre ordinateur), une barre de 750 : « Se déconnecter », à
    /// droite, reste dans la fenêtre ; une barre qui tient reste centrée.
    #[test]
    fn bar_stays_in_a_narrow_window() {
        let r = bar_frame(480.0, 750.0);
        assert!(r.x + r.w <= 480.0 - 8.0 + 0.01);
        let r = bar_frame(1280.0, 700.0);
        assert_eq!(r.x, 290.0);
    }

    /// Un menu déroulant ne sort ni à droite ni en bas : Caméra (300 points) à 900 dans 960, Profil (420) dans 480.
    #[test]
    fn dropdowns_stay_in_the_window() {
        let (x, w, rows) = dropdown_place(960.0, 600.0, 900.0, 52.0, 300.0, 30.0, 4);
        assert!(x + w <= 952.0 && x >= 8.0 && rows == 4);
        let (x, w, _) = dropdown_place(480.0, 270.0, 30.0, 52.0, 420.0, 0.0, 3);
        assert!(x >= 8.0 && x + w <= 472.0);
        let (_, _, rows) = dropdown_place(480.0, 270.0, 30.0, 52.0, 300.0, 30.0, 10);
        assert!(52.0 + 30.0 + 12.0 + rows as f32 * 36.0 <= 270.0 - 8.0 && rows >= 1, "les lignes de trop ne sont pas montrées ({rows})");
    }
}
