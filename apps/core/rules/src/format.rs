//! Petits formats communs (COEUR-41) : le code d'un partage ou d'un prêt, le
//! temps d'un enregistrement, une date UTC (journaux).

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

/// Une date et une heure UTC, sans dépendance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Utc {
    /// La date de `secs` secondes depuis le 1er janvier 1970 (UTC ; négatif : avant).
    pub fn from_unix(secs: i64) -> Utc {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        // Jours depuis 1970 → date civile (algorithme « civil from days » de Howard Hinnant).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Utc { year, month, day, hour: (rem / 3600) as u32, minute: (rem % 3600 / 60) as u32, second: (rem % 60) as u32 }
    }

    /// « 2026-11-27 12:00:00.000 » : l'heure des lignes des journaux.
    pub fn text(&self, millis: u32) -> String {
        format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{millis:03}", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }

    /// « 20261127-091500 » : pour nommer un fichier.
    pub fn stamp(&self) -> String {
        format!("{:04}{:02}{:02}-{:02}{:02}{:02}", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }
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

    #[test]
    fn dates_utc() {
        // Le moment des vecteurs d'essai KNJ1 (docs/relais.md).
        let t = Utc::from_unix(1_795_780_800);
        assert_eq!(t.text(0), "2026-11-27 12:00:00.000");
        assert_eq!(Utc::from_unix(1_795_780_800 + 3_723).text(45), "2026-11-27 13:02:03.045");
        assert_eq!(Utc::from_unix(0).stamp(), "19700101-000000");
        // 29 février d'une année bissextile, et la veille de 1970.
        assert_eq!(Utc::from_unix(951_782_400).text(0), "2000-02-29 00:00:00.000");
        assert_eq!(Utc::from_unix(-1).text(0), "1969-12-31 23:59:59.000");
    }
}
