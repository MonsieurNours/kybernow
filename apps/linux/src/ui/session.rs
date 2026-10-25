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
use super::widgets::{self, eyebrow, legend};
use super::{unit, Frame};
use crate::app::{App, BarMenu, Layout, MenuItem, Phase, Rect, SessionUi, Target, SHORTCUTS};
use crate::i18n::{decimal, tr};
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(s) = &app.session else {
        cv.clear(INK);
        return;
    };
    match &s.phase {
        Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages => passage(app, cv, f, s),
        Phase::Ended { error } => ended(app, cv, f, s, error.as_deref()),
        Phase::Ending => ending(app, cv, s),
        _ => {
            cv.clear_transparent();
            if s.keys {
                keys(app, cv, f);
            } else if s.menu {
                menu(app, cv, f, s);
            } else {
                if s.show_stats {
                    stats(app, cv, s);
                }
                if s.bar && app.layout == Layout::Desktop {
                    bar(app, cv, f, s);
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
        tr("Préparation de l'écran", "Preparing the display"),
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
    widgets::primary(cv, f, Target::Back, ix, y + ch - 82.0 * u, 52.0 * u, tr("Retour à l'accueil", "Back to home"), false);
}

// MARK: - Barre du bureau

fn bar(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let w = cv.w;
    let h = 46.0;
    // Largeur selon le contenu : mesurée d'abord.
    let st = s.stats.clone().unwrap_or_default();
    let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "— ms".into());
    let info = if st.fps > 0.0 { format!("{} · {:.0} {} · {}", st.mode, st.fps, tr("i/s", "fps"), st.decoder) } else { st.mode.clone() };
    let mono14 = Text::new(Font::MonoMedium, 14.0, FROST);
    let mono12 = Text::new(Font::Mono, 12.0, MUTED);
    let lat_w = cv.text_width(mono14, &lat);
    let info_w = cv.text_width(mono12, &info);
    let profile = home::profile_label(s.profile);
    let prof_w = cv.text_width(Text::new(Font::Regular, 13.0, FROST), profile) + 34.0;
    let quit = tr("Se déconnecter", "Disconnect");
    let quit_w = cv.text_width(Text::new(Font::Semibold, 13.0, BAD_TEXT), quit) + 24.0;
    let total = 16.0 + 16.0 + 12.0 + lat_w + 8.0 + 64.0 + 12.0 + 1.0 + 12.0 + info_w + 12.0 + 1.0 + 12.0 + prof_w + 12.0 + 34.0 * 3.0 + 24.0 + 1.0 + 12.0 + quit_w + 10.0;
    let x0 = (w - total) / 2.0;
    let r = Rect::new(x0, -1.0, total, h + 1.0);
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
    cv.rect(x, cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    cv.text(x, cy + 4.0, mono12, &info);
    x += info_w + 12.0;
    cv.rect(x, cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    // Profil (menu).
    let pr = Rect::new(x, cy - 15.0, prof_w, 30.0);
    let (focused, hover) = f.state(Target::BarProfile, pr);
    cv.rrect(pr.x, pr.y, pr.w, pr.h, 7.0, if hover || s.bar_menu == Some(BarMenu::Profile) { INK_3 } else { Rgba(20, 27, 35, 255) });
    cv.rrect_stroke(pr.x, pr.y, pr.w, pr.h, 7.0, 1.0, LINE_2);
    if focused {
        widgets::ring(cv, pr, 7.0);
    }
    cv.text(pr.x + 12.0, cy + 5.0, Text::new(Font::Regular, 13.0, FROST), profile);
    cv.text(pr.x + pr.w - 12.0, cy + 4.0, Text::new(Font::Regular, 11.0, FAINT).align(Align::Right), "▾");
    let profile_x = pr.x;
    x += prof_w + 12.0;
    // Touches, souris, plein écran.
    let keys_x = x;
    for (t, icon, on) in [(Target::BarKeys, Icon::Keys, s.bar_menu == Some(BarMenu::Keys)), (Target::BarMouse, Icon::Mouse, s.relative), (Target::BarFullscreen, Icon::Fullscreen, app.fullscreen)] {
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
        cv.icon(&icon, x + 8.0, cy - 9.0, 18.0, 1.8, if on { FROST } else { MUTED });
        x += 34.0;
    }
    x += 12.0;
    cv.rect(x, cy - 11.0, 1.0, 22.0, LINE_2);
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
        None => {}
    }
}

fn dropdown(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, w: f32, items: &[(Target, String, String, bool)]) {
    let ih = 36.0;
    let h = items.len() as f32 * ih + 12.0;
    cv.rrect(x, y, w, h, 10.0, Rgba(9, 13, 18, 245));
    cv.rrect_stroke(x, y, w, h, 10.0, 1.0, LINE_2);
    let mut yy = y + 6.0;
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
    let k = tr("Écran virtuel", "Virtual display");
    let kw = cv.text_width(Text::new(Font::Mono, 11.0 * u, FAINT).tracking(0.88 * u), &k.to_uppercase());
    eyebrow(cv, w - x0 - kw, head / 2.0 + 22.0 * u, 11.0 * u, FAINT, k);

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
    let tiles: Vec<(MenuItem, String, String, bool, bool)> = vec![
        (MenuItem::Resume, tr("Reprendre", "Resume").into(), tr("Retour au jeu", "Back to the game").into(), false, false),
        (MenuItem::Profile(others[0]), home::profile_label(others[0]).into(), tr("Changer de profil", "Change profile").into(), false, false),
        (MenuItem::Profile(others[1]), home::profile_label(others[1]).into(), tr("Changer de profil", "Change profile").into(), false, false),
        (MenuItem::Sound, tr("Son", "Sound").into(), audio, s.muted, false),
        (MenuItem::PadMouse, tr("Manette en souris", "Controller as mouse").into(), format!("{} · {}", on_off(s.pad_mouse), tr("stick gauche", "left stick")), s.pad_mouse, false),
        (MenuItem::Keyboard, row2_keyboard.into(), tr("Clavier de Steam", "Steam keyboard").into(), false, false),
        (MenuItem::Keys, tr("Touches", "Keys").into(), tr("Ctrl+Alt+Suppr, Alt+Tab", "Ctrl+Alt+Del, Alt+Tab").into(), false, false),
        (MenuItem::Stats, tr("Statistiques", "Statistics").into(), tr("Latence, débit", "Latency, bitrate").into(), s.show_stats, false),
        if tv || app.machine.battery.is_none() {
            (MenuItem::Gamepads, tr("Manettes", "Controllers").into(), format!("{} · {}", app.pads.len(), tr("vibrations", "rumble")), false, false)
        } else {
            (MenuItem::Light, tr("Alléger", "Lighter").into(), tr("60 i/s, débit réduit", "60 fps, lower bitrate").into(), s.light, false)
        },
        (MenuItem::Disconnect, tr("Se déconnecter", "Disconnect").into(), tr("Fin de la session", "Ends the session").into(), false, true),
    ];
    let ty = head + 36.0 * u;
    for (i, (item, title, sub, on, danger)) in tiles.iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        let r = Rect::new(x0 + col as f32 * (tw + gap), ty + row as f32 * (th + gap), tw, th);
        tile(cv, f, Target::Menu(*item), r, u, title, sub, i == 0, *on, *danger);
    }
    // État, au-dessus de la légende.
    let footer = 56.0 * u;
    let sy = h - footer - 30.0 * u;
    widgets::dot(cv, x0 + 4.0 * u, sy - 4.0 * u, 3.5 * u, OK, false);
    let mut line = s.name.clone();
    if s.relative {
        line.push_str(&format!(" · {}", tr("souris des jeux", "game mouse")));
    }
    if app.machine.vrr == Some(true) {
        line.push_str(" · VRR");
    }
    if let Some(p) = app.pads.first() {
        line.push_str(&format!(" · {} {}, {}1", tr("manette :", "controller:"), p.name, tr("joueur ", "player ")));
    }
    cv.text_fit(x0 + 16.0 * u, sy, Text::new(Font::Mono, 12.0 * u, FAINT), &line, w * 0.6);
    cv.text(w - x0, sy, Text::new(Font::Mono, 12.0 * u, FAINT).align(Align::Right), tr("Pendant ce menu, l'ordinateur relâche tout", "While this menu is open, the computer releases everything"));
    // Légende.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx = x0;
    lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
    legend(cv, lx, gy, g, &["B"], tr("Reprendre", "Resume"));
    let label = if app.layout == Layout::Handheld { tr("ouvre ce menu · ou un toucher en haut de l'écran", "opens this menu · or a touch at the top of the screen") } else { tr("ouvre ce menu", "opens this menu") };
    let keys = ["L1", "R1", "☰", "⧉"];
    let lw = widgets::legend_width(cv, g, &keys, label);
    legend(cv, w - x0 - lw, gy, g, &keys, label);
}

#[allow(clippy::too_many_arguments)]
fn tile(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, u: f32, title: &str, sub: &str, primary: bool, on: bool, danger: bool) {
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
    let mut tx = r.x + pad;
    if primary {
        let s = 16.0 * u;
        let cy = r.y + pad + 11.0 * u;
        cv.polygon(&[(tx, cy - s * 0.45), (tx, cy + s * 0.45), (tx + s * 0.75, cy)], fg);
        tx += s + 6.0 * u;
    }
    cv.text_fit(tx, r.y + pad + 17.0 * u, Text::new(Font::Semibold, 17.0 * u, fg), title, r.w - 2.0 * pad);
    let sub_c = if danger { Rgba::hex(0xC98C86) } else if primary { Rgba::hex(0xE9E4FF) } else { FAINT };
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
