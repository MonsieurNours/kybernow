//! Latence de bout en bout et temps de chaque étape de la chaîne : capture,
//! encodage et envoi sur l'ordinateur, réseau, décodage et affichage sur
//! l'appareil.
//!
//! Le serveur horodate chaque image à chaque étape (« acquired », « encoding »,
//! « encoded », « sent », sur son horloge) et l'envoie avec les mesures de
//! Kyber ; les « pings » de Kyber donnent le décalage entre son horloge et
//! celle de l'appareil (offset = serveur − appareil, en µs, horloge de
//! [`now_micros`]). Le rendu signale l'instant où chaque image est reçue,
//! décodée et montrée ; on en tire la médiane de chaque seconde.
//!
//! Kyber donne aussi, toutes les quelques secondes, l'aller-retour réseau et
//! les paquets perdus (cumulés depuis le début du flux). Une image envoyée par
//! l'ordinateur qui n'est ni montrée ni remplacée par une plus récente est
//! perdue (paquets manquants, ou image refusée en attendant une image clé).
//!
//! Les mesures du serveur peuvent arriver après l'affichage de l'image (sans
//! synchro verticale surtout) : les instants d'une image montrée sans mesure
//! sont gardés une seconde, et la latence est calculée à l'arrivée de la
//! mesure. Ce qui n'a pas pu être mesuré est compté (`rejected`), et le
//! rapport images mesurées / images montrées est exposé : une médiane sur
//! peu d'images serait optimiste.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use kyclient_common::metrics::{Metric, NetworkStats, VideoMetricData};

/// Deux secondes d'images à 240 Hz au plus.
const MAX_FRAMES: usize = 480;
const MAX_OFFSETS: usize = 15;
/// Une image montrée attend sa mesure du serveur une seconde.
const LATE_METRIC_MICROS: i64 = 1_000_000;

/// Horloge de l'appareil dans laquelle Kyber exprime l'écart d'horloge, en µs :
/// la même que kyproto (QueryPerformanceCounter sous Windows, CLOCK_MONOTONIC
/// ailleurs).
pub fn now_micros() -> i64 {
    clock::now()
}

mod clock {
    #[cfg(windows)]
    pub fn now() -> i64 {
        unsafe extern "system" {
            fn QueryPerformanceCounter(count: *mut i64) -> i32;
            fn QueryPerformanceFrequency(freq: *mut i64) -> i32;
        }
        let (mut c, mut f) = (0i64, 0i64);
        unsafe {
            QueryPerformanceFrequency(&mut f);
            QueryPerformanceCounter(&mut c);
        }
        if f == 0 {
            return 0;
        }
        (c as i128 * 1_000_000 / f as i128) as i64
    }

    #[cfg(not(windows))]
    pub fn now() -> i64 {
        let mut ts = libc_timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe extern "C" {
            fn clock_gettime(clk: i32, ts: *mut libc_timespec) -> i32;
        }
        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos", target_os = "visionos"))]
        const CLOCK_MONOTONIC: i32 = 6;
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "tvos", target_os = "visionos")))]
        const CLOCK_MONOTONIC: i32 = 1;
        unsafe { clock_gettime(CLOCK_MONOTONIC, &mut ts) };
        ts.tv_sec * 1_000_000 + ts.tv_nsec / 1000
    }

    #[cfg(not(windows))]
    #[repr(C)]
    struct libc_timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
}

/// Temps de chaque étape d'une image, en millisecondes (médianes de la seconde).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChainTimes {
    /// Capture → début de l'encodage (attente sur l'ordinateur).
    pub capture: f64,
    pub encode: f64,
    /// Fin de l'encodage → envoi (mise en paquets, cadence).
    pub send: f64,
    /// Envoi → image complète reçue par l'appareil.
    pub network: f64,
    pub decode: f64,
    /// Décodée → montrée à l'écran (rendu, attente de la synchro).
    pub display: f64,
}

impl ChainTimes {
    pub fn all(&self) -> [f64; 6] {
        [self.capture, self.encode, self.send, self.network, self.decode, self.display]
    }

    pub fn sum(&self) -> f64 {
        self.all().iter().sum()
    }
}

/// Mesures de la seconde écoulée.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Second {
    /// Médiane de la latence capture → affichage.
    pub latency_ms: Option<f64>,
    pub chain: Option<ChainTimes>,
    /// Images montrées, images avec une latence mesurée, images montrées
    /// sans mesure (du serveur) ou avec une valeur absurde.
    pub displayed: u32,
    pub measured: u32,
    pub rejected: u32,
    /// Images envoyées par l'ordinateur jamais montrées.
    pub lost_frames: u32,
    /// Trous dans le flux (paquets perdus au-delà de la correction d'erreurs).
    pub holes: u32,
    pub keyframe_requests: u32,
    /// Paquets perdus d'après Kyber, depuis la mesure précédente.
    pub lost_packets: u32,
    /// Aller-retour réseau (dernière mesure), en millisecondes.
    pub rtt_ms: Option<f64>,
    /// Octets d'image reçus.
    pub bytes: u64,
}

/// Étapes d'une image sur l'ordinateur (µs, son horloge).
#[derive(Default, Clone, Copy)]
struct Sent {
    acquired: Option<i64>,
    encoding: Option<i64>,
    encoded: Option<i64>,
    sent: Option<i64>,
    /// Arrivée de la première mesure, sur l'horloge de l'appareil (µs).
    seen: i64,
}

/// Image montrée dont la mesure du serveur n'est pas encore arrivée.
#[derive(Clone, Copy)]
struct Shown {
    received: i64,
    decoded: i64,
    at: i64,
    seen: i64,
}

#[derive(Default)]
struct Inner {
    /// pts → étapes sur l'ordinateur.
    frames: HashMap<u64, Sent>,
    order: VecDeque<u64>,
    /// Images montrées en attendant la mesure du serveur (une seconde au plus).
    shown: HashMap<u64, Shown>,
    shown_order: VecDeque<u64>,
    /// Images déjà montrées ou remplacées : une mesure en retard ne les rouvre pas.
    done: HashSet<u64>,
    done_order: VecDeque<u64>,
    offsets: VecDeque<i64>,
    samples: Vec<f64>,
    stages: [Vec<f64>; 6],
    second: Second,
    rtt_ms: Option<f64>,
    /// Derniers cumuls de paquets perdus (local, distant).
    lost_totals: [i64; 2],
    /// Médianes de chaque seconde, toute la session (accueil, console).
    session: Vec<f64>,
    /// Mesures reçues de l'ordinateur par étape (journal, une fois par flux).
    kinds: [u32; 4],
    kinds_logged: bool,
}

impl Inner {
    fn new() -> Self {
        Self {
            lost_totals: [-1, -1],
            ..Default::default()
        }
    }

    fn offset(&self) -> Option<i64> {
        if self.offsets.is_empty() {
            return None;
        }
        let mut v: Vec<i64> = self.offsets.iter().copied().collect();
        v.sort_unstable();
        Some(v[v.len() / 2])
    }

    /// Image jamais montrée : perdue si l'ordinateur l'avait envoyée. Montrée
    /// sans mesure arrivée : mesure rejetée.
    fn expire(&mut self, pts: u64) {
        if self.shown.contains_key(&pts) {
            self.second.rejected += 1;
            self.finish(pts);
            return;
        }
        if let Some(f) = self.frames.remove(&pts) {
            if f.sent.is_some() {
                self.second.lost_frames += 1;
            }
        }
    }

    fn finish(&mut self, pts: u64) {
        if self.frames.remove(&pts).is_some() {
            if let Some(i) = self.order.iter().position(|p| *p == pts) {
                self.order.remove(i);
            }
        }
        self.shown.remove(&pts);
        if self.done.insert(pts) {
            self.done_order.push_back(pts);
            if self.done_order.len() > MAX_FRAMES {
                if let Some(old) = self.done_order.pop_front() {
                    self.done.remove(&old);
                }
            }
        }
    }

    /// Latence et étapes d'une image montrée dont la capture est datée.
    fn measure(&mut self, f: Sent, received: i64, decoded: i64, at: i64) {
        let (Some(captured), Some(offset)) = (f.acquired, self.offset()) else {
            self.second.rejected += 1;
            return;
        };
        let ms = (at - (captured - offset)) as f64 / 1000.0;
        // Écarte les valeurs absurdes (horloge pas encore recalée).
        if !(ms > 0.0 && ms < 2000.0) {
            self.second.rejected += 1;
            return;
        }
        self.samples.push(ms);
        self.second.measured += 1;
        let (Some(encoding), Some(encoded), Some(sent)) = (f.encoding, f.encoded, f.sent) else {
            return;
        };
        let steps = [
            encoding - captured,
            encoded - encoding,
            sent - encoded,
            received - (sent - offset),
            decoded - received,
            at - decoded,
        ];
        // L'horloge recalée peut faire passer le réseau un rien sous zéro sur un réseau local.
        for (i, v) in steps.iter().enumerate() {
            self.stages[i].push((*v as f64 / 1000.0).max(0.0));
        }
    }

    fn server(&mut self, slot: usize, pts: u64, ts: i64) {
        self.kinds[slot] += 1;
        if self.done.contains(&pts) {
            return;
        }
        let mut f = match self.frames.get(&pts) {
            Some(f) => *f,
            None => {
                self.order.push_back(pts);
                Sent {
                    seen: now_micros(),
                    ..Default::default()
                }
            }
        };
        match slot {
            0 => f.acquired = Some(ts),
            1 => f.encoding = Some(ts),
            2 => f.encoded = Some(ts),
            _ => f.sent = Some(ts),
        }
        self.frames.insert(pts, f);
        // Image déjà montrée : sa latence se calcule maintenant.
        if f.acquired.is_some() {
            if let Some(s) = self.shown.get(&pts).copied() {
                self.measure(f, s.received, s.decoded, s.at);
                self.finish(pts);
                return;
            }
        }
        while self.order.len() > MAX_FRAMES {
            if let Some(old) = self.order.pop_front() {
                self.expire(old);
            }
        }
    }

    fn network(&mut self, stats: &NetworkStats, slot: usize) {
        if let Some(rtt) = stats.rtt_micros {
            if slot == 0 || self.rtt_ms.is_none() {
                self.rtt_ms = Some(rtt as f64 / 1000.0);
            }
        }
        let Some(lost) = stats.packets_lost else { return };
        let lost = lost as i64;
        let last = self.lost_totals[slot];
        // Cumul : on garde l'écart. S'il recule, le flux a redémarré.
        if last >= 0 {
            self.second.lost_packets += if lost >= last { lost - last } else { lost } as u32;
        }
        self.lost_totals[slot] = lost;
    }
}

pub struct LatencyTracker {
    inner: Mutex<Inner>,
    /// Une image a été montrée depuis la création (jamais remis à zéro).
    shown: AtomicBool,
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    Some(v[v.len() / 2])
}

impl LatencyTracker {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::new()),
            shown: AtomicBool::new(false),
        }
    }

    /// Une image a été montrée depuis la création du suivi, même avant un
    /// [`Self::reset`] : la session a eu ses premières images.
    pub fn any_displayed(&self) -> bool {
        self.shown.load(Ordering::Relaxed)
    }

    /// Les numéros d'image repartent de zéro à chaque démarrage de l'image.
    pub fn reset(&self) {
        let mut i = self.inner.lock().unwrap();
        *i = Inner::new();
    }

    pub fn on_metric(&self, metric: &Metric) {
        let mut i = self.inner.lock().unwrap();
        match metric {
            Metric::Video(v) => {
                let slot = match v.data {
                    VideoMetricData::FrameAcquired => 0,
                    VideoMetricData::FrameEncoding => 1,
                    VideoMetricData::FrameEncoded => 2,
                    VideoMetricData::FrameSent => 3,
                    // Étapes du client : l'app les mesure elle-même.
                    _ => return,
                };
                i.server(slot, v.pts, v.ts);
            }
            Metric::NetworkPing(p) => {
                i.offsets.push_back(p.offset_micros);
                while i.offsets.len() > MAX_OFFSETS {
                    i.offsets.pop_front();
                }
            }
            Metric::NetworkLocal(s) => i.network(s, 0),
            Metric::NetworkRemote(s) => i.network(s, 1),
        }
    }

    /// Paquet d'image reçu de Kyber (fil réseau).
    pub fn received_bytes(&self, bytes: usize) {
        self.inner.lock().unwrap().second.bytes += bytes as u64;
    }

    /// Trou dans le flux.
    pub fn hole(&self) {
        self.inner.lock().unwrap().second.holes += 1;
    }

    pub fn keyframe_requested(&self) {
        self.inner.lock().unwrap().second.keyframe_requests += 1;
    }

    /// Image remplacée par une plus récente avant d'être montrée.
    pub fn skipped(&self, pts: u64) {
        self.inner.lock().unwrap().finish(pts);
    }

    /// Une image vient d'être présentée à l'écran (horloge [`now_micros`]) ;
    /// sans les instants de réception et de décodage, pas de trajet détaillé.
    pub fn displayed(&self, pts: u64, at_micros: i64) {
        self.frame_displayed(pts, at_micros, at_micros, at_micros);
    }

    /// Image `pts` reçue, décodée puis montrée (µs, horloge [`now_micros`]).
    pub fn frame_displayed(&self, pts: u64, received: i64, decoded: i64, at: i64) {
        self.shown.store(true, Ordering::Relaxed);
        let mut i = self.inner.lock().unwrap();
        i.second.displayed += 1;
        if i.done.contains(&pts) {
            return;
        }
        if let Some(f) = i.frames.get(&pts).copied() {
            if f.acquired.is_some() {
                i.measure(f, received, decoded, at);
                i.finish(pts);
                return;
            }
        }
        // Mesure du serveur pas encore là : on garde les instants une seconde.
        let now = now_micros();
        if !i.frames.contains_key(&pts) {
            i.frames.insert(pts, Sent { seen: now, ..Default::default() });
            i.order.push_back(pts);
        }
        i.shown.insert(pts, Shown { received, decoded, at, seen: now });
        i.shown_order.push_back(pts);
        while i.shown_order.len() > MAX_FRAMES {
            if let Some(old) = i.shown_order.pop_front() {
                i.expire(old);
            }
        }
    }

    /// Mesures depuis le dernier appel (une fois par seconde).
    pub fn drain(&self) -> Second {
        let mut i = self.inner.lock().unwrap();
        // Images montrées il y a plus d'une seconde dont la mesure n'est jamais venue.
        let limit = now_micros() - LATE_METRIC_MICROS;
        while let Some(first) = i.shown_order.front().copied() {
            match i.shown.get(&first) {
                None => {
                    i.shown_order.pop_front();
                }
                Some(s) if s.seen < limit => {
                    i.shown_order.pop_front();
                    i.expire(first);
                }
                Some(_) => break,
            }
        }
        // Images vues il y a plus d'une seconde et toujours pas montrées.
        while let Some(first) = i.order.front().copied() {
            match i.frames.get(&first) {
                Some(f) if f.seen < limit => {
                    i.order.pop_front();
                    i.expire(first);
                }
                Some(_) => break,
                None => {
                    i.order.pop_front();
                }
            }
        }
        if !i.kinds_logged && i.kinds[0] > 0 {
            i.kinds_logged = true;
            log::info!(
                "mesures de l'ordinateur : capture {}, encodage {}, encodée {}, envoi {}, décalage d'horloge {}",
                i.kinds[0],
                i.kinds[1],
                i.kinds[2],
                i.kinds[3],
                if i.offsets.is_empty() { "absent" } else { "présent" }
            );
        }
        let mut out = std::mem::take(&mut i.second);
        out.rtt_ms = i.rtt_ms;
        let mut samples = std::mem::take(&mut i.samples);
        if let Some(m) = median(&mut samples) {
            out.latency_ms = Some(m);
            i.session.push(m);
        }
        if !i.stages[0].is_empty() {
            let mut m = [0.0; 6];
            for (k, stage) in i.stages.iter_mut().enumerate() {
                m[k] = median(stage).unwrap_or(0.0);
                stage.clear();
            }
            out.chain = Some(ChainTimes {
                capture: m[0],
                encode: m[1],
                send: m[2],
                network: m[3],
                decode: m[4],
                display: m[5],
            });
        }
        out
    }

    /// Médiane de la dernière seconde seulement (voir [`Self::drain`]).
    pub fn drain_second(&self) -> Option<f64> {
        self.drain().latency_ms
    }

    /// Médiane des médianes de chaque seconde, et 95e centile de la session
    /// (la latence des mauvais moments, montrée avec la médiane sur l'accueil).
    pub fn session(&self) -> (Option<f64>, Option<f64>) {
        let i = self.inner.lock().unwrap();
        let mut per = i.session.clone();
        let med = median(&mut per);
        let p95 = if per.is_empty() {
            None
        } else {
            Some(per[((per.len() as f64 * 0.95) as usize).min(per.len() - 1)])
        };
        (med, p95)
    }

    pub fn rtt_ms(&self) -> Option<f64> {
        self.inner.lock().unwrap().rtt_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kyclient_common::metrics::{NetworkPing, VideoMetric};

    fn video(pts: u64, ts: i64, data: VideoMetricData) -> Metric {
        Metric::Video(VideoMetric { source_id: 1, pts, ts, data })
    }

    #[test]
    fn latence_sur_horloge_recalee() {
        let t = LatencyTracker::new();
        // Ordinateur en avance de 1 s sur l'appareil.
        t.on_metric(&Metric::NetworkPing(NetworkPing { offset_micros: 1_000_000, delay_micros: 500 }));
        t.on_metric(&video(7, 5_000_000, VideoMetricData::FrameAcquired));
        t.displayed(7, 4_008_500);
        assert_eq!(t.drain_second(), Some(8.5));
        // Mesure arrivée après l'affichage.
        t.displayed(8, 4_020_000);
        t.on_metric(&video(8, 5_010_000, VideoMetricData::FrameAcquired));
        assert_eq!(t.drain_second(), Some(10.0));
        let (med, p95) = t.session();
        assert_eq!(med, Some(10.0));
        assert_eq!(p95, Some(10.0));
    }

    #[test]
    fn trajet_d_une_image() {
        let t = LatencyTracker::new();
        t.on_metric(&Metric::NetworkPing(NetworkPing { offset_micros: 0, delay_micros: 0 }));
        t.on_metric(&video(1, 1_000, VideoMetricData::FrameAcquired));
        t.on_metric(&video(1, 2_000, VideoMetricData::FrameEncoding));
        t.on_metric(&video(1, 4_000, VideoMetricData::FrameEncoded));
        t.on_metric(&video(1, 5_000, VideoMetricData::FrameSent));
        t.frame_displayed(1, 8_000, 9_500, 12_000);
        let s = t.drain();
        assert_eq!(s.latency_ms, Some(11.0));
        assert_eq!(s.displayed, 1);
        assert_eq!(s.measured, 1);
        let c = s.chain.unwrap();
        assert_eq!(c.all(), [1.0, 2.0, 1.0, 3.0, 1.5, 2.5]);
    }

    #[test]
    fn pertes_et_rejets() {
        let t = LatencyTracker::new();
        t.on_metric(&Metric::NetworkPing(NetworkPing { offset_micros: 0, delay_micros: 0 }));
        // Image envoyée jamais montrée : perdue dès que la file déborde.
        for pts in 0..=MAX_FRAMES as u64 {
            t.on_metric(&video(pts, 1_000, VideoMetricData::FrameSent));
        }
        // Image montrée sans jamais de mesure : rejetée à l'expiration.
        t.displayed(9_999, now_micros() - 2_000_000);
        // Image remplacée : ni perdue ni rejetée.
        t.skipped(7);
        t.hole();
        t.keyframe_requested();
        t.received_bytes(1500);
        t.on_metric(&Metric::NetworkLocal(NetworkStats { rtt_micros: Some(3_400), packets_lost: Some(10), dropped_packets: None }));
        t.on_metric(&Metric::NetworkLocal(NetworkStats { rtt_micros: None, packets_lost: Some(14), dropped_packets: None }));
        let s = t.drain();
        assert_eq!(s.lost_frames, 1);
        assert_eq!(s.holes, 1);
        assert_eq!(s.keyframe_requests, 1);
        assert_eq!(s.bytes, 1500);
        assert_eq!(s.lost_packets, 4);
        assert_eq!(s.rtt_ms, Some(3.4));
        assert_eq!(s.displayed, 1);
        assert_eq!(s.latency_ms, None);
    }
}
