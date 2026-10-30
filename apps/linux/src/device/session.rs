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
use kybernow_core::relay::{RelayInfo, RelayRoute, RouteInfo, RouteTransport};
use kybernow_core::screens::Screens;
use kybernow_core::session::{Notice, Platform, Recovery, Session, SessionParams, SessionRole, Status, StreamScreen};
use kybernow_core::share::{self, DecoderCaps, ShareEnd, ShareState};
use kybernow_core::{Display, WindowHandle};

use super::media::AudioSink;
use super::video::{Latest, VideoSink};
use super::{sdl, Event, SessionEvent};
use crate::app::{Machine, PathKind, RouteUi, SessionId, ShareGuest, ShareRelay, ShareView};
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

    // KyberNow Share. Hôte : la liste des invités arrive avec le signe de vie.
    fn on_guests(&self, state: Option<&ShareState>) {
        self.0.send(SessionEvent::Guests(state.and_then(share_view)));
    }

    /// Invité : son rôle au départ (`previous` nul), puis chaque changement de l'hôte. Les entrées le
    /// suivent déjà (le cœur) : l'interface montre l'avis et ce que le rôle ouvre.
    fn on_share_role(&self, role: share::ShareRole, previous: Option<share::ShareRole>) {
        self.0.send(SessionEvent::ShareRole { role: role_of(role), previous: previous.map(role_of) });
    }

    /// Invité : c'est le partage qui finit la session ; le texte est aussi la raison de la fin
    /// (`Status::Ended`), que la fenêtre montre : ici, seulement le journal.
    fn on_share_end(&self, end: &ShareEnd, text: &str) {
        self.0.send(SessionEvent::ShareEnd(end.kind().to_string(), text.to_string()));
    }

    /// Invité : l'image qu'il reçoit, au départ puis à chaque changement (signe de vie, réponse à un changement de
    /// qualité). La barre et le menu en montrent la qualité (`label`, nul tant que Kyber n'en sait pas la taille) ;
    /// `notice` est l'avis bref de la Station, dans la langue de l'app (« Image allégée pour suivre. »).
    fn on_share_image(&self, image: &share::ShareImage, _previous: Option<&share::ShareImage>, notice: Option<&str>) {
        self.0.send(SessionEvent::ShareImage { label: image.label(), notice: notice.map(str::to_string) });
    }

    /// Par où passe la session, au départ puis quand le chemin change (par le relais : repli TCP, allers-retours).
    /// La barre en fait sa puce « Par le relais », les statistiques leur ligne « Chemin ».
    fn on_route(&self, route: &RouteInfo) {
        self.0.send(SessionEvent::Route(route_of(route)));
    }

    /// Par le relais, la session finit parce que l'ordinateur n'est pas joignable hors de chez soi : l'erreur typée
    /// (accès coupé dans Admin, éteint ou en veille, relais muet), juste avant `Status::Ended`. C'est elle qui choisit
    /// le texte de la fin (`relay_failure_text`), jamais le texte d'Admin.
    fn on_relay_error(&self, error: &kybernow_core::relay::RelayError, _text: &str) {
        self.0.send(SessionEvent::RelayError(error.clone()));
    }

    /// Prêt de l'ordinateur, hôte : la vue du prêt du signe de vie, quand elle change (le panneau « Prêter
    /// l'ordinateur… », les demandes et leur avis).
    fn on_lend(&self, lend: Option<&kybernow_core::lend::LendView>) {
        self.0.send(SessionEvent::Lend(lend.map(super::jobs::lend_view)));
    }

    /// Emprunt : le préavis de 2 minutes (« {hôte} reprend l'ordinateur dans 2 min : sauvegardez votre partie. »).
    fn on_lend_notice(&self, notice: &kybernow_core::lend::LendNotice, text: &str) {
        self.0.send(SessionEvent::LendNotice { text: text.to_string(), at: notice.at });
    }

    /// Emprunt : le prêt finit la session, juste avant sa fin (le texte est aussi la raison de `Status::Ended`).
    fn on_lend_end(&self, _end: kybernow_core::lend::LendEnd, text: &str) {
        self.0.send(SessionEvent::LendEnd(text.to_string()));
    }
}

/// Le chemin de l'interface d'après celui du cœur.
pub fn route_of(r: &RouteInfo) -> RouteUi {
    let kind = match r.transport {
        RouteTransport::Direct => PathKind::Direct,
        RouteTransport::Relay => PathKind::Relay,
        RouteTransport::RelayTcp => PathKind::RelayTcp,
    };
    RouteUi { kind, rtt_client: r.rtt_client, rtt_station: r.rtt_station }
}

/// La route d'un ordinateur par son relais : celui que sa Station a donné (`info.relay`, gardé avec lui), Admin
/// épinglé sur l'empreinte que l'app connaît ; `None` sans relais chez lui ou sans empreinte (jamais de premier
/// contact par le relais). La session l'essaie quand le réseau ne répond pas (`Route::Auto`).
pub fn relay_route(host: &Host) -> Option<RelayRoute> {
    let r = host.relay.as_ref()?;
    RelayInfo { server: r.server.clone(), id: r.id.clone(), remote_access: r.remote_access }.route(host.fingerprint.as_deref()?)
}

/// Les relais où chercher le code d'un partage en plus du relais public : ceux des ordinateurs de l'appareil
/// (`info.relay.server`), sans doublon ; une adresse illisible est écartée. Le cœur (`share::join_anywhere`)
/// interroge toujours le relais public lui-même : le code d'un ami y part même quand l'ordinateur de l'utilisateur
/// en a un autre. `KYBERNOW_RELAY=off` : aucun relais, et `jobs::share_join` n'appelle alors que le réseau.
pub fn join_relays<'a>(servers: impl IntoIterator<Item = &'a str>) -> Vec<kybernow_core::relay::RelayAddr> {
    use kybernow_core::relay::{RelayAddr, RelayMode};
    if RelayMode::current() == RelayMode::Off {
        return Vec::new();
    }
    let mut out = Vec::new();
    for server in servers {
        if let Ok(addr) = server.trim().parse::<RelayAddr>()
            && !out.contains(&addr)
        {
            out.push(addr);
        }
    }
    out
}

/// Le texte de link-ui (« Hors de chez vous ») d'un échec de session par le relais, avec le nom de l'ordinateur :
/// « {nom} n'est pas joignable hors de chez vous (accès coupé dans Admin). » quand Admin refuse l'accès,
/// « {nom} n'est pas joignable : l'ordinateur est éteint ou en veille. » quand le relais ne le connaît pas,
/// « Le relais ne répond pas. » quand le relais est muet.
///
/// `relay` est l'erreur typée que le cœur a donnée en fin de session (`Platform::on_relay_error`) : c'est elle qui
/// choisit le texte, jamais le texte d'Admin. `error` est la raison de la fin donnée par le cœur ; sans erreur du
/// relais, elle reste telle quelle.
pub fn relay_failure_text(relay: Option<&kybernow_core::relay::RelayError>, error: Option<String>, name: &str) -> Option<String> {
    match (relay, error) {
        (Some(e), Some(_)) => Some(e.text(kybernow_core::i18n::current(), Some(name))),
        (_, error) => error,
    }
}

/// Le rôle de l'interface d'après celui du cœur.
pub fn role_of(r: share::ShareRole) -> crate::app::ShareRole {
    match r {
        share::ShareRole::Spectator => crate::app::ShareRole::Spectator,
        share::ShareRole::Player => crate::app::ShareRole::Player,
        share::ShareRole::Kbm => crate::app::ShareRole::Kbm,
        share::ShareRole::PlayerKbm => crate::app::ShareRole::PlayerKbm,
    }
}

/// Le rôle du cœur d'après celui de l'interface (les deux boutons de droits de l'hôte).
pub fn core_role(r: crate::app::ShareRole) -> share::ShareRole {
    match r {
        crate::app::ShareRole::Spectator => share::ShareRole::Spectator,
        crate::app::ShareRole::Player => share::ShareRole::Player,
        crate::app::ShareRole::Kbm => share::ShareRole::Kbm,
        crate::app::ShareRole::PlayerKbm => share::ShareRole::PlayerKbm,
    }
}

/// La « Qualité de l'image » du cœur d'après celle de l'interface (le choix de « Rejoindre » et de la barre).
pub fn core_quality(q: crate::app::ShareQuality) -> share::ShareQuality {
    match q {
        crate::app::ShareQuality::Auto => share::ShareQuality::Auto,
        crate::app::ShareQuality::Host => share::ShareQuality::Host,
        crate::app::ShareQuality::P2160_60 => share::ShareQuality::P2160_60,
        crate::app::ShareQuality::P1080_60 => share::ShareQuality::P1080_60,
        crate::app::ShareQuality::P720_30 => share::ShareQuality::P720_30,
    }
}

/// La qualité de l'image d'un invité telle que le panneau de l'hôte la montre : seulement quand c'est une image à
/// part (pas celle de l'hôte) dont Kyber sait la taille. Une image encore inconnue compte comme celle de l'hôte.
fn guest_quality(g: &share::ShareGuest) -> Option<String> {
    g.image.as_ref().filter(|i| i.own()).and_then(|i| i.label())
}

// L'attente du lien du relais de l'interface est celle du cœur.
const _: () = assert!(crate::app::RELAY_LINK_WAIT.as_secs() == share::RELAY_LINK_WAIT.as_secs());

/// Le relais d'un partage ou d'un code de prêt (`relay.state` d'Admin) tel que l'interface le montre.
pub fn relay_of(r: Option<share::ShareRelay>) -> Option<ShareRelay> {
    r.map(|r| match r.state {
        share::ShareRelayState::Joined => ShareRelay::Joined,
        share::ShareRelayState::Connecting => ShareRelay::Connecting,
        share::ShareRelayState::Unreachable => ShareRelay::Unreachable,
    })
}

/// Le partage ouvert d'une session d'hôte tel que l'interface le montre (`None` : fermé).
pub fn share_view(st: &ShareState) -> Option<ShareView> {
    st.open.then(|| ShareView {
        code: st.code.clone(),
        link: st.link.clone(),
        web: st.web.clone(),
        relay: relay_of(st.relay),
        // Posé par l'app quand elle reçoit le partage (`ShareView::keep_relay_since`).
        relay_since: None,
        guests: st
            .guests
            .iter()
            .map(|g| ShareGuest { id: g.id.clone(), name: g.name.clone(), role: role_of(g.role), app: g.app.clone(), since: g.since, quality: guest_quality(g), pending: g.pending, via_relay: g.via == share::ShareVia::Relay })
            .collect(),
    })
}

/// Ce que cet appareil décode, pour lire l'image d'un partage (le format de l'hôte qu'il ne lit pas ne le refuse
/// plus : il reçoit une image qu'il lit, `share::guest_format`) : le HEVC par la carte ou le processeur, le 4:4:4
/// avec lui, l'AV1 par la carte.
pub fn guest_capabilities(m: &Machine) -> Capabilities {
    Capabilities { hevc: m.decodes("hevc"), hevc444: m.decodes("hevc"), av1: m.decodes("av1") }
}

/// Ce que le décodeur de cet appareil suit, pour « Auto » (`auto_quality`) : ses formats et, quand on les connaît,
/// la définition et la fréquence au-delà desquelles il décroche (0 : rien de connu au-delà de l'écran). Le Raspberry
/// Pi : ce que sa sortie HDMI affiche (4K sur les Pi 4 et 5, 1080p avant) à 60 Hz au plus ; sans décodeur matériel
/// (le processeur décode) : 1080p, 60 Hz. Ailleurs, la carte graphique suit l'écran.
pub fn decoder_caps(m: &Machine) -> DecoderCaps {
    let capabilities = guest_capabilities(m);
    let (max_width, max_height, max_fps) = if let Some(pi) = &m.pi {
        let height = pi.max_height();
        (height * 16 / 9, height, 60)
    } else if !m.hw_any() {
        (1920, 1080, 60)
    } else {
        (0, 0, 0)
    };
    DecoderCaps { capabilities, max_width, max_height, max_fps }
}

/// L'écran de la fenêtre, pour « Auto » : définition native (en paysage) et fréquence (`auto_quality`).
pub fn auto_screen(pixel: (u32, u32), refresh: u32) -> kybernow_core::screens::DeviceScreen {
    kybernow_core::screens::DeviceScreen { x: 0.0, y: 0.0, width: f64::from(pixel.0), height: f64::from(pixel.1), pixel_width: pixel.0, pixel_height: pixel.1, refresh, primary: true }
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
/// celui d'une session à un écran ; `mode` est celui de cet écran. `guest` : la session d'invité d'un
/// partage (KyberNow Share) : l'écran et l'image sont ceux de l'hôte, jamais de presse-papiers, aucun écran
/// de plus ; `host` n'est alors qu'une enveloppe (nom, adresse, réglages par défaut), absente du magasin.
#[allow(clippy::too_many_arguments)]
pub fn start(id: SessionId, host: &Host, settings: StreamSettings, mode: (u32, u32, u32), m: &Machine, relative: bool, device: DeviceInfo, screens: Option<Screens>, current: usize, guest: Option<&share::Found>, loan: Option<&kybernow_core::lend::Loan>, tx: Sender<Event>) -> Handle {
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
        // Ni l'invité d'un partage ni l'emprunteur n'ont de presse-papiers.
        clipboard: (host.settings.clipboard && guest.is_none() && loan.is_none()).then(|| Arc::new(Clipboard::new())),
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
    let recovery = Recovery { watch_image: true, fallback_bitrate: Some(20_000_000), ..Recovery::default() };
    let params = match (guest, loan) {
        // Un invité : la Station qui partage, le jeton de son partage, l'écran de l'hôte (`SessionParams::guest`).
        (Some(found), _) => SessionParams { auto_reconnect: host.settings.reconnect, gamepad_xbox360: host.settings.xbox360, recovery, ..SessionParams::guest(found, device, settings, guest_capabilities(m), audio_channels) },
        // Un emprunt : la Station de l'ami, le jeton du prêt renouvelé par son signe de vie, l'écran virtuel fait à la
        // taille de cet appareil (`SessionParams::borrower`) ; clavier, souris et manettes, jamais le presse-papiers.
        (None, Some(loan)) => SessionParams { gamepad_xbox360: host.settings.xbox360, recovery, ..SessionParams::borrower(loan, device, settings, audio_channels) },
        (None, None) => SessionParams {
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
            recovery,
            role: SessionRole::Host,
            // Un relais connu pour cet ordinateur : l'image passe par le réseau, et par lui quand le réseau ne répond pas.
            route: kybernow_core::session::Route::Auto(relay_route(host)),
        },
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
            relay: None,
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

    /// KyberNow Share : ce que le cœur dit d'un partage arrive à la boucle, étiqueté par sa session : la liste des invités
    /// de l'hôte (fermé : rien), le rôle d'un invité (au départ puis changé), la fin par le partage ; les rôles passent
    /// de l'un à l'autre sans rien perdre.
    #[test]
    fn share_callbacks_reach_the_loop() {
        let (sh, rx) = shared(0);
        let platform = LinuxPlatform(sh);
        // Invité : le rôle de départ, puis celui que l'hôte donne.
        platform.on_share_role(share::ShareRole::Spectator, None);
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareRole { role, previous })) => assert_eq!((role, previous), (crate::app::ShareRole::Spectator, None)),
            _ => panic!("le rôle de départ arrive avec sa session"),
        }
        platform.on_share_role(share::ShareRole::PlayerKbm, Some(share::ShareRole::Player));
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareRole { role, previous })) => assert_eq!((role, previous), (crate::app::ShareRole::PlayerKbm, Some(crate::app::ShareRole::Player))),
            _ => panic!("le rôle changé arrive avec le précédent"),
        }
        // Hôte : la liste des invités, telle que l'interface la montre ; sans partage ouvert, rien.
        let guest = share::ShareGuest { id: "g1".into(), name: "Léa".into(), app: "ios".into(), role: share::ShareRole::Player, since: Some(1_761_500_000), ..share::ShareGuest::default() };
        let state = ShareState { open: true, code: "482917".into(), link: "kybernow://join?x".into(), web: "https://h:1/link/?join=482917".into(), display: None, guests: vec![guest], ..ShareState::default() };
        platform.on_guests(Some(&state));
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::Guests(Some(v)))) => {
                assert_eq!((v.code.as_str(), v.link.as_str(), v.web.as_str()), ("482917", "kybernow://join?x", "https://h:1/link/?join=482917"));
                assert_eq!(v.guests, vec![ShareGuest { id: "g1".into(), name: "Léa".into(), role: crate::app::ShareRole::Player, app: "ios".into(), since: Some(1_761_500_000), ..ShareGuest::default() }]);
                assert_eq!(v.relay, None, "pas de relais réglé");
            }
            _ => panic!("les invités arrivent avec leur session"),
        }
        platform.on_guests(Some(&ShareState { open: false, ..state.clone() }));
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Guests(None)))), "un partage fermé n'est pas un partage");
        platform.on_guests(None);
        assert!(matches!(rx.try_recv(), Ok(Event::Session(7, SessionEvent::Guests(None)))));
        // Invité : le partage finit la session, avec le texte du cœur.
        platform.on_share_end(&share::ShareEnd::Removed, "Léa vous a retiré du partage.");
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareEnd(kind, text))) => assert_eq!((kind.as_str(), text.as_str()), ("removed", "Léa vous a retiré du partage.")),
            _ => panic!("la fin arrive avec son texte"),
        }
        // Invité : l'image reçue arrive avec sa qualité (nulle tant que Kyber n'en sait pas la taille) et l'avis de la Station.
        let image = share::ShareImage { quality: share::ShareQuality::Auto, width: Some(1920), height: Some(1080), fps: Some(60), follows_host: Some(true), ..share::ShareImage::default() };
        platform.on_share_image(&image, None, None);
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareImage { label, notice })) => assert_eq!((label.as_deref(), notice), (Some("1080p · 60 Hz"), None)),
            _ => panic!("l'image reçue arrive avec sa qualité"),
        }
        let lighter = share::ShareImage { width: Some(1280), height: Some(720), fps: Some(30), follows_host: Some(false), reason: Some(share::ShareImageReason::Lag), ..image.clone() };
        platform.on_share_image(&lighter, Some(&image), Some("Image allégée pour suivre."));
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareImage { label, notice })) => assert_eq!((label.as_deref(), notice.as_deref()), (Some("720p · 30 Hz"), Some("Image allégée pour suivre."))),
            _ => panic!("l'avis arrive avec l'image"),
        }
        platform.on_share_image(&share::ShareImage::default(), None, None);
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::ShareImage { label, notice })) => assert_eq!((label, notice), (None, None), "taille inconnue : pas de qualité à montrer"),
            _ => panic!("une image encore inconnue arrive aussi"),
        }
        // Hôte : la qualité d'un invité n'est dite que pour une image à part dont la taille est connue ; une image
        // inconnue ou celle de l'hôte : rien.
        let guest_with = |image: Option<share::ShareImage>| share::ShareGuest { id: "g".into(), name: "Tom".into(), image, ..share::ShareGuest::default() };
        assert_eq!(guest_quality(&guest_with(Some(lighter.clone()))).as_deref(), Some("720p · 30 Hz"));
        assert_eq!(guest_quality(&guest_with(Some(image.clone()))), None, "l'image de l'hôte");
        assert_eq!(guest_quality(&guest_with(Some(share::ShareImage { follows_host: None, ..lighter.clone() }))), None, "pas encore su : compté comme celui de l'hôte");
        assert_eq!(guest_quality(&guest_with(Some(share::ShareImage { width: None, ..lighter }))), None, "taille inconnue");
        assert_eq!(guest_quality(&guest_with(None)), None, "Station sans l'image");
        // Les qualités de l'interface et celles du cœur se correspondent, avec les mêmes libellés.
        for q in crate::app::ShareQuality::ALL {
            let c = core_quality(q);
            assert_eq!(c.as_str(), q.as_str());
            crate::i18n::force_english(false);
            assert_eq!(c.label(kybernow_core::i18n::Lang::Fr), q.label());
        }
        // Les rôles de l'interface et ceux du cœur se correspondent.
        for r in [share::ShareRole::Spectator, share::ShareRole::Player, share::ShareRole::Kbm, share::ShareRole::PlayerKbm] {
            assert_eq!(core_role(role_of(r)), r);
            let ui = role_of(r);
            assert_eq!((ui.pads(), ui.kbm(), ui.plays()), (r.rights().gamepads, r.rights().keyboard_mouse, r.plays()));
            assert_eq!((ui.pads(), ui.kbm()), (r.pads(), r.kbm()));
            for on in [false, true] {
                assert_eq!(core_role(ui.with_pads(on)), r.with_pads(on));
                assert_eq!(core_role(ui.with_kbm(on)), r.with_kbm(on));
            }
        }
    }

    /// Ce que cet appareil décode pour lire l'image d'un hôte : le HEVC (et son 4:4:4) par la carte ou le processeur,
    /// l'AV1 par la carte seulement. Le format de l'hôte qu'il ne lit pas ne le refuse plus : il reçoit une image qu'il
    /// lit (`share::guest_format`), le HEVC avant l'AV1 avant le H.264.
    #[test]
    fn a_guest_reads_what_the_device_decodes() {
        use kybernow_core::profile::Codec;
        let mut m = Machine::default();
        let caps = guest_capabilities(&m);
        assert!(caps.hevc && caps.hevc444 && !caps.av1);
        assert_eq!(share::guest_format(Some("hevc"), true, caps).unwrap(), Some((Codec::Hevc, true)));
        // L'AV1 de l'hôte sans décodeur de carte : plus de refus, une image HEVC.
        assert_eq!(share::guest_format(Some("av1"), false, caps).unwrap(), Some((Codec::Hevc, false)));
        m.hw_av1 = true;
        assert_eq!(share::guest_format(Some("av1"), false, guest_capabilities(&m)).unwrap(), Some((Codec::Av1, false)));
        // Image de l'hôte pas encore démarrée : rien à vérifier.
        assert_eq!(share::guest_format(None, false, caps).unwrap(), None);
    }

    /// « Auto » : ce que le décodeur suit. Une carte graphique suit l'écran (aucune borne) ; le Raspberry Pi, ce que sa
    /// sortie HDMI affiche à 60 Hz au plus (4K sur les Pi 4 et 5, 1080p avant) ; sans aucun décodeur matériel, 1080p à
    /// 60 Hz. L'écran de la fenêtre se met en paysage, et la mesure rend la plus petite des deux bornes.
    #[test]
    fn auto_is_what_the_screen_and_the_decoder_follow() {
        let m = Machine::default();
        let none = decoder_caps(&m);
        assert_eq!((none.max_width, none.max_height, none.max_fps), (0, 0, 0));
        assert_eq!(share::auto_quality(&auto_screen((3840, 2160), 120), &none, None), share::ShareAuto { width: 3840, height: 2160, fps: 120, codecs: vec!["h264".into(), "hevc".into()], yuv444: true });
        // Le Deck tenu en portrait ou en paysage : toujours en paysage.
        let deck = share::auto_quality(&auto_screen((800, 1280), 60), &none, None);
        assert_eq!((deck.width, deck.height, deck.fps), (1280, 800, 60));
        // Le Pi 3, sur un écran 4K : 1080p ; le Pi 5 : 4K à 60 Hz au plus.
        for (generation, height) in [(3u8, 1080u32), (4, 2160), (5, 2160)] {
            let pi = Machine { pi: Some(crate::app::Pi { generation, ..Default::default() }), ..Machine::default() };
            let a = share::auto_quality(&auto_screen((3840, 2160), 120), &decoder_caps(&pi), None);
            assert_eq!((a.height, a.width, a.fps), (height, height * 16 / 9, 60), "Pi {generation}");
        }
        // Un décodeur logiciel seulement : 1080p à 60 Hz sur un écran 1440p à 144 Hz.
        let cpu = Machine { hw_h264: false, hw_hevc: false, hw_av1: false, ..Machine::default() };
        let a = share::auto_quality(&auto_screen((2560, 1440), 144), &decoder_caps(&cpu), None);
        assert_eq!((a.width, a.height, a.fps), (1920, 1080, 60));
        // L'AV1 par la carte : annoncé.
        let av1 = Machine { hw_av1: true, ..Machine::default() };
        assert!(share::auto_quality(&auto_screen((1920, 1080), 60), &decoder_caps(&av1), None).codecs.contains(&"av1".to_string()));
    }

    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

    /// Les relais d'un code tapé, en plus du relais public (que le cœur interroge toujours) : ceux des ordinateurs de
    /// l'appareil, sans doublon, sans port quand c'est le 443.
    #[test]
    fn relays_of_a_typed_code() {
        use kybernow_core::relay::{RelayAddr, RelayMode};
        if RelayMode::current() == RelayMode::Off {
            return;
        }
        assert_eq!(join_relays([]), Vec::<RelayAddr>::new(), "rien à ajouter au relais public");
        let relays = join_relays(["share.kybernow.app", "relais.exemple.org:9443", "relais.exemple.org:9443", "share.kybernow.app:443", "a b", ""]);
        assert_eq!(relays.len(), 2, "le port se complète, les doublons et les adresses illisibles sont écartés");
        assert_eq!((relays[1].host.as_str(), relays[1].port), ("relais.exemple.org", 9443));
        assert_eq!(relays.iter().map(ToString::to_string).collect::<Vec<_>>(), ["share.kybernow.app", "relais.exemple.org:9443"]);
    }

    /// Les refus par le relais disent le nom de l'ordinateur (link-ui), choisis par l'erreur typée du cœur ; sans
    /// erreur du relais, la raison de la fin reste telle quelle, même si elle ressemble à un refus.
    #[test]
    fn refusals_say_the_name_of_the_computer() {
        use kybernow_core::i18n::Lang;
        use kybernow_core::relay::RelayError;
        crate::i18n::force_english(false);
        kybernow_core::i18n::set_current(Lang::Fr);
        let text = |e: RelayError| relay_failure_text(Some(&e), Some("raison du cœur".into()), "Atelier");
        assert_eq!(text(RelayError::AccessOff).as_deref(), Some("Atelier n'est pas joignable hors de chez vous (accès coupé dans Admin)."));
        assert_eq!(text(RelayError::StationOffline).as_deref(), Some("Atelier n'est pas joignable : l'ordinateur est éteint ou en veille."));
        assert_eq!(text(RelayError::Unreachable("x".into())).as_deref(), Some("Le relais ne répond pas."), "relais muet");
        let looks_like = RelayError::StationOffline.text(Lang::Fr, None);
        assert_eq!(relay_failure_text(None, Some(looks_like.clone()), "Atelier"), Some(looks_like), "le texte ne se lit pas");
        assert_eq!(relay_failure_text(None, Some("Mot de passe refusé".into()), "Atelier").as_deref(), Some("Mot de passe refusé"));
        assert_eq!(relay_failure_text(Some(&RelayError::AccessOff), None, "Atelier"), None, "une fin sans erreur n'en a pas");
        // Les textes anglais sont ceux du cœur, avec le nom.
        assert_eq!(RelayError::AccessOff.text(Lang::En, Some("Atelier")), "Atelier cannot be reached away from home (access turned off in Admin).");
        assert_eq!(RelayError::StationOffline.text(Lang::En, Some("Atelier")), "Atelier cannot be reached: the computer is off or asleep.");
    }

    /// Le cœur dit le refus du relais avant la fin de la session : la plateforme le passe à la boucle principale,
    /// avec son type (le code des ponts : 30 relais muet, 31 éteint ou en veille, 32 accès coupé dans Admin).
    #[test]
    fn the_relay_error_reaches_the_main_loop() {
        use kybernow_core::relay::RelayError;
        let (sh, rx) = shared(7);
        LinuxPlatform(sh).on_relay_error(&RelayError::AccessOff, "texte du cœur");
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::RelayError(e))) => assert_eq!((e.clone(), e.code(), e.kind()), (RelayError::AccessOff, 32, "access_off")),
            _ => panic!("l'erreur du relais arrive avec sa session"),
        }
    }

    /// La route d'un ordinateur par son relais : celui que sa Station a donné (`info.relay`), Admin épinglé sur
    /// l'empreinte que l'app connaît ; sans relais chez lui, sans empreinte ou avec un identifiant illisible, il n'y en
    /// a pas (jamais de premier contact par le relais).
    #[test]
    fn relay_route_of_a_computer() {
        let mut h = host(HostSettings::default());
        h.fingerprint = Some("sha256:aa".into());
        assert_eq!(relay_route(&h), None, "pas de relais chez lui");
        h.relay = Some(crate::model::HostRelay { server: "share.kybernow.app".into(), id: STATION.into(), remote_access: true });
        let r = relay_route(&h).expect("route");
        assert_eq!((r.server.as_str(), r.station.as_str(), r.admin_fp.as_str()), ("share.kybernow.app", STATION, "sha256:aa"));
        h.fingerprint = None;
        assert_eq!(relay_route(&h), None, "jamais sans l'empreinte d'Admin");
        h.fingerprint = Some("sha256:aa".into());
        h.relay.as_mut().unwrap().id = "court".into();
        assert_eq!(relay_route(&h), None, "un identifiant illisible");
    }

    /// Le chemin d'une session : le transport du cœur et les allers-retours, pour la puce et la ligne « Chemin ».
    #[test]
    fn route_of_a_session() {
        assert_eq!(route_of(&RouteInfo::direct()), RouteUi::default());
        let r = route_of(&RouteInfo { transport: RouteTransport::RelayTcp, relay: Some("share.kybernow.app".into()), rtt_client: Some(12.5), rtt_station: Some(1.0) });
        assert_eq!((r.kind, r.total_ms()), (PathKind::RelayTcp, Some(13.5)));
        assert!(r.kind.via_relay());
        // Une session par le relais envoie son chemin à la boucle principale.
        let (sh, rx) = shared(7);
        LinuxPlatform(sh).on_route(&RouteInfo { transport: RouteTransport::Relay, ..RouteInfo::default() });
        match rx.try_recv() {
            Ok(Event::Session(7, SessionEvent::Route(r))) => assert_eq!(r.kind, PathKind::Relay),
            _ => panic!("le chemin arrive avec sa session"),
        }
    }

    /// Le partage de l'hôte, avec le relais et les demandes : le relais joint ou injoignable, une demande en attente
    /// venue par le relais (`pending`, `via`).
    #[test]
    fn share_view_with_the_relay_and_requests() {
        let state: ShareState = serde_json::from_str(
            r#"{"open":true,"code":"482917","link":"kybernow://join?x&relay=r&sid=s","web":"https://share.kybernow.app/j/482917","web_local":"https://h:1/link/?join=482917",
            "relay":{"state":"joined"},
            "guests":[{"id":"g1","name":"Léa","app":"ios","role":"spectator","since":1,"pending":false,"via":"lan"},
                      {"id":"g2","name":"Tom","app":"android","role":"spectator","since":2,"pending":true,"via":"relay"}]}"#,
        )
        .unwrap();
        let v = share_view(&state).expect("partage ouvert");
        assert_eq!(v.relay, Some(ShareRelay::Joined));
        assert_eq!(v.requests().iter().map(|(i, g)| (*i, g.name.as_str())).collect::<Vec<_>>(), vec![(1, "Tom")]);
        assert_eq!(v.accepted().iter().map(|(i, g)| (*i, g.name.as_str())).collect::<Vec<_>>(), vec![(0, "Léa")]);
        assert!(v.guests[1].via_relay && !v.guests[0].via_relay);
        let soon = share_view(&ShareState { relay: Some(share::ShareRelay { state: share::ShareRelayState::Connecting }), ..state.clone() }).unwrap();
        assert_eq!((soon.relay, soon.relay_since), (Some(ShareRelay::Connecting), None));
        let down = share_view(&ShareState { relay: Some(share::ShareRelay { state: share::ShareRelayState::Unreachable }), ..state }).unwrap();
        assert_eq!(down.relay, Some(ShareRelay::Unreachable));
    }
}
