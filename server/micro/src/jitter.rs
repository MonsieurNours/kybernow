//! Le tampon de gigue : les trames décodées attendent la sortie du son, qui
//! les tire à son rythme.
//!
//! 20 ms visés avant de commencer (ou de reprendre après un trou), 60 ms au
//! plus : au-delà, les plus anciennes sont jetées jusqu'à 20 ms (l'appareil
//! envoie un peu plus vite que l'ordinateur ne joue, ou un paquet bloqué est
//! arrivé d'un coup). Rien à jouer : du silence, et l'attente des 20 ms.

use std::collections::VecDeque;

use crate::RATE;

/// Échantillons visés avant de jouer : 20 ms.
pub const TARGET: usize = RATE as usize / 50;
/// Au plus 60 ms en attente.
pub const MAX: usize = 3 * TARGET;

#[derive(Default)]
pub struct Jitter {
    queue: VecDeque<f32>,
    /// On attend `TARGET` avant de jouer.
    priming: bool,
    /// Échantillons jetés (trop de retard) et manqués (rien à jouer), pour le journal.
    pub dropped: u64,
    pub starved: u64,
}

impl Jitter {
    pub fn new() -> Self {
        Self {
            priming: true,
            ..Default::default()
        }
    }

    pub fn push(&mut self, pcm: &[f32]) {
        self.queue.extend(pcm.iter().copied());
        if self.queue.len() > MAX {
            let extra = self.queue.len() - TARGET;
            self.queue.drain(..extra);
            self.dropped += extra as u64;
        }
    }

    /// L'appareil a coupé son micro : plus rien à jouer, tout de suite.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.priming = true;
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Remplit `out` : le son en attente, sinon du silence.
    pub fn pull(&mut self, out: &mut [f32]) {
        if self.priming && self.queue.len() < TARGET {
            out.fill(0.0);
            return;
        }
        self.priming = false;
        let n = out.len().min(self.queue.len());
        for (o, s) in out.iter_mut().zip(self.queue.drain(..n)) {
            *o = s;
        }
        if n < out.len() {
            out[n..].fill(0.0);
            self.starved += (out.len() - n) as u64;
            self.priming = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attend_20_ms_avant_de_jouer() {
        let mut j = Jitter::new();
        j.push(&[0.5; 480]);
        let mut out = [1.0; 480];
        j.pull(&mut out);
        assert!(out.iter().all(|&s| s == 0.0), "10 ms seulement : silence");
        j.push(&[0.5; 480]);
        j.pull(&mut out);
        assert!(out.iter().all(|&s| s == 0.5));
    }

    #[test]
    fn jamais_plus_de_60_ms() {
        let mut j = Jitter::new();
        for _ in 0..10 {
            j.push(&[0.1; 480]);
        }
        assert!(j.len() <= MAX);
        assert!(j.dropped > 0, "le retard est jeté");
    }

    #[test]
    fn un_trou_donne_du_silence_puis_on_rattend() {
        let mut j = Jitter::new();
        j.push(&[0.2; 960]);
        let mut out = [1.0; 720];
        j.pull(&mut out);
        j.pull(&mut out);
        assert_eq!(&out[240..], &[0.0; 480][..]);
        assert_eq!(j.starved, 480);
        j.push(&[0.3; 480]);
        j.pull(&mut out[..480]);
        assert!(out[..480].iter().all(|&s| s == 0.0), "on rattend 20 ms");
    }

    #[test]
    fn coupe_tout_de_suite() {
        let mut j = Jitter::new();
        j.push(&[0.2; 1440]);
        j.clear();
        let mut out = [1.0; 480];
        j.pull(&mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}
