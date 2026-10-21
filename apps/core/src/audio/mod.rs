//! Son sans VLC : paquets Opus de Kyber → libopus → la sortie de la
//! plateforme (Core Audio, WASAPI), comme `AudioPlayer.swift` des apps
//! Apple, écrit une fois en Rust.
//!
//! Latence : un petit tampon entre la réception (un paquet de 10 ou 20 ms à
//! la fois, plus ou moins régulier) et la carte son (qui réclame quelques
//! millisecondes à la fois). Sa marge se règle toute seule : elle grandit de
//! 5 ms après un tampon vide, puis redescend d'une milliseconde toutes les
//! deux secondes sans trou. Un tampon vide est comblé par une trame de
//! masquage d'Opus (PLC, raccord plausible) plutôt que par du silence, et la
//! lecture reprend dès que la marge est de nouveau là (le premier départ
//! attend la marge plus un paquet). La dérive entre les horloges de
//! l'ordinateur et de l'appareil est rattrapée en douceur : un échantillon
//! sauté ou doublé sur 256 (0,4 %, inaudible ; sur 128 au-delà de 6 ms
//! d'écart) jusqu'à ce que le plus bas niveau du tampon retrouve la marge,
//! au lieu d'une coupe sèche ; seule une avance de plus de 10 ms (démarrage,
//! sortie restée arrêtée) est coupée d'un coup.
//!
//! Sortie : la plateforme tire le son (`AudioEngine::fill`) depuis son fil
//! temps réel, sans verrou bloquant (anneau sans verrou, essais de verrou
//! pour le reste) ; si elle a moins de voies que le flux (5.1 ou 7.1 vers un
//! casque), le son est mixé vers le bas (G + 0,7 C + 0,7 surrounds, idem à
//! droite, LFE ignoré) au lieu d'être coupé par index, et les voies sont
//! remises dans l'ordre qu'elle demande (`OutputOrder`). Elle donne la
//! latence de sa sortie pour l'écart son-image.
//!
//! Ce que la plateforme fournit : l'horloge ([`Clock`], celle du module
//! d'entrées), la sortie elle-même (son fil, son nombre de voies, sa
//! latence), et libopus à l'édition de liens (`build.rs`).

pub mod engine;
pub mod mix;
pub mod opus;
pub mod ring;

pub use crate::input::clock::{Clock, ManualClock, MonotonicClock};
pub use engine::{AudioEngine, AudioStats};
pub use mix::OutputOrder;

/// Fréquence d'Opus, et du flux de Kyber.
pub const SAMPLE_RATE: usize = 48_000;
/// Plus grand paquet Opus : 120 ms.
pub const MAX_FRAMES: usize = 5760;
