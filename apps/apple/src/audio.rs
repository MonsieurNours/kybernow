//! Pont C du moteur de son du cœur (`kybernow_core::audio`, voir
//! docs/coeur-apple.md) : Opus, tampon adaptatif, dérive, masquage, mixage
//! descendant. Swift ne garde que la sortie (Core Audio, RemoteIO) : son fil
//! temps réel tire les échantillons par `kn_audio_fill`.
//!
//! Fils : `configure`, `decode`, `conceal` depuis le fil réseau de Kyber
//! (paquets de son) ; `fill` et `take_stats` depuis le fil temps réel de la
//! carte son, sans verrou bloquant ; le reste de n'importe quel fil.

use std::sync::Arc;

use kybernow_core::audio::{AudioEngine, MonotonicClock, OutputOrder};

/// Ordre des voies attendu par la sortie (`kn_audio_new`).
pub const KN_AUDIO_ORDER_OPUS: u32 = 0;
pub const KN_AUDIO_ORDER_WINDOWS: u32 = 1;
pub const KN_AUDIO_ORDER_CORE_AUDIO: u32 = 2;

/// Mesures du son, une fois par seconde.
#[repr(C)]
pub struct KnAudioStats {
    pub channels: u32,
    /// Tampon moyen avant la carte son, en millisecondes.
    pub buffer_ms: f64,
    /// Latence de la sortie, telle que la plateforme l'a donnée.
    pub output_ms: f64,
    /// Trous comblés dans la seconde.
    pub gaps: u32,
    /// Voies envoyées à la sortie (moins que le flux : mixage descendant).
    pub output_channels: u32,
}

/// Le moteur de son d'un flux.
pub struct KnAudio(AudioEngine);

#[unsafe(no_mangle)]
pub extern "C" fn kn_audio_new(order: u32) -> *mut KnAudio {
    let order = match order {
        KN_AUDIO_ORDER_WINDOWS => OutputOrder::Windows,
        KN_AUDIO_ORDER_CORE_AUDIO => OutputOrder::CoreAudio,
        _ => OutputOrder::Opus,
    };
    Box::into_raw(Box::new(KnAudio(AudioEngine::with_order(Arc::new(MonotonicClock::new()), order))))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_free(a: *mut KnAudio) {
    if !a.is_null() {
        drop(unsafe { Box::from_raw(a) });
    }
}

fn bytes<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() || len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data, len) } }
}

/// En-tête `OpusHead` du flux (paquet de configuration) : décodeur ouvert.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_configure(a: *const KnAudio, data: *const u8, len: usize) {
    if let Some(a) = unsafe { a.as_ref() } {
        a.0.configure(bytes(data, len));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_decode(a: *const KnAudio, data: *const u8, len: usize) {
    if let Some(a) = unsafe { a.as_ref() } {
        let data = bytes(data, len);
        if !data.is_empty() {
            a.0.decode(data);
        }
    }
}

/// Données perdues (`missing_samples` d'un trou) : raccord inventé par Opus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_conceal(a: *const KnAudio, samples: u32) {
    if let Some(a) = unsafe { a.as_ref() } {
        a.0.conceal(samples as usize);
    }
}

/// Voies du flux (0 avant la configuration).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_channels(a: *const KnAudio) -> u32 {
    unsafe { a.as_ref() }.map_or(0, |a| a.0.channels() as u32)
}

/// Fil temps réel de la sortie : `frames` images audio entrelacées de
/// `channels` voies (celles de la sortie) écrites dans `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_fill(a: *const KnAudio, out: *mut f32, frames: usize, channels: usize) {
    if out.is_null() || frames == 0 || channels == 0 {
        return;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out, frames * channels) };
    match unsafe { a.as_ref() } {
        Some(a) => a.0.fill(out, frames),
        None => out.fill(0.0),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_set_volume(a: *const KnAudio, volume: f32) {
    if let Some(a) = unsafe { a.as_ref() } {
        a.0.set_volume(volume);
    }
}

/// Latence de la sortie (tampon de la carte son et ce qu'elle annonce), en ms.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_set_output_latency_ms(a: *const KnAudio, latency: f64) {
    if let Some(a) = unsafe { a.as_ref() } {
        a.0.set_output_latency_ms(latency);
    }
}

/// Mesures de la seconde écoulée, une fois prêtes (`false` sinon).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_take_stats(a: *const KnAudio, out: *mut KnAudioStats) -> bool {
    let (Some(a), Some(out)) = (unsafe { a.as_ref() }, unsafe { out.as_mut() }) else { return false };
    let Some(s) = a.0.take_stats() else { return false };
    *out = KnAudioStats {
        channels: s.channels as u32,
        buffer_ms: s.buffer_ms,
        output_ms: s.output_ms,
        gaps: s.gaps as u32,
        output_channels: s.output_channels as u32,
    };
    true
}

/// Fin du flux : plus rien n'est décodé, la sortie reçoit du silence.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_audio_stop(a: *const KnAudio) {
    if let Some(a) = unsafe { a.as_ref() } {
        a.0.stop();
    }
}
