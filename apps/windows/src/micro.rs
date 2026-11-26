//! Le micro de ce PC, envoyé à l'ordinateur (docs/micro.md, docs/link-ui.md
//! « Micro ») : capture WASAPI partagée, mono en flottants à la fréquence du
//! mélangeur de Windows, sur un fil à part, donnée au cœur
//! (`Session::push_microphone`), qui encode et envoie.
//!
//! Le micro : celui que les Réglages de l'app ont choisi s'il est branché,
//! sinon celui du système pour les communications (`eCapture`,
//! `eCommunications`). Un micro choisi débranché n'est pas oublié : la capture
//! y revient dès qu'il est rebranché.
//!
//! « Réduire l'écho et le bruit » allumé : catégorie « Communications »
//! (`AudioCategory_Communications`), qui donne au flux le traitement voix du
//! pilote du micro quand il en a un (écho, bruit) ; coupé : catégorie
//! « Autre » et mode brut (`AUDCLNT_STREAMOPTIONS_RAW`) quand le pilote le
//! permet, sans aucun traitement.
//!
//! Accès refusé par la confidentialité de Windows (`E_ACCESSDENIED`) : le fil
//! s'arrête et le dit (l'app coupe le micro et montre l'avis).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::{GUID, HSTRING, PCWSTR};

/// `PKEY_Device_FriendlyName` (« Microphone (Realtek Audio) »).
const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

/// Tampon demandé à Windows : 20 ms (unités de 100 ns).
const BUFFER_HNS: i64 = 200_000;

/// Les choix des Réglages de l'app.
#[derive(Debug, Clone)]
struct Prefs {
    /// Le micro choisi (identifiant de Windows) ; `None` : celui du système.
    device: Option<String>,
    /// « Réduire l'écho et le bruit ».
    voice: bool,
}

static PREFS: Mutex<Prefs> = Mutex::new(Prefs { device: None, voice: true });
/// Change à chaque choix : une capture en cours rouvre le micro.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Les choix des Réglages de l'app (au lancement, puis à chaque changement) :
/// une capture en cours les prend tout de suite.
pub fn set_prefs(device: Option<String>, voice: bool) {
    *PREFS.lock().unwrap() = Prefs { device, voice };
    GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Un micro de ce PC, pour les Réglages de l'app.
#[derive(Debug, Clone, Serialize)]
pub struct MicDevice {
    pub id: String,
    pub name: String,
}

/// Les micros branchés et actifs de ce PC. Sur un fil à part : COM y est
/// ouvert en multithread sans toucher au fil qui demande (celui des fenêtres,
/// ou celui de Tauri, qui doivent rester en STA).
pub fn devices() -> Vec<MicDevice> {
    std::thread::Builder::new()
        .name("kybernow-micro-liste".into())
        .spawn(|| list_devices())
        .ok()
        .and_then(|t| t.join().ok())
        .unwrap_or_default()
}

fn list_devices() -> Vec<MicDevice> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return Vec::new();
        };
        let Ok(list) = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else { return Vec::new() };
        let count = list.GetCount().unwrap_or(0);
        (0..count)
            .filter_map(|i| {
                let device = list.Item(i).ok()?;
                let id = device_id(&device)?;
                let name = friendly_name(&device).unwrap_or_else(|| id.clone());
                Some(MicDevice { id, name })
            })
            .collect()
    }
}

/// Ce PC a au moins un micro : la puce « Micro » et le réglage se montrent.
pub fn has_microphone() -> bool {
    !devices().is_empty()
}

unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    unsafe {
        let id = device.GetId().ok()?;
        let s = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const c_void));
        s
    }
}

unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let mut value = store.GetValue(&FRIENDLY_NAME).ok()?;
        let text = PropVariantToStringAlloc(&value).ok();
        let _ = PropVariantClear(&mut value);
        let text = text?;
        let s = text.to_string().ok();
        CoTaskMemFree(Some(text.0 as *const c_void));
        s
    }
}

/// Où vont les échantillons : mono, flottants, à la fréquence donnée.
pub type Sink = Arc<dyn Fn(&[f32], u32) + Send + Sync>;

/// Une capture en cours, sur son fil ; arrêtée quand elle est lâchée.
pub struct Capture {
    stop: Arc<AtomicBool>,
}

impl Capture {
    /// Capte jusqu'à l'arrêt. `denied` : Windows refuse l'accès au micro
    /// (confidentialité) ; le fil s'arrête juste après.
    pub fn start(sink: Sink, denied: Box<dyn FnOnce() + Send>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let spawned = std::thread::Builder::new().name("kybernow-micro".into()).spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            // Sans micro, l'ouverture échoue toutes les 500 ms : la même erreur
            // n'est notée qu'une fois de suite.
            let mut last = String::new();
            while !s.load(Ordering::Relaxed) {
                match unsafe { capture(&sink, &s) } {
                    Ok(()) => last.clear(),
                    Err(e) if e.code() == E_ACCESSDENIED => {
                        log::warn!("micro : accès refusé par Windows (confidentialité)");
                        denied();
                        return;
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg != last {
                            log::warn!("micro : {msg}");
                            last = msg;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        });
        if let Err(e) = spawned {
            log::error!("micro : fil refusé ({e})");
        }
        Self { stop }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Le micro choisi s'il est branché, sinon celui du système ; `true` : c'est
/// celui du système à la place du micro choisi (débranché).
unsafe fn open_device(enumerator: &IMMDeviceEnumerator, chosen: Option<&str>) -> windows::core::Result<(IMMDevice, bool)> {
    unsafe {
        if let Some(id) = chosen {
            let wide = HSTRING::from(id);
            if let Ok(device) = enumerator.GetDevice(PCWSTR(wide.as_ptr())) {
                if device.GetState().is_ok_and(|s| s == DEVICE_STATE_ACTIVE) {
                    return Ok((device, false));
                }
            }
        }
        Ok((enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications)?, chosen.is_some()))
    }
}

/// Le micro choisi, débranché, est de nouveau là.
unsafe fn chosen_back(enumerator: &IMMDeviceEnumerator, id: &str) -> bool {
    unsafe {
        let wide = HSTRING::from(id);
        enumerator.GetDevice(PCWSTR(wide.as_ptr())).is_ok_and(|d| d.GetState().is_ok_and(|s| s == DEVICE_STATE_ACTIVE))
    }
}

/// Capte jusqu'à l'arrêt, un autre choix dans les réglages, le retour du
/// micro choisi ou une erreur (micro débranché, celui du système changé).
unsafe fn capture(sink: &Sink, stop: &AtomicBool) -> windows::core::Result<()> {
    unsafe {
        let generation = GENERATION.load(Ordering::SeqCst);
        let prefs = PREFS.lock().unwrap().clone();
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let (device, fallback) = open_device(&enumerator, prefs.device.as_deref())?;
        let client: IAudioClient2 = device.Activate(CLSCTX_ALL, None)?;

        // Traitement voix du système (catégorie Communications), ou rien du
        // tout (mode brut, si le pilote le permet).
        let mut props = AudioClientProperties {
            cbSize: std::mem::size_of::<AudioClientProperties>() as u32,
            bIsOffload: false.into(),
            eCategory: if prefs.voice { AudioCategory_Communications } else { AudioCategory_Other },
            Options: if prefs.voice { AUDCLNT_STREAMOPTIONS_NONE } else { AUDCLNT_STREAMOPTIONS_RAW },
        };
        let mut raw = !prefs.voice;
        if let Err(e) = client.SetClientProperties(&props) {
            if raw {
                // Le pilote n'a pas de mode brut : catégorie « Autre » seule.
                raw = false;
                props.Options = AUDCLNT_STREAMOPTIONS_NONE;
                if let Err(e) = client.SetClientProperties(&props) {
                    log::warn!("micro : catégorie du flux refusée ({e})");
                }
            } else {
                log::warn!("micro : catégorie Communications refusée ({e})");
            }
        }

        // Mono, flottants, à la fréquence du mélangeur (Windows convertit les voies).
        let mix = client.GetMixFormat()?;
        let rate = (*mix).nSamplesPerSec;
        CoTaskMemFree(Some(mix as *const c_void));
        let fmt = WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: 1,
                nSamplesPerSec: rate,
                nAvgBytesPerSec: rate * 4,
                nBlockAlign: 4,
                wBitsPerSample: 32,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
            dwChannelMask: 0x4,
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        };
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            BUFFER_HNS,
            0,
            &fmt as *const _ as *const WAVEFORMATEX,
            None,
        )?;
        let event: HANDLE = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let capture: IAudioCaptureClient = client.GetService()?;
        let name = friendly_name(&device).unwrap_or_default();
        log::info!(
            "micro : « {name} »{}, {rate} Hz, {}",
            if fallback { " (celui du système, le micro choisi est débranché)" } else { "" },
            if prefs.voice { "traitement voix du système" } else if raw { "mode brut" } else { "sans traitement voix" }
        );
        client.Start()?;
        let mut samples: Vec<f32> = Vec::with_capacity(rate as usize / 50);
        let mut checked = Instant::now();
        let result = 'outer: loop {
            if stop.load(Ordering::Relaxed) || GENERATION.load(Ordering::SeqCst) != generation {
                break Ok(());
            }
            // Le micro choisi revient (rebranché) : il reprend sa place.
            if fallback && checked.elapsed() >= Duration::from_secs(2) {
                checked = Instant::now();
                if prefs.device.as_deref().is_some_and(|id| chosen_back(&enumerator, id)) {
                    log::info!("micro : le micro choisi est rebranché");
                    break Ok(());
                }
            }
            if WaitForSingleObject(event, 200) != WAIT_OBJECT_0 {
                continue;
            }
            loop {
                let packet = match capture.GetNextPacketSize() {
                    Ok(n) => n,
                    Err(e) => break 'outer Err(e),
                };
                if packet == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let (mut frames, mut flags) = (0u32, 0u32);
                if let Err(e) = capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None) {
                    break 'outer Err(e);
                }
                if frames > 0 {
                    samples.clear();
                    if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data.is_null() {
                        samples.resize(frames as usize, 0.0);
                    } else {
                        samples.extend_from_slice(std::slice::from_raw_parts(data as *const f32, frames as usize));
                    }
                    sink(&samples, rate);
                }
                if let Err(e) = capture.ReleaseBuffer(frames) {
                    break 'outer Err(e);
                }
            }
        };
        let _ = client.Stop();
        let _ = CloseHandle(event);
        result
    }
}
