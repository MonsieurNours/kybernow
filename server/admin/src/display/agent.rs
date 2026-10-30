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
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
use windows_sys::Win32::Security::{
    DuplicateTokenEx, SecurityImpersonation, SetTokenInformation, TOKEN_ALL_ACCESS, TokenPrimary, TokenSessionId,
};
use windows_sys::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW, GetCurrentProcess, GetCurrentProcessId,
    GetExitCodeProcess, OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
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

/// Jeton du compte connecté à la session console (à fermer par l'appelant).
fn user_token() -> Result<HANDLE> {
    use windows_sys::Win32::System::RemoteDesktop::WTSQueryUserToken;
    let session = unsafe { WTSGetActiveConsoleSessionId() };
    if session == 0xFFFF_FFFF {
        bail!("aucune session ouverte");
    }
    let mut token: HANDLE = null_mut();
    if unsafe { WTSQueryUserToken(session, &mut token) } == 0 {
        bail!("aucun compte connecté à la session {session} : {}", std::io::Error::last_os_error());
    }
    Ok(token)
}

/// SID du compte de `token` (« S-1-5-21-… »).
fn token_sid(token: HANDLE) -> Result<String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_USER, TokenUser};
    unsafe {
        let mut needed = 0u32;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed);
        if needed == 0 {
            bail!("compte du jeton : {}", std::io::Error::last_os_error());
        }
        let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
        if GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut _, needed, &mut needed) == 0 {
            bail!("compte du jeton : {}", std::io::Error::last_os_error());
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut text: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            bail!("SID du compte : {}", std::io::Error::last_os_error());
        }
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
        LocalFree(text as *mut _);
        Ok(sid)
    }
}

/// SID du compte de ce processus (l'agent lancé avec le compte de l'utilisateur).
pub fn own_sid() -> Result<String> {
    use windows_sys::Win32::Security::TOKEN_QUERY;
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            bail!("jeton du processus : {}", std::io::Error::last_os_error());
        }
        let sid = token_sid(token);
        CloseHandle(token);
        sid
    }
}

/// Le compte connecté à la session console, pour agir en son nom. Admin
/// (SYSTEM) ne touche aux fichiers du profil de l'utilisateur qu'avec son
/// identité : une jonction ou un lien posé dans ce dossier ne mène alors
/// qu'où l'utilisateur peut déjà écrire.
pub struct ConsoleUser {
    token: HANDLE,
}

// Le jeton n'est qu'un handle du noyau, utilisable depuis n'importe quel fil.
unsafe impl Send for ConsoleUser {}
unsafe impl Sync for ConsoleUser {}

impl ConsoleUser {
    pub fn open() -> Result<Self> {
        Ok(Self { token: user_token()? })
    }

    /// SID du compte (« S-1-5-21-… »).
    pub fn sid(&self) -> Result<String> {
        token_sid(self.token)
    }

    /// Exécute `f` avec l'identité de l'utilisateur sur ce fil, puis revient à
    /// celle du service (même si `f` panique).
    pub fn act<T>(&self, f: impl FnOnce() -> T) -> Result<T> {
        use windows_sys::Win32::Security::{ImpersonateLoggedOnUser, RevertToSelf};
        struct Revert;
        impl Drop for Revert {
            fn drop(&mut self) {
                unsafe { RevertToSelf() };
            }
        }
        if unsafe { ImpersonateLoggedOnUser(self.token) } == 0 {
            bail!("identité de l'utilisateur : {}", std::io::Error::last_os_error());
        }
        let _revert = Revert;
        Ok(f())
    }

    /// `AppData\Local\KyberNow\agent` de son profil, créé en son nom.
    pub fn agent_dir(&self) -> Result<PathBuf> {
        let dir = profile_dir(self.token)?.join("AppData").join("Local").join("KyberNow").join("agent");
        self.act(|| std::fs::create_dir_all(&dir))?.with_context(|| format!("création de {}", dir.display()))?;
        Ok(dir)
    }
}

impl Drop for ConsoleUser {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.token) };
    }
}

/// Dossier du profil du compte de `token`.
fn profile_dir(token: HANDLE) -> Result<PathBuf> {
    use windows_sys::Win32::UI::Shell::GetUserProfileDirectoryW;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    if unsafe { GetUserProfileDirectoryW(token, buf.as_mut_ptr(), &mut len) } == 0 {
        bail!("profil de l'utilisateur introuvable : {}", std::io::Error::last_os_error());
    }
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Ok(PathBuf::from(String::from_utf16_lossy(&buf[..end])))
}

/// `AppData\Local` du compte connecté à la session console.
pub fn user_local_app_data() -> Result<PathBuf> {
    let token = user_token()?;
    let dir = profile_dir(token);
    unsafe { CloseHandle(token) };
    Ok(dir?.join("AppData").join("Local"))
}

/// Lance `exe` dans la session de l'utilisateur connecté, avec son compte
/// (pas celui du service) : l'icône de la zone de notification.
pub fn launch_as_user(exe: &Path) -> Result<()> {
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    let token = user_token()?;
    unsafe {
        let mut env: *mut std::ffi::c_void = null_mut();
        CreateEnvironmentBlock(&mut env, token, 0);
        let result = spawn_with_token(token, env, &quote(&exe.display().to_string()), CREATE_UNICODE_ENVIRONMENT, None, "l'icône");
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
        CloseHandle(token);
        result.map(|_| ())
    }
}

/// Lance `exe args` dans la session de l'utilisateur connecté, avec son
/// compte, sans l'attendre : l'agent de console du prêt (`kybernow-tray.exe --console`).
pub fn launch_as_user_with(exe: &Path, args: &[String]) -> Result<()> {
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    let token = user_token()?;
    unsafe {
        let mut env: *mut std::ffi::c_void = null_mut();
        CreateEnvironmentBlock(&mut env, token, 0);
        let result = spawn_with_token(token, env, &command_line(exe, args), CREATE_UNICODE_ENVIRONMENT, None, "l'agent de console");
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
        CloseHandle(token);
        result.map(|_| ())
    }
}

/// Lance `display-agent args` **avec le compte de l'utilisateur** de la
/// session console (pas SYSTEM : le menu Démarrer, les apps du Store et les
/// jeux qu'on ouvre sont à lui), attend son résultat JSON au plus
/// `timeout_ms`, et le rend. Le résultat est écrit dans son profil
/// (`AppData\Local\KyberNow\agent`) : le dossier de données d'Admin lui est
/// fermé.
pub fn run_as_user(args: &[String], timeout_ms: u32) -> Result<Value> {
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    let exe = std::env::current_exe()?;
    let user = ConsoleUser::open()?;
    let dir = user.agent_dir()?;
    let out: PathBuf = dir.join(format!("agent-{}.json", rand::random::<u32>()));
    let mut full: Vec<String> = vec!["display-agent".into()];
    full.extend(args.iter().cloned());
    full.push("--out".into());
    full.push(out.display().to_string());
    let cmdline = command_line(&exe, &full);
    let code = unsafe {
        let mut env: *mut std::ffi::c_void = null_mut();
        CreateEnvironmentBlock(&mut env, user.token, 0);
        let result =
            spawn_with_token(user.token, env, &cmdline, CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, Some(timeout_ms), "l'agent utilisateur");
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
        result?
    };
    // Le résultat est dans le profil de l'utilisateur : lu et retiré en son nom.
    let text = user
        .act(|| {
            let text = std::fs::read_to_string(&out);
            let _ = std::fs::remove_file(&out);
            text
        })?
        .with_context(|| format!("l'agent utilisateur n'a rien renvoyé (code {code})"))?;
    let value: Value = serde_json::from_str(&text)?;
    if let Some(err) = value.get("error").and_then(Value::as_str) {
        bail!("{err}");
    }
    Ok(value)
}

/// Lance `display-agent args` avec le compte de l'utilisateur de la session
/// console, sans l'attendre (capture WGC, qui tourne le temps de la session).
pub fn spawn_as_user(user: &ConsoleUser, args: &[String]) -> Result<()> {
    use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    let exe = std::env::current_exe()?;
    let mut full: Vec<String> = vec!["display-agent".into()];
    full.extend(args.iter().cloned());
    let cmdline = command_line(&exe, &full);
    unsafe {
        let mut env: *mut std::ffi::c_void = null_mut();
        CreateEnvironmentBlock(&mut env, user.token, 0);
        let result = spawn_with_token(user.token, env, &cmdline, CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, None, "la capture WGC");
        if !env.is_null() {
            DestroyEnvironmentBlock(env);
        }
        result.map(|_| ())
    }
}

/// `"exe" arg1 "arg 2" …`
fn command_line(exe: &Path, args: &[String]) -> String {
    std::iter::once(quote(&exe.display().to_string())).chain(args.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ")
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
        let result = spawn_with_token(
            token,
            null_mut(),
            &command_line(exe, args),
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            wait,
            "l'agent d'affichage",
        );
        CloseHandle(token);
        result
    }
}

/// Crée le processus `cmdline` avec `token` (et le bloc d'environnement
/// `env`, ou celui du service), sur le bureau de la session du jeton. Avec
/// `wait`, l'attend au plus ce délai (ms) et rend son code de sortie ; sinon
/// rend 0 aussitôt.
unsafe fn spawn_with_token(
    token: HANDLE,
    env: *mut std::ffi::c_void,
    cmdline: &str,
    flags: u32,
    wait: Option<u32>,
    what: &str,
) -> Result<u32> {
    unsafe {
        let mut cmdline: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();
        let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain(std::iter::once(0)).collect();
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_mut_ptr();
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessAsUserW(token, null(), cmdline.as_mut_ptr(), null(), null(), 0, flags, env, null(), &si, &mut pi);
        if ok == 0 {
            bail!("lancement de {what} : {}", std::io::Error::last_os_error());
        }
        let Some(timeout_ms) = wait else {
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            return Ok(0);
        };
        let waited = WaitForSingleObject(pi.hProcess, timeout_ms);
        if waited == WAIT_TIMEOUT {
            // Un agent qui traîne (session en transition, écran verrouillé) ne
            // doit pas finir plus tard, hors de tout contrôle, et changer les écrans.
            TerminateProcess(pi.hProcess, 1);
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            bail!("{what} n'a pas répondu en {} s, arrêté", timeout_ms / 1000);
        }
        let mut code = 0u32;
        GetExitCodeProcess(pi.hProcess, &mut code);
        CloseHandle(pi.hThread);
        CloseHandle(pi.hProcess);
        Ok(code)
    }
}
