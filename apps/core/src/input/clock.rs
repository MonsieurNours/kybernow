//! Horloge monotone fournie par la plateforme. Les modules d'entrées et le
//! son ne lisent jamais l'heure eux-mêmes : une horloge simulée
//! ([`ManualClock`]) les fait tourner dans les tests sans attendre.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Temps écoulé depuis un instant fixe (le démarrage de l'app, par exemple),
/// qui ne recule jamais et n'avance pas pendant la veille de préférence
/// (`CLOCK_UPTIME_RAW` sur Apple, `QueryPerformanceCounter` sur Windows).
pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
}

/// Horloge de la bibliothèque standard (`Instant`), suffisante pour les apps.
pub struct MonotonicClock(Instant);

impl MonotonicClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MonotonicClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
}

/// Horloge avancée à la main (tests, bancs d'essai).
pub struct ManualClock(Mutex<Duration>);

impl ManualClock {
    pub fn new() -> Self {
        Self(Mutex::new(Duration::ZERO))
    }

    pub fn starting_at(at: Duration) -> Self {
        Self(Mutex::new(at))
    }

    pub fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }

    pub fn set(&self, at: Duration) {
        *self.0.lock().unwrap() = at;
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }
}
