//! « Envoyer le journal » par QR code (Apple TV ; docs/link-ui.md, « Réglages
//! de l'app », et docs/relais.md, « Dépôt d'un journal ») : le journal est
//! compressé (gzip) puis chiffré sur l'appareil (AES-256-GCM, clé tirée au
//! hasard pour ce dépôt), déposé sur le relais (`POST /v1/journal`), qui ne
//! peut pas le lire ; la clé ne part que dans le QR code, après `#k=`, que le
//! navigateur du téléphone n'envoie jamais. L'écran du QR code suit le dépôt
//! (`GET /v1/journal/<id>/state`) et le retire à « Terminé »
//! (`DELETE /v1/journal/<id>`).
//!
//! Dépôt : `KNJ1` (4 octets), le nonce (12), puis le texte chiffré et son
//! étiquette (16). 2 Mio au plus une fois compressé : au-delà, l'en-tête
//! (appareil, version, fins précédentes) est gardé avec la fin du journal,
//! la plus récente.

use std::ffi::c_char;
#[cfg(test)]
use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kybernow_core::relay::{DEFAULT_RELAY, RelayAddr};

use crate::{KN_ERROR, KN_OK, give_to, text};

/// Le relais n'a pas répondu (réseau, nom, certificat).
pub const KN_JOURNAL_UNREACHABLE: i32 = 90;
/// Relais d'avant le dépôt des journaux (`404` sur `POST /v1/journal`).
pub const KN_JOURNAL_OLD_RELAY: i32 = 91;
/// Trop de dépôts depuis cette adresse (`429`), ou relais plein (`503`).
pub const KN_JOURNAL_TOO_MANY: i32 = 92;
/// Le dépôt n'est plus là : ouvert puis effacé, expiré (une heure), retiré.
pub const KN_JOURNAL_GONE: i32 = 93;
/// États d'un dépôt (`kn_journal_state`).
pub const KN_JOURNAL_WAITING: i32 = 0;
pub const KN_JOURNAL_OPENED: i32 = 1;

/// Le plus grand journal compressé qu'un dépôt porte (docs/relais.md).
pub const MAX_COMPRESSED: usize = 2 * 1024 * 1024;
const MAGIC: &[u8; 4] = b"KNJ1";
const CONNECT: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(30);

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    // Écrire dans un Vec ne peut pas échouer.
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

/// Le journal compressé, tenu dans `limit` : entier s'il tient, sinon
/// l'en-tête, une ligne qui dit la coupure, puis la fin du journal (coupée
/// au début d'une ligne).
fn compress(header: &str, body: &str, limit: usize) -> Vec<u8> {
    let whole = gzip(format!("{header}{body}").as_bytes());
    if whole.len() <= limit {
        return whole;
    }
    let mut keep = body.len();
    let mut compressed = whole.len();
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
        let out = gzip(format!("{header}[… début du journal retiré : {start} octets]\n{tail}").as_bytes());
        if out.len() <= limit || tail.is_empty() {
            return out;
        }
        compressed = out.len();
        keep = tail.len();
    }
}

/// Le dépôt chiffré et sa clé.
fn seal(header: &str, body: &str) -> Result<(Vec<u8>, [u8; 32]), String> {
    let plain = compress(header, body, MAX_COMPRESSED);
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    Ok((encrypt(&plain, &key, &nonce)?, key))
}

/// `KNJ1`, le nonce, puis le texte chiffré et son étiquette (AES-256-GCM,
/// sans données associées : docs/relais.md, « Dépôt d'un journal »).
fn encrypt(plain: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let sealed = cipher.encrypt(Nonce::from_slice(nonce), plain).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(4 + 12 + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Ce que la page du téléphone fait (essais) : déchiffre et décompresse.
#[cfg(test)]
fn open(deposit: &[u8], key: &[u8; 32]) -> Option<String> {
    if deposit.len() < 4 + 12 + 16 || &deposit[..4] != MAGIC {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let plain = cipher.decrypt(Nonce::from_slice(&deposit[4..16]), &deposit[16..]).ok()?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(plain.as_slice()).read_to_string(&mut s).ok()?;
    Some(s)
}

/// Le relais du compte de l'appareil, sinon le relais public.
fn relay(relay: *const c_char) -> Result<RelayAddr, String> {
    let r = text(relay);
    let r = if r.trim().is_empty() { DEFAULT_RELAY.to_owned() } else { r };
    r.parse::<RelayAddr>().map_err(|e| e.to_string())
}

fn agent(relay: &RelayAddr) -> Result<ureq::Agent, String> {
    // Même vérification du certificat que le tunnel et les comptes :
    // autorités publiques, ou empreinte épinglée d'un relais chez soi.
    let tls = kybernow_tunnel::tls::client_config(relay, None, b"http/1.1", None).map_err(|e| e.to_string())?;
    Ok(ureq::AgentBuilder::new().tls_config(Arc::new(tls)).timeout_connect(CONNECT).timeout(TIMEOUT).build())
}

/// Le code d'un refus du relais.
fn refused(status: u16) -> i32 {
    match status {
        404 => KN_JOURNAL_OLD_RELAY,
        // 413 n'arrive pas (le journal est tenu dans la limite) ; 503 : relais
        // plein, les dépôts partent au bout d'une heure.
        413 | 429 | 503 => KN_JOURNAL_TOO_MANY,
        _ => KN_ERROR,
    }
}

/// Dépose le journal sur le relais (`relay` : celui du compte KyberNow de
/// l'appareil ; vide : le relais public). `header` : appareil, système,
/// version, fins précédentes ; `body` : le journal de l'app, du plus ancien
/// au plus récent. `out` : `{id, owner, url, expires_in}` (`url` : l'adresse
/// du QR code, clé comprise ; `owner` : le jeton pour suivre et retirer le
/// dépôt). Refus : `KN_JOURNAL_*`, `KN_ERROR` (le détail dans `error`).
/// Bloquant (30 s au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_upload(
    relay_addr: *const c_char,
    header: *const c_char,
    body: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(KN_ERROR => {
        let addr = match relay(relay_addr) {
            Ok(a) => a,
            Err(e) => {
                give_to(error, e);
                return KN_ERROR;
            }
        };
        let (deposit, key) = match seal(&text(header), &text(body)) {
            Ok(d) => d,
            Err(e) => {
                give_to(error, e);
                return KN_ERROR;
            }
        };
        let agent = match agent(&addr) {
            Ok(a) => a,
            Err(e) => {
                give_to(error, e);
                return KN_JOURNAL_UNREACHABLE;
            }
        };
        let url = format!("https://{}/v1/journal", addr.web_authority());
        let r = agent.post(&url).set("Content-Type", "application/octet-stream").send_bytes(&deposit);
        let resp = match r {
            Ok(resp) => resp,
            Err(ureq::Error::Status(status, _)) => {
                log::warn!("journal : dépôt refusé par le relais ({status})");
                give_to(error, format!("HTTP {status}"));
                return refused(status);
            }
            Err(e) => {
                log::warn!("journal : relais {} injoignable ({e})", addr.web_authority());
                give_to(error, e.to_string());
                return KN_JOURNAL_UNREACHABLE;
            }
        };
        let v: serde_json::Value = resp.into_json().unwrap_or_default();
        let (Some(id), Some(owner)) = (v["id"].as_str(), v["owner"].as_str()) else {
            give_to(error, "réponse du relais illisible".into());
            return KN_ERROR;
        };
        // Un identifiant est en base64url : rien à échapper dans l'adresse.
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            give_to(error, "identifiant du dépôt illisible".into());
            return KN_ERROR;
        }
        let page = format!("https://{}/journal/{id}#k={}", addr.web_authority(), URL_SAFE_NO_PAD.encode(key));
        log::info!("journal : déposé ({} octets)", deposit.len());
        give_to(
            out,
            serde_json::json!({ "id": id, "owner": owner, "url": page, "expires_in": v["expires_in"].as_u64().unwrap_or(3600) }).to_string(),
        );
        KN_OK
    })
}

/// Où en est un dépôt (l'écran du QR code le relit toutes les 2 s) :
/// `KN_JOURNAL_WAITING`, `KN_JOURNAL_OPENED` (le téléphone l'a ouvert),
/// `KN_JOURNAL_GONE` (expiré), `KN_JOURNAL_UNREACHABLE`. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_state(relay_addr: *const c_char, id: *const c_char, owner: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(KN_JOURNAL_UNREACHABLE => {
        let Ok(addr) = relay(relay_addr) else { return KN_ERROR };
        let Ok(agent) = agent(&addr) else { return KN_JOURNAL_UNREACHABLE };
        let url = format!("https://{}/v1/journal/{}/state", addr.web_authority(), text(id));
        match agent.get(&url).set("x-kn-owner", &text(owner)).call() {
            Ok(resp) => {
                let v: serde_json::Value = resp.into_json().unwrap_or_default();
                if v["state"] == "opened" { KN_JOURNAL_OPENED } else { KN_JOURNAL_WAITING }
            }
            Err(ureq::Error::Status(404, _)) => KN_JOURNAL_GONE,
            Err(ureq::Error::Status(_, _)) => KN_ERROR,
            Err(_) => KN_JOURNAL_UNREACHABLE,
        }
    })
}

/// « Terminé » : retire un dépôt pas encore ouvert (sans effet sinon).
/// Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_delete(relay_addr: *const c_char, id: *const c_char, owner: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(KN_JOURNAL_UNREACHABLE => {
        let Ok(addr) = relay(relay_addr) else { return KN_ERROR };
        let Ok(agent) = agent(&addr) else { return KN_JOURNAL_UNREACHABLE };
        let url = format!("https://{}/v1/journal/{}", addr.web_authority(), text(id));
        match agent.delete(&url).set("x-kn-owner", &text(owner)).call() {
            Ok(_) => KN_OK,
            Err(ureq::Error::Status(404, _)) => KN_JOURNAL_GONE,
            Err(ureq::Error::Status(_, _)) => KN_ERROR,
            Err(_) => KN_JOURNAL_UNREACHABLE,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Vecteurs d'essai de docs/relais.md (« Vecteurs d'essai KNJ1 ») : le
    /// même dépôt que le relais et la page du téléphone, octet pour octet.
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
        assert_eq!(encrypt(&input, &key, &nonce).unwrap(), expected);
        assert_eq!(URL_SAFE_NO_PAD.encode(key), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8");
        let text = open(&expected, &key).unwrap();
        assert_eq!(text, "KyberNow Link 0.11.8\nAppareil : Essai KNJ1\nJournal écrit le 2026-11-27 12:00:00.000 UTC\nBonjour.\n");
    }

    #[test]
    fn depot_chiffre_et_relu() {
        let (deposit, key) = seal("Apple TV · tvOS 26.1 · KyberNow Link 0.11.8\n", "ligne 1\nligne 2\n").unwrap();
        assert_eq!(&deposit[..4], b"KNJ1");
        assert_eq!(open(&deposit, &key).as_deref(), Some("Apple TV · tvOS 26.1 · KyberNow Link 0.11.8\nligne 1\nligne 2\n"));
        // Une autre clé n'ouvre rien ; un octet changé non plus.
        assert!(open(&deposit, &[7u8; 32]).is_none());
        let mut bad = deposit.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(open(&bad, &key).is_none());
        // La clé tient dans 43 signes base64url.
        assert_eq!(URL_SAFE_NO_PAD.encode(key).len(), 43);
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
        let out = compress("EN-TÊTE\n", &body, limit);
        assert!(out.len() <= limit, "{} > {limit}", out.len());
        let mut s = String::new();
        flate2::read::GzDecoder::new(out.as_slice()).read_to_string(&mut s).unwrap();
        assert!(s.starts_with("EN-TÊTE\n[… début du journal retiré : "));
        assert!(s.ends_with(body.lines().last().map(|l| format!("{l}\n")).unwrap().as_str()));
        // La coupure tombe au début d'une ligne.
        let after = s.split_once("]\n").unwrap().1;
        assert!(body.contains(&format!("\n{}", after.lines().next().unwrap())));
    }

    #[test]
    fn relais_par_defaut_et_refus() {
        assert_eq!(relay(std::ptr::null()).unwrap().host, DEFAULT_RELAY.split(':').next().unwrap());
        assert_eq!(refused(404), KN_JOURNAL_OLD_RELAY);
        assert_eq!(refused(429), KN_JOURNAL_TOO_MANY);
        assert_eq!(refused(503), KN_JOURNAL_TOO_MANY);
        assert_eq!(refused(500), KN_ERROR);
    }
}
