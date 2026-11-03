//! Une partie sur la console, du début à la fin : demande de session, canal
//! de contrôle, flux ; l'image et le son partent vers les gestionnaires de
//! paquets de l'app ([`PacketHandler`], le même chemin que les sessions
//! Kyber : décodeur vidéo et moteur de son).
//!
//! L'image : un `Codec` (H.264 ou HEVC), puis chaque image en Annex B
//! (`is_key` sur les images de référence). Le son : un `Codec` Opus, un
//! `OpusHead` (`is_config`), puis les trames, et `Hole` pour une trame
//! perdue.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::audio::{AudioOut, AudioReceiver, opus_head};
use super::regist::Registered;
use super::session::{self, Codec, Ctrl, CtrlEvent, Keys};
use super::stream::{Profile, Stream, StreamEvent};
use crate::media::{FOURCC_H264, FOURCC_HEVC, FOURCC_OPUS, Packet, PacketHandler};

/// Échantillons d'une trame de son (10 ms à 48 kHz).
const FRAME_SAMPLES: u32 = 480;

/// Où en est la partie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayState {
    /// Demande de session et canal de contrôle.
    Connecting,
    /// Le flux est ouvert : définition reçue.
    Streaming { width: u32, height: u32, hdr: bool },
    /// La console ne peut pas afficher ce qu'elle montre (contenu protégé).
    CantDisplay(bool),
    /// Fin, avec la raison (texte pour le journal).
    Ended(PlayEnd),
}

/// Pourquoi la partie s'arrête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayEnd {
    /// Arrêtée par l'app.
    Stopped,
    /// La console est déjà jouée à distance par un autre appareil.
    InUse,
    /// Clés refusées : il faut associer de nouveau.
    NotPaired,
    /// Injoignable, ou rien reçu.
    Unreachable(String),
    /// La console a terminé (mode repos, autre raison).
    Remote(String),
}

/// Réglages d'une partie.
#[derive(Debug, Clone, Copy)]
pub struct PlayOptions {
    pub profile: Profile,
}

/// Une partie en cours.
pub struct Play {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    ctrl: Arc<Mutex<Option<Ctrl>>>,
}

/// NAL de référence (image clé) dans une image en Annex B.
pub(crate) fn has_key_nal(data: &[u8], hevc: bool) -> bool {
    let mut i = 0;
    while i + 3 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            let h = data[i + 3];
            let key = if hevc { matches!((h >> 1) & 0x3f, 16..=21) } else { h & 0x1f == 5 };
            if key {
                return true;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    false
}

impl Play {
    /// Lance la partie sur un fil `ps-play`. `on_state` est appelé sur ce
    /// fil.
    pub fn start(
        addr: Ipv4Addr,
        registered: &Registered,
        options: PlayOptions,
        video: Arc<dyn PacketHandler>,
        audio: Option<Arc<dyn PacketHandler>>,
        on_state: Box<dyn Fn(PlayState) + Send>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ctrl = Arc::new(Mutex::new(None));
        let registered = registered.clone();
        let thread = {
            let stop = stop.clone();
            let ctrl = ctrl.clone();
            std::thread::Builder::new()
                .name("ps-play".into())
                .spawn(move || {
                    let end = run(addr, &registered, options, &video, audio.as_ref(), &on_state, &stop, &ctrl);
                    if let Some(mut c) = ctrl.lock().unwrap().take() {
                        c.stop();
                    }
                    on_state(PlayState::Ended(end));
                })
                .ok()
        };
        Play { stop, thread, ctrl }
    }

    /// Met la console en mode repos (puis la partie se termine).
    pub fn goto_bed(&self) {
        if let Some(c) = self.ctrl.lock().unwrap().as_ref() {
            let _ = c.goto_bed();
        }
    }

    /// Arrête la partie et attend sa fin.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Play {
    fn drop(&mut self) {
        self.stop();
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    addr: Ipv4Addr,
    registered: &Registered,
    options: PlayOptions,
    video: &Arc<dyn PacketHandler>,
    audio: Option<&Arc<dyn PacketHandler>>,
    on_state: &dyn Fn(PlayState),
    stop: &AtomicBool,
    ctrl_slot: &Mutex<Option<Ctrl>>,
) -> PlayEnd {
    on_state(PlayState::Connecting);
    let Some(keys) = Keys::from_registered(registered) else { return PlayEnd::NotPaired };
    // La console garde la session précédente quelques secondes : « déjà en
    // cours » est réessayé pendant 15 s.
    let started = Instant::now();
    let (target, nonce) = loop {
        match session::request_session(addr, registered.target.into(), &keys) {
            Ok(r) => break r,
            Err(session::SessionError::Rejected { reason: Some(super::regist::REASON_IN_USE), .. })
                if started.elapsed() < Duration::from_secs(15) && !stop.load(Ordering::SeqCst) =>
            {
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(session::SessionError::Rejected { reason: Some(super::regist::REASON_IN_USE), .. }) => return PlayEnd::InUse,
            Err(session::SessionError::Rejected { status: 403, .. }) => return PlayEnd::NotPaired,
            Err(e) => return PlayEnd::Unreachable(e.to_string()),
        }
    };
    let profile = options.profile;
    let codec = match (profile.hevc, profile.hdr) {
        (_, true) => Codec::H265Hdr,
        (true, false) => Codec::H265,
        _ => Codec::H264,
    };
    // Juste après un réveil, la console refuse parfois le canal de contrôle.
    let (ctrl, events) = {
        let mut tries = 0;
        loop {
            match Ctrl::connect(addr, target, &nonce, &keys, codec) {
                Ok(c) => break c,
                Err(_) if tries < 5 && !stop.load(Ordering::SeqCst) => {
                    tries += 1;
                    std::thread::sleep(Duration::from_secs(2));
                }
                Err(e) => return PlayEnd::Unreachable(e.to_string()),
            }
        }
    };
    let session_id = loop {
        if stop.load(Ordering::SeqCst) {
            return PlayEnd::Stopped;
        }
        match events.recv_timeout(Duration::from_millis(200)) {
            Ok(CtrlEvent::SessionId(id)) => break id,
            Ok(CtrlEvent::Closed(why)) => return PlayEnd::Unreachable(why),
            Ok(_) => {}
            Err(_) if started.elapsed() > Duration::from_secs(30) => return PlayEnd::Unreachable("pas d'identifiant de session".into()),
            Err(_) => {}
        }
    };
    let _ = ctrl.enable_features(profile.dualsense);
    let crypt = ctrl.crypt().clone();
    *ctrl_slot.lock().unwrap() = Some(ctrl);
    let (mut stream, stream_events) = match Stream::start(addr, target, session_id, crypt, profile) {
        Ok(s) => s,
        Err(e) => return PlayEnd::Unreachable(e.to_string()),
    };
    let hevc_out = profile.hevc || profile.hdr;
    video.packet(Packet::Codec { codec: if hevc_out { FOURCC_HEVC } else { FOURCC_H264 } });
    let mut receiver = AudioReceiver::new();
    let mut audio_configured = false;
    let clock = Instant::now();
    let end = loop {
        if stop.load(Ordering::SeqCst) {
            break PlayEnd::Stopped;
        }
        // Le canal de contrôle : fin et contenu protégé.
        while let Ok(e) = events.try_recv() {
            match e {
                CtrlEvent::CantDisplay(c) => on_state(PlayState::CantDisplay(c)),
                CtrlEvent::Closed(why) => log::info!("PlayStation : canal de contrôle fermé ({why})"),
                _ => {}
            }
        }
        let event = match stream_events.recv_timeout(Duration::from_millis(100)) {
            Ok(e) => e,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break PlayEnd::Remote("flux arrêté".into()),
        };
        match event {
            StreamEvent::Started { resolutions, audio: header } => {
                let (w, h) = resolutions.first().map(|r| (r.width, r.height)).unwrap_or((0, 0));
                on_state(PlayState::Streaming { width: w, height: h, hdr: profile.hdr });
                if let Some(a) = audio {
                    a.packet(Packet::Codec { codec: FOURCC_OPUS });
                    let head = opus_head(header.channels, header.rate);
                    a.packet(Packet::Media { is_config: true, is_key: false, pts: 0, data: &head });
                    audio_configured = true;
                }
            }
            StreamEvent::Video(frame) => {
                let pts = clock.elapsed().as_micros() as u64;
                let is_key = has_key_nal(&frame.data, hevc_out);
                video.packet(Packet::Media { is_config: false, is_key, pts, data: &frame.data });
            }
            StreamEvent::Audio(p) if !p.is_haptics => {
                let Some(a) = audio.filter(|_| audio_configured) else { continue };
                for out in receiver.push(&p) {
                    match out {
                        AudioOut::Frame(data) => {
                            let pts = clock.elapsed().as_micros() as u64;
                            a.packet(Packet::Media { is_config: false, is_key: true, pts, data: &data });
                        }
                        AudioOut::Hole => a.packet(Packet::Hole { missing_samples: FRAME_SAMPLES }),
                    }
                }
            }
            StreamEvent::Audio(_) | StreamEvent::Pad(..) => {}
            StreamEvent::Ended(why) => break PlayEnd::Remote(why),
        }
    };
    stream.stop();
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_nal_detection() {
        // HEVC : IDR_N_LP (20) → clé ; TRAIL_R (1) → non.
        assert!(has_key_nal(&[0, 0, 0, 1, 0x28, 0x01, 0xaa], true));
        assert!(!has_key_nal(&[0, 0, 0, 1, 0x02, 0x01, 0xaa], true));
        // Avec un en-tête VPS (32) devant une image ordinaire : pas une clé.
        assert!(!has_key_nal(&[0, 0, 1, 0x40, 0x01, 0, 0, 1, 0x02, 0x01], true));
        // H.264 : IDR (5).
        assert!(has_key_nal(&[0, 0, 1, 0x65, 0x88], false));
        assert!(!has_key_nal(&[0, 0, 1, 0x41, 0x88], false));
    }
}
