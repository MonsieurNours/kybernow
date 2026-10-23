//! Les écrans de l'app, dessinés d'après les maquettes
//! (https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc) : lancement, accueil,
//! ajout, réglages, manettes, session, chacun pour le bureau, le Steam Deck
//! et le salon. Chaque fonction de dessin lit l'état (`App`) et inscrit les
//! éléments choisissables dans un `Frame`.

pub mod brand;
pub mod canvas;
mod home;
pub mod icons;
mod launch;
mod pads;
mod pair;
mod session;
mod settings;
pub mod theme;
mod widgets;

use std::time::Instant;

pub use home::profile_label;
#[allow(unused_imports)]
pub use launch::{TOTAL as LAUNCH, TOTAL_REDUCED as LAUNCH_REDUCED};

use crate::app::{App, Layout, Phase, Rect, Screen, Target};
use canvas::{Align, Canvas, Font, Rgba, Text};
use theme::*;

/// Les éléments choisissables d'un dessin, le focus et le pointeur.
pub struct Frame {
    pub hits: Vec<(Target, Rect)>,
    pub focus: Target,
    /// L'anneau du focus se voit (manette, clavier).
    pub show_focus: bool,
    pub pointer: Option<(f32, f32)>,
}

impl Frame {
    /// Inscrit un élément ; rend (focus visible, survolé).
    pub fn state(&mut self, t: Target, r: Rect) -> (bool, bool) {
        self.hits.push((t, r));
        let focused = self.focus == t && self.show_focus;
        let hover = self.pointer.is_some_and(|(x, y)| r.contains(x, y));
        (focused, hover)
    }

}

/// Dessine l'écran courant ; rend les éléments choisissables.
pub fn draw(app: &App, cv: &mut Canvas) -> Vec<(Target, Rect)> {
    let mut f = Frame { hits: Vec::new(), focus: app.focus, show_focus: app.keyboard_focus || app.layout.console(), pointer: app.pointer };
    match app.screen {
        Screen::Launch => launch::draw(app, cv),
        Screen::Home => home::draw(app, cv, &mut f),
        Screen::Add => pair::draw(app, cv, &mut f),
        Screen::Settings => settings::draw(app, cv, &mut f),
        Screen::Gamepads => pads::draw(app, cv, &mut f),
        Screen::Session => session::draw(app, cv, &mut f),
    }
    if let Some((msg, at)) = &app.toast
        && at.elapsed().as_secs_f32() < 4.0
    {
        toast(app, cv, msg, at);
    }
    f.hits
}

/// L'interface se voit-elle pendant la session (menu, barre, capsule,
/// passage) ; sinon l'image seule.
pub fn session_overlay_visible(app: &App) -> bool {
    let Some(s) = &app.session else { return false };
    if !matches!(s.phase, Phase::Streaming) || passage_opening(s) {
        return true;
    }
    s.menu || s.keys || s.show_stats || s.bar || s.notice.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2) || app.toast.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs_f32() < 4.2)
}

/// Les moitiés de l'accueil s'écartent encore (1,1 s après la première image).
pub fn passage_opening(s: &crate::app::SessionUi) -> bool {
    s.phase == Phase::Streaming && s.opened_at.is_some_and(|t| t.elapsed().as_secs_f32() < session::OPEN)
}

/// L'échelle des éléments d'une interface par rapport aux maquettes du Deck
/// et du bureau (1280 × 800) : 1 ; salon : 1,5.
pub fn unit(layout: Layout) -> f32 {
    if layout == Layout::Tv { 1.5 } else { 1.0 }
}

/// Capsule passagère en bas de l'écran.
pub fn toast(app: &App, cv: &mut Canvas, msg: &str, at: &Instant) {
    let u = unit(app.layout);
    let t = at.elapsed().as_secs_f32();
    let a = if t > 3.6 { ((4.0 - t) / 0.4).clamp(0.0, 1.0) } else { 1.0 };
    let style = Text::new(Font::Regular, 14.0 * u, FROST.alpha(a));
    let h = 38.0 * u;
    let w = cv.text_width(style, msg) + 52.0 * u;
    let x = (cv.w - w) / 2.0;
    let y = cv.h - if app.layout.console() { 120.0 } else { 90.0 } * u;
    cv.rrect(x, y, w, h, h / 2.0, Rgba(9, 13, 18, (230.0 * a) as u8));
    cv.rrect_stroke(x, y, w, h, h / 2.0, 1.0, LINE_2.alpha(a));
    cv.circle(x + 20.0 * u, y + h / 2.0, 3.5 * u, SPECTRE.alpha(a));
    cv.text(x + 32.0 * u, y + h / 2.0 + 5.0 * u, style, msg);
}

/// Le fond de l'accueil sans image : la signature de l'ordinateur (deux
/// lueurs dont la teinte vient de l'empreinte de son Admin) et le symbole en
/// filigrane, comme sur Windows.
pub fn signature(cv: &mut Canvas, fingerprint: Option<&str>, offline: bool) {
    let (w, h) = (cv.w, cv.h);
    cv.rect(0.0, 0.0, w, h, INK_BG);
    let hue = hue(fingerprint);
    let a = if offline { 0.18 } else { 0.34 };
    let c1 = hsl(hue, 0.9, 0.62);
    let c2 = hsl((hue + 7.0) % 360.0, 0.7, 0.55);
    cv.glow(w * 0.78, h * 0.22, w * 0.7, h * 0.75, c1.alpha(a));
    cv.glow(w * 0.18, h * 0.78, w * 0.55, h * 0.65, c2.alpha(a * 0.42));
    let s = h * 0.95;
    cv.mark(w - s * 0.76, h - s * 0.7, s, Rgba::hex(0x141B23).alpha(0.55), Rgba::hex(0x141B23).alpha(0.55), None);
}

/// Le fondu d'encre qui tient le texte de l'accueil lisible sur l'image,
/// celui de l'app Mac (HomeView) : à gauche sous le texte, en bas sous les
/// ordinateurs, un voile en haut.
pub fn shade(cv: &mut Canvas) {
    let (w, h) = (cv.w, cv.h);
    let ink = |a: f32| INK.alpha(a);
    cv.gradient(0.0, 0.0, w, h, true, &[(0.0, ink(0.88)), (0.38, ink(0.55)), (0.66, ink(0.0))]);
    cv.gradient(0.0, 0.0, w, h, false, &[(0.48, ink(0.0)), (0.76, ink(0.82)), (1.0, ink(1.0))]);
    cv.gradient(0.0, 0.0, w, h * 0.14, false, &[(0.0, ink(0.55)), (1.0, ink(0.0))]);
}

/// Teinte tirée de l'empreinte d'Admin (comme `hue()` de l'app Windows).
pub fn hue(fp: Option<&str>) -> f32 {
    let Some(fp) = fp else { return 258.0 };
    let hex: String = fp.chars().filter(|c| c.is_ascii_hexdigit()).take(4).collect();
    let v = u32::from_str_radix(&hex, 16).unwrap_or(0);
    (v % 360) as f32
}

pub fn hsl(h: f32, s: f32, l: f32) -> Rgba {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Rgba(((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8, 255)
}

/// La barre de titre du bureau (décorations côté client, Wayland et X11) :
/// le logo signé, la version, réglages, réduire, agrandir, fermer.
pub fn title_bar(app: &App, cv: &mut Canvas, f: &mut Frame) -> f32 {
    let h = 40.0;
    if app.fullscreen {
        return 0.0;
    }
    cv.rect(0.0, 0.0, cv.w, h, Rgba(5, 7, 10, 153));
    cv.rect(0.0, h - 1.0, cv.w, 1.0, LINE);
    let lw = cv.logo_link(16.0, 9.0, 22.0, 1.0);
    let mut x = cv.w - 10.0;
    for (t, icon) in [(Target::WinClose, icons::Icon::Close), (Target::WinMaximize, icons::Icon::Square), (Target::WinMinimize, icons::Icon::Minus)] {
        x -= 34.0;
        let r = Rect::new(x, 3.0, 34.0, 34.0);
        let (focused, hover) = f.state(t, r);
        if hover || focused {
            cv.rrect(r.x, r.y, r.w, r.h, 6.0, if t == Target::WinClose { BAD.alpha(0.8) } else { Rgba(227, 237, 240, 22) });
        }
        cv.icon(&icon, x + 9.0, 12.0, 16.0, 1.4, MUTED);
    }
    if app.screen == Screen::Settings {
        // Fil d'Ariane (maquette) : logo › ordinateur › Réglages ; l'ordinateur ramène en arrière.
        let mut cx = 16.0 + lw + 14.0;
        let sep = Text::new(Font::Regular, 14.0, GHOST);
        let crumb = Text::new(Font::Regular, 14.0, MUTED);
        if !app.settings_global
            && let Some(h) = app.selected()
        {
            cx += cv.text(cx, 25.0, sep, "›") + 14.0;
            let w = cv.text_width(crumb, &h.name).min(360.0);
            let r = Rect::new(cx - 6.0, 8.0, w + 12.0, 24.0);
            let (focused, hover) = f.state(Target::Back, r);
            if focused {
                cv.neon(r.x, r.y, r.w, r.h, 6.0, 1.0);
            }
            cx += cv.text_fit(cx, 25.0, crumb.color(if hover { FROST } else { MUTED }), &h.name, 360.0) + 14.0;
        }
        cx += cv.text(cx, 25.0, sep, "›") + 14.0;
        cv.text(cx, 25.0, Text::new(Font::Regular, 14.0, FROST), if app.settings_global { crate::i18n::tr("Réglages de l'app", "App settings") } else { crate::i18n::tr("Réglages", "Settings") });
        return h;
    }
    x -= 36.0;
    let r = Rect::new(x, 3.0, 34.0, 34.0);
    let (focused, hover) = f.state(Target::AppSettings, r);
    if hover || focused {
        cv.rrect(r.x, r.y, r.w, r.h, 6.0, Rgba(227, 237, 240, 22));
    }
    cv.icon(&icons::Icon::Gear, x + 8.0, 11.0, 18.0, 1.6, MUTED);
    // Version, ou la mise à jour publiée.
    match &app.update {
        crate::app::UpdateState::Available { version, .. } => {
            let s = format!("{} {version} {}", crate::i18n::tr("Version", "Version"), crate::i18n::tr("disponible", "available"));
            let st = Text::new(Font::Mono, 11.0, SPECTRE).align(Align::Right);
            let w = cv.text_width(st, &s);
            let r = Rect::new(x - 16.0 - w, 8.0, w + 8.0, 24.0);
            f.state(Target::UpdateNotice, r);
            cv.text(x - 12.0, 24.0, st, &s);
        }
        _ => {
            // Sur le Pi, l'architecture suit la version (« 0.6.0 · aarch64 »), comme sur la maquette.
            let v = if app.machine.pi.is_some() { format!("{} · {}", app.version, std::env::consts::ARCH) } else { app.version.to_string() };
            cv.text(x - 12.0, 24.0, Text::new(Font::Mono, 11.0, FAINT).align(Align::Right), &v);
        }
    }
    h
}

/// La zone de la barre de titre qu'on peut saisir pour déplacer la fenêtre.
pub fn title_drag_area(app: &App, w: f32) -> Option<Rect> {
    (app.layout == Layout::Desktop && !app.fullscreen).then(|| Rect::new(0.0, 0.0, w - 200.0, 40.0))
}
