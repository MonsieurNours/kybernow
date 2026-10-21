//! Latence de bout en bout (capture sur l'ordinateur → affichage sur
//! l'appareil), comme `LatencyTracker.swift` : les horodatages de Kyber
//! (horloge de l'ordinateur) sont ramenés à celle de l'appareil par l'écart
//! médian des 15 derniers échanges d'horloge.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use kyclient_common::metrics::{Metric, VideoMetricData};

const MAX_FRAMES: usize = 480;
const MAX_OFFSETS: usize = 15;
const LATE_METRIC: Duration = Duration::from_secs(1);

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
        #[cfg(target_os = "macos")]
        const CLOCK_MONOTONIC: i32 = 6;
        #[cfg(not(target_os = "macos"))]
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

#[derive(Default)]
struct Inner {
    offsets: VecDeque<i64>,
    acquired: HashMap<u64, i64>,
    order: VecDeque<u64>,
    /// Images affichées dont la mesure de l'ordinateur n'est pas encore là.
    pending: VecDeque<(u64, i64, Instant)>,
    second: Vec<f64>,
    per_second: Vec<f64>,
    all: Vec<f64>,
    rtt_ms: Option<f64>,
}

impl Inner {
    fn offset(&self) -> Option<i64> {
        if self.offsets.is_empty() {
            return None;
        }
        let mut v: Vec<i64> = self.offsets.iter().copied().collect();
        v.sort_unstable();
        Some(v[v.len() / 2])
    }

    fn record(&mut self, pts: u64, displayed: i64) -> bool {
        let (Some(acq), Some(off)) = (self.acquired.get(&pts).copied(), self.offset()) else {
            return false;
        };
        let ms = (displayed - (acq - off)) as f64 / 1000.0;
        if ms > 0.0 && ms < 2000.0 {
            self.second.push(ms);
            if self.all.len() < 200_000 {
                self.all.push(ms);
            }
        }
        true
    }
}

#[derive(Default)]
pub struct LatencyTracker {
    inner: Mutex<Inner>,
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
        Self::default()
    }

    /// Les numéros d'image repartent de zéro à chaque démarrage de l'image.
    pub fn reset(&self) {
        let mut i = self.inner.lock().unwrap();
        i.acquired.clear();
        i.order.clear();
        i.pending.clear();
        i.second.clear();
    }

    pub fn on_metric(&self, metric: &Metric) {
        let mut i = self.inner.lock().unwrap();
        match metric {
            Metric::Video(v) if v.data == VideoMetricData::FrameAcquired => {
                if i.acquired.insert(v.pts, v.ts).is_none() {
                    i.order.push_back(v.pts);
                }
                while i.order.len() > MAX_FRAMES {
                    if let Some(old) = i.order.pop_front() {
                        i.acquired.remove(&old);
                    }
                }
                // Images déjà affichées qui attendaient cette mesure.
                let pending: Vec<_> = i.pending.drain(..).collect();
                for (pts, displayed, at) in pending {
                    if !i.record(pts, displayed) && at.elapsed() < LATE_METRIC {
                        i.pending.push_back((pts, displayed, at));
                    }
                }
            }
            Metric::NetworkPing(p) => {
                i.offsets.push_back(p.offset_micros);
                while i.offsets.len() > MAX_OFFSETS {
                    i.offsets.pop_front();
                }
            }
            Metric::NetworkLocal(s) => {
                if let Some(rtt) = s.rtt_micros {
                    i.rtt_ms = Some(rtt as f64 / 1000.0);
                }
            }
            _ => {}
        }
    }

    /// Une image vient d'être présentée à l'écran (horloge `now_micros`).
    pub fn displayed(&self, pts: u64, at_micros: i64) {
        let mut i = self.inner.lock().unwrap();
        if !i.record(pts, at_micros) {
            i.pending.push_back((pts, at_micros, Instant::now()));
            while i.pending.len() > MAX_FRAMES {
                i.pending.pop_front();
            }
        }
    }

    /// Médiane de la dernière seconde (à appeler une fois par seconde).
    pub fn drain_second(&self) -> Option<f64> {
        let mut i = self.inner.lock().unwrap();
        let mut s = std::mem::take(&mut i.second);
        let m = median(&mut s);
        if let Some(m) = m {
            i.per_second.push(m);
        }
        m
    }

    /// Médiane des médianes de chaque seconde, et 95e centile de la session.
    pub fn session(&self) -> (Option<f64>, Option<f64>) {
        let i = self.inner.lock().unwrap();
        let mut per = i.per_second.clone();
        let mut all = i.all.clone();
        let med = median(&mut per);
        let p95 = if all.is_empty() {
            None
        } else {
            all.sort_by(|a, b| a.total_cmp(b));
            Some(all[((all.len() as f64 * 0.95) as usize).min(all.len() - 1)])
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

    #[test]
    fn latence_sur_horloge_recalee() {
        let t = LatencyTracker::new();
        // Ordinateur en avance de 1 s sur l'appareil.
        t.on_metric(&Metric::NetworkPing(NetworkPing { offset_micros: 1_000_000, delay_micros: 500 }));
        t.on_metric(&Metric::Video(VideoMetric { source_id: 1, pts: 7, ts: 5_000_000, data: VideoMetricData::FrameAcquired }));
        t.displayed(7, 4_008_500);
        assert_eq!(t.drain_second(), Some(8.5));
        // Mesure arrivée après l'affichage.
        t.displayed(8, 4_020_000);
        t.on_metric(&Metric::Video(VideoMetric { source_id: 1, pts: 8, ts: 5_010_000, data: VideoMetricData::FrameAcquired }));
        assert_eq!(t.drain_second(), Some(10.0));
    }
}
