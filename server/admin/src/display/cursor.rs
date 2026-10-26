//! Taille du pointeur de Windows pendant une session à plusieurs écrans.
//!
//! SudoVDA déclare pour chaque écran un curseur matériel de 64 × 64 au plus,
//! avec un même événement pour tous ses écrans : sur les écrans créés après
//! le premier, chaque mouvement du pointeur fait perdre la capture (DXGI
//! `ACCESS_LOST`, écran qui clignote). Un pointeur plus grand que 64 pixels
//! réels ne peut plus être un curseur matériel : Windows le dessine lui-même,
//! et la capture tient. L'agent agrandit donc le pointeur juste assez le
//! temps de la session, comme le fait la page Accessibilité › Pointeur de
//! la souris de Windows, puis le rend.
//!
//! Application en direct : d'abord l'action non documentée 0x2029 de
//! `SystemParametersInfo` (taille en pixels, celle qu'utilisent la page des
//! Paramètres et les scripts de la communauté) ; à défaut, le registre puis
//! `SPI_SETCURSORS`. `GetSystemMetrics(SM_CXCURSOR)` donne toujours la taille
//! par défaut : la taille est relue dans le registre et sur le pointeur
//! affiché (bitmap du curseur courant).
//!
//! Le réglage est celui du profil de l'utilisateur (`HKCU`) : ces fonctions
//! tournent dans l'agent lancé avec son compte (`agent::run_as_user`), et
//! c'est Admin qui garde la taille d'origine (le dossier de données lui est
//! fermé, à lui).

use std::ptr::null_mut;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use windows_sys::Win32::Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CURSORINFO, GetCursorInfo, GetIconInfo, ICONINFO, SPI_SETCURSORS, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    SystemParametersInfoW,
};

const CURSORS: &str = r"Control Panel\Cursors";
const ACCESSIBILITY: &str = r"Software\Microsoft\Accessibility";
/// Action non documentée de `SystemParametersInfo` : taille du pointeur en pixels (32, 48, 64…).
const SPI_SETCURSORBASESIZE: u32 = 0x2029;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_dword(key: &str, name: &str) -> Option<u32> {
    let (key, name) = (wide(key), wide(name));
    let mut value = 0u32;
    let mut size = 4u32;
    let err = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), RRF_RT_REG_DWORD, null_mut(), &mut value as *mut u32 as *mut _, &mut size)
    };
    (err == 0).then_some(value)
}

/// Écrit `value`, ou retire la valeur si `None` (elle n'existait pas).
fn write_dword(key: &str, name: &str, value: Option<u32>) -> Result<()> {
    let (wkey, wname) = (wide(key), wide(name));
    let err = unsafe {
        match value {
            Some(v) => RegSetKeyValueW(HKEY_CURRENT_USER, wkey.as_ptr(), wname.as_ptr(), REG_DWORD, &v as *const u32 as *const _, 4),
            None => RegDeleteKeyValueW(HKEY_CURRENT_USER, wkey.as_ptr(), wname.as_ptr()),
        }
    };
    // Valeur déjà absente : rien à retirer.
    if err != 0 && !(value.is_none() && err == 2) {
        bail!("{key}\\{name} : écriture refusée (code {err})");
    }
    Ok(())
}

/// Taille de la flèche du système telle qu'elle est dessinée en ce moment :
/// largeur de son image en pixels réels, échelle (en %) de l'écran où se
/// trouve le pointeur, et la taille logique qui en découle (pixels réels ×
/// 100 / échelle, à comparer à la base : 32, 48…).
fn shown() -> Option<(u32, u32, u32)> {
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
    use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
    use windows_sys::Win32::UI::WindowsAndMessaging::{IDC_ARROW, LoadCursorW};
    unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mut result = None;
        let arrow = LoadCursorW(null_mut(), IDC_ARROW);
        let mut info = CURSORINFO { cbSize: size_of::<CURSORINFO>() as u32, ..Default::default() };
        let mut icon: ICONINFO = std::mem::zeroed();
        if !arrow.is_null() && GetCursorInfo(&mut info) != 0 && GetIconInfo(arrow, &mut icon) != 0 {
            let bitmap = if icon.hbmColor.is_null() { icon.hbmMask } else { icon.hbmColor };
            let mut bm: BITMAP = std::mem::zeroed();
            let width = (GetObjectW(bitmap, size_of::<BITMAP>() as i32, &mut bm as *mut BITMAP as *mut _) != 0)
                .then_some(bm.bmWidth.max(0) as u32);
            let monitor = MonitorFromPoint(info.ptScreenPos, MONITOR_DEFAULTTONEAREST);
            let (mut dx, mut dy) = (0u32, 0u32);
            let scale = (GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) >= 0 && dx > 0).then(|| dx * 100 / 96);
            if let (Some(width), Some(scale)) = (width, scale) {
                result = Some((width, scale, width * 100 / scale));
            }
            if !icon.hbmColor.is_null() {
                DeleteObject(icon.hbmColor);
            }
            if !icon.hbmMask.is_null() {
                DeleteObject(icon.hbmMask);
            }
        }
        if !previous.is_null() {
            SetThreadDpiAwarenessContext(previous);
        }
        result
    }
}

/// `shown()` en JSON pour le journal d'Admin.
fn shown_value(s: Option<(u32, u32, u32)>) -> Value {
    match s {
        Some((px, scale, logical)) => json!({ "px": px, "scale": scale, "logical": logical }),
        None => Value::Null,
    }
}

/// Attend (1 s au plus) que la flèche dessinée ait la taille logique `base`.
fn wait_shown(base: u32) -> Option<(u32, u32, u32)> {
    let mut last = shown();
    for _ in 0..10 {
        if last.is_some_and(|(_, _, logical)| logical.abs_diff(base) <= 2) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        last = shown();
    }
    last
}

/// Met le pointeur à `base` px en direct : action 0x2029 (taille en pixels,
/// écrite aussi dans le profil ; validée sur l'ordinateur d'essai) ; si elle
/// est refusée, la base dans le registre puis `SPI_SETCURSORS`. Rend la
/// méthode suivie.
fn apply(base: u32) -> Result<&'static str> {
    let flags = SPIF_UPDATEINIFILE | SPIF_SENDCHANGE;
    if unsafe { SystemParametersInfoW(SPI_SETCURSORBASESIZE, 0, base as usize as *mut _, flags) } != 0 {
        // L'action écrit elle-même CursorBaseSize ; on s'en assure.
        write_dword(CURSORS, "CursorBaseSize", Some(base))?;
        return Ok("0x2029");
    }
    write_dword(CURSORS, "CursorBaseSize", Some(base))?;
    unsafe { SystemParametersInfoW(SPI_SETCURSORS, 0, null_mut(), SPIF_SENDCHANGE) };
    Ok("registre et SPI_SETCURSORS (0x2029 refusée)")
}

/// Taille du pointeur en ce moment (valeurs du profil, null si absentes) et
/// SID du compte : ce qu'Admin note avant de l'agrandir.
pub fn read() -> Result<Value> {
    Ok(json!({
        "sid": super::agent::own_sid()?,
        "previous_base": read_dword(CURSORS, "CursorBaseSize"),
        "previous_size": read_dword(ACCESSIBILITY, "CursorSize"),
    }))
}

/// Agrandit le pointeur au cran `step` (base `base` px) s'il est plus petit ;
/// rend la taille d'avant (`previous_base`, `previous_size` : null si la
/// valeur n'existait pas), la méthode suivie, et ce qui se relit ensuite.
pub fn enlarge(step: u32, base: u32) -> Result<Value> {
    let previous_base = read_dword(CURSORS, "CursorBaseSize");
    let previous_size = read_dword(ACCESSIBILITY, "CursorSize");
    let current = previous_base.unwrap_or(32);
    if current >= base {
        return Ok(json!({ "changed": false, "base": current, "previous_base": previous_base, "previous_size": previous_size }));
    }
    let before = shown();
    let method = apply(base)?;
    write_dword(ACCESSIBILITY, "CursorSize", Some(step))?;
    Ok(json!({
        "changed": true, "base": base, "step": step, "method": method,
        "previous_base": previous_base, "previous_size": previous_size,
        "registry_base": read_dword(CURSORS, "CursorBaseSize"),
        "shown_before": shown_value(before), "shown_after": shown_value(wait_shown(base)),
    }))
}

/// Remet la taille d'avant la session (en direct, puis les valeurs du
/// profil telles qu'elles étaient, absentes comprises).
///
/// Échoue (et Admin garde la taille à rendre pour réessayer) si le profil
/// n'a pas repris ses valeurs. Une flèche dessinée encore plus grande que la
/// base (après un second rechargement) est signalée (`shown_ok` faux) : un
/// jeu de pointeurs personnalisé peut avoir une flèche plus grande.
pub fn restore(sid: Option<&str>, previous_base: Option<u32>, previous_size: Option<u32>) -> Result<Value> {
    // La taille notée pour un compte ne va qu'à ce compte.
    if let Some(sid) = sid {
        let own = super::agent::own_sid()?;
        if own != sid {
            bail!("taille notée pour un autre compte ({sid}), l'agent tourne avec {own}");
        }
    }
    let base = previous_base.unwrap_or(32);
    let method = apply(base)?;
    write_dword(CURSORS, "CursorBaseSize", previous_base)?;
    write_dword(ACCESSIBILITY, "CursorSize", previous_size)?;
    let registry_base = read_dword(CURSORS, "CursorBaseSize");
    if registry_base != previous_base || read_dword(ACCESSIBILITY, "CursorSize") != previous_size {
        bail!("profil non remis : base {registry_base:?} au lieu de {previous_base:?}");
    }
    let mut after = wait_shown(base);
    if after.is_some_and(|(_, _, logical)| logical > base + 2) {
        // Encore grand : un second rechargement des pointeurs.
        unsafe { SystemParametersInfoW(SPI_SETCURSORS, 0, null_mut(), SPIF_SENDCHANGE) };
        after = wait_shown(base);
    }
    let shown_ok = !after.is_some_and(|(_, _, logical)| logical > base + 2);
    Ok(json!({ "base": base, "method": method, "registry_base": registry_base, "shown_after": shown_value(after), "shown_ok": shown_ok }))
}
