//! Profil « Auto » (docs/profil-auto.md ; textes : docs/link-ui.md, « Profil
//! Auto et débit adaptatif ») : le test des conditions du cœur
//! (`kybernow_core::autotest::run`) pour Kotlin, et le décodeur de l'app qu'il
//! essaie (`SampleDecoder`).
//!
//! - `autotestStart` lance le test sur un fil à part, avec la console que
//!   l'app utilise pour cet ordinateur (relais compris), dont il prend la
//!   poignée ; l'avancement (`onProgress`, JSON `Progress`) et la fin
//!   (`onDone`) arrivent à l'objet Kotlin `AutotestListener`, sur ce fil.
//!   `autotestCancel` l'arrête entre deux mesures ; `autotestFree` l'arrête et
//!   rend la poignée (le fil finit seul, sans plus rien dire).
//! - Fin (`onDone`) : `{"result": AutoResult, "choices": [Choice]}`, ou
//!   `{"cancelled": true}`, ou `{"error": "Le test n'a pas pu joindre {nom}."}`.
//! - Le décodage : le même MediaCodec que la session (format de
//!   `video::format`, faible latence, clés des fabricants) pour HEVC, AV1 et
//!   H.264, et la même session Vulkan de PyroWave (`pyrowave::PwSession`) ;
//!   l'image va sur la surface d'un `AImageReader` hors écran au lieu de la
//!   `SurfaceView` (rien n'est montré). Chaque extrait est passé aussi vite que
//!   le décodeur le prend, en boucle depuis l'image clé, pendant 1 s ; images
//!   décodées par seconde, `None` si le décodeur refuse.
//!
//! Les fonctions JSON (appareil, choix, réglages, réseau) sont pures et
//! essayées hors d'Android (`cargo test -p kybernow-android`).

use serde_json::{json, Value};

use kybernow_core::autotest::{AutoError, AutoResult, Device};
use kybernow_core::console::Mode;
use kybernow_core::i18n::Lang;
use kybernow_core::profile::Capabilities;

/// L'appareil pour le test, d'après le JSON de Kotlin : `{"screen": {width,
/// height, refresh}, "caps": {hevc, hevc444, av1, hdr, pyrowave}, "hdr": bool,
/// "network": "Wi-Fi" | "Ethernet" | nom du Wi-Fi | null, "wired": bool}`.
pub fn device_from_json(s: &str) -> Option<Device> {
    let v: Value = serde_json::from_str(s).ok()?;
    let screen: Mode = serde_json::from_value(v.get("screen")?.clone()).ok()?;
    if screen.width == 0 || screen.height == 0 {
        return None;
    }
    let caps: Capabilities = match v.get("caps") {
        Some(c) => serde_json::from_value(c.clone()).ok()?,
        None => Capabilities::default(),
    };
    Some(Device {
        screen,
        caps,
        hdr: v.get("hdr").and_then(Value::as_bool).unwrap_or(false),
        network: v.get("network").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
        wired: v.get("wired").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// La fin du test pour `onDone` ; `name` : le nom de l'ordinateur, pour
/// « Le test n'a pas pu joindre {nom}. ».
pub fn done_json(r: &Result<AutoResult, AutoError>, lang: Lang, name: &str) -> String {
    match r {
        Ok(result) => json!({ "result": result, "choices": result.choices(lang) }),
        Err(AutoError::Cancelled) => json!({ "cancelled": true }),
        Err(e) => json!({ "error": e.text(lang, name) }),
    }
    .to_string()
}

fn result(s: &str) -> Option<AutoResult> {
    serde_json::from_str(s).ok()
}

/// Le résumé d'un résultat gardé : `[{key, title, value, reason}]`.
pub fn choices_json(result_json: &str, lang: Lang) -> Option<String> {
    result(result_json).map(|r| serde_json::to_string(&r.choices(lang)).unwrap_or_default())
}

/// Les réglages de session d'Auto (`AutoResult::settings`), le JSON de `sessionStart`.
pub fn settings_json(result_json: &str, hdr: bool, wired: bool) -> Option<String> {
    result(result_json).map(|r| r.settings(hdr, wired).to_json().to_string())
}

/// Le plafond de l'encodeur à garder pour l'ordinateur quand ce résultat est
/// appliqué (« Appliquer » à la fin d'un test : `AutoResult::next_encoder_max`),
/// d'après celui qu'il avait (`previous`) ; un résultat illisible le laisse.
pub fn next_encoder_max(result_json: &str, previous: Option<u32>) -> Option<u32> {
    result(result_json).map_or(previous, |r| r.next_encoder_max(previous))
}

/// Le test a-t-il été fait sur ce réseau (`relay` par le relais) ? Un
/// résultat illisible compte comme fait ailleurs.
pub fn same_network(result_json: &str, network: Option<&str>) -> bool {
    result(result_json).is_some_and(|r| r.same_network(network))
}

#[cfg(target_os = "android")]
mod android {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use jni::objects::{JClass, JObject, JString, JValue};
    use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE};
    use ndk::hardware_buffer::HardwareBufferUsage;
    use ndk::media::image_reader::{AcquireResult, ImageFormat, ImageReader};
    use ndk::media::media_codec::{DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec, MediaCodecDirection};

    use kybernow_core::autotest::{self, Progress, Sample, SampleDecoder, Step, StepProgress, StepState};
    use kybernow_core::console::Console;
    use kybernow_core::i18n;
    use kybernow_core::profile::Codec;

    use crate::exports::{give, jbool, optional, string, Env};
    use crate::jvm::Listener;
    use crate::pyrowave::{PwConfig, PwSession, Shown};

    const BUFFER_FLAG_KEY_FRAME: u32 = 1;
    /// Durée de l'essai d'un extrait, une fois la première image sortie.
    const RUN: Duration = Duration::from_secs(1);
    /// Sans image sortie au bout de ce temps, le décodeur ne sait pas faire.
    const FIRST_IMAGE: Duration = Duration::from_secs(2);

    /// Un test en cours : son drapeau d'arrêt et l'objet Kotlin qui l'écoute.
    struct Job {
        cancel: Arc<AtomicBool>,
        /// Kotlin a rendu la poignée : plus rien ne lui est dit.
        gone: Arc<AtomicBool>,
    }

    /// Lance le test : `console` est une poignée de `consoleNew…` que le test
    /// prend (Kotlin ne la libère pas) ; `device` le JSON de
    /// [`super::device_from_json`] ; `name` le nom de l'ordinateur (textes) ;
    /// `listener` reçoit `onProgress(String)` et `onDone(String)`. 0 si
    /// l'appareil est illisible (la console est alors libérée).
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestStart(
        mut env: Env,
        _: JClass,
        console: jlong,
        device: JString,
        name: JString,
        listener: JObject,
    ) -> jlong {
        kybernow_core::ffi_guard!({
            if console == 0 {
                return 0;
            }
            let console = Arc::new(*unsafe { Box::from_raw(console as *mut Console) });
            let (device, name) = (string(&mut env, &device), string(&mut env, &name));
            let Some(device) = super::device_from_json(&device) else { return 0 };
            let Ok(listener) = Listener::new(&mut env, &listener) else { return 0 };
            let cancel = Arc::new(AtomicBool::new(false));
            let gone = Arc::new(AtomicBool::new(false));
            let (c, g) = (cancel.clone(), gone.clone());
            let spawned = std::thread::Builder::new().name("kybernow-autotest".into()).spawn(move || {
                let lang = i18n::current();
                let say = |method: &str, json: String| {
                    if g.load(Ordering::Acquire) {
                        return;
                    }
                    listener.with(|env, obj| {
                        let s = env.new_string(&json)?;
                        env.call_method(obj, method, "(Ljava/lang/String;)V", &[JValue::Object(&s)]).map(|_| ())
                    });
                };
                let mut progress = |p: &Progress| say("onProgress", serde_json::to_string(p).unwrap_or_default());
                // Les quatre étapes tout de suite, la liaison en cours : le cœur ne parle qu'après l'annonce de l'ordinateur.
                progress(&Progress {
                    steps: Step::ALL
                        .iter()
                        .map(|&step| StepProgress { step, title: step.title(lang), state: if step == Step::Link { StepState::Running } else { StepState::Waiting } })
                        .collect(),
                });
                let mut decoder = AppDecoder { cancel: c.clone() };
                let r = autotest::run(console, &device, &mut decoder, &mut progress, &c, lang);
                match &r {
                    Ok(res) => log::info!("profil Auto : {} {}×{} à {} Hz, {} Mbit/s", res.codec.label(), res.width, res.height, res.refresh, res.mode_mbps),
                    Err(e) => log::warn!("profil Auto : test arrêté ({e:?})"),
                }
                say("onDone", super::done_json(&r, lang, &name));
            });
            if spawned.is_err() {
                return 0;
            }
            Box::into_raw(Box::new(Job { cancel, gone })) as jlong
        })
    }

    /// Rend la poignée : le test s'arrête et ne dit plus rien.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestFree(_: Env, _: JClass, h: jlong) {
        kybernow_core::ffi_guard!({
            if h != 0 {
                let j = unsafe { Box::from_raw(h as *mut Job) };
                j.gone.store(true, Ordering::Release);
                j.cancel.store(true, Ordering::Release);
            }
        })
    }

    /// Le résumé d'un résultat gardé, dans la langue de l'app ; nul s'il est illisible.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestChoices(mut env: Env, _: JClass, result: JString) -> jstring {
        kybernow_core::ffi_guard!(null => {
            let r = string(&mut env, &result);
            match super::choices_json(&r, i18n::current()) {
                Some(s) => give(&mut env, &s),
                None => std::ptr::null_mut(),
            }
        })
    }

    /// Les réglages de session d'Auto pour ce résultat ; nul s'il est illisible.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestSettings(
        mut env: Env,
        _: JClass,
        result: JString,
        hdr: jboolean,
        wired: jboolean,
    ) -> jstring {
        kybernow_core::ffi_guard!(null => {
            let r = string(&mut env, &result);
            match super::settings_json(&r, hdr != JNI_FALSE, wired != JNI_FALSE) {
                Some(s) => give(&mut env, &s),
                None => std::ptr::null_mut(),
            }
        })
    }

    /// « Appliquer » : le plafond de l'encodeur à garder (Mbit/s par écran, 0 : aucun), d'après celui gardé (`previous`, 0 : aucun).
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestEncoderMax(mut env: Env, _: JClass, result: JString, previous: jint) -> jint {
        kybernow_core::ffi_guard!(previous => {
            let r = string(&mut env, &result);
            let previous = u32::try_from(previous).ok().filter(|&p| p > 0);
            super::next_encoder_max(&r, previous).map_or(0, |n| n.min(i32::MAX as u32) as jint)
        })
    }

    /// Le test a-t-il été fait sur ce réseau (`network` : celui du moment, `relay` par le relais) ?
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_autotestSameNetwork(
        mut env: Env,
        _: JClass,
        result: JString,
        network: JString,
    ) -> jboolean {
        kybernow_core::ffi_guard!({
            let (r, n) = (string(&mut env, &result), optional(&mut env, &network));
            jbool(super::same_network(&r, n.as_deref()))
        })
    }

    /// Le décodeur de l'app pour le test : MediaCodec ou PyroWave, sur une
    /// surface hors écran.
    struct AppDecoder {
        cancel: Arc<AtomicBool>,
    }

    impl SampleDecoder for AppDecoder {
        fn decode(&mut self, s: &Sample) -> Option<f64> {
            if s.packets.is_empty() {
                return None;
            }
            let fps = match s.codec {
                Codec::PyroWave => pyrowave(s, &self.cancel),
                _ => media_codec(s, &self.cancel),
            };
            log::info!(
                "profil Auto : décodage {}{} {}×{} : {}",
                s.codec.label(),
                if s.yuv444 { " 4:4:4" } else { "" },
                s.width,
                s.height,
                fps.map_or("refusé".to_string(), |f| format!("{f:.0} i/s"))
            );
            fps
        }
    }

    /// La surface hors écran : un `AImageReader` au format opaque, lisible
    /// par le GPU (ce que demandent MediaCodec et une swapchain Vulkan).
    fn offscreen(s: &Sample, images: i32) -> Option<ImageReader> {
        ImageReader::new_with_usage(s.width as i32, s.height as i32, ImageFormat::PRIVATE, HardwareBufferUsage::GPU_SAMPLED_IMAGE, images)
            .map_err(|e| log::warn!("profil Auto : surface hors écran impossible ({e:?})"))
            .ok()
    }

    /// Images par seconde : celles sorties après la première, sur le temps écoulé depuis elle.
    fn rate(first: Option<Instant>, last: Instant, frames: u32) -> Option<f64> {
        let first = first?;
        let secs = last.duration_since(first).as_secs_f64();
        (frames > 0 && secs > 0.0).then(|| f64::from(frames) / secs)
    }

    fn media_codec(s: &Sample, cancel: &AtomicBool) -> Option<f64> {
        let mime = match s.codec {
            Codec::Hevc => "video/hevc",
            Codec::Av1 => "video/av01",
            _ => "video/avc",
        };
        let reader = offscreen(s, 4)?;
        let window = reader.window().ok()?;
        let codec = MediaCodec::from_decoder_type(mime)?;
        let configured = codec
            .configure(&crate::video::format(mime, s.width, s.height, false, None, true), Some(&window), MediaCodecDirection::Decoder)
            .or_else(|_| codec.configure(&crate::video::format(mime, s.width, s.height, false, None, false), Some(&window), MediaCodecDirection::Decoder));
        if configured.is_err() || codec.start().is_err() {
            return None;
        }
        let started = Instant::now();
        let (mut next, mut pts) = (0usize, 0u64);
        let (mut first, mut last, mut frames) = (None::<Instant>, started, 0u32);
        let mut failed = false;
        'run: loop {
            if cancel.load(Ordering::Relaxed) || first.map_or(started.elapsed() > FIRST_IMAGE, |f| f.elapsed() >= RUN) {
                break;
            }
            // Entrée : le paquet suivant, en boucle depuis l'image clé (ses en-têtes devant).
            match codec.dequeue_input_buffer(Duration::from_millis(2)) {
                Ok(DequeuedInputBufferResult::Buffer(mut buf)) => {
                    let p = &s.packets[next];
                    let dst = buf.buffer_mut();
                    if p.data.len() > dst.len() {
                        let _ = codec.queue_input_buffer(buf, 0, 0, pts, 0);
                        failed = true;
                        break;
                    }
                    // Copie dans le tampon du codec (assez grand : vérifié plus haut).
                    unsafe { std::ptr::copy_nonoverlapping(p.data.as_ptr(), dst.as_mut_ptr() as *mut u8, p.data.len()) };
                    let flags = if p.is_key { BUFFER_FLAG_KEY_FRAME } else { 0 };
                    if codec.queue_input_buffer(buf, 0, p.data.len(), pts, flags).is_err() {
                        failed = true;
                        break;
                    }
                    pts += 16_667;
                    next = (next + 1) % s.packets.len();
                }
                Ok(DequeuedInputBufferResult::TryAgainLater) => {}
                Err(_) => {
                    failed = true;
                    break;
                }
            }
            // Sortie : chaque image décodée est comptée puis rendue sans être montrée.
            loop {
                match codec.dequeue_output_buffer(Duration::ZERO) {
                    Ok(DequeuedOutputBufferInfoResult::Buffer(b)) => {
                        let now = Instant::now();
                        if first.is_none() {
                            first = Some(now);
                        } else {
                            frames += 1;
                        }
                        last = now;
                        let _ = codec.release_output_buffer(b, false);
                    }
                    Ok(DequeuedOutputBufferInfoResult::TryAgainLater) => break,
                    Ok(_) => {}
                    Err(_) => {
                        failed = true;
                        break 'run;
                    }
                }
            }
        }
        let _ = codec.stop();
        drop(codec);
        drop(window);
        drop(reader);
        if failed || cancel.load(Ordering::Relaxed) {
            return None;
        }
        rate(first, last, frames)
    }

    fn pyrowave(s: &Sample, cancel: &AtomicBool) -> Option<f64> {
        let reader = offscreen(s, 2)?;
        let window = reader.window().ok()?;
        let mut pw = PwSession::open(window).map_err(|e| log::warn!("profil Auto : PyroWave impossible ({e})")).ok()?;
        let config = PwConfig { chroma444: s.yuv444, hdr: false, full_range: true, width: s.width & !1, height: s.height & !1 };
        if !pw.configure(config, None) {
            return None;
        }
        let started = Instant::now();
        let (mut first, mut last, mut frames) = (None::<Instant>, started, 0u32);
        let mut next = 0usize;
        let mut lost = false;
        while !cancel.load(Ordering::Relaxed) && !first.map_or(started.elapsed() > FIRST_IMAGE, |f| f.elapsed() >= RUN) {
            match pw.frame(&s.packets[next].data) {
                Shown::Shown => {
                    let now = Instant::now();
                    if first.is_none() {
                        first = Some(now);
                    } else {
                        frames += 1;
                    }
                    last = now;
                }
                Shown::Skipped => {}
                Shown::Lost => {
                    lost = true;
                    break;
                }
            }
            next = (next + 1) % s.packets.len();
            // L'image présentée est reprise tout de suite : la swapchain ne manque jamais d'images libres.
            while let Ok(AcquireResult::Image(img)) = reader.acquire_latest_image() {
                drop(img);
            }
        }
        drop(pw);
        drop(reader);
        if lost || cancel.load(Ordering::Relaxed) {
            return None;
        }
        rate(first, last, frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::autotest::{EncoderMeasure, Link, Measures};

    #[test]
    fn appareil_lu() {
        let d = device_from_json(
            r#"{"screen":{"width":3840,"height":2160,"refresh":60},"caps":{"hevc":true,"hevc444":false,"av1":true,"hdr":false,"pyrowave":true},"hdr":true,"network":"Ethernet","wired":true}"#,
        )
        .unwrap();
        assert_eq!((d.screen.width, d.screen.height, d.screen.refresh), (3840, 2160, 60));
        assert!(d.caps.hevc && d.caps.av1 && d.caps.pyrowave && !d.caps.hevc444);
        assert!(d.hdr && d.wired);
        assert_eq!(d.network.as_deref(), Some("Ethernet"));
        let d = device_from_json(r#"{"screen":{"width":1920,"height":1080,"refresh":120},"network":" "}"#).unwrap();
        assert_eq!(d.network, None, "nom vide : réseau inconnu");
        assert!(!d.caps.hevc && !d.wired);
        assert!(device_from_json(r#"{"screen":{"width":0,"height":1080,"refresh":60}}"#).is_none());
        assert!(device_from_json("{}").is_none());
    }

    fn resultat(network: Option<&str>) -> AutoResult {
        let device = device_from_json(r#"{"screen":{"width":3840,"height":2160,"refresh":60},"caps":{"hevc":true}}"#).unwrap();
        let measures = Measures { link: Link::Wifi, latency: None, bandwidth: None, decode: Vec::new(), encoder: None };
        kybernow_core::autotest::decide(&measures, &Device { network: network.map(str::to_string), ..device }, &Default::default(), 1_700_000_000)
    }

    #[test]
    fn fin_choix_reglages_reseau() {
        let r = resultat(Some("Wi-Fi"));
        let done: Value = serde_json::from_str(&done_json(&Ok(r.clone()), Lang::Fr, "Bureau")).unwrap();
        assert_eq!(done["choices"].as_array().unwrap().len(), 6);
        assert_eq!(done["choices"][0]["title"], "Format vidéo");
        let stored = done["result"].to_string();
        // Le résultat gardé relu : le même résumé, les réglages d'Auto, le réseau.
        let choices: Value = serde_json::from_str(&choices_json(&stored, Lang::En).unwrap()).unwrap();
        assert_eq!(choices[0]["title"], "Video format");
        let settings: Value = serde_json::from_str(&settings_json(&stored, false, true).unwrap()).unwrap();
        assert_eq!(settings["profile"], "auto");
        assert_eq!(settings["codec"], "hevc");
        assert_eq!(settings["adaptive"], true);
        assert_eq!(settings["wired"], true);
        assert!(same_network(&stored, Some("Wi-Fi")));
        assert!(!same_network(&stored, Some("Ethernet")));
        assert!(!same_network(&stored, Some("relay")));
        assert!(!same_network("illisible", Some("Wi-Fi")));
        assert!(choices_json("illisible", Lang::Fr).is_none());
        let cancelled: Value = serde_json::from_str(&done_json(&Err(AutoError::Cancelled), Lang::Fr, "Bureau")).unwrap();
        assert_eq!(cancelled["cancelled"], true);
    }

    #[test]
    fn plafond_encodeur_applique() {
        let mesure = |mbps, limited| EncoderMeasure {
            codec: kybernow_core::profile::Codec::Hevc,
            yuv444: false,
            width: 3840,
            height: 2160,
            refresh: 60,
            mbps,
            limited,
        };
        let avec = |encoder: Option<EncoderMeasure>| {
            let device = device_from_json(r#"{"screen":{"width":3840,"height":2160,"refresh":60},"caps":{"hevc":true}}"#).unwrap();
            let measures = Measures { link: Link::Cable, latency: None, bandwidth: None, decode: Vec::new(), encoder };
            serde_json::to_string(&kybernow_core::autotest::decide(&measures, &device, &Default::default(), 1_700_000_000)).unwrap()
        };
        // Limite trouvée : elle remplace celle gardée.
        let limite = avec(Some(mesure(200, true)));
        assert_eq!(next_encoder_max(&limite, None), Some(200));
        assert_eq!(next_encoder_max(&limite, Some(150)), Some(200));
        let settings: Value = serde_json::from_str(&settings_json(&limite, false, true).unwrap()).unwrap();
        assert_eq!(settings["encoder_max"], 200);
        // Accepté du premier coup au-dessus du plafond gardé : il est retiré ; en dessous, il reste.
        let ok = avec(Some(mesure(300, false)));
        assert_eq!(next_encoder_max(&ok, Some(250)), None);
        assert_eq!(next_encoder_max(&ok, Some(350)), Some(350));
        // Étape sautée (Station ancienne, PyroWave) ou résultat illisible : le plafond gardé reste.
        assert_eq!(next_encoder_max(&avec(None), Some(250)), Some(250));
        assert_eq!(next_encoder_max("illisible", Some(250)), Some(250));
        assert_eq!(next_encoder_max("illisible", None), None);
    }
}
