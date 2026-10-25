//! KyberNow Link OS : les réglages de l'appareil, d'après la maquette
//! (« Réglages de l'appareil ») : à gauche les rubriques et leur valeur, à
//! droite la rubrique ; « Mise à jour » a son panneau (l'emplacement en cours,
//! celui prêt à essayer, redémarrer ou plus tard, l'automatique, le retour
//! arrière). Dessiné en 1920 × 1080, à la manette.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::theme::*;
use super::widgets::{self, eyebrow, legend, primary, toggle};
use super::{launch, settings, Frame};
use crate::app::{App, Layout, NetKind, Rect, Section, Setting, Target, UpdateState, OS_SECTIONS};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(INK_BG);
    cv.glow(w * 0.8, h * 0.1, w * 0.68, h * 0.83, Rgba::hex(0x7A5CFF).alpha(0.22));
    // En-tête : logo › Réglages de l'appareil ; à droite les pastilles.
    let (lx, ly, lh) = launch::header_logo(Layout::Tv);
    let lw = cv.logo_link(lx, ly, lh, 1.0);
    let mut x = lx + lw + 18.0;
    x += cv.text(x, 48.0 + 9.0, Text::new(Font::Regular, 26.0, GHOST), "›") + 18.0;
    cv.text(x, 48.0 + 8.0, Text::new(Font::Regular, 24.0, MUTED), tr("Réglages de l'appareil", "Device settings"));
    let mut pills = Vec::new();
    if let Some(p) = &app.machine.pi {
        let t = app.machine.temp_c.map(|t| format!(" · {} °C", t.round() as i32)).unwrap_or_default();
        pills.push((format!("{} · {} {}{t}", p.model, p.ram_gb, tr("Go", "GB")), MUTED));
    }
    pills.push((net_label(app), MUTED));
    pills.push((crate::app_time::local_hhmm(), MUTED));
    super::home::pills(cv, w - 96.0, 48.0, &pills);

    // Les rubriques.
    let x0 = 96.0;
    let nav_w = 480.0;
    let mut ny = 96.0 + 24.0;
    for (i, s) in OS_SECTIONS.iter().enumerate() {
        let r = Rect::new(x0, ny, nav_w, 64.0);
        let (focused, hover) = f.state(Target::DeviceSection(i), r);
        let on = i == app.device_section;
        if on {
            cv.rrect(r.x, r.y, r.w, r.h, 12.0, INK_3);
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 12.0, 1.0, LINE_2);
        } else if hover {
            cv.rrect(r.x, r.y, r.w, r.h, 12.0, Rgba(227, 237, 240, 12));
        }
        if focused {
            widgets::ring(cv, r, 12.0);
        }
        cv.text(r.x + 24.0, r.y + 32.0 + 8.0, Text::new(Font::Regular, 24.0, if on { FROST } else { MUTED }), s.label());
        let (v, c) = nav_value(app, *s);
        cv.text(r.x + r.w - 24.0, r.y + 32.0 + 6.0, Text::new(Font::Mono, 17.0, c).align(Align::Right), &v);
        ny += 64.0 + 6.0;
    }

    // La rubrique.
    let mx = x0 + nav_w + 56.0;
    let mw = w - mx - 96.0;
    let section = app.device_section();
    let top = 96.0 + 24.0;
    cv.text_shrink(mx - 1.0, top + 46.0, Text::new(Font::Display, 56.0, FROST).tracking(-1.1), section.label(), mw);
    let desc = description(app, section);
    let dh = cv.paragraph(mx, top + 46.0 + 10.0 + 22.0, Text::new(Font::Regular, 22.0, MUTED), &desc, mw.min(1100.0), 33.0);
    let mut y = top + 46.0 + 10.0 + dh + 28.0;
    if section == Section::OsUpdate {
        update_panel(app, cv, f, mx, y, mw, h - 80.0 - 24.0);
    } else {
        let rows = app.rows(section);
        for r in &rows {
            y += settings::row_console(cv, f, r, mx, y, mw, 88.0, 1.5);
        }
    }

    // La légende.
    cv.rect(0.0, h - 80.0, w, 80.0, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - 80.0, w, 1.0, LINE);
    let gy = h - 40.0;
    let mut lx2 = x0;
    lx2 += legend(cv, lx2, gy, 34.0, &["A"], tr("Choisir", "Choose")) + 40.0;
    legend(cv, lx2, gy, 34.0, &["B"], tr("Retour à l'accueil", "Back home"));
    let other = if app.store.app.interface == "desktop" { tr("passer au Salon", "switch to Living room") } else { tr("passer au Bureau", "switch to Desktop") };
    let label = format!("{} : {other}", tr("Tenu 3 s", "Held 3 s"));
    let lw = widgets::legend_width(cv, 34.0, &["☰"], &label);
    legend(cv, w - 96.0 - lw, gy, 34.0, &["☰"], &label);
}

fn net_label(app: &App) -> String {
    match app.net.kind {
        NetKind::Ethernet => match app.net.link_mbps {
            Some(m) => format!("Ethernet {}", super::gbit_label(m)),
            None => "Ethernet".into(),
        },
        NetKind::Wifi => match &app.net.band {
            Some(b) => format!("Wi-Fi {b}"),
            None => "Wi-Fi".into(),
        },
        NetKind::Unknown => tr("Pas de réseau", "No network").into(),
    }
}

/// La valeur affichée à droite de chaque rubrique (maquette), et sa couleur.
fn nav_value(app: &App, s: Section) -> (String, Rgba) {
    let a = &app.store.app;
    let o = app.os.clone().unwrap_or_default();
    match s {
        Section::OsUsage => (if a.interface == "desktop" { tr("Bureau, clavier et souris", "Desktop, keyboard and mouse").into() } else { tr("Salon, à la manette", "Living room, controller").into() }, FAINT),
        Section::OsNetwork => (match app.net.kind {
            NetKind::Ethernet => tr("Ethernet · Wi-Fi éteint", "Ethernet · Wi-Fi off").into(),
            NetKind::Wifi => "Wi-Fi".into(),
            NetKind::Unknown => tr("aucun", "none").into(),
        }, FAINT),
        Section::OsControllers => (format!("{} · {}", app.pads.len(), tr("associer", "pair")), FAINT),
        Section::OsScreen => {
            let m = &app.machine;
            let mode = match a.os_hdmi.as_str() {
                "2160p60" => "4K 60 Hz".to_string(),
                "1080p120" => "1080p 120 Hz".into(),
                "1080p60" => "1080p 60 Hz".into(),
                _ => format!("{} {} Hz", crate::app::Machine::height_label(m.display.1), m.refresh),
            };
            (format!("{mode}{}", if a.os_cec { " · CEC" } else { "" }), FAINT)
        }
        Section::OsSound => (format!("HDMI {}", match app.machine.audio_channels { c if c >= 8 => "7.1", c if c >= 6 => "5.1", _ => tr("stéréo", "stereo") }), FAINT),
        Section::OsUpdate => {
            if let UpdateState::Available { version, .. } = &app.update {
                (format!("{version} {}", tr("prête", "ready")), WARN)
            } else if !o.pending.is_empty() {
                (format!("{} {}", tr("essai de la", "trying"), o.pending), WARN)
            } else {
                (tr("à jour", "up to date").into(), FAINT)
            }
        }
        Section::OsLanguage => (match a.language.as_str() { "fr" => "Français".into(), "en" => "English".into(), _ => format!("Auto · {}", if crate::i18n::english() { "English" } else { "Français" }) }, FAINT),
        Section::OsAdvanced => (format!("SSH {} · {}", if o.ssh { tr("allumé", "on") } else { tr("éteint", "off") }, tr("journal", "log")), FAINT),
        _ => (String::new(), FAINT),
    }
}

fn description(app: &App, s: Section) -> String {
    let _ = app;
    match s {
        Section::OsUsage => tr("Comment ce Raspberry Pi sert : au salon, Link prend l'interface à trois mètres et tout se fait à la manette ; au bureau, la fenêtre de Link remplit l'écran, au clavier et à la souris.", "What this Raspberry Pi is for: in the living room, Link takes the ten-foot interface and everything is done with the controller; at the desk, Link's window fills the screen, with keyboard and mouse.").into(),
        Section::OsNetwork => tr("Ethernet dès qu'un câble est branché ; sinon le Wi-Fi. L'ordinateur doit être sur le même réseau.", "Ethernet as soon as a cable is plugged in; Wi-Fi otherwise. The computer must be on the same network.").into(),
        Section::OsControllers => tr("Jusqu'à quatre manettes, en USB ou en Bluetooth. Le bouton Guide ou PS réveille l'appareil en veille.", "Up to four controllers, over USB or Bluetooth. The Guide or PS button wakes the device from standby.").into(),
        Section::OsScreen => tr("La sortie HDMI du Pi : 4K à 60 Hz au plus, 1080p à 120 Hz si le téléviseur l'annonce, pas de fréquence variable.", "The Pi's HDMI output: 4K at 60 Hz at most, 1080p at 120 Hz when the TV announces it, no variable refresh.").into(),
        Section::OsSound => tr("Le son sort par HDMI, en stéréo ou en PCM 5.1 / 7.1 selon ce que l'ordinateur envoie et ce que le téléviseur ou l'ampli accepte.", "Sound goes out over HDMI, in stereo or PCM 5.1 / 7.1 depending on what the computer sends and what the TV or amplifier accepts.").into(),
        Section::OsUpdate => tr("KyberNow Link OS se met à jour en entier : la nouvelle version s'installe à côté de l'actuelle, l'appareil l'essaie au redémarrage et revient en arrière tout seul si elle ne démarre pas. Vos ordinateurs et réglages restent.", "KyberNow Link OS updates as a whole: the new version installs next to the current one, the device tries it at reboot and rolls back by itself if it does not boot. Your computers and settings stay.").into(),
        Section::OsLanguage => tr("La langue des écrans de Link et de l'OS.", "The language of Link's and the OS's screens.").into(),
        Section::OsAdvanced => tr("Pour qui veut mettre les mains dedans : l'accès SSH, le redémarrage, l'arrêt.", "For those who want to get their hands in: SSH access, restart, shutdown.").into(),
        _ => String::new(),
    }
}

/// Le panneau Mise à jour (maquette) : deux emplacements, les boutons, l'automatique et le retour arrière.
fn update_panel(app: &App, cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, w: f32, bottom: f32) {
    let o = app.os.clone().unwrap_or_default();
    let sw = (w - 20.0) / 2.0;
    let sh = 150.0;
    // En cours.
    slot(cv, Rect::new(x, y, sw, sh), tr("En cours", "Running"), &o.version, &format!("Link {} · {}\n{} {} · {}", app.version, o.built, tr("emplacement", "slot"), o.slot, if o.pending.is_empty() { tr("démarrages sans faute", "boots without a fault") } else { tr("essai en cours", "trial in progress") }), false, FAINT);
    // Prête à essayer, ou à télécharger, ou rien.
    let (label, value, lines, on) = match (&app.update, &app.os_update.downloading, &app.os_update.ready) {
        (UpdateState::Available { version, size, .. }, Some((done, total)), None) => {
            let pct = if *total > 0 { (*done as f64 / *total as f64 * 100.0) as u32 } else { 0 };
            (tr("Téléchargement", "Downloading"), version.clone(), format!("{} % {} {} {}", pct, tr("de", "of"), mb(*size), tr("· vérifiée par le manifeste signé", "· checked against the signed manifest")), true)
        }
        (UpdateState::Available { version, size, .. }, _, Some(_)) => (tr("Prête à essayer", "Ready to try"), version.clone(), format!("{} ({}) · {}\n{}", tr("Téléchargée", "Downloaded"), mb(*size), tr("vérifiée, signée", "checked, signed"), tr("Essayée au redémarrage, retour automatique sinon", "Tried at restart, automatic rollback otherwise")), true),
        (UpdateState::Available { version, size, .. }, None, None) => (tr("Publiée", "Published"), version.clone(), format!("{} {} · {}", mb(*size), tr("à télécharger", "to download"), tr("signée KyberNow", "signed by KyberNow")), true),
        _ if !o.other_version.is_empty() => (tr("Autre emplacement", "Other slot"), o.other_version.clone(), format!("{} {}", tr("Version précédente, emplacement", "Previous version, slot"), o.other), false),
        _ => (tr("Rien de nouveau", "Nothing new"), "—".into(), format!("{} {}", tr("Dernière vérification :", "Last check:"), match &app.update { UpdateState::Checking => tr("en cours…", "in progress…").to_string(), UpdateState::Failed(e) => e.clone(), _ => tr("à l'instant", "just now").to_string() }), false),
    };
    slot(cv, Rect::new(x + sw + 20.0, y, sw, sh), label, &value, &lines, on, if on { SPECTRE } else { FAINT });
    let mut by = y + sh + 28.0;
    if let Some(e) = &app.os_update.error {
        cv.text_fit(x, by + 6.0, Text::new(Font::Regular, 19.0, WARN), e, w);
        by += 36.0;
    }
    // Les boutons.
    let mut bx = x;
    match (&app.update, &app.os_update.ready, &app.os_update.downloading) {
        (UpdateState::Available { version, .. }, Some(_), _) => {
            bx += primary(cv, f, Target::Setting(Setting::OsUpdateApply), bx, by, 72.0, &format!("{} {version}", tr("Redémarrer sur la", "Restart on")), false) + 20.0;
            widgets::ghost(cv, f, Target::Setting(Setting::OsUpdateLater), bx, by, 72.0, 24.0, None, tr("Plus tard", "Later"), MUTED);
        }
        (UpdateState::Available { .. }, None, Some(_)) => {
            widgets::ghost(cv, f, Target::Setting(Setting::OsUpdateLater), bx, by, 72.0, 24.0, None, tr("Téléchargement en cours…", "Downloading…"), MUTED);
        }
        (UpdateState::Available { version, size, .. }, None, None) => {
            bx += primary(cv, f, Target::Setting(Setting::OsUpdateApply), bx, by, 72.0, &format!("{} {version} ({})", tr("Télécharger la", "Download"), mb(*size)), false) + 20.0;
            widgets::ghost(cv, f, Target::Setting(Setting::OsUpdateLater), bx, by, 72.0, 24.0, None, tr("Plus tard", "Later"), MUTED);
        }
        _ => {
            widgets::ghost(cv, f, Target::Setting(Setting::CheckUpdate), bx, by, 72.0, 24.0, None, tr("Chercher une version", "Check for a version"), MUTED);
        }
    }
    // En bas : automatique, et si tout va mal.
    let ph = 150.0;
    let py = bottom - ph;
    cv.rrect(x, py, w, ph, 16.0, Rgba(9, 13, 18, 191));
    cv.rrect_stroke(x, py, w, ph, 16.0, 1.0, LINE);
    let col = (w - 56.0 - 40.0) / 2.0;
    let cx1 = x + 28.0;
    eyebrow(cv, cx1, py + 22.0 + 11.0, 15.0, FAINT, tr("Automatique", "Automatic"));
    let auto_text = tr("Télécharger les versions dès qu'elles sortent ; l'installation attend « Redémarrer sur la … ».", "Download versions as soon as they are out; installing waits for “Restart on …”.");
    cv.paragraph(cx1, py + 22.0 + 20.0 + 8.0 + 21.0, Text::new(Font::Regular, 21.0, MUTED), auto_text, col - 90.0, 31.0);
    let tr_ = Rect::new(cx1 + col - 74.0, py + 22.0 + 20.0 + 10.0, 74.0, 44.0);
    let (focused, _) = f.state(Target::Setting(Setting::OsAutoUpdate), tr_);
    if focused {
        widgets::ring(cv, tr_, 22.0);
    }
    toggle(cv, tr_.x, tr_.y, 44.0, app.store.app.os_auto_update, true);
    let cx2 = x + 28.0 + col + 40.0;
    eyebrow(cv, cx2, py + 22.0 + 11.0, 15.0, FAINT, tr("Si tout va mal", "If things go wrong"));
    let bad = tr("Une version qui ne démarre pas est abandonnée au redémarrage suivant. La carte se reflashe depuis Raspberry Pi Imager ; les ordinateurs et réglages restent.", "A version that does not boot is dropped at the next restart. The card can be reflashed from Raspberry Pi Imager; computers and settings stay.");
    let bh = cv.paragraph(cx2, py + 22.0 + 20.0 + 8.0 + 21.0, Text::new(Font::Regular, 21.0, MUTED), bad, col, 31.0);
    if !o.other_version.is_empty() && o.ab {
        widgets::ghost(cv, f, Target::Setting(Setting::OsRollback), cx2, py + 22.0 + 20.0 + 8.0 + bh + 10.0, 40.0, 16.0, None, &format!("{} {}", tr("Revenir à la", "Go back to"), o.other_version), MUTED);
    }
}

fn mb(bytes: u64) -> String {
    format!("{} {}", (bytes as f64 / 1_000_000.0).round() as u64, tr("Mo", "MB"))
}

/// Un emplacement : étiquette, version, lignes en Martian Mono.
fn slot(cv: &mut Canvas, r: Rect, label: &str, version: &str, lines: &str, on: bool, label_color: Rgba) {
    cv.rrect(r.x, r.y, r.w, r.h, 16.0, Rgba(14, 20, 26, 217));
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 16.0, 1.0, if on { SPECTRE } else { LINE });
    if on {
        cv.rrect_stroke(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0, 17.0, 1.0, SPECTRE.alpha(0.5));
    }
    eyebrow(cv, r.x + 28.0, r.y + 24.0 + 11.0, 14.0, label_color, label);
    cv.text_fit(r.x + 28.0, r.y + 24.0 + 20.0 + 8.0 + 24.0, Text::new(Font::Display, 30.0, FROST).tracking(-0.3), version, r.w - 56.0);
    let mut ly = r.y + 24.0 + 20.0 + 8.0 + 36.0 + 8.0 + 14.0;
    for l in lines.split('\n') {
        cv.text_fit(r.x + 28.0, ly, Text::new(Font::Mono, 17.0, FAINT), l, r.w - 56.0);
        ly += 27.0;
    }
}
