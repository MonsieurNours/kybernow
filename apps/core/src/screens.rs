//! Multi-écrans (contrat : docs/multi-ecrans.md) : des écrans de l'appareil
//! aux écrans virtuels demandés à la Station.
//!
//! L'app donne, pour chaque écran de l'appareil, son cadre dans le repère
//! logique de l'appareil (y vers le bas ; le Mac convertit le repère de
//! Cocoa), sa définition native en pixels, sa fréquence et s'il est
//! principal. [`plan`] en tire, pour quatre écrans au plus, le mode de chaque
//! écran virtuel (« Au plus 4K », limite de l'H.264, fréquence) et sa
//! position en pixels sur le bureau de Windows ([`layout`]).
//!
//! Règles de la disposition :
//! - l'écran principal est en (0, 0) ;
//! - les autres sont placés de proche en proche, en partant du principal :
//!   chacun se colle, du même côté, à un voisin déjà placé avec lequel il
//!   partage un bord sur l'appareil ; son décalage le long de ce bord est
//!   celui de l'appareil, converti avec l'échelle pixels / unités logiques
//!   du voisin ;
//! - deux écrans placés partagent un segment de bord (Windows l'exige) et ne
//!   se chevauchent jamais : un écran qui chevaucherait glisse le long du
//!   bord jusqu'à ne plus chevaucher (parmi les voisins possibles, celui qui
//!   le fait le moins glisser) ;
//! - un écran qui ne touche aucun voisin sur l'appareil se colle au plus
//!   proche, du côté de l'écart le plus grand.

use serde::{Deserialize, Serialize};

use crate::console::{Mode, ScreenMode};
use crate::profile::Codec;

/// Quatre écrans au plus par session.
pub const MAX_SCREENS: usize = 4;
/// Fréquence des écrans autres que le principal, au plus.
pub const OTHER_REFRESH_MAX: u32 = 60;
/// Segment de bord partagé au moins par un écran et le voisin qui le place
/// (en pixels, ou moins si l'un des deux est plus court) : de quoi y faire
/// passer le pointeur.
pub const MIN_SHARED: i64 = 64;
/// Tolérance sur les bords de l'appareil, en unités logiques.
const EDGE_TOLERANCE: f64 = 1.0;

/// Un écran de l'appareil, tel que l'app le voit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DeviceScreen {
    /// Cadre dans le repère logique de l'appareil, y vers le bas.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Définition native en pixels (avant « Au plus 4K »).
    pub pixel_width: u32,
    pub pixel_height: u32,
    /// Fréquence de l'écran (Hz).
    pub refresh: u32,
    /// Écran principal (Mac : la barre des menus ; Windows : l'écran
    /// principal ; Linux : l'écran 0 de SDL). Aucun : le premier.
    #[serde(default)]
    pub primary: bool,
}

/// Les écrans de l'appareil donnés à la session ([`crate::session::Session::start_with_screens`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Screens {
    pub list: Vec<DeviceScreen>,
    /// « Au plus 4K » : chaque écran est ramené à 3840 × 2160 au plus.
    #[serde(default)]
    pub limit_4k: bool,
}

/// Un écran virtuel à demander : l'écran de l'appareil auquel il va
/// (`device`, index dans [`Screens::list`]), son mode et sa position sur le
/// bureau de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedScreen {
    pub device: usize,
    pub mode: Mode,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

impl PlannedScreen {
    pub fn request(&self) -> ScreenMode {
        ScreenMode {
            width: self.mode.width,
            height: self.mode.height,
            refresh: self.mode.refresh,
            x: self.x,
            y: self.y,
            primary: self.primary,
        }
    }
}

/// Définition demandée pour un écran de `width` × `height` pixels natifs :
/// « Au plus 4K » (3840 × 2160), 4096 de large en H.264 (l'encodeur de
/// NVIDIA s'y arrête), 7680 × 4320 sinon (l'écran virtuel) ; proportions
/// gardées, arrondi au pair. Le même calcul que l'app Mac (`DisplayInfo.mode`).
pub fn fit(width: u32, height: u32, limit_4k: bool, codec: Codec) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let (max_w, max_h) = if limit_4k {
        (3840.0, 2160.0)
    } else if codec == Codec::H264 {
        (4096.0, 4096.0)
    } else {
        (7680.0, 4320.0)
    };
    let (w, h) = (f64::from(width), f64::from(height));
    let k = (max_w / w).min(max_h / h).min(1.0);
    let even = |v: f64| ((v * k / 2.0).round() * 2.0) as u32;
    (even(w).max(2), even(h).max(2))
}

/// Fréquence demandée : l'écran principal garde la sienne, les autres sont
/// à 60 Hz au plus.
pub fn refresh(refresh: u32, primary: bool) -> u32 {
    let r = refresh.max(1);
    if primary { r } else { r.min(OTHER_REFRESH_MAX) }
}

/// Index de l'écran principal dans la liste (le premier marqué, sinon le premier).
pub fn primary_index(list: &[DeviceScreen]) -> usize {
    list.iter().position(|s| s.primary).unwrap_or(0)
}

/// Les écrans virtuels à demander pour ces écrans de l'appareil : principal
/// en premier, puis les autres dans l'ordre de l'appareil ; quatre au plus
/// (les plus proches du principal, de proche en proche).
pub fn plan(screens: &Screens, codec: Codec) -> Vec<PlannedScreen> {
    let list = &screens.list;
    if list.is_empty() {
        return Vec::new();
    }
    let primary = primary_index(list);
    let pixels: Vec<(u32, u32)> = list.iter().map(|s| fit(s.pixel_width, s.pixel_height, screens.limit_4k, codec)).collect();
    let inputs: Vec<LayoutScreen> = list
        .iter()
        .zip(&pixels)
        .map(|(s, &(w, h))| LayoutScreen { frame: Rect { x: s.x, y: s.y, width: s.width, height: s.height }, pixels: (w, h) })
        .collect();
    let placed = layout_limited(&inputs, primary, MAX_SCREENS);
    let mut out: Vec<PlannedScreen> = placed
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let (x, y) = (*p)?;
            let is_primary = i == primary;
            Some(PlannedScreen {
                device: i,
                mode: Mode { width: pixels[i].0, height: pixels[i].1, refresh: refresh(list[i].refresh, is_primary) },
                x,
                y,
                primary: is_primary,
            })
        })
        .collect();
    out.sort_by_key(|p| (!p.primary, p.device));
    out
}

/// Un rectangle dans le repère logique de l'appareil (y vers le bas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn right(&self) -> f64 {
        self.x + self.width
    }
    fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// Entrée de [`layout`] : le cadre logique d'un écran de l'appareil et la
/// définition de son écran virtuel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutScreen {
    pub frame: Rect,
    pub pixels: (u32, u32),
}

/// Position en pixels de chaque écran sur le bureau de Windows, dans l'ordre
/// donné ; `primary` en (0, 0). Voir les règles en tête du module.
pub fn layout(screens: &[LayoutScreen], primary: usize) -> Vec<(i32, i32)> {
    layout_limited(screens, primary, usize::MAX).into_iter().map(|p| p.unwrap_or((0, 0))).collect()
}

/// Côté d'un voisin auquel un écran se colle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Right,
    Left,
    Below,
    Above,
}

impl Side {
    fn horizontal(self) -> bool {
        matches!(self, Side::Right | Side::Left)
    }
}

/// Rectangle en pixels du bureau de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixRect {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

impl PixRect {
    fn overlaps(&self, o: &PixRect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// Longueur commune de deux segments `[a, a + la)` et `[b, b + lb)`.
fn common(a: f64, la: f64, b: f64, lb: f64) -> f64 {
    (a + la).min(b + lb) - a.max(b)
}

/// Côté de `n` auquel `s` touche sur l'appareil (bord commun de longueur non nulle).
fn touching(s: &Rect, n: &Rect) -> Option<Side> {
    let t = EDGE_TOLERANCE;
    let vertical = common(s.y, s.height, n.y, n.height) > t;
    let horizontal = common(s.x, s.width, n.x, n.width) > t;
    if vertical && (s.x - n.right()).abs() <= t {
        Some(Side::Right)
    } else if vertical && (s.right() - n.x).abs() <= t {
        Some(Side::Left)
    } else if horizontal && (s.y - n.bottom()).abs() <= t {
        Some(Side::Below)
    } else if horizontal && (s.bottom() - n.y).abs() <= t {
        Some(Side::Above)
    } else {
        None
    }
}

/// Écarts horizontal et vertical entre deux cadres (négatifs s'ils se recouvrent sur cet axe).
fn gaps(s: &Rect, n: &Rect) -> (f64, f64) {
    let dx = (s.x - n.right()).max(n.x - s.right());
    let dy = (s.y - n.bottom()).max(n.y - s.bottom());
    (dx, dy)
}

/// Côté d'un écran isolé : celui de l'écart le plus grand.
fn isolated_side(s: &Rect, n: &Rect) -> Side {
    let (dx, dy) = gaps(s, n);
    let (sc, nc) = ((s.x + s.width / 2.0, s.y + s.height / 2.0), (n.x + n.width / 2.0, n.y + n.height / 2.0));
    if dx >= dy {
        if sc.0 >= nc.0 { Side::Right } else { Side::Left }
    } else if sc.1 >= nc.1 {
        Side::Below
    } else {
        Side::Above
    }
}

/// Où placer `s` collé à `n` (déjà placé en `np`) du côté `side` : la
/// position, et de combien elle a glissé pour ne rien chevaucher. `None` si
/// rien ne convient le long de ce bord.
fn place_against(s: &LayoutScreen, n: &LayoutScreen, np: PixRect, side: Side, placed: &[PixRect]) -> Option<(PixRect, i64)> {
    let (sw, sh) = (i64::from(s.pixels.0), i64::from(s.pixels.1));
    // Décalage le long du bord, à l'échelle pixels / unités logiques du voisin.
    let ideal = if side.horizontal() {
        let k = if n.frame.height > 0.0 { np.h as f64 / n.frame.height } else { 1.0 };
        ((s.frame.y - n.frame.y) * k).round() as i64
    } else {
        let k = if n.frame.width > 0.0 { np.w as f64 / n.frame.width } else { 1.0 };
        ((s.frame.x - n.frame.x) * k).round() as i64
    };
    // Le long du bord : longueur de l'écran, du voisin, et segment commun voulu.
    let (s_len, n_len) = if side.horizontal() { (sh, np.h) } else { (sw, np.w) };
    let m = MIN_SHARED.min(s_len).min(n_len);
    let (lo, hi) = (m - s_len, n_len - m);
    let rect_at = |off: i64| match side {
        Side::Right => PixRect { x: np.x + np.w, y: np.y + off, w: sw, h: sh },
        Side::Left => PixRect { x: np.x - sw, y: np.y + off, w: sw, h: sh },
        Side::Below => PixRect { x: np.x + off, y: np.y + np.h, w: sw, h: sh },
        Side::Above => PixRect { x: np.x + off, y: np.y - sh, w: sw, h: sh },
    };
    let start = ideal.clamp(lo, hi);
    // Positions où l'écran se pose juste à côté de chaque écran placé, le long du bord.
    let base = if side.horizontal() { np.y } else { np.x };
    let mut candidates = vec![start, lo, hi];
    for r in placed {
        let (r0, rl) = if side.horizontal() { (r.y, r.h) } else { (r.x, r.w) };
        candidates.push(r0 + rl - base);
        candidates.push(r0 - s_len - base);
    }
    candidates
        .into_iter()
        .filter(|&c| (lo..=hi).contains(&c))
        .map(|c| (rect_at(c), (c - start).abs() + (start - ideal).abs()))
        .filter(|(r, _)| !placed.iter().any(|p| p.overlaps(r)))
        .min_by_key(|&(r, slide)| (slide, if side.horizontal() { r.y } else { r.x }))
}

/// [`layout`] avec un nombre d'écrans au plus : `None` pour ceux qui ne sont
/// pas pris (les plus loin du principal, de proche en proche).
fn layout_limited(screens: &[LayoutScreen], primary: usize, max: usize) -> Vec<Option<(i32, i32)>> {
    let n = screens.len();
    let mut out: Vec<Option<PixRect>> = vec![None; n];
    if n == 0 || max == 0 {
        return Vec::new();
    }
    let primary = primary.min(n - 1);
    let p = screens[primary];
    out[primary] = Some(PixRect { x: 0, y: 0, w: i64::from(p.pixels.0), h: i64::from(p.pixels.1) });
    let mut order = vec![primary];
    while order.len() < max.min(n) {
        let placed: Vec<PixRect> = order.iter().filter_map(|&i| out[i]).collect();
        // Le prochain : un écran qui touche un écran placé (le premier dans
        // l'ordre de l'appareil), sinon le plus proche d'un écran placé.
        let touching_next = (0..n).filter(|i| out[*i].is_none()).find(|&i| order.iter().any(|&j| touching(&screens[i].frame, &screens[j].frame).is_some()));
        let (next, choices): (usize, Vec<(usize, Side)>) = match touching_next {
            Some(i) => {
                let c = order.iter().filter_map(|&j| touching(&screens[i].frame, &screens[j].frame).map(|side| (j, side))).collect();
                (i, c)
            }
            None => {
                let distance = |i: usize, j: usize| {
                    let (dx, dy) = gaps(&screens[i].frame, &screens[j].frame);
                    dx.max(0.0).hypot(dy.max(0.0))
                };
                let Some((i, j)) = (0..n)
                    .filter(|i| out[*i].is_none())
                    .flat_map(|i| order.iter().map(move |&j| (i, j)))
                    .min_by(|a, b| distance(a.0, a.1).total_cmp(&distance(b.0, b.1)))
                else {
                    break;
                };
                (i, vec![(j, isolated_side(&screens[i].frame, &screens[j].frame))])
            }
        };
        let s = &screens[next];
        let best = choices
            .iter()
            .filter_map(|&(j, side)| place_against(s, &screens[j], out[j].expect("placé"), side, &placed))
            .min_by_key(|&(_, slide)| slide)
            .map(|(r, _)| r);
        // Rien ne convient le long des bords voulus : à droite de l'écran qui
        // s'étend le plus à droite, aligné en haut (rien ne peut y chevaucher).
        let rect = best.unwrap_or_else(|| {
            let far = placed.iter().max_by_key(|r| (r.x + r.w, -r.y)).copied().expect("principal placé");
            PixRect { x: far.x + far.w, y: far.y, w: i64::from(s.pixels.0), h: i64::from(s.pixels.1) }
        });
        out[next] = Some(rect);
        order.push(next);
    }
    out.into_iter().map(|r| r.map(|r| (r.x as i32, r.y as i32))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn screen(x: f64, y: f64, w: f64, h: f64, pw: u32, ph: u32, hz: u32, primary: bool) -> DeviceScreen {
        DeviceScreen { x, y, width: w, height: h, pixel_width: pw, pixel_height: ph, refresh: hz, primary }
    }

    /// Le Mac de l'auteur : XG32UQ 4K à 160 Hz au centre (principal, réglé
    /// « comme 2560 × 1440 »), Studio Display 5K à droite, MacBook Pro 16"
    /// à gauche, plus bas.
    fn auteur() -> Screens {
        Screens {
            list: vec![
                screen(-1728.0, 400.0, 1728.0, 1117.0, 3456, 2234, 120, false),
                screen(0.0, 0.0, 2560.0, 1440.0, 3840, 2160, 160, true),
                screen(2560.0, 0.0, 2560.0, 1440.0, 5120, 2880, 60, false),
            ],
            limit_4k: false,
        }
    }

    /// Rectangles en pixels d'un plan.
    fn rects(plan: &[PlannedScreen]) -> Vec<PixRect> {
        plan.iter().map(|p| PixRect { x: p.x.into(), y: p.y.into(), w: p.mode.width.into(), h: p.mode.height.into() }).collect()
    }

    /// Ce que Windows exige : le principal en (0, 0), aucun chevauchement,
    /// tous reliés par des bords partagés.
    fn windows_accepts(r: &[PixRect], primary: usize) {
        assert_eq!((r[primary].x, r[primary].y), (0, 0), "principal en (0, 0)");
        for (i, a) in r.iter().enumerate() {
            for b in &r[i + 1..] {
                assert!(!a.overlaps(b), "chevauchement : {a:?} {b:?}");
            }
        }
        let shares = |a: &PixRect, b: &PixRect| {
            let v = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            let h = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            (v > 0 && (a.x + a.w == b.x || b.x + b.w == a.x)) || (h > 0 && (a.y + a.h == b.y || b.y + b.h == a.y))
        };
        let mut reached = vec![false; r.len()];
        reached[primary] = true;
        loop {
            let before = reached.iter().filter(|x| **x).count();
            for i in 0..r.len() {
                if !reached[i] && (0..r.len()).any(|j| reached[j] && shares(&r[i], &r[j])) {
                    reached[i] = true;
                }
            }
            if reached.iter().filter(|x| **x).count() == before {
                break;
            }
        }
        assert!(reached.iter().all(|x| *x), "écrans non reliés : {r:?}");
    }

    #[test]
    fn disposition_de_l_auteur() {
        let p = plan(&auteur(), Codec::Hevc);
        assert_eq!(p.len(), 3);
        // Principal en premier, à sa fréquence ; les autres à 60 Hz au plus.
        assert_eq!(p[0], PlannedScreen { device: 1, mode: Mode { width: 3840, height: 2160, refresh: 160 }, x: 0, y: 0, primary: true });
        assert_eq!(p[1], PlannedScreen { device: 0, mode: Mode { width: 3456, height: 2234, refresh: 60 }, x: -3456, y: 600, primary: false });
        assert_eq!(p[2], PlannedScreen { device: 2, mode: Mode { width: 5120, height: 2880, refresh: 60 }, x: 3840, y: 0, primary: false });
        windows_accepts(&rects(&p), 0);
        // Ce qui part à la Station.
        let r = p[1].request();
        assert_eq!((r.width, r.height, r.refresh, r.x, r.y, r.primary), (3456, 2234, 60, -3456, 600, false));
    }

    #[test]
    fn au_plus_4k_et_h264() {
        assert_eq!(fit(5120, 2880, true, Codec::Hevc), (3840, 2160));
        assert_eq!(fit(3456, 2234, true, Codec::Hevc), (3342, 2160));
        assert_eq!(fit(3840, 2160, true, Codec::Hevc), (3840, 2160));
        assert_eq!(fit(1920, 1080, true, Codec::Hevc), (1920, 1080), "jamais agrandi");
        assert_eq!(fit(5120, 2880, false, Codec::H264), (4096, 2304), "H.264 : 4096 de large au plus");
        assert_eq!(fit(6016, 3384, false, Codec::Hevc), (6016, 3384));
        assert_eq!(fit(8192, 4320, false, Codec::Av1), (7680, 4050));
        let mut s = auteur();
        s.limit_4k = true;
        let p = plan(&s, Codec::Hevc);
        let modes: Vec<(usize, u32, u32)> = p.iter().map(|p| (p.device, p.mode.width, p.mode.height)).collect();
        assert_eq!(modes, [(1, 3840, 2160), (0, 3342, 2160), (2, 3840, 2160)]);
        windows_accepts(&rects(&p), 0);
        // L'écran du MacBook garde son décalage, à l'échelle du principal.
        assert_eq!((p[1].x, p[1].y), (-3342, 600));
        let p = plan(&auteur(), Codec::H264);
        assert_eq!((p[2].mode.width, p[2].mode.height), (4096, 2304));
        windows_accepts(&rects(&p), 0);
    }

    #[test]
    fn frequences() {
        assert_eq!(refresh(160, true), 160);
        assert_eq!(refresh(120, false), 60);
        assert_eq!(refresh(50, false), 50, "moins si l'écran ne va pas jusque-là");
        assert_eq!(refresh(0, false), 1);
    }

    #[test]
    fn ecrans_empiles() {
        // Un écran 1080p principal, un 4K au-dessus (Retina, échelle 2), un autre en dessous.
        let s = Screens {
            list: vec![
                screen(0.0, 0.0, 1920.0, 1080.0, 1920, 1080, 144, true),
                screen(0.0, -1080.0, 1920.0, 1080.0, 3840, 2160, 60, false),
                screen(320.0, 1080.0, 1280.0, 800.0, 2560, 1600, 60, false),
            ],
            limit_4k: false,
        };
        let p = plan(&s, Codec::Hevc);
        let at: Vec<(usize, i32, i32)> = p.iter().map(|p| (p.device, p.x, p.y)).collect();
        assert_eq!(at, [(0, 0, 0), (1, 0, -2160), (2, 320, 1080)]);
        windows_accepts(&rects(&p), 0);
    }

    #[test]
    fn echelles_melangees() {
        // MacBook Air (échelle 2) principal, écran 1080p (échelle 1) à droite, monté de 200 points.
        let s = Screens {
            list: vec![
                screen(0.0, 0.0, 1440.0, 900.0, 2880, 1800, 60, true),
                screen(1440.0, -200.0, 1920.0, 1080.0, 1920, 1080, 75, false),
            ],
            limit_4k: false,
        };
        let p = plan(&s, Codec::Hevc);
        // 200 points du principal = 400 pixels.
        assert_eq!((p[1].x, p[1].y, p[1].mode.refresh), (2880, -400, 60));
        windows_accepts(&rects(&p), 0);
    }

    #[test]
    fn ecran_isole() {
        // Un écran posé à l'écart (rien ne touche) : collé au plus proche, du côté de l'écart.
        let s = Screens {
            list: vec![
                screen(0.0, 0.0, 1000.0, 1000.0, 1000, 1000, 60, true),
                screen(1500.0, 200.0, 500.0, 500.0, 500, 500, 60, false),
                screen(-300.0, 1600.0, 800.0, 400.0, 800, 400, 60, false),
            ],
            limit_4k: false,
        };
        let p = plan(&s, Codec::Hevc);
        let at: Vec<(usize, i32, i32)> = p.iter().map(|p| (p.device, p.x, p.y)).collect();
        // Écart horizontal de 500 : à droite, même hauteur relative ; écart vertical de 600 : dessous.
        assert_eq!(at, [(0, 0, 0), (1, 1000, 200), (2, -300, 1000)]);
        windows_accepts(&rects(&p), 0);
    }

    #[test]
    fn glisse_au_lieu_de_chevaucher() {
        // Le principal, un écran à sa droite (échelle 2, donc deux fois plus
        // grand en pixels), et une barre sous les deux : sous le principal
        // elle chevaucherait l'écran de droite ; sous celui de droite, non.
        let s = Screens {
            list: vec![
                screen(0.0, 0.0, 1000.0, 1000.0, 1000, 1000, 60, true),
                screen(1000.0, 0.0, 1000.0, 1000.0, 2000, 2000, 60, false),
                screen(0.0, 1000.0, 2000.0, 500.0, 4000, 1000, 60, false),
            ],
            limit_4k: false,
        };
        let p = plan(&s, Codec::Hevc);
        let at: Vec<(usize, i32, i32)> = p.iter().map(|p| (p.device, p.x, p.y)).collect();
        assert_eq!(at, [(0, 0, 0), (1, 1000, 0), (2, -1000, 2000)]);
        windows_accepts(&rects(&p), 0);
    }

    #[test]
    fn quatre_au_plus_et_principal_par_defaut() {
        let list: Vec<DeviceScreen> = (0..6).map(|i| screen(f64::from(i) * 100.0, 0.0, 100.0, 100.0, 100, 100, 60, false)).collect();
        let p = plan(&Screens { list, limit_4k: false }, Codec::Hevc);
        assert_eq!(p.len(), MAX_SCREENS);
        // Aucun principal marqué : le premier ; les plus proches de lui ensuite.
        assert!(p[0].primary && p[0].device == 0);
        assert_eq!(p.iter().map(|p| p.device).collect::<Vec<_>>(), [0, 1, 2, 3]);
        assert!(plan(&Screens::default(), Codec::Hevc).is_empty());
        let one = plan(&Screens { list: vec![screen(0.0, 0.0, 1512.0, 982.0, 3024, 1964, 120, true)], limit_4k: false }, Codec::Hevc);
        assert_eq!(one, [PlannedScreen { device: 0, mode: Mode { width: 3024, height: 1964, refresh: 120 }, x: 0, y: 0, primary: true }]);
    }

    #[test]
    fn dispositions_variees_toujours_acceptees() {
        // Grilles d'écrans de tailles et d'échelles diverses (générateur fixe) :
        // Windows accepte toujours le résultat.
        let mut seed: u64 = 0x5eed;
        let mut next = |m: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % m
        };
        for _ in 0..300 {
            let count = 2 + next(3) as usize;
            let mut list = Vec::new();
            let mut frames: Vec<Rect> = vec![Rect { x: 0.0, y: 0.0, width: 1000.0, height: 600.0 }];
            for i in 0..count {
                let f = if i == 0 {
                    frames[0]
                } else {
                    // Collé à un écran existant, d'un côté au hasard, décalé au hasard.
                    let base = frames[next(frames.len() as u64) as usize];
                    let w = 400.0 + next(1200) as f64;
                    let h = 300.0 + next(900) as f64;
                    let off = next(800) as f64 - 400.0;
                    match next(5) {
                        0 => Rect { x: base.right(), y: base.y + off, width: w, height: h },
                        1 => Rect { x: base.x - w, y: base.y + off, width: w, height: h },
                        2 => Rect { x: base.x + off, y: base.bottom(), width: w, height: h },
                        3 => Rect { x: base.x + off, y: base.y - h, width: w, height: h },
                        _ => Rect { x: base.right() + 300.0, y: base.y + off, width: w, height: h },
                    }
                };
                if i > 0 && frames.iter().any(|g| common(f.x, f.width, g.x, g.width) > 0.0 && common(f.y, f.height, g.y, g.height) > 0.0) {
                    continue;
                }
                if i > 0 {
                    frames.push(f);
                }
                let scale = [1.0, 1.5, 2.0][next(3) as usize];
                list.push(screen(f.x, f.y, f.width, f.height, (f.width * scale) as u32, (f.height * scale) as u32, 60, i == 0));
            }
            let p = plan(&Screens { list, limit_4k: next(2) == 0 }, Codec::Hevc);
            windows_accepts(&rects(&p), 0);
        }
    }
}
