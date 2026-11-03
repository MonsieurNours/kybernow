//! Consoles PlayStation (docs/link-ui.md, « Consoles PlayStation ») : la console choisie à l'accueil, l'onglet
//! « PlayStation » de la fenêtre d'ajout et ses étapes, et ce que la partie ajoute à une session : le bandeau du
//! clavier de la console, l'aide du clavier et de la souris, le contenu protégé, la barre et les statistiques.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::widgets::{self, eyebrow, legend, primary};
use super::{unit, Frame};
use crate::app::{AddStep, App, Layout, Rect, SessionUi, Target};
use crate::i18n::{decimal, english, tr};
use crate::model::PsConsole;
use crate::ps::{consoles_label, spaced_code, PsState, PsStep};

/// La couleur de l'état d'une console : vert allumée (ou par le relais), ambre en veille, éteint hors ligne.
pub fn state_color(s: PsState) -> (Rgba, bool) {
    match s {
        PsState::Ready | PsState::Away => (OK, true),
        PsState::Standby => (WARN, true),
        PsState::Offline => (GHOST, false),
        PsState::Unknown => (FAINT, false),
    }
}

/// Le fond d'une console : sa signature, d'après son identifiant (assombrie hors ligne).
pub fn backdrop(cv: &mut Canvas, c: &PsConsole, offline: bool) {
    super::signature(cv, Some(&c.host_id), offline);
    super::shade(cv);
}

// MARK: - Accueil

/// La console choisie, en haut de l'accueil : son état, son nom, son modèle et le jeu en cours, le bouton
/// principal, la puce du compte (plusieurs comptes), « Auto » ou « Personnalisé », « Réglages ».
#[allow(clippy::too_many_arguments)]
pub fn hero(app: &App, cv: &mut Canvas, f: &mut Frame, c: &PsConsole, x0: f32, y0: f32, u: f32, hero_max: f32) {
    let desktop = app.layout == Layout::Desktop;
    let live = app.ps_live(&c.host_id);
    let (color, glow) = state_color(live.state);
    let mut y = y0;
    widgets::dot(cv, x0 + 4.0 * u, y + 7.0 * u, 4.0 * u, color, glow);
    eyebrow(cv, x0 + 16.0 * u, y + 11.0 * u, if desktop { 11.0 } else { 12.0 * u }, color, live.state.label());
    y += 26.0 * u;
    let size = if desktop { 60.0 } else { 64.0 * u };
    cv.text_shrink(x0 - 2.0, y + 46.0 * u, Text::new(Font::Display, size, FROST).tracking(-1.2 * u), &c.name, hero_max);
    y += 68.0 * u;
    // Le modèle, en toutes lettres (aucun logo), et le jeu en cours quand la console en annonce un.
    let mut line = c.model.clone();
    if let Some(game) = live.running_app.as_ref().filter(|g| !g.is_empty()) {
        line = format!("{line} · {game}");
    }
    cv.text_fit(x0, y + 12.0 * u, Text::new(Font::Mono, 13.0 * u, MUTED), &line, hero_max);
    y += 28.0 * u + 24.0 * u;
    let bh = if desktop { 52.0 } else { 60.0 * u };
    let (target, label, play) = app.ps_main_action(c);
    let mut x = x0;
    x += primary(cv, f, target, x, y, bh, label, play) + 18.0 * u;
    let chip_h = bh - 6.0 * u;
    let size = if desktop { 14.0 } else { 17.0 * u };
    // Plusieurs comptes : le compte avec lequel la console ouvre la partie.
    let (account, many) = app.ps_account_of(c);
    let mut account_x = x;
    if many {
        account_x = x;
        x += widgets::ghost(cv, f, Target::PsAccountChip, x, y + 3.0 * u, chip_h, size, None, &format!("{account}  ▾"), FROST) + 12.0 * u;
    }
    let custom = c.settings.custom();
    x += widgets::ghost(cv, f, Target::PsCustomChip, x, y + 3.0 * u, chip_h, size, None, if custom { tr("Personnalisé", "Custom") } else { "Auto" }, MUTED) + 12.0 * u;
    widgets::ghost(cv, f, Target::PsSettings, x, y + 3.0 * u, chip_h, size, Some(Icon::Gear), tr("Réglages", "Settings"), MUTED);
    y += bh + 26.0 * u;
    if let Some((id, m)) = &app.ps.message
        && id.eq_ignore_ascii_case(&c.host_id)
    {
        cv.text_fit(x0, y, Text::new(Font::Regular, 15.0 * u, WARN), m, hero_max);
    }
    // Le choix du compte : une ligne par compte associé, le choisi coché.
    if many && app.ps.account_menu {
        let mw = 300.0 * u;
        let rh = 44.0 * u;
        let my = y - 20.0 * u;
        let n = c.pairings.len();
        cv.rrect(account_x, my, mw, rh * n as f32 + 12.0 * u, 10.0 * u, Rgba(9, 13, 18, 245));
        cv.rrect_stroke(account_x, my, mw, rh * n as f32 + 12.0 * u, 10.0 * u, 1.0, LINE_2);
        for (i, p) in c.pairings.iter().enumerate() {
            let r = Rect::new(account_x + 6.0 * u, my + 6.0 * u + i as f32 * rh, mw - 12.0 * u, rh);
            let (focused, hover) = f.state(Target::PsAccountPick(i), r);
            if hover {
                cv.rrect(r.x, r.y, r.w, r.h, 8.0 * u, Rgba(227, 237, 240, 16));
            }
            if focused {
                widgets::ring(cv, r, 8.0 * u);
            }
            let name = app.store.ps_account(&p.account).map(|a| a.label()).unwrap_or_default();
            let chosen = c.pairing().is_some_and(|q| q.account == p.account);
            if chosen {
                cv.icon(&Icon::Check, r.x + 12.0 * u, r.y + r.h / 2.0 - 7.0 * u, 14.0 * u, 2.2, SPECTRE);
            }
            cv.text_fit(r.x + 36.0 * u, r.y + r.h / 2.0 + 5.0 * u, Text::new(Font::Regular, 15.0 * u, FROST), &name, r.w - 48.0 * u);
        }
    }
}

/// La légende de l'accueil à la manette, quand une console est choisie.
pub fn legend_label(app: &App, c: &PsConsole) -> &'static str {
    app.ps_main_action(c).1
}

/// Les cartes des consoles dans la rangée de l'accueil : associées (nom et état), puis celles du réseau pas encore
/// associées (« Sur le réseau »). Rend la place prise.
#[allow(clippy::too_many_arguments)]
pub fn cards(app: &App, cv: &mut Canvas, f: &mut Frame, x: &mut f32, y: f32, cw: f32, ch: f32, gap: f32, u: f32, slots: &mut usize, selected: Option<&str>) {
    for (i, c) in app.store.ps_consoles.iter().enumerate() {
        if *slots == 0 {
            return;
        }
        let state = app.ps_state(&c.host_id);
        let (dot, _) = state_color(state);
        let key = format!("{}{}", crate::ps::SELECTED, c.host_id);
        let line = format!("{} · {}", c.model, state.label());
        widgets::card(cv, f, Target::Console(i), Rect::new(*x, y, cw, ch), u, &c.name, Some(dot), None, &[&line], selected == Some(key.as_str()), false, state == PsState::Offline);
        *x += cw + gap;
        *slots -= 1;
    }
    for (i, fd) in app.new_ps_found().iter().enumerate() {
        if *slots == 0 {
            return;
        }
        let line = format!("{} · {}", fd.model, tr("Sur le réseau", "On your network"));
        widgets::card(cv, f, Target::PsFound(i), Rect::new(*x, y, cw, ch), u, &fd.name, None, None, &[&line], false, true, false);
        *x += cw + gap;
        *slots -= 1;
    }
}

// MARK: - Onglet PlayStation

fn spinner(app: &App, cv: &mut Canvas, cx: f32, cy: f32, r: f32, u: f32) {
    let t = app.screen_since.elapsed().as_secs_f32();
    let a0 = if app.reduce_motion { 0.0 } else { t * 4.0 };
    let pts: Vec<(f32, f32)> = (0..=16)
        .map(|i| {
            let a = a0 + i as f32 * 0.28;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    cv.polyline(&pts, 2.0 * u, SPECTRE);
}

/// L'onglet PlayStation et ses étapes.
pub fn add(app: &App, cv: &mut Canvas, f: &mut Frame, step: &PsStep, x0: f32, y0: f32, u: f32, footer: f32) {
    let mut y = y0;
    if app.layout == Layout::Desktop {
        widgets::ghost(cv, f, Target::Back, x0, y, 40.0, 14.0, Some(Icon::Back), tr("Retour", "Back"), MUTED);
        y += 56.0;
    }
    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, tr("Ajouter un ordinateur ou une console", "Add a computer or a console"));
    match step {
        PsStep::Tab => tab(app, cv, f, x0, y, u, footer),
        PsStep::WhichAccount { console } => which(app, cv, f, console, x0, y, u),
        PsStep::Account { .. } => account(app, cv, f, x0, y, u),
        PsStep::Paste { .. } => paste(app, cv, f, x0, y, u),
        PsStep::ByHand { .. } => by_hand(app, cv, f, x0, y, u),
        PsStep::Code { console, account } => code(app, cv, f, console, account, x0, y, u),
        PsStep::Rename { .. } => rename(app, cv, f, x0, y, u),
        PsStep::Send { name, .. } => send(app, cv, f, name, x0, y, u),
        PsStep::Receive => receive(app, cv, f, x0, y, u),
    }
}

fn title(cv: &mut Canvas, x0: f32, y: f32, u: f32, s: &str, max: f32) {
    cv.text_shrink(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), s, max);
}

/// Une ligne de liste (compte, console) : un fond, puis ce que la fonction y dessine.
fn row_bg(cv: &mut Canvas, r: Rect, u: f32) {
    cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, CARD);
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, LINE);
}

#[allow(clippy::too_many_arguments)]
fn tab(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32, footer: f32) {
    let (w, h) = (cv.w, cv.h);
    let desktop = app.layout == Layout::Desktop;
    let left_w = (620.0 * u).min(w * 0.52);
    title(cv, x0, y0, u, "PlayStation", left_w);
    let mut y = y0 + 84.0 * u;
    let small = if desktop { 13.0 } else { 15.0 * u };
    let bh = if desktop { 34.0 } else { 44.0 * u };
    // Les comptes.
    eyebrow(cv, x0, y + 12.0 * u, 11.5 * u, FAINT, tr("Comptes PlayStation Network", "PlayStation Network accounts"));
    y += 24.0 * u;
    if app.store.ps_accounts.is_empty() {
        cv.text_fit(x0, y + 16.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), tr("Aucun compte : ajoutez-en un pour associer une console.", "No account: add one to pair a console."), left_w);
        y += 30.0 * u;
    }
    for (i, a) in app.store.ps_accounts.iter().enumerate().take(4) {
        let rh = 62.0 * u;
        let r = Rect::new(x0, y, left_w, rh);
        row_bg(cv, r, u);
        let n = app.store.consoles_of(&a.id);
        // Les boutons d'abord : le nom prend la place qui reste.
        let remove_w = cv.text_width(Text::new(Font::Regular, small, MUTED), tr("Retirer", "Remove")) + small * 2.0;
        let rx = r.x + r.w - 12.0 * u - remove_w;
        widgets::ghost(cv, f, Target::PsAccountRemove(i), rx, r.y + (rh - bh) / 2.0, bh, small, None, tr("Retirer", "Remove"), MUTED);
        let rename_w = cv.text_width(Text::new(Font::Regular, small, MUTED), tr("Renommer", "Rename")) + small * 2.0;
        let nx = rx - 8.0 * u - rename_w;
        widgets::ghost(cv, f, Target::PsAccountRename(i), nx, r.y + (rh - bh) / 2.0, bh, small, None, tr("Renommer", "Rename"), MUTED);
        cv.text_fit(r.x + 18.0 * u, r.y + 27.0 * u, Text::new(Font::Semibold, 16.0 * u, FROST), &a.label(), nx - r.x - 30.0 * u);
        cv.text_fit(r.x + 18.0 * u, r.y + 48.0 * u, Text::new(Font::Mono, 11.5 * u, FAINT), &consoles_label(n), nx - r.x - 30.0 * u);
        y += rh + 8.0 * u;
    }
    widgets::ghost(cv, f, Target::PsAddAccount, x0, y + 4.0 * u, bh + 4.0 * u, small, Some(Icon::Plus), tr("Ajouter un compte", "Add an account"), MUTED);
    y += bh + 30.0 * u;
    // Les consoles du réseau.
    eyebrow(cv, x0, y + 12.0 * u, 11.5 * u, FAINT, tr("Consoles sur ce réseau", "Consoles on this network"));
    y += 24.0 * u;
    if app.ps.found.is_empty() {
        spinner(app, cv, x0 + 9.0 * u, y + 12.0 * u, 8.0 * u, u);
        let msg = tr(
            "Aucune console trouvée. Allumez-la ou mettez-la en veille, ou saisissez son adresse dans « Par adresse ».",
            "No console found. Turn it on or put it in rest mode, or enter its address in “By address”.",
        );
        cv.paragraph(x0 + 28.0 * u, y + 17.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT), msg, left_w - 28.0 * u, 20.0 * u);
    }
    let bottom = h - footer - 20.0 * u;
    for (i, fd) in app.ps.found.iter().enumerate() {
        let rh = 66.0 * u;
        if y + rh > bottom {
            break;
        }
        let r = Rect::new(x0, y, left_w, rh);
        row_bg(cv, r, u);
        let (dot, glow) = state_color(fd.state);
        widgets::dot(cv, r.x + 20.0 * u, r.y + 26.0 * u, 4.5 * u, dot, glow);
        let paired: Vec<String> = app.store.console(&fd.host_id).map(|c| c.pairings.iter().filter_map(|p| app.store.ps_account(&p.account).map(|a| a.label())).collect()).unwrap_or_default();
        // À droite : « Associer », ou « Associer avec un autre compte » s'il reste un compte qui n'y est pas associé.
        let free = !app.ps_free_accounts(&fd.host_id).is_empty();
        let mut right = r.x + r.w - 12.0 * u;
        if paired.is_empty() {
            let label = tr("Associer", "Pair");
            let bw = cv.text_width(Text::new(Font::Regular, small, FROST), label) + small * 2.0;
            right -= bw;
            widgets::ghost(cv, f, Target::PsTabFound(i), right, r.y + (rh - bh) / 2.0, bh, small, None, label, FROST);
        } else if free {
            let label = tr("Associer avec un autre compte", "Pair with another account");
            let bw = cv.text_width(Text::new(Font::Regular, small, MUTED), label) + small * 2.0;
            right -= bw;
            widgets::ghost(cv, f, Target::PsPairOther(i), right, r.y + (rh - bh) / 2.0, bh, small, None, label, MUTED);
        }
        let tw = right - r.x - 50.0 * u;
        cv.text_fit(r.x + 36.0 * u, r.y + 30.0 * u, Text::new(Font::Semibold, 16.0 * u, FROST), &fd.name, tw);
        let line = if paired.is_empty() { format!("{} · {}", fd.model, fd.state.label()) } else { format!("{} · {} {}", fd.model, tr("Associée :", "Paired:"), paired.join(", ")) };
        cv.text_fit(r.x + 36.0 * u, r.y + 51.0 * u, Text::new(Font::Mono, 11.5 * u, FAINT), &line, tw);
        y += rh + 8.0 * u;
    }

    // À droite : envoyer (un appareil qui a des comptes, au bureau), recevoir.
    let px = x0 + left_w + 40.0 * u;
    let pw = w - px - x0;
    if pw < 280.0 {
        return;
    }
    let py = y0 + 84.0 * u;
    let ph = (h - footer - 28.0 * u - py).min(560.0 * u);
    cv.rrect(px, py, pw, ph, 14.0 * u, Rgba(9, 13, 18, 205));
    cv.rrect_stroke(px, py, pw, ph, 14.0 * u, 1.0, LINE);
    let ix = px + 28.0 * u;
    let iw = pw - 56.0 * u;
    let mut yy = py + 40.0 * u;
    if let Some(done) = &app.ps.add.done {
        cv.icon(&Icon::Check, ix, yy - 14.0 * u, 18.0 * u, 2.4, OK);
        yy += cv.paragraph(ix + 28.0 * u, yy, Text::new(Font::Regular, 15.0 * u, FROST), done, iw - 28.0 * u, 22.0 * u) + 10.0 * u;
    }
    if desktop && !app.store.ps_accounts.is_empty() {
        eyebrow(cv, ix, yy, 11.5 * u, FAINT, tr("Envoyer à un appareil", "Send to a device"));
        yy += 18.0 * u;
        if app.ps.add.devices.is_empty() {
            spinner(app, cv, ix + 9.0 * u, yy + 12.0 * u, 7.0 * u, u);
            let msg = tr(
                "Aucun appareil en attente. Sur la télé ou l'autre appareil : Ajouter une console › Recevoir d'un autre appareil.",
                "No device waiting. On the TV or the other device: Add a console › Receive from another device.",
            );
            yy += cv.paragraph(ix + 28.0 * u, yy + 17.0 * u, Text::new(Font::Regular, 14.0 * u, FAINT), msg, iw - 28.0 * u, 20.0 * u) + 24.0 * u;
        }
        for (i, d) in app.ps.add.devices.iter().enumerate().take(4) {
            let r = Rect::new(ix, yy, iw, 48.0 * u);
            let (focused, hover) = f.state(Target::PsSendTo(i), r);
            cv.rrect(r.x, r.y, r.w, r.h, 9.0 * u, if hover { INK_3 } else { INK_2 });
            cv.rrect_stroke(r.x, r.y, r.w, r.h, 9.0 * u, 1.0, if focused { SPECTRE } else { LINE_2 });
            if focused {
                widgets::ring(cv, r, 9.0 * u);
            }
            cv.icon(&Icon::Share, r.x + 14.0 * u, r.y + 15.0 * u, 18.0 * u, 1.8, SPECTRE);
            cv.text_fit(r.x + 44.0 * u, r.y + 30.0 * u, Text::new(Font::Regular, 15.0 * u, FROST), &d.label(), r.w - 56.0 * u);
            yy += 56.0 * u;
        }
        yy += 18.0 * u;
    }
    eyebrow(cv, ix, yy, 11.5 * u, FAINT, tr("Recevoir d'un autre appareil", "Receive from another device"));
    yy += 14.0 * u;
    let lead = tr(
        "Les comptes et les consoles déjà associées d'un Mac, d'un PC ou d'un téléphone arrivent ici, sans refaire le code de la console.",
        "Accounts and consoles already paired on a Mac, PC or phone come here, without the console's code again.",
    );
    yy += cv.paragraph(ix, yy + 16.0 * u, Text::new(Font::Regular, 14.0 * u, MUTED), lead, iw, 20.0 * u) + 20.0 * u;
    widgets::ghost(cv, f, Target::PsReceive, ix, yy, bh + 4.0 * u, small, Some(Icon::Remote), tr("Recevoir d'un autre appareil", "Receive from another device"), FROST);
}

/// « Avec quel compte ? » : une ligne par compte pas encore associé à cette console, et « Ajouter un compte ».
fn which(app: &App, cv: &mut Canvas, f: &mut Frame, console: &str, x0: f32, y0: f32, u: f32) {
    let w = (cv.w - 2.0 * x0).min(620.0 * u);
    let name = app.ps.found.iter().find(|c| c.host_id.eq_ignore_ascii_case(console)).map(|c| c.name.clone()).or_else(|| app.store.console(console).map(|c| c.name.clone())).unwrap_or_default();
    title(cv, x0, y0, u, tr("Avec quel compte ?", "Which account?"), w);
    let mut y = y0 + 84.0 * u;
    cv.text_fit(x0, y + 4.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT), &name, w);
    y += 24.0 * u;
    for (i, a) in app.ps_free_accounts(console).iter().enumerate().take(6) {
        let r = Rect::new(x0, y, w, 56.0 * u);
        let (focused, hover) = f.state(Target::PsWhich(i), r);
        cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { CARD });
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, 1.0, if focused { SPECTRE } else { LINE });
        if focused {
            widgets::ring(cv, r, 10.0 * u);
        }
        cv.text_fit(r.x + 20.0 * u, r.y + 34.0 * u, Text::new(Font::Semibold, 17.0 * u, FROST), &a.label(), r.w - 40.0 * u);
        y += 64.0 * u;
    }
    widgets::ghost(cv, f, Target::PsWhichNew, x0, y + 8.0 * u, 44.0 * u, 15.0 * u, Some(Icon::Plus), tr("Ajouter un compte", "Add an account"), MUTED);
}

/// L'étape Compte : se connecter à PSN (bureau : la page de Sony dans le navigateur), recevoir d'un autre appareil,
/// saisir l'identifiant à la main (seuls ces deux-là au salon, sur le Deck et sur KyberNow Link OS).
fn account(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = (cv.w - 2.0 * x0).min(700.0 * u);
    title(cv, x0, y0, u, tr("Ajouter un compte", "Add an account"), w);
    let mut y = y0 + 90.0 * u;
    let desktop = app.layout == Layout::Desktop;
    let lead = if desktop {
        tr(
            "Connectez-vous au compte PlayStation Network à associer. KyberNow lit seulement l'identifiant et le pseudo du compte, une seule fois.",
            "Sign in to the PlayStation Network account to pair. KyberNow only reads the account's ID and online ID, once.",
        )
    } else {
        tr(
            "Recevez le compte d'un Mac, d'un PC ou d'un téléphone où il est déjà, ou saisissez son identifiant.",
            "Receive the account from a Mac, PC or phone that already has it, or enter its ID.",
        )
    };
    y += cv.paragraph(x0, y, Text::new(Font::Regular, 17.0 * u, FROST), lead, w, 25.0 * u) + 18.0 * u;
    let bh = if desktop { 52.0 } else { 60.0 * u };
    if desktop {
        primary(cv, f, Target::PsSignIn, x0, y, bh, tr("Se connecter à PlayStation Network", "Sign in to PlayStation Network"), false);
        y += bh + 20.0 * u;
        let bw = widgets::ghost(cv, f, Target::PsReceive, x0, y, 44.0, 14.0, Some(Icon::Remote), tr("Recevoir d'un autre appareil", "Receive from another device"), MUTED);
        widgets::ghost(cv, f, Target::PsByHand, x0 + bw + 12.0, y, 44.0, 14.0, None, tr("Saisir l'identifiant à la main", "Enter the ID by hand"), MUTED);
    } else {
        let bw = primary(cv, f, Target::PsReceive, x0, y, bh, tr("Recevoir d'un autre appareil", "Receive from another device"), false);
        widgets::ghost(cv, f, Target::PsByHand, x0 + bw + 18.0 * u, y + 4.0 * u, bh - 8.0 * u, 16.0 * u, None, tr("Saisir l'identifiant à la main", "Enter the ID by hand"), MUTED);
    }
    if let Some(m) = &app.ps.add.message {
        cv.text_fit(x0, y + bh + 40.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), m, w);
    }
}

/// Un champ de saisie : son libellé, sa valeur (ou l'exemple en gris), le curseur quand il est actif.
#[allow(clippy::too_many_arguments)]
fn field(app: &App, cv: &mut Canvas, f: &mut Frame, i: u8, label: &str, value: &str, placeholder: &str, x: f32, y: f32, w: f32, u: f32, mono: bool) -> f32 {
    let active = app.ps.add.field == i as usize;
    let mut yy = y;
    if !label.is_empty() {
        eyebrow(cv, x, y + 8.0 * u, 11.5 * u, FAINT, label);
        yy += 20.0 * u;
    }
    let r = Rect::new(x, yy, w, 56.0 * u);
    let (focused, hover) = f.state(Target::PsField(i), r);
    if focused {
        widgets::ring(cv, r, 10.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 10.0 * u, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * u, if active { 1.5 } else { 1.0 }, if active { SPECTRE } else { LINE_2 });
    let empty = value.is_empty();
    let style = Text::new(if mono { Font::Mono } else { Font::Regular }, 20.0 * u, if empty { GHOST } else { FROST });
    // Une longue adresse collée : la fin se voit.
    let shown: String = if empty {
        placeholder.into()
    } else {
        let max_chars = ((r.w - 44.0 * u) / (11.0 * u)).max(8.0) as usize;
        let n = value.chars().count();
        if n > max_chars { format!("…{}", value.chars().skip(n - max_chars + 1).collect::<String>()) } else { value.into() }
    };
    let tw = cv.text_fit(r.x + 18.0 * u, r.y + r.h / 2.0 + 7.0 * u, style, &shown, r.w - 40.0 * u);
    if active && !empty && !app.ps.add.busy && !app.reduce_motion && (app.screen_since.elapsed().as_millis() / 530) % 2 == 0 {
        cv.rect(r.x + 20.0 * u + tw, r.y + 14.0 * u, 2.0, r.h - 28.0 * u, SPECTRE);
    }
    yy - y + 56.0 * u + 16.0 * u
}

/// Le bouton de l'étape, l'attente, le message.
fn go_row(app: &App, cv: &mut Canvas, f: &mut Frame, label: &str, enabled: bool, x0: f32, y: f32, u: f32, w: f32) -> f32 {
    let bh = if app.layout == Layout::Desktop { 52.0 } else { 60.0 * u };
    let bw = widgets::primary_state(cv, f, Target::PsGo, x0, y, bh, label, false, enabled && !app.ps.add.busy);
    let (mx, mid) = (x0 + bw + 24.0 * u, y + bh / 2.0);
    if app.ps.add.busy {
        spinner(app, cv, mx + 9.0 * u, mid, 8.0 * u, u);
    } else if let Some(m) = &app.ps.add.message {
        cv.paragraph(mx, mid + 5.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), m, (x0 + w - mx).max(220.0 * u), 22.0 * u);
    }
    bh
}

/// Au salon et sur le Deck : le pavé à droite du champ (chiffres, ou lettres).
fn pad(app: &App, cv: &mut Canvas, f: &mut Frame, digits: bool, x: f32, y: f32, width: f32, u: f32) {
    if !app.layout.console() {
        return;
    }
    if digits {
        let keys = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "⌫", "0", "OK"];
        let kw = 72.0 * u;
        let kh = 56.0 * u;
        let gap = 10.0 * u;
        for (i, k) in keys.iter().enumerate() {
            let (col, row) = (i % 3, i / 3);
            let r = Rect::new(x + col as f32 * (kw + gap), y + row as f32 * (kh + gap), kw, kh);
            match *k {
                "⌫" => super::pair::key(cv, f, Target::KeyErase, r, "⌫", u),
                "OK" => super::pair::key(cv, f, Target::KeyOk, r, "OK", u),
                k => super::pair::key(cv, f, Target::Key(k.chars().next().unwrap()), r, k, u),
            }
        }
    } else {
        super::join::letter_pad(app.ps.add.shift, cv, f, x, y, width, u);
        // Le base64 d'un identifiant : + / = en plus.
        let kw = 64.0 * u;
        let ky = y + 4.0 * (52.0 * u + 8.0 * u);
        for (i, k) in ["+", "/", "=", "-", "_"].iter().enumerate() {
            super::pair::key(cv, f, Target::Key(k.chars().next().unwrap()), Rect::new(x + i as f32 * (kw + 8.0 * u), ky, kw, 52.0 * u), k, u);
        }
    }
}

/// Bureau : la page de Sony s'est ouverte dans le navigateur ; l'adresse où elle a mené est collée ici.
fn paste(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = (cv.w - 2.0 * x0).min(760.0 * u);
    title(cv, x0, y0, u, tr("Se connecter à PlayStation Network", "Sign in to PlayStation Network"), w);
    let mut y = y0 + 90.0 * u;
    let lead = tr(
        "La page de connexion de Sony s'est ouverte dans votre navigateur. Une fois connecté, Sony affiche une page presque vide : copiez son adresse (elle commence par https://remoteplay.dl.playstation.net) et collez-la ici.",
        "Sony's sign-in page opened in your browser. Once signed in, Sony shows an almost empty page: copy its address (it starts with https://remoteplay.dl.playstation.net) and paste it here.",
    );
    y += cv.paragraph(x0, y, Text::new(Font::Regular, 16.0 * u, MUTED), lead, w, 24.0 * u) + 12.0 * u;
    y += field(app, cv, f, 0, tr("Adresse de la page de Sony", "Sony page address"), &app.ps.add.fields[0], "https://remoteplay.dl.playstation.net/remoteplay/redirect?code=…", x0, y, w, u, true);
    y += 4.0 * u;
    let bh = go_row(app, cv, f, tr("Continuer", "Continue"), !app.ps.add.fields[0].trim().is_empty(), x0, y, u, w);
    y += bh + 24.0 * u;
    let bw = widgets::ghost(cv, f, Target::PsSignIn, x0, y, 40.0, 13.0, Some(Icon::Refresh), tr("Rouvrir la page de Sony", "Open Sony's page again"), MUTED);
    widgets::ghost(cv, f, Target::PsByHand, x0 + bw + 12.0, y, 40.0, 13.0, None, tr("Saisir l'identifiant à la main", "Enter the ID by hand"), MUTED);
}

fn by_hand(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let console = app.layout.console();
    let w = if console { (cv.w * 0.44).min(560.0 * u) } else { (cv.w - 2.0 * x0).min(640.0) };
    title(cv, x0, y0, u, tr("Saisir l'identifiant à la main", "Enter the ID by hand"), cv.w - 2.0 * x0);
    let mut y = y0 + 86.0 * u;
    y += field(app, cv, f, 0, tr("Identifiant du compte", "Account ID"), &app.ps.add.fields[0], "AbCdEfGhIjk=", x0, y, w, u, true);
    y += field(app, cv, f, 1, tr("Nom", "Name"), &app.ps.add.fields[1], "", x0, y, w, u, false);
    cv.text_fit(x0, y - 4.0 * u, Text::new(Font::Regular, 13.0 * u, FAINT), tr("Pour le reconnaître dans la liste.", "To recognise it in the list."), w);
    y += 22.0 * u;
    go_row(app, cv, f, tr("Continuer", "Continue"), !app.ps.add.fields[0].trim().is_empty(), x0, y, u, w);
    let px = x0 + w + 40.0 * u;
    pad(app, cv, f, false, px, y0 + 86.0 * u, cv.w - px - x0, u);
}

/// Le code à 8 chiffres de la console.
#[allow(clippy::too_many_arguments)]
fn code(app: &App, cv: &mut Canvas, f: &mut Frame, console: &str, account: &str, x0: f32, y0: f32, u: f32) {
    let w = if app.layout.console() { (cv.w * 0.5).min(640.0 * u) } else { (cv.w - 2.0 * x0).min(700.0 * u) };
    let name = app.ps.found.iter().find(|c| c.host_id.eq_ignore_ascii_case(console)).map(|c| c.name.clone()).or_else(|| app.store.console(console).map(|c| c.name.clone())).unwrap_or_default();
    title(cv, x0, y0, u, &format!("{} {name}", tr("Associer", "Pair")), cv.w - 2.0 * x0);
    let mut y = y0 + 86.0 * u;
    let pseudo = app.store.ps_account(account).map(|a| a.label()).unwrap_or_default();
    cv.text_fit(x0, y + 4.0 * u, Text::new(Font::Mono, 13.0 * u, FAINT), &format!("{} {pseudo}", tr("Compte :", "Account:")), w);
    y += 26.0 * u;
    let lead = tr(
        "Sur la console : Réglages › Système › Lecture à distance › Associer un appareil. Saisissez le code affiché.",
        "On the console: Settings › System › Remote Play › Link Device. Enter the code shown.",
    );
    y += cv.paragraph(x0, y + 14.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), lead, w, 25.0 * u) + 24.0 * u;
    y += field(app, cv, f, 0, tr("Code à 8 chiffres", "8-digit code"), &app.ps.add.fields[0], "12345678", x0, y, (300.0 * u).min(w), u, true);
    go_row(app, cv, f, tr("Associer", "Pair"), app.ps.add.fields[0].len() == 8, x0, y + 4.0 * u, u, w);
    let px = x0 + w + 40.0 * u;
    pad(app, cv, f, true, px, y0 + 86.0 * u, cv.w - px - x0, u);
}

fn rename(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let console = app.layout.console();
    let w = if console { (cv.w * 0.44).min(560.0 * u) } else { (cv.w - 2.0 * x0).min(640.0) };
    title(cv, x0, y0, u, tr("Renommer", "Rename"), w);
    let mut y = y0 + 86.0 * u;
    y += field(app, cv, f, 0, tr("Nom", "Name"), &app.ps.add.fields[0], tr("Compte PlayStation", "PlayStation account"), x0, y, w, u, false);
    go_row(app, cv, f, tr("Enregistrer", "Save"), true, x0, y + 4.0 * u, u, w);
    let px = x0 + w + 40.0 * u;
    pad(app, cv, f, false, px, y0 + 86.0 * u, cv.w - px - x0, u);
}

/// « Envoyer à un appareil » : le code affiché par l'appareil choisi.
fn send(app: &App, cv: &mut Canvas, f: &mut Frame, name: &str, x0: f32, y0: f32, u: f32) {
    let w = (cv.w - 2.0 * x0).min(640.0 * u);
    title(cv, x0, y0, u, tr("Envoyer à un appareil", "Send to a device"), w);
    let mut y = y0 + 90.0 * u;
    let lead = if english() { format!("Enter the code shown on {name}.") } else { format!("Saisissez le code affiché sur {name}.") };
    y += cv.paragraph(x0, y + 8.0 * u, Text::new(Font::Regular, 17.0 * u, FROST), &lead, w, 25.0 * u) + 20.0 * u;
    y += field(app, cv, f, 0, tr("Code à 6 chiffres", "6-digit code"), &app.ps.add.fields[0], "482913", x0, y, (260.0 * u).min(w), u, true);
    go_row(app, cv, f, tr("Envoyer", "Send"), app.ps.add.fields[0].len() == 6, x0, y + 4.0 * u, u, w);
    let px = x0 + w + 40.0 * u;
    pad(app, cv, f, true, px, y0 + 90.0 * u, cv.w - px - x0, u);
}

/// « Recevoir d'un autre appareil » : le code en grand ; reçu, expiré.
fn receive(app: &App, cv: &mut Canvas, f: &mut Frame, x0: f32, y0: f32, u: f32) {
    let w = (cv.w - 2.0 * x0).min(820.0 * u);
    title(cv, x0, y0, u, tr("Recevoir d'un autre appareil", "Receive from another device"), w);
    let mut y = y0 + 100.0 * u;
    let a = &app.ps.add;
    if let Some(done) = &a.done {
        cv.icon(&Icon::Check, x0, y - 4.0 * u, 26.0 * u, 2.6, OK);
        cv.text_fit(x0 + 40.0 * u, y + 18.0 * u, Text::new(Font::Regular, 22.0 * u, FROST), done, w - 40.0 * u);
        return;
    }
    if a.receive_expired {
        cv.text_fit(x0, y + 18.0 * u, Text::new(Font::Regular, 20.0 * u, WARN), tr("Code expiré.", "Code expired."), w);
        primary(cv, f, Target::PsNewCode, x0, y + 50.0 * u, if app.layout == Layout::Desktop { 52.0 } else { 60.0 * u }, tr("Nouveau code", "New code"), false);
        return;
    }
    match &a.receive_code {
        Some(code) => {
            let style = Text::new(Font::MonoMedium, 96.0 * u, CORE).tracking(6.0 * u);
            cv.text(x0, y + 90.0 * u, style, &spaced_code(code));
            y += 130.0 * u;
        }
        None => {
            spinner(app, cv, x0 + 12.0 * u, y + 30.0 * u, 10.0 * u, u);
            y += 70.0 * u;
        }
    }
    let lead = tr(
        "Sur votre Mac, votre PC ou votre téléphone : Ajouter un ordinateur ou une console › PlayStation › Envoyer à un appareil, puis saisissez ce code.",
        "On your Mac, PC or phone: Add a computer or a console › PlayStation › Send to a device, then enter this code.",
    );
    cv.paragraph(x0, y, Text::new(Font::Regular, 18.0 * u, MUTED), lead, w, 26.0 * u);
    if let Some(m) = &a.message {
        cv.text_fit(x0, y + 70.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), m, w);
    }
}

/// La légende à la manette de l'onglet PlayStation.
pub fn add_legend(app: &App, cv: &mut Canvas, x0: f32, gy: f32, g: f32, u: f32) {
    let AddStep::Ps(step) = &app.add.step else { return };
    let mut x = x0;
    let ok = match step {
        PsStep::Code { .. } | PsStep::ByHand { .. } | PsStep::Rename { .. } | PsStep::Send { .. } | PsStep::Paste { .. } => tr("Touche", "Key"),
        _ => tr("Choisir", "Choose"),
    };
    x += legend(cv, x, gy, g, &["A"], ok) + 28.0 * u;
    legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
}

// MARK: - Partie

/// Ce que la partie montre par-dessus l'image : contenu protégé, clavier de la console, aide du clavier et de la souris.
pub fn session_overlay(app: &App, cv: &mut Canvas, s: &SessionUi) {
    let Some(ps) = &s.console else { return };
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    if ps.protected {
        let msg = tr("La console ne peut pas diffuser ce qu'elle affiche.", "The console can't stream what it's showing.");
        cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 230));
        cv.text_fit(w / 2.0, h / 2.0 + 7.0 * u, Text::new(Font::Regular, 20.0 * u, FROST).align(Align::Center), msg, w - 64.0 * u);
    }
    // Le clavier de la console : le bandeau en bas de l'image, comme les aides de la souris.
    if ps.keyboard.is_some() {
        let msg = tr("Tapez votre texte · Entrée pour valider · Échap pour annuler", "Type your text · Return to accept · Escape to cancel");
        let text = ps.keyboard.clone().unwrap_or_default();
        hint_band(cv, u, msg, (!text.is_empty()).then_some(text.as_str()));
        return;
    }
    // Clavier et souris (bureau, sans manette) : le premier pas, puis un instant le raccourci qui rend la souris.
    if app.layout == Layout::Desktop && !ps.pads && !s.bar && !s.show_stats && s.phase == crate::app::Phase::Streaming {
        match ps.captured_at {
            None => hint_band(cv, u, tr("Cliquez dans l'image pour jouer au clavier et à la souris", "Click the picture to play with keyboard and mouse"), None),
            Some(t) if t.elapsed().as_secs_f32() < 4.0 => hint_band(cv, u, tr("Ctrl+Alt+Maj+M pour rendre la souris", "Ctrl+Alt+Shift+M to release the mouse"), None),
            _ => {}
        }
    }
}

/// Un bandeau en bas de l'image : une ligne d'aide, et le texte tapé au-dessus.
fn hint_band(cv: &mut Canvas, u: f32, msg: &str, typed: Option<&str>) {
    let (w, h) = (cv.w, cv.h);
    let style = Text::new(Font::Regular, 14.0 * u, MUTED);
    let ch = if typed.is_some() { 74.0 * u } else { 40.0 * u };
    let cw = (cv.text_width(style, msg) + 48.0 * u).max(if typed.is_some() { 520.0 * u } else { 0.0 }).min(w - 32.0 * u);
    let (x, y) = ((w - cw) / 2.0, h - 48.0 * u - ch);
    cv.rrect(x, y, cw, ch, 12.0 * u, Rgba(9, 13, 18, 230));
    cv.rrect_stroke(x, y, cw, ch, 12.0 * u, 1.0, LINE_2);
    let mut ty = y + ch / 2.0 + 5.0 * u;
    if let Some(t) = typed {
        cv.text_fit(x + 24.0 * u, y + 30.0 * u, Text::new(Font::Regular, 18.0 * u, FROST), t, cw - 48.0 * u);
        ty = y + ch - 18.0 * u;
    }
    cv.text_fit(w / 2.0, ty, style.align(Align::Center), msg, cw - 32.0 * u);
}

/// Les statistiques de la partie (docs/link-ui.md, « Barre de la partie ») : définition, images par seconde,
/// format, débit demandé ; reçu ; perdues et réparées depuis le début.
pub fn stats(app: &App, cv: &mut Canvas, s: &SessionUi) {
    let Some(ps) = &s.console else { return };
    let u = unit(app.layout);
    let fps_unit = tr("i/s", "fps");
    let line1 = format!("{} × {} · {} {fps_unit} · {} · {} Mbit/s {}", ps.width, ps.height, ps.max_fps, if ps.hevc { "H.265" } else { "H.264" }, ps.target_kbps / 1000, tr("demandés", "requested"));
    let line1 = if ps.width == 0 { format!("— · {} Mbit/s {}", ps.target_kbps / 1000, tr("demandés", "requested")) } else { line1 };
    let line2 = format!("{} {:.0} {fps_unit} · {} Mbit/s", tr("Reçu :", "Received:"), ps.fps, decimal(ps.mbps as f64, 1));
    let line3 = if english() { format!("Pictures lost: {} · repaired: {}", ps.lost, ps.recovered) } else { format!("Images perdues : {} · réparées : {}", ps.lost, ps.recovered) };
    let mut lines = vec![line1, line2, line3];
    if ps.relayed {
        lines.push(format!("{} {}", tr("Chemin :", "Path:"), tr("Par le relais", "Through the relay")));
    }
    let mono = Text::new(Font::Mono, 12.5 * u, FROST);
    let w = lines.iter().map(|l| cv.text_width(mono, l)).fold(0.0f32, f32::max) + 40.0 * u;
    let h = 46.0 * u + lines.len() as f32 * 22.0 * u;
    let x = cv.w - w - 20.0 * u;
    let y = if app.layout == Layout::Desktop { 64.0 } else { 20.0 * u };
    cv.rrect(x, y, w, h, 12.0 * u, Rgba(9, 13, 18, 225));
    cv.rrect_stroke(x, y, w, h, 12.0 * u, 1.0, LINE_2);
    eyebrow(cv, x + 20.0 * u, y + 26.0 * u, 11.0 * u, FAINT, tr("Statistiques", "Statistics"));
    for (i, l) in lines.iter().enumerate() {
        cv.text(x + 20.0 * u, y + 52.0 * u + i as f32 * 22.0 * u, if i == 0 { mono } else { mono.color(MUTED) }, l);
    }
}

/// La barre de la partie au bureau : le même dessin que celle d'une session d'ordinateur, avec ce qui a du sens pour
/// une console (le nom, « Par le relais », statistiques, son, plein écran, souris, réglages, veille, se déconnecter).
pub fn bar(_app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    let Some(ps) = &s.console else { return };
    let h = super::session::BAR_H;
    let name_style = Text::new(Font::Semibold, 14.0, FROST);
    let quit = tr("Se déconnecter", "Disconnect");
    let quit_w = cv.text_width(Text::new(Font::Semibold, 13.0, BAD_TEXT), quit) + 24.0;
    let mut icons: Vec<(Target, Icon, bool, String)> = vec![
        (Target::BarPsStats, Icon::Chart, s.show_stats, tr("Statistiques", "Statistics").into()),
        (Target::BarPsMute, Icon::Speaker, s.muted, if s.muted { tr("Rétablir le son", "Unmute") } else { tr("Couper le son", "Mute") }.into()),
    ];
    icons.push((Target::BarFullscreen, Icon::Fullscreen, s.fullscreen, if s.fullscreen { tr("Quitter le plein écran", "Exit full screen") } else { tr("Plein écran", "Full screen") }.into()));
    if !ps.pads {
        icons.push((Target::BarMouse, Icon::Mouse, s.relative, if s.relative { tr("Rendre la souris", "Release the mouse") } else { tr("Prendre la souris", "Capture the mouse") }.into()));
    }
    icons.push((Target::BarPsSettings, Icon::Gear, false, crate::i18n::settings_for(&s.name)));
    icons.push((Target::BarPsRest, Icon::Power, false, tr("Mettre la console en veille", "Put the console in rest mode").into()));
    let route = ps.relayed.then(|| tr("Par le relais", "Through the relay"));
    let route_w = route.map(|r| cv.text_width(Text::new(Font::Regular, 12.5, FROST), r) + 22.0).unwrap_or(0.0);
    let name_w = cv.text_width(name_style, &s.name).min(260.0);
    let total = 16.0 + 16.0 + 12.0 + name_w + 12.0 + if route.is_some() { route_w + 12.0 } else { 0.0 } + 1.0 + 12.0 + 34.0 * icons.len() as f32 + 12.0 + 1.0 + 12.0 + quit_w + 10.0;
    let r = Rect::new((cv.w - total) / 2.0, -1.0, total, h + 1.0);
    cv.rrect(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, Rgba(9, 13, 18, 235));
    cv.rrect_stroke(r.x, r.y - 12.0, r.w, r.h + 12.0, 12.0, 1.0, LINE_2);
    let cy = h / 2.0;
    let mut x = r.x + 16.0;
    cv.glow(x + 8.0, cy, 14.0, 14.0, HALO.alpha(0.5));
    cv.mark(x, cy - 8.0, 16.0, FROST, SPECTRE, Some(CORE));
    x += 16.0 + 12.0;
    cv.text_fit(x, cy + 5.0, name_style, &s.name, name_w);
    x += name_w + 12.0;
    if let Some(label) = route {
        let rr = Rect::new(x, cy - 12.0, route_w, 24.0);
        cv.rrect(rr.x, rr.y, rr.w, rr.h, 12.0, FLAT.alpha(0.22));
        cv.text(rr.x + rr.w / 2.0, cy + 4.5, Text::new(Font::Regular, 12.5, FROST).align(Align::Center), label);
        x += route_w + 12.0;
    }
    cv.rect(x.round(), cy - 11.0, 1.0, 22.0, LINE_2);
    x += 13.0;
    let mut tip: Option<(Rect, String)> = None;
    for (t, icon, on, name) in icons {
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
        cv.icon(&icon, x + 8.0, cy - 9.0, 18.0, 1.8, if on { FROST } else { MUTED });
        x += 34.0;
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
    if let Some((r, name)) = tip {
        super::session::tooltip(cv, r, h + 4.0, &name);
    }
}

/// La largeur de la barre de la partie (pour son ouverture au pointeur).
pub fn bar_width(cv: &mut Canvas, s: &SessionUi) -> f32 {
    let Some(ps) = &s.console else { return 0.0 };
    let name_w = cv.text_width(Text::new(Font::Semibold, 14.0, FROST), &s.name).min(260.0);
    let quit_w = cv.text_width(Text::new(Font::Semibold, 13.0, BAD_TEXT), tr("Se déconnecter", "Disconnect")) + 24.0;
    let route_w = if ps.relayed { cv.text_width(Text::new(Font::Regular, 12.5, FROST), tr("Par le relais", "Through the relay")) + 34.0 } else { 0.0 };
    let icons = if ps.pads { 5.0 } else { 6.0 };
    16.0 + 16.0 + 12.0 + name_w + 12.0 + route_w + 1.0 + 12.0 + 34.0 * icons + 12.0 + 1.0 + 12.0 + quit_w + 10.0
}

/// Le menu de la partie à la manette (Steam Deck, salon, KyberNow Link OS) : le même que celui d'une session
/// d'ordinateur (en-tête des mesures, tuiles, légende), avec ce qui a du sens pour une console.
pub fn menu(app: &App, cv: &mut Canvas, f: &mut Frame, s: &SessionUi) {
    use crate::app::MenuItem;
    let Some(ps) = &s.console else { return };
    let u = unit(app.layout);
    let tv = app.layout == Layout::Tv;
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, w, h, Rgba(5, 7, 10, 184));
    let x0 = if tv { 96.0 } else { 48.0 };
    let head = 88.0 * u - if tv { 4.0 } else { 0.0 };
    cv.rect(0.0, 0.0, w, head, Rgba(5, 7, 10, 128));
    cv.rect(0.0, head - 1.0, w, 1.0, LINE);
    cv.glow(x0 + 14.0 * u, head / 2.0, 22.0 * u, 22.0 * u, HALO.alpha(0.5));
    cv.mark(x0, head / 2.0 - 14.0 * u, 28.0 * u, FROST, SPECTRE, Some(CORE));
    let mut x = x0 + 28.0 * u + 40.0 * u;
    let cells: Vec<(String, &str, Rgba)> = vec![
        (format!("{:.0} {}", ps.fps, tr("i/s", "fps")), tr("Images", "Frames"), FROST),
        (format!("{} Mbit/s", decimal(ps.mbps as f64, 1)), tr("Débit", "Bitrate"), FROST),
        ((if ps.hevc { "H.265" } else { "H.264" }).into(), tr("Format", "Format"), FROST),
        (format!("{} · {}", ps.lost, ps.recovered), tr("Perdues · réparées", "Lost · repaired"), if ps.lost == 0 { OK } else { WARN }),
    ];
    for (v, k, c) in &cells {
        let vw = cv.text(x, head / 2.0 + 2.0 * u, Text::new(Font::MonoMedium, 20.0 * u, *c), v);
        let kw = eyebrow(cv, x, head / 2.0 + 22.0 * u, 11.0 * u, FAINT, k);
        x += vw.max(kw) + 40.0 * u;
    }
    let mode = if ps.width == 0 { "—".to_string() } else { format!("{} × {}", ps.width, ps.height) };
    cv.text(w - x0, head / 2.0 + 2.0 * u, Text::new(Font::MonoMedium, 20.0 * u, FROST).align(Align::Right), &mode);
    let gap = 14.0 * u;
    let tw = ((w - 2.0 * x0 - 4.0 * gap) / 5.0).min(if tv { 330.0 } else { 216.0 * 1.2 });
    let th = if tv { 170.0 } else { 118.0 };
    let mut tiles: Vec<(MenuItem, String, String, bool, bool)> = vec![
        (MenuItem::Resume, tr("Reprendre", "Resume").into(), tr("Retour au jeu", "Back to the game").into(), false, false),
        (MenuItem::Sound, tr("Son", "Sound").into(), if s.muted { tr("coupé", "muted") } else { tr("sur cet appareil", "on this device") }.into(), s.muted, false),
        (MenuItem::Stats, tr("Statistiques", "Statistics").into(), tr("Images, débit", "Frames, bitrate").into(), s.show_stats, false),
        (MenuItem::Gamepads, tr("Manettes", "Controllers").into(), format!("{} · {}", app.pads.len(), tr("vibrations", "rumble")), false, false),
        (MenuItem::PsSettings, tr("Réglages", "Settings").into(), crate::i18n::settings_for(&s.name), false, false),
        (MenuItem::PsRest, tr("Veille", "Rest mode").into(), tr("Mettre la console en veille", "Put the console in rest mode").into(), false, false),
    ];
    if ps.relayed {
        tiles.push((MenuItem::Route, tr("Par le relais", "Through the relay").into(), tr("Hors de chez vous", "Away from home").into(), false, false));
    }
    tiles.push((MenuItem::Disconnect, tr("Se déconnecter", "Disconnect").into(), tr("Fin de la partie", "Ends the game").into(), false, true));
    let ty = head + 36.0 * u;
    for (i, (item, title, sub, on, danger)) in tiles.iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        let r = Rect::new(x0 + col as f32 * (tw + gap), ty + row as f32 * (th + gap), tw, th);
        super::session::tile(cv, f, Target::Menu(*item), r, u, title, sub, i == 0, *on, *danger, false);
    }
    let footer = 56.0 * u;
    let sy = h - footer - 30.0 * u;
    widgets::dot(cv, x0 + 4.0 * u, sy - 4.0 * u, 3.5 * u, OK, false);
    let mut line = s.name.clone();
    if let Some(p) = app.pads.first() {
        line.push_str(&format!(" · {} {}", tr("manette :", "controller:"), p.name));
    }
    cv.text_fit(x0 + 16.0 * u, sy, Text::new(Font::Mono, 12.0 * u, FAINT), &line, w * 0.6);
    cv.text(w - x0, sy, Text::new(Font::Mono, 12.0 * u, FAINT).align(Align::Right), tr("Pendant ce menu, la console reçoit une manette au repos", "While this menu is open, the console gets an idle controller"));
    cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
    cv.rect(0.0, h - footer, w, 1.0, LINE);
    let gy = h - footer / 2.0;
    let g = 26.0 * u;
    let mut lx = x0;
    lx += legend(cv, lx, gy, g, &["A"], tr("Choisir", "Choose")) + 28.0 * u;
    legend(cv, lx, gy, g, &["B"], tr("Reprendre", "Resume"));
    let label = tr("ouvre ce menu", "opens this menu");
    let keys: &[&str] = &["Options", "Create", "L1", "R1"];
    let lw = widgets::legend_width(cv, g, keys, label);
    legend(cv, w - x0 - lw, gy, g, keys, label);
}
