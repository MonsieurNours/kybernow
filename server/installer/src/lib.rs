//! Cœur de l'installateur KyberNow : vérifications de l'ordinateur, choix des ports,
//! installation et désinstallation. Partagé par le lanceur et l'interface.

pub mod checks;
pub mod events;
pub mod install;
pub mod payload;
pub mod ports;
pub mod sys;
pub mod uninstall;

pub const PRODUCT: &str = "KyberNow";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PUBLISHER: &str = "Les Lignes Claires";
pub const PUBLISHER_URL: &str = "https://www.leslignesclaires.fr";
pub const KYBER_VERSION: &str = "0.29.0";
