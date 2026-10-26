//! File d'envoi vers kynput, et file de secours des paquets qui ne se
//! rattrapent pas. Partagée par toutes les entrées produites par l'app ;
//! les manettes lues par SDL (Mac, Windows) n'y passent pas, kynput les
//! envoie lui-même.
//!
//! La file d'envoi de kynput tient 64 paquets et jette sans retour (le
//! protocole est en deltas : un relâchement jeté = touche collée jusqu'à la
//! fin de la session). `kynput_consumer_consume` rend -1 quand elle est
//! pleine, ou tant que le réseau n'est pas relié : on compte les refus, et
//! les paquets qui ne se rattrapent pas sont réessayés depuis une petite
//! file de secours, toutes les 3 ms, dans l'ordre, jusqu'à succès ou 200 ms.
//!
//! Les réessais sont faits par [`PacketQueue::poll`], à appeler quand
//! [`PacketQueue::next_deadline`] est passée (le fil des entrées de
//! [`super::pipeline::InputPipeline`] s'en charge).

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::clock::Clock;
use super::packet::{OwnedPacket, PacketSink};

/// Un paquet qui ne se rattrape pas est réessayé pendant ce temps au plus.
pub const RETRY_WINDOW: Duration = Duration::from_millis(200);
/// Pas des réessais.
pub const RETRY_STEP: Duration = Duration::from_millis(3);

/// Premier envoi d'une sorte (`"touche"`, `"clic"`…) et s'il est parti.
pub type FirstHook = Box<dyn Fn(&'static str, bool) + Send + Sync>;
/// Un refus (`final` : perdu après les réessais), avec le total des pertes.
pub type DroppedHook = Box<dyn Fn(&'static str, bool, usize) + Send + Sync>;

/// Journal d'un premier envoi par sorte, et des refus.
pub struct Hooks {
    pub on_first: Option<FirstHook>,
    pub on_dropped: Option<DroppedHook>,
}

struct Pending {
    kind: &'static str,
    packet: OwnedPacket,
    deadline: Duration,
}

struct Inner {
    backlog: VecDeque<Pending>,
    /// Prochain réessai prévu (`None` : rien n'attend).
    retry_at: Option<Duration>,
    dropped: usize,
    logged: HashSet<&'static str>,
    stopped: bool,
}

pub struct PacketQueue {
    sink: Arc<dyn PacketSink>,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
    hooks: Mutex<Hooks>,
}

impl PacketQueue {
    pub fn new(sink: Arc<dyn PacketSink>, clock: Arc<dyn Clock>) -> Self {
        Self {
            sink,
            clock,
            inner: Mutex::new(Inner {
                backlog: VecDeque::new(),
                retry_at: None,
                dropped: 0,
                logged: HashSet::new(),
                stopped: false,
            }),
            hooks: Mutex::new(Hooks {
                on_first: None,
                on_dropped: None,
            }),
        }
    }

    pub fn set_hooks(&self, hooks: Hooks) {
        *self.hooks.lock().unwrap() = hooks;
    }

    /// Plus rien ne part ; la file de secours est vidée. Rend le total des
    /// paquets perdus pendant la session.
    pub fn stop(&self) -> usize {
        let mut inner = self.inner.lock().unwrap();
        inner.stopped = true;
        inner.backlog.clear();
        inner.retry_at = None;
        inner.dropped
    }

    /// Des paquets attendent encore un réessai.
    pub fn has_backlog(&self) -> bool {
        !self.inner.lock().unwrap().backlog.is_empty()
    }

    /// Paquets refusés par kynput, jamais arrivés.
    pub fn dropped(&self) -> usize {
        self.inner.lock().unwrap().dropped
    }

    /// Quand appeler [`Self::poll`] au plus tard (`None` : rien n'attend).
    pub fn next_deadline(&self) -> Option<Duration> {
        self.inner.lock().unwrap().retry_at
    }

    /// Paquet qui se rattrape tout seul au suivant (position, déplacement,
    /// molette, axe en mouvement) : envoyé une fois, perdu s'il est refusé.
    /// Derrière une file de secours non vide, il n'est pas envoyé non plus :
    /// il doublerait ce qui attend (le retour au repos d'un axe, puis sa
    /// nouvelle valeur). `false` : perdu. Appelable de tout fil.
    pub fn send(&self, packet: OwnedPacket, kind: &'static str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.stopped || !inner.backlog.is_empty() {
            drop(packet);
            if !inner.stopped {
                self.count_dropped(&mut inner, kind, false);
            }
            return false;
        }
        let sent = self.sink.consume(packet);
        self.log_first(&mut inner, kind, sent);
        if !sent {
            self.count_dropped(&mut inner, kind, false);
        }
        sent
    }

    /// Paquet qui ne se rattrape pas (touche, clic, bouton, branchement,
    /// retour au repos d'un axe, FlushKeys) : réessayé toutes les 3 ms, dans
    /// l'ordre, jusqu'à succès ou 200 ms. Tant que la file de secours n'est
    /// pas vide, tout y passe : un relâchement ne doit pas doubler son appui.
    pub fn send_reliably(&self, packet: OwnedPacket, kind: &'static str) {
        let mut inner = self.inner.lock().unwrap();
        if inner.stopped {
            return;
        }
        if inner.backlog.is_empty() {
            let sent = self.sink.consume(packet.clone_ref());
            self.log_first(&mut inner, kind, sent);
            if sent {
                return;
            }
        }
        let deadline = self.clock.now() + RETRY_WINDOW;
        inner.backlog.push_back(Pending {
            kind,
            packet,
            deadline,
        });
        self.schedule_retry(&mut inner);
    }

    /// Réessaie ce qui attend, dans l'ordre, jusqu'au premier refus.
    pub fn poll(&self) {
        let mut inner = self.inner.lock().unwrap();
        let Some(at) = inner.retry_at else { return };
        let now = self.clock.now();
        if now < at {
            return;
        }
        inner.retry_at = None;
        if inner.stopped {
            inner.backlog.clear();
            return;
        }
        while let Some(next) = inner.backlog.front() {
            if self.sink.consume(next.packet.clone_ref()) {
                inner.backlog.pop_front();
                continue;
            }
            if now > next.deadline {
                let kind = next.kind;
                inner.backlog.pop_front();
                self.count_dropped(&mut inner, kind, true);
                continue;
            }
            self.schedule_retry(&mut inner);
            return;
        }
    }

    // Sous le verrou.
    fn schedule_retry(&self, inner: &mut Inner) {
        if inner.retry_at.is_none() {
            inner.retry_at = Some(self.clock.now() + RETRY_STEP);
        }
    }

    // Sous le verrou.
    fn log_first(&self, inner: &mut Inner, kind: &'static str, sent: bool) {
        if inner.logged.insert(kind)
            && let Some(hook) = &self.hooks.lock().unwrap().on_first
        {
            hook(kind, sent);
        }
    }

    // Sous le verrou.
    fn count_dropped(&self, inner: &mut Inner, kind: &'static str, final_: bool) {
        inner.dropped += 1;
        if (final_ || inner.dropped == 1 || inner.dropped.is_multiple_of(100))
            && let Some(hook) = &self.hooks.lock().unwrap().on_dropped
        {
            hook(kind, final_, inner.dropped);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use kynput_rs::{InputTarget, KeyboardKey, MousePosition};

    use super::*;
    use crate::input::clock::ManualClock;

    /// Un ordinateur qui refuse à volonté et note ce qu'il reçoit.
    #[derive(Default)]
    pub(crate) struct Host {
        /// Refus restants avant d'accepter.
        pub refuse: Mutex<usize>,
        pub refuse_all: Mutex<bool>,
        pub received: Mutex<Vec<String>>,
        pub consumed: Mutex<usize>,
    }

    impl Host {
        pub fn received(&self) -> Vec<String> {
            self.received.lock().unwrap().clone()
        }

        pub fn clear(&self) {
            self.received.lock().unwrap().clear();
        }

        pub fn refuse(&self, n: usize) {
            *self.refuse.lock().unwrap() = n;
        }

        pub fn refuse_all(&self, on: bool) {
            *self.refuse_all.lock().unwrap() = on;
        }
    }

    impl PacketSink for Host {
        fn consume(&self, packet: OwnedPacket) -> bool {
            *self.consumed.lock().unwrap() += 1;
            let mut refuse = self.refuse.lock().unwrap();
            if *self.refuse_all.lock().unwrap() || *refuse > 0 {
                *refuse = refuse.saturating_sub(1);
                return false;
            }
            self.received.lock().unwrap().push(describe(&packet));
            true
        }
    }

    /// Ce que dit un paquet, en clair.
    pub(crate) fn describe(packet: &OwnedPacket) -> String {
        use kynput_rs::{GamepadEvent, InputType, MouseButton, Unicode};
        use kynput_sys as sys;
        let p = packet.borrow();
        let state = |pressed: bool, f: &str, m: &str| if pressed { format!("appuyé{f}") } else { format!("relâché{m}") };
        match p.get_type() {
            InputType::Keyboard => {
                let key = KeyboardKey::from_packet(&p).unwrap();
                format!("touche {:#x} {}", key.scancode, state(key.pressed, "e", "e"))
            }
            InputType::MousePosition => {
                let m = MousePosition::from_packet(p).unwrap();
                if m.display_id == 1 { format!("position {}", m.x) } else { format!("position {} sur {}", m.x, m.display_id) }
            }
            InputType::MouseButton => {
                let b = MouseButton::from_packet(&p).unwrap();
                format!("clic {:?} {}", b.button, state(b.pressed, "", ""))
            }
            InputType::Unicode => format!("texte {}", Unicode::from_packet(&p).unwrap().character),
            InputType::ControlEvent => "relâchement général".into(),
            InputType::GamepadControl => format!("{:?}", kynput_rs::GamepadControl::from_packet(&p).unwrap()),
            InputType::GamepadEvent => {
                if let Some(GamepadEvent::Button { id, type_, pressed }) = GamepadEvent::from_packet(&p) {
                    return format!("manette {id} {type_:?} {}", state(pressed, "e", "e"));
                }
                let mut a = sys::kynput_GamepadEventAxis { id: 0, type_: 0, value: 0 };
                assert!(unsafe { sys::kynput_gamepad_axis_from_packet(p.to_ptr(), &mut a) });
                format!("axe {} {} {}", a.id, a.type_, a.value)
            }
            other => format!("{other:?}"),
        }
    }

    pub(crate) fn key(scancode: u16, pressed: bool) -> OwnedPacket {
        OwnedPacket::from_packet(KeyboardKey { pressed, scancode }.to_packet(InputTarget::Host)).unwrap()
    }

    pub(crate) fn position(x: i16) -> OwnedPacket {
        OwnedPacket::from_packet(
            MousePosition {
                display_id: 1,
                x,
                y: 0,
            }
            .to_packet(InputTarget::Host),
        )
        .unwrap()
    }

    struct Bench {
        host: Arc<Host>,
        clock: Arc<ManualClock>,
        queue: PacketQueue,
        firsts: Arc<Mutex<Vec<&'static str>>>,
        finals: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Bench {
        fn new() -> Self {
            let host = Arc::new(Host::default());
            let clock = Arc::new(ManualClock::new());
            let queue = PacketQueue::new(host.clone(), clock.clone());
            let firsts = Arc::new(Mutex::new(Vec::new()));
            let finals = Arc::new(Mutex::new(Vec::new()));
            let (f, d) = (firsts.clone(), finals.clone());
            queue.set_hooks(Hooks {
                on_first: Some(Box::new(move |kind, _| f.lock().unwrap().push(kind))),
                on_dropped: Some(Box::new(move |kind, final_, _| {
                    if final_ {
                        d.lock().unwrap().push(kind)
                    }
                })),
            });
            Self {
                host,
                clock,
                queue,
                firsts,
                finals,
            }
        }

        /// Fait passer le temps par pas de 1 ms en appelant `poll` à chaque pas.
        fn elapse(&self, ms: u64) {
            for _ in 0..ms {
                self.clock.advance(Duration::from_millis(1));
                self.queue.poll();
            }
        }
    }

    #[test]
    fn paquet_accepte_part_tout_de_suite() {
        let b = Bench::new();
        b.queue.send_reliably(key(0x1E, true), "touche");
        assert_eq!(b.host.received(), ["touche 0x1e appuyée"]);
        assert!(!b.queue.has_backlog());
        assert_eq!(b.queue.dropped(), 0);
        assert!(b.queue.send(position(10), "position"));
        assert_eq!(b.host.received().last().unwrap(), "position 10");
        assert_eq!(*b.firsts.lock().unwrap(), ["touche", "position"], "premier envoi de chaque sorte journalisé une fois");
        b.queue.send_reliably(key(0x1E, false), "touche");
        assert_eq!(b.firsts.lock().unwrap().len(), 2);
    }

    #[test]
    fn refuse_deux_fois_puis_parti_dans_l_ordre() {
        let b = Bench::new();
        b.host.refuse(2);
        b.queue.send_reliably(key(0x1E, false), "touche");
        assert!(b.host.received().is_empty() && b.queue.has_backlog(), "refusé : en file de secours");
        b.queue.send_reliably(key(0x2C, true), "clic");
        assert_eq!(*b.host.consumed.lock().unwrap(), 1, "derrière la file de secours, le suivant n'est pas tenté : il attend son tour");
        assert!(!b.queue.send(position(20), "position"), "une position derrière la file de secours est perdue");
        assert_eq!(b.queue.dropped(), 1);
        assert_eq!(b.queue.next_deadline(), Some(Duration::from_millis(3)));
        b.elapse(2);
        assert!(b.host.received().is_empty(), "pas de réessai avant 3 ms");
        b.elapse(1);
        assert!(b.host.received().is_empty(), "premier réessai à 3 ms : second refus");
        assert_eq!(*b.host.consumed.lock().unwrap(), 2);
        assert_eq!(b.queue.next_deadline(), Some(Duration::from_millis(6)), "réessai suivant 3 ms plus tard");
        b.elapse(3);
        assert_eq!(b.host.received(), ["touche 0x1e relâchée", "touche 0x2c appuyée"], "les deux paquets sont partis dans l'ordre");
        assert!(!b.queue.has_backlog());
        assert_eq!(*b.host.consumed.lock().unwrap(), 4, "le paquet refusé est représenté à chaque essai");
        assert!(b.queue.send(position(30), "position") && b.host.received().last().unwrap() == "position 30", "file vide : les positions repartent");
    }

    #[test]
    fn refuse_sans_fin_abandonne_apres_200_ms() {
        let b = Bench::new();
        b.host.refuse_all(true);
        b.queue.send_reliably(key(0x10, true), "touche");
        b.elapse(120);
        assert!(b.queue.has_backlog() && b.finals.lock().unwrap().is_empty(), "toujours réessayé à 120 ms");
        b.elapse(90);
        assert!(!b.queue.has_backlog(), "perdu après 200 ms de réessais");
        assert_eq!(*b.finals.lock().unwrap(), ["touche"], "journalisé comme perdu");
        assert_eq!(b.queue.dropped(), 1);
        assert_eq!(b.queue.next_deadline(), None);
        b.host.refuse_all(false);
    }

    #[test]
    fn arret_plus_rien_ne_part() {
        let b = Bench::new();
        b.host.refuse_all(true);
        b.queue.send_reliably(key(0x11, true), "touche");
        b.host.refuse_all(false);
        b.host.clear();
        assert_eq!(b.queue.stop(), 0, "l'arrêt rend le total des paquets perdus");
        b.queue.send_reliably(key(0x12, true), "touche");
        assert!(!b.queue.send(position(40), "position"));
        b.elapse(10);
        assert!(b.host.received().is_empty() && !b.queue.has_backlog(), "arrêtée : rien n'est envoyé ni gardé");
    }
}
