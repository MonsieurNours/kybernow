//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : quelle
//! session est active, lesquelles gardent le son. Une seule source pour toute
//! l'app : les fenêtres disent seulement laquelle passe au premier plan
//! (`activate`), la barre et le menu de l'icône disent « Garder le son »
//! (`set_keep_sound`).
//!
//! **Le point d'application unique** : `apply`, appelé à chaque changement
//! (session ouverte ou fermée, fenêtre au premier plan, « Garder le son »),
//! calcule pour chaque session « active » et « muette » et appelle une seule
//! méthode par session, [`Focusable::apply_focus`] (`App` de `window.rs`).
//! Second temps : le registre `focus` du cœur prend cette place (mêmes
//! entrées : `register`, `unregister`, `activate`, `set_keep_sound`), et
//! `apply_focus` lui passe la main pour les entrées, les manettes, le
//! presse-papiers et le son.
//!
//! La session active est celle de la dernière fenêtre de session passée au
//! premier plan : une autre app (ou l'accueil) devant ne la change pas, elle
//! garde le son. Une seule session : toujours active, jamais muette (comme
//! avant). Sans rien de Windows : se teste sur le Mac.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Quatre ordinateurs au plus en même temps (docs/plusieurs-ordinateurs.md).
pub const MAX_SESSIONS: usize = 4;

/// Une session vue par le registre.
pub trait Focusable: Send + Sync {
    /// La seule méthode appelée par session, à chaque changement : `active`,
    /// elle reçoit le clavier, la souris, les manettes et le presse-papiers ;
    /// `muted`, on ne l'entend pas (inactive sans « Garder le son »).
    fn apply_focus(&self, active: bool, muted: bool);
}

/// Ce que `apply` donne à une session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusState {
    pub id: u64,
    pub active: bool,
    pub muted: bool,
}

struct Entry {
    id: u64,
    keep_sound: bool,
    target: Weak<dyn Focusable>,
}

/// Les sessions dans l'ordre d'ouverture, l'active et leur « Garder le son ».
pub struct Registry {
    entries: Vec<Entry>,
    active: Option<u64>,
}

impl Registry {
    pub const fn new() -> Self {
        Self { entries: Vec::new(), active: None }
    }

    /// Une session de plus ; la première devient active (les suivantes le
    /// deviennent quand leur fenêtre passe au premier plan).
    pub fn register(&mut self, id: u64, target: Weak<dyn Focusable>) {
        if self.entries.iter().any(|e| e.id == id) {
            return;
        }
        self.entries.push(Entry { id, keep_sound: false, target });
        if self.active.is_none() {
            self.active = Some(id);
        }
    }

    /// Une session finie ; si c'était l'active, la dernière ouverte qui reste
    /// le devient (sa fenêtre la confirme en passant au premier plan).
    pub fn unregister(&mut self, id: u64) {
        self.entries.retain(|e| e.id != id);
        if self.active == Some(id) {
            self.active = self.entries.last().map(|e| e.id);
        }
    }

    /// Rend vrai si l'active a changé.
    pub fn activate(&mut self, id: u64) -> bool {
        if self.active == Some(id) || !self.entries.iter().any(|e| e.id == id) {
            return false;
        }
        self.active = Some(id);
        true
    }

    /// Rend vrai si quelque chose a changé.
    pub fn set_keep_sound(&mut self, id: u64, on: bool) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) if e.keep_sound != on => {
                e.keep_sound = on;
                true
            }
            _ => false,
        }
    }

    pub fn keep_sound(&self, id: u64) -> bool {
        self.entries.iter().any(|e| e.id == id && e.keep_sound)
    }

    pub fn active(&self) -> Option<u64> {
        self.active
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Pour chaque session : active ou non, muette ou non.
    pub fn states(&self) -> Vec<FocusState> {
        self.entries
            .iter()
            .map(|e| {
                let active = self.active == Some(e.id);
                FocusState { id: e.id, active, muted: !active && !e.keep_sound }
            })
            .collect()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());
/// Tenu pendant `apply` : deux changements simultanés s'appliquent l'un après
/// l'autre, le dernier état l'emporte toujours.
static APPLYING: Mutex<()> = Mutex::new(());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// L'app veut savoir quand l'active ou « Garder le son » change (menu de
/// l'icône, barre).
static LISTENER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Identifiant d'une nouvelle session.
pub fn new_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn register<T: Focusable + 'static>(id: u64, target: &Arc<T>) {
    let weak: Weak<dyn Focusable> = Arc::downgrade(target) as Weak<dyn Focusable>;
    REGISTRY.lock().unwrap().register(id, weak);
    apply();
}

pub fn unregister(id: u64) {
    REGISTRY.lock().unwrap().unregister(id);
    apply();
}

/// Une fenêtre de cette session passe au premier plan.
pub fn activate(id: u64) {
    let changed = REGISTRY.lock().unwrap().activate(id);
    if changed {
        log::info!("session active : {id}");
        apply();
    }
}

/// « Garder le son » de la barre ou du menu de l'icône.
pub fn set_keep_sound(id: u64, on: bool) {
    let changed = REGISTRY.lock().unwrap().set_keep_sound(id, on);
    if changed {
        apply();
    }
}

pub fn keep_sound(id: u64) -> bool {
    REGISTRY.lock().unwrap().keep_sound(id)
}

pub fn active() -> Option<u64> {
    REGISTRY.lock().unwrap().active()
}

pub fn on_change(f: impl Fn() + Send + Sync + 'static) {
    let _ = LISTENER.set(Box::new(f));
}

/// Le point d'application unique (voir l'en-tête).
fn apply() {
    let _turn = APPLYING.lock().unwrap();
    let targets: Vec<(Arc<dyn Focusable>, FocusState)> = {
        let r = REGISTRY.lock().unwrap();
        r.entries.iter().zip(r.states()).filter_map(|(e, s)| e.target.upgrade().map(|t| (t, s))).collect()
    };
    for (target, s) in targets {
        target.apply_focus(s.active, s.muted);
    }
    drop(_turn);
    if let Some(f) = LISTENER.get() {
        f();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe;
    impl Focusable for Probe {
        fn apply_focus(&self, _active: bool, _muted: bool) {}
    }

    fn reg(ids: &[u64]) -> (Registry, Arc<Probe>) {
        let p = Arc::new(Probe);
        let mut r = Registry::new();
        for &id in ids {
            r.register(id, Arc::downgrade(&p) as Weak<dyn Focusable>);
        }
        (r, p)
    }

    fn summary(r: &Registry) -> Vec<(u64, bool, bool)> {
        r.states().iter().map(|s| (s.id, s.active, s.muted)).collect()
    }

    #[test]
    fn une_seule_session_toujours_active_et_audible() {
        let (r, _p) = reg(&[1]);
        assert_eq!(summary(&r), [(1, true, false)]);
    }

    #[test]
    fn la_suivante_attend_sa_fenetre() {
        let (mut r, _p) = reg(&[1, 2]);
        // Ouverte, pas encore au premier plan : muette.
        assert_eq!(summary(&r), [(1, true, false), (2, false, true)]);
        assert!(r.activate(2));
        assert!(!r.activate(2));
        assert_eq!(summary(&r), [(1, false, true), (2, true, false)]);
        // Une session inconnue ne prend pas la place.
        assert!(!r.activate(9));
        assert_eq!(r.active(), Some(2));
    }

    #[test]
    fn garder_le_son() {
        let (mut r, _p) = reg(&[1, 2, 3]);
        assert!(r.set_keep_sound(2, true));
        assert!(!r.set_keep_sound(2, true));
        assert_eq!(summary(&r), [(1, true, false), (2, false, false), (3, false, true)]);
        // Active, « Garder le son » ne change rien ; il reste pour la suite.
        r.activate(2);
        assert_eq!(summary(&r), [(1, false, true), (2, true, false), (3, false, true)]);
        r.activate(3);
        assert!(r.keep_sound(2));
        assert_eq!(summary(&r), [(1, false, true), (2, false, false), (3, true, false)]);
    }

    #[test]
    fn fin_de_l_active() {
        let (mut r, _p) = reg(&[1, 2, 3]);
        r.activate(1);
        r.unregister(1);
        // La dernière ouverte prend la place.
        assert_eq!(r.active(), Some(3));
        r.unregister(2);
        assert_eq!(summary(&r), [(3, true, false)]);
        r.unregister(3);
        assert_eq!(r.active(), None);
        assert_eq!(r.len(), 0);
        // Une fin d'une autre que l'active ne change pas l'active.
        let (mut r, _p) = reg(&[1, 2]);
        r.unregister(2);
        assert_eq!(r.active(), Some(1));
    }
}
