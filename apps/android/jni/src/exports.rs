//! Les fonctions JNI de `fr.leslignesclaires.kybernow.core.Native` (objet
//! Kotlin, méthodes `@JvmStatic external`). Les objets du cœur passent en
//! `Long` (pointeur d'une `Box`), libérés par la fonction `…Free`
//! correspondante. Les réponses d'Admin reviennent en JSON :
//! `{"code": 0, "value": …}` ou `{"code": …, "error": "…"}` (code : 1
//! injoignable, 3 certificat changé, −401 compte refusé, −404 appel inconnu
//! de cette version d'Admin). Les appels de la Station (redémarrer, mettre à
//! jour) ont leurs propres codes de refus, 10 à 15, avec `kind`, `device` et
//! `seconds` (voir `station_envelope`). Le réveil par le réseau (`wake`)
//! rend un code simple : 0 envoyé, 1 rien n'est parti, 20 aucune cible.

use std::sync::OnceLock;
use std::time::Duration;

use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;
use ndk::native_window::NativeWindow;

use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Mode, PairStatus, StationError};
use kybernow_core::i18n::{self, Lang};
use kybernow_core::input::{MouseMode, MouseModeRules, PadAxis, PadButton};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::latency;
use kybernow_core::wake;

use crate::jvm::Listener;
use crate::session::{self, Handle};

pub(crate) type Env<'a> = JNIEnv<'a>;

pub(crate) fn string(env: &mut Env, s: &JString) -> String {
    if s.is_null() {
        return String::new();
    }
    env.get_string(s).map(Into::into).unwrap_or_default()
}

pub(crate) fn optional(env: &mut Env, s: &JString) -> Option<String> {
    let v = string(env, s);
    (!v.is_empty()).then_some(v)
}

pub(crate) fn give(env: &mut Env, s: &str) -> jstring {
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn window(env: &mut Env, surface: &JObject) -> Option<NativeWindow> {
    if surface.is_null() {
        return None;
    }
    unsafe { NativeWindow::from_surface(env.get_raw() as _, surface.as_raw() as _) }
}

fn jbool(b: bool) -> jboolean {
    if b { JNI_TRUE } else { JNI_FALSE }
}

// MARK: - Général

/// Journal vers logcat (étiquette `KyberNow`) et langue des messages du cœur.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_init(mut env: Env, _: JClass, language: JString) {
    static LOG: OnceLock<()> = OnceLock::new();
    LOG.get_or_init(|| {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("KyberNow"));
    });
    let tag = string(&mut env, &language);
    i18n::set_current(Lang::from_tag(&tag));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_nowMicros(_: Env, _: JClass) -> jlong {
    latency::now_micros()
}

// MARK: - Admin

pub(crate) fn envelope<T: serde::Serialize>(r: Result<T, ConsoleError>) -> String {
    match r {
        Ok(v) => serde_json::json!({ "code": 0, "value": v }).to_string(),
        Err(e) => {
            let code = match &e {
                ConsoleError::Http { status, .. } => -(*status as i32),
                ConsoleError::CertificateChanged { .. } => 3,
                _ => 1,
            };
            serde_json::json!({ "code": code, "error": e.to_string() }).to_string()
        }
    }
}

pub(crate) fn console<'a>(h: jlong) -> Option<&'a Console> {
    (h != 0).then(|| unsafe { &*(h as *const Console) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleNew(
    mut env: Env,
    _: JClass,
    host: JString,
    port: jint,
    user: JString,
    password: JString,
    pinned: JString,
) -> jlong {
    let (host, user, password, pinned) =
        (string(&mut env, &host), string(&mut env, &user), string(&mut env, &password), optional(&mut env, &pinned));
    Box::into_raw(Box::new(Console::new(&host, port as u16, &user, &password, pinned))) as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleFree(_: Env, _: JClass, h: jlong) {
    if h != 0 {
        drop(unsafe { Box::from_raw(h as *mut Console) });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleFingerprintSeen(mut env: Env, _: JClass, h: jlong) -> jstring {
    match console(h).and_then(|c| c.fingerprint_seen()) {
        Some(fp) => give(&mut env, &fp),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleInfo(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.info());
    give(&mut env, &s)
}

/// Premier contact : `value` nul si le compte manque (la poignée de main a
/// montré le certificat).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleProbe(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let s = envelope(c.probe());
    give(&mut env, &s)
}

/// Aperçu JPEG de l'écran de l'ordinateur ; nul en cas d'erreur, vide si
/// Admin ne peut pas capturer.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consolePreview<'a>(env: Env<'a>, _: JClass, h: jlong, width: jint) -> JByteArray<'a> {
    let Some(c) = console(h) else { return JByteArray::default() };
    match c.preview(width.max(0) as u32) {
        Ok(bytes) => env.byte_array_from_slice(&bytes).unwrap_or_default(),
        Err(_) => JByteArray::default(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consolePairRequest(
    mut env: Env,
    _: JClass,
    h: jlong,
    code: JString,
    device_json: JString,
) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let (code, device) = (string(&mut env, &code), string(&mut env, &device_json));
    let s = match serde_json::from_str::<DeviceInfo>(&device) {
        Ok(d) => envelope(c.pair_request(&code, &d)),
        Err(e) => serde_json::json!({ "code": 1, "error": e.to_string() }).to_string(),
    };
    give(&mut env, &s)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consolePairStatus(mut env: Env, _: JClass, h: jlong, id: JString) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let id = string(&mut env, &id);
    let r = c.pair_status(&id).map(|s| match s {
        PairStatus::Pending => serde_json::json!({ "status": "pending" }),
        PairStatus::Refused => serde_json::json!({ "status": "refused" }),
        PairStatus::Expired => serde_json::json!({ "status": "expired" }),
        PairStatus::Accepted { username, password } => serde_json::json!({ "status": "accepted", "username": username, "password": password }),
    });
    let s = envelope(r);
    give(&mut env, &s)
}

// MARK: - Station (rubrique « Station » des réglages d'un ordinateur)

/// Réponse d'un appel de la Station : `{"code": 0, "value": …}`, un refus
/// `{"code": 10…15, "error", "kind", "device", "seconds"}` (10 appareil en
/// session : refaire l'appel avec `force` coupe sa session ; 11 trop tôt ; 12
/// rien à installer depuis un appareil ; 13 installateur requis, sur
/// l'ordinateur ; 14 installation ou redémarrage déjà en cours ; 15 Station
/// 0.4.9 ou plus ancienne), ou les codes des autres appels.
fn station_envelope(r: Result<serde_json::Value, StationError>) -> String {
    let e = match r {
        Ok(v) => return serde_json::json!({ "code": 0, "value": v }).to_string(),
        Err(StationError::Other(e)) => return envelope::<()>(Err(e)),
        Err(e) => e,
    };
    let code = match &e {
        StationError::InSession { .. } => 10,
        StationError::TooSoon { .. } => 11,
        StationError::NothingToInstall { .. } => 12,
        StationError::RequiresSetup { .. } => 13,
        StationError::Busy { .. } => 14,
        StationError::Unsupported | StationError::Other(_) => 15,
    };
    let mut v = e.to_json();
    v["code"] = serde_json::json!(code);
    v["error"] = serde_json::json!(e.to_string());
    v.to_string()
}

/// Redémarre la Station entière (Kyber et Admin, environ une minute) ;
/// `device_id` : cet appareil (nommé dans le fil d'Admin), `force` : même si
/// un appareil est en session. `value` : `{"ok": true}`. Bloquant (15 s au plus).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleStationRestart(
    mut env: Env,
    _: JClass,
    h: jlong,
    device_id: JString,
    force: jboolean,
) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let device_id = optional(&mut env, &device_id);
    let r = c.station_restart(device_id.as_deref(), force != JNI_FALSE).map(|()| serde_json::json!({ "ok": true }));
    let s = station_envelope(r);
    give(&mut env, &s)
}

/// Lance la mise à jour de la Station (comme « Installer maintenant » dans
/// Admin). `value` : `{"version": "…"}`. Bloque le temps qu'Admin télécharge
/// et vérifie le paquet (quelques minutes au plus).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleStationUpdate(
    mut env: Env,
    _: JClass,
    h: jlong,
    device_id: JString,
    force: jboolean,
) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let device_id = optional(&mut env, &device_id);
    let r = c.station_update(device_id.as_deref(), force != JNI_FALSE).map(|version| serde_json::json!({ "version": version }));
    let s = station_envelope(r);
    give(&mut env, &s)
}

/// Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_consoleAdminUrl(mut env: Env, _: JClass, h: jlong) -> jstring {
    let Some(c) = console(h) else { return std::ptr::null_mut() };
    let url = c.admin_url();
    give(&mut env, &url)
}

// MARK: - Réveil par le réseau

/// Réveille un ordinateur (Wake-on-LAN) : `targets_json`, les cartes gardées
/// par l'app pour cet ordinateur (`wake` de `consoleInfo`, une liste ; une
/// cible seule, par exemple `{"mac": "…", "ipv4": "…"}` d'une adresse saisie,
/// passe aussi). Le paquet magique part vers la diffusion du sous-réseau de
/// chaque carte, vers 255.255.255.255 et vers sa dernière adresse IPv4, en
/// direct. Rend 0 dès qu'un paquet est parti, 20 sans cible lisible, 1 si
/// aucun n'est parti. Rapide, mais hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_wake(mut env: Env, _: JClass, targets_json: JString) -> jint {
    let json = string(&mut env, &targets_json);
    match wake::send(&wake::targets_from_json(&json)) {
        Ok(_) => 0,
        Err(wake::WakeError::NoTarget) => 20,
        Err(e) => {
            log::warn!("réveil : {e}");
            1
        }
    }
}

// MARK: - Session

pub(crate) fn handle<'a>(h: jlong) -> Option<&'a Handle> {
    (h != 0).then(|| unsafe { &*(h as *const Handle) })
}

/// Démarre une session (JSON des paramètres : voir `session::start`) ; 0 si
/// les paramètres sont illisibles (l'erreur part au journal). `surface` : la
/// surface de la `SurfaceView` (nulle tant qu'elle n'existe pas),
/// `refresh` : fréquence de l'écran de l'appareil.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionStart(
    mut env: Env,
    _: JClass,
    params: JString,
    surface: JObject,
    listener: JObject,
    refresh: jint,
) -> jlong {
    let params = string(&mut env, &params);
    let w = window(&mut env, &surface);
    let Ok(listener) = Listener::new(&mut env, &listener) else { return 0 };
    match session::start(&params, listener, w, refresh.max(0) as u32) {
        Ok(h) => Box::into_raw(Box::new(h)) as jlong,
        Err(e) => {
            log::error!("session : paramètres illisibles ({e})");
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionStop(_: Env, _: JClass, h: jlong) {
    if let Some(h) = handle(h) {
        h.session.stop();
    }
}

/// Libère la session une fois son fil terminé (attente faite ici, hors du
/// fil principal de préférence).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionFree(_: Env, _: JClass, h: jlong) {
    if h == 0 {
        return;
    }
    let h = unsafe { Box::from_raw(h as *mut Handle) };
    h.session.stop();
    h.session.wait();
    h.shared.set_window(None);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetSurface(mut env: Env, _: JClass, h: jlong, surface: JObject, refresh: jint) {
    let Some(h) = handle(h) else { return };
    let w = window(&mut env, &surface);
    if refresh > 0 {
        h.shared.set_device_refresh(refresh as u32);
    }
    h.shared.set_window(w);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetSettings(mut env: Env, _: JClass, h: jlong, json: JString) -> jboolean {
    let Some(h) = handle(h) else { return JNI_FALSE };
    let json = string(&mut env, &json);
    match session::parse_settings(&json) {
        Ok(s) => {
            h.session.set_settings(s);
            JNI_TRUE
        }
        Err(e) => {
            log::warn!("réglages illisibles ({e})");
            JNI_FALSE
        }
    }
}

/// Arrière-plan : débit plafonné (bit/s, 0 le retire) et image figée.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetBackground(_: Env, _: JClass, h: jlong, background: jboolean, cap: jint) {
    let Some(h) = handle(h) else { return };
    let bg = background != 0;
    if bg && let Some(p) = h.shared.current_input() {
        p.release_all();
    }
    h.session.set_bitrate_cap((bg && cap > 0).then_some(cap as u32));
    h.session.set_hidden(bg);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionPickDisplay(_: Env, _: JClass, h: jlong, id: jint) {
    if let Some(h) = handle(h) {
        h.session.pick_display(id as u32);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionSetMode(_: Env, _: JClass, h: jlong, w: jint, hh: jint, refresh: jint) {
    if let Some(h) = handle(h) {
        h.session.set_mode(Mode { width: w as u32, height: hh as u32, refresh: refresh as u32 });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionRestoreMode(_: Env, _: JClass, h: jlong) {
    if let Some(h) = handle(h) {
        h.session.restore_mode();
    }
}

/// Latence de la session : [médiane, p95] en ms, ou nul sans mesure.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_sessionLatency<'a>(env: Env<'a>, _: JClass, h: jlong) -> jni::objects::JDoubleArray<'a> {
    let Some(h) = handle(h) else { return Default::default() };
    let (Some(med), p95) = h.session.latency().session() else { return Default::default() };
    let v = [med, p95.unwrap_or(med)];
    match env.new_double_array(2) {
        Ok(a) => {
            let _ = env.set_double_array_region(&a, 0, &v);
            a
        }
        Err(_) => Default::default(),
    }
}

// MARK: - Entrées

fn input(h: jlong, f: impl FnOnce(&kybernow_core::input::InputPipeline)) {
    if let Some(p) = handle(h).and_then(|h| h.shared.current_input()) {
        f(&p);
    }
}

unsafe extern "C" {
    /// libkynput (Android) : code de touche Linux (evdev, `KeyEvent.getScanCode()`) → scancode Windows.
    fn kynput_keycode_evdev_to_kyber(keycode: u16, out_scancode: *mut u16) -> bool;
}

/// Scancode Windows d'un code de touche Linux ; −1 : inconnu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_scancode(_: Env, _: JClass, evdev: jint) -> jint {
    let mut sc: u16 = 0;
    if evdev > 0 && evdev < 0xFFFF && unsafe { kynput_keycode_evdev_to_kyber(evdev as u16, &mut sc) } { sc as jint } else { -1 }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputKey(_: Env, _: JClass, h: jlong, scancode: jint, pressed: jboolean) {
    input(h, |p| p.key(scancode as u16, pressed != 0));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputText(mut env: Env, _: JClass, h: jlong, text: JString) {
    let s = string(&mut env, &text);
    input(h, |p| p.type_text(&s));
}

/// Pointeur tenu par l'appareil déplacé (pixels de l'écran de l'ordinateur).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_pointerMoveBy(_: Env, _: JClass, h: jlong, dx: jdouble, dy: jdouble) {
    if let Some(h) = handle(h) {
        h.shared.pointer.move_by(dx, dy);
    }
}

/// Pointeur placé à un endroit de l'écran de l'ordinateur (toucher direct).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_pointerMoveTo(_: Env, _: JClass, h: jlong, x: jdouble, y: jdouble) {
    let Some(h) = handle(h) else { return };
    let host = h.shared.pointer.host_size();
    let (cx, cy) = h.shared.pointer.state().position().unwrap_or((host.0 / 2.0, host.1 / 2.0));
    h.shared.pointer.move_by(x - cx, y - cy);
}

/// Souris relative (jeux, pointeur capturé).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputMouseMove(_: Env, _: JClass, h: jlong, dx: jint, dy: jint) {
    let clamp = |v: jint| v.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    input(h, |p| p.mouse_move(clamp(dx), clamp(dy)));
}

/// `button` : 0 gauche, 1 droit, 2 milieu, 3 précédent, 4 suivant.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputMouseButton(_: Env, _: JClass, h: jlong, button: jint, pressed: jboolean) {
    let b = match button {
        1 => MouseButtonType::Right,
        2 => MouseButtonType::Middle,
        3 => MouseButtonType::Side,
        4 => MouseButtonType::Extra,
        _ => MouseButtonType::Left,
    };
    input(h, |p| p.mouse_button(b, pressed != 0));
}

/// Molette, en crans (positif : vers le bas, vers la droite).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputWheel(_: Env, _: JClass, h: jlong, dx: jfloat, dy: jfloat) {
    input(h, |p| p.wheel(dx, dy));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputSetRelative(_: Env, _: JClass, h: jlong, on: jboolean) {
    input(h, |p| p.set_relative(on != 0));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inputReleaseAll(_: Env, _: JClass, h: jlong) {
    input(h, |p| p.release_all());
}

/// Souris relative voulue (règles Auto / Bureau / Jeu, sans exiger le pointeur
/// caché de l'ordinateur, comme sur l'Apple TV). `mode` : 0 Auto, 1 Bureau, 2 Jeu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_mouseWantsRelative(
    _: Env,
    _: JClass,
    mode: jint,
    game_fullscreen: jboolean,
    remote_hidden: jboolean,
    visible_for_ms: jdouble,
    relative_active: jboolean,
) -> jboolean {
    let mode = match mode {
        1 => MouseMode::Desktop,
        2 => MouseMode::Game,
        _ => MouseMode::Auto,
    };
    jbool(MouseModeRules { needs_hidden_pointer: false }.wants_relative(
        mode,
        game_fullscreen != 0,
        remote_hidden != 0,
        Duration::from_secs_f64(visible_for_ms.max(0.0) / 1000.0),
        relative_active != 0,
    ))
}

// MARK: - Manettes

/// Boutons dans l'ordre de `PadButton::ALL` : A, B, X, Y, Back, Guide,
/// Start, stick gauche, stick droit, L1, R1, haut, bas, gauche, droite.
fn button(i: jint) -> Option<PadButton> {
    PadButton::ALL.get(usize::try_from(i).ok()?).copied()
}

/// Axes dans l'ordre de `PadAxis::ALL` : gauche X, Y, droite X, Y, L2, R2.
fn axis(i: jint) -> Option<PadAxis> {
    PadAxis::ALL.get(usize::try_from(i).ok()?).copied()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padPlug(_: Env, _: JClass, h: jlong, pad: jint, vendor: jint, product: jint) {
    input(h, |p| p.gamepad_plug(pad as u32, u16::try_from(vendor).ok(), u16::try_from(product).ok()));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padUnplug(_: Env, _: JClass, h: jlong, pad: jint) {
    input(h, |p| p.gamepad_unplug(pad as u32));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padButton(_: Env, _: JClass, h: jlong, pad: jint, b: jint, pressed: jboolean) {
    let Some(b) = button(b) else { return };
    input(h, |p| p.gamepad_button(pad as u32, b, pressed != 0));
}

/// `value` : −32768 à 32767 pour les sticks (Y positif vers le bas), 0 à 32767 pour les gâchettes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padAxis(_: Env, _: JClass, h: jlong, pad: jint, a: jint, value: jint) {
    let Some(a) = axis(a) else { return };
    input(h, |p| p.gamepad_axis(pad as u32, a, value.clamp(i16::MIN as i32, i16::MAX as i32) as i16));
}

/// Manette rendue au jeu ou prise par le menu : sticks et gâchettes au repos.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padAxesRest(_: Env, _: JClass, h: jlong, pad: jint) {
    input(h, |p| p.gamepad_axes_rest(pad as u32));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_padSetComboEnabled(_: Env, _: JClass, h: jlong, enabled: jboolean) {
    let Some(h) = handle(h) else { return };
    h.shared.combo_enabled.store(enabled != 0, std::sync::atomic::Ordering::Relaxed);
    if let Some(p) = h.shared.current_input() {
        p.combo().set_enabled(enabled != 0);
    }
}

// MARK: - Profils

/// Réglages d'un profil (`jeu`, `bureau`, `max`) ramenés à ce que l'appareil
/// décode en matériel, dans le JSON de `settings` : la même règle que les
/// autres apps (`StreamSettings::for_profile`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_profileSettings(
    mut env: Env,
    _: JClass,
    profile: JString,
    hevc: jboolean,
    hevc444: jboolean,
    av1: jboolean,
) -> jstring {
    use kybernow_core::profile::{Capabilities, Codec, Profile, StreamSettings};
    let p = string(&mut env, &profile);
    let profile = Profile::parse(&p).unwrap_or(Profile::Jeu);
    let s = StreamSettings::for_profile(profile, Capabilities { hevc: hevc != 0, hevc444: hevc444 != 0, av1: av1 != 0 });
    let codec = match s.codec {
        Codec::H264 => "h264",
        Codec::Hevc => "hevc",
        Codec::Av1 => "av1",
    };
    let json = serde_json::json!({
        "profile": p.to_lowercase(),
        "codec": codec,
        "yuv444": s.yuv444,
        "mbps": s.mbps,
        "reliable": s.reliable,
    })
    .to_string();
    give(&mut env, &json)
}
