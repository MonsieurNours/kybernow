//! Cœur de l'installateur de KyberNow Station : vérifications de l'ordinateur, choix des ports,
//! installation et désinstallation. Partagé par le lanceur et l'interface.

pub mod acl;
pub mod checks;
pub mod events;
pub mod i18n;
pub mod install;
pub mod payload;
pub mod ports;
pub mod sys;
pub mod uninstall;

/// Nom technique : dossiers (`C:\Program Files\KyberNow`, `C:\ProgramData\KyberNow`),
/// clé `…\Uninstall\KyberNow`, dossier du menu Démarrer. Ne change pas : les
/// installations en place, la console et `update.ps1` en dépendent.
pub const PRODUCT: &str = "KyberNow";
/// Nom affiché (« Applications installées », raccourci de l'icône, boîtes de message).
pub const DISPLAY_NAME: &str = "KyberNow Station";
/// Raccourcis du menu Démarrer des versions 0.3.1 et précédentes, retirés à
/// l'installation (le dossier `KyberNow` reste) : icône, puis console en
/// français et en anglais.
pub const LEGACY_SHORTCUTS: [&str; 3] = ["KyberNow.lnk", "Console KyberNow.url", "KyberNow console.url"];
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PUBLISHER: &str = "Les Lignes Claires";
pub const PUBLISHER_URL: &str = "https://www.leslignesclaires.fr";
pub const KYBER_VERSION: &str = "0.29.0";
