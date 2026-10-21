//! Langue de l'installateur : celle de Windows (langue d'affichage de
//! l'utilisateur), français si elle commence par `fr`, anglais sinon. Pas de
//! réglage (docs/i18n.md). Les textes sont dans `i18n/fr.json` et `en.json` ;
//! une clé absente en anglais retombe sur le français. Le journal
//! (`setup.log`) reste en français.

use std::fmt::Display;
use std::sync::OnceLock;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    /// Code court, pour l'interface (`?lang=`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }
}

/// `fr`, `fr-FR`, `fr-CA`… : français ; toute autre langue : anglais.
pub fn lang_from_tag(tag: &str) -> Lang {
    let tag = tag.trim().to_ascii_lowercase();
    if tag == "fr" || tag.starts_with("fr-") || tag.starts_with("fr_") { Lang::Fr } else { Lang::En }
}

/// Langue de Windows, lue une fois.
pub fn lang() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(|| lang_from_tag(&system_tag()))
}

#[cfg(windows)]
fn system_tag() -> String {
    use windows_sys::Win32::Globalization::{GetUserDefaultUILanguage, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    unsafe {
        let (mut count, mut len) = (0u32, 0u32);
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, std::ptr::null_mut(), &mut len) != 0 && len > 0 {
            let mut buf = vec![0u16; len as usize];
            if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len) != 0 {
                // Liste de noms séparés par des zéros : le premier compte.
                let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                if end > 0 {
                    return String::from_utf16_lossy(&buf[..end]);
                }
            }
        }
        // LANGID : la langue principale est sur les 10 bits du bas (LANG_FRENCH = 0x0C).
        if GetUserDefaultUILanguage() & 0x3FF == 0x0C { "fr".into() } else { "en".into() }
    }
}

/// Hors de Windows (tests) : variable `LANG`, anglais par défaut.
#[cfg(not(windows))]
fn system_tag() -> String {
    std::env::var("LANG").unwrap_or_default()
}

fn catalogue(lang: Lang) -> &'static Value {
    static FR: OnceLock<Value> = OnceLock::new();
    static EN: OnceLock<Value> = OnceLock::new();
    match lang {
        Lang::Fr => FR.get_or_init(|| serde_json::from_str(include_str!("i18n/fr.json")).expect("i18n/fr.json")),
        Lang::En => EN.get_or_init(|| serde_json::from_str(include_str!("i18n/en.json")).expect("i18n/en.json")),
    }
}

fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    key.split('.').try_fold(catalogue(lang), |v, part| v.get(part))?.as_str()
}

/// Texte de `key` dans `lang`, `{nom}` remplacé par sa valeur. Repli : le
/// français, puis la clé elle-même.
pub fn tr(lang: Lang, key: &str, params: &[(&str, &dyn Display)]) -> String {
    let mut text = lookup(lang, key).or_else(|| lookup(Lang::Fr, key)).unwrap_or(key).to_string();
    for (name, value) in params {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Erreur montrée à l'utilisateur : en français dans le journal (`Display`),
/// dans sa langue à l'écran (`describe`).
#[derive(Debug)]
pub struct Localized {
    key: &'static str,
    params: Vec<(&'static str, String)>,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl Localized {
    pub fn text(&self, lang: Lang) -> String {
        let params: Vec<(&str, &dyn Display)> = self.params.iter().map(|(k, v)| (*k, v as &dyn Display)).collect();
        tr(lang, self.key, &params)
    }
}

impl Display for Localized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(Lang::Fr))
    }
}

impl std::error::Error for Localized {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|e| e as &(dyn std::error::Error + 'static))
    }
}

fn owned(params: &[(&'static str, &dyn Display)]) -> Vec<(&'static str, String)> {
    params.iter().map(|(k, v)| (*k, v.to_string())).collect()
}

/// Nouvelle erreur traduite.
pub fn err(key: &'static str, params: &[(&'static str, &dyn Display)]) -> anyhow::Error {
    anyhow::Error::new(Localized { key, params: owned(params), source: None })
}

/// `.context(…)` traduit.
pub trait TrContext<T> {
    fn tr_context(self, key: &'static str, params: &[(&'static str, &dyn Display)]) -> anyhow::Result<T>;
}

impl<T, E: Into<anyhow::Error>> TrContext<T> for Result<T, E> {
    fn tr_context(self, key: &'static str, params: &[(&'static str, &dyn Display)]) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::Error::new(Localized { key, params: owned(params), source: Some(e.into().into()) }))
    }
}

/// Comme `{e:#}`, avec les maillons traduits dans `lang`.
pub fn describe(lang: Lang, e: &anyhow::Error) -> String {
    let mut parts = Vec::new();
    for link in e.chain() {
        match link.downcast_ref::<Localized>() {
            Some(l) => parts.push(l.text(lang)),
            None => parts.push(link.to_string()),
        }
    }
    // Un `Localized` porteur d'une source : la conversion en Box ajoute un
    // maillon qui répète le message de la source, on l'enlève.
    parts.dedup();
    parts.join(": ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        for tag in ["fr", "fr-FR", "FR-ca", "fr-BE", " fr-CH "] {
            assert_eq!(lang_from_tag(tag), Lang::Fr, "{tag}");
        }
        for tag in ["en-US", "en", "de-DE", "", "fra", "frisian", "es-419"] {
            assert_eq!(lang_from_tag(tag), Lang::En, "{tag}");
        }
    }

    #[test]
    fn substitution_and_fallback() {
        assert_eq!(tr(Lang::Fr, "uninstall.title", &[]), "Désinstaller KyberNow");
        assert_eq!(tr(Lang::En, "uninstall.title", &[]), "Uninstall KyberNow");
        assert_eq!(tr(Lang::En, "checks.existing.title", &[("version", &"0.2.7")]), "KyberNow 0.2.7 is installed");
        assert_eq!(tr(Lang::En, "pas.une.cle", &[]), "pas.une.cle");
    }

    #[test]
    fn localized_errors() {
        let e = err("errors.appWontClose", &[]);
        assert_eq!(format!("{e:#}"), tr(Lang::Fr, "errors.appWontClose", &[]));
        assert_eq!(describe(Lang::En, &e), tr(Lang::En, "errors.appWontClose", &[]));
        let io: Result<(), std::io::Error> = Err(std::io::Error::other("refus"));
        let e = io.tr_context("errors.locked", &[("path", &"C:\\x")]).unwrap_err();
        assert_eq!(describe(Lang::En, &e), "C:\\x is locked: refus");
        assert_eq!(describe(Lang::Fr, &e), format!("{e:#}"));
    }

    fn keys(v: &Value, prefix: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, v)| keys(v, &format!("{prefix}{k}."), out)),
            _ => out.push(prefix.trim_end_matches('.').to_string()),
        }
    }

    #[test]
    fn same_keys() {
        let (mut fr, mut en) = (Vec::new(), Vec::new());
        keys(catalogue(Lang::Fr), "", &mut fr);
        keys(catalogue(Lang::En), "", &mut en);
        fr.sort();
        en.sort();
        assert_eq!(fr, en);
    }
}
