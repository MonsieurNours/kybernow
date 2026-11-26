//! `kybernow-micro` : le micro de l'appareil sur la Station (docs/micro.md).
//!
//! Lancé par kycontroller (`--uplink`, flux montant commun du correctif 0085)
//! pour chaque session qui envoie son micro : lit les paquets Opus sur
//! l'entrée standard, les décode, et les
//! joue dans le micro virtuel de l'ordinateur (VB-CABLE sur Windows,
//! « Micro KyberNow » sur Mac et Linux), au rythme de sa sortie. Pendant
//! l'envoi, le micro virtuel est le micro par défaut ; le précédent revient
//! à la fin.
//!
//! Ses réponses vont à l'app en JSON, une ligne par message, sur la sortie
//! standard (`{"state": "failed", "reason": "no_device"}` : micro virtuel
//! absent). `kybernow-micro --check` (Admin) : 0 si le micro virtuel est là,
//! 3 s'il manque, 1 si rien ne peut marcher ici. Le journal va sur la sortie
//! d'erreur (dans celui de kycontroller).

// Sans fenêtre sur Windows : lancé par kycontroller pendant une session, un
// programme console ouvrait un terminal noir sur le bureau. L'entrée et la
// sortie standard restent les tubes de kycontroller (ou d'Admin, `--check`).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod jitter;
mod out;
mod stream;

use std::io::BufReader;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jitter::Jitter;
use stream::{Decoder, Record};

pub const RATE: u32 = 48_000;

/// Code de sortie de `--check` quand le micro virtuel manque.
const EXIT_NO_DEVICE: u8 = 3;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--check") {
        return match out::check() {
            out::Check::Ready(name) => {
                eprintln!("micro virtuel prêt : {name}");
                ExitCode::SUCCESS
            }
            out::Check::Missing(why) => {
                eprintln!("micro virtuel absent : {why}");
                ExitCode::from(EXIT_NO_DEVICE)
            }
            out::Check::Unsupported(why) => {
                eprintln!("micro impossible ici : {why}");
                ExitCode::FAILURE
            }
        };
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Une réponse à l'app (STATUS du flux montant), sur la sortie standard.
fn answer(json: &str) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{json}");
    let _ = out.flush();
}

fn run() -> Result<(), String> {
    match out::check() {
        out::Check::Ready(_) => {}
        out::Check::Missing(why) => {
            answer(r#"{"state": "failed", "reason": "no_device"}"#);
            return Err(format!("micro virtuel absent : {why}"));
        }
        out::Check::Unsupported(why) => {
            answer(r#"{"state": "failed", "reason": "unsupported"}"#);
            return Err(format!("micro impossible ici : {why}"));
        }
    }
    let jitter = Arc::new(Mutex::new(Jitter::new()));
    let mut player = match out::Player::start(jitter.clone()) {
        Ok(p) => p,
        Err(e) => {
            answer(r#"{"state": "failed", "reason": "no_device"}"#);
            return Err(e);
        }
    };
    eprintln!("micro de l'appareil joué dans « {} »", player.name());

    let mut input = BufReader::new(std::io::stdin().lock());
    let mut decoder: Option<Decoder> = None;
    let mut pcm = Vec::with_capacity(4 * 480);
    let mut defaulted = false;
    let mut last_report = Instant::now();
    loop {
        let record = match stream::read(&mut input) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(e) => {
                eprintln!("entrée illisible : {e}");
                break;
            }
        };
        match record {
            Record::Other => {}
            Record::Start { opus, frame, channels } => {
                if !opus || channels != 1 || frame == 0 {
                    eprintln!("format non pris : Opus {opus}, {frame} échantillons, {channels} voies");
                    decoder = None;
                    continue;
                }
                decoder = match Decoder::new(usize::from(frame)) {
                    Ok(d) => Some(d),
                    Err(code) => {
                        eprintln!("décodeur Opus refusé ({code})");
                        None
                    }
                };
            }
            Record::Media { pts, packet } => {
                let Some(d) = decoder.as_mut() else {
                    continue;
                };
                if !defaulted {
                    // Au premier son, pas avant : un micro proposé mais
                    // coupé ne change rien sur l'ordinateur.
                    defaulted = true;
                    player.make_default();
                }
                pcm.clear();
                d.decode(pts, &packet, &mut pcm);
                jitter.lock().unwrap().push(&pcm);
            }
            Record::Silence => {
                jitter.lock().unwrap().clear();
                if let Some(d) = decoder.as_mut() {
                    d.reset();
                }
            }
        }
        if last_report.elapsed() >= Duration::from_secs(60) {
            last_report = Instant::now();
            let j = jitter.lock().unwrap();
            let (concealed, recovered) = decoder.as_ref().map_or((0, 0), |d| (d.concealed, d.recovered));
            eprintln!(
                "micro : {} ms en attente, {} trames refaites, {} masquées, {} ms jetées, {} ms manquées",
                j.len() * 1000 / RATE as usize,
                recovered,
                concealed,
                j.dropped * 1000 / u64::from(RATE),
                j.starved * 1000 / u64::from(RATE)
            );
        }
    }
    eprintln!("fin du micro de l'appareil");
    player.stop();
    Ok(())
}
