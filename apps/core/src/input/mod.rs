//! Module d'entrées partagé des apps KyberNow Link (`InputPipeline`,
//! `PacketQueue`, `Keymap`, `PadCombo`, `ReleaseAll`, `PointerLayer`,
//! `ClipboardBridge`), écrit une fois en Rust au-dessus de la chaîne kynput
//! de Kyber : Mac, Apple TV, iPhone et iPad (par apps/apple), Windows,
//! Android (par JNI), Linux et webOS.
//!
//! Rien ici n'appelle un système : la plateforme fournit l'horloge
//! ([`Clock`]), son presse-papiers ([`clipboard::Pasteboard`]) et reçoit les
//! rappels vers son interface ([`pipeline::InputPlatform`]) ; elle capte ses
//! événements (AppKit, GameController, Win32, SDL, Android…) et les donne à
//! [`pipeline::InputPipeline`], qui les envoie à l'ordinateur.
//!
//! Minuteries : aucun fil n'est créé par les modules eux-mêmes. Chaque
//! module qui attend quelque chose (réessais de la file d'envoi toutes les
//! 3 ms, axes regroupés par 4 ms, combinaison tenue 250 ms, presse-papiers
//! regardé deux fois par seconde) expose `poll()` et `next_deadline()` ; la
//! plateforme appelle `poll()` au plus tard à l'échéance, depuis la file
//! qu'elle veut, ou laisse [`pipeline::InputPipeline::spawn_driver`] tenir
//! un fil qui le fait. Les tests avancent une horloge simulée et appellent
//! `poll()` eux-mêmes.

pub mod clipboard;
pub mod clock;
mod gamepad_gate;
pub mod keymap;
pub mod packet;
pub mod packet_queue;
pub mod pad_combo;
pub mod pipeline;
pub mod pointer_layer;
pub mod release_all;

pub use clipboard::{ClipboardBridge, Pasteboard};
pub use clock::{Clock, ManualClock, MonotonicClock};
pub use packet::{OwnedPacket, PacketSink};
pub use packet_queue::PacketQueue;
pub use pad_combo::{PadAxis, PadButton, PadCombo, PadId, PadKey};
pub use pipeline::{GamepadRumble, InputPipeline, InputPlatform, Options, ScreenTarget};
pub use pointer_layer::{MouseMode, MouseModeRules, PointerLayer, PointerSummary, RemoteCursor, Shape};
pub use release_all::ReleaseAll;
