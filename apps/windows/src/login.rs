//! « Ouvrir à l'ouverture de session » (réglages de l'app) : la valeur
//! `KyberNow Link` de `HKCU\…\CurrentVersion\Run`, qui lance l'app sans
//! fenêtre (`--background`), dans l'icône seule. Celle de KyberNow Station
//! s'appelle `KyberNow` : les deux ne se gênent pas.
//!
//! Le Gestionnaire des tâches (onglet Démarrage) et les Paramètres de Windows
//! désactivent une valeur sans l'effacer, dans `…\Explorer\StartupApproved\Run` :
//! l'app le reflète et ne la réactive jamais d'elle-même ; seule la case des
//! réglages, cochée par l'utilisateur, la réactive.

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
};
use windows::core::HSTRING;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// Nom de la valeur, montré par le Gestionnaire des tâches à défaut de celui de l'exécutable.
const VALUE: &str = "KyberNow Link";
/// Argument du lancement à l'ouverture de session.
pub const BACKGROUND: &str = "--background";

fn command() -> String {
    format!("\"{}\" {BACKGROUND}", std::env::current_exe().unwrap_or_default().display())
}

fn read_string(key: &str, name: &str) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(name), RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut len))
    };
    if r.is_err() {
        return None;
    }
    let n = (len as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Désactivée dans le Gestionnaire des tâches : premier octet impair (2 :
/// activée, 3 : désactivée).
fn disabled_by_user() -> bool {
    let mut buf = [0u8; 16];
    let mut len = buf.len() as u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(APPROVED_KEY), &HSTRING::from(VALUE), RRF_RT_REG_BINARY, None, Some(buf.as_mut_ptr().cast()), Some(&mut len))
    };
    r.is_ok() && len > 0 && buf[0] & 1 == 1
}

fn write_run() -> bool {
    let data: Vec<u16> = command().encode_utf16().chain(Some(0)).collect();
    let r = unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN_KEY), &HSTRING::from(VALUE), REG_SZ.0, Some(data.as_ptr().cast()), (data.len() * 2) as u32)
    };
    r.is_ok()
}

fn delete(key: &str) {
    unsafe {
        let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(VALUE));
    }
}

/// Ce que montre la case : la valeur est là et Windows ne l'a pas désactivée.
pub fn enabled() -> bool {
    read_string(RUN_KEY, VALUE).is_some() && !disabled_by_user()
}

/// La case des réglages : cochée, la valeur est écrite et réactivée si
/// l'utilisateur l'avait désactivée dans Windows ; décochée, elle est retirée.
pub fn set(on: bool) -> bool {
    if on {
        delete(APPROVED_KEY);
        let ok = write_run();
        log::info!("ouverture de session : {}", if ok { "inscrite" } else { "refusée" });
    } else {
        delete(RUN_KEY);
        delete(APPROVED_KEY);
        log::info!("ouverture de session : retirée");
    }
    enabled()
}

/// À chaque lancement : la valeur suit le dossier de l'app, sans toucher à
/// une désactivation faite dans Windows. Rien si elle n'est pas là.
pub fn refresh() {
    if read_string(RUN_KEY, VALUE).is_some_and(|v| v != command()) {
        write_run();
    }
}

/// Désinstallation : plus rien au démarrage.
pub fn forget() {
    delete(RUN_KEY);
    delete(APPROVED_KEY);
}
