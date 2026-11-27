//! Micro de ce PC vers l'ordinateur, côté app (docs/micro.md, docs/link-ui.md
//! « Micro ») : la puce « Micro » de la barre (et Ctrl+Alt+Maj+V), le choix
//! retenu pour chaque ordinateur (`Settings::microphone`, coupé par défaut),
//! l'avis bref « Micro envoyé à {nom}. », le refus de Windows et les Réglages
//! de l'app (« Micro », « Réduire l'écho et le bruit »).
//!
//! Un invité du partage ou l'ami qui emprunte : coupé au départ, choix gardé
//! le temps de la session (ce n'est pas un ordinateur de l'app).
//!
//! L'état de la puce, ses textes (libellé, infobulle) et l'avis sont ceux du
//! cœur (`micro::MicUi`, WINAPP-56, le même pour toutes les apps) ; l'app
//! garde le choix par ordinateur, le refus de Windows et le raccourci.

use crate::sync::Locked;
use kybernow_core::micro::{MicState, MicTap};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{UiEvents, UiState, emit_computers, share, update_prefs};
use crate::i18n::{self, t};

#[derive(Clone, Serialize)]
struct BarMicEvent {
    id: String,
    mic: Option<serde_json::Value>,
}

fn core_text(key: &str, params: &[(&str, &str)]) -> String {
    kybernow_core::i18n::tr(i18n::lang(), key, params)
}

impl UiEvents {
    /// La puce de cette session, telle que le cœur la décrit (`MicUi::ui_json`,
    /// WINAPP-56 : `on`, `state`, `lit`, `reason`, `label`, `tooltip`, `tile`,
    /// les textes de link-ui) ; absente sans micro sur ce PC, et pour un
    /// invité que l'hôte ne laisse pas parler (`MicUi::shown`).
    pub(crate) fn bar_mic(&self) -> Option<serde_json::Value> {
        let m = self.mic.locked();
        m.shown(self.guest.is_some()).then(|| m.ui_json(i18n::lang(), &t("micro.shortcut", &[])))
    }

    /// Le nom de l'ordinateur de l'avis « Micro envoyé à {nom}. ».
    fn mic_computer(&self) -> String {
        match (&self.guest, &self.borrow) {
            (Some(g), _) => g.computer.clone(),
            (None, Some(b)) => b.title.clone(),
            _ => self.connexion.name.clone(),
        }
    }

    fn emit_mic(&self) {
        let _ = self.app.emit_to("bar", "bar-mic", BarMicEvent { id: self.id.clone(), mic: self.bar_mic() });
        // Barre fermée, l'icône du micro près du trait du bord haut (`window::apply_signs`).
        crate::window::set_signs(self.connexion.session, None, Some(self.mic.locked().sent()));
    }

    /// `HostEvents::microphone` : la barre suit ; un allumage pris est annoncé
    /// (« Micro envoyé à {nom}. », une fois : `MicUi::changed`).
    pub(crate) fn mic_changed(&self, state: MicState) {
        let name = self.mic_computer();
        let tell = self.mic.locked().changed(state, &name);
        self.emit_mic();
        if let Some(text) = tell {
            share::notice(&self.app, &self.id, &self.connexion.name, &text);
        }
    }

    /// `HostEvents::microphone_denied` : coupé, retenu coupé, avis (la raison
    /// propre à Windows, `micro.permission_windows`).
    pub(crate) fn mic_denied(&self) {
        self.mic.locked().denied();
        remember(&self.app, &self.id, false);
        self.emit_mic();
        share::notice(&self.app, &self.id, &self.connexion.name, &core_text("micro.permission_windows", &[]));
    }

    /// `HostEvents::toggle_microphone` (Ctrl+Alt+Maj+V) : `MicUi::tap` ;
    /// refusé par l'ordinateur, la raison en avis, rien ne change.
    pub(crate) fn mic_toggle(&self) {
        if !kybernow_core::micro::device_has_microphone() {
            return;
        }
        match self.mic.locked().tap() {
            MicTap::Refused(r) => share::notice(&self.app, &self.id, &self.connexion.name, &r.text()),
            MicTap::Set(on) => {
                let (app, id) = (self.app.clone(), self.id.clone());
                let _ = self.app.run_on_main_thread(move || {
                    let _ = set(&app, &id, on);
                });
            }
        }
    }
}

/// Retenu pour l'ordinateur (rien pour un invité ou un ami, absents de la liste).
fn remember(app: &AppHandle, id: &str, on: bool) {
    let state = app.state::<UiState>();
    let changed = state.with_store(|s| match s.get_mut(id) {
        Some(c) if c.settings.microphone != on => {
            c.settings.microphone = on;
            s.save();
            true
        }
        _ => false,
    });
    if changed {
        emit_computers(app);
    }
}

/// « Micro » allumé ou coupé (barre, Ctrl+Alt+Maj+V) : retenu pour
/// l'ordinateur et donné à sa session ; rien ne se relance.
pub(crate) fn set(app: &AppHandle, id: &str, on: bool) -> Result<(), String> {
    let state = app.state::<UiState>();
    if !state.in_session(id) {
        return Err(t("errors.no_session", &[]));
    }
    remember(app, id, on);
    apply(app, id, on)
}

/// Le choix donné à la session en cours de cet ordinateur (réglage « Micro »
/// de la rubrique Son, déjà enregistré) ; la barre suit.
pub(crate) fn apply(app: &AppHandle, id: &str, on: bool) -> Result<(), String> {
    let state = app.state::<UiState>();
    let applied = state.with_session(id, |r| {
        r.events.mic.locked().set(on);
        let applied = r.host.set_microphone(on);
        r.events.emit_mic();
        applied
    });
    match applied {
        Some(true) => {
            log::info!("micro {} pour la session", if on { "allumé" } else { "coupé" });
            Ok(())
        }
        Some(false) => Err(t("errors.session_not_ready", &[])),
        None => Err(t("errors.no_session", &[])),
    }
}

#[tauri::command(async)]
pub(crate) fn session_microphone(app: AppHandle, id: String, on: bool) -> Result<(), String> {
    set(&app, &id, on)
}

/// Réglages de l'app : les micros de ce PC, celui choisi, « Réduire l'écho
/// et le bruit ».
#[derive(Serialize)]
pub(crate) struct MicSettings {
    devices: Vec<crate::micro::MicDevice>,
    /// Le micro choisi (`None` : celui du système) ; débranché, il reste choisi.
    device: Option<String>,
    voice: bool,
}

#[tauri::command(async)]
pub(crate) fn mic_settings(app: AppHandle) -> MicSettings {
    let state = app.state::<UiState>();
    let p = state.prefs.locked().clone();
    MicSettings { devices: crate::micro::devices(), device: p.microphone.clone(), voice: p.voice_processing.unwrap_or(true) }
}

/// « Micro » des Réglages de l'app : `None`, celui du système.
#[tauri::command(async)]
pub(crate) fn set_mic_device(app: AppHandle, device: Option<String>) {
    let state = app.state::<UiState>();
    let prefs = update_prefs(&state, |p| {
        p.microphone = device.filter(|d| !d.is_empty());
        p.clone()
    });
    crate::micro::set_prefs(prefs.microphone.clone(), prefs.voice_processing.unwrap_or(true));
}

/// « Réduire l'écho et le bruit » des Réglages de l'app.
#[tauri::command(async)]
pub(crate) fn set_mic_voice(app: AppHandle, on: bool) {
    let state = app.state::<UiState>();
    let prefs = update_prefs(&state, |p| {
        p.voice_processing = Some(on);
        p.clone()
    });
    crate::micro::set_prefs(prefs.microphone.clone(), on);
}
