//! Le HDR de l'app Linux (docs/hdr.md), ce qui se calcule sans écran ni carte
//! graphique (testé sur le Mac aussi) :
//!
//! - les courbes : PQ (SMPTE ST 2084), primaires BT.2020 vers BT.709, la courbe
//!   vers le SDR (genou à 75 % du blanc, compression jusqu'à 4 fois le blanc :
//!   celle de l'image à part en SDR de la Station) et l'EETF de BT.2390 vers un
//!   écran HDR plus faible que le flux ;
//! - le rendu d'une image PQ (`render_for`) : telle quelle, ramenée à la crête
//!   de l'écran, ou vers le SDR (écran sans HDR : fenêtre glissée sur un écran
//!   SDR, HDR coupé) ;
//! - l'écran d'après la gestion des couleurs de Wayland (`WlDescription`) et le
//!   choix de SDL sur Wayland (`want_wayland`).
//!
//! Le shader de `src/native/hdr_gl.c` fait les mêmes calculs que `shade` (le
//! test `tests/hdr/run.sh` les compare sur la carte graphique).

/// Constantes de la PQ (SMPTE ST 2084).
const M1: f64 = 0.1593017578125;
const M2: f64 = 78.84375;
const C1: f64 = 0.8359375;
const C2: f64 = 18.8515625;
const C3: f64 = 18.6875;

/// Luminance (cd/m²) d'un signal PQ de 0 à 1.
pub fn pq_eotf(e: f64) -> f64 {
    let p = e.clamp(0.0, 1.0).powf(1.0 / M2);
    10_000.0 * ((p - C1).max(0.0) / (C2 - C3 * p)).powf(1.0 / M1)
}

/// Signal PQ (0 à 1) d'une luminance en cd/m² (écrêtée à 10 000) ; 0 pour le
/// noir (la formule y donne 7·10⁻⁷).
pub fn pq_oetf(nits: f64) -> f64 {
    if nits <= 0.0 {
        return 0.0;
    }
    let l = (nits / 10_000.0).clamp(0.0, 1.0).powf(M1);
    ((C1 + C2 * l) / (1.0 + C3 * l)).powf(M2)
}

/// Primaires BT.2020 vers BT.709, en lumière linéaire (lignes : R, G, B).
pub const BT2020_TO_709: [[f64; 3]; 3] = [[1.660491, -0.587641, -0.072850], [-0.124550, 1.132900, -0.008349], [-0.018151, -0.100579, 1.118730]];

/// La courbe vers le SDR, sur la plus grande composante `m` en unités du blanc
/// SDR : rien sous le genou (0,75), puis une compression de pente 1 au genou
/// qui ramène 4 fois le blanc à 1 (la courbe de l'image à part en SDR de la
/// Station, correctifs 0022).
pub fn sdr_knee(m: f64) -> f64 {
    if m <= 0.75 {
        return m;
    }
    let x = m - 0.75;
    0.75 + x / (1.0 + 3.692_307_7 * x)
}

/// L'EETF de BT.2390, dans le domaine PQ : `e` (signal PQ) d'un flux dont la
/// crête est `src_hi` (PQ, noir à 0) ramené à un écran de `dst_lo` à `dst_hi`
/// (PQ). Sous le genou (`ks`, 1,5 × crête de l'écran − 0,5, en part de la crête
/// du flux), rien ne change ; au-dessus, une spline d'Hermite rejoint la crête
/// de l'écran. Le noir de l'écran relève les ombres de `dst_lo × (1 − E)⁴`.
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

/// Les valeurs HDR d'un écran de l'appareil, en cd/m² (miroir de
/// `HdrLuminance` du cœur, que l'interface seule n'a pas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Luminance {
    pub max: f64,
    pub full_frame: f64,
    pub min: f64,
    pub sdr_white: f64,
}

impl Default for Luminance {
    /// Celles du cœur par défaut : 1 000 cd/m², 1 000 sur tout l'écran, noir à
    /// 0, blanc du bureau à 203 (BT.2408).
    fn default() -> Self {
        Self { max: 1000.0, full_frame: 1000.0, min: 0.0, sdr_white: 203.0 }
    }
}

/// `transfer_function.st2084_pq` de `wp_color_manager_v1`.
pub const WL_TF_PQ: u32 = 11;

/// Une description d'image de la gestion des couleurs de Wayland
/// (`wp_image_description_info_v1`) : celle qu'un écran préfère pour une
/// surface, ou celle d'une sortie. 0 : valeur absente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WlDescription {
    /// Fonction de transfert nommée (`tf_named`).
    pub tf: u32,
    /// `luminances` : minimum (× 10 000), maximum, blanc de référence.
    pub min_lum: u32,
    pub max_lum: u32,
    pub reference_lum: u32,
    /// `target_luminance` : minimum (× 10 000) et maximum ; `target_max_cll`, `target_max_fall`.
    pub target_min: u32,
    pub target_max: u32,
    pub max_cll: u32,
    pub max_fall: u32,
}

impl WlDescription {
    /// L'écran affiche le HDR : le compositeur lui préfère une image en PQ.
    pub fn is_hdr(&self) -> bool {
        self.tf == WL_TF_PQ
    }

    /// Ses valeurs : crête d'après la cible (`target_max_cll`, sinon le
    /// maximum de `target_luminance`), tout l'écran d'après `target_max_fall`,
    /// noir d'après le minimum de la cible, blanc du bureau d'après le blanc de
    /// référence ; ce qui manque prend la valeur par défaut du cœur.
    pub fn luminance(&self) -> Luminance {
        let d = Luminance::default();
        let max = if self.max_cll > 0 {
            f64::from(self.max_cll)
        } else if self.target_max > 0 {
            f64::from(self.target_max)
        } else {
            d.max
        };
        let full_frame = if self.max_fall > 0 { f64::from(self.max_fall).min(max) } else { max };
        let min = if self.target_min > 0 { f64::from(self.target_min) / 10_000.0 } else { f64::from(self.min_lum) / 10_000.0 };
        let sdr_white = if self.reference_lum > 0 { f64::from(self.reference_lum) } else { d.sdr_white };
        Luminance { max, full_frame, min: min.min(1.0), sdr_white }
    }
}

/// Les métadonnées HDR10 lues dans le flux (FFmpeg : écran de matriçage,
/// niveau de lumière du contenu) ; 0 : absentes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreamLight {
    pub max_cll: f64,
    pub mastering_max: f64,
    pub mastering_min: f64,
}

impl StreamLight {
    /// La crête du flux : MaxCLL, sinon le maximum de l'écran de matriçage,
    /// sinon `fallback` (la crête donnée à la Station : les jeux la suivent).
    pub fn peak(&self, fallback: f64) -> f64 {
        if self.max_cll > 0.0 {
            self.max_cll
        } else if self.mastering_max > 0.0 {
            self.mastering_max
        } else {
            fallback
        }
    }
}

/// Comment montrer une image PQ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Render {
    /// Telle quelle : l'écran est HDR et la crête du flux y tient.
    Pq,
    /// Ramenée à l'écran par l'EETF de BT.2390 (crêtes en cd/m²).
    Eetf { src_max: f64, dst_min: f64, dst_max: f64 },
    /// Vers le SDR, le blanc du bureau de l'ordinateur à `white` cd/m² ramené à 1.
    Sdr { white: f64 },
}

/// Le rendu d'une image PQ sur cet écran (`screen` : ses valeurs s'il est HDR,
/// `None` sinon), d'après les métadonnées du flux et les valeurs données à la
/// Station (`sent` : crête que les jeux suivent, blanc du bureau de Windows).
/// Un flux qui dépasse la crête de l'écran de plus de 2 % passe par l'EETF.
pub fn render_for(screen: Option<Luminance>, light: StreamLight, sent: Luminance) -> Render {
    match screen {
        None => Render::Sdr { white: sent.sdr_white.max(1.0) },
        Some(l) => {
            let peak = light.peak(sent.max);
            if peak <= l.max * 1.02 { Render::Pq } else { Render::Eetf { src_max: peak, dst_min: l.min, dst_max: l.max } }
        }
    }
}

/// Les paramètres du shader (`kn_hdr_tone` de `hdr_gl.c`) : mode (0 PQ telle
/// quelle, 1 EETF, 2 SDR), bornes de l'EETF en PQ, blanc SDR en cd/m².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub mode: i32,
    pub src_hi: f32,
    pub dst_lo: f32,
    pub dst_hi: f32,
    pub white: f32,
}

impl Tone {
    pub fn of(r: Render) -> Self {
        match r {
            Render::Pq => Self { mode: 0, src_hi: 1.0, dst_lo: 0.0, dst_hi: 1.0, white: 203.0 },
            Render::Eetf { src_max, dst_min, dst_max } => Self { mode: 1, src_hi: pq_oetf(src_max) as f32, dst_lo: pq_oetf(dst_min) as f32, dst_hi: pq_oetf(dst_max) as f32, white: 203.0 },
            Render::Sdr { white } => Self { mode: 2, src_hi: 1.0, dst_lo: 0.0, dst_hi: 1.0, white: white as f32 },
        }
    }
}

/// sRGB d'une valeur linéaire de 0 à 1.
pub fn srgb_encode(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// R'G'B' (PQ, BT.2020) d'un pixel Y'CbCr BT.2020 non constant, plage limitée,
/// codes 10 bits.
pub fn ycbcr_to_rgb(y: u16, cb: u16, cr: u16) -> [f64; 3] {
    let yy = (f64::from(y) - 64.0) / 876.0;
    let u = (f64::from(cb) - 512.0) / 896.0;
    let v = (f64::from(cr) - 512.0) / 896.0;
    [yy + 1.4746 * v, yy - 0.16455 * u - 0.57135 * v, yy + 1.8814 * u].map(|c| c.clamp(0.0, 1.0))
}

/// Ce que fait le shader pour un pixel (codes 10 bits) : R'G'B' de 0 à 1 en
/// sortie, en PQ (modes 0 et 1) ou en sRGB (mode 2).
pub fn shade(y: u16, cb: u16, cr: u16, t: &Tone) -> [f64; 3] {
    let e = ycbcr_to_rgb(y, cb, cr);
    match t.mode {
        0 => e,
        1 => {
            let n = e.map(pq_eotf);
            let m = n[0].max(n[1]).max(n[2]);
            if m <= 0.0 {
                return e;
            }
            let k = pq_eotf(eetf(pq_oetf(m), f64::from(t.src_hi), f64::from(t.dst_lo), f64::from(t.dst_hi))) / m;
            n.map(|c| pq_oetf(c * k))
        }
        _ => {
            let n = e.map(pq_eotf);
            let white = f64::from(t.white).max(1.0);
            let mut s = BT2020_TO_709.map(|row| (row[0] * n[0] + row[1] * n[1] + row[2] * n[2]).max(0.0) / white);
            let m = s[0].max(s[1]).max(s[2]);
            if m > 0.75 {
                let k = sdr_knee(m) / m;
                s = s.map(|c| c * k);
            }
            s.map(srgb_encode)
        }
    }
}

/// L'image réduite d'une image PQ (accueil, dernière image) : sRGB 8 bits, le
/// blanc du bureau de l'ordinateur à `white` cd/m².
pub fn pq_to_srgb8(y: u16, cb: u16, cr: u16, white: f64) -> [u8; 3] {
    shade(y, cb, cr, &Tone::of(Render::Sdr { white })).map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Une sortie de Wayland vue par la sonde (`kn_wl_probe`) : sa position dans
/// l'espace du compositeur et sa description d'image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WlOutput {
    pub x: i32,
    pub y: i32,
    pub desc: WlDescription,
}

/// Ce que dit la sonde de Wayland, avant SDL : la gestion des couleurs sait
/// décrire une image PQ BT.2020 (`supported`), et les sorties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WlProbe {
    pub supported: bool,
    pub outputs: Vec<WlOutput>,
}

impl WlProbe {
    /// Une sortie au moins est en HDR.
    pub fn any_hdr(&self) -> bool {
        self.supported && self.outputs.iter().any(|o| o.desc.is_hdr())
    }

    /// La sortie à cette position (celle d'un écran de SDL) ; une seule sortie : elle.
    pub fn at(&self, x: i32, y: i32) -> Option<&WlOutput> {
        if self.outputs.len() == 1 {
            return self.outputs.first();
        }
        self.outputs.iter().find(|o| o.x == x && o.y == y)
    }

    /// Les valeurs de l'écran à cette position s'il est HDR (et que la voie HDR existe).
    pub fn hdr_at(&self, x: i32, y: i32) -> Option<Luminance> {
        self.supported.then(|| self.at(x, y)).flatten().filter(|o| o.desc.is_hdr()).map(|o| o.desc.luminance())
    }
}

/// SDL prend Wayland plutôt que X11 (Xwayland) : seule voie du HDR, quand le
/// compositeur décrit une image PQ BT.2020 et qu'un écran est en HDR au
/// lancement. Jamais sous gamescope (Steam Deck, salon : il ne donne le HDR
/// qu'aux images Vulkan de sa couche), sur KyberNow Link OS (KMS), avec
/// `KYBERNOW_VIDEO` (choix à la main) ni avec `KYBERNOW_HDR=0`.
pub fn want_wayland(probe: &WlProbe, video_env: Option<&str>, gamescope: bool, link_os: bool, disabled: bool) -> bool {
    video_env.is_none() && !gamescope && !link_os && !disabled && probe.any_hdr()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn pq_aller_retour() {
        // Points connus de ST 2084 : 100 cd/m² ≈ 0,508, 1 000 ≈ 0,752, 10 000 = 1.
        assert!(close(pq_oetf(100.0), 0.5081, 1e-3));
        assert!(close(pq_oetf(1000.0), 0.7518, 1e-3));
        assert!(close(pq_oetf(10_000.0), 1.0, 1e-9));
        assert_eq!((pq_oetf(0.0), pq_oetf(-5.0), pq_eotf(0.0)), (0.0, 0.0, 0.0));
        for n in [0.01, 1.0, 80.0, 203.0, 600.0, 1600.0, 4000.0] {
            assert!(close(pq_eotf(pq_oetf(n)), n, n * 1e-6 + 1e-6), "{n}");
        }
    }

    #[test]
    fn courbe_sdr_comme_la_station() {
        assert_eq!(sdr_knee(0.5), 0.5);
        assert_eq!(sdr_knee(0.75), 0.75);
        // 4 fois le blanc ramené au blanc ; pente 1 au genou, croissante, jamais au-delà de 1 avant 4.
        assert!(close(sdr_knee(4.0), 1.0, 1e-6));
        assert!(close((sdr_knee(0.7501) - 0.75) / 0.0001, 1.0, 1e-3));
        let mut last = 0.0;
        for i in 0..400 {
            let v = sdr_knee(i as f64 / 100.0);
            assert!(v >= last && v <= 1.0 + 1e-9);
            last = v;
        }
    }

    #[test]
    fn eetf_bt2390() {
        let (src, lo, hi) = (pq_oetf(4000.0), pq_oetf(0.0), pq_oetf(1000.0));
        // La crête du flux va à la crête de l'écran ; le bas de l'échelle ne bouge pas.
        assert!(close(pq_eotf(eetf(src, src, lo, hi)), 1000.0, 1.0));
        for n in [0.1, 10.0, 100.0, 203.0] {
            assert!(close(pq_eotf(eetf(pq_oetf(n), src, lo, hi)), n, n * 1e-6 + 1e-6), "{n}");
        }
        // Croissante, jamais au-dessus de l'écran.
        let mut last = 0.0;
        for i in 0..=100 {
            let v = eetf(src * i as f64 / 100.0, src, lo, hi);
            assert!(v + 1e-12 >= last && v <= hi + 1e-9, "{i}");
            last = v;
        }
        // Un écran qui tient la crête : rien ne change.
        assert!(close(eetf(0.6, 0.7, 0.0, 0.8), 0.6, 1e-12));
        // Un noir d'écran relève les ombres, sans toucher la crête.
        let black = pq_oetf(0.05);
        assert!(eetf(0.0, src, black, hi) > 0.0);
        assert!(close(pq_eotf(eetf(src, src, black, hi)), 1000.0, 1.0));
    }

    #[test]
    fn rendu_d_une_image_pq() {
        let sent = Luminance { max: 1000.0, sdr_white: 240.0, ..Luminance::default() };
        let screen = Luminance { max: 1000.0, ..Luminance::default() };
        // Écran SDR : vers le SDR, au blanc donné à la Station.
        assert_eq!(render_for(None, StreamLight::default(), sent), Render::Sdr { white: 240.0 });
        // Sans métadonnées : la crête donnée à la Station, qui tient sur l'écran.
        assert_eq!(render_for(Some(screen), StreamLight::default(), sent), Render::Pq);
        // MaxCLL de 4 000 sur un écran de 1 000 : EETF ; l'écran de matriçage sert sans MaxCLL.
        let bright = StreamLight { max_cll: 4000.0, mastering_max: 1000.0, mastering_min: 0.0 };
        assert_eq!(render_for(Some(screen), bright, sent), Render::Eetf { src_max: 4000.0, dst_min: 0.0, dst_max: 1000.0 });
        let mastering = StreamLight { mastering_max: 1600.0, ..StreamLight::default() };
        assert!(matches!(render_for(Some(screen), mastering, sent), Render::Eetf { src_max, .. } if src_max == 1600.0));
        // À 2 % près : telle quelle.
        let near = StreamLight { max_cll: 1015.0, ..StreamLight::default() };
        assert_eq!(render_for(Some(screen), near, sent), Render::Pq);
    }

    #[test]
    fn le_shader_de_reference() {
        let pq = Tone::of(Render::Pq);
        // Noir et blanc vidéo (64 et 940, chroma au milieu) : 0 et 1.
        assert_eq!(shade(64, 512, 512, &pq), [0.0, 0.0, 0.0]);
        assert!(shade(940, 512, 512, &pq).iter().all(|c| close(*c, 1.0, 1e-9)));
        // Un gris au blanc du bureau (203 cd/m² en PQ) : vers le SDR à 203, le blanc passé par le genou (comme
        // l'image à part de la Station) ; à 406, un gris moyen, sous le genou.
        let code = (64.0 + 876.0 * pq_oetf(203.0)).round() as u16;
        let white = shade(code, 512, 512, &Tone::of(Render::Sdr { white: 203.0 }));
        assert!(white.iter().all(|c| close(*c, srgb_encode(sdr_knee(1.0)), 0.01)), "{white:?}");
        let half = shade(code, 512, 512, &Tone::of(Render::Sdr { white: 406.0 }));
        assert!(half.iter().all(|c| close(*c, srgb_encode(0.5), 0.01)), "{half:?}");
        // Quatre fois le blanc : le blanc SDR.
        let bright = (64.0 + 876.0 * pq_oetf(812.0)).round() as u16;
        assert!(shade(bright, 512, 512, &Tone::of(Render::Sdr { white: 203.0 })).iter().all(|c| close(*c, 1.0, 0.01)));
        // L'EETF garde la teinte (les composantes gardent leurs rapports en lumière linéaire).
        let t = Tone::of(Render::Eetf { src_max: 4000.0, dst_min: 0.0, dst_max: 1000.0 });
        let (y, cb, cr) = (900, 480, 600);
        let before = ycbcr_to_rgb(y, cb, cr).map(pq_eotf);
        let after = shade(y, cb, cr, &t).map(pq_eotf);
        assert!(after[0] < before[0]);
        assert!(close(after[1] / after[0], before[1] / before[0], 1e-6));
        // L'image réduite : sRGB 8 bits.
        assert_eq!(pq_to_srgb8(64, 512, 512, 203.0), [0, 0, 0]);
        assert_eq!(pq_to_srgb8(bright, 512, 512, 203.0), [255, 255, 255]);
    }

    #[test]
    fn ecran_d_apres_wayland() {
        // Une sortie HDR de KWin : PQ, cible 0,005 à 600 cd/m², MaxCLL 600, MaxFALL 350, référence 203.
        let kwin = WlDescription { tf: WL_TF_PQ, min_lum: 50, max_lum: 10_000, reference_lum: 203, target_min: 50, target_max: 600, max_cll: 600, max_fall: 350 };
        assert!(kwin.is_hdr());
        assert_eq!(kwin.luminance(), Luminance { max: 600.0, full_frame: 350.0, min: 0.005, sdr_white: 203.0 });
        // Sans cible : les valeurs par défaut du cœur.
        let bare = WlDescription { tf: WL_TF_PQ, ..WlDescription::default() };
        assert_eq!(bare.luminance(), Luminance::default());
        // Une sortie SDR (gamma 2.2).
        assert!(!WlDescription { tf: 2, reference_lum: 80, ..WlDescription::default() }.is_hdr());
    }

    #[test]
    fn wayland_seulement_pour_le_hdr() {
        let hdr = WlOutput { x: 0, y: 0, desc: WlDescription { tf: WL_TF_PQ, target_max: 1000, ..WlDescription::default() } };
        let sdr = WlOutput { x: 2560, y: 0, desc: WlDescription { tf: 9, ..WlDescription::default() } };
        let probe = WlProbe { supported: true, outputs: vec![sdr, hdr] };
        assert!(want_wayland(&probe, None, false, false, false));
        assert!(!want_wayland(&probe, Some("x11"), false, false, false), "choix à la main");
        assert!(!want_wayland(&probe, None, true, false, false), "gamescope");
        assert!(!want_wayland(&probe, None, false, true, false), "KyberNow Link OS");
        assert!(!want_wayland(&probe, None, false, false, true), "KYBERNOW_HDR=0");
        assert!(!want_wayland(&WlProbe { supported: true, outputs: vec![sdr] }, None, false, false, false), "aucun écran HDR");
        assert!(!want_wayland(&WlProbe { supported: false, outputs: vec![hdr] }, None, false, false, false), "pas de PQ");
        // Les écrans de SDL retrouvés par leur position.
        assert_eq!(probe.hdr_at(0, 0).map(|l| l.max), Some(1000.0));
        assert_eq!(probe.hdr_at(2560, 0), None);
        assert_eq!(probe.hdr_at(99, 99), None);
        assert!(WlProbe { supported: true, outputs: vec![hdr] }.hdr_at(99, 99).is_some(), "une seule sortie : elle");
    }
}
