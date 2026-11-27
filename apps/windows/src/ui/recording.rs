//! Enregistrement de la session côté Windows (docs/enregistrement.md, textes
//! de docs/link-ui.md « Enregistrement de la session ») : la Station écrit le
//! fichier, l'app ne fait que le demander et le montrer.
//!
//! - L'état arrive du cœur (`HostEvents::recording`, signe de vie de la
//!   session) : gardé dans la session (`UiEvents::recording`), dit à la barre
//!   (`recording-state`, l'état et les textes du cœur, `ui_json`), et l'avis
//!   bref du cœur montré tel quel, comme ceux du partage.
//! - « Enregistrer », la pastille, Ctrl+Alt+Maj+R et le menu de l'icône
//!   passent par [`session_record`] : `Console::recording` (hors du fil de
//!   l'interface), qui rend l'état ou le texte du refus.
//! - Une Station qui n'enregistre pas ne dit rien : ni bouton, ni pastille, ni
//!   ligne dans le menu de l'icône. L'ami qui emprunte ne voit rien ; un
//!   invité voit la pastille, sans action.

use crate::sync::Locked;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use kybernow_core::console::Console;
use kybernow_core::recording::{RecordingError, RecordingState};

use super::{UiEvents, UiState, console_of, share};
use crate::i18n::{lang, t};
use crate::{system, tray};

/// Le dernier état de l'enregistrement d'une session, et pour un invité le
/// nom de l'ordinateur (« {nom} enregistre la session. »).
#[derive(Debug, Clone)]
pub(super) struct Seen {
    state: RecordingState,
    computer: Option<String>,
}

impl Seen {
    /// L'état et les textes du cœur pour la page ; rien quand la Station
    /// n'enregistre pas.
    fn view(&self) -> Option<serde_json::Value> {
        self.state.available.then(|| self.state.ui_json(lang(), self.computer.as_deref()))
    }
}

/// `recording-state` : l'enregistrement de la session `id` (nul : rien à montrer).
#[derive(Clone, Serialize)]
struct RecordingEvent {
    id: String,
    recording: Option<serde_json::Value>,
}

/// Garde l'état de la session et le dit à la barre et au menu de l'icône.
fn publish(app: &AppHandle, events: &UiEvents, seen: Seen) {
    let view = seen.view();
    *events.recording.locked() = Some(seen);
    let _ = app.emit_to("bar", "recording-state", RecordingEvent { id: events.id.clone(), recording: view });
    tray::refresh();
}

impl UiEvents {
    /// L'état change (lancé, arrêté, coupé dans Admin) : la barre et le menu
    /// de l'icône suivent ; l'avis bref du cœur, dans la capsule de la barre
    /// (ou une notification de Windows quand l'app n'est pas devant).
    pub(super) fn on_recording(&self, state: &RecordingState, notice: Option<&str>, computer: Option<&str>) {
        // L'ami qui emprunte ne voit rien (pas d'enregistrement pendant un prêt).
        if self.borrow.is_some() {
            return;
        }
        log::info!(
            "[{}] enregistrement : {}{}",
            self.connexion.name,
            if state.active { "en cours" } else { "arrêté" },
            if state.allowed { "" } else { " (coupé dans Admin)" }
        );
        publish(&self.app, self, Seen { state: state.clone(), computer: computer.map(str::to_owned) });
        if let Some(text) = notice {
            let title = self.guest.as_ref().map_or_else(|| self.connexion.name.clone(), |g| g.title.clone());
            share::notice(&self.app, &self.id, &title, text);
        }
    }
}

/// Pour `bar-shown` : l'enregistrement de la session `id`, à l'instant (le
/// temps écoulé compris) ; rien pour une Station qui n'enregistre pas.
pub(super) fn bar_view(app: &AppHandle, id: &str) -> Option<serde_json::Value> {
    let seen = app.state::<UiState>().with_session(id, |r| r.events.recording.locked().clone()).flatten()?;
    seen.view()
}

/// La ligne du menu de l'icône (« Enregistrer la session », « Arrêter
/// l'enregistrement (02:14) ») et si elle se choisit (coupé dans Admin :
/// inactive, sauf pour arrêter un enregistrement en cours).
pub(super) fn menu_item(events: &UiEvents) -> Option<(String, bool)> {
    let seen = events.recording.locked();
    let s = &seen.as_ref()?.state;
    s.available.then(|| (s.menu_item(lang()), s.allowed || s.active))
}

/// Admin de l'ordinateur d'une session d'hôte : jamais pour un invité ni pour
/// l'ami qui emprunte.
fn host_console(app: &AppHandle, id: &str) -> Result<Console, String> {
    let is_host = app.state::<UiState>().with_session(id, |r| r.guest.is_none() && r.borrow.is_none()).unwrap_or(false);
    if !is_host {
        return Err(t("errors.no_session", &[]));
    }
    console_of(app, id)
}

/// « Enregistrer » (`record` vrai) ou arrêter, pour la session d'hôte `id` :
/// rend l'état et les textes du cœur (comme `recording-state`), ou l'avis
/// bref du refus (« PyroWave et AV1 ne s'enregistrent pas… »). Appel à Admin
/// (15 s au plus) : hors du fil de l'interface.
/// Le temps d'un enregistrement tel que la barre le montre, chaque seconde :
/// « 02:14 », « 1:02:14 » (`recording::clock` du cœur, la règle de toutes
/// les apps ; l'interface n'en garde pas de copie).
#[tauri::command(async)]
pub fn recording_clock(seconds: u64) -> String {
    kybernow_core::recording::clock(seconds)
}

#[tauri::command(async)]
pub fn session_record(app: AppHandle, id: String, record: bool) -> Result<Option<serde_json::Value>, String> {
    let console = host_console(&app, &id)?;
    let device = system::device_info().id;
    match console.recording(&device, record) {
        Ok(state) => {
            log::info!("[{id}] enregistrement {}", if record { "demandé" } else { "arrêté" });
            let seen = Seen { state, computer: None };
            let view = seen.view();
            let state = app.state::<UiState>();
            let events = state.with_session(&id, |r| r.events.clone());
            if let Some(events) = events {
                publish(&app, &events, seen);
            }
            Ok(view)
        }
        Err(e) => {
            let text = e.text(lang());
            log::warn!("[{id}] enregistrement refusé : {e:?}");
            Err(text)
        }
    }
}

/// Ctrl+Alt+Maj+R et le menu de l'icône : lance ou arrête l'enregistrement de
/// la session d'hôte `id`, selon son état. Le refus (ou la raison quand c'est
/// coupé dans Admin) s'affiche en avis bref. Bloquant : sur un fil à part.
pub(super) fn toggle(app: &AppHandle, id: &str) {
    let state = app.state::<UiState>();
    let Some((seen, name, host)) =
        state.with_session(id, |r| (r.events.recording.locked().clone(), r.name.clone(), r.guest.is_none() && r.borrow.is_none()))
    else {
        return;
    };
    // Station qui n'enregistre pas, invité, ami qui emprunte : rien.
    let Some(seen) = seen.filter(|s| host && s.state.available) else { return };
    let s = &seen.state;
    if !s.allowed && !s.active {
        share::notice(app, id, &name, &RecordingError::Disabled.text(lang()));
        return;
    }
    if let Err(text) = session_record(app.clone(), id.to_string(), !s.active) {
        share::notice(app, id, &name, &text);
    }
}

/// [`toggle`] sur son propre fil (le raccourci arrive sur celui de la fenêtre).
pub(super) fn toggle_in_background(app: &AppHandle, id: &str) {
    let (app, id) = (app.clone(), id.to_string());
    std::thread::spawn(move || toggle(&app, &id));
}
