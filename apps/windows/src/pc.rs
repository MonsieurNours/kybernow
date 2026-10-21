//! Ce PC (carte « Ce PC », réglages des sessions) : écran, son, système, et
//! ce que la carte graphique décode.
//!
//! La sonde de la carte graphique (Direct3D 11 et décodage d'une image de
//! chaque format par FFmpeg) tourne dans un processus à part
//! (`kybernow --probe-json`) : un pilote qui plante n'emporte pas l'app, et
//! son code de sortie va au journal.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{audio, decoder, render, system};

#[derive(Debug, Clone, Serialize, Default)]
pub struct ThisPc {
    pub name: String,
    pub gpu: String,
    pub h264: bool,
    pub hevc: bool,
    pub hevc444: bool,
    pub av1: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub audio_channels: u16,
    pub os: String,
    pub on_battery: bool,
}

/// Résultat de la sonde de la carte graphique.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Gpu {
    pub gpu: String,
    pub h264: bool,
    pub hevc: bool,
    pub hevc444: bool,
    pub av1: bool,
}

/// Côté processus de sonde : la carte et ses décodeurs.
pub fn probe_gpu() -> Gpu {
    log::info!("sonde : appareil Direct3D 11");
    match render::create_device() {
        Ok((device, _)) => {
            let gpu = render::adapter_name(&device);
            log::info!("sonde : {gpu}, décodeurs");
            let p = decoder::probe(&device);
            Gpu { gpu, h264: p.h264, hevc: p.caps.hevc, hevc444: p.caps.hevc444, av1: p.caps.av1 }
        }
        Err(e) => {
            log::warn!("sonde : Direct3D 11 indisponible ({e})");
            Gpu::default()
        }
    }
}

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Lance la sonde dans un processus à part et lit son résultat.
fn probe_isolated() -> Result<Gpu, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .arg("--probe-json")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("lancement : {e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if started.elapsed() > PROBE_TIMEOUT {
            let _ = child.kill();
            return Err("pas de réponse en 20 s".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut out);
    }
    if !status.success() {
        let code = status.code().map(|c| format!("{:#010x}", c as u32)).unwrap_or_else(|| "?".into());
        return Err(format!("le processus de sonde s'est arrêté (code {code}), voir kybernow-probe.log"));
    }
    serde_json::from_str(out.trim()).map_err(|e| format!("réponse illisible : {e}"))
}

pub fn on_battery() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s).is_ok() && s.ACLineStatus == 0 }
}

/// Côté app : tout ce qu'on sait de ce PC.
pub fn this_pc() -> ThisPc {
    let mode = system::screen_mode();
    let gpu = probe_isolated().unwrap_or_else(|e| {
        log::error!("sonde de la carte graphique : {e} ; décodage par le processeur");
        Gpu::default()
    });
    let pc = ThisPc {
        name: std::env::var("COMPUTERNAME").unwrap_or_default(),
        gpu: gpu.gpu,
        h264: gpu.h264,
        hevc: gpu.hevc,
        hevc444: gpu.hevc444,
        av1: gpu.av1,
        width: mode.width,
        height: mode.height,
        refresh: mode.refresh,
        audio_channels: audio::default_output_channels(),
        os: system::os_version(),
        on_battery: on_battery(),
    };
    log::info!("ce PC : {pc:?}");
    pc
}
