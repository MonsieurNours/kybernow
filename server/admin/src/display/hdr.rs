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
    /// Luminosité maximale du profil : DXGI est attendu jusqu'à la rendre.
    pub max: f32,
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
pub use win::{agent_primary, agent_set};

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

    use super::{ColorApply, ColorOriginal, ColorSeen, HdrAsk, HdrSeen, Luminance};
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
            InstallColorProfileW, UninstallColorProfileW, WCS_PROFILE_MANAGEMENT_SCOPE_SYSTEM_WIDE as SCOPE,
        };

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
        pub fn default_profile(luid: LUID, source: u32) -> Option<String> {
            let mut name: *mut u16 = std::ptr::null_mut();
            let hr = unsafe { ColorProfileGetDisplayDefault(SCOPE, luid, source, CPT_ICC, CPST_EXTENDED_DISPLAY_COLOR_MODE, &mut name) };
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
        pub fn associate(name: &str, luid: LUID, source: u32) -> Result<()> {
            check(unsafe { ColorProfileAddDisplayAssociation(SCOPE, wide(name).as_ptr(), luid, source, 1, 1) }, "association du profil")
        }

        pub fn dissociate(name: &str, luid: LUID, source: u32) -> Result<()> {
            check(unsafe { ColorProfileRemoveDisplayAssociation(SCOPE, wide(name).as_ptr(), luid, source, 1) }, "retrait de l'association")
        }

        pub fn set_default(name: &str, luid: LUID, source: u32) -> Result<()> {
            check(
                unsafe { ColorProfileSetDisplayDefaultAssociation(SCOPE, wide(name).as_ptr(), CPT_ICC, CPST_EXTENDED_DISPLAY_COLOR_MODE, luid, source) },
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
        seen.original_profile = wcs::default_profile(luid, source);
        let mut errors = Vec::new();
        if let Some(old) = &apply.replace {
            let _ = wcs::dissociate(old, luid, source);
            let _ = wcs::uninstall(old);
        }
        match wcs::install(&apply.profile).and_then(|name| wcs::associate(&name, luid, source).map(|_| name)) {
            Ok(name) => seen.installed = Some(name),
            Err(e) => errors.push(format!("{e:#}")),
        }
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
        if let Err(e) = wcs::dissociate(&original.installed, luid, source) {
            errors.push(format!("{e:#}"));
        }
        if let Some(profile) = &original.profile
            && let Err(e) = wcs::set_default(profile, luid, source)
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
                        let l = Luminance { min: desc.MinLuminance, max: desc.MaxLuminance, max_full_frame: desc.MaxFullFrameLuminance };
                        return Some((l, desc.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020));
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
                    if state.active != *want {
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
        // Profil et blanc sur les écrans en HDR, puis la luminosité que
        // DXGI rend ensuite (attendue 1 s au plus : ce que liront les jeux).
        let mut expected: Vec<Option<f32>> = vec![None; asks.len()];
        for (i, (ask, restored)) in asks.iter().zip(restored).enumerate() {
            let s = &mut seen[i];
            s.color = restored;
            if let Some(apply) = ask.apply.as_ref().filter(|_| ask.on && s.active) {
                let color = apply_color(&ask.created, apply);
                if color.installed.is_some() {
                    expected[i] = Some(apply.max);
                }
                s.sdr_white = color.white.or(s.sdr_white);
                s.color = Some(color);
            }
        }
        if expected.iter().any(Option::is_some) {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                let mut waiting = false;
                for (s, max) in seen.iter_mut().zip(&expected) {
                    let Some(max) = max else { continue };
                    s.luminance = s.gdi_name.as_deref().and_then(dxgi).map(|(l, _)| l);
                    waiting |= s.luminance.is_none_or(|l| (l.max - max).abs() > 1.0);
                }
                if !waiting || Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
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
