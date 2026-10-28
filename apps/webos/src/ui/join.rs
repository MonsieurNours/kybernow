//! « Rejoindre un partage » (KyberNow Share, docs/link-ui.md, « Partage ») : le code à six chiffres que
//! l'hôte affiche, « Votre prénom » (proposé : le nom de ce téléviseur, puis celui de la dernière fois),
//! « Qualité de l'image » (cinq choix, « Auto » proposé, retenue aussi) et « Rejoindre ». À la télécommande et à la manette, un pavé à droite qui suit le champ : les chiffres pour
//! le code, les lettres pour le prénom ; les touches chiffrées de la télécommande et un clavier branché
//! marchent aussi. La recherche part aux ordinateurs trouvés sur le réseau ; le texte du refus est celui
//! du cœur.

use super::canvas::{Align, Canvas, Font, Rgba, Text, H, W};
use super::pair::key;
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, JoinField, Rect, ShareQuality, Target, NAME_MAX};
use crate::i18n::tr;

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    widgets::label(cv, 96.0, 104.0, "KyberNow Share");
    cv.text(90.0, 196.0, Text::new(Font::Title, 64.0, FROST).tracking(-1.5), tr("Rejoindre un partage", "Join a share"));
    cv.paragraph(96.0, 262.0, Text::new(Font::Regular, 24.0, FROST_2), tr("Le code s'affiche chez la personne qui partage.", "The code is shown by the person sharing."), 860.0, 36.0);
    let j = &app.join;
    let blink = (app.join.started.elapsed().as_millis() / 530) % 2 == 0;

    // Le code : six chiffres, groupés par trois.
    widgets::label(cv, 96.0, 332.0, tr("Code", "Code"));
    let (dw, dh, gap, group) = (116.0, 132.0, 14.0, 30.0);
    let top = 350.0;
    let row = Rect::new(96.0, top, 6.0 * dw + 4.0 * gap + group, dh);
    let focused = f.hit(Target::JoinCode, row);
    if focused {
        cv.neon(row.x - 6.0, row.y - 6.0, row.w + 12.0, row.h + 12.0, 22.0, 0.8);
    }
    let active_code = j.field == JoinField::Code;
    let digits: Vec<char> = j.code.chars().collect();
    let mut dx = 96.0;
    for i in 0..6 {
        if i == 3 {
            dx += group - gap;
        }
        let current = active_code && i == digits.len();
        cv.rrect(dx, top, dw, dh, 18.0, PANEL_SOLID);
        cv.rrect_stroke(dx, top, dw, dh, 18.0, if current { 2.5 } else { 1.0 }, if current { SPECTRE } else { BORDER });
        if let Some(c) = digits.get(i) {
            cv.text(dx + dw / 2.0, top + dh / 2.0 + 30.0, Text::new(Font::MonoMedium, 84.0, FROST).align(Align::Center), &c.to_string());
        } else if current && blink && !j.busy {
            cv.rect(dx + dw / 2.0 - 2.0, top + 34.0, 4.0, dh - 68.0, SPECTRE);
        }
        dx += dw + gap;
    }

    // Le prénom.
    widgets::label(cv, 96.0, 530.0, tr("Votre prénom", "Your name"));
    let field = Rect::new(96.0, 552.0, 860.0, 88.0);
    let focused = f.hit(Target::JoinName, field);
    let active_name = j.field == JoinField::Name;
    if focused {
        cv.neon(field.x, field.y, field.w, field.h, 18.0, 0.8);
    }
    cv.rrect(field.x, field.y, field.w, field.h, 18.0, PANEL_SOLID);
    cv.rrect_stroke(field.x, field.y, field.w, field.h, 18.0, if active_name { 2.5 } else { 1.0 }, if active_name { SPECTRE } else { BORDER });
    let tw = cv.text_fit(field.x + 32.0, field.y + 58.0, Text::new(Font::Regular, 40.0, FROST), &j.name, field.w - 120.0);
    if active_name && blink && !j.busy {
        cv.rect(field.x + 36.0 + tw, field.y + 20.0, 4.0, 48.0, SPECTRE);
    }
    if j.name.chars().count() >= NAME_MAX {
        cv.text(field.x + field.w - 28.0, field.y + 54.0, Text::new(Font::Mono, 18.0, FROST_4).align(Align::Right), &format!("{NAME_MAX}"));
    }

    // « Qualité de l'image » : les cinq choix en pastilles, « Auto » proposé ; retenue pour la fois suivante. La croix
    // de la télécommande passe d'un choix à l'autre, OK choisit.
    widgets::label(cv, 96.0, 680.0, tr("Qualité de l'image", "Picture quality"));
    let mut qx = 96.0;
    for q in ShareQuality::ALL {
        qx += widgets::chip(cv, f, Target::JoinQuality(q), qx, 700.0, q.label(), j.quality == q) + 14.0;
    }

    // « Rejoindre », puis ce que la recherche a dit.
    widgets::primary_state(cv, f, Target::JoinGo, 96.0, 796.0, tr("Rejoindre", "Join"), j.ready() && !j.busy);
    if j.busy {
        let t = j.started.elapsed().as_secs_f32();
        let (cx, cy) = (116.0, 940.0);
        cv.circle(cx, cy, 14.0, HALO.alpha(0.2 + 0.3 * (t * 4.0).sin().abs()));
        cv.circle(cx, cy, 6.0, SPECTRE);
        if j.waiting {
            // Par le relais : la demande est chez l'hôte, qui l'accepte ou non ; « Annuler » la retire.
            let tw = cv.text(cx + 30.0, cy + 9.0, Text::new(Font::Regular, 25.0, FROST), tr("En attente de l'accord de l'hôte…", "Waiting for the host to accept…"));
            widgets::secondary(cv, f, Target::JoinCancel, cx + 30.0 + tw + 40.0, cy - 30.0, tr("Annuler", "Cancel"), 22.0);
        } else {
            cv.text(cx + 30.0, cy + 9.0, Text::new(Font::Regular, 25.0, FROST), tr("Recherche du partage…", "Looking for the share…"));
        }
    } else if let Some(m) = &j.message {
        cv.paragraph(96.0, 926.0, Text::new(Font::Regular, 25.0, DANGER), m, 860.0, 36.0);
    }

    // À droite : le pavé du champ.
    match j.field {
        JoinField::Code => digit_pad(cv, f),
        JoinField::Name => letter_pad(app, cv, f),
    }

    let y = H - 48.0;
    let mut x = 96.0;
    x += widgets::hint(cv, x, y, Key::Ok, tr("Touche", "Key")) + 40.0;
    widgets::hint(cv, x, y, Key::Back, tr("Retour", "Back"));
    cv.text(W - 96.0, y, Text::new(Font::Mono, 15.0, FROST_4).align(Align::Right), "KyberNow Share");
}

/// Le pavé du code : 1 à 9, puis 0 et effacer.
fn digit_pad(cv: &mut Canvas, f: &mut Frame) {
    let (kx, ky, kw, kh, gap) = (1160.0, 300.0, 170.0, 120.0, 18.0);
    for (i, c) in "123456789".chars().enumerate() {
        let (col, row) = (i % 3, i / 3);
        key(cv, f, Target::Key(c), Rect::new(kx + col as f32 * (kw + gap), ky + row as f32 * (kh + gap), kw, kh), &c.to_string(), false);
    }
    let by = ky + 3.0 * (kh + gap);
    key(cv, f, Target::Key('0'), Rect::new(kx + (kw + gap), by, kw, kh), "0", false);
    key(cv, f, Target::KeyErase, Rect::new(kx + 2.0 * (kw + gap), by, kw, kh), "⌫", false);
}

/// Le pavé du prénom : chiffres, lettres, quelques accents, tiret, apostrophe, espace, majuscule, effacer.
fn letter_pad(app: &App, cv: &mut Canvas, f: &mut Frame) {
    let (x, y, gap) = (1030.0, 300.0, 10.0);
    let (kw, kh) = (68.0, 92.0);
    let shift = app.join.shift;
    let label = |c: char| if shift { c.to_uppercase().to_string() } else { c.to_string() };
    let mut ky = y;
    for (ri, row) in ["1234567890", "qwertyuiop", "asdfghjkl"].iter().enumerate() {
        let mut kx = x + if ri == 2 { (kw + gap) / 2.0 } else { 0.0 };
        for c in row.chars() {
            key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), false);
            kx += kw + gap;
        }
        ky += kh + gap;
    }
    let mut kx = x;
    for c in "zxcvbnm".chars() {
        key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), false);
        kx += kw + gap;
    }
    // Majuscule : une flèche vers le haut, au trait.
    let r = Rect::new(kx, ky, kw, kh);
    let focused = f.hit(Target::KeyShift, r);
    let (bg, fg) = if focused {
        cv.neon(r.x, r.y, r.w, r.h, 14.0, 0.9);
        (CORE, INK)
    } else if shift {
        (SPECTRE.alpha(0.25), CORE)
    } else {
        (Rgba(14, 19, 28, 230), FROST)
    };
    cv.rrect(r.x, r.y, r.w, r.h, 14.0, bg);
    if !focused {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, if shift { 2.0 } else { 1.0 }, if shift { SPECTRE } else { BORDER });
    }
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    cv.line(cx - 14.0, cy + 2.0, cx, cy - 14.0, 3.0, fg);
    cv.line(cx + 14.0, cy + 2.0, cx, cy - 14.0, 3.0, fg);
    cv.line(cx, cy - 12.0, cx, cy + 14.0, 3.0, fg);
    kx += kw + gap;
    key(cv, f, Target::KeyErase, Rect::new(kx, ky, kw, kh), "⌫", false);
    ky += kh + gap;
    // Les accents, le tiret, l'apostrophe, l'espace (deux touches de large).
    let mut kx = x;
    for c in "éèàçêô-'".chars() {
        key(cv, f, Target::Key(c), Rect::new(kx, ky, kw, kh), &label(c), false);
        kx += kw + gap;
    }
    let r = Rect::new(kx, ky, kw * 2.0 + gap, kh);
    let focused = f.hit(Target::Key(' '), r);
    let (bg, fg) = if focused {
        cv.neon(r.x, r.y, r.w, r.h, 14.0, 0.9);
        (CORE, INK)
    } else {
        (Rgba(14, 19, 28, 230), FROST)
    };
    cv.rrect(r.x, r.y, r.w, r.h, 14.0, bg);
    if !focused {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 14.0, 1.0, BORDER);
    }
    // L'espace : un « U » ouvert, au trait.
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    cv.line(cx - 26.0, cy - 6.0, cx - 26.0, cy + 10.0, 3.0, fg);
    cv.line(cx - 26.0, cy + 10.0, cx + 26.0, cy + 10.0, 3.0, fg);
    cv.line(cx + 26.0, cy + 10.0, cx + 26.0, cy - 6.0, 3.0, fg);
}
