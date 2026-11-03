//! Références des images HEVC : quelles images passées chaque image utilise
//! pour se reconstruire. Après une perte, la console répare d'elle-même le
//! flux : sur le signalement (CORRUPTFRAME), elle code une image qui s'appuie
//! sur la dernière image saine, deux images plus tard (33 ms). Les images
//! d'entre-deux s'appuient sur la perdue : données au décodeur, elles le
//! bloquent jusqu'à la prochaine image clé (VideoToolbox, -12909, essai du
//! 03/11/2026 sur la PS5 Pro). On ne lui donne donc que les images dont
//! toutes les références sont saines : l'image reprend sans image clé.
//!
//! Lu : dans le SPS, ce qui mène aux ensembles de références et à la taille
//! du compteur d'ordre (POC) ; dans le PPS, ce qui précède le type de
//! tranche ; dans la première tranche de chaque image, son POC et ses
//! références (`short_term_ref_pic_set`, H.265 § 7.3.7 et 7.4.8). Les
//! images sont numérotées par la console une à une, comme leur POC : la
//! référence « POC − n » est l'image « numéro − n ». Tout écart (POC qui ne
//! suit pas les numéros, références à long terme, liste de mise à l'échelle,
//! H.264) rend le tri impossible : [`Verdict::Unknown`], et l'on revient à
//! l'image clé.

use std::collections::VecDeque;

/// Ce que dit le tri d'une image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Toutes ses références sont saines (ou image clé) : au décodeur.
    Clean,
    /// Elle s'appuie sur une image perdue ou écartée : à écarter.
    Broken,
    /// Le tri ne sait pas lire cette image.
    Unknown,
}

/// Images saines retenues (la console en garde 9 en référence).
const GOOD_KEPT: usize = 64;

#[derive(Debug, Clone, Default)]
struct RefSet {
    /// Écarts de POC (négatifs puis positifs) et « utilisée par l'image ».
    s0: Vec<(i32, bool)>,
    s1: Vec<(i32, bool)>,
}

impl RefSet {
    fn num_delta(&self) -> usize {
        self.s0.len() + self.s1.len()
    }
}

#[derive(Debug, Clone)]
struct Sps {
    log2_max_poc_lsb: u32,
    sets: Vec<RefSet>,
    long_term: bool,
    separate_colour_plane: bool,
}

#[derive(Debug, Clone, Copy)]
struct Pps {
    sps_id: u32,
    dependent_slices: bool,
    output_flag: bool,
    extra_bits: u32,
}

/// Le tri des images d'un flux HEVC.
#[derive(Debug, Default)]
pub struct RefTracker {
    sps: Vec<Option<Sps>>,
    pps: Vec<Option<Pps>>,
    good: VecDeque<u16>,
    /// Dernière image saine : son numéro et son POC (bits de poids faible).
    last: Option<(u16, u32)>,
}

impl RefTracker {
    pub fn new() -> Self {
        RefTracker::default()
    }

    /// Une image (Annex B, en-tête compris s'il y en a un) : faut-il la
    /// donner au décodeur ?
    pub fn check(&mut self, index: u16, data: &[u8]) -> Verdict {
        let mut slice = None;
        for nal in nal_units(data) {
            if nal.len() < 3 {
                continue;
            }
            let kind = (nal[0] >> 1) & 0x3f;
            let rbsp = unescape(&nal[2..]);
            match kind {
                33 => self.read_sps(&rbsp),
                34 => self.read_pps(&rbsp),
                0..=31 if slice.is_none() => slice = Some((kind, rbsp)),
                _ => {}
            }
        }
        let Some((kind, rbsp)) = slice else { return Verdict::Unknown };
        let verdict = self.judge(index, kind, &rbsp);
        if verdict == Verdict::Clean {
            self.good.push_back(index);
            if self.good.len() > GOOD_KEPT {
                self.good.pop_front();
            }
        }
        verdict
    }

    fn judge(&mut self, index: u16, kind: u8, rbsp: &[u8]) -> Verdict {
        let mut r = Bits::new(rbsp);
        let first = r.bit();
        if !first {
            return Verdict::Unknown;
        }
        let irap = (16..=23).contains(&kind);
        if irap {
            r.bit(); // no_output_of_prior_pics_flag
        }
        let Some(pps_id) = r.ue() else { return Verdict::Unknown };
        let Some(Some(pps)) = self.pps.get(pps_id as usize).copied() else { return Verdict::Unknown };
        let Some(Some(sps)) = self.sps.get(pps.sps_id as usize).cloned() else { return Verdict::Unknown };
        let _ = pps.dependent_slices; // première tranche : pas de dépendance
        r.skip(pps.extra_bits);
        if r.ue().is_none() {
            return Verdict::Unknown; // slice_type
        }
        if pps.output_flag {
            r.bit();
        }
        if sps.separate_colour_plane {
            r.skip(2);
        }
        if kind == 19 || kind == 20 {
            // IDR : POC 0, aucune référence.
            self.good.clear();
            self.last = Some((index, 0));
            return Verdict::Clean;
        }
        let Some(lsb) = r.bits(sps.log2_max_poc_lsb) else { return Verdict::Unknown };
        let max = 1u32 << sps.log2_max_poc_lsb;
        let set = if !r.bit() {
            match read_set(&mut r, sps.sets.len(), &sps.sets) {
                Some(s) => s,
                None => return Verdict::Unknown,
            }
        } else {
            let n = sps.sets.len();
            let idx = if n > 1 { r.bits(ceil_log2(n as u32)).unwrap_or(u32::MAX) as usize } else { 0 };
            match sps.sets.get(idx) {
                Some(s) => s.clone(),
                None => return Verdict::Unknown,
            }
        };
        if sps.long_term {
            return Verdict::Unknown;
        }
        if irap {
            // CRA, BLA : image intra, sans référence pour elle-même.
            self.last = Some((index, lsb));
            return Verdict::Clean;
        }
        // Le POC suit-il les numéros de la console ? Sinon, pas de tri.
        if let Some((last_index, last_lsb)) = self.last {
            let step = index.wrapping_sub(last_index) as u32;
            if (last_lsb + step) % max != lsb {
                log::info!("PlayStation : POC {lsb} de l'image {index} hors de la suite des numéros : tri arrêté");
                self.last = None;
                return Verdict::Unknown;
            }
        } else {
            return Verdict::Unknown;
        }
        let used = set.s0.iter().chain(set.s1.iter()).filter(|(_, u)| *u);
        for &(delta, _) in used {
            let r = index.wrapping_add(delta as i16 as u16);
            if !self.good.contains(&r) {
                return Verdict::Broken;
            }
        }
        self.last = Some((index, lsb));
        Verdict::Clean
    }

    fn read_sps(&mut self, rbsp: &[u8]) {
        if let Some((id, sps)) = parse_sps(rbsp) {
            let id = id as usize;
            if self.sps.len() <= id {
                self.sps.resize(id + 1, None);
            }
            self.sps[id] = Some(sps);
        }
    }

    fn read_pps(&mut self, rbsp: &[u8]) {
        let mut r = Bits::new(rbsp);
        let (Some(id), Some(sps_id)) = (r.ue(), r.ue()) else { return };
        let dependent_slices = r.bit();
        let output_flag = r.bit();
        let Some(extra_bits) = r.bits(3) else { return };
        if id > 63 {
            return;
        }
        let id = id as usize;
        if self.pps.len() <= id {
            self.pps.resize(id + 1, None);
        }
        self.pps[id] = Some(Pps { sps_id, dependent_slices, output_flag, extra_bits });
    }
}

fn ceil_log2(n: u32) -> u32 {
    32 - (n - 1).leading_zeros()
}

/// `short_term_ref_pic_set(idx)` (H.265 § 7.3.7, 7.4.8), `sets` : ceux
/// déjà lus (prédiction depuis l'un d'eux).
fn read_set(r: &mut Bits, idx: usize, sets: &[RefSet]) -> Option<RefSet> {
    let predicted = idx != 0 && r.bit();
    if predicted {
        let delta_idx = if idx == sets.len() { r.ue()? as usize + 1 } else { 1 };
        let ref_idx = idx.checked_sub(delta_idx)?;
        let refset = sets.get(ref_idx)?;
        let sign = r.bit();
        let abs = r.ue()? as i32 + 1;
        let delta_rps = if sign { -abs } else { abs };
        let n = refset.num_delta();
        let mut used = Vec::with_capacity(n + 1);
        let mut use_delta = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            let u = r.bit();
            used.push(u);
            use_delta.push(if u { true } else { r.bit() });
        }
        // (7-61) et (7-62) : les nouveaux écarts, dans l'ordre.
        let s0_ref: Vec<i32> = refset.s0.iter().map(|x| x.0).collect();
        let s1_ref: Vec<i32> = refset.s1.iter().map(|x| x.0).collect();
        let (n0, n1) = (s0_ref.len(), s1_ref.len());
        let mut s0 = Vec::new();
        for j in (0..n1).rev() {
            let d = s1_ref[j] + delta_rps;
            if d < 0 && use_delta[n0 + j] {
                s0.push((d, used[n0 + j]));
            }
        }
        if delta_rps < 0 && use_delta[n] {
            s0.push((delta_rps, used[n]));
        }
        for j in 0..n0 {
            let d = s0_ref[j] + delta_rps;
            if d < 0 && use_delta[j] {
                s0.push((d, used[j]));
            }
        }
        let mut s1 = Vec::new();
        for j in (0..n0).rev() {
            let d = s0_ref[j] + delta_rps;
            if d > 0 && use_delta[j] {
                s1.push((d, used[j]));
            }
        }
        if delta_rps > 0 && use_delta[n] {
            s1.push((delta_rps, used[n]));
        }
        for j in 0..n1 {
            let d = s1_ref[j] + delta_rps;
            if d > 0 && use_delta[n0 + j] {
                s1.push((d, used[n0 + j]));
            }
        }
        Some(RefSet { s0, s1 })
    } else {
        let neg = r.ue()?;
        let pos = r.ue()?;
        if neg > 16 || pos > 16 {
            return None;
        }
        let mut s0 = Vec::new();
        let mut poc = 0i32;
        for _ in 0..neg {
            poc -= r.ue()? as i32 + 1;
            s0.push((poc, r.bit()));
        }
        let mut s1 = Vec::new();
        poc = 0;
        for _ in 0..pos {
            poc += r.ue()? as i32 + 1;
            s1.push((poc, r.bit()));
        }
        Some(RefSet { s0, s1 })
    }
}

/// Le SPS jusqu'aux références à long terme (H.265 § 7.3.2.2).
fn parse_sps(rbsp: &[u8]) -> Option<(u32, Sps)> {
    let mut r = Bits::new(rbsp);
    r.skip(4); // sps_video_parameter_set_id
    let max_sub_layers = r.bits(3)?;
    r.skip(1);
    // profile_tier_level(1, max_sub_layers)
    r.skip(2 + 1 + 5 + 32 + 4 + 43 + 1 + 8);
    let mut profile_present = Vec::new();
    let mut level_present = Vec::new();
    for _ in 0..max_sub_layers {
        profile_present.push(r.bit());
        level_present.push(r.bit());
    }
    if max_sub_layers > 0 {
        r.skip(2 * (8 - max_sub_layers));
    }
    for i in 0..max_sub_layers as usize {
        if profile_present[i] {
            r.skip(88);
        }
        if level_present[i] {
            r.skip(8);
        }
    }
    let id = r.ue()?;
    let chroma = r.ue()?;
    let separate_colour_plane = chroma == 3 && r.bit();
    r.ue()?;
    r.ue()?; // largeur, hauteur
    if r.bit() {
        for _ in 0..4 {
            r.ue()?;
        }
    }
    r.ue()?;
    r.ue()?; // profondeurs
    let log2_max_poc_lsb = r.ue()? + 4;
    if log2_max_poc_lsb > 16 {
        return None;
    }
    let all = r.bit();
    for _ in (if all { 0 } else { max_sub_layers })..=max_sub_layers {
        r.ue()?;
        r.ue()?;
        r.ue()?;
    }
    for _ in 0..6 {
        r.ue()?;
    }
    if r.bit() && r.bit() {
        return None; // listes de mise à l'échelle dans le SPS : pas lues
    }
    r.skip(2); // amp, sao
    if r.bit() {
        r.skip(8);
        r.ue()?;
        r.ue()?;
        r.skip(1);
    }
    let n = r.ue()?;
    if n > 64 {
        return None;
    }
    let mut sets = Vec::new();
    for i in 0..n as usize {
        let s = read_set(&mut r, i, &sets)?;
        sets.push(s);
    }
    let long_term = r.bit();
    if r.overrun() {
        return None;
    }
    Some((id, Sps { log2_max_poc_lsb, sets, long_term, separate_colour_plane }))
}

/// Les unités NAL d'un flux Annex B (sans le code de début).
fn nal_units(data: &[u8]) -> impl Iterator<Item = &[u8]> {
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
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map(|n| n - 3).unwrap_or(data.len());
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        out.push(&data[s..e]);
    }
    out.into_iter()
}

/// Retire les octets d'échappement (00 00 03) ; le début seul suffit.
fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().min(256));
    let mut zeros = 0;
    for &b in data.iter().take(512) {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Bits<'a> {
    fn new(d: &'a [u8]) -> Self {
        Bits { d, pos: 0 }
    }
    fn overrun(&self) -> bool {
        self.pos > self.d.len() * 8
    }
    fn bit(&mut self) -> bool {
        let b = self.d.get(self.pos / 8).map(|x| (x >> (7 - self.pos % 8)) & 1 == 1).unwrap_or(false);
        self.pos += 1;
        b
    }
    fn skip(&mut self, n: u32) {
        self.pos += n as usize;
    }
    fn bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            v = (v << 1) | self.bit() as u32;
        }
        (!self.overrun()).then_some(v)
    }
    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while !self.bit() {
            zeros += 1;
            if zeros > 31 || self.overrun() {
                return None;
            }
        }
        let rest = self.bits(zeros)?;
        Some((1u32 << zeros) - 1 + rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Le flux réel de l'essai : l'image 600 jetée, 601 et 602 s'appuient
    /// sur elle, 603 sur 599 (réparation de la console), la suite reprend.
    #[test]
    fn real_stream_recovers_two_frames_after_a_loss() {
        let text = include_str!("refs_vectors.txt");
        let mut header = Vec::new();
        let mut frames = Vec::new();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["header", h] => header = hex(h),
                ["frame", n, h] => frames.push((n.parse::<u16>().unwrap(), hex(h))),
                _ => {}
            }
        }
        let mut t = RefTracker::new();
        let mut verdicts = Vec::new();
        for (k, (index, data)) in frames.iter().enumerate() {
            let mut d = if k == 0 { header.clone() } else { Vec::new() };
            d.extend_from_slice(data);
            // La suite des POC s'établit sur la première image vue : on la
            // tient pour saine (comme après une image clé).
            if k == 0 {
                assert_eq!(t.check(*index, &d), Verdict::Unknown);
                t.good.push_back(*index);
                t.last = Some((*index, 10));
                t.good.extend([index - 1, index - 2]);
                continue;
            }
            verdicts.push((*index, t.check(*index, &d)));
        }
        assert_eq!(
            verdicts,
            vec![
                (598, Verdict::Clean),
                (599, Verdict::Clean),
                (601, Verdict::Broken),
                (602, Verdict::Broken),
                (603, Verdict::Clean),
                (604, Verdict::Clean),
                (605, Verdict::Clean),
            ]
        );
    }

    #[test]
    fn exp_golomb() {
        // 1 → 0, 010 → 1, 011 → 2, 00100 → 3
        let mut b = Bits::new(&[0b1010_0110, 0b0100_0000]);
        assert_eq!((b.ue(), b.ue(), b.ue(), b.ue()), (Some(0), Some(1), Some(2), Some(3)));
    }

    #[test]
    fn idr_resets_and_is_clean() {
        let mut t = RefTracker::new();
        t.good.push_back(5);
        // Sans paramètres du flux : rien à lire.
        assert_eq!(t.check(6, &[0, 0, 1, 0x26, 0x01, 0x80]), Verdict::Unknown);
    }
}
