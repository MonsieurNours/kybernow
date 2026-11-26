//! Profil Auto (docs/profil-auto.md), étape « Décodage de cet appareil » : les
//! extraits 4K du cœur (`autotest::samples_for`) passés au vrai décodeur de
//! l'app, celui des sessions — FFmpeg sur l'appareil Direct3D 11 de la carte
//! (D3D11VA, `decoder.rs`) pour HEVC, H.264 et AV1, le chemin Vulkan de
//! PyroWave (`pyrowave.rs`) — aussi vite qu'il les prend, en boucle depuis
//! l'image clé, pendant 1 s au plus : images décodées par seconde.
//!
//! Comme la sonde du lancement (`pc.rs`), le décodage tourne dans un
//! processus à part (`kybernow --decode-samples`) : un pilote qui plante
//! n'emporte pas l'app, l'extrait est seulement compté comme non décodé. Le
//! processus vit le temps du test : l'app lui demande un extrait par ligne
//! (`hevc 0`, `pyrowave 1` : format et 4:4:4), il répond par une ligne (les
//! images par seconde, ou `none`).

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use kybernow_core::autotest::{Sample, SampleDecoder, samples};
use kybernow_core::profile::Codec;
use windows::Win32::Graphics::Direct3D11::{D3D11_QUERY_DESC, D3D11_QUERY_EVENT, ID3D11Device, ID3D11Query};

use crate::decoder::{Decoder, ffmpeg_quiet};
use crate::{pyrowave, render};

/// Décodage d'un extrait : 1 s au plus.
const BUDGET: Duration = Duration::from_secs(1);
/// Attente au plus de la réponse du processus pour un extrait (création du
/// décodeur, de l'appareil Vulkan de PyroWave la première fois).
const ANSWER_WITHIN: Duration = Duration::from_secs(12);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// --- Côté processus de décodage ---

/// `kybernow --decode-samples` : un appareil Direct3D 11 pour tout le test,
/// puis un extrait par ligne lue, jusqu'à la fin de l'entrée.
pub fn serve() -> i32 {
    ffmpeg_quiet();
    let device = match render::create_device() {
        Ok((device, _)) => Some(device),
        Err(e) => {
            log::warn!("décodage du test : Direct3D 11 indisponible ({e})");
            None
        }
    };
    let all = samples();
    let mut pyro: Option<std::sync::Arc<pyrowave::Device>> = None;
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let mut parts = line.split_whitespace();
        let (Some(codec), Some(yuv)) = (parts.next().and_then(Codec::parse), parts.next()) else { continue };
        let yuv444 = yuv == "1";
        let fps = match (all.iter().find(|s| s.codec == codec && s.yuv444 == yuv444), device.as_ref()) {
            (Some(s), Some(d)) => {
                let started = Instant::now();
                let fps = if codec == Codec::PyroWave { bench_pyrowave(s, d, &mut pyro) } else { bench_ffmpeg(s, d) };
                log::info!(
                    "décodage du test : {}{} {} × {} : {} ({} ms)",
                    codec.label(),
                    if yuv444 { " 4:4:4" } else { "" },
                    s.width,
                    s.height,
                    fps.map_or("pas décodé".to_string(), |f| format!("{f:.0} i/s")),
                    started.elapsed().as_millis()
                );
                fps
            }
            _ => None,
        };
        let answer = fps.map_or("none".to_string(), |f| format!("{f:.2}"));
        if writeln!(out, "{answer}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
    0
}

/// Attend que la carte ait fini ce qui lui a été donné (requête d'événement),
/// pour que le temps compte le travail du GPU, pas seulement sa soumission.
fn gpu_idle(device: &ID3D11Device) {
    unsafe {
        let desc = D3D11_QUERY_DESC { Query: D3D11_QUERY_EVENT, MiscFlags: 0 };
        let mut query: Option<ID3D11Query> = None;
        if device.CreateQuery(&desc, Some(&mut query)).is_err() {
            return;
        }
        let (Some(query), Ok(ctx)) = (query, device.GetImmediateContext()) else { return };
        ctx.End(&query);
        let started = Instant::now();
        let mut done = 0i32;
        while started.elapsed() < Duration::from_millis(500) {
            if ctx.GetData(&query, Some(&mut done as *mut i32 as *mut _), 4, 0).is_ok() && done != 0 {
                return;
            }
            std::thread::yield_now();
        }
    }
}

/// HEVC, H.264, AV1 : le décodeur FFmpeg des sessions, sur la carte quand
/// elle sait (sinon par le processeur, comme en session).
fn bench_ffmpeg(sample: &Sample, device: &ID3D11Device) -> Option<f64> {
    if sample.packets.is_empty() {
        return None;
    }
    let mut dec = Decoder::new(sample.codec.fourcc(), Some(device), None).ok()?;
    let mut frames = 0u32;
    let mut errors = 0u32;
    let mut started: Option<Instant> = None;
    let deadline = Instant::now() + BUDGET * 3;
    let mut i = 0usize;
    // Le chrono part à la première image (création du décodeur et de ses
    // surfaces mise à part) et s'arrête au bout d'une seconde.
    while started.is_none_or(|s| s.elapsed() < BUDGET) && Instant::now() < deadline {
        let p = &sample.packets[i % sample.packets.len()];
        i += 1;
        if dec.send(&p.data, i as u64, p.is_key).is_err() {
            errors += 1;
            if errors > 8 {
                return None;
            }
        }
        while let Some(r) = dec.receive() {
            match r {
                Ok(_) => match started {
                    None => started = Some(Instant::now()),
                    Some(_) => frames += 1,
                },
                Err(_) => {
                    errors += 1;
                    if errors > 8 {
                        return None;
                    }
                }
            }
        }
    }
    if dec.hardware {
        gpu_idle(device);
    }
    let elapsed = started?.elapsed().as_secs_f64();
    (frames > 0 && elapsed > 0.0).then(|| f64::from(frames) / elapsed)
}

/// PyroWave : le décodeur Vulkan des sessions, qui écrit dans les textures
/// Direct3D 11 partagées et attend la fin du GPU à chaque image. Le device
/// PyroWave est créé une fois (4:2:0 puis 4:4:4).
fn bench_pyrowave(sample: &Sample, device: &ID3D11Device, kept: &mut Option<std::sync::Arc<pyrowave::Device>>) -> Option<f64> {
    if sample.packets.is_empty() {
        return None;
    }
    let pdev = match kept {
        Some(d) => d.clone(),
        None => match pyrowave::Device::new(device) {
            Ok(d) => {
                *kept = Some(d.clone());
                d
            }
            Err(e) => {
                log::info!("décodage du test : PyroWave indisponible ({e})");
                return None;
            }
        },
    };
    let mut dec = match pyrowave::Decoder::new(pdev, device) {
        Ok(d) => d,
        Err(e) => {
            log::info!("décodage du test : décodeur PyroWave ({e})");
            return None;
        }
    };
    let mut frames = 0u32;
    let mut skipped = 0u32;
    let mut started: Option<Instant> = None;
    let deadline = Instant::now() + BUDGET * 3;
    let mut i = 0usize;
    while started.is_none_or(|s| s.elapsed() < BUDGET) && Instant::now() < deadline {
        let p = &sample.packets[i % sample.packets.len()];
        i += 1;
        match dec.decode(&p.data) {
            Ok(pyrowave::Outcome::Ready(_)) => match started {
                None => started = Some(Instant::now()),
                Some(_) => frames += 1,
            },
            Ok(pyrowave::Outcome::Skipped(_)) => {
                skipped += 1;
                if skipped > 8 && started.is_none() {
                    return None;
                }
            }
            Err(e) => {
                log::info!("décodage du test : PyroWave ({e})");
                return None;
            }
        }
    }
    let elapsed = started?.elapsed().as_secs_f64();
    (frames > 0 && elapsed > 0.0).then(|| f64::from(frames) / elapsed)
}

// --- Côté app ---

/// Le décodeur du test, côté app : le processus de décodage, lancé à la
/// première demande et arrêté à la fin du test. Un processus qui s'arrête ou
/// ne répond pas : l'extrait (et les suivants) comptent comme non décodés.
#[derive(Default)]
pub struct ChildDecoder {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    answers: Option<Receiver<String>>,
    broken: bool,
}

impl ChildDecoder {
    fn start(&mut self) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = Command::new(exe)
            .arg("--decode-samples")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("lancement : {e}"))?;
        let stdout = child.stdout.take().ok_or("sortie absente")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        self.stdin = child.stdin.take();
        self.child = Some(child);
        self.answers = Some(rx);
        Ok(())
    }

    fn ask(&mut self, sample: &Sample) -> Result<Option<f64>, String> {
        if self.child.is_none() {
            self.start()?;
        }
        let stdin = self.stdin.as_mut().ok_or("entrée fermée")?;
        writeln!(stdin, "{} {}", sample.codec.key(), u8::from(sample.yuv444)).and_then(|_| stdin.flush()).map_err(|e| format!("demande : {e}"))?;
        let rx = self.answers.as_ref().ok_or("sortie fermée")?;
        match rx.recv_timeout(ANSWER_WITHIN) {
            Ok(line) => Ok(line.trim().parse::<f64>().ok().filter(|f| f.is_finite() && *f > 0.0)),
            Err(RecvTimeoutError::Timeout) => Err(format!("pas de réponse en {} s", ANSWER_WITHIN.as_secs())),
            Err(RecvTimeoutError::Disconnected) => {
                let code = self.child.as_mut().and_then(|c| c.try_wait().ok().flatten()).and_then(|s| s.code());
                Err(format!("processus arrêté (code {}), voir kybernow-autotest.log", code.map_or("?".into(), |c| format!("{:#010x}", c as u32))))
            }
        }
    }

    fn stop(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(2) {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl SampleDecoder for ChildDecoder {
    fn decode(&mut self, sample: &Sample) -> Option<f64> {
        if self.broken {
            return None;
        }
        match self.ask(sample) {
            Ok(fps) => fps,
            Err(e) => {
                log::warn!("profil Auto : décodage de {} : {e}", sample.codec.label());
                self.broken = true;
                self.stop();
                None
            }
        }
    }
}

impl Drop for ChildDecoder {
    fn drop(&mut self) {
        self.stop();
    }
}
