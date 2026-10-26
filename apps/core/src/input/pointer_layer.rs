//! Pointeur de l'ordinateur et mode de la souris : ce que les vues de session
//! des apps ont en commun. Les vues ne font que dessiner (NSCursor et couche
//! sur Mac, couche sur Apple TV, iPhone et iPad, curseur Win32 sur Windows…)
//! et capter les événements de leur plateforme.
//!
//! Pointeur : l'ordinateur annonce la forme, le point chaud, la position et la
//! visibilité de son pointeur (kyber-patches 0006 : positions seules sans
//! bitmap quand la forme ne change pas). L'Apple TV tient en plus une position
//! à elle (télécommande, manette ou souris Bluetooth en positions absolues :
//! Windows masque son pointeur sans souris branchée et ne le réaffiche que sur
//! une position absolue) ; le Mac et Windows n'en ont pas besoin, leur souris
//! est celle du système.
//!
//! Mode de la souris : Bureau (positions absolues, pointeur de l'appareil),
//! Jeu (déplacements bruts), Auto (jeu quand un jeu est en plein écran sur
//! l'ordinateur, d'après la veille de la console). Sur Mac et Windows, Auto
//! demande en plus que le jeu cache son pointeur (vue subjective), avec une
//! tolérance de 0,6 s pour un pointeur remontré un instant et un recontrôle
//! 0,7 s après ; sur Apple TV le plein écran suffit (la souris Bluetooth
//! passe en brut).
//!
//! Fils : l'état est sous verrou ; `on_changed` est appelé hors du verrou,
//! sur le fil qui a changé l'état (celui de kynput pour les annonces de
//! l'ordinateur, celui des entrées de l'appareil pour la position tenue).
//! Le dessin est à la charge de la vue, qui repasse sur son fil principal.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::clock::Clock;

/// Bureau : souris absolue, pointeur de l'appareil toujours là. Jeu : souris
/// relative (déplacements bruts). Auto : jeu quand un jeu est en plein écran
/// sur l'ordinateur (et, sur Mac et Windows, cache son pointeur).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseMode {
    Auto,
    Desktop,
    Game,
}

/// Règles Auto / Bureau / Jeu, avec la veille du plein écran de la console.
#[derive(Clone, Copy, Debug)]
pub struct MouseModeRules {
    /// Mac, Windows : en Auto, le jeu doit aussi cacher son pointeur ; Apple
    /// TV : le plein écran suffit.
    pub needs_hidden_pointer: bool,
}

impl MouseModeRules {
    /// Un pointeur remontré moins longtemps que ça (menus, clignotement de
    /// Windows) ne fait pas quitter le mode jeu…
    pub const SHOWN_TOLERANCE: Duration = Duration::from_millis(600);
    /// … et on revérifie après ce délai.
    pub const RECHECK_DELAY: Duration = Duration::from_millis(700);

    /// La souris doit-elle être relative. `visible_for` : depuis combien de
    /// temps l'ordinateur montre son pointeur (zéro : caché).
    pub fn wants_relative(&self, mode: MouseMode, game_fullscreen: bool, remote_hidden: bool, visible_for: Duration, relative_active: bool) -> bool {
        match mode {
            MouseMode::Desktop => false,
            MouseMode::Game => true,
            MouseMode::Auto => {
                if !game_fullscreen {
                    return false;
                }
                if !self.needs_hidden_pointer {
                    return true;
                }
                if relative_active { visible_for < Self::SHOWN_TOLERANCE } else { remote_hidden }
            }
        }
    }
}

/// Forme du pointeur, en pixels de l'écran de l'ordinateur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub width: u32,
    pub height: u32,
    /// Pixels ARGB (32 bits, alpha non prémultiplié), ligne par ligne, tels
    /// que kynput les livre.
    pub argb: Arc<[u32]>,
    pub hot_x: u32,
    pub hot_y: u32,
}

impl Shape {
    /// Les mêmes pixels, alpha prémultiplié (ce qu'attendent CGImage sur
    /// Apple et les curseurs de Windows).
    pub fn premultiplied(&self) -> Vec<u32> {
        self.argb
            .iter()
            .map(|&p| {
                let a = (p >> 24) & 0xFF;
                let r = ((p >> 16) & 0xFF) * a / 255;
                let g = ((p >> 8) & 0xFF) * a / 255;
                let b = (p & 0xFF) * a / 255;
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect()
    }
}

/// Pointeur de l'ordinateur, tel qu'il l'annonce.
#[derive(Clone, Debug)]
pub struct RemoteCursor {
    pub visible: bool,
    /// Forme ; `None` : inchangée ou vide (voir `InputPipeline`, qui l'écarte).
    pub image: Option<Shape>,
    /// Position sur l'écran de l'ordinateur, en pixels.
    pub position: (i32, i32),
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// Position annoncée par l'ordinateur (pixels de son écran).
    pub remote_position: Option<(f64, f64)>,
    /// L'ordinateur cache son pointeur.
    pub remote_hidden: bool,
    /// Depuis quand il le montre (`None` : caché).
    pub remote_visible_since: Option<Duration>,
    /// Dernière forme reçue (un masquage ou une forme vide la garde).
    pub shape: Option<Shape>,
    /// Position tenue par l'appareil (Apple TV), à défaut celle de l'ordinateur.
    pub held: Option<(f64, f64)>,
}

impl State {
    pub fn position(&self) -> Option<(f64, f64)> {
        self.held.or(self.remote_position)
    }

    /// Depuis combien de temps l'ordinateur montre son pointeur (zéro : caché).
    pub fn visible_for(&self, now: Duration) -> Duration {
        self.remote_visible_since.map_or(Duration::ZERO, |since| now.saturating_sub(since))
    }
}

/// Le pointeur de l'ordinateur pour la souris Auto, sur tous les écrans d'une
/// session (multi-écrans, docs/multi-ecrans.md) : une seule règle pour toutes
/// les apps. Un pointeur qui quitte un écran y est annoncé caché : il n'est
/// caché que s'il l'est sur tous les écrans où il est déjà passé, et visible
/// depuis le plus long de leurs délais. Un écran où le pointeur n'est jamais
/// passé (aucune annonce de l'ordinateur) ne compte pas : resté « visible »
/// par défaut, il empêcherait le mode jeu de se déclencher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerSummary {
    /// L'ordinateur cache son pointeur partout.
    pub hidden: bool,
    /// Depuis combien de temps il le montre (zéro : caché).
    pub visible_for: Duration,
}

impl PointerSummary {
    /// D'après l'état de la couche de chaque écran, à l'instant `now`.
    pub fn of(states: &[State], now: Duration) -> Self {
        let seen: Vec<&State> = states.iter().filter(|s| s.remote_position.is_some()).collect();
        // Aucune annonce encore : l'état par défaut (visible), comme une session à un écran.
        let counted: Vec<&State> = if seen.is_empty() { states.iter().collect() } else { seen };
        Self {
            hidden: !counted.is_empty() && counted.iter().all(|s| s.remote_hidden),
            visible_for: counted.iter().map(|s| s.visible_for(now)).max().unwrap_or(Duration::ZERO),
        }
    }
}

/// Ce qui a changé ; dans tous les cas, à redessiner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// L'ordinateur a caché ou remontré son pointeur.
    pub visibility: bool,
    /// Nouvelle forme.
    pub shape: bool,
}

/// Cadre d'un pointeur dessiné, dans les unités de la vue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// L'état a changé : la vue redessine.
pub type OnChanged = Box<dyn Fn(&State, Change) + Send + Sync>;

struct Inner {
    state: State,
    /// Écran de l'ordinateur (pixels) ; la position tenue y reste.
    host_size: (f64, f64),
    /// Dernier déplacement de la position tenue : l'ordinateur ne reprend la
    /// main sur elle qu'au repos de nos entrées.
    last_local_move: Option<Duration>,
    /// Envoi d'une position absolue à l'ordinateur (`InputPipeline::mouse`).
    send: Option<Box<dyn Fn(i16, i16) + Send + Sync>>,
}

/// État du pointeur de l'ordinateur.
pub struct PointerLayer {
    clock: Arc<dyn Clock>,
    /// L'appareil tient une position à lui (Apple TV) ; sinon seule celle de
    /// l'ordinateur compte (Mac, Windows).
    pub holds_position: bool,
    inner: Mutex<Inner>,
    on_changed: Mutex<Option<OnChanged>>,
}

const LOCAL_MOVE_WINDOW: Duration = Duration::from_millis(400);

impl PointerLayer {
    pub fn new(clock: Arc<dyn Clock>, holds_position: bool) -> Self {
        Self::with_host_size(clock, holds_position, (1920.0, 1080.0))
    }

    pub fn with_host_size(clock: Arc<dyn Clock>, holds_position: bool, host_size: (f64, f64)) -> Self {
        Self {
            clock,
            holds_position,
            inner: Mutex::new(Inner {
                state: State {
                    remote_visible_since: Some(Duration::ZERO),
                    ..State::default()
                },
                host_size,
                last_local_move: None,
                send: None,
            }),
            on_changed: Mutex::new(None),
        }
    }

    /// L'état a changé : la vue redessine (sur son fil principal).
    pub fn set_on_changed(&self, f: impl Fn(&State, Change) + Send + Sync + 'static) {
        *self.on_changed.lock().unwrap() = Some(Box::new(f));
    }

    pub fn state(&self) -> State {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn host_size(&self) -> (f64, f64) {
        self.inner.lock().unwrap().host_size
    }

    /// Multi-écrans : le pointeur de l'ordinateur pour la souris Auto, sur
    /// les couches de tous les écrans de la session ([`PointerSummary`]).
    pub fn summary<'a>(layers: impl IntoIterator<Item = &'a PointerLayer>) -> PointerSummary {
        let layers: Vec<&PointerLayer> = layers.into_iter().collect();
        let Some(first) = layers.first() else { return PointerSummary { hidden: false, visible_for: Duration::ZERO } };
        let now = first.clock.now();
        let states: Vec<State> = layers.iter().map(|l| l.state()).collect();
        PointerSummary::of(&states, now)
    }

    /// Nouvelle chaîne d'entrées (début ou relance de l'image) : l'écran de
    /// l'ordinateur et l'envoi des positions, sans toucher à la position tenue.
    pub fn attach(&self, host_size: (f64, f64), send: impl Fn(i16, i16) + Send + Sync + 'static) {
        let mut inner = self.inner.lock().unwrap();
        inner.host_size = host_size;
        inner.send = Some(Box::new(send));
    }

    /// Annonce de l'ordinateur (depuis le gestionnaire de kynput).
    pub fn remote_cursor_changed(&self, remote: RemoteCursor) {
        let now = self.clock.now();
        let (state, change) = {
            let mut inner = self.inner.lock().unwrap();
            let position = (f64::from(remote.position.0), f64::from(remote.position.1));
            inner.state.remote_position = Some(position);
            let mut change = Change::default();
            if remote.visible == inner.state.remote_hidden {
                inner.state.remote_hidden = !remote.visible;
                log::info!("pointeur de l'ordinateur {}", if remote.visible { "visible" } else { "caché" });
                inner.state.remote_visible_since = if inner.state.remote_hidden { None } else { Some(now) };
                change.visibility = true;
            }
            // L'ordinateur a déplacé son pointeur lui-même (au repos de nos
            // entrées) : la position tenue repart de la sienne.
            if self.holds_position && inner.last_local_move.is_none_or(|at| now.saturating_sub(at) > LOCAL_MOVE_WINDOW) {
                inner.state.held = Some(position);
            }
            // Forme nouvelle : on l'adopte. Masquage ou forme vide : on garde la dernière.
            if let Some(shape) = remote.image {
                inner.state.shape = Some(shape);
                change.shape = true;
            }
            (inner.state.clone(), change)
        };
        self.changed(&state, change);
    }

    /// L'appareil déplace le pointeur (pixels de l'écran de l'ordinateur) et
    /// envoie sa position (Apple TV : télécommande, manette, souris Bluetooth
    /// en mode bureau). Il part du dernier pointeur connu, ou du centre.
    pub fn move_by(&self, dx: f64, dy: f64) {
        let state = {
            let mut inner = self.inner.lock().unwrap();
            let (w, h) = inner.host_size;
            if !self.holds_position || w <= 0.0 || h <= 0.0 {
                return;
            }
            let (x, y) = inner.state.position().unwrap_or((w / 2.0, h / 2.0));
            let p = ((x + dx).clamp(0.0, w - 1.0), (y + dy).clamp(0.0, h - 1.0));
            inner.state.held = Some(p);
            inner.last_local_move = Some(self.clock.now());
            Self::send_held(&inner);
            inner.state.clone()
        };
        self.changed(&state, Change::default());
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session : la
    /// position tenue garde sa place relative (le centre reste au centre), et
    /// l'ordinateur l'apprend tout de suite.
    pub fn set_host_size(&self, size: (f64, f64)) {
        let state = {
            let mut inner = self.inner.lock().unwrap();
            let (ow, oh) = inner.host_size;
            inner.host_size = size;
            if let Some((x, y)) = inner.state.held
                && ow > 0.0
                && oh > 0.0
            {
                inner.state.held = Some(((x * size.0 / ow).clamp(0.0, size.0 - 1.0), (y * size.1 / oh).clamp(0.0, size.1 - 1.0)));
                Self::send_held(&inner);
            }
            inner.state.clone()
        };
        self.changed(&state, Change::default());
    }

    /// Entrées reliées de nouveau à l'ordinateur (service d'entrées relancé) :
    /// la position tenue est renvoyée à sa place.
    pub fn resend_held(&self) {
        let inner = self.inner.lock().unwrap();
        Self::send_held(&inner);
    }

    fn send_held(inner: &Inner) {
        if let (Some((x, y)), Some(send)) = (inner.state.held, &inner.send) {
            send(clamp_i16(x), clamp_i16(y));
        }
    }

    /// Fin de session : la position tenue et la visibilité sont oubliées, la
    /// forme reste (la prochaine session la remplace).
    pub fn reset(&self) {
        let state = {
            let mut inner = self.inner.lock().unwrap();
            inner.state.held = None;
            inner.state.remote_hidden = false;
            inner.state.remote_visible_since = Some(Duration::ZERO);
            inner.state.clone()
        };
        self.changed(&state, Change::default());
    }

    fn changed(&self, state: &State, change: Change) {
        if let Some(f) = self.on_changed.lock().unwrap().as_ref() {
            f(state, change);
        }
    }

    // Dessin.

    /// Facteur d'échelle d'une forme pour la dessiner dans une case de `box_`
    /// points. Windows dessine ses pointeurs dans une case de 32 pixels à
    /// 100 %, 48 à 150 %, 64 à 200 %… : ramener cette case à `box_` donne la
    /// même taille quelle que soit la mise à l'échelle de Windows ou de
    /// l'appareil ; pointe et image suivent le même facteur. Mac : 32 points,
    /// jamais agrandi (la taille d'un pointeur du Mac) ; Apple TV : 48 points,
    /// jusqu'à deux fois (lisible à 3 mètres).
    pub fn scale(pixels: (u32, u32), box_: f64, max_factor: f64) -> f64 {
        let largest = f64::from(pixels.0.max(pixels.1).max(1));
        max_factor.min(box_ / largest)
    }

    /// Cadre du pointeur dessiné : `local` est la pointe (point actif) dans la
    /// vue, y vers le bas ; `flipped_height` : hauteur de la vue si son y va
    /// vers le haut (Mac).
    pub fn frame(shape: &Shape, local: (f64, f64), factor: f64, flipped_height: Option<f64>) -> Frame {
        let width = f64::from(shape.width) * factor;
        let height = f64::from(shape.height) * factor;
        let x = local.0 - f64::from(shape.hot_x) * factor;
        let top = local.1 - f64::from(shape.hot_y) * factor;
        match flipped_height {
            Some(view_height) => Frame {
                x,
                y: view_height - top - height,
                width,
                height,
            },
            None => Frame { x, y: top, width, height },
        }
    }
}

fn clamp_i16(v: f64) -> i16 {
    v.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;

    fn shape(w: u32, h: u32) -> Shape {
        Shape {
            width: w,
            height: h,
            argb: vec![0xFF00_0000; (w * h) as usize].into(),
            hot_x: 1,
            hot_y: 2,
        }
    }

    #[test]
    fn regles_du_mode_de_souris() {
        let mac = MouseModeRules { needs_hidden_pointer: true };
        let tv = MouseModeRules { needs_hidden_pointer: false };
        assert!(!mac.wants_relative(MouseMode::Desktop, true, true, Duration::ZERO, true), "Bureau : jamais");
        assert!(mac.wants_relative(MouseMode::Game, false, false, Duration::ZERO, false), "Jeu : toujours");
        assert!(!mac.wants_relative(MouseMode::Auto, false, true, Duration::ZERO, false), "Auto sans plein écran : non");
        assert!(mac.wants_relative(MouseMode::Auto, true, true, Duration::ZERO, false), "Auto, plein écran et pointeur caché : oui");
        assert!(!mac.wants_relative(MouseMode::Auto, true, false, Duration::from_secs(1), false), "pointeur visible, mode pas encore actif : non");
        assert!(mac.wants_relative(MouseMode::Auto, true, false, Duration::from_millis(300), true), "pointeur remontré 0,3 s : on reste en jeu");
        assert!(!mac.wants_relative(MouseMode::Auto, true, false, Duration::from_millis(700), true), "remontré 0,7 s : retour au bureau");
        assert!(tv.wants_relative(MouseMode::Auto, true, false, Duration::from_secs(5), false), "Apple TV : le plein écran suffit");
    }

    #[test]
    fn annonces_de_l_ordinateur() {
        let clock = Arc::new(ManualClock::new());
        let layer = Arc::new(PointerLayer::new(clock.clone(), false));
        let changes = Arc::new(Mutex::new(Vec::new()));
        let c = changes.clone();
        layer.set_on_changed(move |_, change| c.lock().unwrap().push(change));
        clock.advance(Duration::from_secs(1));
        layer.remote_cursor_changed(RemoteCursor {
            visible: false,
            image: None,
            position: (10, 20),
        });
        let s = layer.state();
        assert!(s.remote_hidden && s.remote_visible_since.is_none() && s.shape.is_none());
        assert_eq!(s.position(), Some((10.0, 20.0)), "Mac : la position est celle de l'ordinateur");
        assert_eq!(s.visible_for(clock.now()), Duration::ZERO, "caché : zéro");
        layer.remote_cursor_changed(RemoteCursor {
            visible: true,
            image: Some(shape(4, 4)),
            position: (11, 21),
        });
        clock.advance(Duration::from_millis(500));
        let s = layer.state();
        assert!(!s.remote_hidden && s.shape.is_some());
        assert_eq!(s.visible_for(clock.now()), Duration::from_millis(500));
        layer.remote_cursor_changed(RemoteCursor {
            visible: true,
            image: None,
            position: (12, 22),
        });
        assert!(layer.state().shape.is_some(), "forme gardée quand l'annonce n'en porte pas");
        assert_eq!(
            *changes.lock().unwrap(),
            [
                Change { visibility: true, shape: false },
                Change { visibility: true, shape: true },
                Change::default()
            ]
        );
        layer.reset();
        let s = layer.state();
        assert!(!s.remote_hidden && s.shape.is_some(), "fin de session : la forme reste");
    }

    #[test]
    fn position_tenue_par_l_apple_tv() {
        let clock = Arc::new(ManualClock::new());
        let layer = PointerLayer::with_host_size(clock.clone(), true, (100.0, 50.0));
        let sent = Arc::new(Mutex::new(Vec::new()));
        let s = sent.clone();
        layer.attach((100.0, 50.0), move |x, y| s.lock().unwrap().push((x, y)));
        layer.move_by(10.0, -100.0);
        assert_eq!(layer.state().held, Some((60.0, 0.0)), "part du centre, borné à l'écran");
        assert_eq!(*sent.lock().unwrap(), [(60, 0)]);
        // L'ordinateur annonce une position pendant nos déplacements : ignorée.
        layer.remote_cursor_changed(RemoteCursor {
            visible: true,
            image: None,
            position: (5, 5),
        });
        assert_eq!(layer.state().held, Some((60.0, 0.0)), "position tenue gardée pendant 0,4 s");
        clock.advance(Duration::from_millis(500));
        layer.remote_cursor_changed(RemoteCursor {
            visible: true,
            image: None,
            position: (5, 6),
        });
        assert_eq!(layer.state().held, Some((5.0, 6.0)), "au repos, l'ordinateur reprend la main");
        layer.set_host_size((200.0, 100.0));
        assert_eq!(layer.state().held, Some((10.0, 12.0)), "place relative gardée");
        assert_eq!(sent.lock().unwrap().last(), Some(&(10, 12)));
        layer.resend_held();
        assert_eq!(sent.lock().unwrap().len(), 3);
        layer.reset();
        assert_eq!(layer.state().held, None);
        layer.resend_held();
        assert_eq!(sent.lock().unwrap().len(), 3, "rien de tenu : rien d'envoyé");
    }

    #[test]
    fn souris_auto_sur_tous_les_ecrans() {
        let clock = Arc::new(ManualClock::new());
        let a = PointerLayer::new(clock.clone(), false);
        let b = PointerLayer::new(clock.clone(), false);
        let c = PointerLayer::new(clock.clone(), false);
        let hidden = |l: &PointerLayer, visible: bool| l.remote_cursor_changed(RemoteCursor { visible, image: None, position: (1, 1) });
        clock.advance(Duration::from_secs(10));
        // Aucune annonce : visible par défaut, comme une session à un écran.
        let s = PointerLayer::summary([&a, &b, &c]);
        assert!(!s.hidden && s.visible_for == Duration::from_secs(10));
        // Le jeu cache son pointeur sur l'écran principal ; les deux autres ne l'ont jamais vu : caché.
        hidden(&a, false);
        assert!(PointerLayer::summary([&a, &b, &c]).hidden, "un écran jamais visité ne compte pas");
        // Le pointeur est passé sur l'écran b et y est visible : pas caché.
        hidden(&b, true);
        assert!(!PointerLayer::summary([&a, &b, &c]).hidden);
        // Il quitte b (annoncé caché là-bas) : caché partout où il est passé.
        hidden(&b, false);
        let s = PointerLayer::summary([&a, &b, &c]);
        assert!(s.hidden && s.visible_for == Duration::ZERO);
        // Remontré sur a : visible depuis ce moment.
        hidden(&a, true);
        clock.advance(Duration::from_millis(300));
        let s = PointerLayer::summary([&a, &b, &c]);
        assert!(!s.hidden && s.visible_for == Duration::from_millis(300));
        assert_eq!(PointerLayer::summary([]), PointerSummary { hidden: false, visible_for: Duration::ZERO });
    }

    #[test]
    fn echelle_et_cadre() {
        assert_eq!(PointerLayer::scale((32, 32), 32.0, 1.0), 1.0);
        assert_eq!(PointerLayer::scale((64, 64), 32.0, 1.0), 0.5, "Windows à 200 % : ramené à 32 points");
        assert_eq!(PointerLayer::scale((16, 16), 48.0, 2.0), 2.0, "Apple TV : agrandi deux fois au plus");
        let f = PointerLayer::frame(&shape(10, 20), (100.0, 100.0), 2.0, None);
        assert_eq!(f, Frame { x: 98.0, y: 96.0, width: 20.0, height: 40.0 });
        let f = PointerLayer::frame(&shape(10, 20), (100.0, 100.0), 2.0, Some(500.0));
        assert_eq!(f, Frame { x: 98.0, y: 364.0, width: 20.0, height: 40.0 }, "y vers le haut sur Mac");
        let s = Shape {
            width: 1,
            height: 1,
            argb: vec![0x80FF_FF00].into(),
            hot_x: 0,
            hot_y: 0,
        };
        assert_eq!(s.premultiplied(), [0x8080_8000]);
    }
}
