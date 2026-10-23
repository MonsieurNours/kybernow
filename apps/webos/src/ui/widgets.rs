//! Éléments communs : bouton principal, pastilles de profil, tuiles du menu,
//! lignes de réglage, interrupteurs, aides en bas d'écran. Le focus se voit
//! comme sur les maquettes : cœur du néon (#F4F1FF) et halo Spectre.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::Frame;
use crate::app::{Rect, Target};

pub const MONO_LABEL: Text = Text::new(Font::Mono, 16.0, FROST_3).tracking(3.0);

/// Petite étiquette en capitales Martian Mono (« PRÊT À JOUER »).
pub fn label(cv: &mut Canvas, x: f32, y: f32, s: &str) {
    cv.text(x, y, MONO_LABEL, &s.to_uppercase());
}

/// Bouton principal (« Se connecter ») ; rend sa largeur.
pub fn primary(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, s: &str) -> f32 {
    let style = Text::new(Font::Semibold, 30.0, INK);
    let w = cv.text_width(style, s) + 104.0;
    let h = 84.0;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    if focused {
        cv.neon(x, y, w, h, 14.0, 1.0);
        cv.rrect(x, y, w, h, 14.0, CORE);
        cv.text(x + 52.0, y + 53.0, style, s);
    } else {
        cv.rrect(x, y, w, h, 14.0, FLAT);
        cv.text(x + 52.0, y + 53.0, style.color(CORE), s);
    }
    w
}

/// Bouton secondaire (contour) ; rend sa largeur.
pub fn secondary(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, s: &str, size: f32) -> f32 {
    let style = Text::new(Font::Semibold, size, FROST);
    let w = cv.text_width(style, s) + size * 2.0;
    let h = size * 2.4;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    if focused {
        cv.neon(x, y, w, h, 12.0, 0.9);
        cv.rrect(x, y, w, h, 12.0, CORE);
        cv.text(x + size, y + h / 2.0 + size * 0.36, style.color(INK), s);
    } else {
        cv.rrect(x, y, w, h, 12.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(x, y, w, h, 12.0, 1.5, BORDER);
        cv.text(x + size, y + h / 2.0 + size * 0.36, style, s);
    }
    w
}

/// Pastille de profil (Jeu, Bureau, Max) ; rend sa largeur.
pub fn chip(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, s: &str, active: bool) -> f32 {
    let style = Text::new(if active { Font::Semibold } else { Font::Regular }, 22.0, if active { FROST } else { FROST_2 });
    let w = cv.text_width(style, s) + 56.0;
    let h = 56.0;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    if focused {
        cv.neon(x, y, w, h, 28.0, 0.8);
        cv.rrect(x, y, w, h, 28.0, CORE);
        cv.text(x + 28.0, y + 36.0, style.color(INK), s);
    } else if active {
        cv.rrect(x, y, w, h, 28.0, SPECTRE.alpha(0.18));
        cv.rrect_stroke(x, y, w, h, 28.0, 1.5, SPECTRE);
        cv.text(x + 28.0, y + 36.0, style, s);
    } else {
        cv.rrect(x, y, w, h, 28.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
        cv.text(x + 28.0, y + 36.0, style, s);
    }
    w
}

/// Largeur d'une pastille à icône (pour l'aligner à droite).
pub fn icon_chip_width(cv: &mut Canvas, s: &str) -> f32 {
    cv.text_width(Text::new(Font::Regular, 22.0, FROST), s) + 28.0 + 30.0 + 12.0 + 28.0
}

/// Pastille à icône (Réglages…), atteignable à la croix ; rend sa largeur.
#[allow(clippy::too_many_arguments)]
pub fn icon_chip(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, icon: Icon, s: &str) -> f32 {
    let style = Text::new(Font::Regular, 22.0, FROST);
    let w = icon_chip_width(cv, s);
    let h = 56.0;
    let focused = f.hit(t, Rect::new(x, y, w, h));
    let fg = if focused { INK } else { FROST };
    if focused {
        cv.neon(x, y, w, h, 28.0, 0.8);
        cv.rrect(x, y, w, h, 28.0, CORE);
    } else {
        cv.rrect(x, y, w, h, 28.0, Rgba(227, 237, 240, 15));
        cv.rrect_stroke(x, y, w, h, 28.0, 1.0, BORDER);
    }
    cv.icon(&icon, x + 26.0, y + 13.0, 30.0, 1.6, fg);
    cv.text(x + 26.0 + 30.0 + 12.0, y + 36.0, style.color(fg), s);
    w
}

/// Tuile du menu de session.
#[allow(clippy::too_many_arguments)]
pub fn tile(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, icon: Icon, title: &str, sub: Option<&str>, danger: bool, on: bool) {
    let focused = f.hit(t, r);
    let (bg, fg) = if focused {
        cv.neon(r.x, r.y, r.w, r.h, 22.0, 1.0);
        (CORE, INK)
    } else if danger {
        (DANGER_BG, DANGER)
    } else {
        (Rgba(14, 19, 28, 225), FROST)
    };
    cv.rrect(r.x, r.y, r.w, r.h, 22.0, bg);
    if !focused {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 22.0, 1.0, if danger { DANGER_LINE } else if on { SPECTRE } else { BORDER });
    }
    cv.icon(&icon, r.x + 28.0, r.y + 26.0, 40.0, 1.8, fg);
    if on && !focused {
        cv.circle(r.x + r.w - 32.0, r.y + 34.0, 7.0, SPECTRE);
    }
    let title_style = Text::new(Font::Semibold, 26.0, fg);
    match sub {
        Some(sub) => {
            cv.text_fit(r.x + 28.0, r.y + r.h - 54.0, title_style, title, r.w - 56.0);
            let sub_color = if focused { Rgba(5, 7, 10, 170) } else if danger { DANGER } else { FROST_2 };
            cv.text_fit(r.x + 28.0, r.y + r.h - 26.0, Text::new(Font::Mono, 14.0, sub_color), sub, r.w - 56.0);
        }
        None => {
            cv.text_fit(r.x + 28.0, r.y + r.h - 30.0, title_style, title, r.w - 56.0);
        }
    }
}

/// La valeur d'une ligne de réglage.
pub enum Value<'a> {
    Text(String),
    Toggle(bool),
    Choices(&'a [String], usize),
    Unavailable(&'a str),
    Danger(&'a str),
}

/// Ligne de réglage (libellé, aide, valeur à droite). Hauteur 92.
#[allow(clippy::too_many_arguments)]
pub fn row(cv: &mut Canvas, f: &mut Frame, t: Option<Target>, x: f32, y: f32, w: f32, title: &str, help: &str, value: Value) -> f32 {
    let h = 92.0;
    let focused = t.is_some_and(|t| f.hit(t, Rect::new(x, y, w, h)));
    let dashed = matches!(value, Value::Unavailable(_));
    if focused {
        cv.neon(x, y, w, h, 16.0, 0.8);
        cv.rrect(x, y, w, h, 16.0, Rgba(30, 28, 56, 255));
        cv.rrect_stroke(x, y, w, h, 16.0, 2.0, SPECTRE);
    } else if dashed {
        cv.rrect_dashed(x, y, w, h, 16.0, 1.5, BORDER);
    } else {
        cv.rrect(x, y, w, h, 16.0, PANEL);
        cv.rrect_stroke(x, y, w, h, 16.0, 1.0, LINE);
    }
    let title_color = if dashed { FROST_2 } else { FROST };
    cv.text(x + 28.0, y + 40.0, Text::new(Font::Regular, 25.0, title_color), title);
    let right = x + w - 28.0;
    let value_w = match &value {
        Value::Text(s) => cv.text(right, y + 54.0, Text::new(Font::Mono, 20.0, FROST).align(Align::Right), s),
        Value::Unavailable(s) => cv.text(right, y + 54.0, Text::new(Font::Mono, 17.0, FROST_3).align(Align::Right), s),
        Value::Danger(s) => cv.text(right, y + 54.0, Text::new(Font::Semibold, 21.0, DANGER).align(Align::Right), s),
        Value::Toggle(on) => {
            toggle(cv, right - 72.0, y + 26.0, *on);
            72.0
        }
        Value::Choices(opts, sel) => {
            let mut xx = right;
            for (i, o) in opts.iter().enumerate().rev() {
                let style = Text::new(Font::Mono, 18.0, if i == *sel { FROST } else { FROST_3 });
                let tw = cv.text_width(style, o) + 30.0;
                xx -= tw;
                if i == *sel {
                    cv.rrect(xx, y + 24.0, tw, 44.0, 10.0, SPECTRE.alpha(0.2));
                    cv.rrect_stroke(xx, y + 24.0, tw, 44.0, 10.0, 1.0, SPECTRE);
                }
                cv.text(xx + 15.0, y + 53.0, style, o);
                xx -= 8.0;
            }
            right - xx
        }
    };
    cv.text_fit(x + 28.0, y + 70.0, Text::new(Font::Regular, 17.0, FROST_3), help, w - 90.0 - value_w);
    if focused && matches!(value, Value::Choices(..)) {
        // Flèches : gauche et droite changent la valeur.
        cv.text(x + w - 8.0, y + 54.0, Text::new(Font::Mono, 16.0, SPECTRE).align(Align::Right), "›");
    }
    h
}

pub fn toggle(cv: &mut Canvas, x: f32, y: f32, on: bool) {
    if on {
        cv.rrect(x, y, 72.0, 40.0, 20.0, FLAT);
        cv.circle(x + 52.0, y + 20.0, 16.0, CORE);
    } else {
        cv.rrect(x, y, 72.0, 40.0, 20.0, Rgba(227, 237, 240, 40));
        cv.circle(x + 20.0, y + 20.0, 16.0, FROST_2);
    }
}

/// Aide en bas d'écran : une touche et ce qu'elle fait.
pub enum Key {
    Ok,
    Back,
    Dots,
    Pointer,
}

pub fn hint(cv: &mut Canvas, x: f32, y: f32, key: Key, s: &str) -> f32 {
    let mut xx = x;
    match key {
        Key::Ok => {
            cv.rrect_stroke(xx, y - 24.0, 44.0, 30.0, 15.0, 1.5, FROST_3);
            cv.text(xx + 22.0, y - 4.0, Text::new(Font::MonoMedium, 13.0, FROST_3).align(Align::Center), "OK");
            xx += 56.0;
        }
        Key::Back => {
            cv.icon(&Icon::Back, xx, y - 26.0, 30.0, 1.6, FROST_3);
            xx += 42.0;
        }
        Key::Dots => {
            cv.icon(&Icon::Dots, xx, y - 26.0, 30.0, 1.6, FROST_3);
            xx += 42.0;
        }
        Key::Pointer => {
            cv.icon(&Icon::Pointer, xx, y - 26.0, 30.0, 1.6, FROST_3);
            xx += 42.0;
        }
    }
    let w = cv.text(xx, y, Text::new(Font::Regular, 19.0, FROST_3), s);
    xx + w - x
}
