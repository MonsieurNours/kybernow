//! État des cartes graphiques, quelle que soit leur marque.
//!
//! - Les cartes (nom, pilote) : `Win32_VideoController`, triées comme à
//!   l'installation (NVIDIA, puis AMD, puis Intel) : la première est celle qui
//!   encode. Relue toutes les 5 minutes par un fil dédié.
//! - L'activité de chaque carte (charge, encodage, décodage) : compteurs
//!   « GPU Engine » de Windows (WDDM 2.x), les mêmes que le Gestionnaire des
//!   tâches, communs à NVIDIA, AMD et Intel. Relevés chaque seconde par le
//!   même fil, et rattachés à leur carte par DXGI (LUID).
//! - Un flux en cours : kyavserver occupe le moteur d'encodage vidéo d'une
//!   carte (ou NVML compte une session NVENC).
//! - Les mesures propres à NVIDIA (sessions et images par seconde NVENC,
//!   latence, état de performance, température, puissance, mémoire) : NVML,
//!   chargé dynamiquement depuis le pilote.

// Hors Windows (Mac de développement), seul NVML sert : la lecture des
// réponses de Windows n'y est que testée.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{PerformanceState, TemperatureSensor};
use serde::Serialize;

/// Un flux reste « en cours » ce temps après la dernière image encodée :
/// sur un écran immobile, l'encodeur peut ne rien avoir à faire.
const ENCODE_HOLD: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    // Du plus performant au moins performant, comme l'installateur.
    Nvidia,
    Amd,
    Intel,
}

impl Vendor {
    fn from_pci(id: u16) -> Option<Self> {
        match id {
            0x10DE => Some(Self::Nvidia),
            0x1002 | 0x1022 => Some(Self::Amd),
            0x8086 => Some(Self::Intel),
            _ => None,
        }
    }

    fn fallback_name(self) -> &'static str {
        match self {
            Self::Nvidia => "Carte NVIDIA",
            Self::Amd => "Carte AMD",
            Self::Intel => "Carte Intel",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct GpuStatus {
    /// Pilote de la première carte (celle qui encode).
    pub driver: Option<String>,
    pub devices: Vec<GpuDevice>,
}

#[derive(Debug, Serialize)]
pub struct GpuDevice {
    pub name: String,
    pub vendor: Vendor,
    pub driver: Option<String>,
    /// Un flux de Kyber est encodé par cette carte.
    pub encoding: bool,
    pub gpu_utilization: Option<u32>,
    pub encoder_utilization: Option<u32>,
    pub decoder_utilization: Option<u32>,
    /// Sessions NVENC ouvertes (une par flux en cours). NVIDIA seulement.
    pub encoder_sessions: Option<u32>,
    pub encoder_fps: Option<u32>,
    /// Latence moyenne d'encodage rapportée par le pilote, en microsecondes.
    pub encoder_latency_us: Option<u32>,
    pub temperature_c: Option<u32>,
    pub memory_used_mb: Option<u64>,
    pub memory_total_mb: Option<u64>,
    pub power_w: Option<f32>,
    /// État de performance NVIDIA : 0 (P0, puissance maximale) à 15 (P15, minimum).
    pub pstate: Option<u8>,
}

impl GpuDevice {
    fn new(vendor: Vendor, name: String, driver: Option<String>) -> Self {
        GpuDevice {
            name,
            vendor,
            driver,
            encoding: false,
            gpu_utilization: None,
            encoder_utilization: None,
            decoder_utilization: None,
            encoder_sessions: None,
            encoder_fps: None,
            encoder_latency_us: None,
            temperature_c: None,
            memory_used_mb: None,
            memory_total_mb: None,
            power_w: None,
            pstate: None,
        }
    }
}

/// Une carte telle que Windows la décrit.
#[derive(Debug, Clone, PartialEq)]
struct Adapter {
    vendor: Vendor,
    device_id: Option<u16>,
    name: String,
    driver: Option<String>,
    /// Identifiant de la carte pour DXGI et les compteurs « GPU Engine ».
    luid: Option<u64>,
}

/// Activité d'une carte, en pourcentage (comme le Gestionnaire des tâches :
/// le moteur le plus occupé de chaque sorte).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Usage {
    gpu: f64,
    encode: f64,
    decode: f64,
}

#[derive(Default)]
struct Shared {
    adapters: Vec<Adapter>,
    usage: HashMap<u64, Usage>,
    /// Dernière image encodée par kyavserver, par carte.
    last_encode: HashMap<u64, Instant>,
}

impl Shared {
    fn encoding(&self, luid: u64) -> bool {
        self.last_encode.get(&luid).is_some_and(|t| t.elapsed() < ENCODE_HOLD)
    }
}

fn shared() -> &'static Mutex<Shared> {
    static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
    SHARED.get_or_init(|| {
        #[cfg(windows)]
        std::thread::spawn(windows_monitor::run);
        Mutex::new(Shared::default())
    })
}

/// Lance la surveillance des cartes (sinon elle démarre au premier appel).
pub fn start() {
    shared();
}

fn nvml() -> Option<&'static Nvml> {
    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();
    NVML.get_or_init(|| match Nvml::init() {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::info!("NVML indisponible ({e}) : pas de mesures propres à NVIDIA");
            None
        }
    })
    .as_ref()
}

/// Nombre de flux encodés par NVENC en ce moment (tous GPU confondus).
fn nvenc_sessions() -> Option<u32> {
    let nvml = nvml()?;
    let count = nvml.device_count().ok()?;
    Some(
        (0..count)
            .filter_map(|i| nvml.device_by_index(i).ok())
            .filter_map(|d| d.encoder_stats().ok())
            .map(|s| s.session_count)
            .sum(),
    )
}

/// Un flux est-il encodé en ce moment, sur n'importe quelle carte ?
/// Appel bloquant (NVML) mais rapide : appelé chaque seconde par la
/// surveillance de l'écran virtuel et du son.
pub fn encoding() -> bool {
    let recent = {
        let s = shared().lock().unwrap();
        s.last_encode.values().any(|t| t.elapsed() < ENCODE_HOLD)
    };
    recent || nvenc_sessions().unwrap_or(0) > 0
}

/// Appel bloquant : à lancer dans `spawn_blocking`.
pub fn status() -> Option<GpuStatus> {
    let mut devices = {
        let s = shared().lock().unwrap();
        s.adapters
            .iter()
            .map(|a| {
                let mut d = GpuDevice::new(a.vendor, a.name.clone(), a.driver.clone());
                if let Some(luid) = a.luid {
                    if let Some(u) = s.usage.get(&luid) {
                        d.gpu_utilization = Some(percent(u.gpu));
                        d.encoder_utilization = Some(percent(u.encode));
                        d.decoder_utilization = Some(percent(u.decode));
                    }
                    d.encoding = s.encoding(luid);
                }
                d
            })
            .collect::<Vec<_>>()
    };
    add_nvml(&mut devices);
    let driver = devices.first()?.driver.clone();
    Some(GpuStatus { driver, devices })
}

fn percent(value: f64) -> u32 {
    value.round().clamp(0.0, 100.0) as u32
}

/// Complète les cartes NVIDIA par NVML (rattachées par leur nom, sinon dans
/// l'ordre). Sans liste de Windows, NVML donne seul les cartes NVIDIA.
fn add_nvml(devices: &mut Vec<GpuDevice>) {
    let Some(nvml) = nvml() else { return };
    let count = nvml.device_count().unwrap_or(0);
    let nv: Vec<_> = (0..count).filter_map(|i| nvml.device_by_index(i).ok()).collect();
    let mut used = vec![false; devices.len()];
    for d in &nv {
        let name = d.name().unwrap_or_else(|_| Vendor::Nvidia.fallback_name().into());
        let slot = (0..devices.len())
            .find(|&i| !used[i] && devices[i].vendor == Vendor::Nvidia && devices[i].name == name)
            .or_else(|| (0..devices.len()).find(|&i| !used[i] && devices[i].vendor == Vendor::Nvidia));
        let target = match slot {
            Some(i) => {
                used[i] = true;
                &mut devices[i]
            }
            None => {
                let driver = nvml.sys_driver_version().ok();
                devices.push(GpuDevice::new(Vendor::Nvidia, name, driver));
                used.push(true);
                devices.last_mut().unwrap()
            }
        };
        let enc_stats = d.encoder_stats().ok();
        let memory = d.memory_info().ok();
        target.gpu_utilization = d.utilization_rates().ok().map(|u| u.gpu).or(target.gpu_utilization);
        target.encoder_utilization = d.encoder_utilization().ok().map(|u| u.utilization).or(target.encoder_utilization);
        target.decoder_utilization = d.decoder_utilization().ok().map(|u| u.utilization).or(target.decoder_utilization);
        target.encoder_sessions = enc_stats.as_ref().map(|s| s.session_count);
        target.encoder_fps = enc_stats.as_ref().map(|s| s.average_fps);
        target.encoder_latency_us = enc_stats.as_ref().map(|s| s.average_latency);
        target.encoding |= target.encoder_sessions.unwrap_or(0) > 0;
        target.temperature_c = d.temperature(TemperatureSensor::Gpu).ok();
        target.memory_used_mb = memory.as_ref().map(|m| m.used / 1_048_576);
        target.memory_total_mb = memory.as_ref().map(|m| m.total / 1_048_576);
        target.power_w = d.power_usage().ok().map(|mw| mw as f32 / 1000.0);
        target.pstate = d.performance_state().ok().and_then(pstate_number);
    }
    devices.sort_by_key(|d| d.vendor);
}

fn pstate_number(state: PerformanceState) -> Option<u8> {
    use PerformanceState::*;
    Some(match state {
        Zero => 0,
        One => 1,
        Two => 2,
        Three => 3,
        Four => 4,
        Five => 5,
        Six => 6,
        Seven => 7,
        Eight => 8,
        Nine => 9,
        Ten => 10,
        Eleven => 11,
        Twelve => 12,
        Thirteen => 13,
        Fourteen => 14,
        Fifteen => 15,
        Unknown => return None,
    })
}

// ---------------------------------------------------------------------------
// Lecture des réponses de Windows (testée sur toutes les plateformes)

/// Lit la liste de `Win32_VideoController`, avec le même tri que
/// l'installateur : cartes PCI des fabricants connus, en état de marche, avec
/// le pilote du fabricant. Les écrans virtuels, le bureau à distance et
/// l'adaptateur de base de Microsoft (`display.inf`) sont écartés.
fn parse_video_controllers(json: &str) -> Vec<Adapter> {
    // Une seule carte : PowerShell peut rendre un objet au lieu d'un tableau.
    let items = match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::Array(items)) => items,
        Ok(item @ serde_json::Value::Object(_)) => vec![item],
        _ => return Vec::new(),
    };
    let mut adapters: Vec<Adapter> = items
        .iter()
        .filter_map(|item| {
            let field = |key: &str| item.get(key).and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty());
            if item.get("ConfigManagerErrorCode").and_then(|v| v.as_u64()).is_some_and(|code| code != 0) {
                return None;
            }
            if field("InfFilename").is_some_and(|inf| inf.eq_ignore_ascii_case("display.inf")) {
                return None;
            }
            let pnp = field("PNPDeviceID")?.to_ascii_uppercase();
            let ids = pnp.strip_prefix(r"PCI\VEN_")?;
            let vendor = Vendor::from_pci(u16::from_str_radix(ids.get(..4)?, 16).ok()?)?;
            let device_id = ids
                .split('&')
                .find_map(|part| part.strip_prefix("DEV_"))
                .and_then(|dev| u16::from_str_radix(dev.get(..4)?, 16).ok());
            let driver = field("DriverVersion").map(|v| driver_version(vendor, v));
            let name = field("Name").unwrap_or(vendor.fallback_name()).to_string();
            Some(Adapter { vendor, device_id, name, driver, luid: None })
        })
        .collect();
    adapters.sort_by_key(|a| a.vendor);
    adapters
}

/// NVIDIA publie ses pilotes sous un autre numéro que celui de Windows :
/// 32.0.15.6094 est le pilote 560.94 (les cinq derniers chiffres).
fn driver_version(vendor: Vendor, windows: &str) -> String {
    if vendor == Vendor::Nvidia {
        let digits: String = windows.split('.').skip(2).collect();
        if digits.len() >= 5 && digits.bytes().all(|b| b.is_ascii_digit()) {
            let last = &digits[digits.len() - 5..];
            return format!("{}.{}", last[..3].trim_start_matches('0'), &last[3..]);
        }
    }
    windows.to_string()
}

/// Rattache chaque carte à son LUID d'après DXGI (fabricant et modèle PCI) ;
/// deux cartes identiques prennent les LUID dans l'ordre.
fn assign_luids(adapters: &mut [Adapter], dxgi: &[(u16, u16, u64)]) {
    let mut free: Vec<_> = dxgi.to_vec();
    for a in adapters.iter_mut() {
        let pos = free.iter().position(|&(ven, dev, _)| {
            Vendor::from_pci(ven) == Some(a.vendor) && a.device_id.is_none_or(|d| d == dev)
        });
        a.luid = pos.map(|p| free.remove(p).2);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineKind {
    Encode,
    Decode,
    /// Moteur unique encodage + décodage (« Video Codec » de certaines cartes AMD et Intel).
    Codec,
    Other,
}

/// Un relevé « GPU Engine » : un moteur d'une carte, pour un processus.
#[derive(Debug, Clone, PartialEq)]
struct EngineSample {
    pid: u32,
    luid: u64,
    engine: u32,
    kind: EngineKind,
    percent: f64,
}

/// `pid_1234_luid_0x00000000_0x0000C5E4_phys_0_eng_2_engtype_VideoEncode`
fn parse_engine(instance: &str, percent: f64) -> Option<EngineSample> {
    let rest = instance.strip_prefix("pid_")?;
    let (pid, rest) = rest.split_once("_luid_")?;
    let (high, rest) = rest.split_once('_')?;
    let (low, rest) = rest.split_once('_')?;
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).ok();
    let luid = (u64::from(hex(high)?) << 32) | u64::from(hex(low)?);
    let rest = &rest[rest.find("eng_")? + 4..];
    let (engine, engtype) = rest.split_once("_engtype_")?;
    let engtype = engtype.to_ascii_lowercase();
    let kind = if engtype.contains("encode") {
        EngineKind::Encode
    } else if engtype.contains("decode") {
        EngineKind::Decode
    } else if engtype.contains("codec") {
        EngineKind::Codec
    } else {
        EngineKind::Other
    };
    Some(EngineSample { pid: pid.parse().ok()?, luid, engine: engine.parse().ok()?, kind, percent })
}

/// Charge de chaque carte : chaque moteur additionne ses processus, puis la
/// carte prend son moteur le plus occupé (de toute sorte, d'encodage, de décodage).
fn usage_by_adapter(samples: &[EngineSample]) -> HashMap<u64, Usage> {
    let mut engines: HashMap<(u64, u32), (EngineKind, f64)> = HashMap::new();
    for s in samples {
        engines.entry((s.luid, s.engine)).or_insert((s.kind, 0.0)).1 += s.percent;
    }
    let mut usage: HashMap<u64, Usage> = HashMap::new();
    for ((luid, _), (kind, percent)) in engines {
        let u = usage.entry(luid).or_default();
        u.gpu = u.gpu.max(percent);
        if matches!(kind, EngineKind::Encode | EngineKind::Codec) {
            u.encode = u.encode.max(percent);
        }
        if matches!(kind, EngineKind::Decode | EngineKind::Codec) {
            u.decode = u.decode.max(percent);
        }
    }
    usage
}

/// Cartes dont le moteur d'encodage travaille pour l'un de ces processus.
fn encoding_adapters(samples: &[EngineSample], pids: &[u32]) -> Vec<u64> {
    let mut luids: Vec<u64> = samples
        .iter()
        .filter(|s| s.percent > 0.0 && matches!(s.kind, EngineKind::Encode | EngineKind::Codec) && pids.contains(&s.pid))
        .map(|s| s.luid)
        .collect();
    luids.sort_unstable();
    luids.dedup();
    luids
}

// ---------------------------------------------------------------------------
// Fil de surveillance (Windows)

#[cfg(windows)]
mod windows_monitor {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Performance::{
        PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER,
        PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
        PdhGetFormattedCounterArrayW, PdhOpenQueryW,
    };

    use super::{Adapter, encoding_adapters, parse_engine, shared, usage_by_adapter};

    /// Programme de Kyber qui capture et encode l'image.
    const ENCODER_PROCESS: &str = "kyavserver.exe";
    const PDH_FMT_NOCAP100: u32 = 0x0000_8000;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const ADAPTERS_EVERY: Duration = Duration::from_secs(300);

    pub fn run() {
        let mut query: Option<Query> = None;
        let mut adapters_at: Option<Instant> = None;
        let mut warned = false;
        loop {
            if adapters_at.is_none_or(|t| t.elapsed() > ADAPTERS_EVERY) {
                let adapters = adapters();
                shared().lock().unwrap().adapters = adapters;
                adapters_at = Some(Instant::now());
                // Requête neuve à chaque relecture des cartes (carte branchée, pilote mis à jour).
                query = None;
            }
            if query.is_none() {
                query = Query::open();
                if query.is_none() && !warned {
                    warned = true;
                    tracing::info!("compteurs « GPU Engine » indisponibles : activité des cartes inconnue hors NVIDIA");
                }
            }
            if let Some(q) = &query {
                let samples: Vec<_> = q.collect().into_iter().filter_map(|(name, v)| parse_engine(&name, v)).collect();
                let encoding = encoding_adapters(&samples, &process_ids(ENCODER_PROCESS));
                let mut s = shared().lock().unwrap();
                s.usage = usage_by_adapter(&samples);
                let now = Instant::now();
                for luid in encoding {
                    s.last_encode.insert(luid, now);
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn adapters() -> Vec<Adapter> {
        let json = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "ConvertTo-Json -Compress -InputObject @(Get-CimInstance Win32_VideoController | \
                 Select-Object Name,PNPDeviceID,DriverVersion,ConfigManagerErrorCode,InfFilename)",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let mut adapters = super::parse_video_controllers(&json);
        super::assign_luids(&mut adapters, &dxgi_adapters());
        adapters
    }

    /// (fabricant PCI, modèle PCI, LUID) de chaque carte vue par DXGI.
    fn dxgi_adapters() -> Vec<(u16, u16, u64)> {
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
        let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return Vec::new() };
        (0..)
            .map_while(|i| unsafe { factory.EnumAdapters1(i) }.ok())
            .filter_map(|a| unsafe { a.GetDesc1() }.ok())
            .map(|d| {
                let luid = (u64::from(d.AdapterLuid.HighPart as u32) << 32) | u64::from(d.AdapterLuid.LowPart);
                (d.VendorId as u16, d.DeviceId as u16, luid)
            })
            .collect()
    }

    fn process_ids(exe: &str) -> Vec<u32> {
        let mut pids = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return pids;
            }
            let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut ok = Process32FirstW(snap, &mut entry) != 0;
            while ok {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                    pids.push(entry.th32ProcessID);
                }
                ok = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
        pids
    }

    /// `\GPU Engine(*)\Utilization Percentage` : un pourcentage par moteur,
    /// carte et processus, calculé entre deux relevés.
    struct Query {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    // Les poignées PDH ne servent qu'à ce fil.
    unsafe impl Send for Query {}

    impl Query {
        fn open() -> Option<Self> {
            let path: Vec<u16> = r"\GPU Engine(*)\Utilization Percentage".encode_utf16().chain([0]).collect();
            let mut query: PDH_HQUERY = std::ptr::null_mut();
            let mut counter: PDH_HCOUNTER = std::ptr::null_mut();
            unsafe {
                if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                    return None;
                }
                if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 {
                    PdhCloseQuery(query);
                    return None;
                }
                // Premier relevé : référence pour le calcul du pourcentage.
                PdhCollectQueryData(query);
            }
            Some(Query { query, counter })
        }

        fn collect(&self) -> Vec<(String, f64)> {
            let format = PDH_FMT_DOUBLE | PDH_FMT_NOCAP100;
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return Vec::new();
                }
                let (mut size, mut count) = (0u32, 0u32);
                let status =
                    PdhGetFormattedCounterArrayW(self.counter, format, &mut size, &mut count, std::ptr::null_mut());
                if status != PDH_MORE_DATA || size == 0 {
                    return Vec::new();
                }
                // Tampon aligné : les éléments, puis les noms auxquels ils renvoient.
                let item = size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
                let mut buf: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity((size as usize).div_ceil(item));
                let status = PdhGetFormattedCounterArrayW(self.counter, format, &mut size, &mut count, buf.as_mut_ptr());
                if status != 0 {
                    return Vec::new();
                }
                std::slice::from_raw_parts(buf.as_ptr(), count as usize)
                    .iter()
                    .filter(|i| matches!(i.FmtValue.CStatus, PDH_CSTATUS_VALID_DATA | PDH_CSTATUS_NEW_DATA))
                    .filter(|i| !i.szName.is_null())
                    .map(|i| {
                        let len = (0..).take_while(|&n| *i.szName.add(n) != 0).count();
                        let name = String::from_utf16_lossy(std::slice::from_raw_parts(i.szName, len));
                        (name, i.FmtValue.Anonymous.doubleValue)
                    })
                    .collect()
            }
        }
    }

    impl Drop for Query {
        fn drop(&mut self) {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HYBRID: &str = r#"[
        {"Name":"Intel(R) UHD Graphics","PNPDeviceID":"PCI\\VEN_8086&DEV_9A60&SUBSYS_0A6D1028&REV_01\\3&11583659&0&10","DriverVersion":"31.0.101.5186","ConfigManagerErrorCode":0,"InfFilename":"oem42.inf"},
        {"Name":"NVIDIA GeForce RTX 4070 Laptop GPU","PNPDeviceID":"PCI\\VEN_10DE&DEV_2860&SUBSYS_0A6D1028&REV_A1\\4&2B1D6F0E&0&0008","DriverVersion":"32.0.15.6094","ConfigManagerErrorCode":0,"InfFilename":"oem55.inf"},
        {"Name":"SudoMaker Virtual Display Adapter","PNPDeviceID":"ROOT\\DISPLAY\\0000","DriverVersion":"1.0.0.0","ConfigManagerErrorCode":0,"InfFilename":"oem60.inf"}
    ]"#;

    #[test]
    fn cartes_dans_l_ordre_de_l_encodeur() {
        let a = parse_video_controllers(HYBRID);
        assert_eq!(a.len(), 2, "l'écran virtuel est écarté");
        assert_eq!(a[0].vendor, Vendor::Nvidia);
        assert_eq!(a[0].device_id, Some(0x2860));
        assert_eq!(a[0].driver.as_deref(), Some("560.94"));
        assert_eq!(a[1].name, "Intel(R) UHD Graphics");
        assert_eq!(a[1].driver.as_deref(), Some("31.0.101.5186"));
    }

    #[test]
    fn une_seule_carte_amd() {
        // PowerShell 5 rend un objet seul quand il n'y a qu'une carte.
        let json = r#"{"Name":"AMD Radeon RX 7900 XTX","PNPDeviceID":"PCI\\VEN_1002&DEV_744C","DriverVersion":"32.0.12011.1036","ConfigManagerErrorCode":0,"InfFilename":"oem7.inf"}"#;
        let a = parse_video_controllers(json);
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].vendor, a[0].device_id), (Vendor::Amd, Some(0x744C)));
        assert_eq!(a[0].driver.as_deref(), Some("32.0.12011.1036"));
    }

    #[test]
    fn cartes_inutilisables_ecartees() {
        let json = r#"[
            {"Name":"Microsoft Basic Display Adapter","PNPDeviceID":"PCI\\VEN_1002&DEV_73BF","DriverVersion":"10.0.22621.1","ConfigManagerErrorCode":0,"InfFilename":"display.inf"},
            {"Name":"NVIDIA GeForce RTX 3080","PNPDeviceID":"PCI\\VEN_10DE&DEV_2206","DriverVersion":"32.0.15.6094","ConfigManagerErrorCode":22,"InfFilename":"oem5.inf"},
            {"Name":"Microsoft Remote Display Adapter","PNPDeviceID":"SWD\\REMOTEDISPLAYENUM\\RDPIDD_INDIRECTDISPLAY","DriverVersion":"10.0.22621.1","ConfigManagerErrorCode":0,"InfFilename":"rdpidd.inf"}
        ]"#;
        assert!(parse_video_controllers(json).is_empty());
        assert!(parse_video_controllers("").is_empty());
    }

    #[test]
    fn luid_par_fabricant_et_modele() {
        let mut a = parse_video_controllers(HYBRID);
        // DXGI énumère aussi le rendu logiciel de Microsoft (0x1414).
        assign_luids(&mut a, &[(0x8086, 0x9A60, 0xC5E4), (0x1414, 0x008C, 0x1), (0x10DE, 0x2860, 0x1_0000_D1A2)]);
        assert_eq!(a[0].luid, Some(0x1_0000_D1A2));
        assert_eq!(a[1].luid, Some(0xC5E4));
    }

    #[test]
    fn lecture_des_compteurs() {
        let s = parse_engine("pid_4312_luid_0x00000001_0x0000C5E4_phys_0_eng_2_engtype_VideoEncode", 12.5).unwrap();
        assert_eq!(s, EngineSample { pid: 4312, luid: 0x1_0000_C5E4, engine: 2, kind: EngineKind::Encode, percent: 12.5 });
        let codec = parse_engine("pid_8_luid_0x00000000_0x00011A2B_phys_0_eng_4_engtype_Video Codec", 1.0).unwrap();
        assert_eq!((codec.kind, codec.engine), (EngineKind::Codec, 4));
        assert_eq!(parse_engine("pid_8_luid_0x00000000_0x00011A2B_phys_0_eng_0_engtype_3D", 0.0).unwrap().kind, EngineKind::Other);
        assert!(parse_engine("_Total", 0.0).is_none());
    }

    #[test]
    fn charge_et_flux_par_carte() {
        let samples: Vec<_> = [
            ("pid_100_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D", 30.0),
            ("pid_200_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D", 25.0),
            ("pid_100_luid_0x00000000_0x0000AAAA_phys_0_eng_3_engtype_VideoDecode", 40.0),
            ("pid_300_luid_0x00000000_0x0000AAAA_phys_0_eng_5_engtype_VideoEncode", 12.0),
            ("pid_100_luid_0x00000000_0x0000BBBB_phys_0_eng_1_engtype_VideoEncode", 7.0),
        ]
        .iter()
        .filter_map(|(n, v)| parse_engine(n, *v))
        .collect();
        let usage = usage_by_adapter(&samples);
        assert_eq!(usage[&0xAAAA], Usage { gpu: 55.0, encode: 12.0, decode: 40.0 });
        assert_eq!(usage[&0xBBBB].encode, 7.0);
        // Seul le processus de Kyber (300) compte comme un flux.
        assert_eq!(encoding_adapters(&samples, &[300]), vec![0xAAAA]);
        assert!(encoding_adapters(&samples, &[999]).is_empty());
    }
}
