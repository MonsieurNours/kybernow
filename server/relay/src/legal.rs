//! Les textes juridiques du service (lot 1b ; docs/juridique/README.md) :
//! `/conditions` et `/confidentialite`, servis par le relais quand il tient des
//! comptes, en français ou en anglais (`?lang=`).
//!
//! Les textes sont des copies de `docs/juridique/*.md` dans `assets/legal/`
//! (l'image Docker ne voit que `server/relay` ; un essai vérifie qu'elles sont à
//! jour), embarquées dans le binaire et mises en HTML au premier appel par le
//! petit lecteur Markdown d'ici (titres, paragraphes, listes, citations,
//! tableaux, gras, code, liens). Ce sont des **brouillons** : chaque page le dit
//! en tête, tant qu'un juriste ne les a pas relus.
//!
//! La version de chaque texte (« Version 0.1 du 23/11/2026 ») est lue dans le
//! texte français ; l'inscription la note sur le compte.

use std::sync::OnceLock;

use crate::accounts::pages::esc;
use crate::pages::{Lang, t};

const TERMS_FR: &str = include_str!("../assets/legal/conditions.md");
const TERMS_EN: &str = include_str!("../assets/legal/conditions.en.md");
const PRIVACY_FR: &str = include_str!("../assets/legal/confidentialite.md");
const PRIVACY_EN: &str = include_str!("../assets/legal/confidentialite.en.md");

/// L'adresse des autres documents du dépôt (contrat de sous-traitance, cadrage).
const REPO_DOCS: &str = "https://github.com/MonsieurNours/kybernow/blob/main/docs/juridique/";

/// Un texte juridique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doc {
    /// Conditions d'utilisation.
    Terms,
    /// Politique de confidentialité.
    Privacy,
}

impl Doc {
    /// Le texte d'une adresse : `/conditions`, `/confidentialite`.
    pub fn of_path(path: &str) -> Option<Doc> {
        match path.trim_end_matches('/') {
            "/conditions" => Some(Doc::Terms),
            "/confidentialite" => Some(Doc::Privacy),
            _ => None,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            Doc::Terms => "/conditions",
            Doc::Privacy => "/confidentialite",
        }
    }

    fn source(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Doc::Terms, Lang::Fr) => TERMS_FR,
            (Doc::Terms, Lang::En) => TERMS_EN,
            (Doc::Privacy, Lang::Fr) => PRIVACY_FR,
            (Doc::Privacy, Lang::En) => PRIVACY_EN,
        }
    }

    /// La version du texte (« 0.1 »), lue dans le texte français.
    pub fn version(self) -> &'static str {
        static VERSIONS: OnceLock<[String; 2]> = OnceLock::new();
        let v = VERSIONS.get_or_init(|| [version_of(TERMS_FR).unwrap_or_default(), version_of(PRIVACY_FR).unwrap_or_default()]);
        match self {
            Doc::Terms => &v[0],
            Doc::Privacy => &v[1],
        }
    }

    /// La page HTML du texte.
    pub fn page(self, lang: Lang) -> String {
        let title = match self {
            Doc::Terms => t(lang, "Conditions d'utilisation", "Terms of use"),
            Doc::Privacy => t(lang, "Politique de confidentialité", "Privacy policy"),
        };
        let draft = t(
            lang,
            &format!("Brouillon, version {} : ce texte n'est pas encore en vigueur.", self.version()),
            &format!("Draft, version {}: this text is not in force yet.", self.version()),
        );
        let body = format!("<p class=\"legal-draft\" role=\"note\">{draft}</p>\n<article class=\"legal\">\n{}</article>", markdown(self.source(lang)));
        crate::pages::simple(lang, &format!("{title} ({})", t(lang, "brouillon", "draft")), self.path(), &body)
    }
}

/// « Version 0.1 du 23/11/2026 » → « 0.1 ».
pub fn version_of(text: &str) -> Option<String> {
    let rest = &text[text.find("Version ")? + "Version ".len()..];
    let v: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let v = v.trim_end_matches('.');
    (!v.is_empty()).then(|| v.to_string())
}

/// Le lien d'un document : les textes servis ici, les autres sur le dépôt.
fn link(url: &str) -> String {
    let (file, lang) = match url.strip_suffix(".en.md") {
        Some(f) => (f, "en"),
        None => (url.strip_suffix(".md").unwrap_or(url), "fr"),
    };
    match file {
        "conditions" => format!("/conditions?lang={lang}"),
        "confidentialite" => format!("/confidentialite?lang={lang}"),
        _ if url.starts_with("https://") => url.to_string(),
        _ => format!("{REPO_DOCS}{url}"),
    }
}

/// Le Markdown d'une ligne : code, liens, gras ; le reste échappé.
fn inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if let Some(r) = rest.strip_prefix('`')
            && let Some(end) = r.find('`')
        {
            out.push_str(&format!("<code>{}</code>", esc(&r[..end])));
            rest = &r[end + 1..];
            continue;
        }
        if let Some(r) = rest.strip_prefix("**")
            && let Some(end) = r.find("**")
        {
            out.push_str(&format!("<b>{}</b>", inline(&r[..end])));
            rest = &r[end + 2..];
            continue;
        }
        if let Some(r) = rest.strip_prefix('[')
            && let Some(close) = r.find(']')
            && let Some(after) = r[close + 1..].strip_prefix('(')
            && let Some(end) = after.find(')')
        {
            out.push_str(&format!("<a href=\"{}\">{}</a>", esc(&link(&after[..end])), inline(&r[..close])));
            rest = &after[end + 1..];
            continue;
        }
        out.push_str(&esc(&c.to_string()));
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn cells(line: &str) -> Vec<&str> {
    line.trim().trim_start_matches('|').trim_end_matches('|').split('|').map(str::trim).collect()
}

/// Le Markdown des textes juridiques en HTML.
pub fn markdown(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        // Titres : le # du document devient le titre de la page ; ## et ### suivent.
        if let Some(level) = (1..=4).find(|&n| trimmed.starts_with(&format!("{} ", "#".repeat(n)))) {
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline(trimmed[level + 1..].trim())));
            i += 1;
            continue;
        }
        if trimmed.starts_with('>') {
            let mut quote = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            out.push_str(&format!("<blockquote><p>{}</p></blockquote>\n", inline(&quote.join(" "))));
            continue;
        }
        if trimmed.starts_with('|') {
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                rows.push(cells(lines[i]));
                i += 1;
            }
            out.push_str("<div class=\"legal-table\"><table>\n");
            for (n, row) in rows.iter().enumerate() {
                // La ligne |---|---| sépare l'en-tête.
                if n == 1 && row.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')) {
                    continue;
                }
                let tag = if n == 0 { "th" } else { "td" };
                let row: String = row.iter().map(|c| format!("<{tag}>{}</{tag}>", inline(c))).collect();
                out.push_str(&format!("<tr>{row}</tr>\n"));
            }
            out.push_str("</table></div>\n");
            continue;
        }
        if trimmed.starts_with("- ") {
            // Une liste, et ses sous-listes (deux espaces de plus).
            out.push_str("<ul>\n");
            let (mut open, mut nested) = (false, false);
            while i < lines.len() && lines[i].trim().starts_with("- ") {
                let indent = lines[i].len() - lines[i].trim_start().len();
                let item = inline(lines[i].trim()[2..].trim());
                if indent >= 2 {
                    if !nested {
                        out.push_str("\n<ul>\n");
                        nested = true;
                    }
                    out.push_str(&format!("<li>{item}</li>\n"));
                } else {
                    if nested {
                        out.push_str("</ul>\n");
                        nested = false;
                    }
                    if open {
                        out.push_str("</li>\n");
                    }
                    out.push_str(&format!("<li>{item}"));
                    open = true;
                }
                i += 1;
            }
            if nested {
                out.push_str("</ul>\n");
            }
            if open {
                out.push_str("</li>\n");
            }
            out.push_str("</ul>\n");
            continue;
        }
        // Un paragraphe : les lignes qui suivent, jusqu'à une ligne vide ou un autre bloc.
        let mut para = vec![trimmed];
        i += 1;
        while i < lines.len() {
            let next = lines[i].trim();
            if next.is_empty() || next.starts_with('#') || next.starts_with('>') || next.starts_with('|') || next.starts_with("- ") {
                break;
            }
            para.push(next);
            i += 1;
        }
        out.push_str(&format!("<p>{}</p>\n", inline(&para.join(" "))));
    }
    out
}

/// La page d'une adresse, s'il y en a une.
pub fn page(path: &str, lang: Lang) -> Option<String> {
    Doc::of_path(path).map(|doc| doc.page(lang))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_lues_dans_les_textes() {
        assert_eq!(version_of("blabla. Version 0.1 du 23/11/2026."), Some("0.1".into()));
        assert_eq!(version_of("Version 2."), Some("2".into()));
        assert_eq!(version_of("rien"), None);
        for doc in [Doc::Terms, Doc::Privacy] {
            assert!(!doc.version().is_empty(), "{doc:?} sans version");
            for lang in [Lang::Fr, Lang::En] {
                assert_eq!(version_of(doc.source(lang)).as_deref(), Some(doc.version()), "{doc:?} : même version en français et en anglais");
            }
        }
    }

    #[test]
    fn copies_a_jour() {
        // Dans le dépôt, les copies d'assets/legal suivent docs/juridique (l'image Docker n'a que server/relay).
        let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/juridique");
        if !docs.exists() {
            return;
        }
        for (name, copy) in [("conditions.md", TERMS_FR), ("conditions.en.md", TERMS_EN), ("confidentialite.md", PRIVACY_FR), ("confidentialite.en.md", PRIVACY_EN)] {
            let original = std::fs::read_to_string(docs.join(name)).unwrap();
            assert_eq!(original, copy, "assets/legal/{name} n'est plus à jour : cp docs/juridique/{name} server/relay/assets/legal/");
        }
    }

    #[test]
    fn markdown() {
        let html = super::markdown(
            "# Titre\n\nUn **gras** et `<code>` et [lien](confidentialite.en.md), [autre](dpa.md) [pas un lien].\nsuite\n\n> **Note.** a\n> b\n\n- un\n  - sous-un\n  - sous-deux\n- deux\n\n| A | B |\n|---|---|\n| <x> | **y** |\n",
        );
        assert!(html.contains("<h1>Titre</h1>"));
        assert!(html.contains("<p>Un <b>gras</b> et <code>&lt;code&gt;</code> et <a href=\"/confidentialite?lang=en\">lien</a>, "));
        assert!(html.contains(&format!("<a href=\"{REPO_DOCS}dpa.md\">autre</a> [pas un lien]. suite</p>")));
        assert!(html.contains("<blockquote><p><b>Note.</b> a b</p></blockquote>"));
        assert!(html.contains("<ul>\n<li>un\n<ul>\n<li>sous-un</li>\n<li>sous-deux</li>\n</ul>\n</li>\n<li>deux</li>\n</ul>"), "{html}");
        assert!(html.contains("<tr><th>A</th><th>B</th></tr>\n<tr><td>&lt;x&gt;</td><td><b>y</b></td></tr>"));
        assert!(!html.contains("---"));
    }

    #[test]
    fn pages_servies_et_marquees_brouillon() {
        for doc in [Doc::Terms, Doc::Privacy] {
            for lang in [Lang::Fr, Lang::En] {
                let page = page(doc.path(), lang).unwrap();
                assert!(page.contains("class=\"legal-draft\""), "{doc:?} : brouillon");
                assert!(page.contains(&format!("<html lang=\"{}\">", lang.code())));
                assert!(!page.contains("<script") && !page.contains("](") && !page.contains("**"), "{doc:?} : Markdown resté");
                assert!(!page.contains(".md\">") || page.contains(REPO_DOCS), "{doc:?} : lien vers un .md local");
            }
        }
        assert!(page("/conditions", Lang::Fr).unwrap().contains("Brouillon, version"));
        assert!(page("/conditions/", Lang::En).unwrap().contains("Draft, version"));
        assert!(page("/privacy", Lang::Fr).is_none());
    }
}
