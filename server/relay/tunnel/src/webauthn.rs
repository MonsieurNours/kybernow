//! WebAuthn (passkeys), ce qu'il en faut au relais (comptes KyberNow) et à Admin (clé d'Admin à
//! distance, docs/relais.md) : CBOR, clés COSE, `clientDataJSON`, données de l'authentificateur,
//! signatures. Écrit sur `ring`, sans bibliothèque WebAuthn (celles qui existent tirent OpenSSL ou
//! demandent bien plus que ce qu'il faut ici). Aucune attestation n'est demandée
//! (`attestation: "none"`) : seuls comptent l'origine, le défi, les drapeaux et la signature.
//! Algorithmes : ES256 (-7), Ed25519 (-8) et RS256 (-257, les anciennes puces TPM de Windows Hello).
//! La vérification de l'utilisateur (drapeau UV) est toujours exigée.
//!
//! Les défis et la garde des passkeys restent à l'appelant.

use ring::{digest, signature};
use serde::Deserialize;

/// Les algorithmes COSE acceptés.
pub const ES256: i64 = -7;
pub const ED25519: i64 = -8;
pub const RS256: i64 = -257;

/// Égalité en temps constant.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// --- CBOR (ce qu'il faut de RFC 8949 pour WebAuthn) ------------------------------

/// Une valeur CBOR.
#[derive(Debug, Clone, PartialEq)]
pub enum Cbor {
    Int(i128),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    Map(Vec<(Cbor, Cbor)>),
    /// Booléens, null, nombres à virgule : sans usage ici.
    Other,
}

impl Cbor {
    fn get(&self, key: &Cbor) -> Option<&Cbor> {
        match self {
            Cbor::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn int(&self, key: i128) -> Option<&Cbor> {
        self.get(&Cbor::Int(key))
    }

    fn text(&self, key: &str) -> Option<&Cbor> {
        self.get(&Cbor::Text(key.into()))
    }

    fn as_int(&self) -> Option<i128> {
        match self {
            Cbor::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Cbor::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

/// Décode une valeur ; rend la valeur et ce qui la suit. Pas de longueur indéfinie (CTAP2 n'en a pas).
pub fn cbor(input: &[u8]) -> Option<(Cbor, &[u8])> {
    decode(input, 0)
}

fn decode(input: &[u8], depth: usize) -> Option<(Cbor, &[u8])> {
    if depth > 16 {
        return None;
    }
    let (&first, rest) = input.split_first()?;
    let (major, info) = (first >> 5, first & 0x1f);
    let (arg, mut rest) = match info {
        0..=23 => (u64::from(info), rest),
        24 => (u64::from(*rest.first()?), &rest[1..]),
        25 => (u64::from(u16::from_be_bytes(rest.get(..2)?.try_into().ok()?)), &rest[2..]),
        26 => (u64::from(u32::from_be_bytes(rest.get(..4)?.try_into().ok()?)), &rest[4..]),
        27 => (u64::from_be_bytes(rest.get(..8)?.try_into().ok()?), &rest[8..]),
        _ => return None,
    };
    let len = |n: u64| usize::try_from(n).ok().filter(|&n| n <= input.len());
    let value = match major {
        0 => Cbor::Int(i128::from(arg)),
        1 => Cbor::Int(-1 - i128::from(arg)),
        2 | 3 => {
            let n = len(arg)?;
            let bytes = rest.get(..n)?.to_vec();
            rest = &rest[n..];
            if major == 2 { Cbor::Bytes(bytes) } else { Cbor::Text(String::from_utf8(bytes).ok()?) }
        }
        4 => {
            let mut items = Vec::new();
            for _ in 0..len(arg)? {
                let (item, r) = decode(rest, depth + 1)?;
                items.push(item);
                rest = r;
            }
            Cbor::Array(items)
        }
        5 => {
            let mut entries = Vec::new();
            for _ in 0..len(arg)? {
                let (k, r) = decode(rest, depth + 1)?;
                let (v, r) = decode(r, depth + 1)?;
                entries.push((k, v));
                rest = r;
            }
            Cbor::Map(entries)
        }
        // Une étiquette : la valeur qu'elle porte.
        6 => return decode(rest, depth + 1),
        // Simples et flottants : l'argument est déjà lu.
        _ => Cbor::Other,
    };
    Some((value, rest))
}

// --- Clés COSE -----------------------------------------------------------------

/// Une clé publique, telle que gardée en base : ES256, le point non compressé
/// (65 octets) ; Ed25519, 32 octets ; RS256, la longueur de `n` (2 octets), `n`, puis `e`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    pub alg: i64,
    pub key: Vec<u8>,
}

/// Lit une clé COSE (RFC 9053) ; `None` pour un algorithme ou une forme inconnus.
pub fn cose_key(key: &Cbor) -> Option<PublicKey> {
    let kty = key.int(1)?.as_int()?;
    let alg = key.int(3)?.as_int()?;
    match (kty, alg) {
        (2, a) if a == i128::from(ES256) => {
            let (crv, x, y) = (key.int(-1)?.as_int()?, key.int(-2)?.as_bytes()?, key.int(-3)?.as_bytes()?);
            if crv != 1 || x.len() != 32 || y.len() != 32 {
                return None;
            }
            let mut point = vec![4u8];
            point.extend_from_slice(x);
            point.extend_from_slice(y);
            Some(PublicKey { alg: ES256, key: point })
        }
        (1, a) if a == i128::from(ED25519) => {
            let (crv, x) = (key.int(-1)?.as_int()?, key.int(-2)?.as_bytes()?);
            (crv == 6 && x.len() == 32).then(|| PublicKey { alg: ED25519, key: x.to_vec() })
        }
        (3, a) if a == i128::from(RS256) => {
            let (n, e) = (key.int(-1)?.as_bytes()?, key.int(-2)?.as_bytes()?);
            let n_len = u16::try_from(n.len()).ok().filter(|&l| (256..=1024).contains(&l))?;
            if e.is_empty() || e.len() > 8 {
                return None;
            }
            let mut out = n_len.to_be_bytes().to_vec();
            out.extend_from_slice(n);
            out.extend_from_slice(e);
            Some(PublicKey { alg: RS256, key: out })
        }
        _ => None,
    }
}

/// Vérifie une signature de l'authentificateur.
pub fn verify_signature(alg: i64, key: &[u8], message: &[u8], sig: &[u8]) -> bool {
    match alg {
        ES256 => signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_ASN1, key).verify(message, sig).is_ok(),
        ED25519 => signature::UnparsedPublicKey::new(&signature::ED25519, key).verify(message, sig).is_ok(),
        RS256 => {
            let Some(len) = key.get(..2).map(|l| usize::from(u16::from_be_bytes([l[0], l[1]]))) else { return false };
            let Some((n, e)) = key.get(2..).filter(|r| r.len() > len).map(|r| r.split_at(len)) else { return false };
            signature::RsaPublicKeyComponents { n, e }.verify(&signature::RSA_PKCS1_2048_8192_SHA256, message, sig).is_ok()
        }
        _ => false,
    }
}

// --- Cérémonies ----------------------------------------------------------------

/// Le relais vu par WebAuthn : l'identifiant (le nom d'hôte) et l'origine des pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rp {
    pub id: String,
    pub origin: String,
}

impl Rp {
    /// D'après l'adresse publique (`https://share.kybernow.app[:port]`).
    pub fn of(public_url: &str) -> Rp {
        let origin = public_url.trim_end_matches('/').to_string();
        let host = origin.split_once("://").map_or(origin.as_str(), |(_, rest)| rest).split('/').next().unwrap_or("");
        // Une adresse IPv6 ([::1]:8443) garde ses crochets ; le port n'est pas dans l'identifiant.
        let id = match host.strip_prefix('[') {
            Some(v6) => v6.split(']').next().unwrap_or("").to_string(),
            None => host.split(':').next().unwrap_or("").to_string(),
        };
        Rp { id: id.to_ascii_lowercase(), origin }
    }
}

/// Pourquoi une réponse du navigateur est refusée (pour les essais et le journal, jamais montré tel quel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Malformed,
    Type,
    Challenge,
    Origin,
    RpId,
    /// L'utilisateur n'est pas présent ou pas vérifié (UV).
    Flags,
    Algorithm,
    Signature,
    /// Le compteur recule : passkey peut-être copiée.
    Counter,
    UserHandle,
}

#[derive(Deserialize)]
struct ClientData {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
    #[serde(default, rename = "crossOrigin")]
    cross_origin: bool,
}

/// Lit `clientDataJSON` et vérifie le type et l'origine ; rend le défi, à vérifier par l'appelant.
pub fn client_data(rp: &Rp, json: &[u8], kind: &str) -> Result<String, Refused> {
    let data: ClientData = serde_json::from_slice(json).map_err(|_| Refused::Malformed)?;
    if data.kind != kind {
        return Err(Refused::Type);
    }
    if data.origin != rp.origin || data.cross_origin {
        return Err(Refused::Origin);
    }
    Ok(data.challenge)
}

pub const FLAG_UP: u8 = 0x01;
pub const FLAG_UV: u8 = 0x04;
pub const FLAG_AT: u8 = 0x40;

/// Les données de l'authentificateur : `rpIdHash`, drapeaux, compteur, et ce qui suit.
fn auth_data<'a>(rp: &Rp, data: &'a [u8]) -> Result<(u8, i64, &'a [u8]), Refused> {
    if data.len() < 37 {
        return Err(Refused::Malformed);
    }
    if data[..32] != *digest::digest(&digest::SHA256, rp.id.as_bytes()).as_ref() {
        return Err(Refused::RpId);
    }
    let flags = data[32];
    if flags & FLAG_UP == 0 || flags & FLAG_UV == 0 {
        return Err(Refused::Flags);
    }
    let counter = i64::from(u32::from_be_bytes([data[33], data[34], data[35], data[36]]));
    Ok((flags, counter, &data[37..]))
}

/// Vérifie la réponse de `navigator.credentials.create()` (le défi est déjà vérifié par l'appelant).
pub fn verify_registration(rp: &Rp, attestation_object: &[u8]) -> Result<Registration, Refused> {
    let (object, _) = cbor(attestation_object).ok_or(Refused::Malformed)?;
    // Aucune attestation demandée : le format (`none` le plus souvent) et `attStmt` sont ignorés.
    let data = object.text("authData").and_then(Cbor::as_bytes).ok_or(Refused::Malformed)?;
    let (flags, counter, rest) = auth_data(rp, data)?;
    if flags & FLAG_AT == 0 || rest.len() < 18 {
        return Err(Refused::Malformed);
    }
    let id_len = usize::from(u16::from_be_bytes([rest[16], rest[17]]));
    if id_len == 0 || id_len > 1023 || rest.len() < 18 + id_len {
        return Err(Refused::Malformed);
    }
    let credential = rest[18..18 + id_len].to_vec();
    let (key, _extensions) = cbor(&rest[18 + id_len..]).ok_or(Refused::Malformed)?;
    let key = cose_key(&key).ok_or(Refused::Algorithm)?;
    Ok(Registration { credential, public_key: key.key, alg: key.alg, counter })
}

/// Une passkey ajoutée (`navigator.credentials.create()` vérifié).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// L'identifiant de la passkey, donné par l'authentificateur.
    pub credential: Vec<u8>,
    /// La clé publique, sous la forme de [`PublicKey::key`].
    pub public_key: Vec<u8>,
    /// L'algorithme COSE : [`ES256`], [`ED25519`] ou [`RS256`].
    pub alg: i64,
    pub counter: i64,
}

/// Une passkey connue, telle que la garde qui vérifie.
#[derive(Debug, Clone, Copy)]
pub struct StoredKey<'a> {
    pub alg: i64,
    pub public_key: &'a [u8],
    pub counter: i64,
}

/// Ce que le navigateur renvoie de `navigator.credentials.get()`.
#[derive(Debug, Clone)]
pub struct Assertion {
    pub credential: Vec<u8>,
    pub client_data: Vec<u8>,
    pub authenticator: Vec<u8>,
    pub signature: Vec<u8>,
    /// Vide si l'authentificateur ne l'a pas donné.
    pub user: Vec<u8>,
}

/// Vérifie une assertion pour une passkey connue (`key` : son algorithme, sa clé publique et son
/// dernier compteur) ; rend le nouveau compteur.
pub fn verify_assertion(rp: &Rp, key: &StoredKey<'_>, webauthn_user: &[u8], a: &Assertion) -> Result<i64, Refused> {
    let (_, counter, _) = auth_data(rp, &a.authenticator)?;
    let mut message = a.authenticator.clone();
    message.extend_from_slice(digest::digest(&digest::SHA256, &a.client_data).as_ref());
    if !verify_signature(key.alg, key.public_key, &message, &a.signature) {
        return Err(Refused::Signature);
    }
    if !a.user.is_empty() && !ct_eq(&a.user, webauthn_user) {
        return Err(Refused::UserHandle);
    }
    // Un compteur qui ne monte pas alors qu'il servait : la passkey a pu être copiée.
    if (counter != 0 || key.counter != 0) && counter <= key.counter {
        return Err(Refused::Counter);
    }
    Ok(counter)
}

#[cfg(any(test, feature = "test-webauthn"))]
pub mod testing {
    //! Un authentificateur logiciel pour les essais (relais et Admin) : il fabrique ce que
    //! renverrait `navigator.credentials` (CBOR, `clientDataJSON`, signatures).

    use super::*;
    use ring::rand::{SecureRandom, SystemRandom};
    use ring::signature::{EcdsaKeyPair, Ed25519KeyPair, KeyPair};

    /// Encode un en-tête CBOR.
    pub fn head(major: u8, n: u64) -> Vec<u8> {
        let m = major << 5;
        match n {
            0..=23 => vec![m | n as u8],
            24..=0xff => vec![m | 24, n as u8],
            0x100..=0xffff => [vec![m | 25], (n as u16).to_be_bytes().to_vec()].concat(),
            _ => [vec![m | 26], (n as u32).to_be_bytes().to_vec()].concat(),
        }
    }

    pub fn enc_int(i: i64) -> Vec<u8> {
        if i >= 0 { head(0, i as u64) } else { head(1, (-1 - i) as u64) }
    }

    pub fn enc_bytes(b: &[u8]) -> Vec<u8> {
        [head(2, b.len() as u64), b.to_vec()].concat()
    }

    pub fn enc_text(s: &str) -> Vec<u8> {
        [head(3, s.len() as u64), s.as_bytes().to_vec()].concat()
    }

    pub fn enc_map(entries: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
        let mut out = head(5, entries.len() as u64);
        for (k, v) in entries {
            out.extend_from_slice(k);
            out.extend_from_slice(v);
        }
        out
    }

    /// Une passkey logicielle.
    pub struct Authenticator {
        pub credential: Vec<u8>,
        ec: Option<EcdsaKeyPair>,
        ed: Option<Ed25519KeyPair>,
        pub counter: u32,
        pub rp_id: String,
        pub origin: String,
        pub flags: u8,
        pub user: Vec<u8>,
    }

    impl Authenticator {
        pub fn new(alg: i64, origin: &str) -> Authenticator {
            let rng = SystemRandom::new();
            let (ec, ed) = match alg {
                ES256 => {
                    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
                    (Some(EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap()), None)
                }
                _ => {
                    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
                    (None, Some(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()))
                }
            };
            let rp = Rp::of(origin);
            Authenticator {
                credential: {
                    let mut c = [0u8; 16];
                    rng.fill(&mut c).unwrap();
                    c.to_vec()
                },
                ec,
                ed,
                counter: 0,
                rp_id: rp.id,
                origin: rp.origin,
                flags: FLAG_UP | FLAG_UV,
                user: Vec::new(),
            }
        }

        fn cose(&self) -> Vec<u8> {
            match (&self.ec, &self.ed) {
                (Some(ec), _) => {
                    let p = ec.public_key().as_ref();
                    enc_map(&[
                        (enc_int(1), enc_int(2)),
                        (enc_int(3), enc_int(ES256)),
                        (enc_int(-1), enc_int(1)),
                        (enc_int(-2), enc_bytes(&p[1..33])),
                        (enc_int(-3), enc_bytes(&p[33..65])),
                    ])
                }
                (_, Some(ed)) => enc_map(&[
                    (enc_int(1), enc_int(1)),
                    (enc_int(3), enc_int(ED25519)),
                    (enc_int(-1), enc_int(6)),
                    (enc_int(-2), enc_bytes(ed.public_key().as_ref())),
                ]),
                _ => unreachable!(),
            }
        }

        fn auth_data(&self, attested: bool) -> Vec<u8> {
            let mut out = digest::digest(&digest::SHA256, self.rp_id.as_bytes()).as_ref().to_vec();
            out.push(self.flags | if attested { FLAG_AT } else { 0 });
            out.extend_from_slice(&self.counter.to_be_bytes());
            if attested {
                out.extend_from_slice(&[0u8; 16]);
                out.extend_from_slice(&(self.credential.len() as u16).to_be_bytes());
                out.extend_from_slice(&self.credential);
                out.extend_from_slice(&self.cose());
            }
            out
        }

        pub fn client_data(&self, kind: &str, challenge: &str) -> Vec<u8> {
            serde_json::json!({ "type": kind, "challenge": challenge, "origin": self.origin, "crossOrigin": false }).to_string().into_bytes()
        }

        /// `navigator.credentials.create()` : `(clientDataJSON, attestationObject)`.
        pub fn create(&self, challenge: &str) -> (Vec<u8>, Vec<u8>) {
            let object = enc_map(&[
                (enc_text("fmt"), enc_text("none")),
                (enc_text("attStmt"), enc_map(&[])),
                (enc_text("authData"), enc_bytes(&self.auth_data(true))),
            ]);
            (self.client_data("webauthn.create", challenge), object)
        }

        /// `navigator.credentials.get()`.
        pub fn get(&mut self, challenge: &str) -> Assertion {
            self.counter += 1;
            let client_data = self.client_data("webauthn.get", challenge);
            let authenticator = self.auth_data(false);
            let mut message = authenticator.clone();
            message.extend_from_slice(digest::digest(&digest::SHA256, &client_data).as_ref());
            let signature = match (&self.ec, &self.ed) {
                (Some(ec), _) => ec.sign(&SystemRandom::new(), &message).unwrap().as_ref().to_vec(),
                (_, Some(ed)) => ed.sign(&message).as_ref().to_vec(),
                _ => unreachable!(),
            };
            Assertion { credential: self.credential.clone(), client_data, authenticator, signature, user: self.user.clone() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use ring::rand::SystemRandom;

    fn stored(key: &Registration) -> (i64, Vec<u8>, i64) {
        (key.alg, key.public_key.clone(), key.counter)
    }

    fn sk(s: &(i64, Vec<u8>, i64)) -> StoredKey<'_> {
        StoredKey { alg: s.0, public_key: &s.1, counter: s.2 }
    }

    #[test]
    fn cbor_et_cles_cose() {
        let bytes = enc_map(&[(enc_int(-300), enc_text("é")), (enc_text("a"), [head(4, 2), enc_int(70000), enc_bytes(b"xy")].concat())]);
        let (v, rest) = cbor(&[bytes.as_slice(), b"suite"].concat()).map(|(v, r)| (v, r.to_vec())).unwrap();
        assert_eq!(rest, b"suite");
        assert_eq!(v.int(-300), Some(&Cbor::Text("é".into())));
        assert_eq!(v.text("a"), Some(&Cbor::Array(vec![Cbor::Int(70000), Cbor::Bytes(b"xy".to_vec())])));
        // Tronqué, longueur indéfinie, longueur démesurée, trop profond : refusés sans paniquer.
        assert!(cbor(&bytes[..bytes.len() - 1]).is_none());
        assert!(cbor(&[0x5f]).is_none());
        assert!(cbor(&[0x5b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).is_none());
        assert!(cbor(&[0x81; 40]).is_none());
        assert!(cbor(&[]).is_none());
        // Une clé COSE inconnue (ES384) est refusée.
        let es384 = enc_map(&[(enc_int(1), enc_int(2)), (enc_int(3), enc_int(-35)), (enc_int(-1), enc_int(2))]);
        assert_eq!(cose_key(&cbor(&es384).unwrap().0), None);
        let short = enc_map(&[(enc_int(1), enc_int(1)), (enc_int(3), enc_int(-8)), (enc_int(-1), enc_int(6)), (enc_int(-2), enc_bytes(&[1; 31]))]);
        assert_eq!(cose_key(&cbor(&short).unwrap().0), None);
    }

    #[test]
    fn identifiant_et_origine_du_relais() {
        assert_eq!(Rp::of("https://share.kybernow.app"), Rp { id: "share.kybernow.app".into(), origin: "https://share.kybernow.app".into() });
        assert_eq!(Rp::of("https://Relais.Exemple.org:8443").id, "relais.exemple.org");
        assert_eq!(Rp::of("https://relais.exemple.org:8443").origin, "https://relais.exemple.org:8443");
        assert_eq!(Rp::of("http://localhost:8787"), Rp { id: "localhost".into(), origin: "http://localhost:8787".into() });
    }

    #[test]
    fn ajout_puis_connexion_es256_et_ed25519() {
        let rp = Rp::of("https://share.test");
        for alg in [ES256, ED25519] {
            let mut auth = Authenticator::new(alg, "https://share.test");
            let (client, object) = auth.create("defi");
            assert_eq!(client_data(&rp, &client, "webauthn.create"), Ok("defi".into()));
            let key = verify_registration(&rp, &object).unwrap();
            assert_eq!((key.alg, key.credential.clone(), key.counter), (alg, auth.credential.clone(), 0));
            let mut stored = stored(&key);
            let a = auth.get("defi2");
            assert_eq!(client_data(&rp, &a.client_data, "webauthn.get"), Ok("defi2".into()));
            assert_eq!(verify_assertion(&rp, &sk(&stored), b"", &a), Ok(1));
            stored.2 = 1;
            // Rejouée telle quelle : le compteur ne monte pas.
            assert_eq!(verify_assertion(&rp, &sk(&stored), b"", &a), Err(Refused::Counter));
            // Signature d'un autre message.
            let mut b = auth.get("defi3");
            b.client_data = auth.client_data("webauthn.get", "autre");
            assert_eq!(verify_assertion(&rp, &sk(&stored), b"", &b), Err(Refused::Signature));
            // L'identifiant du compte, s'il est donné, doit être le bon.
            auth.user = b"pas le bon".to_vec();
            assert_eq!(verify_assertion(&rp, &sk(&stored), b"le bon", &auth.get("d")), Err(Refused::UserHandle));
            auth.user = b"le bon".to_vec();
            assert!(verify_assertion(&rp, &sk(&stored), b"le bon", &auth.get("d")).is_ok());
        }
    }

    #[test]
    fn refus_origine_type_relais_et_verification() {
        let rp = Rp::of("https://share.test");
        let mut auth = Authenticator::new(ES256, "https://share.test");
        let (client, _) = auth.create("d");
        assert_eq!(client_data(&rp, &client, "webauthn.get"), Err(Refused::Type));
        assert_eq!(client_data(&Rp::of("https://share.test:8443"), &client, "webauthn.create"), Err(Refused::Origin));
        assert_eq!(client_data(&rp, b"{}", "webauthn.create"), Err(Refused::Malformed));
        let cross = br#"{"type":"webauthn.create","challenge":"d","origin":"https://share.test","crossOrigin":true}"#;
        assert_eq!(client_data(&rp, cross, "webauthn.create"), Err(Refused::Origin));
        // Une passkey créée pour un autre relais.
        let other = Authenticator::new(ES256, "https://autre.test");
        assert_eq!(verify_registration(&rp, &other.create("d").1), Err(Refused::RpId));
        // Sans vérification de l'utilisateur (UV), une passkey ne vaut pas double authentification.
        auth.flags = FLAG_UP;
        assert_eq!(verify_registration(&rp, &auth.create("d").1), Err(Refused::Flags));
        auth.flags = FLAG_UP | FLAG_UV;
        let key = verify_registration(&rp, &auth.create("d").1).unwrap();
        auth.flags = FLAG_UV;
        assert_eq!(verify_assertion(&rp, &sk(&stored(&key)), b"", &auth.get("d")), Err(Refused::Flags));
        assert_eq!(verify_registration(&rp, b"\xa0"), Err(Refused::Malformed));
        assert_eq!(verify_registration(&rp, b"pas du cbor"), Err(Refused::Malformed));
    }

    #[test]
    fn rs256() {
        // Une clé RSA de 2048 bits (n, e) et une signature PKCS#1 v1.5 faite par ring.
        // Une clé d'essai seulement (RSAPrivateKey en DER), faite avec openssl.
        let der = include_bytes!("../tests/data/rsa-2048.der");
        let pair = signature::RsaKeyPair::from_der(der).unwrap();
        let components: signature::RsaPublicKeyComponents<Vec<u8>> = pair.public().into();
        let cose = enc_map(&[
            (enc_int(1), enc_int(3)),
            (enc_int(3), enc_int(RS256)),
            (enc_int(-1), enc_bytes(&components.n)),
            (enc_int(-2), enc_bytes(&components.e)),
        ]);
        let key = cose_key(&cbor(&cose).unwrap().0).unwrap();
        let mut sig = vec![0u8; pair.public().modulus_len()];
        pair.sign(&signature::RSA_PKCS1_SHA256, &SystemRandom::new(), b"message", &mut sig).unwrap();
        assert!(verify_signature(RS256, &key.key, b"message", &sig));
        assert!(!verify_signature(RS256, &key.key, b"autre", &sig));
        assert!(!verify_signature(RS256, &[0, 5, 1], b"message", &sig));
        assert!(!verify_signature(-1, &key.key, b"message", &sig));
    }
}
