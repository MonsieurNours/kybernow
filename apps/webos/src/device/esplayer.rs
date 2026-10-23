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

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
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
}

unsafe impl Send for State {}

static ES: Mutex<Option<State>> = Mutex::new(None);
static LOADED: AtomicBool = AtomicBool::new(false);
static FAILED: AtomicBool = AtomicBool::new(false);

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
        0x17 => log::info!("son : chaîne Dolby déchargée {s}"),
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
        *g = Some(State { api, player: std::ptr::null_mut(), loaded_at: None, playing: false, last_pts_ns: 0 });
    }
    Ok(())
}

/// Charge la chaîne audio E-AC-3 5.1 et l'amorce de `silence` (une trame)
/// jusqu'à la confirmation ; faux si le téléviseur ne la confirme pas.
pub fn load(silence: &[u8]) -> Result<(), String> {
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
    // Après l'image : le décodeur de jeu prend ses décodeurs d'abord (une
    // reconnexion le recharge ; chargée avant lui, cette chaîne le bloque).
    let started = Instant::now();
    while !super::ndl::is_loaded() && started.elapsed() < LOAD_BUDGET {
        std::thread::sleep(Duration::from_millis(20));
    }
    unload();
    {
        let mut g = ES.lock().unwrap();
        let st = g.as_mut().ok_or("NDL_ES absent")?;
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
                return Err("NDL_ES_Load a échoué".into());
            }
        }
        st.loaded_at = Some(Instant::now());
        st.playing = false;
        st.last_pts_ns = 0;
    }
    // Amorçage : une trame de silence toutes les 32 ms jusqu'à la confirmation.
    let started = Instant::now();
    let mut primed = 0;
    while !LOADED.load(Ordering::Acquire) && !FAILED.load(Ordering::Acquire) && started.elapsed() < LOAD_BUDGET {
        if feed(silence).is_ok() {
            primed += 1;
        }
        std::thread::sleep(Duration::from_millis(32));
    }
    if !LOADED.load(Ordering::Acquire) {
        unload();
        return Err(format!("chaîne Dolby pas confirmée après {} ms ({primed} trames de silence)", started.elapsed().as_millis()));
    }
    let mut g = ES.lock().unwrap();
    if let Some(st) = g.as_mut()
        && !st.playing
    {
        unsafe {
            (st.api.play)(st.player);
        }
        st.playing = true;
    }
    log::info!("son : chaîne Dolby confirmée après {} ms ({primed} trames de silence)", started.elapsed().as_millis());
    Ok(())
}

/// Une trame E-AC-3, horodatée à l'horloge du chargement (jamais en arrière).
pub fn feed(frame: &[u8]) -> Result<(), String> {
    let mut g = ES.lock().unwrap();
    let st = g.as_mut().ok_or("NDL_ES absent")?;
    let Some(at) = st.loaded_at else { return Err("chaîne Dolby pas chargée".into()) };
    let pts = (at.elapsed().as_nanos() as i64).max(st.last_pts_ns + FRAME_NS);
    st.last_pts_ns = pts;
    let doc = format!("{{\"bufferAddr\":\"{:#x}\",\"bufferSize\":{},\"pts\":{pts},\"esData\":2}}", frame.as_ptr() as usize, frame.len());
    let json = CString::new(doc).map_err(|e| e.to_string())?;
    let r = unsafe { (st.api.feed)(st.player, json.as_ptr()) };
    match r {
        0 => Ok(()),
        -2 => Err("tampon plein".into()),
        -3 => Err("pas prête".into()),
        n => Err(format!("NDL_ES_Feed : {n}")),
    }
}

/// Décharge la chaîne (fin de session, repli).
pub fn unload() {
    let mut g = ES.lock().unwrap();
    if let Some(st) = g.as_mut()
        && st.loaded_at.take().is_some()
    {
        unsafe {
            (st.api.unload)(st.player);
        }
        st.playing = false;
        log::info!("son : chaîne Dolby déchargée");
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
