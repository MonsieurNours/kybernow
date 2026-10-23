//! Dessin de l'interface dans une image RGBA (prémultipliée) de 1920 × 1080,
//! le plan graphique de l'app sur webOS : tiny-skia pour les formes et les
//! images, fontdue pour le texte (polices de la charte, instances fixes de
//! `assets/fonts`). L'image est ensuite envoyée à l'écran par SDL ; pendant la
//! session, les zones transparentes laissent voir l'image de l'ordinateur,
//! que le téléviseur affiche sur son plan vidéo, dessous.

use std::collections::HashMap;

use tiny_skia::{FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

pub const W: f32 = 1920.0;
pub const H: f32 = 1080.0;

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
}

impl Canvas {
    pub fn new() -> Self {
        Self { pix: Pixmap::new(W as u32, H as u32).unwrap(), fonts: Fonts::load() }
    }

    /// Les octets RGBA prémultipliés, ligne par ligne (1920 × 4 par ligne).
    pub fn bytes(&self) -> &[u8] {
        self.pix.data()
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
            self.pix.fill_rect(r, &c.paint(), Transform::identity(), None);
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
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn rrect_stroke(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        if let Some(p) = Self::rrect_path(x + half, y + half, w - width, h - width, (r - half).max(0.0)) {
            let s = Stroke { width, ..Default::default() };
            self.pix.stroke_path(&p, &c.paint(), &s, Transform::identity(), None);
        }
    }

    /// Bordure en pointillés (cartes « sur le réseau », réglages indisponibles).
    pub fn rrect_dashed(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        let half = width / 2.0;
        if let Some(p) = Self::rrect_path(x + half, y + half, w - width, h - width, (r - half).max(0.0)) {
            let s = Stroke { width, dash: tiny_skia::StrokeDash::new(vec![8.0, 7.0], 0.0), ..Default::default() };
            self.pix.stroke_path(&p, &c.paint(), &s, Transform::identity(), None);
        }
    }

    /// Le néon de la charte autour d'une forme : anneau net, puis halo
    /// dégradé (couches de plus en plus larges et transparentes).
    pub fn neon(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, strength: f32) {
        let halo = super::theme::HALO;
        for i in (1..=9).rev() {
            let s = i as f32 * 4.5;
            let a = 0.055 * strength * (1.0 - i as f32 / 10.0);
            self.rrect(x - s, y - s, w + 2.0 * s, h + 2.0 * s, r + s, halo.alpha(a));
        }
        self.rrect_stroke(x - 3.0, y - 3.0, w + 6.0, h + 6.0, r + 3.0, 3.0, super::theme::SPECTRE.alpha(strength));
    }


    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
            self.pix.fill_path(&p, &c.paint(), FillRule::Winding, Transform::identity(), None);
        }
    }

    pub fn circle_stroke(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgba) {
        if let Some(p) = PathBuilder::from_circle(cx, cy, r) {
            let s = Stroke { width, ..Default::default() };
            self.pix.stroke_path(&p, &c.paint(), &s, Transform::identity(), None);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1, y1);
        if let Some(p) = pb.finish() {
            let s = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            self.pix.stroke_path(&p, &c.paint(), &s, Transform::identity(), None);
        }
    }

    /// Un tracé d'icône (coordonnées sur 24, comme les icônes de la charte),
    /// posé en `x, y` à la taille `size`.
    pub fn icon(&mut self, icon: &super::icons::Icon, x: f32, y: f32, size: f32, width: f32, c: Rgba) {
        let scale = size / 24.0;
        let t = Transform::from_row(scale, 0.0, 0.0, scale, x, y);
        let s = Stroke { width: width / scale, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        for path in icon.strokes() {
            self.pix.stroke_path(&path, &c.paint(), &s, t, None);
        }
        for path in icon.fills() {
            self.pix.fill_path(&path, &c.paint(), FillRule::Winding, t, None);
        }
    }

    /// Un texte sur une ligne ; `y` est la ligne de base. Rend la largeur.
    pub fn text(&mut self, x: f32, y: f32, style: Text, s: &str) -> f32 {
        let width = self.fonts.width(style.font, style.size, s, style.tracking);
        let mut pen = match style.align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let mut prev: Option<char> = None;
        let font = style.font;
        let size = style.size;
        for c in s.chars() {
            if let Some(p) = prev {
                pen += self.fonts.fonts[&font].horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let (metrics, coverage) = {
                let g = self.fonts.glyph(font, c, size);
                (g.metrics, g.coverage.clone())
            };
            let gx = (pen + metrics.xmin as f32).round() as i32;
            let gy = (y - metrics.height as f32 - metrics.ymin as f32).round() as i32;
            blit_coverage(&mut self.pix, gx, gy, metrics.width, metrics.height, &coverage, style.color);
            pen += metrics.advance_width + style.tracking;
            prev = Some(c);
        }
        width
    }

    /// Un texte qui ne dépasse pas `max` : coupé avec « … » si besoin.
    pub fn text_fit(&mut self, x: f32, y: f32, style: Text, s: &str, max: f32) -> f32 {
        if self.fonts.width(style.font, style.size, s, style.tracking) <= max {
            return self.text(x, y, style, s);
        }
        let mut cut: String = s.to_string();
        while !cut.is_empty() {
            cut.pop();
            let t = format!("{}…", cut.trim_end());
            if self.fonts.width(style.font, style.size, &t, style.tracking) <= max {
                return self.text(x, y, style, &t);
            }
        }
        0.0
    }

    /// Un paragraphe coupé aux mots sur une largeur ; rend la hauteur occupée.
    pub fn paragraph(&mut self, x: f32, y: f32, style: Text, s: &str, max: f32, line_height: f32) -> f32 {
        let mut line = String::new();
        let mut yy = y;
        let mut lines = 0;
        for word in s.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && self.fonts.width(style.font, style.size, &candidate, style.tracking) > max {
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
        self.fonts.width(style.font, style.size, s, style.tracking)
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

    /// Une image à sa taille (logo), en `x, y`, mise à l'échelle `scale`.
    pub fn image(&mut self, img: &Pixmap, x: f32, y: f32, scale: f32) {
        let t = Transform::from_row(scale, 0.0, 0.0, scale, x, y);
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
