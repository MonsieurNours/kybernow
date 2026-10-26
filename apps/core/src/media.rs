//! Paquets d'image et de son livrés à l'app par Kyber (correctif 0001 des
//! apps, « packet sink »), au lieu de VLC.
//!
//! Kyber appelle le rappel sur son fil réseau, un paquet à la fois et dans
//! l'ordre pour un même flux : le gestionnaire ne doit jamais bloquer (il
//! recopie le paquet et le passe à un autre fil).

use std::ffi::c_void;
use std::sync::Arc;

use kyclient::media_sink::{MediaPacket, MediaPacketKind, PacketSink};

/// Codecs annoncés par Kyber (FourCC).
pub const FOURCC_H264: u32 = 0x6832_3634;
pub const FOURCC_HEVC: u32 = 0x6832_3635;
pub const FOURCC_AV1: u32 = 0x0061_7631;
pub const FOURCC_OPUS: u32 = 0x6F70_7573;

#[derive(Debug)]
pub enum Packet<'a> {
    /// Début d'un flux : format de ce qui suit.
    Codec { codec: u32 },
    /// Données : jeux de paramètres (`is_config`) ou image / trame Opus.
    Media {
        is_config: bool,
        is_key: bool,
        pts: u64,
        data: &'a [u8],
    },
    /// Perte : pour le son, nombre d'échantillons à masquer.
    Hole { missing_samples: u32 },
}

pub trait PacketHandler: Send + Sync {
    fn packet(&self, packet: Packet<'_>);

    /// Le même paquet avec l'identifiant de sa source chez Kyber
    /// (`MediaPacket::source_id`) : c'est ce que Kyber appelle. Par défaut,
    /// la source est laissée de côté ([`Self::packet`]) ; un gestionnaire qui
    /// reçoit plusieurs flux peut s'en servir pour les distinguer.
    fn packet_from(&self, source_id: u32, packet: Packet<'_>) {
        let _ = source_id;
        self.packet(packet);
    }
}

extern "C" fn trampoline(packet: *const MediaPacket, userdata: *mut c_void) {
    if packet.is_null() || userdata.is_null() {
        return;
    }
    let p = unsafe { &*packet };
    let handler = userdata as *mut Arc<dyn PacketHandler>;
    let source = p.source_id;
    match p.kind {
        // Dernier appel pour ce flux : on libère la référence.
        MediaPacketKind::Closed => unsafe { drop(Box::from_raw(handler)) },
        MediaPacketKind::Codec => unsafe { (*handler).packet_from(source, Packet::Codec { codec: p.codec }) },
        MediaPacketKind::Hole => unsafe {
            (*handler).packet_from(source, Packet::Hole {
                missing_samples: p.missing_samples,
            })
        },
        MediaPacketKind::Media => {
            let data = if p.data.is_null() || p.len == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(p.data, p.len) }
            };
            unsafe {
                (*handler).packet_from(source, Packet::Media {
                    is_config: p.is_config,
                    is_key: p.is_key,
                    pts: p.pts,
                    data,
                })
            }
        }
    }
}

/// Un rappel pour Kyber qui garde une référence au gestionnaire jusqu'au
/// paquet `Closed`.
pub fn packet_sink(handler: Arc<dyn PacketHandler>) -> PacketSink {
    let boxed: Box<Arc<dyn PacketHandler>> = Box::new(handler);
    PacketSink {
        cb: trampoline,
        userdata: Box::into_raw(boxed) as usize,
    }
}

/// En-tête Opus (`OpusHead`) envoyé en paquet de configuration du son.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusHead {
    pub channels: u8,
    pub streams: u8,
    pub coupled: u8,
    pub mapping: Vec<u8>,
}

impl OpusHead {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 19 || &data[..8] != b"OpusHead" {
            return None;
        }
        let channels = data[9];
        if channels == 0 {
            return None;
        }
        let family = data[18];
        if family == 0 {
            return Some(Self {
                channels,
                streams: 1,
                coupled: u8::from(channels > 1),
                mapping: if channels > 1 { vec![0, 1] } else { vec![0] },
            });
        }
        let end = 21 + channels as usize;
        if data.len() < end {
            return None;
        }
        Some(Self {
            channels,
            streams: data[19],
            coupled: data[20],
            mapping: data[21..end].to_vec(),
        })
    }

    /// Stéréo par défaut quand l'en-tête manque.
    pub fn stereo() -> Self {
        Self {
            channels: 2,
            streams: 1,
            coupled: 1,
            mapping: vec![0, 1],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_tete_opus() {
        let mut h = b"OpusHead".to_vec();
        h.extend_from_slice(&[1, 2, 0x38, 1, 0x80, 0xbb, 0, 0, 0, 0, 0]);
        assert_eq!(OpusHead::parse(&h), Some(OpusHead::stereo()));
        let mut h6 = b"OpusHead".to_vec();
        h6.extend_from_slice(&[1, 6, 0x38, 1, 0x80, 0xbb, 0, 0, 0, 0, 1, 4, 2, 0, 4, 1, 2, 3, 5]);
        let p = OpusHead::parse(&h6).unwrap();
        assert_eq!((p.channels, p.streams, p.coupled), (6, 4, 2));
        assert_eq!(p.mapping, vec![0, 4, 1, 2, 3, 5]);
    }
}
