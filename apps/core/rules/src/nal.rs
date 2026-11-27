//! Le découpage d'un flux H.264 ou HEVC en Annex B (WEBOS-27) : unités NAL
//! séparées par un code de départ (`00 00 01`, ou `00 00 00 01`), octets
//! d'émulation (`00 00 03`). Avant, le cœur (marques des images, références
//! des images des consoles, image clé d'une partie) et l'app LG (métadonnées
//! HDR10 du flux) avaient chacun le leur.
//!
//! Tout est paresseux : une recherche qui s'arrête à la première tranche
//! (marque d'une image) ne parcourt pas le reste de l'image.

/// Les débuts des unités NAL de `data` : la position qui suit chaque code de
/// départ (le premier octet de l'en-tête de l'unité).
pub fn starts(data: &[u8]) -> Starts<'_> {
    Starts { data, at: 0 }
}

/// L'itérateur de [`starts`].
#[derive(Debug, Clone)]
pub struct Starts<'a> {
    data: &'a [u8],
    at: usize,
}

impl Iterator for Starts<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let found = next_start_code(self.data, self.at)?;
        self.at = found + 3;
        Some(found + 3)
    }
}

/// La position du prochain code de départ (`00 00 01`) à partir de `from`.
fn next_start_code(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= data.len() {
        if data[i + 2] > 1 {
            // Aucun code ne peut commencer dans les trois prochains octets.
            i += 3;
        } else if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

/// L'unité qui commence à `start` (une position rendue par [`starts`]) :
/// jusqu'au code de départ suivant, sans les zéros qui le précèdent (celui
/// d'un code sur quatre octets, ou un bourrage), ou jusqu'à la fin.
pub fn unit_at(data: &[u8], start: usize) -> &[u8] {
    if start >= data.len() {
        return &[];
    }
    let mut end = next_start_code(data, start).unwrap_or(data.len());
    while end > start && data[end - 1] == 0 {
        end -= 1;
    }
    &data[start..end]
}

/// Les unités NAL de `data`, sans code de départ, dans l'ordre.
pub fn units(data: &[u8]) -> impl Iterator<Item = &[u8]> + '_ {
    starts(data).map(move |s| unit_at(data, s))
}

/// Le type d'une unité HEVC (premier octet de l'en-tête de deux octets).
pub fn hevc_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|b| (b >> 1) & 0x3f)
}

/// Le type d'une unité H.264 (en-tête d'un octet).
pub fn h264_type(nal: &[u8]) -> Option<u8> {
    nal.first().map(|b| b & 0x1f)
}

/// Les octets d'une unité sans les octets d'émulation (`00 00 03` → `00 00`).
pub fn unescape(nal: &[u8]) -> Vec<u8> {
    unescape_prefix(nal, usize::MAX)
}

/// [`unescape`] des `max` premiers octets seulement (le début d'une tranche
/// suffit à lire son en-tête).
pub fn unescape_prefix(nal: &[u8], max: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len().min(max));
    let mut zeros = 0;
    for &b in nal.iter().take(max) {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unites_d_un_flux() {
        // Code sur quatre octets, puis sur trois, bourrage avant le suivant.
        let data = [0, 0, 0, 1, 0x40, 0x01, 0xAA, 0, 0, 1, 0x42, 0x01, 0xBB, 0, 0, 0, 0, 1, 0x26, 0x01];
        let u: Vec<&[u8]> = units(&data).collect();
        assert_eq!(u, [&[0x40, 0x01, 0xAA][..], &[0x42, 0x01, 0xBB][..], &[0x26, 0x01][..]]);
        assert_eq!(u.iter().map(|n| hevc_type(n).unwrap()).collect::<Vec<_>>(), [32, 33, 19]);
        assert_eq!(h264_type(&[0x65]), Some(5));
        assert_eq!(starts(&data).collect::<Vec<_>>(), [4, 10, 18]);
        assert_eq!(unit_at(&data, 18), [0x26, 0x01]);
        assert_eq!(unit_at(&data, 99), [0u8; 0]);
        // Sans code de départ : rien ; un code à la toute fin : une unité vide.
        assert_eq!(units(&[1, 2, 3]).count(), 0);
        assert_eq!(units(&[]).count(), 0);
        assert_eq!(units(&[9, 0, 0, 1]).collect::<Vec<_>>(), [&[0u8; 0][..]]);
        // Un 01 qui n'est pas un code de départ.
        assert_eq!(units(&[0, 0, 1, 0x40, 1, 0, 1, 2]).collect::<Vec<_>>(), [&[0x40, 1, 0, 1, 2][..]]);
    }

    #[test]
    fn octets_d_emulation() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 0]), [0, 0, 1, 0, 0, 0]);
        // Un 03 qui ne suit pas deux zéros reste.
        assert_eq!(unescape(&[0, 3, 0, 0, 3]), [0, 3, 0, 0]);
        assert_eq!(unescape_prefix(&[0, 0, 3, 1, 2, 3, 4], 4), [0, 0, 1]);
    }
}
