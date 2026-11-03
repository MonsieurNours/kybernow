//! Takion, le transport de Sony sur UDP (port 9296), d'après `takion.c` de
//! chiaki-ng. Proche de SCTP : poignée de main INIT / INIT_ACK / COOKIE /
//! COOKIE_ACK, puis des messages numérotés (DATA, acquittés par DATA_ACK) et
//! des paquets d'image et de son hors messages.
//!
//! Chaque datagramme commence par son type (4 bits bas). Les messages (type
//! 0) ont un en-tête de 16 octets : étiquette de l'autre côté, GMAC (4),
//! position de clé (4), type de bloc, drapeaux, taille. Une fois les clés
//! connues ([`Takion::set_crypt`]), chaque paquet porte un GMAC calculé avec
//! sa position de clé à zéro (messages et congestion), et l'image et le son
//! sont chiffrés à partir de la position + 16.
//!
//! Pas encore : la retransmission des messages non acquittés (le réseau
//! local en perd peu) ni le réordonnancement des paquets d'image.

use std::collections::BTreeMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::gkcrypt::{BLOCK_SIZE, GMAC_SIZE, GkCrypt, KeyState};

/// Port du flux.
pub const PORT: u16 = 9296;

const A_RWND: u32 = 0x19000;
const STREAMS: u16 = 0x64;
const HEADER_SIZE: usize = 0x10;
const COOKIE_SIZE: usize = 0x20;
const EXPECT_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TRIES: usize = 3;
const POSTPONE_MAX: usize = 32;

/// Types de paquets (4 bits bas du premier octet).
pub mod packet {
    pub const CONTROL: u8 = 0;
    pub const FEEDBACK_HISTORY: u8 = 1;
    pub const VIDEO: u8 = 2;
    pub const AUDIO: u8 = 3;
    pub const CONGESTION: u8 = 5;
    pub const FEEDBACK_STATE: u8 = 6;
}

mod chunk {
    pub const DATA: u8 = 0;
    pub const INIT: u8 = 1;
    pub const INIT_ACK: u8 = 2;
    pub const DATA_ACK: u8 = 3;
    pub const COOKIE: u8 = 0xa;
    pub const COOKIE_ACK: u8 = 0xb;
}

/// Type de contenu d'un message DATA (`ChiakiTakionMessageDataType`).
pub mod data {
    pub const PROTOBUF: u8 = 0;
    pub const RUMBLE: u8 = 7;
    pub const PAD_INFO: u8 = 9;
    pub const TRIGGER_EFFECTS: u8 = 11;
}

fn mac_offset(kind: u8) -> Option<usize> {
    match kind {
        packet::CONTROL => Some(5),
        packet::VIDEO | packet::AUDIO => Some(0xa),
        packet::CONGESTION => Some(7),
        _ => None,
    }
}

fn key_pos_offset(kind: u8) -> Option<usize> {
    match kind {
        packet::CONTROL => Some(9),
        packet::VIDEO | packet::AUDIO => Some(0xe),
        packet::CONGESTION => Some(0xb),
        _ => None,
    }
}

/// GMAC d'un paquet (`chiaki_takion_packet_mac`) : calculé avec la place du
/// GMAC à zéro, et celle de la position de clé aussi pour les messages et la
/// congestion. Écrit le GMAC dans le paquet et rend l'ancien.
pub(crate) fn packet_mac(crypt: Option<&mut GkCrypt>, buf: &mut [u8], key_pos: u64) -> Option<[u8; GMAC_SIZE]> {
    let kind = *buf.first()? & 0xf;
    let mo = mac_offset(kind)?;
    let ko = key_pos_offset(kind)?;
    if buf.len() < mo + GMAC_SIZE || buf.len() < ko + 4 {
        return None;
    }
    let mut old = [0u8; GMAC_SIZE];
    old.copy_from_slice(&buf[mo..mo + GMAC_SIZE]);
    buf[mo..mo + GMAC_SIZE].fill(0);
    if let Some(crypt) = crypt {
        let zero_pos = kind == packet::CONTROL || kind == packet::CONGESTION;
        let saved: [u8; 4] = buf[ko..ko + 4].try_into().ok()?;
        if zero_pos {
            buf[ko..ko + 4].fill(0);
        }
        let mac = crypt.gmac(key_pos, buf);
        if zero_pos {
            buf[ko..ko + 4].copy_from_slice(&saved);
        }
        buf[mo..mo + GMAC_SIZE].copy_from_slice(&mac);
    }
    Some(old)
}

/// En-tête d'un message (`takion_write_message_header`).
fn message_header(out: &mut Vec<u8>, tag: u32, key_pos: u64, chunk_type: u8, flags: u8, payload_len: usize) {
    out.extend_from_slice(&tag.to_be_bytes());
    out.extend_from_slice(&[0; GMAC_SIZE]);
    out.extend_from_slice(&(key_pos as u32).to_be_bytes());
    out.push(chunk_type);
    out.push(flags);
    out.extend_from_slice(&((payload_len + 4) as u16).to_be_bytes());
}

/// Un paquet d'image ou de son, déchiffré (`ChiakiTakionAVPacket`).
#[derive(Debug, Clone)]
pub struct AvPacket {
    pub is_video: bool,
    pub is_haptics: bool,
    pub packet_index: u16,
    pub frame_index: u16,
    pub unit_index: u16,
    /// Unités de l'image, correction d'erreurs comprise.
    pub units_in_frame_total: u16,
    /// Image : unités de correction d'erreurs. Son : taille d'unité (8 bits
    /// hauts), unités de correction (4 bits) et de source (4 bits bas).
    pub units_in_frame_fec: u16,
    pub codec: u8,
    pub adaptive_stream_index: u8,
    pub key_pos: u64,
    pub data: Vec<u8>,
}

/// En-tête des paquets d'image et de son des versions 9 (PS4) et 12 (PS5)
/// (`av_packet_parse`) : rend le paquet et le début des données.
pub(crate) fn parse_av(v12: bool, buf: &[u8], key_state: &mut KeyState) -> Option<(AvPacket, usize)> {
    let kind = *buf.first()? & 0xf;
    if kind != packet::VIDEO && kind != packet::AUDIO {
        return None;
    }
    let is_video = kind == packet::VIDEO;
    let nalu_structs = (buf[0] >> 4) & 1 != 0;
    let av = &buf[1..];
    let header = match (v12, is_video) {
        (_, true) => 0x17,
        (true, false) => 0x13,
        (false, false) => 0x12,
    };
    if av.len() < header + 1 {
        return None;
    }
    let u16_at = |o: usize| u16::from_be_bytes([av[o], av[o + 1]]);
    let dword = u32::from_be_bytes([av[4], av[5], av[6], av[7]]);
    let (unit_index, total, fec) = if is_video {
        (((dword >> 0x15) & 0x7ff) as u16, (((dword >> 0xa) & 0x7ff) + 1) as u16, (dword & 0x3ff) as u16)
    } else {
        (((dword >> 0x18) & 0xff) as u16, (((dword >> 0x10) & 0xff) + 1) as u16, (dword & 0xffff) as u16)
    };
    let key_low = u32::from_be_bytes([av[0xd], av[0xe], av[0xf], av[0x10]]);
    let key_pos = key_state.request(key_low, true);
    let mut off = 1 + 0x11;
    let mut adaptive = 0;
    if is_video {
        adaptive = buf[off + 2] >> 5;
        off += 3;
    } else {
        off += 1;
    }
    if nalu_structs {
        off += 3;
    }
    let mut is_haptics = false;
    if v12 && !is_video {
        is_haptics = *buf.get(off)? == 0x02;
        off += 1;
    }
    if off > buf.len() {
        return None;
    }
    Some((
        AvPacket {
            is_video,
            is_haptics,
            packet_index: u16_at(0),
            frame_index: u16_at(2),
            unit_index,
            units_in_frame_total: total,
            units_in_frame_fec: fec,
            codec: av[8],
            adaptive_stream_index: adaptive,
            key_pos,
            data: Vec::new(),
        },
        off,
    ))
}

/// Ce que Takion annonce.
#[derive(Debug)]
pub enum TakionEvent {
    /// Message DATA, dans l'ordre : type de contenu et contenu.
    Data(u8, Vec<u8>),
    /// Paquet d'image ou de son, déchiffré.
    Av(AvPacket),
    /// Plus rien ne passe (arrêt, erreur réseau).
    Closed(String),
}

struct Local {
    seq_num: u32,
    key_pos: u64,
    crypt: Option<GkCrypt>,
}

struct Shared {
    socket: UdpSocket,
    tag_remote: u32,
    local: Mutex<Local>,
    remote: Mutex<Option<GkCrypt>>,
    stop: AtomicBool,
    /// Paquets d'image et de son reçus et perdus depuis la dernière lecture
    /// (congestion).
    received: AtomicU64,
    lost: AtomicU64,
}

/// Une connexion Takion ouverte.
pub struct Takion {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Takion {
    /// Ouvre la connexion : poignée de main, puis un fil `ps-takion` qui
    /// reçoit. `v12` : version 12 (PS5), sinon 9 (PS4).
    pub fn connect(addr: Ipv4Addr, port: u16, v12: bool) -> io::Result<(Self, Receiver<TakionEvent>)> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.connect(SocketAddrV4::new(addr, port))?;
        let mut tag_local = [0u8; 4];
        getrandom::getrandom(&mut tag_local).map_err(|e| io::Error::other(e.to_string()))?;
        let tag_local = u32::from_ne_bytes(tag_local).max(1);
        let tag_remote = handshake(&socket, tag_local)?;
        log::info!("PlayStation : Takion connecté");
        let shared = Arc::new(Shared {
            socket,
            tag_remote,
            local: Mutex::new(Local { seq_num: tag_local, key_pos: 0, crypt: None }),
            remote: Mutex::new(None),
            stop: AtomicBool::new(false),
            received: AtomicU64::new(0),
            lost: AtomicU64::new(0),
        });
        let (tx, rx) = mpsc::channel();
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("ps-takion".into())
                .spawn(move || receive_loop(shared, tag_local, tag_remote, v12, tx))?
        };
        Ok((Takion { shared, thread: Some(thread) }, rx))
    }

    /// Les clés du flux, une fois l'échange ECDH fait : `local` signe et
    /// chiffre ce que l'app envoie, `remote` vérifie et déchiffre le reste.
    pub fn set_crypt(&self, local: GkCrypt, remote: GkCrypt) {
        self.shared.local.lock().unwrap().crypt = Some(local);
        *self.shared.remote.lock().unwrap() = Some(remote);
    }

    /// Envoie un message DATA (`chiaki_takion_send_message_data`). `first` :
    /// premier morceau (un octet de type de contenu, protobuf) ; sinon suite
    /// d'un message découpé (`…_data_cont`). `flags` : 1 sur le dernier
    /// morceau.
    pub fn send_data(&self, flags: u8, channel: u16, payload: &[u8], first: bool) -> io::Result<u32> {
        let extra = if first { 9 } else { 8 };
        let mut local = self.shared.local.lock().unwrap();
        let key_pos = advance(&mut local, payload.len());
        let seq = local.seq_num;
        local.seq_num = local.seq_num.wrapping_add(1);
        let mut buf = Vec::with_capacity(1 + HEADER_SIZE + extra + payload.len());
        buf.push(packet::CONTROL);
        message_header(&mut buf, self.shared.tag_remote, key_pos, chunk::DATA, flags, extra + payload.len());
        buf.extend_from_slice(&seq.to_be_bytes());
        buf.extend_from_slice(&channel.to_be_bytes());
        buf.extend_from_slice(&[0, 0]);
        if first {
            buf.push(0);
        }
        buf.extend_from_slice(payload);
        packet_mac(local.crypt.as_mut(), &mut buf, key_pos);
        drop(local);
        self.shared.socket.send(&buf)?;
        Ok(seq)
    }

    /// Paquet de congestion : reçus et perdus depuis le précédent
    /// (`chiaki_takion_send_congestion`).
    pub fn send_congestion(&self) -> io::Result<(u16, u16)> {
        let received = self.shared.received.swap(0, Ordering::Relaxed).min(u16::MAX as u64) as u16;
        let lost = self.shared.lost.swap(0, Ordering::Relaxed).min(u16::MAX as u64) as u16;
        let mut local = self.shared.local.lock().unwrap();
        let key_pos = advance(&mut local, 0xf);
        let mut buf = vec![packet::CONGESTION];
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&received.to_be_bytes());
        buf.extend_from_slice(&lost.to_be_bytes());
        buf.extend_from_slice(&[0; 4]);
        buf.extend_from_slice(&(key_pos as u32).to_be_bytes());
        packet_mac(local.crypt.as_mut(), &mut buf, key_pos);
        drop(local);
        self.shared.socket.send(&buf)?;
        Ok((received, lost))
    }

    /// Envoie un paquet de retour (état de la manette, historique) : contenu
    /// chiffré à partir de l'octet 12, position et GMAC aux octets 4 et 8
    /// (`takion_send_feedback_packet`).
    pub fn send_feedback(&self, mut buf: Vec<u8>) -> io::Result<()> {
        if buf.len() < 0xc {
            return Err(io::Error::other("paquet de retour trop court"));
        }
        let mut local = self.shared.local.lock().unwrap();
        let payload = buf.len() - 0xc;
        let key_pos = advance(&mut local, payload + BLOCK_SIZE);
        let Some(crypt) = local.crypt.as_mut() else {
            return Err(io::Error::other("clés du flux pas encore connues"));
        };
        crypt.crypt(key_pos + BLOCK_SIZE as u64, &mut buf[0xc..]);
        buf[4..8].copy_from_slice(&(key_pos as u32).to_be_bytes());
        buf[8..12].fill(0);
        let mac = crypt.gmac(key_pos, &buf);
        buf[8..12].copy_from_slice(&mac);
        drop(local);
        self.shared.socket.send(&buf)?;
        Ok(())
    }

    /// Arrête le fil de réception.
    pub fn close(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Takion {
    fn drop(&mut self) {
        self.close();
    }
}

/// Nouvelle position de clé locale, avancée de `size` (arrondi comme
/// `chiaki_takion_crypt_advance_key_pos`) ; 0 tant que les clés ne sont pas
/// connues.
fn advance(local: &mut Local, size: usize) -> u64 {
    if local.crypt.is_none() {
        return 0;
    }
    let size = (size + size % BLOCK_SIZE) as u64;
    let pos = local.key_pos;
    local.key_pos += size;
    pos
}

fn recv_timeout(socket: &UdpSocket, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
    socket.set_read_timeout(Some(timeout))?;
    match socket.recv(buf) {
        Ok(n) => Ok(Some(n)),
        Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Un message lu : étiquette vérifiée, type, drapeaux, contenu.
fn parse_message(buf: &[u8], tag_local: u32) -> Option<(u8, u8, &[u8])> {
    if buf.len() < 1 + HEADER_SIZE {
        return None;
    }
    let m = &buf[1..];
    let tag = u32::from_be_bytes([m[0], m[1], m[2], m[3]]);
    let size = u16::from_be_bytes([m[0xe], m[0xf]]) as usize;
    if tag != tag_local || m.len() != size + 0xc || size < 4 {
        return None;
    }
    Some((m[0xc], m[0xd], &m[0x10..]))
}

/// INIT → INIT_ACK → COOKIE → COOKIE_ACK ; rend l'étiquette de la console.
fn handshake(socket: &UdpSocket, tag_local: u32) -> io::Result<u32> {
    let mut init = vec![packet::CONTROL];
    message_header(&mut init, 0, 0, chunk::INIT, 0, 0x10);
    init.extend_from_slice(&tag_local.to_be_bytes());
    init.extend_from_slice(&A_RWND.to_be_bytes());
    init.extend_from_slice(&STREAMS.to_be_bytes());
    init.extend_from_slice(&STREAMS.to_be_bytes());
    init.extend_from_slice(&tag_local.to_be_bytes());
    let mut buf = [0u8; 1500];
    let mut init_ack = None;
    for _ in 0..CONNECT_TRIES {
        socket.send(&init)?;
        if let Some(n) = recv_timeout(socket, &mut buf, EXPECT_TIMEOUT)? {
            if let Some((chunk::INIT_ACK, 0, payload)) = parse_message(&buf[..n], tag_local) {
                if payload.len() >= 0x10 + COOKIE_SIZE {
                    init_ack = Some(payload.to_vec());
                    break;
                }
            }
        }
        log::warn!("PlayStation : pas de réponse à INIT, nouvel essai");
    }
    let ack = init_ack.ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "la console ne répond pas sur Takion"))?;
    let tag_remote = u32::from_be_bytes([ack[0], ack[1], ack[2], ack[3]]);
    let outbound = u16::from_be_bytes([ack[8], ack[9]]);
    let inbound = u16::from_be_bytes([ack[0xa], ack[0xb]]);
    if tag_remote == 0 || outbound == 0 || inbound == 0 || outbound > STREAMS || inbound < STREAMS {
        return Err(io::Error::other("INIT_ACK refusé"));
    }
    let mut cookie = vec![packet::CONTROL];
    message_header(&mut cookie, tag_remote, 0, chunk::COOKIE, 0, COOKIE_SIZE);
    cookie.extend_from_slice(&ack[0x10..0x10 + COOKIE_SIZE]);
    for _ in 0..CONNECT_TRIES {
        socket.send(&cookie)?;
        let deadline = Instant::now() + EXPECT_TIMEOUT;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(n) = recv_timeout(socket, &mut buf, left)? else { break };
            match parse_message(&buf[..n], tag_local) {
                Some((chunk::COOKIE_ACK, 0, _)) => return Ok(tag_remote),
                // Un second INIT_ACK arrive parfois avant (chiaki-ng).
                Some((chunk::INIT_ACK, _, _)) => continue,
                _ => continue,
            }
        }
        log::warn!("PlayStation : pas de COOKIE_ACK, nouvel essai");
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "la console n'acquitte pas le cookie"))
}

fn send_data_ack(shared: &Shared, seq: u32) {
    let mut local = shared.local.lock().unwrap();
    let key_pos = advance(&mut local, 1 + HEADER_SIZE + 0xc);
    let mut buf = vec![packet::CONTROL];
    message_header(&mut buf, shared.tag_remote, key_pos, chunk::DATA_ACK, 0, 0xc);
    buf.extend_from_slice(&seq.to_be_bytes());
    buf.extend_from_slice(&A_RWND.to_be_bytes());
    buf.extend_from_slice(&[0; 4]);
    packet_mac(local.crypt.as_mut(), &mut buf, key_pos);
    drop(local);
    let _ = shared.socket.send(&buf);
}

/// Vérifie le GMAC d'un paquet reçu si les clés sont connues
/// (`takion_handle_packet_mac`). `false` : paquet à jeter.
fn check_mac(shared: &Shared, key_state: &mut KeyState, buf: &mut [u8]) -> bool {
    let mut remote = shared.remote.lock().unwrap();
    let Some(crypt) = remote.as_mut() else { return true };
    let kind = buf[0] & 0xf;
    let Some(ko) = key_pos_offset(kind) else { return true };
    if buf.len() < ko + 4 {
        return false;
    }
    let low = u32::from_be_bytes([buf[ko], buf[ko + 1], buf[ko + 2], buf[ko + 3]]);
    let key_pos = key_state.request(low, false);
    let mut copy = buf.to_vec();
    let Some(received) = packet_mac(Some(crypt), &mut copy, key_pos) else { return false };
    let mo = mac_offset(kind).unwrap_or(0);
    if copy[mo..mo + GMAC_SIZE] != received {
        log::warn!("PlayStation : GMAC faux (paquet de type {kind:#x}, position {key_pos:#x})");
        return false;
    }
    key_state.commit(key_pos);
    true
}

fn receive_loop(shared: Arc<Shared>, tag_local: u32, tag_remote: u32, v12: bool, tx: Sender<TakionEvent>) {
    let mut key_state = KeyState::default();
    // Messages DATA : le prochain attendu, et ceux arrivés en avance.
    let mut next_seq = tag_remote;
    let mut pending: BTreeMap<u32, (u8, Vec<u8>)> = BTreeMap::new();
    let mut postponed: Vec<Vec<u8>> = Vec::new();
    let mut last_video_index: Option<u16> = None;
    let mut buf = [0u8; 2048];
    let reason = loop {
        if shared.stop.load(Ordering::SeqCst) {
            break "arrêt demandé".to_string();
        }
        // Paquets d'image arrivés avant les clés : traités dès qu'elles sont là.
        if !postponed.is_empty() && shared.remote.lock().unwrap().is_some() {
            for mut p in std::mem::take(&mut postponed) {
                handle_av(&shared, &mut key_state, &mut p, v12, &mut last_video_index, &tx);
            }
        }
        let n = match recv_timeout(&shared.socket, &mut buf, Duration::from_millis(200)) {
            Ok(Some(n)) if n > 0 => n,
            Ok(_) => continue,
            Err(e) => break format!("réseau : {e}"),
        };
        let packet = &mut buf[..n];
        let kind = packet[0] & 0xf;
        match kind {
            packet::CONTROL => {
                if !check_mac(&shared, &mut key_state, packet) {
                    continue;
                }
                let Some((chunk_type, _flags, payload)) = parse_message(packet, tag_local) else { continue };
                match chunk_type {
                    chunk::DATA if payload.len() >= 9 => {
                        let seq = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                        pending.insert(seq, (payload[8], payload[9..].to_vec()));
                        let mut acked = None;
                        while let Some((data_type, content)) = pending.remove(&next_seq) {
                            acked = Some(next_seq);
                            next_seq = next_seq.wrapping_add(1);
                            let _ = tx.send(TakionEvent::Data(data_type, content));
                        }
                        // Un doublon déjà livré est acquitté de nouveau.
                        let ack = acked.unwrap_or_else(|| next_seq.wrapping_sub(1));
                        pending.retain(|&s, _| super::gkcrypt::seq32_gt(s, ack));
                        send_data_ack(&shared, ack);
                    }
                    chunk::DATA_ACK => {}
                    other => log::debug!("PlayStation : bloc Takion {other:#x} ignoré"),
                }
            }
            packet::VIDEO | packet::AUDIO => {
                if shared.remote.lock().unwrap().is_none() {
                    if postponed.len() < POSTPONE_MAX {
                        postponed.push(packet.to_vec());
                    }
                    continue;
                }
                handle_av(&shared, &mut key_state, packet, v12, &mut last_video_index, &tx);
            }
            other => log::debug!("PlayStation : paquet Takion {other:#x} ignoré"),
        }
    };
    log::info!("PlayStation : Takion fermé ({reason})");
    let _ = tx.send(TakionEvent::Closed(reason));
}

fn handle_av(shared: &Shared, key_state: &mut KeyState, packet: &mut [u8], v12: bool, last_video: &mut Option<u16>, tx: &Sender<TakionEvent>) {
    if !check_mac(shared, key_state, packet) {
        shared.lost.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let Some((mut av, off)) = parse_av(v12, packet, key_state) else { return };
    let mut payload = packet[off..].to_vec();
    if let Some(crypt) = shared.remote.lock().unwrap().as_ref() {
        crypt.crypt(av.key_pos + BLOCK_SIZE as u64, &mut payload);
    }
    av.data = payload;
    if av.is_video {
        if let Some(prev) = *last_video {
            let gap = av.packet_index.wrapping_sub(prev);
            if gap > 1 && gap < 0x8000 {
                shared.lost.fetch_add((gap - 1) as u64, Ordering::Relaxed);
            }
        }
        *last_video = Some(av.packet_index);
    }
    shared.received.fetch_add(1, Ordering::Relaxed);
    let _ = tx.send(TakionEvent::Av(av));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_round_trip() {
        let mut buf = vec![packet::CONTROL];
        message_header(&mut buf, 0x1234_5678, 0x42, chunk::DATA, 1, 3);
        buf.extend_from_slice(&[7, 8, 9]);
        let (t, f, p) = parse_message(&buf, 0x1234_5678).unwrap();
        assert_eq!((t, f, p), (chunk::DATA, 1, &[7u8, 8, 9][..]));
        assert!(parse_message(&buf, 1).is_none());
        assert_eq!(u32::from_be_bytes(buf[9..13].try_into().unwrap()), 0x42);
    }

    #[test]
    fn mac_without_crypt_zeroes() {
        let mut buf = vec![packet::CONTROL];
        message_header(&mut buf, 1, 0, chunk::DATA, 0, 0);
        buf[5..9].copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(packet_mac(None, &mut buf, 0), Some([1, 2, 3, 4]));
        assert_eq!(&buf[5..9], &[0; 4]);
    }

    #[test]
    fn mac_is_checked_with_key_pos_zeroed() {
        let mut c1 = GkCrypt::new(3, &[1; 16], &[2; 32]);
        let mut c2 = GkCrypt::new(3, &[1; 16], &[2; 32]);
        let mut buf = vec![packet::CONGESTION, 0, 0, 0, 5, 0, 1, 0, 0, 0, 0, 0, 0, 0x10, 0];
        packet_mac(Some(&mut c1), &mut buf, 0x1000);
        let sent = buf.clone();
        let old = packet_mac(Some(&mut c2), &mut buf, 0x1000).unwrap();
        assert_eq!(old[..], sent[7..11]);
        assert_eq!(buf, sent);
    }

    #[test]
    fn parses_v12_video_header() {
        // Type 2, index 5, image 9, unité 3 sur 10 dont 2 de correction,
        // codec 6, position 0x20.
        let mut p = vec![packet::VIDEO];
        p.extend_from_slice(&5u16.to_be_bytes());
        p.extend_from_slice(&9u16.to_be_bytes());
        let dword: u32 = (3 << 0x15) | (9 << 0xa) | 2;
        p.extend_from_slice(&dword.to_be_bytes());
        p.push(6);
        p.extend_from_slice(&[0; 4]);
        p.extend_from_slice(&0x20u32.to_be_bytes());
        // Trois octets d'image (le premier est l'« inconnu » de chiaki-ng),
        // flux adaptatif dans les 3 bits hauts du troisième.
        p.extend_from_slice(&[0, 0, 0x40]);
        p.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
        let mut ks = KeyState::default();
        let (av, off) = parse_av(true, &p, &mut ks).unwrap();
        assert!(av.is_video && !av.is_haptics);
        assert_eq!((av.packet_index, av.frame_index, av.unit_index), (5, 9, 3));
        assert_eq!((av.units_in_frame_total, av.units_in_frame_fec), (10, 2));
        assert_eq!((av.codec, av.key_pos, av.adaptive_stream_index), (6, 0x20, 2));
        assert_eq!(&p[off..], &[0xaa, 0xbb, 0xcc, 0xdd]);
    }
}
