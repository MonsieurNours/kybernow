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
use super::quality;
use super::share;
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{share_tile, App, BarMenu, Layout, MenuItem, Online, Phase, Rect, Screen, SessionUi, ShareAvail, Target, SHORTCUTS};
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
                keys(app, cv, f);
            } else if s.menu {
                menu(app, cv, f, s);
            } else if s.share_panel {
                // Un invité n'a pas de panneau d'hôte : le sien est la « Qualité de l'image ».
                if s.guest.is_some() {
                    quality::overlay(app, cv, f, s);
                } else {
                    share::overlay(app, cv, f, s);
                }
            } else {
                if s.show_stats && view.main {
                    stats(app, cv, s);
                }
                if s.bar && view.bar && app.layout == Layout::Desktop {
                    bar(app, cv, f, s);
                } else if bar_hint(app, s) {
                    hint(cv);
                }
                if !view.main {
                    return;
                }
                if let Phase::Reconnecting { attempt, max } = s.phase {
                    capsule(app, cv, &format!("{} ({attempt}/{max})", tr("Reconnexion…", "Reconnecting…")), true);
                } else if let Some((n, t)) = &s.notice
                    && t.elapsed().as_secs_f32() < 4.0
                {
                    capsule(app, cv, n, false);
                }
            }
        }
    }
}

// MARK: - Passage

/// Durée de la fermeture, celle de l'app Mac (apps/macos/KyberNow/Passage.swift).
const CLOSE: f32 = 0.6;

fn ease(p: f32) -> f32 {
    // cubic-bezier(.2, .9, .1, 1) approché : départ vif, arrivée douce.
    1.0 - (1.0 - p.clamp(0.0, 1.0)).powi(4)
}

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
    (ease(t / 0.45), alpha)
}

/// La connexion : le fond de l'ordinateur (son image ou sa signature),
/// assombri ; le tube s'allume dans la couture ; la capsule des étapes.
fn passage(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    home::backdrop(app, cv, app.store.host(&s.host));
    let t = s.started.elapsed().as_secs_f32();
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, (110.0 + 60.0 * ease(t / 0.4)) as u8));
    band(cv, u, 0.75 * ease(t / 0.6));
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
    let steps = [
        if s.expected_screens >= 2 { tr("Préparation des écrans", "Preparing the displays") } else { tr("Préparation de l'écran", "Preparing the display") },
        if restarting { tr("La Station redémarre", "The Station is restarting") } else { tr("Connexion", "Connecting") },
        tr("Premières images", "First frames"),
    ];
    let current = match s.phase {
        Phase::Preparing => 0,
        Phase::Connecting | Phase::StationRestarting => 1,
        _ => 2,
    };
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
    let p = ease(t / dur);
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
        let msg = tr("Déconnexion… l'ordinateur remet son écran", "Disconnecting… the computer restores its screen");
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
    let back = if app.sessions.len() > 1 { tr("Fermer", "Close") } else { tr("Retour à l'accueil", "Back to home") };
    widgets::primary(cv, f, Target::Back, ix, y + ch - 82.0 * u, 52.0 * u, back, false);
}

// MARK: - Barre du bureau

/// Hauteur de la barre du bureau.
const BAR_H: f32 = 46.0;
/// Le pointeur ouvre la barre à 2 points au plus du bord haut ; il la garde
/// ouverte à 8 points au plus de son rectangle.
const BAR_EDGE: f32 = 2.0;
const BAR_MARGIN: f32 = 8.0;
/// Le trait qui signale l'endroit (celui de l'app Mac : capsule blanche à 35 %,
/// 220 × 3 points) et la hauteur du bandeau qui le contient.
const HINT_W: f32 = 220.0;
const HINT_H: f32 = 3.0;
pub const HINT_STRIP: f32 = 4.0;

/// Un bouton de la barre : sa cible, son dessin, s'il est allumé, son nom au survol (le texte de
/// « Partager » quand il est inactif) et s'il est éteint.
struct BarIcon {
    target: Target,
    icon: Icon,
    on: bool,
    name: String,
    dim: bool,
}

/// Les boutons de la barre, de gauche à droite : ordinateurs (plusieurs ordinateurs), touches et souris
/// (une session d'hôte, ou un joueur à qui le clavier et la souris sont donnés), haut-parleur (« Garder le
/// son »), plein écran (une seule fenêtre), « Partager » (une session d'hôte ; la pastille des invités le
/// suit). Une session d'invité n'a ni profil, ni ordinateurs, ni « Partager ».
fn bar_icons(app: &App, s: &SessionUi, computers: bool, sound: bool) -> Vec<BarIcon> {
    let icon = |target, icon, on, name: &str| BarIcon { target, icon, on, name: name.into(), dim: false };
    let mut v = Vec::new();
    if computers {
        v.push(icon(Target::BarComputers, Icon::Monitor, s.bar_menu == Some(BarMenu::Computers), tr("Ordinateurs", "Computers")));
    }
    if s.keyboard_mouse() {
        v.push(icon(Target::BarKeys, Icon::Keys, s.bar_menu == Some(BarMenu::Keys), tr("Touches", "Keys")));
        v.push(icon(Target::BarMouse, Icon::Mouse, s.relative, if s.relative { tr("Souris des jeux", "Game mouse") } else { tr("Souris du bureau", "Desktop mouse") }));
    }
    if sound {
        v.push(icon(Target::BarSound, Icon::Speaker, s.keep_sound, tr("Garder le son", "Keep sound")));
    }
    if !s.multi() {
        v.push(icon(Target::BarFullscreen, Icon::Fullscreen, s.fullscreen, tr("Plein écran", "Full screen")));
    }
    if s.guest.is_none() {
        // Partage impossible (Station trop ancienne, partage coupé) : le bouton est estompé et inactif à la
        // souris, sa raison est son infobulle. Tant qu'Admin n'a pas répondu, il reste comme les autres : le
        // panneau attend la réponse (`App::share_known`).
        let blocked = app.share_avail(s).filter(|a| *a != ShareAvail::Available);
        let name = blocked.and_then(|a| a.text()).unwrap_or(tr("Partager", "Share"));
        v.push(BarIcon { target: Target::BarShare, icon: Icon::Share, on: s.share.is_some() || s.bar_menu == Some(BarMenu::Share), name: name.into(), dim: blocked.is_some() });
    }
    v
}

/// Ce que la barre du bureau montre, mesuré : ses textes, leurs largeurs et sa
/// largeur totale (selon le contenu). Le dessin et le geste du pointeur
/// (zone d'ouverture, rectangle) partent de la même mesure.
struct BarLayout {
    lat: String,
    info: String,
    /// Le profil (menu) d'une session d'hôte ; le rôle d'un invité (« Spectateur », « Joueur »), sans menu.
    profile: &'static str,
    quit: &'static str,
    mono14: Text,
    mono12: Text,
    lat_w: f32,
    info_w: f32,
    prof_w: f32,
    quit_w: f32,
    icons: Vec<BarIcon>,
    /// « 2 invités » : le partage est ouvert (il rouvre le panneau), et sa largeur.
    pill: Option<(String, f32)>,
    /// La puce de « Qualité de l'image » d'un invité : la qualité reçue (le choix tant qu'on ne la connaît pas), et sa
    /// largeur.
    quality: Option<(String, f32)>,
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
        let profile = match &s.guest {
            Some(g) => g.role.label(),
            None => home::profile_label(s.profile),
        };
        let prof_w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), profile) + if s.guest.is_some() { 24.0 } else { 34.0 };
        let quit = if s.guest.is_some() { tr("Quitter le partage", "Leave") } else { tr("Se déconnecter", "Disconnect") };
        let quit_w = cv.text_width(Text::new(Font::Semibold, 13.0, BAD_TEXT), quit) + 24.0;
        // Plusieurs ordinateurs (bureau, hors KyberNow Link OS) : le menu « Ordinateurs » s'il y a un autre
        // ordinateur, le haut-parleur « Garder le son » avec au moins deux sessions. Un invité n'a pas le premier.
        let multi = app.multi_sessions();
        let computers = multi && s.guest.is_none() && app.store.hosts.iter().any(|h| h.id != s.host);
        let sound = multi && app.running_sessions() >= 2;
        let icons = bar_icons(app, s, computers, sound);
        // Un invité : la qualité de l'image qu'il reçoit, en menu (il n'a pas de profil).
        let quality = s.guest.as_ref().map(|g| {
            let t = g.shown().to_string();
            let w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), &t) + 34.0;
            (t, w)
        });
        let pill = share::pill_label(s).map(|t| {
            let w = cv.text_width(Text::new(Font::Regular, 12.5, FROST), &t) + 22.0;
            (t, w)
        });
        let others = 16.0 + 16.0 + 12.0 + lat_w + 8.0 + 64.0 + 12.0 + 1.0 + 12.0 + 12.0 + 1.0 + 12.0 + prof_w + 12.0 + quality.as_ref().map_or(0.0, |(_, qw)| qw + 8.0) + 34.0 * icons.len() as f32 + pill.as_ref().map_or(0.0, |(_, pw)| pw + 4.0) + 24.0 + 1.0 + 12.0 + quit_w + 10.0;
        // Une fenêtre étroite (960 points) avec « Ordinateurs », « Garder le son » et la pastille : les mesures se resserrent.
        info_w = info_w.min((w - 8.0 - others).max(60.0));
        let total = others + info_w;
        Self { lat, info, profile, quit, mono14, mono12, lat_w, info_w, prof_w, quit_w, icons, pill, quality, total }
    }
}

/// Le rectangle de la barre ouverte : sa largeur réelle, centrée en haut.
fn bar_frame(w: f32, total: f32) -> Rect {
    Rect::new((w - total) / 2.0, -1.0, total, BAR_H + 1.0)
}

/// `w` : la largeur de la fenêtre, en points (chaque écran a la sienne).
fn bar_rect(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32) -> Rect {
    let total = BarLayout::measure(cv, app, s, w).total;
    bar_frame(w, total)
}

/// Le pointeur est contre le bord haut, dans la zone centrée de la largeur de
/// la barre : l'endroit où elle s'ouvre (après un quart de seconde).
pub fn at_bar_edge(cv: &mut Canvas, app: &App, s: &SessionUi, w: f32, ux: f32, uy: f32) -> bool {
    if uy > BAR_EDGE {
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
    let BarLayout { lat, info, profile, quit, mono14, mono12, lat_w, info_w, prof_w, quit_w, icons, pill, quality, total } = BarLayout::measure(cv, app, s, width);
    let r = bar_frame(cv.w, total);
    let x0 = r.x;
    cv.rrect(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, Rgba(9, 13, 18, 235));
    cv.rrect_stroke(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, 1.0, LINE_2);
    let cy = h / 2.0;
    let mut x = x0 + 16.0;
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
    if s.guest.is_some() {
        cv.rrect_stroke(pr.x, pr.y, pr.w, pr.h, 7.0, 1.0, LINE_2);
        cv.text(pr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), profile);
    } else {
        let (focused, hover) = f.state(Target::BarProfile, pr);
        cv.rrect(pr.x, pr.y, pr.w, pr.h, 7.0, if hover || s.bar_menu == Some(BarMenu::Profile) { INK_3 } else { Rgba(20, 27, 35, 255) });
        cv.rrect_stroke(pr.x, pr.y, pr.w, pr.h, 7.0, 1.0, LINE_2);
        if focused {
            widgets::ring(cv, pr, 7.0);
        }
        cv.text(pr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), profile);
        cv.text(pr.x + pr.w - 12.0, cy + 4.0, Text::new(Font::Regular, 11.0, FAINT).align(Align::Right), "▾");
    }
    let profile_x = pr.x;
    x += prof_w + 12.0;
    let mut tip: Option<(Rect, String)> = None;
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
    // Ordinateurs et touches (deux menus du même dessin), souris, haut-parleur (« Garder le son »), plein
    // écran, « Partager » ; chacun nomme son bouton au survol.
    let (mut keys_x, mut computers_x, mut share_x) = (x, x, x);
    for BarIcon { target: t, icon, on, name, dim } in icons {
        let ir = Rect::new(x, cy - 17.0, 34.0, 34.0);
        let (focused, hover) = f.state(t, ir);
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
            _ => {}
        }
        cv.icon(&icon, x + 8.0, cy - 9.0, 18.0, 1.8, if on { FROST } else if dim { GHOST } else { MUTED });
        x += 34.0;
    }
    // La pastille « 2 invités » : le partage est ouvert ; elle rouvre le panneau.
    if let Some((label, pw)) = pill {
        let pr = Rect::new(x + 2.0, cy - 12.0, pw, 24.0);
        let (focused, hover) = f.state(Target::BarShare, pr);
        cv.rrect(pr.x, pr.y, pr.w, pr.h, 12.0, FLAT.alpha(if hover || s.bar_menu == Some(BarMenu::Share) { 0.34 } else { 0.22 }));
        if focused {
            widgets::ring(cv, pr, 12.0);
        }
        cv.text(pr.x + pr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), &label);
        x += pw + 4.0;
    }
    x += 12.0;
    cv.rect(x.round(), cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    let qr = Rect::new(x, cy - 15.0, quit_w, 30.0);
    let (focused, hover) = f.state(Target::BarQuit, qr);
    if hover {
        cv.rrect(qr.x, qr.y, qr.w, qr.h, 7.0, BAD.alpha(0.18));
    }
    if focused {
        widgets::ring(cv, qr, 7.0);
    }
    cv.text(qr.x + 12.0, cy + 5.0, Text::new(Font::Semibold, 13.0, BAD_TEXT), quit);

    // Menus déroulants.
    match s.bar_menu {
        Some(BarMenu::Profile) => {
            let items: Vec<(Target, String, String, bool)> = Profile::ALL
                .iter()
                .map(|p| (Target::BarPick(*p), home::profile_label(*p).to_string(), crate::app::profile_phrase(Some(*p)).to_string(), *p == s.profile))
                .collect();
            dropdown(cv, f, profile_x, h + 6.0, 420.0, &items);
        }
        Some(BarMenu::Keys) => {
            let items: Vec<(Target, String, String, bool)> = SHORTCUTS.iter().enumerate().map(|(i, k)| (Target::BarShortcut(i), tr(k.0, k.1).to_string(), String::new(), false)).collect();
            dropdown(cv, f, keys_x, h + 6.0, 280.0, &items);
        }
        Some(BarMenu::Computers) => computers_menu(cv, f, app, s, computers_x, h + 6.0),
        Some(BarMenu::Share) => share::dropdown(cv, f, app, s, share_x + 17.0, h + 6.0),
        Some(BarMenu::Quality) => dropdown_titled(cv, f, quality_x, h + 6.0, 300.0, Some(tr("Qualité de l'image", "Picture quality")), &quality::items(s)),
        None => {
            // Le nom du bouton survolé, sous la barre (jamais par-dessus un menu).
            if let Some((r, name)) = tip {
                tooltip(cv, r, h + 4.0, &name);
            }
        }
    }
}

/// Le nom d'un bouton de la barre, en petite capsule sous lui.
fn tooltip(cv: &mut Canvas, r: Rect, y: f32, name: &str) {
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
        let lw = cv.text(r.x + 32.0, r.y + 23.0, Text::new(Font::Regular, 13.5, FROST), label);
        if !hint.is_empty() {
            cv.text_fit(r.x + 32.0 + lw + 10.0, r.y + 23.0, Text::new(Font::Regular, 12.0, FAINT), hint, r.w - lw - 50.0);
        }
        yy += ih;
    }
}

/// Capsule en haut (bureau) ou en bas (manette).
fn capsule(app: &App, cv: &mut Canvas, msg: &str, busy: bool) {
    let u = unit(app.layout);
    let style = Text::new(Font::Regular, 13.0 * u, MUTED);
    let h = 34.0 * u;
    let w = cv.text_width(style, msg) + 46.0 * u;
    let x = (cv.w - w) / 2.0;
    let y = if app.layout == Layout::Desktop { 70.0 } else { cv.h - 90.0 * u };
    cv.rrect(x, y, w, h, h / 2.0, Rgba(9, 13, 18, 220));
    cv.rrect_stroke(x, y, w, h, h / 2.0, 1.0, LINE_2);
    widgets::dot(cv, x + 18.0 * u, y + h / 2.0, 3.5 * u, if busy { WARN } else { SPECTRE }, true);
    cv.text(x + 32.0 * u, y + h / 2.0 + 5.0 * u, style, msg);
}

// MARK: - Statistiques (par-dessus l'image, à la demande)

fn stats(app: &App, cv: &mut Canvas, s: &SessionUi) {
    let u = unit(app.layout);
    let st = s.stats.clone().unwrap_or_default();
    let x = 24.0 * u;
    let y = if app.layout == Layout::Desktop { 24.0 } else { 24.0 * u };
    let w = 300.0 * u;
    let h = 168.0 * u;
    cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 214));
    cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, LINE);
    let ix = x + 16.0 * u;
    let mut yy = y + 36.0 * u;
    let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "— ms".into());
    cv.text(ix, yy, Text::new(Font::MonoMedium, 24.0 * u, SPECTRE), &lat);
    yy += 12.0 * u;
    let hist = &st.history;
    if hist.len() >= 2 {
        let max = hist.iter().cloned().fold(1.0f64, f64::max).max(16.7);
        let gw = w - 32.0 * u;
        let pts: Vec<(f32, f32)> = hist.iter().enumerate().map(|(i, v)| (ix + i as f32 * gw / (hist.len() - 1) as f32, yy + 36.0 * u - (*v / max) as f32 * 32.0 * u)).collect();
        cv.polyline(&pts, 1.5, SPECTRE);
    }
    yy += 58.0 * u;
    let mono = Text::new(Font::Mono, 11.5 * u, MUTED);
    cv.text_fit(ix, yy, mono, &format!("{} · {:.0} {} · {:.0} Mbit/s", st.mode, st.fps, tr("i/s", "fps"), st.mbps), w - 32.0 * u);
    yy += 19.0 * u;
    let audio = st.audio_ms.map(|a| format!(" · {} {} ms", tr("son", "sound"), decimal(a, 0))).unwrap_or_default();
    cv.text_fit(ix, yy, mono, &format!("{}{audio}", st.decoder), w - 32.0 * u);
    yy += 19.0 * u;
    cv.text_fit(ix, yy, mono, &format!("{} {} · 10 s", st.lost, tr("perdues", "lost")), w - 32.0 * u);
}

// MARK: - Menu à la manette

fn menu(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
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
    let cells: Vec<_> = cells.into_iter().filter(|c| s.guest.is_none() || c.1 != tr("Profil", "Profile")).collect();
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
    if s.guest.is_none() {
        let k = tr("Écran virtuel", "Virtual display");
        let kw = cv.text_width(Text::new(Font::Mono, 11.0 * u, FAINT).tracking(0.88 * u), &k.to_uppercase());
        eyebrow(cv, w - x0 - kw, head / 2.0 + 22.0 * u, 11.0 * u, FAINT, k);
    }

    // Les tuiles : cinq par rangée.
    let gap = 14.0 * u;
    let tw = ((w - 2.0 * x0 - 4.0 * gap) / 5.0).min(if tv { 330.0 } else { 216.0 * 1.2 });
    let th = if tv { 170.0 } else { 118.0 };
    let others: Vec<Profile> = Profile::ALL.into_iter().filter(|p| *p != s.profile).collect();
    let audio = match st.audio_ms {
        Some(a) => format!("{} · {} ms", if s.muted { tr("coupé", "muted") } else { tr("actif", "on") }, decimal(a, 0)),
        None => if s.muted { tr("coupé", "muted").into() } else { tr("sur cet appareil", "on this device").into() },
    };
    let on_off = |b: bool| if b { tr("Actif", "On") } else { tr("Inactif", "Off") };
    let row2_keyboard = if app.layout == Layout::Tv { tr("Taper du texte", "Type text") } else { tr("Clavier", "Keyboard") };
    // Les tuiles de l'hôte ; un invité n'a ni profil, ni « Alléger », ni « Partager », et le clavier, la souris
    // et la manette en souris seulement quand ils lui sont donnés (KyberNow Share).
    let guest = s.guest.is_some();
    let kbm = s.keyboard_mouse();
    let mut tiles: Vec<(MenuItem, String, String, bool, bool)> = vec![(MenuItem::Resume, tr("Reprendre", "Resume").into(), tr("Retour au jeu", "Back to the game").into(), false, false)];
    if !guest {
        for p in &others {
            tiles.push((MenuItem::Profile(*p), home::profile_label(*p).into(), tr("Changer de profil", "Change profile").into(), false, false));
        }
    }
    tiles.push((MenuItem::Sound, tr("Son", "Sound").into(), audio, s.muted, false));
    if kbm {
        tiles.push((MenuItem::PadMouse, tr("Manette en souris", "Controller as mouse").into(), format!("{} · {}", on_off(s.pad_mouse), tr("stick gauche", "left stick")), s.pad_mouse, false));
        tiles.push((MenuItem::Keyboard, row2_keyboard.into(), tr("Clavier de Steam", "Steam keyboard").into(), false, false));
        tiles.push((MenuItem::Keys, tr("Touches", "Keys").into(), tr("Ctrl+Alt+Suppr, Alt+Tab", "Ctrl+Alt+Del, Alt+Tab").into(), false, false));
    }
    tiles.push((MenuItem::Stats, tr("Statistiques", "Statistics").into(), tr("Latence, débit", "Latency, bitrate").into(), s.show_stats, false));
    // Un invité : « Qualité de l'image », qui montre la qualité reçue (le choix tant qu'on ne la connaît pas).
    if let Some(g) = &s.guest {
        tiles.push((MenuItem::Quality, tr("Qualité de l'image", "Picture quality").into(), g.shown().into(), false, false));
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
    if guest {
        tiles.push((MenuItem::Disconnect, tr("Quitter le partage", "Leave").into(), s.name.clone(), false, true));
    } else {
        // « Partager » : son sous-titre dit « 2 invités », le code ou « Code et QR code » (`share_tile`). Impossible,
        // elle est estompée mais se choisit : elle ouvre le panneau réduit à la raison.
        let (sub, on) = share_tile(s.share.as_ref());
        tiles.push((MenuItem::Share, tr("Partager", "Share").into(), sub, on, false));
        tiles.push((MenuItem::Disconnect, tr("Se déconnecter", "Disconnect").into(), tr("Fin de la session", "Ends the session").into(), false, true));
    }
    let ty = head + 36.0 * u;
    let share_blocked = !guest && s.share.is_none() && app.share_avail(s).is_some_and(|a| a != ShareAvail::Available);
    for (i, (item, title, sub, on, danger)) in tiles.iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        let r = Rect::new(x0 + col as f32 * (tw + gap), ty + row as f32 * (th + gap), tw, th);
        tile(cv, f, Target::Menu(*item), r, u, title, sub, i == 0, *on, *danger, *item == MenuItem::Share && share_blocked);
    }
    // État, au-dessus de la légende.
    let footer = 56.0 * u;
    let sy = h - footer - 30.0 * u;
    widgets::dot(cv, x0 + 4.0 * u, sy - 4.0 * u, 3.5 * u, OK, false);
    let mut line = s.name.clone();
    if let Some(g) = &s.guest {
        line.push_str(&format!(" · {}", g.role.label()));
    }
    if s.relative {
        line.push_str(&format!(" · {}", tr("souris des jeux", "game mouse")));
    }
    if app.machine.vrr == Some(true) {
        line.push_str(" · VRR");
    }
    if let Some(p) = app.pads.first().filter(|_| pads) {
        line.push_str(&format!(" · {} {}, {}1", tr("manette :", "controller:"), p.name, tr("joueur ", "player ")));
    }
    cv.text_fit(x0 + 16.0 * u, sy, Text::new(Font::Mono, 12.0 * u, FAINT), &line, w * 0.6);
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

#[allow(clippy::too_many_arguments)]
fn tile(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, u: f32, title: &str, sub: &str, primary: bool, on: bool, danger: bool, dim: bool) {
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

fn keys(app: &App, cv: &mut Canvas, f: &mut Frame) {
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
    for (i, k) in SHORTCUTS.iter().enumerate() {
        let (col, row) = (i % 2, i / 2);
        let r = Rect::new(x0 + col as f32 * (tw + gap), y + row as f32 * (th + gap), tw, th);
        let (focused, hover) = f.state(Target::Shortcut(i), r);
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { Rgba(14, 20, 26, 230) });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE_2);
        cv.text(r.x + 18.0 * u, r.y + r.h / 2.0 + 6.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), tr(k.0, k.1));
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
