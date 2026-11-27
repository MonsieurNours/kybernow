//! Langue de l'installateur : celle de Windows (`fr*` : français, sinon
//! anglais), sans réglage (voir `docs/i18n.md`). Textes dans `i18n/fr.json`
//! et `i18n/en.json`, embarqués ; une clé absente en anglais retombe sur le
//! français. Le journal (`install.log`) reste en français.
//!
//! `KYBERNOW_LANG=en` (ou `fr`) force la langue, pour essayer hors de Windows.

use std::fmt::Display;
use std::sync::OnceLock;

use serde_json::Value;

/// Langue, recherche des clés, `{nom}`, langue de la session : le tronc
/// commun avec Admin et l'icône.
#[path = "../../admin/src/i18n/common.rs"]
mod common;
pub use common::Lang;

const FR: &str = include_str!("../i18n/fr.json");
const EN: &str = include_str!("../i18n/en.json");

fn catalog(lang: Lang) -> &'static Value {
    static CATALOGS: OnceLock<(Value, Value)> = OnceLock::new();
    let (fr, en) = CATALOGS.get_or_init(|| {
        (serde_json::from_str(FR).expect("i18n/fr.json"), serde_json::from_str(EN).expect("i18n/en.json"))
    });
    match lang {
        Lang::Fr => fr,
        Lang::En => en,
    }
}

fn lookup(lang: Lang, key: &str) -> Option<&'static str> {
    common::lookup(catalog(lang), key)
}

/// Texte de `key` (« ecran.cle ») dans `lang`, `{nom}` remplacé par sa valeur.
pub fn tr(lang: Lang, key: &str, args: &[(&str, &dyn Display)]) -> String {
    let text = lookup(lang, key).or_else(|| lookup(Lang::Fr, key)).unwrap_or(key);
    common::fill(text, |name| args.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string()))
}

/// [`tr`] dans la langue de l'installateur.
pub fn t(key: &str, args: &[(&str, &dyn Display)]) -> String {
    tr(lang(), key, args)
}

/// Langue de l'installateur, lue une fois.
pub fn lang() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(common::session_lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_keys_and_placeholders_in_both_languages() {
        common::check_catalogs(catalog(Lang::Fr), catalog(Lang::En)).unwrap();
    }

    #[test]
    fn detection_and_fallback() {
        assert_eq!(Lang::from_tag("fr-CA"), Lang::Fr);
        assert_eq!(Lang::from_tag("de-DE"), Lang::En);
        assert_eq!(tr(Lang::En, "absent.key", &[]), "absent.key");
        assert_eq!(tr(Lang::En, "ports.reservedByWindows", &[("port", &30001)]), "Port 30001 is reserved by Windows.");
    }
}
