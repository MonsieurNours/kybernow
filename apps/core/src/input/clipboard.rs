//! Presse-papiers partagé (texte), dans les deux sens.
//!
//! Protocole de kynput : un côté annonce « du nouveau » (Change), l'autre le
//! demande (DataRequest) et reçoit le texte (Data, 60 Ko au plus). L'appareil
//! demande tout de suite ce que l'ordinateur annonce, pour que ⌘V ou Ctrl+V
//! marche aussitôt ; il annonce ce qui change dans son presse-papiers (le
//! système ne prévient pas : on regarde deux fois par seconde) et répond aux
//! demandes. Fonctions C : kyber-patches/0002-kynput.
//!
//! Le presse-papiers lui-même est celui de la plateforme ([`Pasteboard`] :
//! NSPasteboard sur Mac, UIPasteboard sur iOS, rien sur tvOS, celui de
//! Windows si l'app ne laisse pas kynput s'en charger). Les paquets de
//! l'ordinateur arrivent par [`ClipboardBridge::received`], hors du verrou du
//! routeur kynput (`InputPipeline` les met en attente et les traite sur son
//! fil des entrées) : les réponses repartent par `send` sans réentrer dans
//! le routeur. Le presse-papiers est donc lu et écrit depuis ce fil-là.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kynput_sys as sys;

use super::clock::Clock;
use super::packet::{HOST, OwnedPacket};

/// Presse-papiers de la plateforme, texte seulement.
pub trait Pasteboard: Send + Sync {
    /// Compteur qui change à chaque écriture, par l'app ou par une autre.
    fn change_count(&self) -> u64;
    /// Texte présent, `None` s'il n'y en a pas.
    fn text(&self) -> Option<String>;
    /// Remplace le contenu par ce texte.
    fn set_text(&self, text: &str);
    /// Du texte est là, sans le lire (iOS : `hasStrings` ne demande rien).
    fn has_text(&self) -> bool {
        self.text().is_some()
    }
    /// Lire le texte fait demander l'accord de l'utilisateur (iOS : « Autoriser
    /// à coller ? », qui fait aussi perdre le premier plan à l'app). Le texte
    /// n'est alors lu que quand l'ordinateur le demande : rien n'est annoncé
    /// au départ, et rien n'est relu avant d'écrire.
    fn reading_asks_user(&self) -> bool {
        false
    }
}

/// Taille du texte au-delà de laquelle kynput tronque (`MAX_CLIPBOARD_SIZE`).
pub const MAX_TEXT: usize = 60 * 1024;
/// Le presse-papiers de l'appareil est regardé deux fois par seconde.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Envoi d'un paquet vers l'ordinateur.
pub type SendFn = Box<dyn Fn(OwnedPacket) + Send + Sync>;

struct Inner {
    seen_change_count: u64,
    started: bool,
    next_poll: Option<Duration>,
}

pub struct ClipboardBridge {
    board: Arc<dyn Pasteboard>,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
    send: Mutex<Option<SendFn>>,
}

impl ClipboardBridge {
    /// `board` : le presse-papiers général, ou un presse-papiers privé pour les tests.
    pub fn new(board: Arc<dyn Pasteboard>, clock: Arc<dyn Clock>) -> Self {
        let seen = board.change_count();
        Self {
            board,
            clock,
            inner: Mutex::new(Inner {
                seen_change_count: seen,
                started: false,
                next_poll: None,
            }),
            send: Mutex::new(None),
        }
    }

    /// Envoi d'un paquet vers l'ordinateur (appelable de tout fil).
    pub fn set_send(&self, send: impl Fn(OwnedPacket) + Send + Sync + 'static) {
        *self.send.lock().unwrap() = Some(Box::new(send));
    }

    pub fn start(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.started = true;
            inner.next_poll = Some(self.clock.now() + POLL_INTERVAL);
        }
        // Ce que l'appareil a déjà copié est disponible dès le début de la
        // session (sauf là où le lire dérangerait l'utilisateur).
        if !self.board.reading_asks_user() && self.board.has_text() {
            self.announce();
        }
    }

    pub fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.started = false;
        inner.next_poll = None;
    }

    /// Quand appeler [`Self::poll`] au plus tard (`None` : arrêté).
    pub fn next_deadline(&self) -> Option<Duration> {
        self.inner.lock().unwrap().next_poll
    }

    /// Paquet de l'ordinateur, hors du verrou du routeur kynput.
    pub fn received(&self, packet: &OwnedPacket) {
        let mut kind: sys::kynput_ClipboardEventKind = 0;
        let mut text = false;
        let ptr = packet.borrow().to_ptr();
        if !unsafe { sys::kynput_clipboard_event_from_packet(ptr, &mut kind, &mut text) } || !text {
            return;
        }
        match kind {
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE => {
                self.send(unsafe { sys::kynput_clipboard_text_request_to_packet(HOST) });
            }
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_DATA => {
                let mut len: usize = 0;
                let bytes = unsafe { sys::kynput_clipboard_text_from_packet(ptr, &mut len) };
                if bytes.is_null() {
                    return;
                }
                let slice = unsafe { std::slice::from_raw_parts(bytes, len) };
                if let Ok(string) = std::str::from_utf8(slice) {
                    self.write(string);
                }
            }
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_DATA_REQUEST => self.answer(),
            _ => {}
        }
    }

    /// Regarde si le presse-papiers de l'appareil a changé (deux fois par seconde).
    pub fn poll(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            let Some(at) = inner.next_poll else { return };
            let now = self.clock.now();
            if now < at {
                return;
            }
            inner.next_poll = Some(now + POLL_INTERVAL);
            let count = self.board.change_count();
            if count == inner.seen_change_count {
                return;
            }
            inner.seen_change_count = count;
        }
        if self.board.has_text() {
            self.announce();
        }
    }

    fn announce(&self) {
        self.send(unsafe { sys::kynput_clipboard_text_change_to_packet(HOST) });
    }

    fn answer(&self) {
        match self.board.text() {
            Some(string) => {
                let bytes = string.as_bytes();
                self.send(unsafe { sys::kynput_clipboard_text_data_to_packet(HOST, bytes.as_ptr(), bytes.len()) });
            }
            None => self.send(unsafe { sys::kynput_clipboard_text_unavailable_to_packet(HOST) }),
        }
    }

    fn write(&self, string: &str) {
        if !self.board.reading_asks_user() && self.board.text().as_deref() == Some(string) {
            return;
        }
        self.board.set_text(string);
        // Notre propre écriture ne doit pas repartir vers l'ordinateur.
        self.inner.lock().unwrap().seen_change_count = self.board.change_count();
    }

    fn send(&self, raw: *mut sys::kynput_Packet) {
        let Some(packet) = (unsafe { OwnedPacket::from_raw(raw) }) else { return };
        if let Some(send) = self.send.lock().unwrap().as_ref() {
            send(packet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::clock::ManualClock;

    #[derive(Default)]
    struct Board {
        text: Mutex<Option<String>>,
        count: Mutex<u64>,
    }

    impl Pasteboard for Board {
        fn change_count(&self) -> u64 {
            *self.count.lock().unwrap()
        }
        fn text(&self) -> Option<String> {
            self.text.lock().unwrap().clone()
        }
        fn set_text(&self, text: &str) {
            *self.text.lock().unwrap() = Some(text.into());
            *self.count.lock().unwrap() += 1;
        }
    }

    /// Sorte et texte d'un paquet de presse-papiers.
    fn describe(packet: &OwnedPacket) -> String {
        let mut kind: sys::kynput_ClipboardEventKind = 0;
        let mut text = false;
        let ptr = packet.borrow().to_ptr();
        assert!(unsafe { sys::kynput_clipboard_event_from_packet(ptr, &mut kind, &mut text) });
        match kind {
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE => "annonce".into(),
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_DATA_REQUEST => "demande".into(),
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_DATA_UNAVAILABLE => "rien".into(),
            sys::kynput_ClipboardEventKind_KYNPUT_CLIPBOARD_EVENT_KIND_DATA => {
                let mut len = 0;
                let bytes = unsafe { sys::kynput_clipboard_text_from_packet(ptr, &mut len) };
                format!("texte {}", String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(bytes, len) }))
            }
            _ => unreachable!(),
        }
    }

    struct Bench {
        board: Arc<Board>,
        clock: Arc<ManualClock>,
        bridge: ClipboardBridge,
        sent: Arc<Mutex<Vec<OwnedPacket>>>,
    }

    impl Bench {
        fn new() -> Self {
            let board = Arc::new(Board::default());
            let clock = Arc::new(ManualClock::new());
            let bridge = ClipboardBridge::new(board.clone(), clock.clone());
            let sent = Arc::new(Mutex::new(Vec::new()));
            let s = sent.clone();
            bridge.set_send(move |p| s.lock().unwrap().push(p));
            Self {
                board,
                clock,
                bridge,
                sent,
            }
        }

        fn sent(&self) -> Vec<String> {
            self.sent.lock().unwrap().iter().map(describe).collect()
        }

        fn packet(&self, raw: *mut sys::kynput_Packet) -> OwnedPacket {
            unsafe { OwnedPacket::from_raw(raw) }.unwrap()
        }
    }

    #[test]
    fn vers_l_ordinateur() {
        let b = Bench::new();
        b.bridge.start();
        assert!(b.sent().is_empty(), "rien de copié : rien d'annoncé");
        b.board.set_text("bonjour");
        b.bridge.poll();
        assert!(b.sent().is_empty(), "pas encore regardé");
        b.clock.advance(POLL_INTERVAL);
        b.bridge.poll();
        assert_eq!(b.sent(), ["annonce"], "changement vu : annoncé");
        b.bridge.received(&b.packet(unsafe { sys::kynput_clipboard_text_request_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT) }));
        assert_eq!(b.sent(), ["annonce", "texte bonjour"], "demande : le texte part");
        b.clock.advance(POLL_INTERVAL);
        b.bridge.poll();
        assert_eq!(b.sent().len(), 2, "rien de nouveau : rien d'annoncé");
        b.bridge.stop();
        assert_eq!(b.bridge.next_deadline(), None);
    }

    #[test]
    fn depuis_l_ordinateur() {
        let b = Bench::new();
        b.bridge.start();
        b.bridge.received(&b.packet(unsafe { sys::kynput_clipboard_text_change_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT) }));
        assert_eq!(b.sent(), ["demande"], "l'ordinateur annonce : on demande tout de suite");
        let text = "copié là-bas";
        let data = unsafe { sys::kynput_clipboard_text_data_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT, text.as_ptr(), text.len()) };
        b.bridge.received(&b.packet(data));
        assert_eq!(b.board.text().as_deref(), Some(text), "le texte est écrit dans le presse-papiers");
        b.clock.advance(POLL_INTERVAL);
        b.bridge.poll();
        assert_eq!(b.sent().len(), 1, "notre propre écriture ne repart pas");
        let b2 = Bench::new();
        b2.bridge.start();
        b2.bridge.received(&b2.packet(unsafe { sys::kynput_clipboard_text_request_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT) }));
        assert_eq!(b2.sent(), ["rien"], "demande sans texte : indisponible");
    }

    /// Presse-papiers d'iOS : lire le texte dérange l'utilisateur.
    #[derive(Default)]
    struct AskingBoard {
        inner: Board,
        reads: Mutex<u32>,
    }

    impl Pasteboard for AskingBoard {
        fn change_count(&self) -> u64 {
            self.inner.change_count()
        }
        fn text(&self) -> Option<String> {
            *self.reads.lock().unwrap() += 1;
            self.inner.text()
        }
        fn set_text(&self, text: &str) {
            self.inner.set_text(text)
        }
        fn has_text(&self) -> bool {
            self.inner.text().is_some()
        }
        fn reading_asks_user(&self) -> bool {
            true
        }
    }

    #[test]
    fn lu_seulement_sur_demande() {
        let board = Arc::new(AskingBoard::default());
        board.inner.set_text("copié avant");
        let clock = Arc::new(ManualClock::new());
        let bridge = ClipboardBridge::new(board.clone(), clock.clone());
        let sent = Arc::new(Mutex::new(Vec::<OwnedPacket>::new()));
        let s = sent.clone();
        bridge.set_send(move |p| s.lock().unwrap().push(p));
        let kinds = || sent.lock().unwrap().iter().map(describe).collect::<Vec<_>>();
        let reads = || *board.reads.lock().unwrap();

        bridge.start();
        assert!(kinds().is_empty(), "rien d'annoncé au départ");
        assert_eq!(reads(), 0, "rien de lu au départ");

        board.inner.set_text("copié pendant");
        clock.advance(POLL_INTERVAL);
        bridge.poll();
        assert_eq!(kinds(), ["annonce"], "changement annoncé sans lire");
        assert_eq!(reads(), 0);

        let text = "de l'ordinateur";
        let data = unsafe { sys::kynput_clipboard_text_data_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT, text.as_ptr(), text.len()) };
        bridge.received(&unsafe { OwnedPacket::from_raw(data) }.unwrap());
        assert_eq!(board.inner.text().as_deref(), Some(text), "écrit");
        assert_eq!(reads(), 0, "écrit sans relire");

        let request = unsafe { sys::kynput_clipboard_text_request_to_packet(sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_CLIENT) };
        bridge.received(&unsafe { OwnedPacket::from_raw(request) }.unwrap());
        assert_eq!(reads(), 1, "lu quand l'ordinateur le demande");
    }

    #[test]
    fn deja_copie_au_depart() {
        let b = Bench::new();
        b.board.set_text("avant la session");
        let bridge = ClipboardBridge::new(b.board.clone(), b.clock.clone());
        let sent = b.sent.clone();
        bridge.set_send(move |p| sent.lock().unwrap().push(p));
        bridge.start();
        assert_eq!(b.sent(), ["annonce"], "annoncé dès le début de la session");
        assert_eq!(bridge.next_deadline(), Some(POLL_INTERVAL));
    }
}
