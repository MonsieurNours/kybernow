//! Français et anglais, comme les autres apps : français si le téléviseur est
//! réglé en français, anglais sinon ; réglage Auto / Français / English.
//! Les textes sont écrits en paires, sur place : `tr("…", "…")` ; la langue et
//! ses petites règles sont celles de l'interface commune (`kybernow_ui`).

pub use kybernow_ui::i18n::{de, decimal, english, set_english, settings_for, tr};
#[cfg(test)]
pub use kybernow_ui::i18n::force_english;
pub use kybernow_ui::time::month;
