//! Le HDR sur le téléviseur (docs/hdr.md), ce qui se lit sans lui (testé sur
//! l'ordinateur) : son écran, d'après ses réglages (`from_configs`), et le HDR
//! du flux, lu dans le flux lui-même. La Station (Kyber, correctifs 0022)
//! écrit les métadonnées HDR10 statiques de l'écran de l'ordinateur (SEI
//! « mastering display » et « content light level » en HEVC, OBU de
//! métadonnées en AV1) ; le SPS dit le profil HEVC (Main 10).
//!
//! Le téléviseur ne passe en HDR que si l'app lui donne ces métadonnées
//! (`ndl::ensure`) : jamais pour une image SDR (il afficherait l'image SDR en
//! mode HDR, délavée), d'où la lecture du flux plutôt que du seul réglage.
//!
//! Unités : celles de SMPTE ST 2086 et des SEI HEVC, que NDL prend telles
//! quelles : primaires et point blanc en 0,00002, dans l'ordre vert, bleu,
//! rouge ; luminances de l'écran de matriçage en 0,0001 cd/m² ; MaxCLL et
//! MaxFALL en cd/m².

/// Les réglages du téléviseur qui disent s'il affiche le HDR10
/// (`luna://com.webos.service.config/getConfigs`) : `tv.model.supportHDR`, et
/// `tv.config.supportDolbyHDRContents` (un téléviseur Dolby Vision prend aussi
/// le HDR10).
pub const CONFIGS: [&str; 2] = ["tv.model.supportHDR", "tv.config.supportDolbyHDRContents"];

/// La réponse de `getConfigs` : une des clés vraie (booléen, nombre ou texte
/// « true ») ; `None` si aucune n'y est.
pub fn from_configs(reply: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(reply).ok()?;
    let configs = v.get("configs")?;
    let mut seen = None;
    for key in CONFIGS {
        let on = match configs.get(key) {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::String(s)) => s.eq_ignore_ascii_case("true"),
            Some(serde_json::Value::Number(n)) => n.as_i64().is_some_and(|n| n != 0),
            _ => continue,
        };
        seen = Some(seen.unwrap_or(false) || on);
    }
    seen
}

/// Les métadonnées HDR10 statiques d'un flux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdrMeta {
    /// (x, y) des primaires, vert, bleu, rouge.
    pub primaries: [(u32, u32); 3],
    pub white: (u32, u32),
    pub max_luminance: u32,
    pub min_luminance: u32,
    pub max_cll: u32,
    pub max_fall: u32,
}

impl Default for HdrMeta {
    /// Sans métadonnées dans le flux : primaires BT.2020, blanc D65, écran de
    /// 1000 cd/m² (0,005 au plus bas), MaxCLL 1000, MaxFALL 400.
    fn default() -> Self {
        Self { primaries: [(8500, 39850), (6550, 2300), (35400, 14600)], white: (15635, 16450), max_luminance: 10_000_000, min_luminance: 50, max_cll: 1000, max_fall: 400 }
    }
}

/// Ce qu'une image clé et son jeu de paramètres disent du HDR.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seen {
    /// HEVC Main 10 (SPS).
    pub main10: bool,
    /// SEI « mastering display » ou OBU HDR_MDCV.
    pub mastering: Option<MasteringOnly>,
    /// SEI « content light level » ou OBU HDR_CLL : (MaxCLL, MaxFALL).
    pub light: Option<(u32, u32)>,
}

/// Primaires, blanc et luminances de l'écran de matriçage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasteringOnly {
    pub primaries: [(u32, u32); 3],
    pub white: (u32, u32),
    pub max_luminance: u32,
    pub min_luminance: u32,
}

impl Seen {
    /// Le flux porte des métadonnées HDR10.
    pub fn has_metadata(&self) -> bool {
        self.mastering.is_some() || self.light.is_some()
    }

    /// Les métadonnées à donner au téléviseur, complétées des valeurs par défaut.
    pub fn meta(&self) -> HdrMeta {
        let mut m = HdrMeta::default();
        if let Some(d) = self.mastering {
            m.primaries = d.primaries;
            m.white = d.white;
            m.max_luminance = d.max_luminance;
            m.min_luminance = d.min_luminance;
        }
        if let Some((cll, fall)) = self.light {
            // 0 : inconnu pour la norme ; le téléviseur aime mieux une valeur.
            if cll > 0 {
                m.max_cll = cll;
            }
            if fall > 0 {
                m.max_fall = fall;
            }
        }
        m
    }

    /// Ajoute ce que d'autres données disent (les dernières l'emportent).
    pub fn merge(&mut self, o: Seen) {
        self.main10 |= o.main10;
        self.mastering = o.mastering.or(self.mastering);
        self.light = o.light.or(self.light);
    }
}

/// Le flux est-il HDR ? Oui s'il porte des métadonnées HDR10 ; sinon, en HEVC
/// Main 10 quand la session a demandé le HDR (`expected`) : métadonnées par
/// défaut. Un flux SDR (8 bits, sans métadonnées) ne l'est jamais.
pub fn decide(seen: &Seen, hevc: bool, expected: bool) -> Option<HdrMeta> {
    if seen.has_metadata() || (expected && (seen.main10 || !hevc)) { Some(seen.meta()) } else { None }
}

/// Lit le HDR dans des données HEVC en Annex B (jeu de paramètres, image clé).
pub fn scan_hevc(data: &[u8]) -> Seen {
    let mut seen = Seen::default();
    for nal in annexb_nals(data) {
        if nal.len() < 3 {
            continue;
        }
        let kind = (nal[0] >> 1) & 0x3f;
        let rbsp = unescape(&nal[2..]);
        match kind {
            33 => seen.main10 |= sps_main10(&rbsp),
            39 => seen.merge(sei(&rbsp)),
            _ => {}
        }
    }
    seen
}

/// Les unités NAL d'un flux Annex B (sans code de départ).
fn annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (n, &s) in starts.iter().enumerate() {
        let mut end = starts.get(n + 1).map(|&next| next - 3).unwrap_or(data.len());
        // Le zéro de tête d'un code de départ sur 4 octets.
        while end > s && data[end - 1] == 0 {
            end -= 1;
        }
        out.push(&data[s..end]);
    }
    out
}

/// Retire les octets d'émulation (00 00 03 → 00 00).
fn unescape(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut zeros = 0;
    for &x in b {
        if zeros >= 2 && x == 3 {
            zeros = 0;
            continue;
        }
        zeros = if x == 0 { zeros + 1 } else { 0 };
        out.push(x);
    }
    out
}

/// Profil Main 10 dans un SPS HEVC : `general_profile_idc` 2, ou son drapeau
/// de compatibilité.
fn sps_main10(rbsp: &[u8]) -> bool {
    // Octet 0 : identifiant du VPS, couches, drapeau ; octet 1 : espace,
    // palier, profil ; octets 2 à 5 : les 32 drapeaux de compatibilité.
    if rbsp.len() < 6 {
        return false;
    }
    let idc = rbsp[1] & 0x1f;
    let compat = u32::from_be_bytes([rbsp[2], rbsp[3], rbsp[4], rbsp[5]]);
    idc == 2 || compat & (1 << (31 - 2)) != 0
}

fn be16(b: &[u8], i: usize) -> u32 {
    u32::from(u16::from_be_bytes([b[i], b[i + 1]]))
}

fn be32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Les messages d'un SEI HEVC : 137 (mastering display), 144 (content light level).
fn sei(rbsp: &[u8]) -> Seen {
    let mut seen = Seen::default();
    let mut i = 0;
    while i < rbsp.len() && rbsp[i] != 0x80 {
        let mut kind = 0usize;
        while i < rbsp.len() && rbsp[i] == 0xff {
            kind += 255;
            i += 1;
        }
        let Some(&k) = rbsp.get(i) else { break };
        kind += k as usize;
        i += 1;
        let mut size = 0usize;
        while i < rbsp.len() && rbsp[i] == 0xff {
            size += 255;
            i += 1;
        }
        let Some(&s) = rbsp.get(i) else { break };
        size += s as usize;
        i += 1;
        let Some(p) = rbsp.get(i..i + size) else { break };
        match kind {
            137 if p.len() >= 24 => {
                seen.mastering = Some(MasteringOnly {
                    primaries: [(be16(p, 0), be16(p, 2)), (be16(p, 4), be16(p, 6)), (be16(p, 8), be16(p, 10))],
                    white: (be16(p, 12), be16(p, 14)),
                    max_luminance: be32(p, 16),
                    min_luminance: be32(p, 20),
                })
            }
            144 if p.len() >= 4 => seen.light = Some((be16(p, 0), be16(p, 2))),
            _ => {}
        }
        i += size;
    }
    seen
}

/// Un entier LEB128 d'AV1 ; (valeur, octets lus).
fn leb128(b: &[u8]) -> Option<(usize, usize)> {
    let mut v = 0usize;
    for (i, &x) in b.iter().enumerate().take(8) {
        v |= ((x & 0x7f) as usize) << (7 * i);
        if x & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

/// Lit le HDR dans des OBU AV1 (en-tête de séquence, image clé) : les OBU de
/// métadonnées HDR_CLL (1) et HDR_MDCV (2). AV1 donne les primaires en rouge,
/// vert, bleu et en virgule fixe 0.16, les luminances en 24.8 et 18.14 :
/// ramenées aux unités de ST 2086.
pub fn scan_av1(data: &[u8]) -> Seen {
    let mut seen = Seen::default();
    let mut i = 0;
    while i < data.len() {
        let h = data[i];
        let kind = (h >> 3) & 0x0f;
        let ext = h & 0x04 != 0;
        let has_size = h & 0x02 != 0;
        i += 1 + usize::from(ext);
        let size = if has_size {
            let Some((v, n)) = data.get(i..).and_then(leb128) else { break };
            i += n;
            v
        } else {
            data.len().saturating_sub(i)
        };
        let Some(p) = data.get(i..i + size) else { break };
        if kind == 5
            && let Some((meta, n)) = leb128(p)
        {
            let p = &p[n..];
            match meta {
                1 if p.len() >= 4 => seen.light = Some((be16(p, 0), be16(p, 2))),
                2 if p.len() >= 24 => {
                    let c = |v: u32| (u64::from(v) * 50_000 / 65_536) as u32;
                    let (r, g, b) = ((c(be16(p, 0)), c(be16(p, 2))), (c(be16(p, 4)), c(be16(p, 6))), (c(be16(p, 8)), c(be16(p, 10))));
                    seen.mastering = Some(MasteringOnly {
                        primaries: [g, b, r],
                        white: (c(be16(p, 12)), c(be16(p, 14))),
                        max_luminance: (u64::from(be32(p, 16)) * 10_000 / 256) as u32,
                        min_luminance: (u64::from(be32(p, 20)) * 10_000 / 16_384) as u32,
                    });
                }
                _ => {}
            }
        }
        i += size;
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un SEI préfixe HEVC avec « mastering display » (BT.2020, D65, 1000 cd/m², 0,005) et « content light level ».
    fn sei_nal() -> Vec<u8> {
        let mut p = vec![0x4e, 0x01, 137, 24];
        for v in [8500u16, 39850, 6550, 2300, 35400, 14600, 15635, 16450] {
            p.extend_from_slice(&v.to_be_bytes());
        }
        p.extend_from_slice(&10_000_000u32.to_be_bytes());
        p.extend_from_slice(&50u32.to_be_bytes());
        p.extend_from_slice(&[144, 4, 0x03, 0xe8, 0x01, 0x90, 0x80]);
        p
    }

    #[test]
    fn hevc_sei_and_main10() {
        // SPS Main 10 : VPS 0, une couche ; profil 2, drapeau de compatibilité 2.
        let sps = [0x42, 0x01, 0x01, 0x02, 0x20, 0x00, 0x00, 0x00, 0x90];
        let mut data = vec![0, 0, 0, 1];
        data.extend_from_slice(&sps);
        data.extend_from_slice(&[0, 0, 1]);
        data.extend_from_slice(&sei_nal());
        let seen = scan_hevc(&data);
        assert!(seen.main10);
        assert_eq!(seen.light, Some((1000, 400)));
        assert_eq!(seen.meta(), HdrMeta::default());
        assert_eq!(decide(&seen, true, false), Some(HdrMeta::default()));
    }

    #[test]
    fn sdr_is_never_hdr() {
        // SPS Main (profil 1), sans SEI : jamais HDR, même demandé.
        let data = [0, 0, 0, 1, 0x42, 0x01, 0x01, 0x01, 0x40, 0x00, 0x00, 0x00, 0x90];
        let seen = scan_hevc(&data);
        assert!(!seen.main10 && !seen.has_metadata());
        assert_eq!(decide(&seen, true, true), None);
        // Main 10 sans SEI : HDR seulement demandé, avec les valeurs par défaut.
        let data = [0, 0, 0, 1, 0x42, 0x01, 0x01, 0x02, 0x20, 0x00, 0x00, 0x00, 0x90];
        assert_eq!(decide(&scan_hevc(&data), true, false), None);
        assert_eq!(decide(&scan_hevc(&data), true, true), Some(HdrMeta::default()));
    }

    #[test]
    fn tv_configs() {
        assert_eq!(from_configs(r#"{"returnValue":true,"configs":{"tv.model.supportHDR":true}}"#), Some(true));
        assert_eq!(from_configs(r#"{"returnValue":true,"configs":{"tv.model.supportHDR":"false","tv.config.supportDolbyHDRContents":"TRUE"}}"#), Some(true));
        assert_eq!(from_configs(r#"{"returnValue":true,"configs":{"tv.model.supportHDR":false}}"#), Some(false));
        assert_eq!(from_configs(r#"{"returnValue":true,"configs":{},"missingConfigs":["tv.model.supportHDR"]}"#), None);
        assert_eq!(from_configs("pas du JSON"), None);
    }

    #[test]
    fn emulation_prevention() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 0]), vec![0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn av1_metadata() {
        // OBU de métadonnées HDR_MDCV puis HDR_CLL (avec taille).
        let mut mdcv = vec![2u8];
        // Rouge, vert, bleu, blanc (0.16).
        for v in [0.708f64, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290] {
            mdcv.extend_from_slice(&((v * 65536.0).round() as u16).to_be_bytes());
        }
        mdcv.extend_from_slice(&(1000u32 * 256).to_be_bytes());
        mdcv.extend_from_slice(&((0.005f64 * 16384.0).round() as u32).to_be_bytes());
        let mut data = vec![(5 << 3) | 0x02, mdcv.len() as u8];
        data.extend_from_slice(&mdcv);
        data.extend_from_slice(&[(5 << 3) | 0x02, 5, 1, 0x03, 0xe8, 0x01, 0x90]);
        let seen = scan_av1(&data);
        let m = seen.meta();
        assert_eq!(m.max_luminance, 10_000_000);
        assert!((m.min_luminance as i32 - 50).abs() <= 1);
        assert!((m.primaries[0].0 as i32 - 8500).abs() <= 1 && (m.primaries[2].1 as i32 - 14600).abs() <= 1);
        assert_eq!((m.max_cll, m.max_fall), (1000, 400));
    }
}
