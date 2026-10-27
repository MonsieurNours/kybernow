//! « Rejoindre un partage » (KyberNow Share, docs/link-ui.md, « Partage ») : le code à six chiffres que
//! l'hôte affiche, « Votre prénom » (proposé : le nom de l'appareil, retenu pour la fois suivante) et
//! « Rejoindre ». Au bureau, au clavier ; à la manette (Steam Deck, salon, KyberNow Link OS), un pavé à
//! droite : les chiffres pour le code, les lettres pour le prénom (comme le mot de passe du Wi-Fi) ; un
//! clavier marche partout. La recherche part aux ordinateurs trouvés sur le réseau ; le texte du refus est
//! celui du cœur.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::pair::key;
use super::theme::*;
use super::widgets::{self, eyebrow, legend};
use super::{launch, title_bar, unit, Frame};
use crate::app::{App, JoinField, Layout, Rect, Target, NAME_MAX};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let u = unit(app.layout);
    let (w, h) = (cv.w, cv.h);
    let console = app.layout.console();
    cv.clear(INK_BG);
    cv.glow(w * 0.85, h * 0.15, w * 0.6, h * 0.65, Rgba::hex(0x7A5CFF).alpha(0.2));
    let tv = app.layout == Layout::Tv;
    let x0 = if tv { 96.0 } else if app.layout == Layout::Desktop { 48.0 } else { 40.0 };
    let top = if app.layout == Layout::Desktop {
        title_bar(app, cv, f)
    } else {
        let (lx, ly, lh) = launch::header_logo(app.layout);
        cv.logo_link(lx, ly, lh, 1.0);
        56.0 * u + if tv { 12.0 } else { 0.0 }
    };
    let footer = if console { 56.0 * u } else { 0.0 };
    let mut y = top + 28.0 * u;
    if app.layout == Layout::Desktop {
        widgets::ghost(cv, f, Target::Back, x0, y, 40.0 * u, 14.0 * u, Some(Icon::Back), tr("Retour", "Back"), MUTED);
        y += 56.0;
    }
    let left_w = if console { (w * 0.46).min(560.0 * u) } else { (w - 2.0 * x0).min(640.0) };
    let j = &app.join;

    eyebrow(cv, x0, y + 12.0 * u, 12.0 * u, FAINT, "KyberNow Share");
    cv.text_shrink(x0 - 1.0, y + 58.0 * u, Text::new(Font::Display, 40.0 * u, FROST).tracking(-0.8 * u), tr("Rejoindre un partage", "Join a share"), (w - 2.0 * x0).min(left_w + 80.0 * u));
    y += 80.0 * u;
    y += cv.paragraph(
        x0,
        y + 6.0 * u,
        Text::new(Font::Regular, 16.0 * u, MUTED),
        tr("Le code s'affiche chez la personne qui partage.", "The code is shown by the person sharing."),
        left_w + 60.0 * u,
        24.0 * u,
    ) + 6.0 * u
        + 22.0 * u;

    // Le code : six chiffres, groupés par trois.
    eyebrow(cv, x0, y + 8.0 * u, 11.5 * u, FAINT, tr("Code", "Code"));
    y += 22.0 * u;
    let (dw, dh) = if console { (58.0 * u, 72.0 * u) } else { (64.0 * u, 76.0 * u) };
    let gap = 10.0 * u;
    let group = 14.0 * u;
    let row = Rect::new(x0, y, 6.0 * dw + 5.0 * gap + group - gap, dh);
    let (focused, _) = f.state(Target::JoinCode, row);
    if focused {
        widgets::ring(cv, Rect::new(row.x - 4.0, row.y - 4.0, row.w + 8.0, row.h + 8.0), 12.0 * u);
    }
    let active_code = j.field == JoinField::Code;
    let blink = !app.reduce_motion && (app.screen_since.elapsed().as_millis() / 530) % 2 == 0;
    let digits: Vec<char> = j.code.chars().collect();
    let mut dx = x0;
    for i in 0..6 {
        if i == 3 {
            dx += group - gap;
        }
        let current = active_code && i == digits.len();
        cv.rrect(dx, y, dw, dh, 10.0 * u, INK_2);
        cv.rrect_stroke(dx, y, dw, dh, 10.0 * u, if current || (active_code && i == 0 && digits.is_empty()) { 1.5 } else { 1.0 }, if current { SPECTRE } else { LINE_2 });
        if let Some(c) = digits.get(i) {
            cv.text(dx + dw / 2.0, y + dh / 2.0 + 15.0 * u, Text::new(Font::MonoMedium, 40.0 * u, CORE).align(Align::Center), &c.to_string());
        } else if current && blink && !j.busy {
            cv.rect(dx + dw / 2.0 - 1.0, y + dh * 0.26, 2.0, dh * 0.48, SPECTRE);
        }
        dx += dw + gap;
    }
    y += dh + 22.0 * u;

    // Le prénom.
    eyebrow(cv, x0, y + 8.0 * u, 11.5 * u, FAINT, tr("Votre prénom", "Your name"));
    y += 22.0 * u;
    let fh = 56.0 * u;
    let field = Rect::new(x0, y, left_w, fh);
    let (focused, hover) = f.state(Target::JoinName, field);
    if focused {
        widgets::ring(cv, field, 10.0 * u);
    }
    let active_name = j.field == JoinField::Name;
    cv.rrect(field.x, field.y, field.w, field.h, 10.0 * u, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(field.x, field.y, field.w, field.h, 10.0 * u, if active_name { 1.5 } else { 1.0 }, if active_name { SPECTRE } else { LINE_2 });
    let tw = cv.text_fit(field.x + 18.0 * u, field.y + fh / 2.0 + 8.0 * u, Text::new(Font::Regular, 22.0 * u, FROST), &j.name, field.w - 60.0 * u);
    if active_name && blink && !j.busy {
        cv.rect(field.x + 20.0 * u + tw, field.y + 14.0 * u, 2.0, fh - 28.0 * u, SPECTRE);
    }
    if j.name.chars().count() >= NAME_MAX {
        cv.text(field.x + field.w - 16.0 * u, field.y + fh / 2.0 + 5.0 * u, Text::new(Font::Mono, 11.0 * u, GHOST).align(Align::Right), &format!("{NAME_MAX}"));
    }
    y += fh + 26.0 * u;

    // « Rejoindre », puis ce que la recherche a dit.
    widgets::primary_state(cv, f, Target::JoinGo, x0, y, 56.0 * u, tr("Rejoindre", "Join"), false, j.ready() && !j.busy);
    y += 56.0 * u + 24.0 * u;
    if j.busy {
        let t = j.started.elapsed().as_secs_f32();
        let (cx, cy) = (x0 + 9.0 * u, y + 2.0 * u);
        let a0 = if app.reduce_motion { 0.0 } else { t * 4.0 };
        let pts: Vec<(f32, f32)> = (0..=16)
            .map(|i| {
                let a = a0 + i as f32 * 0.28;
                (cx + 8.0 * u * a.cos(), cy + 8.0 * u * a.sin())
            })
            .collect();
        cv.polyline(&pts, 2.0 * u, SPECTRE);
        cv.text(x0 + 28.0 * u, y + 7.0 * u, Text::new(Font::Regular, 15.0 * u, MUTED), tr("Recherche du partage…", "Looking for the share…"));
    } else if let Some(m) = &j.message {
        cv.paragraph(x0, y + 6.0 * u, Text::new(Font::Regular, 15.0 * u, WARN), m, left_w + 120.0 * u, 22.0 * u);
    }

    if console {
        let px = x0 + left_w + 40.0 * u;
        let pw = w - px - x0;
        let py = top + 28.0 * u + 80.0 * u;
        match j.field {
            JoinField::Code => digit_pad(cv, f, px, py, pw, u),
            JoinField::Name => letter_pad(app, cv, f, px, py, pw, u),
        }
        cv.rect(0.0, h - footer, w, footer, Rgba(5, 7, 10, 178));
        cv.rect(0.0, h - footer, w, 1.0, LINE);
        let gy = h - footer / 2.0;
        let g = 26.0 * u;
        let mut x = x0;
        x += legend(cv, x, gy, g, &["A"], tr("Touche", "Key")) + 28.0 * u;
        legend(cv, x, gy, g, &["B"], tr("Retour", "Back"));
    }
}

/// Le pavé du code : 1 à 9, puis 0 et effacer.
fn digit_pad(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, width: f32, u: f32) {
    let (kw, kh, gap) = (84.0 * u, 66.0 * u, 12.0 * u);
    let x = x + ((width - (3.0 * kw + 2.0 * gap)) / 2.0).max(0.0);
    for (i, c) in "123456789".chars().enumerate() {
        let (col, row) = (i % 3, i / 3);
        key(cv, f, Target::Key(c), Rect::new(x + col as f32 * (kw + gap), y + row as f32 * (kh + gap), kw, kh), &c.to_string(), u);
    }
    let by = y + 3.0 * (kh + gap);
    key(cv, f, Target::Key('0'), Rect::new(x + (kw + gap), by, kw, kh), "0", u);
    key(cv, f, Target::KeyErase, Rect::new(x + 2.0 * (kw + gap), by, kw, kh), "⌫", u);
}

/// Le pavé du prénom : chiffres, lettres, quelques accents, tiret, apostrophe, espace, majuscule, effacer.
fn letter_pad(app: &App, cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, width: f32, u: f32) {
    let gap = 8.0 * u;
    let kw = ((width - 9.0 * gap) / 10.0).min(64.0 * u);
    let kh = 52.0 * u;
    let shift = app.join.shift;
    let label = |c: char| if shift { c.to_uppercase().to_string() } else { c.to_string() };
    let rows: [&str; 3] = ["1234567890", "qwertyuiop", "asdfghjkl"];
    let mut ky = y;
    for (ri, row) in rows.iter().enumerate() {
        let mut kx = x + if ri == 2 { (kw + gap) / 2.0 } else { 0.0 };
        for c in row.chars() {
            key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), u);
            kx += kw + gap;
        }
        ky += kh + gap;
    }
    // zxcvbnm, majuscule, effacer.
    let mut kx = x;
    for c in "zxcvbnm".chars() {
        key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), u);
        kx += kw + gap;
    }
    let r = Rect::new(kx, ky, kw, kh);
    let (focused, hover) = f.state(Target::KeyShift, r);
    if focused {
        widgets::ring(cv, r, 8.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 8.0 * u, if shift || hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 8.0 * u, 1.0, if shift { SPECTRE } else { LINE_2 });
    // Majuscule : une flèche vers le haut, au trait (le glyphe ⇧ manque aux polices).
    let (cx, cy, c) = (r.x + r.w / 2.0, r.y + r.h / 2.0, if shift { SPECTRE } else { FROST });
    cv.polyline(&[(cx - 9.0 * u, cy + 1.0 * u), (cx, cy - 9.0 * u), (cx + 9.0 * u, cy + 1.0 * u)], 1.8 * u, c);
    cv.line(cx, cy - 8.0 * u, cx, cy + 9.0 * u, 1.8 * u, c);
    kx += kw + gap;
    key(cv, f, Target::KeyErase, Rect::new(kx, ky, kw, kh), "⌫", u);
    ky += kh + gap;
    // Les accents, le tiret, l'apostrophe, l'espace (deux touches de large).
    let mut kx = x;
    for c in "éèàçêô-'".chars() {
        key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), u);
        kx += kw + gap;
    }
    let r = Rect::new(kx, ky, kw * 2.0 + gap, kh);
    let (focused, hover) = f.state(Target::Key(' '), r);
    if focused {
        widgets::ring(cv, r, 8.0 * u);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 8.0 * u, if hover { INK_3 } else { INK_2 });
    cv.rrect_stroke(r.x, r.y, r.w, r.h, 8.0 * u, 1.0, LINE_2);
    // L'espace : un « U » ouvert, au trait.
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    cv.polyline(&[(cx - 16.0 * u, cy - 3.0 * u), (cx - 16.0 * u, cy + 5.0 * u), (cx + 16.0 * u, cy + 5.0 * u), (cx + 16.0 * u, cy - 3.0 * u)], 1.8 * u, FROST);
}
