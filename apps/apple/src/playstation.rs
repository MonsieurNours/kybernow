//! Consoles PlayStation pour les apps Apple (docs/playstation.md) :
//! recherche, identifiant du compte PSN, association, réveil et partie.
//! L'image et le son passent par les mêmes rappels que les sessions Kyber
//! (`KnPacketCb` : décodeur VideoToolbox et moteur de son).
//!
//! Les consoles associées sont gardées par l'app telles que
//! `kn_ps_regist` les rend (JSON stable de `Registered`).

use std::ffi::{c_char, c_void};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::media::PacketHandler;
use kybernow_core::playstation::discovery::{self, Model, State};
use kybernow_core::playstation::play::{Play, PlayEnd, PlayOptions, PlayState};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered};
use kybernow_core::playstation::stream::Profile;
use serde_json::json;

use crate::{KnPacketCb, Sink, give, text};

/// Numéros de flux des parties : loin de ceux des sessions Kyber.
static STREAMS: AtomicU64 = AtomicU64::new(1 << 40);

fn parse_addr(p: *const c_char) -> Option<Ipv4Addr> {
    text(p).trim().parse().ok()
}

/// Cherche les consoles pendant `wait_ms` (2000 conseillé) : en diffusion,
/// et en direct vers chaque adresse de `direct_json` (tableau de textes,
/// les consoles déjà connues). Rend un tableau JSON : `address`, `host_id`
/// (le `mac` de la console associée), `name`, `model` (`PS5`/`PS4`),
/// `state` (`ready`, `standby`, `unknown`), `target` (pour `kn_ps_regist`),
/// `system_version`, `running_app`. Bloque : hors du fil de l'interface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_search(direct_json: *const c_char, wait_ms: u32) -> *mut c_char {
    let direct: Vec<Ipv4Addr> = serde_json::from_str::<Vec<String>>(&text(direct_json))
        .unwrap_or_default()
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    let found = discovery::search(Duration::from_millis(wait_ms.clamp(200, 10_000) as u64), &direct).unwrap_or_default();
    let list: Vec<_> = found
        .iter()
        .map(|c| {
            json!({
                "address": c.addr.to_string(),
                "host_id": c.host_id.to_ascii_uppercase(),
                "name": c.name,
                "model": c.model().name(),
                "state": match c.state { State::Ready => "ready", State::Standby => "standby", State::Unknown => "unknown" },
                "target": regist::TargetName::from_target(c.target()),
                "system_version": c.system_version,
                "running_app": c.running_app,
            })
        })
        .collect();
    give(serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
}

/// Page de connexion au compte Sony (à ouvrir dans une fenêtre web ; à
/// libérer par `kn_string_free`).
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_login_url() -> *mut c_char {
    give(psn::LOGIN_URL.to_string())
}

/// Vrai pour l'adresse de retour de Sony : la fenêtre web s'arrête là et
/// passe l'adresse à `kn_ps_account`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_is_redirect(url: *const c_char) -> bool {
    psn::is_redirect(&text(url))
}

/// Identifiant du compte d'après l'adresse de retour de Sony (deux
/// requêtes HTTPS : hors du fil de l'interface). JSON : `{"account":
/// "<base64>"}` ou `{"error": "<raison, pour le journal>"}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_account(redirect_url: *const c_char) -> *mut c_char {
    let url = text(redirect_url);
    let result = match psn::code_from_redirect(&url) {
        None => json!({ "error": psn::PsnError::NoCode.to_string() }),
        Some(code) => match psn::account_id(&code) {
            Ok(id) => json!({ "account": id.to_base64() }),
            Err(e) => {
                log::warn!("PlayStation : identifiant du compte non lu : {e}");
                json!({ "error": e.to_string() })
            }
        },
    };
    give(result.to_string())
}

/// Associe la console (`target` : celui de `kn_ps_search`, `account` :
/// l'identifiant base64, `pin` : le code à 8 chiffres affiché par la
/// console). Bloque quelques secondes. JSON : `{"registered": {…}}`, ou
/// `{"error": "no_answer" | "wrong_code" | "account" | "network" | "other",
/// "detail": "…"}` (`no_answer` : écran « Associer un appareil » fermé).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_regist(address: *const c_char, target: *const c_char, account: *const c_char, pin: u32) -> *mut c_char {
    let fail = |code: &str, detail: String| give(json!({ "error": code, "detail": detail }).to_string());
    let Some(addr) = parse_addr(address) else { return fail("other", "adresse illisible".into()) };
    let Ok(target) = serde_json::from_value::<regist::TargetName>(serde_json::Value::String(text(target))) else {
        return fail("other", "cible illisible".into());
    };
    let Some(id) = PsnId::from_base64(&text(account)) else { return fail("account", "identifiant illisible".into()) };
    match regist::regist(addr, target.into(), &id, pin) {
        Ok(r) => give(json!({ "registered": r }).to_string()),
        Err(e) => {
            log::warn!("PlayStation : association refusée : {e}");
            let code = match &e {
                RegistError::NoAnswer | RegistError::Refused => "no_answer",
                RegistError::Rejected { reason: Some(regist::REASON_REGIST_FAILED), .. } => "wrong_code",
                RegistError::Rejected { reason: Some(regist::REASON_INVALID_PSN_ID), .. } => "account",
                RegistError::Network(_) => "network",
                _ => "other",
            };
            fail(code, e.to_string())
        }
    }
}

/// Réveille une console associée (`registered_json` : ce que rend
/// `kn_ps_regist`). Vrai si le paquet est parti.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_wake(address: *const c_char, registered_json: *const c_char) -> bool {
    let (Some(addr), Ok(r)) = (parse_addr(address), serde_json::from_str::<Registered>(&text(registered_json))) else {
        return false;
    };
    let model = if matches!(r.target, regist::TargetName::Ps5V1) { Model::Ps5 } else { Model::Ps4 };
    let Some(cred) = r.wake_credential() else { return false };
    discovery::wake(std::net::IpAddr::V4(addr), model, cred).is_ok()
}

/// État d'une partie (`on_state`).
pub const KN_PS_CONNECTING: i32 = 0;
pub const KN_PS_STREAMING: i32 = 1;
pub const KN_PS_CANT_DISPLAY: i32 = 2;
pub const KN_PS_CAN_DISPLAY: i32 = 3;
/// Fins : arrêtée par l'app, console déjà jouée ailleurs, à associer de
/// nouveau, injoignable, terminée par la console.
pub const KN_PS_END_STOPPED: i32 = 10;
pub const KN_PS_END_IN_USE: i32 = 11;
pub const KN_PS_END_NOT_PAIRED: i32 = 12;
pub const KN_PS_END_UNREACHABLE: i32 = 13;
pub const KN_PS_END_REMOTE: i32 = 14;

/// Rappels d'une partie, appelés sur les fils du cœur.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnPsCallbacks {
    pub userdata: *mut c_void,
    pub video_packet: KnPacketCb,
    pub audio_packet: KnPacketCb,
    /// `state` : `KN_PS_*` ; `width`, `height`, `hdr` pour `KN_PS_STREAMING` ;
    /// `detail` : la raison d'une fin, pour le journal (peut être vide).
    pub on_state: Option<unsafe extern "C" fn(userdata: *mut c_void, state: i32, width: u32, height: u32, hdr: bool, detail: *const c_char)>,
}

unsafe impl Send for KnPsCallbacks {}
unsafe impl Sync for KnPsCallbacks {}

/// Une partie en cours (opaque pour Swift).
pub struct KnPsPlay {
    play: Play,
}

/// Lance une partie : la console doit être allumée (`kn_ps_wake` sinon).
/// `hdr` : demande le HDR10 (H.265 Main 10, PQ). À terminer par
/// `kn_ps_stop`, qui libère.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_start(address: *const c_char, registered_json: *const c_char, hdr: bool, cb: *const KnPsCallbacks) -> *mut KnPsPlay {
    let (Some(addr), Ok(r), Some(cb)) = (parse_addr(address), serde_json::from_str::<Registered>(&text(registered_json)), unsafe { cb.as_ref() }) else {
        return std::ptr::null_mut();
    };
    let cb = *cb;
    let sink = |packet_cb: KnPacketCb| -> Option<Arc<dyn PacketHandler>> {
        let f = packet_cb?;
        let stream = STREAMS.fetch_add(1, Ordering::Relaxed);
        Some(Arc::new(Sink { cb: f, userdata: cb.userdata, stream, screen: 0 }))
    };
    let Some(video) = sink(cb.video_packet) else { return std::ptr::null_mut() };
    let audio = sink(cb.audio_packet);
    let profile = Profile { hdr, ..Profile::p1080() };
    let on_state = Box::new(move |state: PlayState| {
        // Toute la structure (Send) est prise, pas seulement ses champs.
        let cb: KnPsCallbacks = cb;
        let Some(f) = cb.on_state else { return };
        let detail_text;
        let (code, w, h, hdr, detail) = match &state {
            PlayState::Connecting => (KN_PS_CONNECTING, 0, 0, false, ""),
            PlayState::Streaming { width, height, hdr } => (KN_PS_STREAMING, *width, *height, *hdr, ""),
            PlayState::CantDisplay(true) => (KN_PS_CANT_DISPLAY, 0, 0, false, ""),
            PlayState::CantDisplay(false) => (KN_PS_CAN_DISPLAY, 0, 0, false, ""),
            PlayState::Ended(end) => {
                let (code, d) = match end {
                    PlayEnd::Stopped => (KN_PS_END_STOPPED, String::new()),
                    PlayEnd::InUse => (KN_PS_END_IN_USE, String::new()),
                    PlayEnd::NotPaired => (KN_PS_END_NOT_PAIRED, String::new()),
                    PlayEnd::Unreachable(s) => (KN_PS_END_UNREACHABLE, s.clone()),
                    PlayEnd::Remote(s) => (KN_PS_END_REMOTE, s.clone()),
                };
                detail_text = d;
                (code, 0, 0, false, detail_text.as_str())
            }
        };
        let c = std::ffi::CString::new(detail.replace('\0', " ")).unwrap_or_default();
        unsafe { f(cb.userdata, code, w, h, hdr, c.as_ptr()) };
    });
    let play = Play::start(addr, &r, PlayOptions { profile }, video, audio, on_state);
    Box::into_raw(Box::new(KnPsPlay { play }))
}

/// Met la console en mode repos (la partie se termine ensuite).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_goto_bed(play: *mut KnPsPlay) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.goto_bed();
    }
}

/// Arrête la partie, attend sa fin et la libère (le dernier `on_state` est
/// une fin). Ne pas appeler depuis un rappel de la partie.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_stop(play: *mut KnPsPlay) {
    if play.is_null() {
        return;
    }
    let mut p = unsafe { Box::from_raw(play) };
    p.play.stop();
}
