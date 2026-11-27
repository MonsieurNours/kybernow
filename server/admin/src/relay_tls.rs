//! Le navigateur chiffré de bout en bout (docs/relais.md, « Navigateur chiffré
//! de bout en bout », STWIN-17) : le certificat de la Station pour
//! `<id>.<tls_zone>`, dont la clé ne quitte jamais la Station.
//!
//! - Obtenu et renouvelé par ACME (`crate::acme`), défi TLS-ALPN-01 : le relais
//!   fait passer la connexion de l'autorité comme celle d'un navigateur (flux
//!   `browser_tls`), et [`Resolver`] y répond avec le certificat du défi.
//! - Gardé dans `<données>/relay/tls/` : `cert.pem` (la clé et la chaîne
//!   ensemble, une seule écriture), `account.key` et `account.json` (le compte
//!   ACME), `issued.json` (les numéros de série obtenus, pour la surveillance
//!   des journaux publics).
//! - Surveillance : une fois par jour, `crt.sh` ; un certificat émis pour ce nom
//!   que la Station n'a pas demandé suspend « Admin à distance ».

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::acme::{self, ACME_TLS_ALPN, AccountKey};

/// Nouvel essai après un échec : 5 min, 15 min, 1 h, 4 h, puis toutes les 24 h.
const BACKOFF: [Duration; 5] = [
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(3600),
    Duration::from_secs(4 * 3600),
    Duration::from_secs(24 * 3600),
];
/// Un relais joint qui n'a pas annoncé de zone depuis ce temps ne la fait plus passer.
#[cfg(not(test))]
pub const ZONE_GRACE: Duration = Duration::from_secs(10);
#[cfg(test)]
pub const ZONE_GRACE: Duration = Duration::from_millis(500);
/// La surveillance des journaux publics, une fois par jour.
const CT_EVERY: Duration = Duration::from_secs(24 * 3600);

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// L'annuaire ACME : Let's Encrypt, ou `KYBERNOW_ACME_DIRECTORY` (essais, « staging »).
pub fn directory() -> String {
    std::env::var("KYBERNOW_ACME_DIRECTORY").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_else(|| acme::LETS_ENCRYPT.to_string())
}

/// Où en est le certificat de bout en bout (`remote_tls.state` de `GET /api/relay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Rien ne le demande (relais non joint, ni « Admin à distance » ni partage).
    Off,
    /// Demande en cours, ou à faire.
    Pending,
    /// Un certificat valable pour le nom en cours.
    Ready,
    /// Le dernier essai a échoué ; un autre suivra.
    Error,
    /// Le relais n'annonce pas de zone (relais d'avant la 0.11.8, ou passage coupé).
    Unsupported,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Off => "off",
            State::Pending => "pending",
            State::Ready => "ready",
            State::Error => "error",
            State::Unsupported => "unsupported",
        }
    }
}

/// Le certificat en service.
struct Current {
    name: String,
    key: Arc<CertifiedKey>,
    not_before: i64,
    not_after: i64,
}

impl Current {
    /// À renouveler : moins d'un tiers de sa durée de validité reste.
    fn due(&self, now: i64) -> bool {
        let life = (self.not_after - self.not_before).max(0);
        now >= self.not_after - life / 3
    }
}

#[derive(Default)]
struct Inner {
    current: Option<Current>,
    /// Le nom annoncé par le relais joint (`<id>.<zone>`), et depuis quand il n'en annonce plus.
    name: Option<String>,
    /// Le dernier nom annoncé, gardé d'une connexion à l'autre : les échecs ne s'oublient que
    /// pour un nom nouveau (une reconnexion ne relance pas la demande avant son heure).
    last_name: Option<String>,
    joined: bool,
    zone_missing_since: Option<Instant>,
    wanted: bool,
    failures: u32,
    next_try: Option<Instant>,
    reason: Option<String>,
    ct_checked: Option<Instant>,
}

/// Le compte ACME gardé (`account.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountRecord {
    directory: String,
    kid: String,
}

/// Le certificat de bout en bout de la Station : obtenu, gardé, servi.
pub struct RelayTls {
    dir: PathBuf,
    inner: Mutex<Inner>,
    /// Le certificat du défi en cours (nom, certificat), le temps d'une validation.
    challenge: Mutex<Option<(String, Arc<CertifiedKey>)>>,
    busy: AtomicBool,
    ct_busy: AtomicBool,
}

impl RelayTls {
    pub fn new(data_dir: &Path) -> Arc<RelayTls> {
        let tls = Arc::new(RelayTls {
            dir: data_dir.join("relay").join("tls"),
            inner: Mutex::new(Inner::default()),
            challenge: Mutex::new(None),
            busy: AtomicBool::new(false),
            ct_busy: AtomicBool::new(false),
        });
        match tls.load() {
            Ok(Some(c)) => {
                tracing::info!("relais : certificat de bout en bout pour {} (jusqu'au {})", c.name, c.not_after);
                tls.inner.lock().unwrap().current = Some(c);
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("relais : certificat de bout en bout illisible : {e:#}"),
        }
        tls
    }

    fn cert_path(&self) -> PathBuf {
        self.dir.join("cert.pem")
    }

    fn load(&self) -> Result<Option<Current>> {
        let bytes = match std::fs::read(self.cert_path()) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        current_from_pem(&bytes).map(Some)
    }

    /// Le serveur TLS des flux `browser_tls` (ALPN `http/1.1`, et `acme-tls/1` pour le défi).
    pub fn acceptor(self: &Arc<Self>) -> Result<tokio_rustls::TlsAcceptor> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(Resolver(self.clone())));
        config.alpn_protocols = vec![b"http/1.1".to_vec(), ACME_TLS_ALPN.to_vec()];
        Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
    }

    /// Ce que la veille voit du relais : joint ou non, le nom `<id>.<zone>` annoncé, et si le
    /// navigateur en a besoin (« Admin à distance », partage, prêt, invitation).
    pub fn observe(&self, joined: bool, name: Option<String>, wanted: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.zone_missing_since = if joined && name.is_none() { Some(inner.zone_missing_since.unwrap_or_else(Instant::now)) } else { None };
        if name.is_some() && name != inner.last_name {
            inner.failures = 0;
            inner.next_try = None;
            inner.reason = None;
            inner.last_name = name.clone();
        }
        inner.name = if joined { name } else { None };
        inner.joined = joined;
        inner.wanted = wanted;
    }

    /// Le nom de bout en bout en cours (`<id>.station.share.kybernow.app`), s'il est annoncé.
    pub fn name(&self) -> Option<String> {
        self.inner.lock().unwrap().name.clone()
    }

    /// Un certificat valable pour le nom annoncé par le relais joint.
    pub fn ready(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        let now = unix_now();
        inner.joined && inner.current.as_ref().is_some_and(|c| Some(&c.name) == inner.name.as_ref() && now >= c.not_before && now < c.not_after)
    }

    /// Le relais joint n'annonce plus de zone depuis [`ZONE_GRACE`].
    pub fn zone_missing(&self) -> bool {
        self.inner.lock().unwrap().zone_missing_since.is_some_and(|t| t.elapsed() >= ZONE_GRACE)
    }

    /// Faut-il demander (ou renouveler) le certificat maintenant ? Rend le nom à demander.
    pub fn due(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        let name = inner.name.clone().filter(|_| inner.wanted)?;
        if self.busy.load(Ordering::Relaxed) || inner.next_try.is_some_and(|t| Instant::now() < t) {
            return None;
        }
        let fresh = inner.current.as_ref().is_some_and(|c| c.name == name && !c.due(unix_now()));
        (!fresh).then_some(name)
    }

    /// `remote_tls` de `GET /api/relay` (sans `foreign`, que donnent les réglages).
    pub fn status(&self) -> Value {
        let inner = self.inner.lock().unwrap();
        let now = unix_now();
        let ready = inner.joined && inner.current.as_ref().is_some_and(|c| Some(&c.name) == inner.name.as_ref() && now >= c.not_before && now < c.not_after);
        let state = if ready {
            State::Ready
        } else if !inner.joined {
            State::Off
        } else if inner.zone_missing_since.is_some() {
            if inner.zone_missing_since.is_some_and(|t| t.elapsed() >= ZONE_GRACE) { State::Unsupported } else { State::Pending }
        } else if inner.reason.is_some() {
            State::Error
        } else if inner.wanted || self.busy.load(Ordering::Relaxed) {
            State::Pending
        } else {
            State::Off
        };
        let until = inner.current.as_ref().filter(|c| Some(&c.name) == inner.name.as_ref()).map(|c| c.not_after);
        json!({ "state": state.as_str(), "name": inner.name, "until": until, "reason": inner.reason })
    }

    /// Lance la demande du certificat pour `name` (une seule à la fois), dans un fil à part.
    pub fn spawn_obtain(self: &Arc<Self>, name: String, on_done: impl FnOnce(bool) + Send + 'static) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let tls = self.clone();
        tokio::task::spawn_blocking(move || {
            let result = tls.obtain(&directory(), &name);
            let ok = result.is_ok();
            {
                let mut inner = tls.inner.lock().unwrap();
                match &result {
                    Ok(()) => {
                        tracing::info!("relais : certificat de bout en bout obtenu pour {name}");
                        inner.failures = 0;
                        inner.next_try = None;
                        inner.reason = None;
                    }
                    Err(e) => {
                        let wait = e.downcast_ref::<acme::AcmeError>().and_then(|a| a.retry_after).unwrap_or(BACKOFF[(inner.failures as usize).min(BACKOFF.len() - 1)]);
                        tracing::warn!("relais : certificat de bout en bout pour {name} non obtenu (nouvel essai dans {} min) : {e:#}", wait.as_secs() / 60);
                        inner.failures = inner.failures.saturating_add(1);
                        inner.next_try = Some(Instant::now() + wait);
                        inner.reason = Some(format!("{e:#}").chars().take(300).collect());
                    }
                }
            }
            tls.busy.store(false, Ordering::Release);
            on_done(ok);
        });
    }

    /// Le compte ACME (créé et gardé au premier usage), puis le certificat ; gardé et mis en service.
    fn obtain(&self, directory: &str, name: &str) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let key_path = self.dir.join("account.key");
        let account = match std::fs::read(&key_path) {
            Ok(pem) => AccountKey::from_pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from_pem_slice(&pem).context("account.key")?.secret_pkcs8_der())?,
            Err(_) => {
                let key = AccountKey::generate()?;
                write_secret(&key_path, &pem("PRIVATE KEY", key.pkcs8()))?;
                key
            }
        };
        let record_path = self.dir.join("account.json");
        let record: AccountRecord = std::fs::read(&record_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let kid = (record.directory == directory && !record.kid.is_empty()).then_some(record.kid);
        let mut client = acme::Client::new(directory, account, kid)?;
        let kid = client.account()?;
        let record = AccountRecord { directory: directory.to_string(), kid };
        crate::durable::write(&record_path, serde_json::to_vec_pretty(&record)?)?;
        let cert_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)?;
        let chain = client.obtain(name, &cert_key, &|c| self.present(name, c))?;
        let bundle = format!("{}{}", cert_key.serialize_pem(), chain);
        let current = current_from_pem(bundle.as_bytes())?;
        anyhow::ensure!(current.name == name, "certificat reçu pour {} au lieu de {name}", current.name);
        anyhow::ensure!(current.key.keys_match().is_ok(), "le certificat reçu n'est pas pour notre clé");
        let serial = serial_of(&current.key.cert[0]).ok_or_else(|| anyhow!("numéro de série illisible"))?;
        self.note_issued(&serial)?;
        write_secret(&self.cert_path(), &bundle)?;
        self.inner.lock().unwrap().current = Some(current);
        Ok(())
    }

    fn present(&self, name: &str, cert: Option<Arc<CertifiedKey>>) {
        *self.challenge.lock().unwrap() = cert.map(|c| (name.to_string(), c));
    }

    fn issued_path(&self) -> PathBuf {
        self.dir.join("issued.json")
    }

    /// Les numéros de série des certificats que la Station a obtenus (hexadécimal, sans zéros devant).
    pub fn issued(&self) -> Vec<String> {
        std::fs::read(self.issued_path()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v["serials"].as_array().cloned()).into_iter().flatten().filter_map(|s| s.as_str().map(normalize_serial)).collect()
    }

    fn note_issued(&self, serial: &str) -> Result<()> {
        let mut all = self.issued();
        let serial = normalize_serial(serial);
        if !all.contains(&serial) {
            all.push(serial);
        }
        crate::durable::write(&self.issued_path(), serde_json::to_vec_pretty(&json!({ "serials": all }))?)?;
        Ok(())
    }

    /// La surveillance des journaux publics est-elle à faire (certificat en service, une fois par jour) ?
    pub fn ct_due(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        let name = inner.current.as_ref().map(|c| c.name.clone())?;
        if self.ct_busy.load(Ordering::Relaxed) || inner.ct_checked.is_some_and(|t| t.elapsed() < CT_EVERY) {
            return None;
        }
        Some(name)
    }

    /// Demande à `crt.sh` les certificats émis pour `name` ; `on_foreign` reçoit ceux que la
    /// Station n'a pas obtenus. `crt.sh` injoignable : rien, nouvel essai le lendemain.
    pub fn spawn_ct_check(self: &Arc<Self>, name: String, on_foreign: impl FnOnce(Vec<String>) + Send + 'static) {
        if self.ct_busy.swap(true, Ordering::AcqRel) {
            return;
        }
        self.inner.lock().unwrap().ct_checked = Some(Instant::now());
        let tls = self.clone();
        tokio::task::spawn_blocking(move || {
            let found = fetch_ct(&name);
            tls.ct_busy.store(false, Ordering::Release);
            match found {
                Ok(json) => {
                    let foreign = foreign_serials(&json, &name, &tls.issued());
                    if !foreign.is_empty() {
                        on_foreign(foreign);
                    }
                }
                Err(e) => tracing::info!("relais : journaux publics injoignables ({e:#}), nouvel essai demain"),
            }
        });
    }
}

/// Répond aux flux `browser_tls` : le certificat du défi pour l'ALPN `acme-tls/1` (et ce nom
/// seulement), sinon le certificat de bout en bout pour son nom. Rien d'autre : la négociation échoue.
#[derive(Debug)]
pub struct Resolver(Arc<RelayTls>);

impl std::fmt::Debug for RelayTls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayTls").field("dir", &self.dir).finish_non_exhaustive()
    }
}

impl ResolvesServerCert for Resolver {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = hello.server_name()?.trim_end_matches('.').to_ascii_lowercase();
        let acme = hello.alpn().is_some_and(|mut a| a.any(|p| p == ACME_TLS_ALPN));
        if acme {
            return self.0.challenge.lock().unwrap().as_ref().filter(|(n, _)| *n == sni).map(|(_, c)| c.clone());
        }
        let inner = self.0.inner.lock().unwrap();
        inner.current.as_ref().filter(|c| c.name == sni && unix_now() < c.not_after).map(|c| c.key.clone())
    }
}

/// L'ALPN négocié est-il celui du défi ? (La connexion se ferme alors aussitôt, sans HTTP.)
pub fn is_acme(alpn: Option<&[u8]>) -> bool {
    alpn == Some(ACME_TLS_ALPN)
}

// --- Fichiers et certificats ---------------------------------------------------------

fn pem(label: &str, der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<&str> = b64.as_bytes().chunks(64).map(|c| std::str::from_utf8(c).unwrap_or_default()).collect();
    format!("-----BEGIN {label}-----\n{}\n-----END {label}-----\n", lines.join("\n"))
}

/// Une clé ou un certificat avec sa clé : réservé à SYSTEM et aux Administrateurs (Windows), à son compte (Mac, Linux).
fn write_secret(path: &Path, text: &str) -> Result<()> {
    crate::durable::try_write_then(path, text, |tmp| crate::update::acl::secure_file(tmp).map_err(|e| std::io::Error::other(format!("{e:#}"))))?;
    Ok(())
}

/// La clé (PKCS#8) et la chaîne d'un `cert.pem`, prêtes pour rustls, avec leur nom et leurs dates.
fn current_from_pem(bytes: &[u8]) -> Result<Current> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(bytes).collect::<Result<_, _>>().context("chaîne illisible")?;
    anyhow::ensure!(!chain.is_empty(), "aucun certificat");
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from_pem_slice(bytes).context("clé illisible")?;
    let (not_before, not_after) = validity(&chain[0]).ok_or_else(|| anyhow!("dates du certificat illisibles"))?;
    let name = dns_name(&chain[0]).ok_or_else(|| anyhow!("nom du certificat illisible"))?;
    let key = acme::certified(chain, key.secret_pkcs8_der())?;
    Ok(Current { name, key, not_before, not_after })
}

/// Un élément DER : (étiquette, contenu, reste).
fn der(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, mut rest) = rest.split_first()?;
    let len = if first < 0x80 {
        first as usize
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        let len = rest[..n].iter().fold(0usize, |acc, b| (acc << 8) | *b as usize);
        rest = &rest[n..];
        len
    };
    (rest.len() >= len).then(|| (tag, &rest[..len], &rest[len..]))
}

/// Le `TBSCertificate` : (numéro de série, validité, reste après le sujet).
fn tbs_parts(cert: &[u8]) -> Option<(&[u8], &[u8], &[u8])> {
    let (_, cert, _) = der(cert)?;
    let (_, tbs, _) = der(cert)?;
    let mut rest = tbs;
    let (tag, _, r) = der(rest)?;
    if tag == 0xa0 {
        rest = r;
    }
    let (_, serial, r) = der(rest)?;
    let (_, _, r) = der(r)?; // algorithme
    let (_, _, r) = der(r)?; // émetteur
    let (_, validity, r) = der(r)?;
    let (_, _, r) = der(r)?; // sujet
    Some((serial, validity, r))
}

/// Le numéro de série, en hexadécimal minuscule sans zéros devant.
pub fn serial_of(cert: &[u8]) -> Option<String> {
    let (serial, _, _) = tbs_parts(cert)?;
    Some(normalize_serial(&serial.iter().map(|b| format!("{b:02x}")).collect::<String>()))
}

fn normalize_serial(s: &str) -> String {
    let s = s.trim().to_ascii_lowercase().replace(':', "");
    let t = s.trim_start_matches('0');
    if t.is_empty() { "0".into() } else { t.to_string() }
}

/// Début et fin de validité, en secondes Unix.
fn validity(cert: &[u8]) -> Option<(i64, i64)> {
    let (_, validity, _) = tbs_parts(cert)?;
    let (t1, a, r) = der(validity)?;
    let (t2, b, _) = der(r)?;
    Some((parse_time(t1, std::str::from_utf8(a).ok()?)?, parse_time(t2, std::str::from_utf8(b).ok()?)?))
}

/// Le premier nom DNS de l'extension `subjectAltName`.
fn dns_name(cert: &[u8]) -> Option<String> {
    let (_, _, rest) = tbs_parts(cert)?;
    // Après le sujet : la clé publique, puis les extensions ([3]).
    let (_, _, mut rest) = der(rest)?;
    while let Some((tag, content, r)) = der(rest) {
        rest = r;
        if tag != 0xa3 {
            continue;
        }
        let (_, mut exts, _) = der(content)?;
        while let Some((_, ext, r)) = der(exts) {
            exts = r;
            let (_, oid, after) = der(ext)?;
            if oid != [0x55, 0x1d, 0x11] {
                continue;
            }
            let (tag, val, after2) = der(after)?;
            let val = if tag == 0x01 { der(after2)?.1 } else { val };
            let (_, mut names, _) = der(val)?;
            while let Some((t, n, r)) = der(names) {
                names = r;
                if t == 0x82 {
                    return std::str::from_utf8(n).ok().map(|s| s.to_ascii_lowercase());
                }
            }
        }
    }
    None
}

fn parse_time(tag: u8, s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (year, rest) = match tag {
        0x17 if s.len() >= 12 => {
            let y: i64 = s[..2].parse().ok()?;
            (if y >= 50 { 1900 + y } else { 2000 + y }, &s[2..])
        }
        0x18 if s.len() >= 14 => (s[..4].parse().ok()?, &s[4..]),
        _ => return None,
    };
    let n = |i: usize| -> Option<i64> { rest.get(i..i + 2)?.parse().ok() };
    let (month, day, hour, min, sec) = (n(0)?, n(2)?, n(4)?, n(6)?, n(8)?);
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(days * 86_400 + hour * 3600 + min * 60 + sec)
}

// --- Journaux publics (Certificate Transparency) ------------------------------------------

fn fetch_ct(name: &str) -> Result<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(format!("kyber-admin/{} (ct)", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let url = format!("https://crt.sh/?q={name}&output=json&exclude=expired");
    let mut resp = agent.get(&url).call().context("crt.sh")?;
    Ok(resp.body_mut().with_config().limit(4 * 1024 * 1024).read_to_string()?)
}

/// Les numéros de série des certificats émis pour `name` (réponse JSON de `crt.sh`) que la Station
/// n'a pas obtenus elle-même. Un certificat pour un autre nom (réponse plus large) ne compte pas.
pub fn foreign_serials(json: &str, name: &str, ours: &[String]) -> Vec<String> {
    let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    let mut out: Vec<String> = entries
        .iter()
        .filter(|e| {
            let names = format!("{}\n{}", e["common_name"].as_str().unwrap_or_default(), e["name_value"].as_str().unwrap_or_default());
            names.lines().any(|n| n.trim().eq_ignore_ascii_case(name))
        })
        .filter_map(|e| e["serial_number"].as_str().map(normalize_serial))
        .filter(|s| !ours.contains(s))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn leaf(name: &str, days: i64) -> (String, String) {
        let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
        let mut params = rcgen::CertificateParams::new(vec![name.to_string()]).unwrap();
        params.serial_number = Some(rcgen::SerialNumber::from_slice(&[0x00, 0x04, 0xab, 0xcd]));
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(days);
        let cert = params.self_signed(&key).unwrap();
        (format!("{}{}", key.serialize_pem(), cert.pem()), name.to_string())
    }

    #[test]
    fn lecture_d_un_certificat() {
        let (bundle, name) = leaf("abc.station.share.test", 90);
        let c = current_from_pem(bundle.as_bytes()).unwrap();
        assert_eq!(c.name, name);
        assert!(c.key.keys_match().is_ok());
        let life = c.not_after - c.not_before;
        assert_eq!(life, 91 * 86_400);
        assert!(!c.due(unix_now()), "neuf : pas à renouveler");
        assert!(c.due(c.not_after - life / 3 + 1), "moins d'un tiers restant : à renouveler");
        assert_eq!(serial_of(&c.key.cert[0]).as_deref(), Some("4abcd"));
        assert_eq!(normalize_serial("00:04:AB:CD"), "4abcd");
    }

    #[test]
    fn journaux_publics() {
        let json = r#"[
            {"common_name":"abc.station.share.test","name_value":"abc.station.share.test","serial_number":"04abcd"},
            {"common_name":"abc.station.share.test","name_value":"abc.station.share.test","serial_number":"0badc0de"},
            {"common_name":"x.station.share.test","name_value":"x.station.share.test","serial_number":"77"}
        ]"#;
        assert_eq!(foreign_serials(json, "abc.station.share.test", &["4abcd".into()]), vec!["badc0de".to_string()]);
        assert!(foreign_serials(json, "abc.station.share.test", &["4abcd".into(), "badc0de".into()]).is_empty());
        assert!(foreign_serials("pas du json", "abc.station.share.test", &[]).is_empty());
    }

    /// Une fausse autorité ACME (HTTP sur la boucle locale) : elle vérifie chaque signature JWS,
    /// valide le défi TLS-ALPN-01 en négociant vraiment avec le serveur TLS des flux `browser_tls`
    /// de la Station, et signe la clé de la demande (CSR).
    mod fausse_autorite {
        use std::sync::{Arc, Mutex};

        use axum::body::Bytes;
        use axum::http::{HeaderMap, Method, StatusCode, Uri};
        use axum::response::{IntoResponse, Response};
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
        use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};
        use serde_json::{Value, json};
        use sha2::{Digest, Sha256};

        use super::super::{RelayTls, der};

        pub struct Ca {
            pub base: String,
            pub name: String,
            pub tls: Arc<RelayTls>,
            pub jwk: Option<Value>,
            pub nonce: u32,
            pub validated: bool,
            pub cert: Option<String>,
            pub ca: (rcgen::Certificate, rcgen::KeyPair),
            pub signed: u32,
        }

        fn tlv(input: &[u8]) -> Option<(&[u8], &[u8])> {
            let (_, _, rest) = der(input)?;
            Some((&input[..input.len() - rest.len()], rest))
        }

        /// La clé publique P-256 d'une demande de certificat (le point, hors de sa BIT STRING).
        struct CsrKey(Vec<u8>);

        impl rcgen::PublicKeyData for CsrKey {
            fn der_bytes(&self) -> &[u8] {
                &self.0
            }

            fn algorithm(&self) -> &rcgen::SignatureAlgorithm {
                &rcgen::PKCS_ECDSA_P256_SHA256
            }
        }

        fn csr_key(csr: &[u8]) -> Option<CsrKey> {
            let (_, csr, _) = der(csr)?;
            let (_, cri, _) = der(csr)?;
            let (_, rest) = tlv(cri)?; // version
            let (_, rest) = tlv(rest)?; // sujet
            let (_, spki, _) = der(rest)?;
            let (_, rest) = tlv(spki)?; // algorithme
            let (tag, bits, _) = der(rest)?;
            (tag == 0x03 && bits.first() == Some(&0)).then(|| CsrKey(bits[1..].to_vec()))
        }

        fn point(jwk: &Value) -> Vec<u8> {
            let mut p = vec![4u8];
            p.extend(B64.decode(jwk["x"].as_str().unwrap()).unwrap());
            p.extend(B64.decode(jwk["y"].as_str().unwrap()).unwrap());
            p
        }

        fn thumbprint(jwk: &Value) -> String {
            let c = format!(r#"{{"crv":"P-256","kty":"EC","x":"{}","y":"{}"}}"#, jwk["x"].as_str().unwrap(), jwk["y"].as_str().unwrap());
            B64.encode(Sha256::digest(c.as_bytes()))
        }

        /// Le défi : une vraie négociation TLS, ALPN `acme-tls/1`, contre le serveur des flux `browser_tls`.
        async fn validate(tls: &Arc<RelayTls>, name: &str, key_auth: &str) -> bool {
            #[derive(Debug)]
            struct Any;
            impl rustls::client::danger::ServerCertVerifier for Any {
                fn verify_server_cert(&self, _: &rustls::pki_types::CertificateDer<'_>, _: &[rustls::pki_types::CertificateDer<'_>], _: &rustls::pki_types::ServerName<'_>, _: &[u8], _: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
                    Ok(rustls::client::danger::ServerCertVerified::assertion())
                }
                fn verify_tls12_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                    Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
                }
                fn verify_tls13_signature(&self, _: &[u8], _: &rustls::pki_types::CertificateDer<'_>, _: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
                    Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
                }
                fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                    rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
                }
            }
            let (client, server) = tokio::io::duplex(64 * 1024);
            let acceptor = tls.acceptor().unwrap();
            tokio::spawn(async move {
                let _ = acceptor.accept(server).await;
            });
            let mut cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(Any))
                .with_no_client_auth();
            cfg.alpn_protocols = vec![b"acme-tls/1".to_vec()];
            let Ok(conn) = tokio_rustls::TlsConnector::from(Arc::new(cfg)).connect(rustls::pki_types::ServerName::try_from(name.to_string()).unwrap(), client).await else {
                return false;
            };
            let cert = conn.get_ref().1.peer_certificates().unwrap()[0].as_ref().to_vec();
            let oid = [0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x01, 0x1f];
            let Some(at) = cert.windows(oid.len()).position(|w| w == oid) else { return false };
            cert[at + oid.len() + 7..at + oid.len() + 39] == Sha256::digest(key_auth.as_bytes())[..]
        }

        pub async fn handle(ca: Arc<Mutex<Ca>>, method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Response {
            let path = uri.path().to_string();
            let base = ca.lock().unwrap().base.clone();
            let nonce = {
                let mut c = ca.lock().unwrap();
                c.nonce += 1;
                format!("n{}", c.nonce)
            };
            let reply = |status: StatusCode, body: Value, location: Option<String>| {
                let mut r = (status, axum::Json(body)).into_response();
                r.headers_mut().insert("replay-nonce", nonce.parse().unwrap());
                if let Some(l) = location {
                    r.headers_mut().insert("location", l.parse().unwrap());
                }
                r
            };
            if method == Method::GET && path == "/dir" {
                return reply(StatusCode::OK, json!({ "newNonce": format!("{base}/nonce"), "newAccount": format!("{base}/acct"), "newOrder": format!("{base}/order") }), None);
            }
            if path == "/nonce" {
                return reply(StatusCode::OK, Value::Null, None);
            }
            // Une requête signée : en-tête protégé, signature ES256 vérifiée sur la clé du compte.
            assert_eq!(headers.get("content-type").unwrap(), "application/jose+json");
            let jws: Value = serde_json::from_slice(&body).unwrap();
            let protected: Value = serde_json::from_slice(&B64.decode(jws["protected"].as_str().unwrap()).unwrap()).unwrap();
            assert_eq!(protected["alg"], "ES256");
            assert_eq!(protected["url"], format!("{base}{path}"));
            if path == "/acct" {
                ca.lock().unwrap().jwk = Some(protected["jwk"].clone());
            } else {
                assert_eq!(protected["kid"], format!("{base}/acct/1"), "{path} : par le kid");
            }
            let jwk = ca.lock().unwrap().jwk.clone().unwrap();
            let signed = format!("{}.{}", jws["protected"].as_str().unwrap(), jws["payload"].as_str().unwrap());
            UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, point(&jwk)).verify(signed.as_bytes(), &B64.decode(jws["signature"].as_str().unwrap()).unwrap()).expect("signature JWS");
            ca.lock().unwrap().signed += 1;
            let payload: Value = match jws["payload"].as_str().unwrap() {
                "" => Value::Null,
                p => serde_json::from_slice(&B64.decode(p).unwrap()).unwrap(),
            };
            let name = ca.lock().unwrap().name.clone();
            let token = "jeton-du-defi_42";
            let order = |status: &str| json!({ "status": status, "finalize": format!("{base}/finalize"), "authorizations": [format!("{base}/authz")], "certificate": format!("{base}/cert") });
            match path.as_str() {
                "/acct" => {
                    assert_eq!(payload["termsOfServiceAgreed"], true);
                    reply(StatusCode::CREATED, json!({ "status": "valid" }), Some(format!("{base}/acct/1")))
                }
                "/order" => {
                    assert_eq!(payload["identifiers"][0]["value"], name);
                    reply(StatusCode::CREATED, order("pending"), Some(format!("{base}/order/1")))
                }
                "/authz" => {
                    let status = if ca.lock().unwrap().validated { "valid" } else { "pending" };
                    reply(StatusCode::OK, json!({ "status": status, "challenges": [
                        { "type": "http-01", "url": format!("{base}/autre"), "token": "x" },
                        { "type": "tls-alpn-01", "url": format!("{base}/chall"), "token": token }
                    ] }), None)
                }
                "/chall" => {
                    let tls = ca.lock().unwrap().tls.clone();
                    let ok = validate(&tls, &name, &format!("{token}.{}", thumbprint(&jwk))).await;
                    ca.lock().unwrap().validated = ok;
                    reply(StatusCode::OK, json!({ "type": "tls-alpn-01", "status": if ok { "valid" } else { "invalid" } }), None)
                }
                "/order/1" => {
                    let c = ca.lock().unwrap();
                    let status = if c.cert.is_some() { "valid" } else if c.validated { "ready" } else { "pending" };
                    reply(StatusCode::OK, order(status), None)
                }
                "/finalize" => {
                    let csr = B64.decode(payload["csr"].as_str().unwrap()).unwrap();
                    let spki = csr_key(&csr).unwrap();
                    let mut c = ca.lock().unwrap();
                    let mut params = rcgen::CertificateParams::new(vec![name.clone()]).unwrap();
                    params.serial_number = Some(rcgen::SerialNumber::from_slice(&[0x01, 0x23, 0x45]));
                    let leaf = params.signed_by(&spki, &c.ca.0, &c.ca.1).unwrap();
                    c.cert = Some(format!("{}{}", leaf.pem(), c.ca.0.pem()));
                    reply(StatusCode::OK, order("processing"), None)
                }
                "/cert" => {
                    assert_eq!(headers.get("accept").unwrap(), "application/pem-certificate-chain");
                    let pem = ca.lock().unwrap().cert.clone().unwrap();
                    let mut r = (StatusCode::OK, pem).into_response();
                    r.headers_mut().insert("replay-nonce", nonce.parse().unwrap());
                    r
                }
                _ => reply(StatusCode::NOT_FOUND, json!({ "type": "urn:ietf:params:acme:error:malformed" }), None),
            }
        }
    }

    /// Tout le parcours : compte, commande, défi TLS-ALPN-01 vraiment négocié, finalisation, chaîne ;
    /// puis le certificat gardé, servi pour son nom (et seulement lui), et noté dans `issued.json`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn certificat_obtenu_par_acme() {
        let dir = std::env::temp_dir().join(format!("kn-relay-acme-{}", rand::random::<u64>()));
        let tls = RelayTls::new(&dir);
        let name = "abcdefghijklmnopqrstuvwxyz.station.share.test".to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();
        let ca = Arc::new(Mutex::new(fausse_autorite::Ca { base: base.clone(), name: name.clone(), tls: tls.clone(), jwk: None, nonce: 0, validated: false, cert: None, ca: (ca_cert, ca_key), signed: 0 }));
        let state = ca.clone();
        let app = axum::Router::new().fallback(move |m, u, h, b| fausse_autorite::handle(state.clone(), m, u, h, b));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tls.observe(true, Some(name.clone()), true);
        assert_eq!(tls.due().as_deref(), Some(name.as_str()));
        let (t, n, d) = (tls.clone(), name.clone(), format!("{base}/dir"));
        tokio::task::spawn_blocking(move || t.obtain(&d, &n)).await.unwrap().expect("certificat obtenu");
        assert!(ca.lock().unwrap().validated, "défi validé par une vraie négociation");
        assert!(ca.lock().unwrap().signed >= 6);
        assert!(tls.ready());
        assert_eq!(tls.due(), None, "neuf : rien à renouveler");
        assert_eq!(tls.status()["state"], "ready");
        assert_eq!(tls.issued(), vec!["12345".to_string()]);
        assert!(tls.challenge.lock().unwrap().is_none(), "le certificat du défi est retiré");
        // Relu au redémarrage, compte ACME compris.
        let again = RelayTls::new(&dir);
        again.observe(true, Some(name.clone()), true);
        assert!(again.ready());
        let account: Value = serde_json::from_slice(&std::fs::read(dir.join("relay/tls/account.json")).unwrap()).unwrap();
        assert_eq!(account["kid"], format!("{base}/acct/1"));
        // Le résolveur : ce nom seulement, et pas de défi sans défi en cours.
        let hello = |sni: &str, alpn: &[u8]| {
            let mut cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(rustls::RootCertStore::empty())
                .with_no_client_auth();
            cfg.alpn_protocols = vec![alpn.to_vec()];
            let mut conn = rustls::ClientConnection::new(Arc::new(cfg), rustls::pki_types::ServerName::try_from(sni.to_string()).unwrap()).unwrap();
            let mut raw = Vec::new();
            conn.write_tls(&mut raw).unwrap();
            let mut acceptor = rustls::server::Acceptor::default();
            acceptor.read_tls(&mut raw.as_slice()).unwrap();
            let accepted = acceptor.accept().unwrap().unwrap();
            Resolver(again.clone()).resolve(accepted.client_hello()).is_some()
        };
        assert!(hello(&name, b"http/1.1"));
        assert!(!hello("autre.station.share.test", b"http/1.1"));
        assert!(!hello(&name, ACME_TLS_ALPN), "pas de défi en cours");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Le relais joint annonce le nom ; sans zone pendant [`ZONE_GRACE`], plus de nom ; à demander
    /// seulement quand le navigateur en a besoin.
    #[test]
    fn veille_du_nom() {
        let dir = std::env::temp_dir().join(format!("kn-relay-tls-{}", rand::random::<u64>()));
        let tls = RelayTls::new(&dir);
        tls.observe(true, Some("abc.station.share.test".into()), false);
        assert_eq!(tls.due(), None, "pas de besoin : rien à demander");
        assert_eq!(tls.status()["state"], "off");
        tls.observe(true, Some("abc.station.share.test".into()), true);
        assert_eq!(tls.due().as_deref(), Some("abc.station.share.test"));
        assert_eq!(tls.status()["state"], "pending");
        assert!(!tls.ready());
        tls.observe(true, None, true);
        assert_eq!(tls.name(), None);
        assert!(!tls.zone_missing(), "pas encore : la zone suit le welcome");
        std::thread::sleep(ZONE_GRACE);
        assert!(tls.zone_missing());
        assert_eq!(tls.status()["state"], "unsupported");
        tls.observe(false, None, true);
        assert_eq!(tls.status()["state"], "off");
        let _ = std::fs::remove_dir_all(dir);
    }
}
