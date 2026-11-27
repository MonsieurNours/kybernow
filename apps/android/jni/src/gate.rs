//! La surface retirée, attendue (AND-04) : Android veut que plus personne
//! n'écrive dans la surface d'une `SurfaceView` une fois `surfaceDestroyed`
//! rendu. Chaque changement de surface prend un numéro ; le fil de l'image,
//! qui lit ce numéro avant chaque message, lâche le décodeur (MediaCodec et
//! son fil d'affichage, ou la swapchain de PyroWave) puis dit « rendu » pour
//! ce numéro. Le fil principal, lui, attend ce « rendu » un court instant
//! avant de rendre la main à Android.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Attente au plus, sur le fil principal : une image en cours de décodage
/// (40 ms au plus pour MediaCodec, une présentation Vulkan) a le temps de
/// finir ; au-delà, Android aurait de toute façon repris la surface.
pub const RELEASE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Default)]
pub struct SurfaceGate {
    generation: AtomicU64,
    released: Mutex<u64>,
    cv: Condvar,
}

impl SurfaceGate {
    /// Un changement de surface : son numéro.
    pub fn bump(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Le fil de l'image ne touche plus la surface d'avant `generation`.
    pub fn released(&self, generation: u64) {
        let mut r = self.released.lock().unwrap_or_else(|e| e.into_inner());
        if generation > *r {
            *r = generation;
        }
        self.cv.notify_all();
    }

    /// Attend que le fil de l'image ait lâché la surface `generation` ;
    /// `false` si le délai est passé.
    pub fn wait_released(&self, generation: u64, timeout: Duration) -> bool {
        let r = self.released.lock().unwrap_or_else(|e| e.into_inner());
        let (r, _) = self.cv.wait_timeout_while(r, timeout, |r| *r < generation).unwrap_or_else(|e| e.into_inner());
        *r >= generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    #[test]
    fn waits_for_the_image_thread() {
        let gate = Arc::new(SurfaceGate::default());
        let g = gate.bump();
        let worker = {
            let gate = gate.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(30));
                gate.released(gate.generation());
            })
        };
        let t = Instant::now();
        assert!(gate.wait_released(g, Duration::from_secs(2)));
        assert!(t.elapsed() >= Duration::from_millis(25));
        worker.join().unwrap();
    }

    #[test]
    fn gives_up_after_the_timeout() {
        let gate = SurfaceGate::default();
        let g = gate.bump();
        let t = Instant::now();
        assert!(!gate.wait_released(g, Duration::from_millis(40)));
        assert!(t.elapsed() >= Duration::from_millis(35));
    }

    #[test]
    fn a_later_release_covers_earlier_changes() {
        let gate = SurfaceGate::default();
        let a = gate.bump();
        let b = gate.bump();
        gate.released(b);
        assert!(gate.wait_released(a, Duration::ZERO));
        assert!(gate.wait_released(b, Duration::ZERO));
        // Un « rendu » plus ancien ne recule pas.
        gate.released(a);
        assert!(gate.wait_released(b, Duration::ZERO));
        assert!(!gate.wait_released(gate.bump(), Duration::ZERO));
    }
}
