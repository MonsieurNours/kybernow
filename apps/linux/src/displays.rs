//! Les écrans de l'appareil (multi-écrans, docs/multi-ecrans.md) tels que SDL
//! les donne : cadre dans le repère du bureau, définition native, fréquence.
//! Les écrans en recopie (même cadre) comptent pour un ; l'écran principal est
//! l'écran 0 de SDL. Sans SDL ni cœur : la couche appareil lit SDL
//! (`device/screens.rs`) et en tire ces valeurs, les captures et les tests les
//! posent à la main.

/// Un écran de l'appareil, ou un groupe d'écrans en recopie.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    /// Indice SDL de l'écran (le plus petit du groupe, s'il y a recopie).
    pub sdl: i32,
    pub name: String,
    /// Cadre dans le repère logique du bureau (`SDL_GetDisplayBounds`), y vers le bas.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Définition native, en pixels (`SDL_GetCurrentDisplayMode`).
    pub pixel_w: u32,
    pub pixel_h: u32,
    /// Fréquence, en Hz.
    pub refresh: u32,
}

impl Display {
    pub fn frame(&self) -> (i32, i32, i32, i32) {
        (self.x, self.y, self.w, self.h)
    }

    /// Pixels par unité logique (l'échelle de l'écran).
    pub fn scale(&self) -> f32 {
        self.pixel_w as f32 / self.w.max(1) as f32
    }
}

/// Regroupe les écrans en recopie : ceux qui ont exactement le même cadre ne
/// font qu'un écran (le plus petit indice SDL, au rang du premier du groupe).
/// L'ordre de SDL est gardé : le premier est l'écran principal.
pub fn group(list: Vec<Display>) -> Vec<Display> {
    let mut out: Vec<Display> = Vec::new();
    for d in list {
        match out.iter_mut().find(|o| o.frame() == d.frame()) {
            Some(o) => {
                // En recopie, les deux affichent la même chose : on garde l'écran de plus petit indice.
                if d.sdl < o.sdl {
                    *o = d;
                }
            }
            None => out.push(d),
        }
    }
    out
}

/// L'indice dans la liste regroupée de l'écran SDL `sdl` (ou d'un écran qui
/// lui est en recopie) ; à défaut, l'écran principal.
pub fn index_of(grouped: &[Display], all: &[Display], sdl: i32) -> usize {
    let frame = all.iter().find(|d| d.sdl == sdl).map(Display::frame);
    frame.and_then(|f| grouped.iter().position(|g| g.frame() == f)).unwrap_or(0)
}

/// Les deux listes décrivent la même disposition (le nom ne compte pas).
pub fn same_layout(a: &[Display], b: &[Display]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.frame() == y.frame() && x.pixel_w == y.pixel_w && x.pixel_h == y.pixel_h && x.refresh == y.refresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(sdl: i32, x: i32, y: i32, w: i32, h: i32) -> Display {
        Display { sdl, name: format!("écran {sdl}"), x, y, w, h, pixel_w: w as u32, pixel_h: h as u32, refresh: 60 }
    }

    #[test]
    fn mirrored_displays_count_once() {
        let all = vec![d(0, 0, 0, 3840, 2160), d(1, 3840, 0, 2560, 1440), d(2, 3840, 0, 2560, 1440), d(3, -1920, 300, 1920, 1080)];
        let g = group(all.clone());
        assert_eq!(g.len(), 3);
        assert_eq!(g.iter().map(|x| x.sdl).collect::<Vec<_>>(), vec![0, 1, 3]);
        // L'écran SDL 2, en recopie de 1, se retrouve sur le même écran.
        assert_eq!(index_of(&g, &all, 2), 1);
        assert_eq!(index_of(&g, &all, 3), 2);
        assert_eq!(index_of(&g, &all, 9), 0, "un écran inconnu : le principal");
    }

    #[test]
    fn primary_stays_first() {
        // Le plus petit indice du groupe est gardé, mais l'ordre de SDL aussi : l'écran 0 reste le principal.
        let g = group(vec![d(0, 0, 0, 1920, 1080), d(1, 0, 0, 1920, 1080)]);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].sdl, 0);
    }

    #[test]
    fn same_layout_ignores_names() {
        let a = vec![d(0, 0, 0, 1920, 1080), d(1, 1920, 0, 1920, 1080)];
        let mut b = a.clone();
        b[1].name = "autre nom".into();
        assert!(same_layout(&a, &b));
        b[1].x = 2000;
        assert!(!same_layout(&a, &b));
        b = a.clone();
        b[0].refresh = 144;
        assert!(!same_layout(&a, &b));
        assert!(!same_layout(&a, &a[..1]));
    }

    #[test]
    fn scale_is_pixels_per_unit() {
        let mut x = d(0, 0, 0, 1440, 900);
        x.pixel_w = 2880;
        x.pixel_h = 1800;
        assert!((x.scale() - 2.0).abs() < 1e-6);
    }
}
