//! Accueil, d'après les maquettes : l'ordinateur choisi en plein cadre (son
//! image ou sa signature), son état, l'écran virtuel qui l'attend, « Se
//! connecter » et les profils ; à droite la dernière session (bureau) ou
//! « Prêt à jouer » (salon), sur un panneau d'encre ; en bas les ordinateurs ;
//! à la manette, la légende des boutons.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, card, eyebrow, legend, legend_width, panel, primary, segments};
use super::{launch, shade, signature, title_bar, unit, Frame};
use crate::app::{App, Layout, NetKind, Online, Rect, Target};
use crate::i18n::{decimal, tr};
use crate::model::{Host, Profile};

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    draw_body(app, cv, f, true);
}

/// L'accueil ; `logo` : dessiner le logo de l'en-tête (le lancement le pose lui-même).
pub fn draw_body(app: &App, cv: &mut Canvas, f: &mut Frame, logo: bool) {
    let host = app.selected();
    backdrop(app, cv, host);
    match app.layout {
        Layout::Desktop => desktop(app, cv, f, host),
        Layout::Handheld | Layout::Tv => console(app, cv, f, host, logo),
    }
}

pub fn profile_label(p: Profile) -> &'static str {
    match p {
        Profile::Jeu => tr("Jeu", "Game"),
        Profile::Bureau => tr("Bureau", "Desktop"),
        Profile::Max => "Max",
    }
}

/// Le fond : l'image de l'ordinateur (assombrie hors ligne), sinon sa signature.
fn backdrop(app: &App, cv: &mut Canvas, host: Option<&Host>) {
    let offline = host.and_then(|h| app.live.get(&h.id)).is_some_and(|l| l.online == Online::Offline);
    match host.and_then(|h| app.images.get(&h.id)) {
        Some(img) => {
            cv.clear(INK_BG);
            let (w, h) = (cv.w, cv.h);
            cv.image_cover(img, 0.0, 0.0, w, h, if offline { 0.35 } else { 0.92 });
        }
        None => signature(cv, host.and_then(|h| h.fingerprint.as_deref()), offline),
    }
    shade(cv);
}

/// État de l'ordinateur : texte et couleur.
fn status(app: &App, h: &Host) -> (&'static str, Rgba, bool) {
    match app.live.get(&h.id).map(|l| l.online).unwrap_or_default() {
        Online::Online => (tr("En ligne, disponible", "Online, available"), OK, true),
        Online::Offline => (tr("Hors ligne", "Offline"), FAINT, false),
        Online::Unknown => (tr("Recherche sur le réseau…", "Looking on the network…"), FAINT, false),
    }
}

fn meta(h: &Host) -> String {
    let mut v = Vec::new();
    if let Some(os) = &h.os {
        v.push(os.clone());
    }
    if let Some(g) = &h.gpu {
        v.push(g.clone());
    }
    v.push(h.address.clone());
    v.join(" · ")
}

/// « 1280 × 800 · 90 Hz » : l'écran virtuel qui attend l'appareil.
fn virtual_display(app: &App, h: &Host) -> (String, &'static str) {
    let (w, hh, fps) = app.session_mode(h);
    let like = if h.settings.resolution != 0 || h.settings.fps != 0 {
        tr(", sur mesure", ", custom")
    } else {
        match app.layout {
            Layout::Desktop => tr(", comme cet écran", ", like this screen"),
            Layout::Handheld if app.machine.model.contains("Steam Deck") => tr(", comme le Deck", ", like the Deck"),
            Layout::Handheld => tr(", comme cet écran", ", like this screen"),
            Layout::Tv => tr(", comme le téléviseur", ", like the TV"),
        }
    };
    (format!("{w} × {hh} · {fps} Hz"), like)
}

fn vrr_on(app: &App, h: &Host) -> bool {
    app.machine.vrr == Some(true) && matches!(h.settings.display.as_str(), "auto" | "vrr")
}

/// Une heure locale « 19:42 ».
fn clock() -> String {
    crate::app_time::local_hhmm()
}

// MARK: - Bureau

fn desktop(app: &App, cv: &mut Canvas, f: &mut Frame, host: Option<&Host>) {
    let top = title_bar(app, cv, f);
    let (w, h) = (cv.w, cv.h);
    let x0 = 48.0;
    let panel_w = 320.0;
    let wide = w >= 1080.0;
    let hero_max = if wide { w - 2.0 * x0 - panel_w - 48.0 } else { w - 2.0 * x0 };
    let mut y = top + 44.0;
    match host {
        Some(hst) => {
            let (st, color, glow) = status(app, hst);
            widgets::dot(cv, x0 + 4.0, y + 7.0, 4.0, color, glow);
            eyebrow(cv, x0 + 16.0, y + 11.0, 11.0, color, st);
            y += 26.0;
            cv.text_shrink(x0 - 2.0, y + 46.0, Text::new(Font::Display, 60.0, FROST).tracking(-1.2), &hst.name, hero_max);
            y += 68.0;
            cv.text_fit(x0, y + 12.0, Text::new(Font::Mono, 13.0, MUTED), &meta(hst), hero_max);
            y += 28.0;
            if hst.settings.virtual_display {
                let (mode, like) = virtual_display(app, hst);
                let vrr = if vrr_on(app, hst) { " · VRR" } else { "" };
                widgets::info_chip(cv, x0, y, 36.0, 14.0, Icon::Monitor, &[(tr("Écran virtuel à ", "Virtual display at "), false), (&mode, true), (like, false), (vrr, true)]);
            } else {
                widgets::info_chip(cv, x0, y, 36.0, 14.0, Icon::Monitor, &[(tr("Écrans de l'ordinateur tels quels", "Computer's screens as they are"), false)]);
            }
            y += 36.0 + 24.0;
            let mut x = x0;
            let offline = app.live.get(&hst.id).is_some_and(|l| l.online == Online::Offline);
            if offline && hst.mac.is_some() {
                x += primary(cv, f, Target::Wake, x, y, 52.0, tr("Réveiller l'ordinateur", "Wake the computer"), false) + 18.0;
            } else {
                x += primary(cv, f, Target::Connect, x, y, 52.0, tr("Se connecter", "Connect"), true) + 18.0;
            }
            let items: Vec<(Target, String, bool, bool)> = Profile::ALL.iter().map(|p| (Target::Chip(*p), profile_label(*p).to_string(), hst.settings.profile == *p, false)).collect();
            x += segments(cv, f, x, y + 3.0, 46.0, 14.0, &items) + 18.0;
            widgets::link(cv, f, Target::HostSettings, x, y + 31.0, 14.0, tr("Réglages de l'ordinateur", "Computer settings"));
            y += 52.0 + 26.0;
            let src = if app.images.contains_key(&hst.id) {
                if app.image_is_last.get(&hst.id).copied().unwrap_or(false) { tr("La dernière image de votre dernière session.", "The last picture of your last session.") } else { tr("Un aperçu de son écran.", "A preview of its screen.") }
            } else {
                tr("Sa signature, tirée de son empreinte.", "Its signature, from its fingerprint.")
            };
            let m = &app.machine;
            let this = format!("{} {} · {} · {}", tr("Cet ordinateur :", "This computer:"), m.os, short_gpu(&m.gpu), decoders_short(m));
            cv.text_fit(x0, y, Text::new(Font::Regular, 13.0, FAINT), &format!("{src} {this}"), hero_max);
            if wide {
                last_panel(app, cv, hst, Rect::new(w - x0 - panel_w, top + 46.0, panel_w, 0.0));
            }
        }
        None => welcome(cv, f, x0, y, 1.0, hero_max),
    }
    // Les ordinateurs.
    let ch = 118.0;
    let cy = h - 40.0 - ch;
    eyebrow(cv, x0, cy - 14.0, 11.0, FAINT, tr("Vos ordinateurs", "Your computers"));
    cards(app, cv, f, x0, cy, 244.0, ch, 14.0, w - 2.0 * x0, 1.0, true);
}

/// Le panneau « Dernière session » : latence sur la règle d'une image.
fn last_panel(app: &App, cv: &mut Canvas, h: &Host, at: Rect) {
    let pad = 22.0;
    let height = 206.0;
    panel(cv, Rect::new(at.x, at.y, at.w, height), 14.0);
    let x = at.x + pad;
    let iw = at.w - 2.0 * pad;
    let mut y = at.y + pad + 11.0;
    eyebrow(cv, x, y, 11.0, FAINT, tr("Dernière session", "Last session"));
    let Some(last) = &h.last else {
        cv.paragraph(x, y + 32.0, Text::new(Font::Regular, 14.0, MUTED), tr("Pas encore de session depuis cet appareil. La latence de chacune s'affichera ici.", "No session from this device yet. Each one's latency will show here."), iw, 21.0);
        return;
    };
    y += 46.0;
    let measured = last.median > 0.0;
    let value = if measured { format!("{} ms", decimal(last.median, 1)) } else { "— ms".into() };
    let lw = cv.text(x, y, Text::new(Font::MonoMedium, 40.0, SPECTRE).tracking(-1.0), &value);
    cv.text(x + lw + 10.0, y, Text::new(Font::Mono, 12.0, FAINT), if measured { tr("médiane", "median") } else { tr("non mesurée", "not measured") });
    y += 18.0;
    // La règle d'une image, à la fréquence de cet écran.
    let frame = 1000.0 / app.machine.refresh.max(30) as f64;
    cv.rrect(x, y, iw, 10.0, 5.0, INK_3);
    cv.rrect_stroke(x, y, iw, 10.0, 5.0, 1.0, LINE);
    let fill = (last.median / frame).clamp(0.02, 1.0) as f32;
    cv.rrect(x, y, iw * fill, 10.0, 5.0, FLAT);
    let p95 = (last.p95 / frame).clamp(0.0, 1.0) as f32;
    cv.rect(x + iw * p95 - 1.0, y - 5.0, 2.0, 20.0, FROST.alpha(0.7));
    y += 28.0;
    let mono = Text::new(Font::Mono, 11.0, FAINT);
    cv.text(x, y, mono, "0");
    cv.text(x + iw, y, mono.align(Align::Right), &format!("{} · {} ms", tr("une image", "one frame"), decimal(frame, 1)));
    y += 24.0;
    let m12 = Text::new(Font::Mono, 12.0, MUTED);
    let w = cv.text(x, y, m12, &format!("{} ", tr("95e centile", "95th percentile")));
    cv.text(x + w, y, m12.color(FROST), &if measured { format!("{} ms", decimal(last.p95, 1)) } else { "—".into() });
    y += 20.0;
    cv.text_fit(x, y, m12, &format!("{} · {}", duration(last.seconds), when(last.at)), iw);
    y += 20.0;
    let mbps = if last.mbps > 0.0 { format!(" · {:.0} Mbit/s", last.mbps) } else { String::new() };
    cv.text_fit(x, y, m12, &format!("{}{mbps}", last.mode), iw);
}

fn duration(s: u64) -> String {
    let m = s / 60;
    if m >= 60 { format!("{} h {:02}", m / 60, m % 60) } else { format!("{m} min") }
}

/// « hier, 21:40 », « aujourd'hui, 9:12 », « 12/10 ».
fn when(at: u64) -> String {
    crate::app_time::relative_day(at)
}

fn short_gpu(g: &str) -> String {
    // « Mesa Gallium driver 25.1 for AMD Radeon 780M (radeonsi, …) » → « AMD Radeon 780M ».
    let g = g.split(" for ").nth(1).unwrap_or(g);
    let g = g.split(" (").next().unwrap_or(g);
    if g.is_empty() { tr("carte graphique inconnue", "unknown GPU").into() } else { g.trim().to_string() }
}

fn decoders_short(m: &crate::app::Machine) -> String {
    let mut v = Vec::new();
    if m.hw_hevc {
        v.push("HEVC");
    }
    if m.hw_h264 {
        v.push("H.264");
    }
    if m.hw_av1 {
        v.push("AV1");
    }
    if m.vaapi && !v.is_empty() { format!("{} {}", v.join(", "), tr("par VA-API", "via VA-API")) } else { tr("décodage par le processeur", "CPU decoding").into() }
}

/// Sans ordinateur : bienvenue et ajout.
fn welcome(cv: &mut Canvas, f: &mut Frame, x0: f32, y: f32, u: f32, max: f32) {
    eyebrow(cv, x0, y + 11.0 * u, 11.0 * u, FAINT, tr("Bienvenue", "Welcome"));
    let style = Text::new(Font::Display, 52.0 * u, FROST).tracking(-1.0 * u);
    cv.text_shrink(x0 - 2.0, y + 72.0 * u, style, tr("Votre ordinateur,", "Your computer,"), max);
    cv.text_shrink(x0 - 2.0, y + 130.0 * u, style, tr("sur cet appareil.", "on this device."), max);
    cv.paragraph(
        x0,
        y + 170.0 * u,
        Text::new(Font::Regular, 15.0 * u, MUTED),
        tr(
            "Installez KyberNow Station sur l'ordinateur Windows, puis ajoutez-le ici : il s'affiche en bas dès qu'il est sur le réseau.",
            "Install KyberNow Station on the Windows computer, then add it here: it shows up below as soon as it is on the network.",
        ),
        max.min(620.0 * u),
        23.0 * u,
    );
    primary(cv, f, Target::AddCard, x0, y + 226.0 * u, 52.0 * u, tr("Ajouter un ordinateur", "Add a computer"), false);
}

/// La rangée des ordinateurs : enregistrés, trouvés sur le réseau, ajouter.
#[allow(clippy::too_many_arguments)]
fn cards(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y: f32, cw: f32, ch: f32, gap: f32, width: f32, u: f32, add_card: bool) {
    let fit = (((width + gap) / (cw + gap)).floor() as usize).max(1);
    let selected = app.selected().map(|h| h.id.clone());
    let mut slots = fit;
    let found = app.new_found();
    if add_card {
        slots = slots.saturating_sub(1);
    }
    let mut x = x0;
    for (i, h) in app.store.hosts.iter().enumerate() {
        if slots == 0 {
            break;
        }
        let live = app.live.get(&h.id).map(|l| l.online).unwrap_or_default();
        let (dot, lines): (Rgba, Vec<String>) = match live {
            Online::Online => (OK, vec![short_host(h), h.address.clone()]),
            Online::Offline => (GHOST, vec![tr("Hors ligne", "Offline").into(), h.address.clone()]),
            Online::Unknown => (FAINT, vec![short_host(h), h.address.clone()]),
        };
        let lines: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let lines = if app.layout == Layout::Desktop { lines } else { lines[..1].to_vec() };
        card(cv, f, Target::Host(i), Rect::new(x, y, cw, ch), u, &h.name, Some(dot), None, &lines, selected.as_deref() == Some(&h.id), false, live == Online::Offline);
        x += cw + gap;
        slots -= 1;
    }
    for (i, fd) in found.iter().enumerate() {
        if slots == 0 {
            break;
        }
        let fp = fd.fingerprint.as_deref().map(|p| format!("fp {}", crate::app::short_fp(p))).unwrap_or_default();
        let lines: Vec<&str> = if app.layout == Layout::Desktop { vec![tr("Annoncé par Admin", "Announced by Admin"), &fp] } else { vec![&fp] };
        let badge = if app.layout == Layout::Desktop { tr("Sur le réseau", "On the network") } else { tr("Réseau", "Network") };
        card(cv, f, Target::Found(i), Rect::new(x, y, cw, ch), u, &fd.name, None, Some(badge), &lines, false, true, false);
        x += cw + gap;
        slots -= 1;
    }
    if add_card {
        let r = Rect::new(x, y, cw, ch);
        let (focused, hover) = f.state(Target::AddCard, r);
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { Rgba(20, 27, 35, 235) } else { CARD });
        cv.rrect_dashed(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE_2);
        let label = tr("Ajouter un ordinateur", "Add a computer");
        let st = Text::new(Font::Regular, 15.0 * u, MUTED);
        let tw = cv.text_width(st, label) + 28.0 * u;
        let tx = r.x + (r.w - tw) / 2.0;
        cv.icon(&Icon::Plus, tx, r.y + r.h / 2.0 - 10.0 * u, 20.0 * u, 2.0, MUTED);
        cv.text_fit(tx + 28.0 * u, r.y + r.h / 2.0 + 5.0 * u, st, label, r.w - 40.0 * u);
    }
}

fn short_host(h: &Host) -> String {
    let os = h.os.as_deref().unwrap_or("").replace(" Pro", "").replace(" Home", "").replace(" Famille", "");
    let os: String = os.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    let gpu = h.gpu.as_deref().unwrap_or("").replace("NVIDIA ", "").replace("GeForce ", "").replace("AMD ", "").replace("Radeon ", "");
    [os, gpu].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

// MARK: - Console (Deck, salon)

fn console(app: &App, cv: &mut Canvas, f: &mut Frame, host: Option<&Host>, logo: bool) {
    let u = unit(app.layout);
    let tv = app.layout == Layout::Tv;
    let (w, h) = (cv.w, cv.h);
    let x0 = if tv { 96.0 } else { 40.0 };
    let header = 56.0 * u + if tv { 12.0 } else { 0.0 };
    // En-tête : logo, réseau, batterie, heure.
    if logo {
        let (lx, ly, lh) = launch::header_logo(app.layout);
        cv.logo_link(lx, ly, lh, 1.0);
    }
    status_line(app, cv, w - x0, header / 2.0 + 5.0 * u, u);
    let ready_w = if tv { 560.0 } else { 0.0 };
    let hero_max = if tv { w - 2.0 * x0 - ready_w - 64.0 } else { (w - 2.0 * x0).min(820.0) };
    let mut y = header + 36.0 * u;
    match host {
        Some(hst) => {
            let (st, color, glow) = status(app, hst);
            widgets::dot(cv, x0 + 4.0 * u, y + 8.0 * u, 4.0 * u, color, glow);
            eyebrow(cv, x0 + 16.0 * u, y + 12.0 * u, 12.0 * u, color, st);
            y += 28.0 * u;
            // ‹ nom › : la croix change d'ordinateur quand « Se connecter » a le focus.
            let many = app.store.hosts.len() > 1;
            let arrows = Text::new(Font::Mono, 22.0 * u, GHOST);
            let mut nx = x0;
            if many {
                nx += cv.text(x0, y + 44.0 * u, arrows, "‹") + 18.0 * u;
            }
            let name_w = cv.text_shrink(nx - 2.0, y + 50.0 * u, Text::new(Font::Display, 64.0 * u, FROST).tracking(-1.3 * u), &hst.name, hero_max - 140.0 * u);
            if many {
                let ax = nx + name_w + 18.0 * u;
                let aw = cv.text(ax, y + 44.0 * u, arrows, "›");
                let n = app.selected_index().unwrap_or(0) + 1;
                cv.text(ax + aw + 14.0 * u, y + 42.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT), &format!("{n} / {}", app.store.hosts.len()));
            }
            y += 72.0 * u;
            cv.text_fit(x0, y + 10.0 * u, Text::new(Font::Mono, 14.0 * u, MUTED), &meta(hst), hero_max);
            y += 26.0 * u;
            if hst.settings.virtual_display {
                let (mode, like) = virtual_display(app, hst);
                let vrr = if vrr_on(app, hst) { " · VRR" } else { "" };
                widgets::info_chip(cv, x0, y, 40.0 * u, 16.0 * u, Icon::Monitor, &[(tr("Écran virtuel à ", "Virtual display at "), false), (&mode, true), (like, false), (vrr, true)]);
            } else {
                widgets::info_chip(cv, x0, y, 40.0 * u, 16.0 * u, Icon::Monitor, &[(tr("Écrans de l'ordinateur tels quels", "Computer's screens as they are"), false)]);
            }
            y += 40.0 * u + 28.0 * u;
            let mut x = x0;
            let offline = app.live.get(&hst.id).is_some_and(|l| l.online == Online::Offline);
            if offline && hst.mac.is_some() {
                x += primary(cv, f, Target::Wake, x, y, 60.0 * u, tr("Réveiller l'ordinateur", "Wake the computer"), false) + 22.0 * u;
            } else {
                x += primary(cv, f, Target::Connect, x, y, 60.0 * u, tr("Se connecter", "Connect"), true) + 22.0 * u;
            }
            let items: Vec<(Target, String, bool, bool)> = Profile::ALL.iter().map(|p| (Target::Chip(*p), profile_label(*p).to_string(), hst.settings.profile == *p, false)).collect();
            segments(cv, f, x, y + 4.0 * u, 52.0 * u, 17.0 * u, &items);
            y += 60.0 * u + 26.0 * u;
            let note = match &hst.last {
                Some(l) if l.median > 0.0 => format!("{} · {} ms {} · {}, {}", source(app, hst), decimal(l.median, 1), tr("médiane", "median"), duration(l.seconds), when(l.at)),
        Some(l) => format!("{} · {}, {}", source(app, hst), duration(l.seconds), when(l.at)),
                None => source(app, hst).to_string(),
            };
            cv.text_fit(x0, y, Text::new(Font::Regular, 14.0 * u, FAINT), &note, hero_max);
            if tv {
                ready_panel(app, cv, Rect::new(w - x0 - ready_w, header + 40.0, ready_w, 0.0), u);
            }
        }
        None => welcome(cv, f, x0, y, u, hero_max),
    }

    // Les ordinateurs.
    let footer = 56.0 * u + if tv { 4.0 } else { 0.0 };
    let (cw, ch) = if tv { (320.0, 150.0) } else { (224.0, 112.0) };
    let cy = h - footer - 28.0 * u - ch;
    eyebrow(cv, x0, cy - 14.0 * u, 12.0 * u, FAINT, tr("Vos ordinateurs", "Your computers"));
    cards(app, cv, f, x0, cy, cw, ch, 14.0 * u, w - 2.0 * x0, u, true);

    // La légende des boutons.
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut x = x0;
    if host.is_some() {
        x += legend(cv, x, gy, g, &["A"], tr("Se connecter", "Connect")) + 28.0 * u;
        x += legend(cv, x, gy, g, &["Y"], tr("Réglages de l'ordinateur", "Computer settings")) + 28.0 * u;
    }
    x += legend(cv, x, gy, g, &["X"], tr("Ajouter un ordinateur", "Add a computer")) + 28.0 * u;
    if host.is_some() {
        legend(cv, x, gy, g, &["L1", "R1"], tr("Profil", "Profile"));
    }
    let label = tr("Réglages de l'app", "App settings");
    let lw = legend_width(cv, g, &["☰"], label);
    let r = Rect::new(w - x0 - lw - 8.0, gy - g * 0.7, lw + 16.0, g * 1.4);
    let (focused, hover) = f.state(Target::AppSettings, r);
    if focused || hover {
        cv.rrect(r.x, r.y, r.w, r.h, 8.0, Rgba(227, 237, 240, 18));
    }
    if focused {
        widgets::ring(cv, r, 8.0);
    }
    legend(cv, w - x0 - lw, gy, g, &["☰"], label);
    if let crate::app::UpdateState::Available { version, .. } = &app.update {
        let s = format!("{} {version} {}", tr("Version", "Version"), tr("disponible", "available"));
        cv.text(w - x0 - lw - 28.0 * u, gy + 5.0 * u, Text::new(Font::Mono, 12.0 * u, SPECTRE).align(Align::Right), &s.to_uppercase());
    }
}

fn source(app: &App, h: &Host) -> &'static str {
    if app.images.contains_key(&h.id) {
        if app.image_is_last.get(&h.id).copied().unwrap_or(false) { tr("La dernière image de votre dernière session", "The last picture of your last session") } else { tr("Un aperçu de son écran", "A preview of its screen") }
    } else {
        tr("Sa signature, tirée de son empreinte", "Its signature, from its fingerprint")
    }
}

/// Réseau, batterie et heure, alignés à droite en `right`.
fn status_line(app: &App, cv: &mut Canvas, right: f32, y: f32, u: f32) {
    let mono = Text::new(Font::Mono, 13.0 * u, MUTED);
    let mut parts: Vec<(Option<Icon>, String)> = Vec::new();
    match app.net.kind {
        NetKind::Wifi => parts.push((Some(Icon::Wifi), match &app.net.band { Some(b) => format!("Wi-Fi · {b}"), None => "Wi-Fi".into() })),
        NetKind::Ethernet => parts.push((Some(Icon::Ethernet), match app.net.link_mbps { Some(m) if m >= 1000 => format!("Ethernet {}", gbit(m)), Some(m) => format!("Ethernet {m} Mbit/s"), None => "Ethernet".into() })),
        NetKind::Unknown => {}
    }
    if let Some((pct, _)) = app.machine.battery {
        parts.push((Some(Icon::Battery), format!("{pct} %")));
    }
    parts.push((None, clock()));
    let gap = 16.0 * u;
    let isz = 16.0 * u;
    let mut total = 0.0;
    let mut widths = Vec::new();
    for (i, s) in &parts {
        let w = cv.text_width(mono, s) + if i.is_some() { isz + 6.0 * u } else { 0.0 };
        widths.push(w);
        total += w + gap;
    }
    let mut x = right - total + gap;
    for ((i, s), w) in parts.iter().zip(widths) {
        let mut tx = x;
        if let Some(icon) = i {
            cv.icon(icon, tx, y - isz + 3.0 * u, isz, 1.8, MUTED);
            tx += isz + 6.0 * u;
        }
        cv.text(tx, y, mono, s);
        x += w + gap;
    }
}

fn gbit(m: u32) -> String {
    if m % 1000 == 0 { format!("{} Gbit/s", m / 1000) } else { format!("{} Gbit/s", decimal(m as f64 / 1000.0, 1)) }
}

/// La colonne « Prêt à jouer » du salon, sur son panneau.
fn ready_panel(app: &App, cv: &mut Canvas, at: Rect, u: f32) {
    let m = &app.machine;
    let pad_x = 28.0;
    let mut rows: Vec<(Icon, String, String)> = Vec::new();
    if app.pads.is_empty() {
        rows.push((Icon::Gamepad, tr("Aucune manette", "No controller").into(), tr("Branchez-la, ou associez-la en Bluetooth", "Plug it in, or pair it over Bluetooth").into()));
    } else {
        let title = if app.pads.len() == 1 { format!("1 {}", tr("manette", "controller")) } else { format!("{} {}", app.pads.len(), tr("manettes", "controllers")) };
        let detail = app
            .pads
            .iter()
            .map(|p| format!("{} · {}{}{}", short_pad(&p.name), tr("joueur ", "player "), p.slot + 1, if p.rumble { tr(", vibrations", ", rumble") } else { "" }))
            .collect::<Vec<_>>()
            .join("\n");
        rows.push((Icon::Gamepad, title, detail));
    }
    let sound = match m.audio_channels {
        8.. => "7.1",
        6..=7 => "5.1",
        _ => tr("Stéréo", "Stereo"),
    };
    rows.push((Icon::Sound, format!("{} {sound}", tr("Son", "Sound")), m.audio_name.clone()));
    let (dw, dh) = m.display;
    let res = if dh >= 2160 { "4K".to_string() } else { format!("{dh}p") };
    let vrr = match m.vrr {
        Some(true) => " · VRR",
        _ => "",
    };
    let _ = dw;
    rows.push((Icon::Monitor, format!("{res} · {} Hz{vrr}", m.refresh), if m.display_name.is_empty() { m.os.clone() } else { m.display_name.clone() }));
    rows.push((Icon::Cpu, decoders_short(m), short_gpu(&m.gpu)));
    // Hauteur selon le contenu.
    let line = 26.0;
    let mut height = 24.0 + 15.0 + 22.0 + 22.0;
    for (_, _, d) in &rows {
        height += 86.0 + (d.lines().count().max(1) - 1) as f32 * line;
    }
    panel(cv, Rect::new(at.x, at.y, at.w, height), 18.0);
    let x = at.x + pad_x;
    let mut y = at.y + 24.0 + 15.0;
    eyebrow(cv, x, y, 15.0, FAINT, tr("Prêt à jouer", "Ready to play"));
    y += 22.0;
    let n = rows.len();
    for (i, (icon, title, detail)) in rows.iter().enumerate() {
        let top = y + 16.0;
        cv.rrect(x, top, 40.0, 40.0, 10.0, INK_3);
        cv.icon(icon, x + 9.0, top + 9.0, 22.0, 1.8, SPECTRE);
        cv.text_fit(x + 56.0, top + 22.0, Text::new(Font::Semibold, 20.0, FROST), title, at.w - 2.0 * pad_x - 56.0);
        let mut yy = top + 22.0 + 26.0;
        for l in detail.lines() {
            cv.text_fit(x + 56.0, yy, Text::new(Font::Mono, 15.0, FAINT), l, at.w - 2.0 * pad_x - 56.0);
            yy += line;
        }
        y = yy - line + 22.0;
        if i + 1 < n {
            cv.rect(x, y, at.w - 2.0 * pad_x, 1.0, LINE);
        }
    }
    let _ = u;
}

fn short_pad(name: &str) -> String {
    let n = name.replace("Controller", "").replace("Wireless", "").replace("  ", " ");
    let n = n.trim();
    if n.contains("Xbox") {
        "Xbox".into()
    } else if n.contains("DualSense") || n.contains("PS5") {
        "DualSense".into()
    } else if n.contains("Steam Deck") {
        "Steam Deck".into()
    } else {
        n.to_string()
    }
}
