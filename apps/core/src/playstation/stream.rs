//! Connexion du flux, d'après `streamconnection.c` de chiaki-ng : sur
//! Takion, l'app envoie BIG (version, identifiant de session, « launch
//! spec » chiffrée, clé publique ECDH signée), la console répond BANG (sa
//! clé publique) puis STREAMINFO (définitions avec leur en-tête vidéo,
//! en-tête du son). L'app acquitte, annonce la manette et le micro, puis
//! envoie un battement de cœur par seconde et un paquet de congestion toutes
//! les 200 ms. Pour finir, DISCONNECT.
//!
//! Pas de mesure du réseau (Senkusha) pour l'instant : les valeurs de
//! secours de chiaki-ng (MTU 1454, 1 ms) sont employées.

use std::io;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;

use super::discovery::Target;
use super::gkcrypt::{Ecdh, GkCrypt, HANDSHAKE_KEY_SIZE};
use super::proto::{self, Writer, kind};
use super::rpcrypt::{self, RpCrypt};
use super::controller::{self, ControllerState, History};
use super::frame::{Assembler, Frame};
use super::takion::{self, AvPacket, Takion, TakionEvent};

const EXPECT_TIMEOUT: Duration = Duration::from_secs(5);
const HEARTBEAT: Duration = Duration::from_secs(1);
const CONGESTION: Duration = Duration::from_millis(200);
/// Intervalle entre deux demandes d'image clé tant qu'aucune n'arrive.
const IDR_RETRY: Duration = Duration::from_millis(500);
/// Valeurs de secours de chiaki-ng quand la mesure du réseau échoue.
const MTU: u32 = 1454;
const RTT_MS: u32 = 1;

/// Ce que l'app demande à la console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    pub width: u32,
    pub height: u32,
    pub max_fps: u32,
    /// Débit en kbit/s.
    pub bitrate: u32,
    pub hevc: bool,
    pub hdr: bool,
    /// Annonce une DualSense (sinon une DualShock 4).
    pub dualsense: bool,
}

impl Profile {
    /// 1080p à 60 i/s, H.265, 15 Mbit/s (préréglage 1080p de chiaki-ng).
    pub fn p1080() -> Self {
        Profile { width: 1920, height: 1080, max_fps: 60, bitrate: 15000, hevc: true, hdr: false, dualsense: true }
    }
}

/// Une définition proposée par la console, avec l'en-tête vidéo (SPS/PPS…)
/// à placer devant la première image.
#[derive(Debug, Clone)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub header: Vec<u8>,
}

/// En-tête du son (`ChiakiAudioHeader`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioHeader {
    pub channels: u8,
    pub bits: u8,
    pub rate: u32,
    pub frame_size: u32,
}

impl AudioHeader {
    fn load(b: &[u8]) -> Option<Self> {
        if b.len() != 0xe {
            return None;
        }
        Some(AudioHeader {
            channels: b[0],
            bits: b[1],
            rate: u32::from_be_bytes(b[2..6].try_into().ok()?),
            frame_size: u32::from_be_bytes(b[6..10].try_into().ok()?),
        })
    }
}

/// Ce que le flux annonce.
#[derive(Debug)]
pub enum StreamEvent {
    /// La console envoie l'image : définitions et son.
    Started { resolutions: Vec<Resolution>, audio: AudioHeader },
    /// Une image assemblée (Annex B), prête pour le décodeur.
    Video(Frame),
    /// Un paquet de son (Opus) ou de vibrations HD (`is_haptics`).
    Audio(AvPacket),
    /// Des images ont été perdues : la suite s'appuie sur des références
    /// manquantes jusqu'à la prochaine image clé (déjà demandée).
    VideoLost,
    /// Données de la manette (vibrations, gâchettes, état) : type Takion et
    /// contenu.
    Pad(u8, Vec<u8>),
    /// Fin : la raison donnée par la console, ou la nôtre.
    Ended(String),
}

/// « Launch spec » de la console (`chiaki_launchspec_format`).
pub(crate) fn launch_spec(target: Target, profile: &Profile, handshake_key: &[u8; HANDSHAKE_KEY_SIZE]) -> String {
    let ps5 = rpcrypt::is_ps5(target);
    let (adaptive, codec, range) = if ps5 {
        (
            ",\"adaptiveStreamMode\": \"resize\"",
            if profile.hevc { "\"videoCodec\":\"hevc\"," } else { "\"videoCodec\":\"avc\"," },
            if profile.hdr { "\"dynamicRange\":\"HDR\"," } else { "\"dynamicRange\":\"SDR\"," },
        )
    } else {
        ("", "", "")
    };
    let key = base64::engine::general_purpose::STANDARD.encode(handshake_key);
    format!(
        "{{\"sessionId\":\"sessionId4321\",\"streamResolutions\":[{{\"resolution\":{{\"width\":{},\"height\":{}}},\"maxFps\":{},\"score\":10}}],\
\"network\":{{\"bwKbpsSent\":{},\"bwLoss\":0.001000,\"mtu\":{MTU},\"rtt\":{RTT_MS},\"ports\":[53,2053]}},\"slotId\":1,\
\"appSpecification\":{{\"minFps\":30,\"minBandwidth\":0,\"extTitleId\":\"ps3\",\"version\":1,\"timeLimit\":1,\"startTimeout\":100,\"afkTimeout\":100,\"afkTimeoutDisconnect\":100}},\
\"konan\":{{\"ps3AccessToken\":\"accessToken\",\"ps3RefreshToken\":\"refreshToken\"}},\"requestGameSpecification\":{{\"model\":\"bravia_tv\",\"platform\":\"android\",\
\"audioChannels\":\"5.1\",\"language\":\"sp\",\"acceptButton\":\"X\",\"connectedControllers\":[\"xinput\",\"ds3\",\"ds4\"],\"yuvCoefficient\":\"bt601\",\
\"videoEncoderProfile\":\"hw4.1\",\"audioEncoderProfile\":\"audio1\"{adaptive}}},\"userProfile\":{{\"onlineId\":\"psnId\",\"npId\":\"npId\",\"region\":\"US\",\
\"languagesUsed\":[\"en\",\"jp\"]}},{codec}{range}\"handshakeKey\":\"{key}\"}}",
        profile.width, profile.height, profile.max_fps, profile.bitrate
    )
}

/// Message BIG.
pub(crate) fn big_message(
    target: Target,
    profile: &Profile,
    session_id: &str,
    crypt: &RpCrypt,
    handshake_key: &[u8; HANDSHAKE_KEY_SIZE],
    ecdh: &Ecdh,
) -> Vec<u8> {
    // La launch spec, zéro final compris, est masquée par le flux de
    // `RpCrypt` (des zéros chiffrés au compteur 0), puis mise en base64.
    let mut json = launch_spec(target, profile, handshake_key).into_bytes();
    json.push(0);
    let mut mask = vec![0u8; json.len()];
    crypt.encrypt(0, &mut mask);
    for (j, m) in json.iter_mut().zip(&mask) {
        *j ^= m;
    }
    let spec = base64::engine::general_purpose::STANDARD.encode(&json);
    let (pub_key, sig) = ecdh.public_key(handshake_key);
    let v = if rpcrypt::is_ps5(target) { 12 } else { 9 };
    let big = Writer::default()
        .uint(1, v)
        .bytes(2, session_id.as_bytes())
        .bytes(3, spec.as_bytes())
        .bytes(4, &[0, 0, 0, 0])
        .bytes(5, &pub_key)
        .bytes(6, &sig);
    Writer::default().uint(1, kind::BIG).message(2, big).finish()
}

/// Envoie un message découpé selon la MTU, comme `stream_connection_send_big`.
fn send_split(takion: &Takion, msg: &[u8]) -> io::Result<()> {
    let mtu = (MTU - 50) as usize;
    let mut rest = msg;
    let mut first = true;
    while mtu < rest.len() + 26 || (mtu < rest.len() + 25 && !first) {
        let n = if first { mtu - 26 } else { mtu - 25 };
        takion.send_data(0, 1, &rest[..n], first)?;
        rest = &rest[n..];
        first = false;
    }
    if !rest.is_empty() {
        takion.send_data(1, 1, rest, first)?;
    }
    Ok(())
}

/// STREAMINFO du micro (`stream_connection_enable_microphone`) : l'en-tête
/// tel que chiaki-ng l'écrit (16 bits et 1 voie inversés à l'appel, comme
/// l'app de Sony).
fn mic_streaminfo() -> Vec<u8> {
    let mut header = vec![1u8, 16];
    header.extend_from_slice(&48000u32.to_be_bytes());
    header.extend_from_slice(&480u32.to_be_bytes());
    header.extend_from_slice(&1u32.to_be_bytes());
    Writer::default().uint(1, kind::STREAMINFO).message(15, Writer::default().bytes(2, &header)).finish()
}

fn controller_connection(dualsense: bool) -> Vec<u8> {
    let payload = Writer::default()
        .uint(2, 1)
        .uint(3, if dualsense { proto::CONTROLLER_DUALSENSE } else { proto::CONTROLLER_DUALSHOCK4 });
    Writer::default().uint(1, kind::CONTROLLERCONNECTION).message(22, payload).finish()
}

/// CORRUPTFRAME : images perdues de `start` à `end` (la console renvoie
/// une image de référence).
fn corrupt_frame(start: u16, end: u16) -> Vec<u8> {
    Writer::default()
        .uint(1, kind::CORRUPTFRAME)
        .message(6, Writer::default().uint(1, start as u64).uint(2, end as u64))
        .finish()
}

fn disconnect_message() -> Vec<u8> {
    Writer::default()
        .uint(1, kind::DISCONNECT)
        .message(10, Writer::default().bytes(1, b"Client Disconnecting"))
        .finish()
}

/// Raison d'un DISCONNECT de la console.
fn disconnect_reason(f: &[(u32, proto::Value<'_>)]) -> String {
    proto::bytes(f, 10)
        .and_then(proto::fields)
        .and_then(|d| proto::bytes(&d, 1).map(|r| String::from_utf8_lossy(r).into_owned()))
        .unwrap_or_else(|| "la console a fermé le flux".into())
}

/// STREAMINFO : définitions et en-tête du son.
pub(crate) fn parse_streaminfo(f: &[(u32, proto::Value<'_>)]) -> Option<(Vec<Resolution>, AudioHeader)> {
    let info = proto::fields(proto::bytes(f, 15)?)?;
    let audio = AudioHeader::load(proto::bytes(&info, 2)?)?;
    let resolutions = proto::all_bytes(&info, 1)
        .into_iter()
        .filter_map(|r| {
            let r = proto::fields(r)?;
            Some(Resolution {
                width: proto::uint(&r, 1)? as u32,
                height: proto::uint(&r, 2)? as u32,
                header: proto::bytes(&r, 3)?.to_vec(),
            })
        })
        .collect();
    Some((resolutions, audio))
}

/// Le flux ouvert ; un fil `ps-stream` mène l'échange et le tient en vie.
pub struct Stream {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Stream {
    /// Ouvre le flux après le canal de contrôle : `session_id` vient de
    /// [`super::session::CtrlEvent::SessionId`], `crypt` est celui du canal
    /// de contrôle ([`super::session::Ctrl::crypt`]).
    /// `controller` : lu toutes les 4 ms une fois le flux ouvert (l'état de
    /// la manette à envoyer) ; aucun : pas de manette.
    pub fn start(
        addr: Ipv4Addr,
        target: Target,
        session_id: String,
        crypt: RpCrypt,
        profile: Profile,
        controller: Option<ControllerSource>,
        want_key: Arc<AtomicBool>,
    ) -> io::Result<(Self, Receiver<StreamEvent>)> {
        let (takion, events) = Takion::connect(addr, takion::PORT, rpcrypt::is_ps5(target))?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("ps-stream".into())
                .spawn(move || run(takion, events, target, session_id, crypt, profile, controller, want_key, stop, tx))?
        };
        Ok((Stream { stop, thread: Some(thread) }, rx))
    }

    /// Termine le flux (DISCONNECT envoyé à la console).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Ce qui donne l'état de la manette.
pub type ControllerSource = Box<dyn FnMut() -> ControllerState + Send>;

/// Fil de la manette (`feedbacksender.c`) : l'état à chaque changement (8 ms
/// au moins entre deux) ou toutes les 200 ms, l'historique à chaque
/// changement de boutons, renvoyé 20, 50, 100 et 200 ms après.
fn feedback_loop(sender: takion::TakionSender, mut source: ControllerSource, v12: bool, stop: Arc<AtomicBool>) {
    let poll = Duration::from_millis(4);
    let min_gap = Duration::from_millis(8);
    let max_gap = Duration::from_millis(200);
    let tail = [20u64, 50, 100, 200];
    let mut prev = ControllerState::default();
    let mut history = History::new();
    let mut state_seq: u16 = 0;
    let mut last_state = Instant::now() - max_gap;
    let mut pending_state = true;
    let mut last_history: Option<(Vec<u8>, Instant, usize)> = None;
    while !stop.load(Ordering::SeqCst) {
        let now_state = source();
        if now_state != prev {
            if let Some((seq, count, body)) = history.push(controller::history_events(&prev, &now_state)) {
                let packet = controller::history_packet(seq, count, &body);
                let _ = sender.send_feedback(packet.clone());
                last_history = Some((packet, Instant::now(), 0));
            }
            pending_state = pending_state || (now_state.left_x, now_state.left_y, now_state.right_x, now_state.right_y, now_state.gyro, now_state.accel, now_state.orient)
                != (prev.left_x, prev.left_y, prev.right_x, prev.right_y, prev.gyro, prev.accel, prev.orient);
            prev = now_state;
        }
        let since = last_state.elapsed();
        if (pending_state && since >= min_gap) || since >= max_gap {
            let _ = sender.send_feedback(controller::state_packet(state_seq, &prev, v12));
            state_seq = state_seq.wrapping_add(1);
            last_state = Instant::now();
            pending_state = false;
        }
        if let Some((packet, sent, n)) = last_history.as_mut() {
            if *n < tail.len() && sent.elapsed() >= Duration::from_millis(tail[*n]) {
                let _ = sender.send_feedback(packet.clone());
                *n += 1;
            }
        }
        std::thread::sleep(poll);
    }
}

enum Phase {
    ExpectBang,
    ExpectStreamInfo,
    Running,
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut takion: Takion,
    events: Receiver<TakionEvent>,
    target: Target,
    session_id: String,
    crypt: RpCrypt,
    profile: Profile,
    mut controller: Option<ControllerSource>,
    want_key: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    tx: Sender<StreamEvent>,
) {
    // Demande d'image clé (IDRREQUEST) répétée tant qu'il en faut une.
    let mut last_idr: Option<Instant> = None;
    let mut feedback: Option<std::thread::JoinHandle<()>> = None;
    let feedback_stop = Arc::new(AtomicBool::new(false));
    let mut handshake_key = [0u8; HANDSHAKE_KEY_SIZE];
    let _ = getrandom::getrandom(&mut handshake_key);
    let ecdh = Ecdh::generate();
    let end = |reason: String| {
        log::info!("PlayStation : fin du flux ({reason})");
        reason
    };
    if let Err(e) = send_split(&takion, &big_message(target, &profile, &session_id, &crypt, &handshake_key, &ecdh)) {
        let _ = tx.send(StreamEvent::Ended(end(format!("BIG non envoyé : {e}"))));
        return;
    }
    log::info!("PlayStation : BIG envoyé");
    let mut phase = Phase::ExpectBang;
    let mut early_streaminfo: Option<Vec<u8>> = None;
    let mut deadline = Instant::now() + EXPECT_TIMEOUT;
    let mut next_heartbeat = Instant::now() + HEARTBEAT;
    let mut next_congestion = Instant::now() + CONGESTION;
    let mut send_disconnect = true;
    let mut assembler: Option<Assembler> = None;
    let reason = loop {
        if stop.load(Ordering::SeqCst) {
            break "arrêt demandé".to_string();
        }
        let now = Instant::now();
        if !matches!(phase, Phase::Running) && now >= deadline {
            break match phase {
                Phase::ExpectBang => "pas de BANG de la console".into(),
                _ => "pas de STREAMINFO de la console".into(),
            };
        }
        if now >= next_congestion {
            let _ = takion.send_congestion();
            next_congestion = now + CONGESTION;
        }
        if matches!(phase, Phase::Running) && want_key.load(Ordering::Relaxed) && last_idr.is_none_or(|t| t.elapsed() >= IDR_RETRY) {
            log::info!("PlayStation : image clé demandée à la console");
            let _ = takion.send_data(1, 2, &proto::simple(kind::IDRREQUEST), true);
            last_idr = Some(now);
        }
        if !want_key.load(Ordering::Relaxed) {
            last_idr = None;
        }
        if matches!(phase, Phase::Running) && now >= next_heartbeat {
            let _ = takion.send_data(1, 1, &proto::simple(kind::HEARTBEAT), true);
            next_heartbeat = now + HEARTBEAT;
        }
        let msg = if let Some(early) = early_streaminfo.take().filter(|_| matches!(phase, Phase::ExpectStreamInfo)) {
            TakionEvent::Data(takion::data::PROTOBUF, early)
        } else {
            match events.recv_timeout(Duration::from_millis(50)) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break "Takion arrêté".into(),
            }
        };
        match msg {
            TakionEvent::Closed(why) => {
                send_disconnect = false;
                break why;
            }
            TakionEvent::Av(av) if av.is_video => {
                let Some(a) = assembler.as_mut() else { continue };
                let out = a.push(&av);
                if let Some((start, end)) = out.missing {
                    log::info!("PlayStation : images {start} à {end} perdues, signalées à la console");
                    let _ = takion.send_data(1, 2, &corrupt_frame(start, end), true);
                    want_key.store(true, Ordering::Relaxed);
                    let _ = tx.send(StreamEvent::VideoLost);
                }
                if let Some(frame) = out.frame {
                    let _ = tx.send(StreamEvent::Video(frame));
                }
            }
            TakionEvent::Av(av) => {
                let _ = tx.send(StreamEvent::Audio(av));
            }
            TakionEvent::Data(takion::data::PROTOBUF, body) => {
                let Some(f) = proto::fields(&body) else { continue };
                let t = proto::uint(&f, 1).unwrap_or(u64::MAX);
                if t == kind::DISCONNECT {
                    send_disconnect = false;
                    break disconnect_reason(&f);
                }
                match phase {
                    Phase::ExpectBang if t == kind::BANG => {
                        let Some(bang) = proto::bytes(&f, 3).and_then(proto::fields) else { continue };
                        let accepted = proto::uint(&bang, 4) == Some(1) && proto::uint(&bang, 3) == Some(1);
                        let secret = proto::bytes(&bang, 8).and_then(|k| ecdh.derive(k));
                        match (accepted, secret) {
                            (true, Some(secret)) => {
                                takion.set_crypt(GkCrypt::new(2, &handshake_key, &secret), GkCrypt::new(3, &handshake_key, &secret));
                                log::info!("PlayStation : BANG reçu, clés du flux en place");
                                phase = Phase::ExpectStreamInfo;
                                deadline = Instant::now() + EXPECT_TIMEOUT;
                            }
                            (false, _) => break "la console refuse la version ou la clé".into(),
                            (true, None) => break "clé publique de la console illisible".into(),
                        }
                    }
                    Phase::ExpectBang if t == kind::STREAMINFO => early_streaminfo = Some(body),
                    Phase::ExpectStreamInfo if t == kind::STREAMINFO => {
                        let Some((resolutions, audio)) = parse_streaminfo(&f) else {
                            break "STREAMINFO illisible".into();
                        };
                        let _ = takion.send_data(1, 9, &proto::simple(kind::STREAMINFOACK), true);
                        let _ = takion.send_data(1, 1, &controller_connection(profile.dualsense), true);
                        let _ = takion.send_data(1, 1, &mic_streaminfo(), true);
                        log::info!("PlayStation : flux ouvert ({} définition(s), son {audio:?})", resolutions.len());
                        assembler = Some(Assembler::new(resolutions.iter().map(|r| r.header.clone()).collect()));
                        let _ = tx.send(StreamEvent::Started { resolutions, audio });
                        if let Some(source) = controller.take() {
                            let sender = takion.sender();
                            let fstop = feedback_stop.clone();
                            let v12 = rpcrypt::is_ps5(target);
                            feedback = std::thread::Builder::new()
                                .name("ps-manette".into())
                                .spawn(move || feedback_loop(sender, source, v12, fstop))
                                .ok();
                        }
                        phase = Phase::Running;
                    }
                    Phase::Running if t == kind::CONNECTIONQUALITY => log::debug!("PlayStation : qualité de connexion reçue"),
                    _ => log::debug!("PlayStation : message {t} ignoré"),
                }
            }
            TakionEvent::Data(other, body) => {
                let _ = tx.send(StreamEvent::Pad(other, body));
            }
        }
    };
    feedback_stop.store(true, Ordering::SeqCst);
    if let Some(f) = feedback {
        let _ = f.join();
    }
    if send_disconnect {
        let _ = takion.send_data(1, 1, &disconnect_message(), true);
    }
    takion.close();
    let _ = tx.send(StreamEvent::Ended(end(reason)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_spec_ps5() {
        let s = launch_spec(Target::Ps5V1, &Profile::p1080(), &[0u8; 16]);
        assert!(s.starts_with("{\"sessionId\":\"sessionId4321\",\"streamResolutions\":[{\"resolution\":{\"width\":1920,\"height\":1080},\"maxFps\":60,"));
        assert!(s.contains("\"bwKbpsSent\":15000,\"bwLoss\":0.001000,\"mtu\":1454,\"rtt\":1,"));
        assert!(s.contains("\"audioEncoderProfile\":\"audio1\",\"adaptiveStreamMode\": \"resize\"},"));
        assert!(s.ends_with("\"videoCodec\":\"hevc\",\"dynamicRange\":\"SDR\",\"handshakeKey\":\"AAAAAAAAAAAAAAAAAAAAAA==\"}"));
        serde_json::from_str::<serde_json::Value>(&s).unwrap();
    }

    #[test]
    fn launch_spec_ps4_has_no_extras() {
        let s = launch_spec(Target::Ps4V10, &Profile::p1080(), &[0u8; 16]);
        assert!(!s.contains("videoCodec") && !s.contains("adaptiveStreamMode"));
        assert!(s.contains("\"languagesUsed\":[\"en\",\"jp\"]},\"handshakeKey\""));
    }

    #[test]
    fn big_unmasks_with_session_keys() {
        let crypt = RpCrypt::auth(Target::Ps5V1, &[1; 16], &[2; 16]);
        let ecdh = Ecdh::generate();
        let key = [9u8; 16];
        let msg = big_message(Target::Ps5V1, &Profile::p1080(), "SessionIdABC", &crypt, &key, &ecdh);
        let f = proto::fields(&msg).unwrap();
        assert_eq!(proto::uint(&f, 1), Some(kind::BIG));
        let big = proto::fields(proto::bytes(&f, 2).unwrap()).unwrap();
        assert_eq!(proto::uint(&big, 1), Some(12));
        assert_eq!(proto::bytes(&big, 2), Some(&b"SessionIdABC"[..]));
        assert_eq!(proto::bytes(&big, 4), Some(&[0u8; 4][..]));
        assert_eq!(proto::bytes(&big, 5).unwrap().len(), 65);
        let mut spec = base64::engine::general_purpose::STANDARD.decode(proto::bytes(&big, 3).unwrap()).unwrap();
        let mut mask = vec![0u8; spec.len()];
        crypt.encrypt(0, &mut mask);
        for (s, m) in spec.iter_mut().zip(&mask) {
            *s ^= m;
        }
        assert_eq!(spec.pop(), Some(0));
        assert_eq!(String::from_utf8(spec).unwrap(), launch_spec(Target::Ps5V1, &Profile::p1080(), &key));
    }

    #[test]
    fn streaminfo_parsed() {
        let res = Writer::default().uint(1, 1920).uint(2, 1080).bytes(3, &[0, 0, 0, 1, 0x40]);
        let mut audio = vec![2u8, 16];
        audio.extend_from_slice(&48000u32.to_be_bytes());
        audio.extend_from_slice(&480u32.to_be_bytes());
        audio.extend_from_slice(&1u32.to_be_bytes());
        let msg = Writer::default()
            .uint(1, kind::STREAMINFO)
            .message(15, Writer::default().message(1, res).bytes(2, &audio))
            .finish();
        let (r, a) = parse_streaminfo(&proto::fields(&msg).unwrap()).unwrap();
        assert_eq!((r.len(), r[0].width, r[0].height), (1, 1920, 1080));
        assert_eq!(r[0].header, vec![0, 0, 0, 1, 0x40]);
        assert_eq!(a, AudioHeader { channels: 2, bits: 16, rate: 48000, frame_size: 480 });
    }

    #[test]
    fn corrupt_frame_message() {
        let msg = corrupt_frame(10, 12);
        let f = proto::fields(&msg).unwrap();
        assert_eq!(proto::uint(&f, 1), Some(kind::CORRUPTFRAME));
        let c = proto::fields(proto::bytes(&f, 6).unwrap()).unwrap();
        assert_eq!((proto::uint(&c, 1), proto::uint(&c, 2)), (Some(10), Some(12)));
    }

    #[test]
    fn mic_header_matches_chiaki() {
        let msg = mic_streaminfo();
        let f = proto::fields(&msg).unwrap();
        let info = proto::fields(proto::bytes(&f, 15).unwrap()).unwrap();
        assert_eq!(proto::bytes(&info, 2).unwrap(), &[1, 16, 0, 0, 0xbb, 0x80, 0, 0, 1, 0xe0, 0, 0, 0, 1]);
    }

    #[test]
    fn split_sizes_follow_chiaki() {
        // MTU 1454 - 50 = 1404 : un message de 1500 octets part en 1378 + 122.
        let mtu = 1404usize;
        let total = 1500usize;
        assert!(mtu < total + 26);
        assert_eq!(mtu - 26, 1378);
        assert!(!(mtu < (total - 1378) + 25));
    }
}
