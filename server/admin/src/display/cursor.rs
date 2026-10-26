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
//! Le réglage est celui du profil de l'utilisateur (`HKCU`) : ces fonctions
//! tournent dans l'agent lancé avec son compte (`agent::run_as_user`), et
//! c'est Admin qui garde la taille d'origine (le dossier de données lui est
//! fermé, à lui).

use std::ptr::null_mut;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SM_CXCURSOR, SPI_SETCURSORS, SPIF_SENDCHANGE, SystemParametersInfoW,
};

const CURSORS: &str = r"Control Panel\Cursors";
const ACCESSIBILITY: &str = r"Software\Microsoft\Accessibility";

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

/// Recharge les pointeurs à la taille du registre, en direct.
fn apply() -> u32 {
    unsafe { SystemParametersInfoW(SPI_SETCURSORS, 0, null_mut(), SPIF_SENDCHANGE) };
    unsafe { GetSystemMetrics(SM_CXCURSOR) }.max(0) as u32
}

/// Agrandit le pointeur au cran `step` (base `base` px) s'il est plus petit ;
/// rend la taille d'avant (`previous_base`, `previous_size` : null si la
/// valeur n'existait pas) et ce qui a été fait.
pub fn enlarge(step: u32, base: u32) -> Result<Value> {
    let previous_base = read_dword(CURSORS, "CursorBaseSize");
    let previous_size = read_dword(ACCESSIBILITY, "CursorSize");
    let current = previous_base.unwrap_or(32);
    if current >= base {
        return Ok(json!({ "changed": false, "base": current, "previous_base": previous_base, "previous_size": previous_size }));
    }
    write_dword(CURSORS, "CursorBaseSize", Some(base))?;
    write_dword(ACCESSIBILITY, "CursorSize", Some(step))?;
    let measured = apply();
    Ok(json!({
        "changed": true, "base": base, "step": step,
        "previous_base": previous_base, "previous_size": previous_size,
        "cursor_width": measured,
    }))
}

/// Remet la taille d'avant la session.
pub fn restore(previous_base: Option<u32>, previous_size: Option<u32>) -> Result<Value> {
    write_dword(CURSORS, "CursorBaseSize", previous_base)?;
    write_dword(ACCESSIBILITY, "CursorSize", previous_size)?;
    let measured = apply();
    Ok(json!({ "base": previous_base.unwrap_or(32), "cursor_width": measured }))
}
