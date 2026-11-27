//! Français et anglais, comme les autres apps : la langue de l'app (Auto,
//! Français, English), posée par l'app au lancement et quand le réglage
//! change. Les textes de l'interface sont écrits en paires, sur place :
//! `tr("…", "…")` ; ceux du cœur viennent de son catalogue ([`core_text`]).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

static ENGLISH: AtomicBool = AtomicBool::new(false);

// Les essais tournent chacun sur leur fil : une langue forcée n'en gêne pas un autre.
thread_local! {
    static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

pub fn set_english(en: bool) {
    ENGLISH.store(en, Ordering::Relaxed);
}

pub fn english() -> bool {
    if let Some(en) = FORCED.with(|f| f.get()) {
        return en;
    }
    ENGLISH.load(Ordering::Relaxed)
}

/// La langue du fil de cet essai (sans toucher à celle des autres essais).
#[doc(hidden)]
pub fn force_english(en: bool) {
    FORCED.with(|f| f.set(Some(en)));
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

/// Le titre des réglages d'un ordinateur ou d'une console : « Réglages de
/// Bureau-Salon », « Réglages d'Atelier », “Settings for Atelier”.
pub fn settings_for(name: &str) -> String {
    if english() { format!("Settings for {name}") } else { format!("Réglages {}", de(name)) }
}

/// Un nombre décimal à la française (virgule) ou à l'anglaise (point).
pub fn decimal(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    if english() { s } else { s.replace('.', ",") }
}

/// Langue du réglage (`auto`, `fr`, `en`) et langue du système (`fr-FR`…) :
/// vrai pour l'anglais.
pub fn resolve(setting: &str, system: &str) -> bool {
    match setting {
        "fr" => false,
        "en" => true,
        _ => !system.to_ascii_lowercase().starts_with("fr"),
    }
}

/// Le catalogue du cœur (`apps/core/src/i18n/*.json`), lu une fois.
fn catalog() -> &'static (Value, Value) {
    static CATALOG: OnceLock<(Value, Value)> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let read = |s: &str| serde_json::from_str(s).unwrap_or(Value::Null);
        (read(include_str!("../../core/src/i18n/fr.json")), read(include_str!("../../core/src/i18n/en.json")))
    })
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(root, |v, k| v.get(k))?.as_str()
}

/// Un texte du catalogue du cœur dans la langue de l'app, ses `{paramètres}`
/// remplis : le même mot pour mot que celui des autres apps (`i18n::tr` du
/// cœur ; l'anglais absent retombe sur le français, une clé inconnue est
/// rendue telle quelle).
pub fn core_text(key: &str, params: &[(&str, &str)]) -> String {
    let (fr, en) = catalog();
    let text = if english() { lookup(en, key).or_else(|| lookup(fr, key)) } else { lookup(fr, key) };
    let mut out = text.unwrap_or(key).to_string();
    for (name, value) in params {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textes_du_coeur() {
        force_english(false);
        assert_eq!(core_text("autotest.step.link", &[]), "Liaison");
        assert_eq!(core_text("autotest.unreachable", &[("name", "Atelier")]), "Le test n'a pas pu joindre Atelier.");
        assert!(core_text("lend.warning_windows", &[]).starts_with("Votre ami joue dans votre session Windows."));
        force_english(true);
        assert_eq!(core_text("autotest.step.link", &[]), "Connection");
        assert_eq!(core_text("inconnu.cle", &[]), "inconnu.cle");
        assert_eq!(tr("Oui", "Yes"), "Yes");
        assert_eq!(decimal(2.5, 1), "2.5");
        force_english(false);
        assert_eq!(decimal(2.5, 1), "2,5");
        assert!(resolve("auto", "en-US") && !resolve("auto", "fr-FR") && !resolve("fr", "en"));
        assert_eq!(de("Atelier"), "d'Atelier");
        assert_eq!(de("Hugo-PC"), "d'Hugo-PC");
        assert_eq!(de("Écran"), "d'Écran");
        assert_eq!(de("Bureau-Salon"), "de Bureau-Salon");
        assert_eq!(settings_for("Atelier"), "Réglages d'Atelier");
    }
}
