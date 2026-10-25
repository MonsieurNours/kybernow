//! Français et anglais, comme les autres apps : français si la première langue
//! préférée du système (SDL, sinon `LANG`) est le français, anglais sinon ;
//! réglage Auto / Français / English.
//! Les textes sont écrits en paires, sur place : `tr("…", "…")`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(en: bool) {
    ENGLISH.store(en, Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Le texte dans la langue de l'app.
pub fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if english() { en } else { fr }
}

/// « de » devant un nom, élidé devant une voyelle ou un h : « d'Atelier »,
/// « d'Hugo-PC », mais « de Bureau-Salon ».
pub fn de(name: &str) -> String {
    let first = name.chars().next().and_then(|c| c.to_lowercase().next());
    if first.is_some_and(|c| "aeiouyhàâäéèêëîïôöùûüÿæœ".contains(c)) { format!("d'{name}") } else { format!("de {name}") }
}

/// Le titre des réglages d'un ordinateur : « Réglages de Bureau-Salon »,
/// « Réglages d'Atelier », “Settings for Atelier”.
pub fn settings_for(name: &str) -> String {
    if english() { format!("Settings for {name}") } else { format!("Réglages {}", de(name)) }
}

/// Un nombre décimal à la française (virgule) ou à l'anglaise (point).
pub fn decimal(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    if english() { s } else { s.replace('.', ",") }
}

/// Langue du réglage (`auto`, `fr`, `en`) et langue du système (`fr-FR`…).
pub fn resolve(setting: &str, system: &str) -> bool {
    match setting {
        "fr" => false,
        "en" => true,
        _ => !system.to_ascii_lowercase().starts_with("fr"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elision() {
        assert_eq!(de("Atelier"), "d'Atelier");
        assert_eq!(de("Hugo-PC"), "d'Hugo-PC");
        assert_eq!(de("écran"), "d'écran");
        assert_eq!(de("Bureau-Salon"), "de Bureau-Salon");
        assert_eq!(de("Tour du salon"), "de Tour du salon");
    }
}
