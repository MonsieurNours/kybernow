//! Le relais de KyberNow, côté Station (docs/relais.md, « Admin » et « Garde
//! d'Admin selon la voie ») : partager et se connecter hors de chez soi.
//!
//! - **L'agent** (crate `kybernow-tunnel`) tient la connexion au relais, seulement
//!   quand un partage est ouvert ou que « Accès hors de chez vous » est actif.
//!   L'identité de la Station (clé Ed25519) est créée au premier usage dans
//!   `<données>/relay/station.key` et gardée d'une installation à l'autre.
//! - **Ce qui arrive du relais est servi dans ce processus**, jamais en se
//!   reconnectant sur `127.0.0.1` : `admin_tls` (une app, TLS de bout en bout
//!   sur le certificat d'Admin) et `admin_http` (un navigateur, déchiffré par le
//!   relais) passent par un `Router` à part, gardé par [`guard`]. Chaque
//!   requête y porte l'adresse donnée par le relais, **forcée hors de la boucle
//!   locale et des adresses privées** (`ConnectInfo`), et sa voie ([`Via`]).
//!   Rien de ce qui vient du relais ne passe donc pour « local » (`/api/local/*`).
//! - **Kyber** (flux `kyber_tcp` et flux UDP) n'est joint que pour un client
//!   autorisé (un invité accepté qui donne signe de vie, un appareil après
//!   `prepare`), et toujours **depuis et vers l'adresse du réseau local** :
//!   le correctif 0019 réserve `/kybernow/*` à la boucle locale.
//! - **Codes** : relais joint, le code du partage est réservé chez lui (unique
//!   entre toutes les Stations) ; « Nouveau code » en réserve un autre.
//! - **Blocage** : 20 comptes refusés par le relais en 10 minutes ferment
//!   l'accès des appareils par le relais pendant 10 minutes.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use kybernow_tunnel::{Agent, AgentConfig, AgentState, FlowRequest, Incoming, RelayAddr, StationIdentity, StreamHeader, StreamKind};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Notify, mpsc};
use tokio::task::AbortHandle;

use crate::AppState;
use crate::i18n::Msg;

/// Le relais public, réglé par défaut.
pub const DEFAULT_SERVER: &str = kybernow_tunnel::DEFAULT_RELAY;
/// Un client autorisé à joindre Kyber l'est pour ce temps, renouvelé par le
/// signe de vie (un invité toutes les 5 s, un appareil en session aussi).
const GRANT: Duration = Duration::from_secs(30);
/// Comptes refusés par le relais depuis une même adresse (un /64 en IPv6),
/// sur `LOCK_WINDOW`, avant de bloquer cette adresse seulement.
const LOCK_AFTER: usize = 10;
/// Garde-fou : comptes refusés par le relais, toutes adresses confondues,
/// avant de bloquer tout l'accès des appareils par le relais.
const LOCK_AFTER_ALL: usize = 100;
const LOCK_WINDOW: Duration = Duration::from_secs(10 * 60);
const LOCK_FOR: Duration = Duration::from_secs(10 * 60);
/// Réponse à un compte refusé : ralentit les essais, comme la connexion d'Admin.
#[cfg(not(test))]
const FAILED_TRY_DELAY: Duration = Duration::from_millis(800);
#[cfg(test)]
const FAILED_TRY_DELAY: Duration = Duration::ZERO;
/// Réserver le code chez le relais a échoué : on réessaie après ce temps.
const RESERVE_RETRY: Duration = Duration::from_secs(10);
/// L'agent reste connecté ce temps après le dernier partage : le suivant a
/// son lien hors de chez soi tout de suite.
const LINGER: Duration = Duration::from_secs(5 * 60);
/// Délai pour la négociation TLS d'une app, et pour les en-têtes HTTP.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// L'adresse donnée à une requête du relais dont l'adresse publique est
/// locale ou privée (un client derrière la même box que le relais) : une
/// adresse du bloc « à jeter » d'IPv6 (RFC 6666), jamais locale ni privée.
const OUTSIDE: Ipv6Addr = Ipv6Addr::new(0x100, 0, 0, 0, 0, 0, 0, 1);

// --- Réglages -------------------------------------------------------------------

/// « Hors de chez vous » (`[relay]` de kyber-admin.toml).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaySettings {
    /// Adresse du relais (`hôte`, `hôte:port` hors du 443, et `#sha256:…`
    /// pour un relais chez soi signé par lui-même). Absent : le relais
    /// public ; vide : pas de relais.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// « Accès hors de chez vous » : coupé au départ.
    #[serde(default)]
    pub remote_access: bool,
}

impl RelaySettings {
    /// L'adresse réglée, telle qu'elle s'affiche (vide : pas de relais).
    pub fn server(&self) -> String {
        self.server.clone().unwrap_or_else(|| DEFAULT_SERVER.to_string())
    }

    /// Le relais réglé, s'il y en a un et qu'il se lit.
    pub fn addr(&self) -> Option<RelayAddr> {
        let server = self.server();
        if server.trim().is_empty() { None } else { server.parse().ok() }
    }
}

/// Relit une adresse de relais saisie dans Admin : vide (pas de relais), ou
/// lisible, sous sa forme normale : sans port pour le 443
/// (`share.kybernow.app`), avec lui sinon (`relais.exemple.org:9443`).
pub fn normalize_server(input: &str) -> Result<String, Msg> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }
    input.parse::<RelayAddr>().map(|a| a.to_string()).map_err(|_| Msg::new("relay.badServer").with("server", input))
}

// --- La voie d'une requête --------------------------------------------------------

/// Posée sur toute requête arrivée par le relais : sa sorte de flux et
/// l'identifiant du client chez le relais (celui qu'Admin autorise à joindre Kyber).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Via {
    pub kind: StreamKind,
    pub client: String,
}

/// L'adresse d'une requête du relais : celle du client, sauf si elle est
/// locale ou privée (elle passerait pour l'ordinateur lui-même ou son réseau).
pub fn outside(ip: IpAddr) -> IpAddr {
    let ip = ip.to_canonical();
    let inside = match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // Réseau partagé des opérateurs (100.64.0.0/10).
                || (a == 100 && (64..128).contains(&b))
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback() || v6.is_unspecified() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    };
    if inside { IpAddr::V6(OUTSIDE) } else { ip }
}

// --- État --------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Holder {
    /// Un invité accepté (son identifiant dans le partage).
    Guest(String),
    /// Un appareil appairé en session (son identifiant d'appareil).
    Device(String),
}

struct Grant {
    holder: Holder,
    until: Instant,
}

/// Les comptes refusés d'une adresse (ou de toutes), sur `LOCK_WINDOW`.
#[derive(Default)]
struct Strikes {
    failures: VecDeque<Instant>,
    /// Comptes en cours de vérification : comptés d'avance, pour que des
    /// requêtes simultanées ne dépassent jamais la limite.
    pending: usize,
    until: Option<Instant>,
}

impl Strikes {
    fn prune(&mut self, now: Instant) {
        while self.failures.front().is_some_and(|t| now.saturating_duration_since(*t) >= LOCK_WINDOW) {
            self.failures.pop_front();
        }
    }

    fn locked_for(&self, now: Instant) -> Option<Duration> {
        self.until.filter(|u| *u > now).map(|u| u - now)
    }

    /// Les échecs, plus les essais en cours s'ils échouaient tous, atteignent la limite.
    fn full(&self, limit: usize) -> bool {
        self.failures.len() + self.pending >= limit
    }

    /// Un échec ; vrai s'il déclenche le blocage.
    fn fail(&mut self, now: Instant, limit: usize) -> bool {
        self.failures.push_back(now);
        if self.failures.len() >= limit {
            self.until = Some(now + LOCK_FOR);
            self.failures.clear();
            return true;
        }
        false
    }

    fn idle(&self, now: Instant) -> bool {
        self.failures.is_empty() && self.pending == 0 && self.locked_for(now).is_none()
    }
}

#[derive(Default)]
struct Lockout {
    by_ip: HashMap<IpAddr, Strikes>,
    all: Strikes,
}

/// Ce qu'un compte refusé vient de bloquer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// Cette adresse seulement.
    Address,
    /// Tout l'accès des appareils par le relais (garde-fou).
    All,
}

/// L'adresse comptée pour le blocage : IPv4 telle quelle, IPv6 réduite à son
/// /64 (un abonné en a tout un).
fn ip_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
        }
        v4 => v4,
    }
}

/// L'appareil en session par le relais (`GET /api/relay`, `sessions`).
struct RemoteSession {
    device_id: String,
    name: String,
    since: u64,
}

/// L'agent en marche et la tâche qui sert ce qui arrive du relais.
struct Running {
    relay: RelayAddr,
    agent: Arc<Agent>,
    serve: tokio::task::JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.serve.abort();
        self.agent.stop();
    }
}

pub struct Relay {
    data_dir: PathBuf,
    identity: Mutex<Option<Arc<StationIdentity>>>,
    running: Mutex<Option<Running>>,
    /// Clients du relais autorisés à joindre Kyber, par identifiant de client.
    grants: Mutex<HashMap<String, Grant>>,
    /// Connexions `kyber_tcp` en cours, par client (coupées quand son autorisation tombe).
    kyber_tcp: Mutex<HashMap<String, Vec<AbortHandle>>>,
    lockout: Mutex<Lockout>,
    session: Mutex<Option<RemoteSession>>,
    reserve_failed: Mutex<Option<Instant>>,
    /// La dernière fois qu'un partage était ouvert (voir `LINGER`).
    share_seen: Mutex<Option<Instant>>,
    wake: Notify,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Relay {
    pub fn new(data_dir: &Path) -> Self {
        Relay {
            data_dir: data_dir.to_path_buf(),
            identity: Mutex::new(None),
            running: Mutex::new(None),
            grants: Mutex::new(HashMap::new()),
            kyber_tcp: Mutex::new(HashMap::new()),
            lockout: Mutex::new(Lockout::default()),
            session: Mutex::new(None),
            reserve_failed: Mutex::new(None),
            share_seen: Mutex::new(None),
            wake: Notify::new(),
        }
    }

    /// L'identité de la Station, créée au premier usage (gardée dans `relay\`
    /// du dossier de données, que l'installateur garde).
    pub fn identity(&self) -> Option<Arc<StationIdentity>> {
        let mut identity = self.identity.lock().unwrap();
        if identity.is_none() {
            let path = self.data_dir.join("relay").join("station.key");
            let existed = path.exists();
            match StationIdentity::load_or_create(&path) {
                Ok(id) => {
                    if !existed {
                        tracing::info!("relais : identité de la Station créée ({})", id.id());
                    }
                    *identity = Some(Arc::new(id));
                }
                Err(e) => tracing::warn!("relais : identité de la Station illisible ({}) : {e}", path.display()),
            }
        }
        identity.clone()
    }

    /// L'identifiant de la Station chez le relais.
    pub fn id(&self) -> Option<String> {
        self.identity().map(|i| i.id().to_string())
    }

    /// Relit les réglages et l'état du partage tout de suite (réglage changé,
    /// partage ouvert ou arrêté).
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    fn agent(&self) -> Option<Arc<Agent>> {
        self.running.lock().unwrap().as_ref().map(|r| r.agent.clone())
    }

    /// L'agent, s'il est joint au relais.
    fn joined_agent(&self) -> Option<Arc<Agent>> {
        self.agent().filter(|a| a.state().is_joined())
    }

    /// Relais joint et ce code réservé chez lui : le partage est joignable hors de chez soi.
    pub fn holds(&self, code: &str) -> bool {
        self.joined_agent().is_some_and(|a| a.codes().iter().any(|c| c == code))
    }

    /// Réserve un nouveau code chez le relais (« Nouveau code »), si le relais est joint.
    pub async fn reserve_new(&self) -> Option<(Arc<Agent>, String)> {
        let agent = self.joined_agent()?;
        match agent.reserve(None).await {
            Ok(code) => Some((agent, code)),
            Err(e) => {
                tracing::warn!("relais : nouveau code non réservé : {e}");
                None
            }
        }
    }

    // --- Autorisations vers Kyber ---

    fn grant(&self, client: &str, holder: Holder) {
        let mut grants = self.grants.lock().unwrap();
        grants.insert(client.to_string(), Grant { holder, until: Instant::now() + GRANT });
    }

    /// Un invité accepté joint Kyber par ce client du relais (`wait`, `beat`, `join`).
    pub fn grant_guest(&self, via: &Via, guest_id: &str) {
        self.grant(&via.client, Holder::Guest(guest_id.to_string()));
    }

    /// Un appareil appairé joint Kyber par ce client (`prepare`, signe de vie).
    pub fn grant_device(&self, via: &Via, device_id: &str) {
        self.grant(&via.client, Holder::Device(device_id.to_string()));
    }

    pub fn authorized(&self, client: &str) -> bool {
        self.grants.lock().unwrap().get(client).is_some_and(|g| g.until > Instant::now())
    }

    /// L'invité dont ce client du relais tient l'autorisation en ce moment
    /// (preuve qu'il revient, `share::Proof`).
    pub fn granted_guest(&self, client: &str) -> Option<String> {
        let grants = self.grants.lock().unwrap();
        match grants.get(client) {
            Some(Grant { holder: Holder::Guest(id), until }) if *until > Instant::now() => Some(id.clone()),
            _ => None,
        }
    }

    /// Retire les autorisations qui répondent à `pred` : flux UDP fermés,
    /// connexions TCP vers Kyber coupées.
    fn revoke_where(&self, pred: impl Fn(&Grant) -> bool) {
        let clients: Vec<String> = {
            let mut grants = self.grants.lock().unwrap();
            let gone: Vec<String> = grants.iter().filter(|(_, g)| pred(g)).map(|(c, _)| c.clone()).collect();
            for c in &gone {
                grants.remove(c);
            }
            gone
        };
        if clients.is_empty() {
            return;
        }
        let agent = self.agent();
        let mut tcp = self.kyber_tcp.lock().unwrap();
        for client in clients {
            if let Some(agent) = &agent {
                agent.close_client(&client);
            }
            for task in tcp.remove(&client).unwrap_or_default() {
                task.abort();
            }
            tracing::debug!("relais : {client} n'est plus autorisé à joindre Kyber");
        }
    }

    /// Un invité sort (parti, retiré, sans nouvelles, partage arrêté).
    pub fn revoke_guest(&self, guest_id: &str) {
        self.revoke_where(|g| g.holder == Holder::Guest(guest_id.to_string()));
    }

    /// La session d'un appareil finit.
    pub fn revoke_device(&self, device_id: &str) {
        self.revoke_where(|g| g.holder == Holder::Device(device_id.to_string()));
    }

    /// « Accès hors de chez vous » coupé : plus aucun appareil ne joint Kyber
    /// par le relais, tout de suite (sans attendre la fin de son autorisation).
    fn revoke_devices(&self) {
        self.revoke_where(|g| matches!(g.holder, Holder::Device(_)));
    }

    fn revoke_all(&self) {
        self.revoke_where(|_| true);
    }

    fn expire_grants(&self) {
        let now = Instant::now();
        self.revoke_where(|g| g.until <= now);
    }

    fn track_tcp(&self, client: &str, task: AbortHandle) {
        let mut tcp = self.kyber_tcp.lock().unwrap();
        let tasks = tcp.entry(client.to_string()).or_default();
        tasks.retain(|t| !t.is_finished());
        tasks.push(task);
    }

    // --- Blocage ---

    /// Tout l'accès des appareils par le relais est bloqué (garde-fou) : pour combien de temps.
    fn locked_for_at(&self, now: Instant) -> Option<Duration> {
        self.lockout.lock().unwrap().all.locked_for(now)
    }

    pub fn locked_for(&self) -> Option<Duration> {
        self.locked_for_at(Instant::now())
    }

    /// Avant de vérifier un compte venu du relais depuis `ip` : `Err(attente)`
    /// si cette adresse, ou tout l'accès, est bloquée, ou si les essais déjà
    /// en cours suffiraient à la bloquer (des requêtes simultanées ne
    /// dépassent jamais la limite). Sinon l'essai est compté en cours, et
    /// [`Relay::end_attempt_at`] doit suivre.
    fn begin_attempt_at(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut l = self.lockout.lock().unwrap();
        l.all.prune(now);
        l.by_ip.retain(|_, s| {
            s.prune(now);
            !s.idle(now)
        });
        if let Some(wait) = l.all.locked_for(now) {
            return Err(wait);
        }
        let key = ip_key(ip);
        let mine = l.by_ip.entry(key).or_default();
        if let Some(wait) = mine.locked_for(now) {
            return Err(wait);
        }
        let mine_full = mine.full(LOCK_AFTER);
        if mine_full || l.all.full(LOCK_AFTER_ALL) {
            return Err(Duration::from_secs(1));
        }
        l.by_ip.entry(key).or_default().pending += 1;
        l.all.pending += 1;
        Ok(())
    }

    /// Après la vérification : un compte refusé compte pour son adresse et
    /// pour toutes ; rend ce qu'il vient de bloquer.
    fn end_attempt_at(&self, ip: IpAddr, ok: bool, now: Instant) -> Option<Blocked> {
        let mut l = self.lockout.lock().unwrap();
        let key = ip_key(ip);
        l.all.pending = l.all.pending.saturating_sub(1);
        let mine = l.by_ip.entry(key).or_default();
        mine.pending = mine.pending.saturating_sub(1);
        if ok {
            return None;
        }
        let address = mine.fail(now, LOCK_AFTER);
        let all = l.all.fail(now, LOCK_AFTER_ALL);
        if all {
            Some(Blocked::All)
        } else if address {
            Some(Blocked::Address)
        } else {
            None
        }
    }

    // --- Sessions par le relais ---

    /// Un appareil se prépare par le relais : autorisé à joindre Kyber ; vrai
    /// si sa session par le relais commence (une ligne dans le fil).
    pub fn note_session(&self, via: &Via, device_id: &str, name: &str) -> bool {
        self.grant_device(via, device_id);
        let mut session = self.session.lock().unwrap();
        if session.as_ref().is_some_and(|r| r.device_id == device_id) {
            return false;
        }
        *session = Some(RemoteSession { device_id: device_id.to_string(), name: name.to_string(), since: now_secs() });
        true
    }

    fn sessions(&self, s: &AppState) -> Vec<Value> {
        let live = s.activity.live();
        let mut session = self.session.lock().unwrap();
        match (session.as_ref(), live) {
            (Some(r), Some(l)) if r.device_id == l.device_id => vec![json!({ "device": r.name, "since": r.since })],
            (Some(_), _) => {
                *session = None;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    // --- État pour Admin ---

    /// `GET /api/relay` (et `PUT`).
    pub fn status(&self, s: &AppState) -> Value {
        let settings = s.settings.lock().unwrap().relay.clone();
        let mut out = self.brief(s, &settings);
        let locked_until = self.locked_for().map(|wait| now_secs() + wait.as_secs().max(1));
        out["id"] = json!(self.id());
        out["transport"] = json!(self.agent().and_then(|a| a.transport()).map(|t| t.as_str()));
        out["locked_until"] = json!(locked_until);
        out["sessions"] = json!(self.sessions(s));
        out
    }

    /// `server`, `remote_access`, `state`, `reason` (icône, `/api/local/state`).
    pub fn brief(&self, _s: &AppState, settings: &RelaySettings) -> Value {
        let (state, reason) = match self.agent() {
            None => ("off", None),
            Some(agent) => match agent.state() {
                AgentState::Connecting => ("connecting", None),
                AgentState::Joined { .. } => ("joined", None),
                AgentState::Unreachable { reason } => ("unreachable", Some(reason)),
                AgentState::Stopped => ("unreachable", Some(Msg::new("relay.refused").text())),
            },
        };
        json!({ "server": settings.server(), "remote_access": settings.remote_access, "state": state, "reason": reason })
    }

    /// `relay` de `GET /api/stream/info` : seulement quand un relais est réglé.
    pub fn info(&self, s: &AppState) -> Option<Value> {
        let settings = s.settings.lock().unwrap().relay.clone();
        let addr = settings.addr()?;
        Some(json!({ "server": addr.to_string(), "id": self.id(), "remote_access": settings.remote_access }))
    }

    /// Ce que la vue du partage dit du relais : `None` sans relais réglé ;
    /// sinon le relais (son adresse, l'identifiant de la Station) et où en
    /// est le code : tenu chez le relais, en chemin (connexion, réservation),
    /// ou pas joignable (connexion ou réservation en échec).
    pub fn for_share(&self, s: &AppState, code: &str) -> Option<ShareRelay> {
        let addr = s.settings.lock().unwrap().relay.addr()?;
        let state = if self.holds(code) {
            ShareRelayState::Joined
        } else {
            let failed = self.reserve_failed.lock().unwrap().is_some_and(|t| t.elapsed() < RESERVE_RETRY);
            match self.agent().map(|a| a.state()) {
                Some(AgentState::Unreachable { .. } | AgentState::Stopped) => ShareRelayState::Unreachable,
                Some(AgentState::Joined { .. }) if failed => ShareRelayState::Unreachable,
                _ => ShareRelayState::Connecting,
            }
        };
        let id = if state == ShareRelayState::Joined { self.id() } else { None };
        Some(ShareRelay { addr, id, state })
    }
}

/// Le relais vu du partage.
pub struct ShareRelay {
    pub addr: RelayAddr,
    pub id: Option<String>,
    pub state: ShareRelayState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareRelayState {
    Connecting,
    Joined,
    Unreachable,
}

impl ShareRelayState {
    pub fn as_str(self) -> &'static str {
        match self {
            ShareRelayState::Connecting => "connecting",
            ShareRelayState::Joined => "joined",
            ShareRelayState::Unreachable => "unreachable",
        }
    }
}

// --- Veille : l'agent démarre, s'arrête et tient les codes ---------------------------

/// Toutes les secondes (ou tout de suite après `wake`) : l'agent tourne si un
/// relais est réglé et qu’un partage est ouvert (ou l’était il y a moins de
/// `LINGER`) ou l’accès actif ; le code du
/// partage est tenu chez le relais ; les autorisations échues tombent.
pub fn supervise(s: AppState, router: Router) {
    tokio::spawn(async move {
        loop {
            supervise_tick(&s, &router).await;
            tokio::select! {
                _ = s.relay.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
    });
}

async fn supervise_tick(s: &AppState, router: &Router) {
    let settings = s.settings.lock().unwrap().relay.clone();
    let open = s.share.is_open();
    let lingering = {
        let mut seen = s.relay.share_seen.lock().unwrap();
        if open {
            *seen = Some(Instant::now());
        }
        seen.is_some_and(|t| t.elapsed() < LINGER)
    };
    // Prêt : un code de séance, un prêt en cours, ou des comptes d'amis qui peuvent venir de loin.
    let lending = s.lend.code_value().is_some()
        || s.lend.loan().is_some()
        || (s.settings.lock().unwrap().lend.enabled && s.friends.any_enabled());
    let wanted = settings.addr().filter(|_| settings.remote_access || open || lingering || lending);
    let current = s.relay.running.lock().unwrap().as_ref().map(|r| r.relay.clone());
    if wanted != current {
        let old = s.relay.running.lock().unwrap().take();
        if let Some(old) = old {
            tracing::info!("relais : connexion à {} arrêtée", old.relay);
            drop(old);
            s.relay.revoke_all();
        }
        if let Some(relay) = wanted {
            start(s, router, relay);
        }
    }
    s.relay.expire_grants();
    sync_codes(s).await;
}

fn start(s: &AppState, router: &Router, relay: RelayAddr) {
    let Some(identity) = s.relay.identity() else { return };
    let Some(admin_fp) = s.tls_fingerprint.clone() else {
        tracing::warn!("relais : pas d'empreinte d'Admin, connexion impossible");
        return;
    };
    let acceptor = match tls_acceptor(&s.data_dir) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!("relais : certificat d'Admin illisible, les apps ne passeront pas : {e:#}");
            None
        }
    };
    let force_tcp = std::env::var("KYBERNOW_RELAY_TCP").is_ok_and(|v| v == "1");
    let (agent, incoming) = Agent::start(AgentConfig {
        relay: relay.clone(),
        identity,
        admin_fp,
        version: env!("CARGO_PKG_VERSION").into(),
        roots: None,
        force_tcp,
    });
    let agent = Arc::new(agent);
    tracing::info!("relais : connexion à {relay} (Station {})", agent.id());
    tokio::spawn(log_states(s.clone(), agent.subscribe(), relay.to_string()));
    let serve = tokio::spawn(serve_incoming(s.clone(), router.clone(), incoming, acceptor));
    *s.relay.running.lock().unwrap() = Some(Running { relay, agent, serve });
}

/// Les passages de l'agent d'un état à l'autre, dans le journal.
/// Une fois joint, la veille repasse aussitôt pour réserver le code du partage.
async fn log_states(s: AppState, mut states: tokio::sync::watch::Receiver<AgentState>, relay: String) {
    while states.changed().await.is_ok() {
        let state = states.borrow().clone();
        match state {
            AgentState::Joined { transport } => {
                tracing::info!("relais : {relay} joint ({})", transport.as_str());
                s.relay.wake();
            }
            AgentState::Unreachable { reason } => tracing::info!("relais : {relay} injoignable ({reason})"),
            AgentState::Stopped => tracing::info!("relais : agent arrêté"),
            AgentState::Connecting => {}
        }
    }
}

/// Le code du partage tenu chez le relais : réservé (le même s'il est libre,
/// sinon le relais en donne un autre, que le partage prend), et les codes
/// d'avant rendus.
async fn sync_codes(s: &AppState) {
    let Some(agent) = s.relay.joined_agent() else { return };
    // Le code du partage et celui du prêt (docs/pret.md, « Codes »).
    let wanted: Vec<String> = s.share.snapshot().map(|snap| snap.code).into_iter().chain(s.lend.code_value()).collect();
    for held in agent.codes() {
        if !wanted.contains(&held) {
            agent.release(&held);
        }
    }
    for code in wanted {
        if agent.codes().contains(&code) {
            continue;
        }
        if s.relay.reserve_failed.lock().unwrap().is_some_and(|t| t.elapsed() < RESERVE_RETRY) {
            return;
        }
        match agent.reserve(Some(&code)).await {
            Ok(got) if got != code => {
                if s.share.replace_code(&code, &got) || s.lend.replace_code(&code, &got) {
                    tracing::info!("relais : code pris ailleurs, nouveau code réservé");
                } else {
                    agent.release(&got);
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("relais : code non réservé : {e}");
                *s.relay.reserve_failed.lock().unwrap() = Some(Instant::now());
            }
        }
    }
}

// --- Ce qui arrive du relais ------------------------------------------------------------

fn tls_acceptor(data_dir: &Path) -> anyhow::Result<tokio_rustls::TlsAcceptor> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let dir = data_dir.join("tls");
    let certs = CertificateDer::pem_file_iter(dir.join("cert.pem"))?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(dir.join("key.pem"))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

async fn serve_incoming(s: AppState, router: Router, mut incoming: mpsc::Receiver<Incoming>, acceptor: Option<tokio_rustls::TlsAcceptor>) {
    while let Some(event) = incoming.recv().await {
        match event {
            Incoming::Stream { header, stream } => match header.kind {
                StreamKind::AdminTls => {
                    let Some(acceptor) = acceptor.clone() else { continue };
                    let router = router.clone();
                    tokio::spawn(async move {
                        match tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
                            Ok(Ok(tls)) => serve_http(tls, router, header).await,
                            Ok(Err(e)) => tracing::debug!("relais : TLS refusé pour {} : {e}", header.client),
                            Err(_) => tracing::debug!("relais : TLS trop lent pour {}", header.client),
                        }
                    });
                }
                StreamKind::AdminHttp => {
                    tokio::spawn(serve_http(stream, router.clone(), header));
                }
                StreamKind::KyberTcp => kyber_tcp(&s, &header, stream),
            },
            Incoming::Flow(req) => kyber_flow(&s, req),
            Incoming::ClientGone { client } => tracing::debug!("relais : {client} n'a plus de connexion"),
        }
    }
}

/// Sert une connexion HTTP/1.1 du relais avec le `Router` gardé : chaque
/// requête porte l'adresse du client (forcée hors de la boucle locale et des
/// adresses privées) et sa voie. WebSocket compris (mandataire vers Kyber).
async fn serve_http<I>(io: I, router: Router, header: StreamHeader)
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let peer = SocketAddr::new(outside(header.ip), 0);
    let via = Via { kind: header.kind, client: header.client.clone() };
    let service = hyper::service::service_fn(move |mut req: hyper::Request<hyper::body::Incoming>| {
        req.extensions_mut().insert(ConnectInfo(peer));
        req.extensions_mut().insert(via.clone());
        tower::ServiceExt::oneshot(router.clone(), req.map(axum::body::Body::new))
    });
    let served = hyper::server::conn::http1::Builder::new()
        .timer(hyper_util::rt::TokioTimer::new())
        .header_read_timeout(HANDSHAKE_TIMEOUT)
        .serve_connection(hyper_util::rt::TokioIo::new(io), service)
        .with_upgrades()
        .await;
    if let Err(e) = served {
        tracing::debug!("relais : connexion de {} finie : {e}", header.client);
    }
}

/// Kyber par l'adresse du réseau local de l'ordinateur (jamais la boucle locale).
fn kyber_lan(s: &AppState) -> Option<(IpAddr, u16)> {
    let lan: IpAddr = crate::tls::lan_ip()?.parse().ok()?;
    if lan.is_loopback() || lan.is_unspecified() {
        return None;
    }
    Some((lan, s.kyber.port()?))
}

/// Un flux `kyber_tcp` (contrôle HTTPS de kycontroller, WebSocket) : relié à
/// Kyber depuis et vers l'adresse du réseau local, pour un client autorisé.
fn kyber_tcp<I>(s: &AppState, header: &StreamHeader, mut stream: I)
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    if !s.relay.authorized(&header.client) {
        tracing::debug!("relais : {} n'est pas autorisé à joindre Kyber", header.client);
        return;
    }
    let Some((lan, port)) = kyber_lan(s) else {
        tracing::warn!("relais : adresse du réseau local ou port de Kyber inconnu");
        return;
    };
    let client = header.client.clone();
    let task = tokio::spawn(async move {
        let up = async {
            let socket = match lan {
                IpAddr::V4(_) => tokio::net::TcpSocket::new_v4()?,
                IpAddr::V6(_) => tokio::net::TcpSocket::new_v6()?,
            };
            socket.bind(SocketAddr::new(lan, 0))?;
            socket.connect(SocketAddr::new(lan, port)).await
        };
        match up.await {
            Ok(mut kyber) => {
                let _ = tokio::io::copy_bidirectional(&mut stream, &mut kyber).await;
            }
            Err(e) => tracing::warn!("relais : Kyber injoignable sur {lan}:{port} : {e}"),
        }
    });
    s.relay.track_tcp(&client, task.abort_handle());
}

/// Un flux UDP vers Kyber (l'image) : accepté pour un client autorisé, de
/// l'adresse du réseau local vers Kyber à la même adresse.
fn kyber_flow(s: &AppState, req: FlowRequest) {
    if !s.relay.authorized(&req.client) {
        tracing::debug!("relais : flux UDP refusé à {}", req.client);
        req.refuse("not_authorized");
        return;
    }
    let Some((lan, port)) = kyber_lan(s) else {
        req.refuse("kyber");
        return;
    };
    let client = req.client.clone();
    if let Err(e) = req.accept(SocketAddr::new(lan, 0), SocketAddr::new(lan, port)) {
        tracing::warn!("relais : flux UDP de {client} vers Kyber impossible : {e}");
    }
}

// --- Garde : ce que le relais peut atteindre ------------------------------------------

/// Les chemins, selon qui peut les atteindre par le relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Un invité, quand un partage est ouvert (le code, le secret ou le
    /// cookie font le reste).
    Guest,
    /// Le mandataire vers Kyber du client web : un invité accepté (cookie),
    /// partage ouvert.
    Kyber,
    /// Un appareil appairé (compte Basic), seulement avec « Accès hors de chez vous ».
    Device,
    /// L'ami d'un prêt (code ou compte d'ami), quand le prêt est activé.
    Lend,
    /// Jamais par le relais.
    Never,
}

fn class_of(method: &Method, path: &str) -> Class {
    let read = method == Method::GET || method == Method::HEAD;
    match path {
        "/api/session" if read => return Class::Guest,
        "/api/share/join" | "/api/share/wait" | "/api/share/beat" | "/api/share/quality" | "/api/share/leave" => return Class::Guest,
        // L'ami d'un prêt (docs/pret.md) : code ou compte d'ami, puis son secret.
        "/api/lend/status" | "/api/lend/borrow" | "/api/lend/wait" | "/api/lend/beat" | "/api/lend/leave" => return Class::Lend,
        _ => {}
    }
    // L'hôte d'un prêt : un appareil appairé, comme les routes de l'hôte du partage.
    if path == "/api/lend" || path == "/api/lend/code" || path == "/api/lend/take-back" || path.starts_with("/api/lend/requests/") {
        return Class::Device;
    }
    // Ni redémarrage ni mise à jour de la Station hors de chez soi : une
    // action lourde, qu'on fait sur place ou dans Admin.
    if path.starts_with("/api/stream/station/") {
        return Class::Never;
    }
    if path == "/api/share" || path.starts_with("/api/share/") || path.starts_with("/api/stream/") {
        return Class::Device;
    }
    if path == "/api" || path.starts_with("/api/") {
        return Class::Never;
    }
    if crate::kyber_proxy::is_kyber_path(path) {
        return Class::Kyber;
    }
    if read && (path == "/link" || path.starts_with("/link/") || path.starts_with("/assets/") || path == "/favicon.svg") {
        return Class::Guest;
    }
    Class::Never
}

fn refusal(status: StatusCode, code: &str, msg: Msg) -> Response {
    (status, Json(json!({ "error": msg.text(), "code": code }))).into_response()
}

/// Le partage est fermé : rien à atteindre pour un invité.
fn no_share(path: &str) -> Response {
    if path.starts_with("/api/") {
        (StatusCode::NOT_FOUND, Json(json!({ "error": "no_share" }))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Msg::new("share.noShare").text()).into_response()
    }
}

/// Le cookie de session d'Admin ne vaut jamais par le relais : il est retiré
/// avant tout le reste (le cookie d'un invité reste).
fn strip_console_cookie(headers: &mut HeaderMap) {
    let kept: Vec<HeaderValue> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .map(str::trim)
                .filter(|c| !c.is_empty() && c.split_once('=').is_none_or(|(k, _)| k.trim() != crate::auth::COOKIE))
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|v| !v.is_empty())
        .filter_map(|v| HeaderValue::from_str(&v).ok())
        .collect();
    headers.remove(header::COOKIE);
    for v in kept {
        headers.append(header::COOKIE, v);
    }
}

/// `relay_guard` : posé sur le `Router` que sert l'agent. N'ouvre que ce que
/// docs/relais.md permet par le relais ; tout le reste : 403.
pub async fn guard(State(s): State<AppState>, mut req: Request, next: Next) -> Response {
    strip_console_cookie(req.headers_mut());
    let path = req.uri().path().to_string();
    match class_of(req.method(), &path) {
        Class::Guest => {
            // Partage fermé : ni code à essayer, ni page à servir. Le signe de
            // vie, l'attente et le départ passent (« arrêté », pendant une minute).
            // Les pages servent aussi l'ami d'un prêt quand le prêt est activé.
            let pages_open = s.share.is_open() || s.settings.lock().unwrap().lend.enabled;
            if (path == "/api/share/join" && !s.share.is_open()) || (!path.starts_with("/api/") && !pages_open) {
                return no_share(&path);
            }
        }
        Class::Lend => {
            if !s.settings.lock().unwrap().lend.enabled {
                return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "off" }))).into_response();
            }
        }
        Class::Kyber => {
            // Un invité du partage, ou l'ami d'un prêt (son cookie fait le reste).
            if !s.share.is_open() && s.lend.loan().is_none() {
                return no_share(&path);
            }
            // L'image du navigateur par le relais : en WebSocket seulement,
            // jamais en WebTransport vers le port de Kyber.
            if path == "/kymux/start_mux" {
                return refusal(StatusCode::FORBIDDEN, "relay_forbidden", Msg::new("relay.forbidden"));
            }
        }
        Class::Device => {
            if !s.settings.lock().unwrap().relay.remote_access {
                return refusal(StatusCode::FORBIDDEN, "remote_access_off", Msg::new("relay.accessOff"));
            }
            // L'adresse donnée par le relais (déjà forcée hors des adresses locales).
            let ip = req.extensions().get::<ConnectInfo<SocketAddr>>().map_or(IpAddr::V6(OUTSIDE), |c| c.0.ip());
            if let Err(wait) = s.relay.begin_attempt_at(ip, Instant::now()) {
                let seconds = wait.as_secs().max(1);
                let mut res = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({ "error": Msg::new("relay.locked").text(), "code": "locked", "retry_after": seconds })),
                )
                    .into_response();
                res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
                return res;
            }
            // Vérifié sans rien attendre entre le début et la fin de l'essai.
            let ok = crate::api::stream_user(&s, req.headers()).is_ok();
            match s.relay.end_attempt_at(ip, ok, Instant::now()) {
                Some(Blocked::All) => {
                    tracing::warn!("relais : {LOCK_AFTER_ALL} comptes refusés en 10 min, tout l'accès hors de chez vous bloqué 10 min");
                    s.activity.event("warn", Msg::new("events.relayLocked"));
                }
                Some(Blocked::Address) => {
                    tracing::warn!("relais : {LOCK_AFTER} comptes refusés depuis {ip}, cette adresse est bloquée 10 min");
                    s.activity.event("warn", Msg::new("events.relayLockedAddress").with("address", ip_key(ip).to_string()));
                }
                None => {}
            }
            if !ok {
                tokio::time::sleep(FAILED_TRY_DELAY).await;
                return refusal(StatusCode::UNAUTHORIZED, "unauthorized", Msg::new("api.kyberCredentials"));
            }
        }
        Class::Never => return refusal(StatusCode::FORBIDDEN, "relay_forbidden", Msg::new("relay.forbidden")),
    }
    next.run(req).await
}

// --- Réglages : GET/PUT /api/relay (session d'Admin) ------------------------------------

#[derive(Deserialize)]
pub struct RelayBody {
    #[serde(default)]
    server: Option<String>,
    #[serde(default)]
    remote_access: Option<bool>,
}

pub async fn get_relay(State(s): State<AppState>) -> Json<Value> {
    Json(s.relay.status(&s))
}

pub async fn put_relay(State(s): State<AppState>, Json(body): Json<RelayBody>) -> Response {
    let server = match body.server.as_deref().map(normalize_server) {
        Some(Err(msg)) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": msg.text() }))).into_response(),
        Some(Ok(server)) => Some(server),
        None => None,
    };
    let before = s.settings.lock().unwrap().relay.clone();
    let saved = s.update_settings(|settings| {
        if let Some(server) = server {
            settings.relay.server = if server == DEFAULT_SERVER { None } else { Some(server) };
        }
        if let Some(on) = body.remote_access {
            settings.relay.remote_access = on;
        }
    });
    if let Err(e) = saved {
        return crate::api::ApiError::from(e).into_response();
    }
    let after = s.settings.lock().unwrap().relay.clone();
    if before.server() != after.server() {
        tracing::info!("relais : adresse réglée à « {} »", after.server());
    }
    if before.remote_access != after.remote_access {
        tracing::info!("relais : accès hors de chez vous {}", if after.remote_access { "activé" } else { "coupé" });
        let key = if after.remote_access { "events.relayAccessOn" } else { "events.relayAccessOff" };
        s.activity.event("info", Msg::new(key));
        if !after.remote_access {
            s.relay.revoke_devices();
        }
    }
    s.relay.wake();
    Json(s.relay.status(&s)).into_response()
}

/// Le `Router` servi par l'agent : le même que sur le réseau, sous [`guard`].
pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(crate::api::router(state.clone()))
        .fallback(crate::web)
        .layer(axum::middleware::from_fn_with_state(state.clone(), guard))
        .layer(axum::middleware::from_fn(crate::i18n::scope))
        .with_state(state)
}

#[cfg(all(test, not(windows)))]
mod tests {
    use std::net::Ipv4Addr;

    use axum::body::Body;
    use axum::routing::get;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[test]
    fn une_adresse_du_relais_ne_passe_jamais_pour_locale() {
        for ip in [
            "127.0.0.1", "10.1.2.3", "172.16.0.9", "192.168.1.20", "169.254.3.4", "0.0.0.0", "100.64.1.1", "::1", "::", "fd12::1", "fe80::1",
            "::ffff:127.0.0.1", "::ffff:192.168.1.5",
        ] {
            let got = outside(ip.parse().unwrap());
            assert_eq!(got, IpAddr::V6(OUTSIDE), "{ip}");
            assert!(!got.is_loopback(), "{ip}");
        }
        for ip in ["82.66.39.160", "2a01:e0a::1", "100.128.0.1"] {
            assert_eq!(outside(ip.parse().unwrap()), ip.parse::<IpAddr>().unwrap(), "{ip}");
        }
        // Une adresse projetée revient à sa forme IPv4.
        assert_eq!(outside("::ffff:82.66.39.160".parse().unwrap()), IpAddr::V4(Ipv4Addr::new(82, 66, 39, 160)));
    }

    #[test]
    fn la_garde_ne_laisse_passer_que_le_partage_et_les_appareils() {
        let get = Method::GET;
        let post = Method::POST;
        for (m, p) in [(&post, "/api/share/join"), (&post, "/api/share/wait"), (&post, "/api/share/beat"), (&post, "/api/share/quality"), (&post, "/api/share/leave"), (&get, "/api/session"), (&get, "/link/"), (&get, "/link/kyclient_wasm.js"), (&get, "/assets/link-1.js"), (&get, "/favicon.svg")] {
            assert_eq!(class_of(m, p), Class::Guest, "{m} {p}");
        }
        for p in ["/api/lend/status", "/api/lend/borrow", "/api/lend/wait", "/api/lend/beat", "/api/lend/leave"] {
            assert_eq!(class_of(&post, p), Class::Lend, "{p}");
        }
        for (m, p) in [(&get, "/api/lend"), (&post, "/api/lend/code"), (&Method::DELETE, "/api/lend/code"), (&post, "/api/lend/requests/ab"), (&post, "/api/lend/take-back")] {
            assert_eq!(class_of(m, p), Class::Device, "{m} {p}");
        }
        for (m, p) in [(&get, "/api/lend/settings"), (&get, "/api/lend/friends"), (&get, "/api/lend/friends/x/invite"), (&get, "/api/lend/catalog"), (&get, "/api/local/lend"), (&get, "/api/local/lend/console")] {
            assert_eq!(class_of(m, p), Class::Never, "{m} {p}");
        }
        for p in ["/session/login", "/ws", "/kymux/start_mux", "/websocket/video"] {
            assert_eq!(class_of(&post, p), Class::Kyber, "{p}");
        }
        for (m, p) in [(&get, "/api/stream/info"), (&post, "/api/stream/prepare"), (&post, "/api/stream/heartbeat"), (&get, "/api/share"), (&post, "/api/share/open"), (&post, "/api/share/guest/ab12/accept"), (&Method::DELETE, "/api/share")] {
            assert_eq!(class_of(m, p), Class::Device, "{m} {p}");
        }
        for (m, p) in [
            (&post, "/api/login"), (&post, "/api/setup"), (&post, "/api/password"), (&get, "/api/web/ticket"), (&post, "/api/pair/request"),
            (&get, "/api/pairing"), (&get, "/api/local/state"), (&post, "/api/local/share/guest/x/accept"), (&get, "/api/config"),
            (&get, "/api/users"), (&get, "/api/logs"), (&post, "/api/service/stop"), (&get, "/api/display"), (&get, "/api/audio"),
            (&post, "/api/update/install"), (&get, "/api/devices"), (&get, "/api/apps"), (&get, "/api/events"), (&get, "/api/relay"),
            (&Method::PUT, "/api/relay"), (&get, "/"), (&get, "/index.html"), (&post, "/link/"), (&get, "/ecran/kyclient_wasm.js"), (&post, "/api/session"),
            (&get, "/api/status"), (&get, "/api/shares"), (&get, "/kybernow/sessions"),
            (&post, "/api/stream/station/restart"), (&post, "/api/stream/station/update"),
        ] {
            assert_eq!(class_of(m, p), Class::Never, "{m} {p}");
        }
    }

    /// Un essai complet (début et fin) ; `Err` s'il est refusé d'avance.
    fn attempt(relay: &Relay, ip: &str, ok: bool, at: Instant) -> Result<Option<Blocked>, Duration> {
        let ip: IpAddr = ip.parse().unwrap();
        relay.begin_attempt_at(ip, at)?;
        Ok(relay.end_attempt_at(ip, ok, at))
    }

    #[test]
    fn dix_comptes_refuses_bloquent_cette_adresse_seulement() {
        let relay = Relay::new(Path::new("/nonexistent"));
        let t0 = Instant::now();
        for i in 0..LOCK_AFTER - 1 {
            assert_eq!(attempt(&relay, "203.0.113.9", false, t0 + Duration::from_secs(i as u64)), Ok(None));
        }
        assert_eq!(attempt(&relay, "203.0.113.9", false, t0 + Duration::from_secs(30)), Ok(Some(Blocked::Address)));
        let left = attempt(&relay, "203.0.113.9", true, t0 + Duration::from_secs(31)).unwrap_err();
        assert!(left > Duration::from_secs(9 * 60) && left <= LOCK_FOR);
        // Le propriétaire, d'une autre adresse, passe toujours ; tout l'accès n'est pas bloqué.
        assert_eq!(attempt(&relay, "198.51.100.4", true, t0 + Duration::from_secs(31)), Ok(None));
        assert!(relay.locked_for_at(t0 + Duration::from_secs(31)).is_none());
        // Une adresse IPv6 compte pour son /64.
        for i in 0..LOCK_AFTER {
            let ip = format!("2001:db8:7:1::{:x}", i + 1);
            let _ = attempt(&relay, &ip, false, t0 + Duration::from_secs(40));
        }
        assert!(attempt(&relay, "2001:db8:7:1::ffff", true, t0 + Duration::from_secs(41)).is_err());
        assert!(attempt(&relay, "2001:db8:7:2::1", true, t0 + Duration::from_secs(41)).is_ok());
        // Dix minutes plus tard, l'adresse est libre.
        assert_eq!(attempt(&relay, "203.0.113.9", true, t0 + Duration::from_secs(31) + LOCK_FOR), Ok(None));
        // Des échecs espacés de plus d'une minute ne s'additionnent pas.
        let relay = Relay::new(Path::new("/nonexistent"));
        for i in 0..LOCK_AFTER * 3 {
            assert_eq!(attempt(&relay, "203.0.113.9", false, t0 + Duration::from_secs(70 * i as u64)), Ok(None));
        }
    }

    #[test]
    fn cent_comptes_refuses_bloquent_tout_l_acces() {
        let relay = Relay::new(Path::new("/nonexistent"));
        let t0 = Instant::now();
        // Cent échecs de vingt-cinq adresses, quatre chacune : aucune bloquée seule.
        for i in 0..LOCK_AFTER_ALL {
            let ip = format!("203.0.113.{}", i % 25);
            let blocked = attempt(&relay, &ip, false, t0).unwrap();
            assert_eq!(blocked, if i + 1 == LOCK_AFTER_ALL { Some(Blocked::All) } else { None }, "essai {i}");
        }
        assert!(relay.locked_for_at(t0).is_some());
        assert!(attempt(&relay, "198.51.100.4", true, t0).is_err());
        assert!(attempt(&relay, "198.51.100.4", true, t0 + LOCK_FOR).is_ok());
    }

    /// Des requêtes simultanées : celles qui pourraient faire dépasser la
    /// limite attendent la fin des essais en cours.
    #[test]
    fn des_essais_simultanes_ne_depassent_pas_la_limite() {
        let relay = Relay::new(Path::new("/nonexistent"));
        let ip: IpAddr = "203.0.113.9".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..LOCK_AFTER - 2 {
            attempt(&relay, "203.0.113.9", false, t0).unwrap();
        }
        // Deux essais en même temps, puis un troisième : refusé d'avance.
        relay.begin_attempt_at(ip, t0).unwrap();
        relay.begin_attempt_at(ip, t0).unwrap();
        assert_eq!(relay.begin_attempt_at(ip, t0), Err(Duration::from_secs(1)));
        // Le premier réussit : une place se libère.
        assert_eq!(relay.end_attempt_at(ip, true, t0), None);
        relay.begin_attempt_at(ip, t0).unwrap();
        // Les deux en cours échouent : bloquée exactement au dixième échec.
        assert_eq!(relay.end_attempt_at(ip, false, t0), None);
        assert_eq!(relay.end_attempt_at(ip, false, t0), Some(Blocked::Address));
        assert!(relay.begin_attempt_at(ip, t0).is_err());
    }

    #[test]
    fn une_autorisation_se_donne_par_client_et_tombe() {
        let relay = Relay::new(Path::new("/nonexistent"));
        let via = Via { kind: StreamKind::AdminTls, client: "c1".into() };
        assert!(!relay.authorized("c1"));
        relay.grant_guest(&via, "g1");
        assert!(relay.authorized("c1"));
        assert!(!relay.authorized("c2"));
        relay.revoke_guest("autre");
        assert!(relay.authorized("c1"));
        relay.revoke_guest("g1");
        assert!(!relay.authorized("c1"));
        relay.grant_device(&via, "mac");
        relay.grants.lock().unwrap().get_mut("c1").unwrap().until = Instant::now() - Duration::from_secs(1);
        assert!(!relay.authorized("c1"));
        relay.expire_grants();
        assert!(relay.grants.lock().unwrap().is_empty());
    }

    #[test]
    fn le_cookie_d_admin_est_retire() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, HeaderValue::from_static("kyber_admin_session=abc; __Host-kybernow_guest=xyz"));
        headers.append(header::COOKIE, HeaderValue::from_static("kyber_admin_session=def"));
        strip_console_cookie(&mut headers);
        let all: Vec<&str> = headers.get_all(header::COOKIE).iter().map(|v| v.to_str().unwrap()).collect();
        assert_eq!(all, vec!["__Host-kybernow_guest=xyz"]);
    }

    #[test]
    fn adresse_du_relais() {
        assert_eq!(normalize_server(" share.kybernow.app ").unwrap(), "share.kybernow.app");
        assert_eq!(normalize_server("share.kybernow.app:443").unwrap(), "share.kybernow.app");
        assert_eq!(normalize_server("https://share.kybernow.app/").unwrap(), "share.kybernow.app");
        assert_eq!(normalize_server("relais.exemple.org:9443").unwrap(), "relais.exemple.org:9443");
        assert_eq!(DEFAULT_SERVER, "share.kybernow.app");
        assert_eq!(normalize_server("").unwrap(), "");
        assert!(normalize_server("relais.exemple.org:99999").is_err());
        let settings = RelaySettings::default();
        assert_eq!(settings.server(), DEFAULT_SERVER);
        assert!(settings.addr().is_some());
        let off = RelaySettings { server: Some(String::new()), remote_access: true };
        assert!(off.addr().is_none());
    }

    /// Une requête servie comme l'agent le fait : l'adresse du pair n'est
    /// jamais la boucle locale et la voie est posée.
    #[tokio::test]
    async fn une_requete_du_relais_porte_sa_voie_et_une_adresse_exterieure() {
        let router: Router = Router::new().route(
            "/echo",
            get(|ConnectInfo(peer): ConnectInfo<SocketAddr>, axum::Extension(via): axum::Extension<Via>| async move {
                format!("{} {} {}", peer.ip(), via.kind.as_str(), via.client)
            }),
        );
        let (mut client, server) = tokio::io::duplex(4096);
        let header = StreamHeader { v: 1, kind: StreamKind::AdminHttp, client: "abc".into(), ip: "127.0.0.1".parse().unwrap(), host: None };
        tokio::spawn(serve_http(server, router, header));
        client.write_all(b"GET /echo HTTP/1.1\r\nHost: x.share.test\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut out = String::new();
        client.read_to_string(&mut out).await.unwrap();
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        assert!(out.ends_with("100::1 admin_http abc"), "{out}");
    }

    /// Le `Router` gardé, de bout en bout (sans réseau).
    #[tokio::test]
    async fn la_garde_refuse_admin_et_le_local_par_le_relais() {
        let dir = std::env::temp_dir().join(format!("kyber-admin-relay-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        let router = router(s.clone());
        let call = |method: Method, path: &str| {
            let mut req = axum::http::Request::builder().method(method).uri(path).header("x-kyber-admin", "1");
            req = req.header(header::COOKIE, "kyber_admin_session=nimporte");
            let mut req = req.body(Body::empty()).unwrap();
            req.extensions_mut().insert(ConnectInfo(SocketAddr::new(IpAddr::V6(OUTSIDE), 0)));
            req.extensions_mut().insert(Via { kind: StreamKind::AdminTls, client: "c".into() });
            tower::ServiceExt::oneshot(router.clone(), req)
        };
        for (m, p) in [(Method::POST, "/api/login"), (Method::GET, "/api/local/state"), (Method::GET, "/api/config"), (Method::GET, "/api/relay"), (Method::GET, "/")] {
            let res = call(m.clone(), p).await.unwrap();
            assert_eq!(res.status(), StatusCode::FORBIDDEN, "{m} {p}");
        }
        // Accès coupé (réglage par défaut) : 403 remote_access_off.
        let res = call(Method::GET, "/api/stream/info").await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        let body = axum::body::to_bytes(res.into_body(), 4096).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["code"], "remote_access_off");
        // Sans partage ouvert : rien pour un invité.
        let res = call(Method::POST, "/api/share/join").await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        // `GET /api/session` réduit.
        let res = call(Method::GET, "/api/session").await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(res.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body["relay"], true);
        assert!(body.get("hostname").is_none() && body.get("setup_code_file").is_none(), "{body}");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Accès actif : un compte refusé compte pour le blocage de son adresse,
    /// dix la bloquent ; une autre adresse passe encore.
    #[tokio::test]
    async fn les_comptes_refuses_par_le_relais_bloquent() {
        let dir = std::env::temp_dir().join(format!("kyber-admin-relay-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        s.settings.lock().unwrap().relay.remote_access = true;
        let router = router(s.clone());
        let call = |ip: &str| {
            let mut req = axum::http::Request::builder()
                .uri("/api/stream/info")
                .header(header::AUTHORIZATION, "Basic bWF4Om1hdXZhaXM=")
                .body(Body::empty())
                .unwrap();
            req.extensions_mut().insert(ConnectInfo(SocketAddr::new(ip.parse().unwrap(), 0)));
            req.extensions_mut().insert(Via { kind: StreamKind::AdminTls, client: "c".into() });
            tower::ServiceExt::oneshot(router.clone(), req)
        };
        for _ in 0..LOCK_AFTER {
            assert_eq!(call("203.0.113.9").await.unwrap().status(), StatusCode::UNAUTHORIZED);
        }
        let res = call("203.0.113.9").await.unwrap();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(res.headers().contains_key(header::RETRY_AFTER));
        // Le propriétaire, ailleurs, n'est pas bloqué.
        assert_eq!(call("198.51.100.4").await.unwrap().status(), StatusCode::UNAUTHORIZED);
        assert!(s.relay.locked_for().is_none());
        assert!(s.activity.events(5, crate::i18n::Lang::Fr).iter().any(|e| e.text.contains("203.0.113.9")));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// « Accès hors de chez vous » coupé : un appareil en session par le
    /// relais ne joint plus Kyber tout de suite (pas 30 s plus tard) ; un
    /// invité du partage, si.
    #[tokio::test]
    async fn l_acces_coupe_retire_kyber_aux_appareils_tout_de_suite() {
        let dir = std::env::temp_dir().join(format!("kyber-admin-relay-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        s.settings.lock().unwrap().relay.remote_access = true;
        s.relay.grant_device(&Via { kind: StreamKind::AdminTls, client: "appareil".into() }, "mac-1");
        s.relay.grant_guest(&Via { kind: StreamKind::AdminTls, client: "invite".into() }, "g1");
        let res = put_relay(State(s.clone()), Json(RelayBody { server: None, remote_access: Some(false) })).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert!(!s.relay.authorized("appareil"), "l'appareil joint encore Kyber par le relais");
        assert!(s.relay.authorized("invite"));
        assert_eq!(s.relay.granted_guest("invite").as_deref(), Some("g1"));
        assert_eq!(s.relay.granted_guest("appareil"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Kyber est joint depuis et vers l'adresse du réseau local, jamais par la
    /// boucle locale, et seulement pour un client autorisé.
    #[tokio::test]
    async fn kyber_par_l_adresse_du_reseau_local() {
        let Some(lan) = crate::tls::lan_ip().and_then(|ip| ip.parse::<IpAddr>().ok()).filter(|ip| !ip.is_loopback()) else {
            return; // Pas de réseau sur cette machine : rien à vérifier.
        };
        let kyber = tokio::net::TcpListener::bind(SocketAddr::new(lan, 0)).await.unwrap();
        let port = kyber.local_addr().unwrap().port();
        let dir = std::env::temp_dir().join(format!("kyber-admin-relay-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("kyber_config.toml"), format!("[kycontroller]\nport = {port}\n")).unwrap();
        let s = AppState::for_tests(dir.clone(), Box::new(crate::service::Simulated::default()));
        let header = StreamHeader { v: 1, kind: StreamKind::KyberTcp, client: "c9".into(), ip: "82.66.39.160".parse().unwrap(), host: None };
        // Pas autorisé : rien n'arrive à Kyber.
        let (_client, server) = tokio::io::duplex(1024);
        kyber_tcp(&s, &header, server);
        assert!(tokio::time::timeout(Duration::from_millis(200), kyber.accept()).await.is_err());
        // Autorisé : la connexion vient de l'adresse du réseau local.
        s.relay.grant_guest(&Via { kind: StreamKind::KyberTcp, client: "c9".into() }, "g");
        let (mut client, server) = tokio::io::duplex(1024);
        kyber_tcp(&s, &header, server);
        let (mut conn, from) = tokio::time::timeout(Duration::from_secs(2), kyber.accept()).await.unwrap().unwrap();
        assert_eq!(from.ip(), lan);
        assert!(!from.ip().is_loopback());
        client.write_all(b"ping").await.unwrap();
        let mut buf = [0u8; 4];
        conn.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"ping");
        // L'autorisation retirée coupe la connexion.
        s.relay.revoke_guest("g");
        let mut rest = Vec::new();
        let n = tokio::time::timeout(Duration::from_secs(2), conn.read_to_end(&mut rest)).await.unwrap().unwrap_or(0);
        assert_eq!(n, 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
