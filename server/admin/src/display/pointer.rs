//! Pointeur de souris pendant les sessions.
//!
//! Windows masque le pointeur pendant la frappe (« Masquer le pointeur
//! pendant la frappe », activé par défaut). À distance, chaque masquage fait
//! disparaître le curseur côté appareil, qui bascule alors en souris
//! relative : le curseur clignote ou se perd. On coupe donc ce réglage le
//! temps de la session (valeur active seulement, sans toucher au profil),
//! et on rétablit la valeur d'origine à la fin.
//!
//! Appelé par l'agent, dans la session de l'écran. Idempotent : l'écran
//! virtuel et la sortie audio peuvent l'appeler tous les deux.

use std::path::Path;

use windows_sys::Win32::UI::WindowsAndMessaging::{
    SPI_GETMOUSEVANISH, SPI_SETMOUSEVANISH, SPIF_SENDCHANGE, SystemParametersInfoW,
};

const FILE_NAME: &str = "pointer-session.json";

fn mouse_vanish() -> Option<bool> {
    let mut value = 0i32;
    let ok = unsafe { SystemParametersInfoW(SPI_GETMOUSEVANISH, 0, &mut value as *mut i32 as *mut _, 0) };
    (ok != 0).then_some(value != 0)
}

fn set_mouse_vanish(on: bool) {
    // La valeur est passée dans pvParam ; SPIF_SENDCHANGE sans
    // SPIF_UPDATEINIFILE : effet immédiat, profil de l'utilisateur intact.
    unsafe { SystemParametersInfoW(SPI_SETMOUSEVANISH, 0, on as usize as *mut _, SPIF_SENDCHANGE) };
}

/// Début de session : mémorise le réglage dans `dir` et le coupe.
pub fn begin(dir: &Path) {
    let file = dir.join(FILE_NAME);
    if file.exists() {
        return;
    }
    let Some(previous) = mouse_vanish() else { return };
    let _ = std::fs::write(&file, serde_json::json!({ "mouse_vanish": previous }).to_string());
    if previous {
        set_mouse_vanish(false);
    }
}

/// Fin de session : rétablit le réglage mémorisé par `begin`.
pub fn end(dir: &Path) {
    let file = dir.join(FILE_NAME);
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let previous = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("mouse_vanish").and_then(serde_json::Value::as_bool));
    if previous == Some(true) {
        set_mouse_vanish(true);
    }
    let _ = std::fs::remove_file(file);
}
