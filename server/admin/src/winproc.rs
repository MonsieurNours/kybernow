//! Les processus de Windows, lus d'une seule façon pour Admin, l'icône et
//! l'installateur (STWIN-46 ; ces deux-là l'incluent par `#[path]`) : la liste
//! (`CreateToolhelp32Snapshot`), le chemin d'un exécutable et l'instant de
//! création d'un processus. Chaque poignée est une `OwnedHandle`, fermée en
//! sortant, même par un `?`. Seule dépendance : `windows-sys` (Foundation,
//! Diagnostics_ToolHelp, Threading).

// Chacun des trois programmes n'en prend qu'une partie.
#![allow(dead_code)]

use std::os::windows::io::{FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

/// Un processus de la liste.
#[derive(Debug, Clone)]
pub struct Process {
    pub pid: u32,
    pub parent: u32,
    /// Nom de l'exécutable (`kyavserver.exe`), sans le dossier.
    pub exe: String,
}

/// Une poignée Win32 rendue par une fonction qui renvoie nul en cas d'échec.
fn owned(handle: HANDLE) -> Option<OwnedHandle> {
    (!handle.is_null() && handle != INVALID_HANDLE_VALUE).then(|| unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// Tous les processus, d'un seul relevé.
pub fn processes() -> Vec<Process> {
    use std::os::windows::io::AsRawHandle;
    let Some(snap) = owned(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else { return Vec::new() };
    let mut out = Vec::new();
    let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..unsafe { std::mem::zeroed() } };
    let mut ok = unsafe { Process32FirstW(snap.as_raw_handle(), &mut entry) } != 0;
    while ok {
        let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
        out.push(Process { pid: entry.th32ProcessID, parent: entry.th32ParentProcessID, exe: String::from_utf16_lossy(&entry.szExeFile[..len]) });
        ok = unsafe { Process32NextW(snap.as_raw_handle(), &mut entry) } != 0;
    }
    out
}

/// Les processus dont l'exécutable s'appelle `exe` (sans la casse).
pub fn by_name(exe: &str) -> Vec<u32> {
    processes().into_iter().filter(|p| p.exe.eq_ignore_ascii_case(exe)).map(|p| p.pid).collect()
}

fn open(pid: u32) -> Option<OwnedHandle> {
    owned(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })
}

/// Chemin complet de l'exécutable d'un processus.
pub fn image_path(pid: u32) -> Option<String> {
    use std::os::windows::io::AsRawHandle;
    let process = open(pid)?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process.as_raw_handle(), PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) } != 0;
    ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// L'instant de création d'un processus (FILETIME, en 100 ns).
pub fn created(pid: u32) -> Option<u64> {
    use std::os::windows::io::AsRawHandle;
    let process = open(pid)?;
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    let ok = unsafe { GetProcessTimes(process.as_raw_handle(), &mut created, &mut exited, &mut kernel, &mut user) } != 0;
    ok.then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
