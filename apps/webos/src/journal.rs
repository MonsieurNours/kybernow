//! Bilans de l'image au journal, toutes les 5 s : la file du décodeur du
//! téléviseur (NDL) et la latence étape par étape, en SDR comme en HDR, pour
//! qu'un essai dise où part le temps (le téléviseur garde-t-il une image de
//! plus en HDR, ou est-ce l'ordinateur ?).

use std::time::{Duration, Instant};

/// Durée d'un bilan.
pub const WINDOW: Duration = Duration::from_secs(5);

/// La file de NDL sur un bilan : images en attente d'affichage, lues après
/// chaque image donnée (`NDL_DirectVideoGetRenderBufferLength`), et durée de
/// l'appel qui donne l'image.
pub struct Queue {
    started: Instant,
    frames: u32,
    /// Images sans longueur de file (fonction absente ou refusée).
    unknown: u32,
    sum: u64,
    max: i32,
    play_sum_us: u64,
    play_max_us: u64,
}

impl Queue {
    pub fn new(now: Instant) -> Self {
        Self { started: now, frames: 0, unknown: 0, sum: 0, max: 0, play_sum_us: 0, play_max_us: 0 }
    }

    pub fn add(&mut self, queued: Option<i32>, play: Duration) {
        self.frames += 1;
        match queued {
            Some(n) => {
                let n = n.max(0);
                self.sum += n as u64;
                self.max = self.max.max(n);
            }
            None => self.unknown += 1,
        }
        let us = play.as_micros() as u64;
        self.play_sum_us += us;
        self.play_max_us = self.play_max_us.max(us);
    }

    pub fn due(&self, now: Instant) -> bool {
        now.duration_since(self.started) >= WINDOW
    }

    /// La ligne du bilan (`None` : aucune image), et un bilan neuf.
    pub fn take(&mut self, now: Instant, hdr: bool, period_us: i64) -> Option<String> {
        let line = (self.frames > 0).then(|| {
            let known = self.frames - self.unknown;
            let queue = if known > 0 {
                format!("moyenne {:.2}, max {}", self.sum as f64 / known as f64, self.max)
            } else {
                "inconnue".to_string()
            };
            format!(
                "image : file NDL sur {:.0} s ({}) : {queue} ; période {period_us} µs ({:.0} Hz) ; {} images, appel moyen {:.2} ms, max {:.2} ms{}",
                now.duration_since(self.started).as_secs_f64(),
                if hdr { "HDR" } else { "SDR" },
                1e6 / period_us.max(1) as f64,
                self.frames,
                self.play_sum_us as f64 / self.frames as f64 / 1000.0,
                self.play_max_us as f64 / 1000.0,
                if self.unknown > 0 && known > 0 { format!(" ; {} sans file", self.unknown) } else { String::new() },
            )
        });
        *self = Self::new(now);
        line
    }
}

/// La latence d'un bilan : moyennes des médianes de chaque seconde.
#[derive(Default)]
pub struct Chain {
    seconds: u32,
    latency: f64,
    latency_n: u32,
    /// Capture, encodage, envoi, réseau, décodage, affichage (ms).
    stages: [f64; 6],
    stages_n: u32,
}

impl Chain {
    /// Une seconde de mesures ; rend la ligne du bilan toutes les 5 secondes.
    pub fn add(&mut self, latency_ms: Option<f64>, stages: Option<[f64; 6]>, hdr: bool) -> Option<String> {
        self.seconds += 1;
        if let Some(l) = latency_ms {
            self.latency += l;
            self.latency_n += 1;
        }
        if let Some(s) = stages {
            for (a, b) in self.stages.iter_mut().zip(s) {
                *a += b;
            }
            self.stages_n += 1;
        }
        if self.seconds < WINDOW.as_secs() as u32 {
            return None;
        }
        let line = (self.latency_n > 0).then(|| {
            let mut line = format!("latence sur {} s ({}) : {:.1} ms", self.seconds, if hdr { "HDR" } else { "SDR" }, self.latency / self.latency_n as f64);
            if self.stages_n > 0 {
                let s = self.stages.map(|v| v / self.stages_n as f64);
                line += &format!(
                    " (capture {:.1}, encodage {:.1}, envoi {:.1}, réseau {:.1}, décodage {:.1}, affichage {:.1})",
                    s[0], s[1], s[2], s[3], s[4], s[5]
                );
            }
            line
        });
        *self = Self::default();
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_window() {
        let t = Instant::now();
        let mut q = Queue::new(t);
        assert!(q.take(t, false, 8333).is_none());
        q.add(Some(1), Duration::from_micros(300));
        q.add(Some(2), Duration::from_micros(700));
        q.add(None, Duration::from_micros(500));
        assert!(!q.due(t + Duration::from_secs(4)));
        assert!(q.due(t + WINDOW));
        let line = q.take(t + WINDOW, true, 8333).unwrap();
        assert!(line.contains("(HDR) : moyenne 1.50, max 2"), "{line}");
        assert!(line.contains("période 8333 µs (120 Hz)"), "{line}");
        assert!(line.contains("3 images, appel moyen 0.50 ms, max 0.70 ms ; 1 sans file"), "{line}");
        assert!(q.take(t + WINDOW, true, 8333).is_none());
    }

    #[test]
    fn chain_window() {
        let mut c = Chain::default();
        for _ in 0..4 {
            assert!(c.add(Some(30.0), Some([1.0, 4.0, 0.5, 2.0, 6.0, 16.0]), false).is_none());
        }
        let line = c.add(Some(40.0), Some([1.0, 4.0, 0.5, 2.0, 11.0, 16.0]), false).unwrap();
        assert!(line.starts_with("latence sur 5 s (SDR) : 32.0 ms (capture 1.0, encodage 4.0, envoi 0.5, réseau 2.0, décodage 7.0, affichage 16.0)"), "{line}");
        // Sans mesure : rien.
        for _ in 0..5 {
            assert!(c.add(None, None, true).is_none());
        }
    }
}
