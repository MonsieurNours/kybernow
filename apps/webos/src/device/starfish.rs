//! Le lecteur média de LG (`StarfishMediaAPIs`), sous le décodeur du
//! téléviseur : ses documents JSON de chargement et ses événements, vus par
//! le crochet `src/starfish_hook.c` (journal). Les horodatages passent tels
//! quels (`pts_scale` = 1) : la chaîne de jeu les prend en millisecondes.

use std::ffi::{c_char, CStr};

#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_seen(payload: *const c_char) {
    if payload.is_null() {
        return;
    }
    let s = unsafe { CStr::from_ptr(payload) }.to_string_lossy().into_owned();
    log::info!("lecteur : chargement {s}");
}

/// Un événement du lecteur (`SMP CB` : 0x16 chargé, 0x17 déchargé, 0x1a
/// lecture, erreurs…), journalisé avant d'aller au décodeur.
#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_event(kind: i32, num: i64, s: *const c_char) {
    let s = if s.is_null() { String::new() } else { unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned() };
    log::info!("lecteur : événement 0x{kind:02x} ({num:#x}) {s}");
}

/// `NULL` : le document passe tel quel.
#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_rewrite(_payload: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}

/// Facteur des horodatages donnés au lecteur par le décodeur de jeu (millisecondes, tels quels).
#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_pts_scale() -> i64 {
    1
}
