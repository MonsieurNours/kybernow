//! Mises à jour de l'app : la recherche et la vérification du cœur
//! (`kybernow_core::update`, manifeste signé minisign, même clé que la
//! Station), appelées par `fr.leslignesclaires.kybernow.update.Updates`.
//!
//! La version trouvée reste ici : le téléchargement prend celle-là, jamais
//! une URL ou une empreinte venues de Kotlin.

use std::path::Path;
use std::sync::Mutex;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jstring};
use jni::JNIEnv;

use kybernow_core::update::{self, Available, Channel};

/// Nom de l'APK dans les versions publiées.
pub const APK: &str = "KyberNow-Link-Android.apk";

static FOUND: Mutex<Option<Available>> = Mutex::new(None);

fn give(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// `{"available": {version, channel, notes, page, size}}`, `{"none": true}`
/// ou `{"error": "…"}`. Réseau : à appeler hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_updateCheck(mut env: JNIEnv, _: JClass, current: JString, beta: jboolean) -> jstring {
    let current: String = env.get_string(&current).map(Into::into).unwrap_or_default();
    let channel = if beta != 0 { Channel::Beta } else { Channel::Stable };
    let json = match update::check(&current, channel, &[APK]) {
        Ok(Some(a)) => {
            let v = serde_json::json!({ "available": {
                "version": a.version, "channel": a.channel.as_str(), "notes": a.notes, "page": a.page, "size": a.size,
            }});
            *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(a);
            v
        }
        Ok(None) => {
            *FOUND.lock().unwrap_or_else(|e| e.into_inner()) = None;
            serde_json::json!({ "none": true })
        }
        Err(e) => serde_json::json!({ "error": e }),
    };
    give(&mut env, &json.to_string())
}

/// Télécharge la version trouvée par `updateCheck` dans `dest` et vérifie sa
/// taille et son empreinte ; `progress.onProgress(long)` reçoit les octets
/// reçus. Rend `null`, ou le message d'erreur.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_updateDownload(mut env: JNIEnv, _: JClass, dest: JString, progress: JObject) -> jstring {
    let dest: String = env.get_string(&dest).map(Into::into).unwrap_or_default();
    let Some(found) = FOUND.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return give(&mut env, "aucune mise à jour à télécharger");
    };
    let env_cell = std::cell::RefCell::new(&mut env);
    let last = std::cell::Cell::new(0u64);
    let result = update::download(&found, Path::new(&dest), |n| {
        // Un appel par 256 Kio au plus : assez pour une barre de progression.
        if n - last.get() < (256 << 10) && n != found.size {
            return;
        }
        last.set(n);
        if progress.is_null() {
            return;
        }
        let mut env = env_cell.borrow_mut();
        let _ = env.call_method(&progress, "onProgress", "(J)V", &[JValue::Long(n as i64)]);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
    });
    let env = env_cell.into_inner();
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => give(env, &e),
    }
}
