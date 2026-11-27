//! Enregistrement de la session (docs/enregistrement.md, textes dans
//! docs/link-ui.md, « Enregistrement de la session ») : la Station écrit le
//! fichier, l'app le demande et le montre.
//!
//! - L'état vient du signe de vie (`recording`) : celui de l'hôte
//!   (`POST /api/stream/heartbeat`) et celui de l'invité du partage
//!   (`POST /api/share/beat`). Une Station qui n'enregistre pas ne le donne
//!   pas : ni bouton ni pastille ([`RecordingState::available`] faux).
//! - La session le passe à la plateforme quand il change
//!   ([`crate::session::Platform::on_recording`]), avec l'avis bref à montrer.
//! - L'hôte lance ou arrête par [`crate::console::Console::recording`] ; un
//!   refus a son texte ([`RecordingError::text`]).
//!
//! Le temps est celui de la Station (`elapsed`, en secondes au moment du
//! signe de vie), compté ensuite par l'appareil : tous les appareils montrent
//! le même, quelle que soit leur horloge.

use std::time::Instant;

use serde::{Deserialize, Deserializer};

use crate::i18n::{self, Lang, tr};

/// Pourquoi un enregistrement s'est fini (`last_end.reason` d'Admin).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Arrêté (l'hôte, Admin, l'enregistrement coupé dans Admin).
    Stopped,
    /// La session de l'hôte s'est finie.
    Ended,
    /// Disque plein (moins de 2 Go, ou écriture impossible).
    DiskFull,
    /// L'image est passée en PyroWave ou AV1.
    Codec,
    /// Un prêt commence.
    Lent,
}

impl EndReason {
    fn parse(s: &str) -> Self {
        match s {
            "disk_full" | "write" => Self::DiskFull,
            "codec" => Self::Codec,
            "lent" => Self::Lent,
            "ended" => Self::Ended,
            _ => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawEnd {
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Raw {
    #[serde(default)]
    allowed: bool,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    elapsed: Option<u64>,
    #[serde(default)]
    started_at: Option<u64>,
    #[serde(default)]
    last_end: Option<RawEnd>,
}

/// L'enregistrement, tel que le dit le dernier signe de vie.
#[derive(Debug, Clone)]
pub struct RecordingState {
    /// La Station enregistre (elle le dit) : sinon, ni bouton ni pastille.
    pub available: bool,
    /// Permis dans Admin (sinon le bouton est estompé, avec la raison).
    pub allowed: bool,
    pub active: bool,
    /// L'heure de début sur la Station (ms Unix) : l'identité d'un
    /// enregistrement (un nouveau en a une autre).
    pub started_at: Option<u64>,
    /// Secondes écoulées au moment du signe de vie, et quand il est arrivé.
    elapsed: u64,
    received: Instant,
    /// La fin du dernier enregistrement.
    pub last_end: Option<EndReason>,
}

impl PartialEq for RecordingState {
    /// Le même état pour l'app (le temps qui passe n'en change pas).
    fn eq(&self, other: &Self) -> bool {
        self.available == other.available
            && self.allowed == other.allowed
            && self.active == other.active
            && self.started_at == other.started_at
            && self.last_end == other.last_end
    }
}

impl Eq for RecordingState {}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            available: false,
            allowed: false,
            active: false,
            started_at: None,
            elapsed: 0,
            received: Instant::now(),
            last_end: None,
        }
    }
}

impl RecordingState {
    fn from_raw(raw: Raw) -> Self {
        Self {
            available: true,
            allowed: raw.allowed,
            active: raw.active,
            started_at: raw.started_at.filter(|_| raw.active),
            elapsed: raw.elapsed.unwrap_or(0),
            received: Instant::now(),
            last_end: raw.last_end.map(|e| EndReason::parse(&e.reason)),
        }
    }

    /// Secondes écoulées depuis le début, maintenant.
    pub fn elapsed_secs(&self) -> u64 {
        if !self.active {
            return 0;
        }
        self.elapsed + self.received.elapsed().as_secs()
    }

    /// « 02:14 », « 1:02:14 ».
    pub fn clock(&self) -> String {
        clock(self.elapsed_secs())
    }
}

impl RecordingState {
    /// Pour les ponts des apps (C, JNI, Tauri, client web) : l'état et les
    /// textes de link-ui dans `lang`, `{time}` (à remplir chaque seconde par
    /// [`clock`] de `elapsed` plus le temps passé depuis) et `{shortcut}`
    /// gardés. `computer` : le nom de l'ordinateur, pour un invité.
    ///
    /// `{"available", "allowed", "active", "elapsed", "texts": {"record",
    /// "pill", "record_tooltip", "stop_tooltip", "stop", "on_computer",
    /// "record_menu", "stop_menu", "guest_pill", "guest_tooltip",
    /// "disabled"}}`
    pub fn ui_json(&self, lang: Lang, computer: Option<&str>) -> serde_json::Value {
        let t = |key: &str| tr(lang, &format!("recording.{key}"), &[("time", "{time}"), ("shortcut", "{shortcut}")]);
        serde_json::json!({
            "available": self.available,
            "allowed": self.allowed,
            "active": self.active,
            "elapsed": self.elapsed_secs(),
            "texts": {
                "record": t("record"),
                "pill": t("pill"),
                "record_tooltip": t("record_tooltip"),
                "stop_tooltip": t("stop_tooltip"),
                "stop": t("stop"),
                "on_computer": t("on_computer"),
                "record_menu": t("record_menu"),
                "stop_menu": t("stop_menu"),
                "guest_pill": t("guest_pill"),
                "guest_tooltip": tr(lang, "recording.guest_notice", &[("name", computer.unwrap_or_default())]),
                "disabled": t("disabled"),
            }
        })
    }
}

/// « 02:14 », « 59:59 », « 1:02:14 ».
pub fn clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

/// `recording` d'un signe de vie : absent (Station qui n'enregistre pas) ou
/// illisible → `None`.
pub(crate) fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<Option<RecordingState>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.filter(|v| v.is_object()).and_then(|v| serde_json::from_value::<Raw>(v).ok()).map(RecordingState::from_raw))
}

/// L'état d'une réponse d'Admin (`POST /api/stream/recording`).
pub(crate) fn from_value(v: serde_json::Value) -> Option<RecordingState> {
    v.is_object().then(|| serde_json::from_value::<Raw>(v).ok()).flatten().map(RecordingState::from_raw)
}

/// Qui regarde : l'hôte (il lance et arrête) ou un invité du partage (il voit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer<'a> {
    Host,
    /// Avec le nom de l'ordinateur.
    Guest(&'a str),
}

/// L'avis bref quand l'état change (`before` : l'état d'avant, `None` au
/// premier signe de vie). L'hôte : lancé, terminé, arrêté de lui-même ; un
/// invité : à l'arrivée dans une session enregistrée, au lancement et à l'arrêt.
pub fn notice(before: Option<&RecordingState>, after: &RecordingState, viewer: Viewer<'_>, lang: Lang) -> Option<String> {
    let was = before.is_some_and(|b| b.active);
    let same = before.is_some_and(|b| b.active && b.started_at == after.started_at);
    match (viewer, was, after.active) {
        (Viewer::Guest(name), _, true) if !same => Some(tr(lang, "recording.guest_notice", &[("name", name)])),
        (Viewer::Guest(_), true, false) => Some(tr(lang, "recording.guest_stopped", &[])),
        (Viewer::Host, false, true) if before.is_some() => Some(tr(lang, "recording.started", &[])),
        (Viewer::Host, true, false) => match after.last_end {
            Some(EndReason::DiskFull) => Some(tr(lang, "recording.stopped_disk", &[])),
            Some(EndReason::Codec) => Some(tr(lang, "recording.stopped_codec", &[])),
            Some(EndReason::Lent | EndReason::Ended) => None,
            _ => Some(tr(lang, "recording.saved", &[])),
        },
        _ => None,
    }
}

/// Un refus d'Admin à `POST /api/stream/recording` (son `code`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingError {
    Disabled,
    Codec,
    DiskFull,
    Folder,
    /// Pendant un prêt : avec le prénom de l'ami.
    Lent(String),
    /// Une Station qui n'enregistre pas (404), ou un autre refus.
    Failed,
    /// L'appel n'a pas abouti (réseau, compte) : le texte du cœur.
    Console(String),
}

impl RecordingError {
    /// L'avis bref de link-ui.
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Self::Disabled => tr(lang, "recording.disabled", &[]),
            Self::Codec => tr(lang, "recording.codec", &[]),
            Self::DiskFull => tr(lang, "recording.disk_full", &[]),
            Self::Folder => tr(lang, "recording.folder", &[]),
            Self::Lent(name) => tr(lang, "recording.lent", &[("name", name)]),
            Self::Failed => tr(lang, "recording.failed", &[]),
            Self::Console(text) => text.clone(),
        }
    }

    pub(crate) fn from_code(code: &str, message: &str) -> Self {
        match code {
            "disabled" => Self::Disabled,
            "codec" => Self::Codec,
            "disk_full" => Self::DiskFull,
            "folder" => Self::Folder,
            "lent" => Self::Lent(lent_name(message)),
            _ => Self::Failed,
        }
    }
}

/// Le prénom dans « {prénom} utilise l'ordinateur. » d'Admin (son texte,
/// dans la langue de l'app) ; vide s'il ne se lit pas.
fn lent_name(message: &str) -> String {
    for marker in [" utilise l'ordinateur", " is using the computer"] {
        if let Some(i) = message.find(marker) {
            return message[..i].trim().to_string();
        }
    }
    String::new()
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current()))
    }
}

impl std::error::Error for RecordingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(json: &str) -> RecordingState {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "lenient")]
            recording: Option<RecordingState>,
        }
        serde_json::from_str::<W>(json).unwrap().recording.unwrap()
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(134), "02:14");
        assert_eq!(clock(3599), "59:59");
        assert_eq!(clock(3734), "1:02:14");
    }

    #[test]
    fn absent_or_odd_is_none() {
        #[derive(Deserialize)]
        struct W {
            #[serde(default, deserialize_with = "lenient")]
            recording: Option<RecordingState>,
        }
        assert!(serde_json::from_str::<W>("{}").unwrap().recording.is_none());
        assert!(serde_json::from_str::<W>(r#"{"recording": 3}"#).unwrap().recording.is_none());
        assert!(serde_json::from_str::<W>(r#"{"recording": null}"#).unwrap().recording.is_none());
    }

    #[test]
    fn reads_the_heartbeat() {
        let s = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 134, "started_at": 1764166325000, "last_end": null}}"#);
        assert!(s.available && s.allowed && s.active);
        assert!(s.elapsed_secs() >= 134);
        // Les textes viennent de `ui_json`, la seule source des apps (COEUR-43).
        let texts = |lang| s.ui_json(lang, Some("Atelier"))["texts"].clone();
        assert_eq!(texts(Lang::Fr)["pill"], "Enregistrement · {time}");
        assert_eq!(texts(Lang::En)["pill"], "Recording · {time}");
        assert_eq!(texts(Lang::Fr)["stop_menu"], "Arrêter l'enregistrement ({time})");
        assert_eq!(texts(Lang::Fr)["stop"], "Arrêter l'enregistrement");
        assert_eq!(texts(Lang::Fr)["record"], "Enregistrer");
        assert_eq!(texts(Lang::Fr)["record_tooltip"], "Enregistrer la session sur l'ordinateur ({shortcut})");
        assert_eq!(texts(Lang::En)["disabled"], "Recording is turned off on this computer (Admin › Recordings).");
        assert_eq!(texts(Lang::Fr)["guest_tooltip"], "Atelier enregistre la session.");
        assert_eq!(s.clock(), "02:14");
        let idle = state(r#"{"recording": {"allowed": true, "active": false, "last_end": {"reason": "disk_full", "at": 1}}}"#);
        assert_eq!(idle.last_end, Some(EndReason::DiskFull));
    }

    #[test]
    fn same_state_while_time_passes() {
        let a = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 10, "started_at": 5}}"#);
        let b = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 12, "started_at": 5}}"#);
        assert_eq!(a, b);
        let c = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 0, "started_at": 9}}"#);
        assert_ne!(a, c, "another recording");
    }

    #[test]
    fn notices() {
        let idle = state(r#"{"recording": {"allowed": true, "active": false}}"#);
        let on = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 0, "started_at": 5}}"#);
        let saved = state(r#"{"recording": {"allowed": true, "active": false, "last_end": {"reason": "stopped"}}}"#);
        let full = state(r#"{"recording": {"allowed": true, "active": false, "last_end": {"reason": "write"}}}"#);
        let ended = state(r#"{"recording": {"allowed": true, "active": false, "last_end": {"reason": "ended"}}}"#);
        let fr = Lang::Fr;
        assert_eq!(notice(Some(&idle), &on, Viewer::Host, fr).as_deref(), Some("Enregistrement lancé sur l'ordinateur."));
        assert_eq!(notice(None, &on, Viewer::Host, fr), None, "already running when the session opens");
        assert_eq!(notice(Some(&on), &on, Viewer::Host, fr), None);
        assert_eq!(notice(Some(&on), &saved, Viewer::Host, fr).as_deref(), Some("Enregistrement terminé : retrouvez-le dans Admin › Enregistrements."));
        assert_eq!(notice(Some(&on), &full, Viewer::Host, fr).as_deref(), Some("L'enregistrement s'est arrêté : le disque de l'ordinateur est plein."));
        assert_eq!(notice(Some(&on), &ended, Viewer::Host, fr), None);
        let guest = Viewer::Guest("Atelier");
        assert_eq!(notice(None, &on, guest, fr).as_deref(), Some("Atelier enregistre la session."), "joining a recorded session");
        assert_eq!(notice(Some(&on), &on, guest, fr), None);
        assert_eq!(notice(Some(&on), &saved, guest, Lang::En).as_deref(), Some("Recording stopped."));
        assert_eq!(notice(None, &idle, guest, fr), None);
    }

    #[test]
    fn json_for_the_apps() {
        let s = state(r#"{"recording": {"allowed": true, "active": true, "elapsed": 61, "started_at": 3}}"#);
        let v = s.ui_json(Lang::Fr, Some("Atelier"));
        assert_eq!(v["active"], true);
        assert!(v["elapsed"].as_u64().unwrap() >= 61);
        assert_eq!(v["texts"]["pill"], "Enregistrement · {time}");
        assert_eq!(v["texts"]["record_tooltip"], "Enregistrer la session sur l'ordinateur ({shortcut})");
        assert_eq!(v["texts"]["guest_tooltip"], "Atelier enregistre la session.");
        assert_eq!(s.ui_json(Lang::En, None)["texts"]["stop_menu"], "Stop recording ({time})");
    }

    #[test]
    fn errors() {
        assert_eq!(RecordingError::from_code("codec", "").text(Lang::Fr), "PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264.");
        assert_eq!(RecordingError::from_code("lent", "Léa utilise l'ordinateur.").text(Lang::En), "Léa is using the computer.");
        assert_eq!(RecordingError::from_code("lent", "Léa is using the computer.").text(Lang::Fr), "Léa utilise l'ordinateur.");
        assert_eq!(RecordingError::from_code("???", "").text(Lang::En), "Recording could not start.");
    }
}
