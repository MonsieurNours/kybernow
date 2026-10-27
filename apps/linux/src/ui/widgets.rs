//! Éléments communs, d'après les maquettes : bouton principal au coin coupé à
//! 45°, segments, pastilles, interrupteurs, cartes des ordinateurs, légende
//! des boutons de la manette. Le focus se voit comme sur les maquettes :
//! anneau au cœur du néon (#F4F1FF) et halo Spectre.

use super::canvas::{Align, Canvas, Font, Rgba, Text};
use super::icons::Icon;
use super::theme::*;
use super::Frame;
use crate::app::{Rect, Tag, TagKind, Target};

/// Étiquette en capitales Martian Mono (« DERNIÈRE SESSION »).
pub fn eyebrow(cv: &mut Canvas, x: f32, y: f32, size: f32, color: Rgba, s: &str) -> f32 {
    cv.text(x, y, Text::new(Font::Mono, size, color).tracking(size * 0.08), &s.to_uppercase())
}

/// L'anneau du focus autour d'un rectangle.
pub fn ring(cv: &mut Canvas, r: Rect, radius: f32) {
    cv.neon(r.x, r.y, r.w, r.h, radius, 1.0);
}

/// Bouton principal (« Se connecter ») : aplat Spectre, coin bas droit coupé
/// à 45° ; rend sa largeur. `h` : hauteur (52 bureau, 60 Deck, 76 salon).
pub fn primary(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, s: &str, play: bool) -> f32 {
    primary_state(cv, f, t, x, y, h, s, play, true)
}

/// Le bouton principal, éteint quand il ne peut pas partir (« Rejoindre » sans code) : il reste
/// choisissable, et dit ce qui manque en y menant le focus.
#[allow(clippy::too_many_arguments)]
pub fn primary_state(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, s: &str, play: bool, enabled: bool) -> f32 {
    let size = h * 0.32;
    let ink = if enabled { Rgba(255, 255, 255, 255) } else { FAINT };
    let style = Text::new(Font::Semibold, size, ink);
    let icon = if play { size * 1.05 } else { 0.0 };
    let gap = if play { size * 0.7 } else { 0.0 };
    let pad = h * 0.58;
    let w = cv.text_width(style, s) + 2.0 * pad + icon + gap;
    let r = Rect::new(x, y, w, h);
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 8.0);
    }
    let cut = h * 0.23;
    let c = if !enabled { INK_3 } else if hover { Rgba::hex(0x7E6EFF) } else { FLAT };
    cv.polygon(&[(x, y), (x + w, y), (x + w, y + h - cut), (x + w - cut, y + h), (x, y + h)], c);
    let mut tx = x + pad;
    if play {
        let cy = y + h / 2.0;
        cv.polygon(&[(tx + icon * 0.2, cy - icon * 0.42), (tx + icon * 0.2, cy + icon * 0.42), (tx + icon * 0.9, cy)], ink);
        tx += icon + gap;
    }
    cv.text(tx, y + h / 2.0 + size * 0.36, style, s);
    w
}

/// Segments (profils, réglages du bureau) : `items` = (cible, libellé, choisi, indisponible).
/// Rend la largeur.
pub fn segments(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, h: f32, size: f32, items: &[(Target, String, bool, bool)]) -> f32 {
    let pad = h * 0.08;
    let style = Text::new(Font::Regular, size, MUTED);
    let widths: Vec<f32> = items.iter().map(|(_, s, ..)| cv.text_width(style, s) + size * 2.3).collect();
    let total = widths.iter().sum::<f32>() + pad * 2.0 + (items.len().saturating_sub(1)) as f32 * 3.0;
    cv.rrect(x, y, total, h, h * 0.2, Rgba(9, 13, 18, 190));
    cv.rrect_stroke(x, y, total, h, h * 0.2, 1.0, LINE_2);
    let mut xx = x + pad;
    for (i, (t, s, on, off)) in items.iter().enumerate() {
        let w = widths[i];
        let r = Rect::new(xx, y + pad, w, h - 2.0 * pad);
        let (focused, hover) = if *off { (false, false) } else { f.state(*t, r) };
        if *on {
            cv.rrect(r.x, r.y, r.w, r.h, h * 0.15, INK_3);
            cv.rrect_stroke(r.x, r.y, r.w, r.h, h * 0.15, 1.0, LINE_2);
        } else if hover {
            cv.rrect(r.x, r.y, r.w, r.h, h * 0.15, Rgba(227, 237, 240, 14));
        }
        if focused {
            ring(cv, r, h * 0.15);
        }
        let color = if *off { DIM } else if *on { FROST } else { MUTED };
        let font = if *on { Font::Semibold } else { Font::Regular };
        cv.text(r.x + w / 2.0, r.y + r.h / 2.0 + size * 0.36, Text::new(font, size, color).align(Align::Center), s);
        xx += w + 3.0;
    }
    total
}

/// Une étiquette (« décodeur du Pi », « processeur », « 1080p ») : 22 pt de
/// haut, Martian Mono 11, centrée sur `cy` ; rend sa largeur.
pub fn tag(cv: &mut Canvas, x: f32, cy: f32, t: &Tag) -> f32 {
    let (bg, fg) = match t.kind {
        TagKind::Hw => (Rgba(154, 140, 255, 41), Rgba::hex(0xC9C0FF)),
        TagKind::Cpu => (Rgba(245, 184, 65, 41), WARN),
        TagKind::Note => (INK_3, FAINT),
    };
    let style = Text::new(Font::Mono, 11.0, fg).tracking(0.44);
    let w = cv.text_width(style, &t.text) + 16.0;
    cv.rrect(x, cy - 11.0, w, 22.0, 6.0, bg);
    cv.text(x + 8.0, cy + 4.0, style, &t.text);
    w
}

pub fn tag_width(cv: &mut Canvas, t: &Tag) -> f32 {
    cv.text_width(Text::new(Font::Mono, 11.0, FAINT).tracking(0.44), &t.text) + 16.0
}

/// Segments des réglages du bureau, d'après la maquette : boutons de 36 pt
/// (Archivo 14), étiquette dans le bouton, valeur indisponible barrée ; les
/// boutons passent à la ligne au-delà de `max`. `items` = (cible, libellé,
/// choisi, indisponible, étiquette). Rend (largeur, hauteur).
pub fn segments_tagged(cv: &mut Canvas, f: &mut Frame, x: f32, y: f32, max: f32, items: &[(Target, String, bool, bool, Option<Tag>)]) -> (f32, f32) {
    let (pad, gap, bh, border) = (3.0, 2.0, 36.0, 1.0);
    let style = Text::new(Font::Regular, 14.0, MUTED);
    let widths: Vec<f32> = items.iter().map(|(_, s, _, _, t)| cv.text_width(style, s) + 28.0 + t.as_ref().map(|t| 8.0 + tag_width(cv, t)).unwrap_or(0.0)).collect();
    let inner_max = (max - 2.0 * (pad + border)).max(1.0);
    let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
    let (mut rw, mut widest) = (0.0f32, 0.0f32);
    for (i, w) in widths.iter().enumerate() {
        let first = rows.last().unwrap().is_empty();
        let add = if first { *w } else { *w + gap };
        if !first && rw + add > inner_max {
            widest = widest.max(rw);
            rows.push(Vec::new());
            rw = *w;
        } else {
            rw += add;
        }
        rows.last_mut().unwrap().push(i);
    }
    widest = widest.max(rw);
    let total_w = widest + 2.0 * (pad + border);
    let total_h = rows.len() as f32 * bh + (rows.len() - 1) as f32 * gap + 2.0 * (pad + border);
    cv.rrect(x, y, total_w, total_h, 8.0, Rgba(9, 13, 18, 179));
    cv.rrect_stroke(x, y, total_w, total_h, 8.0, 1.0, LINE_2);
    let mut yy = y + border + pad;
    for row in rows {
        let mut xx = x + border + pad;
        for i in row {
            let (t, s, on, off, tg) = &items[i];
            let r = Rect::new(xx, yy, widths[i], bh);
            let (focused, hover) = if *off { (false, false) } else { f.state(*t, r) };
            if *on {
                cv.rrect(r.x, r.y, r.w, r.h, 6.0, INK_3);
                cv.rrect_stroke(r.x, r.y, r.w, r.h, 6.0, 1.0, LINE_2);
            } else if hover {
                cv.rrect(r.x, r.y, r.w, r.h, 6.0, Rgba(227, 237, 240, 14));
            }
            if focused {
                ring(cv, r, 6.0);
            }
            let color = if *off { GHOST } else if *on { FROST } else { MUTED };
            let cy = yy + bh / 2.0;
            let tw = cv.text(xx + 14.0, cy + 14.0 * 0.36, Text::new(Font::Regular, 14.0, color), s);
            if *off {
                cv.rect(xx + 14.0, cy - 1.0, tw, 1.0, GHOST);
            }
            if let Some(tg) = tg {
                tag(cv, xx + 14.0 + tw + 8.0, cy, tg);
            }
            xx += widths[i] + gap;
        }
        yy += bh + gap;
    }
    (total_w, total_h)
}

/// Un bouton de la colonne des réglages (bureau) : pleine largeur, 40 pt,
/// Archivo 15 ; choisi sur un relief d'encre. `value` : à droite, en Martian Mono.
pub fn nav_button(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, w: f32, s: &str, on: bool) {
    let r = Rect::new(x, y, w, 40.0);
    let (focused, hover) = f.state(t, r);
    if on {
        cv.rrect(r.x, r.y, r.w, r.h, 8.0, INK_3);
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 8.0, 1.0, LINE_2);
    } else if hover {
        cv.rrect(r.x, r.y, r.w, r.h, 8.0, Rgba(227, 237, 240, 12));
    }
    if focused {
        ring(cv, r, 8.0);
    }
    cv.text_fit(x + 14.0, y + 20.0 + 15.0 * 0.36, Text::new(Font::Regular, 15.0, if on { FROST } else { MUTED }), s, w - 28.0);
}

/// Bouton à contour ; rend sa largeur.
pub fn ghost(cv: &mut Canvas, f: &mut Frame, t: Target, x: f32, y: f32, h: f32, size: f32, icon: Option<Icon>, s: &str, color: Rgba) -> f32 {
    let style = Text::new(Font::Regular, size, color);
    let iw = if icon.is_some() { size * 1.15 + size * 0.6 } else { 0.0 };
    let w = cv.text_width(style, s) + size * 2.0 + iw;
    let r = Rect::new(x, y, w, h);
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 8.0);
    }
    cv.rrect(x, y, w, h, 8.0, if hover { Rgba(227, 237, 240, 16) } else { Rgba(9, 13, 18, 120) });
    cv.rrect_stroke(x, y, w, h, 8.0, 1.0, if hover { FAINT } else { LINE_2 });
    let mut tx = x + size;
    if let Some(i) = icon {
        cv.icon(&i, tx, y + (h - size * 1.15) / 2.0, size * 1.15, 1.8, color);
        tx += iw;
    }
    cv.text(tx, y + h / 2.0 + size * 0.36, style.color(if hover { FROST } else { color }), s);
    w
}

/// Pastille d'information (écran virtuel) ; rend sa largeur.
pub fn info_chip(cv: &mut Canvas, x: f32, y: f32, h: f32, size: f32, icon: Icon, parts: &[(&str, bool)]) -> f32 {
    let iw = size * 1.25;
    let mut w = size * 1.0 + iw + size * 0.6;
    for (s, mono) in parts {
        let st = Text::new(if *mono { Font::Mono } else { Font::Regular }, if *mono { size * 0.92 } else { size }, MUTED);
        w += cv.text_width(st, s);
    }
    w += size;
    cv.rrect(x, y, w, h, 8.0, Rgba(9, 13, 18, 184));
    cv.rrect_stroke(x, y, w, h, 8.0, 1.0, LINE_2);
    cv.icon(&icon, x + size, y + (h - iw) / 2.0, iw, 1.6, MUTED);
    let mut tx = x + size + iw + size * 0.6;
    for (s, mono) in parts {
        let st = Text::new(if *mono { Font::Mono } else { Font::Regular }, if *mono { size * 0.92 } else { size }, if *mono { FROST } else { MUTED });
        tx += cv.text(tx, y + h / 2.0 + size * 0.36, st, s);
    }
    w
}

/// Interrupteur.
pub fn toggle(cv: &mut Canvas, x: f32, y: f32, h: f32, on: bool, enabled: bool) {
    let w = h * 1.7;
    let a = if enabled { 1.0 } else { 0.45 };
    cv.rrect(x, y, w, h, h / 2.0, if on { FLAT.alpha(a) } else { LINE_2.alpha(a) });
    let r = h / 2.0 - h * 0.12;
    let cx = if on { x + w - h / 2.0 } else { x + h / 2.0 };
    cv.circle(cx, y + h / 2.0, r, Rgba(255, 255, 255, (255.0 * a) as u8));
}

/// Pastille d'état (en ligne, hors ligne, occupé).
pub fn dot(cv: &mut Canvas, x: f32, y: f32, r: f32, c: Rgba, glow: bool) {
    if glow {
        cv.circle(x, y, r * 2.4, c.alpha(0.22));
    }
    cv.circle(x, y, r, c);
}

/// Un bouton de la manette, pour la légende : rond (A, B, X, Y) ou arrondi
/// (L1, R1, ☰). Rend sa largeur.
pub fn pad_glyph(cv: &mut Canvas, x: f32, cy: f32, size: f32, label: &str) -> f32 {
    let round = label.chars().count() == 1 && label.chars().all(|c| c.is_ascii_alphabetic());
    let style = Text::new(Font::MonoMedium, size * 0.46, FROST);
    let drawn = label == "☰" || label == "⧉" || label == "⏻";
    let w = if round || drawn { size } else { (cv.text_width(style, label) + size * 0.55).max(size) };
    if round {
        cv.rrect_stroke(x, cy - size / 2.0, size, size, size / 2.0, 1.5, MUTED);
    } else {
        cv.rrect_stroke(x, cy - size / 2.0, w, size, size * 0.27, 1.5, MUTED);
    }
    // Start (☰) et Select (⧉) : absents des polices de la charte, dessinés au trait.
    let s = size * 0.36;
    let (gx, gy) = (x + w / 2.0, cy);
    match label {
        "☰" => {
            for dy in [-0.32f32, 0.0, 0.32] {
                cv.line(gx - s * 0.55, gy + dy * s * 1.4, gx + s * 0.55, gy + dy * s * 1.4, 1.6, FROST);
            }
        }
        "⧉" => {
            cv.rrect_stroke(gx - s * 0.62, gy - s * 0.62, s * 0.85, s * 0.85, 1.5, 1.4, FROST);
            cv.rrect_stroke(gx - s * 0.22, gy - s * 0.22, s * 0.85, s * 0.85, 1.5, 1.4, FROST);
        }
        "⏻" => {
            // Un arc ouvert en haut, un trait vertical : marche / veille.
            let r = s * 0.8;
            let pts: Vec<(f32, f32)> = (0..=24).map(|i| { let a = 0.25 * std::f32::consts::PI + i as f32 / 24.0 * 1.5 * std::f32::consts::PI; (gx + r * (a - std::f32::consts::FRAC_PI_2).cos(), gy + r * (a - std::f32::consts::FRAC_PI_2).sin()) }).collect();
            cv.polyline(&pts, 1.6, FROST);
            cv.line(gx, gy - r * 1.05, gx, gy - r * 0.1, 1.6, FROST);
        }
        _ => {
            cv.text(gx, cy + size * 0.17, style.align(Align::Center), label);
        }
    }
    w
}

/// Une entrée de la légende : boutons puis ce qu'ils font. Rend la largeur.
pub fn legend(cv: &mut Canvas, x: f32, cy: f32, size: f32, keys: &[&str], s: &str) -> f32 {
    let mut xx = x;
    for k in keys {
        xx += pad_glyph(cv, xx, cy, size, k) + size * 0.2;
    }
    xx += size * 0.15;
    let w = cv.text(xx, cy + size * 0.2, Text::new(Font::Regular, size * 0.54, MUTED), s);
    xx + w - x
}

/// Largeur d'une entrée de légende (pour l'aligner à droite).
pub fn legend_width(cv: &mut Canvas, size: f32, keys: &[&str], s: &str) -> f32 {
    let style = Text::new(Font::MonoMedium, size * 0.46, FROST);
    let mut w = 0.0;
    for k in keys {
        let round = (k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_alphabetic())) || *k == "☰" || *k == "⧉" || *k == "⏻";
        w += if round { size } else { (cv.text_width(style, k) + size * 0.55).max(size) } + size * 0.2;
    }
    w + size * 0.15 + cv.text_width(Text::new(Font::Regular, size * 0.54, MUTED), s)
}

/// Une carte d'ordinateur : nom, pastille d'état, deux lignes en Martian Mono.
/// `dashed` : trouvé sur le réseau, pas encore ajouté.
#[allow(clippy::too_many_arguments)]
pub fn card(cv: &mut Canvas, f: &mut Frame, t: Target, r: Rect, scale: f32, name: &str, dot_color: Option<Rgba>, badge: Option<&str>, lines: &[&str], selected: bool, dashed: bool, dim: bool) {
    let (focused, hover) = f.state(t, r);
    if focused {
        ring(cv, r, 10.0 * scale);
    }
    cv.rrect(r.x, r.y, r.w, r.h, 10.0 * scale, if hover { Rgba(20, 27, 35, 235) } else { CARD });
    if selected {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * scale, 1.5, SPECTRE);
        cv.rrect_stroke(r.x - 4.0, r.y - 4.0, r.w + 8.0, r.h + 8.0, 13.0 * scale, 3.0, HALO.alpha(0.22));
    } else if dashed {
        cv.rrect_dashed(r.x, r.y, r.w, r.h, 10.0 * scale, 1.0, LINE_2);
    } else {
        cv.rrect_stroke(r.x, r.y, r.w, r.h, 10.0 * scale, 1.0, LINE);
    }
    let pad = 15.0 * scale;
    let name_style = Text::new(Font::Semibold, 15.5 * scale, if dim { MUTED } else { FROST });
    let right = r.x + r.w - pad;
    let mut name_max = r.w - 2.0 * pad - 14.0 * scale;
    if let Some(b) = badge {
        let bs = Text::new(Font::Mono, 9.5 * scale, SPECTRE).tracking(scale);
        let bw = cv.text(right, r.y + pad + 12.0 * scale, bs.align(Align::Right), &b.to_uppercase());
        name_max -= bw + 8.0 * scale;
    }
    cv.text_fit(r.x + pad, r.y + pad + 13.0 * scale, name_style, name, name_max);
    if let Some(c) = dot_color {
        dot(cv, right - 4.0 * scale, r.y + pad + 8.0 * scale, 4.0 * scale, c, false);
    }
    let mono = Text::new(Font::Mono, 10.5 * scale, if dim { GHOST } else { FAINT });
    let mut yy = r.y + r.h - pad - (lines.len().saturating_sub(1)) as f32 * 16.0 * scale;
    for l in lines {
        cv.text_fit(r.x + pad, yy, mono, l, r.w - 2.0 * pad);
        yy += 16.0 * scale;
    }
}

/// Un panneau d'encre translucide (dernière session) :
/// lisible quelle que soit l'image derrière.
pub fn panel(cv: &mut Canvas, r: Rect, radius: f32) {
    cv.rrect(r.x, r.y, r.w, r.h, radius, PANEL);
    cv.rrect_stroke(r.x, r.y, r.w, r.h, radius, 1.0, LINE);
}

/// Un QR code sur une carte claire (marge de quatre modules), net sur les pixels de l'écran : les modules
/// sont calés sur des pixels entiers. `side` : le côté voulu, en points ; rend le côté de la carte. `None` :
/// le texte est trop long pour un QR code.
pub fn qr_card(cv: &mut Canvas, text: &str, x: f32, y: f32, side: f32) -> Option<f32> {
    let qr = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
    let n = qr.size();
    let k = cv.k;
    let module_px = ((side * k) / (n + 8) as f32).floor().max(1.0);
    let card = module_px * (n + 8) as f32 / k;
    let (cx, cy) = (((x) * k).round() / k, (y * k).round() / k);
    cv.rrect(cx, cy, card, card, 8.0, Rgba(255, 255, 255, 255));
    let m = module_px / k;
    for my in 0..n {
        for mx in 0..n {
            if qr.get_module(mx, my) {
                cv.rect(cx + (mx + 4) as f32 * m, cy + (my + 4) as f32 * m, m, m, INK);
            }
        }
    }
    Some(card)
}
