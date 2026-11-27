//! Un client ACME (RFC 8555) réduit à ce dont la Station a besoin : un
//! certificat pour un seul nom, `<id>.<tls_zone>`, prouvé par le défi
//! TLS-ALPN-01 (RFC 8737) (docs/relais.md, « Navigateur chiffré de bout en
//! bout »). Le relais fait passer la connexion de l'autorité jusqu'à la
//! Station ; Admin y répond avec le certificat du défi ([`challenge_cert`]).
//!
//! Bloquant (ureq) : à lancer dans `spawn_blocking`. La clé du compte ACME
//! (ECDSA P-256) et celle du certificat restent sur la Station.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use rustls::sign::CertifiedKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// L'annuaire de Let's Encrypt (production).
pub const LETS_ENCRYPT: &str = "https://acme-v02.api.letsencrypt.org/directory";
/// L'ALPN du défi TLS-ALPN-01.
pub const ACME_TLS_ALPN: &[u8] = b"acme-tls/1";
/// Attente d'une autorisation ou d'une commande, au plus.
const POLL_FOR: Duration = Duration::from_secs(90);
/// Taille d'une réponse de l'autorité, au plus (une chaîne de certificats tient en 16 Kio).
const MAX_REPLY: u64 = 256 * 1024;

/// Un refus ou un échec de l'autorité, avec le délai qu'elle demande (`Retry-After`, limites).
#[derive(Debug)]
pub struct AcmeError {
    pub message: String,
    pub retry_after: Option<Duration>,
}

impl std::fmt::Display for AcmeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AcmeError {}

// --- La clé du compte -------------------------------------------------------------

/// La clé du compte ACME : ECDSA P-256, signatures ES256 des requêtes (JWS).
pub struct AccountKey {
    pair: EcdsaKeyPair,
    pkcs8: Vec<u8>,
}

impl AccountKey {
    pub fn generate() -> Result<AccountKey> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).map_err(|_| anyhow!("clé du compte ACME"))?;
        AccountKey::from_pkcs8(pkcs8.as_ref())
    }

    pub fn from_pkcs8(der: &[u8]) -> Result<AccountKey> {
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der, &SystemRandom::new()).map_err(|_| anyhow!("clé du compte ACME illisible"))?;
        Ok(AccountKey { pair, pkcs8: der.to_vec() })
    }

    pub fn pkcs8(&self) -> &[u8] {
        &self.pkcs8
    }

    /// La clé publique en JWK (RFC 7517), membres dans l'ordre de la RFC 7638.
    pub fn jwk(&self) -> Value {
        let point = self.pair.public_key().as_ref();
        json!({ "crv": "P-256", "kty": "EC", "x": B64.encode(&point[1..33]), "y": B64.encode(&point[33..65]) })
    }

    /// L'empreinte JWK (RFC 7638) : SHA-256 du JSON canonique, en base64url.
    pub fn thumbprint(&self) -> String {
        let jwk = self.jwk();
        let canonical = format!(r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#, jwk["x"].as_str().unwrap_or_default(), jwk["y"].as_str().unwrap_or_default());
        B64.encode(Sha256::digest(canonical.as_bytes()))
    }

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(self.pair.sign(&SystemRandom::new(), data).map_err(|_| anyhow!("signature ES256"))?.as_ref().to_vec())
    }
}

/// L'autorisation de clé d'un défi : `<jeton>.<empreinte JWK>`.
pub fn key_authorization(token: &str, key: &AccountKey) -> String {
    format!("{token}.{}", key.thumbprint())
}

/// Le certificat du défi TLS-ALPN-01 pour `name` : signé par lui-même, ce nom
/// seul, et l'extension `acmeIdentifier` (critique) qui porte le SHA-256 de
/// l'autorisation de clé.
pub fn challenge_cert(name: &str, key_auth: &str) -> Result<Arc<CertifiedKey>> {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
    let mut params = rcgen::CertificateParams::new(vec![name.to_string()])?;
    params.custom_extensions = vec![rcgen::CustomExtension::new_acme_identifier(&Sha256::digest(key_auth.as_bytes()))];
    let cert = params.self_signed(&key)?;
    certified(vec![cert.der().clone()], &key.serialize_der())
}

/// Une clé et sa chaîne, pour rustls.
pub fn certified(chain: Vec<rustls::pki_types::CertificateDer<'static>>, pkcs8: &[u8]) -> Result<Arc<CertifiedKey>> {
    let der = rustls::pki_types::PrivateKeyDer::Pkcs8(pkcs8.to_vec().into());
    let signer = rustls::crypto::ring::sign::any_supported_type(&der).context("clé du certificat")?;
    Ok(Arc::new(CertifiedKey::new(chain, signer)))
}

// --- Le client --------------------------------------------------------------------

struct Directory {
    new_nonce: String,
    new_account: String,
    new_order: String,
}

struct Reply {
    status: u16,
    location: Option<String>,
    retry_after: Option<Duration>,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

/// Une session avec l'autorité : son annuaire, la clé du compte, son adresse (`kid`) une fois créé.
pub struct Client {
    agent: ureq::Agent,
    dir: Directory,
    key: AccountKey,
    kid: Option<String>,
    nonce: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .user_agent(format!("kyber-admin/{} (acme)", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn retry_after(headers: &ureq::http::HeaderMap) -> Option<Duration> {
    let v = headers.get("retry-after")?.to_str().ok()?.trim();
    // En secondes ; une date HTTP n'est pas lue (on garde alors nos propres délais).
    v.parse::<u64>().ok().map(|s| Duration::from_secs(s.min(7 * 86_400)))
}

impl Client {
    /// Lit l'annuaire. `kid` : le compte déjà créé (gardé par Admin), sinon [`Client::account`] le crée.
    pub fn new(directory: &str, key: AccountKey, kid: Option<String>) -> Result<Client> {
        let agent = agent();
        let mut resp = agent.get(directory).call().with_context(|| format!("annuaire ACME {directory}"))?;
        if !resp.status().is_success() {
            bail!("annuaire ACME {directory} : HTTP {}", resp.status().as_u16());
        }
        let v: Value = serde_json::from_slice(&resp.body_mut().with_config().limit(MAX_REPLY).read_to_vec()?).context("annuaire ACME illisible")?;
        let url = |k: &str| v[k].as_str().map(str::to_string).ok_or_else(|| anyhow!("annuaire ACME : {k} manque"));
        let dir = Directory { new_nonce: url("newNonce")?, new_account: url("newAccount")?, new_order: url("newOrder")? };
        Ok(Client { agent, dir, key, kid, nonce: None })
    }

    fn fresh_nonce(&mut self) -> Result<String> {
        if let Some(n) = self.nonce.take() {
            return Ok(n);
        }
        let resp = self.agent.head(&self.dir.new_nonce).call().context("nonce ACME")?;
        resp.headers().get("replay-nonce").and_then(|v| v.to_str().ok()).map(str::to_string).ok_or_else(|| anyhow!("nonce ACME absent"))
    }

    /// Une requête signée (JWS) ; `payload` absent : POST-as-GET. Un `badNonce` est retenté une fois.
    fn post(&mut self, url: &str, payload: Option<&Value>, accept: Option<&str>) -> Result<Reply> {
        for attempt in 0..2 {
            let nonce = self.fresh_nonce()?;
            let mut protected = json!({ "alg": "ES256", "nonce": nonce, "url": url });
            match &self.kid {
                Some(kid) => protected["kid"] = json!(kid),
                None => protected["jwk"] = self.key.jwk(),
            }
            let protected = B64.encode(protected.to_string());
            let payload = payload.map(|p| B64.encode(p.to_string())).unwrap_or_default();
            let signature = B64.encode(self.key.sign(format!("{protected}.{payload}").as_bytes())?);
            let body = json!({ "protected": protected, "payload": payload, "signature": signature }).to_string();
            let mut req = self.agent.post(url).header("content-type", "application/jose+json");
            if let Some(a) = accept {
                req = req.header("accept", a);
            }
            let mut resp = req.send(body.as_bytes()).with_context(|| format!("ACME {url}"))?;
            if let Some(n) = resp.headers().get("replay-nonce").and_then(|v| v.to_str().ok()) {
                self.nonce = Some(n.to_string());
            }
            let reply = Reply {
                status: resp.status().as_u16(),
                location: resp.headers().get("location").and_then(|v| v.to_str().ok()).map(str::to_string),
                retry_after: retry_after(resp.headers()),
                body: resp.body_mut().with_config().limit(MAX_REPLY).read_to_vec()?,
            };
            if reply.status < 400 {
                return Ok(reply);
            }
            let problem = reply.json();
            let kind = problem["type"].as_str().unwrap_or_default();
            if attempt == 0 && kind.ends_with(":badNonce") {
                continue;
            }
            let detail = problem["detail"].as_str().unwrap_or("").chars().take(300).collect::<String>();
            return Err(AcmeError { message: format!("ACME {} {kind} {detail}", reply.status).trim().to_string(), retry_after: reply.retry_after }.into());
        }
        unreachable!("deux essais au plus")
    }

    /// Crée le compte (ou retrouve celui de cette clé) en acceptant les conditions de l'autorité ;
    /// rend son adresse (`kid`), à garder.
    pub fn account(&mut self) -> Result<String> {
        if let Some(kid) = &self.kid {
            return Ok(kid.clone());
        }
        let url = self.dir.new_account.clone();
        let reply = self.post(&url, Some(&json!({ "termsOfServiceAgreed": true })), None)?;
        let kid = reply.location.ok_or_else(|| anyhow!("compte ACME sans adresse"))?;
        self.kid = Some(kid.clone());
        Ok(kid)
    }

    /// Attend qu'un objet (autorisation, commande) quitte `pending`/`processing`, ou `ready` si `ready_ok`.
    fn poll(&mut self, url: &str, done: &[&str]) -> Result<Value> {
        let start = Instant::now();
        loop {
            let reply = self.post(url, None, None)?;
            let v = reply.json();
            let status = v["status"].as_str().unwrap_or_default().to_string();
            if done.contains(&status.as_str()) {
                return Ok(v);
            }
            if status == "invalid" {
                let why = v["error"]["detail"].as_str().or_else(|| v["challenges"].as_array().and_then(|c| c.iter().find_map(|c| c["error"]["detail"].as_str()))).unwrap_or("");
                bail!("ACME : refusé ({why})");
            }
            if start.elapsed() > POLL_FOR {
                bail!("ACME : toujours « {status} » après {} s", POLL_FOR.as_secs());
            }
            std::thread::sleep(reply.retry_after.unwrap_or(Duration::from_secs(2)).clamp(Duration::from_millis(200), Duration::from_secs(10)));
        }
    }

    /// Un certificat pour `name`, pour la clé `cert_key` : commande, défi TLS-ALPN-01 (`present`
    /// pose le certificat du défi, puis l'enlève avec `None`), finalisation, chaîne en PEM.
    pub fn obtain(&mut self, name: &str, cert_key: &rcgen::KeyPair, present: &dyn Fn(Option<Arc<CertifiedKey>>)) -> Result<String> {
        self.account()?;
        let url = self.dir.new_order.clone();
        let reply = self.post(&url, Some(&json!({ "identifiers": [{ "type": "dns", "value": name }] })), None)?;
        let order_url = reply.location.clone().ok_or_else(|| anyhow!("commande ACME sans adresse"))?;
        let order = reply.json();
        let finalize = order["finalize"].as_str().ok_or_else(|| anyhow!("commande ACME sans finalize"))?.to_string();
        let authorizations: Vec<String> = order["authorizations"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect();
        for authz_url in authorizations {
            let authz = self.post(&authz_url, None, None)?.json();
            if authz["status"] == "valid" {
                continue;
            }
            let challenge = authz["challenges"].as_array().and_then(|c| c.iter().find(|c| c["type"] == "tls-alpn-01")).ok_or_else(|| anyhow!("pas de défi tls-alpn-01"))?;
            let (token, chall_url) = (challenge["token"].as_str().unwrap_or_default(), challenge["url"].as_str().unwrap_or_default().to_string());
            if token.is_empty() || token.len() > 200 || !token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || chall_url.is_empty() {
                bail!("défi tls-alpn-01 illisible");
            }
            present(Some(challenge_cert(name, &key_authorization(token, &self.key))?));
            let answered = self.post(&chall_url, Some(&json!({})), None).and_then(|_| self.poll(&authz_url, &["valid"]));
            present(None);
            answered?;
        }
        self.poll(&order_url, &["ready", "valid"])?;
        let csr = rcgen::CertificateParams::new(vec![name.to_string()])?.serialize_request(cert_key)?;
        let reply = self.post(&finalize, Some(&json!({ "csr": B64.encode(csr.der()) })), None)?;
        let mut order = reply.json();
        if order["status"] != "valid" {
            order = self.poll(&order_url, &["valid"])?;
        }
        let cert_url = order["certificate"].as_str().ok_or_else(|| anyhow!("commande ACME sans certificat"))?.to_string();
        let chain = self.post(&cert_url, None, Some("application/pem-certificate-chain"))?;
        String::from_utf8(chain.body).context("chaîne ACME illisible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreinte_jwk_rfc7638() {
        let key = AccountKey::generate().unwrap();
        let jwk = key.jwk();
        assert_eq!(jwk["kty"], "EC");
        assert_eq!(B64.decode(jwk["x"].as_str().unwrap()).unwrap().len(), 32);
        // L'empreinte est celle du JSON canonique (membres triés, sans espaces).
        let canonical = serde_json::to_string(&json!({ "crv": jwk["crv"], "kty": jwk["kty"], "x": jwk["x"], "y": jwk["y"] })).unwrap();
        assert_eq!(key.thumbprint(), B64.encode(Sha256::digest(canonical.as_bytes())));
        assert_eq!(key_authorization("jeton", &key), format!("jeton.{}", key.thumbprint()));
        // Relue depuis PKCS#8 : même clé.
        assert_eq!(AccountKey::from_pkcs8(key.pkcs8()).unwrap().thumbprint(), key.thumbprint());
    }

    /// Le certificat du défi : ce nom, et l'extension acmeIdentifier (1.3.6.1.5.5.7.1.31), critique,
    /// qui porte le SHA-256 de l'autorisation de clé.
    #[test]
    fn certificat_du_defi() {
        let ck = challenge_cert("abc.station.share.test", "jeton.empreinte").unwrap();
        let der = ck.cert[0].as_ref();
        let oid = [0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x01, 0x1f];
        let at = der.windows(oid.len()).position(|w| w == oid).expect("acmeIdentifier");
        let rest = &der[at + oid.len()..];
        assert_eq!(&rest[..3], &[0x01, 0x01, 0xff], "critique");
        assert_eq!(&rest[3..7], &[0x04, 0x22, 0x04, 0x20]);
        assert_eq!(&rest[7..39], Sha256::digest(b"jeton.empreinte").as_slice());
        assert!(der.windows(22).any(|w| w == b"abc.station.share.test"));
    }
}
