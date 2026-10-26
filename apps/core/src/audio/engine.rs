//! Le moteur : paquets de Kyber décodés dès qu'ils arrivent (fil réseau),
//! tampon adaptatif tiré par la sortie (fil temps réel), voir le module.
//!
//! Fils : `configure`, `decode`, `conceal` (et `PacketHandler::packet`) sont
//! appelés depuis le fil réseau de Kyber, un à la fois ; `fill` depuis le
//! fil temps réel de la sortie, un seul à la fois. Entre les deux, un anneau
//! sans verrou et des atomiques ; `fill` ne prend que des essais de verrou
//! (le décodeur pour une trame de masquage, les mesures) et écrit du silence
//! s'ils échouent.
//!
//! Son coupé ([`AudioEngine::set_muted`], plusieurs ordinateurs à la fois) :
//! le son continue d'être décodé et tiré au même rythme, seul ce qui sort
//! est atténué, en 20 ms pour ne pas claquer ; il revient sans décalage.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::mix::{OutputOrder, mix_frame};
use super::opus::Decoder;
use super::ring::Ring;
use super::{Clock, MAX_FRAMES, SAMPLE_RATE};
use crate::media::{OpusHead, Packet, PacketHandler};

/// Mesures du son, une fois par seconde.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioStats {
    pub channels: usize,
    /// Tampon moyen avant la carte son, en millisecondes.
    pub buffer_ms: f64,
    /// Latence de la sortie (carte son, Bluetooth…), en millisecondes, telle
    /// que la plateforme l'a donnée.
    pub output_ms: f64,
    /// Trous comblés (pertes réseau ou tampon vide) dans la seconde.
    pub gaps: usize,
    /// Voies envoyées à la sortie (moins que le flux : mixage descendant).
    pub output_channels: usize,
}

/// Un échantillon sauté ou doublé toutes les `STRETCH_PERIOD` images audio
/// (0,4 %, inaudible) ; deux fois plus souvent quand l'écart dépasse 6 ms.
const STRETCH_PERIOD: usize = 256;
const STRETCH_PERIOD_FAST: usize = 128;

/// Fondu du son coupé ou rendu, en images audio (20 ms).
pub const MUTE_FADE_FRAMES: usize = SAMPLE_RATE / 50;

const fn ms(n: usize) -> usize {
    SAMPLE_RATE * n / 1000
}

/// Le flux en cours : son anneau (une seconde) et ses voies. Remplacé à
/// chaque configuration ; le consommateur s'en aperçoit par la génération.
struct Stream {
    ring: Ring,
    channels: usize,
}

struct Producer {
    decoder: Option<Decoder>,
    pcm: Vec<f32>,
    stream: Option<Arc<Stream>>,
}

/// État du fil de la sortie.
struct Consumer {
    generation: u64,
    stream: Option<Arc<Stream>>,
    output_channels: usize,
    order: Vec<usize>,
    /// Marge visée au plus bas du tampon, en images audio.
    margin: usize,
    playing: bool,
    /// Jamais lu : le premier départ attend la marge plus un paquet ; une
    /// reprise après un trou n'attend que la marge.
    started: bool,
    lowest: usize,
    window_start: Option<Duration>,
    fill_sum: f64,
    fill_count: usize,
    gaps_at_window_start: usize,
    /// Tampons vides dans la fenêtre (pas de rattrapage de dérive dans ce cas).
    underruns: usize,
    calm_since: Duration,
    /// Dérive à rattraper, en images audio : > 0 à sauter, < 0 à doubler.
    stretch: i64,
    stretch_counter: usize,
    stretch_period: usize,
    /// Trame de masquage en cours de lecture.
    plc: Vec<f32>,
    plc_read: usize,
    plc_count: usize,
    frame: Vec<f32>,
    mixed: Vec<f32>,
    /// Gain du son coupé ou rendu (0 : coupé, 1 : plein), suivi image par image.
    gain: f32,
}

impl Consumer {
    fn reset(&mut self, stream: Option<Arc<Stream>>) {
        let channels = stream.as_ref().map_or(0, |s| s.channels);
        self.stream = stream;
        self.margin = ms(8);
        self.playing = false;
        self.started = false;
        self.lowest = usize::MAX;
        self.window_start = None;
        self.fill_sum = 0.0;
        self.fill_count = 0;
        self.underruns = 0;
        self.stretch = 0;
        self.stretch_counter = 0;
        self.stretch_period = STRETCH_PERIOD;
        self.plc = vec![0.0; MAX_FRAMES * channels];
        self.plc_read = 0;
        self.plc_count = 0;
        self.frame = vec![0.0; channels];
    }
}

pub struct AudioEngine {
    clock: Arc<dyn Clock>,
    producer: Mutex<Producer>,
    consumer: Mutex<Consumer>,
    /// Flux en cours, pour le consommateur quand la génération change.
    current: Mutex<Option<Arc<Stream>>>,
    generation: AtomicU64,
    /// Taille du dernier paquet, en images audio (10 ou 20 ms selon le serveur).
    packet_frames: AtomicUsize,
    volume: AtomicU32,
    /// Trous comblés depuis le début (pertes masquées, tampons vides).
    gaps: AtomicUsize,
    output_latency_ms: AtomicU64,
    order: Mutex<OutputOrder>,
    stats: Mutex<Option<AudioStats>>,
    /// Son coupé (session pas active, plusieurs ordinateurs).
    muted: AtomicBool,
}

impl AudioEngine {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self::with_order(clock, OutputOrder::Opus)
    }

    /// `order` : l'ordre des voies qu'attend la sortie.
    pub fn with_order(clock: Arc<dyn Clock>, order: OutputOrder) -> Self {
        Self {
            clock,
            producer: Mutex::new(Producer {
                decoder: None,
                pcm: Vec::new(),
                stream: None,
            }),
            consumer: Mutex::new(Consumer {
                generation: 0,
                stream: None,
                output_channels: 0,
                order: Vec::new(),
                margin: ms(8),
                playing: false,
                started: false,
                lowest: usize::MAX,
                window_start: None,
                fill_sum: 0.0,
                fill_count: 0,
                gaps_at_window_start: 0,
                underruns: 0,
                calm_since: Duration::ZERO,
                stretch: 0,
                stretch_counter: 0,
                stretch_period: STRETCH_PERIOD,
                plc: Vec::new(),
                plc_read: 0,
                plc_count: 0,
                frame: Vec::new(),
                mixed: vec![0.0; 8],
                gain: 1.0,
            }),
            current: Mutex::new(None),
            generation: AtomicU64::new(0),
            packet_frames: AtomicUsize::new(SAMPLE_RATE / 50),
            volume: AtomicU32::new(1f32.to_bits()),
            gaps: AtomicUsize::new(0),
            output_latency_ms: AtomicU64::new(0f64.to_bits()),
            order: Mutex::new(order),
            stats: Mutex::new(None),
            muted: AtomicBool::new(false),
        }
    }

    /// Son coupé (`true`) ou rendu, avec un fondu de 20 ms. Le son continue
    /// d'être décodé et tiré : il revient sans décalage. Sans flux en cours,
    /// le changement est immédiat. Indépendant du volume.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    /// Latence de la sortie, donnée par la plateforme (tampon de la carte son
    /// plus ce qu'elle annonce, Bluetooth compris), pour les mesures.
    pub fn set_output_latency_ms(&self, latency: f64) {
        self.output_latency_ms.store(latency.to_bits(), Ordering::Relaxed);
    }

    pub fn output_latency_ms(&self) -> f64 {
        f64::from_bits(self.output_latency_ms.load(Ordering::Relaxed))
    }

    /// L'ordre des voies qu'attend la sortie (une sortie changée en cours de session).
    pub fn set_order(&self, order: OutputOrder) {
        *self.order.lock().unwrap() = order;
        if let Ok(mut c) = self.consumer.try_lock() {
            c.output_channels = 0;
        }
    }

    /// Voies du flux en cours (0 : pas de flux).
    pub fn channels(&self) -> usize {
        self.current.lock().unwrap().as_ref().map_or(0, |s| s.channels)
    }

    /// Les dernières mesures, une fois par seconde (puis `None` jusqu'aux suivantes).
    pub fn take_stats(&self) -> Option<AudioStats> {
        self.stats.lock().unwrap().take()
    }

    // Paquets (fil réseau).

    /// En-tête `OpusHead` du flux : ouvre le décodeur et un tampon neuf. Un
    /// en-tête au même nombre de voies que le flux en cours ne change rien.
    pub fn configure(&self, opus_head: &[u8]) {
        let Some(head) = OpusHead::parse(opus_head) else {
            log::warn!("son : en-tête Opus illisible");
            return;
        };
        let mut p = self.producer.lock().unwrap();
        if p.decoder.as_ref().is_some_and(|d| d.channels() == usize::from(head.channels)) {
            return;
        }
        self.open(&mut p, &head);
    }

    fn open(&self, p: &mut Producer, head: &OpusHead) {
        let channels = usize::from(head.channels);
        match Decoder::new(head) {
            Ok(decoder) => p.decoder = Some(decoder),
            Err(err) => {
                log::error!("décodeur Opus : erreur {err}");
                p.decoder = None;
                return;
            }
        }
        p.pcm = vec![0.0; MAX_FRAMES * channels];
        let stream = Arc::new(Stream {
            ring: Ring::new(SAMPLE_RATE * channels),
            channels,
        });
        p.stream = Some(stream.clone());
        *self.current.lock().unwrap() = Some(stream);
        self.packet_frames.store(SAMPLE_RATE / 50, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);
        log::info!("son : Opus, {channels} voies, {} flux", head.streams);
    }

    /// Un paquet Opus.
    pub fn decode(&self, data: &[u8]) {
        let mut p = self.producer.lock().unwrap();
        if p.decoder.is_none() {
            // Paquet de configuration perdu (transport rapide) : stéréo d'office.
            log::warn!("son : paquet avant l'en-tête, stéréo supposée");
            self.open(&mut p, &OpusHead::stereo());
        }
        let Producer { decoder, pcm, stream } = &mut *p;
        let (Some(decoder), Some(stream)) = (decoder.as_mut(), stream.as_ref()) else { return };
        let frames = decoder.decode(Some(data), MAX_FRAMES, pcm);
        if frames > 0 {
            Self::push(stream, &pcm[..frames * stream.channels]);
            self.packet_frames.store(frames, Ordering::Relaxed);
        }
    }

    /// Données perdues : Opus invente un raccord plausible au lieu d'un silence sec.
    pub fn conceal(&self, samples: usize) {
        let mut p = self.producer.lock().unwrap();
        let Producer { decoder, pcm, stream } = &mut *p;
        let (Some(decoder), Some(stream)) = (decoder.as_mut(), stream.as_ref()) else { return };
        if samples == 0 {
            return;
        }
        // Par tranches de 20 ms, multiples de 2,5 ms comme l'exige Opus.
        let mut left = samples.min(SAMPLE_RATE / 5);
        while left >= 120 {
            let chunk = (left - left % 120).min(960);
            let frames = decoder.decode(None, chunk, pcm);
            if frames == 0 {
                break;
            }
            Self::push(stream, &pcm[..frames * stream.channels]);
            left -= frames;
        }
        self.gaps.fetch_add(1, Ordering::Relaxed);
    }

    fn push(stream: &Stream, samples: &[f32]) {
        let written = stream.ring.push(samples);
        if written < samples.len() {
            // Tampon plein (sortie arrêtée ?) : le trop-plein est jeté ; la
            // coupe sèche remettra la lecture à la marge au redémarrage.
            log::debug!("son : tampon plein, {} échantillons jetés", samples.len() - written);
        }
    }

    /// Fin de session : décodeur fermé, tampon rendu.
    pub fn stop(&self) {
        let mut p = self.producer.lock().unwrap();
        p.decoder = None;
        p.stream = None;
        *self.current.lock().unwrap() = None;
        self.generation.fetch_add(1, Ordering::Release);
    }

    // Sortie (fil temps réel).

    /// Remplit `out` de `frames` images audio entrelacées, `out.len() /
    /// frames` voies (celles de la sortie) : copie le tampon (mixé vers le bas
    /// si la sortie a moins de voies, voies dans l'ordre demandé), rattrape la
    /// dérive, règle la marge, et comble un tampon vide avec une trame de
    /// masquage d'Opus. Silence tant qu'il n'y a pas de flux.
    pub fn fill(&self, out: &mut [f32], frames: usize) {
        let Ok(mut c) = self.consumer.try_lock() else {
            out.fill(0.0);
            return;
        };
        if frames == 0 {
            return;
        }
        let out_ch = out.len() / frames;
        let generation = self.generation.load(Ordering::Acquire);
        if c.generation != generation {
            let stream = self.current.lock().unwrap().clone();
            c.generation = generation;
            c.reset(stream);
        }
        let Some(stream) = c.stream.clone() else {
            out.fill(0.0);
            // Rien ne joue : le son coupé ou rendu s'applique tout de suite.
            c.gain = if self.is_muted() { 0.0 } else { 1.0 };
            return;
        };
        let ch = stream.channels;
        if c.output_channels != out_ch {
            c.output_channels = out_ch;
            c.order = self.order.lock().map(|o| o.permutation(out_ch)).unwrap_or_else(|_| (0..out_ch).collect());
            c.mixed = vec![0.0; out_ch];
        }
        let volume = self.volume();
        let packet_frames = self.packet_frames.load(Ordering::Relaxed);
        let now = self.clock.now();
        let c = &mut *c;

        if !c.playing {
            // Premier départ : la marge plus un paquet. Reprise après un
            // trou : dès que la marge est là (le trou est comblé par Opus).
            let wanted = if c.started { c.margin } else { c.margin + packet_frames };
            if stream.ring.len() >= wanted * ch {
                c.playing = true;
                c.started = true;
            }
        }
        let mut copied = 0;
        if c.playing {
            let mut count = stream.ring.len();
            let n = frames.min(count / ch);
            while copied < n {
                if !stream.ring.peek(&mut c.frame) {
                    break;
                }
                mix_frame(&c.frame, &mut c.mixed, volume);
                let dst = &mut out[copied * out_ch..(copied + 1) * out_ch];
                for (i, &src) in c.order.iter().enumerate() {
                    dst[i] = c.mixed[src];
                }
                copied += 1;
                c.stretch_counter += 1;
                if c.stretch != 0 && c.stretch_counter >= c.stretch_period {
                    c.stretch_counter = 0;
                    if c.stretch > 0 && count >= 3 * ch {
                        // En avance : une image audio sautée (la lecture avance de deux).
                        stream.ring.advance(2 * ch);
                        count -= 2 * ch;
                        c.stretch -= 1;
                        continue;
                    } else if c.stretch < 0 {
                        // En retard : une image audio doublée (la lecture n'avance pas).
                        c.stretch += 1;
                        continue;
                    }
                }
                stream.ring.advance(ch);
                count -= ch;
            }
            if copied < frames {
                // Tampon vide : masquage d'Opus le temps que la marge revienne, marge plus large.
                c.playing = false;
                self.gaps.fetch_add(1, Ordering::Relaxed);
                c.underruns += 1;
                c.margin = (c.margin + ms(5)).min(ms(60));
                c.calm_since = now;
                c.stretch = 0;
            }
        }

        // Plus bas niveau du tampon sur la fenêtre : ce qui dépasse la marge est du retard inutile.
        let level = stream.ring.len() / ch;
        c.lowest = c.lowest.min(level);
        c.fill_sum += level as f64;
        c.fill_count += 1;
        let window_start = *c.window_start.get_or_insert_with(|| {
            c.calm_since = now;
            c.gaps_at_window_start = self.gaps.load(Ordering::Relaxed);
            now
        });
        if now.saturating_sub(window_start) >= Duration::from_secs(1) {
            if c.playing && c.underruns == 0 && c.lowest != usize::MAX {
                let excess = c.lowest as i64 - c.margin as i64;
                if excess > ms(10) as i64 {
                    // Loin devant (démarrage, sortie restée arrêtée) : coupe sèche, une fois.
                    // À 1 sur 256, rattraper 20 ms prendrait 5 s.
                    stream.ring.advance(excess as usize * ch);
                    c.stretch = 0;
                } else if excess > ms(2) as i64 {
                    c.stretch = excess; // échantillons à sauter (dérive : 3 ms par minute au plus)
                    c.stretch_period = if excess > ms(6) as i64 { STRETCH_PERIOD_FAST } else { STRETCH_PERIOD };
                } else if excess < -(ms(2) as i64) {
                    c.stretch = excess; // échantillons à doubler
                    c.stretch_period = STRETCH_PERIOD;
                }
            }
            // Deux secondes sans trou : la marge redescend d'une milliseconde.
            if now.saturating_sub(c.calm_since) >= Duration::from_secs(2) {
                c.margin = c.margin.saturating_sub(ms(1)).max(ms(4));
                c.calm_since = now;
            }
            let gaps_now = self.gaps.load(Ordering::Relaxed);
            let report = AudioStats {
                channels: ch,
                buffer_ms: if c.fill_count > 0 { c.fill_sum / c.fill_count as f64 * 1000.0 / SAMPLE_RATE as f64 } else { 0.0 },
                output_ms: self.output_latency_ms(),
                gaps: gaps_now - c.gaps_at_window_start,
                output_channels: out_ch,
            };
            if let Ok(mut stats) = self.stats.try_lock() {
                *stats = Some(report);
            }
            c.window_start = Some(now);
            c.lowest = usize::MAX;
            c.fill_sum = 0.0;
            c.fill_count = 0;
            c.gaps_at_window_start = gaps_now;
            c.underruns = 0;
        }

        if copied < frames {
            self.conceal_into(c, &stream, out, copied, frames, out_ch, packet_frames, volume);
        }
        Self::apply_mute(&mut c.gain, self.is_muted(), out, frames, out_ch);
    }

    /// Son coupé ou rendu : le gain va vers 0 ou 1 en [`MUTE_FADE_FRAMES`]
    /// images audio, appliqué image par image.
    fn apply_mute(gain: &mut f32, muted: bool, out: &mut [f32], frames: usize, out_ch: usize) {
        let target = if muted { 0.0 } else { 1.0 };
        if *gain == target {
            if muted {
                out[..frames * out_ch].fill(0.0);
            }
            return;
        }
        let step = 1.0 / MUTE_FADE_FRAMES as f32;
        for frame in out[..frames * out_ch].chunks_exact_mut(out_ch.max(1)) {
            *gain = if muted { (*gain - step).max(0.0) } else { (*gain + step).min(1.0) };
            for v in frame.iter_mut() {
                *v *= *gain;
            }
        }
    }

    /// Tampon vide (fil de la carte son) : trame de masquage d'Opus (PLC),
    /// par paquets, le reste gardé pour la demande suivante. Si le décodeur
    /// est occupé par le fil réseau (un paquet arrive justement), silence.
    #[allow(clippy::too_many_arguments)]
    fn conceal_into(&self, c: &mut Consumer, stream: &Stream, out: &mut [f32], from: usize, frames: usize, out_ch: usize, packet_frames: usize, volume: f32) {
        let ch = stream.channels;
        let mut done = from;
        while done < frames {
            if c.plc_count == 0 {
                let Ok(mut p) = self.producer.try_lock() else { break };
                let produced = match p.decoder.as_mut() {
                    Some(decoder) if !c.plc.is_empty() && decoder.channels() == ch => {
                        let size = packet_frames.clamp(120, MAX_FRAMES) / 120 * 120;
                        decoder.decode(None, size, &mut c.plc)
                    }
                    _ => 0,
                };
                drop(p);
                if produced == 0 {
                    break;
                }
                c.plc_count = produced;
                c.plc_read = 0;
            }
            let n = (frames - done).min(c.plc_count);
            for i in 0..n {
                let src = &c.plc[(c.plc_read + i) * ch..(c.plc_read + i + 1) * ch];
                mix_frame(src, &mut c.mixed, volume);
                let dst = &mut out[(done + i) * out_ch..(done + i + 1) * out_ch];
                for (k, &s) in c.order.iter().enumerate() {
                    dst[k] = c.mixed[s];
                }
            }
            c.plc_read += n;
            c.plc_count -= n;
            done += n;
        }
        if done < frames {
            out[done * out_ch..frames * out_ch].fill(0.0);
        }
    }
}

impl PacketHandler for AudioEngine {
    fn packet(&self, packet: Packet<'_>) {
        match packet {
            Packet::Media { is_config: true, data, .. } => self.configure(data),
            Packet::Media { data, .. } => {
                if !data.is_empty() {
                    self.decode(data)
                }
            }
            Packet::Hole { missing_samples } => self.conceal(missing_samples as usize),
            Packet::Codec { .. } => {}
        }
    }

    fn audio_engine(self: Arc<Self>) -> Option<Arc<AudioEngine>> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::ManualClock;
    use crate::audio::opus::Encoder;

    /// Le banc d'essai : un la (440 Hz) encodé
    /// en Opus par paquets de 20 ms (ou 10), envoyé au moteur au rythme du
    /// serveur avec des à-coups et quelques pertes, la sortie tirant 5 ms à
    /// la fois ; le temps est celui d'une horloge simulée, par pas de 1 ms.
    struct Sim {
        clock: Arc<ManualClock>,
        engine: AudioEngine,
        enc: Encoder,
        channels: usize,
        frame: usize,
        phase: f64,
        pcm: Vec<f32>,
        seed: u64,
    }

    #[derive(Debug, Default)]
    struct Report {
        reports: Vec<AudioStats>,
        /// Images de sortie non muettes (première voie) après la première seconde.
        loud: usize,
        /// Images de sortie tirées après la première seconde.
        frames: usize,
        /// Niveau du tampon à la fin, en ms.
        final_ms: f64,
    }

    impl Report {
        fn settled(&self) -> impl Iterator<Item = &AudioStats> {
            self.reports.iter().skip(2)
        }
        fn buffer_ms(&self) -> f64 {
            let n = self.settled().count().max(1);
            self.settled().map(|s| s.buffer_ms).sum::<f64>() / n as f64
        }
        fn gaps(&self) -> usize {
            self.settled().map(|s| s.gaps).sum()
        }
    }

    impl Sim {
        fn new(channels: usize, frame: usize, order: OutputOrder) -> Self {
            let clock = Arc::new(ManualClock::new());
            let engine = AudioEngine::with_order(clock.clone(), order);
            let enc = Encoder::new(channels);
            engine.configure(&enc.head);
            Self {
                clock,
                engine,
                enc,
                channels,
                frame,
                phase: 0.0,
                pcm: vec![0.0; frame * channels],
                seed: 7,
            }
        }

        fn random(&mut self) -> f64 {
            self.seed = self.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.seed >> 11) as f64 / (1u64 << 53) as f64
        }

        fn packet(&mut self) -> Vec<u8> {
            for i in 0..self.frame {
                let v = (self.phase.sin() * 0.2) as f32;
                self.phase += 2.0 * std::f64::consts::PI * 440.0 / SAMPLE_RATE as f64;
                for c in 0..self.channels {
                    self.pcm[i * self.channels + c] = v;
                }
            }
            self.enc.encode(&self.pcm)
        }

        /// `jitter_ms` : à-coups ± ; `lose_every` : un paquet perdu sur n ;
        /// `rate` : vitesse de l'horloge du serveur par rapport à la nôtre
        /// (1,002 : ses paquets arrivent 0,2 % trop vite) ; `stall_ms` : la
        /// sortie ne tire rien pendant ce temps au départ.
        fn run(&mut self, seconds: u64, out_channels: usize, jitter_ms: f64, lose_every: Option<usize>, rate: f64, stall_ms: u64) -> Report {
            let mut report = Report::default();
            let mut n = 0usize;
            let mut due = 0.0f64;
            let mut next_fill = stall_ms;
            let total_ms = seconds * 1000;
            let mut out = vec![0f32; 240 * out_channels];
            for t in 0..total_ms {
                self.clock.set(Duration::from_millis(t));
                // Paquets dus à cet instant.
                while due <= t as f64 {
                    if lose_every.is_some_and(|k| n % k == k / 2) {
                        self.engine.conceal(self.frame);
                    } else {
                        let p = self.packet();
                        self.engine.decode(&p);
                    }
                    n += 1;
                    let jitter = (self.random() * 2.0 - 1.0) * jitter_ms;
                    due = (n * self.frame) as f64 * 1000.0 / SAMPLE_RATE as f64 / rate + jitter;
                }
                // La sortie tire 5 ms toutes les 5 ms.
                if t >= next_fill {
                    next_fill = t + 5;
                    self.engine.fill(&mut out, 240);
                    if t >= 1000 {
                        report.frames += 240;
                        report.loud += out.chunks_exact(out_channels).filter(|f| f[0].abs() > 0.01).count();
                    }
                    if let Some(s) = self.engine.take_stats() {
                        report.reports.push(s);
                    }
                }
            }
            report.final_ms = self.engine.current.lock().unwrap().as_ref().map_or(0.0, |s| s.ring.len() as f64 / self.channels as f64 * 1000.0 / SAMPLE_RATE as f64);
            report
        }
    }

    #[test]
    fn stereo_au_rythme_reel_avec_a_coups_et_pertes() {
        let mut sim = Sim::new(2, 960, OutputOrder::Opus);
        let r = sim.run(8, 2, 8.0, Some(97), 1.0, 0);
        let losses = 8 * 50 / 97;
        assert!(r.reports.len() >= 7, "un relevé par seconde : {}", r.reports.len());
        assert!(r.loud as f64 > r.frames as f64 * 0.95, "le son passe : {} images sur {} non muettes", r.loud, r.frames);
        assert!(r.buffer_ms() > 5.0 && r.buffer_ms() < 60.0, "tampon moyen d'une vingtaine de ms : {:.1}", r.buffer_ms());
        assert!(r.gaps() <= losses + 4, "trous : {} pour {} pertes simulées", r.gaps(), losses);
        assert_eq!(r.reports[0].channels, 2);
        assert_eq!(r.reports[0].output_channels, 2);
    }

    #[test]
    fn cinq_un_mixe_vers_la_stereo_et_remis_dans_l_ordre() {
        let mut sim = Sim::new(6, 960, OutputOrder::Windows);
        let r = sim.run(5, 2, 8.0, Some(97), 1.0, 0);
        assert!(r.loud as f64 > r.frames as f64 * 0.95, "5.1 → casque : {} sur {}", r.loud, r.frames);
        assert_eq!(r.reports.last().unwrap().output_channels, 2);
        // Les six voies, dans l'ordre de Windows : la LFE (Opus 5) va en position 3.
        let mut sim = Sim::new(6, 960, OutputOrder::Windows);
        let r = sim.run(3, 6, 0.0, None, 1.0, 0);
        assert!(r.loud as f64 > r.frames as f64 * 0.95);
        assert_eq!(r.gaps(), 0);
        let mut out = vec![0f32; 240 * 6];
        sim.engine.fill(&mut out, 240);
        let frame = &out[..6];
        for i in [1, 2, 4, 5] {
            // Les flux d'Opus (G/D, C, surrounds) ne décodent pas au bit près.
            assert!((frame[i] - frame[0]).abs() < 0.05, "même signal sur les cinq voies principales : {frame:?}");
        }
        let lfe: f32 = out.chunks_exact(6).map(|f| f[3].abs()).sum::<f32>() / 240.0;
        let left: f32 = out.chunks_exact(6).map(|f| f[0].abs()).sum::<f32>() / 240.0;
        assert!(lfe < left / 4.0, "la LFE (Opus 5, filtrée par l'encodeur) est en position 3 comme Windows l'attend : {lfe} contre {left}");
    }

    #[test]
    fn paquets_de_10_ms() {
        let mut sim = Sim::new(2, 480, OutputOrder::CoreAudio);
        let r = sim.run(6, 2, 8.0, Some(97), 1.0, 0);
        assert!(r.loud as f64 > r.frames as f64 * 0.95);
        assert!(r.buffer_ms() < 60.0);
    }

    #[test]
    fn derive_rattrapee_sans_coupe() {
        // Le serveur va 0,2 % trop vite : 2 ms d'avance par seconde, que le
        // saut d'un échantillon sur 256 (0,4 %) absorbe sans trou ni coupe.
        let mut sim = Sim::new(2, 960, OutputOrder::Opus);
        let r = sim.run(12, 2, 2.0, None, 1.002, 0);
        assert_eq!(r.gaps(), 0, "aucun trou");
        assert!(r.buffer_ms() < 40.0, "le tampon ne grossit pas : {:.1} ms", r.buffer_ms());
        assert!(r.final_ms < 40.0, "{:.1} ms à la fin", r.final_ms);
        // Le serveur va 0,2 % trop lentement : des échantillons doublés, pas de trou.
        let mut sim = Sim::new(2, 960, OutputOrder::Opus);
        let r = sim.run(12, 2, 2.0, None, 0.998, 0);
        assert!(r.gaps() <= 2, "trous : {}", r.gaps());
    }

    #[test]
    fn avance_de_plus_de_10_ms_coupee_d_un_coup() {
        let mut sim = Sim::new(2, 960, OutputOrder::Opus);
        let r = sim.run(4, 2, 0.0, None, 1.0, 200);
        let first = r.reports[0].buffer_ms;
        let last = r.reports.last().unwrap().buffer_ms;
        assert!(first > 100.0, "sortie arrêtée 200 ms au départ : {first:.1} ms");
        assert!(last < 40.0, "coupe sèche : {last:.1} ms");
        assert_eq!(r.gaps(), 0);
    }

    #[test]
    fn son_coupe_en_fondu_et_rendu_sans_decalage() {
        let mut sim = Sim::new(2, 960, OutputOrder::Opus);
        let r = sim.run(2, 2, 0.0, None, 1.0, 0);
        assert!(r.loud as f64 > r.frames as f64 * 0.95);
        let level = |out: &[f32]| out.chunks_exact(2).map(|f| f[0].abs()).fold(0f32, f32::max);
        // Un paquet de plus pour que le tampon ne se vide pas pendant l'essai.
        for _ in 0..4 {
            let p = sim.packet();
            sim.engine.decode(&p);
        }
        let mut out = vec![0f32; 240 * 2];
        sim.engine.fill(&mut out, 240);
        let full = level(&out);
        assert!(full > 0.1, "le la passe : {full}");
        sim.engine.set_muted(true);
        assert!(sim.engine.is_muted());
        let before = sim.engine.current.lock().unwrap().as_ref().unwrap().ring.len();
        sim.engine.fill(&mut out, 240);
        let gain = |e: &AudioEngine| e.consumer.lock().unwrap().gain;
        assert!(level(&out) > 0.0 && (gain(&sim.engine) - 0.75).abs() < 0.01, "fondu : un quart en 5 ms ({})", gain(&sim.engine));
        let after = sim.engine.current.lock().unwrap().as_ref().unwrap().ring.len();
        assert_eq!(before - after, 240 * 2, "le son coupé est tiré au même rythme");
        for _ in 0..4 {
            sim.engine.fill(&mut out, 240);
        }
        assert_eq!(level(&out), 0.0, "coupé après 20 ms");
        for _ in 0..4 {
            let p = sim.packet();
            sim.engine.decode(&p);
        }
        sim.engine.set_muted(false);
        sim.engine.fill(&mut out, 240);
        assert!(level(&out) > 0.0 && (gain(&sim.engine) - 0.25).abs() < 0.01, "rendu en fondu ({})", gain(&sim.engine));
        for _ in 0..4 {
            sim.engine.fill(&mut out, 240);
        }
        assert!(level(&out) > full * 0.8, "rendu en 20 ms : {} contre {full}", level(&out));
        // Sans flux : immédiat.
        let engine = AudioEngine::new(Arc::new(ManualClock::new()));
        engine.set_muted(true);
        let mut out = vec![1f32; 480];
        engine.fill(&mut out, 240);
        assert_eq!(engine.consumer.lock().unwrap().gain, 0.0);
    }

    #[test]
    fn sans_flux_silence_et_mesures() {
        let clock = Arc::new(ManualClock::new());
        let engine = AudioEngine::new(clock.clone());
        let mut out = vec![1f32; 480];
        engine.fill(&mut out, 240);
        assert!(out.iter().all(|v| *v == 0.0), "pas de flux : silence");
        assert_eq!(engine.channels(), 0);
        engine.set_output_latency_ms(12.5);
        assert_eq!(engine.output_latency_ms(), 12.5);
        let enc = Encoder::new(2);
        engine.configure(&enc.head);
        assert_eq!(engine.channels(), 2);
        engine.configure(&enc.head);
        engine.stop();
        assert_eq!(engine.channels(), 0);
        engine.fill(&mut out, 240);
        assert!(out.iter().all(|v| *v == 0.0));
    }
}
