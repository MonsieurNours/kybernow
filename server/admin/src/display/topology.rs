//! Disposition des écrans (API « CCD » de Windows).
//!
//! Ces fonctions doivent s'exécuter dans la session de l'écran (la console
//! Windows), pas dans la session 0 des services : voir `agent.rs`.
//!
//! La disposition de session est enregistrée dans la base de configuration
//! d'affichage de Windows (`SDC_SAVE_TO_DATABASE`), comme le font Sunshine et
//! Apollo. Cette base est indexée par l'ensemble des écrans branchés : l'entrée
//! « écrans physiques + écran virtuel » devient « écran virtuel seul ». Sans
//! cela, dès qu'un écran éteint se met en veille et se « rebranche » (fréquent
//! en DisplayPort), Windows réapplique l'ancienne disposition étendue : écrans
//! physiques rallumés, écran virtuel à son mode par défaut, image instable.
//! L'entrée « écrans physiques seuls » n'est jamais touchée : une fois l'écran
//! virtuel retiré, on la réapplique pour restaurer.

use std::ptr::null_mut;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use windows_sys::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig, SDC_ALLOW_CHANGES, SDC_APPLY, SDC_SAVE_TO_DATABASE, SDC_TOPOLOGY_EXTEND,
    SDC_USE_DATABASE_CURRENT,
    SDC_USE_SUPPLIED_DISPLAY_CONFIG, SetDisplayConfig,
};
use windows_sys::Win32::Graphics::Gdi::{
    CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH,
    ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Applied {
    pub gdi_name: String,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// Écrans physiques désactivés.
    pub disabled_others: usize,
}

fn active_config() -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>)> {
    for _ in 0..4 {
        let (mut np, mut nm) = (0u32, 0u32);
        let err = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
        if err != 0 {
            bail!("lecture de la configuration d'affichage (code {err})");
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        let err = unsafe {
            QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), null_mut())
        };
        // ERROR_INSUFFICIENT_BUFFER (122) : la configuration a changé entre-temps.
        if err == 122 {
            continue;
        }
        if err != 0 {
            bail!("lecture de la configuration d'affichage (code {err})");
        }
        paths.truncate(np as usize);
        modes.truncate(nm as usize);
        return Ok((paths, modes));
    }
    bail!("la configuration d'affichage change sans arrêt")
}

fn is_target(path: &DISPLAYCONFIG_PATH_INFO, luid_low: u32, luid_high: i32, target: u32) -> bool {
    let t = &path.targetInfo;
    t.adapterId.LowPart == luid_low && t.adapterId.HighPart == luid_high && t.id == target
}

fn gdi_name(path: &DISPLAYCONFIG_PATH_INFO) -> Option<String> {
    let mut name = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
    name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
    name.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
    name.header.adapterId = path.sourceInfo.adapterId;
    name.header.id = path.sourceInfo.id;
    if unsafe { DisplayConfigGetDeviceInfo(&mut name.header) } != 0 {
        return None;
    }
    let len = name.viewGdiDeviceName.iter().position(|c| *c == 0).unwrap_or(32);
    Some(String::from_utf16_lossy(&name.viewGdiDeviceName[..len]))
}

/// Ne garde actif que l'écran virtuel (adaptateur + cible), au mode demandé.
pub fn isolate(luid_low: u32, luid_high: i32, target: u32, width: u32, height: u32, refresh: u32) -> Result<Applied> {
    // 1. Attendre que Windows active le nouvel écran (en mode étendu par défaut).
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut extended = false;
    let (paths, modes, vd) = loop {
        let (paths, modes) = active_config()?;
        if let Some(i) = paths.iter().position(|p| is_target(p, luid_low, luid_high, target)) {
            break (paths, modes, i);
        }
        if !extended && Instant::now() + Duration::from_secs(5) < deadline {
            // Il est branché mais pas activé : on étend le bureau à tous les écrans.
            unsafe { SetDisplayConfig(0, std::ptr::null(), 0, std::ptr::null(), SDC_APPLY | SDC_TOPOLOGY_EXTEND) };
            extended = true;
        }
        if Instant::now() > deadline {
            bail!("l'écran virtuel n'apparaît pas dans la configuration d'affichage");
        }
        std::thread::sleep(Duration::from_millis(250));
    };

    // 2. Ne fournir à Windows que le chemin de l'écran virtuel : les autres sont désactivés.
    let disabled_others = paths.len() - 1;
    let mut path = paths[vd];
    let mut new_modes = Vec::new();
    unsafe {
        for idx in [&mut path.sourceInfo.Anonymous.modeInfoIdx, &mut path.targetInfo.Anonymous.modeInfoIdx] {
            if (*idx as usize) < modes.len() {
                let mut mode = modes[*idx as usize];
                if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                    // L'écran virtuel devient l'écran principal, en haut à gauche.
                    mode.Anonymous.sourceMode.position.x = 0;
                    mode.Anonymous.sourceMode.position.y = 0;
                }
                *idx = new_modes.len() as u32;
                new_modes.push(mode);
            }
        }
    }
    if disabled_others > 0 {
        let err = unsafe {
            SetDisplayConfig(
                1,
                &path,
                new_modes.len() as u32,
                new_modes.as_ptr(),
                SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES | SDC_SAVE_TO_DATABASE,
            )
        };
        if err != 0 {
            bail!("désactivation des écrans physiques (code {err})");
        }
    }

    // 3. Attendre que Windows n'affiche plus que l'écran virtuel.
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        let (paths, _) = active_config()?;
        if paths.len() == 1 && is_target(&paths[0], luid_low, luid_high, target) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // 4. Imposer la résolution et la fréquence demandées.
    let (paths, _) = active_config()?;
    let vd_path = paths
        .iter()
        .find(|p| is_target(p, luid_low, luid_high, target))
        .copied()
        .unwrap_or(path);
    let name = gdi_name(&vd_path).unwrap_or_default();
    let (w, h, hz) = set_mode(&name, width, height, refresh);

    // 5. Laisser le reste du système voir la nouvelle disposition. Sans ce délai,
    //    l'énumérateur d'écrans de Kyber, lancé à la connexion du client, voit
    //    encore l'ancienne pendant quelques dizaines de millisecondes, et le
    //    client choisit un écran qui n'existe plus (refus 403).
    std::thread::sleep(Duration::from_millis(1500));
    Ok(Applied { gdi_name: name, width: w, height: h, refresh: hz, disabled_others })
}

/// Impose un mode à l'écran `name` (nom GDI) et rend celui qu'il a ensuite.
/// Le mode est enregistré lui aussi, pour que Windows le réapplique tel quel.
pub fn set_mode(name: &str, width: u32, height: u32, refresh: u32) -> (u32, u32, u32) {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = size_of::<DEVMODEW>() as u16;
        mode.dmPelsWidth = width;
        mode.dmPelsHeight = height;
        mode.dmDisplayFrequency = refresh;
        mode.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
        let r = ChangeDisplaySettingsExW(wname.as_ptr(), &mode, null_mut(), CDS_UPDATEREGISTRY, std::ptr::null());
        if r != DISP_CHANGE_SUCCESSFUL {
            tracing::warn!("changement de mode refusé ({r}) : le mode préféré de l'écran est conservé");
        }
    }
    current_mode(name)
}

/// Mode actuel de l'écran `name` (nom GDI) : 0 × 0 s'il n'est pas actif.
fn current_mode(name: &str) -> (u32, u32, u32) {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut current: DEVMODEW = std::mem::zeroed();
        current.dmSize = size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(wname.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current) == 0 {
            return (0, 0, 0);
        }
        (current.dmPelsWidth, current.dmPelsHeight, current.dmDisplayFrequency)
    }
}

/// Ce que Windows affiche en ce moment : mode de l'écran virtuel (0 × 0 s'il
/// n'est plus actif), nombre d'écrans actifs, et combien d'entre eux ne sont
/// pas l'écran virtuel (des écrans physiques rallumés pendant la session).
#[derive(Debug, Serialize)]
pub struct Seen {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub gdi_name: String,
    pub displays: u32,
    pub others: u32,
}

pub fn observe(luid_low: u32, luid_high: i32, target: u32) -> Result<Seen> {
    let (paths, _) = active_config()?;
    let vd = paths.iter().find(|p| is_target(p, luid_low, luid_high, target));
    let name = vd.and_then(gdi_name).unwrap_or_default();
    let (width, height, refresh) = if name.is_empty() { (0, 0, 0) } else { current_mode(&name) };
    let displays = paths.len() as u32;
    Ok(Seen { width, height, refresh, gdi_name: name, displays, others: displays - u32::from(vd.is_some()) })
}

/// Réapplique la disposition habituelle des écrans physiques.
pub fn restore() -> Result<()> {
    let err = unsafe { SetDisplayConfig(0, std::ptr::null(), 0, std::ptr::null(), SDC_APPLY | SDC_USE_DATABASE_CURRENT) };
    if err == 0 {
        return Ok(());
    }
    // Pas de disposition connue pour ces écrans : on les étend tous.
    let err2 = unsafe { SetDisplayConfig(0, std::ptr::null(), 0, std::ptr::null(), SDC_APPLY | SDC_TOPOLOGY_EXTEND) };
    if err2 != 0 {
        bail!("restauration des écrans (codes {err} puis {err2})");
    }
    Ok(())
}
