#![cfg_attr(not(windows), allow(dead_code))]

//! Échange entre la capture WGC des écrans secondaires (`display-agent
//! wgc-capture`, lancé avec le compte de l'utilisateur) et la capture de
//! Kyber (txproto dans kyavserver, en SYSTEM), dans la session de l'écran.
//!
//! Admin publie pour la session un repère, [`SESSION_NAME`] (mémoire
//! partagée dans l'espace global, accès SYSTEM seul, en-tête
//! [`SessionHeader`]) : le jeton de la session et les écrans capturés par
//! WGC. txproto ne cherche un anneau que pour ces écrans.
//!
//! Pour l'écran `\\.\DISPLAY6` (identifiant `DISPLAY6`) et le jeton `T`,
//! l'anneau a pour identifiant `T_DISPLAY6` ([`ring_id`]) :
//! - `KyberNowWGC_T_DISPLAY6` : mémoire partagée, un en-tête [`Header`] ;
//! - `KyberNowWGC_T_DISPLAY6_frame` : événement signalé à chaque image ;
//! - `KyberNowWGC_T_DISPLAY6_<génération>_<n>` : les [`SLOTS`] textures D3D11
//!   partagées (BGRA 8 bits, verrou `IDXGIKeyedMutex`, clé 0), sur la carte
//!   de l'écran. La génération change quand la taille change.
//!
//! Ces objets sont dans l'espace de noms de la session de l'écran, ouverts
//! à SYSTEM et au seul compte de l'utilisateur.
//!
//! La même disposition est écrite en C dans le correctif Kyber 0018
//! (`txproto/src/iosys_dxgi.c`) : toute modification va des deux côtés, et
//! `VERSION` change.

/// « KNWG ».
pub const MAGIC: u32 = 0x4757_4E4B;
pub const VERSION: u32 = 2;
/// Textures de l'anneau : la capture en écrit une pendant que Kyber lit la dernière.
pub const SLOTS: u32 = 3;
/// `DXGI_FORMAT_B8G8R8A8_UNORM`.
pub const FORMAT_BGRA8: u32 = 87;

pub const STATE_STARTING: u32 = 0;
pub const STATE_RUNNING: u32 = 1;
pub const STATE_STOPPED: u32 = 2;

/// Repère de session publié par Admin (« KNWS »).
pub const SESSION_NAME: &str = r"Global\KyberNowWGC_session";
pub const SESSION_MAGIC: u32 = 0x5357_4E4B;
pub const SESSION_VERSION: u32 = 1;
/// Écrans au plus dans le repère (les secondaires d'une session à 4 écrans).
pub const SESSION_SCREENS: usize = 4;

/// Repère de session : jeton et identifiants des écrans, en ASCII terminé
/// par un zéro. Écrit une fois par Admin avant de lancer les agents.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SessionHeader {
    pub magic: u32,
    pub version: u32,
    pub count: u32,
    pub reserved: u32,
    pub token: [u8; 24],
    pub ids: [[u8; 32]; SESSION_SCREENS],
}

pub const SESSION_SIZE: usize = size_of::<SessionHeader>();

impl SessionHeader {
    pub fn new(token: &str, ids: &[String]) -> Self {
        fn fill<const N: usize>(text: &str) -> [u8; N] {
            let mut out = [0u8; N];
            for (o, b) in out.iter_mut().zip(text.bytes().take(N - 1)) {
                *o = b;
            }
            out
        }
        let mut header = Self {
            magic: SESSION_MAGIC,
            version: SESSION_VERSION,
            count: ids.len().min(SESSION_SCREENS) as u32,
            reserved: 0,
            token: fill(token),
            ids: [[0u8; 32]; SESSION_SCREENS],
        };
        for (slot, id) in header.ids.iter_mut().zip(ids) {
            *slot = fill(id);
        }
        header
    }
}

/// Identifiant de l'anneau d'un écran pour une session.
pub fn ring_id(token: &str, screen_id: &str) -> String {
    format!("{token}_{screen_id}")
}

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
    fn repere_de_session_identique_au_correctif_c() {
        // Mêmes positions que `KnWgcSession` du correctif 0018.
        assert_eq!(offset_of!(SessionHeader, token), 16);
        assert_eq!(offset_of!(SessionHeader, ids), 40);
        assert_eq!(SESSION_SIZE, 168);
        assert_eq!(&SESSION_MAGIC.to_le_bytes(), b"KNWS");
        let h = SessionHeader::new("0123456789abcdef", &["DISPLAY6".into(), "DISPLAY7".into()]);
        assert_eq!(h.count, 2);
        assert_eq!(&h.token[..17], b"0123456789abcdef\0");
        assert_eq!(&h.ids[1][..9], b"DISPLAY7\0");
        assert_eq!(h.ids[2], [0u8; 32]);
    }

    #[test]
    fn noms_des_objets_partages() {
        assert_eq!(screen_id(r"\\.\DISPLAY6"), "DISPLAY6");
        assert_eq!(header_name("DISPLAY6"), "KyberNowWGC_DISPLAY6");
        assert_eq!(event_name("DISPLAY6"), "KyberNowWGC_DISPLAY6_frame");
        assert_eq!(texture_name("DISPLAY6", 2, 1), "KyberNowWGC_DISPLAY6_2_1");
        assert_eq!(header_name(&ring_id("0123456789abcdef", "DISPLAY6")), "KyberNowWGC_0123456789abcdef_DISPLAY6");
    }
}
