//! Courbes du HDR (WINAPP-49, docs/hdr.md) : une seule écriture de la PQ, de
//! l'EETF de BT.2390, de la courbe vers le SDR et de la matrice YUV, pour que
//! le même flux rende pareil sur toutes les apps. Les shaders (HLSL, GLSL,
//! Metal) les recopient : chaque app vérifie les siens contre ces fonctions
//! par un essai croisé.

/// Constantes de la PQ (SMPTE ST 2084), écrites comme dans la norme.
pub const PQ_M1: f64 = 2610.0 / 16384.0;
pub const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
pub const PQ_C1: f64 = 3424.0 / 4096.0;
pub const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
pub const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

/// Luminance (nits) d'un signal PQ de 0 à 1.
pub fn pq_to_nits(e: f64) -> f64 {
    let p = e.clamp(0.0, 1.0).powf(1.0 / PQ_M2);
    10_000.0 * ((p - PQ_C1).max(0.0) / (PQ_C2 - PQ_C3 * p)).powf(1.0 / PQ_M1)
}

/// Signal PQ (0 à 1) d'une luminance en nits (écrêtée à 10 000) ; 0 pour le
/// noir (la formule y donne 7·10⁻⁷).
pub fn nits_to_pq(nits: f64) -> f64 {
    if nits <= 0.0 {
        return 0.0;
    }
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(PQ_M1);
    ((PQ_C1 + PQ_C2 * y) / (1.0 + PQ_C3 * y)).powf(PQ_M2)
}

/// Primaires BT.2020 vers BT.709, en lumière linéaire (lignes : R, G, B).
pub const BT2020_TO_709: [[f64; 3]; 3] = [
    [1.660491, -0.587641, -0.072850],
    [-0.124550, 1.132900, -0.008349],
    [-0.018151, -0.100579, 1.118730],
];

/// L'EETF de BT.2390, dans le domaine PQ : `e` (0 à 1) d'une image dont le
/// plus clair vaut `src_hi` (PQ), ramené entre `dst_lo` et `dst_hi` (PQ du
/// noir et du plus clair de l'écran). Genou de Hermite, puis relèvement du
/// noir `dst_lo·(1−E)⁴` de la norme. Rien à ramener (`dst_hi ≥ src_hi`,
/// noir nul) : `e` tel quel.
pub fn eetf(e: f64, src_hi: f64, dst_lo: f64, dst_hi: f64) -> f64 {
    if src_hi <= 0.0 {
        return e;
    }
    let e1 = (e / src_hi).clamp(0.0, 1.0);
    let max_lum = (dst_hi / src_hi).min(1.0);
    let min_lum = (dst_lo / src_hi).clamp(0.0, 1.0);
    let ks = (1.5 * max_lum - 0.5).max(0.0);
    let e2 = if e1 < ks || ks >= 1.0 {
        e1
    } else {
        let t = (e1 - ks) / (1.0 - ks);
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * ks + (t3 - 2.0 * t2 + t) * (1.0 - ks) + (-2.0 * t3 + 3.0 * t2) * max_lum
    };
    let e3 = e2 + min_lum * (1.0 - e2).powi(4);
    e3 * src_hi
}

/// Genou de la courbe vers le SDR, en unités du blanc SDR.
pub const SDR_KNEE: f64 = 0.75;
/// Ce que la courbe ramène au blanc : 4 fois le blanc SDR.
pub const SDR_RANGE: f64 = 4.0;
/// La pente de la compression au-dessus du genou (3,6923077 dans les
/// shaders, et dans ceux de l'image à part en SDR de la Station, correctifs
/// 0022) : pente 1 au genou, [`SDR_RANGE`] ramené à 1.
pub const SDR_SLOPE: f64 = ((SDR_RANGE - SDR_KNEE) / (1.0 - SDR_KNEE) - 1.0) / (SDR_RANGE - SDR_KNEE);

/// La courbe vers le SDR (écran SDR, image HDR), sur la plus grande
/// composante `m` en unités du blanc SDR : rien sous le genou (0,75), puis
/// une compression de pente 1 au genou qui ramène 4 fois le blanc à 1 — la
/// courbe de l'image à part en SDR de la Station (correctifs 0022), pour
/// qu'un écran SDR voie la même chose d'où qu'elle vienne.
pub fn sdr_curve(m: f64) -> f64 {
    if m <= SDR_KNEE {
        return m;
    }
    let x = m - SDR_KNEE;
    (SDR_KNEE + x / (1.0 + SDR_SLOPE * x)).min(1.0)
}

/// Une image arrive en PQ : courbe de transfert SMPTE ST 2084 (16), ou non
/// dite (2) en 10 bits d'un flux annoncé HDR.
pub fn frame_is_pq(trc: i32, ten_bit: bool, stream_hdr: bool) -> bool {
    const SMPTE2084: i32 = 16;
    const UNSPECIFIED: i32 = 2;
    trc == SMPTE2084 || (trc == UNSPECIFIED && ten_bit && stream_hdr)
}

/// Le plus clair d'une image HDR quand rien d'autre n'en dit (nits).
pub const DEFAULT_PEAK_NITS: f64 = 1000.0;

/// Le plus clair du contenu (nits) : MaxCLL, sinon le maximum de l'écran de
/// mastering, sinon [`DEFAULT_PEAK_NITS`] ; entre 100 et 10 000.
pub fn content_peak(max_cll: Option<u32>, mastering_max: Option<f64>) -> f64 {
    let peak = match (max_cll.filter(|v| *v > 0), mastering_max.filter(|v| v.is_finite() && *v > 0.0)) {
        (Some(cll), _) => f64::from(cll),
        (None, Some(m)) => m,
        (None, None) => DEFAULT_PEAK_NITS,
    };
    peak.clamp(100.0, 10_000.0)
}

/// Ce que fait le rendu d'une image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Image SDR.
    Sdr,
    /// Image HDR vers un écran HDR (PQ).
    Pq,
    /// Image HDR vers un écran SDR ([`sdr_curve`]).
    PqToSdr,
}

impl Output {
    pub fn of(frame_pq: bool, screen_hdr: bool) -> Self {
        match (frame_pq, screen_hdr) {
            (false, _) => Self::Sdr,
            (true, true) => Self::Pq,
            (true, false) => Self::PqToSdr,
        }
    }

    /// Le code des shaders : 0, 1, 2.
    pub fn shader(self) -> u32 {
        match self {
            Self::Sdr => 0,
            Self::Pq => 1,
            Self::PqToSdr => 2,
        }
    }
}

/// Un contenu plus clair que l'écran de moins de 2 % n'est pas ramené.
pub const TONE_MARGIN: f64 = 1.02;

/// Les valeurs du rendu d'une image HDR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    /// [`Output::shader`].
    pub out_mode: u32,
    /// Ramener le contenu à l'écran ([`eetf`]) : seulement vers un écran HDR
    /// moins lumineux que lui.
    pub tone: bool,
    /// Le plus clair du contenu, en PQ.
    pub src_pq: f64,
    /// Le plus clair et le noir de l'écran, en PQ.
    pub dst_pq: f64,
    pub dst_lo_pq: f64,
    /// Le blanc SDR de l'écran (nits) : l'unité de [`sdr_curve`].
    pub sdr_white: f64,
}

/// Les valeurs du rendu : `peak` le plus clair du contenu
/// ([`content_peak`]), `screen_max`, `screen_min` et `sdr_white` les
/// valeurs de l'écran (nits).
pub fn tone(output: Output, peak: f64, screen_max: f64, screen_min: f64, sdr_white: f64) -> Tone {
    Tone {
        out_mode: output.shader(),
        tone: output == Output::Pq && peak > screen_max * TONE_MARGIN,
        src_pq: nits_to_pq(peak),
        dst_pq: nits_to_pq(screen_max),
        dst_lo_pq: nits_to_pq(screen_min),
        sdr_white,
    }
}

/// La matrice YUV → RGB d'une image, en 3 lignes de 4 (la quatrième colonne
/// est le décalage), pour des valeurs lues en `unit` par code (1/255 en
/// 8 bits normalisés, 64/65535 en P010…) : `colorspace` celui de FFmpeg
/// (5, 6 : BT.601 ; 9, 10 : BT.2020 ; sinon BT.709), plage pleine ou réduite,
/// `bits` 8 ou 10.
pub fn yuv_matrix(colorspace: i32, full_range: bool, bits: u32, unit: f32) -> [[f32; 4]; 3] {
    let (r_cr, g_cb, g_cr, b_cb) = match colorspace {
        5 | 6 => (1.402, -0.344_136, -0.714_136, 1.772),
        9 | 10 => (1.4746, -0.164_553, -0.571_353, 1.8814),
        _ => (1.5748, -0.187_324, -0.468_124, 1.8556),
    };
    let bits = bits.clamp(8, 16);
    let shift = bits - 8;
    let max_code = ((1u32 << bits) - 1) as f32;
    let (y_off, y_range, c_range) = if full_range {
        (0.0, max_code, max_code)
    } else {
        ((16u32 << shift) as f32, (219u32 << shift) as f32, (224u32 << shift) as f32)
    };
    let c_off = (1u32 << (bits - 1)) as f32;
    let (sy, oy, sc, oc) = (1.0 / (y_range * unit), y_off * unit, 1.0 / (c_range * unit), c_off * unit);
    let row = |a: f32, b: f32, c: f32| -> [f32; 4] { [a * sy, b * sc, c * sc, -(a * sy * oy + b * sc * oc + c * sc * oc)] };
    [row(1.0, 0.0, r_cr), row(1.0, g_cb, g_cr), row(1.0, b_cb, 0.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn pq_aller_retour() {
        assert!(near(nits_to_pq(100.0), 0.508, 0.001));
        assert!(near(nits_to_pq(1000.0), 0.752, 0.001));
        assert_eq!(nits_to_pq(0.0), 0.0);
        assert!(near(nits_to_pq(10_000.0), 1.0, 1e-9));
        for nits in [0.05, 1.0, 80.0, 203.0, 600.0, 4000.0] {
            assert!(near(pq_to_nits(nits_to_pq(nits)), nits, nits * 1e-6), "{nits}");
        }
        // Les mêmes constantes que les shaders.
        assert!(near(PQ_M1, 0.1593017578125, 1e-12) && near(PQ_M2, 78.84375, 1e-12));
        assert!(near(PQ_C1, 0.8359375, 1e-12) && near(PQ_C2, 18.8515625, 1e-12) && near(PQ_C3, 18.6875, 1e-12));
    }

    #[test]
    fn courbe_vers_le_sdr() {
        assert!(near(SDR_SLOPE, 3.6923077, 1e-7), "la constante des shaders et de la Station");
        assert_eq!(sdr_curve(0.5), 0.5);
        assert_eq!(sdr_curve(SDR_KNEE), SDR_KNEE);
        assert!(near(sdr_curve(SDR_RANGE), 1.0, 1e-12));
        assert_eq!(sdr_curve(20.0), 1.0);
        // Pente 1 au genou, croissante.
        assert!(near((sdr_curve(SDR_KNEE + 1e-6) - SDR_KNEE) / 1e-6, 1.0, 1e-4));
        let mut last = 0.0;
        for i in 0..400 {
            let v = sdr_curve(i as f64 * 0.01);
            assert!(v >= last);
            last = v;
        }
    }

    #[test]
    fn eetf_de_bt2390() {
        let (src, hi) = (nits_to_pq(4000.0), nits_to_pq(600.0));
        // Sous le genou : inchangé ; le plus clair ramené à l'écran.
        assert!(near(eetf(0.3, src, 0.0, hi), 0.3, 1e-9));
        assert!(near(eetf(src, src, 0.0, hi), hi, 1e-9));
        assert!(eetf(0.9 * src, src, 0.0, hi) <= hi);
        // Écran plus lumineux que le contenu, noir nul : rien à faire.
        assert_eq!(eetf(0.4, nits_to_pq(400.0), 0.0, nits_to_pq(1000.0)), 0.4);
        // Relèvement du noir de la norme.
        let lo = nits_to_pq(0.1);
        assert!(near(eetf(0.0, src, lo, hi), lo, 1e-9));
        assert!(near(eetf(src, src, lo, hi), hi, 1e-3));
    }

    #[test]
    fn rendu_et_contenu() {
        assert_eq!(content_peak(Some(4000), Some(1000.0)), 4000.0);
        assert_eq!(content_peak(None, Some(600.0)), 600.0);
        assert_eq!(content_peak(None, None), DEFAULT_PEAK_NITS);
        assert_eq!(content_peak(Some(20), None), 100.0);
        assert!(frame_is_pq(16, false, false) && frame_is_pq(2, true, true) && !frame_is_pq(2, true, false) && !frame_is_pq(1, true, true));
        assert_eq!(Output::of(true, false), Output::PqToSdr);
        let t = tone(Output::Pq, 4000.0, 600.0, 0.05, 203.0);
        assert!(t.tone && t.out_mode == 1);
        assert!(!tone(Output::Pq, 610.0, 600.0, 0.05, 203.0).tone, "marge de 2 %");
        assert!(!tone(Output::PqToSdr, 4000.0, 600.0, 0.05, 203.0).tone);
    }

    #[test]
    fn matrice_yuv() {
        // BT.709, 8 bits, plage réduite : le noir (16, 128, 128) et le blanc (235, 128, 128).
        let m = yuv_matrix(1, false, 8, 1.0 / 255.0);
        let rgb = |y: f32, u: f32, v: f32| m.map(|r| r[0] * y / 255.0 + r[1] * u / 255.0 + r[2] * v / 255.0 + r[3]);
        for c in rgb(16.0, 128.0, 128.0) {
            assert!(c.abs() < 1e-5);
        }
        for c in rgb(235.0, 128.0, 128.0) {
            assert!((c - 1.0).abs() < 1e-5);
        }
        // Rouge pur de BT.709 (plage pleine) : R = Y + 1,5748·Cr.
        let f = yuv_matrix(1, true, 8, 1.0);
        assert!((f[0][2] - 1.5748 / 255.0).abs() < 1e-7);
    }
}
