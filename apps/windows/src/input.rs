//! Entrées vers l'ordinateur, par la chaîne kynput de Kyber : clavier
//! (codes de touche de Windows tels quels), souris absolue ou relative,
//! boutons, molette. Les manettes (SDL) et le presse-papiers sont déjà gérés
//! par la configuration client de kynput sous Windows.
//!
//! Le pointeur de l'ordinateur (forme et visibilité) arrive par un gestionnaire
//! de paquets `Cursor`.
//!
//! Paquets refusés : la file d'envoi de kynput tient 64 paquets et refuse sans
//! retour quand elle est pleine (souris à 1000 Hz) ou avant la connexion, alors
//! que le protocole est fait de différences : un relâchement perdu laisse une
//! touche enfoncée jusqu'à la fin de la session. Comme sur Mac
//! (`PacketQueue.swift`), touches et boutons refusés sont réessayés toutes les
//! 3 ms, dans l'ordre, pendant 200 ms au plus ; positions, déplacements et
//! molette partent une fois, et pas du tout tant que la file de secours attend.

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kynput_sys as sys;

use kybernow_core::kynput_rs::{
    self, Config, ConsumerInterface, Cursor, CursorFormat, HandlerConsumerInterface, HandlerInterface,
    InputTarget, InputType, InputTypeMetadata, KeyboardKey, MouseButton, MouseButtonType, MouseMove, MousePosition,
    MouseWheel, Packet, Pipeline, RouterProcessingState,
};

const HOST: InputTarget = InputTarget::Host;

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
    on_cursor: Box<dyn Fn(Cursor) + Send + Sync>,
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
        if let Some(c) = Cursor::from_packet(&packet) {
            (self.on_cursor)(c);
        }
    }
}

impl HandlerConsumerInterface for CursorHandler {
    fn handled_types(&self) -> Vec<InputType> {
        vec![InputType::Cursor]
    }
}

/// Envoi à kynput avec sa file de secours.
struct Sender {
    sink: *mut sys::kynput_Consumer,
    backlog: Mutex<VecDeque<(usize, Instant)>>,
    refused: AtomicU32,
}

unsafe impl Send for Sender {}
unsafe impl Sync for Sender {}

const RETRY_EVERY: Duration = Duration::from_millis(3);
const RETRY_FOR: Duration = Duration::from_millis(200);

impl Sender {
    /// Tente un envoi ; kynput garde le paquet qu'on lui passe, accepté ou non.
    fn try_send(&self, pkt: *mut sys::kynput_Packet) -> bool {
        unsafe { sys::kynput_consumer_consume(self.sink, sys::kynput_packet_clone(pkt)) == 0 }
    }

    /// Touches, boutons, tout relâcher : réessayés jusqu'à 200 ms.
    fn reliable(&self, pkt: Packet) {
        let ptr = pkt.to_ptr_mut();
        if ptr.is_null() {
            return;
        }
        let mut q = self.backlog.lock().unwrap();
        if q.is_empty() && self.try_send(ptr) {
            unsafe { sys::kynput_packet_unref(ptr) };
            return;
        }
        self.refused.fetch_add(1, Ordering::Relaxed);
        q.push_back((ptr as usize, Instant::now()));
    }

    /// Positions, déplacements, molette : une fois, sauf si la file attend.
    fn once(&self, pkt: Packet) {
        let ptr = pkt.to_ptr_mut();
        if ptr.is_null() {
            return;
        }
        if !self.backlog.lock().unwrap().is_empty() {
            unsafe { sys::kynput_packet_unref(ptr) };
            return;
        }
        if unsafe { sys::kynput_consumer_consume(self.sink, ptr) } != 0 {
            self.refused.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn retry(&self) {
        let mut q = self.backlog.lock().unwrap();
        while let Some(&(ptr, since)) = q.front() {
            let ptr = ptr as *mut sys::kynput_Packet;
            if self.try_send(ptr) {
                unsafe { sys::kynput_packet_unref(ptr) };
                q.pop_front();
            } else if since.elapsed() > RETRY_FOR {
                log::warn!("entrées : paquet abandonné après 200 ms");
                unsafe { sys::kynput_packet_unref(ptr) };
                q.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        for (ptr, _) in self.backlog.get_mut().unwrap().drain(..) {
            unsafe { sys::kynput_packet_unref(ptr as *mut sys::kynput_Packet) };
        }
        unsafe { sys::kynput_consumer_unref(self.sink) };
    }
}

pub struct Input {
    pipeline: Pipeline,
    sender: Arc<Sender>,
    pub display_id: AtomicU32,
    relative: AtomicBool,
    held_keys: Mutex<HashSet<u16>>,
    held_buttons: Mutex<HashSet<u8>>,
}

fn button_id(b: MouseButtonType) -> u8 {
    match b {
        MouseButtonType::Left => 0,
        MouseButtonType::Right => 1,
        MouseButtonType::Middle => 2,
        MouseButtonType::Side => 3,
        MouseButtonType::Extra => 4,
    }
}

fn button_type(id: u8) -> MouseButtonType {
    match id {
        0 => MouseButtonType::Left,
        1 => MouseButtonType::Right,
        2 => MouseButtonType::Middle,
        3 => MouseButtonType::Side,
        _ => MouseButtonType::Extra,
    }
}

impl Input {
    pub fn new(on_cursor: Box<dyn Fn(Cursor) + Send + Sync>) -> Arc<Self> {
        let pipeline = Pipeline::new(Config::default_client());
        let cursor = kynput_rs::create_handler_consumer_instance(Arc::new(CursorHandler { on_cursor }));
        if let Err(e) = pipeline.plug_handler(cursor) {
            log::warn!("kynput : gestionnaire du pointeur refusé ({e:?})");
        }
        let types: Vec<InputType> = [
            InputType::Keyboard,
            InputType::MousePosition,
            InputType::MouseMove,
            InputType::MouseButton,
            InputType::MouseWheel,
        ]
        .into_iter()
        .filter(|t| !pipeline.produces_type(*t))
        .collect();
        log::info!("entrées produites par l'app : {types:?}");
        let producer = kynput_rs::create_handler_instance(Arc::new(Producer { types }));
        if let Err(e) = pipeline.plug_handler(producer) {
            log::warn!("kynput : producteur refusé ({e:?})");
        }
        let sender = Arc::new(Sender {
            sink: unsafe { sys::kynput_pipeline_get_input_sink(pipeline.to_ptr()) },
            backlog: Mutex::new(VecDeque::new()),
            refused: AtomicU32::new(0),
        });
        let weak = Arc::downgrade(&sender);
        std::thread::Builder::new()
            .name("kybernow-inputs".into())
            .spawn(move || {
                while let Some(s) = weak.upgrade() {
                    s.retry();
                    drop(s);
                    std::thread::sleep(RETRY_EVERY);
                }
            })
            .expect("fil des entrées");
        let input = Arc::new(Self {
            pipeline,
            sender,
            display_id: AtomicU32::new(0),
            relative: AtomicBool::new(false),
            held_keys: Mutex::new(HashSet::new()),
            held_buttons: Mutex::new(HashSet::new()),
        });
        input.set_relative(false);
        input
    }

    pub fn pipeline(&self) -> Pipeline {
        self.pipeline.clone()
    }

    pub fn key(&self, scancode: u16, pressed: bool) {
        {
            let mut held = self.held_keys.lock().unwrap();
            if pressed {
                held.insert(scancode);
            } else if !held.remove(&scancode) {
                // Relâchement d'une touche jamais envoyée (raccourci de l'app).
                return;
            }
        }
        self.sender.reliable(KeyboardKey { pressed, scancode }.to_packet(HOST));
    }

    pub fn position(&self, x: i16, y: i16) {
        let display_id = self.display_id.load(Ordering::Relaxed);
        self.sender.once(MousePosition { display_id, x, y }.to_packet(HOST));
    }

    pub fn motion(&self, dx: i16, dy: i16) {
        if dx != 0 || dy != 0 {
            self.sender.once(MouseMove { dx, dy }.to_packet(HOST));
        }
    }

    pub fn button(&self, button: MouseButtonType, pressed: bool) {
        {
            let mut held = self.held_buttons.lock().unwrap();
            if pressed {
                held.insert(button_id(button));
            } else if !held.remove(&button_id(button)) {
                return;
            }
        }
        self.sender.reliable(MouseButton { button, pressed }.to_packet(HOST));
    }

    pub fn wheel(&self, dx: f32, dy: f32) {
        self.sender.once(MouseWheel { dx, dy }.to_packet(HOST));
    }

    /// Relâche ce que l'app tient sur l'ordinateur (perte du clavier), sans
    /// toucher aux manettes.
    pub fn release_all(&self) {
        let keys: Vec<u16> = self.held_keys.lock().unwrap().drain().collect();
        for scancode in keys {
            self.sender.reliable(KeyboardKey { pressed: false, scancode }.to_packet(HOST));
        }
        let buttons: Vec<u8> = self.held_buttons.lock().unwrap().drain().collect();
        for b in buttons {
            self.sender.reliable(MouseButton { button: button_type(b), pressed: false }.to_packet(HOST));
        }
    }

    /// Souris relative (jeux) : seuls les déplacements bruts partent.
    pub fn set_relative(&self, relative: bool) {
        self.relative.store(relative, Ordering::Relaxed);
        let _ = self
            .pipeline
            .set_type_processing_state(InputType::MousePosition, RouterProcessingState::from_boolean(!relative));
        let _ = self
            .pipeline
            .set_type_processing_state(InputType::MouseMove, RouterProcessingState::from_boolean(relative));
    }

    pub fn relative(&self) -> bool {
        self.relative.load(Ordering::Relaxed)
    }

    /// Paquets refusés par kynput depuis le début (journal de fin de session).
    pub fn refused(&self) -> u32 {
        self.sender.refused.load(Ordering::Relaxed)
    }
}
