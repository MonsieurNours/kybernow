//! Son de la session : Opus décodé par le moteur du cœur (tampon adaptatif,
//! masquage des pertes, dérive des horloges, mixage vers le bas), tiré par le
//! rappel de SDL (PipeWire ou PulseAudio ; ALSA sur KyberNow Link OS), aux
//! voies de la sortie : stéréo, 5.1 ou 7.1, dans l'ordre de SDL (celui de
//! Windows : G, D, C, LFE, arrière, côtés).
//!
//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : une sortie SDL par
//! session, que le système mélange. Le cœur coupe lui-même le son des sessions qui ne sont ni
//! actives ni gardées par « Garder le son » (`Session::set_muted`, fondu de 20 ms, par le registre
//! `focus`) : il trouve le moteur de la sortie par `PacketHandler::audio_engine`. « Couper le son » du
//! menu de session est un autre choix, celui de l'utilisateur : le drapeau de la sortie
//! (`AudioSink::set_silenced`), appliqué ici, avec son propre fondu. Dans les deux cas le son continue
//! d'être décodé et vidé : il revient sans décalage.

use std::ffi::{c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use kybernow_core::audio::{AudioEngine, MonotonicClock, OutputOrder};
use kybernow_core::media::{Packet, PacketHandler};

use super::sdl;

const RATE: i32 = 48_000;

/// Durée du fondu quand le son se coupe ou revient (pas de claquement).
const FADE_MS: f32 = 20.0;

/// Ce que le rappel de SDL reçoit : le moteur et les voies qu'il remplit, le drapeau
/// « Couper le son » de la session et le niveau où le fondu en est (bits d'un `f32`, 0 à 1).
struct Output {
    engine: Arc<AudioEngine>,
    channels: usize,
    silenced: Arc<AtomicBool>,
    gain: AtomicU32,
}

pub struct AudioSink {
    engine: Arc<AudioEngine>,
    device: Mutex<u32>,
    holder: Mutex<Option<Box<Output>>>,
    /// Voies de la sortie (celles demandées à l'ordinateur) ; SDL convertit
    /// vers la carte son si elle en a d'autres.
    channels: u8,
    /// « Couper le son » du menu de la session. Partagé avec les autres sorties de la même session
    /// (une reconnexion en recrée une).
    silenced: Arc<AtomicBool>,
}

impl AudioSink {
    /// `silenced` : le drapeau « Couper le son » de la session ; la sortie le lit à chaque rappel.
    pub fn new(channels: u8, silenced: Arc<AtomicBool>) -> Arc<Self> {
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self { engine, device: Mutex::new(0), holder: Mutex::new(None), channels: channels.max(2), silenced })
    }

    pub fn engine(&self) -> &Arc<AudioEngine> {
        &self.engine
    }

    fn open(&self) {
        let mut dev = self.device.lock().unwrap();
        if *dev != 0 {
            return;
        }
        let start = if self.silenced.load(Ordering::Relaxed) { 0.0f32 } else { 1.0 };
        let holder = Box::new(Output { engine: self.engine.clone(), channels: self.channels as usize, silenced: self.silenced.clone(), gain: AtomicU32::new(start.to_bits()) });
        let user = &*holder as *const Output as *mut c_void;
        // 256 images (5,3 ms) par rappel : PipeWire tient la cadence.
        let want = sdl::SDL_AudioSpec { freq: RATE, format: sdl::AUDIO_F32SYS, channels: self.channels, silence: 0, samples: 256, padding: 0, size: 0, callback: Some(fill), userdata: user };
        let mut have = sdl::SDL_AudioSpec::empty();
        let id = unsafe { sdl::SDL_OpenAudioDevice(std::ptr::null(), 0, &want, &mut have, 0) };
        if id == 0 {
            log::error!("son : sortie SDL impossible ({})", sdl::error());
            return;
        }
        let ms = have.samples as f64 * 1000.0 / have.freq.max(1) as f64;
        self.engine.set_output_latency_ms(ms * 2.0);
        log::info!("son : SDL ({}) {} voies, {} Hz, {} images par rappel", sdl::audio_driver(), have.channels, have.freq, have.samples);
        *self.holder.lock().unwrap() = Some(holder);
        unsafe { sdl::SDL_PauseAudioDevice(id, 0) };
        *dev = id;
    }
}

extern "C" fn fill(user: *mut c_void, stream: *mut u8, len: c_int) {
    let out_ = unsafe { &*(user as *const Output) };
    let n = len.max(0) as usize / 4;
    let out = unsafe { std::slice::from_raw_parts_mut(stream as *mut f32, n) };
    let channels = out_.channels.max(1);
    out_.engine.fill(out, n / channels);
    let target = if out_.silenced.load(Ordering::Relaxed) { 0.0f32 } else { 1.0 };
    let mut gain = f32::from_bits(out_.gain.load(Ordering::Relaxed));
    apply_gain(out, channels, &mut gain, target, 1.0 / (FADE_MS / 1000.0 * RATE as f32));
    out_.gain.store(gain.to_bits(), Ordering::Relaxed);
}

/// Le niveau du son : il rejoint `target` (0 ou 1) d'au plus `step` par image, pour que couper ou
/// rendre le son ne claque pas. Quand il y est (le cas de presque tous les rappels), le tampon
/// n'est touché que s'il faut le taire.
fn apply_gain(out: &mut [f32], channels: usize, gain: &mut f32, target: f32, step: f32) {
    if *gain == target {
        if target == 0.0 {
            out.fill(0.0);
        }
        return;
    }
    for frame in out.chunks_mut(channels) {
        *gain = if *gain < target { (*gain + step).min(target) } else { (*gain - step).max(target) };
        for s in frame {
            *s *= *gain;
        }
    }
}

impl PacketHandler for AudioSink {
    /// Le moteur que le cœur coupe et rend pour les sessions en arrière-plan (`Session::set_muted`).
    fn audio_engine(self: Arc<Self>) -> Option<Arc<AudioEngine>> {
        Some(self.engine.clone())
    }

    fn packet(&self, packet: Packet<'_>) {
        match packet {
            Packet::Codec { .. } => {}
            Packet::Media { is_config: true, data, .. } => {
                self.engine.configure(data);
                self.open();
            }
            Packet::Media { data, .. } => {
                if data.is_empty() {
                    return;
                }
                // Paquet avant l'en-tête : stéréo supposée (comme le moteur).
                self.open();
                self.engine.decode(data);
            }
            Packet::Hole { missing_samples } => self.engine.conceal(missing_samples as usize),
        }
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        self.engine.stop();
        let id = *self.device.lock().unwrap();
        if id != 0 {
            unsafe { sdl::SDL_CloseAudioDevice(id) };
        }
        self.holder.lock().unwrap().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Couper : le niveau descend en vingt millisecondes (960 images), sans saut ; rendre : il remonte ;
    /// un son coupé depuis longtemps est un silence franc.
    #[test]
    fn muting_fades_instead_of_clicking() {
        let step = 1.0 / (FADE_MS / 1000.0 * RATE as f32);
        let mut out = vec![1.0f32; 2 * 1200];
        let mut gain = 1.0;
        apply_gain(&mut out, 2, &mut gain, 0.0, step);
        assert_eq!(gain, 0.0);
        // Descente régulière, aucune marche plus haute qu'un pas.
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        assert!(left.windows(2).all(|w| w[0] - w[1] <= step * 1.001 && w[1] <= w[0]), "pas de marche");
        assert!(left[0] > 0.99 && left[959] < 0.01 && left[1100] == 0.0);
        // Les deux voies d'une image ont le même niveau.
        assert!(out.chunks(2).all(|f| f[0] == f[1]));
        // Déjà coupé : silence franc, niveau inchangé.
        let mut out = vec![0.5f32; 64];
        apply_gain(&mut out, 2, &mut gain, 0.0, step);
        assert!(out.iter().all(|s| *s == 0.0) && gain == 0.0);
        // Rendu : le niveau remonte, il n'est plus touché une fois à 1.
        let mut out = vec![1.0f32; 2 * 1200];
        apply_gain(&mut out, 2, &mut gain, 1.0, step);
        assert_eq!(gain, 1.0);
        assert!(out[0] < 0.01 && out[2 * 959] > 0.99 && out[2 * 1100] == 1.0);
        let mut out = vec![0.25f32; 8];
        apply_gain(&mut out, 2, &mut gain, 1.0, step);
        assert!(out.iter().all(|s| *s == 0.25));
    }
}
