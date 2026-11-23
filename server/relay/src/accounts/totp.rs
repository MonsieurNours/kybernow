//! La double authentification par application (RFC 6238) : HMAC-SHA1, pas de
//! 30 secondes, six chiffres, un pas d'écart permis de chaque côté. Le secret
//! fait 20 octets et s'affiche en base32 (RFC 4648, sans `=`).

use ring::hmac;

/// La durée d'un pas.
pub const STEP: i64 = 30;
/// Les octets du secret.
pub const SECRET_LEN: usize = 20;

/// HOTP (RFC 4226) : le code de `digits` chiffres pour ce compteur.
pub fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32 {
    let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret);
    let tag = hmac::sign(&key, &counter.to_be_bytes());
    let h = tag.as_ref();
    let offset = (h[h.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([h[offset] & 0x7f, h[offset + 1], h[offset + 2], h[offset + 3]]);
    bin % 10u32.pow(digits)
}

/// Le pas d'une heure Unix.
pub fn step_at(unix: i64) -> i64 {
    unix.div_euclid(STEP)
}

/// Le code à six chiffres d'un pas.
pub fn code(secret: &[u8], step: i64) -> String {
    format!("{:06}", hotp(secret, step.max(0) as u64, 6))
}

/// Le pas qu'un code désigne (l'actuel, le précédent ou le suivant), s'il est
/// plus récent que `last_step` : un code déjà accepté ne repasse pas.
pub fn verify(secret: &[u8], input: &str, unix: i64, last_step: i64) -> Option<i64> {
    let input: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if input.len() != 6 || !input.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let now = step_at(unix);
    let mut found = None;
    // Toujours les trois calculs : le temps ne dit pas lequel a marché.
    for step in [now - 1, now, now + 1] {
        if super::ct_eq(code(secret, step).as_bytes(), input.as_bytes()) && step > last_step {
            found = Some(step);
        }
    }
    found
}

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Base32 (RFC 4648), sans `=`.
pub fn base32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Le secret par groupes de quatre, pour le recopier à la main.
pub fn grouped(secret_b32: &str) -> String {
    secret_b32.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join(" ")
}

fn percent(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// L'adresse `otpauth://` que lisent les applications d'authentification.
pub fn otpauth_uri(secret: &[u8], issuer: &str, account: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period={STEP}",
        percent(issuer),
        percent(account),
        base32(secret),
        percent(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238, annexe B (SHA-1, huit chiffres, pas de 30 s).
    #[test]
    fn vecteurs_de_la_rfc_6238() {
        let secret = b"12345678901234567890";
        for (t, expected) in [
            (59i64, 94287082u32),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
            (20000000000, 65353130),
        ] {
            assert_eq!(hotp(secret, step_at(t) as u64, 8), expected, "t = {t}");
        }
        // RFC 4226, annexe D : HOTP à six chiffres.
        let hotp6: Vec<u32> = (0..10).map(|c| hotp(secret, c, 6)).collect();
        assert_eq!(hotp6, [755224, 287082, 359152, 969429, 338314, 254676, 287922, 162583, 399871, 520489]);
    }

    #[test]
    fn un_pas_d_ecart_et_pas_de_rejeu() {
        let secret = b"12345678901234567890";
        let t = 1_700_000_000;
        let now = step_at(t);
        assert_eq!(verify(secret, &code(secret, now), t, 0), Some(now));
        assert_eq!(verify(secret, &code(secret, now - 1), t, 0), Some(now - 1));
        assert_eq!(verify(secret, &code(secret, now + 1), t, 0), Some(now + 1));
        assert_eq!(verify(secret, &code(secret, now - 2), t, 0), None);
        assert_eq!(verify(secret, &code(secret, now), t, now), None, "déjà accepté");
        let spaced = format!("{} {}", &code(secret, now)[..3], &code(secret, now)[3..]);
        assert_eq!(verify(secret, &spaced, t, 0), Some(now), "« 123 456 » accepté");
        assert_eq!(verify(secret, "12345", t, 0), None);
        assert_eq!(verify(secret, "abcdef", t, 0), None);
    }

    #[test]
    fn base32_et_adresse() {
        // RFC 4648, section 10.
        for (input, expected) in [("", ""), ("f", "MY"), ("fo", "MZXQ"), ("foo", "MZXW6"), ("foob", "MZXW6YQ"), ("fooba", "MZXW6YTB"), ("foobar", "MZXW6YTBOI")] {
            assert_eq!(base32(input.as_bytes()), expected);
        }
        assert_eq!(base32(b"12345678901234567890"), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(grouped("GEZDGNBVGY"), "GEZD GNBV GY");
        let uri = otpauth_uri(b"12345678901234567890", "KyberNow (share.kybernow.app)", "a+b@exemple.org");
        assert_eq!(
            uri,
            "otpauth://totp/KyberNow%20%28share.kybernow.app%29:a%2Bb%40exemple.org?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=KyberNow%20%28share.kybernow.app%29&algorithm=SHA1&digits=6&period=30"
        );
    }
}
