//! La session : l'écran de connexion (ses trois étapes), puis rien par-dessus
//! l'image (transparent : le plan vidéo du téléviseur se voit), sauf les
//! états en capsule (reconnexion, manette branchée…), les statistiques à la
//! demande et le menu (bouton Retour, L1 + R1 + Start + Select).

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self};
use super::Frame;
use crate::app::{share_tile, App, MenuItem, Phase, Rect, SessionUi, ShareAvail, Target, SHORTCUTS};
use crate::i18n::{decimal, english, tr};
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear_transparent();
    let Some(s) = &app.session else { return };
    match &s.phase {
        Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages => arrival(app, cv, f, s),
        Phase::Ended { error } => ended(cv, f, s, error.as_deref()),
        Phase::Ending => {
            cv.rect(0.0, 0.0, W, H, Rgba(5, 7, 10, 200));
            capsule(cv, H / 2.0 - 32.0, tr("Déconnexion…", "Disconnecting…"), true);
        }
        Phase::Streaming | Phase::Reconnecting { .. } => {
            if s.menu {
                menu(app, cv, f, s);
            } else if s.share_panel {
                // Un invité n'a pas de panneau d'hôte : le sien est la « Qualité de l'image ».
                if s.guest.is_some() {
                    super::quality::draw(cv, f, s);
                } else {
                    super::share::draw(app, cv, f, s);
                }
            } else if s.show_stats {
                stats_pill(cv, s, 40.0);
            }
            if let Phase::Reconnecting { attempt, max } = s.phase {
                let msg = format!("{} ({attempt}/{max})", tr("Reconnexion", "Reconnecting"));
                capsule(cv, H - 140.0, &msg, true);
            } else if let Some((n, at)) = &s.notice
                && at.elapsed().as_secs_f32() < 4.0
                && !s.menu
            {
                capsule(cv, H - 140.0, n, false);
            }
        }
    }
}

/// Capsule d'état, centrée.
fn capsule(cv: &mut Canvas, y: f32, msg: &str, lit: bool) {
    let style = Text::new(Font::Regular, 24.0, FROST);
    let w = cv.text_width(style, msg) + if lit { 104.0 } else { 72.0 };
    let x = (W - w) / 2.0;
    cv.rrect(x, y, w, 64.0, 32.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, w, 64.0, 32.0, 1.5, SPECTRE.alpha(if lit { 0.7 } else { 0.35 }));
    let mut tx = x + 36.0;
    if lit {
        cv.circle(x + 44.0, y + 32.0, 14.0, HALO.alpha(0.35));
        cv.circle(x + 44.0, y + 32.0, 6.0, SPECTRE);
        tx += 32.0;
    }
    cv.text(tx, y + 41.0, style, msg);
}

/// La connexion (docs/link-ui.md) : le nom, les trois étapes et Annuler,
/// rien d'autre ; puis l'image seule. Pendant le dernier recours (la Station
/// redémarre), « La Station redémarre… » à la place des étapes.
fn arrival(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.clear(INK);
    if let Some(img) = app.images.get(&s.host) {
        cv.image_cover(img, 0.0, 0.0, W, H, 0.55);
        cv.rect(0.0, 0.0, W, H, Rgba(5, 7, 10, 150));
    }
    // La couture à 45° qui s'allume.
    let t = s.started.elapsed().as_secs_f32();
    let glow = 0.4 + 0.3 * (t * 2.2).sin().abs();
    for i in 0..6 {
        cv.line(560.0 - i as f32, 1080.0, 1640.0 - i as f32, 0.0, 14.0 - i as f32 * 2.0, HALO.alpha(glow * 0.12));
    }
    cv.line(560.0, 1080.0, 1640.0, 0.0, 3.0, SPECTRE.alpha(glow + 0.2));

    let current = match s.phase {
        Phase::Preparing => 0,
        Phase::Connecting => 1,
        _ => 2,
    };
    let steps = [
        tr("Préparation de l'écran", "Preparing the display"),
        tr("Connexion", "Connecting"),
        tr("Premières images", "First frames"),
    ];
    let (w, h) = (900.0, 400.0);
    let (x, y) = ((W - w) / 2.0, (H - h) / 2.0);
    cv.rrect(x, y, w, h, 28.0, Rgba(14, 19, 28, 240));
    cv.rrect_stroke(x, y, w, h, 28.0, 1.5, SPECTRE.alpha(0.6));
    cv.text_fit(x + 60.0, y + 112.0, Text::new(Font::Display, 58.0, FROST).tracking(-1.5), &s.name, w - 120.0);
    if s.phase == Phase::StationRestarting {
        let (cx, cy) = (x + 76.0, y + 172.0);
        cv.circle(cx, cy, 14.0, HALO.alpha(0.2 + glow * 0.3));
        cv.circle(cx, cy, 6.0, SPECTRE);
        cv.text(cx + 30.0, cy + 9.0, Text::new(Font::Regular, 25.0, FROST), tr("La Station redémarre…", "The Station is restarting…"));
    } else {
        // Faites, en cours (avec « … »), à venir.
        for (i, label) in steps.iter().enumerate() {
            let (cx, cy) = (x + 76.0, y + 172.0 + i as f32 * 48.0);
            let style = Text::new(Font::Regular, 25.0, if i > current { FROST_4 } else { FROST });
            if i < current {
                cv.icon(&Icon::Check, cx - 14.0, cy - 14.0, 28.0, 2.2, SPECTRE);
                cv.text(cx + 30.0, cy + 9.0, style.color(FROST_2), label);
            } else if i == current {
                cv.circle(cx, cy, 14.0, HALO.alpha(0.2 + glow * 0.3));
                cv.circle(cx, cy, 6.0, SPECTRE);
                cv.text(cx + 30.0, cy + 9.0, style, &format!("{label}…"));
            } else {
                cv.circle_stroke(cx, cy, 6.0, 1.5, FROST_4);
                cv.text(cx + 30.0, cy + 9.0, style, label);
            }
        }
    }
    let bw = 220.0;
    let r = Rect::new(x + w - bw - 40.0, y + h - 84.0, bw, 56.0);
    let focused = f.hit(Target::CancelConnect, r);
    if focused {
        cv.rrect(r.x, r.y, r.w, r.h, 14.0, CORE);
    } else {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.5, BORDER);
    }
    cv.text(r.x + r.w / 2.0, r.y + 37.0, Text::new(Font::Semibold, 22.0, if focused { INK } else { FROST }).align(Align::Center), tr("Annuler", "Cancel"));
}

fn ended(cv: &mut Canvas, f: &mut Frame, s: &SessionUi, error: Option<&str>) {
    cv.clear(INK);
    let (w, h) = (1000.0, 300.0);
    let (x, y) = ((W - w) / 2.0, (H - h) / 2.0);
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    cv.text(x + 60.0, y + 80.0, Text::new(Font::Mono, 16.0, FROST_3).tracking(3.0), &s.name.to_uppercase());
    let title = match (error.is_some(), s.streamed) {
        (true, false) => tr("La connexion a échoué", "The connection failed"),
        (true, true) => tr("La session s'est arrêtée", "The session stopped"),
        _ => tr("Session terminée", "Session ended"),
    };
    cv.text(x + 60.0, y + 140.0, Text::new(Font::Title, 40.0, FROST), title);
    if let Some(e) = error {
        cv.paragraph(x + 60.0, y + 188.0, Text::new(Font::Regular, 21.0, FROST_2), e, w - 120.0, 30.0);
    }
    widgets::secondary(cv, f, Target::Back, x + w - 260.0, y + h - 96.0, tr("Retour à l'accueil", "Back home"), 22.0);
}

/// Santé du flux : latence, images par seconde, débit, mode, pertes.
fn stats_pill(cv: &mut Canvas, s: &SessionUi, y: f32) {
    let Some(st) = &s.stats else {
        capsule(cv, y, tr("Mesures…", "Measuring…"), true);
        return;
    };
    let lat = st.latency_ms.map(|v| format!("{} ms", decimal(v, 1))).unwrap_or_else(|| "— ms".into());
    let lost = if st.lost == 0 { tr("0 perte", "0 loss").to_string() } else { format!("{} {}", st.lost, tr("pertes", "losses")) };
    let parts = [lat, format!("{} {}", st.fps.round(), tr("i/s", "fps")), format!("{} Mbit/s", st.mbps.round()), format!("{} · {}", st.mode, st.codec), lost];
    let style = Text::new(Font::Mono, 19.0, FROST);
    let gap = 36.0;
    let widths: Vec<f32> = parts.iter().map(|p| cv.text_width(style, p)).collect();
    let total = widths.iter().sum::<f32>() + gap * (parts.len() as f32 - 1.0) + 72.0 + 20.0;
    let x = (W - total) / 2.0;
    cv.rrect(x, y, total, 60.0, 30.0, Rgba(14, 19, 28, 235));
    cv.rrect_stroke(x, y, total, 60.0, 30.0, 1.0, SPECTRE.alpha(0.5));
    cv.circle(x + 40.0, y + 30.0, 5.0, SPECTRE);
    let mut xx = x + 56.0;
    for (i, p) in parts.iter().enumerate() {
        cv.text(xx, y + 38.0, if i >= 3 { style.color(FROST_2) } else { style }, p);
        xx += widths[i] + gap;
    }
}

fn menu(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 175));
    stats_pill(cv, s, 70.0);
    // Sous les mesures : l'ordinateur et son profil ; pour un invité, son rôle (il n'a pas de profil).
    let head = match &s.guest {
        Some(g) => format!("{} · {}", s.name.to_uppercase(), g.role.label().to_uppercase()),
        None => format!("{} · {} {}", s.name.to_uppercase(), tr("PROFIL", "PROFILE"), super::home::profile_label(s.profile).to_uppercase()),
    };
    cv.text(W / 2.0, 200.0, Text::new(Font::Mono, 16.0, FROST_3).tracking(3.0).align(Align::Center), &head);

    if s.keys {
        keys(cv, f);
        return;
    }

    // Les tuiles de l'hôte ; un invité n'a ni profil, ni réglages de l'ordinateur, ni « Partager », et la
    // Magic Remote en souris, les touches et la manette en souris seulement avec le clavier et la souris
    // donnés, la tuile des manettes seulement avec les manettes (KyberNow Share).
    let guest = s.guest.is_some();
    let kbm = s.keyboard_mouse();
    let pads = s.role().is_none_or(|r| r.pads());
    let mut tiles: Vec<(MenuItem, Icon, String, Option<String>, bool, bool)> = vec![(MenuItem::Resume, Icon::Play, tr("Reprendre", "Resume").into(), None, false, false)];
    if !guest {
        for p in Profile::ALL.iter().copied().filter(|p| *p != s.profile) {
            let sub = match p {
                Profile::Jeu => tr("faible latence", "low latency"),
                Profile::Bureau => tr("texte net", "sharp text"),
                Profile::Max => tr("tout au plus haut", "everything highest"),
            };
            tiles.push((MenuItem::Profile(p), Icon::Monitor, format!("{} {}", tr("Profil", "Profile"), super::home::profile_label(p)), Some(sub.into()), false, false));
        }
    }
    if kbm {
        tiles.push((MenuItem::RemoteMouse, Icon::Pointer, "Magic Remote".into(), Some(tr("en souris · OK = clic", "as mouse · OK = click").into()), false, s.remote_mouse));
        tiles.push((MenuItem::PadMouse, Icon::Gamepad, tr("Manette en souris", "Controller as mouse").into(), Some(tr("stick gauche", "left stick").into()), false, s.pad_mouse));
        tiles.push((MenuItem::Keys, Icon::Keys, tr("Touches", "Keys").into(), Some("Ctrl+Alt+Suppr…".into()), false, false));
    }
    tiles.push((MenuItem::Stats, Icon::Chart, tr("Statistiques", "Statistics").into(), Some(tr("en haut de l'image", "over the picture").into()), false, s.show_stats));
    if pads {
        tiles.push((MenuItem::Gamepads, Icon::Gamepad, tr("Manettes", "Controllers").into(), Some(format!("{} {}", app.pads.len(), tr("branchée(s)", "connected"))), false, false));
    }
    if let Some(g) = &s.guest {
        // « Qualité de l'image » : la tuile montre la qualité reçue (le choix tant que Kyber ne la donne pas) et ouvre les cinq choix.
        tiles.push((MenuItem::Quality, Icon::Monitor, tr("Qualité de l'image", "Picture quality").into(), Some(g.shown().into()), false, false));
        tiles.push((MenuItem::Disconnect, Icon::Logout, tr("Quitter le partage", "Leave").into(), Some(s.name.clone()), true, false));
    } else {
        // « Partager » : son sous-titre dit « 2 invités », le code ou « Code et QR code » (`share_tile`). Impossible,
        // elle est estompée mais se choisit : elle ouvre le panneau réduit à la raison.
        let (sub, on) = share_tile(s.share.as_ref());
        tiles.push((MenuItem::Share, Icon::Share, tr("Partager", "Share").into(), Some(sub), false, on));
        tiles.push((MenuItem::Settings, Icon::Gear, tr("Réglages", "Settings").into(), Some(tr("prochaine session", "next session").into()), false, false));
        tiles.push((MenuItem::Disconnect, Icon::Logout, tr("Se déconnecter", "Disconnect").into(), None, true, false));
    }

    let rows = tiles.len().div_ceil(5);
    let (x0, y0, cols, gap) = (180.0, if rows > 2 { 250.0 } else { 270.0 }, 5usize, 26.0);
    let tw = (W - 2.0 * x0 - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = if rows > 2 { 168.0 } else { 190.0 };
    let share_blocked = !guest && s.share.is_none() && app.share_avail().is_some_and(|a| a != ShareAvail::Available);
    for (i, (item, icon, title, sub, danger, on)) in tiles.iter().enumerate() {
        let (c, r) = (i % cols, i / cols);
        let rect = Rect::new(x0 + c as f32 * (tw + gap), y0 + r as f32 * (th + gap), tw, th);
        widgets::tile(cv, f, Target::Menu(*item), rect, *icon, title, sub.as_deref(), *danger, *on, *item == MenuItem::Share && share_blocked);
    }

    if s.game_hint {
        let (x, y, w, h) = (180.0, if rows > 2 { 820.0 } else { 760.0 }, W - 360.0, 96.0);
        cv.rrect(x, y, w, h, 18.0, Rgba(14, 19, 28, 230));
        cv.rrect_stroke(x, y, w, h, 18.0, 1.0, BORDER);
        cv.icon(&Icon::Info, x + 30.0, y + 31.0, 34.0, 1.6, SPECTRE);
        let msg = tr(
            "Pour la latence la plus basse : bouton Réglages de la Magic Remote › Image › Mode › Optimiseur de jeu. L'app ne peut pas le changer elle-même.",
            "For the lowest latency: Magic Remote Settings button › Picture › Mode › Game Optimizer. The app cannot change it itself.",
        );
        cv.paragraph(x + 84.0, y + 40.0, Text::new(Font::Regular, 20.0, FROST), msg, w - 400.0, 28.0);
        let label = tr("Ne plus le dire", "Don't remind me");
        let lw = cv.text_width(Text::new(Font::Semibold, 20.0, FROST), label) + 40.0;
        widgets::secondary(cv, f, Target::HintDismiss, x + w - lw - 30.0, y + 22.0, label, 20.0);
    }

    // Sans les manettes, un invité n'envoie aucune touche de manette : pas de combinaison (Start, Select ou B ouvrent le menu).
    let hint = if pads {
        tr("Menu : bouton Retour de la Magic Remote, ou L1 + R1 + Start + Select · pendant le menu, l'ordinateur ne reçoit rien", "Menu: Magic Remote Back, or L1 + R1 + Start + Select · while the menu is open, the computer receives nothing")
    } else {
        tr("Menu : bouton Retour de la Magic Remote, ou Start, Select ou B d'une manette", "Menu: Magic Remote Back, or Start, Select or B on a controller")
    };
    cv.text(W / 2.0, H - 70.0, Text::new(Font::Regular, 19.0, FROST_3).align(Align::Center), hint);
}

fn keys(cv: &mut Canvas, f: &mut Frame) {
    cv.text(W / 2.0, 290.0, Text::new(Font::Title, 40.0, FROST).align(Align::Center), tr("Touches", "Keys"));
    let (x0, y0, cols, gap) = (300.0, 340.0, 2usize, 22.0);
    let w = (W - 2.0 * x0 - gap) / cols as f32;
    for (i, (fr, en, _)) in SHORTCUTS.iter().enumerate() {
        let (c, r) = (i % cols, i / cols);
        let rect = Rect::new(x0 + c as f32 * (w + gap), y0 + r as f32 * 96.0, w, 80.0);
        let focused = f.hit(Target::Shortcut(i), rect);
        if focused {
            cv.neon(rect.x, rect.y, rect.w, rect.h, 16.0, 0.9);
            cv.rrect(rect.x, rect.y, rect.w, rect.h, 16.0, CORE);
        } else {
            cv.rrect(rect.x, rect.y, rect.w, rect.h, 16.0, Rgba(14, 19, 28, 235));
            cv.rrect_stroke(rect.x, rect.y, rect.w, rect.h, 16.0, 1.0, BORDER);
        }
        let label = if english() { *en } else { *fr };
        cv.text(rect.x + 30.0, rect.y + 50.0, Text::new(Font::Semibold, 24.0, if focused { INK } else { FROST }), label);
    }
}
