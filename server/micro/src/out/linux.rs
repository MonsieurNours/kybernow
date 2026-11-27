//! Linux : une source PipeWire « Micro KyberNow », par pipewire-pulse.
//!
//! Au besoin, `pactl` crée une sortie nulle (`kybernow_micro_entree`) et une
//! source qui reprend son retour (`kybernow_micro`, « Micro KyberNow »), une
//! fois pour la session de l'utilisateur (elles restent jusqu'à sa
//! déconnexion, pour les sessions suivantes et les autres aides). Le son est
//! joué dans la sortie nulle par `pacat` ; son entrée bloque quand le tampon
//! du serveur (20 ms) est plein, ce qui cale la lecture sur l'horloge du son.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::Check;
use crate::jitter::Jitter;

const SINK: &str = "kybernow_micro_entree";
const SOURCE: &str = "kybernow_micro";
const NAME: &str = "Micro KyberNow";

fn pactl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("pactl")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("pactl : {e}"))?;
    if !out.status.success() {
        return Err(format!("pactl {} : {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn check() -> Check {
    match pactl(&["info"]) {
        Ok(_) => Check::Ready(NAME.into()),
        Err(e) => Check::Missing(e),
    }
}

/// Crée la sortie nulle et la source si elles manquent.
fn ensure() -> Result<(), String> {
    let sinks = pactl(&["list", "short", "sinks"])?;
    if !sinks.lines().any(|l| l.split('\t').nth(1) == Some(SINK)) {
        pactl(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={SINK}"),
            "sink_properties=device.description=\"KyberNow Micro (entrée)\"",
            "rate=48000",
            "channels=1",
            "channel_map=mono",
        ])?;
    }
    let sources = pactl(&["list", "short", "sources"])?;
    if !sources.lines().any(|l| l.split('\t').nth(1) == Some(SOURCE)) {
        pactl(&[
            "load-module",
            "module-remap-source",
            &format!("master={SINK}.monitor"),
            &format!("source_name={SOURCE}"),
            &format!("source_properties=device.description=\"{NAME}\""),
        ])?;
    }
    Ok(())
}

pub struct Player {
    child: Child,
    stop: Arc<AtomicBool>,
    feeder: Option<JoinHandle<()>>,
    /// Le micro par défaut d'avant, quand c'est nous qui l'avons changé.
    previous: Option<String>,
}

impl Player {
    pub fn start(jitter: Arc<Mutex<Jitter>>) -> Result<Self, String> {
        ensure()?;
        let mut child = Command::new("pacat")
            .args([
                "--playback",
                "--raw",
                &format!("--device={SINK}"),
                "--format=float32le",
                "--rate=48000",
                "--channels=1",
                "--latency-msec=20",
                "--client-name=KyberNow Micro",
                "--stream-name=Micro de l'appareil",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("pacat : {e}"))?;
        let mut stdin = child.stdin.take().ok_or("pacat sans entrée")?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let feeder = std::thread::Builder::new()
            .name("micro-pacat".into())
            .spawn(move || {
                let mut frame = [0f32; 480];
                let mut bytes = Vec::with_capacity(480 * 4);
                while !flag.load(Ordering::Relaxed) {
                    super::pull(&jitter, &mut frame);
                    bytes.clear();
                    for s in frame {
                        bytes.extend_from_slice(&s.to_le_bytes());
                    }
                    if stdin.write_all(&bytes).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            child,
            stop,
            feeder: Some(feeder),
            previous: None,
        })
    }

    pub fn name(&self) -> &str {
        NAME
    }

    pub fn make_default(&mut self) {
        let current = pactl(&["get-default-source"]).map(|s| s.trim().to_owned());
        match current {
            Ok(current) if current == SOURCE => {}
            Ok(current) => match pactl(&["set-default-source", SOURCE]) {
                Ok(_) => {
                    eprintln!("micro par défaut : {NAME} (avant : {current})");
                    self.previous = Some(current);
                }
                Err(e) => eprintln!("micro par défaut inchangé : {e}"),
            },
            Err(e) => eprintln!("micro par défaut inconnu : {e}"),
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(f) = self.feeder.take() {
            let _ = f.join();
        }
        if let Some(previous) = self.previous.take()
            && pactl(&["get-default-source"]).is_ok_and(|s| s.trim() == SOURCE)
        {
            match pactl(&["set-default-source", &previous]) {
                Ok(_) => eprintln!("micro par défaut rendu : {previous}"),
                Err(e) => eprintln!("micro par défaut pas rendu : {e}"),
            }
        }
    }
}
