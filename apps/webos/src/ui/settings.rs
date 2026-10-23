//! Réglages : ceux de l'ordinateur choisi, ou les réglages globaux (ce
//! téléviseur, la langue, les manettes). Rubriques à gauche, lignes à
//! droite ; gauche et droite changent la valeur.

use super::canvas::{Canvas, Font, Rgba, Text, H, W};
use super::theme::*;
use super::widgets::{self, Key, Value};
use super::Frame;
use crate::app::{App, NetKind, Rect, Section, Setting, Target};
use crate::i18n::tr;
use crate::model::Profile;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    let host = if app.settings_global { None } else { app.selected() };
    if !app.settings_global && host.is_none() {
        return;
    }

    // Rubriques.
    cv.rect(520.0, 0.0, 1.0, H, LINE);
    let over = host.map(|h| h.name.as_str()).unwrap_or(tr("KyberNow Link", "KyberNow Link"));
    widgets::label(cv, 96.0, 104.0, over);
    cv.text(92.0, 178.0, Text::new(Font::Title, 56.0, FROST).tracking(-1.5), tr("Réglages", "Settings"));
    let mut y = 228.0;
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
    let (x, w) = (600.0, W - 110.0 - 600.0);
    let mut y = 88.0;
    let s = |s: Setting| Some(Target::Setting(s));
    let Some(h) = host else {
        global(app, cv, f, x, y, w);
        footer(cv);
        return;
    };
    let st = &h.settings;
    let (mw, mh, fps) = app.session_mode(h);
    match app.current_section() {
        Section::Picture => {
            let profiles: Vec<String> = Profile::ALL.iter().map(|p| super::home::profile_label(*p).to_string()).collect();
            let pi = Profile::ALL.iter().position(|p| *p == st.profile).unwrap_or(0);
            y += widgets::row(cv, f, s(Setting::Profile), x, y, w, tr("Profil", "Profile"), tr("Jeu : faible latence · Bureau : texte net · Max : tout au plus haut", "Game: low latency · Desktop: sharp text · Max: everything highest"), Value::Choices(&profiles, pi)) + 14.0;
            let res = if st.resolution == 0 { format!("{} · {mw} × {mh}", tr("Comme ce téléviseur", "Like this TV")) } else { format!("{mw} × {mh}") };
            y += widgets::row(cv, f, s(Setting::Resolution), x, y, w, tr("Définition", "Resolution"), tr("Plus bas : moins de débit, moins de latence", "Lower: less bitrate, less latency"), Value::Text(res)) + 14.0;
            let fps_opts: Vec<String> = vec![tr("Dalle", "Panel").into(), "30".into(), "60".into(), "120 Hz".into()];
            let fi = match st.fps {
                30 => 1,
                60 => 2,
                120 => 3,
                _ => 0,
            };
            let fps_help = format!(
                "{} {} Hz. {}",
                tr("La dalle reste à", "The panel stays at"),
                app.tv.refresh,
                tr("À 30 ou 60 i/s, chaque image y est montrée plusieurs fois, régulièrement.", "At 30 or 60 fps each frame is shown several times, evenly.")
            );
            y += widgets::row(cv, f, s(Setting::Fps), x, y, w, tr("Images par seconde", "Frame rate"), &fps_help, Value::Choices(&fps_opts, fi)) + 14.0;
            let codec = app.codec_for(h).to_uppercase().replace("HEVC", "HEVC 4:2:0");
            let codec_v = if st.codec == "auto" { format!("Auto · {codec}") } else { codec };
            y += widgets::row(cv, f, s(Setting::Codec), x, y, w, tr("Format", "Codec"), tr("HEVC à 120 i/s ; AV1 jusqu'à 60 i/s sur les téléviseurs LG", "HEVC at 120 fps; AV1 up to 60 fps on LG TVs"), Value::Text(codec_v)) + 14.0;
            let mbps = if st.mbps == 0 { format!("Auto · {} {} Mbit/s", tr("au plus", "up to"), app.net.cap_mbps()) } else { format!("{} Mbit/s", st.mbps) };
            y += widgets::row(cv, f, s(Setting::Bitrate), x, y, w, tr("Débit", "Bitrate"), tr("Proportionné au profil, plafonné par le réseau", "Scaled to the profile, capped by the network"), Value::Text(mbps)) + 14.0;
            y += widgets::row(cv, f, s(Setting::GameHint), x, y, w, tr("Rappeler le mode jeu de la télé", "Remind about the TV's game mode"), tr("L'app ne peut pas passer la télé en Optimiseur de jeu : le menu le rappelle", "The app cannot switch the TV to Game Optimizer: the menu reminds you"), Value::Toggle(st.game_mode_hint)) + 14.0;
            y += widgets::row(cv, f, None, x, y, w, tr("Fréquence variable (VRR)", "Variable refresh rate (VRR)"), tr("LG la réserve aux entrées HDMI ; l'ordinateur envoie ses images au rythme de la dalle", "LG keeps it for HDMI inputs; the computer sends frames at the panel's pace"), Value::Unavailable(tr("indisponible", "unavailable"))) + 14.0;
            let _ = (fps, y);
            info_card(app, cv, x, H - 150.0, w);
        }
        Section::Sound => {
            let opts: Vec<String> = vec!["Auto".into(), tr("Stéréo", "Stereo").into(), "5.1 Dolby".into(), "5.1 PCM".into(), tr("Ordinateur", "Computer").into()];
            let i = match st.audio.as_str() {
                "stereo" => 1,
                "dolby" => 2,
                "surround" => 3,
                "computer" => 4,
                _ => 0,
            };
            let help = match st.audio.as_str() {
                "dolby" => tr("Encodé en Dolby Digital Plus par l'app et passé tel quel à l'ampli (eARC, Sortie son numérique en Pass-Through) ; environ 50 ms de son en plus", "Encoded to Dolby Digital Plus by the app and passed as is to the receiver (eARC, Digital Sound Output set to Pass-Through); about 50 ms more on sound"),
                "surround" => tr("PCM 6 voies, sans délai, si l'ampli annonce le PCM multicanal (beaucoup n'annoncent que la stéréo) ; sinon stéréo", "6-channel PCM, no delay, if the receiver announces multichannel PCM (many only announce stereo); stereo otherwise"),
                "computer" => tr("Pour un casque branché à l'ordinateur", "For a headset plugged into the computer"),
                _ => tr("Auto : stéréo, le son le plus direct. 5.1 Dolby : pour un ampli, avec un léger délai. 5.1 PCM : sans délai, rarement accepté", "Auto: stereo, the most direct sound. 5.1 Dolby: for a receiver, with a slight delay. 5.1 PCM: no delay, rarely accepted"),
            };
            y += widgets::row(cv, f, s(Setting::Audio), x, y, w, tr("Son", "Sound"), help, Value::Choices(&opts, i)) + 14.0;
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
            y += widgets::row(cv, f, None, x, y, w, tr("Sortie 5.1 du téléviseur", "TV 5.1 output"), help, Value::Text(state.into())) + 14.0;
            widgets::row(cv, f, None, x, y, w, "7.1", tr("Une app du téléviseur ne peut pas sortir plus que du 5.1 (décodeur LG)", "A TV app cannot output more than 5.1 (LG decoder)"), Value::Unavailable(tr("indisponible", "unavailable")));
        }
        Section::Controllers => {
            y += widgets::row(cv, f, s(Setting::Xbox360), x, y, w, tr("Manettes vues comme des Xbox 360", "Controllers seen as Xbox 360"), tr("Sinon, l'ordinateur voit chaque manette telle qu'elle est", "Otherwise the computer sees each controller as it is"), Value::Toggle(st.xbox360)) + 14.0;
            y += widgets::row(cv, f, s(Setting::Combo), x, y, w, tr("L1 + R1 + Start + Select : menu", "L1 + R1 + Start + Select: menu"), tr("La combinaison ouvre le menu de session", "The combination opens the session menu"), Value::Toggle(st.combo)) + 14.0;
            y += widgets::row(cv, f, s(Setting::BackMenu), x, y, w, tr("Retour de la Magic Remote : menu", "Magic Remote Back: menu"), tr("Sinon, Retour part à l'ordinateur comme Échap", "Otherwise Back goes to the computer as Esc"), Value::Toggle(st.back_opens_menu)) + 14.0;
            widgets::row(cv, f, s(Setting::Pads), x, y, w, tr("Tester les manettes", "Test controllers"), tr("Touches, sticks, gâchettes et vibrations de chaque manette", "Buttons, sticks, triggers and rumble of each controller"), Value::Text("›".into()));
        }
        Section::Network => {
            let link = match app.net.kind {
                NetKind::Ethernet => format!("Ethernet · {} Mbit/s", app.net.link_mbps.unwrap_or(100)),
                NetKind::Wifi => "Wi-Fi".into(),
                NetKind::Unknown => tr("inconnu", "unknown").into(),
            };
            y += widgets::row(cv, f, None, x, y, w, tr("Lien du téléviseur", "TV link"), tr("La prise Ethernet des LG s'arrête à 100 Mbit/s ; le Wi-Fi 6 va souvent plus loin", "LG's Ethernet port stops at 100 Mbit/s; Wi-Fi 6 often goes further"), Value::Text(link)) + 14.0;
            y += widgets::row(cv, f, None, x, y, w, tr("Plafond de débit", "Bitrate cap"), tr("Ethernet : 80 Mbit/s · Wi-Fi : 95 (limite du décodeur)", "Ethernet: 80 Mbit/s · Wi-Fi: 95 (decoder limit)"), Value::Text(format!("{} Mbit/s", app.net.cap_mbps()))) + 14.0;
            y += widgets::row(cv, f, s(Setting::Reliable), x, y, w, tr("Transport fiable", "Reliable transport"), tr("Plus robuste sur un Wi-Fi chargé, un peu plus de latence", "Sturdier on a busy Wi-Fi, a little more latency"), Value::Toggle(st.reliable)) + 14.0;
            y += widgets::row(cv, f, s(Setting::Reconnect), x, y, w, tr("Reconnexion automatique", "Automatic reconnection"), tr("Après une coupure du réseau", "After a network drop"), Value::Toggle(st.reconnect)) + 14.0;
            widgets::row(cv, f, None, x, y, w, tr("Adresse d'Admin", "Admin address"), tr("Trouvée sur le réseau ou tapée à l'ajout", "Found on the network or typed when adding"), Value::Text(format!("{}:{}", h.address, h.port)));
        }
        Section::Account => {
            y += widgets::row(cv, f, None, x, y, w, tr("Compte", "Account"), tr("Propre à ce téléviseur, accordé par le code ; retirable dans Admin", "This TV's own, granted by the code; removable in Admin"), Value::Text(h.user.clone())) + 14.0;
            let fp = h.fingerprint.as_deref().unwrap_or("—").trim_start_matches("sha256:").split(':').take(4).collect::<Vec<_>>().join(" ").to_uppercase();
            y += widgets::row(cv, f, None, x, y, w, tr("Empreinte d'Admin", "Admin fingerprint"), tr("Certificat épinglé à l'ajout", "Certificate pinned when added"), Value::Text(fp)) + 14.0;
            widgets::row(cv, f, s(Setting::Remove), x, y, w, tr("Retirer l'ordinateur", "Remove the computer"), tr("De ce téléviseur seulement", "From this TV only"), Value::Danger(tr("Retirer", "Remove")));
        }
        Section::Tv | Section::Gamepads => {}
    }

    footer(cv);
}

fn footer(cv: &mut Canvas) {
    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Changer", "Change")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
}

fn info_card(app: &App, cv: &mut Canvas, x: f32, y: f32, w: f32) {
    cv.rrect(x, y, w, 96.0, 18.0, SPECTRE.alpha(0.08));
    cv.rrect_stroke(x, y, w, 96.0, 18.0, 1.0, SPECTRE.alpha(0.3));
    let col = w / 3.0;
    let label = Text::new(Font::Mono, 13.0, FROST_3).tracking(2.0);
    let val = Text::new(Font::Mono, 16.0, FROST_2);
    let (pw, ph) = app.tv.panel;
    let items = [
        (tr("TÉLÉVISEUR", "TV"), format!("{} · webOS {}", app.tv.model, app.tv.webos)),
        (tr("DÉCODEUR", "DECODER"), if app.tv.av1 { "HEVC, AV1, H.264".to_string() } else { "HEVC, H.264".to_string() }),
        (tr("DALLE", "PANEL"), format!("{pw} × {ph} · {} Hz · {}", app.tv.refresh, tr("sans VRR", "no VRR"))),
    ];
    for (i, (l, v)) in items.iter().enumerate() {
        let xx = x + 30.0 + i as f32 * col;
        cv.text(xx, y + 38.0, label, l);
        cv.text_fit(xx, y + 68.0, val, v, col - 40.0);
    }
    let _ = Rgba(0, 0, 0, 0);
}

/// Réglages globaux : ce téléviseur (langue, matériel, réseau, version), les
/// manettes branchées et leur test. Sans ordinateur ajouté aussi.
fn global(app: &App, cv: &mut Canvas, f: &mut Frame, x: f32, mut y: f32, w: f32) {
    let s = |s: Setting| Some(Target::Setting(s));
    match app.current_section() {
        Section::Gamepads => {
            y += widgets::row(cv, f, s(Setting::Pads), x, y, w, tr("Tester les manettes", "Test controllers"), tr("Touches, sticks, gâchettes et vibrations de chaque manette", "Buttons, sticks, triggers and rumble of each controller"), Value::Text("›".into())) + 14.0;
            if app.pads.is_empty() {
                y += widgets::row(cv, f, None, x, y, w, tr("Aucune manette", "No controller"), tr("Associez-la en Bluetooth dans les réglages du téléviseur", "Pair it over Bluetooth in the TV's settings"), Value::Text("—".into())) + 14.0;
            }
            for p in &app.pads {
                let r = if p.rumble { tr("vibrations", "rumble") } else { tr("sans vibration", "no rumble") };
                y += widgets::row(cv, f, None, x, y, w, &p.name, &format!("{} {}", tr("Joueur", "Player"), p.slot + 1), Value::Text(r.into())) + 14.0;
            }
            widgets::row(cv, f, None, x, y, w, tr("Réglages propres à chaque ordinateur", "Per-computer settings"), tr("Xbox 360, combinaison du menu, bouton Retour : dans les réglages de l'ordinateur", "Xbox 360, menu combination, Back button: in the computer's settings"), Value::Text(String::new()));
        }
        _ => {
            let langs: Vec<String> = vec!["Auto".into(), "Français".into(), "English".into()];
            let li = match app.store.app.language.as_str() {
                "fr" => 1,
                "en" => 2,
                _ => 0,
            };
            y += widgets::row(cv, f, s(Setting::Language), x, y, w, tr("Langue", "Language"), tr("Auto : celle du téléviseur", "Auto: the TV's"), Value::Choices(&langs, li)) + 14.0;
            y += widgets::row(cv, f, None, x, y, w, tr("Téléviseur", "TV"), "webOS", Value::Text(format!("{} {}", app.tv.model, app.tv.webos))) + 14.0;
            let (pw, ph) = app.tv.panel;
            y += widgets::row(cv, f, None, x, y, w, tr("Dalle", "Panel"), tr("Définition et fréquence de l'écran", "Screen resolution and refresh rate"), Value::Text(format!("{pw} × {ph} · {} Hz", app.tv.refresh))) + 14.0;
            let dec = if app.tv.decoder { tr("NDL (matériel)", "NDL (hardware)") } else { tr("introuvable", "not found") };
            y += widgets::row(cv, f, None, x, y, w, tr("Décodeur", "Decoder"), tr("Décodeur vidéo du téléviseur", "The TV's video decoder"), Value::Text(dec.into())) + 14.0;
            let link = match app.net.kind {
                NetKind::Ethernet => format!("Ethernet · {} Mbit/s", app.net.link_mbps.unwrap_or(100)),
                NetKind::Wifi => "Wi-Fi".into(),
                NetKind::Unknown => tr("inconnu", "unknown").into(),
            };
            y += widgets::row(cv, f, None, x, y, w, tr("Réseau", "Network"), &format!("{} {} Mbit/s", tr("Débit plafonné à", "Bitrate capped at"), app.net.cap_mbps()), Value::Text(link)) + 14.0;
            widgets::row(cv, f, None, x, y, w, "KyberNow Link", tr("Licence AGPL-3.0, sur le cœur de Kyber", "AGPL-3.0 licence, on Kyber's core"), Value::Text(app.version.into()));
        }
    }
}
