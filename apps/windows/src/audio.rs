//! Sortie du son : WASAPI en mode partagé, piloté par événement, qui tire les
//! échantillons du moteur de son du cœur (`kybernow_core::audio::AudioEngine` :
//! libopus, masquage des pertes, tampon adaptatif, dérive rattrapée) depuis
//! son fil. Les voies sont celles du flux, dans l'ordre de Windows
//! (WAVEFORMATEXTENSIBLE) ; Windows convertit fréquence et voies vers la
//! sortie si besoin. La sortie est rouverte quand le flux change de nombre de
//! voies ou qu'elle tombe en erreur (sortie du système changée).

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::*;
use windows::Win32::Media::KernelStreaming::WAVE_FORMAT_EXTENSIBLE;
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};

use kybernow_core::audio::{AudioEngine, Clock, OutputOrder, SAMPLE_RATE};

const RATE: u32 = SAMPLE_RATE as u32;

fn channel_mask(channels: u32) -> u32 {
    match channels {
        1 => 0x4,
        2 => 0x3,
        6 => 0x3F,
        8 => 0x63F,
        _ => 0,
    }
}

/// Le moteur de son de la session et le fil WASAPI qui le joue.
pub struct AudioOutput {
    engine: Arc<AudioEngine>,
    stop: Arc<AtomicBool>,
}

impl AudioOutput {
    /// Le moteur, qui reçoit les paquets de son de Kyber (`PacketHandler`).
    pub fn engine(&self) -> &Arc<AudioEngine> {
        &self.engine
    }

    /// Fin de session : le fil de la sortie s'arrête, le décodeur est fermé.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.engine.stop();
    }
}

/// Voies de la sortie son par défaut (2, 6 ou 8), à demander à la console.
/// Stéréo si elle ne se lit pas, même sans sortie du tout : le son vient
/// alors sur ce PC (0 le laisserait sur l'ordinateur) et le fil du son
/// ouvre la sortie dès qu'elle apparaît (casque branché en session).
pub fn default_output_channels() -> u16 {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return 2;
        };
        let Ok(device) = enumerator.GetDefaultAudioEndpoint(eRender, eConsole) else { return 2 };
        let Ok(client) = device.Activate::<IAudioClient>(CLSCTX_ALL, None) else { return 2 };
        let Ok(mix) = client.GetMixFormat() else { return 2 };
        let ch = (*mix).nChannels;
        CoTaskMemFree(Some(mix as *const c_void));
        match ch {
            0..=2 => 2,
            3..=6 => 6,
            _ => 8,
        }
    }
}

pub fn spawn(clock: Arc<dyn Clock>) -> AudioOutput {
    let engine = Arc::new(AudioEngine::with_order(clock, OutputOrder::Windows));
    let stop = Arc::new(AtomicBool::new(false));
    let (e, s) = (engine.clone(), stop.clone());
    std::thread::Builder::new()
        .name("kybernow-audio".into())
        .spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            // Sans sortie, l'ouverture échoue toutes les 500 ms : la même
            // erreur n'est notée qu'une fois de suite.
            let mut last = String::new();
            while !s.load(Ordering::Relaxed) {
                match unsafe { play(&e, &s) } {
                    Ok(()) => last.clear(),
                    Err(e) => {
                        let msg = e.to_string();
                        if msg != last {
                            log::warn!("son : {msg}");
                            last = msg;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        })
        .expect("fil du son");
    AudioOutput { engine, stop }
}

/// Voies à ouvrir : celles du flux, stéréo tant qu'il n'est pas configuré.
fn wanted_channels(engine: &AudioEngine) -> u32 {
    match engine.channels() {
        0 => 2,
        n => n as u32,
    }
}

/// Joue jusqu'à l'arrêt, une erreur de la sortie ou un changement de voies.
unsafe fn play(engine: &AudioEngine, stop: &AtomicBool) -> windows::core::Result<()> {
    unsafe {
        let channels = wanted_channels(engine);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let fmt = WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: channels as u16,
                nSamplesPerSec: RATE,
                nAvgBytesPerSec: RATE * 4 * channels,
                nBlockAlign: (4 * channels) as u16,
                wBitsPerSample: 32,
                cbSize: 22,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
            dwChannelMask: channel_mask(channels),
            SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
        };
        // Windows convertit fréquence et voies vers la sortie si besoin.
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            0,
            0,
            &fmt as *const _ as *const WAVEFORMATEX,
            None,
        )?;
        let event: HANDLE = CreateEventW(None, false, false, None)?;
        client.SetEventHandle(event)?;
        let render: IAudioRenderClient = client.GetService()?;
        let size = client.GetBufferSize()?;
        // Latence de la sortie pour l'écart son-image : son tampon plus ce
        // que Windows annonce (unités de 100 ns).
        let buffer_ms = f64::from(size) * 1000.0 / f64::from(RATE);
        let stream_ms = client.GetStreamLatency().map(|l| l as f64 / 10_000.0).unwrap_or(0.0);
        engine.set_output_latency_ms(buffer_ms + stream_ms);
        log::info!("son : WASAPI, {channels} voies, tampon de {buffer_ms:.0} ms, latence annoncée {stream_ms:.1} ms");
        client.Start()?;
        let result = loop {
            if stop.load(Ordering::Relaxed) || wanted_channels(engine) != channels {
                break Ok(());
            }
            if WaitForSingleObject(event, 200) != WAIT_OBJECT_0 {
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
            let out = std::slice::from_raw_parts_mut(buf, (avail * channels) as usize);
            // Silence tant qu'il n'y a pas de flux ; masquage d'Opus sur un tampon vide.
            engine.fill(out, avail as usize);
            if let Err(e) = render.ReleaseBuffer(avail, 0) {
                break Err(e);
            }
        };
        let _ = client.Stop();
        let _ = CloseHandle(event);
        result
    }
}
