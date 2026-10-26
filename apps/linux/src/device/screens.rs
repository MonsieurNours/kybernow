//! Multi-écrans côté appareil (docs/multi-ecrans.md) : les écrans que SDL voit
//! (`SDL_GetNumVideoDisplays`, `SDL_GetDisplayBounds`, le mode courant), au
//! format du cœur (`kybernow_core::screens`), et la décision du cœur pour un
//! ordinateur : les écrans que la session prendra. L'accueil, la ligne
//! « Écrans » des réglages et la session passent par le même calcul du cœur
//! (`screens::session_plan_for` : les réglages du flux, ce que décode cette
//! machine, ce que la Station sait créer) : l'app ne décide de rien.

use kybernow_core::console::Mode;
use kybernow_core::profile::{Capabilities, StreamSettings};
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
        let refresh = displays::refresh_of(known, m.refresh_rate);
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

/// Ce que la Station dit de ses écrans (`virtual_display` de `GET /api/stream/info`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StationScreens {
    /// Écrans qu'elle sait créer (1 : une Station antérieure, ou qui n'a pas encore répondu).
    pub max_screens: usize,
    /// Son écran virtuel est utilisable (pilote en place, déclenchement actif) ; inconnu : oui.
    pub virtual_display_available: bool,
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

/// Les écrans de l'appareil au format du cœur : cadre logique, définition
/// native, fréquence (celle du réglage pour l'écran principal), l'écran 0 de
/// SDL est le principal.
pub fn device_screens(d: &Detected, h: &Host, on_battery: bool) -> Vec<DeviceScreen> {
    d.grouped
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
        .collect()
}

/// Les écrans de l'appareil et ce qui décide de leur nombre, au format du
/// cœur. `current` : l'écran de la fenêtre ; `station` : ce que la Station
/// sait créer et son écran virtuel ; `capabilities` : ce que cette machine
/// décode (`session::capabilities`), pour la règle « Personnalisé » du cœur. Une
/// définition choisie à la main (Réglages › Image › Définition) est une
/// « définition sur mesure » : la session ne prend qu'un écran.
pub fn core_screens(d: &Detected, current: usize, h: &Host, station: StationScreens, capabilities: Capabilities, on_battery: bool) -> Screens {
    Screens {
        list: device_screens(d, h, on_battery),
        current,
        limit_4k: false,
        custom_resolution: h.settings.resolution != 0,
        setting: DisplaysSetting::parse(&h.settings.displays).unwrap_or_default(),
        max_screens: station.max_screens,
        virtual_display_available: station.virtual_display_available,
        capabilities: Some(capabilities),
    }
}

/// Les écrans virtuels que la session demandera (au moins deux), ou `None` :
/// une session à un écran. Le calcul du cœur, avec les réglages du flux que la
/// session envoie (ils disent si le profil est « Personnalisé ») et l'écran
/// virtuel demandé (`virtual_display` : le réglage de l'ordinateur).
pub fn session_plan(screens: &Screens, settings: &StreamSettings, virtual_display: bool) -> Option<Vec<PlannedScreen>> {
    screens::session_plan_for(screens, settings, virtual_display)
}

/// Ce que l'accueil et les réglages disent de cette décision.
pub fn summary(screens: &Screens, settings: &StreamSettings, virtual_display: bool) -> ScreensPlan {
    let choice = screens.choice(screens::screens_profile(settings, screens.capabilities), virtual_display);
    ScreensPlan {
        count: session_plan(screens, settings, virtual_display).map_or(1, |p| p.len()),
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
    use crate::app::Machine;
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

    const STATION: StationScreens = StationScreens { max_screens: 4, virtual_display_available: true };

    /// Ce que fait la session : les réglages du flux et ce que décode la machine, comme `Action::Connect`.
    fn settings_of(h: &Host) -> StreamSettings {
        super::super::session::settings(h, h.settings.profile, "hevc", (1920, 1080, 60), &Machine::default(), false)
    }

    fn screens_of(t: &Detected, h: &Host, station: StationScreens) -> Screens {
        core_screens(t, 0, h, station, super::super::session::capabilities(h, "hevc", &Machine::default()), false)
    }

    /// L'ordinateur d'essai de l'auteur, comme le contrat : XG32UQ 4K au centre
    /// (principal), Studio Display 5K à droite, écran du MacBook Pro à gauche et plus bas.
    fn three() -> Detected {
        let all = vec![d(0, 0, 0, 3840, 2160, 160), d(1, 3840, 0, 5120, 2880, 60), d(2, -1920, 400, 1920, 1080, 120)];
        Detected { grouped: displays::group(all.clone()), all }
    }

    fn count(t: &Detected, h: &Host, station: StationScreens, virtual_display: bool) -> usize {
        summary(&screens_of(t, h, station), &settings_of(h), virtual_display).count
    }

    #[test]
    fn auto_follows_the_profile() {
        let t = three();
        // Bureau et Max : tous les écrans ; Jeu : un seul.
        for (profile, n) in [(Profile::Bureau, 3), (Profile::Max, 3), (Profile::Jeu, 1)] {
            assert_eq!(count(&t, &host(profile), STATION, true), n, "{profile:?}");
        }
    }

    /// « Personnalisé » : la règle du cœur (`screens_profile`) sur les réglages du flux, avec ce que
    /// décode la machine ; le profil envoyé à la Station reste celui de base.
    #[test]
    fn custom_settings_take_one_display_in_auto() {
        let t = three();
        let m = Machine { hw_av1: true, ..Machine::default() };
        // Le nombre d'écrans pour ces réglages et ce format (celui que la session envoie).
        let count_of = |h: &Host, codec: &str| {
            let settings = super::super::session::settings(h, h.settings.profile, codec, (1920, 1080, 60), &m, false);
            let screens = core_screens(&t, 0, h, STATION, super::super::session::capabilities(h, codec, &m), false);
            summary(&screens, &settings, true).count
        };
        assert_eq!(count_of(&host(Profile::Bureau), "hevc"), 3, "rien de retouché");
        let retouched: [(&str, Profile, &str, fn(&mut HostSettings)); 5] = [
            ("débit choisi", Profile::Bureau, "hevc", |s| s.mbps = 80),
            ("format choisi : AV1", Profile::Bureau, "av1", |s| s.codec = "av1".into()),
            ("format choisi : H.264", Profile::Max, "h264", |s| s.codec = "h264".into()),
            ("transport fiable hors Bureau", Profile::Max, "hevc", |s| s.reliable = true),
            ("4:4:4 en Jeu (Tous les écrans)", Profile::Jeu, "hevc", |s| s.yuv444 = true),
        ];
        for (what, profile, codec, retouch) in retouched {
            let mut h = host(profile);
            retouch(&mut h.settings);
            if profile == Profile::Jeu {
                // Jeu : un seul écran en Auto de toute façon ; Tous les écrans les prend, avec ou sans écart.
                h.settings.displays = "all".into();
                assert_eq!(count_of(&h, codec), 3, "{what}");
                continue;
            }
            assert_eq!(count_of(&h, codec), 1, "{what} : « Personnalisé », un seul écran en Auto");
            // Tous les écrans : la session les prend quand même.
            h.settings.displays = "all".into();
            assert_eq!(count_of(&h, codec), 3, "{what} : Tous les écrans");
        }
        // Le format du profil, choisi à la main, n'est pas un écart pour le cœur.
        let mut h = host(Profile::Bureau);
        h.settings.codec = "hevc".into();
        assert_eq!(count_of(&h, "hevc"), 3);
        // Un profil retouché reste celui de base pour la Station : le débit suit la définition.
        h.settings.mbps = 80;
        assert_eq!(settings_of(&h).profile, Some(kybernow_core::profile::Profile::Bureau));
    }

    #[test]
    fn setting_is_inactive_with_a_custom_resolution_or_without_virtual_display() {
        let t = three();
        let mut h = host(Profile::Bureau);
        h.settings.displays = "all".into();
        let ok = summary(&screens_of(&t, &h, STATION), &settings_of(&h), true);
        assert_eq!((ok.enabled, ok.shown, ok.count), (true, 2, 3));
        // Sans écran virtuel demandé : inactif, sur « Cet écran ».
        let off = summary(&screens_of(&t, &h, STATION), &settings_of(&h), false);
        assert_eq!((off.enabled, off.shown, off.count), (false, 1, 1));
        // Demandé, mais la Station n'a pas d'écran virtuel utilisable (pilote absent, déclenchement coupé) : pareil.
        let unavailable = StationScreens { max_screens: 1, virtual_display_available: false };
        let off = summary(&screens_of(&t, &h, unavailable), &settings_of(&h), true);
        assert_eq!((off.enabled, off.shown, off.count), (false, 1, 1));
        let unavailable = StationScreens { max_screens: 4, virtual_display_available: false };
        let off = summary(&screens_of(&t, &h, unavailable), &settings_of(&h), true);
        assert_eq!((off.enabled, off.shown, off.count), (false, 1, 1), "même avec max_screens : sans écran virtuel utilisable, un seul écran");
        // Définition choisie à la main : une « définition sur mesure ».
        h.settings.resolution = 1080;
        let custom = summary(&screens_of(&t, &h, STATION), &settings_of(&h), true);
        assert_eq!((custom.enabled, custom.shown, custom.count), (false, 1, 1));
    }

    #[test]
    fn a_station_that_makes_one_display_gets_one() {
        let t = three();
        let h = host(Profile::Bureau);
        // Station antérieure : max_screens absent (1) ; la ligne de l'accueil ne promet qu'un écran.
        assert_eq!(count(&t, &h, StationScreens { max_screens: 1, virtual_display_available: true }, true), 1);
        // Une Station qui en crée deux : les deux plus proches du principal.
        assert_eq!(count(&t, &h, StationScreens { max_screens: 2, virtual_display_available: true }, true), 2);
    }

    #[test]
    fn plan_matches_the_contract_layout() {
        let t = three();
        let h = host(Profile::Bureau);
        let plan = session_plan(&screens_of(&t, &h, STATION), &settings_of(&h), true).unwrap();
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
        assert_eq!(count(&t, &host(Profile::Bureau), STATION, true), 1);
    }

    /// Le plan ne dépend pas de « Cet écran » : le même, quelle que soit la fenêtre.
    #[test]
    fn the_plan_does_not_depend_on_the_window() {
        let t = three();
        let h = host(Profile::Bureau);
        let caps = super::super::session::capabilities(&h, "hevc", &Machine::default());
        let st = settings_of(&h);
        let plans: Vec<_> = (0..3).map(|c| session_plan(&core_screens(&t, c, &h, STATION, caps, false), &st, true)).collect();
        assert!(plans[0].is_some() && plans[0] == plans[1] && plans[1] == plans[2]);
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
