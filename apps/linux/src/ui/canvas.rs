//! Dessin de l'interface dans une image RGBA (prémultipliée) à la taille de la
//! fenêtre, en pixels réels : tiny-skia pour les formes et les images,
//! fontdue pour le texte (polices de la charte, instances fixes de
//! `assets/fonts`). Les écrans dessinent en *points* (ceux des maquettes :
//! 1280 × 800 pour le bureau et le Deck, 1920 × 1080 pour le salon) ; le
//! canevas les multiplie par `k`, les pixels réels par point (écrans HiDPI,
//! fenêtre agrandie). L'image est envoyée à l'écran par SDL ; pendant la
//! session, les zones transparentes laissent voir l'image de l'ordinateur.

use std::collections::HashMap;

use tiny_skia::{FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};


/// Une couleur non prémultipliée (r, g, b, a).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const fn hex(v: u32) -> Self {
        Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
    }

    pub fn alpha(self, a: f32) -> Self {
        Rgba(self.0, self.1, self.2, (self.3 as f32 * a.clamp(0.0, 1.0)).round() as u8)
    }

    fn paint(self) -> Paint<'static> {
        let mut p = Paint::default();
        p.set_color_rgba8(self.0, self.1, self.2, self.3);
        p.anti_alias = true;
        p
    }
}

/// Les polices de l'app.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Font {
    /// Archivo large (125), graisse 720 : le nom de l'ordinateur.
    Display,
    /// Archivo large (125), graisse 650 : titres.
    Title,
    /// Archivo, graisse 420 : texte.
    Regular,
    /// Archivo, graisse 620 : boutons, libellés forts.
    Semibold,
    /// Martian Mono, graisse 400 : données.
    Mono,
    /// Martian Mono, graisse 540.
    MonoMedium,
}

const FONT_FILES: [(Font, &[u8]); 6] = [
    (Font::Display, include_bytes!("../../assets/fonts/archivo-display.ttf")),
    (Font::Title, include_bytes!("../../assets/fonts/archivo-title.ttf")),
    (Font::Regular, include_bytes!("../../assets/fonts/archivo-regular.ttf")),
    (Font::Semibold, include_bytes!("../../assets/fonts/archivo-semibold.ttf")),
    (Font::Mono, include_bytes!("../../assets/fonts/martian-regular.ttf")),
    (Font::MonoMedium, include_bytes!("../../assets/fonts/martian-medium.ttf")),
];

struct Glyph {
    metrics: fontdue::Metrics,
    coverage: Vec<u8>,
}

/// Polices chargées et glyphes déjà rendus (par police, caractère, taille en
/// dixièmes de pixel).
pub struct Fonts {
    fonts: HashMap<Font, fontdue::Font>,
    cache: HashMap<(Font, char, u32), Glyph>,
}

impl Fonts {
    pub fn load() -> Self {
        let mut fonts = HashMap::new();
        for (f, bytes) in FONT_FILES {
            let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).expect("police de la charte illisible");
            fonts.insert(f, font);
        }
        Self { fonts, cache: HashMap::new() }
    }

    fn glyph(&mut self, font: Font, c: char, size: f32) -> &Glyph {
        let key = (font, c, (size * 10.0).round() as u32);
        if !self.cache.contains_key(&key) {
            let f = &self.fonts[&font];
            let (metrics, coverage) = f.rasterize(c, size);
            if self.cache.len() > 6000 {
                self.cache.clear();
            }
            self.cache.insert(key, Glyph { metrics, coverage });
        }
        &self.cache[&key]
    }

    /// Largeur d'un texte, avec l'interlettrage (`tracking`, en pixels).
    pub fn width(&mut self, font: Font, size: f32, text: &str, tracking: f32) -> f32 {
        let mut x = 0.0;
        let mut prev: Option<char> = None;
        for c in text.chars() {
            if let Some(p) = prev {
                x += self.fonts[&font].horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            x += self.glyph(font, c, size).metrics.advance_width + tracking;
            prev = Some(c);
        }
        if prev.is_some() { x - tracking } else { 0.0 }
    }
}

/// Alignement horizontal d'un texte autour de `x`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Un style de texte.
#[derive(Clone, Copy)]
pub struct Text {
    pub font: Font,
    pub size: f32,
    pub color: Rgba,
    pub tracking: f32,
    pub align: Align,
}

impl Text {
    pub const fn new(font: Font, size: f32, color: Rgba) -> Self {
        Self { font, size, color, tracking: 0.0, align: Align::Left }
    }

    pub const fn tracking(mut self, t: f32) -> Self {
        self.tracking = t;
        self
    }

    pub const fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }

    pub const fn color(mut self, c: Rgba) -> Self {
        self.color = c;
        self
    }
}

pub struct Canvas {
    pub pix: Pixmap,
    pub fonts: Fonts,
    /// Pixels réels par point.
    pub k: f32,
    /// Taille en points.
    pub w: f32,
    pub h: f32,
}

impl Canvas {
    pub fn new() -> Self {
        Self { pix: Pixmap::new(1, 1).unwrap(), fonts: Fonts::load(), k: 1.0, w: 1.0, h: 1.0 }
    }

    /// Taille en pixels réels et échelle (pixels par point).
    pub fn resize(&mut self, pw: u32, ph: u32, k: f32) {
        let (pw, ph) = (pw.max(1), ph.max(1));
        if self.pix.width() != pw || self.pix.height() != ph {
            self.pix = Pixmap::new(pw, ph).unwrap();
        }
        self.k = k.max(0.1);
        self.w = pw as f32 / self.k;
        self.h = ph as f32 / self.k;
    }

    /// Les octets RGBA prémultipliés, ligne par ligne.
    pub fn bytes(&self) -> &[u8] {
        self.pix.data()
    }

    fn t(&self) -> Transform {
        Transform::from_scale(self.k, self.k)
    }

    pub fn clear(&mut self, c: Rgba) {
        self.pix.fill(tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3));
    }

    /// Tout transparent : l'image de l'ordinateur (plan vidéo) se voit dessous.
    pub fn clear_transparent(&mut self) {
        self.pix.fill(tiny_skia::Color::TRANSPARENT);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgba) {
        if let Some(r) = Rect::from_xywh(x, y, w.max(0.0), h.max(0.0)) {
            let t = self.t();
            self.pix.fill_rect(r, &c.paint(), t, None);
        }
    }


    fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
        let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
        let k = 0.552_284_8 * r;
        let mut pb = PathBuilder::new();
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
        pb.line_to(x, y + r);
        pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
        pb.close();
        pb.finish()
    }

    pub fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if let Some(p) = Self::rrect_path(x, y, w, h, r) {
            let t = self.t();
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
        }
    }

    /// Un dégradé radial (lueurs de la signature de l'ordinateur).
    pub fn glow(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        let stops = vec![
            tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3)),
            tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(c.0, c.1, c.2, 0)),
        ];
        let Some(shader) = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, 0.0),
            1.0,
            stops,
            tiny_skia::SpreadMode::Pad,
            Transform::from_row(rx, 0.0, 0.0, ry, cx, cy),
        ) else {
            return;
        };
        let paint = Paint { shader, anti_alias: true, ..Default::default() };
        if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, 2.0 * rx, 2.0 * ry) {
            let t = self.t();
            self.pix.fill_rect(r, &paint, t, None);
        }
    }

    /// Un dégradé vertical (ombres sous le texte de l'accueil).
    pub fn vgradient(&mut self, x: f32, y: f32, w: f32, h: f32, top: Rgba, bottom: Rgba) {
        let stops = vec![
            tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(top.0, top.1, top.2, top.3)),
            tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(bottom.0, bottom.1, bottom.2, bottom.3)),
        ];
        let Some(shader) = tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(0.0, y), tiny_skia::Point::from_xy(0.0, y + h), stops, tiny_skia::SpreadMode::Pad, Transform::identity()) else {
            return;
        };
        let paint = Paint { shader, anti_alias: false, ..Default::default() };
        if let Some(r) = Rect::from_xywh(x, y, w.max(0.0), h.max(0.0)) {
            let t = self.t();
            self.pix.fill_rect(r, &paint, t, None);
        }
    }

    /// Un dégradé à plusieurs arrêts (position 0 à 1), vertical ou horizontal.
    pub fn gradient(&mut self, x: f32, y: f32, w: f32, h: f32, horizontal: bool, stops: &[(f32, Rgba)]) {
        let stops: Vec<tiny_skia::GradientStop> = stops.iter().map(|(p, c)| tiny_skia::GradientStop::new(*p, tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3))).collect();
        let (a, b) = if horizontal { ((x, y), (x + w, y)) } else { ((x, y), (x, y + h)) };
        let Some(shader) = tiny_skia::LinearGradient::new(tiny_skia::Point::from_xy(a.0, a.1), tiny_skia::Point::from_xy(b.0, b.1), stops, tiny_skia::SpreadMode::Pad, Transform::identity()) else {
            return;
        };
        let paint = Paint { shader, anti_alias: false, ..Default::default() };
        if let Some(r) = Rect::from_xywh(x, y, w.max(0.0), h.max(0.0)) {
            let t = self.t();
            self.pix.fill_rect(r, &paint, t, None);
        }
    }

    /// Une image de la taille du canevas, découpée par un polygone (en points),
    /// décalée de `(dx, dy)` points et estompée : une moitié de l'accueil au passage.
    pub fn draw_masked(&mut self, src: &Pixmap, poly: &[(f32, f32)], dx: f32, dy: f32, opacity: f32) {
        if opacity <= 0.0 {
            return;
        }
        let k = self.k;
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in poly.iter().enumerate() {
            let (px, py) = ((x + dx) * k, (y + dy) * k);
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        pb.close();
        let Some(path) = pb.finish() else { return };
        let Some(mut mask) = tiny_skia::Mask::new(self.pix.width(), self.pix.height()) else { return };
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        let paint = PixmapPaint { opacity: opacity.clamp(0.0, 1.0), quality: FilterQuality::Nearest, ..Default::default() };
        self.pix.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_translate(dx * k, dy * k), Some(&mask));
    }

    /// Un polygone plein (coins coupés à 45°, couture).
    pub fn polygon(&mut self, pts: &[(f32, f32)], c: Rgba) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 { pb.move_to(*x, *y) } else { pb.line_to(*x, *y) }
        }
        pb.close();
        if let Some(p) = pb.finish() {
            let t = self.t();
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
        }
    }

    /// Le symbole KyberNow (48 × 48 en points de la charte) posé en `x, y`,
    /// à la taille `size` : moitié haute, moitié basse, et la couture.
    pub fn mark(&mut self, x: f32, y: f32, size: f32, top: Rgba, bottom: Rgba, seam: Option<Rgba>) {
        let s = size / 48.0;
        let t = Transform::from_row(s * self.k, 0.0, 0.0, s * self.k, x * self.k, y * self.k);
        for (d, c) in [(MARK_TOP, top), (MARK_BOTTOM, bottom)] {
            if let Some(p) = svg_path(d) {
                self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
            }
        }
        if let Some(c) = seam
            && let Some(p) = svg_path(MARK_SEAM)
        {
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
        }
    }

    pub fn rrect_stroke(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        if let Some(p) = Self::rrect_path(x + half, y + half, w - width, h - width, (r - half).max(0.0)) {
            let s = Stroke { width, ..Default::default() };
            let t = self.t();
            self.pix.stroke_path(&p, &c.paint(), &s, t, None);
        }
    }

    /// Bordure en pointillés (cartes « sur le réseau », réglages indisponibles).
    pub fn rrect_dashed(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        if let Some(p) = Self::rrect_path(x + half, y + half, w - width, h - width, (r - half).max(0.0)) {
            let s = Stroke { width, dash: tiny_skia::StrokeDash::new(vec![6.0, 5.0], 0.0), ..Default::default() };
            let t = self.t();
            self.pix.stroke_path(&p, &c.paint(), &s, t, None);
        }
    }

    /// Le néon de la charte autour d'une forme : anneau net, puis halo
    /// dégradé (couches de plus en plus larges et transparentes).
    pub fn neon(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, strength: f32) {
        let halo = super::theme::HALO;
        for i in (1..=7).rev() {
            let s = i as f32 * 3.0;
            let a = 0.06 * strength * (1.0 - i as f32 / 8.0);
            self.rrect(x - s, y - s, w + 2.0 * s, h + 2.0 * s, r + s, halo.alpha(a));
        }
        // Anneau de la charte : cœur blanc, gaine Spectre.
        self.rrect_stroke(x - 7.0, y - 7.0, w + 14.0, h + 14.0, r + 7.0, 4.0, super::theme::SPECTRE.alpha(0.35 * strength));
        self.rrect_stroke(x - 4.0, y - 4.0, w + 8.0, h + 8.0, r + 4.0, 2.5, super::theme::CORE.alpha(strength));
    }


    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
            let t = self.t();
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
        }
    }

    pub fn circle_stroke(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgba) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
            let s = Stroke { width, ..Default::default() };
            let t = self.t();
            self.pix.stroke_path(&p, &c.paint(), &s, t, None);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgba) {
        self.polyline(&[(x0, y0), (x1, y1)], width, c);
    }

    /// Une ligne brisée (courbe de latence).
    pub fn polyline(&mut self, pts: &[(f32, f32)], width: f32, c: Rgba) {
        if pts.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for (x, y) in &pts[1..] {
            pb.line_to(*x, *y);
        }
        if let Some(p) = pb.finish() {
            let s = Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
            let t = self.t();
            self.pix.stroke_path(&p, &c.paint(), &s, t, None);
        }
    }

    /// Un tracé d'icône (coordonnées sur 24, comme les icônes de la charte),
    /// posé en `x, y` à la taille `size`.
    pub fn icon(&mut self, icon: &super::icons::Icon, x: f32, y: f32, size: f32, width: f32, c: Rgba) {
        let scale = size / 24.0;
        let t = Transform::from_row(scale * self.k, 0.0, 0.0, scale * self.k, x * self.k, y * self.k);
        let s = Stroke { width: width / scale, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        for path in icon.strokes() {
            self.pix.stroke_path(&path, &c.paint(), &s, t, None);
        }
        for path in icon.fills() {
            self.pix.fill_path(&path, &c.paint(), FillRule::Winding, t, None);
        }
    }

    /// Un texte sur une ligne ; `y` est la ligne de base. Rend la largeur (en points).
    pub fn text(&mut self, x: f32, y: f32, style: Text, s: &str) -> f32 {
        let k = self.k;
        let width = self.fonts.width(style.font, style.size * k, s, style.tracking * k) / k;
        let mut pen = match style.align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        } * k;
        let mut prev: Option<char> = None;
        let font = style.font;
        let size = style.size * k;
        let base = y * k;
        for c in s.chars() {
            if let Some(p) = prev {
                pen += self.fonts.fonts[&font].horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let (metrics, coverage) = {
                let g = self.fonts.glyph(font, c, size);
                (g.metrics, g.coverage.clone())
            };
            let gx = (pen + metrics.xmin as f32).round() as i32;
            let gy = (base - metrics.height as f32 - metrics.ymin as f32).round() as i32;
            blit_coverage(&mut self.pix, gx, gy, metrics.width, metrics.height, &coverage, style.color);
            pen += metrics.advance_width + style.tracking * k;
            prev = Some(c);
        }
        width
    }

    /// Un texte qui ne dépasse pas `max` : coupé avec « … » si besoin.
    pub fn text_fit(&mut self, x: f32, y: f32, style: Text, s: &str, max: f32) -> f32 {
        if self.text_width(style, s) <= max {
            return self.text(x, y, style, s);
        }
        let mut cut: String = s.to_string();
        while !cut.is_empty() {
            cut.pop();
            let t = format!("{}…", cut.trim_end());
            if self.text_width(style, &t) <= max {
                return self.text(x, y, style, &t);
            }
        }
        0.0
    }

    /// Un texte rétréci jusqu'à tenir dans `max` (le nom de l'ordinateur).
    pub fn text_shrink(&mut self, x: f32, y: f32, style: Text, s: &str, max: f32) -> f32 {
        let w = self.text_width(style, s);
        if w <= max {
            return self.text(x, y, style, s);
        }
        let size = (style.size * max / w).max(style.size * 0.4);
        self.text_fit(x, y, Text { size, tracking: style.tracking * size / style.size, ..style }, s, max)
    }

    /// Un paragraphe coupé aux mots sur une largeur ; rend la hauteur occupée.
    pub fn paragraph(&mut self, x: f32, y: f32, style: Text, s: &str, max: f32, line_height: f32) -> f32 {
        let mut line = String::new();
        let mut yy = y;
        let mut lines = 0;
        for word in s.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && self.text_width(style, &candidate) > max {
                self.text(x, yy, style, &line);
                yy += line_height;
                lines += 1;
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            self.text(x, yy, style, &line);
            lines += 1;
        }
        lines as f32 * line_height
    }

    pub fn text_width(&mut self, style: Text, s: &str) -> f32 {
        let k = self.k;
        self.fonts.width(style.font, style.size * k, s, style.tracking * k) / k
    }

    /// Une image posée dans un rectangle, recadrée pour le couvrir (comme
    /// `object-fit: cover`), avec une opacité.
    pub fn image_cover(&mut self, img: &Pixmap, x: f32, y: f32, w: f32, h: f32, opacity: f32) {
        self.image_cover_rounded(img, x, y, w, h, 0.0, opacity);
    }

    /// Comme `image_cover`, dans un rectangle aux coins arrondis. L'image est
    /// d'abord réduite dans une image de la taille du rectangle (le masque des
    /// coins ne couvre qu'elle, pas tout l'écran).
    pub fn image_cover_rounded(&mut self, img: &Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, opacity: f32) {
        let k = self.k;
        let (x, y, w, h, r) = (x * k, y * k, w * k, h * k, r * k);
        let (iw, ih) = (img.width() as f32, img.height() as f32);
        let (pw, ph) = (w.round().max(1.0) as u32, h.round().max(1.0) as u32);
        if iw <= 0.0 || ih <= 0.0 {
            return;
        }
        let Some(mut tile) = Pixmap::new(pw, ph) else { return };
        let scale = (w / iw).max(h / ih);
        let (dw, dh) = (iw * scale, ih * scale);
        let t = Transform::from_row(scale, 0.0, 0.0, scale, (w - dw) / 2.0, (h - dh) / 2.0);
        let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
        tile.draw_pixmap(0, 0, img.as_ref(), &paint, t, None);
        if r > 0.0
            && let Some(p) = Self::rrect_path(0.0, 0.0, w, h, r)
            && let Some(mut m) = tiny_skia::Mask::new(pw, ph)
        {
            m.fill_path(&p, FillRule::Winding, true, Transform::identity());
            tile.apply_mask(&m);
        }
        let paint = PixmapPaint { opacity, quality: FilterQuality::Nearest, ..Default::default() };
        self.pix.draw_pixmap(x.round() as i32, y.round() as i32, tile.as_ref(), &paint, Transform::identity(), None);
    }

    #[allow(dead_code)]
    /// Une image (logo) en `x, y`, à la hauteur `height` (en points).
    pub fn image_height(&mut self, img: &Pixmap, x: f32, y: f32, height: f32, opacity: f32) {
        let scale = height * self.k / img.height().max(1) as f32;
        let t = Transform::from_row(scale, 0.0, 0.0, scale, x * self.k, y * self.k);
        let paint = PixmapPaint { opacity, quality: FilterQuality::Bicubic, ..Default::default() };
        self.pix.draw_pixmap(0, 0, img.as_ref(), &paint, t, None);
    }

    #[allow(dead_code)]
    /// Une image à sa taille (logo), en `x, y`, mise à l'échelle `scale`.
    pub fn image(&mut self, img: &Pixmap, x: f32, y: f32, scale: f32) {
        let scale = scale * self.k;
        let t = Transform::from_row(scale, 0.0, 0.0, scale, x * self.k, y * self.k);
        let paint = PixmapPaint { quality: FilterQuality::Bilinear, ..Default::default() };
        self.pix.draw_pixmap(0, 0, img.as_ref(), &paint, t, None);
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}


/// Pose une couverture de glyphe (0–255) dans l'image, en couleur `c`
/// (source-over, prémultiplié).
fn blit_coverage(pix: &mut Pixmap, x: i32, y: i32, w: usize, h: usize, cov: &[u8], c: Rgba) {
    let (pw, ph) = (pix.width() as i32, pix.height() as i32);
    let data = pix.data_mut();
    let ca = c.3 as u32;
    for row in 0..h as i32 {
        let py = y + row;
        if py < 0 || py >= ph {
            continue;
        }
        for col in 0..w as i32 {
            let px = x + col;
            if px < 0 || px >= pw {
                continue;
            }
            let a = cov[(row as usize) * w + col as usize] as u32 * ca / 255;
            if a == 0 {
                continue;
            }
            let i = ((py * pw + px) * 4) as usize;
            let inv = 255 - a;
            data[i] = ((c.0 as u32 * a + data[i] as u32 * inv) / 255) as u8;
            data[i + 1] = ((c.1 as u32 * a + data[i + 1] as u32 * inv) / 255) as u8;
            data[i + 2] = ((c.2 as u32 * a + data[i + 2] as u32 * inv) / 255) as u8;
            data[i + 3] = (a + data[i + 3] as u32 * inv / 255) as u8;
        }
    }
}

/// Décode une image JPEG (aperçu d'Admin, dernière image) en image prémultipliée.
pub fn decode_jpeg(bytes: &[u8]) -> Option<Pixmap> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut d = JpegDecoder::new_with_options(bytes, opts);
    let pixels = d.decode().ok()?;
    let (w, h) = d.dimensions()?;
    let mut pix = Pixmap::new(w as u32, h as u32)?;
    pix.data_mut().copy_from_slice(&pixels[..(w * h * 4)]);
    Some(pix)
}

/// Décode une image PNG (logo) en image prémultipliée.
pub fn decode_png(bytes: &[u8]) -> Option<Pixmap> {
    Pixmap::decode_png(bytes).ok()
}

/// Encode l'image en JPEG… pas besoin : la dernière image de la session est
/// gardée telle quelle (aperçu d'Admin). Ici, l'encodage PNG des captures.
pub fn encode_png(pix: &Pixmap) -> Option<Vec<u8>> {
    pix.encode_png().ok()
}

/// Le symbole « Relais » de la charte (brand/v2/tools/generate.mjs, grille de 48).
const MARK_TOP: &str = "M7 0 L42.275 0 A1.2 1.2 0 0 1 43.123 2.049 L2.049 43.123 A1.2 1.2 0 0 1 0 42.275 L0 7 A7 7 0 0 1 7 0 Z";
const MARK_BOTTOM: &str = "M48 5.725 L48 41 A7 7 0 0 1 41 48 L5.725 48 A1.2 1.2 0 0 1 4.877 45.951 L45.951 4.877 A1.2 1.2 0 0 1 48 5.725 Z";
const MARK_SEAM: &str = "M6.237 43.035 L43.035 6.237 A0.85 0.85 0 0 0 43.035 5.035 L42.965 4.965 A0.85 0.85 0 0 0 41.763 4.965 L4.965 41.763 A0.85 0.85 0 0 0 4.965 42.965 L5.035 43.035 A0.85 0.85 0 0 0 6.237 43.035 Z";

/// Les tracés du symbole : M, L, Z et des arcs de cercle (A), ramenés à des
/// courbes de Bézier quadratiques (les arcs du symbole sont des quarts ou
/// moins).
fn svg_path(d: &str) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let toks: Vec<&str> = d.split_whitespace().collect();
    let mut i = 0;
    let mut cur = (0.0f32, 0.0f32);
    let num = |s: &str| s.trim_start_matches(['M', 'L', 'A']).parse::<f32>().unwrap_or(0.0);
    while i < toks.len() {
        let t = toks[i];
        if t.starts_with('M') || t.starts_with('L') {
            let x = num(t);
            let y = num(toks.get(i + 1)?);
            if t.starts_with('M') { pb.move_to(x, y) } else { pb.line_to(x, y) }
            cur = (x, y);
            i += 2;
        } else if t.starts_with('A') {
            let r = num(t);
            let sweep = num(toks.get(i + 4)?) != 0.0;
            let x = num(toks.get(i + 5)?);
            let y = num(toks.get(i + 6)?);
            arc(&mut pb, cur, (x, y), r, sweep);
            cur = (x, y);
            i += 7;
        } else if t == "Z" {
            pb.close();
            i += 1;
        } else {
            i += 1;
        }
    }
    pb.finish()
}

/// Un petit arc de cercle de rayon `r` de `a` à `b` (sens horaire si `sweep`).
fn arc(pb: &mut PathBuilder, a: (f32, f32), b: (f32, f32), r: f32, sweep: bool) {
    let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = (dx * dx + dy * dy).sqrt().max(1e-6);
    let h = (r * r - d * d / 4.0).max(0.0).sqrt();
    let (nx, ny) = (-dy / d, dx / d);
    let s = if sweep { 1.0 } else { -1.0 };
    let (cx, cy) = (mx + nx * h * s, my + ny * h * s);
    // Point de contrôle : intersection des tangentes en a et b.
    let (ta, tb) = ((a.0 - cx, a.1 - cy), (b.0 - cx, b.1 - cy));
    let cross = ta.0 * tb.1 - ta.1 * tb.0;
    let dot = ta.0 * tb.0 + ta.1 * tb.1;
    let angle = cross.atan2(dot);
    let k = 1.0 / (angle / 2.0).cos().max(0.2);
    let (bx, by) = ((ta.0 + tb.0) / 2.0, (ta.1 + tb.1) / 2.0);
    let bl = (bx * bx + by * by).sqrt().max(1e-6);
    let (qx, qy) = (cx + bx / bl * r * k, cy + by / bl * r * k);
    pb.quad_to(qx, qy, b.0, b.1);
}

/// Un tracé SVG (M, L, H, V, Q, C, Z, en absolu ou relatif, répétitions
/// implicites), en tiny-skia : le nom et la signature du logo.
pub fn parse_svg(d: &str) -> Option<tiny_skia::Path> {
    let b = d.as_bytes();
    let mut i = 0;
    let mut pb = PathBuilder::new();
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    let mut cmd = b'M';
    let skip = |i: &mut usize| {
        while *i < b.len() && (b[*i] == b' ' || b[*i] == b',' || b[*i] == b'\n') {
            *i += 1;
        }
    };
    let number = |i: &mut usize| -> Option<f32> {
        skip(i);
        let start = *i;
        if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
            *i += 1;
        }
        let mut dot = false;
        while *i < b.len() && (b[*i].is_ascii_digit() || (b[*i] == b'.' && !dot)) {
            if b[*i] == b'.' {
                dot = true;
            }
            *i += 1;
        }
        if *i < b.len() && (b[*i] == b'e' || b[*i] == b'E') {
            *i += 1;
            if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
                *i += 1;
            }
            while *i < b.len() && b[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        std::str::from_utf8(&b[start..*i]).ok()?.parse().ok()
    };
    loop {
        skip(&mut i);
        if i >= b.len() {
            break;
        }
        if b[i].is_ascii_alphabetic() {
            cmd = b[i];
            i += 1;
            if cmd == b'Z' || cmd == b'z' {
                pb.close();
                cx = sx;
                cy = sy;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let x = number(&mut i)? + ox;
                let y = number(&mut i)? + oy;
                pb.move_to(x, y);
                (cx, cy, sx, sy) = (x, y, x, y);
                // Les paires suivantes sont des L.
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let x = number(&mut i)? + ox;
                let y = number(&mut i)? + oy;
                pb.line_to(x, y);
                (cx, cy) = (x, y);
            }
            b'H' => {
                let x = number(&mut i)? + ox;
                pb.line_to(x, cy);
                cx = x;
            }
            b'V' => {
                let y = number(&mut i)? + oy;
                pb.line_to(cx, y);
                cy = y;
            }
            b'Q' => {
                let (x1, y1) = (number(&mut i)? + ox, number(&mut i)? + oy);
                let (x, y) = (number(&mut i)? + ox, number(&mut i)? + oy);
                pb.quad_to(x1, y1, x, y);
                (cx, cy) = (x, y);
            }
            b'C' => {
                let (x1, y1) = (number(&mut i)? + ox, number(&mut i)? + oy);
                let (x2, y2) = (number(&mut i)? + ox, number(&mut i)? + oy);
                let (x, y) = (number(&mut i)? + ox, number(&mut i)? + oy);
                pb.cubic_to(x1, y1, x2, y2, x, y);
                (cx, cy) = (x, y);
            }
            _ => {
                i += 1;
            }
        }
    }
    pb.finish()
}

impl Canvas {
    fn fill_svg(&mut self, d: &str, t: Transform, c: Rgba) {
        if let Some(p) = parse_svg(d) {
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, t, None);
        }
    }

    /// Le logo signé de KyberNow Link (symbole, nom, étiquette LINK) en `x, y`,
    /// à la hauteur `h` en points ; rend sa largeur. `alpha` : opacité.
    pub fn logo_link(&mut self, x: f32, y: f32, h: f32, alpha: f32) -> f32 {
        use super::brand::*;
        use super::theme::*;
        let s = h / SIGNED_H;
        let k = self.k;
        self.mark(x, y, MARK * s, FROST.alpha(alpha), SPECTRE.alpha(alpha), None);
        let t = Transform::from_row(s * k, 0.0, 0.0, s * k, (x + WORD_X * s) * k, (y + BASELINE * s) * k);
        self.fill_svg(WORD_KYBER, t, FROST.alpha(alpha));
        self.fill_svg(WORD_NOW, t.pre_translate(NOW_OFFSET, 0.0), FROST.alpha(alpha));
        let t = Transform::from_row(s * k, 0.0, 0.0, s * k, x * k, y * k);
        self.fill_svg(TAG, t, Rgba::hex(0x1E2832).alpha(alpha));
        let ts = Transform::from_row(s * SIG_SCALE * k, 0.0, 0.0, s * SIG_SCALE * k, (x + SIG_X * s) * k, (y + SIG_Y * s) * k);
        self.fill_svg(SIG, ts, Rgba::hex(0xC9D5DA).alpha(alpha));
        SIGNED_W * s
    }

    /// Le nom KyberNow seul, ligne de base en `y`, à la hauteur de capitale
    /// `cap` (en points) ; rend sa largeur.
    pub fn wordmark(&mut self, x: f32, y: f32, cap: f32, c: Rgba) -> f32 {
        use super::brand::*;
        // Hauteur des capitales du nom : 68,66 unités.
        let s = cap / 68.66;
        let k = self.k;
        let t = Transform::from_row(s * k, 0.0, 0.0, s * k, x * k, y * k);
        self.fill_svg(WORD_KYBER, t, c);
        self.fill_svg(WORD_NOW, t.pre_translate(NOW_OFFSET, 0.0), c);
        WORD_W * s
    }

    /// L'étiquette LINK seule (coin coupé), à la hauteur `h`, en `x, y`.
    pub fn link_tag(&mut self, x: f32, y: f32, h: f32, alpha: f32) -> f32 {
        use super::brand::*;
        // L'étiquette du logo : de 586,245 à 713,061 en x, de 97,378 à 133,762 en y.
        let s = h / (133.762 - 97.378);
        let k = self.k;
        let (ox, oy) = (586.245, 97.378);
        let t = Transform::from_row(s * k, 0.0, 0.0, s * k, (x - ox * s) * k, (y - oy * s) * k);
        self.fill_svg(TAG, t, Rgba::hex(0x1E2832).alpha(alpha));
        let ts = Transform::from_row(s * SIG_SCALE * k, 0.0, 0.0, s * SIG_SCALE * k, (x + (SIG_X - ox) * s) * k, (y + (SIG_Y - oy) * s) * k);
        self.fill_svg(SIG, ts, Rgba::hex(0xC9D5DA).alpha(alpha));
        (713.061 - 586.245) * s
    }

    /// Le symbole, avec son néon (lancement, en-têtes) : `lit` de 0 (éteint)
    /// à 1 (allumé), `flash` pour un éclair.
    pub fn mark_lit(&mut self, x: f32, y: f32, size: f32, lit: f32, flash: f32) {
        use super::theme::*;
        let cx = x + size / 2.0;
        let cy = y + size / 2.0;
        if lit > 0.0 {
            self.glow(cx, cy, size * (0.9 + 0.5 * flash), size * (0.9 + 0.5 * flash), HALO.alpha(0.35 * lit + 0.3 * flash));
        }
        let top = Rgba::hex(0x33404B);
        let frost = FROST;
        let mix = |a: Rgba, b: Rgba, t: f32| Rgba(
            (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
            (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
            (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
            255,
        );
        let l = lit.clamp(0.0, 1.0);
        let seam = if flash > 0.0 { mix(CORE, Rgba(255, 255, 255, 255), flash) } else { mix(top, CORE, l) };
        self.mark(x, y, size, mix(top, frost, l), mix(top, SPECTRE, l), Some(seam));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_half_is_drawn() {
        let mut cv = Canvas::new();
        cv.resize(200, 100, 1.0);
        cv.clear(Rgba(200, 10, 10, 255));
        let src = cv.pix.clone();
        cv.clear_transparent();
        cv.draw_masked(&src, &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)], 0.0, 0.0, 1.0);
        let px = cv.pix.pixel(10, 10).unwrap();
        assert!(px.alpha() > 200, "alpha {}", px.alpha());
        assert_eq!(cv.pix.pixel(150, 10).unwrap().alpha(), 0);
    }
}
