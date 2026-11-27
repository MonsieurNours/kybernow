//! L'image fixe du démarrage de KyberNow Link OS (`kybernow --splash`, Plymouth), à part de l'aperçu
//! (`preview.rs`), qui n'est plus compilé dans l'app livrée (LINAPP-39).

use std::path::Path;
use std::time::Instant;

use crate::app::{App, Layout, Screen};
use crate::model::Store;
use crate::ui::{self, canvas::Canvas};

/// L'image fixe du démarrage de KyberNow Link OS (Plymouth) : la première
/// image de l'allumage (l'encre, le symbole éteint), aux pixels de l'app.
pub fn splash(out: &Path, w: u32, h: u32) {
    let mut app = App::new(Store::default(), Layout::Tv);
    app.screen = Screen::Launch;
    app.launched = Instant::now();
    let mut cv = Canvas::new();
    cv.resize(w.max(320), h.max(200), 1.0);
    ui::draw(&app, &mut cv);
    match ui::canvas::encode_png(&cv.pix) {
        Some(bytes) => {
            if let Err(e) = std::fs::write(out, bytes) {
                eprintln!("{} : {e}", out.display());
                std::process::exit(1);
            }
            println!("{}", out.display());
        }
        None => {
            eprintln!("PNG non produit");
            std::process::exit(1);
        }
    }
}
