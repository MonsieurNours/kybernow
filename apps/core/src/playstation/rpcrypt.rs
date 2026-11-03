//! Chiffrement de l'association et du contrôle de session, d'après
//! `rpcrypt.c` de chiaki-ng.
//!
//! Deux clés de 16 octets : `bright`, la clé AES-128, et `ambassador`, qui
//! donne le vecteur d'initialisation avec un compteur (HMAC-SHA256 d'une clé
//! fixe sur `ambassador` suivi du compteur en gros-boutiste, 16 premiers
//! octets). Le chiffrement est de l'AES-128 en CFB-128, sur n'importe quelle
//! longueur.
//!
//! - **Association** ([`RpCrypt::regist`]) : `ambassador` est tiré au hasard,
//!   `bright` vient d'une table de clés et du code à 8 chiffres.
//! - **Session** ([`RpCrypt::auth`]) : les deux clés viennent du `nonce` donné
//!   par la console et du `morning` (la clé `RP-Key` reçue à l'association).
//!
//! Les noms (`bright`, `ambassador`, `morning`, `aeropause`) sont ceux de
//! chiaki-ng, gardés pour suivre son code. Les tables sont dans `keys.rs`.

use aes::Aes128;
use cfb_mode::cipher::{AsyncStreamCipher, KeyIvInit};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::discovery::Target;
use super::keys::*;

/// Taille des clés.
pub const KEY_SIZE: usize = 16;

type Key = [u8; KEY_SIZE];

/// Clés d'un échange chiffré avec une console.
#[derive(Clone)]
pub struct RpCrypt {
    target: Target,
    bright: Key,
    ambassador: Key,
}

impl std::fmt::Debug for RpCrypt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpCrypt").field("target", &self.target).finish_non_exhaustive()
    }
}

/// Valeur numérique de `ChiakiTarget`, pour les comparaisons de chiaki-ng
/// (`target < CHIAKI_TARGET_PS4_10`).
pub(crate) fn target_code(target: Target) -> u32 {
    match target {
        Target::Ps4Unknown => 0,
        Target::Ps4V8 => 800,
        Target::Ps4V9 => 900,
        Target::Ps4V10 => 1000,
        Target::Ps5Unknown => 1_000_000,
        Target::Ps5V1 => 1_000_100,
    }
}

pub(crate) fn is_ps5(target: Target) -> bool {
    target_code(target) >= target_code(Target::Ps5Unknown)
}

/// PS4 avant le système 8.0 (et PS4 inconnue) : l'ancien protocole.
pub(crate) fn is_ps4_pre10(target: Target) -> bool {
    target_code(target) < target_code(Target::Ps4V10)
}

/// `bright` et `ambassador` d'une session, d'après le `nonce` de la console
/// et `morning` (`chiaki_rpcrypt_bright_ambassador`).
pub fn bright_ambassador(target: Target, nonce: &Key, morning: &Key) -> (Key, Key) {
    let mut bright = [0u8; KEY_SIZE];
    let mut ambassador = [0u8; KEY_SIZE];
    if matches!(target, Target::Ps4V8 | Target::Ps4V9) {
        for i in 0..KEY_SIZE {
            ambassador[i] = nonce[i].wrapping_sub(i as u8).wrapping_sub(0x27) ^ ECHO_A[i];
            bright[i] = morning[i].wrapping_sub(i as u8).wrapping_add(0x34) ^ ECHO_B[i] ^ nonce[i];
        }
        return (bright, ambassador);
    }
    // chiaki-ng : `bright_ambassador`, qui refuse les cibles avant PS4 10
    // par une assertion ; une PS4 inconnue y passe comme une PS4 10.
    let ps5 = is_ps5(target);
    let (keys_a, keys_b) = if ps5 { (&KEYS_A_PS5, &KEYS_B_PS5) } else { (&KEYS_A_PS4, &KEYS_B_PS4) };
    let key = &keys_a[(nonce[0] >> 3) as usize * 0x70..];
    for i in 0..KEY_SIZE {
        let mut v = nonce[i];
        if ps5 {
            v = v.wrapping_sub(0x2d).wrapping_sub(i as u8);
        } else {
            v = v.wrapping_add(0x36).wrapping_add(i as u8);
        }
        ambassador[i] = v ^ key[i];
    }
    let key = &keys_b[(nonce[7] >> 3) as usize * 0x70..];
    for i in 0..KEY_SIZE {
        bright[i] = if ps5 {
            morning[i].wrapping_add(0x18).wrapping_add(i as u8) ^ nonce[i] ^ key[i]
        } else {
            (key[i] ^ morning[i]).wrapping_add(0x21).wrapping_add(i as u8) ^ nonce[i]
        };
    }
    (bright, ambassador)
}

/// `ambassador` masqué, tel que l'envoie la demande d'association d'une PS4
/// avant 8.0 (`chiaki_rpcrypt_aeropause_ps4_pre10`).
pub fn aeropause_ps4_pre10(ambassador: &Key) -> Key {
    let mut out = [0u8; KEY_SIZE];
    for i in 0..KEY_SIZE {
        out[i] = ambassador[i].wrapping_sub(i as u8).wrapping_sub(0x29) ^ ECHO_B[i];
    }
    out
}

/// `ambassador` masqué par la table `keys_1` (`chiaki_rpcrypt_aeropause`).
/// `None` avant PS4 8.0 ou si `key_1_off` sort de la table.
pub fn aeropause(target: Target, key_1_off: usize, ambassador: &Key) -> Option<Key> {
    if is_ps4_pre10(target) || key_1_off >= 0x20 {
        return None;
    }
    let (keys, wurzelbert) = if is_ps5(target) { (&AEROPAUSE_PS5, 0u8.wrapping_sub(0x2d)) } else { (&AEROPAUSE_PS4, 0x29) };
    let mut out = [0u8; KEY_SIZE];
    for i in 0..KEY_SIZE {
        let k = keys[i * 0x20 + key_1_off];
        out[i] = (ambassador[i] ^ k).wrapping_add(wurzelbert).wrapping_add(i as u8);
    }
    Some(out)
}

impl RpCrypt {
    /// Clés d'une session (`chiaki_rpcrypt_init_auth`).
    pub fn auth(target: Target, nonce: &Key, morning: &Key) -> Self {
        let (bright, ambassador) = bright_ambassador(target, nonce, morning);
        Self { target, bright, ambassador }
    }

    /// Clés de l'association d'une PS4 avant 8.0
    /// (`chiaki_rpcrypt_init_regist_ps4_pre10`).
    pub fn regist_ps4_pre10(ambassador: &Key, pin: u32) -> Self {
        let mut bright = REGIST_AES_KEY_PS4_PRE10;
        for (i, b) in pin.to_be_bytes().iter().enumerate() {
            bright[i] ^= b;
        }
        Self { target: Target::Ps4V9, bright, ambassador: *ambassador }
    }

    /// Clés de l'association (`chiaki_rpcrypt_init_regist`). `None` avant
    /// PS4 8.0 ou si `key_0_off` sort de la table.
    pub fn regist(target: Target, ambassador: &Key, key_0_off: usize, pin: u32) -> Option<Self> {
        if is_ps4_pre10(target) || key_0_off >= 0x20 {
            return None;
        }
        let keys = if is_ps5(target) { &REGIST_PS5 } else { &REGIST_PS4 };
        let mut bright = [0u8; KEY_SIZE];
        for (i, b) in bright.iter_mut().enumerate() {
            *b = keys[i * 0x20 + key_0_off];
        }
        for (i, b) in pin.to_be_bytes().iter().enumerate() {
            bright[0xc + i] ^= b;
        }
        Some(Self { target, bright, ambassador: *ambassador })
    }

    pub fn ambassador(&self) -> &Key {
        &self.ambassador
    }

    pub fn bright(&self) -> &Key {
        &self.bright
    }

    /// Clé HMAC du vecteur d'initialisation (`rpcrypt_hmac_key`). Comme chez
    /// chiaki-ng, seule `Ps5V1` prend la clé PS5 ; une PS5 inconnue prend
    /// celle de la PS4.
    fn hmac_key(&self) -> &'static Key {
        match self.target {
            Target::Ps5V1 => &HMAC_PS5,
            Target::Ps4V8 | Target::Ps4V9 => &HMAC_PS4_PRE10,
            _ => &HMAC_PS4,
        }
    }

    /// Vecteur d'initialisation pour `counter` (`chiaki_rpcrypt_generate_iv`).
    pub fn iv(&self, counter: u64) -> Key {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.hmac_key()).expect("clé HMAC de 16 octets");
        mac.update(&self.ambassador);
        mac.update(&counter.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let mut iv = [0u8; KEY_SIZE];
        iv.copy_from_slice(&digest[..KEY_SIZE]);
        iv
    }

    /// Chiffre `buf` sur place (AES-128-CFB128).
    pub fn encrypt(&self, counter: u64, buf: &mut [u8]) {
        cfb_mode::Encryptor::<Aes128>::new(&self.bright.into(), &self.iv(counter).into()).encrypt(buf);
    }

    /// Déchiffre `buf` sur place.
    pub fn decrypt(&self, counter: u64, buf: &mut [u8]) {
        cfb_mode::Decryptor::<Aes128>::new(&self.bright.into(), &self.iv(counter).into()).decrypt(buf);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_vectors::*;
    use super::*;

    #[test]
    fn bright_ambassador_ps4_pre10() {
        let (bright, ambassador) = bright_ambassador(
            Target::Ps4V9,
            &RPCRYPT_BRIGHT_AMBASSADOR_PS4_PRE10_NONCE,
            &RPCRYPT_BRIGHT_AMBASSADOR_PS4_PRE10_MORNING,
        );
        assert_eq!(bright, RPCRYPT_BRIGHT_AMBASSADOR_PS4_PRE10_BRIGHT_EXPECTED);
        assert_eq!(ambassador, RPCRYPT_BRIGHT_AMBASSADOR_PS4_PRE10_AMBASSADOR_EXPECTED);
    }

    #[test]
    fn bright_ambassador_ps4() {
        let (bright, ambassador) = bright_ambassador(
            Target::Ps4V10,
            &RPCRYPT_BRIGHT_AMBASSADOR_NONCE_LOCAL,
            &RPCRYPT_BRIGHT_AMBASSADOR_MORNING_LOCAL,
        );
        assert_eq!(ambassador, RPCRYPT_BRIGHT_AMBASSADOR_AMBASSADOR_EXPECTED);
        assert_eq!(bright, RPCRYPT_BRIGHT_AMBASSADOR_BRIGHT_EXPECTED);
    }

    #[test]
    fn iv_ps4_pre10() {
        let c = RpCrypt::auth(Target::Ps4V9, &RPCRYPT_IV_PS4_PRE10_NONCE, &RPCRYPT_IV_PS4_PRE10_MORNING);
        assert_eq!(c.iv(0), RPCRYPT_IV_PS4_PRE10_IV_A_EXPECTED);
        assert_eq!(c.iv(0), RPCRYPT_IV_PS4_PRE10_IV_A_EXPECTED);
        assert_eq!(c.iv(0x0102030405060708), RPCRYPT_IV_PS4_PRE10_IV_B_EXPECTED);
    }

    #[test]
    fn iv_regist() {
        let c = RpCrypt::regist(Target::Ps4V10, &RPCRYPT_IV_REGIST_PS4_AMBASSADOR, 0, 0).unwrap();
        assert_eq!(c.iv(0), RPCRYPT_IV_REGIST_PS4_IV_EXPECTED);
        let c = RpCrypt::regist(Target::Ps5V1, &RPCRYPT_IV_REGIST_PS5_AMBASSADOR, 0, 0).unwrap();
        assert_eq!(c.iv(0), RPCRYPT_IV_REGIST_PS5_IV_EXPECTED);
    }

    #[test]
    fn bright_regist() {
        let c = RpCrypt::regist(Target::Ps4V10, &RPCRYPT_BRIGHT_REGIST_PS4_AMBASSADOR, 0x1e, 78703893).unwrap();
        assert_eq!(c.bright, RPCRYPT_BRIGHT_REGIST_PS4_BRIGHT_EXPECTED);
        let c = RpCrypt::regist(Target::Ps5V1, &RPCRYPT_BRIGHT_REGIST_PS5_AMBASSADOR, 0x1e, 78703893).unwrap();
        assert_eq!(c.bright, RPCRYPT_BRIGHT_REGIST_PS5_BRIGHT_EXPECTED);
    }

    #[test]
    fn encrypt_ps4_pre10() {
        let c = RpCrypt::auth(Target::Ps4V9, &RPCRYPT_ENCRYPT_PS4_PRE10_NONCE, &RPCRYPT_ENCRYPT_PS4_PRE10_MORNING);
        let cases: [(&[u8], &[u8]); 4] = [
            (&RPCRYPT_ENCRYPT_PS4_PRE10_BUF_A, &RPCRYPT_ENCRYPT_PS4_PRE10_CIPHER_EXPECTED_A),
            (&RPCRYPT_ENCRYPT_PS4_PRE10_BUF_B, &RPCRYPT_ENCRYPT_PS4_PRE10_CIPHER_EXPECTED_B),
            (&RPCRYPT_ENCRYPT_PS4_PRE10_BUF_C, &RPCRYPT_ENCRYPT_PS4_PRE10_CIPHER_EXPECTED_C),
            (&RPCRYPT_ENCRYPT_PS4_PRE10_BUF_D, &RPCRYPT_ENCRYPT_PS4_PRE10_CIPHER_EXPECTED_D),
        ];
        for (plain, expected) in cases {
            let mut buf = plain.to_vec();
            c.encrypt(0x0102030405060708, &mut buf);
            assert_eq!(buf, expected);
        }
    }

    #[test]
    fn decrypt_ps4_pre10() {
        let c = RpCrypt::auth(Target::Ps4V9, &RPCRYPT_DECRYPT_PS4_PRE10_NONCE, &RPCRYPT_DECRYPT_PS4_PRE10_MORNING);
        let cases: [(&[u8], &[u8]); 4] = [
            (&RPCRYPT_DECRYPT_PS4_PRE10_BUF_A, &RPCRYPT_DECRYPT_PS4_PRE10_EXPECTED_A),
            (&RPCRYPT_DECRYPT_PS4_PRE10_BUF_B, &RPCRYPT_DECRYPT_PS4_PRE10_EXPECTED_B),
            (&RPCRYPT_DECRYPT_PS4_PRE10_BUF_C, &RPCRYPT_DECRYPT_PS4_PRE10_EXPECTED_C),
            (&RPCRYPT_DECRYPT_PS4_PRE10_BUF_D, &RPCRYPT_DECRYPT_PS4_PRE10_EXPECTED_D),
        ];
        for (cipher, expected) in cases {
            let mut buf = cipher.to_vec();
            c.decrypt(0x0102030405060708, &mut buf);
            assert_eq!(buf, expected);
        }
    }

    #[test]
    fn regist_ps4_pre10_keys() {
        assert_eq!(aeropause_ps4_pre10(&REGIST_AMBASSADOR), REGIST_AEROPAUSE_PS4_PRE10_EXPECTED);
        assert_eq!(RpCrypt::regist_ps4_pre10(&REGIST_AMBASSADOR, 13374201).bright, REGIST_PIN_BRIGHT_PS4_PRE10_EXPECTED);
    }
}
