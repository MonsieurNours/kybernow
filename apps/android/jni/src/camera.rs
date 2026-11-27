//! La caméra de l'appareil vers l'ordinateur (docs/webcam.md, règles dans
//! docs/link-ui.md, « Caméra de l'appareil ») : Kotlin capture (Camera2) et
//! encode (MediaCodec, H.264), le cœur (`kybernow_core::camera`) fait le
//! reste. Mêmes fonctions que le pont Apple (`kn_camera_*`).

use jni::objects::{JByteBuffer, JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};

use kybernow_core::camera::{self, CameraCodec, CameraEvent, CameraResolution, CameraSettings, DeviceNotice, FrameOutcome};
use kybernow_core::i18n;

use crate::exports::{give, handle, string, Env};

/// Ce qu'est devenue une image (`cameraFrame`) : partie, lâchée (la suivante
/// doit être une image clé), caméra éteinte ou plus de flux.
const SENT: jint = 0;
const WANT_KEYFRAME: jint = 1;
const OFF: jint = 2;

/// Allume la caméra, ou change de caméra ou de définition sans l'éteindre.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraStart(
    mut env: Env,
    _: JClass,
    h: jlong,
    hevc: jboolean,
    width: jint,
    height: jint,
    resolution1080: jboolean,
    device: JString,
) -> jboolean {
    let Some(h) = handle(h) else { return JNI_FALSE };
    let settings = CameraSettings {
        codec: if hevc != 0 { CameraCodec::Hevc } else { CameraCodec::H264 },
        width: width.max(0) as u32,
        height: height.max(0) as u32,
        fps: camera::FPS,
        resolution: if resolution1080 != 0 { CameraResolution::P1080 } else { CameraResolution::P720 },
        device: string(&mut env, &device),
    };
    if h.session.camera().start(settings) { JNI_TRUE } else { JNI_FALSE }
}

/// Une image encodée (Annex B, SPS et PPS devant chaque image clé), lue dans
/// un `ByteBuffer` direct de `offset` à `offset + size` ; pts en µs ;
/// `quarterTurns` : quarts de tour (sens horaire) qui redressent l'image.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraFrame(
    env: Env,
    _: JClass,
    h: jlong,
    buffer: JByteBuffer,
    offset: jint,
    size: jint,
    pts_us: jlong,
    key: jboolean,
    quarter_turns: jint,
) -> jint {
    let Some(h) = handle(h) else { return OFF };
    let (Ok(ptr), Ok(cap)) = (env.get_direct_buffer_address(&buffer), env.get_direct_buffer_capacity(&buffer)) else {
        return OFF;
    };
    let (offset, size) = (offset.max(0) as usize, size.max(0) as usize);
    if offset + size > cap {
        return OFF;
    }
    let data = unsafe { std::slice::from_raw_parts(ptr.add(offset), size) };
    match h.session.camera().frame_rotated(data, pts_us.max(0) as u64, key != 0, (quarter_turns & 3) as u8) {
        FrameOutcome::Sent => SENT,
        FrameOutcome::WantKeyframe => WANT_KEYFRAME,
        FrameOutcome::Off => OFF,
    }
}

/// L'app passe en arrière-plan : l'ordinateur garde la dernière image.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraPause(_: Env, _: JClass, h: jlong) {
    if let Some(h) = handle(h) {
        h.session.camera().pause();
    }
}

/// « Éteindre la caméra ».
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraStop(_: Env, _: JClass, h: jlong) {
    if let Some(h) = handle(h) {
        h.session.camera().stop();
    }
}

/// Le débit à donner à l'encodeur, en bit/s.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraTargetBitrate(_: Env, _: JClass, h: jlong) -> jint {
    handle(h).map_or(0, |h| h.session.camera().target_bitrate().min(i32::MAX as u32) as jint)
}

/// Les réponses de l'ordinateur depuis le dernier appel, en JSON (liste de
/// `{event, offered, notice}`, comme `kn_camera_poll`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraPoll(mut env: Env, _: JClass, h: jlong, computer: JString) -> jstring {
    let name = string(&mut env, &computer);
    let lang = i18n::current();
    let events = handle(h).map(|h| h.session.camera().poll()).unwrap_or_default();
    let list: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            let (event, offered) = match e {
                CameraEvent::Started => ("started", None),
                CameraEvent::Busy { .. } => ("busy", None),
                CameraEvent::Failed { .. } => ("failed", None),
                CameraEvent::Stopped => ("stopped", None),
                CameraEvent::KeyframeWanted => ("keyframe", None),
                CameraEvent::Offer(o) => ("offer", Some(*o)),
            };
            serde_json::json!({ "event": event, "offered": offered, "notice": camera::event_notice(e, &name, lang) })
        })
        .collect();
    give(&mut env, &serde_json::Value::Array(list).to_string())
}

/// La ligne « Caméra » des statistiques ; nulle caméra éteinte.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraStatsLine(mut env: Env, _: JClass, h: jlong) -> jstring {
    let stats = handle(h).map(|h| h.session.camera().stats()).unwrap_or_default();
    match camera::stats_line(&stats, i18n::current()) {
        Some(line) => give(&mut env, &line),
        None => std::ptr::null_mut(),
    }
}

/// L'avis d'un refus de l'appareil : 0 accès refusé, 1 caméra prise par une
/// autre app, 2 caméra débranchée (`camera` : son nom), 3 pas d'encodeur.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_cameraDeviceNotice(mut env: Env, _: JClass, which: jint, camera_name: JString) -> jstring {
    let which = match which {
        0 => DeviceNotice::Denied,
        1 => DeviceNotice::InUse,
        2 => DeviceNotice::Unplugged,
        _ => DeviceNotice::NoEncoder,
    };
    let name = string(&mut env, &camera_name);
    give(&mut env, &camera::device_notice(which, &name, i18n::current()))
}
