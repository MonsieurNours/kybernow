//! Sorties audio de Windows (API MMDevice et `IPolicyConfig`).
//!
//! `IPolicyConfig` n'est pas documentée, mais c'est elle qu'utilisent le
//! panneau Son de Windows, Sunshine et Apollo pour choisir la sortie par
//! défaut et son format (nombre de canaux). Ces fonctions tournent dans
//! l'agent, dans la session de l'écran (voir `display/agent.rs`).

#![allow(non_snake_case)] // méthodes COM, noms imposés par Windows

use std::ffi::c_void;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, ERole, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
    WAVEFORMATEXTENSIBLE_0, eCommunications, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ};
use windows::core::{GUID, HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, PCWSTR, interface};

/// Nom de l'adaptateur des haut-parleurs virtuels de Valve.
pub const SPEAKERS_NAME: &str = "Steam Streaming Speakers";

const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
const SUBTYPE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

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

#[derive(Debug, Clone, Serialize)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    /// Canaux du format actuel de la sortie (2, 6, 8…).
    pub channels: u16,
}

#[derive(Debug, Serialize)]
pub struct Status {
    /// Haut-parleurs virtuels, s'ils sont installés.
    pub speakers: Option<Endpoint>,
    /// Sortie par défaut actuelle.
    pub default: Option<Endpoint>,
}

/// Ce qu'il faut pour revenir en arrière après une session.
#[derive(Debug, Serialize)]
pub struct Routed {
    pub previous: String,
    /// Format d'origine des haut-parleurs virtuels (octets en hexadécimal).
    pub previous_format: Option<String>,
    pub speakers: String,
    pub channels: u16,
}

struct Audio {
    enumerator: IMMDeviceEnumerator,
    policy: IPolicyConfig,
}

impl Audio {
    fn new() -> Result<Self> {
        unsafe {
            // Déjà initialisé dans ce fil : sans conséquence.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).context("énumération des sorties audio")?;
            let policy: IPolicyConfig =
                CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).context("réglages audio de Windows")?;
            Ok(Self { enumerator, policy })
        }
    }

    fn endpoint(&self, device: &IMMDevice) -> Result<Endpoint> {
        unsafe {
            let raw = device.GetId()?;
            let id = raw.to_string()?;
            CoTaskMemFree(Some(raw.0 as *const c_void));
            let store = device.OpenPropertyStore(STGM_READ)?;
            let name = store.GetValue(&PKEY_Device_FriendlyName).map(|v| v.to_string()).unwrap_or_default();
            let channels = self.mix_channels(&id).unwrap_or(0);
            Ok(Endpoint { id, name, channels })
        }
    }

    fn mix_channels(&self, id: &str) -> Result<u16> {
        let id = HSTRING::from(id);
        unsafe {
            let mut format: *mut WAVEFORMATEX = std::ptr::null_mut();
            self.policy.GetMixFormat(PCWSTR(id.as_ptr()), &mut format).ok()?;
            if format.is_null() {
                bail!("format inconnu");
            }
            let channels = std::ptr::read_unaligned(std::ptr::addr_of!((*format).nChannels));
            CoTaskMemFree(Some(format as *const c_void));
            Ok(channels)
        }
    }

    fn active(&self) -> Result<Vec<Endpoint>> {
        unsafe {
            let all = self.enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
            let mut out = Vec::new();
            for i in 0..all.GetCount()? {
                if let Ok(e) = self.endpoint(&all.Item(i)?) {
                    out.push(e);
                }
            }
            Ok(out)
        }
    }

    fn default(&self) -> Option<Endpoint> {
        unsafe {
            let device = self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            self.endpoint(&device).ok()
        }
    }

    fn set_default(&self, id: &str) -> Result<()> {
        let wide = HSTRING::from(id);
        for role in [eConsole, eMultimedia, eCommunications] {
            unsafe { self.policy.SetDefaultEndpoint(PCWSTR(wide.as_ptr()), role).ok() }
                .context("choix de la sortie audio par défaut")?;
        }
        Ok(())
    }

    fn device_format(&self, id: &str) -> Option<Vec<u8>> {
        let id = HSTRING::from(id);
        unsafe {
            let mut format: *mut WAVEFORMATEX = std::ptr::null_mut();
            self.policy.GetDeviceFormat(PCWSTR(id.as_ptr()), 0, &mut format).ok().ok()?;
            if format.is_null() {
                return None;
            }
            let extra = std::ptr::read_unaligned(std::ptr::addr_of!((*format).cbSize)) as usize;
            let len = size_of::<WAVEFORMATEX>() + extra;
            let bytes = std::slice::from_raw_parts(format as *const u8, len).to_vec();
            CoTaskMemFree(Some(format as *const c_void));
            Some(bytes)
        }
    }

    fn set_device_format(&self, id: &str, format: *const WAVEFORMATEX) -> Result<()> {
        let id = HSTRING::from(id);
        unsafe { self.policy.SetDeviceFormat(PCWSTR(id.as_ptr()), format, format).ok()? };
        Ok(())
    }

    /// Règle `id` sur `channels` canaux à 48 kHz, en essayant les formats
    /// usuels du meilleur au plus simple (comme Sunshine).
    fn set_channels(&self, id: &str, channels: u16) -> Result<()> {
        let masks: &[u32] = match channels {
            8 => &[0x63F],
            6 => &[0x3F, 0x60F],
            _ => &[0x3],
        };
        let depths: &[(GUID, u16, u16)] = &[(SUBTYPE_FLOAT, 32, 32), (SUBTYPE_PCM, 32, 24), (SUBTYPE_PCM, 24, 24), (SUBTYPE_PCM, 16, 16)];
        let mut last = None;
        for &(subformat, bits, valid) in depths {
            for &mask in masks {
                let format = extensible(channels, bits, valid, mask, subformat);
                match self.set_device_format(id, &format.Format) {
                    Ok(()) => return Ok(()),
                    Err(e) => last = Some(e),
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("aucun format accepté")))
            .with_context(|| format!("la sortie refuse le format à {channels} canaux"))
    }
}

fn extensible(channels: u16, bits: u16, valid: u16, mask: u32, subformat: GUID) -> WAVEFORMATEXTENSIBLE {
    let block = channels * bits / 8;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE,
            nChannels: channels,
            nSamplesPerSec: 48_000,
            nAvgBytesPerSec: 48_000 * u32::from(block),
            nBlockAlign: block,
            wBitsPerSample: bits,
            cbSize: (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: valid },
        dwChannelMask: mask,
        SubFormat: subformat,
    }
}

fn is_speakers(e: &Endpoint) -> bool {
    e.name.contains(SPEAKERS_NAME)
}

pub fn status() -> Result<Status> {
    let audio = Audio::new()?;
    let speakers = audio.active()?.into_iter().find(is_speakers);
    Ok(Status { speakers, default: audio.default() })
}

/// Envoie le son de Windows vers les haut-parleurs virtuels, à `channels` canaux.
pub fn route(channels: u16) -> Result<Routed> {
    let audio = Audio::new()?;
    let all = audio.active()?;
    let Some(speakers) = all.iter().find(|e| is_speakers(e)).cloned() else {
        bail!("les haut-parleurs virtuels ({SPEAKERS_NAME}) ne sont pas installés");
    };
    // Après un plantage, ils peuvent déjà être la sortie par défaut : on
    // reviendra alors sur la première vraie sortie.
    let previous = match audio.default() {
        Some(d) if !is_speakers(&d) => d.id,
        _ => all.iter().find(|e| !is_speakers(e)).map(|e| e.id.clone()).unwrap_or_default(),
    };
    let previous_format = audio.device_format(&speakers.id).map(hex::encode);
    let channels = match channels {
        6 | 8 => channels,
        _ => 2,
    };
    if let Err(e) = audio.set_channels(&speakers.id, channels) {
        tracing::warn!("{e:#} : format actuel conservé");
    }
    audio.set_default(&speakers.id)?;
    let channels = audio.mix_channels(&speakers.id).unwrap_or(channels);
    Ok(Routed { previous, previous_format, speakers: speakers.id, channels })
}

/// Rétablit la sortie d'avant la session, et le format des haut-parleurs virtuels.
pub fn restore(previous: &str, previous_format: Option<&str>, speakers: &str) -> Result<()> {
    let audio = Audio::new()?;
    if let Some(bytes) = previous_format.and_then(|h| hex::decode(h).ok()) {
        if bytes.len() >= size_of::<WAVEFORMATEX>() {
            let _ = audio.set_device_format(speakers, bytes.as_ptr() as *const WAVEFORMATEX);
        }
    }
    // La sortie d'origine peut revenir avec les écrans physiques : on lui laisse 3 s.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !previous.is_empty()
        && !audio.active()?.iter().any(|e| e.id == previous)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let target = if previous.is_empty() || !audio.active()?.iter().any(|e| e.id == previous) {
        // Sortie d'origine disparue (écran éteint, casque débranché) : la première autre.
        audio.active()?.into_iter().find(|e| !is_speakers(e)).map(|e| e.id)
    } else {
        Some(previous.to_string())
    };
    match target {
        Some(id) => audio.set_default(&id),
        None => Ok(()), // aucune autre sortie : rien à rétablir
    }
}
