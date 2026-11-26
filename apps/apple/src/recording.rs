//! Enregistrement de la session pour les apps Apple (docs/enregistrement.md,
//! textes dans docs/link-ui.md, « Enregistrement de la session ») : l'hôte le
//! lance et l'arrête (`kn_console_recording`) ; l'état arrive par le rappel
//! `on_recording` de la session ; `kn_recording_clock` écrit le temps.

use std::ffi::c_char;

use kybernow_core::i18n;
use kybernow_core::recording::RecordingError;

use crate::{KN_ERROR, KN_OK, KnConsole, give, give_to, text};

/// Refus de `kn_console_recording` (`error` : l'avis bref de link-ui) :
/// enregistrement coupé dans Admin, PyroWave ou AV1, moins de 2 Go libres,
/// dossier inaccessible, ordinateur prêté, autre refus ou Station qui
/// n'enregistre pas. Un échec de l'appel (réseau, compte) rend `KN_ERROR`
/// avec le texte du cœur.
pub const KN_RECORDING_DISABLED: i32 = 90;
pub const KN_RECORDING_CODEC: i32 = 91;
pub const KN_RECORDING_DISK_FULL: i32 = 92;
pub const KN_RECORDING_FOLDER: i32 = 93;
pub const KN_RECORDING_LENT: i32 = 94;
pub const KN_RECORDING_FAILED: i32 = 95;

fn code(e: &RecordingError) -> i32 {
    match e {
        RecordingError::Disabled => KN_RECORDING_DISABLED,
        RecordingError::Codec => KN_RECORDING_CODEC,
        RecordingError::DiskFull => KN_RECORDING_DISK_FULL,
        RecordingError::Folder => KN_RECORDING_FOLDER,
        RecordingError::Lent(_) => KN_RECORDING_LENT,
        RecordingError::Failed => KN_RECORDING_FAILED,
        RecordingError::Console(_) => KN_ERROR,
    }
}

/// Lance (`record`) ou arrête l'enregistrement de la session de l'appareil
/// `device_id`. `out` : l'état, comme `on_recording`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_recording(
    c: *const KnConsole,
    device_id: *const c_char,
    record: bool,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match c.0.recording(&text(device_id), record) {
        Ok(state) => {
            give_to(out, state.ui_json(i18n::current(), None).to_string());
            KN_OK
        }
        Err(e) => {
            give_to(error, e.text(i18n::current()));
            code(&e)
        }
    }
}

/// « 02:14 », « 1:02:14 » : le `{time}` des textes de l'enregistrement.
#[unsafe(no_mangle)]
pub extern "C" fn kn_recording_clock(seconds: u64) -> *mut c_char {
    give(kybernow_core::recording::clock(seconds))
}
