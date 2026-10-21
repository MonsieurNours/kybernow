//! Anneau d'échantillons à un producteur (le fil réseau de Kyber, qui
//! décode) et un consommateur (le fil temps réel de la carte son), sans
//! verrou : chacun n'écrit que son index, et lit celui de l'autre.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Ring {
    buf: Box<[Cell<f32>]>,
    /// Compteurs qui ne reviennent jamais en arrière ; la position dans
    /// l'anneau est le reste modulo la capacité.
    write: AtomicUsize,
    read: AtomicUsize,
}

// Les cellules ne sont touchées que selon le protocole producteur /
// consommateur ci-dessus : jamais la même par deux fils à la fois.
unsafe impl Sync for Ring {}
unsafe impl Send for Ring {}

impl Ring {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: (0..capacity).map(|_| Cell::new(0.0)).collect(),
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Échantillons en attente (lisible des deux côtés).
    pub fn len(&self) -> usize {
        self.write.load(Ordering::Acquire).wrapping_sub(self.read.load(Ordering::Acquire))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Producteur seul : ajoute ce qui tient ; rend le nombre écrit (le reste
    /// est jeté : la sortie ne tire plus depuis une seconde).
    pub fn push(&self, samples: &[f32]) -> usize {
        let cap = self.buf.len();
        let write = self.write.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        let free = cap - write.wrapping_sub(read);
        let n = samples.len().min(free);
        for (i, &s) in samples[..n].iter().enumerate() {
            self.buf[(write + i) % cap].set(s);
        }
        self.write.store(write.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consommateur seul : l'image audio (`out.len()` échantillons) au début
    /// de l'anneau, sans avancer. `false` : il n'y en a pas assez.
    pub fn peek(&self, out: &mut [f32]) -> bool {
        let cap = self.buf.len();
        let read = self.read.load(Ordering::Relaxed);
        if self.write.load(Ordering::Acquire).wrapping_sub(read) < out.len() {
            return false;
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.buf[(read + i) % cap].get();
        }
        true
    }

    /// Consommateur seul : avance de `n` échantillons (au plus ce qui attend).
    pub fn advance(&self, n: usize) -> usize {
        let read = self.read.load(Ordering::Relaxed);
        let n = n.min(self.write.load(Ordering::Acquire).wrapping_sub(read));
        self.read.store(read.wrapping_add(n), Ordering::Release);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anneau() {
        let ring = Ring::new(6);
        assert_eq!(ring.push(&[1.0, 2.0, 3.0, 4.0]), 4);
        assert_eq!(ring.len(), 4);
        let mut frame = [0.0; 2];
        assert!(ring.peek(&mut frame) && frame == [1.0, 2.0]);
        assert!(ring.peek(&mut frame) && frame == [1.0, 2.0], "peek n'avance pas");
        assert_eq!(ring.advance(2), 2);
        assert_eq!(ring.push(&[5.0, 6.0, 7.0, 8.0, 9.0]), 4, "plein : le reste est jeté");
        assert_eq!(ring.len(), 6);
        assert!(ring.peek(&mut frame) && frame == [3.0, 4.0]);
        assert_eq!(ring.advance(10), 6, "au plus ce qui attend");
        assert!(ring.is_empty());
        assert!(!ring.peek(&mut frame));
        // Passage par le bord.
        assert_eq!(ring.push(&[1.0, 2.0, 3.0, 4.0, 5.0]), 5);
        ring.advance(4);
        assert_eq!(ring.push(&[6.0, 7.0]), 2);
        let mut three = [0.0; 3];
        assert!(ring.peek(&mut three) && three == [5.0, 6.0, 7.0]);
    }
}
