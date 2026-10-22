//! Droits du dossier de données de KyberNow (`C:\ProgramData\KyberNow`).
//!
//! ProgramData laisse par défaut tout compte y créer des dossiers et en
//! devenir propriétaire : sur un ordinateur à plusieurs comptes, un compte standard
//! pourrait déposer un fichier dans `update\` entre la vérification du paquet
//! et son extraction par PowerShell, qui tourne en SYSTEM. Le dossier ne garde
//! donc que SYSTEM et Administrateurs (contrôle total), sans héritage de
//! ProgramData. Rien n'y est lisible par les autres comptes : l'icône lit le
//! port de la console dans `kybernow.json`, sous Program Files.
//!
//! La pose parcourt l'arborescence de haut en bas, sans jamais suivre un lien :
//! chaque dossier reçoit ses droits (propriétaire Administrateurs, droits
//! explicites remplacés) avant d'être lu, si bien qu'un autre compte ne peut
//! plus rien y créer pendant la suite ; les jonctions, liens symboliques et
//! liens physiques trouvés sont supprimés (le lien, jamais sa cible). Tout
//! passe par des poignées ouvertes en mode sauvegarde, avec les privilèges de
//! sauvegarde, de restauration et de prise de possession : un fichier déposé
//! par un autre compte avec ses propres droits n'arrête pas la pose.
//!
//! Même fichier dans `server/installer/src/acl.rs` (installation) et
//! `server/admin/src/update/acl.rs` (chaque démarrage de KyberNow Admin) : gardez
//! les deux identiques.

use std::path::Path;

use anyhow::Result;

/// Le dossier : propriétaire Administrateurs ; liste protégée (rien n'est
/// hérité de ProgramData) ; SYSTEM et Administrateurs en contrôle total,
/// hérité par les dossiers et fichiers.
#[cfg_attr(not(windows), allow(dead_code))]
pub const DATA_DIR_SDDL: &str = "O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";
/// Un sous-dossier : les mêmes droits, hérités du dossier, et transmis.
#[cfg_attr(not(windows), allow(dead_code))]
const CHILD_DIR_SDDL: &str = "O:BAD:AI(A;OICIID;FA;;;SY)(A;OICIID;FA;;;BA)";
/// Un fichier : les mêmes droits, hérités.
#[cfg_attr(not(windows), allow(dead_code))]
const CHILD_FILE_SDDL: &str = "O:BAD:AI(A;ID;FA;;;SY)(A;ID;FA;;;BA)";
/// Un fichier seul, hors du dossier de données (secrets de Kyber sous Program
/// Files) : liste protégée (rien d'hérité de Program Files, où tout compte
/// peut lire), SYSTEM et Administrateurs seulement.
#[cfg_attr(not(windows), allow(dead_code))]
pub const SECRET_FILE_SDDL: &str = "O:BAD:PAI(A;;FA;;;SY)(A;;FA;;;BA)";

/// Pose les droits sur `dir` et tout ce qu'il contient (le crée s'il manque).
/// Renvoie le nombre de liens retirés.
#[cfg(windows)]
pub fn secure_dir(dir: &Path) -> Result<usize> {
    use anyhow::Context;

    let _privileges = imp::Privileges::enable(&["SeBackupPrivilege", "SeRestorePrivilege", "SeTakeOwnershipPrivilege"]);
    let mut removed = 0;
    // Un lien à la place du dossier lui-même : retiré, sa cible n'est pas touchée.
    if std::fs::symlink_metadata(dir).is_ok_and(|m| imp::is_reparse_point(&m)) {
        imp::delete(dir)?;
        removed += 1;
    }
    std::fs::create_dir_all(dir).with_context(|| format!("création de {}", dir.display()))?;
    let descriptors = imp::Descriptors::new()?;
    imp::apply(dir, &descriptors.root)?;
    removed += imp::walk(dir, &descriptors)?;
    Ok(removed)
}

/// Hors Windows : rien à faire (simulation, développement).
#[cfg(not(windows))]
pub fn secure_dir(dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir)?;
    Ok(0)
}

/// Réserve un fichier seul à SYSTEM et aux Administrateurs (`kyber_config.toml`,
/// certificat et clé de Kyber sous Program Files, lisibles par tous sinon). Un
/// lien à sa place est refusé, jamais suivi.
#[cfg(windows)]
pub fn secure_file(path: &Path) -> Result<()> {
    use anyhow::{Context, bail};

    let _privileges = imp::Privileges::enable(&["SeBackupPrivilege", "SeRestorePrivilege", "SeTakeOwnershipPrivilege"]);
    let meta = std::fs::symlink_metadata(path).with_context(|| format!("lecture de {}", path.display()))?;
    if imp::is_reparse_point(&meta) {
        bail!("{} est un lien : droits non posés", path.display());
    }
    if !meta.is_file() {
        bail!("{} n'est pas un fichier", path.display());
    }
    imp::apply(path, &imp::Descriptor::parse(SECRET_FILE_SDDL)?)
}

/// Hors Windows : rien à faire (simulation, développement).
#[cfg(not(windows))]
pub fn secure_file(path: &Path) -> Result<()> {
    let _ = path;
    Ok(())
}

#[cfg(windows)]
mod imp {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use anyhow::{Context, Result, bail};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LUID, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, DACL_SECURITY_INFORMATION, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, SE_PRIVILEGE_ENABLED, SetKernelObjectSecurity, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    use super::{CHILD_DIR_SDDL, CHILD_FILE_SDDL, DATA_DIR_SDDL};

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_READ_ATTRIBUTES: u32 = 0x80;
    const DELETE: u32 = 0x0001_0000;
    const READ_CONTROL: u32 = 0x0002_0000;
    const WRITE_DAC: u32 = 0x0004_0000;
    const WRITE_OWNER: u32 = 0x0008_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;

    /// Descripteur de sécurité tiré d'une chaîne SDDL (libéré à la fin).
    pub struct Descriptor(PSECURITY_DESCRIPTOR);

    impl Descriptor {
        pub fn parse(sddl: &str) -> Result<Self> {
            let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
            let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(wide.as_ptr(), 1, &mut sd, std::ptr::null_mut()) } == 0 {
                bail!("descripteur de sécurité {sddl} : {}", std::io::Error::last_os_error());
            }
            Ok(Self(sd))
        }
    }

    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe { LocalFree(self.0) };
        }
    }

    pub struct Descriptors {
        pub root: Descriptor,
        dir: Descriptor,
        file: Descriptor,
    }

    impl Descriptors {
        pub fn new() -> Result<Self> {
            Ok(Self { root: Descriptor::parse(DATA_DIR_SDDL)?, dir: Descriptor::parse(CHILD_DIR_SDDL)?, file: Descriptor::parse(CHILD_FILE_SDDL)? })
        }
    }

    /// Ouvre l'objet lui-même (un lien n'est pas suivi), en mode sauvegarde.
    fn open(path: &Path, access: u32, extra_flags: u32) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .access_mode(access)
            .share_mode(7)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | extra_flags)
            .open(path)
    }

    /// Propriétaire et liste d'accès de cet objet seul (aucune propagation :
    /// le parcours s'en charge, sans suivre de lien).
    pub fn apply(path: &Path, descriptor: &Descriptor) -> Result<()> {
        let file = open(path, READ_CONTROL | WRITE_DAC | WRITE_OWNER, 0).with_context(|| format!("ouverture de {}", path.display()))?;
        let ok = unsafe {
            SetKernelObjectSecurity(file.as_raw_handle() as HANDLE, OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, descriptor.0)
        };
        if ok == 0 {
            bail!("droits de {} : {}", path.display(), std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Jonction, lien symbolique ou autre point d'analyse.
    pub fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    /// Supprime l'entrée elle-même (le lien, jamais sa cible), même si un
    /// autre compte l'a protégée.
    pub fn delete(path: &Path) -> Result<()> {
        drop(open(path, DELETE, FILE_FLAG_DELETE_ON_CLOSE).with_context(|| format!("lien {} impossible à retirer", path.display()))?);
        if std::fs::symlink_metadata(path).is_ok() {
            bail!("lien {} toujours là", path.display());
        }
        Ok(())
    }

    /// Nombre de noms du fichier : plus d'un, c'est un lien physique vers un
    /// fichier qui vit aussi ailleurs (dont il ne faut pas changer les droits).
    fn hard_links(path: &Path) -> Result<u32> {
        let file = open(path, FILE_READ_ATTRIBUTES, 0).with_context(|| format!("ouverture de {}", path.display()))?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            bail!("{} : {}", path.display(), std::io::Error::last_os_error());
        }
        Ok(info.nNumberOfLinks)
    }

    /// Contenu de `dir`, qui a déjà ses droits : chaque sous-dossier les reçoit
    /// avant d'être lu. Un dossier illisible arrête tout plutôt que de laisser
    /// un lien qu'on n'a pas vu. Renvoie le nombre de liens retirés.
    pub fn walk(dir: &Path, descriptors: &Descriptors) -> Result<usize> {
        let mut removed = 0;
        for entry in std::fs::read_dir(dir).with_context(|| format!("lecture de {}", dir.display()))? {
            let path = entry?.path();
            let meta = std::fs::symlink_metadata(&path).with_context(|| format!("lecture de {}", path.display()))?;
            if is_reparse_point(&meta) || (meta.is_file() && hard_links(&path)? > 1) {
                delete(&path)?;
                removed += 1;
            } else if meta.is_dir() {
                apply(&path, &descriptors.dir)?;
                removed += walk(&path, descriptors)?;
            } else {
                apply(&path, &descriptors.file)?;
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
