//! Paquets d'image et de son livrés à l'app par Kyber (correctif 0001 des
//! apps, « packet sink »), au lieu de VLC.
//!
//! Kyber appelle le rappel sur son fil réseau, un paquet à la fois et dans
//! l'ordre pour un même flux : le gestionnaire ne doit jamais bloquer (il
//! recopie le paquet et le passe à un autre fil).

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use kyclient::media_sink::{MediaPacket, MediaPacketKind, PacketSink};

/// Codecs annoncés par Kyber (FourCC).
pub const FOURCC_H264: u32 = 0x6832_3634;
pub const FOURCC_HEVC: u32 = 0x6832_3635;
pub const FOURCC_AV1: u32 = 0x0061_7631;
/// PyroWave (docs/pyrowave.md), « pwv1 ».
pub const FOURCC_PYROWAVE: u32 = 0x7077_7631;
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
        /// Image qui répare une perte sans être une image clé
        /// (docs/reprise-apres-perte.md) : elle ne dépend que d'images
        /// jusqu'à ce pts. Un décodeur qui a décodé jusque-là, sans rien
        /// perdre avant, y reprend sans être refait ; sinon, il attend.
        resume: Option<u64>,
    },
    /// Perte : pour le son, nombre d'échantillons à masquer. Pour l'image
    /// (`missing_samples` à 0) : une image au moins manque avant la suivante
    /// (numéros des images, docs/reprise-apres-perte.md) ; le décodeur
    /// attend une image clé ou une image de reprise.
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

    /// Le moteur de son derrière ce gestionnaire de paquets de son, s'il y en
    /// a un : la session y coupe et y rend le son (plusieurs ordinateurs à la
    /// fois, [`crate::session::Session::set_muted`]). Le moteur lui-même se
    /// rend ; un gestionnaire qui l'enveloppe rend le sien (ou la plateforme
    /// le donne par [`crate::session::Session::attach_audio`]).
    fn audio_engine(self: Arc<Self>) -> Option<Arc<crate::audio::AudioEngine>> {
        None
    }
}

/// Ce que le rappel garde d'un flux : son gestionnaire, son format et le
/// numéro de la dernière image (marques de la Station), et le suivi des
/// pertes que la session lit.
struct Sink {
    handler: Arc<dyn PacketHandler>,
    state: Mutex<SinkState>,
    loss: Arc<StreamLoss>,
}

#[derive(Default)]
struct SinkState {
    codec: u32,
    number: Option<u32>,
}

extern "C" fn trampoline(packet: *const MediaPacket, userdata: *mut c_void) {
    // Appelé par Kyber (C) : une panique de l'app ou du cœur n'en sort pas.
    crate::ffi::guard("rappel des paquets de Kyber", (), || unsafe { trampoline_inner(packet, userdata) })
}

unsafe fn trampoline_inner(packet: *const MediaPacket, userdata: *mut c_void) {
    if packet.is_null() || userdata.is_null() {
        return;
    }
    let p = unsafe { &*packet };
    let sink = userdata as *mut Sink;
    let source = p.source_id;
    match p.kind {
        // Dernier appel pour ce flux : on libère la référence.
        MediaPacketKind::Closed => unsafe { drop(Box::from_raw(sink)) },
        MediaPacketKind::Codec => unsafe {
            *(*sink).state.lock().unwrap() = SinkState { codec: p.codec, number: None };
            (*sink).loss.reset();
            (*sink).handler.packet_from(source, Packet::Codec { codec: p.codec })
        },
        MediaPacketKind::Hole => unsafe {
            (*sink).handler.packet_from(source, Packet::Hole {
                missing_samples: p.missing_samples,
            })
        },
        MediaPacketKind::Media => {
            let data = if p.data.is_null() || p.len == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(p.data, p.len) }
            };
            let s = unsafe { &*sink };
            let (gap, resume) = if p.is_config { (false, None) } else { s.mark(p.is_key, data) };
            if gap {
                // Un numéro sauté : une image au moins s'est perdue en route
                // (Kyber ne le dit pas lui-même pour l'image).
                s.loss.hole();
                s.handler.packet_from(source, Packet::Hole { missing_samples: 0 });
            }
            if !p.is_config {
                s.loss.image(p.pts, p.is_key, resume);
            }
            s.handler.packet_from(source, Packet::Media {
                is_config: p.is_config,
                is_key: p.is_key,
                pts: p.pts,
                data,
                resume,
            })
        }
    }
}

impl Sink {
    /// La marque d'une image (HEVC, H.264) : une image sautée depuis la
    /// précédente, et la borne d'une image de reprise. Une image clé repart
    /// de son numéro, sans rien vérifier.
    fn mark(&self, key: bool, data: &[u8]) -> (bool, Option<u64>) {
        let mut st = self.state.lock().unwrap();
        let mark = match st.codec {
            FOURCC_HEVC => image_mark(data, true),
            FOURCC_H264 => image_mark(data, false),
            _ => None,
        };
        let Some(mark) = mark else { return (false, None) };
        let gap = !key && st.number.is_some_and(|n| mark.number != n.wrapping_add(1));
        st.number = Some(mark.number);
        (gap, if key { None } else { mark.bound })
    }
}

/// Un rappel pour Kyber qui garde une référence au gestionnaire jusqu'au
/// paquet `Closed`.
pub fn packet_sink(handler: Arc<dyn PacketHandler>) -> PacketSink {
    sink(handler, None)
}

/// Le même, pour un flux d'image dont la session suit les pertes
/// ([`StreamLoss`]) : c'est de là qu'elle demande une reprise sans image clé.
pub fn packet_sink_tracked(handler: Arc<dyn PacketHandler>, loss: Arc<StreamLoss>) -> PacketSink {
    sink(handler, Some(loss))
}

fn sink(handler: Arc<dyn PacketHandler>, loss: Option<Arc<StreamLoss>>) -> PacketSink {
    let boxed = Box::new(Sink {
        handler,
        state: Mutex::new(SinkState::default()),
        loss: loss.unwrap_or_default(),
    });
    PacketSink {
        cb: trampoline,
        userdata: Box::into_raw(boxed) as usize,
    }
}

/// Marque des images (correctif 0060 de la Station, txproto
/// `kn_recovery.h`) : un SEI « user data unregistered » de cet UUID devant
/// la première tranche de chaque image HEVC ou H.264, avec son numéro (4
/// octets, gros-boutiste, un de plus à chaque image) et, sur l'image qui
/// répare une perte, le pts de l'image la plus récente dont elle dépend (8
/// octets). Une Station sans le correctif n'en met pas.
pub const RESUME_SEI_UUID: &[u8; 16] = b"KyberNow:reprise";

/// Marque d'une image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageMark {
    pub number: u32,
    /// Image de reprise : elle ne dépend que d'images jusqu'à ce pts.
    pub bound: Option<u64>,
}

/// La marque d'une image (Annex B, HEVC ou H.264), s'il y en a une : un
/// SEI placé avant la première tranche.
pub fn image_mark(data: &[u8], hevc: bool) -> Option<ImageMark> {
    let mut i = 0;
    while i + 3 < data.len() {
        if data[i] != 0 || data[i + 1] != 0 || data[i + 2] != 1 {
            i += 1;
            continue;
        }
        let start = i + 3;
        let (slice, sei, header) = if hevc {
            let kind = (data[start] >> 1) & 0x3f;
            (kind < 32, kind == 39, 2)
        } else {
            let kind = data[start] & 0x1f;
            ((1..=5).contains(&kind), kind == 6, 1)
        };
        if slice {
            // Première tranche : la marque vient toujours avant.
            return None;
        }
        // Fin du NAL : début du suivant (ou fin des données).
        let mut end = start + header;
        while end + 2 < data.len() && !(data[end] == 0 && data[end + 1] == 0 && data[end + 2] <= 1) {
            end += 1;
        }
        let end = if end + 2 < data.len() { end } else { data.len() };
        if sei
            && end > start + header
            && let Some(mark) = sei_mark(&unescape(&data[start + header..end]))
        {
            return Some(mark);
        }
        i = end;
    }
    None
}

/// Retire les octets d'émulation (00 00 03).
fn unescape(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &b in nal {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        out.push(b);
        zeros = if b == 0 { zeros + 1 } else { 0 };
    }
    out
}

/// Les messages d'un SEI : la marque, s'il y est.
fn sei_mark(rbsp: &[u8]) -> Option<ImageMark> {
    let mut i = 0;
    while i < rbsp.len() && rbsp[i] != 0x80 {
        let mut kind = 0usize;
        while *rbsp.get(i)? == 0xff {
            kind += 255;
            i += 1;
        }
        kind += *rbsp.get(i)? as usize;
        i += 1;
        let mut size = 0usize;
        while *rbsp.get(i)? == 0xff {
            size += 255;
            i += 1;
        }
        size += *rbsp.get(i)? as usize;
        i += 1;
        let payload = rbsp.get(i..i + size)?;
        if kind == 5 && size >= 20 && &payload[..16] == RESUME_SEI_UUID {
            let number = u32::from_be_bytes(payload[16..20].try_into().ok()?);
            let bound = (size >= 28).then(|| u64::from_be_bytes(payload[20..28].try_into().unwrap()));
            return Some(ImageMark { number, bound });
        }
        i += size;
    }
    None
}

/// Reprise d'un décodeur après une perte (docs/reprise-apres-perte.md), à
/// tenir par le fil de décodage de chaque app. Après un trou, le décodeur
/// n'a plus que les images confiées avant lui : il écarte la suite, puis
/// reprend sur une image clé, ou sur une image de reprise (marque `resume`)
/// qui ne dépend que d'images qu'il a déjà, sans être refait.
#[derive(Debug, Default, Clone, Copy)]
pub struct ResumeGate {
    last_fed: Option<u64>,
}

impl ResumeGate {
    /// Image confiée au décodeur, rien ne manquant avant elle.
    pub fn fed(&mut self, pts: u64) {
        self.last_fed = Some(pts);
    }

    /// Décodeur refait, fermé ou en erreur : il n'a plus rien, seule une
    /// image clé le relance.
    pub fn reset(&mut self) {
        self.last_fed = None;
    }

    /// Le décodeur a des images d'avant la perte : une reprise sans image
    /// clé est possible (la session l'a demandée, rien à redemander ici).
    pub fn intact(&self) -> bool {
        self.last_fed.is_some()
    }

    /// En attente après une perte, un décodeur toujours ouvert reprend-il
    /// sur cette image ? Oui pour une image de reprise qui ne dépend que
    /// d'images qu'il a.
    pub fn resumes(&self, resume: Option<u64>) -> bool {
        matches!((resume, self.last_fed), (Some(r), Some(l)) if r <= l)
    }
}

/// Pertes d'un flux d'image, vues des paquets que Kyber livre à l'app
/// (fil réseau) et lues par la session : le pts de la dernière image reçue
/// avant la perte en cours, celle à partir de laquelle l'ordinateur répare.
#[derive(Debug, Default)]
pub struct StreamLoss {
    state: Mutex<LossState>,
}

#[derive(Debug, Default)]
struct LossState {
    /// Dernière image livrée (hors jeux de paramètres).
    last: Option<u64>,
    /// Perte en cours : la dernière image livrée avant elle.
    lost_after: Option<Option<u64>>,
}

impl StreamLoss {
    /// La perte en cours, s'il y en a une, et l'image d'avant (`None` : rien
    /// de reçu avant, seule une image clé répare).
    pub fn loss_point(&self) -> Option<Option<u64>> {
        self.state.lock().unwrap().lost_after
    }

    fn reset(&self) {
        *self.state.lock().unwrap() = LossState::default();
    }

    fn hole(&self) {
        let mut s = self.state.lock().unwrap();
        if s.lost_after.is_none() {
            s.lost_after = Some(s.last);
        }
    }

    fn image(&self, pts: u64, key: bool, resume: Option<u64>) {
        let mut s = self.state.lock().unwrap();
        s.last = Some(pts);
        let repaired = match s.lost_after {
            None => false,
            Some(_) if key => true,
            Some(before) => resume.is_some_and(|r| before.is_some_and(|b| r <= b)),
        };
        if repaired {
            s.lost_after = None;
        }
    }
}

/// En-tête `OpusHead` de famille 0 (mono ou stéréo), pour les essais.
#[cfg(test)]
pub(crate) fn tests_head(channels: u8) -> Vec<u8> {
    let mut h = b"OpusHead".to_vec();
    h.extend_from_slice(&[1, channels, 0x38, 1, 0x80, 0xbb, 0, 0, 0, 0, 0]);
    h
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
        // Au plus 7.1 ; libopus lit `channels` octets de correspondance : un
        // en-tête de famille 0 à plus de 2 voies lui ferait lire au-delà de
        // `mapping` (COEUR-03).
        if channels == 0 || channels > 8 {
            return None;
        }
        let family = data[18];
        if family == 0 && channels > 2 {
            return None;
        }
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
        let (streams, coupled) = (data[19], data[20]);
        if streams == 0 || coupled > streams || usize::from(streams) + usize::from(coupled) > 255 {
            return None;
        }
        Some(Self {
            channels,
            streams,
            coupled,
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

    /// Une image marquée comme le fait la Station (kn_recovery.h) : SEI
    /// avant la tranche, octets d'émulation compris.
    fn marked(hevc: bool, number: u32, bound: Option<u64>) -> Vec<u8> {
        let mut rbsp = vec![5u8, if bound.is_some() { 28 } else { 20 }];
        rbsp.extend_from_slice(RESUME_SEI_UUID);
        rbsp.extend_from_slice(&number.to_be_bytes());
        if let Some(b) = bound {
            rbsp.extend_from_slice(&b.to_be_bytes());
        }
        rbsp.push(0x80);
        let mut nal = if hevc { vec![0, 0, 0, 1, 39 << 1, 1] } else { vec![0, 0, 0, 1, 6] };
        let mut zeros = 0;
        for b in rbsp {
            if zeros >= 2 && b <= 3 {
                nal.push(3);
                zeros = 0;
            }
            nal.push(b);
            zeros = if b == 0 { zeros + 1 } else { 0 };
        }
        if hevc {
            nal.extend_from_slice(&[0, 0, 0, 1, 1 << 1, 1, 0xaf, 0x00, 0x11]);
        } else {
            nal.extend_from_slice(&[0, 0, 0, 1, 0x41, 0x9a, 0x00, 0x11]);
        }
        nal
    }

    #[test]
    fn marque_des_images() {
        let m = |number, bound| Some(ImageMark { number, bound });
        assert_eq!(image_mark(&marked(true, 7, Some(16_683_333)), true), m(7, Some(16_683_333)));
        assert_eq!(image_mark(&marked(true, 8, None), true), m(8, None));
        assert_eq!(image_mark(&marked(false, 9, None), false), m(9, None));
        assert_eq!(image_mark(&marked(false, 0, Some(0)), false), m(0, Some(0)));
        // Zéros partout : octets d'émulation dans la marque.
        assert_eq!(image_mark(&marked(true, 0, Some(0x0000_0100_0000_0002)), true), m(0, Some(0x0000_0100_0000_0002)));
        // Paramètres devant ; sans marque ; marque après la tranche.
        let mut with_ps = vec![0, 0, 0, 1, 32 << 1, 1, 0x0c, 0, 0, 1, 33 << 1, 1, 0x01];
        with_ps.extend_from_slice(&marked(true, 3, Some(42)));
        assert_eq!(image_mark(&with_ps, true), m(3, Some(42)));
        assert_eq!(image_mark(&[0, 0, 0, 1, 1 << 1, 1, 0xaf], true), None);
        let mut after = vec![0, 0, 0, 1, 1 << 1, 1, 0xaf];
        after.extend_from_slice(&marked(true, 1, None));
        assert_eq!(image_mark(&after, true), None);
        // Autre UUID : pas une marque.
        let mut other = marked(true, 9, None);
        other[8] ^= 0xff;
        assert_eq!(image_mark(&other, true), None);
    }

    /// Les paquets passent par le rappel comme ceux de Kyber : un numéro
    /// sauté devient un trou, l'image de reprise porte sa borne.
    #[test]
    fn trou_d_image() {
        #[derive(Default)]
        struct Seen(Mutex<Vec<String>>);
        impl PacketHandler for Seen {
            fn packet(&self, p: Packet<'_>) {
                let what = match p {
                    Packet::Codec { .. } => "codec".to_string(),
                    Packet::Hole { .. } => "trou".to_string(),
                    Packet::Media { pts, resume, .. } => format!("{pts}{}", resume.map(|r| format!("<{r}")).unwrap_or_default()),
                };
                self.0.lock().unwrap().push(what);
            }
        }
        let seen = Arc::new(Seen::default());
        let loss = Arc::new(StreamLoss::default());
        let sink = packet_sink_tracked(seen.clone(), loss.clone());
        let send = |kind, key: bool, pts: u64, data: &[u8]| {
            let p = MediaPacket {
                kind,
                source_id: 0,
                codec: FOURCC_HEVC,
                is_config: false,
                is_key: key,
                pts,
                missing_samples: 0,
                data: data.as_ptr(),
                len: data.len(),
            };
            (sink.cb)(&p, sink.userdata as *mut c_void);
        };
        send(MediaPacketKind::Codec, false, 0, &[]);
        send(MediaPacketKind::Media, true, 10, &marked(true, 0, None));
        send(MediaPacketKind::Media, false, 20, &marked(true, 1, None));
        send(MediaPacketKind::Media, false, 40, &marked(true, 3, None));
        assert_eq!(loss.loss_point(), Some(Some(20)));
        send(MediaPacketKind::Media, false, 50, &marked(true, 4, Some(20)));
        assert_eq!(loss.loss_point(), None);
        // Le dernier appel (`Closed`, qui libère le gestionnaire) vient du
        // `Drop` du `PacketSink` depuis le correctif KYB-25 de kyclient : le
        // faire aussi à la main libérerait deux fois.
        drop(sink);
        assert_eq!(Arc::strong_count(&seen), 1, "gestionnaire rendu par le dernier appel");
        assert_eq!(*seen.0.lock().unwrap(), ["codec", "10", "20", "trou", "40", "50<20"]);
    }

    #[test]
    fn suivi_des_pertes() {
        let l = StreamLoss::default();
        assert_eq!(l.loss_point(), None);
        l.image(10, true, None);
        l.image(20, false, None);
        l.hole();
        l.image(40, false, None);
        l.hole();
        assert_eq!(l.loss_point(), Some(Some(20)));
        // Réparation pour un autre client (plus récente) : la perte reste.
        l.image(50, false, Some(40));
        assert_eq!(l.loss_point(), Some(Some(20)));
        l.image(60, false, Some(20));
        assert_eq!(l.loss_point(), None);
        // Perte avant toute image : seule une image clé répare.
        l.reset();
        l.hole();
        assert_eq!(l.loss_point(), Some(None));
        l.image(5, false, Some(0));
        assert_eq!(l.loss_point(), Some(None));
        l.image(6, true, None);
        assert_eq!(l.loss_point(), None);
    }

    #[test]
    fn reprise_du_decodeur() {
        let mut g = ResumeGate::default();
        assert!(!g.resumes(Some(0)));
        g.fed(10);
        g.fed(20);
        assert!(g.resumes(Some(20)));
        assert!(g.resumes(Some(15)));
        assert!(!g.resumes(Some(30)));
        assert!(!g.resumes(None));
        g.reset();
        assert!(!g.resumes(Some(10)));
    }

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

    /// COEUR-03 : en-têtes incohérents refusés.
    #[test]
    fn en_tete_opus_incoherent() {
        let head = |channels: u8, family: u8, rest: &[u8]| {
            let mut h = b"OpusHead".to_vec();
            h.extend_from_slice(&[1, channels, 0x38, 1, 0x80, 0xbb, 0, 0, 0, 0, family]);
            h.extend_from_slice(rest);
            h
        };
        // Famille 0 à 6 voies : la correspondance n'aurait que 2 octets.
        assert_eq!(OpusHead::parse(&head(6, 0, &[])), None);
        // Plus de 8 voies.
        assert_eq!(OpusHead::parse(&head(255, 255, &[200, 55])), None);
        // Flux couplés plus nombreux que les flux, ou aucun flux.
        assert_eq!(OpusHead::parse(&head(2, 1, &[1, 2, 0, 1])), None);
        assert_eq!(OpusHead::parse(&head(2, 1, &[0, 0, 0, 1])), None);
        assert!(OpusHead::parse(&head(1, 0, &[])).is_some());
    }
}
