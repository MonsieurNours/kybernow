//! Les fichiers servis sous `/assets/` : la feuille de styles, les polices, le
//! logo et le favicon, embarqués dans le binaire (l'image Docker ne voit que
//! `server/relay`). Les copies viennent de `scripts/relay-assets.py` (jetons de
//! la charte, polices réduites au latin) et de `brand/v2/tools/generate.mjs`
//! (logo, favicon) ; `share.css` est écrit à la main.
//!
//! Chaque adresse porte la version des fichiers (`?v=…`, un condensé de leur
//! contenu) : les réponses se gardent un an, et un fichier changé change
//! d'adresse.

use std::sync::OnceLock;

const TOKENS: &str = include_str!("../assets/kybernow.css");
const STYLE: &str = include_str!("../assets/share.css");
const ARCHIVO: &[u8] = include_bytes!("../assets/archivo.woff2");
const MARTIAN_MONO: &[u8] = include_bytes!("../assets/martian-mono.woff2");
const LOGO: &str = include_str!("../assets/kybernow-share-dark.svg");
const FAVICON: &str = include_str!("../assets/kybernow-favicon.svg");

/// Les icônes de la page d'accueil, écrites en ligne dans la page.
pub const ICONS: &str = include_str!("../assets/icons-symbols.html");

/// Un fichier servi.
pub struct Asset {
    pub mime: &'static str,
    pub body: &'static [u8],
}

/// Les noms servis sous `/assets/`.
pub const NAMES: [&str; 5] = ["share.css", "archivo.woff2", "martian-mono.woff2", "kybernow-share-dark.svg", "kybernow-favicon.svg"];

/// Les huit premiers chiffres hexadécimaux du SHA-256 des fichiers.
pub fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let mut input = Vec::new();
        for part in [TOKENS.as_bytes(), STYLE.as_bytes(), ARCHIVO, MARTIAN_MONO, LOGO.as_bytes(), FAVICON.as_bytes()] {
            input.extend_from_slice(&(part.len() as u64).to_le_bytes());
            input.extend_from_slice(part);
        }
        ring::digest::digest(&ring::digest::SHA256, &input).as_ref().iter().take(4).map(|b| format!("{b:02x}")).collect()
    })
}

/// L'adresse d'un fichier, avec sa version.
pub fn url(name: &str) -> String {
    format!("/assets/{name}?v={}", version())
}

/// La feuille de styles : les jetons de la charte, puis la mise en page, avec
/// la version dans les adresses des polices.
fn css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| format!("{TOKENS}\n{STYLE}").replace("__V__", version()))
}

pub fn get(name: &str) -> Option<Asset> {
    let (mime, body): (&'static str, &'static [u8]) = match name {
        "share.css" => ("text/css; charset=utf-8", css().as_bytes()),
        "archivo.woff2" => ("font/woff2", ARCHIVO),
        "martian-mono.woff2" => ("font/woff2", MARTIAN_MONO),
        "kybernow-share-dark.svg" => ("image/svg+xml", LOGO.as_bytes()),
        "kybernow-favicon.svg" => ("image/svg+xml", FAVICON.as_bytes()),
        _ => return None,
    };
    Some(Asset { mime, body })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tous_les_noms_sont_servis() {
        for name in NAMES {
            let asset = get(name).unwrap_or_else(|| panic!("{name} absent"));
            assert!(!asset.body.is_empty(), "{name} vide");
        }
        assert!(get("share.css.map").is_none());
        assert!(get("../src/web.rs").is_none());
    }

    #[test]
    fn la_version_est_dans_les_adresses_des_polices() {
        let css = String::from_utf8(get("share.css").unwrap().body.to_vec()).unwrap();
        assert!(!css.contains("__V__"));
        assert!(css.contains(&format!("/assets/archivo.woff2?v={}", version())));
        assert!(css.contains("--kn-signal"), "les jetons de la charte précèdent la mise en page");
        assert_eq!(version().len(), 8);
        assert_eq!(url("share.css"), format!("/assets/share.css?v={}", version()));
    }

    #[test]
    fn polices_et_icones_au_poids_voulu() {
        // Les polices sont réduites au latin : bien moins que celles du site (92 et 38 Ko).
        assert!(ARCHIVO.len() < 70_000, "Archivo : {} octets", ARCHIVO.len());
        assert!(MARTIAN_MONO.len() < 25_000, "Martian Mono : {} octets", MARTIAN_MONO.len());
        assert!(ARCHIVO.starts_with(b"wOF2") && MARTIAN_MONO.starts_with(b"wOF2"));
        assert!(ICONS.contains("<symbol id=\"game-controller\""));
    }
}
