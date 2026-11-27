//! Règles du chemin de l'image, sans FFmpeg ni SDL (essayées sur le Mac) : la file vers le décodeur
//! bornée, la matrice et la plage des couleurs qu'on donne à SDL.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Paquets en attente du décodeur au plus (une demi-seconde à 60 i/s). Au-delà, le décodage ne suit plus :
/// les paquets sont jetés jusqu'à la prochaine image clé, demandée à l'ordinateur, plutôt que la latence et la
/// mémoire ne montent sans fin (décodage par le processeur plus lent que le flux).
pub const BACKLOG_MAX: usize = 30;

/// Ce que devient un paquet qui arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admit {
    /// À la file du décodeur.
    Send,
    /// Jeté : la file est pleine, ou on attend l'image clé qui suit un trou.
    Drop,
    /// Le premier jeté d'une série : le décodeur doit savoir qu'il manque des images, et une image clé est demandée.
    DropFirst,
}

/// La file vers le décodeur, comptée des deux côtés (réception, fil du décodeur).
#[derive(Default)]
pub struct Backlog {
    pending: AtomicUsize,
    skipping: AtomicBool,
}

impl Backlog {
    /// Un paquet d'image arrive (`key` : image clé, ou image qui se décode seule).
    pub fn admit(&self, key: bool) -> Admit {
        if self.pending.load(Ordering::Acquire) >= BACKLOG_MAX {
            return if self.skipping.swap(true, Ordering::AcqRel) { Admit::Drop } else { Admit::DropFirst };
        }
        if self.skipping.load(Ordering::Acquire) {
            if !key {
                return Admit::Drop;
            }
            self.skipping.store(false, Ordering::Release);
        }
        self.pending.fetch_add(1, Ordering::AcqRel);
        Admit::Send
    }

    /// Le fil du décodeur a pris un paquet.
    pub fn taken(&self) {
        let _ = self.pending.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
    }

    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }
}

/// La conversion YUV → RGB de SDL pour une image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yuv {
    /// BT.601, plage réduite.
    Bt601,
    /// BT.709, plage réduite.
    Bt709,
    /// BT.601, plage pleine (« JPEG » de SDL).
    Jpeg,
}

/// SDL 2 n'a pas de BT.709 en plage pleine : une telle image (PyroWave SDR, docs/pyrowave.md) est d'abord
/// ramenée en plage réduite ([`narrow_luma`], [`narrow_chroma`]), puis montrée en BT.709. Le BT.601 plein
/// (« JPEG ») ne vaut que pour une image BT.601.
pub fn yuv_for(bt709: bool, full_range: bool) -> Yuv {
    match (bt709, full_range) {
        (true, _) => Yuv::Bt709,
        (false, true) => Yuv::Jpeg,
        (false, false) => Yuv::Bt601,
    }
}

/// L'image doit-elle passer en plage réduite avant SDL ?
pub fn needs_narrowing(bt709: bool, full_range: bool) -> bool {
    bt709 && full_range
}

fn lut(lo: f32, span: f32, centered: bool) -> [u8; 256] {
    let mut t = [0u8; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let x = if centered { 128.0 + (i as f32 - 128.0) * span / 255.0 } else { lo + i as f32 * span / 255.0 };
        *v = x.round().clamp(0.0, 255.0) as u8;
    }
    t
}

/// Luma 8 bits, plage pleine (0-255) → plage réduite (16-235).
pub fn narrow_luma(p: &mut [u8]) {
    let t = lut(16.0, 219.0, false);
    for b in p {
        *b = t[*b as usize];
    }
}

/// Chroma 8 bits, plage pleine (0-255, centre 128) → plage réduite (16-240, centre 128).
pub fn narrow_chroma(p: &mut [u8]) {
    let t = lut(0.0, 224.0, true);
    for b in p {
        *b = t[*b as usize];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_bornee_jusqu_a_l_image_cle() {
        let b = Backlog::default();
        for _ in 0..BACKLOG_MAX {
            assert_eq!(b.admit(false), Admit::Send);
        }
        assert_eq!(b.admit(false), Admit::DropFirst, "file pleine : le premier jeté le dit");
        assert_eq!(b.admit(true), Admit::Drop, "toujours pleine : même une image clé attend");
        for _ in 0..BACKLOG_MAX {
            b.taken();
        }
        assert_eq!(b.pending(), 0);
        assert_eq!(b.admit(false), Admit::Drop, "vidée, mais l'image d'après dépend d'images jetées");
        assert_eq!(b.admit(true), Admit::Send, "l'image clé repart");
        assert_eq!(b.admit(false), Admit::Send);
        b.taken();
        b.taken();
        b.taken();
        assert_eq!(b.pending(), 0, "jamais sous zéro");
    }

    #[test]
    fn matrice_et_plage() {
        assert_eq!(yuv_for(true, false), Yuv::Bt709);
        assert_eq!(yuv_for(true, true), Yuv::Bt709, "BT.709 plein : ramené en plage réduite, pas en JPEG (BT.601)");
        assert_eq!(yuv_for(false, true), Yuv::Jpeg);
        assert_eq!(yuv_for(false, false), Yuv::Bt601);
        assert!(needs_narrowing(true, true) && !needs_narrowing(true, false) && !needs_narrowing(false, true));
        let mut y = [0u8, 255, 128];
        narrow_luma(&mut y);
        assert_eq!(y, [16, 235, 126]);
        let mut c = [0u8, 255, 128];
        narrow_chroma(&mut c);
        assert_eq!(c, [16, 240, 128]);
    }
}
