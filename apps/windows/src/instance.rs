//! Une seule app par utilisateur et par session Windows. Une deuxième
//! ouverture (lien `kybernow://`, liste de la barre des tâches, notification,
//! raccourci) passe ses arguments à l'app déjà ouverte par un tube nommé
//! local, puis se ferme.
//!
//! Le mutex `Local\KyberNow.App` dit aussi à l'installateur que l'app tourne
//! (il attend sa fermeture avant de remplacer les fichiers).

use std::io::{Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAGS_AND_ATTRIBUTES, PIPE_ACCESS_INBOUND};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
use windows::core::{HSTRING, w};

type Sink = Box<dyn Fn(Vec<String>) + Send>;

/// Demandes arrivées avant que l'app soit prête, et qui les reçoit ensuite.
static QUEUE: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

fn session_of(pid: u32) -> Option<u32> {
    let mut sid = 0u32;
    unsafe { ProcessIdToSessionId(pid, &mut sid).ok().map(|_| sid) }
}

/// Un tube par utilisateur et par session (console et bureau à distance
/// ont chacun leur app).
fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let session = session_of(unsafe { GetCurrentProcessId() }).unwrap_or(0);
    format!(r"\\.\pipe\KyberNow-{user}-{session}")
}

/// Attend (2 minutes au plus) que l'installateur de KyberNow ait fini : il
/// tient `Local\KyberNow.Setup` pendant l'installation.
pub fn wait_for_setup() {
    use windows::Win32::System::Threading::{OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(120) {
        match unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, w!("Local\\KyberNow.Setup")) } {
            Ok(h) => unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(h);
            },
            Err(_) => return,
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// `true` : cette app est la première (elle garde le mutex jusqu'à sa fin).
pub fn claim() -> bool {
    unsafe {
        match CreateMutexW(None, false, w!("Local\\KyberNow.App")) {
            // Le descripteur reste ouvert pour toute la vie du processus.
            Ok(_handle) => GetLastError() != ERROR_ALREADY_EXISTS,
            Err(_) => true,
        }
    }
}

/// Passe les arguments à l'app déjà ouverte (elle a 15 s pour écouter, le
/// temps d'un premier démarrage lent).
pub fn forward(args: &[String]) -> bool {
    let line = serde_json::to_string(args).unwrap_or_else(|_| "[]".into());
    let me = session_of(unsafe { GetCurrentProcessId() });
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(15) {
        if let Ok(mut pipe) = std::fs::OpenOptions::new().write(true).open(pipe_name()) {
            // Le tube doit appartenir à un processus de cette session.
            let mut pid = 0u32;
            let server = unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid) }.is_ok();
            if !server || session_of(pid) != me {
                return false;
            }
            unsafe {
                let _ = AllowSetForegroundWindow(ASFW_ANY);
            }
            return pipe.write_all(line.as_bytes()).is_ok();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn deliver(args: Vec<String>) {
    match SINK.lock().unwrap().as_ref() {
        Some(sink) => sink(args),
        None => QUEUE.lock().unwrap().push(args),
    }
}

/// L'app est prête : elle reçoit ce qui attendait, puis les suivantes.
pub fn set_sink(sink: impl Fn(Vec<String>) + Send + 'static) {
    // D'abord le destinataire (plus rien ne s'empile), puis ce qui attendait.
    *SINK.lock().unwrap() = Some(Box::new(sink));
    let waiting: Vec<Vec<String>> = QUEUE.lock().unwrap().drain(..).collect();
    for args in waiting {
        deliver(args);
    }
}

/// Écoute les ouvertures suivantes, dès le lancement (avant la fenêtre).
pub fn listen() {
    let name = HSTRING::from(pipe_name());
    std::thread::Builder::new()
        .name("kybernow-instance".into())
        .spawn(move || {
            loop {
                // Une seule instance du tube à la fois, et la première : un
                // autre programme qui aurait pris ce nom ne reçoit rien.
                let handle: HANDLE = unsafe {
                    CreateNamedPipeW(
                        &name,
                        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_INBOUND.0 | FILE_FLAG_FIRST_PIPE_INSTANCE.0),
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                        1,
                        0,
                        64 * 1024,
                        0,
                        None,
                    )
                };
                if handle == INVALID_HANDLE_VALUE || handle.is_invalid() {
                    log::warn!("instance unique : tube indisponible");
                    return;
                }
                // Le client a pu se connecter juste avant : ERROR_PIPE_CONNECTED.
                let _ = unsafe { ConnectNamedPipe(handle, None) };
                let mut file = unsafe { std::fs::File::from_raw_handle(handle.0) };
                let mut text = String::new();
                let _ = (&mut file).take(64 * 1024).read_to_string(&mut text);
                drop(file);
                match serde_json::from_str::<Vec<String>>(text.trim()) {
                    Ok(args) => {
                        log::info!("autre ouverture : {args:?}");
                        deliver(args);
                    }
                    Err(e) => log::warn!("instance unique : message illisible ({e})"),
                }
            }
        })
        .expect("fil de l'instance unique");
}
