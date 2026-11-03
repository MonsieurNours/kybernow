//! HDR des écrans virtuels (docs/hdr.md, « Station : Admin »).
//!
//! Au début d'une session, chaque écran virtuel demandé en HDR y passe
//! (`DISPLAYCONFIG_SET_HDR_STATE`, Windows 11 24H2), et Admin attend que
//! Windows le confirme (2 s au plus) avant de lancer les captures WGC et de
//! répondre à `prepare`. L'état d'origine de chaque écran est noté dans
//! `hdr-session.json` et remis à la fin, au redémarrage après un plantage et
//! quand l'écran est retiré : Windows garde le HDR d'un écran d'une fois sur
//! l'autre (les écrans virtuels ont des identifiants fixes).
//!
//! Avec le HDR, Admin pose aussi sur l'écran virtuel un profil de couleur
//! généré pour l'écran de l'appareil (`icc.rs` : luminosités, et bureau SDR
//! ramené au gamma 2.2 selon le réglage), associé par les API de couleur de
//! Windows (WCS), et règle le blanc du bureau (niveau de blanc SDR) ; l'état
//! d'avant (profil par défaut, blanc) est noté et remis comme le HDR.
//! Puis l'agent relit ce que Windows annonce aux jeux (`GetDesc1`, blanc
//! SDR) et Admin le compare aux valeurs de l'appareil (`check`) : le verdict
//! va au journal, dans `hdr-session.json` et dans la console.
//!
//! Comme la disposition des écrans, la lecture et la bascule se font dans la
//! session de l'écran, par l'agent d'affichage (`display-agent hdr --request
//! <fichier>`, voir `agent.rs`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Created;

/// Les valeurs de l'écran de l'appareil, en nits (`luminance` de `prepare`,
/// de `POST /api/stream/hdr` et de l'emprunt) : luminosité maximale (petite
/// zone), sur tout l'écran, noir, blanc du bureau. Absentes : 1000, 1000, 0, 203.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScreenLuminance {
    #[serde(default = "default_max")]
    pub max: f32,
    #[serde(default = "default_max")]
    pub full_frame: f32,
    #[serde(default)]
    pub min: f32,
    #[serde(default = "default_white")]
    pub sdr_white: f32,
}

fn default_max() -> f32 {
    1000.0
}
fn default_white() -> f32 {
    203.0
}

impl Default for ScreenLuminance {
    fn default() -> Self {
        ScreenLuminance { max: default_max(), full_frame: default_max(), min: 0.0, sdr_white: default_white() }
    }
}

// Valeurs bornées (`bounded`) : jamais NaN.
impl Eq for ScreenLuminance {}

impl ScreenLuminance {
    /// Bornes du contrat (comme le cœur) : maximum 100 à 10 000, plein écran
    /// au plus le maximum, noir 0 à 1, blanc 80 à 480 ; une valeur illisible
    /// prend sa valeur par défaut.
    pub fn bounded(self) -> Self {
        let d = Self::default();
        let ok = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let max = ok(self.max, d.max).clamp(100.0, 10_000.0);
        ScreenLuminance {
            max,
            full_frame: ok(self.full_frame, max).clamp(1.0, max),
            min: ok(self.min, d.min).clamp(0.0, 1.0),
            sdr_white: ok(self.sdr_white, d.sdr_white).clamp(80.0, 480.0),
        }
    }
}

/// Profil et blanc à poser sur un écran passé en HDR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorApply {
    /// Le profil généré (`icc.rs`), installé sous son nom de fichier.
    pub profile: PathBuf,
    /// Blanc du bureau, en nits.
    pub sdr_white: f32,
    /// Luminosités maximale et plein écran du profil : DXGI est attendu
    /// jusqu'à les rendre (vérification du HDR).
    pub max: f32,
    #[serde(default)]
    pub full_frame: Option<f32>,
    /// Profil de KyberNow posé avant sur cet écran, à retirer.
    #[serde(default)]
    pub replace: Option<String>,
}

/// Ce qu'il faut remettre : notre profil à retirer, et le profil par défaut
/// et le blanc d'avant la session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorOriginal {
    pub installed: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub sdr_white: Option<f32>,
    /// Notre profil associé aussi dans la portée du compte de l'utilisateur
    /// (voie `User`), avec son profil par défaut d'avant dans cette portée.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<UserColor>,
}

/// Portée du compte de l'utilisateur : profil par défaut d'avant notre passage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UserColor {
    #[serde(default)]
    pub original: Option<String>,
}

/// Un écran pour l'agent : son HDR voulu, la couleur à poser ou à remettre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrAsk {
    pub created: Created,
    pub on: bool,
    #[serde(default)]
    pub apply: Option<ColorApply>,
    #[serde(default)]
    pub restore: Option<ColorOriginal>,
    /// Voie `Toggle` : HDR coupé puis remis (profil déjà associé), pour que
    /// Windows relise le profil au passage en couleurs avancées.
    #[serde(default)]
    pub toggle: bool,
    /// Valeurs que `GetDesc1` doit rendre (sans `apply` : voie `Toggle`).
    #[serde(default)]
    pub expect: Option<ScreenLuminance>,
    /// Attente de la vérification, en millisecondes (absente : 2 s ; 0 : une
    /// voie suivante est déjà gardée, celle-ci est faite sans attendre).
    #[serde(default)]
    pub wait_ms: Option<u64>,
}

/// La demande que lit l'agent (`--request`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrRequest {
    pub screens: Vec<HdrAsk>,
}

/// Ce que l'agent a fait de la couleur d'un écran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColorSeen {
    /// Notre profil, associé et par défaut pour l'écran.
    #[serde(default)]
    pub installed: Option<String>,
    /// Profil par défaut et blanc avant notre passage.
    #[serde(default)]
    pub original_profile: Option<String>,
    #[serde(default)]
    pub original_white: Option<f32>,
    /// Blanc relu après la pose (ou la remise).
    #[serde(default)]
    pub white: Option<f32>,
    /// Remise faite (`restore`).
    #[serde(default)]
    pub restored: bool,
    #[serde(default)]
    pub error: Option<String>,
    /// Profil par défaut de l'écran relu après l'association (portée de l'ordinateur).
    #[serde(default)]
    pub system_default: Option<String>,
}

/// Luminance que Windows donne pour un écran (`IDXGIOutput6::GetDesc1`), en nits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Luminance {
    pub min: f32,
    pub max: f32,
    pub max_full_frame: f32,
}

/// Le HDR d'un écran, tel que l'agent l'a lu et laissé.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HdrSeen {
    /// L'état de l'écran a pu être lu (faux : écran introuvable, rien n'est connu).
    #[serde(default)]
    pub read: bool,
    /// L'écran peut passer en HDR.
    #[serde(default)]
    pub supported: bool,
    /// HDR avant la demande.
    #[serde(default)]
    pub before: bool,
    /// HDR en place à la fin de l'attente.
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub luminance: Option<Luminance>,
    /// Blanc du contenu SDR, en nits (curseur « Luminosité du contenu SDR »).
    #[serde(default)]
    pub sdr_white: Option<f32>,
    #[serde(default)]
    pub gdi_name: Option<String>,
    /// Bascule faite : `hdr_state` (24H2) ou `advanced_color` (repli).
    #[serde(default)]
    pub method: Option<String>,
    /// Temps qu'a mis Windows à confirmer, en millisecondes.
    #[serde(default)]
    pub waited_ms: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    /// Profil et blanc (pose ou remise).
    #[serde(default)]
    pub color: Option<ColorSeen>,
    /// Primaire rouge (x, y) que donne `GetDesc1` : celle du profil (0.708,
    /// 0.292) si Windows a pris sa colorimétrie, sinon celle de l'écran virtuel.
    #[serde(default)]
    pub red_primary: Option<[f32; 2]>,
}

/// Les voies essayées, dans l'ordre, pour que Windows annonce aux jeux les
/// valeurs du profil ; elles s'ajoutent l'une à l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Way {
    /// Profil associé pour tout l'ordinateur, par l'agent (SYSTEM).
    System,
    /// Profil associé aussi dans la portée du compte de l'utilisateur, par
    /// un agent lancé avec son compte dans sa session (comme l'outil
    /// d'étalonnage HDR de Windows).
    User,
    /// HDR coupé puis remis, le profil déjà associé.
    Toggle,
}

impl Way {

    /// Nom de la voie au journal.
    pub fn label(self) -> &'static str {
        match self {
            Way::System => "profil pour tout l'ordinateur",
            Way::User => "profil aussi pour le compte de l'utilisateur",
            Way::Toggle => "HDR coupé puis remis",
        }
    }
}

/// Ce que `GetDesc1` a rendu après une voie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Try {
    pub way: Way,
    #[serde(default)]
    pub seen: Option<Luminance>,
    /// Faite sans attendre (une voie suivante était gardée).
    #[serde(default)]
    pub quick: bool,
    #[serde(default)]
    pub error: Option<String>,
}

/// Un écran pour l'agent lancé avec le compte de l'utilisateur (`hdr-user`) :
/// notre profil (déjà installé par l'agent SYSTEM) à associer dans la portée
/// du compte, ou à retirer (`restore`, avec le profil par défaut d'avant).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAsk {
    pub created: Created,
    pub profile: String,
    #[serde(default)]
    pub restore: bool,
    #[serde(default)]
    pub original: Option<String>,
    #[serde(default)]
    pub expect: Option<ScreenLuminance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRequest {
    pub screens: Vec<UserAsk>,
    #[serde(default)]
    pub wait_ms: u64,
}

/// Ce que l'agent de l'utilisateur a fait d'un écran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserSeen {
    /// Portée que Windows applique pour ce compte (`ColorProfileGetDisplayUserScope`) : `user` ou `system`.
    #[serde(default)]
    pub scope: Option<String>,
    /// Profil par défaut de la portée du compte avant, et relu après.
    #[serde(default)]
    pub original: Option<String>,
    #[serde(default)]
    pub default_after: Option<String>,
    #[serde(default)]
    pub associated: bool,
    #[serde(default)]
    pub restored: bool,
    #[serde(default)]
    pub luminance: Option<Luminance>,
    #[serde(default)]
    pub red_primary: Option<[f32; 2]>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Écart toléré entre ce que Windows annonce aux jeux et les valeurs voulues.
pub const CHECK_TOLERANCE: f32 = 0.05;

fn close(seen: f32, wanted: f32) -> bool {
    (seen - wanted).abs() <= wanted.abs() * CHECK_TOLERANCE
}

/// `GetDesc1` annonce la luminosité maximale du profil, à 5 % près. Le
/// plein écran est noté sans être jugé : l'étiquette `MHC2` n'en a pas, et
/// Windows peut le tirer du maximum.
pub fn desc_matches(l: &Luminance, max: f32) -> bool {
    close(l.max, max)
}

/// Verdict de la vérification du HDR d'un écran virtuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Windows annonce aux jeux les valeurs de l'écran de l'appareil.
    Ok,
    /// Les luminosités annoncées ne sont pas celles du profil.
    Profile,
    /// Luminosités justes, blanc du bureau à côté.
    White,
    /// L'écran virtuel n'est pas en HDR.
    NotHdr,
    /// `GetDesc1` ne connaît pas l'écran.
    Unread,
}

/// Vérification du HDR d'un écran virtuel, au début d'une session HDR :
/// ce que Windows annonce aux jeux (`IDXGIOutput6::GetDesc1`, et le blanc
/// SDR), comparé aux valeurs de l'écran de l'appareil. Gardée dans
/// `hdr-session.json` et donnée par `/api/display` (`session.hdr_check`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HdrCheck {
    pub status: CheckStatus,
    /// Valeurs voulues (écran de l'appareil).
    pub wanted: ScreenLuminance,
    /// Ce que `GetDesc1` annonce.
    #[serde(default)]
    pub seen: Option<Luminance>,
    /// Blanc SDR relu, en nits.
    #[serde(default)]
    pub sdr_white: Option<f32>,
    /// La voie qui a fait suivre `GetDesc1` (aucune : `status` le dit).
    #[serde(default)]
    pub way: Option<Way>,
    /// Ce que `GetDesc1` a rendu après chaque voie essayée.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tries: Vec<Try>,
}

/// Compare ce que l'agent a relu aux valeurs voulues (5 % près ; le noir
/// est noté sans être jugé : quelques millièmes de nit n'ont pas de sens
/// en pourcentage).
pub fn check(wanted: &ScreenLuminance, seen: &HdrSeen) -> HdrCheck {
    let status = if !seen.active {
        CheckStatus::NotHdr
    } else {
        match seen.luminance {
            None => CheckStatus::Unread,
            Some(l) if !desc_matches(&l, wanted.max) => CheckStatus::Profile,
            Some(_) if seen.sdr_white.is_some_and(|w| !close(w, wanted.sdr_white)) => CheckStatus::White,
            Some(_) => CheckStatus::Ok,
        }
    };
    HdrCheck {
        status,
        wanted: *wanted,
        seen: seen.luminance.filter(|_| seen.active),
        sdr_white: seen.sdr_white,
        way: None,
        tries: Vec::new(),
    }
}

/// Ligne de journal d'une voie essayée : ce que `GetDesc1` rend ensuite.
pub fn try_line(name: &str, t: &Try, wanted: f32, extra: &str) -> String {
    let mut line = format!("écran {name} : voie « {} »", t.way.label());
    if t.quick {
        line.push_str(" (sans attendre : une voie suivante est gardée)");
    }
    line.push_str(extra);
    match (&t.error, t.seen) {
        (Some(e), _) => line.push_str(&format!(" ; impossible : {e}")),
        (None, Some(l)) => line.push_str(&format!(
            " ; GetDesc1 rend {:.3}–{:.0} nits (plein écran {:.0}) pour {wanted:.0} voulus : {}",
            l.min,
            l.max,
            l.max_full_frame,
            if desc_matches(&l, wanted) { "suivi" } else { "pas suivi" }
        )),
        (None, None) => line.push_str(" ; GetDesc1 illisible"),
    }
    line
}

/// Ligne de journal de la vérification.
pub fn check_line(name: &str, c: &HdrCheck) -> String {
    let mut line = check_verdict(name, c);
    match c.way {
        Some(w) if c.status == CheckStatus::Ok && c.tries.len() > 1 => line.push_str(&format!(" ; voie gardée : « {} »", w.label())),
        None if c.status == CheckStatus::Profile && c.tries.len() > 1 => {
            line.push_str(&format!(" ; aucune des {} voies n'a fait suivre GetDesc1, la session continue en HDR", c.tries.len()))
        }
        _ => {}
    }
    line
}

fn check_verdict(name: &str, c: &HdrCheck) -> String {
    let w = &c.wanted;
    let seen = |l: &Luminance| format!("{:.0} nits (plein écran {:.0}, noir {:.3})", l.max, l.max_full_frame, l.min);
    let white = c.sdr_white.map(|v| format!("{v:.0}")).unwrap_or_else(|| "?".into());
    match (c.status, c.seen.as_ref()) {
        (CheckStatus::Ok, Some(l)) => format!(
            "écran {name} : vérification du HDR bonne : les jeux voient {}, blanc du bureau {white} nits (voulu {:.0} nits, plein écran {:.0}, blanc {:.0})",
            seen(l), w.max, w.full_frame, w.sdr_white
        ),
        (CheckStatus::Profile, Some(l)) => format!(
            "écran {name} : vérification du HDR : Windows n'a pas pris le profil : les jeux voient {} au lieu de {:.0} nits (plein écran {:.0}, noir {:.3})",
            seen(l), w.max, w.full_frame, w.min
        ),
        (CheckStatus::White, Some(l)) => format!(
            "écran {name} : vérification du HDR : les jeux voient {}, mais le blanc du bureau est à {white} nits au lieu de {:.0}",
            seen(l), w.sdr_white
        ),
        (CheckStatus::NotHdr, _) => format!("écran {name} : vérification du HDR : l'écran virtuel n'est pas en HDR"),
        _ => format!("écran {name} : vérification du HDR : GetDesc1 ne connaît pas l'écran, luminosités annoncées aux jeux inconnues"),
    }
}

/// Ligne de journal de la couleur posée sur un écran : valeurs, profil
/// associé, blanc relu, et luminosité que `GetDesc1` rend ensuite (ce que
/// les jeux liront).
pub fn color_line(name: &str, lum: &ScreenLuminance, gamma22: bool, seen: &HdrSeen) -> String {
    let mut line = format!(
        "écran {name} : valeurs de l'écran de l'appareil posées : {:.0} nits (plein écran {:.0}), noir {:.3}, blanc du bureau {:.0} nits, bureau en {}",
        lum.max,
        lum.full_frame,
        lum.min,
        lum.sdr_white,
        if gamma22 { "gamma 2.2" } else { "sRGB" }
    );
    match seen.color.as_ref() {
        Some(c) => {
            match &c.installed {
                Some(p) => line.push_str(&format!(" ; profil {p} associé")),
                None => line.push_str(" ; profil non associé"),
            }
            if let Some(w) = c.white {
                line.push_str(&format!(", blanc relu {w:.0} nits"));
            }
            if let Some(e) = &c.error {
                line.push_str(&format!(" ({e})"));
            }
        }
        None => line.push_str(" ; profil non associé"),
    }
    match seen.luminance {
        Some(l) => line.push_str(&format!(
            " ; GetDesc1 rend ensuite {:.3}–{:.0} nits (pleine image {:.0} nits)",
            l.min, l.max, l.max_full_frame
        )),
        None => line.push_str(" ; GetDesc1 illisible"),
    }
    if let Some([x, y]) = seen.red_primary {
        line.push_str(&format!(", primaire rouge {x:.3},{y:.3} (celle du profil : 0.708,0.292)"));
    }
    line
}

/// Une ligne de journal par écran : HDR demandé, obtenu, luminance donnée
/// par Windows, format de capture que Kyber prendra.
pub fn log_line(name: &str, wanted: bool, seen: &HdrSeen) -> String {
    let yes = |b: bool| if b { "oui" } else { "non" };
    let mut line = format!("écran {name} : HDR demandé {}, obtenu {}", yes(wanted), yes(seen.active));
    if wanted && !seen.supported {
        line.push_str(" (l'écran ne propose pas le HDR)");
    }
    match seen.luminance {
        Some(l) => line.push_str(&format!(
            ", luminance donnée par Windows {:.3}–{:.0} nits (pleine image {:.0} nits)",
            l.min, l.max, l.max_full_frame
        )),
        None => line.push_str(", luminance inconnue"),
    }
    if let Some(w) = seen.sdr_white {
        line.push_str(&format!(", blanc SDR {w:.0} nits"));
    }
    line.push_str(if seen.active { ", capture en R16G16B16A16 flottant" } else { ", capture en BGRA 8 bits" });
    if let (Some(method), Some(ms)) = (&seen.method, seen.waited_ms) {
        line.push_str(&format!(" ; bascule par {method}, confirmée en {ms} ms"));
    }
    if let Some(e) = &seen.error {
        line.push_str(&format!(" ; {e}"));
    }
    line
}

#[cfg(windows)]
pub use win::{agent_primary, agent_set, agent_user};

#[cfg(windows)]
mod win {
    use std::time::{Duration, Instant};

    use anyhow::{Result, bail};
    use serde_json::{Value, json};
    use windows_sys::Win32::Devices::Display::{
        DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
        DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
        DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE,
        DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo,
    };
    use windows_sys::Win32::Foundation::LUID;

    use super::{ColorApply, ColorOriginal, ColorSeen, HdrAsk, HdrSeen, Luminance, UserAsk, UserSeen};
    use crate::display::Created;
    use crate::display::topology;

    // Windows 11 24H2 : absents de windows-sys 0.61.
    const GET_ADVANCED_COLOR_INFO_2: i32 = 15;
    const SET_HDR_STATE: i32 = 16;
    /// `DISPLAYCONFIG_ADVANCED_COLOR_MODE_HDR`.
    const COLOR_MODE_HDR: i32 = 2;
    /// Attente de la confirmation de Windows.
    const CONFIRM: Duration = Duration::from_secs(2);
    /// Non documenté (le curseur « Luminosité du contenu SDR » de Windows) :
    /// `SDRWhiteLevel` en millièmes de 80 nits, puis `finalValue` à 1.
    const SET_SDR_WHITE_LEVEL: i32 = 0xFFFF_FFEEu32 as i32;

    #[repr(C)]
    struct SetSdrWhiteLevel {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        level: u32,
        final_value: u8,
    }

    fn set_sdr_white(c: &Created, nits: f32) -> Result<()> {
        let s = SetSdrWhiteLevel {
            header: header(SET_SDR_WHITE_LEVEL, size_of::<SetSdrWhiteLevel>(), c),
            level: (nits * 1000.0 / 80.0).round() as u32,
            final_value: 1,
        };
        let err = unsafe { DisplayConfigSetDeviceInfo(&s.header) };
        if err != 0 {
            bail!("blanc du bureau refusé par Windows (code {err})");
        }
        Ok(())
    }

    /// Profils de couleur de l'écran (WCS, portée de tout l'ordinateur :
    /// l'agent tourne en SYSTEM).
    mod wcs {
        use std::path::Path;

        use anyhow::{Result, bail};
        use windows_sys::Win32::Foundation::{LUID, LocalFree};
        use windows_sys::Win32::UI::ColorSystem::{
            CPST_EXTENDED_DISPLAY_COLOR_MODE, CPT_ICC, ColorProfileAddDisplayAssociation, ColorProfileGetDisplayDefault,
            ColorProfileRemoveDisplayAssociation, ColorProfileSetDisplayDefaultAssociation, GetColorDirectoryW,
            ColorProfileGetDisplayUserScope, InstallColorProfileW, UninstallColorProfileW, WCS_PROFILE_MANAGEMENT_SCOPE,
            WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER, WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE,
        };

        /// Portée de l'ordinateur (agent SYSTEM) ou du compte (agent de l'utilisateur).
        pub type Scope = WCS_PROFILE_MANAGEMENT_SCOPE;
        pub const SYSTEM: Scope = WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE;
        pub const USER: Scope = WCS_PROFILE_MANAGEMENT_SCOPE_CURRENT_USER;

        /// Portée que Windows applique à l'écran pour le compte qui appelle.
        pub fn user_scope(luid: LUID, source: u32) -> Option<&'static str> {
            let mut scope: Scope = 0;
            let hr = unsafe { ColorProfileGetDisplayUserScope(luid, source, &mut scope) };
            (hr >= 0).then_some(if scope == USER { "user" } else { "system" })
        }

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        fn check(hr: i32, what: &str) -> Result<()> {
            if hr < 0 {
                bail!("{what} : HRESULT {hr:#010x}");
            }
            Ok(())
        }

        /// Profil par défaut de l'écran en mode couleurs avancées (HDR).
        pub fn default_profile(scope: Scope, luid: LUID, source: u32) -> Option<String> {
            let mut name: *mut u16 = std::ptr::null_mut();
            let hr = unsafe { ColorProfileGetDisplayDefault(scope, luid, source, CPT_ICC, CPST_EXTENDED_DISPLAY_COLOR_MODE, &mut name) };
            if hr < 0 || name.is_null() {
                return None;
            }
            let text = unsafe {
                let len = (0..).take_while(|&i| *name.add(i) != 0).count();
                String::from_utf16_lossy(std::slice::from_raw_parts(name, len))
            };
            unsafe { LocalFree(name as *mut _) };
            (!text.is_empty()).then_some(text)
        }

        /// Installe le profil (copié dans le dossier des profils de Windows) ; rend son nom.
        pub fn install(path: &Path) -> Result<String> {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            let _ = uninstall(&name);
            if unsafe { InstallColorProfileW(std::ptr::null(), wide(&path.display().to_string()).as_ptr()) } == 0 {
                bail!("installation du profil {name} : {}", std::io::Error::last_os_error());
            }
            Ok(name)
        }

        pub fn uninstall(name: &str) -> Result<()> {
            let mut buf = [0u16; 512];
            let mut size = (buf.len() * 2) as u32;
            if unsafe { GetColorDirectoryW(std::ptr::null(), buf.as_mut_ptr(), &mut size) } == 0 {
                bail!("dossier des profils : {}", std::io::Error::last_os_error());
            }
            let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            let path = format!("{}\\{name}", String::from_utf16_lossy(&buf[..len]));
            if unsafe { UninstallColorProfileW(std::ptr::null(), wide(&path).as_ptr(), 1) } == 0 {
                bail!("retrait du profil {name} : {}", std::io::Error::last_os_error());
            }
            Ok(())
        }

        /// Associe le profil à l'écran, par défaut, en mode couleurs avancées.
        /// (`setAsDefault` et `associateAsAdvancedColor` à vrai.)
        pub fn associate(scope: Scope, name: &str, luid: LUID, source: u32) -> Result<()> {
            check(unsafe { ColorProfileAddDisplayAssociation(scope, wide(name).as_ptr(), luid, source, 1, 1) }, "association du profil")
        }

        pub fn dissociate(scope: Scope, name: &str, luid: LUID, source: u32) -> Result<()> {
            check(unsafe { ColorProfileRemoveDisplayAssociation(scope, wide(name).as_ptr(), luid, source, 1) }, "retrait de l'association")
        }

        pub fn set_default(scope: Scope, name: &str, luid: LUID, source: u32) -> Result<()> {
            check(
                unsafe { ColorProfileSetDisplayDefaultAssociation(scope, wide(name).as_ptr(), CPT_ICC, CPST_EXTENDED_DISPLAY_COLOR_MODE, luid, source) },
                "profil par défaut",
            )
        }
    }

    /// Adaptateur et source de l'écran (les API de couleur les demandent).
    fn source_of(c: &Created) -> Option<(LUID, u32)> {
        let (paths, _) = topology::active_config().ok()?;
        let path = paths.iter().find(|p| topology::is_target(p, c.luid_low, c.luid_high, c.target))?;
        Some((path.targetInfo.adapterId, path.sourceInfo.id))
    }

    /// Pose notre profil et le blanc du bureau, en notant ce qu'il y avait.
    fn apply_color(c: &Created, apply: &ColorApply) -> ColorSeen {
        let mut seen = ColorSeen { original_white: sdr_white(c), ..ColorSeen::default() };
        let Some((luid, source)) = source_of(c) else {
            seen.error = Some("écran introuvable".into());
            return seen;
        };
        seen.original_profile = wcs::default_profile(wcs::SYSTEM, luid, source);
        let mut errors = Vec::new();
        if let Some(old) = &apply.replace {
            let _ = wcs::dissociate(wcs::SYSTEM, old, luid, source);
            let _ = wcs::uninstall(old);
        }
        match wcs::install(&apply.profile).and_then(|name| wcs::associate(wcs::SYSTEM, &name, luid, source).map(|_| name)) {
            Ok(name) => seen.installed = Some(name),
            Err(e) => errors.push(format!("{e:#}")),
        }
        seen.system_default = wcs::default_profile(wcs::SYSTEM, luid, source);
        if let Err(e) = set_sdr_white(c, apply.sdr_white) {
            errors.push(format!("{e:#}"));
        }
        seen.white = sdr_white(c);
        seen.error = (!errors.is_empty()).then(|| errors.join(" ; "));
        seen
    }

    /// Retire notre profil, remet le profil par défaut et le blanc d'avant.
    fn restore_color(c: &Created, original: &ColorOriginal) -> ColorSeen {
        let mut seen = ColorSeen::default();
        let Some((luid, source)) = source_of(c) else {
            seen.error = Some("écran introuvable".into());
            return seen;
        };
        let mut errors = Vec::new();
        if let Err(e) = wcs::dissociate(wcs::SYSTEM, &original.installed, luid, source) {
            errors.push(format!("{e:#}"));
        }
        if let Some(profile) = &original.profile
            && let Err(e) = wcs::set_default(wcs::SYSTEM, profile, luid, source)
        {
            errors.push(format!("{e:#}"));
        }
        if let Some(white) = original.sdr_white
            && let Err(e) = set_sdr_white(c, white)
        {
            errors.push(format!("{e:#}"));
        }
        let _ = wcs::uninstall(&original.installed);
        seen.white = sdr_white(c);
        seen.restored = errors.is_empty();
        seen.error = (!errors.is_empty()).then(|| errors.join(" ; "));
        seen
    }

    /// `DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO_2` (wingdi.h, 24H2).
    #[repr(C)]
    struct AdvancedColorInfo2 {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        /// Bits : 0 couleurs avancées possibles, 1 actives, 3 limitées par une
        /// règle, 4 HDR possible, 5 HDR choisi par l'utilisateur, 6 et 7 WCG.
        value: u32,
        color_encoding: i32,
        bits_per_color_channel: u32,
        active_color_mode: i32,
    }

    /// `DISPLAYCONFIG_SET_HDR_STATE` (wingdi.h, 24H2).
    #[repr(C)]
    struct SetHdrState {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
        /// Bit 0 : `enableHdr`.
        value: u32,
    }

    fn header(kind: i32, size: usize, c: &Created) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
        DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: kind,
            size: size as u32,
            adapterId: LUID { LowPart: c.luid_low, HighPart: c.luid_high },
            id: c.target,
        }
    }

    struct State {
        supported: bool,
        active: bool,
        /// L'API de 24H2 répond : la bascule passe par `SET_HDR_STATE`.
        hdr_api: bool,
    }

    fn read_state(c: &Created) -> Result<State> {
        let mut info: AdvancedColorInfo2 = unsafe { std::mem::zeroed() };
        info.header = header(GET_ADVANCED_COLOR_INFO_2, size_of::<AdvancedColorInfo2>(), c);
        let err = unsafe { DisplayConfigGetDeviceInfo(&mut info.header) };
        if err == 0 {
            let _ = (info.color_encoding, info.bits_per_color_channel);
            return Ok(State {
                supported: info.value & (1 << 4) != 0,
                active: info.active_color_mode == COLOR_MODE_HDR,
                hdr_api: true,
            });
        }
        // Avant 24H2 : « couleurs avancées » veut dire HDR.
        let mut old: DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO = unsafe { std::mem::zeroed() };
        old.header = header(DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO, size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(), c);
        let err2 = unsafe { DisplayConfigGetDeviceInfo(&mut old.header) };
        if err2 != 0 {
            bail!("état HDR de l'écran illisible (codes {err} puis {err2})");
        }
        let v = unsafe { old.Anonymous.value };
        // Bits : 0 possible, 1 actif, 3 coupé de force.
        Ok(State { supported: v & 1 != 0, active: v & 2 != 0 && v & 8 == 0, hdr_api: false })
    }

    fn write_state(c: &Created, on: bool, hdr_api: bool) -> Result<&'static str> {
        let mut first = None;
        if hdr_api {
            let s = SetHdrState { header: header(SET_HDR_STATE, size_of::<SetHdrState>(), c), value: u32::from(on) };
            let err = unsafe { DisplayConfigSetDeviceInfo(&s.header) };
            if err == 0 {
                return Ok("DISPLAYCONFIG_SET_HDR_STATE");
            }
            first = Some(err);
        }
        let mut s: DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE = unsafe { std::mem::zeroed() };
        s.header = header(DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE, size_of::<DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE>(), c);
        s.Anonymous.value = u32::from(on);
        let err = unsafe { DisplayConfigSetDeviceInfo(&s.header) };
        if err != 0 {
            match first {
                Some(e) => bail!("Windows refuse de changer le HDR de l'écran (codes {e} puis {err})"),
                None => bail!("Windows refuse de changer le HDR de l'écran (code {err})"),
            }
        }
        Ok("DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE")
    }

    /// Blanc SDR de l'écran, en nits (`SDRWhiteLevel` vaut 1000 pour 80 nits).
    fn sdr_white(c: &Created) -> Option<f32> {
        let mut w: DISPLAYCONFIG_SDR_WHITE_LEVEL = unsafe { std::mem::zeroed() };
        w.header = header(DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>(), c);
        (unsafe { DisplayConfigGetDeviceInfo(&mut w.header) } == 0).then(|| w.SDRWhiteLevel as f32 * 80.0 / 1000.0)
    }

    /// Ce que DXGI dit de l'écran `gdi_name` : sa luminance, et s'il est en
    /// HDR pour les captures (espace `RGB_FULL_G2084_NONE_P2020`, que suit
    /// l'agent WGC). Fabrique neuve à chaque lecture : DXGI garde sinon
    /// l'état du moment où elle a été créée.
    fn dxgi(gdi_name: &str) -> Option<(Luminance, bool)> {
        desc(gdi_name).map(|d| (d.luminance, d.hdr))
    }

    /// Ce que `GetDesc1` donne d'un écran.
    struct Desc {
        luminance: Luminance,
        hdr: bool,
        red: [f32; 2],
    }

    fn desc(gdi_name: &str) -> Option<Desc> {
        use windows::Win32::Graphics::Dxgi::Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020;
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6};
        use windows::core::Interface;
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
        let mut a = 0;
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(a) } {
            let mut o = 0;
            while let Ok(output) = unsafe { adapter.EnumOutputs(o) } {
                if let Ok(desc) = output.cast::<IDXGIOutput6>().and_then(|o6| unsafe { o6.GetDesc1() }) {
                    let len = desc.DeviceName.iter().position(|c| *c == 0).unwrap_or(desc.DeviceName.len());
                    if String::from_utf16_lossy(&desc.DeviceName[..len]) == gdi_name {
                        let luminance = Luminance { min: desc.MinLuminance, max: desc.MaxLuminance, max_full_frame: desc.MaxFullFrameLuminance };
                        return Some(Desc {
                            luminance,
                            hdr: desc.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
                            red: desc.RedPrimary,
                        });
                    }
                }
                o += 1;
            }
            a += 1;
        }
        None
    }

    fn gdi_of(c: &Created) -> Option<String> {
        let (paths, _) = topology::active_config().ok()?;
        paths
            .iter()
            .find(|p| topology::is_target(p, c.luid_low, c.luid_high, c.target))
            .and_then(topology::gdi_name)
            .filter(|n| !n.is_empty())
    }

    /// `display-agent hdr` : passe chaque écran au HDR voulu et attend que
    /// Windows le confirme (2 s au plus, pour tous ensemble) ; la capture
    /// suit DXGI, attendu lui aussi quand il connaît l'écran.
    pub fn agent_set(asks: &[HdrAsk]) -> Result<Value> {
        // Couleurs d'avant remises d'abord (fin de session, reprise).
        let restored: Vec<Option<ColorSeen>> = asks.iter().map(|a| a.restore.as_ref().map(|r| restore_color(&a.created, r))).collect();
        let screens: Vec<(Created, bool)> = asks.iter().map(|a| (a.created, a.on)).collect();
        let begun = Instant::now();
        let mut seen: Vec<HdrSeen> = Vec::with_capacity(screens.len());
        for (c, want) in &screens {
            let mut s = HdrSeen { gdi_name: gdi_of(c), ..HdrSeen::default() };
            match read_state(c) {
                Ok(state) => {
                    s.read = true;
                    s.supported = state.supported;
                    s.before = state.active;
                    s.active = state.active;
                    // Voie `Toggle` : HDR coupé (attendu 1 s au plus), puis remis plus bas.
                    let toggle = ask_of(asks, c).is_some_and(|a| a.toggle) && *want && state.active;
                    if toggle {
                        match write_state(c, false, state.hdr_api) {
                            Ok(_) => {
                                let off = Instant::now() + Duration::from_secs(1);
                                while Instant::now() < off && read_state(c).is_ok_and(|st| st.active) {
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                s.active = false;
                            }
                            Err(e) => s.error = Some(format!("HDR non coupé : {e:#}")),
                        }
                    }
                    if s.active != *want {
                        if *want && !state.supported {
                            s.error = Some("l'écran ne propose pas le HDR".into());
                        } else {
                            match write_state(c, *want, state.hdr_api) {
                                Ok(method) => s.method = Some(method.into()),
                                Err(e) => s.error = Some(format!("{e:#}")),
                            }
                        }
                    }
                }
                Err(e) => s.error = Some(format!("{e:#}")),
            }
            seen.push(s);
        }
        let deadline = begun + CONFIRM;
        loop {
            let mut waiting = false;
            for ((c, want), s) in screens.iter().zip(seen.iter_mut()) {
                if s.method.is_none() || s.waited_ms.is_some() {
                    continue;
                }
                if let Ok(state) = read_state(c) {
                    s.active = state.active;
                }
                if s.gdi_name.is_none() {
                    s.gdi_name = gdi_of(c);
                }
                let dxgi_ok = s.gdi_name.as_deref().and_then(dxgi).is_none_or(|(_, hdr)| hdr == *want);
                if s.active == *want && dxgi_ok {
                    s.waited_ms = Some(begun.elapsed().as_millis() as u64);
                } else {
                    waiting = true;
                }
            }
            if !waiting || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        for ((c, _), s) in screens.iter().zip(seen.iter_mut()) {
            if s.method.is_some() && s.waited_ms.is_none() {
                s.error.get_or_insert_with(|| format!("Windows n'a pas confirmé en {} s", CONFIRM.as_secs()));
                // Ce que voit la capture compte : DXGI a le dernier mot.
                if let Some((_, hdr)) = s.gdi_name.as_deref().and_then(dxgi) {
                    s.active = s.active && hdr;
                }
            }
            s.luminance = s.gdi_name.as_deref().and_then(dxgi).map(|(l, _)| l);
            s.sdr_white = sdr_white(c);
        }
        // Profil et blanc sur les écrans en HDR, puis la vérification : ce
        // que DXGI annonce aux jeux et le blanc SDR, relus jusqu'à ce qu'ils
        // suivent le profil (Windows l'applique en différé, 2 s au plus).
        // L'agent tourne dans la session de l'écran : DXGI y voit ses sorties.
        let mut expected: Vec<Option<(f32, f32, u64)>> = vec![None; asks.len()];
        for (i, (ask, restored)) in asks.iter().zip(restored).enumerate() {
            let s = &mut seen[i];
            s.color = restored;
            let wait = |w: Option<u64>| w.unwrap_or(CONFIRM.as_millis() as u64);
            if let Some(apply) = ask.apply.as_ref().filter(|_| ask.on && s.active) {
                let color = apply_color(&ask.created, apply);
                if color.installed.is_some() {
                    expected[i] = Some((apply.max, apply.sdr_white, wait(ask.wait_ms)));
                }
                s.sdr_white = color.white.or(s.sdr_white);
                s.color = Some(color);
            } else if let Some(e) = ask.expect.filter(|_| ask.on && s.active) {
                expected[i] = Some((e.max, e.sdr_white, wait(ask.wait_ms)));
            }
        }
        // Relu toutes les 200 ms, fabrique DXGI neuve à chaque fois (`desc`).
        let begun = Instant::now();
        loop {
            let mut waiting = false;
            for ((s, e), ask) in seen.iter_mut().zip(&expected).zip(asks) {
                let Some((max, white, wait_ms)) = e else { continue };
                let d = s.gdi_name.as_deref().and_then(desc);
                s.luminance = d.as_ref().map(|d| d.luminance);
                s.red_primary = d.as_ref().map(|d| d.red);
                s.sdr_white = sdr_white(&ask.created).or(s.sdr_white);
                let white_ok = s.sdr_white.is_none_or(|w| (w - white).abs() <= white * super::CHECK_TOLERANCE);
                let ok = white_ok && s.luminance.is_some_and(|l| super::desc_matches(&l, *max));
                waiting |= !ok && begun.elapsed() < Duration::from_millis(*wait_ms);
            }
            if !waiting {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        for s in seen.iter_mut().filter(|s| s.red_primary.is_none()) {
            s.red_primary = s.gdi_name.as_deref().and_then(desc).map(|d| d.red);
        }
        Ok(json!({ "screens": seen }))
    }

    fn ask_of<'a>(asks: &'a [HdrAsk], c: &Created) -> Option<&'a HdrAsk> {
        asks.iter().find(|a| a.created == *c)
    }

    /// `display-agent hdr-user`, lancé avec le compte de l'utilisateur dans
    /// sa session : associe notre profil (installé par l'agent SYSTEM) dans
    /// la portée de son compte, par défaut et en couleurs avancées, comme le
    /// fait l'outil d'étalonnage HDR de Windows, puis relit `GetDesc1`
    /// (`wait_ms` au plus) ; ou le retire et remet le profil d'avant (`restore`).
    pub fn agent_user(asks: &[UserAsk], wait_ms: u64) -> Result<Value> {
        let mut seen: Vec<UserSeen> = Vec::with_capacity(asks.len());
        let mut names: Vec<Option<String>> = Vec::with_capacity(asks.len());
        for ask in asks {
            let mut s = UserSeen::default();
            names.push(gdi_of(&ask.created));
            let Some((luid, source)) = source_of(&ask.created) else {
                s.error = Some("écran introuvable".into());
                seen.push(s);
                continue;
            };
            s.scope = wcs::user_scope(luid, source).map(str::to_string);
            if ask.restore {
                let mut errors = Vec::new();
                if let Err(e) = wcs::dissociate(wcs::USER, &ask.profile, luid, source) {
                    errors.push(format!("{e:#}"));
                }
                if let Some(original) = &ask.original
                    && let Err(e) = wcs::set_default(wcs::USER, original, luid, source)
                {
                    errors.push(format!("{e:#}"));
                }
                s.restored = errors.is_empty();
                s.error = (!errors.is_empty()).then(|| errors.join(" ; "));
            } else {
                s.original = wcs::default_profile(wcs::USER, luid, source);
                match wcs::associate(wcs::USER, &ask.profile, luid, source) {
                    Ok(()) => s.associated = true,
                    Err(e) => s.error = Some(format!("{e:#}")),
                }
            }
            s.default_after = wcs::default_profile(wcs::USER, luid, source);
            seen.push(s);
        }
        let begun = Instant::now();
        loop {
            let mut waiting = false;
            for ((s, ask), name) in seen.iter_mut().zip(asks).zip(&names) {
                let Some(e) = ask.expect.filter(|_| s.associated) else { continue };
                let d = name.as_deref().and_then(desc);
                s.luminance = d.as_ref().map(|d| d.luminance);
                s.red_primary = d.as_ref().map(|d| d.red);
                let ok = s.luminance.is_some_and(|l| super::desc_matches(&l, e.max));
                waiting |= !ok && begun.elapsed() < Duration::from_millis(wait_ms);
            }
            if !waiting {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(json!({ "screens": seen }))
    }

    /// `display-agent hdr-primary` : le HDR de l'écran principal du PC
    /// (session sans écran virtuel), lu sans rien changer.
    pub fn agent_primary() -> Result<Value> {
        let (paths, _) = topology::active_config()?;
        for path in &paths {
            let Some(name) = topology::gdi_name(path).filter(|n| !n.is_empty()) else { continue };
            if topology::current_position(&name) != (0, 0) {
                continue;
            }
            let t = &path.targetInfo;
            let c = Created { luid_low: t.adapterId.LowPart, luid_high: t.adapterId.HighPart, target: t.id };
            let state = read_state(&c)?;
            let d = dxgi(&name);
            let seen = HdrSeen {
                read: true,
                supported: state.supported,
                before: state.active,
                active: state.active,
                luminance: d.map(|(l, _)| l),
                sdr_white: sdr_white(&c),
                gdi_name: Some(name),
                ..HdrSeen::default()
            };
            return Ok(serde_json::to_value(seen)?);
        }
        bail!("écran principal introuvable")
    }
}
