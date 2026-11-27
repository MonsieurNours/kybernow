//! Le micro de l'appareil vers l'ordinateur (docs/micro.md) : capture par SDL2 (PipeWire ou PulseAudio, ALSA sur
//! KyberNow Link OS), mono, en flottants, à 48 kHz si le micro le donne (sinon sa fréquence : le cœur ramène à
//! 48 kHz). Le rappel de capture donne ses échantillons à `Session::push_microphone`, qui ne bloque pas.
//!
//! « Réduire l'écho et le bruit » : le rôle « communication » du flux (`SDL_AUDIO_DEVICE_STREAM_ROLE`). PulseAudio
//! charge alors son annulation d'écho (module-filter-apply) ; PipeWire l'applique si son module d'écho
//! (`libpipewire-module-echo-cancel`) est chargé. Sinon le son part tel quel : Linux n'a pas de traitement voix
//! commun à tous les systèmes.

use std::ffi::{c_int, c_void, CString};
use std::sync::{Mutex, Weak};

use kybernow_core::session::Session;

use super::sdl;

/// Le rôle du flux se donne par un indice global de SDL, lu à l'ouverture : les sorties ne doivent pas le prendre.
pub static OPEN: Mutex<()> = Mutex::new(());

/// Les micros de l'appareil, par leur nom chez SDL.
pub fn devices() -> Vec<String> {
    let n = unsafe { sdl::SDL_GetNumAudioDevices(1) };
    (0..n.max(0))
        .filter_map(|i| {
            let p = unsafe { sdl::SDL_GetAudioDeviceName(i, 1) };
            (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned())
        })
        .filter(|n| !n.is_empty())
        .collect()
}

/// Ce que le rappel de SDL reçoit.
struct Sink {
    session: Weak<Session>,
    rate: u32,
}

/// Une capture ouverte ; fermée en la lâchant.
pub struct Capture {
    device: u32,
    _sink: Box<Sink>,
}

impl Capture {
    /// Ouvre `name` (`None` : celui du système) pour cette session ; `None` si SDL ne donne pas le micro.
    pub fn open(session: Weak<Session>, name: Option<&str>, echo: bool) -> Option<Capture> {
        let mut sink = Box::new(Sink { session, rate: 48_000 });
        let user = &mut *sink as *mut Sink as *mut c_void;
        // 480 images : une trame de 10 ms à 48 kHz.
        let want = sdl::SDL_AudioSpec { freq: 48_000, format: sdl::AUDIO_F32SYS, channels: 1, silence: 0, samples: 480, padding: 0, size: 0, callback: Some(captured), userdata: user };
        let mut have = sdl::SDL_AudioSpec::empty();
        let cname = name.and_then(|n| CString::new(n).ok());
        let id = {
            let _guard = OPEN.lock().unwrap();
            if echo {
                sdl::hint("SDL_AUDIO_DEVICE_STREAM_ROLE", "communication");
            }
            // La fréquence du micro est permise (le cœur la ramène à 48 kHz) ; mono et flottants, SDL convertit.
            let id = unsafe { sdl::SDL_OpenAudioDevice(cname.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()), 1, &want, &mut have, sdl::SDL_AUDIO_ALLOW_FREQUENCY_CHANGE) };
            if echo {
                sdl::reset_hint("SDL_AUDIO_DEVICE_STREAM_ROLE");
            }
            id
        };
        if id == 0 {
            log::warn!("micro : capture SDL impossible ({})", sdl::error());
            return None;
        }
        sink.rate = have.freq.max(1) as u32;
        log::info!("micro : SDL ({}) {}, {} Hz, {} images par rappel, écho et bruit {}", sdl::audio_driver(), name.unwrap_or("celui du système"), have.freq, have.samples, if echo { "réduits (rôle communication)" } else { "tels quels" });
        unsafe { sdl::SDL_PauseAudioDevice(id, 0) };
        Some(Capture { device: id, _sink: sink })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // Fermer attend la fin du rappel : le `Sink` vit jusque-là.
        unsafe { sdl::SDL_CloseAudioDevice(self.device) };
    }
}

/// Rappel de SDL (fil du son) : une panique n'en sort pas (`ffi_guard!` du cœur), le tampon est alors perdu.
extern "C" fn captured(user: *mut c_void, stream: *mut u8, len: c_int) {
    kybernow_core::ffi_guard!({
        let sink = unsafe { &*(user as *const Sink) };
        let n = len.max(0) as usize / 4;
        let samples = unsafe { std::slice::from_raw_parts(stream as *const f32, n) };
        if let Some(s) = sink.session.upgrade() {
            s.push_microphone(samples, sink.rate);
        }
    })
}

/// La vue de l'interface (`MicView`) : `MicUi::ui_json` du cœur, textes dans la langue de l'app (raccourci de la puce
/// compris), et si la puce est montrée (`MicUi::shown` : pas un invité que l'hôte ne laisse pas parler).
pub fn view(m: &kybernow_core::micro::MicUi, guest: bool) -> crate::micro::MicView {
    let mut v = m.ui_json(kybernow_core::i18n::current(), &crate::micro::shortcut());
    v["shown"] = serde_json::Value::Bool(m.shown(guest));
    serde_json::from_value(v).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::micro::{MicRefusal, MicState, MicUi};

    /// La vue de l'interface est celle du cœur : « Envoi… » dans la tuile tant que l'ordinateur n'a pas répondu, la
    /// raison d'un refus, la puce cachée pour un invité que l'hôte ne laisse pas parler.
    #[test]
    fn vue_du_micro_du_coeur() {
        kybernow_core::i18n::set_current(kybernow_core::i18n::Lang::Fr);
        kybernow_core::micro::set_device_has_microphone(true);
        let mut m = MicUi::new(false);
        m.set(true);
        assert_eq!(m.changed(MicState::Waiting, "Atelier"), None);
        let v = view(&m, false);
        assert_eq!((v.on, v.lit, v.sent, v.tile.as_str(), v.shown), (true, true, false, "Envoi…", true));
        // Le texte du cœur (son catalogue garde l'espace ordinaire), avec le raccourci de l'app.
        assert_eq!(v.tooltip, "Micro : envoyé à l'ordinateur (Ctrl+Alt+Maj+V)");
        assert_eq!(m.changed(MicState::On, "Atelier").as_deref(), Some("Micro envoyé à Atelier."));
        assert_eq!(view(&m, false).tile, "Envoyé");
        m.changed(MicState::Refused(MicRefusal::NotAllowed), "Atelier");
        let v = view(&m, true);
        assert_eq!((v.shown, v.lit, v.reason.as_deref()), (false, false, Some("L'hôte ne laisse pas les invités parler.")));
    }
}
