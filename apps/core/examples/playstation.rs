//! Essai des consoles PlayStation depuis le terminal (docs/playstation.md).
//!
//! ```text
//! playstation search [adresse…]
//! playstation login                     (affiche la page de Sony, lit l'adresse de retour collée)
//! playstation login <adresse de retour> (même chose sans attendre la saisie)
//! playstation account <base64>          (identifiant saisi à la main, en secours)
//! playstation regist <adresse> <code>   (associe la console : écran « Associer un appareil » ouvert)
//! playstation wake <adresse>            (réveille une console associée)
//! playstation session <adresse> [h264|h265|hdr] (demande de session et canal de contrôle, 20 s)
//! playstation stream <adresse> [secondes] [hdr] (ouvre le flux ; image en Annex B dans PS_DUMP)
//! ```
//!
//! L'identifiant et les consoles associées sont gardés dans le fichier
//! `PS_STATE` (par défaut `playstation-essai.json` dans le dossier courant).

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use kybernow_core::playstation::discovery::{self, Model};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, Registered};
use kybernow_core::playstation::session::{self, Codec, Ctrl, CtrlEvent, Keys};
use kybernow_core::playstation::stream::{Profile, Stream, StreamEvent};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct State {
    account_id: Option<String>,
    /// Par `host-id` (adresse matérielle).
    consoles: BTreeMap<String, Registered>,
}

fn state_path() -> String {
    std::env::var("PS_STATE").unwrap_or_else(|_| "playstation-essai.json".into())
}

fn load() -> State {
    std::fs::read_to_string(state_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(state: &State) {
    std::fs::write(state_path(), serde_json::to_string_pretty(state).unwrap()).expect("écriture de l'état");
}

fn find(addr: Ipv4Addr) -> discovery::Console {
    discovery::search(Duration::from_secs(2), &[addr])
        .expect("recherche")
        .into_iter()
        .find(|c| c.addr == addr)
        .expect("aucune console à cette adresse")
}

fn main() {
    let _ = simple_log();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("search") => {
            let direct: Vec<Ipv4Addr> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
            let consoles = discovery::search(Duration::from_secs(3), &direct).expect("recherche");
            if consoles.is_empty() {
                println!("Aucune console trouvée.");
            }
            let state = load();
            for c in consoles {
                println!(
                    "{} « {} » {} {:?} id={} système={} cible={:?} jeu={} associée={}",
                    c.model().name(),
                    c.name,
                    c.addr,
                    c.state,
                    c.host_id,
                    c.system_version,
                    c.target(),
                    c.running_app.as_deref().unwrap_or("-"),
                    if state.consoles.contains_key(&c.host_id.to_ascii_uppercase()) { "oui" } else { "non" },
                );
            }
        }
        Some("login") => {
            let url = match args.get(1) {
                Some(u) => u.clone(),
                None => {
                    println!("Ouvrez cette page, connectez-vous, puis collez ici l'adresse de la page « redirect » :\n\n{}\n", psn::LOGIN_URL);
                    let mut line = String::new();
                    std::io::stdin().read_line(&mut line).expect("lecture");
                    line
                }
            };
            let code = psn::code_from_redirect(url.trim()).expect("pas de code dans l'adresse");
            let id = psn::account_id(&code).unwrap_or_else(|e| panic!("{e}"));
            let mut state = load();
            state.account_id = id.to_base64();
            save(&state);
            println!("Identifiant du compte lu et gardé dans {}.", state_path());
        }
        Some("account") if args.len() == 2 => {
            let id = PsnId::from_base64(&args[1]).expect("identifiant base64 de 8 octets");
            let mut state = load();
            state.account_id = id.to_base64();
            save(&state);
            println!("Identifiant gardé dans {}.", state_path());
        }
        Some("regist") if args.len() == 3 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let pin: u32 = args[2].replace(' ', "").parse().expect("code à 8 chiffres");
            let mut state = load();
            let id = state
                .account_id
                .as_deref()
                .and_then(PsnId::from_base64)
                .expect("pas d'identifiant : lancez d'abord « login »");
            let console = find(addr);
            println!("Association de « {} » ({:?})…", console.name, console.target());
            match regist::regist(addr, console.target(), &id, pin) {
                Ok(r) => {
                    println!("Associée : « {} », {}, clé de réveil {:?}", r.nickname, r.mac, r.wake_credential().is_some());
                    state.consoles.insert(r.mac.clone(), r);
                    save(&state);
                }
                Err(e) => {
                    eprintln!("Échec : {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("wake") if args.len() == 2 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let console = find(addr);
            let state = load();
            let r = state
                .consoles
                .get(&console.host_id.to_ascii_uppercase())
                .expect("console pas encore associée : lancez « regist »");
            let model = if console.model() == Model::Ps5 { Model::Ps5 } else { Model::Ps4 };
            discovery::wake(IpAddr::V4(addr), model, r.wake_credential().expect("clé de réveil")).expect("réveil");
            println!("Réveil envoyé à « {} ».", console.name);
        }
        Some("session") if args.len() >= 2 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let codec = match args.get(2).map(String::as_str) {
                Some("h264") => Codec::H264,
                Some("hdr") => Codec::H265Hdr,
                _ => Codec::H265,
            };
            let console = find(addr);
            let state = load();
            let r = state.consoles.get(&console.host_id.to_ascii_uppercase()).expect("console pas encore associée");
            let keys = Keys::from_registered(r).expect("association illisible");
            let target = r.target.into();
            let (target, nonce) = session::request_session(addr, target, &keys).unwrap_or_else(|e| {
                eprintln!("Demande de session refusée : {e}");
                std::process::exit(1)
            });
            println!("Session accordée ({target:?}), ouverture du canal de contrôle…");
            let (mut ctrl, events) = Ctrl::connect(addr, target, &nonce, &keys, codec).unwrap_or_else(|e| {
                eprintln!("Canal de contrôle refusé : {e}");
                std::process::exit(1)
            });
            println!("Type de console : {:?}, affichage interdit : {}", ctrl.server_type, ctrl.prohibited);
            let start = std::time::Instant::now();
            let deadline = start + Duration::from_secs(20);
            while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
                match events.recv_timeout(left) {
                    Ok(CtrlEvent::SessionId(id)) => {
                        println!("Identifiant de session reçu ({} caractères).", id.len());
                        ctrl.enable_features(true).expect("envoi");
                    }
                    Ok(CtrlEvent::Closed(why)) => {
                        println!("Canal fermé après {:.1} s : {why}", start.elapsed().as_secs_f32());
                        break;
                    }
                    Ok(e) => println!("{:.1} s : {e:?}", start.elapsed().as_secs_f32()),
                    Err(_) => break,
                }
            }
            ctrl.stop();
            println!("Fin de l'essai après {:.1} s.", start.elapsed().as_secs_f32());
        }
        Some("stream") if args.len() >= 2 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
            let hdr = args.get(3).map(String::as_str) == Some("hdr");
            let console = find(addr);
            let state = load();
            let r = state.consoles.get(&console.host_id.to_ascii_uppercase()).expect("console pas encore associée");
            let keys = Keys::from_registered(r).expect("association illisible");
            // Comme l'app (play.rs) : « déjà en cours » réessayé 30 s, canal
            // de contrôle réessayé sur la même session (juste après un réveil).
            let asked = std::time::Instant::now();
            let (target, nonce) = loop {
                match session::request_session(addr, r.target.into(), &keys) {
                    Ok(t) => break t,
                    Err(e) if asked.elapsed() < Duration::from_secs(30) => {
                        eprintln!("Demande de session refusée ({e}), nouvel essai");
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    Err(e) => {
                        eprintln!("Demande de session refusée : {e}");
                        std::process::exit(1)
                    }
                }
            };
            let codec = if hdr { Codec::H265Hdr } else { Codec::H265 };
            let mut tries = 0;
            let (mut ctrl, events) = loop {
                match Ctrl::connect(addr, target, &nonce, &keys, codec) {
                    Ok(c) => break c,
                    Err(_) if tries < 5 => {
                        tries += 1;
                        std::thread::sleep(Duration::from_secs(2));
                    }
                    Err(e) => {
                        eprintln!("Canal de contrôle refusé : {e}");
                        std::process::exit(1)
                    }
                }
            };
            let session_id = loop {
                match events.recv_timeout(Duration::from_secs(10)) {
                    Ok(CtrlEvent::SessionId(id)) => break id,
                    Ok(_) => {}
                    Err(_) => {
                        eprintln!("Pas d'identifiant de session.");
                        std::process::exit(1)
                    }
                }
            };
            ctrl.enable_features(true).expect("envoi");
            // PS_RES=2560x1440, PS_KBPS=40000 : essais de définition et de débit.
            let mut profile = Profile { hdr, ..Profile::p1080() };
            // PS_IDR_AT=3 : demande une image clé à 3 s (essai de la reprise).
            let want_key = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let idr_at: Option<u64> = std::env::var("PS_IDR_AT").ok().and_then(|v| v.parse().ok());
            if let Some((w, h)) = std::env::var("PS_RES").ok().and_then(|r| r.split_once('x').map(|(w, h)| (w.parse().unwrap_or(1920), h.parse().unwrap_or(1080)))) {
                profile.width = w;
                profile.height = h;
            }
            if let Some(k) = std::env::var("PS_KBPS").ok().and_then(|k| k.parse().ok()) {
                profile.bitrate = k;
            }
            let (mut stream, events) = Stream::start(addr, target, session_id, ctrl.crypt().clone(), profile, None, want_key.clone()).unwrap_or_else(|e| {
                eprintln!("Flux impossible : {e}");
                std::process::exit(1)
            });
            // PS_DUMP : l'image en Annex B, lisible par ffprobe / ffplay.
            let mut dump = std::env::var("PS_DUMP").ok().map(|p| std::fs::File::create(p).expect("PS_DUMP"));
            let start = std::time::Instant::now();
            let (mut frames, mut recovered, mut audio, mut haptics, mut bytes, mut keys) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let mut first_frame: Option<f32> = None;
            while start.elapsed() < Duration::from_secs(secs) {
                match events.recv_timeout(Duration::from_millis(200)) {
                    Ok(StreamEvent::Started { resolutions, audio: a }) => {
                        println!("{:.1} s : flux ouvert, son {a:?}", start.elapsed().as_secs_f32());
                        for r in &resolutions {
                            println!("  définition {}x{}, en-tête vidéo de {} octets", r.width, r.height, r.header.len());
                        }
                    }
                    Ok(StreamEvent::Video(f)) => {
                        if let Some(at) = idr_at {
                            if start.elapsed().as_secs() >= at && frames > 0 && !want_key.load(std::sync::atomic::Ordering::Relaxed) && keys == 1 {
                                println!("{:.1} s : image clé demandée", start.elapsed().as_secs_f32());
                                want_key.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        // Image clé : NAL HEVC 16 à 21 après un code de début.
                        if f.data.windows(4).any(|w| w[0] == 0 && w[1] == 0 && w[2] == 1 && (16..=21).contains(&((w[3] >> 1) & 0x3f))) {
                            keys += 1;
                            want_key.store(false, std::sync::atomic::Ordering::Relaxed);
                            println!("{:.1} s : image clé (n° {})", start.elapsed().as_secs_f32(), f.index);
                        }
                        if std::env::var("PS_FRAMES").is_ok() {
                            println!("{:.2} s : image n° {} {} ({} Ko)", start.elapsed().as_secs_f32(), f.index, hevc_kind(&f.data), f.data.len() / 1024);
                        }
                        if f.data.len() > 100_000 || f.recovered {
                            println!("{:.1} s : image n° {} de {} Ko{}", start.elapsed().as_secs_f32(), f.index, f.data.len() / 1024, if f.recovered { ", réparée" } else { "" });
                        }
                        frames += 1;
                        bytes += f.data.len() as u64;
                        recovered += f.recovered as u64;
                        first_frame.get_or_insert(start.elapsed().as_secs_f32());
                        if let Some(out) = dump.as_mut() {
                            use std::io::Write;
                            let _ = out.write_all(&f.data);
                        }
                    }
                    Ok(StreamEvent::Audio(p)) => {
                        if p.is_haptics {
                            haptics += 1
                        } else {
                            audio += 1
                        }
                    }
                    Ok(StreamEvent::VideoDropped) => println!("{:.1} s : images perdues, réparation attendue", start.elapsed().as_secs_f32()),
                    Ok(StreamEvent::VideoLost) => println!("{:.1} s : images perdues, image clé demandée", start.elapsed().as_secs_f32()),
                    Ok(StreamEvent::Pad(t, b)) => println!("{:.1} s : données manette type {t} ({} octets)", start.elapsed().as_secs_f32(), b.len()),
                    Ok(StreamEvent::Ended(why)) => {
                        println!("{:.1} s : fin : {why}", start.elapsed().as_secs_f32());
                        break;
                    }
                    Err(_) => {}
                }
            }
            println!(
                "Images : {frames} ({:.1} Mo, {recovered} réparées), première après {:?} s ; son : {audio} paquets ; vibrations : {haptics}",
                bytes as f64 / 1e6,
                first_frame
            );
            stream.stop();
            ctrl.stop();
        }
        _ => eprintln!("usage : search [adresse…] | login [adresse de retour] | account <base64> | regist <adresse> <code> | wake <adresse> | session <adresse> [h264|h265|hdr] | stream <adresse> [secondes]"),
    }
}

fn simple_log() -> Result<(), log::SetLoggerError> {
    struct L;
    impl log::Log for L {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, r: &log::Record) {
            eprintln!("[{}] {}", r.level(), r.args());
        }
        fn flush(&self) {}
    }
    log::set_logger(&L).map(|()| log::set_max_level(if std::env::var("PS_DEBUG").is_ok() { log::LevelFilter::Debug } else { log::LevelFilter::Info }))
}

/// Genre de la première tranche HEVC d'une image : type de NAL et type de
/// tranche (0 B, 1 P, 2 I), en supposant 0 bit d'en-tête en plus (PPS).
fn hevc_kind(data: &[u8]) -> String {
    let mut i = 0;
    while i + 4 < data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            let nal = (data[i + 3] >> 1) & 0x3f;
            if nal < 32 {
                let mut r = BitReader { d: &data[i + 5..], bit: 0 };
                let first = r.bit();
                if (16..=23).contains(&nal) {
                    r.bit();
                }
                let _pps = r.ue();
                let ty = if first == 1 { r.ue() } else { 99 };
                return format!("NAL {nal} tranche {}", match ty { 0 => "B", 1 => "P", 2 => "I", _ => "?" });
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    "sans tranche".into()
}

struct BitReader<'a> {
    d: &'a [u8],
    bit: usize,
}

impl BitReader<'_> {
    fn bit(&mut self) -> u32 {
        let b = self.d.get(self.bit / 8).map(|x| (x >> (7 - self.bit % 8)) & 1).unwrap_or(0);
        self.bit += 1;
        b as u32
    }
    fn ue(&mut self) -> u32 {
        let mut zeros = 0;
        while self.bit() == 0 && zeros < 31 {
            zeros += 1;
        }
        let mut v = 1u32;
        for _ in 0..zeros {
            v = (v << 1) | self.bit();
        }
        v - 1
    }
}
