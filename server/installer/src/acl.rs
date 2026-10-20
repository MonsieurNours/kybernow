//! Droits du dossier de données de KyberNow (`C:\ProgramData\KyberNow`).
//!
//! ProgramData laisse par défaut tout compte y créer des dossiers et en
//! devenir propriétaire : sur un ordinateur partagé, un compte standard
//! pourrait déposer un fichier dans `update\` entre la vérification du paquet
//! et son extraction par PowerShell, qui tourne en SYSTEM. Le dossier ne garde
//! donc que SYSTEM et Administrateurs (contrôle total), sans héritage de
//! ProgramData : propriétaire Administrateurs partout, droits explicites des
//! fichiers retirés, liens (jonctions, liens symboliques, liens physiques)
//! supprimés sans être suivis. Rien n'y est lisible par les autres comptes :
//! l'icône lit le port de la console dans `kybernow.json`, sous Program Files.
//!
//! Même fichier dans `server/installer/src/acl.rs` (installation) et
//! `server/admin/src/update/acl.rs` (chaque démarrage de la console) : gardez
//! les deux identiques.

use std::path::Path;

use anyhow::Result;

/// Propriétaire Administrateurs ; liste protégée (rien n'est hérité de
/// ProgramData) ; SYSTEM et Administrateurs en contrôle total, hérité par les
/// dossiers et fichiers.
#[cfg_attr(not(windows), allow(dead_code))]
pub const DATA_DIR_SDDL: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";

/// Pose les droits sur `dir` et tout ce qu'il contient (le crée s'il manque).
/// Renvoie le nombre de liens retirés.
#[cfg(windows)]
pub fn secure_dir(dir: &Path) -> Result<usize> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    use anyhow::{Context, bail};
    use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, ProgressInvokeNever, SE_FILE_OBJECT, TreeResetNamedSecurityInfoW,
    };
    use windows_sys::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetSecurityDescriptorOwner, OWNER_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };

    // Lecture, restauration et prise de possession : pour reprendre la main
    // sur ce qu'un autre compte aurait déposé avec ses propres droits.
    let _privileges = imp::Privileges::enable(&["SeBackupPrivilege", "SeRestorePrivilege", "SeTakeOwnershipPrivilege"]);
    let mut removed = 0;
    // Un lien à la place du dossier lui-même : retiré, sa cible n'est pas touchée.
    if std::fs::symlink_metadata(dir).is_ok_and(|m| imp::is_reparse_point(&m)) {
        imp::remove_link(dir)?;
        removed += 1;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))?;
    removed += imp::remove_links(dir)?;

    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let sddl = wide(DATA_DIR_SDDL.as_ref());
    let path = wide(dir.as_os_str());
    unsafe {
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
            bail!("descripteur de sécurité : {}", std::io::Error::last_os_error());
        }
        let mut owner: PSID = null_mut();
        let mut dacl: *mut ACL = null_mut();
        let (mut present, mut defaulted) = (0, 0);
        let parsed = GetSecurityDescriptorOwner(sd, &mut owner, &mut defaulted) != 0
            && GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted) != 0
            && present != 0;
        // Propriétaire et liste sur le dossier ; ses enfants perdent leurs
        // droits explicites (KeepExplicit = FALSE) et héritent de lui.
        let err = if parsed {
            TreeResetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                owner,
                null_mut(),
                dacl,
                null(),
                0,
                None,
                ProgressInvokeNever,
                null(),
            )
        } else {
            ERROR_SUCCESS
        };
        LocalFree(sd);
        if !parsed {
            bail!("descripteur de sécurité illisible");
        }
        if err != ERROR_SUCCESS {
            bail!("droits de {} : {}", dir.display(), std::io::Error::from_raw_os_error(err as i32));
        }
    }
    Ok(removed)
}

/// Hors Windows : rien à faire (simulation, développement).
#[cfg(not(windows))]
pub fn secure_dir(dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir)?;
    Ok(0)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use anyhow::{Context, Result, bail};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES,
        TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

    /// Jonction, lien symbolique ou autre point d'analyse.
    pub fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    /// Retire le lien lui-même (jamais sa cible).
    pub fn remove_link(path: &Path) -> Result<()> {
        let dir = std::fs::symlink_metadata(path).is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0);
        let result = if dir { std::fs::remove_dir(path) } else { std::fs::remove_file(path) };
        result.with_context(|| format!("lien {} impossible à retirer", path.display()))
    }

    /// Nombre de noms du fichier : plus d'un, c'est un lien physique vers un
    /// fichier qui vit aussi ailleurs (et dont il ne faut pas changer les droits).
    fn hard_links(path: &Path) -> Result<u32> {
        let file = std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(7)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .with_context(|| format!("ouverture de {}", path.display()))?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            bail!("{} : {}", path.display(), std::io::Error::last_os_error());
        }
        Ok(info.nNumberOfLinks)
    }

    /// Retire tous les liens sous `dir`, sans les suivre. Un dossier illisible
    /// arrête tout : mieux vaut ne pas poser les droits que les propager à
    /// travers un lien qu'on n'a pas vu.
    pub fn remove_links(dir: &Path) -> Result<usize> {
        let mut removed = 0;
        for entry in std::fs::read_dir(dir).with_context(|| format!("lecture de {}", dir.display()))? {
            let path = entry?.path();
            let meta = std::fs::symlink_metadata(&path)?;
            if is_reparse_point(&meta) || (meta.is_file() && hard_links(&path)? > 1) {
                remove_link(&path)?;
                removed += 1;
            } else if meta.is_dir() {
                removed += remove_links(&path)?;
            }
        }
        Ok(removed)
    }

    /// Privilèges activés le temps de la pose, désactivés ensuite.
    pub struct Privileges(Vec<LUID>);

    impl Privileges {
        pub fn enable(names: &[&str]) -> Self {
            let luids: Vec<LUID> = names
                .iter()
                .filter_map(|name| {
                    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                    let mut luid = LUID { LowPart: 0, HighPart: 0 };
                    (unsafe { LookupPrivilegeValueW(std::ptr::null(), wide.as_ptr(), &mut luid) } != 0).then_some(luid)
                })
                .collect();
            set(&luids, SE_PRIVILEGE_ENABLED);
            Self(luids)
        }
    }

    impl Drop for Privileges {
        fn drop(&mut self) {
            set(&self.0, 0);
        }
    }

    fn set(luids: &[LUID], attributes: u32) {
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
                return;
            }
            for luid in luids {
                let privileges = TOKEN_PRIVILEGES {
                    PrivilegeCount: 1,
                    Privileges: [LUID_AND_ATTRIBUTES { Luid: *luid, Attributes: attributes }],
                };
                // Un privilège absent du jeton est ignoré (ERROR_NOT_ALL_ASSIGNED).
                AdjustTokenPrivileges(token, 0, &privileges, 0, std::ptr::null_mut(), std::ptr::null_mut());
            }
            CloseHandle(token);
        }
    }
}
