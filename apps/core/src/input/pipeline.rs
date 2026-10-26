//! Entrées vers l'ordinateur (chaîne kynput) et ce qui en revient : le
//! pointeur de l'ordinateur, les vibrations des manettes, le presse-papiers.
//! Assemble les autres modules du dossier au-dessus de `kynput_rs`.
//!
//! La chaîne kynput produit déjà certaines entrées sur Mac et Windows
//! (manettes via SDL, presse-papiers sur Windows) ; l'app produit les autres
//! depuis les événements de sa plateforme et les donne ici. Ailleurs (Apple
//! TV, iPhone, iPad, Android, Linux, webOS), kynput ne produit rien lui-même :
//! manettes, clavier, souris et texte viennent de l'app
//! (`Options::gamepads_from_app`).
//!
//! Tout ce que l'app produit passe par [`PacketQueue`] : envoi à kynput,
//! refus comptés, file de secours pour les paquets qui ne se rattrapent pas.
//!
//! Fils : toutes les méthodes sont appelables de tout fil. Les minuteries
//! (réessais, axes regroupés, combinaison, presse-papiers) tournent dans
//! [`InputPipeline::poll`], que la plateforme appelle elle-même ou confie au
//! fil créé par [`InputPipeline::spawn_driver`]. Les rappels de kynput
//! (pointeur, vibrations) arrivent sur le fil réseau de kynput : le pointeur
//! va à [`PointerLayer`] (qui prévient la vue), les vibrations à
//! [`InputPlatform::rumble`] ; les paquets du presse-papiers sont copiés et
//! traités par `poll`, hors du verrou du routeur (la réponse y réentrerait).
//!
//! Multi-écrans ([`InputPipeline::set_screens`]) : une position absolue est
//! donnée avec l'index de l'écran de l'appareil ([`InputPipeline::mouse_on`])
//! et part avec l'identifiant d'écran de Kyber correspondant ; le pointeur
//! annoncé par l'ordinateur va à la couche de pointeur de l'écran dont il
//! porte l'identifiant. Clavier, souris relative et manettes ne changent pas.

use std::collections::{HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use kynput_rs::{
    self, Config, ConsumerInterface, ControlEvent, Cursor, CursorData, CursorFormat, GamepadControl, HandlerConsumerInterface, HandlerInterface, InputTarget, InputType,
    InputTypeMetadata, KeyboardKey, MouseButton, MouseButtonType, MouseMove, MousePosition, MouseWheel, Packet, Pipeline, RouterProcessingState, Unicode,
};
use kynput_sys as sys;

use super::clipboard::{ClipboardBridge, Pasteboard};
use super::clock::Clock;
use super::packet::{HOST, KynputSink, OwnedPacket, PacketSink};
use super::packet_queue::{Hooks, PacketQueue};
use super::gamepad_gate::{GamepadGate, GateHook};
use super::pad_combo::{PadAxis, PadButton, PadCombo, PadId, PadKey};
use super::pointer_layer::{PointerLayer, RemoteCursor, Shape};
use super::release_all::{Release, ReleaseAll};

/// Vibration demandée par le jeu pour une manette (moteurs de 0 à 65535,
/// durée en millisecondes, -1 : jusqu'à nouvel ordre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamepadRumble {
    pub id: PadId,
    pub large_motor: u16,
    pub small_motor: u16,
    pub duration_ms: i32,
}

/// Rappels vers l'interface de l'app. Les fils sont dits à chaque méthode :
/// la plateforme repasse sur son fil principal si elle en a besoin.
pub trait InputPlatform: Send + Sync {
    /// Vibration des manettes produites par l'app (kynput ne les joue pas
    /// lui-même). Sur le fil réseau de kynput.
    fn rumble(&self, _rumble: GamepadRumble) {}
    /// Combinaison L1 + R1 + Start + Select formée pour cette manette (sur le
    /// fil qui appelle `poll`). La suite est au contrôleur de session :
    /// prendre la manette, ou la rendre.
    fn combo_formed(&self, _pad: PadId) {}
    /// Les entrées rejoignent l'ordinateur (début de session, reconnexion) :
    /// une app qui produit ses manettes les y réannonce. Sur le fil de
    /// `network_connected`.
    fn connected(&self) {}
    /// Touche de navigation d'une manette SDL prise par l'app (barre ouverte,
    /// voir [`InputPipeline::install_gamepad_gate`]). Sur le fil de kynput,
    /// sous le verrou de son routeur : ne rien y envoyer.
    fn navigate(&self, _key: PadKey) {}
}

pub struct Options {
    /// Écran de l'ordinateur affiché (`KyberDisplay`) et sa taille en pixels
    /// au départ de l'image (Apple TV : vitesse du pointeur rapportée à l'écran).
    pub display_id: u32,
    pub host_size: (u32, u32),
    /// Presse-papiers partagé ; `None` : désactivé dans les réglages, plateforme
    /// sans presse-papiers (tvOS), ou kynput s'en charge lui-même (Windows).
    pub clipboard: Option<Arc<dyn Pasteboard>>,
    /// Manettes produites par l'app (Apple TV, iPhone, iPad : GameController ;
    /// Android, Linux, webOS) : types annoncés à kynput, vibrations reçues.
    /// `false` : kynput les lit via SDL (Mac, Windows) et joue les vibrations.
    pub gamepads_from_app: bool,
}

/// Multi-écrans : un écran de l'ordinateur pris par la session
/// ([`crate::session::StreamScreen`]), vu des entrées.
#[derive(Clone)]
pub struct ScreenTarget {
    /// Index de l'écran de l'appareil (celui de [`InputPipeline::mouse_on`]).
    pub screen: usize,
    /// Identifiant d'écran de Kyber (`StreamScreen::display.id`).
    pub display_id: u32,
    /// Taille de l'écran de l'ordinateur en pixels (repère des positions).
    pub host_size: (u32, u32),
    /// Couche de pointeur de la vue de cet écran ; `None` : celle donnée à
    /// [`InputPipeline::new`] (l'écran principal).
    pub pointer: Option<Arc<PointerLayer>>,
}

/// Dernière valeur acceptée et valeurs en attente, par manette et axe.
#[derive(Default)]
struct Axes {
    last: HashMap<PadId, HashMap<PadAxis, i16>>,
    pending: HashMap<PadId, HashMap<PadAxis, i16>>,
    flush_at: Option<Duration>,
}

/// Réveil du fil des entrées.
struct Driver {
    woken: Mutex<bool>,
    cvar: Condvar,
}

pub struct InputPipeline {
    kynput: Pipeline,
    display_id: u32,
    host_size: (u32, u32),
    clock: Arc<dyn Clock>,
    platform: Arc<dyn InputPlatform>,
    packets: PacketQueue,
    pointer: Arc<PointerLayer>,
    clipboard: Option<ClipboardBridge>,
    combo: PadCombo,
    held: Mutex<ReleaseAll>,
    axes: Mutex<Axes>,
    relative: AtomicBool,
    stopped: AtomicBool,
    /// Paquets du presse-papiers reçus, à traiter hors du routeur.
    received: Mutex<VecDeque<OwnedPacket>>,
    driver: Arc<Driver>,
    /// Manettes SDL lues par kynput (Mac) : combinaison et prise.
    gate: OnceLock<GamepadGate>,
    /// Multi-écrans : les écrans de la session (vide : un écran, `display_id`).
    screens: Mutex<Vec<ScreenTarget>>,
}

/// Les axes d'une tranche de 4 ms partent ensemble.
pub const AXIS_SLICE: Duration = Duration::from_millis(4);

impl InputPipeline {
    /// `pointer` : la couche de pointeur de la vue de session, qui reçoit les
    /// annonces de l'ordinateur ; elle survit aux relances de l'image.
    pub fn new(options: Options, pointer: Arc<PointerLayer>, platform: Arc<dyn InputPlatform>, clock: Arc<dyn Clock>) -> Arc<Self> {
        let kynput = Pipeline::new(Config::default_client());
        let sink = Arc::new(KynputSink::of(&kynput));
        Self::build(kynput, sink, options, pointer, platform, clock)
    }

    /// Même chose avec un envoi substitué (tests, bancs d'essai : un
    /// [`PacketSink`] qui note ou refuse ce qu'on lui donne).
    pub fn new_with_sink(options: Options, pointer: Arc<PointerLayer>, platform: Arc<dyn InputPlatform>, clock: Arc<dyn Clock>, sink: Arc<dyn PacketSink>) -> Arc<Self> {
        let kynput = Pipeline::new(Config::default_client());
        Self::build(kynput, sink, options, pointer, platform, clock)
    }

    fn build(kynput: Pipeline, sink: Arc<dyn PacketSink>, options: Options, pointer: Arc<PointerLayer>, platform: Arc<dyn InputPlatform>, clock: Arc<dyn Clock>) -> Arc<Self> {
        let packets = PacketQueue::new(sink, clock.clone());
        packets.set_hooks(Hooks {
            on_first: Some(Box::new(|kind, sent| log::info!("entrées : premier envoi {kind} ({})", if sent { "parti" } else { "refusé" }))),
            on_dropped: Some(Box::new(|kind, final_, total| {
                if final_ {
                    log::error!("entrées : {kind} perdu après 200 ms de réessais ({total} perdus)");
                } else {
                    log::info!("entrées : {kind} refusé par la file d'envoi ({total} perdus)");
                }
            })),
        });
        let clipboard = options.clipboard.map(|board| ClipboardBridge::new(board, clock.clone()));
        let combo = PadCombo::new(clock.clone());
        let this = Arc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            combo.set_on_formed(move |id| {
                if let Some(p) = w.upgrade() {
                    log::info!("combinaison L1 + R1 + Start + Select (manette {id})");
                    p.platform.combo_formed(id);
                }
            });
            Self {
                kynput,
                display_id: options.display_id,
                host_size: options.host_size,
                clock,
                platform,
                packets,
                pointer,
                clipboard,
                combo,
                held: Mutex::new(ReleaseAll::new()),
                axes: Mutex::new(Axes::default()),
                relative: AtomicBool::new(false),
                stopped: AtomicBool::new(false),
                received: Mutex::new(VecDeque::new()),
                driver: Arc::new(Driver {
                    woken: Mutex::new(false),
                    cvar: Condvar::new(),
                }),
                gate: OnceLock::new(),
                screens: Mutex::new(Vec::new()),
            }
        });
        let weak = Arc::downgrade(&this);

        // Pointeur de l'ordinateur.
        this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(CursorHandler {
            pipeline: weak.clone(),
            last_shapes: Mutex::new(HashMap::new()),
        })));

        // Presse-papiers : reçoit les paquets de l'ordinateur, produit les nôtres.
        if this.clipboard.is_some() {
            this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(ClipboardHandler { pipeline: weak.clone() })));
        }

        // Vibrations : l'ordinateur les envoie, la plateforme les joue.
        if options.gamepads_from_app {
            this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(RumbleHandler { pipeline: weak.clone() })));
        }

        // Entrées que la chaîne ne produit pas elle-même : déclarées ici,
        // envoyées par l'app.
        let mut wanted = vec![InputType::Keyboard, InputType::MousePosition, InputType::MouseMove, InputType::MouseButton, InputType::MouseWheel];
        if options.gamepads_from_app {
            wanted.extend([InputType::GamepadControl, InputType::GamepadEvent, InputType::Unicode]);
        }
        let missing: Vec<InputType> = wanted.iter().copied().filter(|t| !this.kynput.produces_type(*t)).collect();
        log::info!(
            "entrées : {} types produits par kynput, {} par l'app (écran {})",
            wanted.len() - missing.len(),
            missing.len(),
            options.display_id
        );
        if !missing.is_empty() {
            this.plug(kynput_rs::create_handler_instance(Arc::new(Producer { types: missing })));
        }

        // La couche de pointeur envoie ses positions par nous.
        let w = weak.clone();
        this.pointer.attach((f64::from(options.host_size.0), f64::from(options.host_size.1)), move |x, y| {
            if let Some(p) = w.upgrade() {
                p.mouse(x, y);
            }
        });

        // Souris absolue au départ : le pointeur de l'ordinateur suit celui de l'appareil.
        this.set_relative(false);
        this
    }

    fn plug(&self, handler: kynput_rs::Handler) {
        if let Err(e) = self.kynput.plug_handler(handler) {
            log::warn!("kynput : gestionnaire refusé ({e:?})");
        }
    }

    /// La chaîne kynput, à donner à la session (`Platform::input_pipeline`).
    pub fn kynput(&self) -> Pipeline {
        self.kynput.clone()
    }

    pub fn display_id(&self) -> u32 {
        self.display_id
    }

    /// Taille de l'écran de l'ordinateur au départ de l'image, en pixels.
    pub fn host_size(&self) -> (u32, u32) {
        self.host_size
    }

    pub fn pointer(&self) -> &Arc<PointerLayer> {
        &self.pointer
    }

    /// Multi-écrans : les écrans de la session, reçus de
    /// [`crate::session::Platform::on_screens`] (à chaque démarrage de
    /// l'image, et quand la taille de l'un d'eux change). Chaque couche de
    /// pointeur donnée est reliée à son écran (taille, envoi de la position
    /// tenue) ; le pointeur de l'ordinateur ira à celle de l'écran qu'il
    /// annonce. Une liste vide revient à un écran.
    pub fn set_screens(self: &Arc<Self>, screens: Vec<ScreenTarget>) {
        for target in &screens {
            let Some(layer) = &target.pointer else { continue };
            if Arc::ptr_eq(layer, &self.pointer) {
                continue;
            }
            let w = Arc::downgrade(self);
            let screen = target.screen;
            layer.attach((f64::from(target.host_size.0), f64::from(target.host_size.1)), move |x, y| {
                if let Some(p) = w.upgrade() {
                    p.mouse_on(screen, x, y);
                }
            });
        }
        log::info!(
            "entrées : {} écran(s) ({})",
            screens.len().max(1),
            screens.iter().map(|s| format!("{} → {}", s.screen, s.display_id)).collect::<Vec<_>>().join(", ")
        );
        *self.screens.lock().unwrap() = screens;
    }

    /// Le pointeur de l'ordinateur pour la souris Auto, sur tous les écrans
    /// de cette chaîne (couche principale et celle de chaque écran ; voir
    /// [`PointerLayer::summary`]).
    pub fn pointer_summary(&self) -> super::pointer_layer::PointerSummary {
        let mut layers: Vec<Arc<PointerLayer>> = vec![self.pointer.clone()];
        for t in self.screens.lock().unwrap().iter() {
            if let Some(p) = &t.pointer
                && !layers.iter().any(|l| Arc::ptr_eq(l, p))
            {
                layers.push(p.clone());
            }
        }
        PointerLayer::summary(layers.iter().map(|l| l.as_ref()))
    }

    pub fn screens(&self) -> Vec<ScreenTarget> {
        self.screens.lock().unwrap().clone()
    }

    /// Identifiant d'écran de Kyber de l'écran `screen` de l'appareil.
    pub fn screen_display_id(&self, screen: usize) -> Option<u32> {
        self.screens.lock().unwrap().iter().find(|s| s.screen == screen).map(|s| s.display_id)
    }

    /// La couche de pointeur qui reçoit le pointeur annoncé sur cet écran de
    /// Kyber : celle de son écran ; sans multi-écrans, la couche principale ;
    /// `None` pour un écran que la session n'affiche pas.
    fn pointer_for(&self, display_id: u32) -> Option<Arc<PointerLayer>> {
        let screens = self.screens.lock().unwrap();
        // Un écran (session à un écran, avec ou sans écran virtuel) : tout va
        // à la couche principale, comme avant le multi-écrans, même un
        // pointeur annoncé sur un autre écran de l'ordinateur.
        if screens.len() <= 1 {
            return Some(screens.first().and_then(|t| t.pointer.clone()).unwrap_or_else(|| self.pointer.clone()));
        }
        let target = screens.iter().find(|s| s.display_id == display_id)?;
        Some(target.pointer.clone().unwrap_or_else(|| self.pointer.clone()))
    }

    /// Combinaison et touches tenues de chaque manette (l'app y passe ses
    /// touches de manette ; le hook des manettes SDL aussi, sur Mac).
    pub fn combo(&self) -> &PadCombo {
        &self.combo
    }

    /// Ce que l'appareil tient sur l'ordinateur par nous (touches, clics).
    pub fn held(&self) -> &Mutex<ReleaseAll> {
        &self.held
    }

    /// Manettes lues par kynput via SDL (le Mac l'installe ; Windows non) :
    /// chaque paquet de manette passe par la combinaison L1 + R1 + Start + Select, et la
    /// manette peut être prise par l'app ([`Self::set_gamepad_captured`]).
    /// Une fois par chaîne ; sans effet avec `gamepads_from_app`.
    pub fn install_gamepad_gate(self: &Arc<Self>) {
        if self.gate.set(GamepadGate::default()).is_err() {
            return;
        }
        let hook = GateHook {
            pipeline: Arc::downgrade(self),
        };
        if let Err(e) = self.kynput.set_hook(Box::new(hook)) {
            log::warn!("kynput : hook des manettes refusé ({e:?})");
        }
    }

    pub(crate) fn gamepad_gate(&self) -> Option<&GamepadGate> {
        self.gate.get()
    }

    pub(crate) fn navigate(&self, key: PadKey) {
        self.platform.navigate(key);
    }

    /// L'app prend les manettes SDL (barre ouverte) ou les rend (barre
    /// fermée). À la prise, tout est remis à zéro sur l'ordinateur : touches
    /// tenues relâchées (les trois premières de la combinaison, une gâchette)
    /// et les six axes au repos, pour chaque manette connue ; kynput ne dit
    /// pas l'état des axes. Sans effet sans [`Self::install_gamepad_gate`].
    pub fn set_gamepad_captured(&self, captured: bool) {
        let Some(gate) = self.gate.get() else { return };
        if !gate.set_captured(captured) {
            return;
        }
        for id in self.combo.pads() {
            for button in self.combo.pressed(id) {
                self.reliably(Self::button_packet(id, button, false), "manette");
            }
            for axis in PadAxis::ALL {
                gate.note_axis(id, axis.to_u32(), 0);
                self.reliably(Self::axis_packet(id, axis, 0), "axe de manette");
            }
        }
    }

    /// Souris relative (jeux) : seuls les déplacements partent.
    pub fn set_relative(&self, on: bool) {
        self.relative.store(on, Ordering::Relaxed);
        let _ = self.kynput.set_type_processing_state(InputType::MouseMove, RouterProcessingState::from_boolean(on));
    }

    /// Souris relative active : lisible de n'importe quel fil (Apple TV : la
    /// souris Bluetooth envoie ses déplacements bruts depuis la file des
    /// entrées, sans passer par le fil principal).
    pub fn is_relative(&self) -> bool {
        self.relative.load(Ordering::Relaxed)
    }

    /// Paquets refusés par kynput, jamais arrivés.
    pub fn dropped_packets(&self) -> usize {
        self.packets.dropped()
    }

    /// Fin de session : plus rien ne part, l'ordinateur relâche tout.
    pub fn stop(&self) {
        if let Some(c) = &self.clipboard {
            c.stop();
        }
        {
            let mut axes = self.axes.lock().unwrap();
            axes.pending.clear();
            axes.flush_at = None;
        }
        let dropped = self.packets.stop();
        if dropped > 0 {
            log::info!("entrées : {dropped} paquets perdus pendant la session (file d'envoi pleine ou réseau absent)");
        }
        let _ = self.kynput.flush_keys();
        self.stopped.store(true, Ordering::Relaxed);
        self.wake();
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    /// Arrête les gestionnaires de kynput (manettes SDL) ; sans effet s'ils le sont déjà.
    pub fn shutdown(&self) {
        let _ = self.kynput.stop();
    }

    /// À appeler une fois la chaîne branchée au réseau.
    pub fn start_clipboard(self: &Arc<Self>) {
        let Some(clipboard) = &self.clipboard else { return };
        let weak = Arc::downgrade(self);
        clipboard.set_send(move |packet| {
            if let Some(p) = weak.upgrade() {
                p.packets.send(packet, "presse-papiers");
            }
        });
        clipboard.start();
        self.wake();
    }

    /// Appelé quand les entrées rejoignent l'ordinateur (début de session,
    /// reconnexion).
    pub fn network_connected(&self) {
        log::info!("entrées reliées à l'ordinateur");
        self.platform.connected();
    }

    // Envoi.

    /// Paquet qui se rattrape tout seul au suivant : envoyé une fois, perdu
    /// s'il est refusé (voir `PacketQueue`).
    fn once(&self, packet: Option<OwnedPacket>, kind: &'static str) -> bool {
        match packet {
            Some(p) => self.packets.send(p, kind),
            None => false,
        }
    }

    /// Paquet qui ne se rattrape pas : réessayé jusqu'à succès ou 200 ms.
    fn reliably(&self, packet: Option<OwnedPacket>, kind: &'static str) {
        if let Some(p) = packet {
            self.packets.send_reliably(p, kind);
            if self.packets.has_backlog() {
                self.wake();
            }
        }
    }

    pub fn mouse_move(&self, dx: i16, dy: i16) {
        self.once(OwnedPacket::from_packet(MouseMove { dx, dy }.to_packet(InputTarget::Host)), "déplacement relatif");
    }

    /// Touche par son scancode (Windows : tel quel ; Apple : `keymap::scancode`).
    /// Notée dans [`Self::held`] pour « tout relâcher ».
    pub fn key(&self, scancode: u16, pressed: bool) {
        self.held.lock().unwrap().key(scancode, pressed);
        self.reliably(OwnedPacket::from_packet(KeyboardKey { pressed, scancode }.to_packet(InputTarget::Host)), "touche");
    }

    /// Position absolue sur l'écran de l'ordinateur affiché (multi-écrans :
    /// l'écran principal).
    pub fn mouse(&self, x: i16, y: i16) {
        self.mouse_at(self.display_id, x, y);
    }

    /// Multi-écrans : position absolue sur l'écran `screen` de l'appareil,
    /// en pixels de l'écran de l'ordinateur qui lui correspond. Écran
    /// inconnu (pas de multi-écrans, Station à un écran) : l'écran principal.
    pub fn mouse_on(&self, screen: usize, x: i16, y: i16) {
        let display_id = self.screen_display_id(screen).unwrap_or(self.display_id);
        self.mouse_at(display_id, x, y);
    }

    fn mouse_at(&self, display_id: u32, x: i16, y: i16) {
        self.once(OwnedPacket::from_packet(MousePosition { display_id, x, y }.to_packet(InputTarget::Host)), "position");
    }

    pub fn mouse_button(&self, button: MouseButtonType, pressed: bool) {
        self.held.lock().unwrap().mouse_button(button, pressed);
        self.reliably(OwnedPacket::from_packet(MouseButton { button, pressed }.to_packet(InputTarget::Host)), "clic");
    }

    pub fn wheel(&self, dx: f32, dy: f32) {
        self.once(OwnedPacket::from_packet(MouseWheel { dx, dy }.to_packet(InputTarget::Host)), "molette");
    }

    /// L'ordinateur relâche tout ce qu'il tient de nous : touches, boutons de
    /// souris, boutons et axes des manettes. Même file que les autres paquets,
    /// donc réessayé s'il est refusé.
    pub fn flush_keys(&self) {
        self.reliably(OwnedPacket::from_packet(ControlEvent::FlushKeys.to_packet(InputTarget::Host)), "relâchement général");
    }

    /// Relâche sur l'ordinateur les touches et clics que nous y tenons (la
    /// fenêtre perd le clavier), sans toucher aux manettes.
    pub fn release_all(&self) {
        let mut releases = Vec::new();
        self.held.lock().unwrap().release(|r| releases.push(r));
        for r in releases {
            match r {
                Release::MouseButton(button) => self.reliably(OwnedPacket::from_packet(MouseButton { button, pressed: false }.to_packet(InputTarget::Host)), "clic"),
                Release::Key { scancode } => self.reliably(OwnedPacket::from_packet(KeyboardKey { pressed: false, scancode }.to_packet(InputTarget::Host)), "touche"),
            }
        }
    }

    /// Texte tapé (clavier de l'écran, de l'iPhone) : un caractère à la fois.
    pub fn type_text(&self, text: &str) {
        for character in text.chars() {
            let packet = Unicode { character }.to_packet(InputTarget::Host).and_then(OwnedPacket::from_packet);
            self.reliably(packet, "texte");
        }
    }

    // Manettes produites par l'app (Apple TV, iPhone, iPad, Android, Linux, webOS).

    pub fn gamepad_plug(&self, id: PadId, vendor_id: Option<u16>, product_id: Option<u16>) {
        self.combo.note(id);
        let packet = GamepadControl::Plug { id, vendor_id, product_id }.to_packet(InputTarget::Host).and_then(OwnedPacket::from_packet);
        self.reliably(packet, "manette branchée");
    }

    pub fn gamepad_unplug(&self, id: PadId) {
        {
            let mut axes = self.axes.lock().unwrap();
            axes.last.remove(&id);
            axes.pending.remove(&id);
        }
        self.combo.forget(id);
        let packet = GamepadControl::Unplug { id }.to_packet(InputTarget::Host).and_then(OwnedPacket::from_packet);
        self.reliably(packet, "manette débranchée");
    }

    /// Touche de manette : envoyée tout de suite, et passée à la combinaison.
    pub fn gamepad_button(&self, id: PadId, button: PadButton, pressed: bool) {
        self.combo.button(id, button, pressed);
        if self.combo.next_deadline().is_some() {
            self.wake();
        }
        self.reliably(Self::button_packet(id, button, pressed), "manette");
    }

    fn button_packet(id: PadId, button: PadButton, pressed: bool) -> Option<OwnedPacket> {
        let b = sys::kynput_GamepadEventButton {
            id,
            type_: button.to_u32(),
            pressed,
        };
        unsafe { OwnedPacket::from_raw(sys::kynput_gamepad_button_to_packet(HOST, &b)) }
    }

    fn axis_packet(id: PadId, axis: PadAxis, value: i16) -> Option<OwnedPacket> {
        let a = sys::kynput_GamepadEventAxis {
            id,
            type_: axis.to_u32(),
            value,
        };
        unsafe { OwnedPacket::from_raw(sys::kynput_gamepad_axis_to_packet(HOST, &a)) }
    }

    // Axes : un axe ne part que s'il a bougé (au 1/512 de la course), et les
    // axes d'une tranche de 4 ms partent ensemble : GameController signale X et
    // Y d'un stick même si un seul a changé, et une DualSense en Bluetooth
    // produit jusqu'à ~1 500 paquets par seconde, de quoi remplir la file
    // d'envoi de kynput (64 paquets, jetés sans retour) et perdre des
    // relâchements. Un axe en mouvement se rattrape au suivant ; son retour au
    // repos, non : il part par la file de secours. Une valeur refusée (file
    // pleine, ou un retour au repos qui attend encore devant elle) reste en
    // attente de la tranche suivante. Les touches partent tout de suite, elles.

    /// Changement au 1/512 de la course ; un retour exact au repos passe toujours.
    pub fn axis_moved(last: Option<i16>, value: i16) -> bool {
        match last {
            None => true,
            Some(last) if value == 0 => last != 0,
            Some(last) => (last >> 7) != (value >> 7),
        }
    }

    /// Valeur d'un axe produit par l'app : filtrée, regroupée, envoyée (voir ci-dessus).
    pub fn gamepad_axis(&self, id: PadId, axis: PadAxis, value: i16) {
        let mut axes = self.axes.lock().unwrap();
        let last = axes.last.get(&id).and_then(|m| m.get(&axis)).copied();
        if !Self::axis_moved(last, value) {
            return;
        }
        axes.last.entry(id).or_default().insert(axis, value);
        axes.pending.entry(id).or_default().insert(axis, value);
        if axes.flush_at.is_none() {
            axes.flush_at = Some(self.clock.now() + AXIS_SLICE);
            drop(axes);
            self.wake();
        }
    }

    /// Les six axes d'une manette au repos, tout de suite et sûrement (l'app
    /// prend la manette, arrière-plan, fin de session) ; ce qui attendait est oublié.
    pub fn gamepad_axes_rest(&self, id: PadId) {
        {
            let mut axes = self.axes.lock().unwrap();
            axes.pending.remove(&id);
            let last = axes.last.entry(id).or_default();
            for axis in PadAxis::ALL {
                last.insert(axis, 0);
            }
        }
        for axis in PadAxis::ALL {
            self.send_axis(id, axis, 0);
        }
    }

    fn flush_axes(&self) {
        let mut axes = self.axes.lock().unwrap();
        let Some(at) = axes.flush_at else { return };
        let now = self.clock.now();
        if now < at {
            return;
        }
        axes.flush_at = None;
        let mut pending = std::mem::take(&mut axes.pending);
        for (id, values) in pending.iter_mut() {
            values.retain(|axis, value| !self.send_axis(*id, *axis, *value));
        }
        pending.retain(|_, values| !values.is_empty());
        if !pending.is_empty() {
            axes.flush_at = Some(now + AXIS_SLICE);
        }
        // Ce qui est arrivé pendant l'envoi s'ajoute à ce qui reste.
        for (id, values) in axes.pending.drain().collect::<Vec<_>>() {
            pending.entry(id).or_default().extend(values);
        }
        axes.pending = pending;
    }

    /// `false` : la valeur n'est pas partie (à représenter).
    fn send_axis(&self, id: PadId, axis: PadAxis, value: i16) -> bool {
        if value == 0 {
            self.reliably(Self::axis_packet(id, axis, 0), "axe de manette");
            return true;
        }
        self.once(Self::axis_packet(id, axis, value), "axe de manette")
    }

    // Minuteries.

    /// Fait tout ce qui est dû : paquets du presse-papiers reçus, réessais,
    /// axes regroupés, combinaison, presse-papiers de l'appareil.
    pub fn poll(&self) {
        loop {
            let next = self.received.lock().unwrap().pop_front();
            match next {
                Some(packet) => {
                    if let Some(c) = &self.clipboard {
                        c.received(&packet);
                    }
                }
                None => break,
            }
        }
        self.packets.poll();
        self.flush_axes();
        self.combo.poll();
        if let Some(c) = &self.clipboard {
            c.poll();
        }
    }

    /// Quand appeler [`Self::poll`] au plus tard (`None` : rien n'attend).
    pub fn next_deadline(&self) -> Option<Duration> {
        if !self.received.lock().unwrap().is_empty() {
            return Some(self.clock.now());
        }
        [
            self.packets.next_deadline(),
            self.axes.lock().unwrap().flush_at,
            self.combo.next_deadline(),
            self.clipboard.as_ref().and_then(|c| c.next_deadline()),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Réveille le fil des entrées (quelque chose attend ou vient d'arriver).
    pub fn wake(&self) {
        *self.driver.woken.lock().unwrap() = true;
        self.driver.cvar.notify_one();
    }

    /// Un fil qui appelle [`Self::poll`] à chaque échéance et à chaque réveil,
    /// jusqu'à [`Self::stop`] (ou la disparition de la chaîne). Les apps qui
    /// n'ont pas de file à elles pour cela s'en servent ; l'horloge donnée
    /// doit alors avancer avec le temps réel.
    pub fn spawn_driver(self: &Arc<Self>) -> std::io::Result<JoinHandle<()>> {
        let weak = Arc::downgrade(self);
        let driver = self.driver.clone();
        std::thread::Builder::new().name("kybernow-entrees".into()).spawn(move || {
            loop {
                let Some(p) = weak.upgrade() else { break };
                if p.is_stopped() {
                    break;
                }
                p.poll();
                let wait = p.next_deadline().map(|at| at.saturating_sub(p.clock.now()));
                drop(p);
                let mut woken = driver.woken.lock().unwrap();
                if !*woken {
                    woken = match wait {
                        Some(d) => driver.cvar.wait_timeout(woken, d).unwrap().0,
                        None => driver.cvar.wait(woken).unwrap(),
                    };
                }
                *woken = false;
            }
        })
    }
}

impl Drop for InputPipeline {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        *self.driver.woken.lock().unwrap() = true;
        self.driver.cvar.notify_all();
    }
}

// Gestionnaires kynput.

struct Producer {
    types: Vec<InputType>,
}

impl HandlerInterface for Producer {
    fn start(&self) {}
    fn stop(&self) {}
    fn produced_types(&self) -> Option<Vec<InputType>> {
        Some(self.types.clone())
    }
}

struct CursorHandler {
    pipeline: Weak<InputPipeline>,
    /// Empreinte de la dernière forme reçue, par écran de l'ordinateur : un
    /// simple déplacement renvoie la même image (kyber-patches serveur 0006),
    /// inutile de la refaire ; un pointeur qui passe sur un autre écran y
    /// arrive avec sa forme, que la couche de cet écran n'a pas encore.
    last_shapes: Mutex<HashMap<u32, Option<u64>>>,
}

impl HandlerInterface for CursorHandler {
    fn start(&self) {}
    fn stop(&self) {}
    fn query_type_metadata(&self, t: InputType) -> Option<InputTypeMetadata> {
        (t == InputType::Cursor).then_some(InputTypeMetadata::Cursor { format: CursorFormat::Raw })
    }
}

impl ConsumerInterface for CursorHandler {
    fn consume(&self, packet: Packet) {
        let Some(cursor) = Cursor::from_packet(&packet) else { return };
        let Some(pipeline) = self.pipeline.upgrade() else { return };
        let display_id = cursor.display_id;
        let Some(layer) = pipeline.pointer_for(display_id) else { return };
        let remote = {
            let mut shapes = self.last_shapes.lock().unwrap();
            decode_cursor(cursor, shapes.entry(display_id).or_default())
        };
        layer.remote_cursor_changed(remote);
    }
}

impl HandlerConsumerInterface for CursorHandler {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Cursor]
    }
}

/// Forme ARGB du pointeur (32 bits, non prémultipliée) telle que kynput la
/// livre, dédoublonnée : `None` quand elle n'a pas changé, ou qu'elle est
/// entièrement transparente (lecteurs vidéo, certains jeux : le Mac
/// garderait un pointeur invisible ; on garde la forme précédente).
pub(crate) fn decode_cursor(c: Cursor, last_shape: &mut Option<u64>) -> RemoteCursor {
    let position = (c.x, c.y);
    let visible = c.visible;
    let (width, height) = (c.width, c.height);
    let CursorData::Raw(raw) = c.data else {
        return RemoteCursor { visible, image: None, position };
    };
    if width == 0 || height == 0 || raw.len() < (width * height) as usize {
        return RemoteCursor { visible, image: None, position };
    }
    let pixels = &raw[..(width * height) as usize];
    let mut hasher = DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    pixels.hash(&mut hasher);
    let shape = hasher.finish();
    if *last_shape == Some(shape) {
        return RemoteCursor { visible, image: None, position };
    }
    *last_shape = Some(shape);
    if pixels.iter().all(|p| p >> 24 == 0) {
        return RemoteCursor { visible, image: None, position };
    }
    RemoteCursor {
        visible,
        image: Some(Shape {
            width,
            height,
            argb: pixels.into(),
            hot_x: c.xhot,
            hot_y: c.yhot,
        }),
        position,
    }
}

struct ClipboardHandler {
    pipeline: Weak<InputPipeline>,
}

impl HandlerInterface for ClipboardHandler {
    fn start(&self) {}
    fn stop(&self) {}
    fn produced_types(&self) -> Option<Vec<InputType>> {
        Some(vec![InputType::Clipboard])
    }
}

impl ConsumerInterface for ClipboardHandler {
    /// Le routeur appelle sous son verrou : le paquet est copié et traité par
    /// `poll`, d'où la réponse repart sans réentrer dans le routeur.
    fn consume(&self, packet: Packet) {
        let Some(pipeline) = self.pipeline.upgrade() else { return };
        let copy = unsafe { OwnedPacket::from_raw(sys::kynput_packet_clone(packet.to_ptr())) };
        if let Some(copy) = copy {
            pipeline.received.lock().unwrap().push_back(copy);
            pipeline.wake();
        }
    }
}

impl HandlerConsumerInterface for ClipboardHandler {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Clipboard]
    }
}

struct RumbleHandler {
    pipeline: Weak<InputPipeline>,
}

impl HandlerInterface for RumbleHandler {
    fn start(&self) {}
    fn stop(&self) {}
}

impl ConsumerInterface for RumbleHandler {
    fn consume(&self, packet: Packet) {
        let mut r = sys::kynput_GamepadEventRumble {
            id: 0,
            large_motor: 0,
            small_motor: 0,
            duration: 0,
        };
        if !unsafe { sys::kynput_gamepad_rumble_from_packet(packet.to_ptr(), &mut r) } {
            return;
        }
        let Some(pipeline) = self.pipeline.upgrade() else { return };
        pipeline.platform.rumble(GamepadRumble {
            id: r.id,
            large_motor: r.large_motor,
            small_motor: r.small_motor,
            duration_ms: i32::from(r.duration),
        });
    }
}

impl HandlerConsumerInterface for RumbleHandler {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::GamepadEvent]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;
    use crate::input::packet_queue::tests::Host;

    struct Quiet;
    impl InputPlatform for Quiet {}

    struct Bench {
        host: Arc<Host>,
        clock: Arc<ManualClock>,
        pipeline: Arc<InputPipeline>,
    }

    impl Bench {
        fn new() -> Self {
            let host = Arc::new(Host::default());
            let clock = Arc::new(ManualClock::new());
            let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
            let pipeline = InputPipeline::new_with_sink(
                Options {
                    display_id: 1,
                    host_size: (1920, 1080),
                    clipboard: None,
                    gamepads_from_app: true,
                },
                pointer,
                Arc::new(Quiet),
                clock.clone(),
                host.clone(),
            );
            Self { host, clock, pipeline }
        }

        fn elapse(&self, ms: u64) {
            for _ in 0..ms {
                self.clock.advance(Duration::from_millis(1));
                self.pipeline.poll();
            }
        }
    }

    #[test]
    fn filtrage_des_axes() {
        assert!(InputPipeline::axis_moved(None, 0) && InputPipeline::axis_moved(None, 5), "première valeur : passe toujours");
        assert!(!InputPipeline::axis_moved(Some(1000), 1020) && InputPipeline::axis_moved(Some(1000), 1200), "même tranche de 1/512 écartée, tranche changée gardée");
        assert!(InputPipeline::axis_moved(Some(3), 0) && !InputPipeline::axis_moved(Some(0), 0), "retour exact au repos : passe toujours, sauf s'il y était déjà");
        assert!(InputPipeline::axis_moved(Some(-200), 200) && !InputPipeline::axis_moved(Some(32000), 32100), "signe changé gardé, bruit en butée écarté");
    }

    #[test]
    fn axes_regroupes_par_4_ms() {
        let b = Bench::new();
        b.pipeline.gamepad_axis(1, PadAxis::LeftX, 1000);
        b.pipeline.gamepad_axis(1, PadAxis::LeftY, -1000);
        b.pipeline.gamepad_axis(1, PadAxis::LeftX, 1010);
        assert!(b.host.received().is_empty(), "rien ne part avant la tranche");
        assert_eq!(b.pipeline.next_deadline(), Some(AXIS_SLICE));
        b.elapse(4);
        let mut got = b.host.received();
        got.sort();
        let axis = |a: PadAxis, v: i16| format!("axe 1 {} {v}", a.to_u32());
        assert_eq!(got, [axis(PadAxis::LeftX, 1000), axis(PadAxis::LeftY, -1000)], "les deux axes partent ensemble, la valeur dans la même tranche est écartée");
        b.host.clear();
        b.pipeline.gamepad_axis(1, PadAxis::LeftX, 0);
        b.elapse(4);
        assert_eq!(b.host.received(), [axis(PadAxis::LeftX, 0)], "retour au repos envoyé");
        // Refusé : la valeur attend la tranche suivante.
        b.host.clear();
        b.host.refuse(1);
        b.pipeline.gamepad_axis(1, PadAxis::RightX, 5000);
        b.elapse(4);
        assert!(b.host.received().is_empty(), "refusé");
        b.elapse(4);
        assert_eq!(b.host.received(), [axis(PadAxis::RightX, 5000)], "représenté à la tranche suivante");
    }

    #[test]
    fn remise_au_repos_et_touches() {
        let b = Bench::new();
        b.pipeline.gamepad_axis(2, PadAxis::LeftTrigger, 3000);
        b.pipeline.gamepad_axes_rest(2);
        assert_eq!(b.host.received().len(), 6, "six axes au repos, tout de suite");
        assert!(b.host.received().iter().all(|r| r.ends_with(" 0")));
        b.elapse(4);
        assert_eq!(b.host.received().len(), 6, "ce qui attendait est oublié");
        b.host.clear();
        b.pipeline.gamepad_button(2, PadButton::A, true);
        assert_eq!(b.host.received(), ["manette 2 A appuyée"], "les touches partent tout de suite");
        assert_eq!(b.pipeline.combo().pressed(2).len(), 1, "et passent par la combinaison");
        b.pipeline.gamepad_plug(3, Some(0x054C), Some(0x0CE6));
        assert!(b.pipeline.combo().pads().contains(&3));
        b.pipeline.gamepad_unplug(3);
        assert!(!b.pipeline.combo().pads().contains(&3));
    }

    #[test]
    fn tout_relacher_et_texte() {
        let b = Bench::new();
        b.pipeline.key(0x1E, true);
        b.pipeline.mouse_button(MouseButtonType::Left, true);
        b.pipeline.key(0x1E, false);
        b.pipeline.key(0x1D, true);
        b.host.clear();
        b.pipeline.release_all();
        let mut got = b.host.received();
        got.sort();
        assert_eq!(got, ["clic Left relâché", "touche 0x1d relâchée"]);
        assert!(b.pipeline.held().lock().unwrap().scancodes().is_empty());
        b.host.clear();
        b.pipeline.type_text("é!");
        assert_eq!(b.host.received(), ["texte é", "texte !"]);
        b.pipeline.flush_keys();
        assert_eq!(b.host.received().last().unwrap(), "relâchement général");
        b.pipeline.stop();
        b.pipeline.key(0x1E, true);
        assert_eq!(b.host.received().len(), 3, "arrêté : plus rien ne part");
    }

    #[test]
    fn positions_par_ecran() {
        let b = Bench::new();
        // Sans multi-écrans : l'écran de la session, quel que soit l'index.
        b.pipeline.mouse_on(2, 10, 0);
        b.pipeline.mouse(11, 0);
        assert_eq!(b.host.received(), ["position 10", "position 11"]);
        b.host.clear();
        let second = Arc::new(PointerLayer::new(b.clock.clone(), true));
        b.pipeline.set_screens(vec![
            ScreenTarget { screen: 1, display_id: 1, host_size: (1920, 1080), pointer: None },
            ScreenTarget { screen: 0, display_id: 7, host_size: (3456, 2234), pointer: Some(second.clone()) },
        ]);
        b.pipeline.mouse_on(0, 20, 0);
        b.pipeline.mouse_on(1, 30, 0);
        b.pipeline.mouse_on(5, 40, 0);
        assert_eq!(b.host.received(), ["position 20 sur 7", "position 30", "position 40"], "écran inconnu : le principal");
        assert_eq!(b.pipeline.screen_display_id(0), Some(7));
        // La couche du second écran envoie sa position tenue sur son écran, à sa taille.
        b.host.clear();
        assert_eq!(second.host_size(), (3456.0, 2234.0));
        second.move_by(5.0, 5.0);
        assert_eq!(b.host.received(), ["position 1733 sur 7"]);
        // Pointeur de l'ordinateur : à la couche de l'écran annoncé.
        assert!(Arc::ptr_eq(&b.pipeline.pointer_for(7).unwrap(), &second));
        assert!(Arc::ptr_eq(&b.pipeline.pointer_for(1).unwrap(), b.pipeline.pointer()));
        assert!(b.pipeline.pointer_for(3).is_none(), "écran que la session n'affiche pas");
        b.pipeline.set_screens(Vec::new());
        assert!(Arc::ptr_eq(&b.pipeline.pointer_for(3).unwrap(), b.pipeline.pointer()), "un écran : tout à la couche principale");
        // Session à un écran (une seule cible) : un pointeur annoncé sur un autre écran de
        // l'ordinateur (sans écran virtuel) va à la couche principale, comme avant.
        b.pipeline.set_screens(vec![ScreenTarget { screen: 0, display_id: 1, host_size: (1920, 1080), pointer: None }]);
        assert!(Arc::ptr_eq(&b.pipeline.pointer_for(5).unwrap(), b.pipeline.pointer()));
    }

    #[test]
    fn pointeur_dedoublonne() {
        let cursor = |pixels: Vec<u32>, x: i32| Cursor {
            display_id: 1,
            visible: true,
            x,
            y: 0,
            xhot: 1,
            yhot: 1,
            width: 2,
            height: 1,
            data: CursorData::Raw(pixels),
        };
        let mut last = None;
        let r = decode_cursor(cursor(vec![0xFF00_0000, 0], 1), &mut last);
        assert!(r.image.is_some() && r.position == (1, 0));
        let r = decode_cursor(cursor(vec![0xFF00_0000, 0], 2), &mut last);
        assert!(r.image.is_none(), "même forme : seule la position change");
        let r = decode_cursor(cursor(vec![0, 0], 3), &mut last);
        assert!(r.image.is_none(), "forme entièrement transparente : gardée");
        let r = decode_cursor(cursor(vec![0, 0xFF00_0000], 4), &mut last);
        assert_eq!(r.image.map(|s| (s.hot_x, s.width)), Some((1, 2)));
    }
}
