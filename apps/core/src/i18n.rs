//! Langue des clients (docs/i18n.md) : français si la langue du système
//! commence par `fr`, anglais pour toute autre ; choix manuel possible
//! (« Auto », « Français », « English »).
//!
//! Les textes vivent dans des catalogues JSON par langue (`fr.json`,
//! `en.json`, mêmes clés, objets imbriqués), embarqués avec `include_str!`.
//! Le français est la source : une clé absente en anglais retombe sur lui.
//! Ceux du cœur sont dans `src/i18n/` ; l'app Windows a les siens et se sert
//! du même [`Catalog`].
//!
//! La langue choisie est aussi celle envoyée à la console (`Accept-Language`)
//! et celle des notes de version ([`release_notes`]).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    /// `fr` ou `en` (en-tête `Accept-Language`, interfaces).
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    /// Langue d'après une étiquette du système (`fr-FR`, `fr_CA`, `en-US`,
    /// `de`…) : français pour `fr…`, anglais pour tout le reste.
    pub fn from_tag(tag: &str) -> Lang {
        let t = tag.trim().to_ascii_lowercase();
        if t == "fr" || t.starts_with("fr-") || t.starts_with("fr_") { Lang::Fr } else { Lang::En }
    }

    /// Choix de l'app (`auto`, `fr`, `en`) ; `auto` (ou inconnu) : la langue
    /// du système, lue seulement dans ce cas.
    pub fn resolve(choice: &str, system: impl FnOnce() -> Option<String>) -> Lang {
        match choice {
            "fr" => Lang::Fr,
            "en" => Lang::En,
            _ => system().map(|t| Lang::from_tag(&t)).unwrap_or(Lang::En),
        }
    }
}

/// Langue en cours de l'app (français tant qu'elle n'est pas posée).
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 { Lang::En } else { Lang::Fr }
}

pub fn set_current(lang: Lang) {
    CURRENT.store(if lang == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

/// Un catalogue de textes en deux langues, lu à la première recherche.
pub struct Catalog {
    fr: &'static str,
    en: &'static str,
    parsed: OnceLock<(Value, Value)>,
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(root, |v, k| v.get(k))?.as_str()
}

fn fill(text: &str, params: &[(&str, &str)]) -> String {
    let mut out = text.to_string();
    for (name, value) in params {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// Clés (chemins `a.b.c`) d'un catalogue, triées.
fn keys(v: &Value, prefix: &str, out: &mut Vec<String>) {
    if let Value::Object(map) = v {
        for (k, child) in map {
            let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
            if child.is_object() {
                keys(child, &path, out);
            } else {
                out.push(path);
            }
        }
    }
}

impl Catalog {
    pub const fn new(fr: &'static str, en: &'static str) -> Self {
        Self { fr, en, parsed: OnceLock::new() }
    }

    fn parsed(&self) -> &(Value, Value) {
        self.parsed.get_or_init(|| {
            let read = |s: &str| serde_json::from_str(s).unwrap_or(Value::Null);
            (read(self.fr), read(self.en))
        })
    }

    /// Texte de `key` dans `lang`, `{nom}` remplacés ; repli sur le
    /// français, puis sur la clé elle-même.
    pub fn tr(&self, lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
        let (fr, en) = self.parsed();
        let text = match lang {
            Lang::En => lookup(en, key).or_else(|| lookup(fr, key)),
            Lang::Fr => lookup(fr, key),
        };
        fill(text.unwrap_or(key), params)
    }

    /// Clés présentes dans une langue et pas dans l'autre (vide : les deux
    /// catalogues se correspondent). Pour les tests.
    pub fn mismatched_keys(&self) -> Vec<String> {
        let (fr, en) = self.parsed();
        assert!(fr.is_object() && en.is_object(), "catalogue illisible");
        let (mut a, mut b) = (Vec::new(), Vec::new());
        keys(fr, "", &mut a);
        keys(en, "", &mut b);
        let mut out: Vec<String> = a.iter().filter(|k| !b.contains(k)).map(|k| format!("fr seulement : {k}")).collect();
        out.extend(b.iter().filter(|k| !a.contains(k)).map(|k| format!("en seulement : {k}")));
        out
    }
}

static CORE: Catalog = Catalog::new(include_str!("i18n/fr.json"), include_str!("i18n/en.json"));

/// Texte du cœur (erreurs de la console, de la session, des mises à jour).
pub fn tr(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    CORE.tr(lang, key, params)
}

/// Ligne qui sépare les notes de version françaises (avant) des anglaises
/// (après), dans le texte d'une release GitHub.
pub const NOTES_MARKER: &str = "<!-- en -->";

/// La partie des notes de version dans `lang` ; tout le texte s'il n'y a pas
/// de marqueur (versions d'avant la 0.3.0, en français).
pub fn release_notes(body: &str, lang: Lang) -> String {
    let mut fr = Vec::new();
    let mut en = Vec::new();
    let mut seen = false;
    for line in body.lines() {
        if !seen && line.trim() == NOTES_MARKER {
            seen = true;
            continue;
        }
        if seen { en.push(line) } else { fr.push(line) }
    }
    if !seen {
        return body.to_string();
    }
    let part = if lang == Lang::En { en } else { fr };
    part.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langue_du_systeme() {
        for t in ["fr", "fr-FR", "fr-CA", "FR-be", "fr_CH", " fr-LU "] {
            assert_eq!(Lang::from_tag(t), Lang::Fr, "{t}");
        }
        for t in ["en-US", "en-GB", "de-DE", "es", "", "frr", "fy-NL"] {
            assert_eq!(Lang::from_tag(t), Lang::En, "{t}");
        }
    }

    #[test]
    fn choix_manuel() {
        assert_eq!(Lang::resolve("fr", || Some("en-US".into())), Lang::Fr);
        assert_eq!(Lang::resolve("en", || Some("fr-FR".into())), Lang::En);
        assert_eq!(Lang::resolve("auto", || Some("fr-FR".into())), Lang::Fr);
        assert_eq!(Lang::resolve("auto", || Some("ja-JP".into())), Lang::En);
        assert_eq!(Lang::resolve("auto", || None), Lang::En);
        assert_eq!(Lang::resolve("", || Some("fr".into())), Lang::Fr);
    }

    #[test]
    fn textes_et_repli() {
        let c = Catalog::new(r#"{"a":{"b":"Bonjour {nom}"},"seul":"FR"}"#, r#"{"a":{"b":"Hello {nom}"}}"#);
        assert_eq!(c.tr(Lang::Fr, "a.b", &[("nom", "Ada")]), "Bonjour Ada");
        assert_eq!(c.tr(Lang::En, "a.b", &[("nom", "Ada")]), "Hello Ada");
        assert_eq!(c.tr(Lang::En, "seul", &[]), "FR");
        assert_eq!(c.tr(Lang::En, "absente", &[]), "absente");
        assert_eq!(c.mismatched_keys(), vec!["fr seulement : seul".to_string()]);
    }

    #[test]
    fn catalogues_du_coeur_complets() {
        assert_eq!(CORE.mismatched_keys(), Vec::<String>::new());
    }

    #[test]
    fn notes_par_langue() {
        let body = "## 0.3.0\n- Nouveau.\n\n<!-- en -->\n## 0.3.0\n- New.\n";
        assert_eq!(release_notes(body, Lang::Fr), "## 0.3.0\n- Nouveau.");
        assert_eq!(release_notes(body, Lang::En), "## 0.3.0\n- New.");
        // Marqueur entouré d'espaces, fins de ligne Windows.
        let crlf = "Français\r\n  <!-- en -->  \r\nEnglish\r\n";
        assert_eq!(release_notes(crlf, Lang::Fr), "Français");
        assert_eq!(release_notes(crlf, Lang::En), "English");
        // Sans marqueur : tout le texte, quelle que soit la langue.
        assert_eq!(release_notes("- Ancienne version.", Lang::En), "- Ancienne version.");
        // Le marqueur au milieu d'une ligne n'en est pas un.
        let inline = "Voir <!-- en --> ici";
        assert_eq!(release_notes(inline, Lang::En), inline);
    }
}
