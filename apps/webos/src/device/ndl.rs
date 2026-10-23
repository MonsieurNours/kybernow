//! Décodage de l'image par le téléviseur : NDL DirectMedia (API 2, webOS 5
//! et après), la bibliothèque de LG qu'utilisent moonlight-tv et punktfunk.
//! L'image décodée va sur le plan vidéo du téléviseur, sous le plan graphique
//! de l'app (transparent pendant la session).
//!
//! Ce qui compte, appris par moonlight-tv (ss4s) et punktfunk :
//! - la bibliothèque est ouverte par `dlopen`, jamais liée ;
//! - `NDL_DirectMediaInit` au premier chargement, pas au lancement ;
//! - chaque chargement porte un plan de son Opus stéréo, amorcé de trames
//!   vides : sans lui, NDL ne cale pas les images sur leur horodatage et
//!   l'image saccade au-dessus du 1080p sur une dalle à 120 Hz. Le son, lui,
//!   sort par SDL (moteur du cœur) ;
//! - un chargement avec plan de son ne se termine (`LOADCOMPLETED`, 0x16)
//!   qu'une fois ce plan nourri : une seule trame ne suffit pas à tous les
//!   téléviseurs (webOS 26 sur une G2 : image noire, sans erreur). Des
//!   trames vides de 5 ms sont donc envoyées jusqu'à la confirmation, et les
//!   images attendent (punktfunk, `ndl/v2/load.rs`) ;
//! - horodatages en millisecondes depuis le chargement, jamais en arrière ;
//! - jamais de vidage (`Flush`) en cours de session : il coupe le plan de son
//!   pour toujours ; après une perte, on attend l'image clé suivante ;
//! - aucune fonction n'est sûre entre fils : un seul verrou.

#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const VIDEO_H264: i32 = 1;
pub const VIDEO_H265: i32 = 2;
pub const VIDEO_AV1: i32 = 4;
const AUDIO_OPUS: i32 = 3;

/// Trame Opus vide, stéréo (1 flux, 1 couplé), celle de ss4s.
const OPUS_EMPTY_STEREO: [u8; 3] = [0xec, 0xff, 0xfe];
const AUDIO_PCM: i32 = 1;
/// 5 ms de silence PCM S16LE en 6 voies (240 images).
const PCM_SILENCE_51: [u8; 240 * 6 * 2] = [0; 240 * 6 * 2];

/// Le plan de son du chargement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioPlane {
    /// Opus stéréo, jamais nourri de son réel : il cale les images sur leur
    /// horodatage (le son sort par SDL).
    OpusClock,
    /// PCM S16LE 6 voies (G, D, C, LFE, arrière G, arrière D), le PCM
    /// multicanal de la TV vers l'HDMI (Sortie son numérique en Pass-Through,
    /// ampli en eARC). Le son décodé par l'app y passe.
    Pcm51,
}

static AUDIO_PLANE: AtomicU8 = AtomicU8::new(0);

/// Le plan de son des prochains chargements.
pub fn set_audio_plane(p: AudioPlane) {
    AUDIO_PLANE.store(p as u8, Ordering::Release);
}

fn wanted_plane() -> AudioPlane {
    if AUDIO_PLANE.load(Ordering::Acquire) == AudioPlane::Pcm51 as u8 { AudioPlane::Pcm51 } else { AudioPlane::OpusClock }
}

fn empty_frame(plane: AudioPlane) -> &'static [u8] {
    match plane {
        AudioPlane::Pcm51 => &PCM_SILENCE_51,
        AudioPlane::OpusClock => &OPUS_EMPTY_STEREO,
    }
}

/// Durée d'une trame vide (ms), et avance des trames sur l'horloge du chargement.
const PRIME_PACKET_MS: i64 = 5;
const PRIME_LEAD_MS: i64 = 40;
/// Attente de `LOADCOMPLETED` au chargement, puis entre deux salves.
const PRIME_BUDGET: Duration = Duration::from_millis(500);
const PRIME_RETRY: Duration = Duration::from_millis(20);
/// Sans confirmation passé ce délai, les images partent quand même.
const FEED_ANYWAY_AFTER: Duration = Duration::from_millis(2000);

/// `LOADCOMPLETED` reçu pour le chargement en cours.
static LOAD_COMPLETED: AtomicBool = AtomicBool::new(false);
/// Déchargé par le téléviseur lui-même (`UNLOADCOMPLETED` sans demande : sa
/// politique de ressources, après un arrêt du son par exemple).
static TV_UNLOADED: AtomicBool = AtomicBool::new(false);
/// Numéro du chargement en cours (le fil qui nourrit le plan de son s'arrête quand il change).
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[repr(C)]
#[derive(Clone, Copy)]
struct VideoInfo {
    width: c_int,
    height: c_int,
    kind: c_int,
    unknown1: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OpusInfo {
    kind: c_int,
    unknown1: c_int,
    channels: c_int,
    unknown2: c_int,
    sample_rate: f64,
    stream_header: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PcmInfo {
    kind: c_int,
    unknown1: c_int,
    format: *const c_char,
    layout: *const c_char,
    channel_mode: *const c_char,
    /// 1 : 48 kHz.
    sample_rate: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
union AudioInfo {
    kind: c_int,
    opus: OpusInfo,
    pcm: PcmInfo,
    padding: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DataInfo {
    video: VideoInfo,
    audio: AudioInfo,
}

type LoadCallback = extern "C" fn(c_int, i64, *const c_char);

struct Api {
    init: unsafe extern "C" fn(*const c_char, *const c_void) -> c_int,
    quit: unsafe extern "C" fn() -> c_int,
    load: unsafe extern "C" fn(*mut DataInfo, LoadCallback) -> c_int,
    unload: unsafe extern "C" fn() -> c_int,
    video_play: unsafe extern "C" fn(*const c_void, u32, i64) -> c_int,
    audio_play: unsafe extern "C" fn(*const c_void, u32, i64) -> c_int,
    error: unsafe extern "C" fn() -> *const c_char,
    render_length: Option<unsafe extern "C" fn(*mut c_int) -> c_int>,
    /// Écrit son verdict par le pointeur (punktfunk : la valeur rendue n'est pas le verdict).
    multichannel: Option<unsafe extern "C" fn(*mut c_int) -> c_int>,
}

unsafe impl Send for Api {}

struct State {
    api: Api,
    initialized: bool,
    /// Chargé : (format, largeur, hauteur), instant du chargement.
    loaded: Option<((i32, u32, u32), Instant)>,
    /// Plan de son du chargement en cours.
    plane: AudioPlane,
    last_pts: i64,
    /// Dernier horodatage donné au plan de son (jamais en arrière : NDL le couperait).
    last_audio_pts: i64,
    /// Images envoyées au décodeur depuis le chargement (après confirmation).
    feeding: bool,
}

static NDL: Mutex<Option<State>> = Mutex::new(None);

unsafe fn sym<T>(lib: *mut c_void, name: &str) -> Option<T> {
    let n = CString::new(name).ok()?;
    let p = unsafe { libc::dlsym(lib, n.as_ptr()) };
    if p.is_null() { None } else { Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) }) }
}

/// Ouvre la bibliothèque du téléviseur ; faux si elle manque (pas un
/// téléviseur LG, ou webOS trop ancien : il faut l'API 2, webOS 5 et après).
pub fn available() -> bool {
    let mut g = NDL.lock().unwrap();
    if g.is_some() {
        return true;
    }
    unsafe {
        let name = CString::new("libNDL_directmedia.so.1").unwrap();
        let lib = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if lib.is_null() {
            log::warn!("image : libNDL_directmedia.so.1 introuvable");
            return false;
        }
        let api = (|| {
            Some(Api {
                init: sym(lib, "NDL_DirectMediaInit")?,
                quit: sym(lib, "NDL_DirectMediaQuit")?,
                load: sym(lib, "NDL_DirectMediaLoad")?,
                unload: sym(lib, "NDL_DirectMediaUnload")?,
                video_play: sym(lib, "NDL_DirectVideoPlay")?,
                audio_play: sym(lib, "NDL_DirectAudioPlay")?,
                error: sym(lib, "NDL_DirectMediaGetError")?,
                render_length: sym(lib, "NDL_DirectVideoGetRenderBufferLength"),
                multichannel: sym(lib, "NDL_DirectAudioSupportMultiChannel"),
            })
        })();
        match api {
            Some(api) => {
                *g = Some(State { api, initialized: false, loaded: None, plane: AudioPlane::OpusClock, last_pts: 0, last_audio_pts: 0, feeding: false });
                true
            }
            None => {
                log::warn!("image : NDL sans l'API 2 (webOS 5 et après requis)");
                false
            }
        }
    }
}

fn last_error(api: &Api) -> String {
    unsafe {
        let p = (api.error)();
        if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}

extern "C" fn on_load(kind: c_int, value: i64, s: *const c_char) {
    let s = if s.is_null() { String::new() } else { unsafe { CStr::from_ptr(s).to_string_lossy().into_owned() } };
    match kind {
        0x16 => {
            LOAD_COMPLETED.store(true, Ordering::Release);
            log::info!("image : NDL chargé (confirmé) {s}");
        }
        0x17 => {
            if LOAD_COMPLETED.swap(false, Ordering::AcqRel) {
                TV_UNLOADED.store(true, Ordering::Release);
                log::warn!("image : NDL déchargé par le téléviseur {s}");
            } else {
                log::info!("image : NDL déchargé {s}");
            }
        }
        0x1a => log::info!("image : NDL lecture {s}"),
        _ => log::info!("image : NDL état 0x{kind:02x} ({value:#x}) {s}"),
    }
}

/// Charge le décodeur pour un format et une taille (le refait s'ils changent).
pub fn ensure(kind: i32, width: u32, height: u32) -> Result<(), String> {
    let mut g = NDL.lock().unwrap();
    let st = g.as_mut().ok_or("NDL absent")?;
    let plane = wanted_plane();
    if TV_UNLOADED.swap(false, Ordering::AcqRel) && st.loaded.take().is_some() {
        // Déjà déchargé par le téléviseur : rien à décharger, tout à recharger.
        GENERATION.fetch_add(1, Ordering::AcqRel);
    }
    if st.loaded.as_ref().is_some_and(|(k, _)| *k == (kind, width, height)) && st.plane == plane {
        return Ok(());
    }
    unsafe {
        if st.loaded.take().is_some() {
            (st.api.unload)();
        }
        if !st.initialized {
            let app = CString::new(std::env::var("APPID").unwrap_or_else(|_| crate::APP_ID.into())).unwrap();
            if (st.api.init)(app.as_ptr(), std::ptr::null()) != 0 {
                return Err(format!("NDL_DirectMediaInit : {}", last_error(&st.api)));
            }
            st.initialized = true;
        }
        static S16LE: &CStr = c"S16LE";
        static INTERLEAVED: &CStr = c"interleaved";
        static SIX: &CStr = c"6-channel";
        let load = |plane: AudioPlane| -> Result<(), String> {
            let mut info = DataInfo {
                video: VideoInfo { width: width as c_int, height: height as c_int, kind, unknown1: 0 },
                audio: AudioInfo { padding: [0; 32] },
            };
            match plane {
                AudioPlane::OpusClock => {
                    info.audio.opus = OpusInfo { kind: AUDIO_OPUS, unknown1: 0, channels: 2, unknown2: 0, sample_rate: 48.0, stream_header: std::ptr::null() };
                }
                AudioPlane::Pcm51 => {
                    info.audio.pcm = PcmInfo { kind: AUDIO_PCM, unknown1: 0, format: S16LE.as_ptr(), layout: INTERLEAVED.as_ptr(), channel_mode: SIX.as_ptr(), sample_rate: 1 };
                }
            }
            LOAD_COMPLETED.store(false, Ordering::Release);
            GENERATION.fetch_add(1, Ordering::AcqRel);
            if (st.api.load)(&mut info, on_load) != 0 {
                return Err(format!("NDL_DirectMediaLoad ({plane:?}) : {}", last_error(&st.api)));
            }
            Ok(())
        };
        let at = Instant::now();
        let plane = match load(plane).or_else(|e| {
            // Refusé (état invalide après un déchargement par le téléviseur…) :
            // le décodeur est réinitialisé, puis un nouvel essai.
            log::warn!("image : {e} ; réinitialisation de NDL");
            (st.api.unload)();
            (st.api.quit)();
            st.initialized = false;
            let app = CString::new(std::env::var("APPID").unwrap_or_else(|_| crate::APP_ID.into())).unwrap();
            if (st.api.init)(app.as_ptr(), std::ptr::null()) != 0 {
                return Err(format!("NDL_DirectMediaInit : {}", last_error(&st.api)));
            }
            st.initialized = true;
            load(plane)
        }) {
            Ok(()) => plane,
            Err(e) if plane != AudioPlane::OpusClock => {
                // Le plan refusé : l'image d'abord, le son repassera par SDL.
                log::warn!("son : {e} ; plan Opus stéréo à la place");
                load(AudioPlane::OpusClock)?;
                AudioPlane::OpusClock
            }
            Err(e) => return Err(e),
        };
        st.loaded = Some(((kind, width, height), at));
        st.plane = plane;
        st.last_pts = 0;
        st.last_audio_pts = 0;
        st.feeding = false;
        log::info!("image : NDL chargé ({}, {width} × {height}, son {plane:?})", name(kind));
    }
    // Le plan de son, amorcé jusqu'à la confirmation du chargement : NDL cale
    // alors les images sur leur horodatage.
    let generation = GENERATION.load(Ordering::Acquire);
    let mut primed = 0;
    while !LOAD_COMPLETED.load(Ordering::Acquire) && st.loaded.as_ref().is_some_and(|(_, at)| at.elapsed() < PRIME_BUDGET) {
        primed += prime(st);
        let until = Instant::now() + PRIME_RETRY;
        while Instant::now() < until && !LOAD_COMPLETED.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let at = st.loaded.as_ref().map(|(_, at)| *at).unwrap_or_else(Instant::now);
    if LOAD_COMPLETED.load(Ordering::Acquire) {
        log::info!("image : NDL confirmé après {} ms ({primed} trames vides)", at.elapsed().as_millis());
    } else {
        log::warn!("image : NDL pas confirmé après {} ms ; plan de son nourri en fond", at.elapsed().as_millis());
        std::thread::Builder::new()
            .name("ndl-prime".into())
            .spawn(move || carry_prime(generation))
            .ok();
    }
    Ok(())
}

/// Une salve de trames vides, jusqu'à l'avance voulue sur l'horloge du
/// chargement ; rend le nombre de trames.
fn prime(st: &mut State) -> u32 {
    let Some((_, at)) = st.loaded else { return 0 };
    let target = at.elapsed().as_millis() as i64 + PRIME_LEAD_MS;
    let mut n = 0;
    let frame = empty_frame(st.plane);
    while st.last_audio_pts < target {
        let pts = st.last_audio_pts;
        if unsafe { (st.api.audio_play)(frame.as_ptr() as *const c_void, frame.len() as u32, pts) } != 0 {
            log::warn!("image : trame vide refusée à {pts} ms : {}", last_error(&st.api));
            st.last_audio_pts += PRIME_PACKET_MS;
            break;
        }
        st.last_audio_pts += PRIME_PACKET_MS;
        n += 1;
    }
    n
}

/// Nourrit le plan de son en fond tant que le chargement n'est pas confirmé.
fn carry_prime(generation: u64) {
    loop {
        std::thread::sleep(PRIME_RETRY);
        if LOAD_COMPLETED.load(Ordering::Acquire) || GENERATION.load(Ordering::Acquire) != generation {
            return;
        }
        let mut g = NDL.lock().unwrap();
        let Some(st) = g.as_mut() else { return };
        if st.loaded.is_none() || GENERATION.load(Ordering::Acquire) != generation {
            return;
        }
        prime(st);
    }
}

pub fn name(kind: i32) -> &'static str {
    match kind {
        VIDEO_H265 => "HEVC",
        VIDEO_AV1 => "AV1",
        _ => "H.264",
    }
}

/// Envoie une image (Annex B) au décodeur ; rend le nombre d'images en
/// attente d'affichage, si NDL le dit.
pub fn play(data: &[u8]) -> Result<Option<i32>, String> {
    let mut g = NDL.lock().unwrap();
    let st = g.as_mut().ok_or("NDL absent")?;
    let Some((_, at)) = st.loaded else { return Err("NDL non chargé".into()) };
    if TV_UNLOADED.load(Ordering::Acquire) {
        return Err(UNLOADED_BY_TV.into());
    }
    if !st.feeding {
        // Avant la confirmation, une image donnée à NDL est perdue (image noire) :
        // elle attend, et l'image clé est redemandée.
        if !LOAD_COMPLETED.load(Ordering::Acquire) && at.elapsed() < FEED_ANYWAY_AFTER {
            return Err(NOT_READY.into());
        }
        if !LOAD_COMPLETED.load(Ordering::Acquire) {
            log::warn!("image : NDL toujours pas confirmé après {} ms, images envoyées quand même", at.elapsed().as_millis());
        }
        st.feeding = true;
    }
    let pts = (at.elapsed().as_millis() as i64).max(st.last_pts + 1);
    st.last_pts = pts;
    unsafe {
        if (st.api.video_play)(data.as_ptr() as *const c_void, data.len() as u32, pts) != 0 {
            return Err(format!("NDL_DirectVideoPlay : {}", last_error(&st.api)));
        }
        let mut n: c_int = 0;
        Ok(st.api.render_length.and_then(|f| (f(&mut n) == 0).then_some(n)))
    }
}

/// Numéro du chargement en cours : change à chaque chargement (le décodeur
/// repart de zéro, il lui faut les paramètres et une image clé).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Le décodeur est chargé et confirmé.
pub fn is_loaded() -> bool {
    NDL.lock().unwrap().as_ref().is_some_and(|st| st.loaded.is_some()) && LOAD_COMPLETED.load(Ordering::Acquire)
}

/// Du son réel (PCM S16LE 6 voies) au plan de son de NDL, horodaté à
/// l'horloge du chargement, jamais en arrière (NDL couperait le son pour le
/// reste de la session). `Ok(false)` : pas encore de plan confirmé, paquet
/// laissé de côté ; `Err` si le chargement n'a pas ce plan.
pub fn play_audio(data: &[u8]) -> Result<bool, String> {
    let mut g = NDL.lock().unwrap();
    let st = g.as_mut().ok_or("NDL absent")?;
    let Some((_, at)) = st.loaded else { return Ok(false) };
    if st.plane != AudioPlane::Pcm51 {
        return Err("le chargement n'a pas de plan PCM 6 voies".into());
    }
    if !LOAD_COMPLETED.load(Ordering::Acquire) {
        return Ok(false);
    }
    let pts = (at.elapsed().as_millis() as i64).max(st.last_audio_pts + 1);
    st.last_audio_pts = pts;
    if unsafe { (st.api.audio_play)(data.as_ptr() as *const c_void, data.len() as u32, pts) } != 0 {
        return Err(format!("NDL_DirectAudioPlay : {}", last_error(&st.api)));
    }
    Ok(true)
}

/// Ce que le téléviseur ferait d'un son multicanal décodé par NDL
/// (`NDL_DirectAudioSupportMultiChannel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Multichannel {
    /// Fonction absente (webOS ancien) ou réponse illisible.
    Unknown,
    /// Pas de multicanal sur ce téléviseur.
    Unsupported,
    /// Aucun appareil branché ne le prend (pas d'ampli en eARC).
    NoDevice,
    /// L'ampli le prend, mais la Sortie son numérique n'est pas en Pass-Through.
    NotPassthrough,
    /// Le multicanal sortira.
    Ready,
}

/// Dernière réponse du téléviseur (0xff : jamais lue).
static LAST_MULTICHANNEL: AtomicU8 = AtomicU8::new(0xff);

/// Interroge le téléviseur (réglages de son et ampli branché, à l'instant).
/// NDL ne répond qu'initialisé, donc en session : hors session, la dernière
/// réponse lue.
pub fn multichannel() -> Multichannel {
    let g = NDL.lock().unwrap();
    let Some(f) = g.as_ref().filter(|st| st.initialized).and_then(|st| st.api.multichannel) else {
        return decode_multichannel(LAST_MULTICHANNEL.load(Ordering::Acquire) as c_int);
    };
    // Seul le paramètre de sortie compte : la valeur rendue n'a pas de sens
    // (punktfunk, docs/NOTES.md : lue, elle donne n'importe quoi sur ARM).
    let mut v: c_int = -1;
    let ret = unsafe { f(&mut v) };
    static LOGGED: AtomicU8 = AtomicU8::new(0xff);
    if LOGGED.swap(v.clamp(0, 0xfe) as u8, Ordering::AcqRel) != v.clamp(0, 0xfe) as u8 {
        log::info!("son : NDL_DirectAudioSupportMultiChannel → {v} (rendu {ret})");
    }
    if (0..=3).contains(&v) {
        LAST_MULTICHANNEL.store(v as u8, Ordering::Release);
    }
    decode_multichannel(v)
}

fn decode_multichannel(v: c_int) -> Multichannel {
    match v {
        0 => Multichannel::Unsupported,
        1 => Multichannel::NoDevice,
        2 => Multichannel::NotPassthrough,
        3 => Multichannel::Ready,
        _ => Multichannel::Unknown,
    }
}

/// Erreur de `play` quand le téléviseur a déchargé le décodeur de lui-même.
pub const UNLOADED_BY_TV: &str = "décodeur déchargé par le téléviseur";

/// Erreur de `play` tant que NDL n'a pas confirmé le chargement.
pub const NOT_READY: &str = "NDL pas encore prêt";

/// Décharge le décodeur (fin de l'image).
pub fn unload() {
    let mut g = NDL.lock().unwrap();
    if let Some(st) = g.as_mut()
        && st.loaded.take().is_some()
    {
        GENERATION.fetch_add(1, Ordering::AcqRel);
        unsafe {
            (st.api.unload)();
        }
        log::info!("image : NDL déchargé");
    }
}

/// À la sortie de l'app.
pub fn quit() {
    unload();
    let mut g = NDL.lock().unwrap();
    if let Some(st) = g.as_mut()
        && st.initialized
    {
        unsafe {
            (st.api.quit)();
        }
        st.initialized = false;
    }
}
