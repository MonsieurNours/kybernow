//! Étape facultative « Votre compte KyberNow » de l'installateur (docs/comptes.md,
//! « Dans les installateurs ») : la page de connexion du relais s'ouvre dans
//! la fenêtre de l'installateur, comme la fenêtre du système de l'app
//! (`/compte/connexion?appareil=…&retour=kybernow://compte&etat=…`) ; au
//! retour (`kybernow://compte?code=…&etat=…`), le code est échangé contre la
//! session d'appareil (`POST /v1/devices/token`), remise à l'app :
//!
//! - le jeton dans le Gestionnaire d'identification de Windows, cible
//!   `KyberNow:account:<relais>` (utilisateur : l'e-mail du compte, secret :
//!   le jeton en UTF-16LE, comme les mots de passe des ordinateurs de l'app) ;
//! - le reste dans `%APPDATA%\KyberNow\accounts.json`
//!   (`{"accounts":[{relay, account:{id,name,email}, device:{id,name}, added}]}`).
//!
//! L'app démarre alors connectée (lot 3, `apps/core` et `apps/windows`).

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Le relais public de KyberNow.
pub const DEFAULT_RELAY: &str = "share.kybernow.app";
/// Le retour vers l'app, intercepté par la fenêtre de l'installateur.
pub const RETURN_URI: &str = "kybernow://compte";
/// Préfixe des cibles du Gestionnaire d'identification (celui de l'app).
pub const CREDENTIAL_PREFIX: &str = "KyberNow:account:";
/// Le fichier des comptes connectés, dans `%APPDATA%\KyberNow`.
pub const ACCOUNTS_FILE: &str = "accounts.json";

// --- Adresse d'un relais ---------------------------------------------------------

/// `hôte[:port][#sha256:…]` (docs/relais.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayAddr {
    pub host: String,
    pub port: u16,
    pub pin: Option<[u8; 32]>,
}

impl RelayAddr {
    pub fn parse(input: &str) -> Option<RelayAddr> {
        let s = input.trim();
        let s = s.strip_prefix("https://").unwrap_or(s).trim_end_matches('/');
        let (main, pin) = match s.split_once('#') {
            Some((m, p)) => (m, Some(parse_fingerprint(p)?)),
            None => (s, None),
        };
        let (host, port) = if let Some(rest) = main.strip_prefix('[') {
            let (h, after) = rest.split_once(']')?;
            let port = match after.strip_prefix(':') {
                Some(p) => p.parse().ok()?,
                None if after.is_empty() => 443,
                None => return None,
            };
            (h.to_string(), port)
        } else {
            match main.rsplit_once(':') {
                Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().ok()?),
                Some(_) => return None,
                None => (main.to_string(), 443),
            }
        };
        let host = host.to_ascii_lowercase();
        let valid = !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
        (valid && port != 0).then_some(RelayAddr { host, port, pin })
    }

    /// `hôte`, ou `hôte:port` hors du 443 : le nom du relais, et l'autorité de ses pages.
    pub fn web_authority(&self) -> String {
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        if self.port == 443 { host } else { format!("{host}:{}", self.port) }
    }

    pub fn origin(&self) -> String {
        format!("https://{}", self.web_authority())
    }
}

impl std::fmt::Display for RelayAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.web_authority())?;
        if let Some(pin) = &self.pin {
            write!(f, "#sha256:{}", pin.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":"))?;
        }
        Ok(())
    }
}

/// « sha256:aa:bb:… », « aa:bb:… » ou 64 caractères hexadécimaux.
pub fn parse_fingerprint(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    let text = text.strip_prefix("sha256:").or_else(|| text.strip_prefix("SHA256:")).unwrap_or(text);
    let hex: String = text.chars().filter(|c| *c != ':').collect();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

// --- Les pages du relais ------------------------------------------------------------

/// Encodage d'une valeur de requête (`appareil=PC de Léa`).
fn pct(value: &str) -> String {
    value
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

fn unpct(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// La page de connexion d'un appareil, pour l'app « KyberNow Link » de ce PC.
pub fn login_url(relay: &RelayAddr, device: &str, state: &str, lang: &str) -> String {
    format!("{}/compte/connexion?appareil={}&retour={}&etat={}&lang={lang}", relay.origin(), pct(device), pct(RETURN_URI), pct(state))
}

/// La création d'un compte (sur le site : une app ne crée pas de compte,
/// mais l'installateur n'est pas une app d'un magasin).
pub fn signup_url(relay: &RelayAddr, lang: &str) -> String {
    format!("{}/compte/inscription?lang={lang}", relay.origin())
}

/// Le retour vers l'app : `(code, etat)`, ou l'erreur (`annule`).
pub fn parse_return(url: &str) -> Option<Result<(String, String), String>> {
    let rest = url.strip_prefix(RETURN_URI)?;
    let query = rest.trim_start_matches('/').strip_prefix('?').unwrap_or("");
    let mut code = None;
    let mut state = String::new();
    let mut error = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "code" => code = Some(unpct(v)),
            "etat" => state = unpct(v),
            "erreur" => error = Some(unpct(v)),
            _ => {}
        }
    }
    Some(match (code, error) {
        (Some(c), None) if !c.is_empty() => Ok((c, state)),
        (_, e) => Err(e.unwrap_or_else(|| "sans_code".into())),
    })
}

/// Où va la page du relais dans la fenêtre : rester sur le relais, le retour
/// vers l'app (intercepté), ou le navigateur pour tout autre lien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Navigation {
    Allow,
    Return(String),
    External,
    Block,
}

pub fn navigation(url: &str, origin: &str) -> Navigation {
    if url.starts_with(RETURN_URI) {
        return Navigation::Return(url.to_string());
    }
    if url == origin || url.starts_with(&format!("{origin}/")) || url.starts_with(&format!("{origin}?")) {
        return Navigation::Allow;
    }
    if url.starts_with("https://") || url.starts_with("http://") { Navigation::External } else { Navigation::Block }
}

/// Un `etat` tiré au hasard (le retour n'est pris que s'il le porte).
pub fn new_state() -> String {
    let mut bytes = [0u8; 16];
    getrandom(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn getrandom(buf: &mut [u8]) {
    let _ = rustls::crypto::ring::default_provider().secure_random.fill(buf);
}

// --- HTTPS vers le relais ------------------------------------------------------------

/// Réponse : code et corps.
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}

fn config(pin: Option<[u8; 32]>) -> Result<Arc<rustls::ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone()).with_safe_default_protocol_versions().context("versions TLS")?;
    let config = match pin {
        Some(fingerprint) => builder.dangerous().with_custom_certificate_verifier(Arc::new(Pinned { fingerprint, provider })).with_no_client_auth(),
        None => {
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            builder.with_root_certificates(roots).with_no_client_auth()
        }
    };
    Ok(Arc::new(config))
}

/// Une requête au relais (`Connection: close`, corps JSON facultatif).
pub fn request(relay: &RelayAddr, method: &str, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Reply> {
    let config = config(relay.pin)?;
    let addrs: Vec<SocketAddr> = (relay.host.as_str(), relay.port).to_socket_addrs().with_context(|| format!("{} introuvable", relay.host))?.collect();
    let tcp = addrs
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, timeout).ok())
        .with_context(|| format!("{} injoignable", relay.web_authority()))?;
    tcp.set_read_timeout(Some(timeout))?;
    tcp.set_write_timeout(Some(timeout))?;
    let name = match relay.host.parse::<IpAddr>() {
        Ok(ip) => ServerName::IpAddress(ip.into()),
        Err(_) => ServerName::try_from(relay.host.clone()).context("nom d'hôte invalide")?,
    };
    let conn = rustls::ClientConnection::new(config, name).context("connexion TLS")?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n", relay.web_authority(), payload.len());
    if body.is_some() {
        head.push_str("Content-Type: application/json\r\n");
    }
    head.push_str("\r\n");
    tls.write_all(head.as_bytes())?;
    tls.write_all(payload.as_bytes())?;
    let mut raw = Vec::new();
    match tls.read_to_end(&mut raw) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {}
        Err(e) => return Err(e).context("lecture de la réponse du relais"),
    }
    parse_reply(&raw)
}

pub fn parse_reply(raw: &[u8]) -> Result<Reply> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").context("réponse HTTP incomplète")?;
    let head = std::str::from_utf8(&raw[..end]).context("en-têtes illisibles")?;
    let status: u16 = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).context("ligne d'état illisible")?;
    let chunked = head.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = &raw[end + 4..];
    Ok(Reply { status, body: if chunked { dechunk(body) } else { body.to_vec() } })
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = rest.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(std::str::from_utf8(&rest[..line_end]).unwrap_or("0").split(';').next().unwrap_or("0").trim(), 16).unwrap_or(0);
        rest = &rest[line_end + 2..];
        if size == 0 || rest.len() < size {
            out.extend_from_slice(&rest[..size.min(rest.len())]);
            break;
        }
        out.extend_from_slice(&rest[..size]);
        rest = rest.get(size + 2..).unwrap_or_default();
    }
    out
}

#[derive(Debug)]
struct Pinned {
    fingerprint: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(&self, end_entity: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        let seen: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();
        if seen == self.fingerprint { Ok(ServerCertVerified::assertion()) } else { Err(rustls::Error::General("certificat du relais inattendu".into())) }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

// --- « Autre relais… » ----------------------------------------------------------------

/// Ce que l'installateur sait d'un relais saisi (`/v1/health`, `/v1/keys`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayProbe {
    pub address: String,
    pub ok: bool,
    pub version: Option<String>,
    pub accounts: bool,
    pub error: Option<String>,
}

pub fn probe(input: &str) -> RelayProbe {
    let Some(relay) = RelayAddr::parse(input) else {
        return RelayProbe { address: String::new(), ok: false, version: None, accounts: false, error: Some("bad_address".into()) };
    };
    let address = relay.to_string();
    let timeout = Duration::from_secs(6);
    let fail = |e: String| RelayProbe { address: address.clone(), ok: false, version: None, accounts: false, error: Some(e) };
    let health: Value = match request(&relay, "GET", "/v1/health", None, timeout) {
        Ok(r) if r.status == 200 => serde_json::from_slice(&r.body).unwrap_or_default(),
        Ok(r) => return fail(format!("HTTP {}", r.status)),
        Err(e) => return fail(format!("{e:#}")),
    };
    if health["ok"].as_bool() != Some(true) {
        return fail("ce n'est pas un relais KyberNow".into());
    }
    let accounts = request(&relay, "GET", "/v1/keys", None, timeout)
        .ok()
        .filter(|r| r.status == 200)
        .and_then(|r| serde_json::from_slice::<Value>(&r.body).ok())
        .is_some_and(|v| v["keys"].as_array().is_some_and(|k| !k.is_empty()));
    RelayProbe { address, ok: true, version: health["version"].as_str().map(str::to_string), accounts, error: None }
}

// --- La session d'appareil -------------------------------------------------------------

/// Ce que rend `POST /v1/devices/token`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceSession {
    pub token: String,
    pub device: Named,
    pub account: AccountInfo,
    /// Le nom du relais (celui de ses jetons).
    #[serde(default)]
    pub relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
}

/// Échange le code du retour contre la session d'appareil.
pub fn exchange(relay: &RelayAddr, code: &str) -> Result<DeviceSession> {
    let reply = request(relay, "POST", "/v1/devices/token", Some(&json!({ "code": code })), Duration::from_secs(15))?;
    if reply.status != 200 {
        let v: Value = serde_json::from_slice(&reply.body).unwrap_or_default();
        bail!("le relais refuse le code ({}, {})", reply.status, v["error"].as_str().unwrap_or("?"));
    }
    let mut session: DeviceSession = serde_json::from_slice(&reply.body).context("réponse du relais illisible")?;
    if session.token.is_empty() {
        bail!("le relais n'a pas donné de session");
    }
    if session.relay.is_empty() {
        session.relay = relay.web_authority();
    }
    Ok(session)
}

/// L'entrée de `accounts.json` (sans le jeton, qui est dans le Gestionnaire d'identification).
pub fn accounts_entry(relay: &RelayAddr, session: &DeviceSession, now: u64) -> Value {
    json!({
        "relay": relay.to_string(),
        "account": session.account,
        "device": session.device,
        "added": now,
    })
}

/// Ajoute (ou remplace, même relais) l'entrée dans le contenu de `accounts.json`.
pub fn merge_accounts(existing: Option<&str>, entry: Value) -> Value {
    let mut doc: Value = existing.and_then(|t| serde_json::from_str(t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    let relay = entry["relay"].clone();
    let mut list: Vec<Value> = doc["accounts"].as_array().cloned().unwrap_or_default();
    list.retain(|a| a["relay"] != relay);
    list.push(entry);
    doc["accounts"] = Value::Array(list);
    doc
}

/// La cible du Gestionnaire d'identification d'un relais.
pub fn credential_target(relay: &RelayAddr) -> String {
    format!("{CREDENTIAL_PREFIX}{}", relay.web_authority())
}

/// Remet la session à l'app : le jeton dans le Gestionnaire d'identification,
/// le reste dans `%APPDATA%\KyberNow\accounts.json`.
#[cfg(windows)]
pub fn store(relay: &RelayAddr, session: &DeviceSession) -> Result<()> {
    write_credential(&credential_target(relay), &session.account.email, &session.token)?;
    let dir = std::env::var_os("APPDATA").map(std::path::PathBuf::from).context("APPDATA absent")?.join(crate::sys::PRODUCT);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(ACCOUNTS_FILE);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let doc = merge_accounts(std::fs::read_to_string(&path).ok().as_deref(), accounts_entry(relay, session, now));
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&doc)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(windows)]
fn write_credential(target: &str, user: &str, secret: &str) -> Result<()> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::Security::Credentials::{CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW};
    let mut target = crate::sys::wide(target);
    let mut user = crate::sys::wide(user);
    let blob: Vec<u8> = secret.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let cred = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        Comment: std::ptr::null_mut(),
        LastWritten: FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 },
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: std::ptr::null_mut(),
        UserName: user.as_mut_ptr(),
    };
    if unsafe { CredWriteW(&cred, 0) } == 0 {
        bail!("jeton non enregistré dans le Gestionnaire d'identification ({})", std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresses() {
        let r = RelayAddr::parse("share.kybernow.app").unwrap();
        assert_eq!(r.origin(), "https://share.kybernow.app");
        let p = RelayAddr::parse(&format!("relais.exemple.org:9443#sha256:{}", ["AB"; 32].join(":"))).unwrap();
        assert_eq!(p.web_authority(), "relais.exemple.org:9443");
        assert_eq!(p.pin, Some([0xab; 32]));
        assert!(p.to_string().ends_with(&["ab"; 32].join(":")));
        for bad in ["", "a b", "relais:x", "relais#sha256:12"] {
            assert!(RelayAddr::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn pages_et_retour() {
        let r = RelayAddr::parse("share.kybernow.app").unwrap();
        assert_eq!(
            login_url(&r, "PC de Léa", "abc", "fr"),
            "https://share.kybernow.app/compte/connexion?appareil=PC%20de%20L%C3%A9a&retour=kybernow%3A%2F%2Fcompte&etat=abc&lang=fr"
        );
        assert_eq!(parse_return("kybernow://compte?code=XYZ&etat=abc"), Some(Ok(("XYZ".into(), "abc".into()))));
        // Windows écrit parfois une barre avant le « ? ».
        assert_eq!(parse_return("kybernow://compte/?code=X%2BY&etat=a"), Some(Ok(("X+Y".into(), "a".into()))));
        assert_eq!(parse_return("kybernow://compte?erreur=annule&etat=abc"), Some(Err("annule".into())));
        assert_eq!(parse_return("https://share.kybernow.app/"), None);
        let origin = r.origin();
        assert_eq!(navigation("https://share.kybernow.app/compte/connexion", &origin), Navigation::Allow);
        assert!(matches!(navigation("kybernow://compte?code=1", &origin), Navigation::Return(_)));
        assert_eq!(navigation("https://share.kybernow.app.exemple.org/", &origin), Navigation::External);
        assert_eq!(navigation("file:///C:/", &origin), Navigation::Block);
        assert_eq!(new_state().len(), 32);
        assert_ne!(new_state(), new_state());
    }

    #[test]
    fn session_et_fichier_des_comptes() {
        let s: DeviceSession = serde_json::from_str(
            r#"{"token":"t0k","device":{"id":"dev_1","name":"PC-BUREAU"},"account":{"id":"acc_1","name":"Léa","email":"lea@exemple.fr"},"relay":"share.kybernow.app"}"#,
        )
        .unwrap();
        let r = RelayAddr::parse("share.kybernow.app").unwrap();
        assert_eq!(credential_target(&r), "KyberNow:account:share.kybernow.app");
        let entry = accounts_entry(&r, &s, 1795000000);
        assert!(entry.get("token").is_none(), "le jeton ne va pas dans le fichier");
        let doc = merge_accounts(Some(r#"{"accounts":[{"relay":"share.kybernow.app","account":{"id":"vieux"}},{"relay":"autre.test"}],"v":1}"#), entry.clone());
        let list = doc["accounts"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1]["account"]["id"], "acc_1");
        assert_eq!(doc["v"], 1, "le reste du fichier est gardé");
        assert_eq!(merge_accounts(Some("pas du json"), entry)["accounts"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn reponse_decoupee() {
        let r = parse_reply(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{\"a\r\n3\r\n\":1\r\n1\r\n}\r\n0\r\n\r\n").unwrap();
        assert_eq!(r.body, b"{\"a\":1}");
    }
}
