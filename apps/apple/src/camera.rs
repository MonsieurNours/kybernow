//! La caméra de l'appareil vers l'ordinateur pour les apps Apple
//! (docs/webcam.md, règles dans docs/link-ui.md, « Caméra de l'appareil ») :
//! Swift capture (AVFoundation) et encode (VideoToolbox, H.264), le cœur
//! (`kybernow_core::camera`) fait le reste. `KnPlatform::sends_camera` vrai :
//! le cœur demande la caméra à l'ordinateur à chaque départ de l'image ;
//! `on_camera_offer` dit quand l'entrée « Caméra » apparaît ou disparaît.

use std::ffi::c_char;

use kybernow_core::camera::{self, CameraCodec, CameraEvent, CameraResolution, CameraSettings, DeviceNotice, FrameOutcome};
use kybernow_core::i18n;

use crate::{KnSession, give, text, with};

/// Ce qu'est devenue une image (`kn_camera_frame`).
pub const KN_CAMERA_SENT: i32 = 0;
/// Lâchée : la prochaine image doit être une image clé.
pub const KN_CAMERA_WANT_KEYFRAME: i32 = 1;
/// Caméra éteinte, ou plus de flux.
pub const KN_CAMERA_OFF: i32 = 2;

/// Les avis de l'appareil (`kn_camera_device_notice`).
pub const KN_CAMERA_DENIED: i32 = 0;
pub const KN_CAMERA_IN_USE: i32 = 1;
pub const KN_CAMERA_UNPLUGGED: i32 = 2;
pub const KN_CAMERA_NO_ENCODER: i32 = 3;

/// L'ordinateur propose la caméra pour l'image en cours.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_offered(s: *const KnSession) -> bool {
    with(s, |s| s.camera().offered()).unwrap_or(false)
}

/// La caméra est allumée (allumage en cours compris).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_is_on(s: *const KnSession) -> bool {
    with(s, |s| s.camera().is_on()).unwrap_or(false)
}

/// Allume la caméra, ou change de caméra ou de définition sans l'éteindre.
/// `hevc` : HEVC (sinon H.264) ; `width`×`height` : la taille encodée ;
/// `resolution_1080` : le réglage « Définition de la caméra » ; `device` :
/// le nom de l'appareil. Faux si l'ordinateur ne la propose pas.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_start(
    s: *const KnSession,
    hevc: bool,
    width: u32,
    height: u32,
    resolution_1080: bool,
    device: *const c_char,
) -> bool {
    let settings = CameraSettings {
        codec: if hevc { CameraCodec::Hevc } else { CameraCodec::H264 },
        width,
        height,
        fps: camera::FPS,
        resolution: if resolution_1080 { CameraResolution::P1080 } else { CameraResolution::P720 },
        device: text(device),
    };
    with(s, |s| s.camera().start(settings)).unwrap_or(false)
}

/// Une image encodée (Annex B, paramètres avec chaque image clé), pts en
/// microsecondes : `KN_CAMERA_SENT`, `KN_CAMERA_WANT_KEYFRAME`, `KN_CAMERA_OFF`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_frame(s: *const KnSession, data: *const u8, len: usize, pts_us: u64, key: bool) -> i32 {
    if data.is_null() {
        return KN_CAMERA_OFF;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    match with(s, |s| s.camera().frame(bytes, pts_us, key)) {
        Some(FrameOutcome::Sent) => KN_CAMERA_SENT,
        Some(FrameOutcome::WantKeyframe) => KN_CAMERA_WANT_KEYFRAME,
        _ => KN_CAMERA_OFF,
    }
}

/// L'app passe en arrière-plan : l'ordinateur garde la dernière image.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_pause(s: *const KnSession) {
    with(s, |s| s.camera().pause());
}

/// « Éteindre la caméra ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_stop(s: *const KnSession) {
    with(s, |s| s.camera().stop());
}

/// Le débit à donner à l'encodeur, en bit/s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_target_bitrate(s: *const KnSession) -> u32 {
    with(s, |s| s.camera().target_bitrate()).unwrap_or(0)
}

/// Ce que l'ordinateur a répondu depuis le dernier appel, en JSON : une
/// liste de `{"event": "started"|"busy"|"failed"|"stopped"|"keyframe"|"offer",
/// "offered": bool, "notice": texte de l'avis (busy, failed)}` ; `computer`
/// est le nom de l'ordinateur des avis. À libérer (`kn_string_free`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_poll(s: *const KnSession, computer: *const c_char) -> *mut c_char {
    let name = text(computer);
    let lang = i18n::current();
    let events = with(s, |s| s.camera().poll()).unwrap_or_default();
    let list: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            let notice = camera::event_notice(e, &name, lang);
            let (event, offered) = match e {
                CameraEvent::Started => ("started", None),
                CameraEvent::Busy { .. } => ("busy", None),
                CameraEvent::Failed { .. } => ("failed", None),
                CameraEvent::Stopped => ("stopped", None),
                CameraEvent::KeyframeWanted => ("keyframe", None),
                CameraEvent::Offer(o) => ("offer", Some(*o)),
            };
            serde_json::json!({ "event": event, "offered": offered, "notice": notice })
        })
        .collect();
    give(serde_json::Value::Array(list).to_string())
}

/// La ligne « Caméra » des statistiques (« 1280 × 720 · 30 i/s · H.264 ·
/// 1,4 Mbit/s », « En pause ») ; nul caméra éteinte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_stats_line(s: *const KnSession) -> *mut c_char {
    let stats = with(s, |s| s.camera().stats()).unwrap_or_default();
    match camera::stats_line(&stats, i18n::current()) {
        Some(line) => give(line),
        None => std::ptr::null_mut(),
    }
}

/// L'avis d'un refus de l'appareil (`KN_CAMERA_DENIED`, `_IN_USE`,
/// `_UNPLUGGED` avec le nom de la caméra, `_NO_ENCODER`). À libérer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_camera_device_notice(which: i32, camera_name: *const c_char) -> *mut c_char {
    let which = match which {
        KN_CAMERA_DENIED => DeviceNotice::Denied,
        KN_CAMERA_IN_USE => DeviceNotice::InUse,
        KN_CAMERA_UNPLUGGED => DeviceNotice::Unplugged,
        _ => DeviceNotice::NoEncoder,
    };
    give(camera::device_notice(which, &text(camera_name), i18n::current()))
}
