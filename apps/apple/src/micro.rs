//! Le micro tel que l'interface le montre (`micro::MicUi` du cœur, docs/micro.md
//! et link-ui « Micro ») : puce, tuile (« Envoyé », « Envoi… », « Coupé »),
//! infobulle, avis « Micro envoyé à {nom}. », raison d'un refus. Une poignée
//! par session (`KnMicUi`), tenue par l'app sur le fil principal.

use std::ffi::c_char;
use std::sync::Mutex;

use kybernow_core::i18n;
use kybernow_core::micro::{MicRefusal, MicState, MicTap, MicUi};

use crate::{give, text};

/// Le micro d'une session vu par l'interface.
pub struct KnMicUi(Mutex<MicUi>);

/// Ce que fait un appui (`kn_mic_ui_tap`) : le couper, l'allumer, ou montrer
/// la raison du refus en avis (`reason` de `kn_mic_ui_json`).
pub const KN_MIC_TAP_OFF: i32 = 0;
pub const KN_MIC_TAP_ON: i32 = 1;
pub const KN_MIC_TAP_REFUSED: i32 = 2;

fn with<R: Default>(m: *const KnMicUi, f: impl FnOnce(&mut MicUi) -> R) -> R {
    let Some(m) = (unsafe { m.as_ref() }) else { return R::default() };
    f(&mut m.0.lock().unwrap_or_else(|e| e.into_inner()))
}

/// L'état donné par `on_microphone` (`MicState::to_json` du cœur).
fn state(json: &str) -> MicState {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    match v["state"].as_str() {
        Some("waiting") => MicState::Waiting,
        Some("on") => MicState::On,
        Some("refused") => MicState::Refused(match v["reason"].as_str() {
            Some("not_allowed") => MicRefusal::NotAllowed,
            Some("no_device") => MicRefusal::NoDevice,
            Some("old_station") => MicRefusal::OldStation,
            _ => MicRefusal::Unsupported,
        }),
        _ => MicState::Off,
    }
}

/// Une poignée pour une session ; `on` : le choix gardé pour cet ordinateur.
#[unsafe(no_mangle)]
pub extern "C" fn kn_mic_ui_new(on: bool) -> *mut KnMicUi {
    kybernow_core::ffi_guard!(null => { Box::into_raw(Box::new(KnMicUi(Mutex::new(MicUi::new(on))))) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_free(m: *mut KnMicUi) {
    kybernow_core::ffi_guard!({
        if !m.is_null() {
            drop(unsafe { Box::from_raw(m) });
        }
    })
}

/// L'utilisateur l'allume ou le coupe (à donner aussi à la session).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_set(m: *const KnMicUi, on: bool) {
    kybernow_core::ffi_guard!({ with(m, |m| m.set(on)) })
}

/// Le système refuse l'accès au micro : coupé (l'app dit sa raison).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_denied(m: *const KnMicUi) {
    kybernow_core::ffi_guard!({ with(m, |m| m.denied()) })
}

/// Une réponse de l'ordinateur (`state_json` de `on_microphone`) ; rend
/// l'avis à montrer (« Micro envoyé à {nom}. », une fois, quand il est pris
/// après avoir été allumé), nul sinon.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_changed(m: *const KnMicUi, state_json: *const c_char, computer: *const c_char) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let s = state(&text(state_json));
        let name = text(computer);
        with(m, |m| m.changed(s, &name)).map_or(std::ptr::null_mut(), give)
    })
}

/// Un appui sur la puce ou la tuile : `KN_MIC_TAP_*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_tap(m: *const KnMicUi) -> i32 {
    kybernow_core::ffi_guard!(KN_MIC_TAP_OFF => {
        with(m, |m| match m.tap() {
            MicTap::Refused(_) => KN_MIC_TAP_REFUSED,
            MicTap::Set(true) => KN_MIC_TAP_ON,
            MicTap::Set(false) => KN_MIC_TAP_OFF,
        })
    })
}

/// Ce que montre l'interface, dans la langue de l'app : `{on, state, lit,
/// sent, reason, label, tooltip, tile, shown, capturing}` (`state` : `off`,
/// `waiting`, `on`, `refused` ; `tile` : « Envoyé », « Envoi… », « Coupé » ;
/// `shown` : la puce a sa place, `guest` vrai pour un invité ; `capturing` :
/// l'app capte). `shortcut` : celui de l'app (« ⌃⌥⌘V »), vide sans.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_mic_ui_json(m: *const KnMicUi, shortcut: *const c_char, guest: bool) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let shortcut = text(shortcut);
        let Some(m) = (unsafe { m.as_ref() }) else { return std::ptr::null_mut() };
        let ui = *m.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut v = ui.ui_json(i18n::current(), &shortcut);
        if let Some(map) = v.as_object_mut() {
            map.insert("shown".into(), ui.shown(guest).into());
            map.insert("capturing".into(), (ui.on && ui.state.capturing()).into());
        }
        give(v.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    fn json(m: *const KnMicUi) -> serde_json::Value {
        let p = unsafe { kn_mic_ui_json(m, c"⌃⌥⌘V".as_ptr(), false) };
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { crate::kn_string_free(p) };
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn tuile_envoi_puis_envoye() {
        i18n::set_current(i18n::Lang::Fr);
        kybernow_core::micro::set_device_has_microphone(true);
        let m = kn_mic_ui_new(false);
        assert_eq!(json(m)["tile"], "Coupé");
        assert_eq!(unsafe { kn_mic_ui_tap(m) }, KN_MIC_TAP_ON);
        unsafe { kn_mic_ui_set(m, true) };
        let waiting = CString::new(r#"{"state":"waiting"}"#).unwrap();
        assert!(unsafe { kn_mic_ui_changed(m, waiting.as_ptr(), c"Atelier".as_ptr()) }.is_null());
        let v = json(m);
        assert_eq!((v["tile"].as_str(), v["lit"].as_bool(), v["capturing"].as_bool()), (Some("Envoi…"), Some(true), Some(true)));
        let on = CString::new(r#"{"state":"on"}"#).unwrap();
        let notice = unsafe { kn_mic_ui_changed(m, on.as_ptr(), c"Atelier".as_ptr()) };
        assert!(!notice.is_null());
        let t = unsafe { CStr::from_ptr(notice) }.to_string_lossy().into_owned();
        unsafe { crate::kn_string_free(notice) };
        assert!(t.contains("Atelier"), "{t}");
        assert_eq!(json(m)["tile"], "Envoyé");
        unsafe { kn_mic_ui_free(m) };
    }

    #[test]
    fn refus_de_l_ordinateur() {
        i18n::set_current(i18n::Lang::Fr);
        let m = kn_mic_ui_new(true);
        let refused = CString::new(r#"{"state":"refused","reason":"no_device","text":"…"}"#).unwrap();
        assert!(unsafe { kn_mic_ui_changed(m, refused.as_ptr(), c"Atelier".as_ptr()) }.is_null());
        assert_eq!(unsafe { kn_mic_ui_tap(m) }, KN_MIC_TAP_REFUSED);
        let v = json(m);
        assert_eq!(v["state"], "refused");
        assert!(v["reason"].as_str().unwrap().contains("micro virtuel"));
        assert_eq!(v["lit"], false);
        unsafe { kn_mic_ui_free(m) };
    }
}
