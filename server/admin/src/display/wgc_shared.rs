#![cfg_attr(not(windows), allow(dead_code))]

//! Échange entre la capture WGC des écrans secondaires (`display-agent
//! wgc-capture`, lancé avec le compte de l'utilisateur) et la capture de
//! Kyber (txproto dans kyavserver, en SYSTEM), dans la session de l'écran.
//!
//! Pour l'écran `\\.\DISPLAY6` (identifiant `DISPLAY6`) :
//! - `KyberNowWGC_DISPLAY6` : mémoire partagée, un en-tête [`Header`] ;
//! - `KyberNowWGC_DISPLAY6_frame` : événement signalé à chaque image ;
//! - `KyberNowWGC_DISPLAY6_<génération>_<n>` : les [`SLOTS`] textures D3D11
//!   partagées (BGRA 8 bits, verrou `IDXGIKeyedMutex`, clé 0), sur la carte
//!   de l'écran. La génération change quand la taille change.
//!
//! La même disposition est écrite en C dans le correctif Kyber 0018
//! (`txproto/src/iosys_dxgi.c`) : toute modification va des deux côtés, et
//! `VERSION` change.

/// « KNWG ».
pub const MAGIC: u32 = 0x4757_4E4B;
pub const VERSION: u32 = 1;
/// Textures de l'anneau : la capture en écrit une pendant que Kyber lit la dernière.
pub const SLOTS: u32 = 3;
/// `DXGI_FORMAT_B8G8R8A8_UNORM`.
pub const FORMAT_BGRA8: u32 = 87;

pub const STATE_STARTING: u32 = 0;
pub const STATE_RUNNING: u32 = 1;
pub const STATE_STOPPED: u32 = 2;

/// En-tête de la mémoire partagée. Les temps sont en unités de 100 ns sur
/// l'horloge de `QueryPerformanceCounter` (celle de `SystemRelativeTime`).
/// `frame_counter` est écrit en dernier (ordre « release ») : quand il
/// change, `latest_slot`, `generation` et les temps sont à jour.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct Header {
    pub magic: u32,
    pub version: u32,
    pub state: u32,
    pub generation: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub slots: u32,
    pub luid_low: u32,
    pub luid_high: i32,
    pub refresh: u32,
    pub latest_slot: u32,
    pub frame_counter: u64,
    /// Image : `SystemRelativeTime` de WGC.
    pub frame_time: i64,
    /// Image copiée dans l'anneau.
    pub publish_time: i64,
    /// Signal de vie de la capture (deux fois par seconde).
    pub heartbeat_time: i64,
    pub producer_pid: u32,
    /// 1 si Windows impose la bordure jaune, 0 sinon.
    pub border_required: u32,
}

pub const HEADER_SIZE: usize = size_of::<Header>();

/// `\\.\DISPLAY6` → `DISPLAY6`.
pub fn screen_id(gdi_name: &str) -> String {
    gdi_name.trim_start_matches(['\\', '.']).chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

pub fn header_name(id: &str) -> String {
    format!("KyberNowWGC_{id}")
}

pub fn event_name(id: &str) -> String {
    format!("KyberNowWGC_{id}_frame")
}

pub fn texture_name(id: &str, generation: u32, slot: u32) -> String {
    format!("KyberNowWGC_{id}_{generation}_{slot}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn disposition_identique_au_correctif_c() {
        // Mêmes positions que `KnWgcHeader` du correctif 0018.
        assert_eq!(offset_of!(Header, latest_slot), 44);
        assert_eq!(offset_of!(Header, frame_counter), 48);
        assert_eq!(offset_of!(Header, frame_time), 56);
        assert_eq!(offset_of!(Header, publish_time), 64);
        assert_eq!(offset_of!(Header, heartbeat_time), 72);
        assert_eq!(offset_of!(Header, producer_pid), 80);
        assert_eq!(HEADER_SIZE, 88);
        assert_eq!(&MAGIC.to_le_bytes(), b"KNWG");
    }

    #[test]
    fn noms_des_objets_partages() {
        assert_eq!(screen_id(r"\\.\DISPLAY6"), "DISPLAY6");
        assert_eq!(header_name("DISPLAY6"), "KyberNowWGC_DISPLAY6");
        assert_eq!(event_name("DISPLAY6"), "KyberNowWGC_DISPLAY6_frame");
        assert_eq!(texture_name("DISPLAY6", 2, 1), "KyberNowWGC_DISPLAY6_2_1");
    }
}
