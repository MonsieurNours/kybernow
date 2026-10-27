//! Une session vue de Linux : ce que l'app fournit au cœur
//! (`session::Platform`) et les rappels vers la boucle principale, comme sur
//! webOS (apps/webos/src/device/session.rs) et Android.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, Options, PadId, PadKey, Pasteboard, PointerLayer, ScreenTarget};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::Second;
use kybernow_core::media::PacketHandler;
use kybernow_core::profile::{Capabilities, Codec, Profile as CoreProfile, StreamSettings};
use kybernow_core::screens::Screens;
use kybernow_core::session::{Notice, Platform, Recovery, Session, SessionParams, SessionRole, Status, StreamScreen};
use kybernow_core::{Display, WindowHandle};

use super::media::AudioSink;
use super::video::{Latest, VideoSink};
use super::{sdl, Event, SessionEvent};
use crate::app::{Machine, SessionId};
use crate::model::{Host, Profile};

/// Le presse-papiers du système, vu depuis le fil des entrées : SDL ne se
/// touche que depuis le fil principal, qui recopie ici le texte du système
/// (`system_text`) et applique ce que l'ordinateur envoie (`write_outgoing`). Plusieurs ordinateurs à
/// la fois : c'est le pont du cœur qui se met en pause quand la session n'est pas active (rien n'est
/// lu ici, rien n'y est écrit, ce que l'ordinateur annonce est ignoré, `Session::set_active`) ; le
/// texte ne passe donc que par l'appareil, jamais d'un ordinateur à l'autre en arrière-plan. Le
/// texte recopié doit être frais quand une session reprend la main : `device::focus` le renouvelle
/// avant chaque changement de session active.
pub struct Clipboard {
    text: Mutex<Option<String>>,
    count: AtomicU64,
    outgoing: Mutex<Option<String>>,
}

impl Clipboard {
    fn new() -> Self {
        Self { text: Mutex::new(None), count: AtomicU64::new(0), outgoing: Mutex::new(None) }
    }

    /// Le texte du système (fil principal).
    pub fn system_text(&self, t: Option<String>) {
        let mut cur = self.text.lock().unwrap();
        if *cur != t {
            *cur = t;
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Ce que l'ordinateur a envoyé est posé sur le presse-papiers de l'appareil (fil principal).
    /// Le pont du cœur n'envoie rien ici tant que la session n'est pas active.
    pub fn write_outgoing(&self) {
        if let Some(t) = self.outgoing.lock().unwrap().take() {
            sdl::set_clipboard(&t);
        }
    }
}

impl Pasteboard for Clipboard {
    fn change_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
    fn text(&self) -> Option<String> {
        self.text.lock().unwrap().clone()
    }
    fn set_text(&self, text: &str) {
        *self.text.lock().unwrap() = Some(text.to_string());
        self.count.fetch_add(1, Ordering::Relaxed);
        *self.outgoing.lock().unwrap() = Some(text.to_string());
    }
}

/// Ce que partagent la session, ses flux et la boucle principale.
pub struct Shared {
    /// L'identifiant de la session de l'app : tout ce qu'elle envoie à la boucle en est étiqueté.
    pub id: SessionId,
    tx: Mutex<Sender<Event>>,
    session: OnceLock<Weak<Session>>,
    pub pointer: Arc<PointerLayer>,
    pub clock: Arc<MonotonicClock>,
    pub input: Mutex<Option<Arc<InputPipeline>>>,
    display: Mutex<(u32, u32, u32)>,
    mode: Mutex<Mode>,
    audio_channels: u8,
    pub combo_enabled: AtomicBool,
    pub relative: AtomicBool,
    codec: Mutex<String>,
    /// L'image la plus récente de chaque écran de l'appareil, pour le fil
    /// principal (une seule en dehors du multi-écrans).
    latests: Mutex<HashMap<usize, Arc<Latest>>>,
    /// Multi-écrans : les écrans pris par la session, les couches de pointeur
    /// des écrans autres que le principal, et l'écran de l'appareil de
    /// l'écran principal de la session.
    screens: Mutex<ScreensState>,
    layers: Mutex<HashMap<usize, Arc<PointerLayer>>>,
    primary: AtomicUsize,
    /// Décodeur matériel par format : H.264, HEVC, AV1 (VA-API, ou V4L2 sur le Pi).
    hw: [bool; 3],
    /// Lequel : 1 VA-API, 2 V4L2 (Raspberry Pi).
    pub hw_backend: i32,
    pub clipboard: Option<Arc<Clipboard>>,
    pub audio: Mutex<Option<Weak<AudioSink>>>,
    /// « Couper le son » du menu de session : le choix de l'utilisateur, distinct du son que le cœur
    /// coupe aux sessions qui ne sont ni actives ni gardées (`Session::set_muted`). Partagé avec les
    /// sorties de la session, que la reconnexion recrée.
    silenced: Arc<AtomicBool>,
}

/// Multi-écrans : ce que la plateforme a reçu de `Platform::on_screens`.
#[derive(Clone, Default)]
pub struct ScreensState {
    /// Les écrans pris, le principal en premier.
    pub streams: Vec<StreamScreen>,
    /// Les écrans de l'appareil que l'ordinateur n'a pas pu créer.
    pub not_created: Vec<usize>,
}

impl Shared {
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.get().and_then(Weak::upgrade)
    }

    fn send(&self, e: SessionEvent) {
        let _ = self.tx.lock().unwrap().send(Event::Session(self.id, e));
    }

    /// « Couper le son » du menu de session, ou le rendre (fondu de 20 ms). Le son que le cœur coupe aux
    /// sessions en arrière-plan est à part (`Session::set_muted`, par le registre `focus`).
    pub fn set_silenced(&self, silenced: bool) {
        self.silenced.store(silenced, Ordering::Relaxed);
    }

    pub fn host_size(&self) -> (u32, u32) {
        let d = self.display.lock().unwrap();
        if d.1 > 0 && d.2 > 0 {
            (d.1, d.2)
        } else {
            let m = self.mode.lock().unwrap();
            (m.width.max(1), m.height.max(1))
        }
    }

    /// Taille de l'écran de l'ordinateur qui va à l'écran `screen` de l'appareil.
    pub fn host_size_of(&self, screen: usize) -> (u32, u32) {
        let found = self.screens.lock().unwrap().streams.iter().find(|s| s.screen == screen).map(|s| (s.display.width.max(1) as u32, s.display.height.max(1) as u32));
        found.unwrap_or_else(|| self.host_size())
    }

    /// L'écran de l'appareil de l'écran principal de la session.
    pub fn primary(&self) -> usize {
        self.primary.load(Ordering::Relaxed)
    }

    /// Les écrans pris par la session et ceux que l'ordinateur n'a pas pu créer.
    pub fn screens(&self) -> ScreensState {
        self.screens.lock().unwrap().clone()
    }

    /// L'image la plus récente de cet écran (la file est créée si elle manque).
    pub fn latest_for(&self, screen: usize) -> Arc<Latest> {
        self.latests.lock().unwrap().entry(screen).or_default().clone()
    }

    /// Un flux d'image démarre pour cet écran : une file neuve.
    pub fn fresh_latest(&self, screen: usize) -> Arc<Latest> {
        let latest = Arc::new(Latest::default());
        self.latests.lock().unwrap().insert(screen, latest.clone());
        latest
    }

    /// Le flux de l'écran `from` va maintenant à l'écran `to` (il continue, avec sa file d'images).
    fn move_latest(&self, from: usize, to: usize) {
        let mut latests = self.latests.lock().unwrap();
        if let Some(latest) = latests.remove(&from) {
            latests.insert(to, latest);
        }
    }

    /// Le pointeur de l'ordinateur pour la souris Auto, sur tous les écrans :
    /// la règle du cœur (`PointerLayer::summary`, docs/multi-ecrans.md).
    pub fn pointer_summary(&self) -> (bool, Duration) {
        let layers = self.layers.lock().unwrap();
        let s = PointerLayer::summary(std::iter::once(self.pointer.as_ref()).chain(layers.values().map(|l| l.as_ref())));
        (s.hidden, s.visible_for)
    }

    /// Les écrans de la session vus des entrées : chacun avec sa couche de
    /// pointeur (l'écran principal : celle de la chaîne).
    fn targets(&self) -> Vec<ScreenTarget> {
        let streams = self.screens.lock().unwrap().streams.clone();
        let layers = self.layers.lock().unwrap();
        let primary = streams.first().map(|s| s.screen);
        streams
            .iter()
            .map(|s| ScreenTarget {
                screen: s.screen,
                display_id: s.display.id,
                host_size: (s.display.width.max(0) as u32, s.display.height.max(0) as u32),
                pointer: if Some(s.screen) == primary { None } else { layers.get(&s.screen).cloned() },
            })
            .collect()
    }

    /// Une couche de pointeur pour un écran autre que le principal : sa
    /// position et sa visibilité vont à la fenêtre de cet écran.
    fn new_layer(self: &Arc<Self>, screen: usize, host: (f64, f64)) -> Arc<PointerLayer> {
        let layer = Arc::new(PointerLayer::with_host_size(self.clock.clone(), false, host));
        let weak = Arc::downgrade(self);
        layer.set_on_changed(move |state, change| {
            let Some(sh) = weak.upgrade() else { return };
            let shape = if change.shape { state.shape.as_ref().map(|s| (s.width, s.height, s.hot_x, s.hot_y, s.premultiplied())) } else { None };
            sh.send(SessionEvent::Pointer { screen, position: state.position(), hidden: state.remote_hidden, shape });
        });
        layer
    }

    pub fn current_input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().clone()
    }

    pub fn set_codec(&self, name: &str) {
        *self.codec.lock().unwrap() = name.to_string();
    }

    pub fn codec(&self) -> String {
        self.codec.lock().unwrap().clone()
    }

    pub fn decoder_failed(&self, e: &str) {
        self.send(SessionEvent::DecoderFailed(e.to_string()));
    }

    /// Un décodeur matériel pour ce format (0 H.264, 1 HEVC, 2 AV1).
    pub fn hw_for(&self, codec: i32) -> bool {
        self.hw.get(codec as usize).copied().unwrap_or(false)
    }

    /// Latence du son jusqu'à la sortie, si le moteur la connaît.
    pub fn audio_ms(&self) -> Option<f64> {
        let a = self.audio.lock().unwrap().as_ref().and_then(Weak::upgrade)?;
        let s = a.engine().take_stats()?;
        Some(s.buffer_ms + s.output_ms)
    }
}

struct LinuxPlatform(Arc<Shared>);

impl Platform for LinuxPlatform {
    fn window_handle(&self) -> WindowHandle {
        // L'image et le son vont à l'app : Kyber ne crée pas de lecteur VLC.
        WindowHandle::Xlib(0)
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone(), self.0.primary())
    }

    /// Multi-écrans : un flux, un décodeur et une image par écran de l'appareil.
    fn screen_video_handler(&self, screen: usize) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone(), screen)
    }

    /// Les écrans pris par la session, le principal en premier ; les fenêtres
    /// suivent (fil principal, `Event::Screens`) et les entrées reçoivent leur
    /// écran : position de la souris par écran, pointeur annoncé par écran.
    fn on_screens(&self, screens: &[StreamScreen], not_created: &[usize]) {
        let sh = &self.0;
        let primary = screens.first().map_or(sh.primary(), |s| s.screen);
        sh.primary.store(primary, Ordering::Relaxed);
        let before = std::mem::replace(&mut *sh.screens.lock().unwrap(), ScreensState { streams: screens.to_vec(), not_created: not_created.to_vec() });
        // Une session à un écran qui suit la fenêtre sur un autre écran de l'appareil (« Cet écran »
        // a changé, `Session::set_screens`) : le flux continue sans repartir, son image est
        // retrouvée sous le nouvel écran (sinon la fenêtre resterait noire).
        if let ([old], [new]) = (before.streams.as_slice(), screens)
            && old.screen != new.screen
        {
            sh.move_latest(old.screen, new.screen);
        }
        {
            let mut layers = sh.layers.lock().unwrap();
            // Une couche par écran qui n'est pas le principal ; celles des écrans disparus s'en vont.
            layers.retain(|k, _| screens.iter().any(|s| s.screen == *k && s.screen != primary));
            for s in screens.iter().filter(|s| s.screen != primary) {
                layers.entry(s.screen).or_insert_with(|| sh.new_layer(s.screen, (f64::from(s.display.width), f64::from(s.display.height))));
            }
        }
        // La chaîne d'entrées déjà en place (la taille d'un écran a changé) en est prévenue.
        if let Some(p) = sh.current_input() {
            p.set_screens(sh.targets());
        }
        sh.send(SessionEvent::Screens);
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        if self.0.audio_channels == 0 {
            return None;
        }
        let sink = AudioSink::new(self.0.audio_channels, self.0.silenced.clone());
        *self.0.audio.lock().unwrap() = Some(Arc::downgrade(&sink));
        Some(sink as Arc<dyn PacketHandler>)
    }

    fn input_pipeline(&self) -> Pipeline {
        let (id, w, h) = *self.0.display.lock().unwrap();
        let callbacks: Arc<dyn InputPlatform> = Arc::new(InputCallbacks(self.0.clone()));
        let clock: Arc<dyn Clock> = self.0.clock.clone();
        let clipboard = self.0.clipboard.clone().map(|c| c as Arc<dyn Pasteboard>);
        let pipeline = InputPipeline::new(Options { display_id: id, host_size: (w, h), clipboard, gamepads_from_app: true }, self.0.pointer.clone(), callbacks, clock);
        pipeline.combo().set_enabled(self.0.combo_enabled.load(Ordering::Relaxed));
        pipeline.set_relative(self.0.relative.load(Ordering::Relaxed));
        // Multi-écrans : les écrans de la session (reçus par `on_screens`, juste avant).
        let targets = self.0.targets();
        if !targets.is_empty() {
            pipeline.set_screens(targets);
        }
        if let Err(e) = pipeline.spawn_driver() {
            log::error!("entrées : fil des minuteries impossible ({e})");
        }
        let kynput = pipeline.kynput();
        if let Some(old) = self.0.input.lock().unwrap().replace(pipeline) {
            old.stop();
            old.wake();
        }
        kynput
    }

    fn on_status(&self, status: &Status) {
        if matches!(status, Status::Ended { .. }) {
            self.0.pointer.reset();
        }
        self.0.send(SessionEvent::Status(status.clone()));
    }

    fn on_display(&self, d: &Display) {
        *self.0.display.lock().unwrap() = (d.id, d.width.max(0) as u32, d.height.max(0) as u32);
        self.0.pointer.set_host_size((d.width as f64, d.height as f64));
    }

    fn on_mode(&self, mode: Mode, _virtual_display: bool) {
        *self.0.mode.lock().unwrap() = mode;
        self.0.send(SessionEvent::Mode(mode));
    }

    fn on_stream_stopped(&self) {
        if let Some(p) = self.0.input.lock().unwrap().take() {
            p.stop();
            p.wake();
        }
    }

    fn on_input_connected(&self) {
        if let Some(p) = self.0.current_input() {
            p.network_connected();
            if self.0.clipboard.is_some() {
                p.start_clipboard();
            }
        }
        self.0.pointer.resend_held();
    }

    fn on_second(&self, s: &Second) {
        self.0.send(SessionEvent::Second {
            latency_ms: s.latency_ms,
            fps: s.displayed as f64,
            mbps: s.bytes as f64 * 8.0 / 1e6,
            lost: s.lost_frames + s.holes,
            codec: self.0.codec(),
            audio_ms: self.0.audio_ms(),
        });
    }

    fn on_game_fullscreen(&self, game: bool) {
        self.0.send(SessionEvent::GameFullscreen(game));
    }

    fn on_notice(&self, notice: &Notice) {
        use crate::i18n::tr;
        let msg = match notice {
            Notice::Relaunched => tr("Image relancée", "Picture restarted").to_string(),
            Notice::ModeChanged { width, height, .. } => format!("{} {width} × {height}", tr("L'ordinateur est passé en", "The computer switched to")),
            Notice::Fallback { bitrate_mbps, .. } => format!("{} {bitrate_mbps} Mbit/s", tr("Image allégée :", "Lighter picture:")),
            Notice::Adapting { width, height } => format!("{} {width} × {height}…", tr("Écran virtuel adapté :", "Adapting virtual display:")),
            Notice::Adapted { width, height } => format!("{} {width} × {height}", tr("Écran virtuel :", "Virtual display:")),
            Notice::AdaptFailed { error } => error.clone(),
            Notice::RestoreOffer { .. } => return,
        };
        self.0.send(SessionEvent::Notice(msg));
    }
}

struct InputCallbacks(Arc<Shared>);

impl InputPlatform for InputCallbacks {
    fn rumble(&self, r: GamepadRumble) {
        self.0.send(SessionEvent::Rumble(r));
    }

    fn combo_formed(&self, _pad: PadId) {
        self.0.send(SessionEvent::Combo);
    }

    fn navigate(&self, _key: PadKey) {}
}

pub struct Handle {
    pub shared: Arc<Shared>,
    pub session: Arc<Session>,
}

pub fn core_profile(p: Profile) -> CoreProfile {
    CoreProfile::parse(p.id()).unwrap_or(CoreProfile::Jeu)
}

/// Ce que cette machine décode pour ce format (`StreamSettings::for_profile`) :
/// HEVC par la carte ou le processeur, le 4:4:4 seulement s'il est demandé
/// (HEVC), AV1 si c'est le format. C'est aussi ce que le cœur compare aux
/// réglages pour dire « Personnalisé » au nombre d'écrans
/// (`Screens::capabilities`, docs/multi-ecrans.md).
pub fn capabilities(host: &Host, codec: &str, m: &Machine) -> Capabilities {
    let yuv444 = host.settings.yuv444 && codec == "hevc";
    Capabilities { hevc: m.decodes("hevc"), hevc444: yuv444, av1: codec == "av1" }
}

/// Les réglages du flux : le profil ramené à ce que décode cette machine, le
/// format choisi, le 4:4:4 s'il est demandé (HEVC seulement), le débit choisi
/// ou allégé. Le profil reste celui de base, même quand un réglage s'en
/// écarte : la Station reçoit le profil (libellé, politique de mode `follow` /
/// `restore`) et son débit suit la définition. « Personnalisé » ne sert qu'au
/// nombre d'écrans, et c'est le cœur qui le décide (`screens::screens_profile`).
pub fn settings(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), m: &Machine, light: bool) -> StreamSettings {
    let yuv444 = host.settings.yuv444 && codec == "hevc";
    let mut s = StreamSettings::for_profile(core_profile(profile), capabilities(host, codec, m));
    s.codec = match codec {
        "h264" => Codec::H264,
        "av1" => Codec::Av1,
        _ => Codec::Hevc,
    };
    s.yuv444 = yuv444;
    if host.settings.reliable {
        s.reliable = true;
    }
    let (w, h, fps) = mode;
    let ratio = (w as f64 * h as f64 * fps.max(1) as f64) / (3840.0 * 2160.0 * 60.0);
    let scale = ratio.powf(0.75).clamp(0.4, 3.0);
    // Débit choisi : la base du profil ramenée pour l'atteindre à ce mode.
    if host.settings.mbps > 0 {
        s.mbps = ((host.settings.mbps as f64 / scale).floor() as u32).max(5);
    }
    // Alléger (batterie) : 20 Mbit/s au plus.
    if light {
        let actual = s.bitrate(w, h, fps) / 1_000_000;
        if actual > 20 {
            s.mbps = ((20.0 / scale).floor() as u32).max(5);
        }
    }
    s
}

/// Voies de son demandées à l'ordinateur.
pub fn audio_channels(host: &Host, m: &Machine) -> u16 {
    match host.settings.audio.as_str() {
        "computer" => 0,
        "stereo" => 2,
        "5.1" => 6,
        "7.1" => 8,
        _ => match m.audio_channels {
            8.. => 8,
            6..=7 => 6,
            _ => 2,
        },
    }
}

/// Démarre une session. `settings` : les réglages du flux (`settings`), que
/// l'app garde aussi pour le nombre d'écrans : ce sont ceux que le cœur lit. `screens` :
/// les écrans de l'appareil, pour l'interface de bureau (multi-écrans : le cœur
/// décide du nombre d'écrans) ; `None` : une session à un écran, comme avant
/// (Steam Deck, salon, KyberNow Link OS). `current` : l'écran de la fenêtre,
/// celui d'une session à un écran ; `mode` est celui de cet écran.
#[allow(clippy::too_many_arguments)]
pub fn start(id: SessionId, host: &Host, settings: StreamSettings, mode: (u32, u32, u32), m: &Machine, relative: bool, device: DeviceInfo, screens: Option<Screens>, current: usize, tx: Sender<Event>) -> Handle {
    let (w, h, fps) = mode;
    let clock = Arc::new(MonotonicClock::new());
    let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
    let audio_channels = audio_channels(host, m);
    let shared = Arc::new(Shared {
        id,
        tx: Mutex::new(tx),
        session: OnceLock::new(),
        pointer: pointer.clone(),
        clock,
        input: Mutex::new(None),
        display: Mutex::new((0, w, h)),
        mode: Mutex::new(Mode { width: w, height: h, refresh: fps }),
        audio_channels: audio_channels as u8,
        combo_enabled: AtomicBool::new(host.settings.combo),
        relative: AtomicBool::new(relative),
        codec: Mutex::new(String::new()),
        latests: Mutex::new(HashMap::new()),
        screens: Mutex::new(ScreensState::default()),
        layers: Mutex::new(HashMap::new()),
        primary: AtomicUsize::new(current),
        hw: [m.hw_h264 && (m.vaapi || m.pi.is_some()), m.hw_hevc && (m.vaapi || m.pi.is_some()), m.hw_av1 && m.vaapi],
        hw_backend: if m.pi.is_some() && !m.vaapi { 2 } else { 1 },
        clipboard: host.settings.clipboard.then(|| Arc::new(Clipboard::new())),
        audio: Mutex::new(None),
        silenced: Arc::new(AtomicBool::new(false)),
    });
    // Le pointeur de l'ordinateur : position et forme, pour la fenêtre.
    let weak = Arc::downgrade(&shared);
    pointer.set_on_changed(move |state, change| {
        let Some(sh) = weak.upgrade() else { return };
        let shape = if change.shape { state.shape.as_ref().map(|s| (s.width, s.height, s.hot_x, s.hot_y, s.premultiplied())) } else { None };
        sh.send(SessionEvent::Pointer { screen: sh.primary(), position: state.position(), hidden: state.remote_hidden, shape });
    });
    let params = SessionParams {
        console_host: host.address.clone(),
        console_port: host.port,
        user: host.user.clone(),
        password: host.password.clone(),
        console_fingerprint: host.fingerprint.clone(),
        device,
        mode: Mode { width: w, height: h, refresh: fps },
        audio_channels,
        virtual_display: host.settings.virtual_display,
        settings,
        auto_reconnect: host.settings.reconnect,
        gamepad_xbox360: host.settings.xbox360,
        clipboard: host.settings.clipboard,
        recovery: Recovery { watch_image: true, fallback_bitrate: Some(20_000_000), ..Recovery::default() },
        role: SessionRole::Host,
    };
    let platform = Arc::new(LinuxPlatform(shared.clone()));
    let session = match screens {
        Some(screens) => Session::start_with_screens(params, screens, platform),
        None => Session::start(params, platform),
    };
    let _ = shared.session.set(Arc::downgrade(&session));
    Handle { shared, session }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HostSettings;
    use kybernow_core::screens;

    fn host(settings: HostSettings) -> Host {
        Host {
            id: "a".into(),
            name: "Atelier".into(),
            address: "192.168.1.20".into(),
            port: 24811,
            fingerprint: None,
            user: "u".into(),
            password: String::new(),
            os: None,
            gpu: None,
            av1: false,
            mac: None,
            wake: Vec::new(),
            settings,
            last: None,
        }
    }

    const MODE: (u32, u32, u32) = (1920, 1080, 60);

    fn shared(current: usize) -> (Arc<Shared>, std::sync::mpsc::Receiver<Event>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let clock = Arc::new(MonotonicClock::new());
        let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
        let shared = Arc::new(Shared {
            id: 7,
            tx: Mutex::new(tx),
            session: OnceLock::new(),
            pointer: pointer.clone(),
            clock,
            input: Mutex::new(None),
            display: Mutex::new((0, 3840, 2160)),
            mode: Mutex::new(Mode { width: 3840, height: 2160, refresh: 60 }),
            audio_channels: 2,
            combo_enabled: AtomicBool::new(true),
            relative: AtomicBool::new(false),
            codec: Mutex::new(String::new()),
            latests: Mutex::new(HashMap::new()),
            screens: Mutex::new(ScreensState::default()),
            layers: Mutex::new(HashMap::new()),
            primary: AtomicUsize::new(current),
            hw: [false; 3],
            hw_backend: 1,
            clipboard: None,
            audio: Mutex::new(None),
            silenced: Arc::new(AtomicBool::new(false)),
        });
        let weak = Arc::downgrade(&shared);
        pointer.set_on_changed(move |state, change| {
            let Some(sh) = weak.upgrade() else { return };
            let shape = if change.shape { state.shape.as_ref().map(|s| (s.width, s.height, s.hot_x, s.hot_y, s.premultiplied())) } else { None };
            sh.send(SessionEvent::Pointer { screen: sh.primary(), position: state.position(), hidden: state.remote_hidden, shape });
        });
        (shared, rx)
    }

    fn stream(screen: usize, id: u32, w: i32, h: i32, primary: bool) -> StreamScreen {
        StreamScreen { screen, display: Display { id, width: w, height: h, x: 0, y: 0 }, refresh: 60, primary }
    }

    fn cursor(visible: bool) -> kybernow_core::input::RemoteCursor {
        kybernow_core::input::RemoteCursor { visible, image: None, position: (10, 20) }
    }

    /// `on_screens` : les écrans pris, leur écran principal, une couche de pointeur pour chaque autre
    /// écran, les écrans que l'ordinateur n'a pas pu créer.
    #[test]
    fn screens_reach_the_windows() {
        let (sh, rx) = shared(0);
        let platform = LinuxPlatform(sh.clone());
        // L'écran 2 de l'appareil est l'écran principal de la session (un seul écran : « Cet écran »).
        platform.on_screens(&[stream(2, 5, 1920, 1080, true)], &[]);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Screens))));
        assert_eq!((sh.primary(), sh.screens().streams.len(), sh.layers.lock().unwrap().len()), (2, 1, 0));
        assert_eq!(sh.host_size_of(2), (1920, 1080));
        // Trois écrans, dont un que l'ordinateur n'a pas pu créer.
        platform.on_screens(&[stream(0, 1, 3840, 2160, true), stream(1, 7, 5120, 2880, false)], &[2]);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Screens))));
        let st = sh.screens();
        assert_eq!((sh.primary(), st.streams.len(), st.not_created.clone()), (0, 2, vec![2]));
        assert_eq!((sh.host_size_of(0), sh.host_size_of(1)), ((3840, 2160), (5120, 2880)));
        assert_eq!(sh.layers.lock().unwrap().keys().copied().collect::<Vec<_>>(), vec![1], "une couche par écran qui n'est pas le principal");
        let targets = sh.targets();
        assert_eq!(targets.iter().map(|t| (t.screen, t.display_id, t.host_size)).collect::<Vec<_>>(), vec![(0, 1, (3840, 2160)), (1, 7, (5120, 2880))]);
        assert!(targets[0].pointer.is_none() && targets[1].pointer.is_some());
        // Chaque flux a sa file d'images, neuve à chaque démarrage de l'image.
        let (a, b) = (sh.latest_for(0), sh.latest_for(1));
        assert!(!Arc::ptr_eq(&a, &b));
        let fresh = sh.fresh_latest(0);
        assert!(!Arc::ptr_eq(&a, &fresh) && Arc::ptr_eq(&fresh, &sh.latest_for(0)));
        // Moins d'écrans : la couche de l'écran disparu s'en va.
        platform.on_screens(&[stream(0, 1, 3840, 2160, true)], &[]);
        assert!(sh.layers.lock().unwrap().is_empty());
    }

    /// Souris Auto : le pointeur de l'ordinateur n'est caché que s'il l'est sur tous les écrans, et
    /// visible depuis le plus long des délais ; chaque écran annonce le sien avec son indice.
    #[test]
    fn auto_mouse_looks_at_every_display() {
        let (sh, rx) = shared(0);
        let platform = LinuxPlatform(sh.clone());
        platform.on_screens(&[stream(0, 1, 3840, 2160, true), stream(1, 7, 5120, 2880, false)], &[]);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Screens))));
        let other = sh.layers.lock().unwrap().get(&1).cloned().unwrap();
        // Le jeu cache son pointeur sur l'écran principal ; l'autre écran ne l'a jamais vu : caché
        // (un écran jamais visité ne compte pas, règle du cœur).
        sh.pointer.remote_cursor_changed(cursor(false));
        assert!(sh.pointer.state().remote_hidden && sh.pointer_summary().0);
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::Pointer { screen, hidden, .. })) => assert_eq!((screen, hidden), (0, true)),
            _ => panic!("le pointeur de l'écran principal arrive avec son écran"),
        }
        // Le pointeur passe sur l'autre écran, où il est visible : pas caché.
        other.remote_cursor_changed(cursor(true));
        assert!(!sh.pointer_summary().0);
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::Pointer { screen, hidden, .. })) => assert_eq!((screen, hidden), (1, false)),
            _ => panic!("le pointeur de l'autre écran arrive avec son écran"),
        }
        // Caché sur les deux : caché.
        other.remote_cursor_changed(cursor(false));
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::Pointer { screen, hidden, .. })) => assert_eq!((screen, hidden), (1, true)),
            _ => panic!("le pointeur de l'autre écran arrive avec son écran"),
        }
        assert!(sh.pointer_summary().0);
        // Remontré sur l'un des deux : plus caché, et visible depuis ce moment.
        other.remote_cursor_changed(cursor(true));
        let (hidden, since) = sh.pointer_summary();
        assert!(!hidden && since < Duration::from_secs(1));
    }

    /// Un profil sans réglage retouché : le débit suit les pixels par seconde, comme avant.
    #[test]
    fn a_profile_keeps_its_bitrate_rule() {
        let m = Machine::default();
        let h = host(HostSettings::default());
        let s = settings(&h, Profile::Jeu, "hevc", MODE, &m, false);
        assert_eq!(s.profile, Some(CoreProfile::Jeu));
        assert_eq!(s.bitrate(1920, 1080, 60), 24_000_000);
        assert_eq!(s.bitrate(3840, 2160, 60), 60_000_000);
        // Alléger sur batterie : 20 Mbit/s au plus.
        let light = settings(&h, Profile::Max, "hevc", (3840, 2160, 60), &m, true);
        assert_eq!(light.profile, Some(CoreProfile::Max));
        assert!(light.bitrate(3840, 2160, 60) <= 21_000_000);
    }

    /// Des réglages retouchés : la Station reçoit toujours le profil de base (libellé, politique de
    /// mode, débit qui suit la définition) ; « Personnalisé » ne sert qu'au nombre d'écrans, et c'est
    /// le cœur qui le décide (`screens_profile`, avec ce que cette machine décode).
    #[test]
    fn a_retouched_profile_is_sent_as_its_profile() {
        let m = Machine::default();
        let retouched = [
            (HostSettings { mbps: 80, ..HostSettings::default() }, "hevc"),
            (HostSettings { codec: "h264".into(), ..HostSettings::default() }, "h264"),
            (HostSettings { reliable: true, ..HostSettings::default() }, "hevc"),
            (HostSettings { yuv444: true, ..HostSettings::default() }, "hevc"),
        ];
        for (st, codec) in retouched {
            let h = host(st);
            let s = settings(&h, Profile::Jeu, codec, MODE, &m, false);
            assert_eq!(s.profile, Some(CoreProfile::Jeu), "{:?}", h.settings);
            assert_eq!((s.label(), s.mode_policy()), ("Jeu", "follow"));
            assert_eq!(screens::screens_profile(&s, Some(capabilities(&h, codec, &m))), None, "{:?} : « Personnalisé » pour le nombre d'écrans", h.settings);
        }
        // Débit choisi : la base du profil ramenée pour l'atteindre à ce mode.
        let h = host(HostSettings { mbps: 80, ..HostSettings::default() });
        assert_eq!(settings(&h, Profile::Jeu, "hevc", MODE, &m, false).bitrate(1920, 1080, 60), 80_000_000);
        // Format choisi seul : le débit du profil, qui suit la définition.
        let h264 = settings(&host(HostSettings { codec: "h264".into(), ..HostSettings::default() }), Profile::Jeu, "h264", MODE, &m, false);
        assert_eq!(h264.bitrate(1920, 1080, 60), 24_000_000);
        assert_eq!(h264.bitrate(3840, 2160, 60), 60_000_000);
        // Sans réglage retouché : le profil, pour le cœur aussi.
        let h = host(HostSettings::default());
        for p in [Profile::Jeu, Profile::Bureau, Profile::Max] {
            let s = settings(&h, p, "hevc", MODE, &m, false);
            assert_eq!(screens::screens_profile(&s, Some(capabilities(&h, "hevc", &m))), s.profile, "{p:?}");
        }
        // 4:4:4 demandé en Bureau, que le profil prend de lui-même quand il est permis : pas un écart.
        let h = host(HostSettings { profile: Profile::Bureau, yuv444: true, ..HostSettings::default() });
        let s = settings(&h, Profile::Bureau, "hevc", MODE, &m, false);
        assert!(s.yuv444);
        assert_eq!(screens::screens_profile(&s, Some(capabilities(&h, "hevc", &m))), Some(CoreProfile::Bureau));
        // Alléger sur batterie ramène le débit à 20 Mbit/s : un écart, un seul écran en Auto.
        let h = host(HostSettings::default());
        let light = settings(&h, Profile::Bureau, "hevc", (3840, 2160, 60), &m, true);
        assert_eq!(light.profile, Some(CoreProfile::Bureau));
        assert_eq!(screens::screens_profile(&light, Some(capabilities(&h, "hevc", &m))), None);
    }

    /// Une session à un écran qui suit la fenêtre sur un autre écran de l'appareil (le cœur annonce
    /// le même flux sous un autre écran) : son image se retrouve sous le nouvel écran, la fenêtre
    /// n'en reste pas à une file vide.
    #[test]
    fn a_single_stream_keeps_its_picture_when_its_screen_changes() {
        let (sh, rx) = shared(1);
        let platform = LinuxPlatform(sh.clone());
        platform.on_screens(&[stream(1, 5, 1920, 1080, true)], &[]);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Screens))));
        let decoder_side = sh.fresh_latest(1);
        platform.on_screens(&[stream(0, 5, 1920, 1080, true)], &[]);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Screens))));
        assert_eq!(sh.primary(), 0);
        assert!(Arc::ptr_eq(&decoder_side, &sh.latest_for(0)), "la file du flux suit son écran");
        // Plusieurs écrans : rien ne bouge d'un écran à l'autre.
        platform.on_screens(&[stream(0, 5, 1920, 1080, true), stream(1, 7, 1920, 1080, false)], &[]);
        assert!(Arc::ptr_eq(&decoder_side, &sh.latest_for(0)));
    }
}
