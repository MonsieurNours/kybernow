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

/// L'écran `screen` des deux listes est au même endroit, avec la même taille : une fenêtre
/// posée dessus est toujours bien placée. Un indice qui a changé de sens (écran débranché
/// avant lui, disposition refaite) ou qui n'existe plus : non.
pub fn same_screen(old: &[Display], new: &[Display], screen: usize) -> bool {
    matches!((old.get(screen), new.get(screen)), (Some(a), Some(b)) if a.frame() == b.frame())
}

/// Le cadre de la fenêtre d'une session de plus (plusieurs ordinateurs,
/// docs/plusieurs-ordinateurs.md) : centrée sur l'écran de la fenêtre active, aux deux tiers
/// de cet écran, en 16:9 (la largeur ou la hauteur des deux tiers, selon ce qui tient).
/// Rend (x, y, largeur, hauteur), dans le repère logique du bureau.
pub fn windowed_frame(d: &Display) -> (i32, i32, i32, i32) {
    let (sw, sh) = (d.w.max(2) as f32, d.h.max(2) as f32);
    let w = ((sw * 2.0 / 3.0).min(sh * 2.0 / 3.0 * 16.0 / 9.0) as i32 & !1).max(2);
    let h = ((w as f32 * 9.0 / 16.0).round() as i32).max(1);
    (d.x + (d.w - w) / 2, d.y + (d.h - h) / 2, w, h)
}

/// La fréquence d'un écran que SDL a décrit (`SDL_GetCurrentDisplayMode`) : celle qu'il
/// annonce, 30 Hz au moins (certains pilotes disent 0 : comme avant le multi-écrans, qui
/// lisait `max(30)`), 60 Hz quand SDL ne donne aucun mode.
pub fn refresh_of(mode_known: bool, hz: i32) -> u32 {
    if mode_known { hz.max(30) as u32 } else { 60 }
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
    fn a_screen_that_moved_is_not_the_same() {
        let old = vec![d(0, 0, 0, 1920, 1080), d(1, 1920, 0, 1920, 1080), d(2, 3840, 0, 1920, 1080)];
        // L'écran du milieu est débranché : l'indice 1 désigne maintenant l'ancien troisième, l'indice 2 n'existe plus.
        let new = vec![d(0, 0, 0, 1920, 1080), d(1, 1920, 0, 1920, 1080)];
        let new_shifted = vec![d(0, 0, 0, 1920, 1080), d(1, 3840, 0, 1920, 1080)];
        assert!(same_screen(&old, &new, 0) && same_screen(&old, &new, 1));
        assert!(!same_screen(&old, &new, 2), "plus d'écran 2");
        assert!(same_screen(&old, &new_shifted, 0));
        assert!(!same_screen(&old, &new_shifted, 1), "l'indice 1 désigne un autre écran");
        // Une fréquence qui change ne déplace pas la fenêtre.
        let mut faster = old.clone();
        faster[1].refresh = 144;
        assert!(same_screen(&old, &faster, 1));
    }

    /// SDL qui annonce 0 Hz : 30, comme avant le multi-écrans (cette valeur donne la fréquence
    /// « Comme cet écran » et se relit toutes les deux secondes).
    #[test]
    fn refresh_of_zero_hz_is_thirty() {
        assert_eq!(refresh_of(true, 0), 30);
        assert_eq!(refresh_of(true, -1), 30);
        assert_eq!(refresh_of(true, 24), 30);
        assert_eq!(refresh_of(true, 60), 60);
        assert_eq!(refresh_of(true, 144), 144);
        assert_eq!(refresh_of(false, 0), 60, "aucun mode : 60 Hz");
    }

    /// La fenêtre d'une session de plus : deux tiers de l'écran, 16:9, centrée sur cet écran.
    #[test]
    fn windowed_frame_is_two_thirds_in_sixteen_by_nine() {
        // 16:9 : deux tiers de la largeur et de la hauteur.
        assert_eq!(windowed_frame(&d(0, 0, 0, 1920, 1080)), (320, 180, 1280, 720));
        // 16:10 : la largeur fait les deux tiers, la hauteur suit le 16:9.
        let (x, y, w, h) = windowed_frame(&d(0, 0, 0, 2560, 1600));
        assert_eq!((w, h), (1706, 960));
        assert_eq!((x, y), ((2560 - w) / 2, (1600 - h) / 2));
        // Un écran large : la hauteur fait les deux tiers.
        let (_, _, w, h) = windowed_frame(&d(0, 0, 0, 3440, 1440));
        assert_eq!((w, h), (1706, 960));
        // Centrée sur l'écran de la fenêtre active, où qu'il soit dans le bureau.
        let (x, y, w, h) = windowed_frame(&d(1, 3840, -200, 1920, 1080));
        assert_eq!((x, y, w, h), (3840 + 320, -200 + 180, 1280, 720));
        // Jamais plus grande que l'écran, même minuscule.
        let (_, _, w, h) = windowed_frame(&d(0, 0, 0, 1, 1));
        assert!(w >= 2 && h >= 1);
    }

    #[test]
    fn scale_is_pixels_per_unit() {
        let mut x = d(0, 0, 0, 1440, 900);
        x.pixel_w = 2880;
        x.pixel_h = 1800;
        assert!((x.scale() - 2.0).abs() < 1e-6);
    }
}
