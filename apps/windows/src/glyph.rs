//! Dessin de l'icône de KyberNow Link près de l'horloge : le symbole de
//! KyberNow Link (état `now` de brand/v2 ; géométrie de
//! brand/v2/tools/generate.mjs : grille de 48, couture de 4, rayons de 7 et
//! 1,2) d'une seule couleur, dessiné à la taille que Windows demande.
//!
//! Au repos, la moitié haute (l'ordinateur) en contour et la moitié basse
//! (vous) pleine : l'état `now` sans couleur. En session, les deux moitiés
//! pleines : le symbole s'allume. Rien d'une tuile ni d'une pastille (l'icône
//! de KyberNow Station). Sans rien de Windows : testé sur le Mac.

const GRID: f64 = 48.0;
/// Points par axe et par pixel pour l'anticrénelage.
const SAMPLES: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Au repos : moitié haute en contour, moitié basse pleine.
    Idle,
    /// En session : les deux moitiés pleines.
    Lit,
}

/// Points d'un arc de centre `c` et de rayon `r`, de `a0` à `a1` degrés
/// (sens des aiguilles d'une montre à l'écran, y vers le bas), sans le premier.
fn arc(points: &mut Vec<(f64, f64)>, c: (f64, f64), r: f64, a0: f64, a1: f64) {
    let steps = (r * 4.0).ceil().max(4.0) as u32;
    for i in 1..=steps {
        let a = (a0 + (a1 - a0) * f64::from(i) / f64::from(steps)).to_radians();
        points.push((c.0 + r * a.cos(), c.1 + r * a.sin()));
    }
}

/// Contour de la moitié haute (`TOP` de brand/v2/mark/kybernow-mark-now-*.svg) :
/// le coin du carré arrondi de 7, les deux pointes de 1,2.
fn top() -> Vec<(f64, f64)> {
    let mut p = vec![(7.0, 0.0), (42.275, 0.0)];
    arc(&mut p, (42.275, 1.2), 1.2, -90.0, 45.0);
    p.push((2.049, 43.123));
    arc(&mut p, (1.2, 42.275), 1.2, 45.0, 180.0);
    p.push((0.0, 7.0));
    arc(&mut p, (7.0, 7.0), 7.0, 180.0, 270.0);
    p
}

/// La moitié basse : la haute retournée autour du centre.
fn bottom() -> Vec<(f64, f64)> {
    top().into_iter().map(|(x, y)| (GRID - x, GRID - y)).collect()
}

fn inside(poly: &[(f64, f64)], (x, y): (f64, f64)) -> bool {
    let mut odd = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// Distance au bord du contour.
fn edge(poly: &[(f64, f64)], (x, y): (f64, f64)) -> f64 {
    let mut best = f64::MAX;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (ax, ay) = poly[j];
        let (bx, by) = poly[i];
        let (dx, dy) = (bx - ax, by - ay);
        let len = dx * dx + dy * dy;
        let t = if len > 0.0 { (((x - ax) * dx + (y - ay) * dy) / len).clamp(0.0, 1.0) } else { 0.0 };
        let (px, py) = (ax + t * dx - x, ay + t * dy - y);
        best = best.min(px * px + py * py);
        j = i;
    }
    best.sqrt()
}

/// Marge et trait, en pixels entiers : nets à 16, 20, 24 et 32 px.
fn metrics(size: u32) -> (f64, f64) {
    let unit = (f64::from(size) / 16.0).round().max(1.0);
    (unit, unit)
}

/// L'icône en RGBA (alpha non prémultiplié), `size` × `size`, de couleur `rgb`.
pub fn rgba(size: u32, look: Look, rgb: [u8; 3]) -> Vec<u8> {
    let (pad, stroke_px) = metrics(size);
    let scale = (f64::from(size) - 2.0 * pad) / GRID;
    let stroke = stroke_px / scale;
    let (top, bottom) = (top(), bottom());
    let mut out = vec![0u8; (size * size * 4) as usize];
    let n = SAMPLES;
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0u32;
            for sy in 0..n {
                for sx in 0..n {
                    let gx = (f64::from(px) + (f64::from(sx) + 0.5) / f64::from(n) - pad) / scale;
                    let gy = (f64::from(py) + (f64::from(sy) + 0.5) / f64::from(n) - pad) / scale;
                    let p = (gx, gy);
                    let on = inside(&bottom, p)
                        || (inside(&top, p) && (look == Look::Lit || edge(&top, p) <= stroke));
                    hits += u32::from(on);
                }
            }
            let o = ((py * size + px) * 4) as usize;
            out[o..o + 3].copy_from_slice(&rgb);
            out[o + 3] = ((f64::from(hits) / f64::from(n * n)) * 255.0).round() as u8;
        }
    }
    out
}

/// Le point de couleur d'un ordinateur dans le menu de l'icône (son état),
/// en RGBA non prémultiplié : un disque de la moitié de `size`, centré, net
/// à 100, 150 et 200 % (16, 24 et 32 px).
pub fn dot(size: u32, rgb: [u8; 3]) -> Vec<u8> {
    let r = f64::from(size) / 4.0;
    let c = f64::from(size) / 2.0;
    let n = SAMPLES;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0u32;
            for sy in 0..n {
                for sx in 0..n {
                    let x = f64::from(px) + (f64::from(sx) + 0.5) / f64::from(n) - c;
                    let y = f64::from(py) + (f64::from(sy) + 0.5) / f64::from(n) - c;
                    hits += u32::from(x * x + y * y <= r * r);
                }
            }
            let o = ((py * size + px) * 4) as usize;
            out[o..o + 3].copy_from_slice(&rgb);
            out[o + 3] = ((f64::from(hits) / f64::from(n * n)) * 255.0).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point() {
        for size in [16u32, 24, 32] {
            let d = dot(size, [0x4c, 0xd6, 0x8a]);
            let a = |x: u32, y: u32| d[((y * size + x) * 4 + 3) as usize];
            let (c, r) = (size / 2, size / 4);
            // Plein au centre, vide aux coins, symétrique.
            assert_eq!(a(c, c), 255);
            assert_eq!(a(c - 1, c - 1), 255);
            assert_eq!(a(0, 0), 0);
            assert_eq!(a(c - r - 1, c), 0);
            for y in 0..size {
                for x in 0..size {
                    assert_eq!(a(x, y), a(size - 1 - x, y));
                    assert_eq!(a(x, y), a(x, size - 1 - y));
                }
            }
        }
    }

    fn alpha(img: &[u8], size: u32, x: u32, y: u32) -> u8 {
        img[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn dessin() {
        for size in [16, 20, 24, 32] {
            let idle = rgba(size, Look::Idle, [255, 255, 255]);
            let lit = rgba(size, Look::Lit, [255, 255, 255]);
            let (pad, _) = metrics(size);
            let pad = pad as u32;
            // Le coin bas droit (vous) est plein dans les deux états.
            assert_eq!(alpha(&idle, size, size - pad - 3, size - pad - 2), 255, "{size} px");
            // Le milieu de la moitié haute : vide au repos, plein en session.
            let (x, y) = (pad + (size - 2 * pad) / 4, pad + (size - 2 * pad) / 4);
            assert_eq!(alpha(&idle, size, x, y), 0, "{size} px");
            assert_eq!(alpha(&lit, size, x, y), 255, "{size} px");
            // Le bord haut de la moitié haute : un trait d'un pixel net.
            assert_eq!(alpha(&idle, size, size / 2, pad), 255, "{size} px");
            // La marge reste vide.
            assert!(idle.chunks(4).enumerate().all(|(i, p)| {
                let (x, y) = (i as u32 % size, i as u32 / size);
                p[3] == 0 || (x >= pad && y >= pad && x < size - pad && y < size - pad)
            }));
            // La couture reste visible : sur la diagonale, au centre, un creux.
            let c = size / 2;
            assert!(alpha(&lit, size, c, c) < 200, "{size} px");
        }
    }

    /// Aperçus pour l'œil : `KN_GLYPH_DIR=<dossier> cargo test -p kybernow-windows glyph`
    /// écrit chaque taille et chaque état en RGBA brut (`<taille>-<état>.rgba`).
    #[test]
    fn apercus() {
        let Some(dir) = std::env::var_os("KN_GLYPH_DIR") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        for size in [16, 20, 24, 32, 48] {
            for (name, look) in [("idle", Look::Idle), ("lit", Look::Lit)] {
                std::fs::write(dir.join(format!("{size}-{name}.rgba")), rgba(size, look, [255, 255, 255])).unwrap();
            }
            std::fs::write(dir.join(format!("{size}-dot.rgba")), dot(size, [255, 255, 255])).unwrap();
        }
    }
}
