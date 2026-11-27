//! « Stylet : Pression / Souris » côté app (docs/link-ui.md, « Stylet » ;
//! docs/stylet.md) : la ligne « Stylet » du menu « Souris » de la barre,
//! Ctrl+Alt+Maj+P et son avis bref, le choix retenu pour chaque ordinateur
//! (`Settings::pen`, « Pression » par défaut) et envoyé à ses réglages
//! synchronisés (`pen`).
//!
//! La règle de la ligne (un stylet a servi dans la session ou ce PC en a un,
//! et l'ordinateur reçoit le stylet), ses textes, ceux de la rubrique
//! Commandes et l'avis du raccourci sont ceux du cœur (`input::PenChoice`,
//! `input::PenMode`, catalogue `pen.*`) ; l'app garde le choix par ordinateur
//! et le donne à la session (`SessionHost::set_pen_mode`).
//!
//! Un invité du partage ou l'ami qui emprunte : « Pression » au départ, choix
//! gardé le temps de la session (ce n'est pas un ordinateur de l'app).

use kybernow_core::input::{PenChoice, PenMode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{UiEvents, UiState, emit_computers, share, synced};
use crate::i18n::{self, t};

/// `bar-pen` : la ligne « Stylet » de la session `id` a changé (choix,
/// première apparition) ; `None` : pas de ligne.
#[derive(Clone, Serialize)]
struct BarPenEvent {
    id: String,
    pen: Option<serde_json::Value>,
}

/// La ligne « Stylet » du menu « Souris » de la barre pour la session `id` :
/// `PenChoice::ui_json` du cœur (`shown`, `mode`, `title`, `label`,
/// `choices`, `help`), seulement quand elle se montre.
pub(crate) fn bar_view(app: &AppHandle, id: &str) -> Option<serde_json::Value> {
    let choice = app.state::<UiState>().with_session(id, |r| r.host.pen_choice()).flatten()?;
    choice.shown().then(|| choice.ui_json(i18n::lang()))
}

fn emit(app: &AppHandle, id: &str) {
    let _ = app.emit_to("bar", "bar-pen", BarPenEvent { id: id.to_string(), pen: bar_view(app, id) });
}

impl UiEvents {
    /// `HostEvents::toggle_pen` (Ctrl+Alt+Maj+P, fil des fenêtres) : sur le
    /// fil principal, comme le micro.
    pub(crate) fn pen_toggle(&self) {
        let (app, id, name) = (self.app.clone(), self.id.clone(), self.connexion.name.clone());
        let _ = self.app.run_on_main_thread(move || toggle(&app, &id, &name));
    }
}

/// Ctrl+Alt+Maj+P : l'autre choix, retenu et donné à la session, avec l'avis
/// bref du cœur (« Stylet : souris » / « Stylet : pression »).
fn toggle(app: &AppHandle, id: &str, name: &str) {
    let state = app.state::<UiState>();
    let Some(mode) = state.with_session(id, |r| r.host.pen_mode()).flatten() else { return };
    let next = mode.toggled();
    if set(app, id, next).is_ok() {
        share::notice(app, id, name, &next.notice(i18n::lang()));
    }
}

/// Retenu pour l'ordinateur et envoyé à ses réglages synchronisés (rien pour
/// un invité ou un ami, absents de la liste).
fn remember(app: &AppHandle, id: &str, mode: PenMode) {
    let state = app.state::<UiState>();
    let before = state.with_store(|s| match s.get_mut(id) {
        Some(c) if c.settings.pen_mode() != mode => {
            let before = c.settings.clone();
            c.settings.pen = mode.key().to_string();
            s.save();
            Some(before)
        }
        _ => None,
    });
    if let Some(before) = before {
        synced::changed(app, id, &before);
        emit_computers(app);
    }
}

/// « Stylet » choisi dans la barre ou par Ctrl+Alt+Maj+P : retenu pour
/// l'ordinateur et donné à sa session ; rien ne se relance.
pub(crate) fn set(app: &AppHandle, id: &str, mode: PenMode) -> Result<(), String> {
    let state = app.state::<UiState>();
    if !state.in_session(id) {
        return Err(t("errors.no_session", &[]));
    }
    remember(app, id, mode);
    apply(app, id, mode)
}

/// Le choix donné à la session en cours de cet ordinateur (rubrique
/// Commandes, déjà enregistré) ; la barre suit.
pub(crate) fn apply(app: &AppHandle, id: &str, mode: PenMode) -> Result<(), String> {
    let applied = app.state::<UiState>().with_session(id, |r| r.host.set_pen_mode(mode));
    emit(app, id);
    match applied {
        Some(true) => {
            log::info!("stylet : {} pour la session", mode.key());
            Ok(())
        }
        Some(false) => Err(t("errors.session_not_ready", &[])),
        None => Err(t("errors.no_session", &[])),
    }
}

/// « Pression » / « Souris » de la ligne « Stylet » de la barre.
#[tauri::command(async)]
pub(crate) fn session_pen(app: AppHandle, id: String, mode: String) -> Result<(), String> {
    set(&app, &id, PenMode::parse(&mode).unwrap_or_default())
}

/// La ligne « Stylet » à l'ouverture du menu « Souris » (un stylet a pu
/// servir depuis que la barre s'est montrée) ; `None` : pas de ligne.
#[tauri::command(async)]
pub(crate) fn session_pen_state(app: AppHandle, id: String) -> Option<serde_json::Value> {
    bar_view(&app, &id)
}

/// Réglages de l'ordinateur, rubrique Commandes : les textes de « Stylet »
/// (titre, choix, aide) du cœur, dans la langue de l'app ; le réglage y est
/// toujours montré.
#[tauri::command]
pub(crate) fn pen_texts() -> serde_json::Value {
    PenChoice::default().ui_json(i18n::lang())
}
