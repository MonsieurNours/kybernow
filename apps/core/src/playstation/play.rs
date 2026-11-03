//! Une partie sur la console, du début à la fin : demande de session, canal
//! de contrôle, flux ; l'image et le son partent vers les gestionnaires de
//! paquets de l'app ([`PacketHandler`], le même chemin que les sessions
//! Kyber : décodeur vidéo et moteur de son).
//!
//! L'image : un `Codec` (H.264 ou HEVC), puis chaque image en Annex B
//! (`is_key` sur les images de référence). Le son : un `Codec` Opus, un
//! `OpusHead` (`is_config`), puis les trames, et `Hole` pour une trame
//! perdue.
//!
//! La DualSense ([`super::dualsense`]) : vibrations HD vers la carte son de
//! la manette branchée en USB (sinon vibrations simples), intensités des
//! vibrations et des gâchettes ([`Play::set_intensities`]). Le clavier de la
//! console ([`KeyboardEvent`], [`Play::keyboard_text`]) : quand elle ouvre
//! son clavier à l'écran, l'app y écrit.

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::audio::{AudioOut, AudioReceiver, opus_head};
use super::controller::ControllerState;
use super::dualsense::{self, HapticsFrames, Intensities};
use super::keymouse::{KeyMouse, Layout};
use crate::gamepads;
use crate::input::pipeline::GamepadRumble;
use super::regist::Registered;
use super::session::{self, Codec, Ctrl, CtrlEvent, Keys};
use super::settings::PlaySettings;
use super::stream::{Stream, StreamEvent};
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

/// Ce que la manette dit à l'app (fil `ps-manette`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadEvent {
    /// Options + Create + L1 + R1 tenus 250 ms : ouvrir la barre ou le menu.
    Menu,
    /// Manette prise par l'app (barre ouverte) : une touche de navigation.
    Navigate(crate::input::PadKey),
}

/// Le clavier à l'écran de la console (docs/playstation.md, « Clavier de la
/// console »), annoncé à l'app sur le fil de la partie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardEvent {
    /// La console ouvre son clavier, avec le texte déjà dans son champ.
    Open(String),
    /// Le texte du champ, renvoyé par la console : il remplace celui de l'app.
    Text(String),
    /// La console a fermé son clavier (ou la partie s'est interrompue).
    Closed,
}

type KeyboardCb = Arc<dyn Fn(KeyboardEvent) + Send + Sync>;

/// Ce que la partie partage avec l'app pour la DualSense et le clavier de
/// la console.
struct Feedback {
    intensities: Intensities,
    on_keyboard: Mutex<Option<KeyboardCb>>,
}

impl Feedback {
    fn keyboard(&self, e: KeyboardEvent) {
        let cb = self.on_keyboard.lock().unwrap().clone();
        if let Some(cb) = cb {
            cb(e);
        }
    }
}

/// Mesures de la partie, remises à zéro à chaque lecture ([`Play::stats`]).
#[derive(Debug, Default)]
pub(crate) struct Counters {
    frames: AtomicU64,
    bytes: AtomicU64,
    lost: AtomicU64,
    recovered: AtomicU64,
}

/// Ce que montrent les statistiques, sur le temps depuis la lecture
/// précédente.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
pub struct PlayStats {
    pub width: u32,
    pub height: u32,
    pub hevc: bool,
    pub hdr: bool,
    /// Débit demandé à la console, kbit/s.
    pub target_kbps: u32,
    /// Images par seconde reçues.
    pub fps: f32,
    /// Débit reçu, Mbit/s.
    pub mbps: f32,
    /// Images perdues et réparées (correction d'erreurs) depuis la lecture
    /// précédente.
    pub lost: u64,
    pub recovered: u64,
}

/// Réglages d'une partie.
#[derive(Debug, Clone)]
pub struct PlayOptions {
    /// Les réglages de la console.
    pub settings: PlaySettings,
    /// L'appareil décode le H.265 (Auto en profite).
    pub hevc_decoder: bool,
}

/// Une partie en cours.
pub struct Play {
    stop: Arc<AtomicBool>,
    /// Clavier et souris traduits en manette (ignorés dès qu'une manette est
    /// branchée).
    keymouse: Arc<Mutex<KeyMouse>>,
    /// La barre ou le menu de l'app est ouvert : la console reçoit une
    /// manette au repos, la manette navigue dans l'app.
    captured: Arc<AtomicBool>,
    counters: Arc<Counters>,
    stats_at: Mutex<(Instant, PlayStats)>,
    /// Une image clé est attendue : demandée à la console jusqu'à ce qu'elle
    /// arrive (images perdues, décodeur de l'app qui a perdu sa référence,
    /// plus d'image depuis une seconde).
    want_key: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    ctrl: Arc<Mutex<Option<Ctrl>>>,
    feedback: Arc<Feedback>,
}

/// Une fin qui vaut une nouvelle tentative : réseau, ou flux coupé sans que
/// la console l'ait voulu (pas « Server shutting down » : mise en veille).
pub(crate) fn recoverable(end: &PlayEnd) -> bool {
    match end {
        PlayEnd::Unreachable(_) => true,
        PlayEnd::Remote(why) => !why.contains("shutting down"),
        PlayEnd::Stopped | PlayEnd::InUse | PlayEnd::NotPaired => false,
    }
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
        on_pad: Box<dyn Fn(PadEvent) + Send + Sync>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let want_key = Arc::new(AtomicBool::new(false));
        let keymouse = Arc::new(Mutex::new(KeyMouse::default()));
        let captured = Arc::new(AtomicBool::new(false));
        let counters = Arc::new(Counters::default());
        let on_pad: Arc<dyn Fn(PadEvent) + Send + Sync> = Arc::from(on_pad);
        let initial = {
            let p = options.settings.profile(options.hevc_decoder, true);
            PlayStats { width: p.width, height: p.height, hevc: p.hevc, hdr: p.hdr, target_kbps: p.bitrate, ..Default::default() }
        };
        let ctrl = Arc::new(Mutex::new(None));
        let (rumble_pct, triggers_pct) = options.settings.intensities();
        let feedback = Arc::new(Feedback { intensities: Intensities::new(rumble_pct, triggers_pct), on_keyboard: Mutex::new(None) });
        let registered = registered.clone();
        let thread = {
            let stop = stop.clone();
            let ctrl = ctrl.clone();
            let want_key = want_key.clone();
            let keymouse = keymouse.clone();
            let captured = captured.clone();
            let counters = counters.clone();
            let on_pad = on_pad.clone();
            let feedback = feedback.clone();
            std::thread::Builder::new()
                .name("ps-play".into())
                .spawn(move || {
                    // Une partie qui tombe (réseau, console qui ne répond
                    // plus) reprend toute seule, trois fois au plus ; une fin
                    // voulue (app, mise en veille, « déjà en cours »,
                    // association perdue) termine.
                    let mut retries = 0;
                    let end = loop {
                        let started = Instant::now();
                        let end = run(addr, &registered, options.clone(), &video, audio.as_ref(), &on_state, &stop, &ctrl, &want_key, &keymouse, &captured, &counters, &on_pad, &feedback);
                        if let Some(mut c) = ctrl.lock().unwrap().take() {
                            c.stop();
                        }
                        // Une partie qui a tenu une minute remet le compte à zéro.
                        if started.elapsed() > Duration::from_secs(60) {
                            retries = 0;
                        }
                        if stop.load(Ordering::SeqCst) || !recoverable(&end) || retries >= 3 {
                            break end;
                        }
                        retries += 1;
                        log::warn!("PlayStation : partie interrompue ({end:?}), reprise {retries}/3");
                        std::thread::sleep(Duration::from_secs(1));
                    };
                    if let Some(mut c) = ctrl.lock().unwrap().take() {
                        c.stop();
                    }
                    on_state(PlayState::Ended(end));
                })
                .ok()
        };
        Play { stop, keymouse, captured, counters, stats_at: Mutex::new((Instant::now(), initial)), want_key, thread, ctrl, feedback }
    }

    /// Intensités de la DualSense, de 0 à 100 % (vibrations, gâchettes),
    /// tout de suite : les derniers effets de la console sont reposés.
    pub fn set_intensities(&self, rumble: u8, triggers: u8) {
        self.feedback.intensities.set(rumble, triggers);
        log::info!("PlayStation : intensités de la manette : vibrations {}, gâchettes {} %", rumble.min(100), triggers.min(100));
    }

    /// Ce que l'app reçoit du clavier de la console (fil de la partie).
    pub fn set_on_keyboard(&self, f: Option<Box<dyn Fn(KeyboardEvent) + Send + Sync>>) {
        *self.feedback.on_keyboard.lock().unwrap() = f.map(Arc::from);
    }

    fn with_ctrl(&self, f: impl FnOnce(&Ctrl) -> std::io::Result<()>) -> bool {
        match self.ctrl.lock().unwrap().as_ref() {
            Some(c) => f(c).map_err(|e| log::warn!("PlayStation : message du clavier pas envoyé : {e}")).is_ok(),
            None => false,
        }
    }

    /// Le texte entier du champ de la console, à chaque changement. Faux
    /// sans canal de contrôle.
    pub fn keyboard_text(&self, text: &str) -> bool {
        self.with_ctrl(|c| c.keyboard_set_text(text))
    }

    /// Entrée : le texte est validé.
    pub fn keyboard_accept(&self) -> bool {
        self.with_ctrl(|c| c.keyboard_accept())
    }

    /// Échap : le texte est refusé.
    pub fn keyboard_reject(&self) -> bool {
        self.with_ctrl(|c| c.keyboard_reject())
    }

    /// La barre ou le menu de l'app s'ouvre (`true`) ou se ferme : la manette
    /// (et le clavier) ne vont plus à la console tant qu'ils sont ouverts.
    pub fn set_captured(&self, on: bool) {
        self.captured.store(on, Ordering::SeqCst);
        if on {
            self.keymouse.lock().unwrap().release_all();
        }
    }

    /// Statistiques depuis la lecture précédente (une fois par seconde).
    pub fn stats(&self) -> PlayStats {
        let mut g = self.stats_at.lock().unwrap();
        let dt = g.0.elapsed().as_secs_f32().max(0.001);
        g.0 = Instant::now();
        let c = &self.counters;
        let mut s = g.1;
        s.fps = c.frames.swap(0, Ordering::Relaxed) as f32 / dt;
        s.mbps = c.bytes.swap(0, Ordering::Relaxed) as f32 * 8.0 / dt / 1e6;
        s.lost = c.lost.swap(0, Ordering::Relaxed);
        s.recovered = c.recovered.swap(0, Ordering::Relaxed);
        s
    }

    /// Une touche (usage HID) du clavier de l'appareil.
    pub fn key(&self, usage: u16, pressed: bool) {
        self.keymouse.lock().unwrap().key(usage, pressed);
    }

    /// Un bouton de la souris (0 gauche, 1 droit, 2 milieu…).
    pub fn mouse_button(&self, b: u8, pressed: bool) {
        self.keymouse.lock().unwrap().mouse_button(b, pressed);
    }

    /// Déplacement brut de la souris capturée (comptes, y vers le bas).
    pub fn mouse_move(&self, dx: f32, dy: f32) {
        self.keymouse.lock().unwrap().mouse_move(dx, dy, Instant::now());
    }

    /// Tout relâcher (fenêtre qui perd le clavier, souris rendue).
    pub fn release_all(&self) {
        self.keymouse.lock().unwrap().release_all();
    }

    /// La disposition du clavier et de la souris de cette console.
    pub fn set_layout(&self, layout: Layout) {
        self.keymouse.lock().unwrap().set_layout(layout);
    }

    /// Le décodeur de l'app a perdu sa référence : une image clé est
    /// demandée à la console, de nouveau toutes les 500 ms.
    pub fn key_frame_lost(&self) {
        self.want_key.store(true, Ordering::Relaxed);
    }

    /// Le décodeur de l'app a retrouvé une image clé.
    pub fn key_frame_recovered(&self) {
        self.want_key.store(false, Ordering::Relaxed);
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
    want_key: &Arc<AtomicBool>,
    keymouse: &Arc<Mutex<KeyMouse>>,
    captured: &Arc<AtomicBool>,
    counters: &Arc<Counters>,
    on_pad: &Arc<dyn Fn(PadEvent) + Send + Sync>,
    feedback: &Arc<Feedback>,
) -> PlayEnd {
    on_state(PlayState::Connecting);
    let Some(keys) = Keys::from_registered(registered) else { return PlayEnd::NotPaired };
    // La console garde la session précédente quelques secondes : « déjà en
    // cours » est réessayé pendant 30 s (après une coupure, la console garde
    // l'ancienne session jusqu'à une minute).
    let started = Instant::now();
    let (target, nonce) = loop {
        match session::request_session(addr, registered.target.into(), &keys) {
            Ok(r) => break r,
            Err(session::SessionError::Rejected { reason: Some(super::regist::REASON_IN_USE), .. })
                if started.elapsed() < Duration::from_secs(30) && !stop.load(Ordering::SeqCst) =>
            {
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(session::SessionError::Rejected { reason: Some(super::regist::REASON_IN_USE), .. }) => return PlayEnd::InUse,
            Err(session::SessionError::Rejected { status: 403, .. }) => return PlayEnd::NotPaired,
            Err(e) => return PlayEnd::Unreachable(e.to_string()),
        }
    };
    // La cible n'est sûre qu'après la demande de session : une PS4 ne
    // reçoit que du H.264, sans HDR.
    let profile = options.settings.profile(options.hevc_decoder, super::rpcrypt::is_ps5(target));
    log::info!(
        "PlayStation : {}×{} à {} i/s, {} kbit/s, {}{}",
        profile.width,
        profile.height,
        profile.max_fps,
        profile.bitrate,
        if profile.hevc { "H.265" } else { "H.264" },
        if profile.hdr { ", HDR" } else { "" }
    );
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
    let _ = ctrl.enable_features(profile.dualsense, true);
    let crypt = ctrl.crypt().clone();
    *ctrl_slot.lock().unwrap() = Some(ctrl);
    // Les manettes du lecteur du cœur (fil SDL, Mac et Windows), réunies :
    // une seule manette pour la console.
    gamepads::hold_sdl();
    let _sdl = ReleaseSdl;
    // Les manettes du lecteur du cœur ; sans manette branchée, le clavier et
    // la souris (une manette branchée les exclut).
    let mut reader = super::controller::PadReader::default();
    let keymouse = keymouse.clone();
    let captured = captured.clone();
    let on_pad = on_pad.clone();
    let mut menu = super::controller::PadMenu::default();
    let source: super::stream::ControllerSource = Box::new(move || {
        let g = gamepads::global();
        let now = Instant::now();
        if g.pads().is_empty() {
            let s = keymouse.lock().unwrap().read(now);
            return if captured.load(Ordering::SeqCst) { ControllerState::default() } else { s };
        }
        let s = reader.read(g);
        let open = captured.load(Ordering::SeqCst);
        for e in menu.update(&s, open, now) {
            on_pad(e);
        }
        if open { ControllerState::default() } else { s }
    });
    let (mut stream, stream_events) = match Stream::start(addr, target, session_id, crypt, profile, Some(source), want_key.clone()) {
        Ok(s) => s,
        Err(e) => return PlayEnd::Unreachable(e.to_string()),
    };
    let hevc_out = profile.hevc || profile.hdr;
    video.packet(Packet::Codec { codec: if hevc_out { FOURCC_HEVC } else { FOURCC_H264 } });
    let mut receiver = AudioReceiver::new();
    let mut haptics = HapticsFrames::default();
    let hd = dualsense::hd_haptics();
    // Vibrations HD : le chemin pris (annoncé au journal quand il change),
    // le dernier échantillon (raccord entre deux trames), le tampon.
    let mut hd_path: Option<bool> = None;
    let mut hd_last = [0.0f32; 2];
    let mut hd_buf: Vec<f32> = Vec::with_capacity(2 * 16 * 30 * 4);
    // Derniers effets de la console, reposés quand les intensités changent.
    let intensities = &feedback.intensities;
    let mut generation = intensities.generation();
    let mut last_rumble = (0u8, 0u8);
    let mut last_triggers: Option<(u8, [u8; 10], u8, [u8; 10])> = None;
    let mut keyboard_open = false;
    let mut audio_configured = false;
    let clock = Instant::now();
    // Dernière image rendue : sans image depuis une seconde, une image clé
    // est demandée (perte que rien n'a signalée).
    let mut last_frame: Option<Instant> = None;
    let end = loop {
        if stop.load(Ordering::SeqCst) {
            break PlayEnd::Stopped;
        }
        // Le canal de contrôle : fin, contenu protégé, clavier de la console.
        while let Ok(e) = events.try_recv() {
            match e {
                CtrlEvent::CantDisplay(c) => on_state(PlayState::CantDisplay(c)),
                CtrlEvent::Closed(why) => log::info!("PlayStation : canal de contrôle fermé ({why})"),
                CtrlEvent::KeyboardOpen(text) => {
                    keyboard_open = true;
                    feedback.keyboard(KeyboardEvent::Open(text));
                }
                CtrlEvent::KeyboardText(text) => feedback.keyboard(KeyboardEvent::Text(text)),
                CtrlEvent::KeyboardClosed => {
                    keyboard_open = false;
                    feedback.keyboard(KeyboardEvent::Closed);
                }
                _ => {}
            }
        }
        // Intensités changées pendant la partie : les derniers effets reposés.
        let g = intensities.generation();
        if g != generation {
            generation = g;
            if last_rumble != (0, 0) {
                rumble(last_rumble.0, last_rumble.1, intensities.rumble());
            }
            if let Some((tl, l, tr, r)) = last_triggers {
                triggers(tl, &l, tr, &r, intensities.triggers());
            }
        }
        if last_frame.is_some_and(|t| t.elapsed() > Duration::from_secs(1)) && !want_key.load(Ordering::Relaxed) {
            log::warn!("PlayStation : plus d'image depuis 1 s, image clé demandée");
            want_key.store(true, Ordering::Relaxed);
            last_frame = Some(Instant::now());
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
                if is_key {
                    want_key.store(false, Ordering::Relaxed);
                }
                last_frame = Some(Instant::now());
                counters.frames.fetch_add(1, Ordering::Relaxed);
                counters.bytes.fetch_add(frame.data.len() as u64, Ordering::Relaxed);
                if frame.recovered {
                    counters.recovered.fetch_add(1, Ordering::Relaxed);
                }
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
            StreamEvent::Pad(super::takion::data::RUMBLE, buf) if buf.len() >= 3 => {
                last_rumble = (buf[1], buf[2]);
                rumble(buf[1], buf[2], intensities.rumble());
            }
            StreamEvent::Pad(super::takion::data::TRIGGER_EFFECTS, buf) if buf.len() >= 25 => {
                let mut l = [0u8; 10];
                let mut r = [0u8; 10];
                l.copy_from_slice(&buf[5..15]);
                r.copy_from_slice(&buf[15..25]);
                last_triggers = Some((buf[1], l, buf[2], r));
                triggers(buf[1], &l, buf[2], &r, intensities.triggers());
            }
            StreamEvent::Audio(p) if p.is_haptics => {
                let frames = haptics.push(&p);
                let to_hd = hd.has_output();
                if hd_path != Some(to_hd) {
                    log::info!(
                        "PlayStation : vibrations HD {}",
                        if to_hd { "jouées par la carte son de la DualSense (USB, voies 3 et 4, 48 kHz)" } else { "traduites en vibrations simples" }
                    );
                    hd_path = Some(to_hd);
                    hd_last = [0.0; 2];
                }
                if to_hd {
                    let gain = intensities.rumble() as f32 / 100.0;
                    for f in &frames {
                        dualsense::upsample(f, gain, &mut hd_last, &mut hd_buf);
                    }
                    hd.push(&hd_buf);
                    hd_buf.clear();
                } else if let Some(f) = frames.last() {
                    let (left, right) = strength(f);
                    let pct = intensities.rumble();
                    rumble16(dualsense::scale_rumble(left, pct), dualsense::scale_rumble(right, pct), 60);
                }
            }
            // Le décodeur de l'app attend alors une image clé au lieu de
            // décoder des images aux références perdues.
            StreamEvent::VideoLost => {
                counters.lost.fetch_add(1, Ordering::Relaxed);
                video.packet(Packet::Hole { missing_samples: 0 })
            }
            // Perte réparée par la console : le décodeur ne voit que des images saines.
            StreamEvent::VideoDropped => {
                counters.lost.fetch_add(1, Ordering::Relaxed);
            }
            StreamEvent::Audio(_) | StreamEvent::Pad(..) => {}
            StreamEvent::Ended(why) => break PlayEnd::Remote(why),
        }
    };
    stream.stop();
    rumble(0, 0, 100);
    // Gâchettes rendues libres (SDL ne le fait pas tout seul).
    triggers(0x05, &[0; 10], 0x05, &[0; 10], 100);
    // Partie interrompue clavier ouvert : l'app ferme son bandeau (une reprise
    // repart sans clavier).
    if keyboard_open {
        feedback.keyboard(KeyboardEvent::Closed);
    }
    end
}

/// DualSense et DualSense Edge.
fn is_dualsense(p: &gamepads::PadInfo) -> bool {
    p.vendor == Some(0x054c) && matches!(p.product, Some(0x0ce6) | Some(0x0df2))
}

/// Rapport d'effets de la DualSense (`DS5EffectsState_t` de SDL, 47
/// octets) qui pose les deux gâchettes adaptatives : type d'effet puis 10
/// octets de paramètres, tels que la console les envoie.
pub(crate) fn trigger_report(type_left: u8, left: &[u8], type_right: u8, right: &[u8]) -> Vec<u8> {
    let mut r = vec![0u8; 47];
    r[0] = 0x04 | 0x08;
    r[10] = type_right;
    r[11..21].copy_from_slice(&right[..10]);
    r[21] = type_left;
    r[22..32].copy_from_slice(&left[..10]);
    r
}

/// Gâchettes adaptatives sur les DualSense branchées, la force de chaque
/// effet à `pct` pour cent.
fn triggers(type_left: u8, left: &[u8; 10], type_right: u8, right: &[u8; 10], pct: u8) {
    let (type_left, left) = dualsense::scale_trigger(type_left, left, pct);
    let (type_right, right) = dualsense::scale_trigger(type_right, right, pct);
    let report = trigger_report(type_left, &left, type_right, &right);
    for p in gamepads::global().pads().iter().filter(|p| is_dualsense(p)) {
        gamepads::effect(p.id, report.clone());
    }
}

fn rumble16(large: u16, small: u16, ms: i32) {
    for p in gamepads::global().pads() {
        gamepads::rumble(GamepadRumble { id: p.id, large_motor: large, small_motor: small, duration_ms: ms });
    }
}

/// Vibrations HD (son PCM 16 bits stéréo envoyé par la console à la place
/// des vibrations simples quand la manette est une DualSense) traduites en
/// vibrations ordinaires quand aucune DualSense n'est branchée en USB, comme
/// chiaki-ng sans sortie de vibrations HD : force (gros moteur, petit
/// moteur) d'une trame, l'amplitude moyenne de chaque voie au-dessus d'un
/// seuil.
pub(crate) fn strength(pcm: &[u8]) -> (u16, u16) {
    let frames = pcm.len() / 4;
    if frames == 0 {
        return (0, 0);
    }
    let (mut l, mut r) = (0u32, 0u32);
    for f in pcm.chunks_exact(4) {
        l += i16::from_le_bytes([f[0], f[1]]).unsigned_abs() as u32 * 2;
        r += i16::from_le_bytes([f[2], f[3]]).unsigned_abs() as u32 * 2;
    }
    let level = |sum: u32| {
        let v = sum / frames as u32;
        if v <= 100 {
            0
        } else {
            v.clamp(1 << 9, u16::MAX as u32) as u16
        }
    };
    (level(l), level(r))
}

/// Relâche le fil SDL des manettes à la fin de la partie.
struct ReleaseSdl;

impl Drop for ReleaseSdl {
    fn drop(&mut self) {
        gamepads::release_sdl();
    }
}

/// Vibrations de la console (gros moteur à gauche, petit à droite, 0 à
/// 255) sur toutes les manettes, à `pct` pour cent ; tenues jusqu'au message
/// suivant (la console envoie 0, 0 pour arrêter).
fn rumble(left: u8, right: u8, pct: u8) {
    let (large, small) = (dualsense::scale_rumble(left as u16 * 257, pct), dualsense::scale_rumble(right as u16 * 257, pct));
    for p in gamepads::global().pads() {
        gamepads::rumble(GamepadRumble {
            id: p.id,
            large_motor: large,
            small_motor: small,
            duration_ms: if large == 0 && small == 0 { 0 } else { 10_000 },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_retried() {
        assert!(recoverable(&PlayEnd::Unreachable("réseau".into())));
        assert!(recoverable(&PlayEnd::Remote("Takion arrêté".into())));
        assert!(!recoverable(&PlayEnd::Remote("Server shutting down".into())));
        assert!(!recoverable(&PlayEnd::Stopped));
        assert!(!recoverable(&PlayEnd::InUse));
        assert!(!recoverable(&PlayEnd::NotPaired));
    }

    #[test]
    fn trigger_report_layout() {
        let r = trigger_report(0x21, &[1; 10], 0x26, &[2; 10]);
        assert_eq!(r.len(), 47);
        assert_eq!(r[0], 0x0c);
        assert_eq!((r[10], r[11], r[20]), (0x26, 2, 2));
        assert_eq!((r[21], r[22], r[31]), (0x21, 1, 1));
        assert_eq!(r[32], 0);
    }

    #[test]
    fn haptics_strength() {
        assert_eq!(strength(&[0; 120]), (0, 0));
        let mut pcm = Vec::new();
        for _ in 0..30 {
            pcm.extend_from_slice(&1000i16.to_le_bytes());
            pcm.extend_from_slice(&(-30i16).to_le_bytes());
        }
        assert_eq!(strength(&pcm), (2000, 0));
    }

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
