//! Les fichiers de Steam de l'hôte pendant un prêt « Son compte Steam »
//! (docs/pret.md, « Steam » ; STWIN-14), sous Windows.
//!
//! L'agent de console tourne avec le compte de l'hôte, comme les apps de
//! l'ami : tout ce qu'il laisse sur le disque, l'ami peut le lire. Les comptes
//! retenus de Steam (`loginusers.vdf`, `config.vdf` et ses jetons, `local.vdf`)
//! ne restent donc pas à côté de l'original : l'agent en confie le contenu à
//! Admin (SYSTEM), qui le garde dans `C:\ProgramData\KyberNow\lend\`
//! (réservé à SYSTEM et aux Administrateurs), puis les efface. À la fin, Admin
//! le lui rend ; l'agent les réécrit avec le compte de l'hôte (Admin n'écrit
//! jamais dans le profil de l'utilisateur), puis dit à Admin de les oublier.
//!
//! Admin ne parle de ces fichiers qu'au vrai agent : le processus à l'autre
//! bout de la connexion doit être `kybernow-tray.exe` de l'installation (un
//! programme de l'ami qui lirait le jeton local n'y suffit pas). Il ne les
//! rend jamais pendant le prêt (seulement pendant la remise en état, ou une
//! fois le prêt fini), et les garde tant que l'agent n'a pas dit les avoir
//! remis : un agent ou un Admin tombé entre-temps ne perd rien, la remise est
//! retentée (au prochain démarrage de l'icône).

use std::path::{Path, PathBuf};

use base64::Engine;
use serde_json::{Value, json};

/// Les seuls fichiers gardés (par leur nom ; Admin ne reçoit jamais de chemin).
pub const NAMES: [&str; 3] = ["loginusers.vdf", "config.vdf", "local.vdf"];
/// Taille au plus de l'ensemble (un `config.vdf` chargé fait quelques centaines de Kio).
pub const MAX_BYTES: usize = 8 << 20;

/// Où Admin les garde.
pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("lend").join("steam-aside.json")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub lend: String,
    pub files: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeepError {
    /// Des fichiers d'un prêt d'avant sont encore là, pas remis : rien ne
    /// les remplace (ce sont peut-être les seules copies de l'hôte).
    AlreadyKept,
    /// Nom inconnu, contenu illisible ou trop gros.
    Invalid,
    Io,
}

/// Lit le corps d'une demande `{"lend", "files": [{"name", "data"}]}` (`data` en base64).
pub fn parse(body: &Value) -> Result<Kept, KeepError> {
    let lend = body["lend"].as_str().filter(|l| !l.is_empty()).ok_or(KeepError::Invalid)?.to_string();
    let mut files = Vec::new();
    let mut total = 0usize;
    for f in body["files"].as_array().ok_or(KeepError::Invalid)? {
        let name = f["name"].as_str().filter(|n| NAMES.contains(n)).ok_or(KeepError::Invalid)?;
        if files.iter().any(|(n, _): &(String, Vec<u8>)| n == name) {
            return Err(KeepError::Invalid);
        }
        let data = base64::engine::general_purpose::STANDARD.decode(f["data"].as_str().ok_or(KeepError::Invalid)?).map_err(|_| KeepError::Invalid)?;
        total += data.len();
        if total > MAX_BYTES {
            return Err(KeepError::Invalid);
        }
        files.push((name.to_string(), data));
    }
    Ok(Kept { lend, files })
}

fn to_json(kept: &Kept) -> Value {
    json!({
        "lend": kept.lend,
        "files": kept.files.iter().map(|(n, d)| json!({ "name": n, "data": base64::engine::general_purpose::STANDARD.encode(d) })).collect::<Vec<_>>(),
    })
}

/// Ce qu'Admin garde, s'il garde quelque chose.
pub fn load(data_dir: &Path) -> Option<Kept> {
    let v: Value = serde_json::from_slice(&std::fs::read(path(data_dir)).ok()?).ok()?;
    parse(&v).ok()
}

/// Garde les fichiers d'un prêt (écrits d'un bloc, sur le disque avant de
/// répondre : l'agent n'efface les originaux qu'après).
pub fn keep(data_dir: &Path, kept: &Kept) -> Result<(), KeepError> {
    match load(data_dir) {
        // Le même prêt qui redemande (agent relancé avant d'effacer) : remplacé.
        Some(old) if old.lend != kept.lend => return Err(KeepError::AlreadyKept),
        _ => {}
    }
    let file = path(data_dir);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|_| KeepError::Io)?;
    }
    crate::durable::write(&file, to_json(kept).to_string()).map_err(|_| KeepError::Io)
}

/// L'agent les a remis : oubliés (seulement ceux de ce prêt).
pub fn forget(data_dir: &Path, lend: &str) -> bool {
    match load(data_dir) {
        Some(k) if k.lend == lend => std::fs::remove_file(path(data_dir)).is_ok(),
        _ => false,
    }
}

/// La réponse à l'agent : `{"lend", "files": [{"name", "data"}]}`.
pub fn answer(kept: &Kept) -> Value {
    to_json(kept)
}

/// Le processus à l'autre bout de la connexion locale `peer` est-il l'agent
/// de console de cette installation (`<installation>\tray\kybernow-tray.exe`) ?
#[cfg(windows)]
pub fn caller_is_agent(peer: std::net::SocketAddr) -> bool {
    let Some(pid) = owner_pid(peer) else { return false };
    let Some(image) = process_image(pid) else { return false };
    let expected = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.parent()?.join("tray").join("kybernow-tray.exe")));
    let same = expected.is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(&image.to_string_lossy()));
    if !same {
        tracing::warn!("prêt : fichiers de Steam demandés par {} (pid {pid}), pas par l'agent de console : refusé", image.display());
    }
    same
}

/// Hors Windows, Admin tourne avec le compte de l'hôte : rien à lui confier
/// que l'ami ne puisse lire (les routes refusent). Les essais simulent l'agent.
#[cfg(not(windows))]
pub fn caller_is_agent(_peer: std::net::SocketAddr) -> bool {
    cfg!(test)
}

/// Le processus qui tient le bout client de la connexion `peer` (table TCP
/// de Windows, IPv4 et IPv6).
#[cfg(windows)]
fn owner_pid(peer: std::net::SocketAddr) -> Option<u32> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_CONNECTIONS,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
    let table = |family: u32| -> Option<Vec<u8>> {
        let mut size = 0u32;
        unsafe { GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, family, TCP_TABLE_OWNER_PID_CONNECTIONS, 0) };
        for _ in 0..4 {
            let mut buf = vec![0u8; size as usize + 4096];
            size = buf.len() as u32;
            let r = unsafe { GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut size, 0, family, TCP_TABLE_OWNER_PID_CONNECTIONS, 0) };
            if r == 0 {
                return Some(buf);
            }
        }
        None
    };
    let port = peer.port();
    match peer.ip().to_canonical() {
        std::net::IpAddr::V4(ip) => {
            let buf = table(AF_INET as u32)?;
            let t = buf.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
            let n = unsafe { (*t).dwNumEntries } as usize;
            let rows = unsafe { std::slice::from_raw_parts((*t).table.as_ptr().cast::<MIB_TCPROW_OWNER_PID>(), n) };
            rows.iter()
                .find(|r| u16::from_be(r.dwLocalPort as u16) == port && std::net::Ipv4Addr::from(u32::from_be(r.dwLocalAddr)) == ip)
                .map(|r| r.dwOwningPid)
        }
        std::net::IpAddr::V6(ip) => {
            let buf = table(AF_INET6 as u32)?;
            let t = buf.as_ptr().cast::<MIB_TCP6TABLE_OWNER_PID>();
            let n = unsafe { (*t).dwNumEntries } as usize;
            let rows = unsafe { std::slice::from_raw_parts((*t).table.as_ptr().cast::<MIB_TCP6ROW_OWNER_PID>(), n) };
            rows.iter()
                .find(|r| u16::from_be(r.dwLocalPort as u16) == port && std::net::Ipv6Addr::from(r.ucLocalAddr) == ip)
                .map(|r| r.dwOwningPid)
        }
    }
}

/// Chemin complet du programme d'un processus.
#[cfg(windows)]
fn process_image(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) } != 0;
    unsafe { CloseHandle(handle) };
    ok.then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(lend: &str, files: &[(&str, &[u8])]) -> Value {
        json!({ "lend": lend, "files": files.iter().map(|(n, d)| json!({ "name": n, "data": base64::engine::general_purpose::STANDARD.encode(d) })).collect::<Vec<_>>() })
    }

    #[test]
    fn noms_et_tailles() {
        let ok = parse(&body("l1", &[("config.vdf", b"jetons"), ("local.vdf", b"")])).unwrap();
        assert_eq!(ok.files, [("config.vdf".to_string(), b"jetons".to_vec()), ("local.vdf".to_string(), Vec::new())]);
        // Jamais un chemin : seulement les trois noms de Steam.
        assert_eq!(parse(&body("l1", &[("..\\..\\Windows\\win.ini", b"x")])), Err(KeepError::Invalid));
        assert_eq!(parse(&body("l1", &[("config.vdf", b"a"), ("config.vdf", b"b")])), Err(KeepError::Invalid));
        assert_eq!(parse(&body("", &[])), Err(KeepError::Invalid));
        assert_eq!(parse(&json!({ "lend": "l1", "files": [{ "name": "config.vdf", "data": "pas du base64 !" }] })), Err(KeepError::Invalid));
        let big = vec![0u8; MAX_BYTES + 1];
        assert_eq!(parse(&body("l1", &[("config.vdf", &big)])), Err(KeepError::Invalid));
    }

    #[test]
    fn garde_rend_et_oublie() {
        let dir = std::env::temp_dir().join(format!("kn-lend-steam-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(&dir), None);
        let first = parse(&body("l1", &[("loginusers.vdf", b"comptes de l'hote"), ("config.vdf", b"jetons de l'hote")])).unwrap();
        keep(&dir, &first).unwrap();
        assert_eq!(load(&dir), Some(first.clone()));
        // Le même prêt redemande : remplacé ; un autre prêt : refusé, rien ne bouge.
        keep(&dir, &first).unwrap();
        let other = parse(&body("l2", &[("config.vdf", b"jetons de l'ami")])).unwrap();
        assert_eq!(keep(&dir, &other), Err(KeepError::AlreadyKept));
        assert_eq!(load(&dir), Some(first.clone()));
        assert_eq!(parse(&answer(&first)).unwrap(), first);
        // Oublier un autre prêt ne fait rien ; le bon, si.
        assert!(!forget(&dir, "l2"));
        assert!(forget(&dir, "l1"));
        assert_eq!(load(&dir), None);
        keep(&dir, &other).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
