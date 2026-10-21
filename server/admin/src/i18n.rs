//! Langue des textes que la console renvoie aux personnes : messages d'erreur
//! et de refus de l'API, état des mises à jour, phrases du fil des événements,
//! réglages de Kyber (`/api/schema`).
//!
//! Les textes vivent dans `i18n/fr.json` (la source) et `i18n/en.json`, mêmes
//! clés, objets imbriqués par domaine ; `tr(lang, "cle", &[("nom", …)])`
//! remplace `{nom}`. Une clé absente en anglais retombe sur le français. Les
//! réglages de Kyber gardent leur français dans `schema.rs`, l'anglais est
//! dans `i18n/schema.en.json`.
//!
//! La langue d'une requête vient de son en-tête `Accept-Language` (première
//! langue préférée : français si elle commence par « fr », anglais sinon ;
//! **sans en-tête, français**, pour les apps d'avant les deux langues). Elle
//! est posée pour toute la requête (`scope`) et relue par `current()`, y
//! compris dans les fils de `api::blocking`.
//!
//! Un texte produit hors d'une requête et montré plus tard (événement du fil,
//! refus d'une version, échec d'une installation) est gardé en `Msg` (clé et
//! paramètres) et écrit au moment de l'afficher, dans la langue de qui le lit.
//! Les journaux restent en français (`Display` d'un `Msg`).

use std::cell::Cell;
use std::fmt;
use std::sync::LazyLock;

use axum::extract::Request;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

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

    /// Langue d'un en-tête `Accept-Language` : la langue préférée (plus forte
    /// pondération, puis ordre), français si elle commence par « fr »,
    /// anglais sinon. Sans en-tête (ou vide) : français.
    pub fn from_accept_language(value: Option<&str>) -> Lang {
        let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) else { return Lang::Fr };
        let mut best: Option<(&str, f32)> = None;
        for part in value.split(',') {
            let mut pieces = part.split(';');
            let tag = pieces.next().unwrap_or("").trim();
            if tag.is_empty() {
                continue;
            }
            let q = pieces
                .filter_map(|p| p.trim().strip_prefix("q="))
                .find_map(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            if q > 0.0 && best.is_none_or(|(_, b)| q > b) {
                best = Some((tag, q));
            }
        }
        match best {
            None => Lang::Fr,
            Some((tag, _)) if tag.to_ascii_lowercase().starts_with("fr") => Lang::Fr,
            Some(_) => Lang::En,
        }
    }
}

static FR: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("i18n/fr.json")).expect("i18n/fr.json"));
static EN: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(include_str!("i18n/en.json")).expect("i18n/en.json"));
static SCHEMA_EN: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("i18n/schema.en.json")).expect("i18n/schema.en.json"));

fn catalog(lang: Lang) -> &'static Value {
    match lang {
        Lang::Fr => &FR,
        Lang::En => &EN,
    }
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.').try_fold(root, |node, part| node.get(part))?.as_str()
}

/// Texte brut de `key` (repli : français, puis la clé elle-même).
fn text(lang: Lang, key: &str) -> &str {
    lookup(catalog(lang), key).or_else(|| lookup(&FR, key)).unwrap_or(key)
}

fn fill(template: &str, mut value_of: impl FnMut(&str) -> Option<String>) -> String {
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

/// Texte de `key` dans `lang`, `{nom}` remplacé par les paramètres.
pub fn tr(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    fill(text(lang, key), |name| params.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_string()))
}

// --- Langue de la requête en cours -------------------------------------------

tokio::task_local! {
    static REQUEST_LANG: Lang;
}

thread_local! {
    static THREAD_LANG: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Langue de la requête en cours (français hors requête).
pub fn current() -> Lang {
    REQUEST_LANG.try_with(|l| *l).ok().or_else(|| THREAD_LANG.with(Cell::get)).unwrap_or_default()
}

/// Exécute `f` (dans un fil de `spawn_blocking`) avec la langue `lang`.
pub fn with_lang<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let before = THREAD_LANG.with(|l| l.replace(Some(lang)));
    let out = f();
    THREAD_LANG.with(|l| l.set(before));
    out
}

/// Couche axum : la langue d'`Accept-Language` vaut pour toute la requête.
pub async fn scope(req: Request, next: Next) -> Response {
    let lang = Lang::from_accept_language(req.headers().get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()));
    REQUEST_LANG.scope(lang, next.run(req)).await
}

// --- Messages gardés (clé et paramètres) -------------------------------------

/// Un texte à écrire plus tard, dans la langue de qui le lit : sa clé et ses
/// paramètres. Un paramètre est un texte, un nombre, ou un objet :
/// - `{"key": …, "params": …}` : un autre message ;
/// - `{"list": ["A", "B"]}` : « A et B » / “A and B” ;
/// - `{"duration": secondes}` : « 42 minutes », « 2 h 05 » ;
/// - `{"ms": 8.3}` : « 8,3 ms » / “8.3 ms” ;
/// - `{"field": "video.encoder"}` : libellé d'un réglage de Kyber ;
/// - `{"fr": …, "en": …}` : texte déjà écrit dans chaque langue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Msg {
    pub key: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

impl Msg {
    pub fn new(key: &str) -> Self {
        Msg { key: key.to_string(), params: Map::new() }
    }

    /// Ajoute un paramètre (texte, nombre, ou objet décrit plus haut).
    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.params.insert(name.to_string(), value.into());
        self
    }

    /// Ajoute un message en paramètre.
    pub fn with_msg(self, name: &str, msg: Msg) -> Self {
        self.with(name, msg.to_value())
    }

    pub fn to_value(&self) -> Value {
        json!({ "key": self.key, "params": self.params })
    }

    pub fn render(&self, lang: Lang) -> String {
        render(lang, &self.key, &self.params)
    }

    /// Dans la langue de la requête en cours.
    pub fn text(&self) -> String {
        self.render(current())
    }
}

/// Français : les journaux et `{e:#}` restent en français.
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(Lang::Fr))
    }
}

impl std::error::Error for Msg {}

pub fn render(lang: Lang, key: &str, params: &Map<String, Value>) -> String {
    fill(text(lang, key), |name| params.get(name).map(|v| param(lang, v)))
}

fn param(lang: Lang, value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => items.iter().map(|v| param(lang, v)).collect::<Vec<_>>().join(", "),
        Value::Object(o) => {
            if let Some(key) = o.get("key").and_then(Value::as_str) {
                let empty = Map::new();
                render(lang, key, o.get("params").and_then(Value::as_object).unwrap_or(&empty))
            } else if let Some(items) = o.get("list").and_then(Value::as_array) {
                list(lang, &items.iter().map(|v| param(lang, v)).collect::<Vec<_>>())
            } else if let Some(seconds) = o.get("duration").and_then(Value::as_u64) {
                duration(lang, seconds)
            } else if let Some(ms) = o.get("ms").and_then(Value::as_f64) {
                milliseconds(lang, ms)
            } else if let Some(id) = o.get("field").and_then(Value::as_str) {
                field_label(lang, id)
            } else {
                o.get(lang.code()).or_else(|| o.get("fr")).and_then(Value::as_str).unwrap_or_default().to_string()
            }
        }
    }
}

/// Paramètre écrit d'avance dans chaque langue (une erreur de `anyhow`, par exemple).
pub fn both(write: impl Fn(Lang) -> String) -> Value {
    json!({ "fr": write(Lang::Fr), "en": write(Lang::En) })
}

/// « A », « A et B », « A, B et C ».
pub fn list(lang: Lang, items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => tr(lang, "fmt.list", &[("rest", &rest.join(", ")), ("last", last)]),
    }
}

/// « moins d'une minute », « 1 minute », « 42 minutes », « 2 h 05 ».
pub fn duration(lang: Lang, seconds: u64) -> String {
    let minutes = seconds / 60;
    match minutes {
        0 => tr(lang, "fmt.lessThanMinute", &[]),
        1 => tr(lang, "fmt.oneMinute", &[]),
        m if m < 60 => tr(lang, "fmt.minutes", &[("n", &m.to_string())]),
        m => tr(lang, "fmt.hours", &[("h", &(m / 60).to_string()), ("m", &format!("{:02}", m % 60))]),
    }
}

/// « 8,3 ms » (anglais : « 8.3 ms »), « 120 ms ».
pub fn milliseconds(lang: Lang, ms: f64) -> String {
    if ms >= 100.0 {
        format!("{} ms", ms.round() as u64)
    } else {
        let text = format!("{ms:.1} ms");
        if lang == Lang::Fr { text.replace('.', ",") } else { text }
    }
}

/// Erreur lisible (chaîne complète, comme `{e:#}`), les `Msg` de la chaîne
/// écrits dans `lang`.
pub fn error_text(lang: Lang, e: &anyhow::Error) -> String {
    e.chain()
        .map(|cause| match cause.downcast_ref::<Msg>() {
            Some(m) => m.render(lang),
            None => cause.to_string(),
        })
        .collect::<Vec<_>>()
        .join(": ")
}

/// Une erreur en paramètre d'un message gardé : écrite dans chaque langue.
pub fn error_param(e: &anyhow::Error) -> Value {
    both(|lang| error_text(lang, e))
}

/// `msg` avec sa cause en paramètre `{error}` (« GitHub ne répond pas : … »),
/// à la place d'un `context` que l'on ne saurait pas traduire.
pub fn because(msg: Msg, cause: &anyhow::Error) -> anyhow::Error {
    anyhow::Error::new(msg.with("error", error_param(cause)))
}

// --- Réglages de Kyber et notes de version -----------------------------------

/// Libellé d'un réglage de Kyber dans `lang`.
pub fn field_label(lang: Lang, id: &str) -> String {
    let english = (lang == Lang::En).then(|| SCHEMA_EN["fields"][id]["label"].as_str()).flatten();
    english
        .map(str::to_string)
        .or_else(|| crate::schema::fields().into_iter().find(|f| f.id == id).map(|f| f.label.to_string()))
        .unwrap_or_else(|| id.to_string())
}

/// Le catalogue des réglages (`/api/schema`) dans `lang` : l'anglais de
/// `schema.en.json` par-dessus le français de `schema.rs`.
pub fn localize_schema(lang: Lang, mut groups: Value, mut fields: Value) -> (Value, Value) {
    if lang == Lang::Fr {
        return (groups, fields);
    }
    let en = &*SCHEMA_EN;
    let patch = |target: &mut Value, from: &Value, names: &[&str]| {
        for name in names {
            if let (Some(slot), Some(text)) = (target.get_mut(*name), from.get(*name).and_then(Value::as_str))
                && !slot.is_null()
            {
                *slot = Value::String(text.to_string());
            }
        }
    };
    for g in groups.as_array_mut().into_iter().flatten() {
        let id = g["id"].as_str().unwrap_or_default().to_string();
        patch(g, &en["groups"][&id], &["label", "help"]);
    }
    for f in fields.as_array_mut().into_iter().flatten() {
        let id = f["id"].as_str().unwrap_or_default().to_string();
        let from = &en["fields"][&id];
        patch(f, from, &["label", "help", "details", "advice", "caution"]);
        for o in f["kind"]["options"].as_array_mut().into_iter().flatten() {
            let value = o["value"].as_str().unwrap_or_default().to_string();
            patch(o, &from["options"][&value], &["label", "hint"]);
        }
    }
    (groups, fields)
}

/// Notes d'une version publiée : la release porte le français, une ligne
/// `<!-- en -->`, puis l'anglais. Rend la partie de `lang` ; sans marqueur
/// (versions d'avant la 0.3.0, en français), tout le texte.
pub fn notes_for(notes: &str, lang: Lang) -> String {
    let mut offset = 0;
    for line in notes.split_inclusive('\n') {
        let bare = line.trim();
        if bare.starts_with("<!--") && bare.ends_with("-->") && bare[4..bare.len() - 3].trim().eq_ignore_ascii_case("en") {
            let (before, after) = (&notes[..offset], &notes[offset + line.len()..]);
            return match lang {
                Lang::Fr => before.trim().to_string(),
                Lang::En => after.trim().to_string(),
            };
        }
        offset += line.len();
    }
    notes.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(node: &Value, prefix: &str, out: &mut Vec<(String, String)>) {
        if let Some(o) = node.as_object() {
            for (k, v) in o {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                match v {
                    Value::Object(_) => keys(v, &key, out),
                    other => out.push((key, other.as_str().unwrap_or_default().to_string())),
                }
            }
        }
    }

    fn placeholders(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        fill(text, |name| {
            found.push(name.to_string());
            Some(String::new())
        });
        found.sort();
        found
    }

    #[test]
    fn catalogues_alignes() {
        let (mut fr, mut en) = (Vec::new(), Vec::new());
        keys(&FR, "", &mut fr);
        keys(&EN, "", &mut en);
        let fr_keys: Vec<_> = fr.iter().map(|(k, _)| k.clone()).collect();
        let en_keys: Vec<_> = en.iter().map(|(k, _)| k.clone()).collect();
        for k in &fr_keys {
            assert!(en_keys.contains(k), "absent de en.json : {k}");
        }
        for k in &en_keys {
            assert!(fr_keys.contains(k), "absent de fr.json : {k}");
        }
        for (k, v) in &fr {
            let english = &en.iter().find(|(e, _)| e == k).unwrap().1;
            assert_eq!(placeholders(v), placeholders(english), "paramètres de {k}");
        }
    }

    #[test]
    fn schema_entierement_traduit() {
        let en = &*SCHEMA_EN;
        for g in crate::schema::GROUPS {
            for name in ["label", "help"] {
                assert!(en["groups"][g.id][name].is_string(), "groupe {} : {name}", g.id);
            }
        }
        for f in crate::schema::fields() {
            let e = &en["fields"][f.id];
            for name in ["label", "help", "details", "advice"] {
                assert!(e[name].is_string(), "{} : {name}", f.id);
            }
            assert_eq!(f.caution.is_some(), e["caution"].is_string(), "{} : caution", f.id);
            if let crate::schema::Kind::Enum { options } = f.kind {
                for o in options {
                    assert!(e["options"][o.value]["label"].is_string() && e["options"][o.value]["hint"].is_string(), "{} : {}", f.id, o.value);
                }
            }
        }
        let schema = serde_json::to_value(crate::schema::fields()).unwrap();
        let groups = serde_json::to_value(crate::schema::GROUPS).unwrap();
        let (_, fields) = localize_schema(Lang::En, groups, schema);
        assert_eq!(fields[0]["label"], "Video encoder");
    }

    #[test]
    fn langue_de_la_requete() {
        assert_eq!(Lang::from_accept_language(None), Lang::Fr);
        assert_eq!(Lang::from_accept_language(Some("")), Lang::Fr);
        assert_eq!(Lang::from_accept_language(Some("en")), Lang::En);
        assert_eq!(Lang::from_accept_language(Some("fr")), Lang::Fr);
        assert_eq!(Lang::from_accept_language(Some("fr-CA,en;q=0.8")), Lang::Fr);
        assert_eq!(Lang::from_accept_language(Some("en-US,en;q=0.9,fr;q=0.8")), Lang::En);
        assert_eq!(Lang::from_accept_language(Some("de-DE")), Lang::En);
        assert_eq!(Lang::from_accept_language(Some("en;q=0.5, fr;q=0.9")), Lang::Fr);
        assert_eq!(Lang::from_accept_language(Some("*")), Lang::En);
    }

    #[test]
    fn messages_et_parametres() {
        let m = Msg::new("events.disconnected")
            .with("name", "MacBook")
            .with("duration", json!({ "duration": 7500 }))
            .with_msg("latency", Msg::new("events.latencySuffix").with("ms", json!({ "ms": 8.3 })))
            .with("takeover", "");
        assert_eq!(m.render(Lang::Fr), "MacBook s'est déconnecté après 2 h 05, écart médian 8,3 ms");
        assert_eq!(m.render(Lang::En), "MacBook disconnected after 2 h 05, median latency 8.3 ms");
        assert_eq!(m.to_string(), m.render(Lang::Fr));
        // Relu tel qu'enregistré.
        let back: Msg = serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        assert_eq!(list(Lang::En, &["A".into(), "B".into(), "C".into()]), "A, B and C");
        assert_eq!(list(Lang::Fr, &["A".into(), "B".into()]), "A et B");
        assert_eq!(tr(Lang::En, "cle.inconnue", &[]), "cle.inconnue");
        assert_eq!(fill("{a} {b} {", |n| (n == "a").then(|| "1".into())), "1 {b} {");
    }

    #[test]
    fn erreurs_dans_la_langue_demandee() {
        let e = anyhow::Error::new(Msg::new("auth.wrongCurrentPassword")).context("contexte");
        assert_eq!(error_text(Lang::En, &e), "contexte: incorrect current password");
        assert_eq!(format!("{e:#}"), "contexte: mot de passe actuel incorrect");
    }

    #[tokio::test]
    async fn langue_portee_par_la_requete() {
        assert_eq!(current(), Lang::Fr);
        let inside = REQUEST_LANG.scope(Lang::En, async { current() }).await;
        assert_eq!(inside, Lang::En);
        assert_eq!(with_lang(Lang::En, current), Lang::En);
        assert_eq!(current(), Lang::Fr);
    }

    #[test]
    fn notes_de_version_par_langue() {
        let notes = "## Nouveau\n\n- Une chose\n\n<!-- en -->\n\n## New\n\n- One thing\n";
        assert_eq!(notes_for(notes, Lang::Fr), "## Nouveau\n\n- Une chose");
        assert_eq!(notes_for(notes, Lang::En), "## New\n\n- One thing");
        // Sans marqueur : tout le texte, quelle que soit la langue.
        assert_eq!(notes_for("Seulement en français", Lang::En), "Seulement en français");
        // Le marqueur doit être seul sur sa ligne ; espaces et fins de ligne Windows tolérés.
        assert_eq!(notes_for("Voir <!-- en --> plus bas", Lang::En), "Voir <!-- en --> plus bas");
        assert_eq!(notes_for("Avant\r\n  <!--en-->  \r\nAfter\r\n", Lang::En), "After");
        assert_eq!(notes_for("Avant\r\n<!-- EN -->\r\nAfter", Lang::Fr), "Avant");
    }
}
