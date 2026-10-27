//! Une session vue du téléviseur : ce que webOS fournit au cœur
//! (`session::Platform`) et les rappels vers la boucle principale, comme le
//! pont JNI d'Android (apps/android/jni/src/session.rs).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use kybernow_core::console::{DeviceInfo, Mode};
use kybernow_core::input::{Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, Options, PadId, PadKey, PointerLayer};
use kybernow_core::kynput_rs::Pipeline;
use kybernow_core::latency::Second;
use kybernow_core::media::PacketHandler;
use kybernow_core::profile::{Capabilities, Codec, Profile as CoreProfile, StreamSettings};
use kybernow_core::session::{Notice, Platform, Recovery, Session, SessionParams, SessionRole, Status};
use kybernow_core::share::{self, ShareEnd, ShareState};
use kybernow_core::{Display, WindowHandle};

use super::ndl;
use super::media::{AudioSink, VideoSink};
use super::Event;
use crate::app::{ShareGuest, ShareView};
use crate::model::{Host, Profile};

/// Ce que partagent la session, ses flux et la boucle principale.
pub struct Shared {
    tx: Mutex<Sender<Event>>,
    session: OnceLock<Weak<Session>>,
    pub pointer: Arc<PointerLayer>,
    clock: Arc<MonotonicClock>,
    pub input: Mutex<Option<Arc<InputPipeline>>>,
    display: Mutex<(u32, u32, u32)>,
    mode: Mutex<Mode>,
    refresh: AtomicU32,
    has_audio: bool,
    /// Son 5.1, décodé par l'app.
    surround: bool,
    /// … encodé en E-AC-3.
    dolby: bool,
    pub combo_enabled: AtomicBool,
    codec: Mutex<String>,
}

impl Shared {
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session.get().and_then(Weak::upgrade)
    }

    fn send(&self, e: Event) {
        let _ = self.tx.lock().unwrap().send(e);
    }

    pub fn video_size(&self) -> (u32, u32) {
        let m = self.mode.lock().unwrap();
        if m.width > 0 && m.height > 0 { (m.width, m.height) } else { (1920, 1080) }
    }

    pub fn host_size(&self) -> (u32, u32) {
        let d = self.display.lock().unwrap();
        if d.1 > 0 && d.2 > 0 { (d.1, d.2) } else { self.video_size() }
    }

    pub fn frame_period_micros(&self) -> i64 {
        1_000_000 / self.refresh.load(Ordering::Relaxed).max(24) as i64
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
        self.send(Event::DecoderFailed(e.to_string()));
    }
}

struct WebOsPlatform(Arc<Shared>);

impl Platform for WebOsPlatform {
    fn window_handle(&self) -> WindowHandle {
        // L'image et le son vont à l'app : Kyber ne crée pas de lecteur VLC.
        WindowHandle::Xlib(0)
    }

    fn video_handler(&self) -> Arc<dyn PacketHandler> {
        VideoSink::new(self.0.clone())
    }

    fn audio_handler(&self) -> Option<Arc<dyn PacketHandler>> {
        self.0.has_audio.then(|| AudioSink::new(self.0.surround, self.0.dolby) as Arc<dyn PacketHandler>)
    }

    fn input_pipeline(&self) -> Pipeline {
        let (id, w, h) = *self.0.display.lock().unwrap();
        let callbacks: Arc<dyn InputPlatform> = Arc::new(InputCallbacks(self.0.clone()));
        let clock: Arc<dyn Clock> = self.0.clock.clone();
        let pipeline = InputPipeline::new(
            Options { display_id: id, host_size: (w, h), clipboard: None, gamepads_from_app: true },
            self.0.pointer.clone(),
            callbacks,
            clock,
        );
        pipeline.combo().set_enabled(self.0.combo_enabled.load(Ordering::Relaxed));
        if let Err(e) = pipeline.spawn_driver() {
            log::error!("entrées : fil des minuteries impossible ({e})");
        }
        let kynput = pipeline.kynput();
        if let Some(old) = self.0.input.lock().unwrap().replace(pipeline) {
            old.stop();
            old.wake();
        }
        // Les manettes ne sont annoncées qu'une fois la liaison prête
        // (`connected`) : une seule annonce par liaison (deux annonces du même
        // joueur recréent sa manette virtuelle sur l'ordinateur).
        kynput
    }

    fn on_status(&self, status: &Status) {
        if matches!(status, Status::Ended { .. }) {
            self.0.pointer.reset();
        }
        self.0.send(Event::Status(status.clone()));
    }

    fn on_display(&self, d: &Display) {
        *self.0.display.lock().unwrap() = (d.id, d.width.max(0) as u32, d.height.max(0) as u32);
        self.0.pointer.set_host_size((d.width as f64, d.height as f64));
    }

    fn on_mode(&self, mode: Mode, _virtual_display: bool) {
        *self.0.mode.lock().unwrap() = mode;
        self.0.send(Event::Mode(mode));
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
        }
        self.0.pointer.resend_held();
    }

    fn on_second(&self, s: &Second) {
        self.0.send(Event::Second {
            latency_ms: s.latency_ms,
            fps: s.displayed as f64,
            mbps: s.bytes as f64 * 8.0 / 1e6,
            lost: s.lost_frames + s.holes,
            codec: self.0.codec(),
        });
    }

    fn on_game_fullscreen(&self, game: bool) {
        self.0.send(Event::GameFullscreen(game));
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
        self.0.send(Event::Notice(msg));
    }

    // KyberNow Share. Hôte : la liste des invités arrive avec le signe de vie.
    fn on_guests(&self, state: Option<&ShareState>) {
        self.0.send(Event::Guests(state.and_then(share_view)));
    }

    /// Invité : son rôle au départ (`previous` nul), puis chaque changement de l'hôte. Les entrées le
    /// suivent déjà (le cœur) : l'interface montre l'avis et ce que le rôle ouvre.
    fn on_share_role(&self, role: share::ShareRole, previous: Option<share::ShareRole>) {
        self.0.send(Event::ShareRole { role: role_of(role), previous: previous.map(role_of) });
    }

    /// Invité : c'est le partage qui finit la session ; le texte est aussi la raison de la fin
    /// (`Status::Ended`), que l'écran de fin montre : ici, seulement le journal.
    fn on_share_end(&self, end: &ShareEnd, text: &str) {
        self.0.send(Event::ShareEnd(end.kind().to_string(), text.to_string()));
    }

    /// Invité : l'image qu'il reçoit, au départ puis à chaque changement (signe de vie, réponse à un changement de
    /// qualité). Le menu en montre la qualité (`label`, nul tant que Kyber n'en sait pas la taille) ; `notice` est
    /// l'avis bref de la Station, dans la langue de l'app (« Image allégée pour suivre. »).
    fn on_share_image(&self, image: &share::ShareImage, _previous: Option<&share::ShareImage>, notice: Option<&str>) {
        self.0.send(Event::ShareImage { label: image.label(), notice: notice.map(str::to_string) });
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

/// La « Qualité de l'image » du cœur d'après celle de l'interface (le choix de « Rejoindre » et du menu).
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

/// Le partage ouvert d'une session d'hôte tel que l'interface le montre (`None` : fermé).
pub fn share_view(st: &ShareState) -> Option<ShareView> {
    st.open.then(|| ShareView {
        code: st.code.clone(),
        link: st.link.clone(),
        web: st.web.clone(),
        guests: st.guests.iter().map(|g| ShareGuest { id: g.id.clone(), name: g.name.clone(), role: role_of(g.role), app: g.app.clone(), since: g.since, quality: guest_quality(g) }).collect(),
    })
}

/// « Auto » (KyberNow Share, invité) : ce que ce téléviseur sait afficher et décoder, pour `share::auto_quality` :
/// la sortie du téléviseur (sa dalle en paysage, sa fréquence) et ce que son décodeur suit (HEVC, AV1 s'il l'a,
/// jamais de 4:4:4 ; aucune autre borne connue que la dalle). La Station tranche et borne par l'image de l'hôte.
pub fn auto_quality(tv: &crate::app::TvInfo) -> share::ShareAuto {
    let screen = kybernow_core::screens::DeviceScreen { x: 0.0, y: 0.0, width: f64::from(tv.panel.0), height: f64::from(tv.panel.1), pixel_width: tv.panel.0, pixel_height: tv.panel.1, refresh: tv.refresh, primary: true };
    share::auto_quality(&screen, &share::DecoderCaps::from(guest_capabilities(tv.av1)), None)
}

/// Ce que décode ce téléviseur pour lire l'image d'un partage (le format de l'hôte qu'il ne lit pas ne le refuse
/// plus : il reçoit une image qu'il lit, `share::guest_format`) : le HEVC par le décodeur du téléviseur (jamais en
/// 4:4:4), l'AV1 s'il l'a.
pub fn guest_capabilities(av1: bool) -> Capabilities {
    Capabilities { hevc: true, hevc444: false, av1 }
}

struct InputCallbacks(Arc<Shared>);

impl InputPlatform for InputCallbacks {
    fn rumble(&self, r: GamepadRumble) {
        self.0.send(Event::Rumble(r));
    }

    fn combo_formed(&self, _pad: PadId) {
        self.0.send(Event::Combo);
    }

    /// Les entrées sont reliées à l'ordinateur : les manettes y sont annoncées,
    /// une fois par liaison. Annoncées plus tôt, à la création des entrées,
    /// elles se perdaient quand la liaison n'était pas encore prête (deuxième
    /// session, reconnexion : « aucune manette détectée » dans le jeu).
    fn connected(&self) {
        self.0.send(Event::InputReady);
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

/// Les réglages du flux : le profil ramené à ce que décode le téléviseur
/// (HEVC, pas de 4:4:4), le format et le transport choisis, le débit
/// plafonné par le réseau.
pub fn settings(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), cap_mbps: u32) -> StreamSettings {
    let caps = Capabilities { hevc: true, hevc444: false, av1: codec == "av1" };
    let mut s = StreamSettings::for_profile(core_profile(profile), caps);
    s.codec = match codec {
        "h264" => Codec::H264,
        "av1" => Codec::Av1,
        _ => Codec::Hevc,
    };
    s.yuv444 = false;
    // Transport de l'image : choisi (Rapide, Fiable), sinon celui du profil.
    match host.settings.transport.as_str() {
        "reliable" => s.reliable = true,
        "fast" => s.reliable = false,
        _ => {}
    }
    // Le débit du profil suit les pixels par seconde ; on ramène sa base pour
    // ne pas dépasser le plafond (ou le débit choisi) au mode de la session.
    let (w, h, fps) = mode;
    let wanted = if host.settings.mbps > 0 { host.settings.mbps.min(cap_mbps) } else { cap_mbps };
    let actual = s.bitrate(w, h, fps) / 1_000_000;
    if host.settings.mbps > 0 || actual > wanted {
        let ratio = (w as f64 * h as f64 * fps.max(1) as f64) / (3840.0 * 2160.0 * 60.0);
        let scale = ratio.powf(0.75).clamp(0.4, 3.0);
        s.mbps = ((wanted as f64 / scale).floor() as u32).max(5);
    }
    s
}

/// Démarre une session. `guest` : la session d'invité d'un partage (KyberNow Share), avec ce que décode le
/// téléviseur : l'écran et l'image sont ceux de l'hôte, jamais de réglage de l'ordinateur ; `host` n'est alors
/// qu'une enveloppe (nom, adresse, réglages par défaut), absente du magasin.
#[allow(clippy::too_many_arguments)]
pub fn start(host: &Host, profile: Profile, codec: &str, mode: (u32, u32, u32), refresh: u32, cap_mbps: u32, device: DeviceInfo, guest: Option<(&share::Found, Capabilities)>, tx: Sender<Event>) -> Handle {
    let (w, h, fps) = mode;
    let clock = Arc::new(MonotonicClock::new());
    let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
    // 5.1 : demandé à l'ordinateur et confié au décodeur du téléviseur (NDL),
    // en PCM 6 voies (« surround ») ou en E-AC-3 (« dolby »), vers l'ampli.
    let dolby = host.settings.audio == "dolby";
    let surround = (dolby || host.settings.audio == "surround") && ndl::available();
    let audio_channels = match host.settings.audio.as_str() {
        "computer" => 0,
        _ if surround => 6,
        _ => 2,
    };
    // L'image reste sur la chaîne de jeu de NDL, avec son plan Opus muet qui
    // la cadence (comme en stéréo) ; le Dolby a sa propre chaîne, audio seule.
    ndl::set_audio_plane(if surround && !dolby { ndl::AudioPlane::Pcm51 } else { ndl::AudioPlane::OpusClock });
    let _ = (w, h, codec);
    let shared = Arc::new(Shared {
        tx: Mutex::new(tx),
        session: OnceLock::new(),
        pointer: pointer.clone(),
        clock,
        input: Mutex::new(None),
        display: Mutex::new((0, w, h)),
        mode: Mutex::new(Mode { width: w, height: h, refresh: fps }),
        refresh: AtomicU32::new(refresh),
        has_audio: audio_channels > 0,
        surround,
        dolby: surround && dolby,
        combo_enabled: AtomicBool::new(host.settings.combo),
        codec: Mutex::new(String::new()),
    });
    // Le pointeur de l'ordinateur, dessiné par l'app : position et forme.
    let weak = Arc::downgrade(&shared);
    pointer.set_on_changed(move |state, change| {
        let Some(sh) = weak.upgrade() else { return };
        let shape = if change.shape { state.shape.as_ref().map(|s| (s.width, s.height, s.hot_x, s.hot_y, s.premultiplied())) } else { None };
        sh.send(Event::Pointer { position: state.position(), hidden: state.remote_hidden, shape });
    });
    let recovery = Recovery { watch_image: true, fallback_bitrate: Some((cap_mbps / 2).max(20) * 1_000_000), ..Recovery::default() };
    let stream = settings(host, profile, codec, mode, cap_mbps);
    let params = match guest {
        // Un invité : la Station qui partage, le jeton de son partage, l'écran de l'hôte (`SessionParams::guest`).
        Some((found, caps)) => SessionParams { auto_reconnect: host.settings.reconnect, gamepad_xbox360: host.settings.xbox360, recovery, ..SessionParams::guest(found, device, stream, caps, audio_channels) },
        None => SessionParams {
            console_host: host.address.clone(),
            console_port: host.port,
            user: host.user.clone(),
            password: host.password.clone(),
            console_fingerprint: host.fingerprint.clone(),
            device,
            mode: Mode { width: w, height: h, refresh: fps },
            audio_channels,
            virtual_display: true,
            settings: stream,
            auto_reconnect: host.settings.reconnect,
            gamepad_xbox360: host.settings.xbox360,
            clipboard: false,
            recovery,
            role: SessionRole::Host,
        },
    };
    let session = Session::start(params, Arc::new(WebOsPlatform(shared.clone())));
    let _ = shared.session.set(Arc::downgrade(&session));
    Handle { shared, session }
}
