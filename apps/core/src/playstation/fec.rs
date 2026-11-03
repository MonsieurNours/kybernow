//! Correction d'erreurs des images, d'après `fec.c` de chiaki-ng, qui
//! emploie jerasure : Reed-Solomon à matrice de Cauchy « originale » sur
//! GF(2⁸) (polynôme 0x11d), `k` unités de données et `m` unités de
//! correction. Ligne `i` de la matrice, colonne `j` : 1 / (i xor (m + j)).
//! Avec au plus `m` unités perdues, les données se retrouvent en inversant
//! la matrice des `k` premières unités reçues.

/// Tables de GF(2⁸), polynôme x⁸ + x⁴ + x³ + x² + 1 (0x11d, celui de
/// jerasure pour w = 8).
struct Gf {
    exp: [u8; 512],
    log: [u8; 256],
}

impl Gf {
    const fn new() -> Self {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut x: u16 = 1;
        let mut i = 0;
        while i < 255 {
            exp[i] = x as u8;
            exp[i + 255] = x as u8;
            log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x11d;
            }
            i += 1;
        }
        Gf { exp, log }
    }

    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
        }
    }

    fn inv(&self, a: u8) -> u8 {
        debug_assert!(a != 0);
        self.exp[255 - self.log[a as usize] as usize]
    }
}

static GF: Gf = Gf::new();

/// Matrice de Cauchy (`cauchy_original_coding_matrix`), `m` lignes de `k`.
fn cauchy(k: usize, m: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(k * m);
    for i in 0..m {
        for j in 0..k {
            out.push(GF.inv((i ^ (m + j)) as u8));
        }
    }
    out
}

/// Inverse d'une matrice carrée `n` × `n` (Gauss-Jordan). `None` si elle est
/// singulière.
fn invert(mut a: Vec<u8>, n: usize) -> Option<Vec<u8>> {
    let mut inv = vec![0u8; n * n];
    for i in 0..n {
        inv[i * n + i] = 1;
    }
    for col in 0..n {
        let pivot = (col..n).find(|&r| a[r * n + col] != 0)?;
        if pivot != col {
            for c in 0..n {
                a.swap(pivot * n + c, col * n + c);
                inv.swap(pivot * n + c, col * n + c);
            }
        }
        let p = GF.inv(a[col * n + col]);
        for c in 0..n {
            a[col * n + c] = GF.mul(a[col * n + c], p);
            inv[col * n + c] = GF.mul(inv[col * n + c], p);
        }
        for r in 0..n {
            let f = a[r * n + col];
            if r != col && f != 0 {
                for c in 0..n {
                    a[r * n + c] ^= GF.mul(f, a[col * n + c]);
                    inv[r * n + c] ^= GF.mul(f, inv[col * n + c]);
                }
            }
        }
    }
    Some(inv)
}

/// Retrouve les unités de données perdues (`chiaki_fec_decode`). `units` :
/// `k + m` unités de `unit_size` octets au moins (données puis correction),
/// `erased` : indices des unités perdues. Seules les unités de données
/// perdues sont réécrites. `false` si c'est impossible (trop de pertes).
pub fn decode(units: &mut [Vec<u8>], unit_size: usize, k: usize, m: usize, erased: &[usize]) -> bool {
    if units.len() != k + m || erased.len() > m || units.iter().any(|u| u.len() < unit_size) {
        return false;
    }
    let lost_data: Vec<usize> = erased.iter().copied().filter(|&e| e < k).collect();
    if lost_data.is_empty() {
        return true;
    }
    let matrix = cauchy(k, m);
    // Les k premières unités reçues, et leur ligne dans la matrice
    // génératrice (identité pour les données, Cauchy pour la correction).
    let survivors: Vec<usize> = (0..k + m).filter(|i| !erased.contains(i)).take(k).collect();
    if survivors.len() < k {
        return false;
    }
    let mut a = vec![0u8; k * k];
    for (r, &dev) in survivors.iter().enumerate() {
        if dev < k {
            a[r * k + dev] = 1;
        } else {
            a[r * k..r * k + k].copy_from_slice(&matrix[(dev - k) * k..(dev - k) * k + k]);
        }
    }
    let Some(inv) = invert(a, k) else { return false };
    for &d in &lost_data {
        let mut out = vec![0u8; unit_size];
        for (r, &dev) in survivors.iter().enumerate() {
            let f = inv[d * k + r];
            if f == 0 {
                continue;
            }
            for (o, &b) in out.iter_mut().zip(&units[dev][..unit_size]) {
                *o ^= GF.mul(f, b);
            }
        }
        units[d][..unit_size].copy_from_slice(&out);
    }
    true
}

/// Unités de correction de `data` (`chiaki_fec_encode`), pour les essais.
pub fn encode(data: &[Vec<u8>], unit_size: usize, m: usize) -> Vec<Vec<u8>> {
    let k = data.len();
    let matrix = cauchy(k, m);
    (0..m)
        .map(|i| {
            let mut out = vec![0u8; unit_size];
            for (j, unit) in data.iter().enumerate() {
                let f = matrix[i * k + j];
                for (o, &b) in out.iter_mut().zip(&unit[..unit_size]) {
                    *o ^= GF.mul(f, b);
                }
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn gf_inverse() {
        for a in 1..=255u8 {
            assert_eq!(GF.mul(a, GF.inv(a)), 1);
        }
    }

    #[test]
    fn chiaki_cases() {
        let text = include_str!("fec_vectors.txt");
        let mut n = 0;
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let parts: Vec<&str> = line.split(' ').collect();
            let (k, m): (usize, usize) = (parts[0].parse().unwrap(), parts[1].parse().unwrap());
            let erased: Vec<usize> = parts[2].split(',').map(|e| e.parse().unwrap()).collect();
            let size: usize = parts[3].parse().unwrap();
            let all = base64::engine::general_purpose::STANDARD.decode(parts[4]).unwrap();
            let reference: Vec<Vec<u8>> = all.chunks(size).map(<[u8]>::to_vec).collect();
            let mut units = reference.clone();
            for &e in &erased {
                units[e].fill(0x42);
            }
            assert!(decode(&mut units, size, k, m, &erased), "cas {line:.20}");
            for i in 0..k {
                assert_eq!(units[i], reference[i], "unité {i}");
            }
            n += 1;
        }
        assert_eq!(n, 6);
    }

    #[test]
    fn encode_then_lose_everything_possible() {
        let k = 7;
        let m = 3;
        let data: Vec<Vec<u8>> = (0..k).map(|i| (0..50u8).map(|b| b.wrapping_mul(31).wrapping_add(i as u8 * 7)).collect()).collect();
        let mut all = data.clone();
        all.extend(encode(&data, 50, m));
        for erased in [vec![0, 1, 2], vec![6, 8], vec![3], vec![0, 7, 9]] {
            let mut units = all.clone();
            for &e in &erased {
                units[e].fill(0);
            }
            assert!(decode(&mut units, 50, k, m, &erased));
            assert_eq!(&units[..k], &data[..]);
        }
        let mut units = all.clone();
        assert!(!decode(&mut units, 50, k, m, &[0, 1, 2, 3]));
    }
}
