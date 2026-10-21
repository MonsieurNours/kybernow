//! Fil de l'image : reçoit les paquets de Kyber, décode, dessine, présente,
//! et note l'heure d'affichage de chaque image pour la latence.
//!
//! Chaque paquet est décodé, mais seule l'image la plus récente est montrée :
//! si d'autres attendent (rafale du réseau, écran un peu plus lent que
//! l'ordinateur), celle-ci est sautée, sans quoi le retard s'accumulerait
//! derrière la synchro verticale. Comme sur Mac.
//!
//! Un seul fil possède l'appareil Direct3D, le décodeur et la chaîne
//! d'échange : rien n'attend ailleurs.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use kybernow_core::i18n::Lang;
use kybernow_core::latency::{LatencyTracker, now_micros};
use kybernow_core::media::{FOURCC_AV1, Packet, PacketHandler};
use kybernow_core::session::Session;

use crate::decoder::{self, Decoder, codec_name, is_corrupt};
use crate::render::{self, Renderer};

pub enum VideoMsg {
    Codec(u32),
    Config(Vec<u8>),
    Frame { data: Vec<u8>, pts: u64, key: bool },
    Stop,
}

/// État partagé entre la fenêtre, le fil de l'image et la session.
#[derive(Default)]
pub struct Shared {
    /// Taille de la zone cliente en pixels (largeur << 32 | hauteur).
    pub client_size: AtomicU64,
    pub session: OnceLock<Weak<Session>>,
    pub latency: OnceLock<Arc<LatencyTracker>>,
    pub frames: AtomicU64,
    pub first_image: AtomicBool,
    pub decoder: Mutex<String>,
    /// Flux démarré (heure, images montrées à ce moment) : sans nouvelle
    /// image en 4 s, il est relancé.
    pub watch: Mutex<Option<(Instant, u64)>>,
    /// Dernière image de la session (JPEG), écrite à la fin du fil de l'image.
    pub last_image: Mutex<Option<Vec<u8>>>,
    /// Format demandé pour la session (`media::FOURCC_*`), suivi en cas de
    /// changement de profil : il sert si le paquet de format se perd.
    pub codec: std::sync::atomic::AtomicU32,
}

impl Shared {
    pub fn set_client_size(&self, w: u32, h: u32) {
        self.client_size.store(((w as u64) << 32) | h as u64, Ordering::Relaxed);
    }

    pub fn client_size(&self) -> (u32, u32) {
        let v = self.client_size.load(Ordering::Relaxed);
        ((v >> 32) as u32, v as u32)
    }

    /// À appeler au démarrage (ou à la relance) du flux.
    pub fn arm_watch(&self) {
        *self.watch.lock().unwrap() = Some((Instant::now(), self.frames.load(Ordering::Relaxed)));
    }

    pub fn disarm_watch(&self) {
        *self.watch.lock().unwrap() = None;
    }

    /// Le flux tourne depuis 4 s sans une image : on le relance (une fois par démarrage).
    fn check_watch(&self) {
        let fire = {
            let mut w = self.watch.lock().unwrap();
            match *w {
                Some((at, frames)) if at.elapsed() >= Duration::from_secs(4) => {
                    *w = None;
                    self.frames.load(Ordering::Relaxed) == frames
                }
                _ => false,
            }
        };
        if fire {
            log::warn!("image : rien en 4 s depuis le démarrage du flux");
            if let Some(s) = self.session.get().and_then(Weak::upgrade) {
                s.no_image();
            }
        }
    }

    fn request_keyframe(&self) {
        if let Some(s) = self.session.get().and_then(Weak::upgrade) {
            s.request_keyframe();
        }
    }
}

pub struct VideoHandler {
    tx: SyncSender<VideoMsg>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    overflow: AtomicBool,
    /// Images en attente de décodage.
    pending: AtomicUsize,
}

impl PacketHandler for VideoHandler {
    fn packet(&self, p: Packet<'_>) {
        let msg = match p {
            Packet::Codec { codec } => VideoMsg::Codec(codec),
            Packet::Media { is_config: true, data, .. } => VideoMsg::Config(data.to_vec()),
            Packet::Media { is_key, pts, data, .. } => VideoMsg::Frame {
                data: data.to_vec(),
                pts,
                key: is_key,
            },
            Packet::Hole { .. } => return,
        };
        match msg {
            // Début de flux et paramètres : jamais perdus (rares, et sans eux
            // le décodeur ne repart pas).
            VideoMsg::Codec(_) | VideoMsg::Config(_) => {
                let _ = self.tx.send(msg);
            }
            msg => {
                self.pending.fetch_add(1, Ordering::Relaxed);
                if let Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) = self.tx.try_send(msg) {
                    self.pending.fetch_sub(1, Ordering::Relaxed);
                    // Le décodage ne suit plus : il faudra repartir d'une image clé.
                    self.overflow.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}

impl VideoHandler {
    pub fn stop(&self) {
        let _ = self.tx.try_send(VideoMsg::Stop);
    }

    /// Arrête le fil de l'image et attend sa fin (borné).
    pub fn stop_and_wait(&self, timeout: Duration) {
        self.stop();
        let Some(t) = self.thread.lock().unwrap().take() else { return };
        let started = Instant::now();
        while !t.is_finished() && started.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(20));
        }
        if t.is_finished() {
            let _ = t.join();
        }
    }
}

pub struct VideoOptions {
    pub hardware: bool,
    pub vsync: bool,
    /// Format demandé pour la session (`media::FOURCC_*`) : il sert si le
    /// paquet qui annonce le format du flux s'est perdu.
    pub codec: u32,
}

/// Démarre le fil de l'image, une fois l'appareil Direct3D et la chaîne
/// d'échange prêts.
pub fn spawn(hwnd: isize, opts: VideoOptions, shared: Arc<Shared>) -> Result<Arc<VideoHandler>, String> {
    let (tx, rx) = mpsc::sync_channel(240);
    let handler = Arc::new(VideoHandler {
        tx,
        thread: Mutex::new(None),
        overflow: AtomicBool::new(false),
        pending: AtomicUsize::new(0),
    });
    let (ptx, prx) = mpsc::channel();
    let h = handler.clone();
    let thread = std::thread::Builder::new()
        .name("kybernow-video".into())
        .spawn(move || run(hwnd, opts, shared, h, rx, ptx))
        .map_err(|e| e.to_string())?;
    *handler.thread.lock().unwrap() = Some(thread);
    // La fenêtre est créée sur ce fil : ses messages doivent passer pendant que
    // le fil de l'image crée la chaîne d'échange (DXGI peut lui en envoyer).
    loop {
        match prx.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(r) => break r?,
            Err(mpsc::RecvTimeoutError::Timeout) => crate::window::pump_messages(),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(crate::i18n::t("errors.video_thread", &[])),
        }
    }
    Ok(handler)
}

fn run(
    hwnd: isize,
    opts: VideoOptions,
    shared: Arc<Shared>,
    handler: Arc<VideoHandler>,
    rx: Receiver<VideoMsg>,
    ready_tx: mpsc::Sender<Result<(), String>>,
) {
    let (device, ctx) = match render::create_device() {
        Ok(d) => d,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("Direct3D 11 : {e}")));
            return;
        }
    };
    let mut renderer = match Renderer::new(
        device.clone(),
        ctx,
        HWND(hwnd as *mut _),
        shared.client_size(),
        opts.vsync,
    ) {
        Ok(r) => r,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("rendu : {e}")));
            return;
        }
    };
    let _ = ready_tx.send(Ok(()));
    renderer.clear();

    let mut decoder: Option<Decoder> = None;
    let mut fourcc = 0u32;
    let mut config: Vec<u8> = Vec::new();
    let mut send_config = false;
    let mut waiting_key = true;
    let mut hw_allowed = opts.hardware;
    let mut unsupported_logged = false;
    let mut last_request = Instant::now() - Duration::from_secs(1);
    // Diagnostic : ce qui arrive vraiment (le journal disait seulement « rien »).
    let (mut seen_codec, mut seen_config, mut seen_frame, mut seen_key) = (false, false, false, false);
    let mut dropped: u64 = 0;
    // Format voulu par la session, et s'il a été annoncé depuis son dernier changement.
    shared.codec.store(opts.codec, Ordering::Relaxed);
    let mut last_wanted = opts.codec;
    let mut codec_announced = false;
    let mut keyframe = |shared: &Shared, force: bool| {
        if force || last_request.elapsed() >= Duration::from_millis(300) {
            last_request = Instant::now();
            shared.request_keyframe();
        }
    };
    let open = |fourcc: u32, hw: bool, shared: &Shared| -> Option<Decoder> {
        match Decoder::new(fourcc, hw.then_some(&device)) {
            Ok(d) => {
                let key = if d.hardware { "decoder.hardware" } else { "decoder.software" };
                let what = |lang: Lang| format!("{} · {}", codec_name(fourcc), crate::i18n::tr(lang, key, &[]));
                log::info!("décodeur : {}", what(Lang::Fr));
                // Montré dans la barre et le titre de la session : langue de l'app.
                *shared.decoder.lock().unwrap() = what(crate::i18n::lang());
                Some(d)
            }
            Err(e) => {
                log::error!("décodeur {} : {e}", codec_name(fourcc));
                None
            }
        }
    };

    loop {
        renderer.resize(shared.client_size());
        shared.check_watch();
        let msg = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                // Comme sur Mac : une image clé se demande après une perte,
                // pas avant les premières données (l'encodeur commence par
                // une image clé ; le harceler pendant qu'il démarre le bloque).
                if waiting_key && seen_frame {
                    keyframe(&shared, false);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match msg {
            VideoMsg::Stop => break,
            VideoMsg::Codec(c) => {
                codec_announced = true;
                if !seen_codec {
                    seen_codec = true;
                    log::info!("image : format annoncé, {}", codec_name(c));
                }
                // Début d'un flux : décodeur neuf.
                fourcc = c;
                decoder.take();
                decoder = open(c, hw_allowed, &shared);
                waiting_key = true;
                send_config = true;
                shared.first_image.store(false, Ordering::Relaxed);
            }
            VideoMsg::Config(d) => {
                if !seen_config {
                    seen_config = true;
                    log::info!("image : paramètres du flux reçus ({} octets)", d.len());
                }
                // AV1 : un `av1C` (4 octets de résumé, puis les OBU) n'entre
                // pas tel quel dans un flux d'OBU.
                config = if fourcc == FOURCC_AV1 && d.len() > 4 && d[0] == 0x81 { d[4..].to_vec() } else { d };
                send_config = true;
            }
            VideoMsg::Frame { data, pts, key } => {
                // D'autres images attendent derrière celle-ci : on la décode
                // sans la montrer.
                let newer_waiting = handler.pending.fetch_sub(1, Ordering::Relaxed) > 1;
                if !seen_frame {
                    seen_frame = true;
                    log::info!("image : premières données ({} octets, image clé : {key})", data.len());
                }
                if key && !seen_key {
                    seen_key = true;
                    log::info!("image : première image clé ({} octets)", data.len());
                }
                // Le paquet qui annonce le format s'est perdu (transport rapide,
                // avec pertes) : sans lui, aucun décodeur et un écran noir sans
                // fin. On ouvre celui du format demandé pour la session.
                let wanted = shared.codec.load(Ordering::Relaxed);
                if wanted != last_wanted {
                    last_wanted = wanted;
                    codec_announced = false;
                }
                // Profil changé vers un autre format et paquet de format perdu.
                let stale = decoder.is_some() && wanted != 0 && fourcc != wanted && !codec_announced;
                if (decoder.is_none() && fourcc == 0 && wanted != 0) || stale {
                    log::warn!("image : format jamais annoncé, décodeur {} ouvert d'office", codec_name(wanted));
                    fourcc = wanted;
                    decoder = open(fourcc, hw_allowed, &shared);
                    send_config = true;
                    waiting_key = true;
                }
                if handler.overflow.swap(false, Ordering::Relaxed) {
                    log::warn!("image : file pleine, reprise sur une image clé");
                    waiting_key = true;
                }
                if waiting_key && !key {
                    keyframe(&shared, false);
                    continue;
                }
                if key {
                    waiting_key = false;
                }
                let Some(dec) = decoder.as_mut() else {
                    dropped += 1;
                    if dropped == 1 || dropped % 600 == 0 {
                        log::warn!("image : {dropped} images reçues sans décodeur");
                    }
                    continue;
                };
                let sent = if send_config && !config.is_empty() {
                    send_config = false;
                    let mut buf = config.clone();
                    buf.extend_from_slice(&data);
                    dec.send(&buf, pts, key)
                } else {
                    dec.send(&data, pts, key)
                };
                if let Err(e) = sent {
                    log::warn!("décodage : {}", decoder::error_text(e));
                    waiting_key = true;
                    keyframe(&shared, true);
                    continue;
                }
                let mut fallback = false;
                while let Some(r) = dec.receive() {
                    match r {
                        Ok(frame) => {
                            if is_corrupt(frame) {
                                keyframe(&shared, false);
                            }
                            let pts = frame.pts as u64;
                            if newer_waiting {
                                continue;
                            }
                            match renderer.draw(frame) {
                                Ok(()) => {
                                    if let Some(l) = shared.latency.get() {
                                        l.displayed(pts, now_micros());
                                    }
                                    shared.frames.fetch_add(1, Ordering::Relaxed);
                                    if !shared.first_image.swap(true, Ordering::Relaxed) {
                                        // La fenêtre se montre à la première image.
                                        unsafe {
                                            let _ = PostMessageW(
                                                Some(HWND(hwnd as *mut _)),
                                                crate::window::WM_APP_FIRST_IMAGE,
                                                WPARAM(0),
                                                LPARAM(0),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    if dec.hardware {
                                        log::warn!("rendu : {}", e.0);
                                        fallback = true;
                                    } else if !unsupported_logged {
                                        // Format que l'app ne sait pas montrer (10 bits…).
                                        log::error!("rendu impossible : {}", e.0);
                                        unsupported_logged = true;
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("décodage : {}", decoder::error_text(e));
                            waiting_key = true;
                            keyframe(&shared, true);
                            break;
                        }
                    }
                }
                if fallback {
                    // La carte décode dans un format qu'on ne sait pas montrer.
                    log::warn!("décodage par le processeur à la place de la carte graphique");
                    hw_allowed = false;
                    decoder = open(fourcc, false, &shared);
                    send_config = true;
                    waiting_key = true;
                    keyframe(&shared, true);
                }
            }
        }
    }
    if let Some((px, w, h)) = renderer.take_snapshot() {
        match crate::snapshot::jpeg(&px, w, h) {
            Ok(j) => *shared.last_image.lock().unwrap() = Some(j),
            Err(e) => log::warn!("dernière image : {e}"),
        }
    }
    log::info!("fil de l'image arrêté");
}
