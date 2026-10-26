//! Multi-écrans côté appareil (docs/multi-ecrans.md) : les écrans que SDL voit
//! (`SDL_GetNumVideoDisplays`, `SDL_GetDisplayBounds`, le mode courant), au
//! format du cœur (`kybernow_core::screens`), et la décision du cœur pour un
//! ordinateur : les écrans que la session prendra. L'accueil, la ligne
//! « Écrans » des réglages et la session passent par le même calcul.

use kybernow_core::console::Mode;
use kybernow_core::profile::{Codec, Profile as CoreProfile};
use kybernow_core::screens::{self, DeviceScreen, DisplaysSetting, PlannedScreen, Screens};

use super::sdl;
use crate::app::ScreensPlan;
use crate::displays::{self, Display};
use crate::model::Host;

/// Les écrans de l'appareil, tels que SDL les donne et regroupés (recopies).
#[derive(Clone, Debug, Default)]
pub struct Detected {
    /// Tous les écrans de SDL, dans son ordre (l'écran 0 est le principal).
    pub all: Vec<Display>,
    /// Les recopies comptent pour un ; le premier est le principal.
    pub grouped: Vec<Display>,
}

/// Lit les écrans de SDL : cadre, définition native, fréquence.
pub fn detect() -> Detected {
    let n = unsafe { sdl::SDL_GetNumVideoDisplays() }.max(0);
    let mut all = Vec::new();
    for i in 0..n {
        let mut r = sdl::SDL_Rect::default();
        if unsafe { sdl::SDL_GetDisplayBounds(i, &mut r) } != 0 || r.w <= 0 || r.h <= 0 {
            continue;
        }
        let mut m = sdl::SDL_DisplayMode::default();
        let known = unsafe { sdl::SDL_GetCurrentDisplayMode(i, &mut m) } == 0 && m.w > 0 && m.h > 0;
        let (pixel_w, pixel_h) = if known { (m.w as u32, m.h as u32) } else { (r.w as u32, r.h as u32) };
        let refresh = if known && m.refresh_rate > 0 { m.refresh_rate as u32 } else { 60 };
        let name = unsafe {
            let p = sdl::SDL_GetDisplayName(i);
            if p.is_null() { String::new() } else { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() }
        };
        all.push(Display { sdl: i, name, x: r.x, y: r.y, w: r.w, h: r.h, pixel_w, pixel_h, refresh });
    }
    let grouped = displays::group(all.clone());
    Detected { all, grouped }
}

impl Detected {
    /// « Cet écran » : celui de la fenêtre (index dans la liste regroupée).
    pub fn current(&self, window: *mut sdl::SDL_Window) -> usize {
        let sdl_index = unsafe { sdl::SDL_GetWindowDisplayIndex(window) };
        displays::index_of(&self.grouped, &self.all, sdl_index.max(0))
    }

    /// L'indice SDL de l'écran `screen` de la liste regroupée (où poser sa fenêtre).
    pub fn sdl_index(&self, screen: usize) -> i32 {
        self.grouped.get(screen).map_or(0, |d| d.sdl)
    }
}

/// Le profil tel que le cœur le voit pour décider du nombre d'écrans :
/// `None` en Personnalisé (Auto prend alors un seul écran).
pub fn core_profile_of(h: &Host) -> Option<CoreProfile> {
    (!h.settings.custom()).then(|| super::session::core_profile(h.settings.profile))
}

pub fn core_codec(codec: &str) -> Codec {
    match codec {
        "h264" => Codec::H264,
        "av1" => Codec::Av1,
        _ => Codec::Hevc,
    }
}

/// La fréquence demandée pour l'écran principal : celle qu'on a réglée
/// (« Fréquence », moins de 60 sur batterie quand on allège), sinon la sienne.
pub fn primary_refresh(h: &Host, screen_refresh: u32, on_battery: bool) -> u32 {
    let mut fps = if h.settings.fps == 0 { screen_refresh.max(30) } else { h.settings.fps };
    if h.settings.battery_light && on_battery {
        fps = fps.min(60);
    }
    fps
}

/// Les écrans de l'appareil et ce qui décide de leur nombre, au format du
/// cœur. `current` : l'écran de la fenêtre ; `max_screens` : ce que la Station
/// sait créer (1 : une Station antérieure) ; une définition choisie à la main
/// (Réglages › Image › Définition) est une « définition sur mesure » : la
/// session ne prend qu'un écran.
pub fn core_screens(d: &Detected, current: usize, h: &Host, max_screens: usize, on_battery: bool) -> Screens {
    let list = d
        .grouped
        .iter()
        .enumerate()
        .map(|(i, s)| DeviceScreen {
            x: f64::from(s.x),
            y: f64::from(s.y),
            width: f64::from(s.w),
            height: f64::from(s.h),
            pixel_width: s.pixel_w,
            pixel_height: s.pixel_h,
            refresh: if i == 0 { primary_refresh(h, s.refresh, on_battery) } else { s.refresh },
            primary: i == 0,
        })
        .collect();
    Screens {
        list,
        current,
        limit_4k: false,
        custom_resolution: h.settings.resolution != 0,
        setting: DisplaysSetting::parse(&h.settings.displays).unwrap_or_default(),
        max_screens,
    }
}

/// Les écrans virtuels que la session demandera (au moins deux), ou `None` :
/// une session à un écran.
pub fn session_plan(screens: &Screens, h: &Host, virtual_display: bool, codec: &str) -> Option<Vec<PlannedScreen>> {
    screens::session_plan(screens, core_profile_of(h), virtual_display, core_codec(codec))
}

/// Ce que l'accueil et les réglages disent de cette décision.
pub fn summary(screens: &Screens, h: &Host, virtual_display: bool, codec: &str) -> ScreensPlan {
    let choice = screens.choice(core_profile_of(h), virtual_display);
    ScreensPlan {
        count: session_plan(screens, h, virtual_display, codec).map_or(1, |p| p.len()),
        enabled: choice.setting_enabled(),
        shown: match choice.shown_setting() {
            DisplaysSetting::Auto => 0,
            DisplaysSetting::This => 1,
            DisplaysSetting::All => 2,
        },
    }
}

/// Le mode de l'écran principal du plan (celui que la session affiche).
pub fn primary_mode(plan: &[PlannedScreen]) -> Option<Mode> {
    plan.first().map(|p| p.mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HostSettings, Profile};

    fn d(sdl: i32, x: i32, y: i32, w: i32, h: i32, hz: u32) -> Display {
        Display { sdl, name: format!("écran {sdl}"), x, y, w, h, pixel_w: w as u32, pixel_h: h as u32, refresh: hz }
    }

    fn host(profile: Profile) -> Host {
        Host {
            id: "a".into(),
            name: "Atelier".into(),
            address: "192.168.1.20".into(),
            port: 24811,
            fingerprint: None,
            user: "u".into(),
            password: String::new(),
            os: None,
            gpu: None,
            av1: false,
            mac: None,
            wake: Vec::new(),
            settings: HostSettings { profile, ..HostSettings::default() },
            last: None,
        }
    }

    /// L'ordinateur d'essai de l'auteur, comme le contrat : XG32UQ 4K au centre
    /// (principal), Studio Display 5K à droite, écran du MacBook Pro à gauche et plus bas.
    fn three() -> Detected {
        let all = vec![d(0, 0, 0, 3840, 2160, 160), d(1, 3840, 0, 5120, 2880, 60), d(2, -1920, 400, 1920, 1080, 120)];
        Detected { grouped: displays::group(all.clone()), all }
    }

    #[test]
    fn auto_follows_the_profile() {
        let t = three();
        // Bureau et Max : tous les écrans ; Jeu : un seul.
        for (profile, n) in [(Profile::Bureau, 3), (Profile::Max, 3), (Profile::Jeu, 1)] {
            let h = host(profile);
            let s = core_screens(&t, 0, &h, 4, false);
            assert_eq!(summary(&s, &h, true, "hevc").count, n, "{profile:?}");
        }
    }

    #[test]
    fn custom_profile_takes_one_display_in_auto() {
        let t = three();
        let mut h = host(Profile::Bureau);
        h.settings.mbps = 80;
        assert!(h.settings.custom());
        let s = core_screens(&t, 0, &h, 4, false);
        assert_eq!(summary(&s, &h, true, "hevc").count, 1);
        // Tous les écrans : la session les prend quand même.
        h.settings.displays = "all".into();
        let s = core_screens(&t, 0, &h, 4, false);
        assert_eq!(summary(&s, &h, true, "hevc").count, 3);
    }

    #[test]
    fn setting_is_inactive_with_a_custom_resolution_or_without_virtual_display() {
        let t = three();
        let mut h = host(Profile::Bureau);
        h.settings.displays = "all".into();
        let s = core_screens(&t, 0, &h, 4, false);
        let ok = summary(&s, &h, true, "hevc");
        assert_eq!((ok.enabled, ok.shown, ok.count), (true, 2, 3));
        // Sans écran virtuel : inactif, sur « Cet écran ».
        let off = summary(&s, &h, false, "hevc");
        assert_eq!((off.enabled, off.shown, off.count), (false, 1, 1));
        // Définition choisie à la main : une « définition sur mesure ».
        h.settings.resolution = 1080;
        let s = core_screens(&t, 0, &h, 4, false);
        let custom = summary(&s, &h, true, "hevc");
        assert_eq!((custom.enabled, custom.shown, custom.count), (false, 1, 1));
    }

    #[test]
    fn a_station_that_makes_one_display_gets_one() {
        let t = three();
        let h = host(Profile::Bureau);
        // Station antérieure : max_screens absent (1) ; la ligne de l'accueil ne promet qu'un écran.
        let s = core_screens(&t, 0, &h, 1, false);
        assert_eq!(summary(&s, &h, true, "hevc").count, 1);
        // Une Station qui en crée deux : les deux plus proches du principal.
        let s = core_screens(&t, 0, &h, 2, false);
        assert_eq!(summary(&s, &h, true, "hevc").count, 2);
    }

    #[test]
    fn plan_matches_the_contract_layout() {
        let t = three();
        let h = host(Profile::Bureau);
        let s = core_screens(&t, 0, &h, 4, false);
        let plan = session_plan(&s, &h, true, "hevc").unwrap();
        // Le principal en (0, 0), à sa fréquence ; les autres à 60 Hz au plus.
        assert_eq!((plan[0].device, plan[0].x, plan[0].y, plan[0].mode.refresh), (0, 0, 0, 160));
        assert_eq!(plan[1].mode.refresh, 60);
        assert_eq!(plan[2].mode.refresh, 60);
        assert_eq!(primary_mode(&plan).map(|m| (m.width, m.height)), Some((3840, 2160)));
        // Le Studio Display à droite du principal, l'écran du MacBook à gauche.
        let right = plan.iter().find(|p| p.device == 1).unwrap();
        let left = plan.iter().find(|p| p.device == 2).unwrap();
        assert_eq!((right.x, left.x < 0), (3840, true));
    }

    #[test]
    fn a_single_display_is_a_single_session() {
        let one = vec![d(0, 0, 0, 2560, 1440, 144)];
        let t = Detected { grouped: displays::group(one.clone()), all: one };
        let h = host(Profile::Bureau);
        let s = core_screens(&t, 0, &h, 4, false);
        assert_eq!(summary(&s, &h, true, "hevc").count, 1);
    }

    #[test]
    fn primary_refresh_follows_the_setting_and_the_battery() {
        let mut h = host(Profile::Jeu);
        assert_eq!(primary_refresh(&h, 144, false), 144);
        h.settings.fps = 120;
        assert_eq!(primary_refresh(&h, 60, false), 120);
        assert_eq!(primary_refresh(&h, 60, true), 60, "sur batterie : 60 au plus");
        h.settings.battery_light = false;
        assert_eq!(primary_refresh(&h, 60, true), 120);
    }
}
