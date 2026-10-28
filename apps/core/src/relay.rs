//! Le relais vu du cœur (docs/relais.md) : un tunnel vers une Station, qui
//! expose Admin et Kyber sur la boucle locale de l'appareil (`127.0.0.1`, en
//! TCP et en UDP sur le même numéro pour Kyber) ; Admin y est épinglé sur
//! son empreinte, Kyber sur la sienne, jamais de premier contact. kyclient
//! et `ureq` y passent sans rien savoir du relais.
//!
//! - [`open`] : un tunnel par (relais, Station), gardé dans un cache et fermé
//!   30 s après son dernier usage ([`RelayTunnel`] tenu, flux en cours) ;
//! - [`lookup`] : la Station d'un code de partage chez un relais ;
//! - [`presence`] : quelles Stations sont joignables par le relais (accueil) ;
//! - [`RouteInfo`] : le chemin d'une session, pour la puce « Par le relais »
//!   et la ligne « Chemin » des statistiques.
//!
//! Le tunnel (`kybernow-tunnel`) est tokio : le cœur tient son propre
//! runtime, deux fils (`kybernow-relais`), créé au premier usage.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::i18n::{self, Lang, tr};

pub use kybernow_tunnel::lookup::CodeLookup;
pub use kybernow_tunnel::{DEFAULT_RELAY, RelayAddr, Transport, TunnelError};

use kybernow_tunnel::{ClientConfig, LocalListener, Tunnel as KyTunnel};

/// Un tunnel sans usage depuis ce délai est fermé.
pub const IDLE_CLOSE: Duration = Duration::from_secs(30);
/// La Station est attendue chez le relais au plus ce temps à l'ouverture.
pub const STATION_WAIT: Duration = Duration::from_secs(3);

// MARK: - Où passer

/// Une Station par le relais : `server` (le relais, `share.kybernow.app`,
/// `hôte:port` ou `hôte:port#sha256:…` ; sans port, le 443), `station` (son identifiant chez le relais, 26
/// caractères), `admin_fp` (l'empreinte d'Admin, obligatoire : jamais de
/// premier contact par le relais).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelayRoute {
    pub server: String,
    pub station: String,
    pub admin_fp: String,
}

impl RelayRoute {
    /// L'adresse du relais, et une route complète (identifiant de Station
    /// valable, empreinte d'Admin donnée).
    pub fn relay_addr(&self) -> Result<RelayAddr, RelayError> {
        if !kybernow_tunnel::id::is_station_id(&self.station) {
            return Err(RelayError::Bad(format!("identifiant de Station illisible : {}", self.station)));
        }
        if self.admin_fp.trim().is_empty() {
            return Err(RelayError::Bad("empreinte d'Admin absente".into()));
        }
        self.server.parse::<RelayAddr>().map_err(|e| RelayError::Bad(e.to_string()))
    }
}

/// `info.relay` (Station 0.8.0 et suivantes, relais réglé) : à garder pour
/// chaque ordinateur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayInfo {
    /// Le relais de la Station (`share.kybernow.app`, ou `hôte:port`).
    #[serde(default)]
    pub server: String,
    /// Son identifiant chez le relais.
    #[serde(default)]
    pub id: String,
    /// « Accès hors de chez vous » actif dans Admin.
    #[serde(default)]
    pub remote_access: bool,
}

impl RelayInfo {
    /// La route de cette Station par son relais, Admin épinglé sur
    /// `admin_fp` (celle que l'app garde pour l'ordinateur). `None` sans
    /// relais ni identifiant, ou sans empreinte.
    /// Le relais sous sa forme normale ([`normalize`]).
    pub fn route(&self, admin_fp: &str) -> Option<RelayRoute> {
        let route = RelayRoute { server: self.server.clone(), station: self.id.clone(), admin_fp: admin_fp.trim().to_string() };
        let relay = route.relay_addr().ok()?;
        Some(RelayRoute { server: relay.to_string(), ..route })
    }
}

/// Une adresse de relais sous sa forme normale : nom en minuscules, sans
/// schéma, sans port quand c'est le 443 (`share.kybernow.app`), avec lui
/// sinon (`relais.exemple.org:9443`), empreinte épinglée gardée. `None` si
/// elle ne se lit pas. Deux relais sont le même si leurs formes normales
/// sont égales.
pub fn normalize(addr: &str) -> Option<String> {
    addr.parse::<RelayAddr>().ok().map(|a| a.to_string())
}

/// `KYBERNOW_RELAY` : `force` (toujours par le relais, pour essayer à la
/// maison), `off` (jamais), sinon le choix de la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayMode {
    Normal,
    Force,
    Off,
}

impl RelayMode {
    pub fn current() -> Self {
        Self::from_env(std::env::var("KYBERNOW_RELAY").ok().as_deref())
    }

    pub fn from_env(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("force") => Self::Force,
            Some("off") => Self::Off,
            _ => Self::Normal,
        }
    }

    /// Pour les ponts : 0 normal, 1 toujours le relais, 2 jamais.
    pub fn code(self) -> i32 {
        match self {
            Self::Normal => 0,
            Self::Force => 1,
            Self::Off => 2,
        }
    }
}

// MARK: - Erreurs

/// Pourquoi le relais ne sert pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayError {
    /// « Le relais ne répond pas. »
    Unreachable(String),
    /// L'ordinateur n'est pas connecté au relais (éteint, en veille).
    StationOffline,
    /// Le relais refuse (trop de connexions, banni, version).
    Refused(String),
    /// Trop de recherches de code (429) ; `seconds` : l'attente.
    TooMany { seconds: Option<u64> },
    /// Route ou adresse illisible.
    Bad(String),
    /// Admin, joint par le relais, refuse : « Accès hors de chez vous » est
    /// coupé (403 `remote_access_off`).
    AccessOff,
}

impl RelayError {
    /// Le texte à montrer (docs/link-ui.md, « Hors de chez vous ») ; `name` :
    /// le nom de l'ordinateur quand on le connaît.
    pub fn text(&self, lang: Lang, name: Option<&str>) -> String {
        match (self, name.filter(|n| !n.trim().is_empty())) {
            (Self::Unreachable(_), _) => tr(lang, "relay.unreachable", &[]),
            (Self::StationOffline, Some(n)) => tr(lang, "relay.offline_named", &[("name", n)]),
            (Self::StationOffline, None) => tr(lang, "relay.offline", &[]),
            (Self::Refused(why), _) => tr(lang, "relay.refused", &[("reason", why)]),
            (Self::TooMany { .. }, _) => tr(lang, "share.too_many", &[]),
            (Self::Bad(why), _) => tr(lang, "relay.bad", &[("reason", why)]),
            (Self::AccessOff, Some(n)) => tr(lang, "relay.access_off_named", &[("name", n)]),
            (Self::AccessOff, None) => tr(lang, "relay.access_off", &[]),
        }
    }

    /// Nom stable pour les ponts : `unreachable`, `station_offline`,
    /// `access_off`, `refused`, `too_many`, `bad`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unreachable(_) => "unreachable",
            Self::StationOffline => "station_offline",
            Self::AccessOff => "access_off",
            Self::Refused(_) => "refused",
            Self::TooMany { .. } => "too_many",
            Self::Bad(_) => "bad",
        }
    }

    /// Le code des ponts (C, JNI) : 31 ordinateur absent du relais
    /// ([`Self::StationOffline`]), 32 accès coupé dans Admin
    /// ([`Self::AccessOff`]), 30 pour le reste (relais muet, refus du
    /// relais, adresse illisible).
    pub fn code(&self) -> i32 {
        match self {
            Self::StationOffline => CODE_STATION_OFFLINE,
            Self::AccessOff => CODE_ACCESS_OFF,
            _ => CODE_UNREACHABLE,
        }
    }

    /// L'erreur d'un code des ponts (pour en avoir le texte, avec le nom de
    /// l'ordinateur) ; `None` pour un autre code.
    pub fn from_code(code: i32) -> Option<Self> {
        match code {
            CODE_UNREACHABLE => Some(Self::Unreachable(String::new())),
            CODE_STATION_OFFLINE => Some(Self::StationOffline),
            CODE_ACCESS_OFF => Some(Self::AccessOff),
            _ => None,
        }
    }
}

/// Codes des ponts : relais muet (et le reste des erreurs du relais).
pub const CODE_UNREACHABLE: i32 = 30;
/// Codes des ponts : l'ordinateur n'est pas chez le relais (éteint, en veille).
pub const CODE_STATION_OFFLINE: i32 = 31;
/// Codes des ponts : « Accès hors de chez vous » coupé dans Admin.
pub const CODE_ACCESS_OFF: i32 = 32;

impl std::fmt::Display for RelayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(i18n::current(), None))
    }
}

impl std::error::Error for RelayError {}

impl From<TunnelError> for RelayError {
    fn from(e: TunnelError) -> Self {
        match e {
            TunnelError::StationOffline => Self::StationOffline,
            TunnelError::Refused(code, _) => Self::Refused(code.as_str().to_string()),
            other => Self::Unreachable(other.to_string()),
        }
    }
}

// MARK: - Chemin d'une session

/// Par où passe une session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteTransport {
    /// Sur le réseau, sans relais.
    #[default]
    Direct,
    /// Par le relais, en QUIC.
    Relay,
    /// Par le relais, repli TCP (l'UDP ne passe pas) : bon pour regarder,
    /// moins pour jouer.
    RelayTcp,
}

impl RouteTransport {
    /// Pour les ponts : 0 direct, 1 relais, 2 relais en TCP.
    pub fn code(self) -> i32 {
        match self {
            Self::Direct => 0,
            Self::Relay => 1,
            Self::RelayTcp => 2,
        }
    }

    /// La ligne « Chemin » des statistiques : « Direct », « Par le relais »,
    /// « Par le relais (TCP) ».
    pub fn label(self, lang: Lang) -> String {
        match self {
            Self::Direct => tr(lang, "relay.path_direct", &[]),
            Self::Relay => tr(lang, "relay.path_relay", &[]),
            Self::RelayTcp => tr(lang, "relay.path_relay_tcp", &[]),
        }
    }
}

/// Le chemin d'une session ([`crate::session::Platform::on_route`]) : puce
/// « Par le relais », ligne « Chemin » des statistiques, `route` de la
/// « Dernière session ».
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteInfo {
    pub transport: RouteTransport,
    /// Le relais (`share.kybernow.app`), par le relais.
    #[serde(default)]
    pub relay: Option<String>,
    /// Aller-retour appareil ↔ relais, en millisecondes.
    #[serde(default)]
    pub rtt_client: Option<f64>,
    /// Aller-retour relais ↔ Station, en millisecondes.
    #[serde(default)]
    pub rtt_station: Option<f64>,
}

impl RouteInfo {
    pub fn direct() -> Self {
        Self::default()
    }

    pub fn via_relay(&self) -> bool {
        self.transport != RouteTransport::Direct
    }

    /// Les mesures arrondies (0,5 ms) : un changement visible seulement.
    pub(crate) fn rounded(&self) -> Self {
        let r = |v: Option<f64>| v.map(|v| (v * 2.0).round() / 2.0);
        Self { rtt_client: r(self.rtt_client), rtt_station: r(self.rtt_station), ..self.clone() }
    }
}

// MARK: - Runtime

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("kybernow-relais")
            .enable_all()
            .build()
            .expect("runtime du relais")
    })
}

// MARK: - Tunnels

struct Entry {
    tunnel: KyTunnel,
    admin: LocalListener,
    kyber: Mutex<Option<LocalListener>>,
    route: RelayRoute,
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.tunnel.close();
    }
}

struct Cached {
    entry: Arc<Entry>,
    /// Sans usage depuis cet instant (`None` : en usage au dernier passage).
    idle_since: Option<Instant>,
}

/// Les tunnels ouverts, par (relais, Station).
fn cache() -> &'static Mutex<HashMap<(String, String), Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        // Le ménage : toutes les 5 s, un tunnel sans usage depuis 30 s est fermé.
        runtime().spawn(async {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                sweep(Instant::now());
            }
        });
        Mutex::new(HashMap::new())
    })
}

fn sweep(now: Instant) {
    let mut c = cache().lock().unwrap();
    c.retain(|key, cached| {
        // En usage : un RelayTunnel tenu (autre que le cache), ou des flux en cours.
        let used = Arc::strong_count(&cached.entry) > 1 || cached.entry.tunnel.active() > 0;
        if used {
            cached.idle_since = None;
            return true;
        }
        let since = *cached.idle_since.get_or_insert(now);
        let keep = now.duration_since(since) < IDLE_CLOSE;
        if !keep {
            log::info!("relais : tunnel vers {} fermé (sans usage depuis {} s)", key.1, IDLE_CLOSE.as_secs());
        }
        keep
    });
}

/// Un tunnel ouvert vers une Station : Admin sur `127.0.0.1:A`, Kyber sur
/// `127.0.0.1:L` (TCP et UDP). Il reste ouvert tant qu'on le tient (et 30 s
/// après), partagé entre la console et la session de la même Station.
#[derive(Clone)]
pub struct RelayTunnel(Arc<Entry>);

/// Le nom du contrat (docs/relais.md) : `relay::Tunnel::open(&RelayRoute)`.
pub type Tunnel = RelayTunnel;

impl std::fmt::Debug for RelayTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RelayTunnel({} → {}, Admin {})", self.0.route.server, self.0.route.station, self.0.admin.port())
    }
}

impl RelayTunnel {
    /// [`open`] : le tunnel vers cette Station (du cache, ou neuf).
    pub fn open(route: &RelayRoute) -> Result<RelayTunnel, RelayError> {
        open(route)
    }

    /// Le chemin en ce moment (puce, ligne « Chemin ») : [`Self::route_info`].
    pub fn stats(&self) -> RouteInfo {
        self.route_info()
    }

    pub fn route(&self) -> &RelayRoute {
        &self.0.route
    }

    /// Admin de la Station, sur la boucle locale : à donner à la console
    /// (épinglée sur `route().admin_fp`).
    pub fn admin_addr(&self) -> SocketAddr {
        self.0.admin.addr()
    }

    /// Kyber de la Station, sur la boucle locale (TCP et UDP, même numéro), à
    /// la place de `port` (celui que donne Admin : `prepare.kyber_port`,
    /// `share_join.kyber.port`) dans la configuration de kyclient.
    pub fn kyber_addr(&self, port: u16) -> Result<SocketAddr, RelayError> {
        let mut kyber = self.0.kyber.lock().unwrap();
        if let Some(k) = kyber.as_ref() {
            return Ok(k.addr());
        }
        let tunnel = self.0.tunnel.clone();
        let listener = runtime().block_on(async move { tunnel.listen_kyber().await }).map_err(|e| RelayError::Unreachable(e.to_string()))?;
        log::info!("relais : Kyber de la Station (port {port}) sur {}", listener.addr());
        let addr = listener.addr();
        *kyber = Some(listener);
        Ok(addr)
    }

    /// Le chemin en ce moment : relais en QUIC ou en TCP, allers-retours.
    pub fn route_info(&self) -> RouteInfo {
        let s = self.0.tunnel.stats();
        RouteInfo {
            transport: if s.transport == Some(Transport::Tcp) { RouteTransport::RelayTcp } else { RouteTransport::Relay },
            relay: Some(self.0.route.server.clone()),
            rtt_client: s.rtt_client.map(|d| d.as_secs_f64() * 1000.0),
            rtt_station: s.rtt_station.map(|d| d.as_secs_f64() * 1000.0),
        }
    }

    pub fn station_online(&self) -> bool {
        self.0.tunnel.station_online()
    }
}

/// Le tunnel vers cette Station par ce relais : celui du cache s'il est
/// ouvert, sinon un neuf (relais joint, puis la Station attendue 3 s).
/// Bloquant ; à appeler hors du fil de l'interface. `RelayError::StationOffline`
/// si la Station n'est pas chez le relais (le tunnel reste en cache, prêt
/// pour l'essai suivant).
pub fn open(route: &RelayRoute) -> Result<RelayTunnel, RelayError> {
    open_with(route, None)
}

/// [`open`] avec d'autres autorités que celles du web (essais).
pub(crate) fn open_with(route: &RelayRoute, roots: Option<Arc<rustls::RootCertStore>>) -> Result<RelayTunnel, RelayError> {
    let relay = route.relay_addr()?;
    // La route gardée porte le relais sous sa forme normale (chemin, journaux).
    let route = &RelayRoute { server: relay.to_string(), ..route.clone() };
    let key = (relay.to_string(), route.station.clone());
    let cached = cache().lock().unwrap().get(&key).map(|c| c.entry.clone());
    let entry = match cached {
        Some(e) if e.route.admin_fp == route.admin_fp => e,
        _ => {
            let station = route.station.clone();
            let mut cfg = ClientConfig::new(relay.clone(), station.clone());
            cfg.roots = roots;
            cfg.force_tcp = std::env::var("KYBERNOW_RELAY_TCP").is_ok_and(|v| v == "1");
            cfg.version = Some(format!("KyberNow Link {}", env!("CARGO_PKG_VERSION")));
            let (tunnel, admin) = runtime().block_on(async move {
                let tunnel = KyTunnel::open(cfg).await?;
                let admin = tunnel.listen_admin().await.map_err(|e| TunnelError::Io(e.to_string()))?;
                Ok::<_, TunnelError>((tunnel, admin))
            })?;
            log::info!("relais {relay} : tunnel vers {station} ({}), Admin sur {}", tunnel.client_id(), admin.addr());
            let entry = Arc::new(Entry { tunnel, admin, kyber: Mutex::new(None), route: route.clone() });
            cache().lock().unwrap().insert(key, Cached { entry: entry.clone(), idle_since: None });
            entry
        }
    };
    let handle = RelayTunnel(entry);
    if !handle.station_online() {
        let tunnel = handle.0.tunnel.clone();
        if !runtime().block_on(async move { tunnel.wait_station(STATION_WAIT).await }) {
            return Err(RelayError::StationOffline);
        }
    }
    Ok(handle)
}

/// Tunnels ouverts en ce moment (journaux, essais).
pub fn open_tunnels() -> usize {
    cache().lock().unwrap().len()
}

// MARK: - Recherche d'un code, présence

/// La Station d'un code de partage chez ce relais (`GET /v1/codes/{code}`) :
/// `None` si le relais ne le connaît pas. La réponse est vérifiée
/// ([`CodeLookup::verify`] : clé ↔ identifiant, signature de l'empreinte).
/// Bloquant (6 s au plus).
pub fn lookup(relay: &RelayAddr, code: &str) -> Result<Option<CodeLookup>, RelayError> {
    lookup_with(relay, code, None)
}

pub(crate) fn lookup_with(relay: &RelayAddr, code: &str, roots: Option<Arc<rustls::RootCertStore>>) -> Result<Option<CodeLookup>, RelayError> {
    if !kybernow_tunnel::lookup::is_code(code) {
        return Ok(None);
    }
    let tls = kybernow_tunnel::tls::client_config(relay, roots, b"http/1.1", None).map_err(|e| RelayError::Bad(e.to_string()))?;
    let agent = ureq::AgentBuilder::new().tls_config(Arc::new(tls)).timeout_connect(Duration::from_secs(4)).timeout(Duration::from_secs(6)).build();
    // `https://share.kybernow.app/v1/codes/…` : sans « :443 ».
    let url = format!("https://{}{}", relay.web_authority(), kybernow_tunnel::lookup::lookup_path(code));
    let r = agent.get(&url).set("Accept-Language", i18n::current().code()).call();
    match r {
        Ok(resp) => {
            let found: CodeLookup = resp.into_json().map_err(|e| RelayError::Unreachable(e.to_string()))?;
            lookup_answer(200, Some(found), None, None)
        }
        Err(ureq::Error::Status(status, resp)) => {
            let header = resp.header("Retry-After").and_then(|h| h.trim().parse().ok());
            let body: serde_json::Value = resp.into_json().unwrap_or(serde_json::Value::Null);
            lookup_answer(status, None, body.get("retry_after").and_then(|v| v.as_u64()), header)
        }
        Err(e) => Err(RelayError::Unreachable(e.to_string())),
    }
}

/// La réponse de `GET /v1/codes/{code}`.
fn lookup_answer(status: u16, found: Option<CodeLookup>, retry_after: Option<u64>, header: Option<u64>) -> Result<Option<CodeLookup>, RelayError> {
    match status {
        200 => match found {
            Some(f) if f.verify() => Ok(Some(f)),
            Some(_) => {
                log::warn!("relais : réponse de code dont la signature ne tient pas, écartée");
                Ok(None)
            }
            None => Ok(None),
        },
        404 | 400 => Ok(None),
        429 => Err(RelayError::TooMany { seconds: retry_after.or(header) }),
        s => Err(RelayError::Unreachable(format!("HTTP {s}"))),
    }
}

/// Quelles Stations sont joignables par ce relais (16 au plus), sans tunnel
/// (accueil : « Hors de chez vous »). Bloquant.
pub fn presence(relay: &RelayAddr, stations: &[String]) -> Result<Vec<bool>, RelayError> {
    let (relay, stations) = (relay.clone(), stations.to_vec());
    runtime().block_on(async move { kybernow_tunnel::client::presence(&relay, None, &stations).await }).map_err(RelayError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

    #[test]
    fn routes() {
        let r = RelayRoute { server: DEFAULT_RELAY.into(), station: STATION.into(), admin_fp: "sha256:aa".into() };
        assert_eq!(r.relay_addr().unwrap(), RelayAddr::public());
        assert!(RelayRoute { station: "court".into(), ..r.clone() }.relay_addr().is_err(), "identifiant de 26 caractères");
        assert!(RelayRoute { admin_fp: " ".into(), ..r.clone() }.relay_addr().is_err(), "jamais sans l'empreinte d'Admin");
        assert!(RelayRoute { server: "a b".into(), ..r.clone() }.relay_addr().is_err());
        let pinned = RelayRoute { server: "relais.exemple.org:9443#sha256:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB:AB".into(), ..r.clone() };
        assert_eq!(pinned.relay_addr().unwrap().pin.as_deref(), Some("sha256:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab:ab"), "relais chez soi, épinglé");
        // info.relay : gardé par l'app, route avec l'empreinte d'Admin connue,
        // relais sous sa forme normale (le 443 sans port).
        let info: RelayInfo = serde_json::from_str(r#"{"server":"Share.Kybernow.app:443","id":"abcdefghijklmnopqrstuvwxyz","remote_access":true}"#).unwrap();
        assert!(info.remote_access);
        assert_eq!(info.route("sha256:aa"), Some(r));
        assert_eq!(info.route(""), None);
        assert_eq!(RelayInfo { id: String::new(), ..info.clone() }.route("sha256:aa"), None);
        let other = RelayInfo { server: "relais.exemple.org:9443".into(), ..info };
        assert_eq!(other.route("sha256:aa").unwrap().server, "relais.exemple.org:9443");
    }

    #[test]
    fn adresses_normales() {
        assert_eq!(DEFAULT_RELAY, "share.kybernow.app");
        assert_eq!(RelayAddr::public().port, 443);
        for s in ["share.kybernow.app", "share.kybernow.app:443", "https://share.kybernow.app/", " SHARE.kybernow.app "] {
            assert_eq!(normalize(s).as_deref(), Some("share.kybernow.app"), "{s}");
        }
        assert_eq!(normalize("share.kybernow.app:8443").as_deref(), Some("share.kybernow.app:8443"));
        assert_eq!(normalize("relais.exemple.org:9443").as_deref(), Some("relais.exemple.org:9443"));
        assert_eq!(normalize("[2001:db8::1]:443").as_deref(), Some("[2001:db8::1]"));
        assert_eq!(normalize("a b"), None);
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn mode_d_essai() {
        assert_eq!(RelayMode::from_env(Some("force")), RelayMode::Force);
        assert_eq!(RelayMode::from_env(Some(" OFF ")), RelayMode::Off);
        assert_eq!(RelayMode::from_env(Some("oui")), RelayMode::Normal);
        assert_eq!(RelayMode::from_env(None), RelayMode::Normal);
    }

    #[test]
    fn chemins() {
        let i = RouteInfo { transport: RouteTransport::RelayTcp, relay: Some(DEFAULT_RELAY.into()), rtt_client: Some(12.26), rtt_station: None };
        assert_eq!(
            serde_json::to_value(&i).unwrap(),
            serde_json::json!({ "transport": "relay_tcp", "relay": "share.kybernow.app", "rtt_client": 12.26, "rtt_station": null })
        );
        assert_eq!(i.rounded().rtt_client, Some(12.5));
        assert!(i.via_relay() && !RouteInfo::direct().via_relay());
        assert_eq!(
            [RouteTransport::Direct, RouteTransport::Relay, RouteTransport::RelayTcp].map(|t| t.label(Lang::Fr)),
            ["Direct", "Par le relais", "Par le relais (TCP)"]
        );
        assert_eq!(RouteTransport::RelayTcp.label(Lang::En), "Via the relay (TCP)");
        assert_eq!([RouteTransport::Direct, RouteTransport::Relay, RouteTransport::RelayTcp].map(RouteTransport::code), [0, 1, 2]);
    }

    #[test]
    fn textes_des_refus() {
        assert_eq!(RelayError::Unreachable("x".into()).text(Lang::Fr, None), "Le relais ne répond pas.");
        assert_eq!(RelayError::Unreachable("x".into()).text(Lang::En, None), "The relay is not answering.");
        assert_eq!(RelayError::StationOffline.text(Lang::Fr, Some("PC-ATELIER")), "PC-ATELIER n'est pas joignable : l'ordinateur est éteint ou en veille.");
        assert_eq!(RelayError::StationOffline.text(Lang::En, Some("PC-ATELIER")), "PC-ATELIER cannot be reached: the computer is off or asleep.");
        assert!(RelayError::StationOffline.text(Lang::Fr, None).contains("éteint ou en veille"));
        assert_eq!(RelayError::from(TunnelError::StationOffline), RelayError::StationOffline);
        assert!(matches!(RelayError::from(TunnelError::Timeout), RelayError::Unreachable(_)));
        // Accès coupé dans Admin (docs/link-ui.md, « Hors de chez vous »).
        assert_eq!(RelayError::AccessOff.text(Lang::Fr, Some("PC-ATELIER")), "PC-ATELIER n'est pas joignable hors de chez vous (accès coupé dans Admin).");
        assert_eq!(RelayError::AccessOff.text(Lang::En, Some("PC-ATELIER")), "PC-ATELIER cannot be reached away from home (access turned off in Admin).");
        assert_eq!(RelayError::AccessOff.text(Lang::Fr, None), "L'accès hors de chez vous est coupé dans Admin.");
        assert_eq!(RelayError::AccessOff.text(Lang::En, Some(" ")), "Access away from home is turned off in Admin.");
        // Codes des ponts, et retour au texte avec le nom.
        let codes = [(RelayError::Unreachable("x".into()), 30, "unreachable"), (RelayError::StationOffline, 31, "station_offline"), (RelayError::AccessOff, 32, "access_off")];
        for (e, code, kind) in codes {
            assert_eq!((e.code(), e.kind()), (code, kind));
            assert_eq!(RelayError::from_code(code).map(|r| r.text(Lang::Fr, Some("Atelier"))), Some(e.text(Lang::Fr, Some("Atelier"))));
        }
        assert_eq!((RelayError::Refused("banned".into()).code(), RelayError::TooMany { seconds: None }.code(), RelayError::Bad("x".into()).code()), (30, 30, 30));
        assert_eq!(RelayError::from_code(0), None);
    }

    #[test]
    fn reponses_d_une_recherche() {
        assert_eq!(lookup_answer(404, None, None, None), Ok(None), "code inconnu");
        assert_eq!(lookup_answer(400, None, None, None), Ok(None));
        assert_eq!(lookup_answer(429, None, Some(30), Some(5)), Err(RelayError::TooMany { seconds: Some(30) }));
        assert_eq!(lookup_answer(429, None, None, Some(5)), Err(RelayError::TooMany { seconds: Some(5) }));
        assert!(matches!(lookup_answer(502, None, None, None), Err(RelayError::Unreachable(_))));
        // Une signature qui ne tient pas : écartée.
        let forged = CodeLookup { station: STATION.into(), station_key: "AAAA".into(), admin_fp: "sha256:aa".into(), sig: "AAAA".into(), online: true };
        assert_eq!(lookup_answer(200, Some(forged), None, None), Ok(None));
        // Pas un code : rien à demander.
        assert_eq!(lookup(&RelayAddr::public(), "12ab56"), Ok(None));
    }
}

/// Le relais et Admin sur la boucle locale : un faux relais (QUIC `kn-ctl/1`
/// et `kn-media/1`, et `GET /v1/codes/{code}` en HTTPS sur le même port) qui
/// fait suivre les flux `admin_tls` vers un faux Admin (TLS, quelques
/// routes). Le tunnel est le vrai (`kybernow-tunnel`), la console aussi.
#[cfg(test)]
pub(crate) mod essai {
    use std::sync::Arc;

    use kybernow_tunnel::frame::{read_msg, write_msg};
    use kybernow_tunnel::msg::{Msg, Service, Welcome};
    use kybernow_tunnel::quic::{ALPN_CTL, ALPN_MEDIA};
    use kybernow_tunnel::transport::Conn;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    /// Une réponse du faux Admin à (méthode, chemin) : statut et corps JSON.
    pub(crate) type Route = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

    pub(crate) struct Bench {
        /// `127.0.0.1:P#sha256:…` (relais épinglé : certificat d'essai).
        pub relay: String,
        pub station: String,
        pub admin_fp: String,
        /// La réponse de `GET /v1/codes/123456`.
        pub code: super::CodeLookup,
        _rt: tokio::runtime::Runtime,
    }

    fn cert(name: &str) -> (CertificateDer<'static>, Vec<u8>) {
        let key = rcgen::KeyPair::generate().unwrap();
        let c = rcgen::CertificateParams::new(vec![name.into()]).unwrap().self_signed(&key).unwrap();
        (c.der().clone(), key.serialize_der())
    }

    fn server_config(cert: &(CertificateDer<'static>, Vec<u8>), alpn: &[&[u8]]) -> rustls::ServerConfig {
        let mut sc = rustls::ServerConfig::builder_with_provider(kybernow_tunnel::tls::provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.0.clone()], PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.1.clone())))
            .unwrap();
        sc.alpn_protocols = alpn.iter().map(|a| a.to_vec()).collect();
        sc
    }

    /// Une requête HTTP/1.1 lue (en-têtes, puis le corps annoncé), une réponse JSON, fermée.
    async fn serve_http<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(mut s: S, route: &Route) {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let head_end = loop {
            let Ok(n) = s.read(&mut chunk).await else { return };
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let length: usize = head
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0)))
            .unwrap_or(0);
        while buf.len() < head_end + length {
            let Ok(n) = s.read(&mut chunk).await else { return };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let mut first = head.lines().next().unwrap_or_default().split_whitespace();
        let (method, path) = (first.next().unwrap_or_default().to_string(), first.next().unwrap_or_default().to_string());
        let (status, body) = route(&method, &path);
        let reply = format!("HTTP/1.1 {status} R\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let _ = s.write_all(reply.as_bytes()).await;
        let _ = s.shutdown().await;
    }

    /// Une Station d'essai : clé Ed25519, identifiant, signature de `admin_fp`.
    fn station(admin_fp: &str) -> (String, String, String) {
        use base64::Engine;
        use ring::signature::KeyPair;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let mut spki = vec![0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
        spki.extend_from_slice(pair.public_key().as_ref());
        let sig = pair.sign(&kybernow_tunnel::lookup::sig_message(admin_fp));
        let b64 = base64::engine::general_purpose::STANDARD;
        (kybernow_tunnel::id::station_id(&spki), b64.encode(&spki), b64.encode(sig.as_ref()))
    }

    /// Le faux relais et le faux Admin ; `admin` répond aux routes d'Admin.
    pub(crate) fn start(admin: Route) -> Bench {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let relay_cert = cert("relay.test");
        let admin_cert = cert("admin.test");
        let relay_fp = super::super::console::fingerprint(&relay_cert.0);
        let admin_fp = super::super::console::fingerprint(&admin_cert.0);
        let (station, station_key, sig) = station(&admin_fp);
        let code = super::CodeLookup { station: station.clone(), station_key, admin_fp: admin_fp.clone(), sig, online: true };
        let lookup_body = serde_json::to_string(&code).unwrap();
        let (endpoint, tcp, admin_port) = rt.block_on(async {
            // Faux Admin.
            let admin_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let admin_port = admin_listener.local_addr().unwrap().port();
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config(&admin_cert, &[b"http/1.1"])));
            let route = admin.clone();
            tokio::spawn(async move {
                while let Ok((tcp, _)) = admin_listener.accept().await {
                    let (acceptor, route) = (acceptor.clone(), route.clone());
                    tokio::spawn(async move {
                        if let Ok(tls) = acceptor.accept(tcp).await {
                            serve_http(tls, &route).await;
                        }
                    });
                }
            });
            // Faux relais : QUIC sur UDP P, HTTPS sur TCP P.
            let qsc = quinn::crypto::rustls::QuicServerConfig::try_from(server_config(&relay_cert, &[ALPN_CTL, ALPN_MEDIA])).unwrap();
            let endpoint = quinn::Endpoint::server(quinn::ServerConfig::with_crypto(Arc::new(qsc)), "127.0.0.1:0".parse().unwrap()).unwrap();
            let port = endpoint.local_addr().unwrap().port();
            let tcp = TcpListener::bind(("127.0.0.1", port)).await.expect("TCP sur le port du relais");
            (endpoint, tcp, admin_port)
        });
        let port = endpoint.local_addr().unwrap().port();
        let web_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config(&relay_cert, &[b"http/1.1"])));
        let web: Route = Arc::new(move |_, path| if path == "/v1/codes/123456" { (200, lookup_body.clone()) } else { (404, r#"{"error":"unknown_code"}"#.into()) });
        rt.spawn(async move {
            while let Ok((tcp, _)) = tcp.accept().await {
                let (acceptor, web) = (web_acceptor.clone(), web.clone());
                tokio::spawn(async move {
                    if let Ok(tls) = acceptor.accept(tcp).await {
                        serve_http(tls, &web).await;
                    }
                });
            }
        });
        rt.spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                tokio::spawn(async move {
                    let Ok(conn) = incoming.await else { return };
                    let alpn = conn.handshake_data().and_then(|h| h.downcast::<quinn::crypto::rustls::HandshakeData>().ok()).and_then(|h| h.protocol);
                    if alpn.as_deref() == Some(ALPN_MEDIA) {
                        // Les datagrammes ne servent pas ici : la connexion est tenue.
                        conn.closed().await;
                        return;
                    }
                    let conn = Conn::Quic(conn);
                    let Some(mut ctl) = conn.accept().await else { return };
                    let Ok(Some(Msg::Hello(hello))) = read_msg(&mut ctl).await else { return };
                    let id = kybernow_tunnel::id::client_id(hello.token.as_deref().unwrap_or_default());
                    let welcome = Welcome { v: 1, id, relay: "essai".into(), online: Some(true), codes: Vec::new() };
                    if write_msg(&mut ctl, &Msg::Welcome(welcome)).await.is_err() {
                        return;
                    }
                    // Le flux des messages reste ouvert ; les autres flux vont à Admin.
                    tokio::spawn(async move {
                        while let Ok(Some(_)) = read_msg(&mut ctl).await {}
                    });
                    while let Some(mut s) = conn.accept().await {
                        tokio::spawn(async move {
                            if let Ok(Some(Msg::Open { service: Service::AdminTls })) = read_msg(&mut s).await
                                && let Ok(mut admin) = TcpStream::connect(("127.0.0.1", admin_port)).await
                            {
                                let _ = tokio::io::copy_bidirectional(&mut s, &mut admin).await;
                            }
                        });
                    }
                });
            }
        });
        Bench { relay: format!("127.0.0.1:{port}#{relay_fp}"), station, admin_fp, code, _rt: rt }
    }

    impl Bench {
        pub(crate) fn route(&self) -> super::RelayRoute {
            super::RelayRoute { server: self.relay.clone(), station: self.station.clone(), admin_fp: self.admin_fp.clone() }
        }

        pub(crate) fn addr(&self) -> super::RelayAddr {
            self.relay.parse().unwrap()
        }
    }

    #[test]
    fn console_et_code_par_le_relais() {
        let bench = start(Arc::new(|method, path| match (method, path) {
            ("GET", "/api/stream/info") => (200, r#"{"hostname":"PC-RELAIS","relay":{"server":"share.kybernow.app","id":"x","remote_access":true}}"#.into()),
            _ => (404, r#"{"error":"inconnu"}"#.into()),
        }));
        let route = bench.route();
        // Le tunnel : Admin et Kyber sur la boucle locale, partagé par (relais, Station).
        let t = super::open(&route).expect("tunnel");
        assert!(t.admin_addr().ip().is_loopback());
        let again = super::open(&route).unwrap();
        assert_eq!(again.admin_addr(), t.admin_addr(), "un seul tunnel par (relais, Station)");
        let kyber = t.kyber_addr(24812).unwrap();
        assert!(kyber.ip().is_loopback());
        assert_eq!(t.kyber_addr(24812).unwrap(), kyber, "un seul point d'écoute de Kyber");
        assert!(std::net::UdpSocket::bind(kyber).is_err(), "UDP pris sur le même numéro");
        assert_eq!(t.route_info().transport, super::RouteTransport::Relay);
        // La console par le tunnel, Admin épinglé.
        let c = crate::console::Console::new("192.0.2.1", 9, "u", "p", None).with_relay(Some(route.clone()));
        let info = c.info().expect("info par le relais");
        assert_eq!(info.hostname, "PC-RELAIS");
        assert_eq!(info.relay.map(|r| r.remote_access), Some(true));
        // Une autre empreinte d'Admin : refusée, jamais de premier contact.
        let other = super::RelayRoute { admin_fp: format!("sha256:{}", ["00"; 32].join(":")), ..route };
        let c = crate::console::Console::new("192.0.2.1", 9, "u", "p", None).with_relay(Some(other));
        assert!(matches!(c.info(), Err(crate::console::ConsoleError::CertificateChanged { .. })), "empreinte d'Admin imposée");
        // Le code chez le relais (HTTPS, relais épinglé), signature vérifiée.
        let found = super::lookup(&bench.addr(), "123456").unwrap().expect("code connu");
        assert_eq!((found.station.as_str(), found.admin_fp.as_str()), (bench.station.as_str(), bench.admin_fp.as_str()));
        assert!(found.verify());
        assert_eq!(super::lookup(&bench.addr(), "654321").unwrap(), None, "code inconnu");
    }

    #[test]
    fn relais_muet() {
        // Rien n'écoute : « Le relais ne répond pas. », sans attendre longtemps.
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let fp = format!("sha256:{}", ["ab"; 32].join(":"));
        let route = super::RelayRoute { server: format!("127.0.0.1:{port}#{fp}"), station: "abcdefghijklmnopqrstuvwxyz".into(), admin_fp: fp.clone() };
        let started = std::time::Instant::now();
        assert!(matches!(super::open(&route), Err(super::RelayError::Unreachable(_))));
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        let c = crate::console::Console::guest("192.0.2.1", 9, None).with_relay(Some(route));
        assert!(matches!(c.info(), Err(crate::console::ConsoleError::Relay(super::RelayError::Unreachable(_)))));
    }
}
