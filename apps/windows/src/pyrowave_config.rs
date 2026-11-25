//! Le paquet de configuration de PyroWave (docs/pyrowave.md), à part du
//! décodeur (`pyrowave.rs`, Windows seulement) pour se tester sur le Mac.

/// Paquet de configuration PyroWave (16 octets) : `"KNPW"` | version (1) |
/// chroma (0 : 4:2:0, 1 : 4:4:4) | couleur (0 : SDR BT.709, 1 : HDR10
/// BT.2020 PQ) | plage (0 : pleine, 1 : réduite) | largeur u32 LE | hauteur u32 LE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub chroma444: bool,
    pub hdr: bool,
    pub full_range: bool,
    pub width: u32,
    pub height: u32,
}

impl Config {
    pub const SIZE: usize = 16;

    /// Lit un paquet de configuration ; `None` s'il n'en est pas un (version
    /// inconnue, définition hors des bornes de PyroWave).
    pub fn parse(d: &[u8]) -> Option<Self> {
        if d.len() < Self::SIZE || &d[..4] != b"KNPW" || d[4] != 1 || d[5] > 1 || d[6] > 1 || d[7] > 1 {
            return None;
        }
        let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        let c = Self { chroma444: d[5] == 1, hdr: d[6] == 1, full_range: d[7] == 0, width: u32le(8), height: u32le(12) };
        c.valid().then_some(c)
    }

    /// Un paquet média qui est en fait le paquet de configuration (la Station
    /// peut l'envoyer sans le drapeau de configuration).
    pub fn is_config(d: &[u8]) -> bool {
        d.len() == Self::SIZE && &d[..4] == b"KNPW"
    }

    /// Repli si le paquet de configuration s'est perdu : définition et chroma
    /// de l'en-tête de séquence du flux (8 octets en tête d'un paquet
    /// PyroWave, bit « extended ») ; couleur et plage n'y sont pas fiables :
    /// SDR, plage pleine. Comme sur Mac.
    pub fn from_sequence_header(d: &[u8]) -> Option<Self> {
        if d.len() < 8 {
            return None;
        }
        let w0 = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
        let w1 = u32::from_le_bytes([d[4], d[5], d[6], d[7]]);
        if w0 >> 31 != 1 || (w1 >> 24) & 3 != 0 {
            return None;
        }
        let c = Self {
            chroma444: (w1 >> 26) & 1 == 1,
            hdr: false,
            full_range: true,
            width: (w0 & 0x3FFF) + 1,
            height: ((w0 >> 14) & 0x3FFF) + 1,
        };
        c.valid().then_some(c)
    }

    fn valid(&self) -> bool {
        (1..=16384).contains(&self.width) && (1..=16384).contains(&self.height) && (self.chroma444 || (self.width % 2 == 0 && self.height % 2 == 0))
    }

    pub fn chroma_size(&self) -> (u32, u32) {
        if self.chroma444 { (self.width, self.height) } else { (self.width / 2, self.height / 2) }
    }

    pub fn label(&self) -> String {
        format!(
            "{}×{} {} {} {}",
            self.width,
            self.height,
            if self.chroma444 { "4:4:4" } else { "4:2:0" },
            if self.hdr { "HDR10" } else { "SDR" },
            if self.full_range { "pleine" } else { "réduite" }
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[..4].copy_from_slice(b"KNPW");
        b[4] = 1;
        b[5] = self.chroma444 as u8;
        b[6] = self.hdr as u8;
        b[7] = (!self.full_range) as u8;
        b[8..12].copy_from_slice(&self.width.to_le_bytes());
        b[12..16].copy_from_slice(&self.height.to_le_bytes());
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &[u8] = include_bytes!("probe/pyrowave.bin");

    #[test]
    fn paquet_de_configuration() {
        let c = Config { chroma444: true, hdr: true, full_range: false, width: 3840, height: 2160 };
        let b = c.bytes();
        assert_eq!(&b[..8], b"KNPW\x01\x01\x01\x01");
        assert_eq!(Config::parse(&b), Some(c));
        assert!(Config::is_config(&b));
        let mut v2 = b;
        v2[4] = 2;
        assert_eq!(Config::parse(&v2), None, "version inconnue");
        let odd = Config { chroma444: false, hdr: false, full_range: true, width: 1919, height: 1080 };
        assert_eq!(Config::parse(&odd.bytes()), None, "4:2:0 de largeur impaire");
        assert_eq!(c.chroma_size(), (3840, 2160));
        assert_eq!(Config { chroma444: false, ..c }.chroma_size(), (1920, 1080));
    }

    #[test]
    fn en_tete_de_sequence_de_l_image_de_test() {
        let c = Config::from_sequence_header(PROBE).expect("en-tête de séquence");
        assert_eq!((c.width, c.height, c.chroma444), (128, 128, false));
    }
}
