//! Français et anglais, comme les autres apps : français si la première langue
//! préférée du système (SDL, sinon `LANG`) est le français, anglais sinon ;
//! réglage Auto / Français / English.
//! Les textes sont écrits en paires, sur place : `tr("…", "…")`.

pub use kybernow_ui::i18n::{core_text, de, decimal, english, force_english, set_english, settings_for, tr};
