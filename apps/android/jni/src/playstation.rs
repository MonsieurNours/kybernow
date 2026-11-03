//! Consoles PlayStation pour l'app Android (docs/playstation.md) : les
//! fonctions JNI de l'objet Kotlin `fr.leslignesclaires.kybernow.core.PsNative`,
//! l'équivalent du pont des apps Apple (`apps/apple/src/playstation.rs`) :
//! recherche, identifiant du compte PSN, association, réveil, hors de chez
//! soi par une Station, partie, envoi entre appareils.
//!
//! L'image et le son d'une partie passent par les mêmes MediaCodec et AAudio
//! que les sessions d'ordinateur (`video`, `audio`, par [`MediaHost`]) ; les
//! états, le clavier de la console, la combinaison et les vibrations vont à
//! l'objet Kotlin `PsListener`. Les manettes de l'app sont données au lecteur
//! du cœur (`gamepads`) le temps de la partie, et ses vibrations reviennent
//! par `gamepads::AppFeedback`.
//!
//! **Adresse d'une console** : une adresse IPv4 sur le réseau (ou par un
//! VPN), ou, hors de chez soi, ce que rend `remoteAddress` (un JSON : la
//! boucle locale et les ports du tunnel vers une Station qui voit la console).
//!
//! Jamais au journal : l'identifiant du compte, les clés d'association.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jfloat, jint, jlong, jstring, JNI_FALSE};
use ndk::native_window::NativeWindow;
use serde_json::json;

use kybernow_core::audio::AudioEngine;
use kybernow_core::gamepads::{self, AppFeedback};
use kybernow_core::input::pipeline::GamepadRumble;
use kybernow_core::input::{PadAxis, PadButton, PadId, PadKey};
use kybernow_core::media::PacketHandler;
use kybernow_core::playstation::discovery::{self, Console, Model, State};
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::handoff;
use kybernow_core::playstation::keymouse::Layout;
use kybernow_core::playstation::play::{KeyboardEvent, PadEvent, Play, PlayEnd, PlayOptions, PlayState};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered};
use kybernow_core::playstation::remote::{Remote, RemoteError};
use kybernow_core::playstation::settings::PlaySettings;

use crate::audio::AudioSink;
use crate::exports::{console, envelope, give, jbool, string, window, Env};
use crate::jvm::Listener;
use crate::media::MediaHost;
use crate::video::{Msg, VideoSink};

fn parse_addr(text: &str) -> Option<Endpoint> {
    Endpoint::parse(text)
}

fn found_json(found: &[Console]) -> String {
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
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
}

fn wait(ms: jint) -> Duration {
    Duration::from_millis(ms.clamp(200, 10_000) as u64)
}

// MARK: - Recherche et réveil

/// Cherche les consoles pendant `waitMs` (2000 conseillé) : en diffusion, et
/// en direct vers chaque adresse de `directJson` (tableau de textes). JSON :
/// `[{address, host_id, name, model, state, target, system_version,
/// running_app}]`. Bloque : hors du fil principal, verrou de multidiffusion pris
/// par l'app.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_search(mut env: Env, _: JClass, direct_json: JString, wait_ms: jint) -> jstring {
    let direct: Vec<Ipv4Addr> = serde_json::from_str::<Vec<String>>(&string(&mut env, &direct_json))
        .unwrap_or_default()
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();
    let found = discovery::search(wait(wait_ms), &direct).unwrap_or_default();
    give(&mut env, &found_json(&found))
}

/// Une seule console à `address` (adresse ou adresse par le relais), sans
/// diffusion. Même JSON. Bloque.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_searchAt(mut env: Env, _: JClass, address: JString, wait_ms: jint) -> jstring {
    let Some(at) = parse_addr(&string(&mut env, &address)) else { return give(&mut env, "[]") };
    let found = discovery::search_at(wait(wait_ms), false, &[at]).unwrap_or_default();
    give(&mut env, &found_json(&found))
}

/// Réveille une console associée (`registeredJson` : l'association). Vrai si le paquet est parti.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_wake(mut env: Env, _: JClass, address: JString, registered_json: JString) -> jboolean {
    let (addr, reg) = (string(&mut env, &address), string(&mut env, &registered_json));
    let (Some(addr), Ok(r)) = (parse_addr(&addr), serde_json::from_str::<Registered>(&reg)) else { return JNI_FALSE };
    let model = if matches!(r.target, regist::TargetName::Ps5V1) { Model::Ps5 } else { Model::Ps4 };
    let Some(cred) = r.wake_credential() else { return JNI_FALSE };
    jbool(discovery::wake_at(&addr, model, cred).is_ok())
}

// MARK: - Compte PSN et association

/// Page de connexion au compte Sony (WebView).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_loginUrl(mut env: Env, _: JClass) -> jstring {
    give(&mut env, psn::LOGIN_URL)
}

/// Vrai pour l'adresse de retour de Sony : la WebView s'arrête là.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_isRedirect(mut env: Env, _: JClass, url: JString) -> jboolean {
    jbool(psn::is_redirect(&string(&mut env, &url)))
}

/// Identifiant du compte d'après l'adresse de retour (deux requêtes HTTPS :
/// hors du fil principal). JSON : `{"account": "<base64>", "online_id": …}`
/// ou `{"error": "…"}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_account(mut env: Env, _: JClass, redirect_url: JString) -> jstring {
    let url = string(&mut env, &redirect_url);
    let result = match psn::code_from_redirect(&url) {
        None => json!({ "error": psn::PsnError::NoCode.to_string() }),
        Some(code) => match psn::account(&code) {
            Ok(a) => json!({ "account": a.id.to_base64(), "online_id": a.online_id }),
            Err(e) => {
                log::warn!("PlayStation : identifiant du compte non lu : {e}");
                json!({ "error": e.to_string() })
            }
        },
    };
    give(&mut env, &result.to_string())
}

/// L'identifiant saisi à la main se lit (base64 de 8 octets).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_accountIdValid(mut env: Env, _: JClass, id: JString) -> jboolean {
    jbool(PsnId::from_base64(&string(&mut env, &id)).is_some())
}

/// Associe la console (`target` de la recherche, `account` en base64, `pin` :
/// le code à 8 chiffres). Bloque quelques secondes. JSON : `{"registered":
/// {…}}` ou `{"error": "no_answer" | "wrong_code" | "account" | "network" |
/// "other", "detail": "…"}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_regist(
    mut env: Env,
    _: JClass,
    address: JString,
    target: JString,
    account: JString,
    pin: jint,
) -> jstring {
    let (address, target, account) = (string(&mut env, &address), string(&mut env, &target), string(&mut env, &account));
    let fail = |env: &mut Env, code: &str, detail: String| give(env, &json!({ "error": code, "detail": detail }).to_string());
    let Some(addr) = parse_addr(&address) else { return fail(&mut env, "other", "adresse illisible".into()) };
    let Ok(target) = serde_json::from_value::<regist::TargetName>(serde_json::Value::String(target)) else {
        return fail(&mut env, "other", "cible illisible".into());
    };
    let Some(id) = PsnId::from_base64(&account) else { return fail(&mut env, "account", "identifiant illisible".into()) };
    match regist::regist_at(&addr, target.into(), &id, pin.max(0) as u32) {
        Ok(r) => give(&mut env, &json!({ "registered": r }).to_string()),
        Err(e) => {
            log::warn!("PlayStation : association refusée : {e}");
            let code = match &e {
                RegistError::NoAnswer | RegistError::Refused => "no_answer",
                RegistError::Rejected { reason: Some(regist::REASON_REGIST_FAILED), .. } => "wrong_code",
                RegistError::Rejected { reason: Some(regist::REASON_INVALID_PSN_ID), .. } => "account",
                RegistError::Network(_) => "network",
                _ => "other",
            };
            fail(&mut env, code, e.to_string())
        }
    }
}

// MARK: - Hors de chez soi (docs/relais.md, « Consoles PlayStation »)

/// La Station ne voit pas (ou plus) cette console sur son réseau.
const NOT_SEEN: i32 = 40;

/// `GET /api/consoles` d'une console d'Admin (`Native.consoleNew…`) : réponse
/// d'Admin `{"code": 0, "value": [{host_id, name, model, state, seen}]}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_stationConsoles(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return give(&mut env, "{\"code\":1,\"error\":\"\"}") };
    let s = envelope(c.consoles());
    give(&mut env, &s)
}

/// Joint la console `hostId` par la Station de `h` (une console d'Admin par
/// le relais, `Native.consoleNewRelay`) ; le signe de vie repart toutes les
/// 10 s tant que la poignée vit (`remoteFree`). JSON : `{"code": 0,
/// "handle": …}`, ou `{"code": 30 | 31 | 32 | −401 | −404 | 40 | 1, "error": …}`
/// (40 : la Station ne voit pas la console). Bloque.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_remoteOpen(mut env: Env, _: JClass, h: jlong, host_id: JString) -> jstring {
    let host_id = string(&mut env, &host_id);
    let Some(c) = console(h) else { return give(&mut env, "{\"code\":1,\"error\":\"\"}") };
    let s = match Remote::open(c, &host_id) {
        Ok(remote) => json!({ "code": 0, "handle": Box::into_raw(Box::new(remote)) as jlong }).to_string(),
        Err(RemoteError::NotSeen) => json!({ "code": NOT_SEEN, "error": RemoteError::NotSeen.to_string() }).to_string(),
        Err(RemoteError::NotRelayed) => json!({ "code": 1, "error": RemoteError::NotRelayed.to_string() }).to_string(),
        Err(RemoteError::Console(e)) => envelope::<()>(Err(e)),
    };
    give(&mut env, &s)
}

fn remote<'a>(h: jlong) -> Option<&'a Remote> {
    (h != 0).then(|| unsafe { &*(h as *const Remote) })
}

/// L'adresse de la console par le relais, à passer aux autres fonctions.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_remoteAddress(mut env: Env, _: JClass, h: jlong) -> jstring {
    match remote(h) {
        Some(r) => give(&mut env, &r.endpoint().to_text()),
        None => std::ptr::null_mut(),
    }
}

/// L'état vu par la Station au dernier signe de vie (`ready`, `standby`,
/// `unknown`) ; nul si elle ne la voit plus.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_remoteState(mut env: Env, _: JClass, h: jlong) -> jstring {
    match remote(h).and_then(|r| r.seen()) {
        Some(c) => give(&mut env, &c.state),
        None => std::ptr::null_mut(),
    }
}

/// Le chemin (JSON de `onRoute` des sessions : `transport`, `relay`, `rtt_client`, `rtt_station`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_remoteRoute(mut env: Env, _: JClass, h: jlong) -> jstring {
    match remote(h) {
        Some(r) => give(&mut env, &serde_json::to_string(&r.route_info()).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_remoteFree(_: Env, _: JClass, h: jlong) {
    if h != 0 {
        drop(unsafe { Box::from_raw(h as *mut Remote) });
    }
}

// MARK: - Réglages

/// Les réglages par défaut d'une console (JSON) : Auto, 60 i/s.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_defaultSettings(mut env: Env, _: JClass) -> jstring {
    give(&mut env, &serde_json::to_string(&PlaySettings::default()).unwrap_or_default())
}

/// Vrai si les réglages s'écartent d'Auto (« Personnalisé » à l'accueil).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_settingsCustom(mut env: Env, _: JClass, json: JString) -> jboolean {
    jbool(serde_json::from_str::<PlaySettings>(&string(&mut env, &json)).is_ok_and(|s| s.is_custom()))
}

/// La disposition par défaut du clavier et de la souris (JSON).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_defaultLayout(mut env: Env, _: JClass) -> jstring {
    give(&mut env, &serde_json::to_string(&Layout::default()).unwrap_or_default())
}

// MARK: - Partie

/// État d'une partie (`PsListener.onPsState`).
const CONNECTING: i32 = 0;
const STREAMING: i32 = 1;
const CANT_DISPLAY: i32 = 2;
const CAN_DISPLAY: i32 = 3;
/// Fins : arrêtée par l'app, déjà jouée ailleurs, à associer de nouveau, injoignable, terminée par la console.
const END_STOPPED: i32 = 10;
const END_IN_USE: i32 = 11;
const END_NOT_PAIRED: i32 = 12;
const END_UNREACHABLE: i32 = 13;
const END_REMOTE: i32 = 14;

/// Clavier de la console (`PsListener.onPsKeyboard`) : ouvert, texte renvoyé, fermé.
const KEYBOARD_OPEN: i32 = 0;
const KEYBOARD_TEXT: i32 = 1;
const KEYBOARD_CLOSED: i32 = 2;

/// Ce que partagent la partie, son image, son son et les appels de Kotlin.
struct Host {
    listener: Listener,
    window: Mutex<Option<NativeWindow>>,
    video_tx: Mutex<Option<Sender<Msg>>>,
    refresh: AtomicU32,
    size: (u32, u32),
    hdr: bool,
    play: OnceLock<Weak<Mutex<Option<Play>>>>,
    engine: Mutex<Option<Arc<AudioEngine>>>,
    muted: std::sync::atomic::AtomicBool,
}

impl Host {
    fn with_play(&self, f: impl FnOnce(&Play)) {
        if let Some(p) = self.play.get().and_then(Weak::upgrade) {
            if let Some(p) = p.lock().unwrap().as_ref() {
                f(p);
            }
        }
    }

    fn set_window(&self, w: Option<NativeWindow>) {
        *self.window.lock().unwrap() = w;
        if let Some(tx) = self.video_tx.lock().unwrap().as_ref() {
            let _ = tx.send(Msg::Surface);
        }
    }
}

impl MediaHost for Host {
    fn listener(&self) -> &Listener {
        &self.listener
    }

    fn window(&self) -> Option<NativeWindow> {
        self.window.lock().unwrap().clone()
    }

    fn video_size(&self) -> (u32, u32) {
        self.size
    }

    fn frame_period_micros(&self) -> i64 {
        1_000_000 / self.refresh.load(Ordering::Relaxed).max(24) as i64
    }

    fn set_video_tx(&self, tx: Sender<Msg>) {
        *self.video_tx.lock().unwrap() = Some(tx);
    }

    fn hdr(&self) -> bool {
        self.hdr
    }

    // La console répète la demande d'image clé toutes les 500 ms jusqu'à ce
    // qu'elle arrive : demander et perdre reviennent au même.
    fn request_keyframe(&self) {
        self.with_play(Play::key_frame_lost);
    }

    fn keyframe_lost(&self) {
        self.with_play(Play::key_frame_lost);
    }

    fn keyframe_recovered(&self) {
        self.with_play(Play::key_frame_recovered);
    }

    fn audio_engine(&self, engine: &Arc<AudioEngine>) {
        engine.set_muted(self.muted.load(Ordering::Relaxed));
        *self.engine.lock().unwrap() = Some(engine.clone());
    }
}

/// Vibrations de la console vers l'app (`PsListener.onPsRumble`) : les
/// gâchettes adaptatives n'ont pas d'interface publique sur Android (rapport
/// d'effets ignoré, voir apps/android/README.md).
struct Feedback(Arc<Host>);

impl AppFeedback for Feedback {
    fn rumble(&self, r: GamepadRumble) {
        self.0.listener.void(
            "onPsRumble",
            "(IIII)V",
            &[JValue::Int(r.id as i32), JValue::Int(r.large_motor as i32), JValue::Int(r.small_motor as i32), JValue::Int(r.duration_ms)],
        );
    }
}

/// Une partie en cours : l'objet rendu à Kotlin.
pub struct Handle {
    host: Arc<Host>,
    play: Arc<Mutex<Option<Play>>>,
}

fn handle<'a>(h: jlong) -> Option<&'a Handle> {
    (h != 0).then(|| unsafe { &*(h as *const Handle) })
}

fn with_play(h: jlong, f: impl FnOnce(&Play)) {
    if let Some(h) = handle(h) {
        if let Some(p) = h.play.lock().unwrap().as_ref() {
            f(p);
        }
    }
}

/// Lance une partie (la console allumée : `wake` sinon). `settingsJson` : les
/// réglages de la console (vide ou illisible : Auto), `hdr` déjà décidé par
/// l'app ; `hevcDecoder` : l'appareil décode le H.265 ; `surface` : celle de
/// la `SurfaceView` (nulle tant qu'elle n'existe pas) ; `listener` : un
/// `PsListener` ; `refresh` : fréquence de l'écran. 0 si l'adresse ou
/// l'association ne se lisent pas. À terminer par `stop` (hors du fil
/// principal : il attend la fin).
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_start(
    mut env: Env,
    _: JClass,
    address: JString,
    registered_json: JString,
    settings_json: JString,
    hevc_decoder: jboolean,
    surface: JObject,
    listener: JObject,
    refresh: jint,
) -> jlong {
    let (address, reg, settings) = (string(&mut env, &address), string(&mut env, &registered_json), string(&mut env, &settings_json));
    let (Some(addr), Ok(r)) = (parse_addr(&address), serde_json::from_str::<Registered>(&reg)) else {
        log::error!("PlayStation : adresse ou association illisible");
        return 0;
    };
    let w = window(&mut env, &surface);
    let Ok(listener) = Listener::new(&mut env, &listener) else { return 0 };
    let settings = serde_json::from_str::<PlaySettings>(&settings).unwrap_or_default();
    let ps5 = matches!(r.target, regist::TargetName::Ps5V1);
    let profile = settings.profile(hevc_decoder != 0, ps5);
    let host = Arc::new(Host {
        listener,
        window: Mutex::new(w),
        video_tx: Mutex::new(None),
        refresh: AtomicU32::new(refresh.max(0) as u32),
        size: (profile.width, profile.height),
        hdr: profile.hdr,
        play: OnceLock::new(),
        engine: Mutex::new(None),
        muted: std::sync::atomic::AtomicBool::new(false),
    });
    let video: Arc<dyn PacketHandler> = VideoSink::new(host.clone() as Arc<dyn MediaHost>);
    let audio: Arc<dyn PacketHandler> = AudioSink::new(host.clone() as Arc<dyn MediaHost>);
    gamepads::set_app_feedback(Some(Arc::new(Feedback(host.clone()))));
    let h = host.clone();
    let on_state = Box::new(move |state: PlayState| {
        let (code, w, hh, hdr, detail) = match &state {
            PlayState::Connecting => (CONNECTING, 0, 0, false, String::new()),
            PlayState::Streaming { width, height, hdr } => (STREAMING, *width, *height, *hdr, String::new()),
            PlayState::CantDisplay(true) => (CANT_DISPLAY, 0, 0, false, String::new()),
            PlayState::CantDisplay(false) => (CAN_DISPLAY, 0, 0, false, String::new()),
            PlayState::Ended(end) => match end {
                PlayEnd::Stopped => (END_STOPPED, 0, 0, false, String::new()),
                PlayEnd::InUse => (END_IN_USE, 0, 0, false, String::new()),
                PlayEnd::NotPaired => (END_NOT_PAIRED, 0, 0, false, String::new()),
                PlayEnd::Unreachable(s) => (END_UNREACHABLE, 0, 0, false, s.clone()),
                PlayEnd::Remote(s) => (END_REMOTE, 0, 0, false, s.clone()),
            },
        };
        h.listener.with(|env, obj| {
            let d = env.new_string(&detail)?;
            env.call_method(
                obj,
                "onPsState",
                "(IIIZLjava/lang/String;)V",
                &[JValue::Int(code), JValue::Int(w as i32), JValue::Int(hh as i32), JValue::Bool(hdr as u8), JValue::Object(&d)],
            )
            .map(|_| ())
        });
    });
    let h = host.clone();
    let on_pad = Box::new(move |e: PadEvent| match e {
        PadEvent::Menu => h.listener.void("onPsMenu", "()V", &[]),
        PadEvent::Navigate(k) => {
            // Comme les touches de navigation des apps Apple (`KN_PAD_KEY_*`).
            let key = match k {
                PadKey::Left => 0,
                PadKey::Right => 1,
                PadKey::Up => 2,
                PadKey::Down => 3,
                PadKey::Accept => 4,
                PadKey::Back => 5,
            };
            h.listener.void("onPsNavigate", "(I)V", &[JValue::Int(key)]);
        }
    });
    let play = Play::start(addr, &r, PlayOptions { settings, hevc_decoder: hevc_decoder != 0 }, video, Some(audio), on_state, on_pad);
    let h = host.clone();
    play.set_on_keyboard(Some(Box::new(move |e: KeyboardEvent| {
        let (code, t) = match e {
            KeyboardEvent::Open(t) => (KEYBOARD_OPEN, t),
            KeyboardEvent::Text(t) => (KEYBOARD_TEXT, t),
            KeyboardEvent::Closed => (KEYBOARD_CLOSED, String::new()),
        };
        h.listener.with(|env, obj| {
            let s = env.new_string(&t)?;
            env.call_method(obj, "onPsKeyboard", "(ILjava/lang/String;)V", &[JValue::Int(code), JValue::Object(&s)]).map(|_| ())
        });
    })));
    let play = Arc::new(Mutex::new(Some(play)));
    let _ = host.play.set(Arc::downgrade(&play));
    Box::into_raw(Box::new(Handle { host, play })) as jlong
}

/// La surface a changé (recréée, retirée en arrière-plan).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_setSurface(mut env: Env, _: JClass, h: jlong, surface: JObject, refresh: jint) {
    let Some(h) = handle(h) else { return };
    let w = window(&mut env, &surface);
    if refresh > 0 {
        h.host.refresh.store(refresh as u32, Ordering::Relaxed);
    }
    h.host.set_window(w);
}

/// Arrête la partie, attend sa fin (le dernier `onPsState` est une fin) et
/// la libère. Hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_stop(_: Env, _: JClass, h: jlong) {
    if h == 0 {
        return;
    }
    let h = unsafe { Box::from_raw(h as *mut Handle) };
    // Le verrou n'est pas tenu pendant l'attente : le fil du décodeur peut encore le prendre.
    let play = h.play.lock().unwrap().take();
    if let Some(mut p) = play {
        p.stop();
    }
    gamepads::set_app_feedback(None);
    for p in gamepads::pads() {
        gamepads::unplug(p.id);
    }
    h.host.set_window(None);
}

/// Touche du clavier (usage USB HID), traduite en manette d'après la disposition.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_key(_: Env, _: JClass, h: jlong, usage: jint, pressed: jboolean) {
    with_play(h, |p| p.key(usage as u16, pressed != 0));
}

/// Bouton de la souris : 0 gauche, 1 droit, 2 milieu, 3 et 4 latéraux.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_mouseButton(_: Env, _: JClass, h: jlong, button: jint, pressed: jboolean) {
    with_play(h, |p| p.mouse_button(button as u8, pressed != 0));
}

/// Déplacement brut de la souris capturée.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_mouseMove(_: Env, _: JClass, h: jlong, dx: jfloat, dy: jfloat) {
    with_play(h, |p| p.mouse_move(dx, dy));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_releaseAll(_: Env, _: JClass, h: jlong) {
    with_play(h, Play::release_all);
}

/// Pose la disposition du clavier et de la souris de la console. Faux si le JSON est illisible.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_setLayout(mut env: Env, _: JClass, h: jlong, json: JString) -> jboolean {
    let Ok(layout) = serde_json::from_str::<Layout>(&string(&mut env, &json)) else { return JNI_FALSE };
    with_play(h, |p| p.set_layout(layout));
    jbool(true)
}

/// Intensités de la manette (0 à 100 %), tout de suite.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_setIntensities(_: Env, _: JClass, h: jlong, rumble: jint, triggers: jint) {
    with_play(h, |p| p.set_intensities(rumble.clamp(0, 100) as u8, triggers.clamp(0, 100) as u8));
}

/// La barre (ou le menu du téléviseur) s'ouvre : la console reçoit une manette au repos.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_setCaptured(_: Env, _: JClass, h: jlong, on: jboolean) {
    with_play(h, |p| p.set_captured(on != 0));
}

/// Son coupé ou rétabli (« Couper le son »).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_setMuted(_: Env, _: JClass, h: jlong, muted: jboolean) {
    let Some(h) = handle(h) else { return };
    h.host.muted.store(muted != 0, Ordering::Relaxed);
    if let Some(e) = h.host.engine.lock().unwrap().as_ref() {
        e.set_muted(muted != 0);
    }
}

/// Mesures depuis l'appel précédent (une fois par seconde) : `width`,
/// `height`, `hevc`, `hdr`, `target_kbps`, `fps`, `mbps`, `lost`, `recovered`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_stats(mut env: Env, _: JClass, h: jlong) -> jstring {
    let mut out = None;
    with_play(h, |p| out = serde_json::to_string(&p.stats()).ok());
    match out {
        Some(s) => give(&mut env, &s),
        None => std::ptr::null_mut(),
    }
}

/// « Mettre la console en veille » (la partie se termine ensuite).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_gotoBed(_: Env, _: JClass, h: jlong) {
    with_play(h, |p| {
        p.goto_bed();
    });
}

/// Clavier de la console ouvert : le texte entier de son champ, à chaque changement.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_keyboardText(mut env: Env, _: JClass, h: jlong, text: JString) -> jboolean {
    let t = string(&mut env, &text);
    let mut ok = false;
    with_play(h, |p| ok = p.keyboard_text(&t));
    jbool(ok)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_keyboardAccept(_: Env, _: JClass, h: jlong) -> jboolean {
    let mut ok = false;
    with_play(h, |p| ok = p.keyboard_accept());
    jbool(ok)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_keyboardReject(_: Env, _: JClass, h: jlong) -> jboolean {
    let mut ok = false;
    with_play(h, |p| ok = p.keyboard_reject());
    jbool(ok)
}

// MARK: - Manettes de l'app, le temps d'une partie

/// Boutons dans l'ordre de `PadButton::ALL` (comme `Native.padButton`).
fn button(i: jint) -> Option<PadButton> {
    PadButton::ALL.get(usize::try_from(i).ok()?).copied()
}

fn axis(i: jint) -> Option<PadAxis> {
    PadAxis::ALL.get(usize::try_from(i).ok()?).copied()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padPlug(_: Env, _: JClass, pad: jint, vendor: jint, product: jint) {
    gamepads::plug(pad as PadId, u16::try_from(vendor).ok(), u16::try_from(product).ok());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padUnplug(_: Env, _: JClass, pad: jint) {
    gamepads::unplug(pad as PadId);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padButton(_: Env, _: JClass, pad: jint, b: jint, pressed: jboolean) {
    if let Some(b) = button(b) {
        gamepads::button(pad as PadId, b, pressed != 0);
    }
}

/// `value` : −32768 à 32767 pour les sticks (Y positif vers le bas), 0 à 32767 pour les gâchettes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padAxis(_: Env, _: JClass, pad: jint, a: jint, value: jint) {
    if let Some(a) = axis(a) {
        gamepads::axis(pad as PadId, a, value.clamp(i16::MIN as i32, i16::MAX as i32) as i16);
    }
}

/// Pavé tactile : un doigt (0 ou 1) posé à `(x, y)` (de 0 à 1), ou levé.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padTouch(_: Env, _: JClass, pad: jint, finger: jint, down: jboolean, x: jfloat, y: jfloat) {
    let at = (down != 0).then_some((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
    gamepads::global().touch_finger(pad as PadId, finger.max(0) as usize, at);
}

/// Clic du pavé tactile.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_padTouchClick(_: Env, _: JClass, pad: jint, pressed: jboolean) {
    gamepads::global().touch_click(pad as PadId, pressed != 0);
}

// MARK: - Envoi entre appareils (docs/playstation.md, « Plusieurs appareils »)

/// Commence à attendre un envoi (`name` : nom de l'appareil, `platform` :
/// `android` ou `androidtv`). 0 si l'écoute est impossible ; à libérer par `handoffStop`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffReceive(mut env: Env, _: JClass, name: JString, platform: JString) -> jlong {
    let (name, platform) = (string(&mut env, &name), string(&mut env, &platform));
    match handoff::Receiver::start(&name, &platform) {
        Ok(r) => Box::into_raw(Box::new(r)) as jlong,
        Err(e) => {
            log::warn!("PlayStation : attente d'un envoi impossible ({e})");
            0
        }
    }
}

fn receiver<'a>(h: jlong) -> Option<&'a handoff::Receiver> {
    (h != 0).then(|| unsafe { &*(h as *const handoff::Receiver) })
}

/// Le code à 6 chiffres à afficher.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffCode(mut env: Env, _: JClass, h: jlong) -> jstring {
    match receiver(h) {
        Some(r) => give(&mut env, &r.code()),
        None => std::ptr::null_mut(),
    }
}

/// Le prochain évènement, attendu `timeoutMs` au plus (bloque) :
/// `{"event":"received","bundle":"<JSON>"}`, `{"event":"new_code","code":"…"}`,
/// `{"event":"expired"}`, ou nul.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffNext(mut env: Env, _: JClass, h: jlong, timeout_ms: jint) -> jstring {
    let Some(r) = receiver(h) else { return std::ptr::null_mut() };
    let s = match r.next(Duration::from_millis(timeout_ms.max(0) as u64)) {
        Some(handoff::Event::Received(bundle)) => json!({ "event": "received", "bundle": bundle }).to_string(),
        Some(handoff::Event::NewCode(code)) => json!({ "event": "new_code", "code": code }).to_string(),
        Some(handoff::Event::Expired) => json!({ "event": "expired" }).to_string(),
        None => return std::ptr::null_mut(),
    };
    give(&mut env, &s)
}

/// Arrête l'attente et la libère.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffStop(_: Env, _: JClass, h: jlong) {
    if h != 0 {
        drop(unsafe { Box::from_raw(h as *mut handoff::Receiver) });
    }
}

/// Les appareils du réseau qui attendent un envoi, cherchés `waitMs` (bloque) :
/// `[{"name","platform","address":"ip:port"}]`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffDiscover(mut env: Env, _: JClass, wait_ms: jint) -> jstring {
    let list = handoff::discover(Duration::from_millis(wait_ms.max(0) as u64), &[]).unwrap_or_default();
    let v: Vec<_> = list.iter().map(|w| json!({ "name": w.name, "platform": w.platform, "address": w.addr.to_string() })).collect();
    give(&mut env, &serde_json::Value::Array(v).to_string())
}

/// Envoie le paquet à `address` (`ip:port`) avec le code tapé (bloque
/// quelques secondes) : `{"ok":true}` ou `{"error":"refused"|"unreachable","detail":"…"}`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_PsNative_handoffSend(mut env: Env, _: JClass, address: JString, code: JString, bundle: JString) -> jstring {
    let (address, code, bundle) = (string(&mut env, &address), string(&mut env, &code), string(&mut env, &bundle));
    let s = match address.parse::<std::net::SocketAddr>() {
        Err(_) => json!({ "error": "unreachable", "detail": "adresse illisible" }),
        Ok(to) => match handoff::send(to, &code, &bundle) {
            Ok(()) => json!({ "ok": true }),
            Err(handoff::SendError::Refused) => json!({ "error": "refused", "detail": "" }),
            Err(handoff::SendError::Unreachable(d)) => json!({ "error": "unreachable", "detail": d }),
        },
    };
    give(&mut env, &s.to_string())
}
