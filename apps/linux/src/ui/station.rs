//! Les rubriques Statistiques et Station des réglages d'un ordinateur
//! (docs/link-ui.md) : la carte « Dernière session » (celle de l'app Mac,
//! `LastSessionPanel` et `FrameRuler` : la médiane en grand, la règle en
//! millisecondes avec les repères 160, 120 et 60 Hz, la bande de la médiane au
//! 95e centile) ; l'état de la Station (version, mise à jour, redémarrage en
//! cours) et le QR code d'Admin, à la manette et sur KyberNow Link OS.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::theme::*;
use super::widgets::{eyebrow, panel};
use crate::app::{admin_url, Rect, requires_setup_text, too_old_text, App, Online, StationKind, StationStep};
use crate::i18n::{decimal, tr};
use crate::model::Host;

/// « 42 min », « 1 h 12 ».
fn duration(s: u64) -> String {
    let m = s / 60;
    if m >= 60 { format!("{} h {:02}", m / 60, m % 60) } else { format!("{m} min") }
}

/// La carte « Dernière session » ; rend sa hauteur. `u` : l'échelle de l'interface.
pub fn last_session(cv: &mut Canvas, h: &Host, x: f32, y: f32, w: f32, u: f32) -> f32 {
    let pad = 22.0 * u;
    let ix = x + pad;
    let iw = w - 2.0 * pad;
    let Some(last) = &h.last else {
        let height = pad + 13.0 * u + 18.0 * u + 21.0 * u + pad;
        panel(cv, Rect::new(x, y, w, height), 14.0 * u);
        eyebrow(cv, ix, y + pad + 11.0 * u, 11.0 * u, FAINT, tr("Dernière session", "Last session"));
        cv.text_fit(ix, y + pad + 13.0 * u + 18.0 * u + 15.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), tr("Pas encore de session sur cet ordinateur.", "No session on this computer yet."), iw);
        return height;
    };
    let measured = last.median > 0.0;
    let transmitted = last.transmitted();
    // Les lignes sous la règle : « 95 % des images sous … », ce qui a été transmis.
    let lines = measured as usize + !transmitted.is_empty() as usize;
    let below = if lines == 0 { 0.0 } else { 18.0 * u + (lines - 1) as f32 * 20.0 * u };
    let height = pad + 11.0 * u + 48.0 * u + 12.0 * u + RULER * u + below + pad - 2.0 * u;
    panel(cv, Rect::new(x, y, w, height), 14.0 * u);
    let mut yy = y + pad + 11.0 * u;
    eyebrow(cv, ix, yy, 11.0 * u, FAINT, tr("Dernière session", "Last session"));
    let when = format!("{} · {}", duration(last.seconds), crate::app_time::relative_day(last.at));
    cv.text(x + w - pad, yy, Text::new(Font::Regular, 13.0 * u, MUTED).align(Align::Right), &when);
    yy += 2.0 * u + 46.0 * u;
    let value = if measured { format!("{} ms", decimal(last.median, 1)) } else { "— ms".into() };
    let vw = cv.text(ix, yy, Text::new(Font::MonoMedium, 34.0 * u, FROST).tracking(-0.6 * u), &value);
    cv.text(ix + vw + 10.0 * u, yy, Text::new(Font::Mono, 12.0 * u, MUTED), tr("médiane", "median"));
    yy += 12.0 * u;
    ruler(cv, ix, yy, iw, measured.then_some(last.median), (measured && last.p95 > 0.0).then_some(last.p95), u);
    yy += RULER * u + 18.0 * u;
    let small = Text::new(Font::Regular, 13.0 * u, FAINT);
    if measured {
        cv.text_fit(ix, yy, small, &format!("{} {} ms", tr("95 % des images sous", "95% of frames under"), decimal(last.p95.max(last.median), 1)), iw);
        yy += 20.0 * u;
    }
    if !transmitted.is_empty() {
        cv.text_fit(ix, yy, Text::new(Font::Mono, 12.0 * u, MUTED), &transmitted, iw);
    }
    height
}

/// La hauteur de la règle (étiquettes comprises), en points avant l'échelle.
const RULER: f32 = 52.0;

/// La règle en millisecondes, de 0 à 20 ms (plus si la mesure dépasse) : les
/// repères d'une image à 160, 120 et 60 Hz, la bande de la médiane au 95e
/// centile, la médiane. Une étiquette seulement si elle a la place.
pub fn ruler(cv: &mut Canvas, x: f32, y: f32, w: f32, median: Option<f64>, p95: Option<f64>, u: f32) {
    let top = median.unwrap_or(0.0).max(p95.unwrap_or(0.0));
    let scale = ((top * 1.25 / 10.0).ceil() * 10.0).max(20.0);
    let px = |ms: f64| x + (ms.min(scale) / scale) as f32 * w;
    let line_y = y + 24.0 * u;
    cv.rect(x, line_y - 1.0 * u, w, 2.0 * u, Rgba(255, 255, 255, 41));
    if let (Some(m), Some(p)) = (median, p95)
        && p > m
    {
        let (a, b) = (px(m), px(p));
        let bw = (b - a).max(4.0 * u);
        cv.rrect(a, line_y - 5.0 * u, bw, 10.0 * u, 3.0 * u, SPECTRE.alpha(0.2));
        cv.rrect_stroke(a, line_y - 5.0 * u, bw, 10.0 * u, 3.0 * u, 1.0, SPECTRE.alpha(0.5));
    }
    let label = Text::new(Font::Mono, 9.5 * u, FAINT);
    let ticks = [(1000.0 / 160.0, "160 Hz", true), (1000.0 / 120.0, "120 Hz", false), (1000.0 / 60.0, "60 Hz", false)];
    for (ms, text, up) in ticks {
        let tx = px(ms);
        // Calé sur la grille des pixels : un trait d'un pixel à cheval sur deux
        // fait buter le remplissage anticrénelé de tiny-skia.
        cv.rect((tx - 0.5).round(), line_y - 9.0 * u, 1.0, 18.0 * u, FAINT.alpha(0.7));
        let crowded = ticks.iter().any(|(o, _, ou)| *ou == up && *o > ms && px(*o) - tx < 38.0 * u);
        if !crowded {
            cv.text(tx, if up { y + 8.0 * u } else { line_y + 22.0 * u }, label.align(Align::Center), text);
        }
    }
    // Les bouts de l'échelle, sous la règle.
    cv.text(x, line_y + 22.0 * u, label, "0");
    cv.text(x + w, line_y + 22.0 * u, label.align(Align::Right), &format!("{} ms", scale as u32));
    if let Some(m) = median {
        cv.rrect(px(m) - 1.5 * u, line_y - 12.0 * u, 3.0 * u, 24.0 * u, 2.0 * u, SPECTRE);
    }
}

/// L'aide de la carte.
pub fn ruler_help() -> &'static str {
    tr(
        "Le délai entre l'ordinateur et cet écran. Sous un repère, l'image arrive avant l'image suivante à cette fréquence.",
        "The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.",
    )
}

/// L'état de la Station : la version (titre) et une ligne (à jour, version
/// disponible, en cours, fait, refusé…) avec sa couleur ; `busy` : une
/// attente en cours (le point respire).
pub struct StationState {
    pub title: String,
    pub line: String,
    pub color: Rgba,
    pub busy: bool,
}

pub fn station_state(app: &App, h: &Host) -> StationState {
    let live = app.live.get(&h.id);
    let st = live.and_then(|l| l.station.as_ref());
    let title = match st.map(|s| s.version.as_str()).filter(|v| !v.is_empty()) {
        Some(v) => format!("KyberNow Station {v}"),
        None => "KyberNow Station".into(),
    };
    let state = |line: String, color: Rgba, busy: bool| StationState { title: title.clone(), line, color, busy };
    if let Some(op) = app.station_op.as_ref().filter(|o| o.host == h.id) {
        let doing = if op.kind == StationKind::Update { tr("Mise à jour de la Station…", "Updating the Station…") } else { tr("La Station redémarre…", "The Station is restarting…") };
        match &op.step {
            StationStep::Asking | StationStep::Waiting { .. } => return state(doing.into(), SPECTRE, true),
            StationStep::Done(m) => return state(m.clone(), OK, false),
            StationStep::Failed(m) => return state(m.clone(), WARN, false),
        }
    }
    match live.map(|l| l.online).unwrap_or_default() {
        Online::Offline => return state(tr("Hors ligne", "Offline").into(), FAINT, false),
        Online::Unknown => return state(tr("Recherche…", "Searching…").into(), FAINT, false),
        Online::Online => {}
    }
    let Some(s) = st else { return state(tr("Recherche…", "Searching…").into(), FAINT, false) };
    if !s.supported {
        return state(too_old_text().into(), WARN, false);
    }
    if s.restarting {
        return state(tr("La Station redémarre…", "The Station is restarting…").into(), SPECTRE, true);
    }
    if s.installing {
        return state(tr("Mise à jour de la Station…", "Updating the Station…").into(), SPECTRE, true);
    }
    let v = s.update_version.clone().unwrap_or_default();
    if s.available && s.requires_setup {
        return state(requires_setup_text(&v), WARN, false);
    }
    if s.available {
        return state(format!("{} {v} {}", tr("Version", "Version"), tr("disponible", "available")), SPECTRE, false);
    }
    state(tr("À jour", "Up to date").into(), OK, false)
}

/// Le bloc de l'état : la version en titre, l'état dessous (un paragraphe) ;
/// rend sa hauteur.
pub fn station_block(app: &App, cv: &mut Canvas, h: &Host, x: f32, y: f32, w: f32, u: f32) -> f32 {
    let s = station_state(app, h);
    cv.text_fit(x, y + 20.0 * u, Text::new(Font::Semibold, 18.0 * u, FROST), &s.title, w);
    let dot_x = x + 4.0 * u;
    let line_y = y + 20.0 * u + 26.0 * u;
    let a = if s.busy && !app.reduce_motion { 0.55 + 0.45 * (app.screen_since.elapsed().as_secs_f32() * 3.0).sin().abs() } else { 1.0 };
    cv.circle(dot_x, line_y - 5.0 * u, 4.0 * u, s.color.alpha(a));
    let lh = cv.paragraph(x + 16.0 * u, line_y, Text::new(Font::Regular, 14.5 * u, s.color), &s.line, w - 16.0 * u, 21.0 * u);
    20.0 * u + 26.0 * u + lh - 14.0 * u + 18.0 * u
}

/// Le QR code d'Admin (`https://adresse:port/`) sur une carte claire, la
/// phrase et l'adresse en clair dessous ; rend la hauteur. Les modules sont
/// calés sur les pixels de l'écran, la marge fait quatre modules.
pub fn admin_qr(cv: &mut Canvas, h: &Host, x: f32, y: f32, w: f32, u: f32) -> f32 {
    let url = admin_url(&h.address, h.port);
    let Ok(qr) = qrcodegen::QrCode::encode_text(&url, qrcodegen::QrCodeEcc::Medium) else { return 0.0 };
    let n = qr.size();
    let k = cv.k;
    let side = (w * 0.78).min(240.0 * u);
    // Taille d'un module en pixels entiers : un QR net, lisible de loin.
    let module_px = ((side * k) / (n + 8) as f32).floor().max(1.0);
    let card_px = module_px * (n + 8) as f32;
    let card = card_px / k;
    let cx = ((x + (w - card) / 2.0) * k).round() / k;
    let cy = (y * k).round() / k;
    cv.rrect(cx - 6.0 * u, cy - 6.0 * u, card + 12.0 * u, card + 12.0 * u, 12.0 * u, Rgba(255, 255, 255, 255));
    let m = module_px / k;
    for my in 0..n {
        for mx in 0..n {
            if qr.get_module(mx, my) {
                cv.rect(cx + (mx + 4) as f32 * m, cy + (my + 4) as f32 * m, m, m, INK);
            }
        }
    }
    let mut yy = cy + card + 6.0 * u + 26.0 * u;
    let text = Text::new(Font::Regular, 14.0 * u, MUTED).align(Align::Center);
    yy += cv.paragraph(x + w / 2.0, yy, text, tr("Scannez ce code avec votre téléphone pour ouvrir Admin.", "Scan this code with your phone to open Admin."), w, 20.0 * u);
    yy += 2.0 * u;
    cv.text_fit(x + w / 2.0, yy, Text::new(Font::Mono, 12.0 * u, FAINT).align(Align::Center), &url, w);
    yy + 8.0 * u - y
}
