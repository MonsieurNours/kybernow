//! Mixage descendant et ordre des voies.
//!
//! Ordre des voies d'Opus (celui de Vorbis) : G, C, D, surround G, surround
//! D, LFE pour le 5.1 ; G, C, D, latéral G, latéral D, arrière G, arrière D,
//! LFE pour le 7.1. Chaque sortie a le sien : Windows (WAVEFORMATEXTENSIBLE :
//! G, D, C, LFE, arrière G, arrière D, latéral G, latéral D), Core Audio
//! (MPEG 5.1 A et 7.1 C : G, D, C, LFE, surround G, surround D, arrière G,
//! arrière D ; les latérales d'Opus y sont les surrounds, ses arrières les
//! arrières).

/// Dans quel ordre la sortie attend ses voies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputOrder {
    /// Celui d'Opus (Vorbis), tel quel.
    Opus,
    /// WAVEFORMATEXTENSIBLE (WASAPI).
    Windows,
    /// Dispositions MPEG 5.1 A et 7.1 C de Core Audio.
    CoreAudio,
    /// Pour chaque voie de la sortie, l'index de la voie Opus qui y va.
    Custom(Vec<usize>),
}

impl OutputOrder {
    /// Pour chaque voie de la sortie (`channels` voies, ordre Opus en
    /// entrée), l'index de la voie Opus à y mettre. Identité quand l'ordre
    /// ne définit rien pour ce nombre de voies.
    pub fn permutation(&self, channels: usize) -> Vec<usize> {
        let identity = || (0..channels).collect();
        match self {
            Self::Opus => identity(),
            Self::Windows => match channels {
                3 => vec![0, 2, 1],
                6 => vec![0, 2, 1, 5, 3, 4],
                8 => vec![0, 2, 1, 7, 5, 6, 3, 4],
                _ => identity(),
            },
            Self::CoreAudio => match channels {
                3 => vec![0, 2, 1],
                6 => vec![0, 2, 1, 5, 3, 4],
                8 => vec![0, 2, 1, 7, 3, 4, 5, 6],
                _ => identity(),
            },
            Self::Custom(p) if p.len() == channels && p.iter().all(|&i| i < channels) => p.clone(),
            Self::Custom(_) => identity(),
        }
    }
}

/// Une image audio du flux (`src.len()` voies, ordre Opus) vers la sortie
/// (`dst.len()` voies, ordre Opus), au volume donné. Moins de voies :
/// G + 0,7 C + 0,7 surrounds, D + 0,7 C + 0,7 surrounds, LFE ignoré ;
/// mono = moyenne ; plus de voies : les voies du flux, le reste à zéro.
#[inline]
pub fn mix_frame(src: &[f32], dst: &mut [f32], volume: f32) {
    let (ch, out) = (src.len(), dst.len());
    if ch == out {
        for (d, s) in dst.iter_mut().zip(src) {
            *d = s * volume;
        }
        return;
    }
    let clamp = |v: f32| (v * volume).clamp(-1.0, 1.0);
    const HALF: f32 = std::f32::consts::FRAC_1_SQRT_2;
    match (ch, out) {
        (6, 2) => {
            dst[0] = clamp(src[0] + HALF * src[1] + HALF * src[3]);
            dst[1] = clamp(src[2] + HALF * src[1] + HALF * src[4]);
        }
        (8, 2) => {
            dst[0] = clamp(src[0] + HALF * src[1] + HALF * (src[3] + src[5]));
            dst[1] = clamp(src[2] + HALF * src[1] + HALF * (src[4] + src[6]));
        }
        (8, 6) => {
            dst[0] = src[0] * volume;
            dst[1] = src[1] * volume;
            dst[2] = src[2] * volume;
            dst[3] = clamp(src[3] + HALF * src[5]);
            dst[4] = clamp(src[4] + HALF * src[6]);
            dst[5] = src[7] * volume;
        }
        (_, 1) => {
            let mut left = src[0];
            let mut right = src[if ch > 1 { if ch >= 6 { 2 } else { 1 } } else { 0 }];
            if ch >= 6 {
                left += HALF * src[1] + HALF * src[3];
                right += HALF * src[1] + HALF * src[4];
            }
            dst[0] = clamp((left + right) / 2.0);
        }
        _ => {
            for (c, d) in dst.iter_mut().enumerate() {
                *d = if c < ch { src[c] * volume } else { 0.0 };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixage_descendant() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let mut out = [0.0; 2];
        mix_frame(&[0.1, 0.2, 0.3, 0.4, 0.5, 1.0], &mut out, 1.0);
        assert!((out[0] - (0.1 + h * 0.2 + h * 0.4)).abs() < 1e-6, "G + 0,7 C + 0,7 surround G, LFE ignoré");
        assert!((out[1] - (0.3 + h * 0.2 + h * 0.5)).abs() < 1e-6);
        mix_frame(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0], &mut out, 1.0);
        assert!((out[0] - (0.1 + h * 0.2 + h * (0.4 + 0.6))).abs() < 1e-6, "7.1 : latérale et arrière gauches");
        let mut six = [0.0; 6];
        mix_frame(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8], &mut six, 1.0);
        assert!((six[3] - (0.4 + h * 0.6)).abs() < 1e-6 && six[5] == 0.8, "7.1 → 5.1 : arrières repliées sur les surrounds, LFE gardé");
        let mut mono = [0.0; 1];
        mix_frame(&[0.5, -0.5], &mut mono, 1.0);
        assert_eq!(mono, [0.0], "mono = moyenne");
        let mut same = [0.0; 2];
        mix_frame(&[0.5, -0.5], &mut same, 0.5);
        assert_eq!(same, [0.25, -0.25], "mêmes voies : volume seulement");
        let mut more = [1.0; 6];
        mix_frame(&[0.5, -0.5], &mut more, 1.0);
        assert_eq!(more, [0.5, -0.5, 0.0, 0.0, 0.0, 0.0], "plus de voies : le reste à zéro");
        mix_frame(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0], &mut out, 1.0);
        assert_eq!(out, [1.0, 1.0], "somme bornée");
    }

    #[test]
    fn ordre_des_voies() {
        assert_eq!(OutputOrder::Windows.permutation(6), [0, 2, 1, 5, 3, 4]);
        assert_eq!(OutputOrder::Windows.permutation(8), [0, 2, 1, 7, 5, 6, 3, 4]);
        assert_eq!(OutputOrder::CoreAudio.permutation(8), [0, 2, 1, 7, 3, 4, 5, 6]);
        assert_eq!(OutputOrder::CoreAudio.permutation(2), [0, 1]);
        assert_eq!(OutputOrder::Opus.permutation(6), [0, 1, 2, 3, 4, 5]);
        assert_eq!(OutputOrder::Custom(vec![1, 0]).permutation(2), [1, 0]);
        assert_eq!(OutputOrder::Custom(vec![1, 0]).permutation(6), [0, 1, 2, 3, 4, 5], "permutation inadaptée : identité");
    }
}
