//! État des GPU NVIDIA via NVML (chargé dynamiquement depuis le pilote).

use std::sync::OnceLock;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{PerformanceState, TemperatureSensor};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct GpuStatus {
    pub driver: Option<String>,
    pub devices: Vec<GpuDevice>,
}

#[derive(Debug, Serialize)]
pub struct GpuDevice {
    pub name: String,
    pub gpu_utilization: Option<u32>,
    pub encoder_utilization: Option<u32>,
    pub decoder_utilization: Option<u32>,
    /// Sessions NVENC ouvertes (une par flux en cours).
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

fn nvml() -> Option<&'static Nvml> {
    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();
    NVML.get_or_init(|| match Nvml::init() {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::info!("NVML indisponible ({e}) : pas de supervision GPU");
            None
        }
    })
    .as_ref()
}

/// Nombre de flux encodés par NVENC en ce moment (tous GPU confondus).
pub fn encoder_sessions() -> Option<u32> {
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

/// Appel bloquant : à lancer dans `spawn_blocking`.
pub fn status() -> Option<GpuStatus> {
    let nvml = nvml()?;
    let count = nvml.device_count().ok()?;
    let devices = (0..count)
        .filter_map(|i| nvml.device_by_index(i).ok())
        .map(|d| {
            let enc_stats = d.encoder_stats().ok();
            let memory = d.memory_info().ok();
            GpuDevice {
                name: d.name().unwrap_or_else(|_| "GPU NVIDIA".into()),
                gpu_utilization: d.utilization_rates().ok().map(|u| u.gpu),
                encoder_utilization: d.encoder_utilization().ok().map(|u| u.utilization),
                decoder_utilization: d.decoder_utilization().ok().map(|u| u.utilization),
                encoder_sessions: enc_stats.as_ref().map(|s| s.session_count),
                encoder_fps: enc_stats.as_ref().map(|s| s.average_fps),
                encoder_latency_us: enc_stats.as_ref().map(|s| s.average_latency),
                temperature_c: d.temperature(TemperatureSensor::Gpu).ok(),
                memory_used_mb: memory.as_ref().map(|m| m.used / 1_048_576),
                memory_total_mb: memory.as_ref().map(|m| m.total / 1_048_576),
                power_w: d.power_usage().ok().map(|mw| mw as f32 / 1000.0),
                pstate: d.performance_state().ok().and_then(pstate_number),
            }
        })
        .collect();
    Some(GpuStatus { driver: nvml.sys_driver_version().ok(), devices })
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
