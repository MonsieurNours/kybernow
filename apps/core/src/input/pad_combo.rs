//! Combinaison manette L1 + R1 + Start + Select, et touches tenues de chaque
//! manette : ce que le Mac (manettes SDL vues par le hook de kynput),
//! l'Apple TV (GameController) et Windows ont en commun.
//!
//! La combinaison ouvre la barre ou le menu : la manette pilote alors
//! KyberNow et le jeu ne reçoit plus rien. Ici, on ne décide pas : on
//! signale la combinaison (`on_formed`), et c'est le contrôleur de session
//! qui prend la manette (« tout à zéro » : touches tenues relâchées, six
//! axes au repos, clics et touches de la manette en souris relâchés) quand
//! la barre ou le menu s'ouvre vraiment.
//!
//! Les quatre touches partent au jeu comme les autres ; la combinaison ne se
//! forme qu'au bout de 250 ms tenues sans qu'aucune soit lâchée. En dessous
//! (un jeu qui les presse ensemble, un appui maladroit), rien ne change pour
//! le jeu. Formée, les touches tenues seront relâchées pour l'ordinateur par
//! la capture.
//!
//! La minuterie est tirée par [`PadCombo::poll`], à appeler quand
//! [`PadCombo::next_deadline`] est passée ; `on_formed` est appelé hors du
//! verrou de la combinaison, sur le fil qui appelle `poll`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kynput_rs::{GamepadAxis, GamepadButton};
use kynput_sys as sys;

use super::clock::Clock;

/// Identifiant d'une manette dans kynput.
pub type PadId = kynput_rs::GamepadId;

/// Touches de manette, dans le vocabulaire de kynput (celui de SDL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    A,
    B,
    X,
    Y,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl PadButton {
    pub const ALL: [PadButton; 15] = [
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::Back,
        Self::Guide,
        Self::Start,
        Self::LeftStick,
        Self::RightStick,
        Self::LeftShoulder,
        Self::RightShoulder,
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
    ];

    /// Valeur du paquet kynput.
    pub fn to_u32(self) -> u32 {
        match self {
            Self::A => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_A,
            Self::B => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_B,
            Self::X => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_X,
            Self::Y => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_Y,
            Self::Back => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_BACK,
            Self::Guide => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_GUIDE,
            Self::Start => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_START,
            Self::LeftStick => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_LEFT_STICK,
            Self::RightStick => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_RIGHT_STICK,
            Self::LeftShoulder => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_LEFT_SHOULDER,
            Self::RightShoulder => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_RIGHT_SHOULDER,
            Self::DpadUp => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_D_PAD_UP,
            Self::DpadDown => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_D_PAD_DOWN,
            Self::DpadLeft => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_D_PAD_LEFT,
            Self::DpadRight => sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_D_PAD_RIGHT,
        }
    }

    /// Touche d'un paquet kynput (manettes SDL du Mac et de Windows).
    pub fn from_u32(value: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.to_u32() == value)
    }

    pub fn from_kynput(button: GamepadButton) -> Self {
        match button {
            GamepadButton::A => Self::A,
            GamepadButton::B => Self::B,
            GamepadButton::X => Self::X,
            GamepadButton::Y => Self::Y,
            GamepadButton::Back => Self::Back,
            GamepadButton::Guide => Self::Guide,
            GamepadButton::Start => Self::Start,
            GamepadButton::LeftStick => Self::LeftStick,
            GamepadButton::RightStick => Self::RightStick,
            GamepadButton::LeftShoulder => Self::LeftShoulder,
            GamepadButton::RightShoulder => Self::RightShoulder,
            GamepadButton::DPadUp => Self::DpadUp,
            GamepadButton::DPadDown => Self::DpadDown,
            GamepadButton::DPadLeft => Self::DpadLeft,
            GamepadButton::DPadRight => Self::DpadRight,
        }
    }

    pub fn to_kynput(self) -> GamepadButton {
        match self {
            Self::A => GamepadButton::A,
            Self::B => GamepadButton::B,
            Self::X => GamepadButton::X,
            Self::Y => GamepadButton::Y,
            Self::Back => GamepadButton::Back,
            Self::Guide => GamepadButton::Guide,
            Self::Start => GamepadButton::Start,
            Self::LeftStick => GamepadButton::LeftStick,
            Self::RightStick => GamepadButton::RightStick,
            Self::LeftShoulder => GamepadButton::LeftShoulder,
            Self::RightShoulder => GamepadButton::RightShoulder,
            Self::DpadUp => GamepadButton::DPadUp,
            Self::DpadDown => GamepadButton::DPadDown,
            Self::DpadLeft => GamepadButton::DPadLeft,
            Self::DpadRight => GamepadButton::DPadRight,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

impl PadAxis {
    /// Les six axes à remettre au repos à la capture (kynput ne dit pas leur état).
    pub const ALL: [PadAxis; 6] = [Self::LeftX, Self::LeftY, Self::RightX, Self::RightY, Self::LeftTrigger, Self::RightTrigger];

    pub fn to_u32(self) -> u32 {
        match self {
            Self::LeftX => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_LEFT_X,
            Self::LeftY => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_LEFT_Y,
            Self::RightX => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_RIGHT_X,
            Self::RightY => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_RIGHT_Y,
            Self::LeftTrigger => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_TRIGGER_LEFT,
            Self::RightTrigger => sys::kynput_GamepadAxis_KYNPUT_GAMEPAD_AXIS_TRIGGER_RIGHT,
        }
    }

    pub fn from_u32(value: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.to_u32() == value)
    }

    pub fn from_kynput(axis: GamepadAxis) -> Self {
        match axis {
            GamepadAxis::LeftX => Self::LeftX,
            GamepadAxis::LeftY => Self::LeftY,
            GamepadAxis::RightX => Self::RightX,
            GamepadAxis::RightY => Self::RightY,
            GamepadAxis::TriggerLeft => Self::LeftTrigger,
            GamepadAxis::TriggerRight => Self::RightTrigger,
        }
    }

    pub fn to_kynput(self) -> GamepadAxis {
        match self {
            Self::LeftX => GamepadAxis::LeftX,
            Self::LeftY => GamepadAxis::LeftY,
            Self::RightX => GamepadAxis::RightX,
            Self::RightY => GamepadAxis::RightY,
            Self::LeftTrigger => GamepadAxis::TriggerLeft,
            Self::RightTrigger => GamepadAxis::TriggerRight,
        }
    }
}

/// Touche de navigation pendant que la manette pilote l'app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKey {
    Left,
    Right,
    Up,
    Down,
    Accept,
    Back,
}

/// Les quatre touches de la combinaison.
pub const COMBO: [PadButton; 4] = [PadButton::LeftShoulder, PadButton::RightShoulder, PadButton::Start, PadButton::Back];
/// Durée tenue avant que la combinaison se forme.
pub const HOLD_DELAY: Duration = Duration::from_millis(250);

/// Combinaison formée pour une manette.
pub type OnFormed = Box<dyn Fn(PadId) + Send + Sync>;

struct Inner {
    /// Manettes vues passer (branchées, ou dont une touche ou un axe est passé).
    pads: HashSet<PadId>,
    pressed: HashMap<PadId, HashSet<PadButton>>,
    /// Combinaison formée et touches pas toutes relâchées depuis : ses touches
    /// n'agissent pas au relâchement (manette en souris). Oubliée au premier
    /// événement qui suit le relâchement complet, pour que le dernier
    /// relâchement la voie encore formée.
    formed: HashSet<PadId>,
    /// Minuteries armées : instant où la combinaison se forme.
    timers: HashMap<PadId, Duration>,
}

pub struct PadCombo {
    clock: Arc<dyn Clock>,
    delay: Duration,
    /// Réglage « la combinaison ouvre le menu » (Apple TV) ; `false` : rien n'est armé.
    enabled: AtomicBool,
    inner: Mutex<Inner>,
    on_formed: Mutex<Option<OnFormed>>,
}

impl PadCombo {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self::with_delay(clock, HOLD_DELAY)
    }

    pub fn with_delay(clock: Arc<dyn Clock>, delay: Duration) -> Self {
        Self {
            clock,
            delay,
            enabled: AtomicBool::new(true),
            inner: Mutex::new(Inner {
                pads: HashSet::new(),
                pressed: HashMap::new(),
                formed: HashSet::new(),
                timers: HashMap::new(),
            }),
            on_formed: Mutex::new(None),
        }
    }

    /// Combinaison formée pour cette manette (sur le fil qui appelle `poll`,
    /// hors du verrou).
    pub fn set_on_formed(&self, f: impl Fn(PadId) + Send + Sync + 'static) {
        *self.on_formed.lock().unwrap() = Some(Box::new(f));
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Manettes vues passer.
    pub fn pads(&self) -> Vec<PadId> {
        self.inner.lock().unwrap().pads.iter().copied().collect()
    }

    /// Touches tenues d'une manette, d'après ce qui est passé par ici.
    pub fn pressed(&self, id: PadId) -> HashSet<PadButton> {
        self.inner.lock().unwrap().pressed.get(&id).cloned().unwrap_or_default()
    }

    /// La combinaison s'est formée et une de ses touches est encore tenue.
    pub fn formed(&self, id: PadId) -> bool {
        self.inner.lock().unwrap().formed.contains(&id)
    }

    /// Manette vue (branchement, axe) : à remettre à zéro à la capture.
    pub fn note(&self, id: PadId) {
        self.inner.lock().unwrap().pads.insert(id);
    }

    /// Manette débranchée.
    pub fn forget(&self, id: PadId) {
        let mut inner = self.inner.lock().unwrap();
        inner.pads.remove(&id);
        inner.pressed.remove(&id);
        inner.formed.remove(&id);
        inner.timers.remove(&id);
    }

    /// Tout oublier (fin de session).
    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.pads.clear();
        inner.pressed.clear();
        inner.formed.clear();
        inner.timers.clear();
    }

    /// État réel d'une manette (resynchronisation après une remise à zéro).
    pub fn set_pressed(&self, id: PadId, buttons: HashSet<PadButton>) {
        let mut inner = self.inner.lock().unwrap();
        inner.pads.insert(id);
        inner.pressed.insert(id, buttons);
    }

    /// Une touche change : les quatre tenues arment la minuterie, une lâchée
    /// l'annule.
    pub fn button(&self, id: PadId, button: PadButton, down: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.pads.insert(id);
        let held = inner.pressed.entry(id).or_default();
        if COMBO.iter().all(|b| !held.contains(b)) {
            inner.formed.remove(&id);
        }
        let held = inner.pressed.entry(id).or_default();
        if down {
            held.insert(button);
        } else {
            held.remove(&button);
        }
        if !COMBO.contains(&button) {
            return;
        }
        if down && self.enabled() && COMBO.iter().all(|b| held.contains(b)) {
            let at = self.clock.now() + self.delay;
            inner.timers.insert(id, at);
        } else if !down {
            inner.timers.remove(&id);
        }
    }

    /// Quand appeler [`Self::poll`] au plus tard (`None` : rien d'armé).
    pub fn next_deadline(&self) -> Option<Duration> {
        self.inner.lock().unwrap().timers.values().min().copied()
    }

    /// Forme les combinaisons dont les 250 ms sont écoulées, si leurs quatre
    /// touches sont encore tenues.
    pub fn poll(&self) {
        let now = self.clock.now();
        let mut formed = Vec::new();
        {
            let mut inner = self.inner.lock().unwrap();
            let due: Vec<PadId> = inner.timers.iter().filter(|(_, at)| **at <= now).map(|(id, _)| *id).collect();
            for id in due {
                inner.timers.remove(&id);
                let all_held = inner.pressed.get(&id).is_some_and(|held| COMBO.iter().all(|b| held.contains(b)));
                if self.enabled() && all_held {
                    inner.formed.insert(id);
                    formed.push(id);
                }
            }
        }
        if formed.is_empty() {
            return;
        }
        let on_formed = self.on_formed.lock().unwrap();
        if let Some(f) = on_formed.as_ref() {
            for id in formed {
                f(id);
            }
        }
    }

    /// Croix et A / B pendant que l'app tient la manette.
    pub fn navigation_key(button: PadButton) -> Option<PadKey> {
        match button {
            PadButton::DpadLeft => Some(PadKey::Left),
            PadButton::DpadRight => Some(PadKey::Right),
            PadButton::DpadUp => Some(PadKey::Up),
            PadButton::DpadDown => Some(PadKey::Down),
            PadButton::A => Some(PadKey::Accept),
            PadButton::B => Some(PadKey::Back),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;

    struct Bench {
        clock: Arc<ManualClock>,
        combo: Arc<PadCombo>,
        formed: Arc<Mutex<Vec<PadId>>>,
    }

    impl Bench {
        fn new() -> Self {
            let clock = Arc::new(ManualClock::new());
            let combo = Arc::new(PadCombo::new(clock.clone()));
            let formed = Arc::new(Mutex::new(Vec::new()));
            let f = formed.clone();
            combo.set_on_formed(move |id| f.lock().unwrap().push(id));
            Self { clock, combo, formed }
        }

        fn press(&self, id: PadId, buttons: &[PadButton], down: bool) {
            for b in buttons {
                self.combo.button(id, *b, down);
            }
        }

        fn elapse(&self, ms: u64) {
            for _ in 0..ms {
                self.clock.advance(Duration::from_millis(1));
                self.combo.poll();
            }
        }

        fn formed(&self) -> Vec<PadId> {
            self.formed.lock().unwrap().clone()
        }
    }

    fn set(buttons: &[PadButton]) -> HashSet<PadButton> {
        buttons.iter().copied().collect()
    }

    #[test]
    fn tenue_250_ms_puis_relachement_complet() {
        let b = Bench::new();
        b.press(1, &COMBO, true);
        assert_eq!(b.combo.pressed(1), set(&COMBO), "touches tenues");
        assert_eq!(b.combo.pads(), [1], "manette connue");
        assert_eq!(b.combo.next_deadline(), Some(Duration::from_millis(250)));
        b.elapse(120);
        assert!(b.formed().is_empty(), "pas encore formée à 120 ms");
        b.elapse(250);
        assert_eq!(b.formed(), [1], "formée une fois après 250 ms tenues");
        assert!(b.combo.formed(1));
        b.elapse(300);
        assert_eq!(b.formed().len(), 1, "pas reformée tant que rien ne bouge");
        b.press(1, &[PadButton::LeftShoulder, PadButton::RightShoulder, PadButton::Start], false);
        assert!(b.combo.formed(1) && b.combo.pressed(1) == set(&[PadButton::Back]), "formée tant que Select est tenu");
        b.press(1, &[PadButton::Back], false);
        assert!(b.combo.formed(1), "formée encore au dernier relâchement");
        b.press(1, &[PadButton::A], true);
        b.press(1, &[PadButton::A], false);
        assert!(!b.combo.formed(1) && b.combo.pressed(1).is_empty(), "tout relâché : plus rien de formé ni de tenu");
    }

    #[test]
    fn breve_rien_ne_se_forme() {
        let b = Bench::new();
        b.press(2, &COMBO, true);
        b.elapse(100);
        b.press(2, &[PadButton::Start], false);
        assert_eq!(b.combo.next_deadline(), None, "une touche lâchée désarme la minuterie");
        b.elapse(350);
        assert!(b.formed().is_empty() && !b.combo.formed(2), "combinaison brève : rien ne se forme");
    }

    #[test]
    fn reglage_coupe_et_etat_repris() {
        let b = Bench::new();
        b.combo.set_pressed(2, set(&[PadButton::LeftShoulder, PadButton::Y]));
        assert_eq!(b.combo.pressed(2), set(&[PadButton::LeftShoulder, PadButton::Y]), "état réel repris");
        b.combo.set_enabled(false);
        b.press(2, &[PadButton::RightShoulder, PadButton::Start, PadButton::Back], true);
        b.elapse(350);
        assert!(b.formed().is_empty(), "réglage coupé : rien ne s'arme");
        b.combo.set_enabled(true);
        b.press(2, &[PadButton::Back], false);
        b.press(2, &[PadButton::Back], true);
        b.combo.set_enabled(false);
        b.elapse(350);
        assert!(b.formed().is_empty(), "réglage coupé pendant la minuterie : rien ne se forme");
        b.combo.forget(2);
        assert!(b.combo.pads().is_empty() && b.combo.pressed(2).is_empty(), "manette oubliée");
    }

    #[test]
    fn navigation_et_valeurs_kynput() {
        assert_eq!(PadCombo::navigation_key(PadButton::DpadLeft), Some(PadKey::Left));
        assert_eq!(PadCombo::navigation_key(PadButton::A), Some(PadKey::Accept));
        assert_eq!(PadCombo::navigation_key(PadButton::B), Some(PadKey::Back));
        assert_eq!(PadCombo::navigation_key(PadButton::X), None);
        for b in PadButton::ALL {
            assert_eq!(PadButton::from_u32(b.to_u32()), Some(b), "touches kynput ↔ PadButton");
            assert_eq!(PadButton::from_kynput(b.to_kynput()), b);
        }
        for a in PadAxis::ALL {
            assert_eq!(PadAxis::from_u32(a.to_u32()), Some(a));
            assert_eq!(PadAxis::from_kynput(a.to_kynput()), a);
        }
        assert_eq!(PadButton::from_u32(sys::kynput_GamepadButton_KYNPUT_GAMEPAD_BUTTON_D_PAD_UP), Some(PadButton::DpadUp));
        assert_ne!(PadButton::RightStick.to_u32(), PadButton::RightShoulder.to_u32(), "le stick droit n'est pas la gâchette droite (kynput-rs les confond)");
    }
}
