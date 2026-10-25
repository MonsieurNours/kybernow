//! Interface Svelte compilée (`server/web/dist`), embarquée dans le binaire :
//! la console (`index.html`) et le client web (`link/index.html`).

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../web/dist/"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // Le client web vit dans /link/ : le moteur y charge ses fichiers par
    // des chemins relatifs à la page.
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    if path == "link" {
        return Redirect::permanent(&format!("/link/{query}")).into_response();
    }
    // Ancienne adresse du client web (0.5.0 et avant : favoris, écran final des
    // installateurs déjà publiés, guide) : renvoyée vers /link/.
    if path == "ecran" || path.starts_with("ecran/") {
        let rest = path.strip_prefix("ecran").unwrap_or("").trim_start_matches('/');
        return Redirect::permanent(&format!("/link/{rest}{query}")).into_response();
    }
    let link = path == "link/" || (path.starts_with("link/") && Assets::get(path).is_none());
    let (file, path) = match Assets::get(path) {
        Some(f) if !path.is_empty() && !path.ends_with('/') => (f, path),
        _ => {
            // Application monopage : toute autre route renvoie sa page.
            let page = if link { "link/index.html" } else { "index.html" };
            match Assets::get(page) {
                Some(f) => (f, page),
                None => {
                    return (
                        StatusCode::NOT_FOUND,
                        "Interface non compilée : lancez « npm run build » dans server/web.",
                    )
                        .into_response();
                }
            }
        }
    };
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Les fichiers de assets/ ont une empreinte dans leur nom : cache long.
    let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let mut res = (
        [(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, cache.to_string())],
        file.data,
    )
        .into_response();
    if path.starts_with("link/") {
        crate::kyber_proxy::isolate(res.headers_mut());
    }
    res
}
