//! Module d'entrées partagé des clients KyberNow : ce que les apps Mac et
//! Apple TV tenaient dans KyberKit (`InputPipeline`, `PacketQueue`, `Keymap`,
//! `PadCombo`, `ReleaseAll`, `PointerLayer`, `ClipboardBridge`), écrit une
//! fois en Rust au-dessus de la chaîne kynput de Kyber, pour le Mac, l'Apple
//! TV et Windows.
//!
//! Rien ici n'appelle un système : la plateforme fournit l'horloge
//! ([`Clock`]), son presse-papiers ([`clipboard::Pasteboard`]) et reçoit les
//! rappels vers son interface ([`pipeline::InputPlatform`]) ; elle capte ses
//! événements (AppKit, GameController, Win32) et les donne à
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

pub mod clock;
pub mod packet;
pub mod packet_queue;

pub use clock::{Clock, ManualClock, MonotonicClock};
pub use packet::{OwnedPacket, PacketSink};
pub use packet_queue::PacketQueue;
