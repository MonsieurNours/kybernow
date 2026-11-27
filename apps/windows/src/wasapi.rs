//! Ce que la sortie du son (`audio.rs`) et le micro (`micro.rs`) partagent
//! autour de WASAPI : le fil (COM multithread, priorité MMCSS « Pro Audio »),
//! le format flottant 32 bits, l'événement du mode piloté par événement
//! (fermé quoi qu'il arrive), le suivi du périphérique par défaut de Windows
//! et la boucle qui rouvre le périphérique après une erreur.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW};
use windows::core::w;

/// Le périphérique par défaut est relu au plus à ce rythme.
const DEFAULT_CHECK: Duration = Duration::from_secs(1);
/// Après une erreur d'ouverture, on réessaie au bout de ce temps.
const RETRY: Duration = Duration::from_millis(500);

/// Le fil d'un flux WASAPI : COM multithread et priorité MMCSS « Pro Audio »
/// (le planificateur de Windows ne le fait plus attendre derrière les autres),
/// rendus à la fin du fil.
pub struct AudioThread {
    mmcss: Option<HANDLE>,
    com: bool,
}

impl AudioThread {
    pub fn enter(what: &str) -> Self {
        let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let mut index = 0u32;
        let mmcss = match unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut index) } {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("{what} : priorité MMCSS refusée ({e})");
                None
            }
        };
        Self { mmcss, com }
    }
}

impl Drop for AudioThread {
    fn drop(&mut self) {
        unsafe {
            if let Some(h) = self.mmcss.take() {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
            if self.com {
                CoUninitialize();
            }
        }
    }
}

/// L'événement que WASAPI signale à chaque tampon, fermé quand il est lâché
/// (aussi quand l'ouverture échoue après sa création).
pub struct Event(HANDLE);

impl Event {
    pub fn new() -> windows::core::Result<Self> {
        Ok(Self(unsafe { CreateEventW(None, false, false, None)? }))
    }

    pub fn handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Flottants 32 bits, `channels` voies dans l'ordre de `mask`.
pub fn float_format(channels: u32, rate: u32, mask: u32) -> WAVEFORMATEXTENSIBLE {
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: channels as u16,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * 4 * channels,
            nBlockAlign: (4 * channels) as u16,
            wBitsPerSample: 32,
            cbSize: 22,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
        dwChannelMask: mask,
        SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
    }
}

/// L'identifiant de Windows d'un périphérique.
pub unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    unsafe {
        let id = device.GetId().ok()?;
        let s = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const std::ffi::c_void));
        s
    }
}

/// Suit le périphérique par défaut de Windows (sortie ou micro de
/// communication) : un flux ouvert sur un `IMMDevice` y reste tant qu'il
/// existe, même quand on choisit « Casque » dans le menu son de Windows ;
/// la boucle du flux le rouvre quand `changed` le dit.
pub struct DefaultWatch {
    flow: EDataFlow,
    role: ERole,
    id: Option<String>,
    checked: Instant,
}

impl DefaultWatch {
    /// Suivi du périphérique par défaut, en partant de `device` (celui ouvert).
    pub unsafe fn new(flow: EDataFlow, role: ERole, device: &IMMDevice) -> Self {
        Self { flow, role, id: unsafe { device_id(device) }, checked: Instant::now() }
    }

    /// Le périphérique par défaut n'est plus celui ouvert (relu une fois par
    /// seconde au plus).
    pub unsafe fn changed(&mut self, enumerator: &IMMDeviceEnumerator) -> bool {
        if self.checked.elapsed() < DEFAULT_CHECK {
            return false;
        }
        self.checked = Instant::now();
        let now = unsafe { enumerator.GetDefaultAudioEndpoint(self.flow, self.role).ok().and_then(|d| device_id(&d)) };
        default_changed(self.id.as_deref(), now.as_deref())
    }
}

/// Le défaut a changé : un autre identifiant. Aucun défaut (sortie
/// débranchée) n'en est pas un : l'erreur du flux s'en charge.
pub fn default_changed(opened: Option<&str>, now: Option<&str>) -> bool {
    matches!((opened, now), (Some(a), Some(b)) if a != b)
}

/// Rouvre le flux tant que `stop` n'est pas levé : `once` joue jusqu'à une
/// fin normale (changement de voies ou de périphérique) ou une erreur ; après
/// une erreur, nouvel essai au bout de 500 ms, la même erreur notée une fois
/// de suite. Une erreur pour laquelle `fatal` est vrai arrête et la rend.
pub fn keep_open(
    what: &str,
    stop: &AtomicBool,
    fatal: impl Fn(&windows::core::Error) -> bool,
    mut once: impl FnMut() -> windows::core::Result<()>,
) -> windows::core::Result<()> {
    let mut last = String::new();
    while !stop.load(Ordering::Relaxed) {
        match once() {
            Ok(()) => last.clear(),
            Err(e) if fatal(&e) => return Err(e),
            Err(e) => {
                let msg = e.to_string();
                if msg != last {
                    log::warn!("{what} : {msg}");
                    last = msg;
                }
                std::thread::sleep(RETRY);
            }
        }
    }
    Ok(())
}
