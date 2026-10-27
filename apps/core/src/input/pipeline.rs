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
//!
//! Plusieurs ordinateurs à la fois ([`InputPipeline::set_active`],
//! docs/plusieurs-ordinateurs.md) : la chaîne d'une session qui n'est pas
//! active ne laisse rien partir de l'appareil (clavier, souris, texte,
//! touches et axes des manettes), relâche ce qu'elle tenait, met son
//! presse-papiers en pause et ignore les vibrations ; les branchements de
//! manettes, les relâchements et les retours au repos passent toujours. Les
//! manettes de Mac et de Windows sont lues par le lecteur unique du cœur
//! ([`crate::gamepads`]) quand libkynput est construite sans le sien.
//!
//! KyberNow Share ([`InputPipeline::set_rights`], docs/kybernow-share.md) :
//! la chaîne d'un invité joueur ne laisse partir que ce que son rôle permet
//! (ses manettes ; le clavier et la souris quand l'hôte les donne ; jamais
//! le presse-papiers). Un droit retiré relâche ce qu'il tenait, comme une
//! session qui passe derrière. La Station l'impose aussi de son côté.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
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
use crate::journal::{self, Label};

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
    /// `false` (Mac, Windows) : kynput les lit via SDL et joue les
    /// vibrations ; ou, avec une libkynput construite sans son lecteur SDL,
    /// le lecteur unique du cœur ([`crate::gamepads`], démarré au besoin).
    pub gamepads_from_app: bool,
}

/// Ce que la chaîne laisse partir vers l'ordinateur (KyberNow Share) : tout
/// pour un hôte ([`Self::ALL`], par défaut) ; pour un invité, ce que son rôle
/// permet ([`crate::share::ShareRole::rights`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputRights {
    /// Touches et axes des manettes, branchements, vibrations reçues.
    pub gamepads: bool,
    /// Clavier, souris (positions, déplacements, clics, molette), texte.
    pub keyboard_mouse: bool,
    /// Presse-papiers partagé.
    pub clipboard: bool,
}

impl InputRights {
    pub const ALL: Self = Self { gamepads: true, keyboard_mouse: true, clipboard: true };
    pub const NONE: Self = Self { gamepads: false, keyboard_mouse: false, clipboard: false };
}

impl Default for InputRights {
    fn default() -> Self {
        Self::ALL
    }
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
    /// Plusieurs ordinateurs : la session est active (par défaut, oui).
    active: AtomicBool,
    /// Souris relative demandée par l'app ([`Self::set_relative`]) ; elle ne
    /// joue que pendant que la session est active.
    relative_wanted: AtomicBool,
    /// Manettes lues par le fil SDL du cœur (Mac, Windows avec une libkynput
    /// sans lecteur SDL).
    core_pads: bool,
    /// Chaîne servie par le lecteur de manettes du cœur (celle d'une session).
    reader_attached: AtomicBool,
    /// Le nom de l'ordinateur au début des lignes du journal.
    label: Arc<Label>,
    /// Passage d'active à inactive, et appuis (touches, clics, manettes) et
    /// axes de l'app : jamais en même temps. Un appui qui a vu la chaîne
    /// active part avant la mise au repos, jamais après.
    switch: Mutex<()>,
    /// La chaîne tient le fil SDL du cœur (à rendre à l'arrêt).
    sdl_held: AtomicBool,
    /// KyberNow Share : les droits de la chaîne ([`InputRights`], tous par défaut).
    kbm_allowed: AtomicBool,
    pads_allowed: AtomicBool,
    clipboard_allowed: AtomicBool,
    /// Les entrées ont rejoint l'ordinateur ([`Self::network_connected`]).
    connected: AtomicBool,
    /// Manettes que l'ordinateur fait vibrer par cette chaîne (arrêtées
    /// quand elle passe derrière).
    rumbling: Mutex<HashSet<PadId>>,
}

/// Les axes d'une tranche de 4 ms partent ensemble.
pub const AXIS_SLICE: Duration = Duration::from_millis(4);

thread_local! {
    /// Chaînes créées pendant [`created_during`] sur ce fil.
    static CREATED: RefCell<Option<Vec<Weak<InputPipeline>>>> = const { RefCell::new(None) };
}

/// Appelle `f` et rend la dernière chaîne d'entrées créée sur ce fil pendant
/// l'appel : celle que la plateforme crée dans `Platform::input_pipeline`
/// (toutes les apps la créent là, sur le fil de session) et dont elle ne
/// rend que la chaîne kynput.
pub(crate) fn created_during<R>(f: impl FnOnce() -> R) -> (R, Option<Arc<InputPipeline>>) {
    let before = CREATED.with(|c| c.borrow_mut().replace(Vec::new()));
    let result = f();
    let created = CREATED.with(|c| std::mem::replace(&mut *c.borrow_mut(), before));
    let last = created.unwrap_or_default().iter().rev().find_map(Weak::upgrade);
    (result, last)
}

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
        // Créée sur le fil de session (`Platform::input_pipeline`) : ses lignes
        // portent le nom de l'ordinateur de la session.
        let label = journal::current().unwrap_or_default();
        let packets = PacketQueue::new(sink, clock.clone());
        let (l1, l2) = (label.clone(), label.clone());
        packets.set_hooks(Hooks {
            on_first: Some(Box::new(move |kind, sent| log::info!("{}entrées : premier envoi {kind} ({})", l1.prefix(), if sent { "parti" } else { "refusé" }))),
            on_dropped: Some(Box::new(move |kind, final_, total| {
                if final_ {
                    log::error!("{}entrées : {kind} perdu après 200 ms de réessais ({total} perdus)", l2.prefix());
                } else {
                    log::info!("{}entrées : {kind} refusé par la file d'envoi ({total} perdus)", l2.prefix());
                }
            })),
        });
        // Mac, Windows : libkynput sans son lecteur SDL, les manettes sont lues
        // par celui du cœur (un seul pour toutes les sessions de l'app).
        let core_pads = !options.gamepads_from_app && crate::gamepads::sdl_compiled() && !kynput.produces_type(InputType::GamepadEvent);
        if core_pads {
            crate::gamepads::hold_sdl();
        }
        let app_pads = options.gamepads_from_app || core_pads;
        let clipboard = options.clipboard.map(|board| ClipboardBridge::new(board, clock.clone()));
        let combo = PadCombo::new(clock.clone());
        let this = Arc::new_cyclic(|weak: &Weak<Self>| {
            let w = weak.clone();
            combo.set_on_formed(move |id| {
                if let Some(p) = w.upgrade() {
                    log::info!("{}combinaison L1 + R1 + Start + Select (manette {id})", p.label.prefix());
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
                active: AtomicBool::new(true),
                relative_wanted: AtomicBool::new(false),
                core_pads,
                reader_attached: AtomicBool::new(false),
                label,
                switch: Mutex::new(()),
                sdl_held: AtomicBool::new(core_pads),
                rumbling: Mutex::new(HashSet::new()),
                kbm_allowed: AtomicBool::new(true),
                pads_allowed: AtomicBool::new(true),
                clipboard_allowed: AtomicBool::new(true),
                connected: AtomicBool::new(false),
            }
        });
        let weak = Arc::downgrade(&this);
        CREATED.with(|c| {
            if let Some(list) = c.borrow_mut().as_mut() {
                list.push(weak.clone());
            }
        });

        // Pointeur de l'ordinateur.
        this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(CursorHandler {
            pipeline: weak.clone(),
            last_shapes: Mutex::new(HashMap::new()),
        })));

        // Presse-papiers : reçoit les paquets de l'ordinateur, produit les nôtres.
        if this.clipboard.is_some() {
            this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(ClipboardHandler { pipeline: weak.clone() })));
        }

        // Vibrations : l'ordinateur les envoie, la plateforme (ou le fil SDL
        // du cœur) les joue.
        if app_pads {
            this.plug(kynput_rs::create_handler_consumer_instance(Arc::new(RumbleHandler { pipeline: weak.clone() })));
        }

        // Entrées que la chaîne ne produit pas elle-même : déclarées ici,
        // envoyées par l'app (ou par le lecteur de manettes du cœur).
        let mut wanted = vec![InputType::Keyboard, InputType::MousePosition, InputType::MouseMove, InputType::MouseButton, InputType::MouseWheel];
        if app_pads {
            wanted.extend([InputType::GamepadControl, InputType::GamepadEvent]);
        }
        if options.gamepads_from_app {
            wanted.push(InputType::Unicode);
        }
        let missing: Vec<InputType> = wanted.iter().copied().filter(|t| !this.kynput.produces_type(*t)).collect();
        log::info!(
            "{}entrées : {} types produits par kynput, {} par l'app (écran {}){}",
            this.label.prefix(),
            wanted.len() - missing.len(),
            missing.len(),
            options.display_id,
            if core_pads { ", manettes lues par le cœur" } else { "" }
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
            log::warn!("{}kynput : gestionnaire refusé ({e:?})", self.label.prefix());
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
            "{}entrées : {} écran(s) ({})",
            self.label.prefix(),
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
            log::warn!("{}kynput : hook des manettes refusé ({e:?})", self.label.prefix());
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

    /// Souris relative (jeux) : seuls les déplacements partent. Session pas
    /// active : retenue, elle reprendra quand la session redeviendra active.
    pub fn set_relative(&self, on: bool) {
        self.relative_wanted.store(on, Ordering::Relaxed);
        self.apply_relative();
    }

    fn apply_relative(&self) {
        let on = self.relative_wanted.load(Ordering::Relaxed) && self.kbm_ok();
        self.relative.store(on, Ordering::Relaxed);
        let _ = self.kynput.set_type_processing_state(InputType::MouseMove, RouterProcessingState::from_boolean(on));
    }

    /// Souris relative active : lisible de n'importe quel fil (Apple TV : la
    /// souris Bluetooth envoie ses déplacements bruts depuis la file des
    /// entrées, sans passer par le fil principal). Faux pendant que la
    /// session n'est pas active.
    pub fn is_relative(&self) -> bool {
        self.relative.load(Ordering::Relaxed)
    }

    // Plusieurs ordinateurs à la fois.

    /// La session de cette chaîne devient active (`true`) ou passe derrière
    /// une autre (`false`), docs/plusieurs-ordinateurs.md. Pas active : plus
    /// rien ne part de l'appareil (clavier, souris, molette, texte, touches et
    /// axes des manettes ; branchements, relâchements et retours au repos
    /// passent toujours), touches et clics tenus relâchés, manettes au repos
    /// (touches relâchées, axes au centre, gâchettes relâchées), souris
    /// relative coupée (reprise au retour), presse-papiers en pause (rien lu
    /// sur l'appareil, rien écrit dessus), vibrations en cours arrêtées puis
    /// ignorées (l'arrêt envoyé ensuite par le jeu le serait aussi). De
    /// nouveau active : le contenu du presse-papiers de l'appareil est
    /// annoncé à l'ordinateur. Par défaut, une chaîne est active. La session
    /// la tient d'un démarrage de l'image à l'autre
    /// ([`crate::session::Session::set_active`]).
    pub fn set_active(&self, active: bool) {
        // Aucun appui ni axe de l'app pendant le passage : ce qui a vu la
        // chaîne active est parti avant la mise au repos.
        let _switch = self.switch.lock().unwrap();
        if self.active.swap(active, Ordering::SeqCst) == active {
            return;
        }
        log::debug!("{}entrées : session {}", self.label.prefix(), if active { "active" } else { "en arrière-plan" });
        if active {
            self.apply_relative();
            if let Some(c) = &self.clipboard
                && self.clipboard_allowed.load(Ordering::SeqCst)
            {
                c.set_paused(false);
                self.wake();
            }
            return;
        }
        if let Some(c) = &self.clipboard {
            c.set_paused(true);
        }
        self.apply_relative();
        self.release_all();
        self.rest_gamepads();
        self.stop_rumble();
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Clavier et souris peuvent partir : session active, et droit donné.
    fn kbm_ok(&self) -> bool {
        self.is_active() && self.kbm_allowed.load(Ordering::SeqCst)
    }

    /// Appuis et axes des manettes peuvent partir : session active, et droit donné.
    fn pads_ok(&self) -> bool {
        self.is_active() && self.pads_allowed.load(Ordering::SeqCst)
    }

    /// Les manettes sont permises (KyberNow Share), active ou non.
    pub(crate) fn pads_allowed(&self) -> bool {
        self.pads_allowed.load(Ordering::SeqCst)
    }

    // KyberNow Share.

    /// Ce que la chaîne laisse partir (KyberNow Share ; un hôte : tout). Un
    /// droit retiré se comporte comme une session qui passe derrière : le
    /// clavier et la souris retirés relâchent touches et clics tenus et
    /// coupent la souris relative ; les manettes retirées sont mises au repos
    /// (touches relâchées, axes au centre), leurs vibrations arrêtées puis
    /// ignorées, et une manette branchée ensuite n'est pas annoncée ; le
    /// presse-papiers retiré est en pause. Les manettes rendues sont
    /// réannoncées à l'ordinateur (lecteur du cœur, et
    /// [`InputPlatform::connected`] pour une app qui produit les siennes).
    /// La session de l'invité l'appelle elle-même à chaque démarrage de
    /// l'image et quand son rôle change.
    pub fn set_rights(&self, rights: InputRights) {
        let regained_pads;
        {
            let _switch = self.switch.lock().unwrap();
            let kbm = self.kbm_allowed.swap(rights.keyboard_mouse, Ordering::SeqCst);
            let pads = self.pads_allowed.swap(rights.gamepads, Ordering::SeqCst);
            let clipboard = self.clipboard_allowed.swap(rights.clipboard, Ordering::SeqCst);
            if (kbm, pads, clipboard) == (rights.keyboard_mouse, rights.gamepads, rights.clipboard) {
                return;
            }
            log::info!(
                "{}entrées : manettes {}, clavier et souris {}, presse-papiers {}",
                self.label.prefix(),
                if rights.gamepads { "permises" } else { "retenues" },
                if rights.keyboard_mouse { "permis" } else { "retenus" },
                if rights.clipboard { "permis" } else { "retenu" }
            );
            if let Some(c) = &self.clipboard {
                let paused = !(rights.clipboard && self.is_active());
                c.set_paused(paused);
                if !paused {
                    self.wake();
                }
            }
            self.apply_relative();
            if kbm && !rights.keyboard_mouse {
                self.release_all();
            }
            if pads && !rights.gamepads {
                self.rest_gamepads();
                self.stop_rumble();
            }
            regained_pads = !pads && rights.gamepads;
        }
        if regained_pads && self.connected.load(Ordering::SeqCst) {
            self.announce_gamepads();
        }
    }

    pub fn rights(&self) -> InputRights {
        InputRights {
            gamepads: self.pads_allowed.load(Ordering::SeqCst),
            keyboard_mouse: self.kbm_allowed.load(Ordering::SeqCst),
            clipboard: self.clipboard_allowed.load(Ordering::SeqCst),
        }
    }

    /// Les manettes présentes, (ré)annoncées à l'ordinateur.
    fn announce_gamepads(&self) {
        if self.reader_attached() {
            crate::gamepads::global().connected(self);
        }
        self.platform.connected();
    }

    /// Toutes les manettes connues au repos : touches tenues relâchées, six
    /// axes au centre.
    fn rest_gamepads(&self) {
        let mut pads: HashSet<PadId> = self.combo.pads().into_iter().collect();
        pads.extend(self.axes.lock().unwrap().last.keys().copied());
        let mut pads: Vec<PadId> = pads.into_iter().collect();
        pads.sort_unstable();
        for id in pads {
            let mut held: Vec<PadButton> = self.combo.pressed(id).into_iter().collect();
            held.sort_by_key(|b| b.to_u32());
            for button in held {
                self.combo.button(id, button, false);
                self.reliably(Self::button_packet(id, button, false), "manette");
            }
            self.gamepad_axes_rest(id);
        }
    }

    /// Chaîne servie par le lecteur de manettes du cœur ([`crate::gamepads`]).
    pub(crate) fn set_reader_attached(&self, on: bool) {
        self.reader_attached.store(on, Ordering::Relaxed);
    }

    pub(crate) fn reader_attached(&self) -> bool {
        self.reader_attached.load(Ordering::Relaxed)
    }

    /// Manettes lues par le fil SDL du cœur pour cette chaîne (Mac, Windows).
    pub fn core_reads_gamepads(&self) -> bool {
        self.core_pads
    }

    /// Le nom de l'ordinateur au début des lignes du journal de cette chaîne.
    pub(crate) fn label(&self) -> &Arc<Label> {
        &self.label
    }

    /// Vibration reçue de l'ordinateur : ignorée si la session n'est pas
    /// active ; jouée par le fil SDL du cœur s'il tourne, et donnée à la
    /// plateforme. Sur le fil réseau de kynput, sous le verrou de son routeur :
    /// pas le verrou du passage (`switch`, pris par `set_active` avant
    /// d'envoyer), celui des vibrations, que `set_active` prend après avoir
    /// basculé.
    pub(crate) fn rumble_received(&self, r: GamepadRumble) {
        let mut rumbling = self.rumbling.lock().unwrap();
        if !self.pads_ok() {
            return;
        }
        if r.large_motor != 0 || r.small_motor != 0 {
            rumbling.insert(r.id);
        } else {
            rumbling.remove(&r.id);
        }
        self.play_rumble(r);
    }

    fn play_rumble(&self, r: GamepadRumble) {
        if self.reader_attached() || self.core_pads {
            crate::gamepads::rumble(r);
        }
        self.platform.rumble(r);
    }

    /// La chaîne passe derrière : les manettes qu'elle faisait vibrer
    /// s'arrêtent (vibration nulle) ; leurs vibrations suivantes, arrêt du
    /// jeu compris, sont ignorées.
    fn stop_rumble(&self) {
        let mut rumbling = self.rumbling.lock().unwrap();
        let mut pads: Vec<PadId> = rumbling.drain().collect();
        pads.sort_unstable();
        for id in pads {
            self.play_rumble(GamepadRumble { id, large_motor: 0, small_motor: 0, duration_ms: 0 });
        }
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
            log::info!("{}entrées : {dropped} paquets perdus pendant la session (file d'envoi pleine ou réseau absent)", self.label.prefix());
        }
        let _ = self.kynput.flush_keys();
        self.stopped.store(true, Ordering::Relaxed);
        self.release_sdl();
        self.wake();
    }

    /// Le fil SDL du cœur n'est plus tenu par cette chaîne (une fois).
    fn release_sdl(&self) {
        if self.sdl_held.swap(false, Ordering::SeqCst) {
            crate::gamepads::release_sdl();
        }
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
        log::info!("{}entrées reliées à l'ordinateur", self.label.prefix());
        self.connected.store(true, Ordering::SeqCst);
        // Les manettes du lecteur du cœur y sont branchées (l'app qui produit
        // les siennes les réannonce dans `connected`).
        self.announce_gamepads();
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
        if !self.kbm_ok() {
            return;
        }
        self.once(OwnedPacket::from_packet(MouseMove { dx, dy }.to_packet(InputTarget::Host)), "déplacement relatif");
    }

    /// Touche par son scancode (Windows : tel quel ; Apple : `keymap::scancode`).
    /// Notée dans [`Self::held`] pour « tout relâcher ».
    pub fn key(&self, scancode: u16, pressed: bool) {
        let _switch = self.switch.lock().unwrap();
        if pressed && !self.kbm_ok() {
            return;
        }
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
        if !self.kbm_ok() {
            return;
        }
        self.once(OwnedPacket::from_packet(MousePosition { display_id, x, y }.to_packet(InputTarget::Host)), "position");
    }

    pub fn mouse_button(&self, button: MouseButtonType, pressed: bool) {
        let _switch = self.switch.lock().unwrap();
        if pressed && !self.kbm_ok() {
            return;
        }
        self.held.lock().unwrap().mouse_button(button, pressed);
        self.reliably(OwnedPacket::from_packet(MouseButton { button, pressed }.to_packet(InputTarget::Host)), "clic");
    }

    pub fn wheel(&self, dx: f32, dy: f32) {
        if !self.kbm_ok() {
            return;
        }
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
        if !self.kbm_ok() {
            return;
        }
        for character in text.chars() {
            let packet = Unicode { character }.to_packet(InputTarget::Host).and_then(OwnedPacket::from_packet);
            self.reliably(packet, "texte");
        }
    }

    // Manettes produites par l'app (Apple TV, iPhone, iPad, Android, Linux, webOS).

    pub fn gamepad_plug(&self, id: PadId, vendor_id: Option<u16>, product_id: Option<u16>) {
        self.combo.note(id);
        // Manettes retenues (KyberNow Share) : annoncée quand elles seront rendues.
        if !self.pads_allowed() {
            return;
        }
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
        let _switch = self.switch.lock().unwrap();
        if pressed && !self.pads_ok() {
            return;
        }
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
        let _switch = self.switch.lock().unwrap();
        if value != 0 && !self.pads_ok() {
            return;
        }
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
        let _switch = self.switch.lock().unwrap();
        let mut axes = self.axes.lock().unwrap();
        let Some(at) = axes.flush_at else { return };
        let now = self.clock.now();
        if now < at {
            return;
        }
        axes.flush_at = None;
        let mut pending = std::mem::take(&mut axes.pending);
        if !self.pads_ok() {
            // Passée derrière depuis (ou manettes retenues) : seuls les retours au repos partent.
            for values in pending.values_mut() {
                values.retain(|_, value| *value == 0);
            }
        }
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
        let label = self.label.clone();
        std::thread::Builder::new().name("kybernow-entrees".into()).spawn(move || {
            journal::enter(label);
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
        self.release_sdl();
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
        pipeline.rumble_received(GamepadRumble {
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
    fn axes_en_attente_jetes_derriere() {
        let b = Bench::new();
        b.pipeline.set_active(false);
        b.host.clear();
        // Un axe arrivé juste avant le passage, encore en attente de sa tranche.
        {
            let mut axes = b.pipeline.axes.lock().unwrap();
            axes.pending.entry(4).or_default().insert(PadAxis::LeftX, 9000);
            axes.pending.entry(4).or_default().insert(PadAxis::LeftY, 0);
            axes.flush_at = Some(b.clock.now() + AXIS_SLICE);
        }
        b.elapse(4);
        assert_eq!(b.host.received(), [format!("axe 4 {} 0", PadAxis::LeftY.to_u32())], "seul le retour au repos part");
        b.pipeline.gamepad_axis(4, PadAxis::LeftX, 12_000);
        b.elapse(4);
        assert_eq!(b.host.received().len(), 1, "et rien de neuf en arrière-plan");
    }

    /// Un fil appuie et relâche sans arrêt pendant que la session passe
    /// derrière : une fois `set_active(false)` revenu, plus aucun appui ne
    /// part (ce qui avait vu la chaîne active est parti avant la mise au repos).
    #[test]
    fn aucun_appui_apres_le_passage_derriere() {
        for _ in 0..20 {
            let b = Bench::new();
            b.pipeline.gamepad_plug(0, None, None);
            let p = b.pipeline.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let s = stop.clone();
            let presser = std::thread::spawn(move || {
                let mut i = 0u32;
                while !s.load(Ordering::Relaxed) {
                    p.gamepad_button(0, PadButton::A, true);
                    p.key(0x1E, true);
                    p.gamepad_axis(0, PadAxis::LeftX, 1000 + (i % 30) as i16 * 512);
                    p.gamepad_button(0, PadButton::A, false);
                    p.key(0x1E, false);
                    i += 1;
                }
            });
            std::thread::sleep(Duration::from_millis(2));
            b.pipeline.set_active(false);
            let after = b.host.received().len();
            std::thread::sleep(Duration::from_millis(2));
            stop.store(true, Ordering::Relaxed);
            presser.join().unwrap();
            b.elapse(8);
            let late: Vec<String> = b.host.received()[after..].to_vec();
            let pressed_or_moved = |r: &String| r.ends_with("appuyée") || (r.starts_with("axe ") && !r.ends_with(" 0"));
            assert!(!late.iter().any(pressed_or_moved), "après le passage : {late:?}");
        }
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
