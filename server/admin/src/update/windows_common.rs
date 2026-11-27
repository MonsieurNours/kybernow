//! Ce que l'applicateur des mises à jour d'Admin (`update::apply`) et
//! l'installateur (`server/installer`, qui inclut ce fichier par `#[path]`)
//! doivent dire de la même façon : processus à arrêter, droits et relance des
//! services, marqueur de bascule, verrou entre processus. `update.ps1` en garde
//! une copie (PowerShell), comparée à celle-ci par un essai d'Admin.
//!
//! Seule dépendance : `windows-sys` (Foundation, Security, Storage_FileSystem,
//! System_Threading), sous Windows ; ce fichier se compile dans les deux crates.

#![allow(dead_code)]

/// Droits Windows par défaut, plus démarrer et arrêter (RP, WP) pour les
/// utilisateurs interactifs (IU) : l'icône pilote les services sans droits
/// administrateur. Suspendre (DT) n'est pas donné, changer la configuration
/// (DC) reste réservé aux administrateurs.
pub const SERVICE_SDDL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)";

/// Processus qui tiennent des fichiers de `kyber\` : le serveur Kyber, et les
/// aides du micro et de la caméra de l'appareil (lancées pendant une session).
pub const KYBER_PROCESSES: [&str; 7] = [
    "kyavserver.exe",
    "kycontroller.exe",
    "kynputserver.exe",
    "kyclient.exe",
    "kyservice.exe",
    "kybernow-micro.exe",
    "kybernow-camera.exe",
];

/// Bibliothèques de la caméra, sous `kyber\`, chargées par d'autres
/// processus : la source de Windows 11 par Frame Server, le filtre de
/// Windows 10 par toute app qui filme. Encore chargées, elles empêchent de
/// renommer ou de remplacer `kyber\` : elles sont mises de côté (renommées,
/// effacées au redémarrage).
pub const CAMERA_LIBRARIES: [&str; 2] = ["KyberNowCameraSource.dll", "KyberNowCameraFilter.dll"];

/// Services de Windows qui chargent la source de la caméra (Windows 11) :
/// arrêtés pendant une mise à jour, Windows les relance à la demande.
pub const FRAME_SERVER: [&str; 2] = ["FrameServerMonitor", "FrameServer"];

/// Relance automatique après un arrêt anormal (`sc failure <service> reset=
/// 86400 actions= …`) : trois essais, à 5 s, 5 s puis 30 s. Posée pour `kyber`
/// et pour `kyber-admin`.
pub const RECOVERY_RESET: &str = "86400";
pub const RECOVERY_ACTIONS: &str = "restart/5000/restart/5000/restart/30000";

/// Marqueur de bascule (`C:\ProgramData\KyberNow\update\bascule.json`) :
/// écrit avant le premier renommage, effacé quand tous les dossiers sont en
/// place. Trouvé au démarrage (coupure de courant, gel), il dit à Admin ou à
/// l'installateur de finir de remettre les dossiers.
pub const SWITCH_MARKER: &str = "bascule.json";

/// Verrou entre processus : une seule installation à la fois (applicateur
/// d'Admin, `update.ps1`, installateur).
pub const UPDATE_MUTEX: &str = r"Global\KyberNowUpdate";

/// Dossier (sous `<installation>\.staging`) où partent les bibliothèques
/// encore chargées.
pub const SET_ASIDE_DIR: &str = "verrouilles";

/// Verrou des installations (`UPDATE_MUTEX`), tenu jusqu'à sa destruction.
#[cfg(windows)]
pub struct UpdateLock(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for UpdateLock {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Threading::ReleaseMutex(self.0);
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Prend le verrou des installations, sans attendre : `None` si une autre
/// installation le tient (ou si Windows refuse de le créer). Un compte
/// standard ne peut pas créer d'objet `Global\` : il ne peut pas le bloquer.
#[cfg(windows)]
pub fn lock_updates() -> Option<UpdateLock> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
    let name = wide(UPDATE_MUTEX);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return None;
        }
        match WaitForSingleObject(handle, 0) {
            // Abandonné : son dernier détenteur est mort (applicateur tué) ; il est à nous.
            WAIT_OBJECT_0 | WAIT_ABANDONED => Some(UpdateLock(handle)),
            _ => {
                CloseHandle(handle);
                None
            }
        }
    }
}

/// Une installation est-elle en cours ? Le verrou n'existe que tant qu'un
/// processus le garde ouvert ; il n'est ici qu'ouvert puis refermé, jamais
/// pris (ce qui pourrait faire refuser une installation qui démarre).
#[cfg(windows)]
pub fn updates_locked() -> bool {
    use windows_sys::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    let name = wide(UPDATE_MUTEX);
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            return false;
        }
        windows_sys::Win32::Foundation::CloseHandle(handle);
        true
    }
}

/// Met de côté les bibliothèques de la caméra encore chargées par un autre
/// processus (Frame Server, une app qui filme) : renommées dans
/// `set_aside_dir` (même volume : Windows permet de renommer une
/// bibliothèque chargée, pas de l'effacer ni de l'écraser), effacées au
/// prochain redémarrage. Renvoie chaque déplacement (origine, nouvelle place),
/// pour les remettre si la mise à jour est annulée.
#[cfg(windows)]
pub fn set_aside_libraries(kyber: &std::path::Path, set_aside_dir: &std::path::Path) -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_DELAY_UNTIL_REBOOT, MoveFileExW};
    let mut moved = Vec::new();
    for name in CAMERA_LIBRARIES {
        let path = kyber.join(name);
        if !path.is_file() {
            continue;
        }
        // Ouvrable sans partage : personne ne la tient, elle se remplace normalement.
        if std::fs::OpenOptions::new().read(true).write(true).share_mode(0).open(&path).is_ok() {
            continue;
        }
        let _ = std::fs::create_dir_all(set_aside_dir);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let target = set_aside_dir.join(format!("{name}.{stamp}.old"));
        if std::fs::rename(&path, &target).is_ok() {
            let wide_target: Vec<u16> = target.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            unsafe {
                MoveFileExW(wide_target.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT);
            }
            moved.push((path, target));
        }
    }
    moved
}

/// Remet les bibliothèques mises de côté (mise à jour annulée) : recopiées à
/// leur place si elle est libre (la copie mise de côté reste chargée).
#[cfg(windows)]
pub fn put_back_libraries(moved: &[(std::path::PathBuf, std::path::PathBuf)]) {
    for (original, aside) in moved {
        if !original.exists() {
            let _ = std::fs::copy(aside, original);
        }
    }
}

/// Code de sortie d'un outil, lisible dans un journal : un plantage
/// (exception Windows, `0xC0000005` : accès mémoire interdit) n'est pas un
/// refus ordinaire.
pub fn exit_text(code: Option<i32>) -> String {
    match code {
        None => "arrêté sans code".to_string(),
        Some(c) if c < 0 => {
            let status = c as u32;
            let what = match status {
                0xC000_0005 => " : accès mémoire interdit",
                0xC000_0409 => " : dépassement de tampon",
                0xC000_0135 => " : bibliothèque introuvable",
                _ => "",
            };
            format!("plantage 0x{status:08X}{what}")
        }
        Some(c) => format!("code {c}"),
    }
}
