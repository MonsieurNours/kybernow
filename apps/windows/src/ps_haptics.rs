//! Vibrations HD de la DualSense branchée en USB (docs/playstation.md,
//! « Vibrations HD ») : la manette (ou une DualSense Edge) est aussi une
//! carte son à quatre voies, dont les voies 3 et 4 tiennent ses moteurs. Le
//! cœur rééchantillonne à 48 kHz le flux de vibrations de la console et le
//! garde dans un anneau (`kybernow_core::playstation::dualsense`) ; ici,
//! seulement la sortie : WASAPI en mode partagé sur la carte son de la
//! manette, dont le fil tire les échantillons (`HdHaptics::fill`).
//!
//! Une seule sortie pour l'app (le cœur n'a qu'un anneau) : prise par chaque
//! partie ([`retain`]), rendue à sa fin ([`release`]). La carte son est
//! cherchée toutes les 2 s tant qu'une partie tourne : manette branchée
//! pendant la partie (sa carte son apparaît un instant après elle),
//! débranchée ou passée en Bluetooth (le cœur revient aux vibrations
//! simples). Le son du jeu n'est pas touché : il reste sur la sortie par
//! défaut de Windows.
//!
//! La carte son se reconnaît à son nom (« Wireless Controller », « DualSense
//! Wireless Controller », « DualSense Edge Wireless Controller ») et à ses
//! quatre voies.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::GUID;

use kybernow_core::playstation::dualsense;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 4;
/// Voies des moteurs : 3 et 4 (à partir de 0 : 2 et 3).
const FIRST: usize = 2;

/// `PKEY_Device_FriendlyName` (« Haut-parleurs (Wireless Controller) »).
const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };

struct Output {
    users: usize,
    stop: Option<Arc<AtomicBool>>,
}

static OUTPUT: Mutex<Output> = Mutex::new(Output { users: 0, stop: None });

/// Le fil de sortie en cours : un fil remplacé (partie rendue puis reprise
/// aussitôt) ne coupe plus la sortie de celui qui le suit.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Seul le fil en cours dit au cœur si la sortie est ouverte.
fn set_output(generation: u64, on: bool) {
    if GENERATION.load(Ordering::SeqCst) == generation {
        dualsense::hd_haptics().set_output(on);
    }
}

/// L'objet d'attente de WASAPI, fermé quoi qu'il arrive.
struct Event(HANDLE);

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Une partie commence : la sortie s'ouvre dès qu'une DualSense est branchée en USB.
pub fn retain() {
    let mut o = OUTPUT.lock().unwrap();
    o.users += 1;
    if o.users > 1 {
        return;
    }
    let stop = Arc::new(AtomicBool::new(false));
    o.stop = Some(stop.clone());
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let spawned = std::thread::Builder::new().name("ps-vibrations-hd".into()).spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let mut last = String::new();
        while !stop.load(Ordering::Relaxed) {
            match unsafe { find_controller() } {
                Some((name, device)) => {
                    log::info!("PlayStation : vibrations HD par « {name} »");
                    match unsafe { play(&device, &stop, generation) } {
                        Ok(()) => last.clear(),
                        Err(e) => {
                            let msg = e.to_string();
                            if msg != last {
                                log::warn!("PlayStation : sortie des vibrations HD fermée ({msg})");
                                last = msg;
                            }
                            // Carte son prise par une autre app, format refusé : on réessaie plus tard.
                            set_output(generation, false);
                            std::thread::sleep(Duration::from_secs(2));
                        }
                    }
                    set_output(generation, false);
                }
                None => std::thread::sleep(Duration::from_secs(2)),
            }
        }
        set_output(generation, false);
    });
    if let Err(e) = spawned {
        log::warn!("PlayStation : fil des vibrations HD refusé ({e})");
    }
}

/// La partie se termine : la dernière ferme la sortie.
pub fn release() {
    let mut o = OUTPUT.lock().unwrap();
    o.users = o.users.saturating_sub(1);
    if o.users == 0 {
        if let Some(stop) = o.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Le nom d'une carte son de DualSense (comparaison sans casse).
pub fn is_controller_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("dualsense") || n.contains("wireless controller")
}

/// La carte son d'une DualSense branchée en USB, s'il y en a une.
unsafe fn find_controller() -> Option<(String, IMMDevice)> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let list = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE).ok()?;
        let count = list.GetCount().ok()?;
        for i in 0..count {
            let Ok(device) = list.Item(i) else { continue };
            let Some(name) = friendly_name(&device) else { continue };
            if !is_controller_name(&name) {
                continue;
            }
            // Quatre voies : la carte son de la manette en USB (en Bluetooth, il n'y en a pas).
            let Ok(client) = device.Activate::<IAudioClient>(CLSCTX_ALL, None) else { continue };
            let Ok(mix) = client.GetMixFormat() else { continue };
            let channels = (*mix).nChannels;
            CoTaskMemFree(Some(mix as *const _));
            if u32::from(channels) >= CHANNELS {
                return Some((name, device));
            }
        }
        None
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
        CoTaskMemFree(Some(text.0 as *const _));
        s
    }
}

/// Joue les vibrations jusqu'à l'arrêt ou une erreur (manette débranchée).
unsafe fn play(device: &IMMDevice, stop: &AtomicBool, generation: u64) -> windows::core::Result<()> {
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: CHANNELS as u16,
                nSamplesPerSec: RATE,
                nAvgBytesPerSec: RATE * 4 * CHANNELS,
                nBlockAlign: (4 * CHANNELS) as u16,
                wBitsPerSample: 32,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
            // Avant gauche, avant droite, arrière gauche, arrière droite : les
            // moteurs sont sur les deux dernières.
            dwChannelMask: 0x1 | 0x2 | 0x10 | 0x20,
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        };
        // Tampon court (20 ms) : les vibrations suivent l'image.
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            200_000,
            0,
            &fmt as *const _ as *const WAVEFORMATEX,
            None,
        )?;
        let event = Event(CreateEventW(None, false, false, None)?);
        client.SetEventHandle(event.0)?;
        let render: IAudioRenderClient = client.GetService()?;
        let size = client.GetBufferSize()?;
        client.Start()?;
        set_output(generation, true);
        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            if WaitForSingleObject(event.0, 200) != WAIT_OBJECT_0 {
                continue;
            }
            let padding = match client.GetCurrentPadding() {
                Ok(p) => p,
                Err(e) => break Err(e),
            };
            let avail = size.saturating_sub(padding);
            if avail == 0 {
                continue;
            }
            let buf = match render.GetBuffer(avail) {
                Ok(b) => b as *mut f32,
                Err(e) => break Err(e),
            };
            let out = std::slice::from_raw_parts_mut(buf, (avail * CHANNELS) as usize);
            dualsense::hd_haptics().fill(out, avail as usize, CHANNELS as usize, FIRST);
            if let Err(e) = render.ReleaseBuffer(avail, 0) {
                break Err(e);
            }
        };
        set_output(generation, false);
        let _ = client.Stop();
        result
    }
}
