//! Ce que les traductions d'Admin, de l'icône et de l'installateur ont en
//! commun (STWIN-46, docs/i18n.md) : la langue (`fr*` : français, sinon
//! anglais), la recherche d'une clé (« ecran.cle ») dans un catalogue JSON, le
//! remplissage des `{nom}`, le singulier et le pluriel, la langue de la
//! session (icône, installateur ; Admin suit celle de chaque requête) et la
//! vérification des catalogues. L'icône et l'installateur incluent ce fichier
//! par `#[path]` ; chacun garde ses textes. Dépendances : `serde_json`, et
//! `windows-sys` (Globalization) sous Windows.

// Chacun des trois programmes n'en prend qu'une partie.
#![allow(dead_code)]

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
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

    /// La forme du singulier pour `n` ? Français : 0 et 1 ; anglais : 1 seulement.
    pub fn singular(self, n: u64) -> bool {
        match self {
            Lang::Fr => n < 2,
            Lang::En => n == 1,
        }
    }
}

/// Le texte de `key` (« ecran.cle ») dans `root`.
pub fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(root, |node, part| node.get(part))?.as_str()
}

/// Remplace chaque `{nom}` de `template` par `value_of(nom)`, en un seul
/// passage (une valeur qui contient elle-même `{…}` n'est pas relue) ; un nom
/// inconnu reste tel quel.
pub fn fill(template: &str, mut value_of: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if !after[..end].is_empty() && after[..end].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') => {
                let name = &after[..end];
                match value_of(name) {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// La langue de la session : `KYBERNOW_LANG` s'il est donné (essais), sinon
/// celle du système ; français si rien ne la dit.
pub fn session_lang() -> Lang {
    if let Ok(forced) = std::env::var("KYBERNOW_LANG")
        && !forced.trim().is_empty()
    {
        return Lang::from_tag(&forced);
    }
    system().map_or(Lang::Fr, |tag| Lang::from_tag(&tag))
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

/// Mac : première langue préférée de l'utilisateur (`AppleLanguages`, par
/// exemple `"fr-FR"`, entre guillemets ou non), comme les apps de macOS.
#[cfg(target_os = "macos")]
fn system() -> Option<String> {
    let out = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output().ok()?;
    apple_languages(&String::from_utf8_lossy(&out.stdout))
}

/// La première langue de la liste que rend `defaults read -g AppleLanguages`.
pub fn apple_languages(text: &str) -> Option<String> {
    text.split('"').nth(1).map(str::to_string).or_else(|| {
        text.lines().map(|l| l.trim().trim_end_matches(',')).find(|l| !l.is_empty() && *l != "(" && *l != ")").map(str::to_string)
    })
}

/// Linux : la langue de la session (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`).
#[cfg(target_os = "linux")]
fn system() -> Option<String> {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| locale_tag(&std::env::var(v).ok()?))
}

/// `fr_FR.UTF-8`, `fr:en` → `fr-FR`, `fr` ; `C` et `POSIX` : rien.
pub fn locale_tag(value: &str) -> Option<String> {
    let first = value.split(':').next()?.split(['.', '@']).next()?.replace('_', "-");
    (!first.is_empty() && first != "C" && first != "POSIX").then_some(first)
}

/// Ailleurs (essais) : français, sauf `KYBERNOW_LANG`.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn system() -> Option<String> {
    None
}

/// Toutes les clés d'un catalogue (« ecran.cle »).
pub fn keys(root: &Value) -> Vec<String> {
    fn walk(prefix: &str, v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, v) in map {
                    walk(&if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") }, v, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }
    let mut out = Vec::new();
    walk("", root, &mut out);
    out.sort();
    out
}

/// Les deux catalogues ont-ils les mêmes clés, et chaque texte les mêmes
/// `{nom}` dans les deux langues ? Sinon, ce qui ne va pas (pour les essais).
pub fn check_catalogs(fr: &Value, en: &Value) -> Result<(), String> {
    let (fr_keys, en_keys) = (keys(fr), keys(en));
    let missing: Vec<_> = fr_keys.iter().filter(|k| !en_keys.contains(k)).collect();
    let extra: Vec<_> = en_keys.iter().filter(|k| !fr_keys.contains(k)).collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!("absentes en anglais : {missing:?} ; en trop : {extra:?}"));
    }
    let names = |s: &str| {
        let mut v = Vec::new();
        fill(s, |name| {
            v.push(name.to_string());
            Some(String::new())
        });
        v.sort();
        v
    };
    for key in fr_keys {
        let (f, e) = (lookup(fr, &key).unwrap_or_default(), lookup(en, &key).unwrap_or_default());
        if names(f) != names(e) {
            return Err(format!("{key} : {{…}} différents ({f} / {e})"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod common_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn langue_remplissage_et_catalogues() {
        assert_eq!(Lang::from_tag("fr-CA"), Lang::Fr);
        assert_eq!(Lang::from_tag("de-DE"), Lang::En);
        assert!(Lang::Fr.singular(0) && Lang::Fr.singular(1) && !Lang::Fr.singular(2));
        assert!(!Lang::En.singular(0) && Lang::En.singular(1));
        assert_eq!(fill("{a} et {b} {inconnu} {pas un nom} {", |n| (n != "inconnu").then(|| format!("<{n}>"))), "<a> et <b> {inconnu} {pas un nom} {");
        assert_eq!(fill("{a}", |_| Some("{b}".into())), "{b}", "une valeur n'est pas relue");
        assert_eq!(locale_tag("fr_FR.UTF-8"), Some("fr-FR".into()));
        assert_eq!(locale_tag("C.UTF-8"), None);
        assert_eq!(apple_languages("(\n    \"fr-FR\",\n    \"en-FR\"\n)"), Some("fr-FR".into()));
        assert_eq!(apple_languages("(\n    en,\n)"), Some("en".into()));
        let fr = json!({ "a": { "b": "Bonjour {nom}" } });
        assert_eq!(lookup(&fr, "a.b"), Some("Bonjour {nom}"));
        assert!(check_catalogs(&fr, &json!({ "a": { "b": "Hello {nom}" } })).is_ok());
        assert!(check_catalogs(&fr, &json!({ "a": { "b": "Hello {name}" } })).is_err());
        assert!(check_catalogs(&fr, &json!({ "a": {} })).is_err());
    }
}
