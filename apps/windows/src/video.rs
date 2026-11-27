//! Fil de l'image : reçoit les paquets de Kyber, décode, dessine, présente,
//! et note l'heure d'affichage de chaque image pour la latence.
//!
//! Chaque paquet est décodé, mais seule l'image la plus récente est montrée :
//! si d'autres attendent (rafale du réseau, écran un peu plus lent que
//! l'ordinateur), celle-ci est sautée, sans quoi le retard s'accumulerait
//! derrière la synchro verticale. Comme sur Mac.
//!
//! Un fil par écran (fenêtre) : il possède le décodeur et la chaîne d'échange
//! de son flux, et partage l'appareil Direct3D de la session (`gpu.rs`) avec
//! ceux des autres écrans ; rien n'attend ailleurs.

use crate::sync::Locked;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use kybernow_core::i18n::Lang;
use kybernow_core::latency::{LatencyTracker, now_micros};
use kybernow_core::media::{FOURCC_AV1, FOURCC_PYROWAVE, Packet, PacketHandler, ResumeGate};
use kybernow_core::session::Session;

use crate::decoder::{self, Decoder, codec_name, is_corrupt};
use crate::gpu::{Gpu, SessionGpu};
use crate::hdr::ScreenHdr;
use crate::pyrowave::{self, Config as PyroConfig, Outcome};
use crate::render::{DrawError, Renderer};

/// Tampons des images, repris d'une image à l'autre : le fil du réseau ne
/// fait plus une allocation par image (jusqu'à 240 par seconde).
#[derive(Default)]
pub struct BufferPool(Mutex<Vec<Vec<u8>>>);

/// Au plus ce nombre de tampons gardés (les autres sont rendus au système).
const POOL_KEEP: usize = 8;

impl BufferPool {
    /// Un tampon qui contient `data`.
    pub fn take(self: &Arc<Self>, data: &[u8]) -> Pooled {
        let mut v = self.0.locked().pop().unwrap_or_default();
        v.clear();
        v.extend_from_slice(data);
        Pooled { data: v, pool: Arc::downgrade(self) }
    }
}

/// Un tampon d'image ; rendu à son `BufferPool` quand il est lâché.
pub struct Pooled {
    data: Vec<u8>,
    pool: Weak<BufferPool>,
}

impl std::ops::Deref for Pooled {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.data
    }
}

impl Drop for Pooled {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.upgrade() {
            let mut kept = pool.0.locked();
            if kept.len() < POOL_KEEP {
                kept.push(std::mem::take(&mut self.data));
            }
        }
    }
}

pub enum VideoMsg {
    Codec(u32),
    Config(Vec<u8>),
    /// `received` : l'image complète vient d'arriver (µs, [`now_micros`]).
    /// `resume` : image de reprise après une perte (docs/reprise-apres-perte.md).
    Frame { data: Pooled, pts: u64, key: bool, received: i64, resume: Option<u64> },
    /// Une image perdue en route (console PlayStation qui ne l'a pas
    /// réparée, ou numéro d'image sauté sur l'image de la Station) : le
    /// décodeur attend une image clé ou une image de reprise au lieu de
    /// décoder des images aux références perdues.
    Lost,
    /// La fenêtre a changé de taille : le fil se réveille pour redessiner
    /// l'image en place tout de suite (bureau immobile, aucune image ne vient).
    Resized,
    Stop,
}

/// Ce que tous les écrans d'une session ont en commun : la session du cœur, le
/// suivi de la latence et le format demandé.
#[derive(Default)]
pub struct Link {
    pub session: OnceLock<Weak<Session>>,
    pub latency: OnceLock<Arc<LatencyTracker>>,
    /// Format demandé pour la session (`media::FOURCC_*`), suivi en cas de
    /// changement de profil : il sert si le paquet de format se perd.
    pub codec: AtomicU32,
    /// Temps de décodage et d'affichage de ce PC sur l'écran principal (ms) :
    /// le trajet d'une image les garde quand l'ordinateur n'envoie pas ses
    /// mesures (docs/link-ui.md « Statistiques »).
    local: Mutex<[Vec<f64>; 2]>,
    /// Console PlayStation (`ps_session.rs`) : pas de session de Kyber, la
    /// partie du cœur demande elle-même ses images clés.
    pub console: OnceLock<ConsoleLink>,
}

/// Ce que le fil de l'image dit à une partie sur console : le décodeur a
/// perdu sa référence (une image clé est demandée à la console, de nouveau
/// toutes les 500 ms par le cœur), puis l'a retrouvée.
pub struct ConsoleLink {
    pub lost: Box<dyn Fn() + Send + Sync>,
    pub recovered: Box<dyn Fn() + Send + Sync>,
}

/// Au-delà, plus rien ne s'ajoute avant la seconde suivante (aucune session
/// ne montre autant d'images en une seconde).
const LOCAL_MAX: usize = 1000;

impl Link {
    fn push_local(&self, decode_ms: f64, display_ms: f64) {
        let mut l = self.local.locked();
        if l[0].len() < LOCAL_MAX {
            l[0].push(decode_ms.max(0.0));
            l[1].push(display_ms.max(0.0));
        }
    }

    /// Médianes de décodage et d'affichage depuis l'appel précédent (une fois
    /// par seconde) ; `None` sans image montrée.
    pub fn local_second(&self) -> (Option<f64>, Option<f64>) {
        let mut l = self.local.locked();
        let median = |v: &mut Vec<f64>| {
            if v.is_empty() {
                return None;
            }
            v.sort_by(|a, b| a.total_cmp(b));
            let m = v[v.len() / 2];
            v.clear();
            Some(m)
        };
        let [decode, display] = &mut *l;
        (median(decode), median(display))
    }
}

/// État partagé entre la fenêtre d'un écran, son fil de l'image et la session.
pub struct Shared {
    pub link: Arc<Link>,
    /// L'écran principal de la session : il fait les mesures (latence, images
    /// par seconde) et la veille « aucune image » ; un autre écran muet est
    /// retiré par le cœur lui-même.
    pub primary: AtomicBool,
    /// Taille de la zone cliente en pixels (largeur << 32 | hauteur).
    pub client_size: AtomicU64,
    pub frames: AtomicU64,
    pub first_image: AtomicBool,
    pub decoder: Mutex<String>,
    /// Flux démarré (heure, images montrées à ce moment) : sans nouvelle
    /// image en 4 s, il est relancé.
    pub watch: Mutex<Option<(Instant, u64)>>,
    /// Dernière image de la session (JPEG), écrite à la fin du fil de l'image.
    pub last_image: Mutex<Option<Vec<u8>>>,
    /// Trait du bord haut (qui signale la barre de session) : l'échelle de
    /// l'écran en centièmes (100 : 100 %), 0 quand il n'a pas lieu d'être.
    /// Posé par la fenêtre, lu à chaque image.
    pub edge_hint: AtomicU32,
    /// HDR (docs/hdr.md) : l'écran de ce PC où est la fenêtre, posé par le
    /// fil des fenêtres (création, fenêtre glissée, écrans changés), lu à
    /// chaque image par le rendu.
    pub screen_hdr: Mutex<ScreenHdr>,
    /// Le flux de cet écran est annoncé HDR (`Platform::on_hdr`, ou la
    /// console) : une image 10 bits sans étiquette compte comme PQ.
    pub stream_hdr: AtomicBool,
}

impl Shared {
    pub fn new(link: Arc<Link>, primary: bool) -> Arc<Self> {
        Arc::new(Self {
            link,
            primary: AtomicBool::new(primary),
            client_size: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            first_image: AtomicBool::new(false),
            decoder: Mutex::new(String::new()),
            watch: Mutex::new(None),
            last_image: Mutex::new(None),
            edge_hint: AtomicU32::new(0),
            screen_hdr: Mutex::new(ScreenHdr::default()),
            stream_hdr: AtomicBool::new(false),
        })
    }

    pub fn is_primary(&self) -> bool {
        self.primary.load(Ordering::Relaxed)
    }

    pub fn set_client_size(&self, w: u32, h: u32) {
        self.client_size.store(((w as u64) << 32) | h as u64, Ordering::Relaxed);
    }

    pub fn client_size(&self) -> (u32, u32) {
        let v = self.client_size.load(Ordering::Relaxed);
        ((v >> 32) as u32, v as u32)
    }

    /// À appeler au démarrage (ou à la relance) du flux.
    pub fn arm_watch(&self) {
        *self.watch.locked() = Some((Instant::now(), self.frames.load(Ordering::Relaxed)));
    }

    pub fn disarm_watch(&self) {
        *self.watch.locked() = None;
    }

    /// Le flux tourne depuis 4 s sans une image : on le relance (une fois par
    /// démarrage). Seul l'écran principal le demande au cœur ; un autre écran
    /// muet est retiré par le cœur lui-même.
    fn check_watch(&self) {
        let fire = {
            let mut w = self.watch.locked();
            match *w {
                Some((at, frames)) if at.elapsed() >= Duration::from_secs(4) => {
                    *w = None;
                    self.frames.load(Ordering::Relaxed) == frames
                }
                _ => false,
            }
        };
        if !fire {
            return;
        }
        if self.is_primary() {
            log::warn!("image : rien en 4 s depuis le démarrage du flux");
            if let Some(s) = self.link.session.get().and_then(Weak::upgrade) {
                s.no_image();
            }
        } else {
            log::warn!("image : rien en 4 s sur un écran secondaire (le cœur le retire s'il reste muet)");
        }
    }

    fn request_keyframe(&self) {
        if let Some(c) = self.link.console.get() {
            (c.lost)();
            return;
        }
        if let Some(s) = self.link.session.get().and_then(Weak::upgrade) {
            s.request_keyframe();
        }
    }

    /// Une image perdue sur l'image de la Station : le cœur demande la
    /// reprise sans image clé, puis une image clé si rien ne vient.
    fn image_lost(&self) {
        if let Some(s) = self.link.session.get().and_then(Weak::upgrade) {
            s.keyframe_lost();
        }
    }

    /// Le décodeur a repris (image clé ou image de reprise).
    fn image_recovered(&self) {
        if let Some(c) = self.link.console.get() {
            (c.recovered)();
            return;
        }
        if let Some(s) = self.link.session.get().and_then(Weak::upgrade) {
            s.keyframe_recovered();
        }
    }
}

pub struct VideoHandler {
    tx: SyncSender<VideoMsg>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    overflow: AtomicBool,
    /// Images en attente de décodage.
    pending: AtomicUsize,
    /// Arrêt demandé (le message d'arrêt peut être refusé par une file pleine).
    stopped: AtomicBool,
    pool: Arc<BufferPool>,
}

impl PacketHandler for VideoHandler {
    fn packet(&self, p: Packet<'_>) {
        let msg = match p {
            Packet::Codec { codec } => VideoMsg::Codec(codec),
            Packet::Media { is_config: true, data, .. } => VideoMsg::Config(data.to_vec()),
            Packet::Media { is_key, pts, data, resume, .. } => VideoMsg::Frame {
                data: self.pool.take(data),
                pts,
                key: is_key,
                received: now_micros(),
                resume,
            },
            // Une perte : d'une console, ou vue par le cœur aux numéros des
            // images de la Station (Kyber ne le dit pas lui-même).
            Packet::Hole { .. } => VideoMsg::Lost,
        };
        match msg {
            // Début de flux et paramètres : jamais perdus (rares, et sans eux
            // le décodeur ne repart pas).
            // Une perte ne bloque pas le fil de la console : file pleine, la reprise sur
            // une image clé est déjà demandée par le débordement.
            VideoMsg::Lost => {
                if self.tx.try_send(VideoMsg::Lost).is_err() {
                    self.overflow.store(true, Ordering::Relaxed);
                }
            }
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
    /// La fenêtre a changé de taille (sa nouvelle taille est déjà dans
    /// `Shared`) : le fil de l'image redessine sans attendre une image neuve.
    /// Perdu sans dommage si la file est pleine (des images arrivent).
    pub fn resized(&self) {
        let _ = self.tx.try_send(VideoMsg::Resized);
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        let _ = self.tx.try_send(VideoMsg::Stop);
    }

    /// Arrête le fil de l'image et attend sa fin (borné).
    pub fn stop_and_wait(&self, timeout: Duration) {
        self.stop();
        let Some(t) = self.thread.locked().take() else { return };
        let started = Instant::now();
        while !t.is_finished() && started.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(20));
        }
        if t.is_finished() {
            let _ = t.join();
        }
    }
}

/// Paquets d'un écran qui n'est plus un écran de ce PC, ou dont la fenêtre
/// n'a pas pu se créer : jetés.
pub struct Discard;

impl PacketHandler for Discard {
    fn packet(&self, _p: Packet<'_>) {}
}

/// Paquets d'un écran dont la fenêtre n'existe pas encore quand le cœur en
/// demande le gestionnaire (fil des fenêtres en retard, voir
/// `window::screens_changed`) : jetés jusqu'à ce qu'elle arrive, puis passés
/// à son fil de l'image. Le début du flux et ses paramètres, vus avant, lui
/// sont redonnés ; le fil de l'image redemande une image clé à la première
/// donnée qui n'en est pas une, comme après une perte.
#[derive(Default)]
pub struct LateFeed {
    state: Mutex<LateState>,
}

#[derive(Default)]
struct LateState {
    target: Option<Arc<VideoHandler>>,
    codec: Option<u32>,
    config: Option<Vec<u8>>,
}

impl PacketHandler for LateFeed {
    fn packet(&self, p: Packet<'_>) {
        let mut st = self.state.locked();
        if let Some(target) = st.target.clone() {
            drop(st);
            target.packet(p);
            return;
        }
        match p {
            Packet::Codec { codec } => {
                st.codec = Some(codec);
                st.config = None;
            }
            Packet::Media { is_config: true, data, .. } => st.config = Some(data.to_vec()),
            _ => {}
        }
    }
}

impl LateFeed {
    /// La fenêtre de l'écran est là : ses paquets vont désormais à son fil de l'image.
    pub fn attach(&self, target: Arc<VideoHandler>) {
        let mut st = self.state.locked();
        if let Some(codec) = st.codec.take() {
            target.packet(Packet::Codec { codec });
        }
        if let Some(data) = st.config.take() {
            target.packet(Packet::Media { is_config: true, is_key: false, pts: 0, data: &data, resume: None });
        }
        st.target = Some(target);
    }
}

pub struct VideoOptions {
    pub hardware: bool,
    /// Format demandé pour la session (`media::FOURCC_*`) : il sert si le
    /// paquet qui annonce le format du flux s'est perdu.
    pub codec: u32,
}

/// Démarre le fil de l'image d'un écran, avec son rendu : la chaîne
/// d'échange est déjà créée, sur le fil de la fenêtre (DXGI parle alors à la
/// fenêtre depuis son propre fil).
pub fn spawn(renderer: Renderer, hwnd: isize, gpu: Arc<SessionGpu>, opts: VideoOptions, shared: Arc<Shared>) -> Result<Arc<VideoHandler>, String> {
    let (tx, rx) = mpsc::sync_channel(240);
    let handler = Arc::new(VideoHandler {
        tx,
        thread: Mutex::new(None),
        overflow: AtomicBool::new(false),
        pending: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        pool: Arc::default(),
    });
    let h = handler.clone();
    let thread = std::thread::Builder::new()
        .name("kybernow-video".into())
        .spawn(move || run(renderer, hwnd, gpu, opts, shared, h, rx))
        .map_err(|e| e.to_string())?;
    *handler.thread.locked() = Some(thread);
    Ok(handler)
}

/// Après la perte de la carte, on réessaie de la recréer pendant au plus ce
/// temps (un TDR rend la carte en 2 à 3 s), puis le fil s'arrête.
const RECOVER_WITHIN: Duration = Duration::from_secs(15);

fn run(
    mut renderer: Renderer,
    hwnd: isize,
    session_gpu: Arc<SessionGpu>,
    opts: VideoOptions,
    shared: Arc<Shared>,
    handler: Arc<VideoHandler>,
    rx: Receiver<VideoMsg>,
) {
    // Rien n'est présenté avant la première image (la fenêtre reste cachée
    // jusque-là) : une présentation sans attendre l'objet d'attente de la
    // chaîne fausserait son compte (première image retardée, ou une image de
    // plus en file pour toute la session).
    let mut gpu = session_gpu.current();
    let mut decoder: Option<Decoder> = None;
    // PyroWave (docs/pyrowave-client-windows.md) : son propre décodeur, à la
    // place de FFmpeg, et le dernier paquet de configuration reçu.
    let mut pyro: Option<pyrowave::Decoder> = None;
    let mut pyro_config: Option<PyroConfig> = None;
    let mut pyro_skipped: u64 = 0;
    // L'ouverture du décodeur PyroWave a échoué pour ce format : pas de nouvel
    // essai à chaque image (fence, handle partagé, journal) avant le prochain
    // changement de format.
    let mut pyro_failed = false;
    let mut fourcc = 0u32;
    let mut config: Vec<u8> = Vec::new();
    let mut send_config = false;
    let mut waiting_key = true;
    // Ce que le décodeur a reçu : reprise sans image clé possible.
    let mut gate = ResumeGate::default();
    let mut hw_allowed = opts.hardware;
    let mut unsupported_logged = false;
    let mut last_request = Instant::now() - Duration::from_secs(1);
    // Diagnostic : le journal note le format annoncé, les paramètres, les
    // premières données et la première image clé.
    let (mut seen_codec, mut seen_config, mut seen_frame, mut seen_key) = (false, false, false, false);
    let mut dropped: u64 = 0;
    // Format voulu par la session, et s'il a été annoncé depuis son dernier changement.
    shared.link.codec.store(opts.codec, Ordering::Relaxed);
    let mut last_wanted = opts.codec;
    let mut codec_announced = false;
    // Arrivée des dernières images (pts, µs) : le décodeur peut rendre une
    // image un peu après d'autres arrivées.
    let mut arrivals: VecDeque<(u64, i64)> = VecDeque::with_capacity(32);
    let mut keyframe = |shared: &Shared, force: bool| {
        if force || last_request.elapsed() >= Duration::from_millis(300) {
            last_request = Instant::now();
            shared.request_keyframe();
        }
    };
    let open_pyro = |gpu: &Gpu, shared: &Shared, config: Option<PyroConfig>| -> Option<pyrowave::Decoder> {
        let device = gpu.pyrowave()?;
        match pyrowave::Decoder::new(device, &gpu.device) {
            Ok(mut d) => {
                if let Some(c) = config {
                    if let Err(e) = d.configure(c) {
                        log::error!("PyroWave : {e}");
                    }
                }
                let what = |lang: Lang| format!("PyroWave · {}", crate::i18n::tr(lang, "decoder.hardware", &[]));
                log::info!("décodeur : {}", what(Lang::Fr));
                *shared.decoder.locked() = what(crate::i18n::lang());
                Some(d)
            }
            Err(e) => {
                log::error!("décodeur PyroWave : {e}");
                None
            }
        }
    };
    let open = |gpu: &Gpu, fourcc: u32, hw: bool, shared: &Shared| -> Option<Decoder> {
        match Decoder::new(fourcc, hw.then_some(&gpu.device), Some(&gpu.lock)) {
            Ok(d) => {
                let key = if d.hardware { "decoder.hardware" } else { "decoder.software" };
                let what = |lang: Lang| format!("{} · {}", codec_name(fourcc), crate::i18n::tr(lang, key, &[]));
                log::info!("décodeur : {}", what(Lang::Fr));
                // Montré dans la barre et le titre de la session : langue de l'app.
                *shared.decoder.locked() = what(crate::i18n::lang());
                Some(d)
            }
            Err(e) => {
                log::error!("décodeur {} : {e}", codec_name(fourcc));
                None
            }
        }
    };

    loop {
        if handler.stopped.load(Ordering::Relaxed) {
            break;
        }
        // Carte graphique perdue : carte de la session et rendu recréés,
        // décodeurs rouverts sur elle, image clé demandée.
        if let Some(why) = renderer.lost().map(str::to_string) {
            log::error!("image : carte graphique perdue ({why}), reprise");
            decoder = None;
            pyro = None;
            pyro_failed = false;
            let vsync = renderer.vsync;
            // Une seule chaîne d'échange par fenêtre : l'ancienne part d'abord.
            drop(renderer);
            let started = Instant::now();
            let mut lost = gpu.clone();
            let fresh = loop {
                let made = session_gpu.recover(&lost).and_then(|g| match Renderer::new(&g, HWND(hwnd as *mut _), shared.client_size(), vsync) {
                    Ok(r) => Ok((g, r)),
                    Err(e) => {
                        // La carte neuve est déjà perdue (TDR pas fini) : la suivante sera recréée.
                        if g.is_lost() {
                            lost = g;
                        }
                        Err(e)
                    }
                });
                match made {
                    Ok(done) => break Some(done),
                    Err(e) if started.elapsed() < RECOVER_WITHIN && !handler.stopped.load(Ordering::Relaxed) => {
                        log::warn!("image : carte pas encore revenue ({e}), nouvel essai dans 1 s");
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    Err(e) => {
                        log::error!("image : carte graphique pas revenue ({e}), fil de l'image arrêté");
                        break None;
                    }
                }
            };
            let Some((g, r)) = fresh else { return };
            gpu = g;
            renderer = r;
            log::info!("image : rendu recréé sur la nouvelle carte");
            hw_allowed = opts.hardware;
            if fourcc != 0 && fourcc != FOURCC_PYROWAVE {
                decoder = open(&gpu, fourcc, hw_allowed, &shared);
            }
            gate.reset();
            send_config = true;
            waiting_key = true;
            unsupported_logged = false;
            keyframe(&shared, true);
        }
        if renderer.resize(shared.client_size()) {
            // Plein écran ↔ fenêtre sur un bureau immobile : l'image en place
            // reprend la nouvelle taille tout de suite, sans attendre la suivante.
            renderer.redraw(shared.edge_hint.load(Ordering::Relaxed));
        }
        shared.check_watch();
        let msg = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => {
                // Le trait du bord haut a changé sans image neuve (écran fixe) :
                // l'image en place est redessinée, avec ou sans le trait.
                renderer.repaint(shared.edge_hint.load(Ordering::Relaxed));
                // Comme sur Mac : une image clé se demande après une perte,
                // pas avant les premières données (l'encodeur commence par
                // une image clé ; le harceler pendant qu'il démarre le bloque).
                // Décodeur intact après une perte : le cœur a demandé la
                // reprise et redemande lui-même une image clé.
                if waiting_key && seen_frame && !gate.intact() {
                    keyframe(&shared, false);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match msg {
            VideoMsg::Stop => break,
            // Le redessin se fait en haut de la boucle, au tour suivant.
            VideoMsg::Resized => {}
            VideoMsg::Lost => {
                if !waiting_key {
                    log::info!("image : perte non réparée, attente d'une image clé ou d'une reprise");
                }
                let first = !waiting_key;
                waiting_key = true;
                if shared.link.console.get().is_some() {
                    keyframe(&shared, true);
                } else if first {
                    shared.image_lost();
                }
            }
            VideoMsg::Codec(c) => {
                codec_announced = true;
                if !seen_codec {
                    seen_codec = true;
                    log::info!("image : format annoncé, {}", codec_name(c));
                }
                // Début d'un flux : décodeur neuf.
                fourcc = c;
                decoder.take();
                gate.reset();
                pyro.take();
                pyro_failed = false;
                if c == FOURCC_PYROWAVE {
                    // Le paquet de configuration suit ; celui d'avant ne vaut plus.
                    pyro_config = None;
                    pyro = open_pyro(&gpu, &shared, None);
                    pyro_failed = pyro.is_none();
                } else {
                    decoder = open(&gpu, c, hw_allowed, &shared);
                }
                waiting_key = true;
                send_config = true;
                shared.first_image.store(false, Ordering::Relaxed);
            }
            VideoMsg::Config(d) => {
                if !seen_config {
                    seen_config = true;
                    log::info!("image : paramètres du flux reçus ({} octets)", d.len());
                }
                if fourcc == FOURCC_PYROWAVE || PyroConfig::is_config(&d) {
                    pyro_configure(&mut pyro, &mut pyro_config, &d);
                    continue;
                }
                // AV1 : un `av1C` (4 octets de résumé, puis les OBU) n'entre
                // pas tel quel dans un flux d'OBU.
                config = if fourcc == FOURCC_AV1 && d.len() > 4 && d[0] == 0x81 { d[4..].to_vec() } else { d };
                send_config = true;
            }
            VideoMsg::Frame { data, pts, key, received, resume } => {
                if arrivals.len() >= 32 {
                    arrivals.pop_front();
                }
                arrivals.push_back((pts, received));
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
                let wanted = shared.link.codec.load(Ordering::Relaxed);
                if wanted != last_wanted {
                    last_wanted = wanted;
                    codec_announced = false;
                }
                // Profil changé vers un autre format et paquet de format perdu.
                let stale = decoder.is_some() && wanted != 0 && fourcc != wanted && !codec_announced;
                if (decoder.is_none() && fourcc == 0 && wanted != 0) || stale {
                    log::warn!("image : format jamais annoncé, décodeur {} ouvert d'office", codec_name(wanted));
                    fourcc = wanted;
                    // PyroWave n'est pas pour FFmpeg : son décodeur s'ouvre plus bas.
                    decoder = if fourcc == FOURCC_PYROWAVE { None } else { open(&gpu, fourcc, hw_allowed, &shared) };
                    gate.reset();
                    send_config = true;
                    waiting_key = true;
                }
                // PyroWave : chaque image est complète (intra seule), rien
                // n'attend une image clé ; une image en retard ou incomplète
                // est seulement sautée.
                if fourcc == FOURCC_PYROWAVE || (fourcc == 0 && wanted == FOURCC_PYROWAVE) {
                    if fourcc == 0 {
                        log::warn!("image : format jamais annoncé, décodeur PyroWave ouvert d'office");
                        fourcc = FOURCC_PYROWAVE;
                        decoder.take();
                    }
                    handler.overflow.store(false, Ordering::Relaxed);
                    if PyroConfig::is_config(&data) {
                        pyro_configure(&mut pyro, &mut pyro_config, &data);
                        continue;
                    }
                    if pyro.is_none() && !pyro_failed {
                        pyro = open_pyro(&gpu, &shared, pyro_config);
                        if pyro.is_none() {
                            // Noté une fois (`open_pyro`) ; pas d'autre essai avant un autre format.
                            pyro_failed = true;
                        }
                    }
                    // D'autres images attendent : celle-ci n'est pas décodée du tout.
                    let Some(dec) = pyro.as_mut().filter(|_| !newer_waiting) else {
                        if !newer_waiting {
                            dropped += 1;
                            if dropped == 1 || dropped % 600 == 0 {
                                log::warn!("image : {dropped} images reçues sans décodeur PyroWave");
                            }
                        }
                        continue;
                    };
                    match dec.decode(&data) {
                        Ok(Outcome::Ready(img)) => {
                            let decoded = now_micros();
                            renderer.set_screen(*shared.screen_hdr.locked());
                            renderer.set_snapshots(shared.is_primary());
                            match renderer.draw_pyrowave(dec, img.config, shared.edge_hint.load(Ordering::Relaxed)) {
                                Ok(()) => shown(&shared, hwnd, &arrivals, pts, decoded),
                                // Repris en haut de la boucle.
                                Err(DrawError::Lost(_)) => {}
                                Err(e) => {
                                    if !unsupported_logged {
                                        log::error!("rendu PyroWave : {e}");
                                        unsupported_logged = true;
                                    }
                                }
                            }
                        }
                        Ok(Outcome::Skipped(why)) => {
                            pyro_skipped += 1;
                            if pyro_skipped <= 3 || pyro_skipped % 600 == 0 {
                                log::warn!("PyroWave : {why}, image sautée ({pyro_skipped} en tout)");
                            }
                        }
                        Err(e) => {
                            pyro_skipped += 1;
                            if pyro_skipped <= 3 || pyro_skipped % 600 == 0 {
                                log::error!("PyroWave : {e} ({pyro_skipped} images sautées en tout)");
                            }
                        }
                    }
                    continue;
                }
                if handler.overflow.swap(false, Ordering::Relaxed) {
                    log::warn!("image : file pleine, reprise sur une image clé");
                    waiting_key = true;
                    gate.reset();
                }
                // Après une perte : une image clé, ou une image de reprise qui
                // ne dépend que d'images que ce décodeur a déjà (il n'est pas
                // refait). Décodeur intact : le cœur redemande lui-même.
                let resuming = waiting_key && !key && decoder.is_some() && gate.resumes(resume);
                if waiting_key && !key && !resuming {
                    if !gate.intact() {
                        keyframe(&shared, false);
                    }
                    continue;
                }
                if waiting_key {
                    if resuming {
                        log::info!("image : reprise sans image clé");
                    }
                    shared.image_recovered();
                }
                waiting_key = false;
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
                    gate.reset();
                    keyframe(&shared, true);
                    continue;
                }
                gate.fed(pts);
                let mut fallback = false;
                while let Some(r) = dec.receive() {
                    match r {
                        Ok(frame) => {
                            let decoded = now_micros();
                            if is_corrupt(frame) {
                                keyframe(&shared, false);
                            }
                            let pts = frame.pts as u64;
                            if newer_waiting {
                                continue;
                            }
                            renderer.set_screen(*shared.screen_hdr.locked());
                            renderer.set_snapshots(shared.is_primary());
                            match renderer.draw(frame, shared.edge_hint.load(Ordering::Relaxed), shared.stream_hdr.load(Ordering::Relaxed)) {
                                Ok(()) => shown(&shared, hwnd, &arrivals, pts, decoded),
                                // Carte perdue : pas un repli sur le processeur, la
                                // carte est recréée en haut de la boucle.
                                Err(DrawError::Lost(_)) => break,
                                Err(e) => {
                                    if dec.hardware {
                                        log::warn!("rendu : {e}");
                                        fallback = true;
                                    } else if !unsupported_logged {
                                        // Format que l'app ne sait pas montrer (10 bits…).
                                        log::error!("rendu impossible : {e}");
                                        unsupported_logged = true;
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("décodage : {}", decoder::error_text(e));
                            waiting_key = true;
                            gate.reset();
                            keyframe(&shared, true);
                            break;
                        }
                    }
                }
                if fallback {
                    // La carte décode dans un format qu'on ne sait pas montrer.
                    log::warn!("décodage par le processeur à la place de la carte graphique");
                    hw_allowed = false;
                    decoder = open(&gpu, fourcc, false, &shared);
                    gate.reset();
                    send_config = true;
                    waiting_key = true;
                    keyframe(&shared, true);
                }
            }
        }
    }
    if let Some((px, w, h)) = renderer.take_snapshot() {
        match crate::snapshot::jpeg(&px, w, h) {
            Ok(j) => *shared.last_image.locked() = Some(j),
            Err(e) => log::warn!("dernière image : {e}"),
        }
    }
    log::info!("fil de l'image arrêté");
}

/// Une image vient d'être montrée : les mesures de l'écran principal
/// (arrivée, décodage et affichage, pour le trajet d'une image), le compte
/// des images, et la fenêtre montrée à sa première image.
fn shown(shared: &Shared, hwnd: isize, arrivals: &VecDeque<(u64, i64)>, pts: u64, decoded: i64) {
    if shared.is_primary() {
        let at = now_micros();
        let received = arrivals.iter().rev().find(|(p, _)| *p == pts).map_or(decoded, |(_, r)| *r);
        if let Some(l) = shared.link.latency.get() {
            l.frame_displayed(pts, received, decoded, at);
        }
        shared.link.push_local((decoded - received) as f64 / 1000.0, (at - decoded) as f64 / 1000.0);
    }
    shared.frames.fetch_add(1, Ordering::Relaxed);
    if !shared.first_image.swap(true, Ordering::Relaxed) {
        // La fenêtre de cet écran se montre à sa première image.
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), crate::window::WM_APP_FIRST_IMAGE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Paquet de configuration PyroWave : gardé, et donné au décodeur (qui se
/// recrée s'il change). Illisible : la définition sera lue dans le flux.
fn pyro_configure(pyro: &mut Option<pyrowave::Decoder>, kept: &mut Option<PyroConfig>, data: &[u8]) {
    let Some(c) = PyroConfig::parse(data) else {
        log::warn!("PyroWave : paquet de configuration illisible ({} octets)", data.len());
        return;
    };
    if *kept != Some(c) {
        log::info!("PyroWave : configuration {}", c.label());
    }
    *kept = Some(c);
    if let Some(d) = pyro.as_mut() {
        if let Err(e) = d.configure(c) {
            log::error!("PyroWave : {e}");
        }
    }
}
