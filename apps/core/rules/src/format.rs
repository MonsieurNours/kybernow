//! Petits formats communs (COEUR-41) : le code d'un partage ou d'un prêt, le
//! temps d'un enregistrement.

/// Les six chiffres d'un code tapé (« 123 456 », « 123-456 », « 123.456 ») ;
/// `None` s'il n'y en a pas exactement six.
pub fn normalize_code(text: &str) -> Option<String> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace() && *c != '-' && *c != '.').collect();
    (digits.len() == 6 && digits.chars().all(|c| c.is_ascii_digit())).then_some(digits)
}

/// Le code à afficher : « 123 456 » (tel quel s'il n'a pas six chiffres).
pub fn format_code(code: &str) -> String {
    match normalize_code(code) {
        Some(d) => format!("{} {}", &d[..3], &d[3..]),
        None => code.to_string(),
    }
}

/// Le temps d'un enregistrement : « 02:14 », « 1:02:14 » au-delà d'une heure.
pub fn recording_clock(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_et_temps() {
        assert_eq!(normalize_code(" 482-917 ").as_deref(), Some("482917"));
        assert_eq!(normalize_code("482 9171"), None);
        assert_eq!(normalize_code("48291a"), None);
        assert_eq!(format_code("482917"), "482 917");
        assert_eq!(format_code("482.917"), "482 917");
        assert_eq!(format_code("12"), "12");
        assert_eq!(recording_clock(0), "00:00");
        assert_eq!(recording_clock(134), "02:14");
        assert_eq!(recording_clock(3734), "1:02:14");
    }
}
