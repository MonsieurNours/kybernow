//! Les écrans de l'app, dessinés d'après les maquettes webOS
//! (https://claude.ai/artifact/2TNZZrYYZpHvdQqLXzSLGv) et les écrans communs
//! des apps KyberNow Link (docs/link-ui.md) : accueil, ajout, réglages,
//! manettes, session. Chaque fonction de dessin lit l'état (`App`)
//! et inscrit les éléments choisissables dans un `Frame`.

pub mod canvas;
mod home;
pub mod icons;
mod join;
mod lend;
mod pads;
mod pair;
mod quality;
mod session;
mod settings;
mod share;
pub mod theme;
mod widgets;

use std::time::Instant;

use tiny_skia::Pixmap;

use crate::app::{App, Rect, Screen, Target};
use canvas::{Canvas, Font, Rgba, Text};
use theme::*;

/// Images embarquées.
pub struct Assets {
    pub logo: Pixmap,
}

impl Assets {
    pub fn load() -> Self {
        let logo = canvas::decode_png(include_bytes!("../../assets/logo.png")).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
        Self { logo }
    }
}

/// Les éléments choisissables d'un dessin et le focus courant.
pub struct Frame {
    pub hits: Vec<(Target, Rect)>,
    pub focus: Target,
}

impl Frame {
    /// Inscrit un élément ; rend vrai s'il a le focus.
    pub fn hit(&mut self, t: Target, r: Rect) -> bool {
        self.hits.push((t, r));
        self.focus == t
    }
}

/// Ce qui doit être dessiné pour la session : rien (l'image seule), ou
/// l'interface par-dessus.
pub fn session_overlay_visible(app: &App) -> bool {
    let Some(s) = &app.session else { return false };
    use crate::app::Phase::*;
    s.menu
        || s.keys
        || s.share_panel
        || s.lend_panel
        || app.dialog.is_some()
        || s.show_stats
        || !matches!(s.phase, Streaming)
        || s.notice.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.0)
}

/// Dessine l'écran courant ; rend les éléments choisissables.
pub fn draw(app: &App, cv: &mut Canvas, assets: &Assets) -> Vec<(Target, Rect)> {
    let mut f = Frame { hits: Vec::new(), focus: app.focus };
    match app.screen {
        Screen::Home => home::draw(app, cv, assets, &mut f),
        Screen::Add => pair::draw(app, cv, &mut f),
        Screen::Join => join::draw(app, cv, &mut f),
        // Une confirmation ouverte : seuls ses boutons se choisissent.
        Screen::Settings if app.dialog.is_some() => {
            let mut under = Frame { hits: Vec::new(), focus: Target::DialogOk };
            settings::draw(app, cv, &mut under);
            settings::dialog(app, cv, &mut f);
        }
        Screen::Settings => settings::draw(app, cv, &mut f),
        Screen::Gamepads => pads::draw(app, cv, &mut f),
        Screen::Session => session::draw(app, cv, &mut f),
    }
    // Une confirmation par-dessus l'accueil (retirer un ordinateur emprunté) ou la session (rendre l'ordinateur) :
    // seuls ses boutons se choisissent.
    if app.dialog.is_some() && matches!(app.screen, Screen::Home | Screen::Session) {
        f.hits.clear();
        settings::dialog(app, cv, &mut f);
    }
    if let Some((msg, at)) = &app.toast
        && at.elapsed().as_secs_f32() < 4.0
    {
        toast(cv, msg, at);
    }
    f.hits
}

/// Capsule passagère en bas de l'écran.
fn toast(cv: &mut Canvas, msg: &str, at: &Instant) {
    let t = at.elapsed().as_secs_f32();
    let a = if t > 3.6 { ((4.0 - t) / 0.4).clamp(0.0, 1.0) } else { 1.0 };
    let style = Text::new(Font::Regular, 24.0, FROST.alpha(a));
    let w = cv.text_width(style, msg) + 72.0;
    let x = (canvas::W - w) / 2.0;
    let y = canvas::H - 150.0;
    cv.rrect(x, y, w, 64.0, 32.0, Rgba(14, 19, 28, (235.0 * a) as u8));
    cv.rrect_stroke(x, y, w, 64.0, 32.0, 1.5, SPECTRE.alpha(0.5 * a));
    cv.text(x + 36.0, y + 41.0, style, msg);
}
