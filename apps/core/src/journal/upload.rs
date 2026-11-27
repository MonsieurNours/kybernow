//! « Envoyer le journal » par QR code (docs/link-ui.md, « Réglages de l'app ›
//! À propos » ; contrat : docs/relais.md, « Dépôt d'un journal ») : une seule
//! copie pour toutes les apps (Apple TV, Android TV, Linux au salon et Link OS,
//! LG), qui l'écrivaient chacune.
//!
//! Le journal est compressé (gzip), puis chiffré sur l'appareil en
//! AES-256-GCM, sans données associées, avec une clé et un nonce tirés au
//! hasard pour ce dépôt. Le dépôt : `KNJ1` (4 octets), le nonce (12), le texte
//! chiffré et son étiquette (16). 2 Mio au plus une fois compressé : au-delà,
//! l'en-tête est gardé avec la fin du journal, la plus récente. Il part au
//! relais du compte de l'appareil, sinon au relais public, par l'agent HTTPS
//! du relais ([`crate::relay::relay_agent`] : autorités du web, ou empreinte
//! épinglée d'un relais chez soi). La clé ne part que dans l'adresse du QR
//! code, après `#k=`, que le navigateur du téléphone n'envoie jamais.
//!
//! Bloquant (30 s au plus) : hors du fil de l'interface.

use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;

use crate::i18n::Lang;
use crate::relay::{DEFAULT_RELAY, RelayAddr};

/// Le plus grand journal compressé qu'un dépôt porte (le relais prend 2 Mio et
/// 64 octets : en-tête et étiquette compris).
pub const DEPOSIT_MAX: usize = 2 * 1024 * 1024;
/// Le début d'un dépôt.
pub const MAGIC: &[u8; 4] = b"KNJ1";
/// L'écran du QR code relit l'état du dépôt à ce rythme.
pub const CHECK_EVERY: Duration = Duration::from_secs(2);
/// Durée de vie d'un dépôt quand le relais ne la dit pas.
pub const DEFAULT_LIFETIME_SECS: u64 = 3600;
const CONNECT: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(30);

/// Un dépôt fait : de quoi le montrer, le suivre et le retirer. `owner` ne
/// part jamais ailleurs qu'au relais.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deposit {
    /// Le relais tel que l'app l'a donné (vide : le relais public), pour
    /// [`state`] et [`discard`].
    pub relay: String,
    pub id: String,
    pub owner: String,
    /// L'adresse du QR code, clé comprise (`https://<relais>/journal/<id>#k=<clé>`).
    pub url: String,
    /// Secondes avant que le relais l'efface (une heure).
    pub expires_in: u64,
}

impl Deposit {
    /// `{relay, id, owner, url, expires_in}` (ponts C et JNI).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Ce qui empêche le dépôt (textes de link-ui dans les apps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositError {
    /// Le relais ne répond pas (réseau, nom, certificat), ou une réponse illisible :
    /// « Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez. »
    Unreachable(String),
    /// Relais d'avant le dépôt des journaux (`404`) :
    /// « Ce relais ne prend pas encore les journaux : il doit être mis à jour. »
    OldRelay,
    /// Trop de dépôts depuis cette adresse (`429`), ou relais plein (`503`) :
    /// « Trop d'envois : réessayez dans une heure. »
    TooMany,
    /// Rien n'a pu partir (adresse du relais illisible, tirage au hasard impossible).
    Failed(String),
}

impl DepositError {
    /// Code des ponts : 90 injoignable, 91 relais d'avant, 92 trop d'envois, 1 autre échec.
    pub fn code(&self) -> i32 {
        match self {
            DepositError::Unreachable(_) => 90,
            DepositError::OldRelay => 91,
            DepositError::TooMany => 92,
            DepositError::Failed(_) => 1,
        }
    }

    /// Le détail pour le journal de l'app.
    pub fn detail(&self) -> String {
        match self {
            DepositError::Unreachable(d) | DepositError::Failed(d) => d.clone(),
            DepositError::OldRelay => "HTTP 404".into(),
            DepositError::TooMany => "HTTP 429/503".into(),
        }
    }

    /// Un refus du relais au dépôt.
    fn of_status(status: u16) -> DepositError {
        match status {
            404 => DepositError::OldRelay,
            // 413 n'arrive pas (le journal est tenu dans la limite) ; 503 : relais
            // plein, les dépôts partent au bout d'une heure.
            413 | 429 | 503 => DepositError::TooMany,
            s => DepositError::Unreachable(format!("HTTP {s}")),
        }
    }
}

/// Ce que le relais dit d'un dépôt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remote {
    Waiting,
    /// « Journal ouvert sur le téléphone. »
    Opened,
    /// Plus là : ouvert puis effacé, expiré, retiré.
    Gone,
    /// Pas de réponse cette fois : relire plus tard.
    Unknown,
}

impl Remote {
    /// Code des ponts : 0 en attente, 1 ouvert, 93 plus là, 90 pas de réponse.
    pub fn code(self) -> i32 {
        match self {
            Remote::Waiting => 0,
            Remote::Opened => 1,
            Remote::Gone => 93,
            Remote::Unknown => 90,
        }
    }
}

/// L'en-tête du journal, dans les termes que lit la page du relais (« Appareil
/// : … », « Journal écrit le … » ; “Device: …”, “Log written on …”, dans les
/// 4 premiers Kio) : la version de l'app, l'appareil et son système, les
/// lignes de l'app (`extra` : interface, fins précédentes…), le moment, puis
/// une ligne vide.
pub fn header(lang: Lang, version: &str, device: &str, extra: &[String], now_secs: u64) -> String {
    let at = kybernow_rules::format::Utc::from_unix(now_secs as i64).text(0);
    let mut h = format!("KyberNow Link {version}\n");
    h.push_str(&match lang {
        Lang::Fr => format!("Appareil : {device}\n"),
        Lang::En => format!("Device: {device}\n"),
    });
    for l in extra {
        let l = l.trim_end();
        if !l.is_empty() {
            h.push_str(l);
            h.push('\n');
        }
    }
    h.push_str(&match lang {
        Lang::Fr => format!("Journal écrit le {at} UTC\n\n"),
        Lang::En => format!("Log written on {at} UTC\n\n"),
    });
    h
}

/// [`header`] à l'heure qu'il est.
pub fn header_now(lang: Lang, version: &str, device: &str, extra: &[String]) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    header(lang, version, device, extra, now)
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    // Écrire dans un Vec ne peut pas échouer.
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

/// Le journal compressé, tenu dans `limit` : entier s'il tient, sinon
/// l'en-tête, une ligne qui dit la coupure, puis la fin du journal (coupée au
/// début d'une ligne).
pub fn compress(lang: Lang, header: &str, body: &str, limit: usize) -> Vec<u8> {
    let whole = gzip(format!("{header}{body}").as_bytes());
    if whole.len() <= limit {
        return whole;
    }
    let (mut keep, mut compressed) = (body.len(), whole.len());
    loop {
        // Ce que la place permet, d'après le taux de compression du dernier essai.
        keep = ((keep as f64) * (limit as f64 / compressed as f64) * 0.9) as usize;
        let mut start = body.len().saturating_sub(keep);
        while !body.is_char_boundary(start) {
            start += 1;
        }
        if start > 0
            && let Some(nl) = body[start..].find('\n')
        {
            start += nl + 1;
        }
        let tail = &body[start..];
        let cut = match lang {
            Lang::Fr => format!("[… début du journal retiré : {start} octets]\n"),
            Lang::En => format!("[… start of the log removed: {start} bytes]\n"),
        };
        let out = gzip(format!("{header}{cut}{tail}").as_bytes());
        if out.len() <= limit || tail.is_empty() {
            return out;
        }
        compressed = out.len();
        keep = tail.len();
    }
}

/// Chiffre un journal déjà compressé : `KNJ1`, le nonce, le texte chiffré et
/// son étiquette (AES-256-GCM, sans données associées).
pub fn seal_with(plain: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let sealed = cipher.encrypt(Nonce::from_slice(nonce), plain).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Le dépôt chiffré du journal (`header` puis `body`, du plus ancien au plus
/// récent) et sa clé, tirés au hasard (jamais une clé pour deux dépôts).
pub fn seal(lang: Lang, header: &str, body: &str) -> Result<(Vec<u8>, [u8; 32]), String> {
    let plain = compress(lang, header, body, DEPOSIT_MAX);
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    Ok((seal_with(&plain, &key, &nonce)?, key))
}

/// Ce que fait la page du téléphone : déchiffre et décompresse (`None` si le
/// dépôt ne s'ouvre pas avec cette clé).
pub fn open(deposit: &[u8], key: &[u8; 32]) -> Option<String> {
    use std::io::Read;
    if deposit.len() < MAGIC.len() + 12 + 16 || &deposit[..4] != MAGIC {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let plain = cipher.decrypt(Nonce::from_slice(&deposit[4..16]), &deposit[16..]).ok()?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(plain.as_slice()).read_to_string(&mut s).ok()?;
    Some(s)
}

/// L'adresse du QR code : la page du journal chez ce relais, la clé en
/// base64url (43 signes) après `#k=`.
pub fn page_url(relay_authority: &str, id: &str, key: &[u8; 32]) -> String {
    format!("https://{relay_authority}/journal/{id}#k={}", URL_SAFE_NO_PAD.encode(key))
}

/// Un identifiant ou un jeton du relais lisible : base64url, rien à échapper
/// dans une adresse ni dans un en-tête.
pub fn valid_token(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Le relais du dépôt : celui donné (relais du compte), sinon le relais public.
fn relay_of(relay: &str) -> Result<RelayAddr, String> {
    let r = relay.trim();
    let r = if r.is_empty() { DEFAULT_RELAY } else { r };
    r.parse::<RelayAddr>().map_err(|e| e.to_string())
}

fn agent(relay: &RelayAddr) -> Result<ureq::Agent, String> {
    crate::relay::relay_agent(relay, CONNECT, TIMEOUT)
}

/// La réponse du relais au dépôt (`201 {"id","owner","expires_in"}`).
fn read_posted(relay: &str, authority: &str, key: &[u8; 32], v: &serde_json::Value) -> Result<Deposit, DepositError> {
    let (Some(id), Some(owner)) = (v["id"].as_str().filter(|s| valid_token(s)), v["owner"].as_str().filter(|s| valid_token(s))) else {
        return Err(DepositError::Unreachable("réponse du relais illisible".into()));
    };
    let expires_in = v["expires_in"].as_u64().unwrap_or(DEFAULT_LIFETIME_SECS).clamp(1, DEFAULT_LIFETIME_SECS);
    Ok(Deposit { relay: relay.to_string(), id: id.to_string(), owner: owner.to_string(), url: page_url(authority, id, key), expires_in })
}

/// Compresse, chiffre et dépose le journal sur `relay` (celui du compte
/// KyberNow de l'appareil ; vide : le relais public). `header` : voir
/// [`header`] ; `body` : le journal de l'app, du plus ancien au plus récent.
/// Bloquant (30 s au plus).
pub fn deposit(relay: &str, header: &str, body: &str) -> Result<Deposit, DepositError> {
    let lang = crate::i18n::current();
    let addr = relay_of(relay).map_err(DepositError::Failed)?;
    let (sealed, key) = seal(lang, header, body).map_err(DepositError::Failed)?;
    let agent = agent(&addr).map_err(DepositError::Unreachable)?;
    let authority = addr.web_authority();
    let r = agent.post(&format!("https://{authority}/v1/journal")).set("Content-Type", "application/octet-stream").send_bytes(&sealed);
    let resp = match r {
        Ok(resp) => resp,
        Err(ureq::Error::Status(status, _)) => {
            log::warn!("journal : dépôt refusé par le relais ({status})");
            return Err(DepositError::of_status(status));
        }
        Err(e) => {
            log::warn!("journal : relais {authority} injoignable ({e})");
            return Err(DepositError::Unreachable(e.to_string()));
        }
    };
    let v: serde_json::Value = resp.into_json().unwrap_or_default();
    let d = read_posted(relay, &authority, &key, &v)?;
    log::info!("journal : déposé ({} octets)", sealed.len());
    Ok(d)
}

/// Où en est un dépôt (l'écran du QR code le relit toutes les [`CHECK_EVERY`]).
/// Bloquant.
pub fn state(relay: &str, id: &str, owner: &str) -> Remote {
    if !valid_token(id) {
        return Remote::Gone;
    }
    let Ok(addr) = relay_of(relay) else { return Remote::Unknown };
    let Ok(agent) = agent(&addr) else { return Remote::Unknown };
    match agent.get(&format!("https://{}/v1/journal/{id}/state", addr.web_authority())).set("x-kn-owner", owner).call() {
        Ok(resp) => match resp.into_json::<serde_json::Value>().ok().and_then(|v| v["state"].as_str().map(str::to_string)).as_deref() {
            Some("opened") => Remote::Opened,
            Some("waiting") => Remote::Waiting,
            _ => Remote::Unknown,
        },
        Err(ureq::Error::Status(404, _)) => Remote::Gone,
        Err(_) => Remote::Unknown,
    }
}

/// « Terminé » : retire un dépôt pas encore ouvert (sans effet sinon).
/// `false` si le relais n'a pas répondu. Bloquant.
pub fn discard(relay: &str, id: &str, owner: &str) -> bool {
    if !valid_token(id) {
        return true;
    }
    let Ok(addr) = relay_of(relay) else { return false };
    let Ok(agent) = agent(&addr) else { return false };
    match agent.delete(&format!("https://{}/v1/journal/{id}", addr.web_authority())).set("x-kn-owner", owner).call() {
        Ok(_) => {
            log::info!("journal : dépôt retiré");
            true
        }
        Err(ureq::Error::Status(404, _)) => true,
        Err(e) => {
            log::warn!("journal : retrait impossible ({e})");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Vecteurs d'essai de docs/relais.md (« Vecteurs d'essai KNJ1 ») : le même
    /// dépôt que le relais et la page du téléphone, octet pour octet.
    #[test]
    fn vecteur_d_essai_knj1() {
        let key: [u8; 32] = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f").try_into().unwrap();
        let nonce: [u8; 12] = hex("a0a1a2a3a4a5a6a7a8a9aaab").try_into().unwrap();
        let input = hex(concat!(
            "1f8b08000000000002fff3ae4c4a2df2cb2f57f0c9cccb5630d03334d4b3e0722c28482c4acdcc51b052702d2e4ecc54f0f6f332e4f2ca2f2dca4bcc5138bc32b928b344212755c1c8c0c84cd7d050d7c85cc1d0c8cac00088f40c0c0c1442439cb99cf2f3b2801af4b800b1b88fb162000000"
        ));
        let expected = hex(concat!(
            "4b4e4a31a0a1a2a3a4a5a6a7a8a9aaabf993742d45cb02bf609a747d4b30ed2cbb830ee05b7b893aacde15b2ab189573fe5e0fd3e5ef9f6cefce74e527344fadb7edb57a8622d0516c944092f54839820d94362b1182b1213c223754581e9c6d05576a740d0b597a66813296bd8e0fd7133db759f01c0f816a6306e13d075c4a2450614699bf3b2059e75ad4459d934d121ce0"
        ));
        assert_eq!(expected.len(), 147);
        assert_eq!(seal_with(&input, &key, &nonce).unwrap(), expected);
        assert_eq!(URL_SAFE_NO_PAD.encode(key), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8");
        let text = open(&expected, &key).unwrap();
        assert_eq!(text.len(), 98);
        assert_eq!(text, "KyberNow Link 0.11.8\nAppareil : Essai KNJ1\nJournal écrit le 2026-11-27 12:00:00.000 UTC\nBonjour.\n");
        // L'en-tête du cœur donne les lignes du vecteur.
        assert_eq!(header(Lang::Fr, "0.11.8", "Essai KNJ1", &[], 1_795_780_800), "KyberNow Link 0.11.8\nAppareil : Essai KNJ1\nJournal écrit le 2026-11-27 12:00:00.000 UTC\n\n");
    }

    #[test]
    fn en_tete_dans_les_deux_langues() {
        let extra = vec!["Interface : Salon".to_string(), String::new(), "Fin précédente : normale\n".to_string()];
        assert_eq!(
            header(Lang::Fr, "0.11.8", "LG OLED55G26LA · webOS 23.20", &extra, 1_795_780_800),
            "KyberNow Link 0.11.8\nAppareil : LG OLED55G26LA · webOS 23.20\nInterface : Salon\nFin précédente : normale\nJournal écrit le 2026-11-27 12:00:00.000 UTC\n\n"
        );
        assert_eq!(header(Lang::En, "0.11.8", "Steam Deck", &[], 1_795_780_800), "KyberNow Link 0.11.8\nDevice: Steam Deck\nLog written on 2026-11-27 12:00:00.000 UTC\n\n");
    }

    #[test]
    fn depot_chiffre_et_relu() {
        let (deposit, key) = seal(Lang::Fr, "Apple TV · tvOS 26.1 · KyberNow Link 0.11.8\n", "ligne 1\nligne 2\n").unwrap();
        assert_eq!(&deposit[..4], b"KNJ1");
        assert_eq!(open(&deposit, &key).as_deref(), Some("Apple TV · tvOS 26.1 · KyberNow Link 0.11.8\nligne 1\nligne 2\n"));
        // Une autre clé n'ouvre rien ; un octet changé non plus.
        assert!(open(&deposit, &[7u8; 32]).is_none());
        let mut bad = deposit.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(open(&bad, &key).is_none());
        // Jamais deux fois la même clé ni le même nonce.
        let (again, key2) = seal(Lang::Fr, "x\n", "y\n").unwrap();
        assert_ne!(key, key2);
        assert_ne!(deposit[4..16], again[4..16]);
    }

    #[test]
    fn journal_trop_long_garde_l_en_tete_et_la_fin() {
        // Un journal qui se compresse mal (pseudo-aléatoire), bien plus grand que la limite.
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut body = String::new();
        for i in 0..60_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            body.push_str(&format!("{i} {x:016x}{:016x}\n", x.rotate_left(29)));
        }
        let limit = 256 * 1024;
        for (lang, cut) in [(Lang::Fr, "[… début du journal retiré : "), (Lang::En, "[… start of the log removed: ")] {
            let out = compress(lang, "EN-TÊTE\n", &body, limit);
            assert!(out.len() <= limit, "{} > {limit}", out.len());
            let mut s = String::new();
            flate2::read::GzDecoder::new(out.as_slice()).read_to_string(&mut s).unwrap();
            assert!(s.starts_with(&format!("EN-TÊTE\n{cut}")));
            assert!(s.ends_with(body.lines().last().map(|l| format!("{l}\n")).unwrap().as_str()));
            // La coupure tombe au début d'une ligne.
            let after = s.split_once("]\n").unwrap().1;
            assert!(body.contains(&format!("\n{}", after.lines().next().unwrap())));
        }
        // Petit : tout.
        let mut s = String::new();
        flate2::read::GzDecoder::new(compress(Lang::Fr, "H\n", "court\n", limit).as_slice()).read_to_string(&mut s).unwrap();
        assert_eq!(s, "H\ncourt\n");
    }

    #[test]
    fn adresse_reponses_et_refus() {
        let key = [7u8; 32];
        let url = page_url("share.kybernow.app", "AbC_d-9", &key);
        assert!(url.starts_with("https://share.kybernow.app/journal/AbC_d-9#k="));
        assert_eq!(url.split("#k=").nth(1).unwrap().len(), 43);
        assert!(valid_token("AbC_d-9") && !valid_token("a/b") && !valid_token("") && !valid_token(&"a".repeat(65)));
        // Le relais public par défaut, un relais à empreinte épinglée gardé tel quel.
        assert_eq!(relay_of(" ").unwrap().host, DEFAULT_RELAY);
        let pinned = "relais.exemple.org:9443#sha256:".to_string() + &"ab:".repeat(31) + "ab";
        let r = relay_of(&pinned).unwrap();
        assert_eq!(r.web_authority(), "relais.exemple.org:9443");
        let v = serde_json::json!({"id": "AAAAAAAAAAAAAAAAAAAAAA", "owner": "BBBBBBBBBBBBBBBBBBBBBB", "expires_in": 3600});
        let d = read_posted(&pinned, &r.web_authority(), &key, &v).unwrap();
        assert_eq!(d.relay, pinned, "le relais du suivi garde son empreinte");
        assert!(d.url.starts_with("https://relais.exemple.org:9443/journal/AAAAAAAAAAAAAAAAAAAAAA#k="));
        assert_eq!(d.expires_in, 3600);
        assert!(d.to_json().contains("\"owner\":\"BBBBBBBBBBBBBBBBBBBBBB\""));
        assert!(matches!(read_posted("", "x", &key, &serde_json::json!({"id": "a/b", "owner": "c"})), Err(DepositError::Unreachable(_))));
        assert_eq!(DepositError::of_status(404), DepositError::OldRelay);
        assert_eq!(DepositError::of_status(429), DepositError::TooMany);
        assert_eq!(DepositError::of_status(503), DepositError::TooMany);
        assert!(matches!(DepositError::of_status(500), DepositError::Unreachable(_)));
        assert_eq!((DepositError::Unreachable(String::new()).code(), DepositError::OldRelay.code(), DepositError::TooMany.code()), (90, 91, 92));
        assert_eq!((Remote::Waiting.code(), Remote::Opened.code(), Remote::Gone.code(), Remote::Unknown.code()), (0, 1, 93, 90));
    }
}
