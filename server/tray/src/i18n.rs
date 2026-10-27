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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    /// `fr`, `fr-FR`, `fr-CA`… : français ; toute autre langue : anglais.
    pub fn from_tag(tag: &str) -> Lang {
        if tag.trim().to_ascii_lowercase().starts_with("fr") { Lang::Fr } else { Lang::En }
    }
}

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
    key.split('.').try_fold(catalog(lang), |v, part| v.get(part))?.as_str()
}

/// Texte de `key` (« ecran.cle ») dans `lang`, `{nom}` remplacé par sa valeur.
pub fn tr(lang: Lang, key: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut text = lookup(lang, key).or_else(|| lookup(Lang::Fr, key)).unwrap_or(key).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// [`tr`] dans la langue de l'icône.
pub fn t(key: &str, args: &[(&str, &dyn Display)]) -> String {
    tr(lang(), key, args)
}

/// Texte au singulier ou au pluriel : `key` a deux formes, `.one` et `.other`
/// (`{n}` y est le nombre). Français : 0 et 1 au singulier ; anglais : 1 seulement.
pub fn tn(key: &str, n: usize) -> String {
    let one = match lang() {
        Lang::Fr => n < 2,
        Lang::En => n == 1,
    };
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
    *LANG.get_or_init(detect)
}

fn detect() -> Lang {
    if let Ok(forced) = std::env::var("KYBERNOW_LANG")
        && !forced.trim().is_empty()
    {
        return Lang::from_tag(&forced);
    }
    system().map_or(Lang::Fr, |tag| Lang::from_tag(&tag))
}

/// Script posé dans le panneau avant son chargement : sa langue et ses
/// textes (`panel` de la langue, et du français pour le repli).
pub fn panel_script() -> String {
    let lang = lang();
    format!(
        "window.__knLang = {:?}; window.__knI18n = {{ strings: {}, fallback: {} }};",
        lang.code(),
        catalog(lang)["panel"],
        catalog(Lang::Fr)["panel"]
    )
}

/// Première langue d'affichage de l'utilisateur (« fr-FR », « en-US »…).
#[cfg(windows)]
fn system() -> Option<String> {
    use windows_sys::Win32::Globalization::{GetUserDefaultUILanguage, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    unsafe {
        let (mut count, mut len) = (0u32, 0u32);
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, std::ptr::null_mut(), &mut len) != 0 && len > 0 {
            let mut buf = vec![0u16; len as usize];
            if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len) != 0 {
                let first = buf.split(|c| *c == 0).next().unwrap_or_default();
                if !first.is_empty() {
                    return Some(String::from_utf16_lossy(first));
                }
            }
        }
        // LANGID : langue principale dans les 10 bits du bas (LANG_FRENCH = 0x0C).
        let id = GetUserDefaultUILanguage();
        Some(if id & 0x3FF == 0x0C { "fr".into() } else { "en".into() })
    }
}

/// Hors Windows (essais sur Mac) : français, sauf `KYBERNOW_LANG`.
#[cfg(not(windows))]
fn system() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(prefix: &str, v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, v) in map {
                    keys(&if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, v, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }

    #[test]
    fn same_keys_in_both_languages() {
        let (mut fr, mut en) = (Vec::new(), Vec::new());
        keys("", catalog(Lang::Fr), &mut fr);
        keys("", catalog(Lang::En), &mut en);
        fr.sort();
        en.sort();
        let missing: Vec<_> = fr.iter().filter(|k| !en.contains(k)).collect();
        let extra: Vec<_> = en.iter().filter(|k| !fr.contains(k)).collect();
        assert!(missing.is_empty() && extra.is_empty(), "absentes en anglais : {missing:?} ; en trop : {extra:?}");
    }

    #[test]
    fn placeholders_match() {
        let mut fr = Vec::new();
        keys("", catalog(Lang::Fr), &mut fr);
        let names = |s: &str| {
            let mut v: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split('}').next()).map(str::to_owned).collect();
            v.sort();
            v
        };
        for key in fr {
            let (f, e) = (lookup(Lang::Fr, &key).unwrap(), lookup(Lang::En, &key).unwrap_or_default());
            assert_eq!(names(f), names(e), "{key}");
        }
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
