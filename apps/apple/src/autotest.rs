//! Profil « Auto » pour les apps Apple (docs/profil-auto.md) : le test des
//! conditions (`autotest::run`), mené sur un fil à part avec la console de
//! l'ordinateur que l'app tient déjà (relais compris), et les aides de son
//! résultat (résumé, réglages, réseau).
//!
//! Le décodage des extraits est celui de l'app : le rappel `decode` reçoit un
//! extrait (`KnAutoSample`, lu par les `kn_auto_sample_*`), le passe à son
//! décodeur (VideoToolbox, PyroWave sur Metal) aussi vite qu'il le prend,
//! pendant 1 s au plus, et rend les images par seconde (négatif : pas su le
//! décoder). Les rappels sont appelés sur le fil du test ; les chaînes qu'ils
//! reçoivent ne valent que pendant l'appel.

use std::ffi::{c_char, c_void, CString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kybernow_core::autotest::{self, AutoError, AutoResult, Device, Progress, Sample};
use kybernow_core::console::Mode;
use kybernow_core::i18n::{self, Lang};
use kybernow_core::profile::Capabilities;
use serde::Deserialize;

use crate::{KnConsole, give, optional, text};

/// Un extrait vidéo du test de décodage (opaque), valable pendant le rappel
/// `decode` seulement.
pub struct KnAutoSample {
    sample: *const Sample,
}

fn sample<'a>(s: *const KnAutoSample) -> Option<&'a Sample> {
    unsafe { s.as_ref().and_then(|s| s.sample.as_ref()) }
}

/// Format de l'extrait : FourCC des paquets de Kyber (`h264`, `h265`, `av1`
/// sur 4 octets, `pwv1`), comme `VideoDecoder.Codec` ; 0 si nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_fourcc(s: *const KnAutoSample) -> u32 {
    sample(s).map(|s| s.codec.fourcc()).unwrap_or(0)
}

/// L'extrait est en 4:4:4.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_yuv444(s: *const KnAutoSample) -> bool {
    sample(s).is_some_and(|s| s.yuv444)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_width(s: *const KnAutoSample) -> u32 {
    sample(s).map(|s| s.width).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_height(s: *const KnAutoSample) -> u32 {
    sample(s).map(|s| s.height).unwrap_or(0)
}

/// Nombre de paquets : une image clé (en-têtes devant, en H.264 et HEVC)
/// puis les suivantes ; en PyroWave, une image intra.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_packet_count(s: *const KnAutoSample) -> usize {
    sample(s).map(|s| s.packets.len()).unwrap_or(0)
}

/// Le paquet `index` : ses octets (à l'extrait, valables pendant le rappel),
/// leur nombre dans `len` et l'image clé dans `is_key`. Nul hors des bornes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_auto_sample_packet(s: *const KnAutoSample, index: usize, len: *mut usize, is_key: *mut bool) -> *const u8 {
    let Some(p) = sample(s).and_then(|s| s.packets.get(index)) else { return std::ptr::null() };
    unsafe {
        if !len.is_null() {
            *len = p.data.len();
        }
        if !is_key.is_null() {
            *is_key = p.is_key;
        }
    }
    p.data.as_ptr()
}

/// Rappels du test, appelés sur son fil.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KnAutoCallbacks {
    pub userdata: *mut c_void,
    /// Décode l'extrait (lu par les `kn_auto_sample_*`) aussi vite que
    /// possible, 1 s au plus, et rend les images par seconde ; négatif : pas
    /// su le décoder. Absent : aucun format essayé.
    pub decode: Option<unsafe extern "C" fn(userdata: *mut c_void, sample: *const KnAutoSample) -> f64>,
    /// L'avancement (`Progress` en JSON : `{steps: [{step, title, state, text}]}`,
    /// `state` : `waiting`, `running`, `done`, `skipped`), à chaque changement.
    pub on_progress: Option<unsafe extern "C" fn(userdata: *mut c_void, progress_json: *const c_char)>,
    /// La fin, toujours appelée une fois (aussi après `kn_autotest_cancel`
    /// ou `kn_autotest_free`) : c'est là que l'app libère `userdata`.
    /// `result_json` : l'`AutoResult` en JSON, à garder pour cet ordinateur ;
    /// sinon nul, et `error` porte le texte de l'échec (« Le test n'a pas pu
    /// joindre {nom}. »), `cancelled` vrai si le test a été arrêté.
    pub on_done: Option<unsafe extern "C" fn(userdata: *mut c_void, result_json: *const c_char, error: *const c_char, cancelled: bool)>,
}

unsafe impl Send for KnAutoCallbacks {}

/// Un test en cours (opaque).
pub struct KnAutoTest {
    cancel: Arc<AtomicBool>,
}

/// Ce que l'app dit de l'appareil (`device_json`).
#[derive(Deserialize)]
struct DeviceJson {
    /// L'écran de l'appareil : `{width, height, refresh}`.
    screen: Mode,
    /// `{hevc, hevc444, av1, hdr, pyrowave}`.
    #[serde(default)]
    caps: Capabilities,
    /// Le choix « HDR » de l'app pour cet ordinateur.
    #[serde(default)]
    hdr: bool,
    /// Le réseau du moment (nom du Wi-Fi, « Ethernet ») ; absent : inconnu.
    #[serde(default)]
    network: Option<String>,
    /// Relié par câble au réseau de l'ordinateur.
    #[serde(default)]
    wired: bool,
    /// Le nom de l'ordinateur, pour le texte d'un échec.
    #[serde(default)]
    name: String,
}

fn cstring(s: &str) -> CString {
    CString::new(s.replace('\0', " ")).unwrap_or_default()
}

/// Lance le test sur un fil à part, avec la console de l'ordinateur (tenue
/// par l'app : elle peut la libérer pendant le test). `device_json` :
/// `{screen: {width, height, refresh}, caps, hdr, network, wired, name}`.
/// Textes dans la langue de l'app (`kn_set_language`). Nul si un argument est
/// illisible (et `on_done` n'est alors pas appelé). À libérer par
/// `kn_autotest_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_start(console: *const KnConsole, device_json: *const c_char, cb: *const KnAutoCallbacks) -> *mut KnAutoTest {
    let (Some(console), Ok(d), Some(cb)) = (unsafe { console.as_ref() }, serde_json::from_str::<DeviceJson>(&text(device_json)), unsafe { cb.as_ref() })
    else {
        return std::ptr::null_mut();
    };
    let console = console.0.clone();
    let cb = *cb;
    let cancel = Arc::new(AtomicBool::new(false));
    let stop = cancel.clone();
    let lang = i18n::current();
    let device = Device { screen: d.screen, caps: d.caps, hdr: d.hdr, network: d.network, wired: d.wired };
    let name = d.name;
    std::thread::Builder::new()
        .name("kybernow-autotest".into())
        .spawn(move || {
            // Toute la structure (Send) est prise, pas seulement ses champs.
            let cb: KnAutoCallbacks = cb;
            let mut decoder = |s: &Sample| -> Option<f64> {
                let f = cb.decode?;
                let handle = KnAutoSample { sample: s };
                let fps = unsafe { f(cb.userdata, &handle) };
                (fps.is_finite() && fps > 0.0).then_some(fps)
            };
            let mut progress = |p: &Progress| {
                let Some(f) = cb.on_progress else { return };
                let json = cstring(&serde_json::to_string(p).unwrap_or_default());
                unsafe { f(cb.userdata, json.as_ptr()) };
            };
            let result = autotest::run(console, &device, &mut decoder, &mut progress, &stop, lang);
            let Some(done) = cb.on_done else { return };
            match result {
                Ok(r) => {
                    let json = cstring(&serde_json::to_string(&r).unwrap_or_default());
                    unsafe { done(cb.userdata, json.as_ptr(), std::ptr::null(), false) };
                }
                Err(e) => {
                    if let AutoError::Unreachable(err) = &e {
                        log::warn!("test du profil Auto : {err}");
                    }
                    let error = cstring(&e.text(lang, &name));
                    unsafe { done(cb.userdata, std::ptr::null(), error.as_ptr(), matches!(e, AutoError::Cancelled)) };
                }
            }
        })
        .ok()
        .map(|_| Box::into_raw(Box::new(KnAutoTest { cancel })))
        .unwrap_or(std::ptr::null_mut())
}

/// Arrête le test (entre deux mesures) : `on_done` suit, avec `cancelled`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_cancel(t: *const KnAutoTest) {
    if let Some(t) = unsafe { t.as_ref() } {
        t.cancel.store(true, Ordering::Relaxed);
    }
}

/// Arrête le test s'il tourne encore et libère la poignée, sans attendre :
/// `on_done` sera tout de même appelé, une fois.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_free(t: *mut KnAutoTest) {
    if !t.is_null() {
        let t = unsafe { Box::from_raw(t) };
        t.cancel.store(true, Ordering::Relaxed);
    }
}

fn result(json: *const c_char) -> Option<AutoResult> {
    serde_json::from_str(&text(json)).ok()
}

/// Le résumé d'un résultat (`result_json`, rendu par `on_done`) dans la
/// langue `lang` (`fr`, `en` ; vide : celle de l'app) : une liste JSON de
/// `{key, title, value, reason}`, dans l'ordre (format, définition, 4:4:4,
/// débit, transport, débit adaptatif). Nul si le JSON est illisible. À
/// libérer par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_choices(result_json: *const c_char, lang: *const c_char) -> *mut c_char {
    let Some(r) = result(result_json) else { return std::ptr::null_mut() };
    let lang = optional(lang).map(|t| Lang::from_tag(&t)).unwrap_or_else(i18n::current);
    match serde_json::to_string(&r.choices(lang)) {
        Ok(json) => give(json),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Les réglages de session d'un résultat (`AutoResult::settings`), avec le
/// choix HDR de l'app et le câble : le JSON des réglages (`profile: "auto"`,
/// `codec`, `yuv444`, `mbps`, `reliable`, `hdr`, `wired`, `adaptive`,
/// `encoder_max` : la limite trouvée par le test, sinon nul), comme
/// `settings` de `kn_session_start`. Nul si le JSON est illisible. À libérer
/// par `kn_string_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_settings(result_json: *const c_char, hdr: bool, wired: bool) -> *mut c_char {
    match result(result_json) {
        Some(r) => give(r.settings(hdr, wired).to_json().to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Le plafond de l'encodeur à garder pour cet ordinateur quand ce résultat
/// est appliqué (`AutoResult::next_encoder_max`, docs/profil-auto.md),
/// d'après celui qu'il avait (`previous`, 0 : aucun) : en Mbit/s par écran,
/// 0 pour aucun. `previous` pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_next_encoder_max(result_json: *const c_char, previous: u32) -> u32 {
    let previous = (previous > 0).then_some(previous);
    match result(result_json) {
        Some(r) => r.next_encoder_max(previous).unwrap_or(0),
        None => previous.unwrap_or(0),
    }
}

/// Le test a-t-il été fait sur ce réseau ? `network` : celui du moment (nom
/// donné au test), `relay` par le relais, vide : inconnu. Vrai aussi pour un
/// JSON illisible (pas de bandeau sans raison).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_autotest_same_network(result_json: *const c_char, network: *const c_char) -> bool {
    result(result_json).is_none_or(|r| r.same_network(optional(network).as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn count(userdata: *mut c_void, s: *const KnAutoSample) -> f64 {
        let n = unsafe { &mut *(userdata as *mut Vec<(u32, bool, usize)>) };
        let packets = unsafe { kn_auto_sample_packet_count(s) };
        let (mut len, mut key) = (0usize, false);
        let first = unsafe { kn_auto_sample_packet(s, 0, &mut len, &mut key) };
        assert!(!first.is_null() && len > 0 && key);
        assert!(unsafe { kn_auto_sample_packet(s, packets, &mut len, &mut key) }.is_null());
        assert_eq!((unsafe { kn_auto_sample_width(s) }, unsafe { kn_auto_sample_height(s) }), (3840, 2160));
        n.push((unsafe { kn_auto_sample_fourcc(s) }, unsafe { kn_auto_sample_yuv444(s) }, packets));
        -1.0
    }

    #[test]
    fn extraits_lus_par_le_pont() {
        let mut seen: Vec<(u32, bool, usize)> = Vec::new();
        let cb = KnAutoCallbacks { userdata: &mut seen as *mut _ as *mut c_void, decode: Some(count), on_progress: None, on_done: None };
        let caps = Capabilities { hevc: true, hevc444: true, av1: true, hdr: false, pyrowave: false };
        for s in autotest::samples_for(caps) {
            let handle = KnAutoSample { sample: &s };
            let fps = unsafe { (cb.decode.unwrap())(cb.userdata, &handle) };
            assert!(fps < 0.0);
        }
        assert!(seen.iter().any(|(c, yuv, _)| *c == u32::from_be_bytes(*b"h265") && !yuv));
        assert!(seen.iter().any(|(c, yuv, _)| *c == u32::from_be_bytes(*b"h265") && *yuv));
        assert!(seen.iter().all(|(_, _, n)| *n >= 1));
    }

    #[test]
    fn aides_du_resultat() {
        assert!(unsafe { kn_autotest_choices(std::ptr::null(), std::ptr::null()) }.is_null());
        assert!(unsafe { kn_autotest_settings(std::ptr::null(), false, false) }.is_null());
        assert!(unsafe { kn_autotest_same_network(std::ptr::null(), std::ptr::null()) });

        use kybernow_core::autotest::{Link, Measures, Station, decide};
        let caps = Capabilities { hevc: true, hevc444: true, av1: true, hdr: true, pyrowave: false };
        let device = Device { screen: Mode { width: 2560, height: 1440, refresh: 60 }, caps, hdr: false, network: Some("Maison".into()), wired: false };
        let m = Measures { link: Link::Wifi, latency: None, bandwidth: None, decode: Vec::new(), encoder: None };
        let r = decide(&m, &device, &Station { name: "PC".into(), ..Default::default() }, 1);
        let json = cstring(&serde_json::to_string(&r).unwrap());
        let take = |p: *mut c_char| unsafe { CString::from_raw(p) }.into_string().unwrap();
        let choices: serde_json::Value = serde_json::from_str(&take(unsafe { kn_autotest_choices(json.as_ptr(), cstring("fr").as_ptr()) })).unwrap();
        assert_eq!(choices.as_array().map(Vec::len), Some(6));
        assert_eq!(choices[0]["title"], "Format vidéo");
        let settings: serde_json::Value = serde_json::from_str(&take(unsafe { kn_autotest_settings(json.as_ptr(), false, false) })).unwrap();
        assert_eq!((settings["profile"].as_str(), settings["adaptive"].as_bool()), (Some("auto"), Some(true)));
        assert!(unsafe { kn_autotest_same_network(json.as_ptr(), cstring("Maison").as_ptr()) });
        assert!(!unsafe { kn_autotest_same_network(json.as_ptr(), cstring("relay").as_ptr()) });
    }
}
