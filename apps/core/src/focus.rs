//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : le
//! registre des sessions de l'app, seule source de « qui est active ».
//!
//! Les apps Mac, Windows et Linux (interface de bureau) y inscrivent chaque
//! session ([`Focus::register`], quatre au plus) et lui disent seulement
//! quelle fenêtre passe au premier plan ([`Focus::activate`]) et quelles
//! sessions gardent leur son ([`Focus::set_keep_sound`], « Garder le son »).
//! À chaque changement, le registre l'applique à toutes les sessions
//! inscrites :
//!
//! - la session active reçoit le clavier, la souris, les manettes et le
//!   presse-papiers ([`Session::set_active`]) ; les autres ne reçoivent rien,
//!   relâchent ce qu'elles tenaient, mettent leurs manettes au repos (le
//!   lecteur de manettes du cœur, [`crate::gamepads`], ne sert que les
//!   sessions actives) et mettent leur presse-papiers en pause ;
//! - le son de toutes les autres est coupé ([`Session::set_muted`]), sauf
//!   celles qui gardent leur son.
//!
//! Les sessions qui perdent la main passent d'abord en arrière-plan, puis la
//! nouvelle devient active : ce que l'une relâche part avant ce que l'autre
//! reçoit, et le presse-papiers de l'appareil n'est jamais lu pour deux.
//!
//! Une session inscrite devient active, comme sa fenêtre qui s'ouvre
//! par-dessus. Quand la session active quitte le registre, celle qui l'était
//! juste avant reprend la main (la fenêtre que le système remet devant ;
//! l'app corrige par `activate` si besoin). Une app à une session n'a pas
//! besoin du registre : une session seule est active et s'entend.

use std::fmt;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::session::{FocusState, Session};

/// Sessions au plus dans une app (« Quatre ordinateurs au plus en même temps. »).
pub const MAX_SESSIONS: usize = 4;

/// Une session inscrite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FocusId(u64);

impl FocusId {
    /// La valeur passée aux ponts (toujours positive).
    pub fn as_u64(self) -> u64 {
        self.0
    }

    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }
}

/// Inscription refusée : déjà [`MAX_SESSIONS`] sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

impl fmt::Display for Full {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{MAX_SESSIONS} sessions au plus en même temps")
    }
}

impl std::error::Error for Full {}

struct Entry {
    id: FocusId,
    state: Weak<FocusState>,
    keep_sound: bool,
}

#[derive(Default)]
struct Inner {
    next: u64,
    /// Dans l'ordre d'inscription (celui de « Ordinateur suivant »).
    entries: Vec<Entry>,
    /// Les sessions dans l'ordre où elles ont eu la main (la dernière en dernier).
    recent: Vec<FocusId>,
    active: Option<FocusId>,
}

impl Inner {
    /// Oublie les sessions disparues sans s'être désinscrites ; s'il y en
    /// avait, applique le nouvel état (une autre a pu reprendre la main).
    fn prune(&mut self) {
        let gone: Vec<FocusId> = self.entries.iter().filter(|e| e.state.strong_count() == 0).map(|e| e.id).collect();
        if gone.is_empty() {
            return;
        }
        for id in gone {
            self.remove(id);
        }
        self.apply();
    }

    fn remove(&mut self, id: FocusId) -> Option<Entry> {
        let i = self.entries.iter().position(|e| e.id == id)?;
        let entry = self.entries.remove(i);
        self.recent.retain(|r| *r != id);
        if self.active == Some(id) {
            self.active = self.recent.last().copied();
        }
        Some(entry)
    }

    fn touch(&mut self, id: FocusId) {
        self.recent.retain(|r| *r != id);
        self.recent.push(id);
        self.active = Some(id);
    }

    /// Applique « active » et « son coupé » à toutes les sessions : celles
    /// qui perdent la main d'abord, puis la session active.
    fn apply(&self) {
        let mut active = None;
        for e in &self.entries {
            let Some(state) = e.state.upgrade() else { continue };
            if Some(e.id) == self.active {
                active = Some(state);
                continue;
            }
            state.set_active(false);
            state.set_muted(!e.keep_sound);
        }
        if let Some(state) = active {
            state.set_active(true);
            state.set_muted(false);
        }
    }
}

/// Le registre (un par processus : [`global`] ; les tests en créent d'autres).
#[derive(Default)]
pub struct Focus {
    inner: Mutex<Inner>,
}

impl Focus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inscrit une session, `name` : le nom de son ordinateur (journal). Elle
    /// devient active. Une session déjà inscrite garde son identifiant.
    /// [`Full`] au-delà de [`MAX_SESSIONS`].
    pub fn register(&self, session: &Session, name: &str) -> Result<FocusId, Full> {
        self.register_state(&session.focus_state(), name)
    }

    pub(crate) fn register_state(&self, state: &Arc<FocusState>, name: &str) -> Result<FocusId, Full> {
        let mut inner = self.inner.lock().unwrap();
        inner.prune();
        if let Some(e) = inner.entries.iter().find(|e| e.state.as_ptr() == Arc::as_ptr(state)) {
            return Ok(e.id);
        }
        if inner.entries.len() >= MAX_SESSIONS {
            log::warn!("ordinateurs : {name} refusé, déjà {MAX_SESSIONS} sessions");
            return Err(Full);
        }
        state.label().set(name);
        inner.next += 1;
        let id = FocusId(inner.next);
        inner.entries.push(Entry { id, state: Arc::downgrade(state), keep_sound: false });
        inner.touch(id);
        log::info!("ordinateurs : {name} ouvert ({} session(s))", inner.entries.len());
        inner.apply();
        Ok(id)
    }

    /// Désinscrit une session (fermeture de sa fenêtre, « Se déconnecter »,
    /// fin de session) : elle passe en arrière-plan, son son est coupé ; si
    /// elle avait la main, la session qui l'avait juste avant la reprend.
    pub fn unregister(&self, id: FocusId) {
        let mut inner = self.inner.lock().unwrap();
        let Some(entry) = inner.remove(id) else { return };
        if let Some(state) = entry.state.upgrade() {
            if !inner.entries.is_empty() {
                state.set_active(false);
                state.set_muted(true);
            }
            log::info!("ordinateurs : {} fermé ({} session(s))", state.label().name(), inner.entries.len());
        }
        inner.prune();
        inner.apply();
    }

    /// La fenêtre de cette session passe au premier plan : elle devient la
    /// session active. Sans effet pour une session inconnue.
    pub fn activate(&self, id: FocusId) {
        let mut inner = self.inner.lock().unwrap();
        inner.prune();
        if !inner.entries.iter().any(|e| e.id == id) || inner.active == Some(id) {
            return;
        }
        inner.touch(id);
        inner.apply();
    }

    /// « Garder le son » : la session reste audible même quand elle n'est pas
    /// active (coupé par défaut, pour la session en cours).
    pub fn set_keep_sound(&self, id: FocusId, keep: bool) {
        let mut inner = self.inner.lock().unwrap();
        let Some(e) = inner.entries.iter_mut().find(|e| e.id == id) else { return };
        if e.keep_sound == keep {
            return;
        }
        e.keep_sound = keep;
        inner.apply();
    }

    pub fn keep_sound(&self, id: FocusId) -> bool {
        self.inner.lock().unwrap().entries.iter().find(|e| e.id == id).is_some_and(|e| e.keep_sound)
    }

    /// La session active.
    pub fn active(&self) -> Option<FocusId> {
        self.inner.lock().unwrap().active
    }

    /// Sessions inscrites.
    pub fn count(&self) -> usize {
        let mut inner = self.inner.lock().unwrap();
        inner.prune();
        inner.entries.len()
    }

    /// Les sessions, dans l'ordre d'ouverture.
    pub fn ids(&self) -> Vec<FocusId> {
        self.inner.lock().unwrap().entries.iter().map(|e| e.id).collect()
    }

    /// « Ordinateur suivant » : la session ouverte après l'active (la première
    /// après la dernière). À la plateforme de faire passer sa fenêtre devant.
    pub fn next(&self) -> Option<FocusId> {
        let inner = self.inner.lock().unwrap();
        let ids: Vec<FocusId> = inner.entries.iter().map(|e| e.id).collect();
        let at = inner.active.and_then(|a| ids.iter().position(|i| *i == a));
        match at {
            Some(i) => ids.get((i + 1) % ids.len()).copied(),
            None => ids.first().copied(),
        }
    }

    /// Le nom d'ordinateur donné à l'inscription.
    pub fn name(&self, id: FocusId) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.entries.iter().find(|e| e.id == id).and_then(|e| e.state.upgrade()).map(|s| s.label().name())
    }
}

/// Le registre du processus.
pub fn global() -> &'static Focus {
    static GLOBAL: OnceLock<Focus> = OnceLock::new();
    GLOBAL.get_or_init(Focus::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioEngine, ManualClock};
    use crate::gamepads::Gamepads;
    use crate::input::packet_queue::tests::Host;
    use crate::input::{InputPipeline, InputPlatform, Options, PadAxis, PadButton, PointerLayer};

    struct Quiet;
    impl InputPlatform for Quiet {}

    /// Une session sans réseau : son état, sa chaîne d'entrées, son moteur de son.
    struct Fake {
        state: Arc<FocusState>,
        input: Arc<InputPipeline>,
        host: Arc<Host>,
        audio: Arc<AudioEngine>,
    }

    fn fake(clock: &Arc<ManualClock>) -> Fake {
        let state = FocusState::new();
        let host = Arc::new(Host::default());
        let input = InputPipeline::new_with_sink(
            Options { display_id: 1, host_size: (1920, 1080), clipboard: None, gamepads_from_app: true },
            Arc::new(PointerLayer::new(clock.clone(), true)),
            Arc::new(Quiet),
            clock.clone(),
            host.clone(),
        );
        state.attach_input(&input);
        let audio = Arc::new(AudioEngine::new(clock.clone()));
        state.attach_audio(&audio);
        Fake { state, input, host, audio }
    }

    #[test]
    fn inscription_activation_et_son() {
        let clock = Arc::new(ManualClock::new());
        let focus = Focus::new();
        let (a, b) = (fake(&clock), fake(&clock));
        let ia = focus.register_state(&a.state, "PC-DE-MATTHIEU").unwrap();
        assert_eq!(focus.active(), Some(ia));
        assert!(a.state.is_active() && !a.state.is_muted(), "seule : active, on l'entend");
        assert_eq!(a.state.label().name(), "PC-DE-MATTHIEU", "son nom au journal");
        let ib = focus.register_state(&b.state, "PC-ATELIER").unwrap();
        assert_eq!(focus.active(), Some(ib), "la nouvelle fenêtre s'ouvre par-dessus et devient active");
        assert!(!a.input.is_active() && a.audio.is_muted(), "l'autre ne reçoit plus rien, et se tait");
        assert!(b.input.is_active() && !b.audio.is_muted());
        assert_eq!(focus.register_state(&b.state, "PC-ATELIER"), Ok(ib), "déjà inscrite : même identifiant");
        assert_eq!(focus.count(), 2);
        assert_eq!(focus.ids(), [ia, ib], "ordre d'ouverture");
        focus.activate(ia);
        assert!(a.input.is_active() && !a.audio.is_muted());
        assert!(!b.input.is_active() && b.audio.is_muted());
        assert_eq!(focus.next(), Some(ib), "ordinateur suivant");
        focus.activate(ib);
        assert_eq!(focus.next(), Some(ia), "après le dernier, le premier");
        assert_eq!(focus.name(ia).as_deref(), Some("PC-DE-MATTHIEU"));
    }

    #[test]
    fn garder_le_son() {
        let clock = Arc::new(ManualClock::new());
        let focus = Focus::new();
        let (a, b) = (fake(&clock), fake(&clock));
        let ia = focus.register_state(&a.state, "A").unwrap();
        let ib = focus.register_state(&b.state, "B").unwrap();
        assert!(a.audio.is_muted());
        assert!(!focus.keep_sound(ia), "coupé par défaut");
        focus.set_keep_sound(ia, true);
        assert!(focus.keep_sound(ia));
        assert!(!a.audio.is_muted(), "« Garder le son » : audible sans être active");
        assert!(!a.input.is_active(), "mais ne reçoit toujours rien");
        focus.activate(ia);
        assert!(b.audio.is_muted(), "l'autre, sans « Garder le son », se tait");
        focus.activate(ib);
        assert!(!a.audio.is_muted(), "toujours gardé");
        focus.set_keep_sound(ia, false);
        assert!(a.audio.is_muted());
        assert!(!focus.keep_sound(FocusId(999)), "session inconnue");
    }

    #[test]
    fn quatre_au_plus_et_desinscription() {
        let clock = Arc::new(ManualClock::new());
        let focus = Focus::new();
        let fakes: Vec<Fake> = (0..5).map(|_| fake(&clock)).collect();
        let ids: Vec<FocusId> = fakes[..4].iter().map(|f| focus.register_state(&f.state, "x").unwrap()).collect();
        assert_eq!(focus.register_state(&fakes[4].state, "cinquième"), Err(Full), "quatre ordinateurs au plus");
        assert!(fakes[4].state.label().name().is_empty(), "refusée : rien n'a changé pour elle");
        // La dernière ouverte a la main ; avant elle, la première.
        focus.activate(ids[0]);
        focus.activate(ids[3]);
        focus.unregister(ids[3]);
        assert_eq!(focus.active(), Some(ids[0]), "celle qui avait la main juste avant la reprend");
        assert!(fakes[0].input.is_active() && !fakes[0].audio.is_muted());
        assert!(!fakes[3].input.is_active() && fakes[3].audio.is_muted(), "la session fermée ne reçoit plus rien");
        assert_eq!(focus.count(), 3);
        assert!(focus.register_state(&fakes[4].state, "cinquième").is_ok(), "une place libérée");
        focus.unregister(FocusId(999));
        for id in focus.ids() {
            focus.unregister(id);
        }
        assert_eq!(focus.count(), 0);
        assert_eq!(focus.active(), None);
        assert!(fakes[4].input.is_active() && !fakes[4].audio.is_muted(), "la dernière fermée garde son état d'une session seule");
    }

    #[test]
    fn session_disparue_oubliee() {
        let clock = Arc::new(ManualClock::new());
        let focus = Focus::new();
        let a = fake(&clock);
        let ia = focus.register_state(&a.state, "A").unwrap();
        {
            let b = fake(&clock);
            focus.register_state(&b.state, "B").unwrap();
        }
        assert_eq!(focus.count(), 1, "session libérée sans désinscription : oubliée");
        focus.activate(ia);
        assert!(a.input.is_active());
    }

    #[test]
    fn manettes_et_clavier_suivent_la_session_active() {
        let clock = Arc::new(ManualClock::new());
        let focus = Focus::new();
        let pads = Gamepads::new();
        let (a, b) = (fake(&clock), fake(&clock));
        pads.attach(&a.input);
        pads.attach(&b.input);
        let ia = focus.register_state(&a.state, "A").unwrap();
        let ib = focus.register_state(&b.state, "B").unwrap();
        pads.plug(0, None, None);
        focus.activate(ia);
        a.host.clear();
        b.host.clear();
        pads.button(0, PadButton::A, true);
        a.input.key(0x1E, true);
        b.input.key(0x1F, true);
        assert_eq!(a.host.received(), ["manette 0 A appuyée", "touche 0x1e appuyée"]);
        assert!(b.host.received().is_empty(), "rien pour la session en arrière-plan, même si l'app se trompe de fenêtre");
        focus.activate(ib);
        let released = a.host.received();
        assert!(released.contains(&"touche 0x1e relâchée".to_string()), "touche relâchée en passant derrière : {released:?}");
        assert!(released.contains(&"manette 0 A relâchée".to_string()));
        assert_eq!(released.iter().filter(|r| r.starts_with("axe 0 ")).count(), PadAxis::ALL.len(), "axes au repos");
        pads.button(0, PadButton::B, true);
        assert_eq!(b.host.received(), ["manette 0 B appuyée"], "la manette suit, sans être rebranchée");
    }
}
