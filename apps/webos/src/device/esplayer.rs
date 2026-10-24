//! Seconde chaîne média, pour le son seul : le lecteur de LG (`NDL_ES_*`,
//! façade C de `StarfishMediaAPIs` dans `libNDL_esmedia.so.1`) chargé en
//! `BUFFERSTREAM` avec un seul flux, `AC3 PLUS` (E-AC-3), que le téléviseur
//! passe tel quel à l'ampli. L'image reste sur la chaîne de jeu de NDL
//! (`ndl.rs`), à sa latence : dans une chaîne `BUFFERSTREAM` à deux flux,
//! les puits se synchronisent sur l'horloge GStreamer et l'image se cale sur
//! le son (essayé sur la G2 : tout en retard).
//!
//! Signatures lues dans le code machine de la façade et de son implémentation
//! (webOS 22 à 26 ont le même jeu de fonctions) :
//! `NDL_ESMediaInit()`, `NDL_ES_CreatePlayer(appid)` → lecteur,
//! `NDL_ES_Load(lecteur, json, rappel, données)`, `NDL_ES_Feed(lecteur, json)`
//! → 0 Ok, −2 BufferFull, −3 NotReady, `NDL_ES_Play`, `NDL_ES_Unload`,
//! `NDL_ES_DeletePlayer`. Horodatages en nanosecondes.
//!
//! Le téléviseur décharge cette chaîne de lui-même avec celle de jeu (sa
//! politique de ressources, à l'arrêt du son) : `feed` la recharge alors en
//! fond, après le décodeur d'image, et les trames sont sautées en attendant.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

type Callback = extern "C" fn(c_int, i64, *const c_char, *mut c_void);

struct Api {
    create: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    delete: unsafe extern "C" fn(*mut c_void) -> c_int,
    load: unsafe extern "C" fn(*mut c_void, *const c_char, Callback, *mut c_void) -> c_int,
    unload: unsafe extern "C" fn(*mut c_void) -> c_int,
    play: unsafe extern "C" fn(*mut c_void) -> c_int,
    feed: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
}

unsafe impl Send for Api {}

struct State {
    api: Api,
    player: *mut c_void,
    loaded_at: Option<Instant>,
    /// Lecture lancée (après la confirmation).
    playing: bool,
    last_pts_ns: i64,
    /// Trame de silence du dernier chargement (amorce d'un rechargement).
    silence: Vec<u8>,
    /// Une trame acceptée depuis la confirmation : une erreur de `feed` vaut
    /// alors un rechargement (une seule fois, pas en boucle).
    fed: bool,
}

unsafe impl Send for State {}

static ES: Mutex<Option<State>> = Mutex::new(None);
static LOADED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);
/// Déchargée par le téléviseur lui-même (`UNLOADCOMPLETED` sans demande).
static TV_UNLOADED: AtomicBool = AtomicBool::new(false);
/// Rechargement en cours, en fond.
static RELOADING: AtomicBool = AtomicBool::new(false);
/// Rechargement raté : l'erreur, rendue par `feed` (le son repasse par SDL).
static RELOAD_FAILED: Mutex<Option<String>> = Mutex::new(None);
/// Change à chaque chargement et déchargement demandés : un rechargement en
/// fond d'une session finie ou remplacée s'arrête là.
static EPOCH: AtomicU64 = AtomicU64::new(0);

/// Erreur de `feed` pendant un rechargement : trame sautée.
pub const RELOADING_ERR: &str = "rechargement en cours";

const LOAD_BUDGET: Duration = Duration::from_millis(2500);
/// Durée d'une trame E-AC-3 (1536 images à 48 kHz).
const FRAME_NS: i64 = 32_000_000;

unsafe fn sym<T>(lib: *mut c_void, name: &str) -> Option<T> {
    let n = CString::new(name).ok()?;
    let p = unsafe { libc::dlsym(lib, n.as_ptr()) };
    if p.is_null() { None } else { Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) }) }
}

extern "C" fn on_event(kind: c_int, num: i64, s: *const c_char, _data: *mut c_void) {
    let s = if s.is_null() { String::new() } else { unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned() };
    match kind {
        0x16 => {
            LOADED.store(true, Ordering::Release);
            log::info!("son : chaîne Dolby chargée (confirmé) {s}");
        }
        0x17 => {
            // `unload` baisse LOADED avant de décharger : levé ici, le
            // déchargement vient du téléviseur.
            if LOADED.swap(false, Ordering::AcqRel) {
                TV_UNLOADED.store(true, Ordering::Release);
                log::warn!("son : chaîne Dolby déchargée par le téléviseur {s}");
            } else {
                log::info!("son : chaîne Dolby déchargée {s}");
            }
        }
        0x1a => log::info!("son : chaîne Dolby en lecture {s}"),
        0x1f | 0x20 | 0x2712 => {
            log::warn!("son : chaîne Dolby, événement 0x{kind:02x} ({num:#x}) {s}");
            FAILED.store(true, Ordering::Release);
        }
        _ => log::debug!("son : chaîne Dolby, événement 0x{kind:02x} ({num:#x}) {s}"),
    }
}

fn open() -> Result<(), String> {
    let mut g = ES.lock().unwrap();
    if g.is_some() {
        return Ok(());
    }
    unsafe {
        let name = CString::new("libNDL_esmedia.so.1").unwrap();
        let lib = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if lib.is_null() {
            return Err("libNDL_esmedia.so.1 introuvable".into());
        }
        let init: Option<unsafe extern "C" fn() -> c_int> = sym(lib, "NDL_ESMediaInit");
        let api = (|| {
            Some(Api {
                create: sym(lib, "NDL_ES_CreatePlayer")?,
                delete: sym(lib, "NDL_ES_DeletePlayer")?,
                load: sym(lib, "NDL_ES_Load")?,
                unload: sym(lib, "NDL_ES_Unload")?,
                play: sym(lib, "NDL_ES_Play")?,
                feed: sym(lib, "NDL_ES_Feed")?,
            })
        })()
        .ok_or("NDL_ES_* incomplet")?;
        if let Some(init) = init
            && init() != 0
        {
            return Err("NDL_ESMediaInit a échoué".into());
        }
        *g = Some(State { api, player: std::ptr::null_mut(), loaded_at: None, playing: false, last_pts_ns: 0, silence: Vec::new(), fed: false });
    }
    Ok(())
}

/// Charge la chaîne audio E-AC-3 5.1 et l'amorce de `silence` (une trame)
/// jusqu'à la confirmation ; faux si le téléviseur ne la confirme pas.
pub fn load(silence: &[u8]) -> Result<(), String> {
    let epoch = EPOCH.fetch_add(1, Ordering::AcqRel) + 1;
    *RELOAD_FAILED.lock().unwrap() = None;
    start(silence, epoch)
}

/// Le chargement, pour `load` et pour un rechargement en fond : abandonné
/// dès que `EPOCH` n'est plus `epoch` (session finie, autre chargement).
fn start(silence: &[u8], epoch: u64) -> Result<(), String> {
    open()?;
    let doc = serde_json::json!({
        "args": [{
            "mediaTransportType": "BUFFERSTREAM",
            "option": {
                "appId": crate::APP_ID,
                "transmission": { "contentsType": "LIVE", "trickType": "client-side" },
                "seekMode": "keep-rate",
                "needVideo": false,
                "externalStreamingInfo": {
                    "streamQualityInfo": true,
                    "bufferingCtrInfo": {
                        "preBufferByte": 0, "bufferMinLevel": 0, "bufferMaxLevel": 0,
                        "qBufferLevelAudio": 0,
                        "srcBufferLevelAudio": { "minimum": 0, "maximum": 2 * 1024 * 1024 }
                    },
                    "contents": {
                        "codec": { "audio": "AC3 PLUS" },
                        "ac3PlusInfo": { "channels": 6, "frequency": 48.0 },
                        "format": "RAW",
                        "esInfo": { "ptsToDecode": 0, "pauseAtDecodeTime": true, "seperatedPTS": true }
                    }
                }
            }
        }]
    })
    .to_string();
    let json = CString::new(doc).map_err(|e| e.to_string())?;
    let app = CString::new(crate::APP_ID).unwrap();
    let current = || EPOCH.load(Ordering::Acquire) == epoch;
    // Après l'image : le décodeur de jeu prend ses décodeurs d'abord (une
    // reconnexion le recharge, le téléviseur aussi après avoir tout
    // déchargé ; chargée avant lui, cette chaîne le bloque).
    let started = Instant::now();
    while !super::ndl::is_loaded() && started.elapsed() < LOAD_BUDGET && current() {
        std::thread::sleep(Duration::from_millis(20));
    }
    {
        let mut g = ES.lock().unwrap();
        if !current() {
            return Err(CANCELLED.into());
        }
        let st = g.as_mut().ok_or("NDL_ES absent")?;
        release(st);
        st.silence = silence.to_vec();
        unsafe {
            if st.player.is_null() {
                st.player = (st.api.create)(app.as_ptr());
                if st.player.is_null() {
                    return Err("NDL_ES_CreatePlayer a échoué".into());
                }
            }
            LOADED.store(false, Ordering::Release);
            FAILED.store(false, Ordering::Release);
            if (st.api.load)(st.player, json.as_ptr(), on_event, std::ptr::null_mut()) != 0 {
                // Refusé (lecteur laissé en mauvais état par un déchargement
                // du téléviseur…) : un nouveau lecteur, puis un nouvel essai.
                log::warn!("son : NDL_ES_Load refusé ; nouveau lecteur");
                (st.api.delete)(st.player);
                st.player = (st.api.create)(app.as_ptr());
                if st.player.is_null() || (st.api.load)(st.player, json.as_ptr(), on_event, std::ptr::null_mut()) != 0 {
                    return Err("NDL_ES_Load a échoué".into());
                }
            }
        }
        st.loaded_at = Some(Instant::now());
        st.playing = false;
        st.last_pts_ns = 0;
        st.fed = false;
    }
    // Amorçage : une trame de silence toutes les 32 ms jusqu'à la confirmation.
    let started = Instant::now();
    let mut primed = 0;
    while !LOADED.load(Ordering::Acquire) && !FAILED.load(Ordering::Acquire) && started.elapsed() < LOAD_BUDGET && current() {
        if ES.lock().unwrap().as_mut().is_some_and(|st| push(st, silence).is_ok()) {
            primed += 1;
        }
        std::thread::sleep(Duration::from_millis(32));
    }
    let mut g = ES.lock().unwrap();
    if !current() {
        // Déchargée ou rechargée entre-temps par un autre : rien à faire ici.
        return Err(CANCELLED.into());
    }
    let st = g.as_mut().ok_or("NDL_ES absent")?;
    if !LOADED.load(Ordering::Acquire) {
        release(st);
        return Err(format!("chaîne Dolby pas confirmée après {} ms ({primed} trames de silence)", started.elapsed().as_millis()));
    }
    if !st.playing {
        unsafe {
            (st.api.play)(st.player);
        }
        st.playing = true;
    }
    log::info!("son : chaîne Dolby confirmée après {} ms ({primed} trames de silence)", started.elapsed().as_millis());
    Ok(())
}

/// Erreur d'un chargement abandonné (`EPOCH` a changé).
const CANCELLED: &str = "chargement abandonné";

/// Une trame à la chaîne, horodatée à l'horloge du chargement (jamais en
/// arrière) ; rend le code de `NDL_ES_Feed` s'il n'est pas 0.
fn push(st: &mut State, frame: &[u8]) -> Result<(), c_int> {
    let Some(at) = st.loaded_at else { return Err(-3) };
    let pts = (at.elapsed().as_nanos() as i64).max(st.last_pts_ns + FRAME_NS);
    st.last_pts_ns = pts;
    let doc = format!("{{\"bufferAddr\":\"{:#x}\",\"bufferSize\":{},\"pts\":{pts},\"esData\":2}}", frame.as_ptr() as usize, frame.len());
    let Ok(json) = CString::new(doc) else { return Err(-1) };
    match unsafe { (st.api.feed)(st.player, json.as_ptr()) } {
        0 => Ok(()),
        n => Err(n),
    }
}

/// Une trame E-AC-3 à la chaîne. Déchargée par le téléviseur, ou en erreur
/// après avoir marché, elle est rechargée en fond : `RELOADING_ERR` en
/// attendant (trame sautée), l'erreur du rechargement s'il rate.
pub fn feed(frame: &[u8]) -> Result<(), String> {
    if RELOADING.load(Ordering::Acquire) {
        return Err(RELOADING_ERR.into());
    }
    if let Some(e) = RELOAD_FAILED.lock().unwrap().take() {
        return Err(e);
    }
    let mut g = ES.lock().unwrap();
    let st = g.as_mut().ok_or("NDL_ES absent")?;
    if st.loaded_at.is_none() {
        return Err("chaîne Dolby pas chargée".into());
    }
    if TV_UNLOADED.load(Ordering::Acquire) {
        return reload(st, "déchargée par le téléviseur");
    }
    match push(st, frame) {
        Ok(()) => {
            st.fed = true;
            Ok(())
        }
        Err(-2) => Err("tampon plein".into()),
        // Elle marchait : déchargée sans que l'événement soit arrivé,
        // probablement. Un seul essai, jusqu'à la prochaine trame acceptée.
        Err(n) if st.fed => {
            st.fed = false;
            reload(st, &format!("NDL_ES_Feed : {n}"))
        }
        Err(-3) => Err("pas prête".into()),
        Err(n) => Err(format!("NDL_ES_Feed : {n}")),
    }
}

/// Lance le rechargement en fond (il attend le décodeur d'image, rechargé à
/// l'image clé suivante) ; rend `RELOADING_ERR`.
fn reload(st: &mut State, why: &str) -> Result<(), String> {
    log::warn!("son : chaîne Dolby {why} ; rechargement");
    let silence = st.silence.clone();
    let epoch = EPOCH.load(Ordering::Acquire);
    RELOADING.store(true, Ordering::Release);
    let spawned = std::thread::Builder::new().name("kybernow-dolby".into()).spawn(move || {
        match start(&silence, epoch) {
            Ok(()) => log::info!("son : chaîne Dolby rechargée"),
            Err(e) if EPOCH.load(Ordering::Acquire) != epoch => log::info!("son : rechargement de la chaîne Dolby abandonné ({e})"),
            Err(e) => *RELOAD_FAILED.lock().unwrap() = Some(format!("rechargement : {e}")),
        }
        RELOADING.store(false, Ordering::Release);
    });
    if let Err(e) = spawned {
        RELOADING.store(false, Ordering::Release);
        return Err(format!("rechargement impossible ({e})"));
    }
    Err(RELOADING_ERR.into())
}

/// Décharge la chaîne si elle est chargée ; déjà déchargée par le téléviseur,
/// rien à décharger.
fn release(st: &mut State) {
    let by_tv = TV_UNLOADED.swap(false, Ordering::AcqRel);
    if st.loaded_at.take().is_some() {
        st.playing = false;
        if by_tv {
            return;
        }
        LOADED.store(false, Ordering::Release);
        unsafe {
            (st.api.unload)(st.player);
        }
        log::info!("son : chaîne Dolby déchargée");
    }
}

/// Décharge la chaîne (fin de session, repli) et arrête un rechargement en cours.
pub fn unload() {
    EPOCH.fetch_add(1, Ordering::AcqRel);
    let mut g = ES.lock().unwrap();
    if let Some(st) = g.as_mut() {
        release(st);
    }
}

/// À la sortie de l'app.
pub fn quit() {
    unload();
    let mut g = ES.lock().unwrap();
    if let Some(st) = g.as_mut()
        && !st.player.is_null()
    {
        unsafe {
            (st.api.delete)(st.player);
        }
        st.player = std::ptr::null_mut();
    }
}
