//! Le lecteur média de LG (`StarfishMediaAPIs`), sous le décodeur du
//! téléviseur : son document JSON de chargement, vu et réécrit par le crochet
//! `src/starfish_hook.c`. Réécriture : la section audio PCM 6 voies, que
//! l'API publique sait demander, devient de l'E-AC-3 (`"AC3 PLUS"` et
//! `ac3PlusInfo`, comme les lecteurs qui passent le Dolby à l'ampli) ; le
//! reste du document (vidéo, mode jeu, faible latence) est gardé tel quel.

use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

/// Dernier document de chargement vu.
static LAST: Mutex<Option<String>> = Mutex::new(None);
/// Réécrire le prochain chargement en E-AC-3 (0 : non, sinon le nombre de voies).
static REWRITE_EAC3: AtomicU8 = AtomicU8::new(0);
/// La dernière réécriture a été faite (document reconnu).
static REWRITTEN: AtomicBool = AtomicBool::new(false);
/// Nom du codec Dolby dans le document : `AC3 PLUS` (celui des lecteurs
/// connus) d'abord, `EAC3` (celui des chaînes internes de LG) en second.
pub const DOLBY_NAMES: [&str; 2] = ["AC3 PLUS", "EAC3"];
static DOLBY_NAME: AtomicU8 = AtomicU8::new(0);

/// Choisit le nom du codec Dolby des prochains chargements (indice dans `DOLBY_NAMES`).
pub fn set_dolby_name(i: u8) {
    DOLBY_NAME.store(i.min(DOLBY_NAMES.len() as u8 - 1), Ordering::Release);
}

pub fn dolby_name() -> u8 {
    DOLBY_NAME.load(Ordering::Acquire)
}

/// Les chargements qui suivent demandent du Dolby (E-AC-3) à `channels` voies, ou rien.
pub fn set_rewrite_eac3(channels: u8) {
    REWRITE_EAC3.store(channels, Ordering::Release);
    REWRITTEN.store(false, Ordering::Release);
}

/// Le dernier chargement a bien été réécrit en E-AC-3.
pub fn rewritten() -> bool {
    REWRITTEN.load(Ordering::Acquire)
}

/// Le document réécrit, ou `None` s'il n'a pas la forme attendue (la
/// section `pcmInfo` et `codec.audio` dans `contents`).
pub fn rewrite_eac3(payload: &str, channels: u8) -> Option<String> {
    rewrite_dolby(payload, channels, DOLBY_NAMES[dolby_name() as usize])
}

/// La chaîne du décodeur de jeu (`DIRECTMEDIA-ES-PLAYER`) ne décode que
/// PCM, MP3 et Opus : son puits audio n'a pas de sortie bitstream et plante
/// sur un premier tampon Dolby (vérifié sur la G2, webOS 26). Le Dolby passe
/// par la chaîne des apps (`BUFFERSTREAM`, celle des lecteurs de films), avec
/// ses champs, sans tampon ajouté, et les horodatages en nanosecondes
/// (`pts_scale`) ; `lowDelayMode` et le mode jeu sont gardés.
fn rewrite_dolby(payload: &str, channels: u8, name: &str) -> Option<String> {
    use serde_json::json;
    let mut doc: serde_json::Value = serde_json::from_str(payload).ok()?;
    let arg = doc.get_mut("args")?.get_mut(0)?.as_object_mut()?;
    arg.insert("mediaTransportType".into(), "BUFFERSTREAM".into());
    let option = arg.get_mut("option")?.as_object_mut()?;
    option.insert("transmission".into(), json!({ "contentsType": "LIVE", "trickType": "client-side" }));
    option.insert("seekMode".into(), "keep-rate".into());
    let ext = option.get_mut("externalStreamingInfo")?.as_object_mut()?;
    ext.insert("streamQualityInfo".into(), true.into());
    ext.insert(
        "bufferingCtrInfo".into(),
        json!({
            "preBufferByte": 0, "bufferMinLevel": 0, "bufferMaxLevel": 0,
            "qBufferLevelVideo": 0, "qBufferLevelAudio": 0,
            "srcBufferLevelVideo": { "minimum": 0, "maximum": 8 * 1024 * 1024 },
            "srcBufferLevelAudio": { "minimum": 0, "maximum": 2 * 1024 * 1024 }
        }),
    );
    let contents = ext.get_mut("contents")?.as_object_mut()?;
    contents.get("codec")?.get("audio")?.as_str()?;
    contents.remove("pcmInfo")?;
    contents.get_mut("codec")?.as_object_mut()?.insert("audio".into(), name.into());
    contents.insert("ac3PlusInfo".into(), json!({ "channels": channels, "frequency": 48.0 }));
    contents.insert("format".into(), "RAW".into());
    let es = contents.entry("esInfo").or_insert_with(|| json!({})).as_object_mut()?;
    es.insert("ptsToDecode".into(), 0.into());
    es.insert("pauseAtDecodeTime".into(), true.into());
    es.insert("seperatedPTS".into(), true.into());
    serde_json::to_string(&doc).ok()
}

/// Facteur des horodatages donnés au lecteur : 1 avec le décodeur de jeu
/// (millisecondes), 1 000 000 pour la chaîne des apps (nanosecondes).
#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_pts_scale() -> i64 {
    if REWRITTEN.load(Ordering::Acquire) && REWRITE_EAC3.load(Ordering::Acquire) != 0 { 1_000_000 } else { 1 }
}

pub fn last_load() -> Option<String> {
    LAST.lock().unwrap().clone()
}

#[unsafe(no_mangle)]
pub extern "C" fn kybernow_starfish_seen(payload: *const c_char) {
    if payload.is_null() {
        return;
    }
    let s = unsafe { CStr::from_ptr(payload) }.to_string_lossy().into_owned();
    log::info!("lecteur : chargement {s}");
    *LAST.lock().unwrap() = Some(s);
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
pub extern "C" fn kybernow_starfish_rewrite(payload: *const c_char) -> *mut c_char {
    let channels = REWRITE_EAC3.load(Ordering::Acquire);
    if channels == 0 || payload.is_null() {
        return std::ptr::null_mut();
    }
    let s = unsafe { CStr::from_ptr(payload) }.to_string_lossy();
    match rewrite_eac3(&s, channels) {
        Some(doc) => {
            log::info!("lecteur : chargement réécrit en E-AC-3 {channels} voies : {doc}");
            REWRITTEN.store(true, Ordering::Release);
            into_c(doc)
        }
        None => {
            log::warn!("lecteur : document de chargement inattendu, son Dolby impossible sur ce téléviseur");
            std::ptr::null_mut()
        }
    }
}

fn into_c(s: String) -> *mut c_char {
    // libc::free côté C : même allocateur que malloc.
    let c = CString::new(s).unwrap_or_default();
    let bytes = c.as_bytes_with_nul();
    let p = unsafe { libc::malloc(bytes.len()) } as *mut u8;
    if p.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
    p as *mut c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reecriture_eac3() {
        let doc = r#"{"args":[{"mediaTransportType":"DIRECTMEDIA-ES-PLAYER","option":{"appId":"x","lowDelayMode":true,"externalStreamingInfo":{"contents":{"codec":{"video":"H265","audio":"PCM"},"esInfo":{"videoHeight":2160},"pcmInfo":{"sampleRate":1,"channelMode":"6-channel"}}},"videoInfo":{"isGameMode":true}}}]}"#;
        let out = rewrite_eac3(doc, 6).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let c = &v["args"][0]["option"]["externalStreamingInfo"]["contents"];
        assert_eq!(c["codec"]["audio"], "AC3 PLUS");
        assert_eq!(c["codec"]["video"], "H265");
        assert!(c.get("pcmInfo").is_none());
        assert_eq!(c["ac3PlusInfo"]["channels"], 6);
        assert_eq!(v["args"][0]["option"]["lowDelayMode"], true);
        assert_eq!(v["args"][0]["mediaTransportType"], "BUFFERSTREAM");
        assert_eq!(c["esInfo"]["videoHeight"], 2160);
        assert!(rewrite_eac3(r#"{"args":[{"option":{}}]}"#, 6).is_none(), "sans section audio : rien");
    }
}
