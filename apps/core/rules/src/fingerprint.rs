//! Empreinte du certificat d'Admin (TRANS-34, MAC-20) : une seule règle pour
//! la dire « la même », et un seul affichage court, celui d'Admin.

/// Les chiffres d'une empreinte, sans le préfixe `sha256:` (toute casse),
/// sans deux-points, espaces ni tirets, en minuscules.
fn digits(fp: &str) -> String {
    let t = fp.trim();
    let t = match t.get(..7) {
        Some(p) if p.eq_ignore_ascii_case("sha256:") => &t[7..],
        _ => t,
    };
    t.chars().filter(|c| !matches!(c, ':' | '-') && !c.is_whitespace()).flat_map(char::to_lowercase).collect()
}

/// La forme des apps et du relais, `sha256:aa:bb:…` (32 octets), d'une
/// empreinte écrite avec son préfixe (`sha256:` ou `SHA256:`), avec ou sans
/// deux-points, en majuscules ou minuscules. `None` si elle n'en est pas
/// une (la même règle que `kybernow_tunnel::id::normalize_fingerprint`).
pub fn normalize(fp: &str) -> Option<String> {
    let t = fp.trim();
    let rest = t.strip_prefix("sha256:").or_else(|| t.strip_prefix("SHA256:"))?;
    let hex: String = rest.chars().filter(|c| *c != ':').collect::<String>().to_ascii_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let pairs: Vec<&str> = (0..32).map(|i| &hex[i * 2..i * 2 + 2]).collect();
    Some(format!("sha256:{}", pairs.join(":")))
}

/// Deux empreintes sont la même : mêmes chiffres, sans égard au préfixe
/// `sha256:`, aux deux-points, aux espaces ni à la casse. Une empreinte vide
/// n'est jamais la même qu'une autre.
pub fn same(a: &str, b: &str) -> bool {
    let a = digits(a);
    !a.is_empty() && a == digits(b)
}

/// [`same`] pour des empreintes peut-être inconnues : faux si l'une manque.
pub fn same_opt(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if same(a, b))
}

/// L'affichage court d'Admin et de toutes les apps : les seize premiers
/// chiffres, en majuscules, en quatre groupes de quatre
/// (`7C41 E09B AABB CCDD`). Vide sans chiffre.
pub fn short(fp: &str) -> String {
    let hex: Vec<char> = digits(fp).chars().filter(char::is_ascii_hexdigit).take(16).map(|c| c.to_ascii_uppercase()).collect();
    hex.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "sha256:7c:41:e0:9b:aa:bb:cc:dd:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77";

    #[test]
    fn affichage_court() {
        assert_eq!(short(FP), "7C41 E09B AABB CCDD");
        assert_eq!(short("SHA256:7C41E09BAABBCCDD0011"), "7C41 E09B AABB CCDD");
        assert_eq!(short("7c41 e09b aabb ccdd eeff"), "7C41 E09B AABB CCDD");
        assert_eq!(short("sha256:7c:41:e0"), "7C41 E0", "le dernier groupe partiel reste");
        assert_eq!(short(""), "");
        assert_eq!(short("sha256:"), "");
    }

    #[test]
    fn meme_empreinte() {
        let upper = FP.to_uppercase();
        let bare: String = FP.trim_start_matches("sha256:").replace(':', "");
        assert!(same(FP, &upper));
        assert!(same(FP, &bare), "sans préfixe ni deux-points");
        assert!(same(&format!("  {FP} "), &bare.to_uppercase()));
        assert!(!same(FP, "sha256:00"));
        assert!(!same("", ""), "vide : jamais la même");
        assert!(!same("sha256:", "SHA256:"));
        assert!(same_opt(Some(FP), Some(&upper)));
        assert!(!same_opt(None, Some(FP)) && !same_opt(Some(FP), None) && !same_opt(None, None));
    }

    #[test]
    fn forme_normale() {
        assert_eq!(normalize(&FP.to_uppercase().replace("SHA256:", "sha256:")).as_deref(), Some(FP));
        let bare: String = FP.trim_start_matches("sha256:").replace(':', "").to_uppercase();
        assert_eq!(normalize(&format!("SHA256:{bare}")).as_deref(), Some(FP));
        assert_eq!(normalize(&bare), None, "préfixe exigé");
        assert_eq!(normalize("sha256:abcd"), None);
        assert_eq!(normalize(&format!("sha256:{}", "zz".repeat(32))), None);
    }
}
