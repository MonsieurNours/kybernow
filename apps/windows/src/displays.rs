//! Les écrans de ce PC, pour le multi-écrans (docs/multi-ecrans.md) : ce que
//! l'app donne au cœur (`kybernow_core::screens::Screens`), qui en tire le
//! nombre d'écrans de la session et leur disposition sur l'ordinateur.
//!
//! `EnumDisplayMonitors` liste les écrans actifs du bureau de Windows (des
//! écrans en recopie n'y comptent que pour un) ; l'app est « per-monitor
//! DPI aware » : les cadres sont en pixels réels du bureau, sans mise à
//! l'échelle (le cœur s'en sert tels quels, sa disposition ne dépend que des
//! bords partagés). L'écran principal de Windows est l'écran principal de la
//! session ; « Cet écran » (session à un écran) est celui de la fenêtre de
//! l'accueil, comme sur Mac et Linux.

use kybernow_core::profile::Capabilities;
use kybernow_core::screens::{DeviceScreen, DisplaysSetting, Screens};

/// Un écran de ce PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// Nom du périphérique (`\\.\DISPLAY2`) : ce qui reconnaît un écran d'un
    /// relevé à l'autre (les index changent quand on en débranche un).
    pub key: String,
    /// Cadre sur le bureau de Windows, en pixels.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    /// Fréquence actuelle (Hz).
    pub refresh: u32,
    /// Échelle de l'écran, en DPI (96 : 100 %).
    pub dpi: u32,
}

impl Monitor {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        (self.x..self.right()).contains(&x) && (self.y..self.bottom()).contains(&y)
    }
}

/// Ce qui, avec les écrans, décide du nombre d'écrans d'une session : les
/// réglages de l'ordinateur et ce que la Station sait créer.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    /// Réglage « Écrans ».
    pub setting: DisplaysSetting,
    /// « Au plus 4K ».
    pub limit_4k: bool,
    /// Définition sur mesure : un seul écran.
    pub custom_resolution: bool,
    /// `virtual_display.max_screens` de `GET /api/stream/info` (1 : Station antérieure).
    pub max_screens: usize,
    /// Sur batterie, alléger : 30 Hz au plus pour tous les écrans (comme la
    /// session à un écran).
    pub lighten: bool,
    /// L'écran virtuel de la Station est utilisable (`virtual_display.
    /// available` de `GET /api/stream/info` ; inconnu tant qu'elle n'a pas
    /// répondu : oui, comme sur Mac). Sans lui, un seul écran, et le réglage
    /// « Écrans » est inactif.
    pub virtual_display_available: bool,
    /// Ce que ce PC décode : la règle « Personnalisé » du nombre d'écrans
    /// (`screens::screens_profile`) compare les réglages au profil avec elles.
    pub capabilities: Option<Capabilities>,
}

/// Les écrans de ce PC et ce qui, avec eux, décide du nombre d'écrans : de
/// quoi redonner au cœur ses `Screens` quand les écrans changent en session.
#[derive(Debug, Clone)]
pub struct Layout {
    pub monitors: Vec<Monitor>,
    pub spec: Spec,
    /// « Cet écran » relevé au lancement de la session (nom du périphérique),
    /// avant que l'accueil passe sur l'écran principal pour la connexion : la
    /// session le garde jusqu'à sa fin (profil changé en session, écrans de
    /// ce PC changés). `None`, ou écran débranché : l'écran de la fenêtre de
    /// l'accueil en ce moment.
    pub this_screen: Option<String>,
    /// Plusieurs ordinateurs : une session ouverte pendant qu'une autre
    /// tourne, qui prend « Cet écran » (celui de la fenêtre active, dans
    /// `this_screen`) quel que soit le réglage (`screens_for_extra_session`
    /// du cœur), au lancement comme quand les écrans de ce PC changent.
    pub extra_session: bool,
}

impl Layout {
    pub fn screens(&self) -> Screens {
        let mut s = screens(&self.monitors, self.spec);
        if let Some(i) = self.this_screen.as_ref().and_then(|k| self.monitors.iter().position(|m| m.key.eq_ignore_ascii_case(k))) {
            s.current = i;
        }
        if self.extra_session { kybernow_core::screens::screens_for_extra_session(&s) } else { s }
    }

    /// Les écrans de ce PC maintenant, et « Cet écran » : celui de la fenêtre
    /// de l'accueil à cet instant (gardé ensuite, voir `this_screen`).
    #[cfg(windows)]
    pub fn now(spec: Spec) -> Self {
        let monitors = enumerate();
        let this_screen = monitors.get(current(&monitors)).map(|m| m.key.clone());
        Self { monitors, spec, this_screen, extra_session: false }
    }

    /// L'écran de ce nom.
    pub fn find(&self, key: &str) -> Option<(usize, &Monitor)> {
        self.monitors.iter().enumerate().find(|(_, m)| m.key == key)
    }
}

/// Les écrans de l'appareil tels que le cœur les attend. « Cet écran » est
/// l'écran de la fenêtre de l'accueil (`current`) : c'est de lui que la
/// session à un écran prend son mode (`system::screen_mode`) et où sa fenêtre
/// se montre.
pub fn screens(monitors: &[Monitor], spec: Spec) -> Screens {
    Screens {
        list: monitors
            .iter()
            .map(|m| DeviceScreen {
                x: f64::from(m.x),
                y: f64::from(m.y),
                width: f64::from(m.width),
                height: f64::from(m.height),
                pixel_width: m.width.max(0) as u32,
                pixel_height: m.height.max(0) as u32,
                refresh: if spec.lighten { m.refresh.min(30) } else { m.refresh },
                primary: m.primary,
            })
            .collect(),
        current: current(monitors),
        limit_4k: spec.limit_4k,
        custom_resolution: spec.custom_resolution,
        setting: spec.setting,
        max_screens: spec.max_screens,
        virtual_display_available: spec.virtual_display_available,
        capabilities: spec.capabilities,
    }
}

/// Écrans que la Station sait créer : ce qu'elle annonce (`virtual_display.
/// max_screens` d'`info`, 1 avec une Station antérieure), ou, pour l'essai
/// sans Station multi-écrans, `KYBERNOW_MAX_SCREENS=4` (comme sur Mac,
/// `KN_MAX_SCREENS`) : l'app fait comme si la Station en créait autant.
pub fn max_screens(station: usize) -> usize {
    std::env::var("KYBERNOW_MAX_SCREENS").ok().and_then(|v| v.trim().parse::<usize>().ok()).map_or(station, |n| n.clamp(1, 4))
}

/// « Cet écran » : l'écran de la fenêtre de l'accueil, sinon l'écran principal.
pub fn current(monitors: &[Monitor]) -> usize {
    #[cfg(windows)]
    if let Some(key) = crate::system::home_monitor_key()
        && let Some(i) = monitors.iter().position(|m| m.key.eq_ignore_ascii_case(&key))
    {
        return i;
    }
    primary(monitors)
}

/// Index de l'écran principal de Windows (le premier à défaut).
pub fn primary(monitors: &[Monitor]) -> usize {
    monitors.iter().position(|m| m.primary).unwrap_or(0)
}

/// L'écran qui contient le point (bureau de Windows), sinon le plus proche.
pub fn at(monitors: &[Monitor], x: i32, y: i32) -> Option<usize> {
    if let Some(i) = monitors.iter().position(|m| m.contains(x, y)) {
        return Some(i);
    }
    let distance = |m: &Monitor| {
        let dx = (m.x - x).max(0).max(x - m.right());
        let dy = (m.y - y).max(0).max(y - m.bottom());
        i64::from(dx) * i64::from(dx) + i64::from(dy) * i64::from(dy)
    };
    (0..monitors.len()).min_by_key(|&i| distance(&monitors[i]))
}

/// Les écrans de ce PC, relevés par Windows.
#[cfg(windows)]
pub fn enumerate() -> Vec<Monitor> {
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        MONITORINFOEXW,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows::core::{BOOL, PCWSTR};

    const MONITORINFOF_PRIMARY: u32 = 1;

    unsafe extern "system" fn each(monitor: HMONITOR, _hdc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
        unsafe {
            let out = &mut *(data.0 as *mut Vec<Monitor>);
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if !GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
                return BOOL(1);
            }
            let r = info.monitorInfo.rcMonitor;
            let name_len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
            let key = String::from_utf16_lossy(&info.szDevice[..name_len]);
            let mut mode = DEVMODEW { dmSize: std::mem::size_of::<DEVMODEW>() as u16, ..Default::default() };
            let refresh = if EnumDisplaySettingsW(PCWSTR(info.szDevice.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode).as_bool() {
                // 0 et 1 : « matériel par défaut » pour Windows.
                if mode.dmDisplayFrequency > 1 { mode.dmDisplayFrequency } else { 60 }
            } else {
                60
            };
            let (mut dpi_x, mut dpi_y) = (96u32, 96u32);
            let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            out.push(Monitor {
                key,
                x: r.left,
                y: r.top,
                width: r.right - r.left,
                height: r.bottom - r.top,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                refresh: refresh.max(30),
                dpi: dpi_x.max(48),
            });
            BOOL(1)
        }
    }

    let mut out: Vec<Monitor> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut out as *mut Vec<Monitor> as isize));
    }
    // Sans écran lisible (session distante, pilote de base) : un écran par défaut.
    if out.is_empty() {
        let m = crate::system::screen_mode();
        out.push(Monitor { key: "\\\\.\\DISPLAY1".into(), x: 0, y: 0, width: m.width as i32, height: m.height as i32, primary: true, refresh: m.refresh, dpi: 96 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(key: &str, x: i32, y: i32, width: i32, height: i32, primary: bool, refresh: u32) -> Monitor {
        Monitor { key: key.into(), x, y, width, height, primary, refresh, dpi: 96 }
    }

    fn auteur() -> Vec<Monitor> {
        vec![
            monitor("\\\\.\\DISPLAY2", -2560, 400, 2560, 1600, false, 120),
            monitor("\\\\.\\DISPLAY1", 0, 0, 3840, 2160, true, 160),
            monitor("\\\\.\\DISPLAY3", 3840, 0, 2560, 1440, false, 60),
        ]
    }

    fn spec() -> Spec {
        Spec {
            setting: DisplaysSetting::Auto,
            limit_4k: false,
            custom_resolution: false,
            max_screens: 4,
            lighten: false,
            virtual_display_available: true,
            capabilities: Some(Capabilities { hevc: true, hevc444: true, av1: false }),
        }
    }

    #[test]
    fn cet_ecran_est_le_principal_de_windows() {
        let m = auteur();
        assert_eq!(primary(&m), 1);
        let s = screens(&m, spec());
        assert_eq!(s.current, 1);
        assert_eq!(s.list.len(), 3);
        assert!(s.list[1].primary && !s.list[0].primary);
        assert_eq!((s.list[1].pixel_width, s.list[1].pixel_height, s.list[1].refresh), (3840, 2160, 160));
        // Sans écran marqué principal : le premier.
        let none: Vec<Monitor> = m.iter().cloned().map(|x| Monitor { primary: false, ..x }).collect();
        assert_eq!(primary(&none), 0);
    }

    #[test]
    fn les_reglages_passent_au_coeur() {
        let s = screens(
            &auteur(),
            Spec { setting: DisplaysSetting::All, limit_4k: true, custom_resolution: true, max_screens: 2, virtual_display_available: false, ..spec() },
        );
        assert_eq!(s.setting, DisplaysSetting::All);
        assert!(s.limit_4k && s.custom_resolution);
        assert_eq!(s.max(), 2);
        assert!(!s.virtual_display_available);
        assert_eq!(s.capabilities, spec().capabilities);
    }

    #[test]
    fn cet_ecran_garde_au_lancement_de_la_session() {
        // L'accueil était sur l'écran de gauche au lancement, puis il est passé
        // sur l'écran principal pour la connexion : la session garde celui de gauche.
        let layout = Layout { monitors: auteur(), spec: spec(), this_screen: Some("\\\\.\\display2".into()), extra_session: false };
        assert_eq!(layout.screens().current, 0);
        // Écran débranché depuis : l'écran de l'accueil, à défaut le principal.
        let gone = Layout { this_screen: Some("\\\\.\\DISPLAY9".into()), ..layout.clone() };
        assert_eq!(gone.screens().current, 1);
        let none = Layout { this_screen: None, ..layout };
        assert_eq!(none.screens().current, 1);
    }

    #[test]
    fn regles_du_coeur_pour_l_accueil_le_reglage_et_la_session() {
        use kybernow_core::profile::{Profile, StreamSettings};
        use kybernow_core::screens::{screens_profile, session_plan_for};
        let caps = spec().capabilities.unwrap();
        let bureau = StreamSettings::for_profile(Profile::Bureau, caps);
        let s = screens(&auteur(), spec());
        assert_eq!(session_plan_for(&s, &bureau, true).map(|p| p.len()), Some(3));
        assert!(s.choice(screens_profile(&bureau, s.capabilities), true).setting_enabled());
        // Écran virtuel demandé mais pas utilisable sur la Station : un écran,
        // réglage inactif sur « Cet écran ».
        let off = screens(&auteur(), Spec { virtual_display_available: false, ..spec() });
        assert!(session_plan_for(&off, &bureau, true).is_none());
        let choice = off.choice(screens_profile(&bureau, off.capabilities), true);
        assert!(!choice.setting_enabled() && !choice.virtual_display);
        assert_eq!(choice.shown_setting(), DisplaysSetting::This);
        // Profil Bureau retouché (débit) : « Personnalisé », un seul écran en Auto.
        let retouched = StreamSettings { mbps: 50, ..bureau.clone() };
        assert_eq!(screens_profile(&retouched, s.capabilities), None);
        assert!(session_plan_for(&s, &retouched, true).is_none());
        // Réglages calculés par l'app pour un profil non retouché : le profil reste.
        assert_eq!(screens_profile(&bureau, s.capabilities), Some(Profile::Bureau));
    }

    #[test]
    fn sur_batterie_30_hz_au_plus() {
        let s = screens(&auteur(), Spec { lighten: true, ..spec() });
        assert_eq!(s.list.iter().map(|d| d.refresh).collect::<Vec<_>>(), [30, 30, 30]);
        // Un écran à 24 Hz n'est pas relevé.
        let m = vec![monitor("a", 0, 0, 1920, 1080, true, 24)];
        assert_eq!(screens(&m, Spec { lighten: true, ..spec() }).list[0].refresh, 24);
    }

    #[test]
    fn disposition_de_l_auteur_comme_le_coeur() {
        // Le même XG32UQ au centre, un écran à gauche plus bas et un à droite : trois écrans,
        // le principal en (0, 0), ceux de gauche et de droite collés à lui.
        let m = auteur();
        let s = screens(&m, spec());
        let plan = kybernow_core::screens::session_plan(&s, Some(kybernow_core::profile::Profile::Bureau), true, kybernow_core::profile::Codec::Hevc).expect("trois écrans");
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].device, 1);
        assert_eq!((plan[0].x, plan[0].y), (0, 0));
        assert!(plan[0].primary);
        let left = plan.iter().find(|p| p.device == 0).unwrap();
        assert_eq!((left.x + left.mode.width as i32, left.y), (0, 400));
        let right = plan.iter().find(|p| p.device == 2).unwrap();
        assert_eq!((right.x, right.y), (3840, 0));
        // Jeu, ou réglage « Cet écran » : un seul écran.
        assert!(kybernow_core::screens::session_plan(&s, Some(kybernow_core::profile::Profile::Jeu), true, kybernow_core::profile::Codec::Hevc).is_none());
        let this = screens(&m, Spec { setting: DisplaysSetting::This, ..spec() });
        assert!(kybernow_core::screens::session_plan(&this, Some(kybernow_core::profile::Profile::Bureau), true, kybernow_core::profile::Codec::Hevc).is_none());
        // Station qui ne crée qu'un écran.
        let old = screens(&m, Spec { max_screens: 1, ..spec() });
        assert!(kybernow_core::screens::session_plan(&old, Some(kybernow_core::profile::Profile::Bureau), true, kybernow_core::profile::Codec::Hevc).is_none());
    }

    #[test]
    fn session_de_plus_sur_cet_ecran() {
        // Plusieurs ordinateurs : une session de plus prend « Cet écran »,
        // celui de la fenêtre active, quel que soit le réglage (la règle du
        // cœur), et sa fenêtre est celle du cœur, en pixels de ce PC.
        let layout = Layout { monitors: auteur(), spec: Spec { setting: DisplaysSetting::All, ..spec() }, this_screen: Some("\\\\.\\DISPLAY3".into()), extra_session: true };
        let s = layout.screens();
        assert_eq!(s.current, 2);
        assert_eq!(s.setting, DisplaysSetting::This);
        let caps = spec().capabilities.unwrap();
        let bureau = kybernow_core::profile::StreamSettings::for_profile(kybernow_core::profile::Profile::Bureau, caps);
        assert!(kybernow_core::screens::session_plan_for(&s, &bureau, true).is_none(), "un seul écran");
        let w = kybernow_core::screens::extra_session_window(&s.list[s.current]);
        assert_eq!((w.mode.width, w.mode.height, w.mode.refresh), (1706, 960, 60));
        assert_eq!((w.x.round() as i32, w.y.round() as i32), (3840 + 427, 240));
        // La première session : le réglage tel quel.
        let first = Layout { extra_session: false, ..layout };
        assert_eq!(first.screens().setting, DisplaysSetting::All);
    }

    #[test]
    fn ecran_sous_un_point() {
        let m = auteur();
        assert_eq!(at(&m, 100, 100), Some(1));
        assert_eq!(at(&m, -100, 500), Some(0));
        assert_eq!(at(&m, 4000, 10), Some(2));
        // Hors de tout écran : le plus proche (juste sous l'écran de droite).
        assert_eq!(at(&m, 6000, 1600), Some(2));
        assert_eq!(at(&[], 0, 0), None);
    }
}
