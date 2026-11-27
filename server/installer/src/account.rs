//! Étape « Compte KyberNow » des installateurs de la Station (Windows et Mac)
//! et enrôlement de l'installation silencieuse (docs/comptes.md, « Dans les
//! installateurs »).
//!
//! - **Relais** : `share.kybernow.app` proposé ; « Autre relais… » est vérifié
//!   en direct ([`probe`] : `GET /v1/health` pour la version, `GET /v1/keys`
//!   pour les comptes).
//! - **Rattachement** : dès qu'Admin a démarré (tâche « admin » finie), le
//!   relais choisi est réglé et le rattachement demandé par les routes
//!   locales d'Admin (`/api/local/account…`, jeton local, certificat
//!   épinglé) ; la page du relais s'affiche dans la fenêtre de l'installateur
//!   (vue enfant, posée par l'hôte), puis l'installateur suit Admin jusqu'au
//!   rattachement.
//! - **Enrôlement** ([`enroll`]) : le jeton passe à Admin, qui le donne au
//!   relais par sa connexion de contrôle (`enroll`).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::http::Client;

/// Le relais public de KyberNow, proposé d'office.
pub const DEFAULT_RELAY: &str = "share.kybernow.app";
/// Admin met un peu de temps à répondre après son démarrage : on réessaie ce temps.
const ADMIN_PATIENCE: Duration = Duration::from_secs(120);
/// Le code de rattachement du relais vaut 15 minutes.
const ATTACH_PATIENCE: Duration = Duration::from_secs(15 * 60);
const POLL_EVERY: Duration = Duration::from_secs(2);

// --- Adresse d'un relais ------------------------------------------------------------

/// `hôte[:port][#sha256:…]` (docs/relais.md) : sans port, le 443 ; avec une
/// empreinte, un relais signé par lui-même.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayAddr {
    pub host: String,
    pub port: u16,
    /// `sha256:aa:bb:…`, en minuscules.
    pub pin: Option<String>,
}

impl RelayAddr {
    pub fn parse(input: &str) -> Option<RelayAddr> {
        let s = input.trim();
        let s = s.strip_prefix("https://").unwrap_or(s).trim_end_matches('/');
        let (main, pin) = match s.split_once('#') {
            Some((m, p)) => (m, Some(normalize_pin(p)?)),
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

    /// `hôte`, ou `hôte:port` hors du 443 (crochets pour IPv6).
    pub fn web_authority(&self) -> String {
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        if self.port == 443 { host } else { format!("{host}:{}", self.port) }
    }

    pub fn is_default(&self) -> bool {
        self.pin.is_none() && self.web_authority() == DEFAULT_RELAY
    }
}

impl std::fmt::Display for RelayAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.pin {
            Some(pin) => write!(f, "{}#{pin}", self.web_authority()),
            None => f.write_str(&self.web_authority()),
        }
    }
}

/// `sha256:aa:bb:…` (deux-points facultatifs, majuscules permises).
fn normalize_pin(text: &str) -> Option<String> {
    let bytes = crate::http::parse_fingerprint(text)?;
    Some(format!("sha256:{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")))
}

// --- Vérification en direct (« Autre relais… ») ------------------------------------------

/// Ce que l'installateur sait d'un relais saisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayProbe {
    /// L'adresse sous sa forme normale (vide : illisible).
    pub address: String,
    /// Le relais a répondu (`/v1/health`).
    pub ok: bool,
    pub version: Option<String>,
    /// Il tient des comptes KyberNow (`/v1/keys`).
    pub accounts: bool,
    /// `bad_address`, ou la raison en clair d'un relais injoignable.
    pub error: Option<String>,
}

/// Joint le relais : version (`GET /v1/health`), comptes (`GET /v1/keys`,
/// absent d'un relais sans comptes).
pub fn probe(input: &str) -> RelayProbe {
    let Some(addr) = RelayAddr::parse(input) else {
        return RelayProbe { address: String::new(), ok: false, version: None, accounts: false, error: Some("bad_address".into()) };
    };
    let address = addr.to_string();
    let unreachable = |e: anyhow::Error| RelayProbe { address: address.clone(), ok: false, version: None, accounts: false, error: Some(format!("{e:#}")) };
    let client = match Client::relay(addr.pin.as_deref()) {
        Ok(c) => c,
        Err(e) => return unreachable(e),
    };
    let timeout = Duration::from_secs(6);
    let health = match client.get(&addr.host, addr.port, "/v1/health", timeout) {
        Ok(r) if r.status == 200 => serde_json::from_slice::<Value>(&r.body).unwrap_or_default(),
        Ok(r) => return unreachable(anyhow::anyhow!("HTTP {}", r.status)),
        Err(e) => return unreachable(e),
    };
    if health["ok"].as_bool() != Some(true) {
        return unreachable(anyhow::anyhow!("ce n'est pas un relais KyberNow"));
    }
    let accounts = client
        .get(&addr.host, addr.port, "/v1/keys", timeout)
        .ok()
        .filter(|r| r.status == 200)
        .and_then(|r| serde_json::from_slice::<Value>(&r.body).ok())
        .is_some_and(|v| v["keys"].as_array().is_some_and(|k| !k.is_empty()));
    RelayProbe { address, ok: true, version: health["version"].as_str().map(str::to_string), accounts, error: None }
}

// --- Admin, par ses routes locales -----------------------------------------------------

/// Admin installé : port, certificat épinglé et jeton local.
pub struct Admin {
    client: Client,
    port: u16,
    token: String,
    /// `installer-token` d'Admin (vide pour un Admin d'avant 0.11.8, qui ne le demande pas).
    installer: String,
}

/// Une réponse d'Admin qui n'est pas un succès : le `code` des refus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Refusal {}

impl Admin {
    /// L'Admin de cette installation (`kybernow.json` et `local-token`).
    pub fn locate() -> Result<Admin> {
        let (fingerprint, port, token_file) = locate_files()?;
        let token = std::fs::read_to_string(&token_file).map(|t| t.trim().to_string()).with_context(|| format!("jeton local illisible ({})", token_file.display()))?;
        // Jeton de l'installateur, à côté (dossier réservé aux administrateurs) :
        // Admin l'exige pour rattacher la Station ou changer son relais.
        let installer_file = token_file.with_file_name("installer-token");
        let installer = std::fs::read_to_string(&installer_file).map(|t| t.trim().to_string()).unwrap_or_default();
        Ok(Admin { client: Client::pinned(&fingerprint)?, port, token, installer })
    }

    fn call(&self, method: &str, path: &str, body: Option<Value>, timeout: Duration) -> Result<Value> {
        let bytes = body.map(|b| b.to_string().into_bytes());
        let reply = self.client.send(method, self.port, path, &[("x-kyber-local", &self.token), ("x-kyber-installer", &self.installer)], bytes.as_deref(), timeout)?;
        let v: Value = serde_json::from_slice(&reply.body).unwrap_or_default();
        if reply.status != 200 {
            let code = v["code"].as_str().unwrap_or(if reply.status == 404 { "not_found" } else { "refused" }).to_string();
            let message = v["error"].as_str().unwrap_or("KyberNow Admin a refusé").to_string();
            return Err(Refusal { status: reply.status, code, message }.into());
        }
        Ok(v)
    }

    pub fn state(&self) -> Result<Value> {
        self.call("GET", "/api/local/account", None, Duration::from_secs(5))
    }

    pub fn set_relay(&self, relay: &str) -> Result<Value> {
        self.call("PUT", "/api/local/account/relay", Some(json!({ "relay": relay })), Duration::from_secs(5))
    }

    /// Le rattachement : la page du relais à ouvrir (`{url, expires}`).
    pub fn attach(&self, relay: &str) -> Result<Value> {
        self.call("POST", "/api/local/account/attach", Some(json!({ "relay": relay })), Duration::from_secs(20))
    }

    pub fn cancel(&self) -> Result<Value> {
        self.call("DELETE", "/api/local/account/attach", None, Duration::from_secs(5))
    }

    pub fn enroll(&self, token: &str, relay: Option<&str>) -> Result<Value> {
        self.call("POST", "/api/local/account/enroll", Some(json!({ "token": token, "relay": relay })), Duration::from_secs(30))
    }
}

#[cfg(target_os = "macos")]
fn locate_files() -> Result<(String, u16, std::path::PathBuf)> {
    let record = crate::mac::read_record().context("KyberNow Station n'est pas installée")?;
    let fingerprint = record.console_fingerprint.context("certificat d'Admin inconnu (Admin n'a pas encore démarré)")?;
    Ok((fingerprint, record.console_port, crate::mac::admin_dir().join("local-token")))
}

#[cfg(not(target_os = "macos"))]
fn locate_files() -> Result<(String, u16, std::path::PathBuf)> {
    let record = crate::install::read_record(&crate::sys::install_dir()).context("KyberNow Station n'est pas installée")?;
    let fingerprint = record.console_fingerprint.context("certificat d'Admin inconnu (Admin n'a pas encore démarré)")?;
    Ok((fingerprint, record.console_port, crate::sys::data_dir().join("local-token")))
}

/// Refus d'Admin qu'il ne sert à rien de réessayer (le reste : Admin pas
/// encore prêt, ou une version d'avant ces routes, `404`).
fn final_refusal(e: &anyhow::Error) -> Option<&Refusal> {
    e.downcast_ref::<Refusal>().filter(|r| !matches!(r.status, 401 | 403 | 404) || r.code == "bad_token")
}

/// Réessaie `f` tant qu'Admin ne répond pas (ou pas encore ces routes),
/// `patience` au plus, et tant que `alive()`.
fn with_admin<T>(patience: Duration, alive: &dyn Fn() -> bool, f: &dyn Fn(&Admin) -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + patience;
    loop {
        let result = Admin::locate().and_then(|a| f(&a));
        match result {
            Ok(v) => return Ok(v),
            Err(e) if final_refusal(&e).is_some() => return Err(e),
            Err(e) if Instant::now() >= deadline || !alive() => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_secs(1)),
        }
    }
}

// --- Messages avec la fenêtre ---------------------------------------------------------

/// Ce que l'étape « Compte KyberNow » envoie à la fenêtre (même canal que
/// les autres événements de l'installateur).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AccountEvent {
    /// Réponse à `relayProbe` (`input` : ce qui a été saisi).
    RelayProbed {
        input: String,
        #[serde(flatten)]
        probe: RelayProbe,
    },
    /// La page du relais attend qu'Admin ait démarré.
    AccountWaiting,
    /// La page du relais à afficher ; `external` : ouverte dans le navigateur
    /// (relais signé par lui-même : la vue de la fenêtre le refuserait).
    AccountPage { url: String, relay: String, external: bool },
    /// Rattachée : à qui, et ce qui s'est allumé.
    AccountAttached { relay: String, owner: String, email: Option<String>, organization: bool, accept: bool, remote_access: bool },
    /// Le relais choisi est réglé (« Plus tard »).
    AccountRelaySet { relay: String },
    AccountFailed { code: String, message: String },
}

/// Ce que la fenêtre demande pour l'étape « Compte KyberNow ».
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AccountCommand {
    RelayProbe { address: String },
    /// « Se connecter » / « Créer un compte » : rattacher dès qu'Admin a démarré.
    AccountStart { relay: String },
    /// « Plus tard » : seulement le relais choisi, s'il n'est pas celui d'office.
    AccountLater { relay: String },
    /// « Retour » depuis la page du relais.
    AccountCancel,
}

/// Ce que la fenêtre demande à l'hôte pour la page du relais (vue enfant).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ViewCommand {
    /// Afficher `url` dans le cadre donné (points CSS, depuis le coin de la
    /// fenêtre) ; `signup` : aller à l'inscription plutôt qu'à la connexion.
    AccountView { url: String, x: f64, y: f64, width: f64, height: f64, #[serde(default)] signup: bool },
    AccountViewHide,
}

/// La page du relais ne quitte pas le relais : tout autre lien part dans le
/// navigateur. Avec `signup`, la première page de connexion est remplacée par
/// l'inscription (le cookie de suite, posé par le relais, ramène ensuite au
/// rattachement). Rend l'adresse à charger à la place, le cas échéant.
pub fn view_navigation(url: &str, origin: &str, signup: &AtomicBool) -> Navigation {
    let same = url == origin || url.starts_with(&format!("{origin}/")) || url.starts_with(&format!("{origin}?"));
    if !same {
        return if url.starts_with("https://") || url.starts_with("http://") { Navigation::External } else { Navigation::Block };
    }
    let path = &url[origin.len()..];
    if path.starts_with("/compte/connexion") && signup.swap(false, Ordering::SeqCst) {
        return Navigation::Redirect(format!("{origin}/compte/inscription"));
    }
    Navigation::Allow
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Navigation {
    Allow,
    /// Ouvrir dans le navigateur, pas dans la fenêtre.
    External,
    Block,
    Redirect(String),
}

/// `https://hôte[:port]` d'une adresse (l'origine de la page du relais).
pub fn origin_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    (end > 0).then(|| format!("https://{}", &rest[..end]))
}

// --- Le déroulé, côté installateur ------------------------------------------------------

pub type Emit = Arc<dyn Fn(AccountEvent) + Send + Sync>;

/// Mène l'étape pendant l'installation : attend qu'Admin ait démarré, puis
/// règle le relais, rattache et suit le rattachement. Une seule demande à la
/// fois : une nouvelle annule la précédente.
#[derive(Clone)]
pub struct Coordinator {
    emit: Emit,
    ready: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
}

impl Coordinator {
    pub fn new(emit: Emit) -> Coordinator {
        Coordinator { emit, ready: Arc::default(), generation: Arc::default() }
    }

    /// Admin a démarré (tâche « admin » finie) : la demande en attente part.
    pub fn admin_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    pub fn is_admin_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub fn handle(&self, command: AccountCommand) {
        match command {
            AccountCommand::RelayProbe { address } => {
                let emit = self.emit.clone();
                std::thread::spawn(move || emit(AccountEvent::RelayProbed { probe: probe(&address), input: address }));
            }
            AccountCommand::AccountStart { relay } => {
                let (me, generation) = self.next();
                std::thread::spawn(move || me.attach(generation, &relay));
            }
            AccountCommand::AccountLater { relay } => {
                let (me, generation) = self.next();
                std::thread::spawn(move || me.later(generation, &relay));
            }
            AccountCommand::AccountCancel => {
                let (me, generation) = self.next();
                std::thread::spawn(move || {
                    if me.wait_ready(generation, false) {
                        let _ = Admin::locate().and_then(|a| a.cancel());
                    }
                });
            }
        }
    }

    fn next(&self) -> (Coordinator, u64) {
        (self.clone(), self.generation.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn alive(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    fn send(&self, generation: u64, event: AccountEvent) {
        if self.alive(generation) {
            (self.emit)(event);
        }
    }

    /// Attend qu'Admin ait démarré ; faux si la demande a été remplacée.
    fn wait_ready(&self, generation: u64, announce: bool) -> bool {
        if !self.is_admin_ready() && announce {
            self.send(generation, AccountEvent::AccountWaiting);
        }
        while !self.is_admin_ready() {
            if !self.alive(generation) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        self.alive(generation)
    }

    fn fail(&self, generation: u64, e: &anyhow::Error) {
        let (code, message) = match e.downcast_ref::<Refusal>() {
            Some(r) => (r.code.clone(), r.message.clone()),
            None => ("admin_unreachable".into(), format!("{e:#}")),
        };
        self.send(generation, AccountEvent::AccountFailed { code, message });
    }

    fn attach(&self, generation: u64, relay: &str) {
        let Some(addr) = RelayAddr::parse(relay) else {
            return self.send(generation, AccountEvent::AccountFailed { code: "bad_relay".into(), message: relay.into() });
        };
        if !self.wait_ready(generation, true) {
            return;
        }
        let alive = || self.alive(generation);
        let relay = addr.to_string();
        let page = match with_admin(ADMIN_PATIENCE, &alive, &|a| a.attach(&relay)) {
            Ok(v) => v,
            Err(e) => return self.fail(generation, &e),
        };
        let Some(url) = page["url"].as_str().filter(|u| u.starts_with("https://")) else {
            return self.send(generation, AccountEvent::AccountFailed { code: "relay_unreachable".into(), message: page.to_string() });
        };
        self.send(generation, AccountEvent::AccountPage { url: url.to_string(), relay: addr.web_authority(), external: addr.pin.is_some() });
        self.follow(generation);
    }

    /// Suit Admin jusqu'au rattachement (15 minutes au plus).
    fn follow(&self, generation: u64) {
        let deadline = Instant::now() + ATTACH_PATIENCE;
        while self.alive(generation) && Instant::now() < deadline {
            std::thread::sleep(POLL_EVERY);
            let Ok(state) = Admin::locate().and_then(|a| a.state()) else { continue };
            if state["attached"].as_bool() == Some(true) {
                return self.send(generation, attached_event(&state));
            }
            if state["attaching"].as_bool() == Some(false) {
                // Abandonné dans Admin, ou Admin redémarré : le code ne sera plus pris.
                return self.send(generation, AccountEvent::AccountFailed { code: "expired".into(), message: String::new() });
            }
        }
        if self.alive(generation) {
            self.send(generation, AccountEvent::AccountFailed { code: "expired".into(), message: String::new() });
        }
    }

    fn later(&self, generation: u64, relay: &str) {
        let addr = RelayAddr::parse(relay);
        if !self.wait_ready(generation, false) {
            return;
        }
        let alive = || self.alive(generation);
        let _ = with_admin(Duration::from_secs(20), &alive, &|a| a.cancel());
        if let Some(addr) = addr.filter(|a| !a.is_default()) {
            let relay = addr.to_string();
            match with_admin(ADMIN_PATIENCE, &alive, &|a| a.set_relay(&relay)) {
                Ok(_) => self.send(generation, AccountEvent::AccountRelaySet { relay: addr.web_authority() }),
                Err(e) => self.fail(generation, &e),
            }
        }
    }
}

/// `AccountAttached` à partir de la vue d'Admin (`GET /api/local/account`).
pub fn attached_event(state: &Value) -> AccountEvent {
    let owner = &state["owner"];
    AccountEvent::AccountAttached {
        relay: state["relay"].as_str().unwrap_or_default().to_string(),
        owner: owner["name"].as_str().unwrap_or_default().to_string(),
        email: owner["email"].as_str().filter(|e| !e.is_empty()).map(str::to_string),
        organization: owner["kind"].as_str() == Some("organization"),
        accept: state["accept"].as_bool().unwrap_or(false),
        remote_access: state["remote_access"].as_bool().unwrap_or(false),
    }
}

// --- Enrôlement (installation silencieuse) ------------------------------------------------

/// Enrôle la Station installée : le jeton part à Admin, qui le passe au
/// relais (`enroll`). Admin vient de démarrer : on lui laisse `patience`.
pub fn enroll(token: &str, relay: Option<&str>, patience: Duration) -> Result<Value> {
    let relay = match relay {
        Some(r) => Some(RelayAddr::parse(r).with_context(|| format!("adresse de relais illisible : {r}"))?.to_string()),
        None => None,
    };
    let token = token.trim();
    if token.is_empty() {
        bail!("jeton d'enrôlement vide");
    }
    with_admin(patience, &|| true, &|a| a.enroll(token, relay.as_deref()))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn adresses_de_relais() {
        let a = RelayAddr::parse("share.kybernow.app").unwrap();
        assert!(a.is_default());
        assert_eq!(a.to_string(), "share.kybernow.app");
        let b = RelayAddr::parse(" https://Relais.Exemple.org:9443/ ").unwrap();
        assert_eq!((b.host.as_str(), b.port), ("relais.exemple.org", 9443));
        assert_eq!(b.to_string(), "relais.exemple.org:9443");
        assert!(!b.is_default());
        assert_eq!(RelayAddr::parse("share.kybernow.app:443").unwrap().to_string(), "share.kybernow.app");
        let pin = ["AB"; 32].join(":");
        let c = RelayAddr::parse(&format!("192.168.1.10:8443#sha256:{pin}")).unwrap();
        assert_eq!(c.to_string(), format!("192.168.1.10:8443#sha256:{}", ["ab"; 32].join(":")));
        let v6 = RelayAddr::parse("[2001:db8::1]:9443").unwrap();
        assert_eq!(v6.web_authority(), "[2001:db8::1]:9443");
        for bad in ["", "relais exemple", "relais:port", "relais:0", "relais#sha256:12", "a/b", "2001:db8::1"] {
            assert_eq!(RelayAddr::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn adresse_illisible_sans_reseau() {
        let p = probe("pas une adresse");
        assert_eq!(p.error.as_deref(), Some("bad_address"));
        assert!(!p.ok);
    }

    #[test]
    fn la_page_du_relais_reste_sur_le_relais() {
        let origin = origin_of("https://share.kybernow.app/compte/rattacher?code=abc").unwrap();
        assert_eq!(origin, "https://share.kybernow.app");
        assert_eq!(origin_of("https://relais.exemple.org:9443?x").as_deref(), Some("https://relais.exemple.org:9443"));
        let signup = AtomicBool::new(false);
        assert_eq!(view_navigation("https://share.kybernow.app/compte/connexion", &origin, &signup), Navigation::Allow);
        assert_eq!(view_navigation("https://share.kybernow.app.evil.test/", &origin, &signup), Navigation::External);
        assert_eq!(view_navigation("https://www.leslignesclaires.fr/", &origin, &signup), Navigation::External);
        assert_eq!(view_navigation("javascript:alert(1)", &origin, &signup), Navigation::Block);
        // « Créer un compte » : la connexion demandée par le rattachement devient l'inscription, une fois.
        let signup = AtomicBool::new(true);
        assert_eq!(view_navigation("https://share.kybernow.app/compte/rattacher?code=abc", &origin, &signup), Navigation::Allow);
        assert_eq!(
            view_navigation("https://share.kybernow.app/compte/connexion", &origin, &signup),
            Navigation::Redirect("https://share.kybernow.app/compte/inscription".into())
        );
        assert_eq!(view_navigation("https://share.kybernow.app/compte/connexion", &origin, &signup), Navigation::Allow);
    }

    #[test]
    fn messages_de_la_fenetre() {
        let c: AccountCommand = serde_json::from_str(r#"{"cmd":"accountStart","relay":"share.kybernow.app"}"#).unwrap();
        assert_eq!(c, AccountCommand::AccountStart { relay: "share.kybernow.app".into() });
        assert!(serde_json::from_str::<AccountCommand>(r#"{"cmd":"install"}"#).is_err());
        let v: ViewCommand = serde_json::from_str(r#"{"cmd":"accountView","url":"https://x","x":1,"y":2,"width":3,"height":4}"#).unwrap();
        assert!(matches!(v, ViewCommand::AccountView { signup: false, .. }));
        let probed = AccountEvent::RelayProbed {
            input: "x".into(),
            probe: RelayProbe { address: "x".into(), ok: true, version: Some("0.9.10".into()), accounts: true, error: None },
        };
        let text = serde_json::to_string(&probed).unwrap();
        assert_eq!(text, r#"{"type":"relayProbed","input":"x","address":"x","ok":true,"version":"0.9.10","accounts":true,"error":null}"#);
        let state = json!({ "attached": true, "relay": "share.kybernow.app", "accept": true, "remote_access": true,
            "owner": { "kind": "organization", "id": "org_1", "name": "Les Lignes Claires", "email": null } });
        let a = serde_json::to_value(attached_event(&state)).unwrap();
        assert_eq!(a["type"], "accountAttached");
        assert_eq!(a["organization"], true);
        assert_eq!(a["remoteAccess"], true);
    }

    /// Une demande remplacée ne parle plus : « Plus tard » après « Se connecter ».
    #[test]
    fn une_demande_remplacee_se_tait() {
        let seen: Arc<Mutex<Vec<AccountEvent>>> = Arc::default();
        let s2 = seen.clone();
        let c = Coordinator::new(Arc::new(move |e| s2.lock().unwrap().push(e)));
        c.handle(AccountCommand::AccountStart { relay: "share.kybernow.app".into() });
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(*seen.lock().unwrap(), vec![AccountEvent::AccountWaiting]);
        c.handle(AccountCommand::AccountLater { relay: "share.kybernow.app".into() });
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(seen.lock().unwrap().len(), 1);
        c.handle(AccountCommand::AccountStart { relay: "pas une adresse".into() });
        std::thread::sleep(Duration::from_millis(100));
        assert!(matches!(seen.lock().unwrap().last(), Some(AccountEvent::AccountFailed { code, .. }) if code == "bad_relay"));
    }
}
