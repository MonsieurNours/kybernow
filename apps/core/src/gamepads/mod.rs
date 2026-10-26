//! Les manettes de l'app, lues une seule fois pour toutes ses sessions
//! (plusieurs ordinateurs à la fois, docs/plusieurs-ordinateurs.md).
//!
//! SDL ne s'ouvre qu'une fois par processus : deux chaînes kynput qui liraient
//! chacune les manettes par SDL (Mac, Windows) se voleraient leurs événements,
//! et la seconde les perdrait en silence. Un seul lecteur, ici, repris de
//! celui de l'app Linux (`apps/linux/src/device/pads.rs`) :
//!
//! - **ses manettes** : quatre au plus, numérotées dans l'ordre d'arrivée
//!   (0 à 3, l'identifiant donné à kynput), avec leurs touches tenues et
//!   leurs axes ;
//! - **deux façons de le nourrir** : son propre fil SDL ([`start_sdl`], Mac et
//!   Windows, où rien d'autre n'ouvre SDL ; il démarre aussi tout seul à la
//!   première chaîne d'entrées d'une app dont la libkynput n'a pas de lecteur
//!   SDL), ou les événements de l'app ([`plug`], [`unplug`], [`button`],
//!   [`axis`] : Linux, dont la boucle SDL lit déjà manettes et fenêtres dans
//!   la même file, et où un second lecteur volerait ses événements) ;
//! - **le routage** : chaque manette est branchée sur la chaîne d'entrées de
//!   chaque session (la Station garde sa manette virtuelle, un jeu ne la perd
//!   pas quand on change d'ordinateur), réannoncée quand une chaîne rejoint
//!   son ordinateur ; boutons et axes ne vont qu'aux chaînes actives
//!   ([`crate::session::Session::set_active`], que le registre
//!   [`crate::focus`] tient pour une seule session) ; une chaîne qui devient
//!   inactive remet elle-même ses manettes au repos (touches relâchées, axes
//!   au centre, gâchettes relâchées) ;
//! - **les vibrations** : seulement depuis la session active (une chaîne
//!   inactive les ignore) ; jouées ici avec le fil SDL, par l'app sinon
//!   (`InputPlatform::rumble`, comme avant).
//!
//! Une app à une session se comporte comme avant : sa session est active.
//! Les apps qui lisent leurs manettes elles-mêmes sans passer par ici
//! (Apple TV, iPhone, iPad, Android, webOS) ne changent pas : le lecteur n'a
//! alors aucune manette.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::input::pipeline::{GamepadRumble, InputPipeline};
use crate::input::{PadAxis, PadButton, PadId};

#[cfg(feature = "sdl")]
mod sdl;

/// Manettes suivies au plus (joueurs 1 à 4).
pub const MAX_PADS: usize = 4;

/// Une manette vue par le lecteur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PadInfo {
    /// L'identifiant donné à kynput (fil SDL : 0 à 3 ; Linux : celui de l'app).
    pub id: PadId,
    pub vendor: Option<u16>,
    pub product: Option<u16>,
    /// Le nom donné par SDL (vide quand l'app nourrit le lecteur).
    pub name: String,
}

struct Pad {
    info: PadInfo,
    pressed: HashSet<PadButton>,
    axes: [i16; 6],
}

#[derive(Default)]
struct State {
    pads: BTreeMap<PadId, Pad>,
    /// Les chaînes d'entrées des sessions (une par démarrage de l'image).
    targets: Vec<Weak<InputPipeline>>,
}

/// Le lecteur de manettes (un par processus : [`global`] ; les tests en
/// créent d'autres).
#[derive(Default)]
pub struct Gamepads {
    state: Mutex<State>,
}

impl Gamepads {
    pub fn new() -> Self {
        Self::default()
    }

    /// Les chaînes vivantes (pas arrêtées), hors du verrou pour les appels.
    fn targets(&self) -> Vec<Arc<InputPipeline>> {
        let mut state = self.state.lock().unwrap();
        state.targets.retain(|w| w.upgrade().is_some_and(|p| !p.is_stopped()));
        state.targets.iter().filter_map(Weak::upgrade).collect()
    }

    /// La chaîne d'entrées d'une session (à chaque démarrage de l'image) :
    /// elle recevra les manettes, branchées quand elle rejoindra son
    /// ordinateur ([`Self::connected`]).
    pub fn attach(&self, input: &Arc<InputPipeline>) {
        input.set_reader_attached(true);
        let mut state = self.state.lock().unwrap();
        state.targets.retain(|w| w.upgrade().is_some_and(|p| !p.is_stopped()));
        if !state.targets.iter().any(|w| w.as_ptr() == Arc::as_ptr(input)) {
            state.targets.push(Arc::downgrade(input));
        }
    }

    /// Une chaîne rejoint son ordinateur (début de l'image, reconnexion) :
    /// les manettes présentes y sont branchées.
    pub fn connected(&self, input: &InputPipeline) {
        let pads: Vec<PadInfo> = self.state.lock().unwrap().pads.values().map(|p| p.info.clone()).collect();
        for p in pads {
            input.gamepad_plug(p.id, p.vendor, p.product);
        }
    }

    /// Une manette branchée : sur toutes les sessions.
    pub fn plug(&self, id: PadId, vendor: Option<u16>, product: Option<u16>) {
        self.plug_named(id, vendor, product, String::new());
    }

    fn plug_named(&self, id: PadId, vendor: Option<u16>, product: Option<u16>, name: String) {
        let info = PadInfo { id, vendor, product, name };
        self.state.lock().unwrap().pads.insert(id, Pad { info, pressed: HashSet::new(), axes: [0; 6] });
        for t in self.targets() {
            t.gamepad_plug(id, vendor, product);
        }
    }

    /// Une manette débranchée : de toutes les sessions.
    pub fn unplug(&self, id: PadId) {
        if self.state.lock().unwrap().pads.remove(&id).is_none() {
            return;
        }
        for t in self.targets() {
            t.gamepad_unplug(id);
        }
    }

    /// Une touche : à la session active.
    pub fn button(&self, id: PadId, button: PadButton, pressed: bool) {
        {
            let mut state = self.state.lock().unwrap();
            let Some(pad) = state.pads.get_mut(&id) else { return };
            if pressed {
                pad.pressed.insert(button);
            } else {
                pad.pressed.remove(&button);
            }
        }
        for t in self.targets().into_iter().filter(|t| t.is_active()) {
            t.gamepad_button(id, button, pressed);
        }
    }

    /// Un axe : à la session active (filtré et regroupé par sa chaîne).
    pub fn axis(&self, id: PadId, axis: PadAxis, value: i16) {
        {
            let mut state = self.state.lock().unwrap();
            let Some(pad) = state.pads.get_mut(&id) else { return };
            if let Some(i) = PadAxis::ALL.iter().position(|a| *a == axis) {
                pad.axes[i] = value;
            }
        }
        for t in self.targets().into_iter().filter(|t| t.is_active()) {
            t.gamepad_axis(id, axis, value);
        }
    }

    /// Les manettes présentes.
    pub fn pads(&self) -> Vec<PadInfo> {
        self.state.lock().unwrap().pads.values().map(|p| p.info.clone()).collect()
    }

    /// Touches tenues d'une manette.
    pub fn pressed(&self, id: PadId) -> HashSet<PadButton> {
        self.state.lock().unwrap().pads.get(&id).map(|p| p.pressed.clone()).unwrap_or_default()
    }

    /// Les six axes d'une manette, dans l'ordre de [`PadAxis::ALL`].
    pub fn axes(&self, id: PadId) -> [i16; 6] {
        self.state.lock().unwrap().pads.get(&id).map(|p| p.axes).unwrap_or_default()
    }
}

/// Le lecteur du processus.
pub fn global() -> &'static Gamepads {
    static GLOBAL: OnceLock<Gamepads> = OnceLock::new();
    GLOBAL.get_or_init(Gamepads::new)
}

/// Le fil SDL du lecteur (Mac, Windows) : les manettes sont lues par SDL
/// (GameController) sur un fil du cœur, `kybernow-manettes`, et routées vers
/// les sessions ; les vibrations de la session active y sont jouées. Une
/// fois par processus (les appels suivants ne font rien) ; à n'appeler que
/// si rien d'autre n'ouvre SDL dans l'app (pas sous Linux) et avec une
/// libkynput construite sans son lecteur SDL. `false` : cœur construit sans
/// SDL (feature `sdl`), libkynput qui a encore son lecteur SDL (les deux se
/// voleraient leurs événements : ses manettes restent alors celles de
/// kynput), ou SDL n'a pas pu s'ouvrir.
pub fn start_sdl() -> bool {
    #[cfg(feature = "sdl")]
    {
        let probe = kynput_rs::Pipeline::new(kynput_rs::Config::default_client());
        if probe.produces_type(kynput_rs::InputType::GamepadEvent) {
            log::warn!("manettes : libkynput a encore son lecteur SDL, celui du cœur ne démarre pas (construire libkynput sans SDL)");
            return false;
        }
        drop(probe);
        sdl::start()
    }
    #[cfg(not(feature = "sdl"))]
    {
        log::warn!("manettes : cœur construit sans SDL, aucun lecteur");
        false
    }
}

/// Le fil SDL, quand la chaîne d'entrées qui le demande a déjà vu que
/// libkynput n'a pas de lecteur SDL.
pub(crate) fn start_sdl_for_chain() {
    #[cfg(feature = "sdl")]
    sdl::start();
}

/// Le cœur sait lire les manettes par SDL (feature `sdl` : Mac, Windows).
pub fn sdl_compiled() -> bool {
    cfg!(feature = "sdl")
}

/// Le fil SDL du lecteur tourne.
pub fn sdl_running() -> bool {
    #[cfg(feature = "sdl")]
    {
        sdl::running()
    }
    #[cfg(not(feature = "sdl"))]
    {
        false
    }
}

/// Manette branchée (Linux : l'app nourrit le lecteur).
pub fn plug(id: PadId, vendor: Option<u16>, product: Option<u16>) {
    global().plug(id, vendor, product);
}

/// Manette débranchée (Linux).
pub fn unplug(id: PadId) {
    global().unplug(id);
}

/// Touche de manette (Linux), à la session active.
pub fn button(id: PadId, button: PadButton, pressed: bool) {
    global().button(id, button, pressed);
}

/// Axe de manette (Linux), à la session active.
pub fn axis(id: PadId, axis: PadAxis, value: i16) {
    global().axis(id, axis, value);
}

/// Les manettes présentes.
pub fn pads() -> Vec<PadInfo> {
    global().pads()
}

/// Vibration demandée par l'ordinateur de la session active : jouée par le
/// fil SDL s'il tourne (sinon c'est l'app qui la joue, `InputPlatform::rumble`).
pub(crate) fn rumble(r: GamepadRumble) {
    #[cfg(feature = "sdl")]
    sdl::rumble(r);
    #[cfg(not(feature = "sdl"))]
    let _ = r;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;
    use crate::input::packet_queue::tests::Host;
    use crate::input::pipeline::{InputPlatform, Options};
    use crate::input::pointer_layer::PointerLayer;
    use std::time::Duration;

    struct Quiet;
    impl InputPlatform for Quiet {}

    fn chain(clock: &Arc<ManualClock>) -> (Arc<InputPipeline>, Arc<Host>) {
        let host = Arc::new(Host::default());
        let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
        let p = InputPipeline::new_with_sink(
            Options { display_id: 1, host_size: (1920, 1080), clipboard: None, gamepads_from_app: true },
            pointer,
            Arc::new(Quiet),
            clock.clone(),
            host.clone(),
        );
        (p, host)
    }

    fn elapse(clock: &ManualClock, chains: &[&Arc<InputPipeline>], ms: u64) {
        for _ in 0..ms {
            clock.advance(Duration::from_millis(1));
            for c in chains {
                c.poll();
            }
        }
    }

    #[test]
    fn branchees_partout_touches_a_la_session_active() {
        let clock = Arc::new(ManualClock::new());
        let pads = Gamepads::new();
        let (a, host_a) = chain(&clock);
        let (b, host_b) = chain(&clock);
        pads.attach(&a);
        pads.attach(&b);
        pads.attach(&a);
        b.set_active(false);
        pads.plug(0, Some(0x054C), Some(0x0CE6));
        assert_eq!(host_a.received().len(), 1, "branchée sur la session active");
        assert_eq!(host_b.received().len(), 1, "et sur l'autre : la Station garde sa manette");
        pads.button(0, PadButton::A, true);
        pads.axis(0, PadAxis::LeftX, 12_000);
        elapse(&clock, &[&a, &b], 4);
        assert_eq!(host_a.received()[1..], ["manette 0 A appuyée".to_string(), format!("axe 0 {} 12000", PadAxis::LeftX.to_u32())]);
        assert_eq!(host_b.received().len(), 1, "rien d'autre pour la session pas active");
        assert_eq!(pads.pressed(0).len(), 1);
        assert_eq!(pads.axes(0)[0], 12_000);
        // Une touche inconnue (manette jamais branchée) : rien.
        pads.button(7, PadButton::B, true);
        assert_eq!(host_a.received().len(), 3);
    }

    #[test]
    fn changement_de_session_repos_puis_suite_ailleurs() {
        let clock = Arc::new(ManualClock::new());
        let pads = Gamepads::new();
        let (a, host_a) = chain(&clock);
        let (b, host_b) = chain(&clock);
        pads.attach(&a);
        pads.attach(&b);
        b.set_active(false);
        pads.plug(1, None, None);
        pads.button(1, PadButton::RightShoulder, true);
        pads.axis(1, PadAxis::RightTrigger, 30_000);
        elapse(&clock, &[&a, &b], 4);
        host_a.clear();
        host_b.clear();
        // Ordinateur suivant : l'ancienne remet tout au repos, la nouvelle reçoit la suite.
        a.set_active(false);
        b.set_active(true);
        let got = host_a.received();
        assert_eq!(got[0], "manette 1 RightShoulder relâchée", "touche tenue relâchée");
        assert_eq!(got.len(), 1 + 6, "puis les six axes au repos");
        assert!(got[1..].iter().all(|r| r.ends_with(" 0")));
        pads.axis(1, PadAxis::RightTrigger, 20_000);
        elapse(&clock, &[&a, &b], 4);
        assert_eq!(host_a.received().len(), 7, "plus rien pour l'ancienne");
        assert_eq!(host_b.received(), [format!("axe 1 {} 20000", PadAxis::RightTrigger.to_u32())]);
    }

    #[test]
    fn rebranchees_quand_la_chaine_rejoint_son_ordinateur() {
        let clock = Arc::new(ManualClock::new());
        let pads = Gamepads::new();
        pads.plug(2, Some(0x045E), Some(0x0B13));
        let (a, host_a) = chain(&clock);
        pads.attach(&a);
        assert!(host_a.received().is_empty(), "rien avant que la chaîne rejoigne l'ordinateur");
        pads.connected(&a);
        assert_eq!(host_a.received().len(), 1, "la manette présente est branchée");
        // Image relancée : chaîne neuve, l'ancienne arrêtée n'est plus servie.
        a.stop();
        let (a2, host_a2) = chain(&clock);
        pads.attach(&a2);
        pads.connected(&a2);
        pads.button(2, PadButton::X, true);
        assert_eq!(host_a2.received().len(), 2);
        assert_eq!(host_a.received().len(), 1);
        pads.unplug(2);
        assert_eq!(host_a2.received().len(), 3, "débranchée");
        assert!(pads.pads().is_empty());
    }

    /// Avec une libkynput qui a encore son lecteur SDL (celle du rootfs des
    /// tests tant que build-kyclient.sh ne l'a pas refaite), le lecteur du
    /// cœur refuse de démarrer : les deux se voleraient leurs événements.
    /// Avec une libkynput sans SDL, rien n'est essayé ici (SDL ouvrirait les
    /// manettes de la machine).
    #[cfg(feature = "sdl")]
    #[test]
    fn fil_sdl_refuse_a_cote_de_celui_de_kynput() {
        let kynput_has_sdl = kynput_rs::Pipeline::new(kynput_rs::Config::default_client()).produces_type(kynput_rs::InputType::GamepadEvent);
        if kynput_has_sdl {
            assert!(!start_sdl());
            assert!(!sdl_running());
        }
    }

    #[test]
    fn vibrations_seulement_de_la_session_active() {
        use std::sync::Mutex as M;
        #[derive(Default)]
        struct Rumbles(M<Vec<GamepadRumble>>);
        impl InputPlatform for Rumbles {
            fn rumble(&self, r: GamepadRumble) {
                self.0.lock().unwrap().push(r);
            }
        }
        let clock = Arc::new(ManualClock::new());
        let rumbles = Arc::new(Rumbles::default());
        let pointer = Arc::new(PointerLayer::new(clock.clone(), true));
        let p = InputPipeline::new_with_sink(
            Options { display_id: 1, host_size: (1920, 1080), clipboard: None, gamepads_from_app: true },
            pointer,
            rumbles.clone(),
            clock.clone(),
            Arc::new(Host::default()),
        );
        let r = GamepadRumble { id: 0, large_motor: 40_000, small_motor: 0, duration_ms: 100 };
        p.rumble_received(r);
        p.set_active(false);
        p.rumble_received(r);
        assert_eq!(rumbles.0.lock().unwrap().len(), 1, "ignorée en arrière-plan");
    }
}
