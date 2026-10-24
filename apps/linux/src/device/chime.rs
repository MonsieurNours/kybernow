//! L'allumage de KyberNow Link OS, en son : la pièce esquissée dans la maquette
//! (« Démarrage, animé avec son »), synthétisée ici et jouée par SDL dès le
//! lancement, calée sur les éclairs de `ui/launch.rs` :
//!
//!   0,25 · 0,34 · 0,44 s  trois crépitements du néon (souffle filtré), un coup
//!                         sourd sur le premier ;
//!   0,57 s →              la nappe grave (la, deux octaves) s'ouvre quand le tube
//!                         reste allumé ;
//!   0,8 s                 l'accord de signature avec le nom : la majeur 9, trois
//!                         dents de scie désaccordées par note, filtre qui s'ouvre
//!                         puis se referme ;
//!   1,15 → 1,6 s          le carillon qui monte (mi, la, do#, mi) ;
//!   3,4 s                 la résolution quand l'accueil arrive : la basse descend
//!                         sur ré, une cloche douce au-dessus ; puis le silence.
//!
//! Tout est rendu dans un tampon stéréo à 48 kHz (environ 5 s), avec une
//! réverbération simple (quatre peignes et deux passe-tout, à la Schroeder) et
//! un compresseur doux, puis mis en file sur une sortie SDL à part (le son de
//! la session a la sienne). Une esquisse fidèle à l'intention ; la bande
//! définitive pourra la remplacer par un fichier.

use std::ffi::c_void;

use super::sdl;

const RATE: u32 = 48_000;
const SECONDS: f32 = 5.2;

/// Joue l'allumage ; rend la sortie SDL ouverte (à fermer après).
pub fn play() -> Option<u32> {
    let samples = render();
    let want = sdl::SDL_AudioSpec { freq: RATE as i32, format: sdl::AUDIO_F32SYS, channels: 2, silence: 0, samples: 1024, padding: 0, size: 0, callback: None, userdata: std::ptr::null_mut() };
    let mut have = sdl::SDL_AudioSpec::empty();
    let dev = unsafe { sdl::SDL_OpenAudioDevice(std::ptr::null(), 0, &want, &mut have, 0) };
    if dev == 0 {
        log::warn!("allumage : sortie son indisponible ({})", sdl::error());
        return None;
    }
    let bytes = samples.len() * std::mem::size_of::<f32>();
    unsafe {
        sdl::SDL_QueueAudio(dev, samples.as_ptr() as *const c_void, bytes as u32);
        sdl::SDL_PauseAudioDevice(dev, 0);
    }
    Some(dev)
}

pub fn stop(dev: u32) {
    unsafe { sdl::SDL_CloseAudioDevice(dev) };
}

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Filtre à deux pôles (état-variable), passe-bas ou passe-bande.
struct Svf {
    low: f32,
    band: f32,
}
impl Svf {
    fn new() -> Self {
        Svf { low: 0.0, band: 0.0 }
    }
    fn run(&mut self, x: f32, cutoff: f32, q: f32) -> (f32, f32) {
        let f = (2.0 * std::f32::consts::PI * cutoff / RATE as f32).sin().min(0.99) * 2.0;
        let d = 1.0 / q.max(0.3);
        let high = x - self.low - d * self.band;
        self.band += f * high;
        self.low += f * self.band;
        (self.low, self.band)
    }
}

fn env(t: f32, start: f32, attack: f32, hold_until: f32, release_until: f32) -> f32 {
    if t < start {
        0.0
    } else if t < start + attack {
        ((t - start) / attack).powf(1.6)
    } else if t < hold_until {
        1.0
    } else if t < release_until {
        let p = (t - hold_until) / (release_until - hold_until);
        (1.0 - p).powf(2.2)
    } else {
        0.0
    }
}

fn render() -> Vec<f32> {
    let n = (RATE as f32 * SECONDS) as usize;
    let dt = 1.0 / RATE as f32;
    let mut dry = vec![0.0f32; n];
    let mut wet = vec![0.0f32; n];
    let mut rng = Rng(0x9A8CFF);
    let mut crackle_f = Svf::new();
    let mut pad_f = Svf::new();
    let mut sub_f = Svf::new();
    // Les oscillateurs de la nappe : quatre notes × trois dents de scie désaccordées.
    let chord = [220.0f32, 277.18, 329.63, 493.88];
    let detune = [-8.0f32, 0.0, 8.0];
    let mut pad_phase = [[0.0f32; 3]; 4];
    let (mut sub1, mut sub2) = (0.0f32, 0.0f32);
    let arps = [(1.15f32, 659.25f32), (1.3, 880.0), (1.45, 1108.73), (1.6, 1318.51)];
    let mut arp_phase = [0.0f32; 4];
    let (mut thump_phase, mut bass_phase, mut bell1, mut bell2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let crackles = [0.25f32, 0.34, 0.44];
    let resolve = 3.4f32;
    for i in 0..n {
        let t = i as f32 * dt;
        let mut d = 0.0f32;
        let mut send = 0.0f32;
        // 1 · Crépitements : souffle passe-bande, 70 ms, sur chaque éclair.
        let mut noise = 0.0;
        for (k, c) in crackles.iter().enumerate() {
            if t >= *c && t < c + 0.08 {
                let p = (t - c) / 0.08;
                noise += rng.next() * (1.0 - p) * (1.0 - p);
                let (_, band) = crackle_f.run(noise, 2400.0 + k as f32 * 600.0, 1.4);
                let g = 0.28 * (1.0 - p * 0.6);
                d += band * g;
                send += band * g * 0.35;
            }
        }
        // Le coup sourd, 90 → 38 Hz, sur le premier éclair.
        if t >= crackles[0] && t < crackles[0] + 0.4 {
            let p = (t - crackles[0]) / 0.3;
            let f = 90.0 * (38.0f32 / 90.0).powf(p.min(1.0));
            thump_phase += f * dt;
            let a = 0.5 * (-(t - crackles[0]) * 9.0).exp();
            d += (thump_phase * std::f32::consts::TAU).sin() * a;
        }
        // 2 · La nappe grave : 55 et 110 Hz, filtrée, de 0,57 s à l'arrivée de l'accueil.
        let sub_env = env(t, 0.57, 0.5, resolve - 0.1, resolve + 1.0);
        if sub_env > 0.0 {
            sub1 += 55.0 * dt;
            sub2 += 110.0 * dt;
            let tri = 4.0 * ((sub2 % 1.0) - 0.5).abs() - 1.0;
            let s = (sub1 * std::f32::consts::TAU).sin() + 0.6 * tri;
            let (low, _) = sub_f.run(s, 220.0, 0.7);
            d += low * 0.16 * sub_env;
        }
        // 3 · L'accord de signature, avec le nom : attaque douce, filtre qui s'ouvre puis se referme.
        let pad_env = env(t, 0.8, 0.45, resolve - 0.2, resolve + 1.2);
        if pad_env > 0.0 {
            let mut s = 0.0;
            for (ni, f0) in chord.iter().enumerate() {
                for (di, c) in detune.iter().enumerate() {
                    let f = f0 * 2.0f32.powf(c / 1200.0);
                    pad_phase[ni][di] = (pad_phase[ni][di] + f * dt) % 1.0;
                    s += (pad_phase[ni][di] * 2.0 - 1.0) * 0.33;
                }
            }
            let since = t - 0.8;
            let cutoff = if since < 0.9 { 500.0 * (3200.0f32 / 500.0).powf(since / 0.9) } else { 3200.0 * (900.0f32 / 3200.0).powf(((since - 0.9) / 2.3).min(1.0)) };
            let (low, _) = pad_f.run(s / chord.len() as f32, cutoff, 0.7);
            d += low * 0.07 * pad_env;
            send += low * 0.07 * pad_env * 0.9;
        }
        // 4 · Le carillon qui monte : des triangles brefs, très réverbérés.
        for (k, (at, f)) in arps.iter().enumerate() {
            if t >= *at && t < at + 1.0 {
                arp_phase[k] = (arp_phase[k] + f * dt) % 1.0;
                let tri = 4.0 * (arp_phase[k] - 0.5).abs() - 1.0;
                let p = t - at;
                let a = 0.09 * (1.0 - (p / 0.02).min(1.0) * 0.0) * (-(p) * 5.5).exp() * (p / 0.02).min(1.0);
                d += tri * a;
                send += tri * a * 1.1;
            }
        }
        // 5 · La résolution : la basse descend sur ré, une cloche douce au-dessus.
        if t >= resolve && t < resolve + 1.8 {
            let p = t - resolve;
            bass_phase += 73.42 * dt;
            let a = 0.14 * (p / 0.2).min(1.0) * (-(p) * 2.4).exp();
            d += (bass_phase * std::f32::consts::TAU).sin() * a;
            bell1 += 1760.0 * dt;
            bell2 += 2637.02 * dt;
            let b = (p / 0.03).min(1.0) * (-(p) * 3.0).exp();
            let bell = (bell1 * std::f32::consts::TAU).sin() * 0.05 + (bell2 * std::f32::consts::TAU).sin() * 0.025;
            d += bell * b;
            send += bell * b * 1.2;
        }
        dry[i] = d;
        wet[i] = send;
    }
    // La réverbération : quatre peignes, deux passe-tout, environ 2,8 s de queue.
    let combs = [1557usize, 1617, 1491, 1422];
    let mut comb_buf: Vec<Vec<f32>> = combs.iter().map(|c| vec![0.0; *c]).collect();
    let mut comb_lp = [0.0f32; 4];
    let allpasses = [225usize, 556];
    let mut ap_buf: Vec<Vec<f32>> = allpasses.iter().map(|c| vec![0.0; *c]).collect();
    let mut rev = vec![0.0f32; n];
    for i in 0..n {
        let x = wet[i];
        let mut acc = 0.0;
        for (k, len) in combs.iter().enumerate() {
            let idx = i % len;
            let y = comb_buf[k][idx];
            comb_lp[k] = y * 0.3 + comb_lp[k] * 0.7;
            comb_buf[k][idx] = x + comb_lp[k] * 0.84;
            acc += y;
        }
        let mut y = acc * 0.25;
        for (k, len) in allpasses.iter().enumerate() {
            let idx = i % len;
            let b = ap_buf[k][idx];
            let out = -y + b;
            ap_buf[k][idx] = y + b * 0.5;
            y = out;
        }
        rev[i] = y * 0.5;
    }
    // Mixage, compression douce, stéréo (la réverbération un peu décalée à droite).
    let mut out = Vec::with_capacity(n * 2);
    let mut gain = 1.0f32;
    for i in 0..n {
        let r_l = rev[i];
        let r_r = rev[i.saturating_sub(23)];
        let l = dry[i] + r_l;
        let r = dry[i] + r_r;
        let peak = l.abs().max(r.abs());
        let target = if peak > 0.5 { 0.5 / peak } else { 1.0 };
        gain = if target < gain { target } else { gain + (target - gain) * 0.0005 };
        out.push((l * gain * 0.9).clamp(-1.0, 1.0));
        out.push((r * gain * 0.9).clamp(-1.0, 1.0));
    }
    out
}
