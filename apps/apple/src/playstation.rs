//! Consoles PlayStation pour les apps Apple (docs/playstation.md) :
//! recherche, identifiant du compte PSN, association, réveil et partie.
//! L'image et le son passent par les mêmes rappels que les sessions Kyber
//! (`KnPacketCb` : décodeur VideoToolbox et moteur de son).
//!
//! Les consoles associées sont gardées par l'app telles que
//! `kn_ps_regist` les rend (JSON stable de `Registered`).
//!
//! **Adresse d'une console** (`address` des fonctions ci-dessous) : une
//! adresse IPv4 sur le réseau (ou par un VPN), ou, hors de chez soi, ce que
//! rend `kn_ps_remote_address` (un JSON : la boucle locale et les ports du
//! tunnel vers une Station qui voit la console ; docs/relais.md, « Consoles
//! PlayStation »).

use std::ffi::{c_char, c_void};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::media::PacketHandler;
use kybernow_core::playstation::discovery::{self, Console, Model, State};
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::remote::{Remote, RemoteError};
use kybernow_core::playstation::keymouse::Layout;
use kybernow_core::input::PadKey;
use kybernow_core::playstation::dualsense;
use kybernow_core::playstation::play::{KeyboardEvent, PadEvent, Play, PlayEnd, PlayOptions, PlayState};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered};
use kybernow_core::playstation::settings::PlaySettings;
use serde_json::json;

use crate::{KN_ERROR, KN_OK, KnConsole, KnPacketCb, Sink, console_result, failure, give, give_to, text};

/// Numéros de flux des parties : loin de ceux des sessions Kyber.
static STREAMS: AtomicU64 = AtomicU64::new(1 << 40);

/// Une adresse IPv4, ou l'adresse par le relais de `kn_ps_remote_address`.
fn parse_addr(p: *const c_char) -> Option<Endpoint> {
    Endpoint::parse(&text(p))
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
    give(found_json(&found))
}

/// Cherche une seule console à `address` (sans diffusion), pendant
/// `wait_ms` : par le relais, l'état de la console à travers la Station (le
/// JSON de `kn_ps_search` ; `address` y vaut alors `127.0.0.1`, à ne pas
/// retenir). Bloque.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_search_at(address: *const c_char, wait_ms: u32) -> *mut c_char {
    let Some(at) = parse_addr(address) else { return give("[]".into()) };
    let found = discovery::search_at(Duration::from_millis(wait_ms.clamp(200, 10_000) as u64), false, &[at]).unwrap_or_default();
    give(found_json(&found))
}

// MARK: - Hors de chez soi (docs/relais.md, « Consoles PlayStation »)

/// La Station ne voit pas (ou plus) cette console sur son réseau.
pub const KN_PS_NOT_SEEN: i32 = 40;

/// `GET /api/consoles` : les consoles que voit la Station de cette console
/// d'Admin (sur le réseau, ou par le relais avec `kn_console_new_relay`), en
/// JSON `[{host_id, name, model ("ps5"|"ps4"), state ("ready"|"standby"), seen}]`.
/// L'app retient, par `host_id`, les Stations qui voient chaque console.
/// `−404` : Station d'avant les consoles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_consoles(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.consoles(), out, error)
}

/// Une console jointe par une Station et le relais (opaque pour Swift).
pub struct KnPsRemote {
    remote: Remote,
}

/// Joint la console `host_id` par la Station de `c` (une console d'Admin
/// **par le relais**, `kn_console_new_relay`) : signe de vie à Admin, puis
/// les ports du tunnel ; le signe de vie repart toutes les 10 s tant que
/// `out` vit (à libérer par `kn_ps_remote_free`). Codes : `KN_OK`,
/// `KN_RELAY_*` (relais muet, Station absente, accès coupé), `−401`,
/// `−404` (Station d'avant les consoles), `KN_PS_NOT_SEEN`. Bloque.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_remote_open(c: *const KnConsole, host_id: *const c_char, out: *mut *mut KnPsRemote, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    match Remote::open(&c.0, &text(host_id)) {
        Ok(remote) => {
            if !out.is_null() {
                unsafe { *out = Box::into_raw(Box::new(KnPsRemote { remote })) };
            }
            KN_OK
        }
        Err(RemoteError::NotSeen) => {
            give_to(error, RemoteError::NotSeen.to_string());
            KN_PS_NOT_SEEN
        }
        Err(RemoteError::NotRelayed) => {
            give_to(error, RemoteError::NotRelayed.to_string());
            KN_ERROR
        }
        Err(RemoteError::Console(e)) => failure(e, error),
    }
}

/// L'adresse de la console par le relais, à passer aux autres `kn_ps_*`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_remote_address(r: *const KnPsRemote) -> *mut c_char {
    match unsafe { r.as_ref() } {
        Some(r) => give(r.remote.endpoint().to_text()),
        None => std::ptr::null_mut(),
    }
}

/// L'état de la console vu par la Station au dernier signe de vie : `ready`,
/// `standby`, `unknown` ; nul si elle ne la voit plus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_remote_state(r: *const KnPsRemote) -> *mut c_char {
    match unsafe { r.as_ref() }.and_then(|r| r.remote.seen()) {
        Some(c) => give(c.state),
        None => std::ptr::null_mut(),
    }
}

/// Le chemin (JSON de `on_route` : `transport`, `relay`, `rtt_client`,
/// `rtt_station`) : puce « Par le relais », ligne « Chemin » des statistiques.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_remote_route(r: *const KnPsRemote) -> *mut c_char {
    match unsafe { r.as_ref() } {
        Some(r) => give(serde_json::to_string(&r.remote.route_info()).unwrap_or_default()),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_remote_free(r: *mut KnPsRemote) {
    if !r.is_null() {
        drop(unsafe { Box::from_raw(r) });
    }
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
/// "<base64>", "online_id": "<pseudo>" | null}` ou `{"error": "<raison,
/// pour le journal>"}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_account(redirect_url: *const c_char) -> *mut c_char {
    let url = text(redirect_url);
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
    match regist::regist_at(&addr, target.into(), &id, pin) {
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
    discovery::wake_at(&addr, model, cred).is_ok()
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

/// Clavier de la console (`on_keyboard`) : ouvert, avec le texte déjà dans
/// son champ ; texte du champ renvoyé par la console (il remplace celui de
/// l'app) ; fermé par la console (ou partie interrompue), texte vide.
pub const KN_PS_KEYBOARD_OPEN: i32 = 0;
pub const KN_PS_KEYBOARD_TEXT: i32 = 1;
pub const KN_PS_KEYBOARD_CLOSED: i32 = 2;

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
    /// Options + Create + L1 + R1 tenus 250 ms : ouvrir la barre ou le menu
    /// (fil de la manette).
    pub on_menu: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    /// Barre ouverte (`kn_ps_set_captured`) : touche de navigation de la
    /// manette (`KN_PAD_KEY_*`, comme les sessions d'ordinateur).
    pub on_navigate: Option<unsafe extern "C" fn(userdata: *mut c_void, key: u32)>,
    /// Clavier de la console (`KN_PS_KEYBOARD_*`) ; `text` en UTF-8, valable
    /// pendant l'appel (fil de la partie). Ajouté en dernier : une app qui
    /// remplit la structure à zéro sans le connaître n'en reçoit rien.
    pub on_keyboard: Option<unsafe extern "C" fn(userdata: *mut c_void, event: i32, text: *const c_char)>,
}

unsafe impl Send for KnPsCallbacks {}
unsafe impl Sync for KnPsCallbacks {}

/// Une partie en cours (opaque pour Swift).
pub struct KnPsPlay {
    play: Play,
}

/// Lance une partie : la console doit être allumée (`kn_ps_wake` sinon).
/// `settings_json` : les réglages de la console (`kn_ps_default_settings`,
/// tels que l'app les garde ; vide ou illisible : Auto), HDR compris ;
/// `hevc_decoder` : l'appareil décode le H.265 (Auto en profite). À terminer
/// par `kn_ps_stop`, qui libère.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_start(
    address: *const c_char,
    registered_json: *const c_char,
    settings_json: *const c_char,
    hevc_decoder: bool,
    cb: *const KnPsCallbacks,
) -> *mut KnPsPlay {
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
    let settings = serde_json::from_str::<PlaySettings>(&text(settings_json)).unwrap_or_default();
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
    let on_pad = Box::new(move |e: PadEvent| {
        let cb: KnPsCallbacks = cb;
        match e {
            PadEvent::Menu => {
                if let Some(f) = cb.on_menu {
                    unsafe { f(cb.userdata) };
                }
            }
            PadEvent::Navigate(k) => {
                if let Some(f) = cb.on_navigate {
                    let key = match k {
                        PadKey::Left => crate::input::KN_PAD_KEY_LEFT,
                        PadKey::Right => crate::input::KN_PAD_KEY_RIGHT,
                        PadKey::Up => crate::input::KN_PAD_KEY_UP,
                        PadKey::Down => crate::input::KN_PAD_KEY_DOWN,
                        PadKey::Accept => crate::input::KN_PAD_KEY_ACCEPT,
                        PadKey::Back => crate::input::KN_PAD_KEY_BACK,
                    };
                    unsafe { f(cb.userdata, key) };
                }
            }
        }
    });
    let play = Play::start(addr, &r, PlayOptions { settings, hevc_decoder }, video, audio, on_state, on_pad);
    if cb.on_keyboard.is_some() {
        play.set_on_keyboard(Some(Box::new(move |e: KeyboardEvent| {
            let cb: KnPsCallbacks = cb;
            let Some(f) = cb.on_keyboard else { return };
            let (code, t) = match e {
                KeyboardEvent::Open(t) => (KN_PS_KEYBOARD_OPEN, t),
                KeyboardEvent::Text(t) => (KN_PS_KEYBOARD_TEXT, t),
                KeyboardEvent::Closed => (KN_PS_KEYBOARD_CLOSED, String::new()),
            };
            let c = std::ffi::CString::new(t.replace('\0', "")).unwrap_or_default();
            unsafe { f(cb.userdata, code, c.as_ptr()) };
        })));
    }
    Box::into_raw(Box::new(KnPsPlay { play }))
}

/// Le décodeur a perdu sa référence : une image clé est demandée à la
/// console, de nouveau toutes les 500 ms jusqu'à `kn_ps_keyframe_recovered`
/// ou l'arrivée d'une image clé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_keyframe_lost(play: *mut KnPsPlay) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.key_frame_lost();
    }
}

/// Le décodeur a retrouvé une image clé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_keyframe_recovered(play: *mut KnPsPlay) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.key_frame_recovered();
    }
}

/// Touche du clavier (usage USB HID, la place de la touche : table kVK →
/// HID de l'app), traduite en manette d'après la disposition de la console.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_key(play: *mut KnPsPlay, usage: u16, pressed: bool) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.key(usage, pressed);
    }
}

/// Bouton de la souris : 0 gauche, 1 droit, 2 milieu, 3 et 4 latéraux.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_mouse_button(play: *mut KnPsPlay, button: u8, pressed: bool) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.mouse_button(button, pressed);
    }
}

/// Déplacement brut de la souris capturée (comptes, y vers le bas).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_mouse_move(play: *mut KnPsPlay, dx: f32, dy: f32) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.mouse_move(dx, dy);
    }
}

/// Tout relâcher : fenêtre qui perd le clavier, souris rendue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_release_all(play: *mut KnPsPlay) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.release_all();
    }
}

/// Intensités de la DualSense, de 0 à 100 % (vibrations HD et simples,
/// gâchettes adaptatives : 0 les rend libres), tout de suite pendant la
/// partie.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_set_intensities(play: *mut KnPsPlay, rumble: u8, triggers: u8) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.set_intensities(rumble, triggers);
    }
}

/// Clavier de la console ouvert : le texte entier de son champ (UTF-8), à
/// chaque changement. Faux s'il n'est pas parti (pas de canal de contrôle).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_keyboard_text(play: *mut KnPsPlay, utf8: *const c_char) -> bool {
    let Some(p) = (unsafe { play.as_ref() }) else { return false };
    p.play.keyboard_text(&text(utf8))
}

/// Entrée : le texte du clavier de la console est validé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_keyboard_accept(play: *mut KnPsPlay) -> bool {
    unsafe { play.as_ref() }.is_some_and(|p| p.play.keyboard_accept())
}

/// Échap : le texte du clavier de la console est refusé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_keyboard_reject(play: *mut KnPsPlay) -> bool {
    unsafe { play.as_ref() }.is_some_and(|p| p.play.keyboard_reject())
}

/// Vibrations HD : l'app a ouvert (`true`) ou fermé la carte son d'une
/// DualSense branchée en USB. Ouverte, les vibrations HD de la partie y vont
/// (`kn_ps_haptics_fill`) ; fermée, elles sont traduites en vibrations
/// simples. Une seule sortie pour l'app.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_haptics_set_output(on: bool) {
    dualsense::hd_haptics().set_output(on);
}

/// Fil temps réel de la carte son de la DualSense : `frames` images de
/// `channels` voies (flottants entrelacés, 48 kHz) dans `out`, les
/// vibrations (gauche, droite) sur les voies `first` et `first + 1` (2 et 3
/// pour les voies 3 et 4 d'une DualSense), du silence ailleurs. Sans verrou.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_haptics_fill(out: *mut f32, frames: usize, channels: usize, first: usize) {
    if out.is_null() || frames == 0 || channels == 0 {
        return;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out, frames * channels) };
    dualsense::hd_haptics().fill(out, frames, channels, first);
}

/// Les réglages par défaut d'une console (JSON, à libérer) : Auto, 60 i/s,
/// plein écran.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_default_settings() -> *mut c_char {
    give(serde_json::to_string(&PlaySettings::default()).unwrap_or_default())
}

/// Vrai si les réglages s'écartent d'Auto (« Personnalisé » à l'accueil).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_settings_custom(settings_json: *const c_char) -> bool {
    serde_json::from_str::<PlaySettings>(&text(settings_json)).is_ok_and(|s| s.is_custom())
}

/// La disposition par défaut du clavier et de la souris (JSON, à libérer).
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_default_layout() -> *mut c_char {
    give(serde_json::to_string(&Layout::default()).unwrap_or_default())
}

/// Pose la disposition de la console (JSON de `kn_ps_default_layout`, tel
/// que l'app le garde). Faux si le JSON est illisible (disposition gardée).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_set_layout(play: *mut KnPsPlay, layout_json: *const c_char) -> bool {
    let (Some(p), Ok(layout)) = (unsafe { play.as_ref() }, serde_json::from_str::<Layout>(&text(layout_json))) else {
        return false;
    };
    p.play.set_layout(layout);
    true
}

/// La barre ou le menu de l'app s'ouvre (`true`) ou se ferme : tant qu'ils
/// sont ouverts, la console reçoit une manette au repos et la manette
/// navigue dans l'app (`on_navigate`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_set_captured(play: *mut KnPsPlay, on: bool) {
    if let Some(p) = unsafe { play.as_ref() } {
        p.play.set_captured(on);
    }
}

/// Statistiques depuis l'appel précédent (une fois par seconde), JSON à
/// libérer : `width`, `height`, `hevc`, `hdr`, `target_kbps`, `fps`,
/// `mbps`, `lost`, `recovered`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_stats(play: *mut KnPsPlay) -> *mut c_char {
    let Some(p) = (unsafe { play.as_ref() }) else { return std::ptr::null_mut() };
    give(serde_json::to_string(&p.play.stats()).unwrap_or_default())
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

// MARK: - Envoi entre appareils (docs/playstation.md, « Plusieurs appareils »)

/// Un appareil qui attend un envoi (`kn_ps_handoff_receive`).
pub struct KnPsHandoff {
    receiver: kybernow_core::playstation::handoff::Receiver,
}

/// Commence à attendre un envoi : `name` (nom de l'appareil), `platform`
/// (`macos`, `ios`, `tvos`). Nul si l'écoute est impossible ; à libérer
/// par `kn_ps_handoff_stop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_receive(name: *const c_char, platform: *const c_char) -> *mut KnPsHandoff {
    match kybernow_core::playstation::handoff::Receiver::start(&text(name), &text(platform)) {
        Ok(receiver) => Box::into_raw(Box::new(KnPsHandoff { receiver })),
        Err(e) => {
            log::warn!("PlayStation : attente d'un envoi impossible ({e})");
            std::ptr::null_mut()
        }
    }
}

/// Le code à 6 chiffres à afficher (texte à libérer).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_code(h: *mut KnPsHandoff) -> *mut c_char {
    let Some(h) = (unsafe { h.as_ref() }) else { return std::ptr::null_mut() };
    give(h.receiver.code())
}

/// Le prochain évènement, attendu `timeout_ms` au plus (bloque : hors du fil
/// de l'interface). JSON à libérer : `{"event":"received","bundle":"<JSON>"}`,
/// `{"event":"new_code","code":"123456"}`, `{"event":"expired"}`, ou nul
/// s'il n'y a rien.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_next(h: *mut KnPsHandoff, timeout_ms: u32) -> *mut c_char {
    use kybernow_core::playstation::handoff::Event;
    let Some(h) = (unsafe { h.as_ref() }) else { return std::ptr::null_mut() };
    match h.receiver.next(Duration::from_millis(timeout_ms as u64)) {
        Some(Event::Received(bundle)) => give(json!({ "event": "received", "bundle": bundle }).to_string()),
        Some(Event::NewCode(code)) => give(json!({ "event": "new_code", "code": code }).to_string()),
        Some(Event::Expired) => give(json!({ "event": "expired" }).to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Arrête l'attente et la libère.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_stop(h: *mut KnPsHandoff) {
    if !h.is_null() {
        drop(unsafe { Box::from_raw(h) });
    }
}

/// Les appareils du réseau qui attendent un envoi, cherchés `wait_ms`
/// (bloque). JSON à libérer : `[{"name","platform","address":"ip:port"}]`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_discover(wait_ms: u32) -> *mut c_char {
    let list = kybernow_core::playstation::handoff::discover(Duration::from_millis(wait_ms as u64), &[]).unwrap_or_default();
    let v: Vec<_> = list
        .iter()
        .map(|w| json!({ "name": w.name, "platform": w.platform, "address": w.addr.to_string() }))
        .collect();
    give(serde_json::Value::Array(v).to_string())
}

/// Envoie le paquet (JSON du contrat) à `address` (`ip:port` de
/// `kn_ps_handoff_discover`) avec le code tapé (bloque quelques secondes).
/// JSON à libérer : `{"ok":true}` ou `{"error":"refused"|"unreachable","detail":"…"}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_ps_handoff_send(address: *const c_char, code: *const c_char, bundle: *const c_char) -> *mut c_char {
    use kybernow_core::playstation::handoff::{SendError, send};
    let Ok(to) = text(address).parse::<std::net::SocketAddr>() else {
        return give(json!({ "error": "unreachable", "detail": "adresse illisible" }).to_string());
    };
    match send(to, &text(code), &text(bundle)) {
        Ok(()) => give(json!({ "ok": true }).to_string()),
        Err(SendError::Refused) => give(json!({ "error": "refused", "detail": "" }).to_string()),
        Err(SendError::Unreachable(d)) => give(json!({ "error": "unreachable", "detail": d }).to_string()),
    }
}

// MARK: - Manettes lues par l'app (Apple TV, iPhone, iPad)

/// Une manette branchée, lue par l'app (GameController) : pour une partie
/// sur console, le lecteur de manettes du cœur la reçoit de l'app
/// (`vendor_id`, `product_id` : identifiants USB, -1 inconnus ; une DualSense
/// se dit 054C:0CE6 pour ses gâchettes adaptatives). Les sessions
/// d'ordinateur gardent `kn_input_gamepad_*`.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_plug(pad: u32, vendor_id: i32, product_id: i32) {
    kybernow_core::gamepads::plug(pad, u16::try_from(vendor_id).ok(), u16::try_from(product_id).ok());
}

#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_unplug(pad: u32) {
    kybernow_core::gamepads::unplug(pad);
}

/// `button` : valeur kynput (`KYNPUT_GAMEPAD_BUTTON_*`, comme `kn_input_gamepad_button`).
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_button(pad: u32, button: u32, pressed: bool) {
    if let Some(b) = kybernow_core::input::PadButton::from_u32(button) {
        kybernow_core::gamepads::button(pad, b, pressed);
    }
}

/// `axis` : valeur kynput (`KYNPUT_GAMEPAD_AXIS_*`) ; sticks de -32768 à
/// 32767 (vers le bas : positif), gâchettes de 0 à 32767.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_axis(pad: u32, axis: u32, value: i16) {
    if let Some(a) = kybernow_core::input::PadAxis::from_u32(axis) {
        kybernow_core::gamepads::axis(pad, a, value);
    }
}

/// Clic du pavé tactile.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_touch_click(pad: u32, pressed: bool) {
    kybernow_core::gamepads::touch_click(pad, pressed);
}

/// Doigt `finger` (0 ou 1) posé en (`x`, `y`), de 0 à 1 depuis le coin haut
/// gauche du pavé, ou levé (`down` faux).
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_touch(pad: u32, finger: u32, down: bool, x: f32, y: f32) {
    let at = down.then(|| (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)));
    kybernow_core::gamepads::touch_finger(pad, finger as usize, at);
}

/// Mesure du gyroscope (`gyro` : rad/s) ou de l'accéléromètre (m/s²), dans
/// le repère de SDL ; `timestamp_us` croissant.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_sensor(pad: u32, gyro: bool, x: f32, y: f32, z: f32, timestamp_us: u64) {
    kybernow_core::gamepads::sensor(pad, gyro, [x, y, z], timestamp_us);
}

/// Retour d'une partie vers une manette lue par l'app : vibration (`effect`
/// nul ; moteurs de 0 à 65535, `duration_ms` : 0 arrête), ou rapport
/// d'effets de la DualSense (`effect` : 47 octets, `DS5EffectsState_t` de
/// SDL, valables pendant l'appel ; gâchettes adaptatives). Fils de la partie.
pub type KnPsPadFeedback =
    Option<unsafe extern "C" fn(userdata: *mut c_void, pad: u32, large: u16, small: u16, duration_ms: i32, effect: *const u8, effect_len: usize)>;

/// Pose (ou retire, `cb` nul) le rappel des vibrations et des gâchettes
/// adaptatives des parties sur console.
#[unsafe(no_mangle)]
pub extern "C" fn kn_ps_pad_set_feedback(cb: KnPsPadFeedback, userdata: *mut c_void) {
    use kybernow_core::gamepads::{ConsoleFeedback, set_console_feedback};
    let Some(f) = cb else {
        set_console_feedback(None);
        return;
    };
    struct Ud(*mut c_void);
    unsafe impl Send for Ud {}
    unsafe impl Sync for Ud {}
    let ud = Ud(userdata);
    set_console_feedback(Some(Arc::new(move |e: ConsoleFeedback| {
        let ud = &ud;
        match e {
            ConsoleFeedback::Rumble(r) => unsafe { f(ud.0, r.id, r.large_motor, r.small_motor, r.duration_ms, std::ptr::null(), 0) },
            ConsoleFeedback::Effect(id, data) => unsafe { f(ud.0, id, 0, 0, 0, data.as_ptr(), data.len()) },
        }
    })));
}
