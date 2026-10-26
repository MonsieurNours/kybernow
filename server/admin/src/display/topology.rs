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
//!
//! Multi-écrans (`isolate_many`) : même principe pour plusieurs écrans
//! virtuels, placés aux positions demandées (le principal en (0, 0)), avec
//! leur entrée à eux dans la base.

use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use windows_sys::Win32::Devices::Display::{
    DISPLAYCONFIG_ADAPTER_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
    DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ALL_PATHS,
    QDC_ONLY_ACTIVE_PATHS, QUERY_DISPLAY_CONFIG_FLAGS, QueryDisplayConfig, SDC_ALLOW_CHANGES,
    SDC_ALLOW_PATH_ORDER_CHANGES, SDC_APPLY, SDC_SAVE_TO_DATABASE, SDC_TOPOLOGY_EXTEND, SDC_TOPOLOGY_SUPPLIED,
    SDC_USE_DATABASE_CURRENT, SDC_USE_SUPPLIED_DISPLAY_CONFIG, SetDisplayConfig,
};
use windows_sys::Win32::Foundation::POINTL;
use windows_sys::Win32::Graphics::Gdi::{
    CDS_NORESET, CDS_SET_PRIMARY, CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW, DISP_CHANGE_SUCCESSFUL,
    DISPLAYCONFIG_PATH_ACTIVE, DISPLAYCONFIG_PATH_MODE_IDX_INVALID, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH,
    DM_POSITION, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW,
};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Created, Placement};

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
    query(QDC_ONLY_ACTIVE_PATHS)
}

/// Tous les chemins que Windows connaît (actifs ou non) : un par couple
/// source-cible possible, les cibles débranchées comprises.
fn all_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>> {
    query(QDC_ALL_PATHS).map(|(paths, _)| paths)
}

fn query(flags: QUERY_DISPLAY_CONFIG_FLAGS) -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>)> {
    for _ in 0..4 {
        let (mut np, mut nm) = (0u32, 0u32);
        let err = unsafe { GetDisplayConfigBufferSizes(flags, &mut np, &mut nm) };
        if err != 0 {
            bail!("lecture de la configuration d'affichage (code {err})");
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        let err = unsafe { QueryDisplayConfig(flags, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), null_mut()) };
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

/// Pousse Windows à activer la cible de l'écran virtuel quand il ne le fait
/// pas seul : extension du bureau au premier essai, puis, si la cible figure
/// parmi les chemins qu'il connaît sans être active, disposition fournie avec
/// ce seul chemin (les écrans physiques s'éteignent du même coup, mode choisi
/// par Windows). Rend ce qui a été vu et tenté, pour le message d'erreur si
/// rien n'y fait : il dit si la cible est absente ou présente mais
/// inactive, et ce que Windows a répondu.
fn activate(luid_low: u32, luid_high: i32, target: u32, attempt: u32, forced: &mut bool) -> String {
    let extend = || unsafe { SetDisplayConfig(0, null(), 0, null(), SDC_APPLY | SDC_TOPOLOGY_EXTEND) };
    if attempt == 0 {
        return format!("extension du bureau : code {}", extend());
    }
    let known = match all_paths() {
        Ok(paths) => paths,
        Err(e) => return format!("{e:#}"),
    };
    let found = known
        .iter()
        .find(|p| is_target(p, luid_low, luid_high, target) && p.targetInfo.targetAvailable != 0)
        .copied();
    let Some(mut path) = found else {
        return format!("cible absente des {} chemins connus de Windows ; extension du bureau : code {}", known.len(), extend());
    };
    path.flags = DISPLAYCONFIG_PATH_ACTIVE;
    path.sourceInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
    path.targetInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
    let err = unsafe { SetDisplayConfig(1, &path, 0, null(), SDC_APPLY | SDC_TOPOLOGY_SUPPLIED | SDC_ALLOW_PATH_ORDER_CHANGES) };
    if err == 0 {
        *forced = true;
    }
    format!("cible connue de Windows mais inactive ; disposition fournie avec ce seul chemin : code {err}")
}

/// Ne garde actif que l'écran virtuel (adaptateur + cible), au mode demandé.
pub fn isolate(luid_low: u32, luid_high: i32, target: u32, width: u32, height: u32, refresh: u32) -> Result<Applied> {
    // 1. Attendre que Windows active le nouvel écran. D'ordinaire il le fait
    //    seul : bureau étendu à son arrivée, ou disposition « écran virtuel
    //    seul » enregistrée par une session précédente. Sinon on l'y aide
    //    (`activate`), une fois par seconde jusqu'à l'échéance.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut next_attempt = Instant::now();
    let mut attempts = 0u32;
    let mut forced = false;
    let mut others_before = None;
    let mut seen = String::new();
    let (paths, modes, vd) = loop {
        let (paths, modes) = active_config()?;
        if let Some(i) = paths.iter().position(|p| is_target(p, luid_low, luid_high, target)) {
            break (paths, modes, i);
        }
        others_before.get_or_insert(paths.len());
        let now = Instant::now();
        if now > deadline {
            bail!("l'écran virtuel n'apparaît pas dans la configuration d'affichage ({seen})");
        }
        if now >= next_attempt {
            seen = activate(luid_low, luid_high, target, attempts, &mut forced);
            attempts += 1;
            next_attempt = now + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(250));
    };

    // 2. Ne fournir à Windows que le chemin de l'écran virtuel : les autres sont
    //    désactivés. Si c'est `activate` qui l'a allumé seul, les autres le
    //    sont déjà, mais la disposition reste à enregistrer.
    let disabled_others = if forced { others_before.unwrap_or(0) } else { paths.len() - 1 };
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
    if disabled_others > 0 || forced {
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
        let r = ChangeDisplaySettingsExW(wname.as_ptr(), &mode, null_mut(), CDS_UPDATEREGISTRY, null());
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

/// Ce que Windows affiche en ce moment : mode de l'écran virtuel principal
/// (0 × 0 s'il n'est plus actif), nombre d'écrans actifs, et combien d'entre
/// eux ne sont pas des écrans virtuels de la session (des écrans physiques
/// rallumés pendant la session) ; `extra` : les autres écrans virtuels.
#[derive(Debug, Serialize)]
pub struct Seen {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub gdi_name: String,
    pub displays: u32,
    pub others: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<SeenScreen>,
}

/// Mode courant d'un autre écran virtuel de la session (0 × 0 s'il n'est plus actif).
#[derive(Debug, Default, Serialize)]
pub struct SeenScreen {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub gdi_name: String,
}

/// `targets` : les écrans virtuels de la session, le principal en premier.
pub fn observe(targets: &[Created]) -> Result<Seen> {
    let (paths, _) = active_config()?;
    let mut ours = 0u32;
    let mut screens: Vec<SeenScreen> = targets
        .iter()
        .map(|c| {
            let vd = paths.iter().find(|p| is_target(p, c.luid_low, c.luid_high, c.target));
            ours += u32::from(vd.is_some());
            let gdi_name = vd.and_then(gdi_name).unwrap_or_default();
            let (width, height, refresh) = if gdi_name.is_empty() { (0, 0, 0) } else { current_mode(&gdi_name) };
            SeenScreen { width, height, refresh, gdi_name }
        })
        .collect();
    let primary = if screens.is_empty() { SeenScreen::default() } else { screens.remove(0) };
    let displays = paths.len() as u32;
    Ok(Seen {
        width: primary.width,
        height: primary.height,
        refresh: primary.refresh,
        gdi_name: primary.gdi_name,
        displays,
        others: displays.saturating_sub(ours),
        extra: screens,
    })
}

fn is_placed(path: &DISPLAYCONFIG_PATH_INFO, p: &Placement) -> bool {
    is_target(path, p.created.luid_low, p.created.luid_high, p.created.target)
}

fn same_source(a: &DISPLAYCONFIG_PATH_INFO, b: &DISPLAYCONFIG_PATH_INFO) -> bool {
    a.sourceInfo.adapterId.LowPart == b.sourceInfo.adapterId.LowPart
        && a.sourceInfo.adapterId.HighPart == b.sourceInfo.adapterId.HighPart
        && a.sourceInfo.id == b.sourceInfo.id
}

/// `activate` pour plusieurs écrans : extension du bureau au premier essai,
/// puis disposition fournie avec un chemin par cible connue de Windows,
/// chacun sur sa propre source.
fn activate_many(screens: &[Placement], attempt: u32, forced: &mut bool) -> String {
    let extend = || unsafe { SetDisplayConfig(0, null(), 0, null(), SDC_APPLY | SDC_TOPOLOGY_EXTEND) };
    if attempt == 0 {
        return format!("extension du bureau : code {}", extend());
    }
    let known = match all_paths() {
        Ok(paths) => paths,
        Err(e) => return format!("{e:#}"),
    };
    let mut chosen: Vec<DISPLAYCONFIG_PATH_INFO> = Vec::new();
    for p in screens {
        let found = known
            .iter()
            .find(|k| is_placed(k, p) && k.targetInfo.targetAvailable != 0 && !chosen.iter().any(|c| same_source(c, k)))
            .copied();
        if let Some(mut path) = found {
            path.flags = DISPLAYCONFIG_PATH_ACTIVE;
            path.sourceInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
            path.targetInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
            chosen.push(path);
        }
    }
    if chosen.is_empty() {
        return format!("cibles absentes des {} chemins connus de Windows ; extension du bureau : code {}", known.len(), extend());
    }
    let err = unsafe {
        SetDisplayConfig(chosen.len() as u32, chosen.as_ptr(), 0, null(), SDC_APPLY | SDC_TOPOLOGY_SUPPLIED | SDC_ALLOW_PATH_ORDER_CHANGES)
    };
    if err == 0 {
        *forced = true;
    }
    format!("{} cible(s) sur {} connues de Windows mais inactives ; disposition fournie avec ces chemins : code {err}", chosen.len(), screens.len())
}

/// Taille de la source d'un chemin actif, d'après ses modes.
fn source_size(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> Option<(u32, u32)> {
    let idx = unsafe { path.sourceInfo.Anonymous.modeInfoIdx } as usize;
    let mode = modes.get(idx).filter(|m| m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE)?;
    let source = unsafe { mode.Anonymous.sourceMode };
    Some((source.width, source.height))
}

/// Ne garde actifs que les écrans virtuels `screens` (le principal en
/// premier), chacun à son mode et à sa place. Un écran que Windows n'active
/// pas dans les 8 s est laissé de côté (null dans `screens`) ; le principal,
/// lui, est indispensable. Rend, comme `isolate`, le mode du principal, et
/// dans `screens` ce qui est en place pour chacun (mode et position relus).
pub fn isolate_many(screens: &[Placement]) -> Result<Value> {
    let Some(primary) = screens.first() else { bail!("aucun écran virtuel à placer") };
    let is_ours = |path: &DISPLAYCONFIG_PATH_INFO| screens.iter().any(|p| is_placed(path, p));

    // 1. Attendre que Windows active les nouveaux écrans (en l'y aidant au besoin).
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut next_attempt = Instant::now();
    let mut attempts = 0u32;
    let mut forced = false;
    let mut others_before = None;
    let mut seen = String::new();
    let (paths, modes) = loop {
        let (paths, modes) = active_config()?;
        let found = screens.iter().filter(|p| paths.iter().any(|q| is_placed(q, p))).count();
        if found == screens.len() {
            break (paths, modes);
        }
        others_before.get_or_insert(paths.iter().filter(|q| !is_ours(q)).count());
        let now = Instant::now();
        if now > deadline {
            if paths.iter().any(|q| is_placed(q, primary)) {
                tracing::warn!("{found} écran(s) virtuel(s) sur {} activé(s) par Windows ({seen})", screens.len());
                break (paths, modes);
            }
            bail!("l'écran virtuel n'apparaît pas dans la configuration d'affichage ({seen})");
        }
        if now >= next_attempt {
            seen = activate_many(screens, attempts, &mut forced);
            attempts += 1;
            next_attempt = now + Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(250));
    };

    // 2. Ne fournir à Windows que les chemins des écrans virtuels : les autres
    //    sont désactivés. Places demandées si les tailles y sont déjà, sinon
    //    en rang provisoire (des tailles qui changent encore se
    //    chevaucheraient) ; l'étape 4 les met à leur place.
    let ours: Vec<(usize, DISPLAYCONFIG_PATH_INFO)> = screens
        .iter()
        .enumerate()
        .filter_map(|(i, p)| paths.iter().find(|q| is_placed(q, p)).map(|q| (i, *q)))
        .collect();
    let others = paths.len() - ours.len();
    let disabled_others = if forced { others_before.unwrap_or(others) } else { others };
    let sized = ours
        .iter()
        .all(|(i, path)| source_size(path, &modes) == Some((screens[*i].mode.width, screens[*i].mode.height)));
    let mut new_paths = Vec::new();
    let mut new_modes = Vec::new();
    let mut next_x = 0i32;
    for (i, path) in &ours {
        let p = &screens[*i];
        let mut path = *path;
        let width = source_size(&path, &modes).map_or(p.mode.width, |(w, _)| w);
        let (x, y) = if sized {
            (p.x, p.y)
        } else {
            let x = next_x;
            next_x += width as i32;
            (x, 0)
        };
        unsafe {
            for idx in [&mut path.sourceInfo.Anonymous.modeInfoIdx, &mut path.targetInfo.Anonymous.modeInfoIdx] {
                if (*idx as usize) < modes.len() {
                    let mut mode = modes[*idx as usize];
                    if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                        mode.Anonymous.sourceMode.position.x = x;
                        mode.Anonymous.sourceMode.position.y = y;
                    }
                    *idx = new_modes.len() as u32;
                    new_modes.push(mode);
                }
            }
        }
        new_paths.push(path);
    }
    let err = unsafe {
        SetDisplayConfig(
            new_paths.len() as u32,
            new_paths.as_ptr(),
            new_modes.len() as u32,
            new_modes.as_ptr(),
            SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES | SDC_SAVE_TO_DATABASE,
        )
    };
    if err != 0 {
        bail!("désactivation des écrans physiques et placement des écrans virtuels (code {err})");
    }

    // 3. Attendre que Windows n'affiche plus que les écrans virtuels.
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        let (paths, _) = active_config()?;
        if paths.len() == ours.len() && paths.iter().all(is_ours) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // 4. Modes et places demandés, appliqués d'un coup (Windows vérifie la
    //    disposition entière) ; le principal en (0, 0) devient l'écran
    //    principal. Ce qui ne passe pas est noté (`notes`) : l'agent n'a pas
    //    de journal, Admin écrit ces notes dans le sien.
    let mut notes: Vec<String> = Vec::new();
    let (paths, _) = active_config()?;
    let names: Vec<Option<String>> = screens
        .iter()
        .map(|p| paths.iter().find(|q| is_placed(q, p)).and_then(gdi_name).filter(|n| !n.is_empty()))
        .collect();
    let same = |name: &str, p: &Placement| {
        let (w, h, hz) = current_mode(name);
        (w, h) == (p.mode.width, p.mode.height) && hz.abs_diff(p.mode.refresh) <= 1
    };
    let mut changed = false;
    for (i, (p, name)) in screens.iter().zip(&names).enumerate() {
        let Some(name) = name else { continue };
        if !same(name, p) || current_position(name) != (p.x, p.y) {
            let r = stage(name, p, i == 0);
            if r != DISP_CHANGE_SUCCESSFUL {
                notes.push(format!("{name} : {} en ({}, {}) refusé (code {r})", label(p), p.x, p.y));
            }
            changed = true;
        }
    }
    if changed {
        let r = unsafe { ChangeDisplaySettingsExW(null(), null(), null_mut(), 0, null()) };
        if r != DISP_CHANGE_SUCCESSFUL {
            notes.push(format!("disposition des écrans virtuels refusée (code {r}) : modes un par un"));
        }
        // Mode refusé dans la disposition : on le réessaie seul.
        for (p, name) in screens.iter().zip(&names) {
            let Some(name) = name else { continue };
            if !same(name, p) {
                set_mode(name, p.mode.width, p.mode.height, p.mode.refresh);
            }
        }
    }

    // 5. Un écran resté à un autre mode (Windows l'impose) : on dit pourquoi,
    //    et il reste collé à son voisin, du côté demandé (un écran à gauche
    //    du principal garde son bord droit, un écran au-dessus son bord bas).
    let mut glued = false;
    for (p, name) in screens.iter().zip(&names) {
        let Some(name) = name else { continue };
        if same(name, p) {
            continue;
        }
        let (w, h, hz) = current_mode(name);
        notes.push(format!("{name} : {} demandé, {w} × {h} à {hz} Hz obtenu ; {}", label(p), offered(name, p)));
        let at = p.anchored(w, h);
        if current_position(name) != at {
            let r = stage_position(name, at);
            if r != DISP_CHANGE_SUCCESSFUL {
                notes.push(format!("{name} : place ({}, {}) refusée (code {r})", at.0, at.1));
            }
            glued = true;
        }
    }
    if glued {
        let r = unsafe { ChangeDisplaySettingsExW(null(), null(), null_mut(), 0, null()) };
        if r != DISP_CHANGE_SUCCESSFUL {
            notes.push(format!("écrans virtuels recollés : refusé (code {r})"));
        }
    }

    // 6. La disposition finale remplace la provisoire dans la base : un écran
    //    qui se « rebranche » retrouve les bonnes places.
    if changed || glued {
        if let Ok((paths, modes)) = active_config() {
            let err = unsafe {
                SetDisplayConfig(
                    paths.len() as u32,
                    paths.as_ptr(),
                    modes.len() as u32,
                    modes.as_ptr(),
                    SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES | SDC_SAVE_TO_DATABASE,
                )
            };
            if err != 0 {
                notes.push(format!("enregistrement de la disposition des écrans virtuels (code {err})"));
            }
        }
    }

    // 7. Comme `isolate` : laisser l'énumérateur d'écrans de Kyber voir la
    //    nouvelle disposition avant de répondre.
    std::thread::sleep(Duration::from_millis(1500));
    // Ce que Windows a mis en place pour chaque écran, échelle comprise
    // (`facts`, écrit par Admin dans son journal pour comparer les écrans).
    let mut facts: Vec<String> = Vec::new();
    let list: Vec<Value> = names
        .iter()
        .map(|name| {
            let Some(name) = name else { return Value::Null };
            let (width, height, refresh) = current_mode(name);
            if width == 0 {
                return Value::Null;
            }
            let (x, y) = current_position(name);
            let scale = scale_at(x, y).map_or("inconnue".to_string(), |s| format!("{s} %"));
            facts.push(format!("{name} : {width} × {height} à {refresh} Hz en ({x}, {y}), échelle {scale}"));
            json!({ "gdi_name": name, "width": width, "height": height, "refresh": refresh, "x": x, "y": y })
        })
        .collect();
    let first = list[0].clone();
    Ok(json!({
        "gdi_name": first["gdi_name"],
        "width": first["width"],
        "height": first["height"],
        "refresh": first["refresh"],
        "disabled_others": disabled_others,
        "screens": list,
        "notes": notes,
        "facts": facts,
    }))
}

/// Échelle (en %) de l'écran dont le coin haut gauche est en (`x`, `y`),
/// lue comme la voit une app qui gère le DPI par écran.
fn scale_at(x: i32, y: i32) -> Option<u32> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint};
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI, SetThreadDpiAwarenessContext,
    };
    unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let monitor = MonitorFromPoint(POINT { x: x + 1, y: y + 1 }, MONITOR_DEFAULTTONULL);
        let (mut dx, mut dy) = (0u32, 0u32);
        let ok = !monitor.is_null() && GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) >= 0;
        if !previous.is_null() {
            SetThreadDpiAwarenessContext(previous);
        }
        (ok && dx > 0).then(|| dx * 100 / 96)
    }
}

/// « 3456 × 2234 à 60 Hz » (notes).
fn label(p: &Placement) -> String {
    format!("{} × {} à {} Hz", p.mode.width, p.mode.height, p.mode.refresh)
}

/// Le mode demandé figure-t-il parmi ceux que Windows propose pour l'écran ?
/// (Pour les notes : pilote qui ne l'offre pas, ou Windows qui le refuse.)
fn offered(name: &str, p: &Placement) -> String {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut count = 0u32;
    let mut found = false;
    let mut largest = (0u32, 0u32, 0u32);
    unsafe {
        loop {
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(wname.as_ptr(), count, &mut mode) == 0 || count > 2000 {
                break;
            }
            count += 1;
            let m = (mode.dmPelsWidth, mode.dmPelsHeight, mode.dmDisplayFrequency);
            if (m.0, m.1) == (p.mode.width, p.mode.height) && m.2.abs_diff(p.mode.refresh) <= 1 {
                found = true;
            }
            if u64::from(m.0) * u64::from(m.1) > u64::from(largest.0) * u64::from(largest.1) {
                largest = m;
            }
        }
    }
    if found {
        format!("le mode est proposé par Windows ({count} modes) mais n'a pas été appliqué")
    } else {
        format!("le mode n'est pas proposé ({count} modes, le plus grand : {} × {} à {} Hz)", largest.0, largest.1, largest.2)
    }
}

/// Prépare (sans l'appliquer) le mode et la place d'un écran ; rend le code de Windows.
fn stage(name: &str, p: &Placement, primary: bool) -> i32 {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = size_of::<DEVMODEW>() as u16;
        mode.dmPelsWidth = p.mode.width;
        mode.dmPelsHeight = p.mode.height;
        mode.dmDisplayFrequency = p.mode.refresh;
        mode.Anonymous1.Anonymous2.dmPosition = POINTL { x: p.x, y: p.y };
        mode.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY | DM_POSITION;
        let flags = CDS_UPDATEREGISTRY | CDS_NORESET | if primary { CDS_SET_PRIMARY } else { 0 };
        ChangeDisplaySettingsExW(wname.as_ptr(), &mode, null_mut(), flags, null())
    }
}

/// Prépare (sans l'appliquer) la seule place d'un écran.
fn stage_position(name: &str, (x, y): (i32, i32)) -> i32 {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut mode: DEVMODEW = std::mem::zeroed();
        mode.dmSize = size_of::<DEVMODEW>() as u16;
        mode.Anonymous1.Anonymous2.dmPosition = POINTL { x, y };
        mode.dmFields = DM_POSITION;
        ChangeDisplaySettingsExW(wname.as_ptr(), &mode, null_mut(), CDS_UPDATEREGISTRY | CDS_NORESET, null())
    }
}

/// Cartes graphiques que la configuration d'affichage connaît : LUID et
/// chemin du périphérique (`\\?\PCI#VEN_10DE&DEV_…` pour une carte NVIDIA ;
/// l'adaptateur de SudoVDA n'est pas sur le bus PCI).
pub fn adapters() -> Result<Value> {
    let paths = all_paths()?;
    let mut seen: Vec<(u32, i32)> = Vec::new();
    let mut list = Vec::new();
    for id in paths.iter().flat_map(|p| [p.sourceInfo.adapterId, p.targetInfo.adapterId]) {
        if seen.contains(&(id.LowPart, id.HighPart)) {
            continue;
        }
        seen.push((id.LowPart, id.HighPart));
        let mut name = DISPLAYCONFIG_ADAPTER_NAME::default();
        name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME;
        name.header.size = size_of::<DISPLAYCONFIG_ADAPTER_NAME>() as u32;
        name.header.adapterId = id;
        let path = if unsafe { DisplayConfigGetDeviceInfo(&mut name.header) } == 0 {
            let len = name.adapterDevicePath.iter().position(|c| *c == 0).unwrap_or(name.adapterDevicePath.len());
            String::from_utf16_lossy(&name.adapterDevicePath[..len])
        } else {
            String::new()
        };
        list.push(json!({ "luid_low": id.LowPart, "luid_high": id.HighPart, "path": path }));
    }
    Ok(json!({ "adapters": list }))
}

/// Position actuelle de l'écran `name` (nom GDI) sur le bureau.
fn current_position(name: &str) -> (i32, i32) {
    let wname: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut current: DEVMODEW = std::mem::zeroed();
        current.dmSize = size_of::<DEVMODEW>() as u16;
        if EnumDisplaySettingsW(wname.as_ptr(), ENUM_CURRENT_SETTINGS, &mut current) == 0 {
            return (0, 0);
        }
        let p = current.Anonymous1.Anonymous2.dmPosition;
        (p.x, p.y)
    }
}

/// Réapplique la disposition habituelle des écrans physiques.
pub fn restore() -> Result<()> {
    let err = unsafe { SetDisplayConfig(0, null(), 0, null(), SDC_APPLY | SDC_USE_DATABASE_CURRENT) };
    if err == 0 {
        return Ok(());
    }
    // Pas de disposition connue pour ces écrans : on les étend tous.
    let err2 = unsafe { SetDisplayConfig(0, null(), 0, null(), SDC_APPLY | SDC_TOPOLOGY_EXTEND) };
    if err2 != 0 {
        bail!("restauration des écrans (codes {err} puis {err2})");
    }
    Ok(())
}
