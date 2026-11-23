//! Les mots de passe (docs/comptes.md, « Sécurité du relais ») : Argon2id,
//! 10 caractères au moins, les mots de passe connus refusés.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Au moins ces caractères.
pub const MIN_CHARS: usize = 10;
/// Au plus ces caractères (le reste est refusé, pas tronqué).
pub const MAX_CHARS: usize = 256;

/// Pourquoi un nouveau mot de passe est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weak {
    TooShort,
    TooLong,
    /// Dans la liste des mots de passe connus, ou une suite trop simple.
    Known,
    /// L'adresse e-mail elle-même, ou sa partie avant `@`.
    LikeEmail,
}

/// Des mots de passe d'au moins 10 caractères parmi les plus répandus (fuites
/// publiques), en minuscules. Une courte liste : les suites et répétitions
/// sont refusées à part ([`check_new`]).
const KNOWN: &[&str] = &[
    "1234567890", "12345678910", "123456789a", "123456789q", "1234567890a", "12345678900", "123456789123", "1234512345",
    "0123456789", "9876543210", "0987654321", "1122334455", "1111122222", "11223344556677", "1q2w3e4r5t", "1q2w3e4r5t6y",
    "q1w2e3r4t5", "q1w2e3r4t5y6", "a1b2c3d4e5", "1qaz2wsx3edc", "zaq12wsxcde", "qazwsxedcrfv", "qwertyuiop", "qwerty1234",
    "qwerty12345", "qwerty123456", "azertyuiop", "azerty1234", "azerty12345", "azerty123456", "asdfghjkl1", "zxcvbnm123",
    "password12", "password123", "password1234", "password!1", "passw0rd123", "p@ssw0rd123", "p@ssword123", "pa$$w0rd123",
    "motdepasse", "motdepasse1", "motdepasse12", "motdepasse123", "mot2passe123", "iloveyou12", "iloveyou123", "jetaime1234",
    "football123", "baseball123", "starwars123", "superman123", "batman12345", "pokemon1234", "princess123", "sunshine123",
    "welcome123", "welcome1234", "letmein123", "changeme123", "computer123", "internet123", "administrator", "admin12345",
    "adminadmin", "soleil1234", "bonjour123", "bonjour1234", "doudou1234", "chouchou123", "marseille13", "loulou1234",
    "nicolas123", "jordan2323", "michael123", "liverpool1", "chelsea123", "arsenal123", "trustno1234", "abcdefghij",
    "abcdefg123", "abc1234567", "abcd123456", "aaaaaa1111", "qwertzuiop", "kybernow123", "kybernow2026", "kybernowshare",
];

/// Un nouveau mot de passe est-il acceptable ?
pub fn check_new(password: &str, email: &str) -> Result<(), Weak> {
    let n = password.chars().count();
    if n < MIN_CHARS {
        return Err(Weak::TooShort);
    }
    if n > MAX_CHARS {
        return Err(Weak::TooLong);
    }
    let lower = password.to_lowercase();
    let email = email.to_lowercase();
    let local = email.split('@').next().unwrap_or("");
    if lower == email || (local.chars().count() >= MIN_CHARS && lower == local) {
        return Err(Weak::LikeEmail);
    }
    if KNOWN.contains(&lower.as_str()) {
        return Err(Weak::Known);
    }
    // Un seul caractère répété, ou une suite (« 1234567890 », « abcdefghijk »).
    let chars: Vec<char> = lower.chars().collect();
    let same = chars.windows(2).all(|w| w[0] == w[1]);
    let up = chars.windows(2).all(|w| (w[1] as i64) - (w[0] as i64) == 1);
    let down = chars.windows(2).all(|w| (w[0] as i64) - (w[1] as i64) == 1);
    if same || up || down {
        return Err(Weak::Known);
    }
    Ok(())
}

/// Le calcul des empreintes.
#[derive(Clone)]
pub struct Hasher {
    argon: Argon2<'static>,
    /// L'empreinte d'un mot de passe quelconque : vérifiée quand le compte
    /// n'existe pas, pour que la réponse prenne le même temps.
    dummy: String,
}

impl Hasher {
    /// Les réglages d'Argon2id recommandés (19 Mio, 2 passes, 1 fil).
    pub fn new() -> Hasher {
        Hasher::with(Params::default())
    }

    /// Des réglages bas, pour les essais seulement.
    pub fn fast() -> Hasher {
        Hasher::with(Params::new(256, 1, 1, None).expect("réglages d'Argon2"))
    }

    fn with(params: Params) -> Hasher {
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut h = Hasher { argon, dummy: String::new() };
        h.dummy = h.hash("kybernow : aucun compte");
        h
    }

    /// L'empreinte (chaîne PHC, sel aléatoire de 16 octets).
    pub fn hash(&self, password: &str) -> String {
        let salt = SaltString::encode_b64(&super::random::<16>()).expect("sel");
        self.argon.hash_password(password.as_bytes(), &salt).expect("Argon2").to_string()
    }

    /// Le mot de passe correspond-il à l'empreinte ? (`None` : pas de compte, même temps de calcul.)
    pub fn verify(&self, password: &str, hash: Option<&str>) -> bool {
        let (hash, real) = match hash {
            Some(h) => (h, true),
            None => (self.dummy.as_str(), false),
        };
        let Ok(parsed) = PasswordHash::new(hash) else { return false };
        // Les paramètres viennent de l'empreinte : une empreinte faite avec d'autres réglages reste valable.
        let ok = Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok();
        ok && real
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Hasher::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regles_du_mot_de_passe() {
        assert_eq!(check_new("court", "a@b.org"), Err(Weak::TooShort));
        assert_eq!(check_new("lune9verte", "a@b.org"), Ok(()));
        assert_eq!(check_new(&"x".repeat(MAX_CHARS + 1), "a@b.org"), Err(Weak::TooLong));
        assert_eq!(check_new("MotDePasse123", "a@b.org"), Err(Weak::Known));
        assert_eq!(check_new("aaaaaaaaaaaa", "a@b.org"), Err(Weak::Known));
        assert_eq!(check_new("0123456789", "a@b.org"), Err(Weak::Known));
        assert_eq!(check_new("abcdefghijklm", "a@b.org"), Err(Weak::Known));
        assert_eq!(check_new("9876543210", "a@b.org"), Err(Weak::Known));
        assert_eq!(check_new("Jean.Dupont@exemple.org", "jean.dupont@exemple.org"), Err(Weak::LikeEmail));
        assert_eq!(check_new("jean.dupont.martin", "jean.dupont.martin@exemple.org"), Err(Weak::LikeEmail));
        assert_eq!(check_new("cheval agrafe pile", "a@b.org"), Ok(()));
        // Compté en caractères, pas en octets.
        assert_eq!(check_new("éééééééé9", "a@b.org"), Err(Weak::TooShort));
        for p in KNOWN {
            assert!(p.chars().count() >= MIN_CHARS, "{p} : trop court pour la liste");
            assert_eq!(*p, p.to_lowercase(), "{p} : en minuscules");
        }
    }

    #[test]
    fn argon2id() {
        let h = Hasher::fast();
        let hash = h.hash("cheval agrafe pile");
        assert!(hash.starts_with("$argon2id$v=19$"), "{hash}");
        assert_ne!(hash, h.hash("cheval agrafe pile"), "sel aléatoire");
        assert!(h.verify("cheval agrafe pile", Some(&hash)));
        assert!(!h.verify("cheval agrafe pilE", Some(&hash)));
        assert!(!h.verify("kybernow : aucun compte", None), "jamais vrai sans compte");
        assert!(!h.verify("x", Some("pas une empreinte")));
        // Les réglages par défaut sont ceux d'OWASP : 19 Mio, 2 passes.
        let real = Hasher::new().hash("x");
        assert!(real.contains("m=19456,t=2,p=1"), "{real}");
    }
}
