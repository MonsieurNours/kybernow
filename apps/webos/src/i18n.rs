//! Français et anglais, comme les autres apps : français si le téléviseur est
//! réglé en français, anglais sinon ; réglage Auto / Français / English.
//! Les textes sont écrits en paires, sur place : `tr("…", "…")`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_english(en: bool) {
    ENGLISH.store(en, Ordering::Relaxed);
}

pub fn english() -> bool {
    #[cfg(test)]
    if let Some(en) = FORCED.with(|f| f.get()) {
        return en;
    }
    ENGLISH.load(Ordering::Relaxed)
}

// Les tests tournent chacun sur leur fil : une langue forcée n'en gêne pas un autre.
#[cfg(test)]
thread_local! {
    static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// La langue du fil de ce test (sans toucher à celle des autres tests).
#[cfg(test)]
pub fn force_english(en: bool) {
    FORCED.with(|f| f.set(Some(en)));
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

/// Le nom d'un mois (0 à 11) dans la langue de l'app : « novembre », “November” (WEBOS-26 : écrit une fois).
pub fn month(m: usize) -> &'static str {
    const FR: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    if english() { EN[m.min(11)] } else { FR[m.min(11)] }
}

/// Le nom d'un jour de la semaine (0 : dimanche) : « mercredi », “Wednesday”.
pub fn weekday(d: usize) -> &'static str {
    const FR: [&str; 7] = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
    const EN: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    if english() { EN[d.min(6)] } else { FR[d.min(6)] }
}

/// L'heure locale du téléviseur (son fuseau, `localtime_r`) d'un moment en secondes depuis 1970.
pub fn local_time(at: u64) -> Option<libc::tm> {
    let t = at as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    (!r.is_null()).then_some(tm)
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
