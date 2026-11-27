//! Langue de l'icône : celle de Windows (`fr*` : français, sinon anglais).
//! L'icône n'a pas de réglages : pas de choix manuel (voir `docs/i18n.md`).
//! Textes dans `i18n/fr.json` et `i18n/en.json`, embarqués : `tray` pour le
//! menu, les infobulles et les boîtes de dialogue, `panel` pour le panneau
//! (passé à la page par [`panel_script`]). Une clé absente en anglais
//! retombe sur le français. Les lignes de `eprintln!` restent en français.
//!
//! `KYBERNOW_LANG=en` (ou `fr`) force la langue, pour essayer.

use std::fmt::Display;
use std::sync::OnceLock;

use serde_json::Value;

/// Langue, recherche des clés, `{nom}`, langue de la session : le tronc
/// commun avec Admin et l'installateur.
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

/// [`tr`] dans la langue de l'icône.
pub fn t(key: &str, args: &[(&str, &dyn Display)]) -> String {
    tr(lang(), key, args)
}

/// Texte au singulier ou au pluriel : `key` a deux formes, `.one` et `.other`
/// (`{n}` y est le nombre). Français : 0 et 1 au singulier ; anglais : 1 seulement.
pub fn tn(key: &str, n: usize) -> String {
    let one = lang().singular(n as u64);
    t(&format!("{key}.{}", if one { "one" } else { "other" }), &[("n", &n)])
}

/// « Léa », « Léa et Max », « Léa, Max et Zoé » (« Léa, Max and Zoé »).
pub fn list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => t("tray.list", &[("rest", &rest.join(", ")), ("last", last)]),
    }
}

/// Langue de l'icône, lue une fois.
pub fn lang() -> Lang {
    static LANG: OnceLock<Lang> = OnceLock::new();
    *LANG.get_or_init(common::session_lang)
}

/// Script posé dans le panneau avant son chargement : sa langue et ses
/// textes (`panel` de la langue, et du français pour le repli). Le
/// panneau n'existe que sous Windows.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn panel_script() -> String {
    let lang = lang();
    format!(
        "window.__knLang = {:?}; window.__knI18n = {{ strings: {}, fallback: {} }};",
        lang.code(),
        catalog(lang)["panel"],
        catalog(Lang::Fr)["panel"]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_keys_and_placeholders_in_both_languages() {
        common::check_catalogs(catalog(Lang::Fr), catalog(Lang::En)).unwrap();
    }

    /// Chaque `t('cle')` de `panel.html` existe dans `panel` du français.
    #[test]
    fn panel_keys_exist() {
        let html = include_str!("panel.html");
        // `t('` précédé d'autre chose qu'une lettre (pas `format('`, `closest('`…).
        let used: Vec<&str> = html
            .match_indices("t('")
            .filter(|(i, _)| !html[..*i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.'))
            .filter_map(|(i, _)| html[i + 3..].split('\'').next())
            .collect();
        assert!(used.len() > 20, "{used:?}");
        for key in used {
            assert!(lookup(Lang::Fr, &format!("panel.{key}")).is_some(), "panel.{key} absente de fr.json");
        }
    }

    #[test]
    fn detection_and_fallback() {
        assert_eq!(Lang::from_tag("fr-CA"), Lang::Fr);
        assert_eq!(Lang::from_tag("de-DE"), Lang::En);
        assert_eq!(tr(Lang::En, "absent.key", &[]), "absent.key");
        assert_eq!(tr(Lang::En, "tray.errors.serviceTimeout", &[("service", &"kyber"), ("seconds", &30)]), "The “kyber” service is not responding after 30 seconds.");
        assert!(panel_script().contains("window.__knI18n"));
    }
}
