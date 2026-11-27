//! L'interface commune des apps KyberNow Link sur Linux (bureau, Steam Deck,
//! salon, Link OS) et sur les téléviseurs LG (LINAPP-42, WEBOS-26) : ce que
//! les deux interfaces écrivaient chacune, mot pour mot. Comme elles, la
//! caisse se compile sans le cœur ni SDL (captures, essais sur le Mac) ; les
//! règles pures viennent de `kybernow-rules`, les textes du cœur de son
//! catalogue, lu tel quel ([`i18n::core_text`]).
//!
//! - [`i18n`] : la langue de l'app (Auto, français, anglais), `tr`, nombres.
//! - [`time`] : heure locale, jours, mois, dates des créneaux et du test Auto.
//! - [`autotest`] : le panneau du test du profil Auto (étapes, résumé, textes).
//! - [`station_offer`] : la mise à jour des Stations proposée par l'app.
//! - [`share`], [`lend`] : ce que l'interface montre du partage et du prêt.
//! - `from_core` (fonctionnalité `core`, pour la couche appareil) : ce que le
//!   cœur dit, dans ces types.

pub mod autotest;
#[cfg(feature = "core")]
pub mod from_core;
pub mod i18n;
pub mod lend;
pub mod share;
pub mod station_offer;
pub mod time;
