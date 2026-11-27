//! Micro de ce PC vers l'ordinateur, côté app (docs/micro.md, docs/link-ui.md
//! « Micro ») : la puce « Micro » de la barre (et Ctrl+Alt+Maj+V), le choix
//! retenu pour chaque ordinateur (`Settings::microphone`, coupé par défaut),
//! l'avis bref « Micro envoyé à {nom}. », le refus de Windows et les Réglages
//! de l'app (« Micro », « Réduire l'écho et le bruit »).
//!
//! Un invité du partage ou l'ami qui emprunte : coupé au départ, choix gardé
//! le temps de la session (ce n'est pas un ordinateur de l'app).

use kybernow_core::micro::{MicRefusal, MicState};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{UiEvents, UiState, emit_computers, share, update_prefs};
use crate::i18n::{self, t};

/// Le micro d'une session : le choix de l'utilisateur, l'état dit par le cœur,
/// et un allumage à annoncer quand l'ordinateur le prend.
pub(crate) struct MicSlot {
    on: bool,
    state: MicState,
    asked: bool,
}

impl MicSlot {
    pub(crate) fn new(on: bool) -> Self {
        Self { on, state: MicState::Off, asked: false }
    }
}

impl Default for MicSlot {
    fn default() -> Self {
        Self::new(false)
    }
}

/// La puce « Micro » de la barre ; absente sans micro sur ce PC, et pour un
/// invité que l'hôte ne laisse pas parler.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct BarMic {
    /// Le choix : allumé ou coupé.
    on: bool,
    /// `off`, `waiting`, `on`, `refused`.
    state: &'static str,
    /// La raison du refus (infobulle de la puce estompée).
    reason: Option<String>,
}

#[derive(Clone, Serialize)]
struct BarMicEvent {
    id: String,
    mic: Option<BarMic>,
}

fn state_code(s: MicState) -> &'static str {
    match s {
        MicState::Off => "off",
        MicState::Waiting => "waiting",
        MicState::On => "on",
        MicState::Refused(_) => "refused",
    }
}

fn core_text(key: &str, params: &[(&str, &str)]) -> String {
    kybernow_core::i18n::tr(i18n::lang(), key, params)
}

impl UiEvents {
    /// La puce de cette session.
    pub(crate) fn bar_mic(&self) -> Option<BarMic> {
        if !kybernow_core::micro::device_has_microphone() {
            return None;
        }
        let m = self.mic.lock().unwrap();
        // Invité : « Micro » seulement quand l'hôte laisse les invités parler.
        if self.guest.is_some() && m.state == MicState::Refused(MicRefusal::NotAllowed) {
            return None;
        }
        let reason = match m.state {
            MicState::Refused(r) => Some(r.text()),
            _ => None,
        };
        Some(BarMic { on: m.on, state: state_code(m.state), reason })
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
    }

    /// `HostEvents::microphone` : la barre suit ; un allumage pris est annoncé.
    pub(crate) fn mic_changed(&self, state: MicState) {
        let tell = {
            let mut m = self.mic.lock().unwrap();
            m.state = state;
            let tell = m.asked && state == MicState::On;
            if tell || matches!(state, MicState::Refused(_)) {
                m.asked = false;
            }
            tell
        };
        self.emit_mic();
        if tell {
            let name = self.mic_computer();
            share::notice(&self.app, &self.id, &self.connexion.name, &core_text("micro.sent", &[("name", &name)]));
        }
    }

    /// `HostEvents::microphone_denied` : coupé, retenu coupé, avis.
    pub(crate) fn mic_denied(&self) {
        {
            let mut m = self.mic.lock().unwrap();
            m.on = false;
            m.asked = false;
        }
        remember(&self.app, &self.id, false);
        self.emit_mic();
        share::notice(&self.app, &self.id, &self.connexion.name, &core_text("micro.permission_windows", &[]));
    }

    /// `HostEvents::toggle_microphone` (Ctrl+Alt+Maj+V) ; refusé par
    /// l'ordinateur : la raison en avis, rien ne change.
    pub(crate) fn mic_toggle(&self) {
        let (on, refused) = {
            let m = self.mic.lock().unwrap();
            (m.on, match m.state {
                MicState::Refused(r) => Some(r),
                _ => None,
            })
        };
        if !kybernow_core::micro::device_has_microphone() {
            return;
        }
        if let Some(r) = refused {
            share::notice(&self.app, &self.id, &self.connexion.name, &r.text());
            return;
        }
        let (app, id) = (self.app.clone(), self.id.clone());
        let _ = self.app.run_on_main_thread(move || {
            let _ = set(&app, &id, !on);
        });
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
        {
            let mut m = r.events.mic.lock().unwrap();
            m.on = on;
            m.asked = on;
        }
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
    let p = state.prefs.lock().unwrap().clone();
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
