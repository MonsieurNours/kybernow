//! Le jeton d'accès des comptes KyberNow (docs/comptes.md, « Le jeton
//! d'accès ») : délivré par le relais à un appareil connecté pour un
//! ordinateur (`POST /v1/access`), présenté à Admin
//! (`Authorization: KyberNow <jeton>`), vérifié par la Station avec la clé du
//! relais épinglée au rattachement.
//!
//! Forme compacte : `<charge>.<signature>`, chacune en base64url sans
//! remplissage. La charge est le JSON des [`Claims`] ; la signature Ed25519
//! porte sur `kybernow-access/1 ` suivi de la charge encodée (les octets
//! exacts du jeton avant le point), pour qu'aucune autre signature de la clé
//! du relais ne puisse passer pour un jeton.
//!
//! - côté relais : [`Signer`] (clé Ed25519 en PKCS#8) ;
//! - côté Station : [`Verifier`] (clés épinglées, Station, échéance avec
//!   tolérance d'horloge, `jti` jamais rejoué) ;
//! - la clé publique publiée à `GET /v1/keys` : [`KeySet`], [`PublicKey`].

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::digest::{SHA256, digest};
use ring::rand::SystemRandom;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use crate::id::{base32_lower, random_bytes};

/// Version du format (`v`).
pub const VERSION: u32 = 1;
/// Durée de vie d'un jeton délivré par le relais : 5 minutes.
pub const LIFETIME_SECS: u64 = 5 * 60;
/// Durée de vie la plus longue acceptée par la Station (`exp - iat`) : un
/// relais qui délivrerait des jetons plus longs est refusé.
pub const MAX_LIFETIME_SECS: u64 = LIFETIME_SECS;
/// Tolérance d'horloge entre le relais et la Station, dans les deux sens.
pub const CLOCK_SKEW_SECS: u64 = 60;
/// Le mot du schéma d'autorisation HTTP : `Authorization: KyberNow <jeton>`.
pub const SCHEME: &str = "KyberNow";
/// Le chemin de la clé publique chez le relais.
pub const KEYS_PATH: &str = "/v1/keys";
/// L'algorithme annoncé dans [`PublicKey::alg`].
pub const ALG: &str = "Ed25519";
/// Taille maximale d'un jeton accepté (octets), avant tout décodage.
pub const MAX_TOKEN_LEN: usize = 4096;

const SIG_CONTEXT: &[u8] = b"kybernow-access/1 ";
/// `jti` retenus au plus (au-delà, les plus vieux échus partent d'abord ;
/// s'ils sont tous valides, le jeton est refusé : `Busy`).
const MAX_SEEN: usize = 100_000;

// --- Droits ---------------------------------------------------------------------------

/// Un droit sur un ordinateur (docs/comptes.md, « Droits sur un ordinateur »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Right {
    /// Le bureau entier : les routes `/api/stream/*` d'Admin.
    Session,
    /// Le lanceur du prêt (docs/pret.md).
    Lend,
    /// Rejoindre les partages de l'ordinateur sans code ni accord de l'hôte.
    Share,
    /// Redémarrer et mettre à jour la Station, y compris par le relais.
    Admin,
}

impl Right {
    pub const ALL: [Right; 4] = [Right::Session, Right::Lend, Right::Share, Right::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Right::Session => "session",
            Right::Lend => "lend",
            Right::Share => "share",
            Right::Admin => "admin",
        }
    }

    pub fn parse(s: &str) -> Option<Right> {
        Right::ALL.into_iter().find(|r| r.as_str() == s)
    }
}

impl fmt::Display for Right {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Les droits d'un jeton : une liste de mots (`["session","share"]`). Un mot
/// inconnu (droit d'une version suivante) est ignoré, sans refuser le jeton.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rights(Vec<Right>);

impl Rights {
    pub fn new(rights: impl IntoIterator<Item = Right>) -> Self {
        let mut v: Vec<Right> = rights.into_iter().collect();
        v.sort();
        v.dedup();
        Rights(v)
    }

    pub fn has(&self, right: Right) -> bool {
        self.0.contains(&right)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Right> + '_ {
        self.0.iter().copied()
    }

    /// Les droits communs aux deux (ceux du jeton, bornés par ceux gardés par la Station).
    pub fn intersect(&self, other: &Rights) -> Rights {
        Rights(self.0.iter().copied().filter(|r| other.has(*r)).collect())
    }
}

impl FromIterator<Right> for Rights {
    fn from_iter<I: IntoIterator<Item = Right>>(iter: I) -> Self {
        Rights::new(iter)
    }
}

impl Serialize for Rights {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|r| r.as_str()))
    }
}

impl<'de> Deserialize<'de> for Rights {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let words = Vec::<String>::deserialize(d)?;
        Ok(words.iter().filter_map(|w| Right::parse(w)).collect())
    }
}

// --- Charge --------------------------------------------------------------------------

/// Ce que dit un jeton.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Version du format : [`VERSION`].
    pub v: u32,
    /// Le relais qui délivre (son adresse, `share.kybernow.app`) : celui auquel la Station est rattachée.
    pub relay: String,
    /// L'identifiant de la Station visée (`id::station_id`).
    pub station: String,
    /// L'identifiant du compte KyberNow.
    pub account: String,
    /// L'identifiant de l'appareil connecté (session d'appareil du relais).
    pub device: String,
    /// Les droits du compte sur cet ordinateur, à cet instant.
    pub rights: Rights,
    /// Ce qu'Admin affiche : le nom de la personne et de son appareil.
    pub name: Names,
    /// Délivré à (secondes Unix).
    pub iat: u64,
    /// Échéance (secondes Unix).
    pub exp: u64,
    /// Identifiant unique du jeton (16 octets en base32).
    pub jti: String,
}

/// Les noms portés par le jeton, pour la liste locale d'Admin.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Names {
    /// Le nom de la personne (« Léa Martin »).
    pub account: String,
    /// Son adresse e-mail, si le relais la donne.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Le nom de l'appareil (« iPhone de Léa »).
    pub device: String,
}

impl Claims {
    /// Un jeton neuf délivré à `now` pour [`LIFETIME_SECS`], avec un `jti` tiré au hasard.
    pub fn issue(relay: &str, station: &str, account: &str, device: &str, rights: Rights, name: Names, now: u64) -> Claims {
        Claims {
            v: VERSION,
            relay: relay.into(),
            station: station.into(),
            account: account.into(),
            device: device.into(),
            rights,
            name,
            iat: now,
            exp: now + LIFETIME_SECS,
            jti: new_jti(),
        }
    }
}

/// Un `jti` : 16 octets aléatoires en base32 minuscule.
pub fn new_jti() -> String {
    base32_lower(&random_bytes::<16>())
}

// --- Clés ---------------------------------------------------------------------------

/// Une clé publique du relais, telle que publiée à `GET /v1/keys` et épinglée par la Station.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey {
    /// Identifiant de la clé : les 8 premiers octets du SHA-256 de la clé brute, en base32.
    pub kid: String,
    /// Toujours [`ALG`].
    pub alg: String,
    /// La clé Ed25519 brute (32 octets), en base64url sans remplissage.
    pub key: String,
}

impl PublicKey {
    /// D'après la clé brute (32 octets).
    pub fn from_raw(raw: &[u8]) -> Option<PublicKey> {
        (raw.len() == 32).then(|| PublicKey { kid: key_id(raw), alg: ALG.into(), key: URL_SAFE_NO_PAD.encode(raw) })
    }

    /// La clé brute, si la forme tient (algorithme, longueur, `kid` cohérent).
    pub fn raw(&self) -> Option<Vec<u8>> {
        if self.alg != ALG {
            return None;
        }
        let raw = URL_SAFE_NO_PAD.decode(self.key.trim()).ok()?;
        (raw.len() == 32 && key_id(&raw) == self.kid).then_some(raw)
    }

    /// Bien formée.
    pub fn is_valid(&self) -> bool {
        self.raw().is_some()
    }

    /// L'empreinte qu'Admin affiche (« clé du relais ») : `sha256:` et les
    /// 32 octets du SHA-256 de la clé brute, en hexadécimal par paires.
    pub fn fingerprint(&self) -> Option<String> {
        let raw = self.raw()?;
        let hex: Vec<String> = digest(&SHA256, &raw).as_ref().iter().map(|b| format!("{b:02x}")).collect();
        Some(format!("sha256:{}", hex.join(":")))
    }
}

fn key_id(raw: &[u8]) -> String {
    base32_lower(&digest(&SHA256, raw).as_ref()[..8])
}

/// La réponse de `GET /v1/keys` : une clé, ou deux pendant une rotation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySet {
    pub keys: Vec<PublicKey>,
}

impl KeySet {
    /// Les clés bien formées seulement.
    pub fn valid(&self) -> Vec<PublicKey> {
        self.keys.iter().filter(|k| k.is_valid()).cloned().collect()
    }
}

// --- Erreurs -----------------------------------------------------------------------

/// Pourquoi un jeton est refusé. `code()` donne le mot rendu par Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessError {
    /// Pas un jeton (forme, base64, JSON, taille).
    Malformed,
    /// Une version inconnue.
    Version,
    /// Aucune clé épinglée ne vérifie la signature.
    Signature,
    /// Délivré par un autre relais que celui du rattachement.
    Relay,
    /// Pour une autre Station.
    Station,
    /// Échu (au-delà de la tolérance d'horloge).
    Expired,
    /// Délivré dans le futur (au-delà de la tolérance), ou plus long que permis.
    NotYetValid,
    /// Ce `jti` a déjà servi (pour un autre contenu, ou une seule fois permise).
    Replayed,
    /// Trop de jetons retenus à la fois.
    Busy,
    /// Aucune clé épinglée (Station pas rattachée).
    NoKey,
}

impl AccessError {
    pub fn code(self) -> &'static str {
        match self {
            AccessError::Malformed => "malformed",
            AccessError::Version => "version",
            AccessError::Signature => "signature",
            AccessError::Relay => "relay",
            AccessError::Station => "station",
            AccessError::Expired => "expired",
            AccessError::NotYetValid => "not_yet_valid",
            AccessError::Replayed => "replayed",
            AccessError::Busy => "busy",
            AccessError::NoKey => "no_key",
        }
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for AccessError {}

// --- Relais : signer ---------------------------------------------------------------

/// La clé de signature du relais.
pub struct Signer {
    pair: Ed25519KeyPair,
    public: PublicKey,
}

impl fmt::Debug for Signer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signer").field("kid", &self.public.kid).finish_non_exhaustive()
    }
}

impl Signer {
    /// Une nouvelle clé, en PKCS#8 (DER), à garder dans un fichier réservé au relais.
    pub fn generate_pkcs8() -> Result<Vec<u8>, AccessError> {
        Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map(|d| d.as_ref().to_vec()).map_err(|_| AccessError::NoKey)
    }

    /// D'après une clé Ed25519 en PKCS#8 (DER).
    pub fn from_pkcs8(der: &[u8]) -> Result<Signer, AccessError> {
        let pair = Ed25519KeyPair::from_pkcs8_maybe_unchecked(der).map_err(|_| AccessError::NoKey)?;
        let public = PublicKey::from_raw(pair.public_key().as_ref()).ok_or(AccessError::NoKey)?;
        Ok(Signer { pair, public })
    }

    /// La clé publique, à publier à `GET /v1/keys`.
    pub fn public_key(&self) -> &PublicKey {
        &self.public
    }

    /// Le jeton compact de ces `claims`.
    pub fn sign(&self, claims: &Claims) -> String {
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("charge JSON"));
        let sig = self.pair.sign(&signed_message(&payload));
        format!("{payload}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()))
    }
}

fn signed_message(payload_b64: &str) -> Vec<u8> {
    let mut m = SIG_CONTEXT.to_vec();
    m.extend_from_slice(payload_b64.as_bytes());
    m
}

/// Lit la charge d'un jeton sans rien vérifier (affichage, journal). Ne
/// jamais s'en servir pour décider d'un accès : [`Verifier::verify`].
pub fn peek(token: &str) -> Result<Claims, AccessError> {
    let (payload, _) = split(token)?;
    decode_claims(payload)
}

fn split(token: &str) -> Result<(&str, &str), AccessError> {
    let token = token.trim();
    if token.is_empty() || token.len() > MAX_TOKEN_LEN {
        return Err(AccessError::Malformed);
    }
    let (payload, sig) = token.split_once('.').ok_or(AccessError::Malformed)?;
    if payload.is_empty() || sig.is_empty() || sig.contains('.') {
        return Err(AccessError::Malformed);
    }
    Ok((payload, sig))
}

fn decode_claims(payload: &str) -> Result<Claims, AccessError> {
    let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|_| AccessError::Malformed)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| AccessError::Malformed)?;
    // La version d'abord : une charge d'une version suivante n'a peut-être pas la même forme.
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(VERSION) => {}
        Some(_) => return Err(AccessError::Version),
        None => return Err(AccessError::Malformed),
    }
    serde_json::from_value(value).map_err(|_| AccessError::Malformed)
}

/// Le jeton d'un en-tête `Authorization`, s'il est du schéma `KyberNow`.
pub fn from_authorization(value: &str) -> Option<&str> {
    let (scheme, rest) = value.trim().split_once(' ')?;
    let token = rest.trim();
    (scheme.eq_ignore_ascii_case(SCHEME) && !token.is_empty()).then_some(token)
}

// --- Station : vérifier ------------------------------------------------------------

/// Comment un jeton est employé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// Le même jeton sert à tous les appels de son appareil jusqu'à son
    /// échéance (session, signe de vie) ; son `jti` n'est jamais accepté
    /// pour un autre contenu.
    Reusable,
    /// Une seule fois (redémarrer, mettre à jour) : le `jti` est consommé,
    /// et le jeton ne sert plus à rien d'autre.
    Once,
}

/// Ce que la Station a épinglé au rattachement, et les `jti` déjà vus.
pub struct Verifier {
    relay: String,
    station: String,
    keys: Vec<Vec<u8>>,
    skew: u64,
    seen: Mutex<HashMap<String, Seen>>,
}

#[derive(Debug, Clone, Copy)]
struct Seen {
    /// Empreinte (SHA-256) du jeton entier : le même `jti` avec un autre contenu est refusé.
    hash: [u8; 32],
    /// Retenu jusqu'à cette date (échéance plus la tolérance).
    until: u64,
    /// Consommé par un emploi `Once`.
    spent: bool,
}

impl fmt::Debug for Verifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Verifier").field("relay", &self.relay).field("station", &self.station).field("keys", &self.keys.len()).finish_non_exhaustive()
    }
}

impl Verifier {
    /// Pour la Station `station`, rattachée au relais `relay`, avec ses clés
    /// épinglées (une, ou deux pendant une rotation). Les clés mal formées sont ignorées.
    pub fn new(relay: &str, station: &str, keys: &[PublicKey]) -> Verifier {
        Verifier {
            relay: relay.into(),
            station: station.into(),
            keys: keys.iter().filter_map(PublicKey::raw).collect(),
            skew: CLOCK_SKEW_SECS,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Une autre tolérance d'horloge (essais).
    pub fn with_skew(mut self, skew: u64) -> Verifier {
        self.skew = skew;
        self
    }

    pub fn relay(&self) -> &str {
        &self.relay
    }

    pub fn station(&self) -> &str {
        &self.station
    }

    /// Vérifie un jeton à l'instant `now` (secondes Unix) : forme, version,
    /// signature d'une clé épinglée, relais, Station, dates (avec la
    /// tolérance), `jti`. Rend ce qu'il dit.
    pub fn verify(&self, token: &str, now: u64, use_: Use) -> Result<Claims, AccessError> {
        if self.keys.is_empty() {
            return Err(AccessError::NoKey);
        }
        let (payload, sig) = split(token)?;
        let sig = URL_SAFE_NO_PAD.decode(sig).map_err(|_| AccessError::Malformed)?;
        let message = signed_message(payload);
        // La signature avant de lire quoi que ce soit de la charge.
        if !self.keys.iter().any(|k| UnparsedPublicKey::new(&ED25519, k).verify(&message, &sig).is_ok()) {
            return Err(AccessError::Signature);
        }
        let claims = decode_claims(payload)?;
        if claims.relay != self.relay {
            return Err(AccessError::Relay);
        }
        if claims.station != self.station {
            return Err(AccessError::Station);
        }
        if claims.exp <= claims.iat || claims.exp - claims.iat > MAX_LIFETIME_SECS || claims.iat > now + self.skew {
            return Err(AccessError::NotYetValid);
        }
        if claims.exp + self.skew <= now {
            return Err(AccessError::Expired);
        }
        if claims.jti.is_empty() || claims.jti.len() > 64 {
            return Err(AccessError::Malformed);
        }
        self.note_jti(&claims, token.trim(), now, use_)?;
        Ok(claims)
    }

    fn note_jti(&self, claims: &Claims, token: &str, now: u64, use_: Use) -> Result<(), AccessError> {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(digest(&SHA256, token.as_bytes()).as_ref());
        let until = claims.exp + self.skew;
        let mut seen = self.seen.lock().unwrap();
        if let Some(prev) = seen.get_mut(&claims.jti) {
            if prev.hash != hash || prev.spent {
                return Err(AccessError::Replayed);
            }
            if use_ == Use::Once {
                prev.spent = true;
            }
            return Ok(());
        }
        if seen.len() >= MAX_SEEN {
            seen.retain(|_, s| s.until > now);
            if seen.len() >= MAX_SEEN {
                return Err(AccessError::Busy);
            }
        }
        seen.insert(claims.jti.clone(), Seen { hash, until, spent: use_ == Use::Once });
        Ok(())
    }

    /// Oublie les `jti` échus (appelé de temps en temps ; `verify` le fait aussi quand la table est pleine).
    pub fn prune(&self, now: u64) {
        self.seen.lock().unwrap().retain(|_, s| s.until > now);
    }

    /// Nombre de `jti` retenus.
    pub fn remembered(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_795_000_000;
    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";
    const RELAY: &str = "share.kybernow.app";

    fn signer() -> Signer {
        Signer::from_pkcs8(&Signer::generate_pkcs8().unwrap()).unwrap()
    }

    fn claims(rights: &[Right]) -> Claims {
        Claims::issue(
            RELAY,
            STATION,
            "acc_1",
            "dev_1",
            rights.iter().copied().collect(),
            Names { account: "Léa Martin".into(), email: Some("lea@example.org".into()), device: "iPhone de Léa".into() },
            NOW,
        )
    }

    fn verifier(s: &Signer) -> Verifier {
        Verifier::new(RELAY, STATION, std::slice::from_ref(s.public_key()))
    }

    #[test]
    fn aller_retour() {
        let s = signer();
        let c = claims(&[Right::Session, Right::Share]);
        let token = s.sign(&c);
        assert_eq!(token.matches('.').count(), 1);
        assert!(!token.contains('=') && !token.contains('+') && !token.contains('/'));
        let got = verifier(&s).verify(&token, NOW + 10, Use::Reusable).unwrap();
        assert_eq!(got, c);
        assert!(got.rights.has(Right::Session) && got.rights.has(Right::Share) && !got.rights.has(Right::Admin));
        assert_eq!(c.exp - c.iat, LIFETIME_SECS);
        assert_eq!(peek(&token).unwrap(), c);
    }

    #[test]
    fn forme_json_de_la_charge() {
        let c = claims(&[Right::Admin, Right::Session, Right::Session]);
        let v = serde_json::to_value(&c).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        for k in ["v", "relay", "station", "account", "device", "rights", "name", "iat", "exp", "jti"] {
            assert!(keys.contains(&k), "{k}");
        }
        assert_eq!(v["rights"], serde_json::json!(["session", "admin"]));
        assert_eq!(v["v"], 1);
        assert_eq!(c.jti.len(), 26);
    }

    #[test]
    fn droit_inconnu_ignore() {
        let r: Rights = serde_json::from_str(r#"["session","plus-tard","lend"]"#).unwrap();
        assert_eq!(r, Rights::new([Right::Lend, Right::Session]));
        let other = Rights::new([Right::Lend, Right::Admin]);
        assert_eq!(r.intersect(&other), Rights::new([Right::Lend]));
    }

    #[test]
    fn signature_d_une_autre_cle_refusee() {
        let (a, b) = (signer(), signer());
        let token = b.sign(&claims(&[Right::Session]));
        assert_eq!(verifier(&a).verify(&token, NOW, Use::Reusable), Err(AccessError::Signature));
        // Deux clés épinglées (rotation) : l'une ou l'autre.
        let both = Verifier::new(RELAY, STATION, &[a.public_key().clone(), b.public_key().clone()]);
        assert!(both.verify(&token, NOW, Use::Reusable).is_ok());
        // Aucune clé : la Station n'est pas rattachée.
        assert_eq!(Verifier::new(RELAY, STATION, &[]).verify(&token, NOW, Use::Reusable), Err(AccessError::NoKey));
    }

    #[test]
    fn charge_modifiee_refusee() {
        let s = signer();
        let token = s.sign(&claims(&[Right::Session]));
        let (_, sig) = token.split_once('.').unwrap();
        let mut forged = claims(&[Right::Session, Right::Admin]);
        forged.jti = "autre".into();
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&forged).unwrap());
        assert_eq!(verifier(&s).verify(&format!("{payload}.{sig}"), NOW, Use::Reusable), Err(AccessError::Signature));
        // La signature seule, sans le contexte, ne fait pas un jeton.
        let raw_sig = s.pair.sign(payload.as_bytes());
        let t = format!("{payload}.{}", URL_SAFE_NO_PAD.encode(raw_sig.as_ref()));
        assert_eq!(verifier(&s).verify(&t, NOW, Use::Reusable), Err(AccessError::Signature));
    }

    #[test]
    fn autre_station_autre_relais() {
        let s = signer();
        let mut c = claims(&[Right::Session]);
        c.station = "zzzzzzzzzzzzzzzzzzzzzzzzzz".into();
        assert_eq!(verifier(&s).verify(&s.sign(&c), NOW, Use::Reusable), Err(AccessError::Station));
        let mut c = claims(&[Right::Session]);
        c.relay = "relais.example.org".into();
        assert_eq!(verifier(&s).verify(&s.sign(&c), NOW, Use::Reusable), Err(AccessError::Relay));
    }

    #[test]
    fn echeance_et_tolerance() {
        let s = signer();
        let token = s.sign(&claims(&[Right::Session]));
        let v = verifier(&s);
        // Jusqu'à l'échéance plus une minute.
        assert!(v.verify(&token, NOW + LIFETIME_SECS + CLOCK_SKEW_SECS - 1, Use::Reusable).is_ok());
        assert_eq!(verifier(&s).verify(&token, NOW + LIFETIME_SECS + CLOCK_SKEW_SECS, Use::Reusable), Err(AccessError::Expired));
        // Horloge de la Station en retard d'une minute : accepté ; de deux : refusé.
        assert!(verifier(&s).verify(&token, NOW - CLOCK_SKEW_SECS, Use::Reusable).is_ok());
        assert_eq!(verifier(&s).verify(&token, NOW - CLOCK_SKEW_SECS - 1, Use::Reusable), Err(AccessError::NotYetValid));
        // Sans tolérance.
        let strict = verifier(&s).with_skew(0);
        assert_eq!(strict.verify(&token, NOW + LIFETIME_SECS, Use::Reusable), Err(AccessError::Expired));
    }

    #[test]
    fn jeton_trop_long_refuse() {
        let s = signer();
        let mut c = claims(&[Right::Session]);
        c.exp = c.iat + 24 * 3600;
        assert_eq!(verifier(&s).verify(&s.sign(&c), NOW, Use::Reusable), Err(AccessError::NotYetValid));
        let mut c = claims(&[Right::Session]);
        c.exp = c.iat;
        assert_eq!(verifier(&s).verify(&s.sign(&c), NOW, Use::Reusable), Err(AccessError::NotYetValid));
    }

    #[test]
    fn jti_jamais_rejoue() {
        let s = signer();
        let v = verifier(&s);
        let c = claims(&[Right::Session]);
        let token = s.sign(&c);
        // Le même jeton sert à tous les appels de son appareil.
        assert!(v.verify(&token, NOW, Use::Reusable).is_ok());
        assert!(v.verify(&token, NOW + 30, Use::Reusable).is_ok());
        // Le même jti avec un autre contenu (même signé par le relais) : refusé.
        let mut other = c.clone();
        other.device = "dev_2".into();
        assert_eq!(v.verify(&s.sign(&other), NOW, Use::Reusable), Err(AccessError::Replayed));
        // Une seule fois : consommé, puis refusé même pour un emploi ordinaire.
        let once = s.sign(&claims(&[Right::Admin]));
        assert!(v.verify(&once, NOW, Use::Once).is_ok());
        assert_eq!(v.verify(&once, NOW, Use::Once), Err(AccessError::Replayed));
        assert_eq!(v.verify(&once, NOW, Use::Reusable), Err(AccessError::Replayed));
        // Déjà employé pour la session, puis pour redémarrer : consommé à ce moment-là.
        let reused = s.sign(&claims(&[Right::Admin]));
        assert!(v.verify(&reused, NOW, Use::Reusable).is_ok());
        assert!(v.verify(&reused, NOW, Use::Once).is_ok());
        assert_eq!(v.verify(&reused, NOW, Use::Reusable), Err(AccessError::Replayed));
        // Les jti échus s'oublient (le jeton, lui, est refusé par son échéance).
        assert_eq!(v.remembered(), 3);
        v.prune(NOW + LIFETIME_SECS + CLOCK_SKEW_SECS);
        assert_eq!(v.remembered(), 0);
        assert_eq!(v.verify(&token, NOW + LIFETIME_SECS + CLOCK_SKEW_SECS, Use::Reusable), Err(AccessError::Expired));
    }

    #[test]
    fn formes_refusees() {
        let s = signer();
        let v = verifier(&s);
        for t in ["", "abc", "a.b.c", ".", "a.", ".b", "!!!.???"] {
            assert!(matches!(v.verify(t, NOW, Use::Reusable), Err(AccessError::Malformed | AccessError::Signature)), "{t:?}");
        }
        assert_eq!(v.verify(&"a".repeat(MAX_TOKEN_LEN + 1), NOW, Use::Reusable), Err(AccessError::Malformed));
        // Signée, mais pas du JSON attendu.
        let payload = URL_SAFE_NO_PAD.encode(br#"{"v":1,"bonjour":true}"#);
        let sig = s.pair.sign(&signed_message(&payload));
        let t = format!("{payload}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()));
        assert_eq!(v.verify(&t, NOW, Use::Reusable), Err(AccessError::Malformed));
        // Une version suivante.
        let payload = URL_SAFE_NO_PAD.encode(br#"{"v":2}"#);
        let sig = s.pair.sign(&signed_message(&payload));
        let t = format!("{payload}.{}", URL_SAFE_NO_PAD.encode(sig.as_ref()));
        assert_eq!(v.verify(&t, NOW, Use::Reusable), Err(AccessError::Version));
    }

    #[test]
    fn en_tete_authorization() {
        assert_eq!(from_authorization("KyberNow abc.def"), Some("abc.def"));
        assert_eq!(from_authorization("kybernow   abc.def "), Some("abc.def"));
        assert_eq!(from_authorization("Basic YTpi"), None);
        assert_eq!(from_authorization("KyberNow "), None);
        assert_eq!(from_authorization("KyberNow"), None);
    }

    #[test]
    fn cle_publique_publiee() {
        let s = signer();
        let k = s.public_key().clone();
        assert!(k.is_valid());
        assert_eq!(k.alg, "Ed25519");
        assert_eq!(k.kid.len(), 13);
        assert_eq!(URL_SAFE_NO_PAD.decode(&k.key).unwrap().len(), 32);
        let fp = k.fingerprint().unwrap();
        assert!(fp.starts_with("sha256:") && fp.len() == 7 + 32 * 3 - 1);
        // Le JSON de `GET /v1/keys`, et retour.
        let set = KeySet { keys: vec![k.clone()] };
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.starts_with(r#"{"keys":[{"kid":""#));
        let back: KeySet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.valid(), vec![k.clone()]);
        // Un kid qui ne correspond pas, un autre algorithme : ignorés.
        let mut bad = k.clone();
        bad.kid = "aaaaaaaaaaaaa".into();
        let mut rsa = k.clone();
        rsa.alg = "RS256".into();
        assert!(KeySet { keys: vec![bad, rsa] }.valid().is_empty());
        // La même clé relue d'après son PKCS#8.
        let der = Signer::generate_pkcs8().unwrap();
        assert_eq!(Signer::from_pkcs8(&der).unwrap().public_key(), Signer::from_pkcs8(&der).unwrap().public_key());
        assert!(Signer::from_pkcs8(b"pas une cle").is_err());
    }
}
