//! KyberNow Link OS : langue, pays et fuseau (docs/link-ui.md, « KyberNow Link OS : langue, pays et fuseau »,
//! LINAPP-30). Le pays règle le fuseau horaire (le premier du pays ; un second choix pour un pays qui en a plusieurs),
//! le pays du Wi-Fi (canaux et puissance permis) et la disposition d'un clavier USB (AZERTY pour la France et la
//! Belgique, QWERTZ pour l'Allemagne, l'Autriche et la Suisse, QWERTY sinon). Proposé d'après la langue : la France
//! pour le français, les États-Unis pour l'anglais.

use crate::i18n::tr;

/// Un pays : son code ISO 3166 (pays du Wi-Fi), son nom, ses fuseaux (le premier par défaut), la disposition xkb de
/// son clavier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    pub name: (&'static str, &'static str),
    pub zones: &'static [&'static str],
    pub keymap: &'static str,
}

impl Country {
    pub fn name(&self) -> &'static str {
        tr(self.name.0, self.name.1)
    }
}

const fn c(code: &'static str, fr: &'static str, en: &'static str, zones: &'static [&'static str], keymap: &'static str) -> Country {
    Country { code, name: (fr, en), zones, keymap }
}

/// Les pays proposés, par nom français.
pub const COUNTRIES: &[Country] = &[
    c("DE", "Allemagne", "Germany", &["Europe/Berlin"], "de"),
    c("AR", "Argentine", "Argentina", &["America/Argentina/Buenos_Aires"], "latam"),
    c("AU", "Australie", "Australia", &["Australia/Sydney", "Australia/Melbourne", "Australia/Brisbane", "Australia/Adelaide", "Australia/Perth", "Australia/Darwin", "Australia/Hobart"], "us"),
    c("AT", "Autriche", "Austria", &["Europe/Vienna"], "at"),
    c("BE", "Belgique", "Belgium", &["Europe/Brussels"], "be"),
    c("BR", "Brésil", "Brazil", &["America/Sao_Paulo", "America/Manaus", "America/Fortaleza", "America/Recife", "America/Rio_Branco"], "br"),
    c("CA", "Canada", "Canada", &["America/Toronto", "America/Montreal", "America/Vancouver", "America/Edmonton", "America/Winnipeg", "America/Halifax", "America/St_Johns"], "ca"),
    c("CI", "Côte d'Ivoire", "Côte d'Ivoire", &["Africa/Abidjan"], "fr"),
    c("DK", "Danemark", "Denmark", &["Europe/Copenhagen"], "dk"),
    c("ES", "Espagne", "Spain", &["Europe/Madrid", "Atlantic/Canary"], "es"),
    c("US", "États-Unis", "United States", &["America/New_York", "America/Chicago", "America/Denver", "America/Phoenix", "America/Los_Angeles", "America/Anchorage", "Pacific/Honolulu"], "us"),
    c("FI", "Finlande", "Finland", &["Europe/Helsinki"], "fi"),
    c("FR", "France", "France", &["Europe/Paris"], "fr"),
    c("GR", "Grèce", "Greece", &["Europe/Athens"], "us"),
    c("IN", "Inde", "India", &["Asia/Kolkata"], "us"),
    c("IE", "Irlande", "Ireland", &["Europe/Dublin"], "ie"),
    c("IT", "Italie", "Italy", &["Europe/Rome"], "it"),
    c("JP", "Japon", "Japan", &["Asia/Tokyo"], "jp"),
    c("LU", "Luxembourg", "Luxembourg", &["Europe/Luxembourg"], "fr"),
    c("MA", "Maroc", "Morocco", &["Africa/Casablanca"], "fr"),
    c("MX", "Mexique", "Mexico", &["America/Mexico_City", "America/Cancun", "America/Tijuana"], "latam"),
    c("MC", "Monaco", "Monaco", &["Europe/Monaco"], "fr"),
    c("NO", "Norvège", "Norway", &["Europe/Oslo"], "no"),
    c("NZ", "Nouvelle-Zélande", "New Zealand", &["Pacific/Auckland"], "us"),
    c("NL", "Pays-Bas", "Netherlands", &["Europe/Amsterdam"], "us"),
    c("PL", "Pologne", "Poland", &["Europe/Warsaw"], "pl"),
    c("PT", "Portugal", "Portugal", &["Europe/Lisbon", "Atlantic/Azores", "Atlantic/Madeira"], "pt"),
    c("GB", "Royaume-Uni", "United Kingdom", &["Europe/London"], "gb"),
    c("SN", "Sénégal", "Senegal", &["Africa/Dakar"], "fr"),
    c("SE", "Suède", "Sweden", &["Europe/Stockholm"], "se"),
    c("CH", "Suisse", "Switzerland", &["Europe/Zurich"], "ch"),
    c("CZ", "Tchéquie", "Czechia", &["Europe/Prague"], "cz"),
    c("TN", "Tunisie", "Tunisia", &["Africa/Tunis"], "fr"),
];

/// Le pays d'un code (`FR`).
pub fn by_code(code: &str) -> Option<&'static Country> {
    COUNTRIES.iter().find(|c| c.code.eq_ignore_ascii_case(code))
}

/// Le pays proposé pour une langue : la France pour le français, les États-Unis pour l'anglais.
pub fn for_language(lang: &str) -> &'static Country {
    by_code(if lang == "fr" { "FR" } else { "US" }).expect("pays par défaut")
}

/// Le fuseau d'un pays : celui choisi s'il est du pays, sinon le premier.
pub fn zone<'a>(c: &'a Country, chosen: &str) -> &'a str {
    c.zones.iter().copied().find(|z| *z == chosen).unwrap_or(c.zones[0])
}

/// Un fuseau à montrer : sa ville (« America/New_York » → « New York »).
pub fn zone_label(zone: &str) -> String {
    zone.rsplit('/').next().unwrap_or(zone).replace('_', " ")
}

/// « Pays », « Fuseau horaire », l'aide sous la ligne.
pub fn country_label() -> &'static str {
    tr("Pays", "Country")
}

pub fn zone_title() -> &'static str {
    tr("Fuseau horaire", "Time zone")
}

pub fn help() -> &'static str {
    tr("Le Wi-Fi, l'heure et le clavier suivent le pays.", "Wi-Fi, time and keyboard follow the country.")
}

/// Le fichier que l'app laisse à root (`kybernow-os-ctl locale`, qui le vérifie) : le pays, le fuseau, le clavier.
pub fn request(c: &Country, zone: &str) -> String {
    format!("COUNTRY={}\nTIMEZONE={zone}\nKEYMAP={}\n", c.code, c.keymap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pays_fuseau_et_clavier() {
        assert_eq!(for_language("fr").code, "FR");
        assert_eq!(for_language("en").code, "US");
        // AZERTY pour la France et la Belgique, QWERTZ pour l'Allemagne, l'Autriche et la Suisse.
        for (code, km) in [("FR", "fr"), ("BE", "be"), ("DE", "de"), ("AT", "at"), ("CH", "ch"), ("US", "us"), ("GB", "gb")] {
            assert_eq!(by_code(code).unwrap().keymap, km, "{code}");
        }
        let us = by_code("us").unwrap();
        assert_eq!(zone(us, ""), "America/New_York");
        assert_eq!(zone(us, "America/Los_Angeles"), "America/Los_Angeles");
        assert_eq!(zone(us, "Europe/Paris"), "America/New_York", "un fuseau d'un autre pays : le premier");
        assert_eq!(zone_label("America/New_York"), "New York");
        assert_eq!(request(by_code("FR").unwrap(), "Europe/Paris"), "COUNTRY=FR\nTIMEZONE=Europe/Paris\nKEYMAP=fr\n");
        // Ce que root accepte : deux lettres, un fuseau en lettres et barres, un clavier en minuscules.
        for c in COUNTRIES {
            assert!(c.code.len() == 2 && c.code.chars().all(|x| x.is_ascii_uppercase()), "{}", c.code);
            assert!(!c.zones.is_empty() && c.zones.iter().all(|z| z.chars().all(|x| x.is_ascii_alphanumeric() || "/_+-".contains(x))), "{}", c.code);
            assert!(c.keymap.chars().all(|x| x.is_ascii_lowercase()), "{}", c.code);
        }
        // Par nom français.
        let names: Vec<&str> = COUNTRIES.iter().map(|c| c.name.0).collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|n| n.to_lowercase().replace('é', "e").replace('É', "e").replace('è', "e").replace('ô', "o").replace('ç', "c"));
        assert_eq!(names, sorted);
    }
}
