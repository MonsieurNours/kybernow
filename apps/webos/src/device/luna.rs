//! Les services du téléviseur (bus luna), appelés comme la SDL de webOS le
//! fait pour la dalle et sa fréquence : `HLunaServiceCall` de `libhelpers.so.2`
//! (bibliothèque du téléviseur, déjà ouverte par la SDL), par `dlopen`, sur le
//! bus public. Une réponse, attendue au plus `timeout`.

use std::ffi::{c_char, c_int, c_ulong, c_void, CStr, CString};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// `HContext` de libhelpers (même disposition que la SDL de webOS).
#[repr(C)]
struct HContext {
    callback: extern "C" fn(*mut c_void, *mut c_void, *mut HContext) -> c_int,
    userdata: *mut c_void,
    unknown: *mut c_void,
    multiple: c_int,
    public: c_int,
    ret_token: c_ulong,
}

type Call = unsafe extern "C" fn(*const c_char, *const c_char, *mut HContext) -> c_int;
type Message = unsafe extern "C" fn(*mut c_void) -> *const c_char;

struct Api {
    call: Call,
    message: Message,
}

unsafe impl Send for Api {}

static API: Mutex<Option<Option<Api>>> = Mutex::new(None);

fn api() -> Option<(Call, Message)> {
    let mut g = API.lock().unwrap();
    let api = g.get_or_insert_with(|| unsafe {
        let name = CString::new("libhelpers.so.2").unwrap();
        let lib = libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if lib.is_null() {
            log::warn!("téléviseur : libhelpers.so.2 introuvable");
            return None;
        }
        let call = libc::dlsym(lib, c"HLunaServiceCall".as_ptr());
        let message = libc::dlsym(lib, c"HLunaServiceMessage".as_ptr());
        if call.is_null() || message.is_null() {
            log::warn!("téléviseur : libhelpers sans HLunaServiceCall");
            return None;
        }
        Some(Api { call: std::mem::transmute::<*mut c_void, Call>(call), message: std::mem::transmute::<*mut c_void, Message>(message) })
    });
    api.as_ref().map(|a| (a.call, a.message))
}

/// La réponse, partagée entre l'appelant et le rappel (fil de libhelpers).
struct Reply {
    text: Mutex<Option<String>>,
    done: Condvar,
    message: Message,
}

extern "C" fn on_reply(_sh: *mut c_void, msg: *mut c_void, ctx: *mut HContext) -> c_int {
    // Le contexte vit jusqu'à la réponse (laissé en mémoire si l'appelant n'attend plus).
    let reply = unsafe { &*((*ctx).userdata as *const Reply) };
    let text = unsafe {
        let p = (reply.message)(msg);
        if p.is_null() { String::new() } else { CStr::from_ptr(p).to_string_lossy().into_owned() }
    };
    // Signalé sous le verrou : l'appelant ne libère rien avant qu'il soit rendu.
    let mut g = reply.text.lock().unwrap();
    *g = Some(text);
    reply.done.notify_all();
    drop(g);
    0
}

/// Appelle `uri` (`luna://service/méthode`) avec ce JSON ; la réponse JSON, ou
/// `None` (bibliothèque absente, refus, pas de réponse à temps).
pub fn call(uri: &str, payload: &str, timeout: Duration) -> Option<String> {
    let (call, message) = api()?;
    let (uri_c, payload_c) = (CString::new(uri).ok()?, CString::new(payload).ok()?);
    // Le rappel peut arriver après l'attente : sans réponse à temps, la réponse
    // et le contexte restent en mémoire (quelques octets par appel muet).
    let reply = Arc::new(Reply { text: Mutex::new(None), done: Condvar::new(), message });
    let ctx = Box::into_raw(Box::new(HContext {
        callback: on_reply,
        userdata: Arc::into_raw(reply.clone()) as *mut c_void,
        unknown: std::ptr::null_mut(),
        multiple: 0,
        public: 1,
        ret_token: 0,
    }));
    if unsafe { call(uri_c.as_ptr(), payload_c.as_ptr(), ctx) } != 0 {
        log::warn!("téléviseur : {uri} refusé");
        unsafe {
            drop(Arc::from_raw((*ctx).userdata as *const Reply));
            drop(Box::from_raw(ctx));
        }
        return None;
    }
    let g = reply.text.lock().unwrap();
    let (g, _) = reply.done.wait_timeout_while(g, timeout, |t| t.is_none()).unwrap();
    let text = g.clone();
    drop(g);
    match &text {
        // Répondu : libhelpers n'y revient plus (la SDL libère son contexte de même).
        Some(_) => unsafe {
            drop(Arc::from_raw((*ctx).userdata as *const Reply));
            drop(Box::from_raw(ctx));
        },
        None => log::warn!("téléviseur : {uri} sans réponse après {} ms", timeout.as_millis()),
    }
    text
}
