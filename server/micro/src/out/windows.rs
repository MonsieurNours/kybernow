//! Windows : VB-CABLE (VB-Audio Software, donationware : vb-cable.com). Le son
//! est joué en WASAPI partagé dans sa sortie « CABLE Input » ; les apps de
//! l'ordinateur l'entendent dans son entrée « CABLE Output ». Le micro par
//! défaut passe par `IPolicyConfig`, comme la sortie par défaut d'Admin
//! (server/admin/src/audio/endpoint.rs).

#![allow(non_snake_case)] // méthodes COM, noms imposés par Windows

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE, EDataFlow, ERole, IAudioClient, IAudioRenderClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, eCapture, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, ReleaseMutex, WaitForSingleObject,
};
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

use super::Check;
use crate::RATE;
use crate::jitter::Jitter;

/// La sortie où l'on joue et l'entrée que les apps prennent.
const CABLE_IN: &str = "CABLE Input";
const CABLE_OUT: &str = "CABLE Output";

const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, device: PCWSTR, format: *mut *mut WAVEFORMATEX) -> HRESULT;
    fn GetDeviceFormat(&self, device: PCWSTR, default: i32, format: *mut *mut WAVEFORMATEX) -> HRESULT;
    fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, device: PCWSTR, endpoint: *const WAVEFORMATEX, mix: *const WAVEFORMATEX) -> HRESULT;
    fn GetProcessingPeriod(&self, device: PCWSTR, default: i32, period: *mut i64, minimum: *mut i64) -> HRESULT;
    fn SetProcessingPeriod(&self, device: PCWSTR, period: *const i64) -> HRESULT;
    fn GetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    fn SetShareMode(&self, device: PCWSTR, mode: *const c_void) -> HRESULT;
    fn GetPropertyValue(&self, device: PCWSTR, key: *const PROPERTYKEY, value: *mut c_void) -> HRESULT;
    fn SetPropertyValue(&self, device: PCWSTR, key: *const PROPERTYKEY, value: *const c_void) -> HRESULT;
    fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
    fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
}

fn enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe {
        // Déjà initialisé dans ce fil : sans conséquence.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| format!("appareils audio : {e}"))
    }
}

fn id_of(device: &IMMDevice) -> Option<String> {
    unsafe {
        let raw = device.GetId().ok()?;
        let id = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const c_void));
        id
    }
}

/// Le premier appareil actif de ce sens dont le nom contient `name`.
fn find(enumerator: &IMMDeviceEnumerator, flow: EDataFlow, name: &str) -> Option<(IMMDevice, String, String)> {
    unsafe {
        let all = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE).ok()?;
        for i in 0..all.GetCount().ok()? {
            let Ok(device) = all.Item(i) else { continue };
            let Ok(store) = device.OpenPropertyStore(STGM_READ) else { continue };
            let friendly = store.GetValue(&PKEY_Device_FriendlyName).map(|v| v.to_string()).unwrap_or_default();
            if friendly.contains(name)
                && let Some(id) = id_of(&device)
            {
                return Some((device, id, friendly));
            }
        }
        None
    }
}

/// Registre du micro par défaut (`crate::ledger`) : dans le dossier de
/// données de la Station, sous un verrou entre processus.
fn ledger_path() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into())).join("KyberNow").join("micro-defaut.txt")
}

/// Une aide tourne-t-elle encore ?
fn alive(pid: u32) -> bool {
    const STILL_ACTIVE: u32 = 259;
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut code = 0u32;
        let running = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE;
        let _ = CloseHandle(h);
        running
    }
}

/// Lit le registre, le passe à `f`, puis l'écrit (ou l'efface s'il est vide),
/// sous le verrou `Global\KyberNowMicroDefault`.
fn with_ledger<T>(f: impl FnOnce(&mut crate::ledger::Ledger) -> T) -> T {
    let name = HSTRING::from("Global\\KyberNowMicroDefault");
    let mutex = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())).ok() };
    if let Some(m) = mutex {
        unsafe { WaitForSingleObject(m, 5000) };
    }
    let path = ledger_path();
    let mut ledger = crate::ledger::Ledger::parse(&std::fs::read_to_string(&path).unwrap_or_default());
    let out = f(&mut ledger);
    if ledger.is_empty() {
        let _ = std::fs::remove_file(&path);
    } else {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, ledger.to_text());
    }
    if let Some(m) = mutex {
        unsafe {
            let _ = ReleaseMutex(m);
            let _ = CloseHandle(m);
        }
    }
    out
}

/// Rend le micro d'avant si plus aucune aide ne tourne (`pid` : celle qui s'arrête).
fn give_back(pid: Option<u32>) {
    let Some(previous) = with_ledger(|l| l.leave(pid, alive)) else { return };
    let (Ok(enumerator), Some(policy)) = (enumerator(), policy()) else { return };
    let cable = find(&enumerator, eCapture, CABLE_OUT).map(|(_, id, _)| id);
    // Seulement si CABLE l'est encore : la personne a pu choisir un autre micro.
    if cable.is_some() && default_capture(&enumerator) == cable && set_default(&policy, &previous) {
        eprintln!("micro par défaut rendu");
    }
}

pub fn check() -> Check {
    // Une aide tuée (fin de session brutale) n'a pas rendu le micro : Admin
    // lance `--check` à son démarrage et pour sa page, qui le rend.
    give_back(None);
    let enumerator = match enumerator() {
        Ok(e) => e,
        Err(e) => return Check::Unsupported(e),
    };
    match (find(&enumerator, eRender, CABLE_IN), find(&enumerator, eCapture, CABLE_OUT)) {
        (Some(_), Some((_, _, name))) => Check::Ready(name),
        _ => Check::Missing("VB-CABLE n'est pas installé (CABLE Input / CABLE Output absents)".into()),
    }
}

pub struct Player {
    name: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Cette aide est notée dans le registre du micro par défaut.
    entered: bool,
}

/// Joue le tampon dans « CABLE Input » jusqu'à `stop`, sur un fil à lui.
fn render(device: IMMDevice, jitter: Arc<Mutex<Jitter>>, stop: Arc<AtomicBool>) -> Result<(), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(|e| format!("CABLE Input : {e}"))?;
        // Mono en flottants à 48 kHz ; Windows convertit vers le format de VB-CABLE.
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_IEEE_FLOAT,
            nChannels: 1,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * 4,
            nBlockAlign: 4,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        // 20 ms de tampon chez Windows, en unités de 100 ns.
        let duration = 200_000i64;
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                duration,
                0,
                &format,
                None,
            )
            .map_err(|e| format!("CABLE Input refuse le format : {e}"))?;
        let event = CreateEventW(None, false, false, None).map_err(|e| e.to_string())?;
        // L'événement est fermé même si la suite échoue.
        let ready = (|| -> Result<(u32, IAudioRenderClient), String> {
            client.SetEventHandle(event).map_err(|e| e.to_string())?;
            let size = client.GetBufferSize().map_err(|e| e.to_string())?;
            let renderer: IAudioRenderClient = client.GetService().map_err(|e| e.to_string())?;
            client.Start().map_err(|e| e.to_string())?;
            Ok((size, renderer))
        })();
        let (size, renderer) = match ready {
            Ok(r) => r,
            Err(e) => {
                let _ = CloseHandle(event);
                return Err(e);
            }
        };
        let mut scratch = vec![0f32; size as usize];
        while !stop.load(Ordering::Relaxed) {
            if WaitForSingleObject(event, 100) != WAIT_OBJECT_0 {
                continue;
            }
            let Ok(padding) = client.GetCurrentPadding() else { break };
            let free = size.saturating_sub(padding);
            if free == 0 {
                continue;
            }
            let Ok(buffer) = renderer.GetBuffer(free) else { break };
            let out = &mut scratch[..free as usize];
            // Jamais bloqué par le décodage : du silence pour cette fois.
            super::pull(&jitter, out);
            std::ptr::copy_nonoverlapping(out.as_ptr(), buffer as *mut f32, free as usize);
            let _ = renderer.ReleaseBuffer(free, 0);
        }
        let _ = client.Stop();
        let _ = CloseHandle(event);
        Ok(())
    }
}

fn policy() -> Option<IPolicyConfig> {
    unsafe { CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).ok() }
}

fn set_default(policy: &IPolicyConfig, id: &str) -> bool {
    let wide = HSTRING::from(id);
    [eConsole, eMultimedia, eCommunications]
        .into_iter()
        .all(|role| unsafe { policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role).is_ok() })
}

fn default_capture(enumerator: &IMMDeviceEnumerator) -> Option<String> {
    unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole).ok().and_then(|d| id_of(&d)) }
}

impl Player {
    pub fn start(jitter: Arc<Mutex<Jitter>>) -> Result<Self, String> {
        let devices = enumerator()?;
        let (_, _, name) = find(&devices, eCapture, CABLE_OUT).ok_or("VB-CABLE n'est pas installé")?;
        // Le fil de rendu ouvre son propre appareil (COM, un objet par fil).
        find(&devices, eRender, CABLE_IN).ok_or("VB-CABLE n'est pas installé")?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("micro-wasapi".into())
            .spawn(move || {
                let device = match enumerator().ok().and_then(|e| find(&e, eRender, CABLE_IN)) {
                    Some((d, _, _)) => d,
                    None => {
                        eprintln!("CABLE Input a disparu");
                        return;
                    }
                };
                if let Err(e) = render(device, jitter, flag) {
                    eprintln!("{e}");
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            name,
            stop,
            thread: Some(thread),
            entered: false,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn make_default(&mut self) {
        let (Ok(enumerator), Some(policy)) = (enumerator(), policy()) else {
            eprintln!("micro par défaut inchangé : réglages audio de Windows injoignables");
            return;
        };
        let Some((_, cable, _)) = find(&enumerator, eCapture, CABLE_OUT) else {
            return;
        };
        let current = default_capture(&enumerator);
        let pid = std::process::id();
        let set = with_ledger(|l| l.enter(pid, current.as_deref(), &cable, alive));
        self.entered = true;
        if !set {
            return;
        }
        if set_default(&policy, &cable) {
            eprintln!("micro par défaut : {}", self.name);
        } else {
            eprintln!("micro par défaut inchangé : Windows refuse");
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if self.entered {
            give_back(Some(std::process::id()));
        }
    }
}
