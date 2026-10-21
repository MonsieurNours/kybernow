//! Entrées vers l'ordinateur, par la chaîne kynput de Kyber : clavier
//! (codes de touche de Windows tels quels), souris absolue ou relative,
//! boutons, molette. Les manettes (SDL) et le presse-papiers sont déjà gérés
//! par la configuration client de kynput sous Windows.
//!
//! Le pointeur de l'ordinateur (forme et visibilité) arrive par un gestionnaire
//! de paquets `Cursor`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

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

pub struct Input {
    pipeline: Pipeline,
    sink: kynput_rs::Consumer,
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
        let sink = pipeline.get_input_sink();
        let input = Arc::new(Self {
            pipeline,
            sink,
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
        self.sink.consume(KeyboardKey { pressed, scancode }.to_packet(HOST));
    }

    pub fn position(&self, x: i16, y: i16) {
        let display_id = self.display_id.load(Ordering::Relaxed);
        self.sink.consume(MousePosition { display_id, x, y }.to_packet(HOST));
    }

    pub fn motion(&self, dx: i16, dy: i16) {
        if dx != 0 || dy != 0 {
            self.sink.consume(MouseMove { dx, dy }.to_packet(HOST));
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
        self.sink.consume(MouseButton { button, pressed }.to_packet(HOST));
    }

    pub fn wheel(&self, dx: f32, dy: f32) {
        self.sink.consume(MouseWheel { dx, dy }.to_packet(HOST));
    }

    /// Relâche ce que l'app tient sur l'ordinateur (perte du clavier), sans
    /// toucher aux manettes.
    pub fn release_all(&self) {
        let keys: Vec<u16> = self.held_keys.lock().unwrap().drain().collect();
        for scancode in keys {
            self.sink.consume(KeyboardKey { pressed: false, scancode }.to_packet(HOST));
        }
        let buttons: Vec<u8> = self.held_buttons.lock().unwrap().drain().collect();
        for b in buttons {
            self.sink.consume(MouseButton { button: button_type(b), pressed: false }.to_packet(HOST));
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
}
