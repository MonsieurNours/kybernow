//! Profil de couleur des écrans virtuels en HDR (docs/hdr.md, « Réglage du HDR »).
//!
//! Un profil ICC v4 d'écran avec l'étiquette `MHC2` de Microsoft, du même
//! type que celui de l'outil d'étalonnage HDR de Windows : il donne à
//! Windows les luminosités de l'écran de l'appareil (les jeux les lisent par
//! `IDXGIOutput6::GetDesc1` et font leur tone mapping d'après elles), et sa
//! table (`regamma`, appliquée par Windows aux valeurs PQ envoyées à l'écran)
//! ramène le bureau SDR au gamma 2.2 au lieu de la courbe sRGB par morceaux,
//! qui remonte les noirs. Avec « sRGB de Windows », la table ne change rien.
//!
//! Primaires déclarées : BT.2020, matrice MHC2 identité : le profil ne
//! restreint pas les couleurs, l'écran de l'appareil fait le reste.

use super::hdr::ScreenLuminance;

/// Entrées de la table `regamma` (Windows en accepte de 2 à 4096).
const LUT_SIZE: usize = 1024;

const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

/// Bradford, D65 → D50 (étiquette `chad`, colorants adaptés).
const CHAD: [[f64; 3]; 3] = [
    [1.0478112, 0.0228866, -0.0501270],
    [0.0295424, 0.9904844, -0.0170491],
    [-0.0092345, 0.0150436, 0.7521316],
];

/// Primaires BT.2020 (x, y) et blanc D65.
const PRIMARIES: [[f64; 2]; 3] = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]];
const WHITE_D65: [f64; 2] = [0.3127, 0.3290];

// SMPTE ST 2084 (PQ).
const M1: f64 = 2610.0 / 16384.0;
const M2: f64 = 2523.0 / 4096.0 * 128.0;
const C1: f64 = 3424.0 / 4096.0;
const C2: f64 = 2413.0 / 4096.0 * 32.0;
const C3: f64 = 2392.0 / 4096.0 * 32.0;

/// Valeur PQ (0 à 1) → nits.
pub fn pq_to_nits(p: f64) -> f64 {
    let e = p.clamp(0.0, 1.0).powf(1.0 / M2);
    10_000.0 * ((e - C1).max(0.0) / (C2 - C3 * e)).powf(1.0 / M1)
}

/// Nits → valeur PQ (0 à 1).
pub fn nits_to_pq(nits: f64) -> f64 {
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(M1);
    ((C1 + C2 * y) / (1.0 + C3 * y)).powf(M2)
}

/// Valeur codée en sRGB (par morceaux) d'une valeur linéaire de 0 à 1.
fn srgb_encode(x: f64) -> f64 {
    if x <= 0.003_130_8 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

/// La table `regamma` : valeur PQ voulue pour chaque valeur PQ que Windows
/// envoie. Avec `gamma22`, sous le blanc du bureau, une valeur que Windows a
/// décodée avec la courbe sRGB est redécodée en gamma 2.2 ; au-dessus, rien
/// ne change (pas de tone mapping dans la Station).
pub fn regamma(white: f64, gamma22: bool) -> Vec<f64> {
    (0..LUT_SIZE)
        .map(|i| {
            let p = i as f64 / (LUT_SIZE - 1) as f64;
            if !gamma22 {
                return p;
            }
            let nits = pq_to_nits(p);
            // Le noir reste le noir ; au-dessus du blanc, rien ne change.
            if nits <= 0.0 || nits >= white {
                return p;
            }
            let v = srgb_encode(nits / white);
            nits_to_pq(v.powf(2.2) * white)
        })
        .collect()
}

/// Colorants rouge, vert et bleu (XYZ, adaptés à D50).
fn colorants() -> [[f64; 3]; 3] {
    let xyz = |[x, y]: [f64; 2]| [x / y, 1.0, (1.0 - x - y) / y];
    let p: Vec<[f64; 3]> = PRIMARIES.iter().map(|c| xyz(*c)).collect();
    let w = xyz(WHITE_D65);
    // Intensités des primaires qui donnent le blanc : P · s = W.
    let m = [[p[0][0], p[1][0], p[2][0]], [p[0][1], p[1][1], p[2][1]], [p[0][2], p[1][2], p[2][2]]];
    let s = solve3(m, w);
    let mut out = [[0.0; 3]; 3];
    for c in 0..3 {
        let col = [p[c][0] * s[c], p[c][1] * s[c], p[c][2] * s[c]];
        for (r, row) in CHAD.iter().enumerate() {
            out[c][r] = row[0] * col[0] + row[1] * col[1] + row[2] * col[2];
        }
    }
    out
}

/// Résout `m · x = b` (Cramer).
fn solve3(m: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(m);
    let mut x = [0.0; 3];
    for (i, xi) in x.iter_mut().enumerate() {
        let mut mi = m;
        for r in 0..3 {
            mi[r][i] = b[r];
        }
        *xi = det(mi) / d;
    }
    x
}

fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_type(v: [f64; 3]) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    v.iter().for_each(|c| t.extend(s15f16(*c)));
    t
}

fn sf32_type(values: impl IntoIterator<Item = f64>) -> Vec<u8> {
    let mut t = b"sf32\0\0\0\0".to_vec();
    values.into_iter().for_each(|v| t.extend(s15f16(v)));
    t
}

/// Texte d'une étiquette `desc` ou `cprt` (v4 : `mluc`, en-US).
fn mluc(text: &str) -> Vec<u8> {
    let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let mut t = b"mluc\0\0\0\0".to_vec();
    t.extend(1u32.to_be_bytes());
    t.extend(12u32.to_be_bytes());
    t.extend(b"enUS");
    t.extend((utf16.len() as u32).to_be_bytes());
    t.extend(28u32.to_be_bytes());
    t.extend(utf16);
    t
}

/// Étiquette `MHC2` : luminosités, matrice 3 × 4 identité, une table par
/// voie (la même pour les trois).
fn mhc2(lum: &ScreenLuminance, lut: &[f64]) -> Vec<u8> {
    const HEADER: u32 = 36;
    let matrix = sf32_type([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let table = sf32_type(lut.iter().copied());
    let lut0 = HEADER + matrix.len() as u32;
    let mut t = b"MHC2\0\0\0\0".to_vec();
    t.extend((lut.len() as u32).to_be_bytes());
    t.extend(s15f16(f64::from(lum.min)));
    t.extend(s15f16(f64::from(lum.max)));
    t.extend(HEADER.to_be_bytes());
    for i in 0..3 {
        t.extend((lut0 + i * table.len() as u32).to_be_bytes());
    }
    t.extend(matrix);
    for _ in 0..3 {
        t.extend(&table);
    }
    t
}

/// Le profil ICC complet pour un écran de luminosités `lum`.
pub fn profile(lum: &ScreenLuminance, gamma22: bool) -> Vec<u8> {
    let c = colorants();
    let curv = b"curv\0\0\0\0\0\0\0\0".to_vec(); // aucune entrée : linéaire
    let desc = format!(
        "KyberNow HDR {:.0} nits, {}",
        lum.max,
        if gamma22 { "gamma 2.2" } else { "sRGB" }
    );
    let lumi = [D50[0] * f64::from(lum.max), f64::from(lum.max), D50[2] * f64::from(lum.max)];
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", mluc(&desc)),
        (b"cprt", mluc("KyberNow")),
        (b"wtpt", xyz_type(D50)),
        (b"lumi", xyz_type(lumi)),
        (b"rXYZ", xyz_type(c[0])),
        (b"gXYZ", xyz_type(c[1])),
        (b"bXYZ", xyz_type(c[2])),
        (b"rTRC", curv.clone()),
        (b"gTRC", curv.clone()),
        (b"bTRC", curv),
        (b"chad", sf32_type(CHAD.iter().flatten().copied())),
        (b"MHC2", mhc2(lum, &regamma(f64::from(lum.sdr_white), gamma22))),
    ];
    let table_len = 4 + 12 * tags.len();
    let mut data = Vec::new();
    let mut entries = Vec::new();
    for (sig, body) in &tags {
        let offset = 128 + table_len + data.len();
        entries.push((sig, offset as u32, body.len() as u32));
        data.extend(body);
        while data.len() % 4 != 0 {
            data.push(0);
        }
    }
    let size = (128 + table_len + data.len()) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend(size.to_be_bytes());
    out.extend([0u8; 4]); // CMM
    out.extend(0x0440_0000u32.to_be_bytes()); // version 4.4
    out.extend(b"mntrRGB XYZ ");
    out.extend([0u8; 12]); // date : sans
    out.extend(b"acspMSFT");
    out.extend([0u8; 4]); // drapeaux
    out.extend([0u8; 8]); // fabricant, modèle
    out.extend([0u8; 8]); // attributs
    out.extend(0u32.to_be_bytes()); // intention perceptuelle
    D50.iter().for_each(|v| out.extend(s15f16(*v)));
    out.extend(b"KNOW");
    out.resize(128, 0); // identifiant du profil et réserve
    out.extend((tags.len() as u32).to_be_bytes());
    for (sig, offset, len) in entries {
        out.extend(*sig);
        out.extend(offset.to_be_bytes());
        out.extend(len.to_be_bytes());
    }
    out.extend(data);
    out
}

/// Nom du fichier du profil : un par écran et par contenu (Windows garde
/// les profils installés par leur nom).
pub fn file_name(target: u32, bytes: &[u8]) -> String {
    let sum = bytes.iter().fold(0u32, |h, b| h.rotate_left(5) ^ u32::from(*b)).wrapping_mul(0x9E37_79B1);
    format!("KyberNow-HDR-{target}-{sum:08x}.icm")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
    }
    fn fixed_at(b: &[u8], at: usize) -> f64 {
        f64::from(i32::from_be_bytes(b[at..at + 4].try_into().unwrap())) / 65536.0
    }

    #[test]
    fn pq_aller_retour() {
        for nits in [0.0, 0.05, 80.0, 203.0, 1000.0, 4000.0, 10_000.0] {
            assert!((pq_to_nits(nits_to_pq(nits)) - nits).abs() < 1e-6 * nits.max(1.0), "{nits}");
        }
        assert!((nits_to_pq(10_000.0) - 1.0).abs() < 1e-9);
        assert!((nits_to_pq(100.0) - 0.508).abs() < 0.001);
    }

    #[test]
    fn table_gamma_22_sous_le_blanc_seulement() {
        let white = 203.0;
        let lut = regamma(white, true);
        assert_eq!(lut.len(), LUT_SIZE);
        // Monotone, de 0 à 1.
        assert!(lut.windows(2).all(|w| w[1] >= w[0]));
        assert!(lut[0] == 0.0 && (lut[LUT_SIZE - 1] - 1.0).abs() < 1e-9);
        for (i, out) in lut.iter().enumerate() {
            let p = i as f64 / (LUT_SIZE - 1) as f64;
            let nits = pq_to_nits(p);
            if nits >= white {
                assert_eq!(*out, p, "au-dessus du blanc : rien ne change");
            } else if nits < white * 0.02 {
                // Le gamma 2.2 assombrit les ombres que sRGB remontait.
                assert!(*out <= p + 1e-12, "{nits} nits");
            }
        }
        // Un gris sombre du bureau : sRGB 10 % de valeur → gamma 2.2.
        let v: f64 = 0.1;
        let srgb_nits = ((v + 0.055) / 1.055).powf(2.4) * white;
        let i = (nits_to_pq(srgb_nits) * (LUT_SIZE - 1) as f64).round() as usize;
        let expected = v.powf(2.2) * white;
        assert!((pq_to_nits(lut[i]) - expected).abs() / expected < 0.05);
        // « sRGB de Windows » : table identité.
        assert!(regamma(white, false).iter().enumerate().all(|(i, o)| *o == i as f64 / (LUT_SIZE - 1) as f64));
    }

    #[test]
    fn colorants_bt2020_adaptes_a_d50() {
        let c = colorants();
        for r in 0..3 {
            let sum: f64 = c.iter().map(|col| col[r]).sum();
            assert!((sum - D50[r]).abs() < 0.002, "blanc D50, composante {r} : {sum}");
        }
        // Le rouge BT.2020 adapté, comme dans les profils de Windows.
        assert!((c[0][0] - 0.6734).abs() < 0.002 && (c[0][1] - 0.2790).abs() < 0.002);
    }

    #[test]
    fn profil_icc_avec_mhc2() {
        let lum = ScreenLuminance { max: 1600.0, full_frame: 1000.0, min: 0.005, sdr_white: 240.0 };
        let p = profile(&lum, true);
        assert_eq!(u32_at(&p, 0) as usize, p.len());
        assert_eq!(&p[12..20], b"mntrRGB ");
        assert_eq!(&p[36..40], b"acsp");
        let count = u32_at(&p, 128) as usize;
        assert_eq!(count, 12);
        let tag = |sig: &[u8; 4]| {
            (0..count).map(|i| 132 + 12 * i).find(|at| &p[*at..*at + 4] == sig).map(|at| (u32_at(&p, at + 4) as usize, u32_at(&p, at + 8) as usize)).unwrap()
        };
        for i in 0..count {
            let offset = u32_at(&p, 132 + 12 * i + 4) as usize;
            assert_eq!(offset % 4, 0, "étiquettes alignées sur 4 octets");
        }
        let (lumi, _) = tag(b"lumi");
        assert!((fixed_at(&p, lumi + 12) - 1600.0).abs() < 0.01);
        let (m, len) = tag(b"MHC2");
        assert_eq!(&p[m..m + 4], b"MHC2");
        assert_eq!(u32_at(&p, m + 8) as usize, LUT_SIZE);
        assert!((fixed_at(&p, m + 12) - 0.005).abs() < 0.0001);
        assert!((fixed_at(&p, m + 16) - 1600.0).abs() < 0.01);
        let matrix = m + u32_at(&p, m + 20) as usize;
        assert_eq!(&p[matrix..matrix + 4], b"sf32");
        assert_eq!(fixed_at(&p, matrix + 8), 1.0);
        let lut2 = m + u32_at(&p, m + 32) as usize;
        assert_eq!(&p[lut2..lut2 + 4], b"sf32");
        assert_eq!(lut2 + 8 + 4 * LUT_SIZE, m + len, "la dernière table finit l'étiquette");
        // Un profil par contenu : le nom change avec les valeurs.
        let other = profile(&ScreenLuminance { max: 1000.0, ..lum }, true);
        assert_ne!(file_name(3, &p), file_name(3, &other));
        assert!(file_name(3, &p).starts_with("KyberNow-HDR-3-") && file_name(3, &p).ends_with(".icm"));
    }
}
