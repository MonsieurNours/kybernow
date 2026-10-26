//! Client du pilote d'écran virtuel SudoVDA (interface IOCTL d'Apollo,
//! `third-party/sudovda/sudovda-ioctl.h` dans les sources d'Apollo, protocole 0.2).
//!
//! Le pilote retire tout seul ses écrans s'il ne reçoit plus de signal de vie
//! (`ping`) pendant la durée de son chien de garde (3 s par défaut).

use std::ptr::{null, null_mut};

use anyhow::{Context, Result, bail};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LUID};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::core::GUID;

/// {e5bcc234-1e0c-418a-a0d4-ef8b7501414d}
const INTERFACE_GUID: GUID = GUID {
    data1: 0xe5bc_c234,
    data2: 0x1e0c,
    data3: 0x418a,
    data4: [0xa0, 0xd4, 0xef, 0x8b, 0x75, 0x01, 0x41, 0x4d],
};

/// CTL_CODE(FILE_DEVICE_UNKNOWN, fonction, METHOD_BUFFERED, FILE_ANY_ACCESS)
const fn ctl(function: u32) -> u32 {
    (0x22 << 16) | (function << 2)
}
const IOCTL_ADD: u32 = ctl(0x800);
const IOCTL_REMOVE: u32 = ctl(0x801);
const IOCTL_GET_WATCHDOG: u32 = ctl(0x803);
const IOCTL_PING: u32 = ctl(0x888);
const IOCTL_PROTOCOL_VERSION: u32 = ctl(0x8FF);

#[repr(C)]
struct AddParams {
    width: u32,
    height: u32,
    refresh_rate: u32,
    monitor_guid: GUID,
    device_name: [u8; 14],
    serial_number: [u8; 14],
}

#[repr(C)]
#[derive(Default)]
struct AddOut {
    adapter_luid: LUID,
    target_id: u32,
}

#[repr(C)]
struct RemoveParams {
    monitor_guid: GUID,
}

#[repr(C)]
#[derive(Default)]
struct Watchdog {
    timeout: u32,
    countdown: u32,
}

#[repr(C)]
#[derive(Default)]
struct ProtocolVersion {
    major: u8,
    minor: u8,
    incremental: u8,
    test_build: u8,
}

/// Écran créé : où le retrouver dans la configuration d'affichage de Windows.
#[derive(Debug, Clone, Copy)]
pub struct Created {
    pub adapter_luid_low: u32,
    pub adapter_luid_high: i32,
    pub target_id: u32,
}

pub struct Device(HANDLE);

// Le handle est un fichier noyau : utilisable depuis plusieurs fils.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub fn guid_from_bytes(b: [u8; 16]) -> GUID {
    GUID {
        data1: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        data2: u16::from_le_bytes([b[4], b[5]]),
        data3: u16::from_le_bytes([b[6], b[7]]),
        data4: [b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]],
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    for (i, b) in s.bytes().take(N - 1).enumerate() {
        out[i] = b;
    }
    out
}

/// Nom et numéro de série de l'écran de rang `slot`.
fn slot_names(slot: usize) -> (String, String) {
    let name = if slot == 0 { "KyberNow".to_string() } else { format!("KyberNow {}", slot + 1) };
    (name, format!("KYBERNOW-{:04}", slot + 1))
}

impl Device {
    /// Ouvre le pilote. Échoue s'il n'est pas installé.
    pub fn open() -> Result<Self> {
        let path = interface_path()?.context("pilote SudoVDA introuvable (non installé ?)")?;
        let handle = unsafe {
            CreateFileW(
                wide(&path).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            bail!("ouverture du pilote SudoVDA : {}", std::io::Error::last_os_error());
        }
        Ok(Self(handle))
    }

    fn ioctl<I, O>(&self, code: u32, input: Option<&I>, output: Option<&mut O>) -> Result<()> {
        let (in_ptr, in_len) = match input {
            Some(i) => (i as *const I as *const _, size_of::<I>() as u32),
            None => (null(), 0),
        };
        let (out_ptr, out_len) = match output {
            Some(o) => (o as *mut O as *mut _, size_of::<O>() as u32),
            None => (null_mut(), 0),
        };
        let mut returned = 0u32;
        let ok = unsafe { DeviceIoControl(self.0, code, in_ptr, in_len, out_ptr, out_len, &mut returned, null_mut()) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            return Err(anyhow::Error::new(err).context(format!("SudoVDA (code {code:#x})")));
        }
        Ok(())
    }

    pub fn protocol_version(&self) -> Result<String> {
        let mut v = ProtocolVersion::default();
        self.ioctl::<(), _>(IOCTL_PROTOCOL_VERSION, None, Some(&mut v))?;
        Ok(format!("{}.{}.{}{}", v.major, v.minor, v.incremental, if v.test_build != 0 { " (test)" } else { "" }))
    }

    /// (délai du chien de garde, compte à rebours) en secondes.
    pub fn watchdog(&self) -> Result<(u32, u32)> {
        let mut w = Watchdog::default();
        self.ioctl::<(), _>(IOCTL_GET_WATCHDOG, None, Some(&mut w))?;
        Ok((w.timeout, w.countdown))
    }

    pub fn ping(&self) -> Result<()> {
        self.ioctl::<(), ()>(IOCTL_PING, None, None)
    }

    /// Crée l'écran de rang `slot` (0 : le principal). Nom et numéro de série
    /// suivent le rang (« KyberNow », « KyberNow 2 »… ; `KYBERNOW-0001` à
    /// `-0004`) : Windows garde les réglages de chacun d'une session à l'autre.
    pub fn add(&self, guid: [u8; 16], slot: usize, width: u32, height: u32, refresh: u32) -> Result<Created> {
        let (name, serial) = slot_names(slot);
        let params = AddParams {
            width,
            height,
            refresh_rate: refresh,
            monitor_guid: guid_from_bytes(guid),
            device_name: fixed(&name),
            serial_number: fixed(&serial),
        };
        let mut out = AddOut::default();
        self.ioctl(IOCTL_ADD, Some(&params), Some(&mut out))?;
        Ok(Created {
            adapter_luid_low: out.adapter_luid.LowPart,
            adapter_luid_high: out.adapter_luid.HighPart,
            target_id: out.target_id,
        })
    }

    pub fn remove(&self, guid: [u8; 16]) -> Result<()> {
        let params = RemoveParams { monitor_guid: guid_from_bytes(guid) };
        match self.ioctl::<_, ()>(IOCTL_REMOVE, Some(&params), None) {
            // ERROR_NOT_FOUND : le pilote l'a déjà retiré (chien de garde, rechargement).
            Err(e) if e.downcast_ref::<std::io::Error>().and_then(|e| e.raw_os_error()) == Some(1168) => {
                tracing::warn!("l'écran virtuel avait déjà été retiré par le pilote");
                Ok(())
            }
            other => other,
        }
    }
}

/// Chemin du périphérique SudoVDA (`\\?\...`), s'il est présent.
fn interface_path() -> Result<Option<String>> {
    unsafe {
        let set = SetupDiGetClassDevsW(&INTERFACE_GUID, null(), null_mut(), DIGCF_PRESENT | DIGCF_DEVICEINTERFACE);
        if set == INVALID_HANDLE_VALUE as isize {
            bail!("énumération des périphériques : {}", std::io::Error::last_os_error());
        }
        let mut data = SP_DEVICE_INTERFACE_DATA { cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
        let found = SetupDiEnumDeviceInterfaces(set, null(), &INTERFACE_GUID, 0, &mut data) != 0;
        let path = if found {
            let mut required = 0u32;
            SetupDiGetDeviceInterfaceDetailW(set, &data, null_mut(), 0, &mut required, null_mut());
            // Tampon aligné sur 8 octets pour la structure à taille variable.
            let mut buf = vec![0u64; (required as usize).div_ceil(8) + 1];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(set, &data, detail, required, null_mut(), null_mut()) == 0 {
                None
            } else {
                let start = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
                let mut len = 0;
                while *start.add(len) != 0 {
                    len += 1;
                }
                Some(String::from_utf16_lossy(std::slice::from_raw_parts(start, len)))
            }
        } else {
            None
        };
        SetupDiDestroyDeviceInfoList(set);
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_codes_match_the_c_header() {
        assert_eq!(IOCTL_ADD, 0x0022_2000);
        assert_eq!(IOCTL_REMOVE, 0x0022_2004);
        assert_eq!(IOCTL_PING, 0x0022_2220);
        assert_eq!(IOCTL_PROTOCOL_VERSION, 0x0022_23FC);
        assert_eq!(size_of::<AddParams>(), 56);
        assert_eq!(size_of::<AddOut>(), 12);
    }

    #[test]
    fn noms_et_numeros_de_serie() {
        // L'écran principal garde ceux de toujours.
        assert_eq!(slot_names(0), ("KyberNow".into(), "KYBERNOW-0001".into()));
        assert_eq!(slot_names(3), ("KyberNow 4".into(), "KYBERNOW-0004".into()));
        // 13 caractères au plus (14 octets avec le zéro final).
        assert_eq!(fixed::<14>(&slot_names(3).1)[..13], *b"KYBERNOW-0004");
    }
}
