//! Enregistrement de la session pour l'app Android (docs/enregistrement.md,
//! textes dans docs/link-ui.md, « Enregistrement de la session ») : les mêmes
//! fonctions que le pont C (`apps/apple/src/recording.rs`), pour `Native`.
//! L'hôte lance et arrête (`consoleRecording`) ; l'état arrive par
//! `SessionListener.onRecording` (session.rs). Le temps (« 02:14 ») s'écrit
//! dans l'app, chaque seconde (`RecordingState.clock` de Kotlin, comme
//! `recording::clock` du cœur).
//!
//! Réponse comme les autres appels : `{"code": 0, "value": …}` (l'état, comme
//! `onRecording`) ou `{"code": …, "error": "…"}`, `error` étant l'avis bref
//! de link-ui : 90 coupé dans Admin, 91 PyroWave ou AV1, 92 moins de 2 Go
//! libres, 93 dossier inaccessible, 94 ordinateur prêté, 95 autre refus (ou
//! Station qui n'enregistre pas) ; 1 l'appel n'a pas abouti (le texte du cœur).

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jlong, jstring, JNI_FALSE};

use kybernow_core::i18n;
use kybernow_core::recording::RecordingError;

use crate::exports::{console, give, string, Env};

fn code(e: &RecordingError) -> i32 {
    match e {
        RecordingError::Disabled => 90,
        RecordingError::Codec => 91,
        RecordingError::DiskFull => 92,
        RecordingError::Folder => 93,
        RecordingError::Lent(_) => 94,
        RecordingError::Failed => 95,
        RecordingError::Console(_) => 1,
    }
}

/// Lance (`record`) ou arrête l'enregistrement de la session de l'appareil
/// `device_id`. Bloquant (15 s au plus) : jamais sur le fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleRecording(mut env: Env, _: JClass, h: jlong, device_id: JString, record: jboolean) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let Some(c) = console(h) else { return std::ptr::null_mut() };
        let device_id = string(&mut env, &device_id);
        let lang = i18n::current();
        let s = match c.recording(&device_id, record != JNI_FALSE) {
            Ok(state) => serde_json::json!({ "code": 0, "value": state.ui_json(lang, None) }).to_string(),
            Err(e) => serde_json::json!({ "code": code(&e), "error": e.text(lang) }).to_string(),
        };
        give(&mut env, &s)
    })
}
