//! Chiffrement du flux Takion, d'après `gkcrypt.c` et `ecdh.c` de chiaki-ng.
//!
//! - **Échange de clés** ([`Ecdh`]) : ECDH sur secp256k1. La clé publique
//!   (65 octets, non compressée) est signée par HMAC-SHA256 avec la clé de
//!   poignée de main (16 octets tirés par session) ; le secret partagé est
//!   la coordonnée x (32 octets).
//! - **Clés du flux** ([`GkCrypt::new`]) : `HMAC-SHA256(secret, 01 index 00
//!   clé_de_poignée_de_main 01 00)` donne la clé AES (16 premiers octets) et
//!   le vecteur d'initialisation (16 suivants). Index 2 pour ce que l'app
//!   envoie, 3 pour ce qu'elle reçoit.
//! - **Chiffrement** : AES-128 en mode compteur, compteur = vecteur
//!   d'initialisation + position / 16, additionné en petit-boutiste. La
//!   position (`key_pos`) avance avec les octets chiffrés.
//! - **Authentification** ([`GkCrypt::gmac`]) : GMAC (AES-GCM sans texte
//!   chiffré, vecteur d'initialisation de 16 octets) tronqué à 4 octets ; sa
//!   clé est renouvelée tous les 45 000 octets de position.

use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit};
use aes_gcm::aead::AeadInPlace;
use aes_gcm::aead::consts::U16;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// Taille d'un bloc et des clés.
pub const BLOCK_SIZE: usize = 16;
/// Taille du GMAC dans les paquets.
pub const GMAC_SIZE: usize = 4;
/// Taille de la clé de poignée de main.
pub const HANDSHAKE_KEY_SIZE: usize = 16;
/// Taille du secret ECDH.
pub const ECDH_SECRET_SIZE: usize = 32;
const GMAC_KEY_REFRESH_KEY_POS: u64 = 45000;
const GMAC_KEY_REFRESH_IV_OFFSET: u64 = 44910;

type Block = [u8; BLOCK_SIZE];

/// `base + v`, en petit-boutiste sur 16 octets (`counter_add`).
fn counter_add(base: &Block, v: u64) -> Block {
    let mut out = *base;
    let mut carry = v as u128;
    for b in out.iter_mut() {
        if carry == 0 {
            break;
        }
        let r = *b as u128 + carry;
        *b = (r & 0xff) as u8;
        carry = r >> 8;
    }
    out
}

/// Clé GMAC de l'index `index` (`chiaki_gkcrypt_gen_gmac_key`).
pub(crate) fn gmac_key(index: u64, key_base: &Block, iv: &Block) -> Block {
    let mut data = [0u8; 32];
    data[..16].copy_from_slice(key_base);
    data[16..].copy_from_slice(&counter_add(iv, index.wrapping_mul(GMAC_KEY_REFRESH_IV_OFFSET)));
    let md = Sha256::digest(data);
    let mut out = [0u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        out[i] = md[i] ^ md[i + 16];
    }
    out
}

/// Chiffrement d'un sens du flux.
pub struct GkCrypt {
    index: u8,
    #[cfg_attr(not(test), allow(dead_code))]
    key_base: Block,
    iv: Block,
    gmac_base: Block,
    gmac_current: Block,
    gmac_index: u64,
    aes: Aes128,
}

impl std::fmt::Debug for GkCrypt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GkCrypt").field("index", &self.index).finish_non_exhaustive()
    }
}

impl GkCrypt {
    pub fn new(index: u8, handshake_key: &[u8; HANDSHAKE_KEY_SIZE], secret: &[u8; ECDH_SECRET_SIZE]) -> Self {
        let mut data = Vec::with_capacity(3 + HANDSHAKE_KEY_SIZE + 2);
        data.extend_from_slice(&[1, index, 0]);
        data.extend_from_slice(handshake_key);
        data.extend_from_slice(&[1, 0]);
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).expect("clé HMAC");
        mac.update(&data);
        let h = mac.finalize().into_bytes();
        let mut key_base = [0u8; BLOCK_SIZE];
        let mut iv = [0u8; BLOCK_SIZE];
        key_base.copy_from_slice(&h[..16]);
        iv.copy_from_slice(&h[16..32]);
        Self::from_parts(index, key_base, iv)
    }

    fn from_parts(index: u8, key_base: Block, iv: Block) -> Self {
        let gmac_base = gmac_key(0, &key_base, &iv);
        GkCrypt { index, key_base, iv, gmac_base, gmac_current: gmac_base, gmac_index: 0, aes: Aes128::new(&key_base.into()) }
    }

    /// Flux de clé à partir de `key_pos` (multiple de 16), `len` multiple de 16.
    fn key_stream(&self, key_pos: u64, len: usize) -> Vec<u8> {
        debug_assert!(key_pos % BLOCK_SIZE as u64 == 0 && len % BLOCK_SIZE == 0);
        let mut out = vec![0u8; len];
        let first = key_pos / BLOCK_SIZE as u64;
        for (i, chunk) in out.chunks_exact_mut(BLOCK_SIZE).enumerate() {
            let mut block = counter_add(&self.iv, first + i as u64).into();
            self.aes.encrypt_block(&mut block);
            chunk.copy_from_slice(&block);
        }
        out
    }

    /// Chiffre ou déchiffre `buf` sur place à la position `key_pos`
    /// (`chiaki_gkcrypt_decrypt`, le même pour les deux sens).
    pub fn crypt(&self, key_pos: u64, buf: &mut [u8]) {
        let pre = (key_pos % BLOCK_SIZE as u64) as usize;
        let full = (pre + buf.len()).div_ceil(BLOCK_SIZE) * BLOCK_SIZE;
        let ks = self.key_stream(key_pos - pre as u64, full);
        for (b, k) in buf.iter_mut().zip(&ks[pre..]) {
            *b ^= k;
        }
    }

    /// GMAC de `buf` à la position `key_pos` (`chiaki_gkcrypt_gmac`).
    pub fn gmac(&mut self, key_pos: u64, buf: &[u8]) -> [u8; GMAC_SIZE] {
        let iv = counter_add(&self.iv, key_pos / 16);
        let key_index = key_pos.saturating_sub(1) / GMAC_KEY_REFRESH_KEY_POS;
        let key = if key_index > self.gmac_index {
            self.gmac_current = gmac_key(key_index, &self.gmac_base, &self.iv);
            self.gmac_index = key_index;
            self.gmac_current
        } else if key_index < self.gmac_index {
            if key_index == 0 { self.gmac_base } else { gmac_key(key_index, &self.gmac_base, &self.iv) }
        } else {
            self.gmac_current
        };
        let cipher = aes_gcm::AesGcm::<Aes128, U16>::new(&key.into());
        let tag = cipher
            .encrypt_in_place_detached(&iv.into(), buf, &mut [])
            .expect("GMAC sans texte chiffré");
        let mut out = [0u8; GMAC_SIZE];
        out.copy_from_slice(&tag[..GMAC_SIZE]);
        out
    }

    #[cfg(test)]
    fn for_gmac_test(gmac_current: Block, iv: Block) -> Self {
        GkCrypt {
            index: 0,
            key_base: [0; 16],
            iv,
            gmac_base: [0; 16],
            gmac_current,
            gmac_index: 0,
            aes: Aes128::new(&[0u8; 16].into()),
        }
    }
}

/// Moitié locale de l'échange ECDH.
pub struct Ecdh {
    secret: k256::SecretKey,
}

impl Ecdh {
    /// Une clé tirée au hasard.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        loop {
            let _ = getrandom::getrandom(&mut bytes);
            if let Ok(secret) = k256::SecretKey::from_slice(&bytes) {
                return Ecdh { secret };
            }
        }
    }

    #[cfg(test)]
    fn from_private(bytes: &[u8]) -> Self {
        Ecdh { secret: k256::SecretKey::from_slice(bytes).unwrap() }
    }

    /// Clé publique non compressée (65 octets) et sa signature
    /// (`chiaki_ecdh_get_local_pub_key`).
    pub fn public_key(&self, handshake_key: &[u8; HANDSHAKE_KEY_SIZE]) -> (Vec<u8>, [u8; 32]) {
        use k256::elliptic_curve::sec1::ToEncodedPoint;
        let point = self.secret.public_key().to_encoded_point(false);
        let key = point.as_bytes().to_vec();
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(handshake_key).expect("clé HMAC");
        mac.update(&key);
        (key, mac.finalize().into_bytes().into())
    }

    /// Secret partagé avec la clé publique de la console
    /// (`chiaki_ecdh_derive_secret` ; comme chiaki-ng, la signature de la
    /// console n'est pas vérifiée). `None` si la clé est illisible.
    pub fn derive(&self, remote: &[u8]) -> Option<[u8; ECDH_SECRET_SIZE]> {
        let remote = k256::PublicKey::from_sec1_bytes(remote).ok()?;
        let shared = k256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), remote.as_affine());
        Some((*shared.raw_secret_bytes()).into())
    }
}

/// Position de clé sur 64 bits d'après ses 32 bits bas et la précédente
/// (`chiaki_key_state_request_pos`).
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyState {
    prev: u64,
}

pub(crate) fn seq32_lt(a: u32, b: u32) -> bool {
    a != b && ((a < b && b - a < 1 << 31) || (a > b && a - b > 1 << 31))
}

pub(crate) fn seq32_gt(a: u32, b: u32) -> bool {
    a != b && !seq32_lt(a, b)
}

impl KeyState {
    pub fn request(&mut self, low: u32, commit: bool) -> u64 {
        let prev_low = self.prev as u32;
        let mut high = (self.prev >> 32) as u32;
        if seq32_gt(low, prev_low) && low < prev_low {
            high = high.wrapping_add(1);
        } else if seq32_lt(low, prev_low) && low > prev_low && high > 0 {
            high -= 1;
        }
        let res = ((high as u64) << 32) | low as u64;
        if commit {
            self.prev = res;
        }
        res
    }

    pub fn commit(&mut self, pos: u64) {
        self.prev = pos;
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_vectors::*;
    use super::*;

    fn arr<const N: usize>(s: &[u8]) -> [u8; N] {
        s.try_into().unwrap()
    }

    #[test]
    fn ecdh_matches_chiaki() {
        let e = Ecdh::from_private(&GK_ECDH_LOCAL_PRIVATE_KEY);
        let (key, sig) = e.public_key(&arr(&GK_ECDH_HANDSHAKE_KEY));
        assert_eq!(key, GK_ECDH_LOCAL_PUBLIC_KEY.to_vec());
        assert_eq!(sig, GK_ECDH_LOCAL_PUBLIC_KEY_SIG);
        assert_eq!(e.derive(&GK_ECDH_REMOTE_PUBLIC_KEY).unwrap(), GK_ECDH_SECRET);
    }

    #[test]
    fn key_stream_matches_chiaki() {
        let g = GkCrypt::new(42, &arr(&GK_KEY_STREAM_HANDSHAKE_KEY), &arr(&GK_KEY_STREAM_ECDH_SECRET));
        assert_eq!(g.key_base, GK_KEY_STREAM_GKCRYPT_KEY);
        assert_eq!(g.iv, GK_KEY_STREAM_GKCRYPT_IV);
        assert_eq!(g.key_stream(0x30, 0x20), GK_KEY_STREAM_KEY_STREAM.to_vec());
        assert_eq!(g.key_stream((1 << 32) + 0x420, 0x20), GK_KEY_STREAM_KEY_STREAM_HIGH.to_vec());
    }

    #[test]
    fn endecrypt_matches_chiaki() {
        let g = GkCrypt::new(42, &arr(&GK_ENDECRYPT_HANDSHAKE_KEY), &arr(&GK_ENDECRYPT_ECDH_SECRET));
        let mut buf = GK_ENDECRYPT_CLEAR_DATA[..0x25].to_vec();
        g.crypt(0x11, &mut buf);
        assert_eq!(buf, GK_ENDECRYPT_ENC_DATA[..0x25].to_vec());
        let mut buf = GK_ENDECRYPT_CLEAR_DATA[..0x25].to_vec();
        g.crypt((1 << 32) + 0x420, &mut buf);
        assert_eq!(buf, GK_ENDECRYPT_ENC_DATA_HIGH[..0x25].to_vec());
    }

    #[test]
    fn gmac_matches_chiaki() {
        let mut g = GkCrypt::for_gmac_test(GK_GMAC_GKCRYPT_KEY, GK_GMAC_GKCRYPT_IV);
        assert_eq!(g.gmac(0x69a0, &GK_GMAC_BUF), GK_GMAC_GMAC_EXPECTED);
        let mut g = GkCrypt::for_gmac_test(GK_GMAC_GKCRYPT_KEY, GK_GMAC_GKCRYPT_IV);
        assert_eq!(g.gmac((1 << 32) + 0x420, &GK_GMAC_BUF), GK_GMAC_GMAC_EXPECTED_HIGH);
        let mut g = GkCrypt::for_gmac_test(GK_GMAC_SPLIT_GKCRYPT_KEY, GK_GMAC_SPLIT_GKCRYPT_IV);
        assert_eq!(g.gmac(0xadf0, &GK_GMAC_SPLIT_BUF), GK_GMAC_SPLIT_GMAC_EXPECTED);
        let mut g = GkCrypt::for_gmac_test(GK_GMAC_SPLIT_GKCRYPT_KEY, GK_GMAC_SPLIT_GKCRYPT_IV);
        assert_eq!(g.gmac((1 << 32) + 0x420, &GK_GMAC_SPLIT_BUF), GK_GMAC_SPLIT_GMAC_EXPECTED_HIGH);
    }

    #[test]
    fn gmac_key_matches_chiaki() {
        assert_eq!(gmac_key(1, &GK_GEN_GMAC_KEY_KEY_INITIAL, &GK_GEN_GMAC_KEY_IV), GK_GEN_GMAC_KEY_KEY_RESULT);
    }

    #[test]
    fn gmac_at_key_refresh_matches_chiaki() {
        let mut g = GkCrypt::new(
            3,
            &arr(&GK_GMAC_MULTIPLE_OF_KEY_REFRESH_HANDSHAKE_KEY),
            &arr(&GK_GMAC_MULTIPLE_OF_KEY_REFRESH_ECDH_SECRET),
        );
        assert_eq!(g.gmac(0x6b1de0, &GK_GMAC_MULTIPLE_OF_KEY_REFRESH_DATA), GK_GMAC_MULTIPLE_OF_KEY_REFRESH_GMAC_EXPECTED);
    }

    #[test]
    fn key_state_wraps() {
        let mut k = KeyState::default();
        assert_eq!(k.request(0x10, true), 0x10);
        assert_eq!(k.request(0xffff_fff0, true), 0xffff_fff0);
        assert_eq!(k.request(0x20, true), (1 << 32) + 0x20);
        assert_eq!(k.request(0xffff_ff00, false), 0xffff_ff00);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// `cargo test --release --lib gkcrypt::bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn per_packet_cost() {
        let mut c = GkCrypt::new(3, &[1; 16], &[2; 32]);
        let mut buf = vec![7u8; 1454];
        let n = 20_000u64;
        let t = std::time::Instant::now();
        for i in 0..n {
            let pos = 0x1000 + i * 1400;
            let _ = c.gmac(pos, &buf);
            c.crypt(pos + 16, &mut buf);
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        println!("GMAC + déchiffrement : {us:.2} µs par paquet de 1454 octets ; à 9 000 paquets/s : {:.1} % d'un cœur", us * 9000.0 / 1e4);
    }
}
