//! Réglages (docs/link-ui.md) : ceux de l'ordinateur choisi (« Réglages de
//! {nom} » : le profil en tête, puis Image, Son, Commandes, Réseau,
//! Statistiques et Station), ou ceux de l'app (ce téléviseur, la langue, les
//! manettes). Rubriques à gauche, lignes à droite ; gauche et droite changent
//! la valeur, OK agit.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, Key, Value};
use super::Frame;
use crate::app::{self, App, Dialog, NetKind, Online, Rect, Section, Setting, StationKnown, StationOp, StationPhase, Target, UpdateState};
use crate::i18n::{decimal, english, tr};
use crate::model::{Host, LastSession, Profile};

/// La colonne des lignes.
const X: f32 = 600.0;
const WIDTH: f32 = W - 110.0 - 600.0;
const GAP: f32 = 14.0;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    let host = if app.settings_global { None } else { app.selected() };
    if !app.settings_global && host.is_none() {
        return;
    }

    // En tête : le titre, et le profil pour un ordinateur.
    cv.rect(520.0, 0.0, 1.0, H, LINE);
    let mut y = match host {
        Some(h) => {
            widgets::label(cv, 96.0, 104.0, if crate::i18n::english() { "Settings for" } else if crate::i18n::de(&h.name) == "d'" { "Réglages d'" } else { "Réglages de" });
            cv.text_fit(92.0, 172.0, Text::new(Font::Title, 56.0, FROST).tracking(-1.5), &h.name, 410.0);
            widgets::label(cv, 96.0, 240.0, tr("Profil", "Profile"));
            let custom = h.settings.is_custom();
            let mut x = 96.0;
            for p in Profile::ALL {
                x += widgets::chip(cv, f, Target::Chip(p), x, 262.0, super::home::profile_label(p), !custom && h.settings.profile == p) + 10.0;
            }
            let phrase = if custom { tr("Personnalisé : vos propres réglages.", "Custom: your own settings.") } else { super::home::profile_phrase(h.settings.profile) };
            cv.paragraph(98.0, 356.0, Text::new(Font::Regular, 19.0, FROST_3), phrase, 384.0, 26.0);
            420.0
        }
        None => {
            widgets::label(cv, 96.0, 104.0, "KyberNow Link");
            let used = cv.paragraph(92.0, 170.0, Text::new(Font::Title, 48.0, FROST).tracking(-1.2), tr("Réglages de l'app", "App settings"), 410.0, 56.0);
            170.0 + used
        }
    };

    // Rubriques.
    for (i, sec) in app.sections().iter().enumerate() {
        let label = sec.label();
        let r = Rect::new(96.0, y, 384.0, 72.0);
        let focused = f.hit(Target::Section(i), r);
        let current = app.section == i;
        if focused {
            cv.neon(r.x, r.y, r.w, r.h, 14.0, 0.8);
            cv.rrect(r.x, r.y, r.w, r.h, 14.0, CORE);
            cv.text_fit(r.x + 26.0, r.y + 46.0, Text::new(Font::Semibold, 25.0, INK), label, r.w - 40.0);
        } else {
            if current {
                cv.rrect(r.x, r.y, r.w, r.h, 14.0, SPECTRE.alpha(0.14));
            }
            cv.text_fit(r.x + 26.0, r.y + 46.0, Text::new(if current { Font::Semibold } else { Font::Regular }, 25.0, FROST), label, r.w - 40.0);
        }
        y += 84.0;
    }

    // Lignes.
    match host {
        Some(h) => host_section(app, cv, f, h),
        None => global(app, cv, f),
    }
    footer(cv);
}

fn host_section(app: &App, cv: &mut Canvas, f: &mut Frame, h: &Host) {
    let s = |s: Setting| Some(Target::Setting(s));
    let (x, w) = (X, WIDTH);
    let mut y = 88.0;
    let st = &h.settings;
    match app.current_section() {
        Section::Picture => {
            let (mw, mh, _) = app.session_mode(h);
            let res = if st.resolution == 0 { format!("{} · {mw} × {mh}", tr("Comme cet écran", "Match this display")) } else { format!("{mw} × {mh}") };
            y += widgets::row(cv, f, s(Setting::Resolution), x, y, w, tr("Définition", "Resolution"), tr("Plus bas : moins de débit, moins de latence", "Lower: less bitrate, less latency"), Value::Text(res)) + GAP;
            let fps_opts: Vec<String> = vec!["Auto".into(), "30 Hz".into(), "60 Hz".into(), "120 Hz".into()];
            let fi = match st.fps {
                30 => 1,
                60 => 2,
                120 => 3,
                _ => 0,
            };
            let hz = app.tv.refresh;
            // L'écran reste à sa fréquence : plus bas, chaque image y est montrée plusieurs fois.
            let fps_help = if english() {
                format!("Auto: this screen's ({hz} Hz). Lower, each frame is shown several times.")
            } else {
                format!("Auto : celle de cet écran ({hz} Hz). Plus bas, chaque image s'affiche plusieurs fois.")
            };
            y += widgets::row(cv, f, s(Setting::Fps), x, y, w, tr("Fréquence", "Refresh rate"), &fps_help, Value::Choices(&fps_opts, fi)) + GAP;
            let codec = codec_name(app.codec_for(h));
            let codec_v = if st.codec == "auto" { format!("Auto · {codec}") } else { codec.into() };
            y += widgets::row(cv, f, s(Setting::Codec), x, y, w, tr("Format vidéo", "Video format"), tr("HEVC jusqu'à 120 Hz ; AV1 jusqu'à 60 Hz sur les téléviseurs LG", "HEVC up to 120 Hz; AV1 up to 60 Hz on LG TVs"), Value::Text(codec_v)) + GAP;
            let mbps = if st.mbps == 0 {
                if english() { format!("Auto · up to {} Mbit/s", app.net.cap_mbps()) } else { format!("Auto · au plus {} Mbit/s", app.net.cap_mbps()) }
            } else {
                format!("{} Mbit/s", st.mbps)
            };
            y += widgets::row(cv, f, s(Setting::Bitrate), x, y, w, tr("Débit", "Bitrate"), tr("Proportionné au profil, plafonné par le réseau du téléviseur", "Scaled to the profile, capped by the TV's network"), Value::Text(mbps)) + GAP;
            y += widgets::row(cv, f, s(Setting::GameHint), x, y, w, tr("Rappeler le mode jeu de la télé", "Remind about the TV's game mode"), tr("L'app ne peut pas passer la télé en Optimiseur de jeu : le menu le rappelle", "The app cannot switch the TV to Game Optimizer: the menu reminds you"), Value::Toggle(st.game_mode_hint)) + GAP;
            widgets::row(cv, f, None, x, y, w, tr("Fréquence variable (VRR)", "Variable refresh rate (VRR)"), tr("LG la réserve aux entrées HDMI ; l'ordinateur envoie ses images au rythme de l'écran", "LG keeps it for HDMI inputs; the computer sends frames at the screen's pace"), Value::Unavailable(tr("indisponible", "unavailable")));
        }
        Section::Sound => {
            let opts: Vec<String> = vec!["Auto".into(), tr("Stéréo", "Stereo").into(), "5.1 Dolby".into(), "5.1 PCM".into(), tr("Sur l'ordinateur", "On the computer").into()];
            let i = match st.audio.as_str() {
                "stereo" => 1,
                "dolby" => 2,
                "surround" => 3,
                "computer" => 4,
                _ => 0,
            };
            let help = match st.audio.as_str() {
                "dolby" => tr("Pour un ampli en eARC, Sortie son numérique en Pass-Through ; environ 50 ms de son en plus", "For a receiver over eARC, Digital Sound Output set to Pass-Through; about 50 ms more on sound"),
                "surround" => tr("Sans délai, si l'ampli accepte le PCM multicanal (beaucoup ne prennent que la stéréo)", "No delay, if the receiver takes multichannel PCM (many only take stereo)"),
                "computer" => tr("Pour un casque branché à l'ordinateur", "For a headset plugged into the computer"),
                "stereo" => tr("Le son le plus direct", "The most direct sound"),
                _ => tr("Auto : stéréo, le son le plus direct", "Auto: stereo, the most direct sound"),
            };
            y += widgets::row(cv, f, s(Setting::Audio), x, y, w, tr("Son", "Sound"), help, Value::Choices(&opts, i)) + GAP;
            let (state, help) = match app.tv.multichannel {
                4 => (tr("prête", "ready"), tr("Le téléviseur sortira le 5.1 vers l'ampli", "The TV will send 5.1 to the receiver")),
                3 => (
                    tr("pas en Pass-Through", "not Pass-Through"),
                    tr("Téléviseur : Son › Sortie son numérique › Pass-Through (sinon l'ampli reçoit de la stéréo)", "TV: Sound › Digital Sound Output › Pass-Through (otherwise the receiver gets stereo)"),
                ),
                2 => (tr("pas d'ampli", "no receiver"), tr("Aucun appareil multicanal branché (ampli ou barre de son en eARC)", "No multichannel device connected (receiver or soundbar over eARC)")),
                1 => (tr("impossible", "unsupported"), tr("Ce téléviseur ne sort pas le multicanal d'une app", "This TV does not output multichannel from an app")),
                _ => (tr("inconnue", "unknown"), tr("Le téléviseur ne le dit pas", "The TV does not say")),
            };
            y += widgets::row(cv, f, None, x, y, w, tr("Sortie 5.1 du téléviseur", "TV 5.1 output"), help, Value::Text(state.into())) + GAP;
            widgets::row(cv, f, None, x, y, w, "7.1", tr("Le son Dolby de l'app s'arrête au 5.1", "The app's Dolby sound stops at 5.1"), Value::Unavailable(tr("indisponible", "unavailable")));
        }
        Section::Controls => {
            let pads: Vec<String> = vec!["Xbox 360".into(), tr("Fidèle", "As is").into()];
            y += widgets::row(cv, f, s(Setting::Gamepad), x, y, w, tr("Manette vue par l'ordinateur", "Controller seen by the computer"), tr("Xbox 360 : reconnue par tous les jeux. Fidèle : chaque manette telle qu'elle est.", "Xbox 360: recognised by every game. As is: each controller as it is."), Value::Choices(&pads, usize::from(!st.xbox360))) + GAP;
            y += widgets::row(cv, f, s(Setting::Combo), x, y, w, tr("L1 + R1 + Start + Select : menu", "L1 + R1 + Start + Select: menu"), tr("La combinaison ouvre le menu de session", "The combination opens the session menu"), Value::Toggle(st.combo)) + GAP;
            widgets::row(cv, f, s(Setting::BackMenu), x, y, w, tr("Retour de la Magic Remote : menu", "Magic Remote Back: menu"), tr("Sinon, Retour part à l'ordinateur comme Échap", "Otherwise Back goes to the computer as Esc"), Value::Toggle(st.back_opens_menu));
        }
        Section::Network => {
            let opts: Vec<String> = vec![tr("Rapide", "Fast").into(), tr("Fiable", "Reliable").into()];
            y += widgets::row(
                cv,
                f,
                s(Setting::Transport),
                x,
                y,
                w,
                tr("Transport de l'image", "Picture transport"),
                tr(
                    "Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai.",
                    "Fast suits your home network and gaming. Reliable resends what gets lost, at the cost of a little delay.",
                ),
                Value::Choices(&opts, usize::from(st.reliable_transport())),
            ) + GAP;
            y += widgets::row(cv, f, s(Setting::Reconnect), x, y, w, tr("Reconnexion automatique", "Reconnect automatically"), tr("Après une coupure du réseau", "After a network drop"), Value::Toggle(st.reconnect)) + GAP;
            // Réveil par le réseau : appris de la Station, rien à saisir ici.
            let help = tr(
                "Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système).",
                "Turns the computer on remotely when it is asleep. It must also be allowed on the computer (Admin › System).",
            );
            // L'aide, trop longue pour la ligne, dessous.
            let (value, state) = match h.wake_mac() {
                Some(mac) => (format!("{} · {}", tr("Prêt", "Ready"), mac.to_uppercase()), ""),
                None => (String::new(), tr("Pas encore d'adresse : connectez-vous une fois à cet ordinateur.", "No address yet: connect to this computer once.")),
            };
            widgets::row(cv, f, None, x, y, w, tr("Réveil par le réseau", "Wake on LAN"), state, Value::Text(value));
            cv.paragraph(x + 4.0, y + 92.0 + 40.0, Text::new(Font::Regular, 20.0, FROST_3), help, w - 8.0, 28.0);
        }
        Section::Statistics => statistics(cv, h, x, y, w),
        Section::Station => station(app, cv, f, h, x, y, w),
        Section::Tv | Section::Gamepads => {}
    }
}

fn codec_name(c: &str) -> &'static str {
    match c {
        "h264" => "H.264",
        "av1" => "AV1",
        _ => "HEVC",
    }
}

fn footer(cv: &mut Canvas) {
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Changer", "Change")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

/// Statistiques : la dernière session (sur le modèle de la carte de l'app
/// Mac), puis cet ordinateur.
fn statistics(cv: &mut Canvas, h: &Host, x: f32, mut y: f32, w: f32) {
    let pad = 44.0;
    match &h.last {
        Some(last) => {
            let ch = 404.0;
            cv.rrect(x, y, w, ch, 18.0, PANEL);
            cv.rrect_stroke(x, y, w, ch, 18.0, 1.0, LINE);
            widgets::label(cv, x + pad, y + 58.0, tr("Dernière session", "Last session"));
            let when = format!("{} · {}", duration(last.seconds), moment(last.at, crate::model::now_secs()));
            cv.text(x + w - pad, y + 58.0, Text::new(Font::Regular, 22.0, FROST_2).align(Align::Right), &when);
            let big = Text::new(Font::MonoMedium, 72.0, FROST);
            let bw = cv.text(x + pad, y + 150.0, big.tracking(-6.0), &ms(last.median, false));
            let uw = cv.text(x + pad + bw + 14.0, y + 150.0, Text::new(Font::Mono, 28.0, FROST), "ms");
            cv.text(x + pad + bw + 14.0 + uw + 18.0, y + 150.0, Text::new(Font::Mono, 20.0, FROST_3), tr("médiane", "median"));
            frame_ruler(cv, x + pad, y + 184.0, w - 2.0 * pad, last.median, last.p95);
            let p95 = if english() { format!("95% of frames under {}", ms(last.p95, true)) } else { format!("95 % des images sous {}", ms(last.p95, true)) };
            cv.text(x + pad, y + 320.0, Text::new(Font::Regular, 24.0, FROST_2), &p95);
            cv.text_fit(x + pad, y + 362.0, Text::new(Font::Mono, 19.0, FROST_3), &what(last), w - 2.0 * pad);
            y += ch + 40.0;
            cv.paragraph(
                x + 4.0,
                y,
                Text::new(Font::Regular, 20.0, FROST_3),
                tr(
                    "Le délai entre l'ordinateur et cet écran. Sous un repère, l'image arrive avant l'image suivante à cette fréquence.",
                    "The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.",
                ),
                w - 8.0,
                28.0,
            );
            y += 70.0;
        }
        None => {
            let ch = 150.0;
            cv.rrect(x, y, w, ch, 18.0, PANEL);
            cv.rrect_stroke(x, y, w, ch, 18.0, 1.0, LINE);
            widgets::label(cv, x + pad, y + 58.0, tr("Dernière session", "Last session"));
            cv.text(x + pad, y + 108.0, Text::new(Font::Regular, 25.0, FROST_2), tr("Pas encore de session sur cet ordinateur.", "No session on this computer yet."));
            y += ch + 40.0;
        }
    }

    // Cet ordinateur.
    y += 20.0;
    widgets::label(cv, x + 4.0, y, tr("Cet ordinateur", "This computer"));
    y += 24.0;
    let ch = 220.0;
    cv.rrect(x, y, w, ch, 18.0, PANEL);
    cv.rrect_stroke(x, y, w, ch, 18.0, 1.0, LINE);
    let fp = h.fingerprint.as_deref().map(fingerprint_groups).unwrap_or_else(|| "—".into());
    let items = [
        (tr("Système", "System"), h.os.clone().unwrap_or_else(|| "—".into()), false),
        (tr("Carte graphique", "Graphics card"), h.gpu.clone().unwrap_or_else(|| "—".into()), false),
        (tr("Adresse", "Address"), format!("{}:{}", h.address, h.port), true),
        (tr("Empreinte", "Fingerprint"), fp, true),
    ];
    let col = (w - 2.0 * pad) / 2.0;
    for (i, (label, value, mono)) in items.iter().enumerate() {
        let (cx, cy) = (x + pad + (i % 2) as f32 * col, y + 64.0 + (i / 2) as f32 * 96.0);
        cv.text(cx, cy, Text::new(Font::Regular, 18.0, FROST_3), label);
        let style = if *mono { Text::new(Font::Mono, 21.0, FROST) } else { Text::new(Font::Regular, 25.0, FROST) };
        cv.text_fit(cx, cy + 36.0, style, value, col - 40.0);
    }
}

/// Les quatre premiers groupes de l'empreinte (`sha256:9f:3a:…` → `9F 3A C2 71`).
fn fingerprint_groups(fp: &str) -> String {
    fp.trim_start_matches("sha256:").split(':').take(4).collect::<Vec<_>>().join(" ").to_uppercase()
}

/// Une latence : une décimale sous 100 ms, virgule en français.
fn ms(v: f64, unit: bool) -> String {
    let s = if v >= 100.0 { format!("{}", v.round() as i64) } else { decimal(v, 1) };
    if unit { format!("{s} ms") } else { s }
}

/// « 42 min », « 1 h 05 ».
fn duration(seconds: u64) -> String {
    let m = seconds / 60;
    if m < 60 { format!("{} min", m.max(1)) } else { format!("{} h {:02}", m / 60, m % 60) }
}

/// Ce qui a été transmis : « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s ».
fn what(last: &LastSession) -> String {
    let mut parts = Vec::new();
    if last.width > 0 {
        parts.push(format!("{} × {}", last.width, last.height));
        if last.refresh > 0 {
            parts.push(format!("{} Hz", last.refresh));
        }
        if !last.codec.is_empty() {
            parts.push(last.codec.clone());
        }
    } else if !last.mode.is_empty() {
        // Session gardée par la 0.4.x : « 3840 × 2160 · HEVC ».
        parts.push(last.mode.clone());
    }
    if last.mbps > 0.0 {
        parts.push(format!("{} Mbit/s", last.mbps.round() as i64));
    }
    parts.join(" · ")
}

/// Le moment d'une session : « à l'instant », « aujourd'hui », « hier », « il
/// y a 3 jours », « le 3 octobre », en jours du calendrier local.
pub fn moment(at: u64, now: u64) -> String {
    let (day, month, mday) = local_day(at);
    let (today, _, _) = local_day(now);
    let days = today - day;
    if days <= 0 {
        return if now.saturating_sub(at) < 120 { tr("à l'instant", "just now").into() } else { tr("aujourd'hui", "today").into() };
    }
    if days == 1 {
        return tr("hier", "yesterday").into();
    }
    if days < 7 {
        return if english() { format!("{days} days ago") } else { format!("il y a {days} jours") };
    }
    const FR: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let m = (month as usize).min(11);
    if english() { format!("{mday} {}", EN[m]) } else { format!("le {mday} {}", FR[m]) }
}

/// Jour du calendrier local (numéro de jour continu), mois (0 à 11) et jour du mois.
fn local_day(t: u64) -> (i64, i32, i32) {
    let secs = t as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&secs, &mut tm) }.is_null() {
        return ((t / 86_400) as i64, 0, 1);
    }
    // Jours depuis le 1er mars de l'an 0 (calendrier grégorien), pour la différence.
    let (y, m) = (tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1);
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let n = 365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + tm.tm_mday as i64;
    (n, tm.tm_mon, tm.tm_mday)
}

/// Règle de trame : la latence lue contre la durée d'une image à 160, 120
/// et 60 Hz, de 0 à 20 ms (plus si la mesure dépasse), avec la bande de la
/// médiane au 95e centile (`FrameRuler` de l'app Mac).
fn frame_ruler(cv: &mut Canvas, x: f32, y: f32, w: f32, median: f64, p95: f64) {
    let scale = 20f64.max(((median.max(p95) * 1.25) / 10.0).ceil() * 10.0);
    let px = |ms: f64| x + (ms.min(scale) / scale) as f32 * w;
    let base = y + 40.0;
    cv.rect(x, base - 2.0, w, 4.0, Rgba(227, 237, 240, 41));
    let lo = (median - (p95 - median) * 0.6).max(0.0);
    let (bx, bw) = (px(lo), (px(p95) - px(lo)).max(8.0));
    cv.rrect(bx, base - 12.0, bw, 24.0, 6.0, SPECTRE.alpha(0.2));
    cv.rrect_stroke(bx, base - 12.0, bw, 24.0, 6.0, 1.5, SPECTRE.alpha(0.5));
    // Repères ; une étiquette seulement si elle a la place (160 Hz au-dessus).
    let ticks = [(1000.0 / 160.0, "160 Hz", true), (1000.0 / 120.0, "120 Hz", false), (1000.0 / 60.0, "60 Hz", false)];
    let label = Text::new(Font::Mono, 16.0, FROST_3).align(Align::Center);
    for (ms, name, up) in ticks {
        let tx = px(ms);
        cv.rect(tx - 1.0, base - 18.0, 2.0, 36.0, FROST_3.alpha(0.7));
        let crowded = ticks.iter().any(|(m, _, u)| *u == up && *m > ms && px(*m) - tx < 76.0);
        if !crowded {
            cv.text(tx, if up { base - 28.0 } else { base + 46.0 }, label, name);
        }
    }
    // Les bornes de l'échelle.
    let ends = Text::new(Font::Mono, 15.0, FROST_4);
    cv.text(x, base + 46.0, ends, "0");
    cv.text(x + w, base + 46.0, ends.align(Align::Right), &format!("{} ms", scale as i64));
    let mx = px(median);
    cv.rrect(mx - 3.0, base - 24.0, 6.0, 48.0, 3.0, SPECTRE);
}

/// Station : ouvrir Admin (QR code), la version, redémarrer, mettre à jour,
/// le compte et « Retirer cet ordinateur ».
fn station(app: &App, cv: &mut Canvas, f: &mut Frame, h: &Host, x: f32, mut y: f32, w: f32) {
    // Le QR code d'Admin, à droite des lignes.
    let qw = 380.0;
    let w = w - qw - 30.0;
    admin_panel(cv, h, x + w + 30.0, y, qw);

    let s = |s: Setting| Some(Target::Setting(s));
    let live = app.live.get(&h.id).cloned().unwrap_or_default();
    let ui = app.station.as_ref().filter(|s| s.host == h.id);
    let phase = ui.map(|s| s.phase.clone()).unwrap_or(StationPhase::Idle);
    let mut note: Option<String> = ui.and_then(|s| s.message.clone());

    // La version, et ce qu'on peut faire.
    let mut actions = false;
    match (&live.station, live.online) {
        (StationKnown::Info(st), on) => {
            let title = format!("KyberNow Station {}", st.version);
            let state = if st.update_available {
                let v = st.update_version.as_deref().unwrap_or_default();
                if english() { format!("Version {v} available") } else { format!("Version {v} disponible") }
            } else {
                tr("À jour", "Up to date").into()
            };
            // Une version qui s'installe avec l'installateur : dit sous les lignes.
            if let (Some(v), true, None) = (&st.update_version, st.update_available && st.requires_setup, &note) {
                note = Some(app::setup_text(v));
            }
            // Par le relais, la Station ne se redémarre ni ne se met à jour : seul son état se lit sur le réseau.
            let value = match on {
                Online::Offline => tr("Hors ligne", "Offline").into(),
                Online::Away => tr("Hors de chez vous", "Away from home").into(),
                _ => state,
            };
            y += widgets::row(cv, f, None, x, y, w, &title, "", Value::Text(value)) + GAP;
            actions = on == Online::Online;
        }
        (StationKnown::Old(v), _) => {
            let title = match v {
                Some(v) => format!("KyberNow Station {v}"),
                None => "KyberNow Station".into(),
            };
            y += widgets::row(cv, f, None, x, y, w, &title, "", Value::Text(String::new())) + GAP;
            if note.is_none() {
                note = Some(app::station_too_old().into());
            }
        }
        (StationKnown::Unknown, on) => {
            let value = if on == Online::Offline { tr("Hors ligne", "Offline") } else { tr("Recherche…", "Searching…") };
            y += widgets::row(cv, f, None, x, y, w, "KyberNow Station", "", Value::Text(value.into())) + GAP;
        }
    }
    match &phase {
        StationPhase::Asking(op) | StationPhase::Waiting { op, .. } => {
            let msg = match op {
                StationOp::Restart => tr("La Station redémarre…", "The Station is restarting…"),
                StationOp::Update => tr("Mise à jour de la Station…", "Updating the Station…"),
            };
            let r = Rect::new(x, y, w, 92.0);
            cv.rrect(r.x, r.y, r.w, r.h, 16.0, SPECTRE.alpha(0.1));
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 16.0, 1.0, SPECTRE.alpha(0.5));
            cv.circle(r.x + 44.0, r.y + 46.0, 14.0, HALO.alpha(0.35));
            cv.circle(r.x + 44.0, r.y + 46.0, 6.0, SPECTRE);
            cv.text(r.x + 76.0, r.y + 55.0, Text::new(Font::Regular, 25.0, FROST), msg);
            y += 92.0 + GAP;
        }
        StationPhase::Idle if actions => {
            if let StationKnown::Info(st) = &live.station {
                y += widgets::row(cv, f, s(Setting::Restart), x, y, w, tr("Redémarrer la Station", "Restart the Station"), tr("Environ 30 secondes. Une session en cours sera coupée.", "About 30 seconds. A session in progress will be cut off."), Value::Text("›".into())) + GAP;
                if st.update_available && st.installable && !st.requires_setup {
                    let v = st.update_version.as_deref().unwrap_or_default();
                    let help = if english() { format!("Version {v}. About a minute. A session in progress will be cut off.") } else { format!("Version {v}. Environ une minute. Une session en cours sera coupée.") };
                    y += widgets::row(cv, f, s(Setting::Update), x, y, w, tr("Mettre à jour la Station", "Update the Station"), &help, Value::Text("›".into())) + GAP;
                }
            }
        }
        StationPhase::Idle => {}
    }
    if let Some(n) = note {
        cv.icon(&Icon::Info, x + 6.0, y + 4.0, 30.0, 1.6, SPECTRE);
        let used = cv.paragraph(x + 50.0, y + 28.0, Text::new(Font::Regular, 22.0, FROST), &n, w - 60.0, 32.0);
        y += used + 28.0;
    }

    // Le compte et le retrait, un peu à part.
    let y = y + 26.0;
    // Sur ce téléviseur, tout compte vient du code ; Admin le confirme (`own_device`).
    let own = if live.own_device.unwrap_or(true) { tr("Accès propre à cet appareil", "This device's own access") } else { "" };
    let y = y + widgets::row(cv, f, None, x, y, w, tr("Compte", "Account"), own, Value::Text(h.user.clone())) + GAP;
    widgets::row(cv, f, s(Setting::Remove), x, y, w, tr("Retirer cet ordinateur", "Remove this computer"), tr("De ce téléviseur seulement", "From this TV only"), Value::Danger("›"));
}

/// Le QR code de l'adresse d'Admin, à scanner au téléphone, et l'adresse en clair.
fn admin_panel(cv: &mut Canvas, h: &Host, x: f32, y: f32, w: f32) {
    let url = app::admin_url(&h.address, h.port);
    let side = w - 68.0;
    // Le code, la phrase (deux lignes) et l'adresse.
    let ph = 76.0 + side + 40.0 + 56.0 + 10.0 + 36.0;
    cv.rrect(x, y, w, ph, 18.0, PANEL);
    cv.rrect_stroke(x, y, w, ph, 18.0, 1.0, LINE);
    widgets::label(cv, x + 34.0, y + 52.0, "Admin");
    widgets::qr(cv, &url, x + 34.0, y + 76.0, side);
    let used = cv.paragraph(
        x + 34.0,
        y + 76.0 + side + 40.0,
        Text::new(Font::Regular, 20.0, FROST_2),
        tr("Scannez ce code avec votre téléphone pour ouvrir Admin.", "Scan this code with your phone to open Admin."),
        side,
        28.0,
    );
    cv.text_fit(x + 34.0, y + 76.0 + side + 40.0 + used + 10.0, Text::new(Font::Mono, 15.0, FROST_3), &url, side);
}

/// La fenêtre de confirmation, par-dessus les réglages : retirer
/// l'ordinateur, redémarrer ou mettre à jour malgré une session.
pub fn dialog(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let Some(d) = &app.dialog else { return };
    cv.rect(0.0, 0.0, W, H, Rgba(2, 3, 4, 200));
    let (title, message, ok): (String, String, &str) = match d {
        Dialog::Remove { name, .. } => (
            if english() { format!("Remove {name} from this device?") } else { format!("Retirer {name} de cet appareil ?") },
            tr("Vous pourrez l'ajouter à nouveau.", "You can add it again later.").into(),
            tr("Retirer", "Remove"),
        ),
        Dialog::Force { op, device, .. } => (
            if english() { format!("{device} is connected") } else { format!("{device} est connecté") },
            match op {
                StationOp::Restart => tr("Redémarrer la Station coupera sa session.", "Restarting the Station will end its session."),
                StationOp::Update => tr("La mise à jour coupera sa session.", "The update will end its session."),
            }
            .into(),
            match op {
                StationOp::Restart => tr("Redémarrer quand même", "Restart anyway"),
                StationOp::Update => tr("Mettre à jour quand même", "Update anyway"),
            },
        ),
    };
    let (w, h) = (1000.0, 340.0);
    let (x, y) = ((W - w) / 2.0, (H - h) / 2.0);
    cv.rrect(x, y, w, h, 28.0, PANEL_SOLID);
    cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    cv.text_fit(x + 60.0, y + 100.0, Text::new(Font::Title, 40.0, FROST), &title, w - 120.0);
    cv.paragraph(x + 60.0, y + 156.0, Text::new(Font::Regular, 24.0, FROST_2), &message, w - 120.0, 34.0);
    let by = y + h - 120.0;
    let bw = widgets::primary(cv, f, Target::DialogOk, x + 60.0, by, ok);
    widgets::secondary(cv, f, Target::DialogCancel, x + 60.0 + bw + 28.0, by + 10.0, tr("Annuler", "Cancel"), 26.0);
}

/// Réglages de l'app : ce téléviseur (langue, matériel, réseau, version), les
/// manettes branchées et leur test. Sans ordinateur ajouté aussi.
fn global(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let s = |s: Setting| Some(Target::Setting(s));
    let (x, w) = (X, WIDTH);
    let mut y = 88.0;
    match app.current_section() {
        Section::Gamepads => {
            y += widgets::row(cv, f, s(Setting::Pads), x, y, w, tr("Tester les manettes", "Test controllers"), tr("Touches, sticks, gâchettes et vibrations de chaque manette", "Buttons, sticks, triggers and rumble of each controller"), Value::Text("›".into())) + GAP;
            if app.pads.is_empty() {
                y += widgets::row(cv, f, None, x, y, w, tr("Aucune manette", "No controller"), tr("Associez-la en Bluetooth dans les réglages du téléviseur", "Pair it over Bluetooth in the TV's settings"), Value::Text("—".into())) + GAP;
            }
            for p in &app.pads {
                let r = if p.rumble { tr("vibrations", "rumble") } else { tr("sans vibration", "no rumble") };
                y += widgets::row(cv, f, None, x, y, w, &p.name, &format!("{} {}", tr("Joueur", "Player"), p.slot + 1), Value::Text(r.into())) + GAP;
            }
            widgets::row(cv, f, None, x, y, w, tr("Réglages propres à chaque ordinateur", "Per-computer settings"), tr("Manette vue par l'ordinateur, combinaison du menu, bouton Retour : rubrique Commandes de ses réglages", "Controller seen by the computer, menu combination, Back button: Controls, in its settings"), Value::Text(String::new()));
        }
        _ => {
            let langs: Vec<String> = vec!["Auto".into(), "Français".into(), "English".into()];
            let li = match app.store.app.language.as_str() {
                "fr" => 1,
                "en" => 2,
                _ => 0,
            };
            y += widgets::row(cv, f, s(Setting::Language), x, y, w, tr("Langue", "Language"), tr("Auto : celle du téléviseur", "Auto: the TV's"), Value::Choices(&langs, li)) + GAP;
            y += widgets::row(cv, f, None, x, y, w, tr("Téléviseur", "TV"), "webOS", Value::Text(format!("{} {}", app.tv.model, app.tv.webos))) + GAP;
            let (pw, ph) = app.tv.panel;
            y += widgets::row(cv, f, None, x, y, w, tr("Écran", "Screen"), tr("Définition et fréquence de la dalle ; pas de fréquence variable pour une app", "Panel resolution and refresh rate; no variable refresh rate for an app"), Value::Text(format!("{pw} × {ph} · {} Hz", app.tv.refresh))) + GAP;
            let dec = if !app.tv.decoder {
                tr("introuvable", "not found")
            } else if app.tv.av1 {
                "HEVC, AV1, H.264"
            } else {
                "HEVC, H.264"
            };
            y += widgets::row(cv, f, None, x, y, w, tr("Décodeur", "Decoder"), tr("Les formats que le téléviseur décode lui-même", "The formats the TV decodes itself"), Value::Text(dec.into())) + GAP;
            let (link, help) = match app.net.kind {
                NetKind::Ethernet => (
                    format!("Ethernet · {} Mbit/s", app.net.link_mbps.unwrap_or(100)),
                    if english() { format!("Bitrate capped at {} Mbit/s: LG's Ethernet port stops at 100 Mbit/s", app.net.cap_mbps()) } else { format!("Débit plafonné à {} Mbit/s : la prise Ethernet des LG s'arrête à 100 Mbit/s", app.net.cap_mbps()) },
                ),
                NetKind::Wifi => (
                    "Wi-Fi".into(),
                    if english() { format!("Bitrate capped at {} Mbit/s, the decoder's limit", app.net.cap_mbps()) } else { format!("Débit plafonné à {} Mbit/s, la limite du décodeur", app.net.cap_mbps()) },
                ),
                NetKind::Unknown => (tr("inconnu", "unknown").into(), String::new()),
            };
            y += widgets::row(cv, f, None, x, y, w, tr("Réseau", "Network"), &help, Value::Text(link)) + GAP;
            y += widgets::row(cv, f, None, x, y, w, &format!("KyberNow Link {}", app.version), &update_help(app), Value::Text(update_value(app))) + GAP;
            let chan: Vec<String> = vec![tr("Stables", "Stable").into(), tr("Aussi les bêtas", "Betas too").into()];
            widgets::row(cv, f, s(Setting::UpdateChannel), x, y, w, tr("Versions proposées", "Versions offered"), tr("Les bêtas arrivent plus tôt, parfois avec des défauts.", "Betas arrive earlier, sometimes with rough edges."), Value::Choices(&chan, usize::from(app.store.app.beta)));
        }
    }
}

/// L'état de la recherche de mise à jour.
fn update_value(app: &App) -> String {
    match &app.update {
        UpdateState::Available { version, .. } => {
            if english() { format!("Version {version} available") } else { format!("Version {version} disponible") }
        }
        UpdateState::Checking => tr("Recherche…", "Searching…").into(),
        UpdateState::UpToDate => tr("À jour", "Up to date").into(),
        UpdateState::Failed(_) | UpdateState::Unknown => String::new(),
    }
}

fn update_help(app: &App) -> String {
    match &app.update {
        UpdateState::Available { channel, .. } => format!(
            "{}{}",
            if channel == "beta" { tr("Bêta. ", "Beta. ") } else { "" },
            tr(
                "Installez KyberNow-Link-webOS.ipk depuis l'ordinateur, avec Dev Manager ou ares-install.",
                "Install KyberNow-Link-webOS.ipk from the computer, with Dev Manager or ares-install."
            )
        ),
        UpdateState::Failed(e) => format!("{} : {e}", tr("Recherche impossible", "Check failed")),
        _ => tr("Cherchée au lancement, puis toutes les 6 heures. Licence AGPL-3.0.", "Checked at launch, then every 6 hours. AGPL-3.0 licence.").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moments() {
        crate::i18n::set_english(false);
        let now = 1_761_400_000;
        assert_eq!(moment(now - 30, now), "à l'instant");
        assert_eq!(moment(now - 86_400, now), "hier");
        assert_eq!(moment(now - 3 * 86_400, now), "il y a 3 jours");
        assert!(moment(now - 30 * 86_400, now).starts_with("le "));
        assert_eq!(duration(42 * 60 + 10), "42 min");
        assert_eq!(duration(3900), "1 h 05");
        assert_eq!(ms(8.94, true), "8,9 ms");
        assert_eq!(fingerprint_groups("sha256:9f:3a:c2:71:0b"), "9F 3A C2 71");
    }

    #[test]
    fn transmitted() {
        crate::i18n::set_english(false);
        let mut last = LastSession { median: 9.0, p95: 14.0, seconds: 60, mbps: 79.6, at: 0, width: 2560, height: 1440, refresh: 120, codec: "HEVC".into(), ..Default::default() };
        assert_eq!(what(&last), "2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s");
        last.width = 0;
        last.mode = "3840 × 2160 · HEVC".into();
        assert_eq!(what(&last), "3840 × 2160 · HEVC · 80 Mbit/s");
    }
}
