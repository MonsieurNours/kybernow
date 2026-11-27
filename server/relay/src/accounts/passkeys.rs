//! Les passkeys (WebAuthn, docs/comptes.md, « Lot 1 », partie 1b) : ajouter une
//! passkey depuis `/compte` ou `/compte/proteger`, se connecter avec
//! (`/compte/connexion`), la renommer, la retirer ; elle confirme aussi la
//! suppression du compte.
//!
//! La vérification (CBOR, clés COSE, signatures, sur `ring`) est dans le crate
//! du tunnel, `kybernow_tunnel::webauthn`, partagée avec Admin (clé d'Admin à
//! distance). Le relais ne demande aucune attestation (`attestation: "none"`) :
//! seuls comptent l'origine, le défi, les drapeaux et la signature.
//! Algorithmes : ES256 (-7), Ed25519 (-8) et RS256 (-257, les anciennes puces
//! TPM de Windows Hello).
//!
//! Une passkey vaut double authentification : la vérification de l'utilisateur
//! (empreinte, visage, code de l'appareil) est exigée (`userVerification:
//! "required"`, drapeau UV).
//!
//! Les défis ne sont pas gardés en base : un défi porte son échéance et une
//! signature (HMAC de la clé du relais) qui le lie au navigateur (son cookie
//! CSRF) et à l'usage (connexion, ou ajout à tel compte). Une fois la
//! signature WebAuthn vérifiée, son empreinte est notée (`webauthn_used`) : il
//! ne sert qu'une fois. Une page affichée n'écrit donc rien en base.
//!
//! Le navigateur passe par `assets/passkey.js`, le seul script des pages : il
//! appelle `navigator.credentials` et envoie la réponse par le formulaire de
//! la page. Sans lui, les formulaires des passkeys restent cachés et tout le
//! reste marche avec le mot de passe.
//!
//! Ce module est un enfant de [`super`] (`web.rs`) : il se sert de son contexte.

use ring::digest;

use super::super::store::{NewPasskey, Passkey};
use super::super::{Keys, b64, random, unb64};
use super::{Cx, Reply, account_page, device_name, lang_of, to_sign_in};
use crate::accounts::mail::Message;
use crate::accounts::pages::{self, Notice, Registration};

/// Un défi vaut dix minutes (la page peut rester ouverte un moment).
pub const CHALLENGE_SECS: i64 = 10 * 60;
/// Le nom d'une passkey : 60 caractères au plus.
pub const NAME_MAX: usize = 60;
/// Un compte garde 20 passkeys au plus.
pub const MAX_PER_ACCOUNT: usize = 20;

// --- Défis ---------------------------------------------------------------------

/// Un défi : 16 octets aléatoires, l'échéance, 16 octets de signature, en base64url.
pub fn challenge(keys: &Keys, purpose: &str, browser: &str, now: i64) -> String {
    let nonce = random::<16>();
    let exp = now + CHALLENGE_SECS;
    let mut raw = nonce.to_vec();
    raw.extend_from_slice(&exp.to_be_bytes());
    raw.extend_from_slice(&challenge_mac(keys, purpose, browser, &nonce, exp));
    b64(&raw)
}

fn challenge_mac(keys: &Keys, purpose: &str, browser: &str, nonce: &[u8], exp: i64) -> Vec<u8> {
    let data = format!("webauthn:{purpose}:{browser}:{}:{exp}", b64(nonce));
    let mut mac = unb64(&keys.sign(data.as_bytes())).unwrap_or_default();
    mac.truncate(16);
    mac
}

/// Le défi est-il de ce relais, pour cet usage et ce navigateur, et encore valable ? Rend son échéance.
pub fn check_challenge(keys: &Keys, purpose: &str, browser: &str, challenge: &str, now: i64) -> Option<i64> {
    let raw = unb64(challenge)?;
    if raw.len() != 40 {
        return None;
    }
    let (nonce, rest) = raw.split_at(16);
    let exp = i64::from_be_bytes(rest[..8].try_into().ok()?);
    let ok = super::super::ct_eq(&challenge_mac(keys, purpose, browser, nonce, exp), &rest[8..]) && exp > now;
    ok.then_some(exp)
}

// --- Vérification (crate du tunnel, partagée avec Admin) -------------------------

pub use kybernow_tunnel::webauthn::{Assertion, Refused, Rp, client_data};

/// Vérifie la réponse de `navigator.credentials.create()` (le défi est déjà vérifié par l'appelant).
pub fn verify_registration(rp: &Rp, attestation_object: &[u8]) -> Result<NewPasskey, Refused> {
    let r = kybernow_tunnel::webauthn::verify_registration(rp, attestation_object)?;
    Ok(NewPasskey { credential: r.credential, public_key: r.public_key, alg: r.alg, counter: r.counter })
}

/// Vérifie une assertion pour une passkey du compte ; rend le nouveau compteur.
pub fn verify_assertion(rp: &Rp, key: &Passkey, webauthn_user: &[u8], a: &Assertion) -> Result<i64, Refused> {
    let stored = kybernow_tunnel::webauthn::StoredKey { alg: key.alg, public_key: &key.public_key, counter: key.counter };
    kybernow_tunnel::webauthn::verify_assertion(rp, &stored, webauthn_user, a)
}

// --- Pages ---------------------------------------------------------------------

/// L'usage du défi d'un ajout : lié au compte.
fn create_purpose(account: i64) -> String {
    format!("create:{account}")
}

/// L'usage du défi d'une connexion ou d'une confirmation.
pub const GET_PURPOSE: &str = "get";

impl Cx<'_> {
    fn rp(&self) -> Rp {
        Rp::of(&self.acc.public_url)
    }

    /// Les options d'ajout d'une passkey à ce compte ; `None` si le compte en a déjà le maximum.
    pub(super) fn registration(&self, account: &super::Account) -> Option<Registration> {
        // Avant de prendre la base : `recent_auth` la lit aussi.
        let password = !self.recent_auth();
        let db = self.acc.db();
        let keys = db.passkeys(account.id).ok()?;
        if keys.len() >= MAX_PER_ACCOUNT {
            return None;
        }
        let user = db.webauthn_user(account.id).ok()?;
        Some(Registration {
            challenge: challenge(&self.acc.keys, &create_purpose(account.id), &self.csrf_cookie, self.now),
            user_id: b64(&user),
            user_name: account.email.clone(),
            display_name: account.display_name(),
            exclude: keys.iter().map(|k| b64(&k.credential)).collect(),
            password,
        })
    }

    /// Le défi de `clientDataJSON`, s'il est bon pour cet usage ; rend son empreinte et son échéance.
    fn take_challenge(&self, purpose: &str, challenge: &str) -> Option<(Vec<u8>, i64)> {
        let exp = check_challenge(&self.acc.keys, purpose, &self.csrf_cookie, challenge, self.now)?;
        Some((digest::digest(&digest::SHA256, challenge.as_bytes()).as_ref().to_vec(), exp))
    }

    /// L'assertion envoyée par le formulaire.
    fn assertion(&self) -> Option<Assertion> {
        let f = |k: &str| unb64(self.req.form(k));
        Some(Assertion {
            credential: f("credential").filter(|c| !c.is_empty())?,
            client_data: f("client_data")?,
            authenticator: f("authenticator")?,
            signature: f("signature")?,
            user: f("user").unwrap_or_default(),
        })
    }

    /// Vérifie une passkey envoyée par le formulaire (connexion ou confirmation) ;
    /// rend la passkey. `account` : elle doit être de ce compte.
    pub(super) fn check_passkey(&self, account: Option<i64>) -> Result<Passkey, Refused> {
        let a = self.assertion().ok_or(Refused::Malformed)?;
        let rp = self.rp();
        let challenge = client_data(&rp, &a.client_data, "webauthn.get")?;
        let (hash, exp) = self.take_challenge(GET_PURPOSE, &challenge).ok_or(Refused::Challenge)?;
        let db = self.acc.db();
        let key = db.passkey_by_credential(&a.credential).ok().flatten().ok_or(Refused::Signature)?;
        if account.is_some_and(|id| id != key.account) {
            return Err(Refused::Signature);
        }
        let user = db.webauthn_user(key.account).map_err(|_| Refused::Malformed)?;
        let counter = verify_assertion(&rp, &key, &user, &a)?;
        if !db.use_challenge(&hash, exp).unwrap_or(false) {
            return Err(Refused::Challenge);
        }
        let _ = db.use_passkey(key.id, counter, self.now);
        Ok(key)
    }
}

/// `POST /compte/connexion/passkey` : « Continuer avec une passkey ».
pub(super) fn sign_in(mut cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let mail_on = acc.mailer.is_some();
    // Les échecs comptent par adresse IP (le compte n'est connu qu'après la signature) :
    // une clé par adresse, pour qu'un tiers ne bloque pas les passkeys de tout le monde.
    let key = format!("passkey:{}", super::super::ip_key(cx.req.ip));
    let limit = acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in(cx.req.ip, &key, false);
    if let Err(wait) = limit {
        return cx.limited(wait, |cx, n| pages::sign_in(&cx.view(), "", Some(n), mail_on, mail_on));
    }
    let passkey = match cx.check_passkey(None) {
        Ok(passkey) => passkey,
        Err(why) => {
            tracing::debug!(raison = ?why, "passkey refusée à la connexion");
            let page = pages::sign_in(&cx.view(), "", Some(Notice::PasskeyFailed), mail_on, mail_on);
            return cx.page(200, page);
        }
    };
    acc.limits.lock().unwrap_or_else(|e| e.into_inner()).sign_in_ok(cx.req.ip, &key);
    let account = acc.db().account(passkey.account).ok().flatten();
    let Some(account) = account.filter(|a| a.verified()) else {
        let page = pages::sign_in(&cx.view(), "", Some(Notice::PasskeyFailed), mail_on, mail_on);
        return cx.page(200, page);
    };
    // Une passkey vaut double authentification : la session est entière.
    cx.open_session(account.id, true);
    cx.audit(Some(account.id), "connexion_passkey");
    // Lot 3 : comme après le mot de passe, avec `?appareil=` ou une suite retenue
    // (rattachement, consentement d'un appareil), on y va.
    super::device_pages::after_sign_in(cx)
}

/// `POST /compte/passkeys/ajouter` : la réponse de `navigator.credentials.create()`.
pub(super) fn add(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    // Une session ancienne (cookie volé ?) redonne le mot de passe avant d'ajouter une passkey (REL-06).
    if let Err(notice) = cx.reauth(&account) {
        return account_page(cx, Some(notice));
    }
    let rp = cx.rp();
    let result = (|| {
        let client = unb64(cx.req.form("client_data")).ok_or(Refused::Malformed)?;
        let object = unb64(cx.req.form("attestation")).ok_or(Refused::Malformed)?;
        let challenge = client_data(&rp, &client, "webauthn.create")?;
        let (hash, exp) = cx.take_challenge(&create_purpose(account.id), &challenge).ok_or(Refused::Challenge)?;
        let key = verify_registration(&rp, &object)?;
        if !acc.db().use_challenge(&hash, exp).unwrap_or(false) {
            return Err(Refused::Challenge);
        }
        Ok(key)
    })();
    let key = match result {
        Ok(key) => key,
        Err(why) => {
            tracing::debug!(raison = ?why, "passkey refusée à l'ajout");
            return account_page(cx, Some(Notice::PasskeyFailed));
        }
    };
    let transports: String = cx.req.form("transports").split(',').filter(|t| !t.is_empty() && t.len() <= 16 && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')).take(8).collect::<Vec<_>>().join(",");
    let name = default_name(&cx);
    let added = {
        let db = acc.db();
        match db.passkeys(account.id) {
            Ok(list) if list.len() < MAX_PER_ACCOUNT => db.add_passkey(account.id, &key, &name, &transports, cx.now),
            _ => Ok(None),
        }
    };
    match added {
        Ok(Some(_)) => {
            cx.audit(Some(account.id), "passkey_ajoutee");
            let link = cx.url(&format!("/compte?lang={}", lang_of(&account).code()));
            cx.mail(&account.email, lang_of(&account), Message::PasskeyAdded { name: &name, account: &link });
            // Depuis le tour d'accueil : l'étape suivante.
            if cx.req.form("suite") == "bienvenue" {
                let q = cx.q();
                return cx.redirect(format!("/compte/bienvenue{q}&etape=2"));
            }
            account_page(cx, Some(Notice::PasskeyAdded))
        }
        Ok(None) => account_page(cx, Some(Notice::PasskeyFailed)),
        Err(e) => {
            tracing::warn!("passkey non gardée : {e:#}");
            cx.failed()
        }
    }
}

/// Le nom donné d'office : le navigateur et le système (« Chrome, Mac »).
fn default_name(cx: &Cx<'_>) -> String {
    let device = device_name(&cx.req.user_agent);
    if device.is_empty() { "Passkey".to_string() } else { device }
}

/// Un nom tapé : sans caractères de contrôle, 60 caractères au plus.
pub fn clean_name(input: &str) -> String {
    crate::accounts::stations::clean_text(input, NAME_MAX)
}

/// `POST /compte/passkeys/renommer`.
pub(super) fn rename(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    let name = clean_name(cx.req.form("name"));
    let Ok(id) = cx.req.form("id").parse::<i64>() else { return account_page(cx, None) };
    if name.is_empty() {
        return account_page(cx, Some(Notice::NameMissing));
    }
    if acc.db().rename_passkey(id, account.id, &name).unwrap_or(false) {
        cx.audit(Some(account.id), "passkey_renommee");
        return account_page(cx, Some(Notice::PasskeyRenamed));
    }
    account_page(cx, None)
}

/// `POST /compte/passkeys/retirer`.
pub(super) fn remove(cx: Cx<'_>) -> Reply {
    let acc = cx.acc;
    let Some((_, account)) = cx.full_session() else { return to_sign_in(cx) };
    let Ok(id) = cx.req.form("id").parse::<i64>() else { return account_page(cx, None) };
    if acc.db().delete_passkey(id, account.id).unwrap_or(false) {
        cx.audit(Some(account.id), "passkey_retiree");
        return account_page(cx, Some(Notice::PasskeyRemoved));
    }
    account_page(cx, None)
}

/// Les passkeys d'un compte, pour la page du compte.
pub(super) fn views(cx: &Cx<'_>, account: i64) -> Vec<pages::PasskeyView> {
    cx.acc
        .db()
        .passkeys(account)
        .unwrap_or_default()
        .into_iter()
        .map(|p| pages::PasskeyView { id: p.id, name: p.name, created_at: p.created_at, seen_at: p.seen_at })
        .collect()
}

#[cfg(test)]
pub mod tests {
    //! L'authentificateur logiciel des essais vient du crate du tunnel (`webauthn::testing`).

    use super::*;
    pub use kybernow_tunnel::webauthn::testing::Authenticator;
    use kybernow_tunnel::webauthn::ES256;

    fn stored(auth: &Authenticator, key: &NewPasskey) -> Passkey {
        Passkey {
            id: 1,
            account: 1,
            credential: auth.credential.clone(),
            public_key: key.public_key.clone(),
            alg: key.alg,
            counter: key.counter,
            name: "x".into(),
            transports: String::new(),
            created_at: 0,
            seen_at: None,
        }
    }

    #[test]
    fn defis_lies_au_navigateur_a_l_usage_et_au_temps() {
        let keys = Keys::new(&[3u8; 32]);
        let c = challenge(&keys, "get", "navigateur", 100);
        assert_eq!(unb64(&c).unwrap().len(), 40);
        assert_eq!(check_challenge(&keys, "get", "navigateur", &c, 100), Some(100 + CHALLENGE_SECS));
        assert_eq!(check_challenge(&keys, "get", "autre", &c, 100), None, "autre navigateur");
        assert_eq!(check_challenge(&keys, "create:1", "navigateur", &c, 100), None, "autre usage");
        assert_eq!(check_challenge(&keys, "get", "navigateur", &c, 100 + CHALLENGE_SECS), None, "expiré");
        assert_eq!(check_challenge(&Keys::new(&[4u8; 32]), "get", "navigateur", &c, 100), None, "autre relais");
        let mut raw = unb64(&c).unwrap();
        raw[20] ^= 1;
        assert_eq!(check_challenge(&keys, "get", "navigateur", &b64(&raw), 100), None, "échéance changée");
        assert_ne!(challenge(&keys, "get", "navigateur", 100), c);
    }

    #[test]
    fn passkey_du_compte_ajoutee_puis_verifiee() {
        // Les enveloppes du relais autour de la vérification du tunnel : `NewPasskey`, `Passkey`, compteur.
        let rp = Rp::of("https://share.test");
        let mut auth = Authenticator::new(ES256, "https://share.test");
        let key = verify_registration(&rp, &auth.create("d").1).unwrap();
        assert_eq!((key.alg, key.credential.clone(), key.counter), (ES256, auth.credential.clone(), 0));
        let mut stored = stored(&auth, &key);
        let a = auth.get("d2");
        assert_eq!(verify_assertion(&rp, &stored, b"", &a), Ok(1));
        stored.counter = 1;
        assert_eq!(verify_assertion(&rp, &stored, b"", &a), Err(Refused::Counter));
    }

    #[test]
    fn noms() {
        assert_eq!(clean_name("  Clé\u{7} USB  "), "Clé USB");
        assert_eq!(clean_name(&"x".repeat(100)).len(), NAME_MAX);
    }
}
