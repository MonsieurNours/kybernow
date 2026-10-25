//! Français et anglais, comme les autres apps : français si le téléviseur est
//! réglé en français, anglais sinon ; réglage Auto / Français / English.
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

/// « de » devant un nom, élidé en « d' » devant une voyelle ou un h
/// (« Réglages d'Atelier », « Réglages de Bureau-Salon »).
pub fn de(name: &str) -> &'static str {
    let elide = name.chars().next().is_some_and(|c| "aeiouyhàâäéèêëîïôöùûüÿæœAEIOUYHÀÂÄÉÈÊËÎÏÔÖÙÛÜŸÆŒ".contains(c));
    if elide { "d'" } else { "de " }
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
mod tests_de {
    use super::de;

    #[test]
    fn elision() {
        assert_eq!(de("Atelier"), "d'");
        assert_eq!(de("Hugo-PC"), "d'");
        assert_eq!(de("Écran"), "d'");
        assert_eq!(de("Bureau-Salon"), "de ");
    }
}
