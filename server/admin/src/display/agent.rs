//! Lancement de l'agent d'affichage dans la session de l'écran.
//!
//! Un service Windows vit dans la session 0, qui n'a pas d'écran : il ne peut
//! pas changer la disposition des écrans. On relance donc `kyber-admin
//! display-agent …` dans la session console, avec le jeton SYSTEM du service
//! rattaché à cette session (comme Kyber, Sunshine ou Apollo). Cela marche
//! aussi sur l'écran de connexion de Windows.

use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::Security::{
    DuplicateTokenEx, SecurityImpersonation, SetTokenInformation, TOKEN_ALL_ACCESS, TokenPrimary, TokenSessionId,
};
use windows_sys::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, GetCurrentProcess, GetCurrentProcessId,
    GetExitCodeProcess, OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOW, WaitForSingleObject,
};

const TIMEOUT_MS: u32 = 30_000;

fn in_session_zero() -> bool {
    let mut session = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) != 0 && session == 0 }
}

/// Lance l'agent avec `args` (sans `--out`) et renvoie son résultat JSON.
pub fn run(data_dir: &Path, args: &[String]) -> Result<Value> {
    run_with_timeout(data_dir, args, TIMEOUT_MS)
}

/// Comme `run`, en attendant l'agent au plus `timeout_ms`.
pub fn run_with_timeout(data_dir: &Path, args: &[String], timeout_ms: u32) -> Result<Value> {
    let dir = data_dir.join("display");
    std::fs::create_dir_all(&dir)?;
    let out: PathBuf = dir.join(format!("agent-{}.json", rand::random::<u32>()));
    let exe = std::env::current_exe()?;
    let mut full: Vec<String> = vec!["display-agent".into()];
    full.extend(args.iter().cloned());
    full.push("--out".into());
    full.push(out.display().to_string());

    let code = if in_session_zero() {
        start_in_console_session(&exe, &full, Some(timeout_ms))?
    } else {
        std::process::Command::new(&exe).args(&full).status()?.code().unwrap_or(-1) as u32
    };
    let text = std::fs::read_to_string(&out).with_context(|| format!("l'agent d'affichage n'a rien renvoyé (code {code})"));
    let _ = std::fs::remove_file(&out);
    let value: Value = serde_json::from_str(&text?)?;
    if let Some(err) = value.get("error").and_then(Value::as_str) {
        bail!("{err}");
    }
    Ok(value)
}

fn quote(arg: &str) -> String {
    if arg.is_empty() || arg.contains([' ', '\t', '"']) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.to_string()
    }
}

/// Lance `exe` dans la session de l'utilisateur connecté, avec son compte
/// (pas celui du service) : l'icône de la zone de notification.
pub fn launch_as_user(exe: &Path) -> Result<()> {
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use windows_sys::Win32::System::RemoteDesktop::WTSQueryUserToken;
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == 0xFFFF_FFFF {
        bail!("aucune session ouverte");
    }
    unsafe {
        let mut token: HANDLE = null_mut();
        if WTSQueryUserToken(session, &mut token) == 0 {
            bail!("jeton de l'utilisateur : {}", std::io::Error::last_os_error());
        }
        let mut env: *mut std::ffi::c_void = null_mut();
        CreateEnvironmentBlock(&mut env, token, 0);
        let mut cmdline: Vec<u16> = quote(&exe.display().to_string()).encode_utf16().chain(std::iter::once(0)).collect();
        let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain(std::iter::once(0)).collect();
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_mut_ptr();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessAsUserW(
            token, null(), cmdline.as_mut_ptr(), null(), null(), 0, CREATE_UNICODE_ENVIRONMENT, env, null(), &si, &mut pi,
        );
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
        CloseHandle(token);
        if ok == 0 {
            bail!("lancement : {}", std::io::Error::last_os_error());
        }
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
    }
    Ok(())
}

/// Lance l'agent avec `args` sans l'attendre (veille de longue durée).
pub fn launch(data_dir: &Path, args: &[String]) -> Result<()> {
    let _ = data_dir;
    let exe = std::env::current_exe()?;
    let mut full: Vec<String> = vec!["display-agent".into()];
    full.extend(args.iter().cloned());
    if in_session_zero() {
        start_in_console_session(&exe, &full, None)?;
    } else {
        std::process::Command::new(&exe).args(&full).spawn()?;
    }
    Ok(())
}

/// Lance `exe` dans la session console ; avec `wait`, l'attend au plus ce délai (ms).
fn start_in_console_session(exe: &Path, args: &[String], wait: Option<u32>) -> Result<u32> {
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == 0xFFFF_FFFF {
        bail!("aucune session console active");
    }
    unsafe {
        let mut own: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ALL_ACCESS, &mut own) == 0 {
            bail!("jeton du service : {}", std::io::Error::last_os_error());
        }
        let mut token: HANDLE = null_mut();
        let dup = DuplicateTokenEx(own, TOKEN_ALL_ACCESS, null(), SecurityImpersonation, TokenPrimary, &mut token);
        CloseHandle(own);
        if dup == 0 {
            bail!("copie du jeton : {}", std::io::Error::last_os_error());
        }
        if SetTokenInformation(token, TokenSessionId, &session as *const u32 as *const _, 4) == 0 {
            CloseHandle(token);
            bail!("rattachement à la session {session} : {}", std::io::Error::last_os_error());
        }

        let mut cmdline: Vec<u16> = std::iter::once(quote(&exe.display().to_string()))
            .chain(args.iter().map(|a| quote(a)))
            .collect::<Vec<_>>()
            .join(" ")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain(std::iter::once(0)).collect();
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_mut_ptr();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessAsUserW(
            token,
            null(),
            cmdline.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            null(),
            null(),
            &si,
            &mut pi,
        );
        CloseHandle(token);
        if ok == 0 {
            bail!("lancement de l'agent d'affichage : {}", std::io::Error::last_os_error());
        }
        let Some(timeout_ms) = wait else {
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            return Ok(0);
        };
        WaitForSingleObject(pi.hProcess, timeout_ms);
        let mut code = 0u32;
        GetExitCodeProcess(pi.hProcess, &mut code);
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        Ok(code)
    }
}
