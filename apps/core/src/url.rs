//! Petites briques d'adresse communes au cœur (COEUR-40) : chiffres
//! hexadécimaux, décodage `%xx` et paramètres d'une adresse. Avant, trois
//! copies (comptes, partage, PSN), toutes avec le même défaut (un `%xx` à la
//! fin n'était pas décodé).

/// Un chiffre hexadécimal (octet ASCII).
pub(crate) fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Deux chiffres hexadécimaux en un octet.
pub(crate) fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_digit(hi)? << 4 | hex_digit(lo)?)
}

/// `%xx` et `+` (espace) décodés, sur les octets : un texte non ASCII ne fait
/// jamais paniquer ; un `%` qui n'est pas suivi de deux chiffres reste tel
/// quel.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 3 <= bytes.len()
            && let Some(b) = hex_pair(bytes[i + 1], bytes[i + 2])
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Les paramètres d'une adresse (`a=1&b=2`), décodés.
pub(crate) fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodage_commun() {
        assert_eq!(percent_decode("%aé"), "%aé");
        assert_eq!(percent_decode("é%41"), "éA");
        assert_eq!(percent_decode("a%2F"), "a/", "un %xx final est décodé");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("a+b%20c"), "a b c");
        assert_eq!(query_pairs("a=1&&b=%C3%A9&c"), vec![("a".into(), "1".into()), ("b".into(), "é".into()), ("c".into(), String::new())]);
    }
}
