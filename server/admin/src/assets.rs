//! Interface Svelte compilée (`server/web/dist`), embarquée dans le binaire :
//! la console (`index.html`) et le client web (`ecran/index.html`).

use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Redirect, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../web/dist/"]
#[allow_missing = true]
struct Assets;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    // Le client web vit dans /ecran/ : le moteur y charge ses fichiers par
    // des chemins relatifs à la page.
    if path == "ecran" {
        let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
        return Redirect::permanent(&format!("/ecran/{query}")).into_response();
    }
    let ecran = path == "ecran/" || (path.starts_with("ecran/") && Assets::get(path).is_none());
    let (file, path) = match Assets::get(path) {
        Some(f) if !path.is_empty() && !path.ends_with('/') => (f, path),
        _ => {
            // Application monopage : toute autre route renvoie sa page.
            let page = if ecran { "ecran/index.html" } else { "index.html" };
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
    if path.starts_with("ecran/") {
        crate::kyber_proxy::isolate(res.headers_mut());
    }
    res
}
