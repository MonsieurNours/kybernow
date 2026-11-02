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
//! Comme la disposition des écrans, la lecture et la bascule se font dans la
//! session de l'écran, par l'agent d'affichage (`display-agent hdr`, voir
//! `agent.rs`).

use serde::{Deserialize, Serialize};

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
}

/// `luid_low,luid_high,target,0|1` : un écran et le HDR voulu (`display-agent hdr --screen`).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn screen_arg(created: &super::Created, on: bool) -> String {
    format!("{},{}", created.to_arg(), u8::from(on))
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_screen_arg(arg: &str) -> anyhow::Result<(super::Created, bool)> {
    let [low, high, target, on] = super::numbers::<4>(arg)?;
    let created = super::Created {
        luid_low: super::narrow(low, arg)?,
        luid_high: super::narrow(high, arg)?,
        target: super::narrow(target, arg)?,
    };
    Ok((created, on != 0))
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

    use super::{HdrSeen, Luminance};
    use crate::display::Created;
    use crate::display::topology;

    // Windows 11 24H2 : absents de windows-sys 0.61.
    const GET_ADVANCED_COLOR_INFO_2: i32 = 15;
    const SET_HDR_STATE: i32 = 16;
    /// `DISPLAYCONFIG_ADVANCED_COLOR_MODE_HDR`.
    const COLOR_MODE_HDR: i32 = 2;
    /// Attente de la confirmation de Windows.
    const CONFIRM: Duration = Duration::from_secs(2);

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
    pub fn agent_set(screens: &[(Created, bool)]) -> Result<Value> {
        let begun = Instant::now();
        let mut seen: Vec<HdrSeen> = Vec::with_capacity(screens.len());
        for (c, want) in screens {
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
