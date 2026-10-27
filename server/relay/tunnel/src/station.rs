//! L'agent de la Station (Admin) : la connexion tenue au relais, les flux qui
//! en arrivent (Admin les sert lui-même, dans son processus), les flux UDP
//! vers Kyber et les codes de partage (docs/relais.md, « Admin » et « Garde
//! d'Admin selon la voie »).
//!
//! L'agent ne décide rien : chaque flux arrive avec son en-tête
//! ([`Incoming::Stream`]), chaque flux UDP demande l'accord d'Admin
//! ([`Incoming::Flow`], [`FlowRequest::accept`] vers l'adresse du réseau
//! local de Kyber, jamais la boucle locale).

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use rustls::RootCertStore;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::timeout;

use crate::addr::RelayAddr;
use crate::error::TunnelError;
use crate::dial::{DialConfig, DialError, Pinger, QUIC_RETRY_EVERY, Session, backoff, dial, dial_quic_only};
use crate::frame::read_msg;
use crate::identity::StationIdentity;
use crate::msg::{ErrorCode, ErrorMsg, Hello, Msg, Role, StreamHeader};
use crate::quic::STATION_KEEP_ALIVE;
use crate::transport::{AbortOnDrop, ActiveCounter, ActiveGuard, Conn, MediaReceiver, MediaSender, RelayStream, Transport};

/// Un socket UDP vers Kyber reste ce temps après la fin de son flux : un
/// client qui revient avec la même clé le retrouve.
pub const FLOW_SOCKET_KEEP: Duration = Duration::from_secs(25);
/// Délai pour lire l'en-tête d'un flux.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);

/// L'agent à lancer.
#[derive(Clone)]
pub struct AgentConfig {
    pub relay: RelayAddr,
    pub identity: Arc<StationIdentity>,
    /// L'empreinte d'Admin (`sha256:aa:bb:…`), signée et donnée au relais.
    pub admin_fp: String,
    /// Version d'Admin, pour les journaux du relais.
    pub version: String,
    /// Autorités à la place de celles du web (essais).
    pub roots: Option<Arc<RootCertStore>>,
    /// Toujours le repli TCP (essais).
    pub force_tcp: bool,
}

impl fmt::Debug for AgentConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentConfig").field("relay", &self.relay).field("station", &self.identity.id()).finish_non_exhaustive()
    }
}

/// L'état de l'agent, pour `GET /api/relay` et le panneau du partage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// Première connexion en cours.
    Connecting,
    /// Connecté au relais (« Relais joint »).
    Joined { transport: Transport },
    /// Le dernier essai a échoué (« Relais injoignable ({raison}) ») ; un autre suivra.
    Unreachable { reason: String },
    /// Arrêté, ou refusé pour de bon (banni, version).
    Stopped,
}

impl AgentState {
    pub fn is_joined(&self) -> bool {
        matches!(self, AgentState::Joined { .. })
    }
}

/// Ce qui arrive du relais, pour Admin.
pub enum Incoming {
    /// Un flux, avec son en-tête écrit par le relais. `admin_tls` : TLS sur le
    /// certificat d'Admin, puis HTTP ; `admin_http` : HTTP ; `kyber_tcp` : à
    /// relier à Kyber par l'adresse du réseau local, pour un client autorisé.
    Stream { header: StreamHeader, stream: RelayStream },
    /// Un client demande un flux UDP vers Kyber.
    Flow(FlowRequest),
    /// Plus aucune connexion de ce client chez le relais.
    ClientGone { client: String },
}

impl fmt::Debug for Incoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Incoming::Stream { header, .. } => write!(f, "Stream({:?} de {})", header.kind, header.client),
            Incoming::Flow(r) => write!(f, "Flow({} clé {})", r.client, r.key),
            Incoming::ClientGone { client } => write!(f, "ClientGone({client})"),
        }
    }
}

struct Live {
    conn: Conn,
    ctl: mpsc::UnboundedSender<Msg>,
    media: MediaSender,
    transport: Transport,
    next_req: AtomicU32,
    pending: Mutex<HashMap<u32, oneshot::Sender<Msg>>>,
}

type FlowKey = (String, u32);

struct FlowSock {
    target: SocketAddr,
    socket: Arc<UdpSocket>,
    /// Numéro de flux du relais en cours (0 : aucun).
    relay_flow: Arc<AtomicU64>,
    orphan_since: Option<Instant>,
    _reader: AbortOnDrop,
    _guard: ActiveGuard,
}

#[derive(Default)]
struct StationFlows {
    by_key: HashMap<FlowKey, FlowSock>,
    by_relay: HashMap<u64, FlowKey>,
}

impl StationFlows {
    fn orphan(&mut self, relay_flow: u64) {
        if let Some(key) = self.by_relay.remove(&relay_flow)
            && let Some(s) = self.by_key.get_mut(&key)
        {
            s.relay_flow.store(0, Ordering::Relaxed);
            s.orphan_since = Some(Instant::now());
        }
    }

    fn orphan_all(&mut self) {
        self.by_relay.clear();
        for s in self.by_key.values_mut() {
            s.relay_flow.store(0, Ordering::Relaxed);
            s.orphan_since.get_or_insert_with(Instant::now);
        }
    }
}

struct AgentShared {
    cfg: AgentConfig,
    state: watch::Sender<AgentState>,
    live: watch::Sender<Option<Arc<Live>>>,
    codes: Mutex<BTreeSet<String>>,
    flows: Mutex<StationFlows>,
    incoming: mpsc::Sender<Incoming>,
    active: ActiveCounter,
}

impl AgentShared {
    fn current(&self) -> Option<Arc<Live>> {
        self.live.borrow().clone()
    }

    async fn wait_live(&self, within: Duration) -> Result<Arc<Live>, TunnelError> {
        let mut rx = self.live.subscribe();
        match timeout(within, rx.wait_for(|l| l.is_some())).await {
            Ok(Ok(l)) => Ok(l.clone().expect("connexion présente")),
            Ok(Err(_)) => Err(TunnelError::Closed),
            Err(_) => Err(match &*self.state.borrow() {
                AgentState::Unreachable { reason } => TunnelError::Unreachable(reason.clone()),
                _ => TunnelError::Timeout,
            }),
        }
    }
}

/// Une demande de flux UDP. Sans réponse, elle est refusée quand on la lâche.
pub struct FlowRequest {
    flow: u64,
    /// L'identifiant du client chez le relais.
    pub client: String,
    /// La clé du flux chez le client (stable tant que sa source vit).
    pub key: u32,
    /// L'adresse publique du client, vue par le relais.
    pub ip: IpAddr,
    shared: Arc<AgentShared>,
    live: Weak<Live>,
    answered: bool,
}

impl FlowRequest {
    /// Accepte : les paquets vont de `bind` (adresse du réseau local de la
    /// Station, port 0) vers `target` (Kyber, à la même adresse). Un socket
    /// gardé pour ce client et cette clé vers la même cible est repris.
    pub fn accept(mut self, bind: SocketAddr, target: SocketAddr) -> io::Result<()> {
        self.answered = true;
        let Some(live) = self.live.upgrade() else { return Err(io::Error::new(io::ErrorKind::NotConnected, "connexion au relais refaite")) };
        let key: FlowKey = (self.client.clone(), self.key);
        let mut flows = self.shared.flows.lock().unwrap();
        let reuse = flows.by_key.get(&key).is_some_and(|s| s.target == target);
        if reuse {
            let s = flows.by_key.get_mut(&key).expect("socket présent");
            let old = s.relay_flow.swap(self.flow, Ordering::Relaxed);
            s.orphan_since = None;
            if old != 0 {
                flows.by_relay.remove(&old);
            }
        } else {
            let opened = (|| {
                let std = std::net::UdpSocket::bind(bind)?;
                std.connect(target)?;
                std.set_nonblocking(true)?;
                UdpSocket::from_std(std)
            })();
            let socket = match opened {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    let _ = live.ctl.send(Msg::FlowRefuse { flow: self.flow, reason: "socket".into() });
                    return Err(e);
                }
            };
            let relay_flow = Arc::new(AtomicU64::new(self.flow));
            let reader = tokio::spawn(flow_reader(socket.clone(), relay_flow.clone(), self.shared.live.subscribe()));
            if let Some(old) = flows.by_key.insert(
                key.clone(),
                FlowSock {
                    target,
                    socket,
                    relay_flow,
                    orphan_since: None,
                    _reader: AbortOnDrop(reader),
                    _guard: self.shared.active.guard(),
                },
            ) {
                let old_flow = old.relay_flow.load(Ordering::Relaxed);
                if old_flow != 0 {
                    flows.by_relay.remove(&old_flow);
                }
            }
        }
        flows.by_relay.insert(self.flow, key);
        let _ = live.ctl.send(Msg::FlowAccept { flow: self.flow });
        Ok(())
    }

    /// Refuse (client pas autorisé à joindre Kyber, accès coupé…).
    pub fn refuse(mut self, reason: &str) {
        self.answered = true;
        if let Some(live) = self.live.upgrade() {
            let _ = live.ctl.send(Msg::FlowRefuse { flow: self.flow, reason: reason.into() });
        }
    }
}

impl Drop for FlowRequest {
    fn drop(&mut self) {
        if !self.answered
            && let Some(live) = self.live.upgrade()
        {
            let _ = live.ctl.send(Msg::FlowRefuse { flow: self.flow, reason: "unanswered".into() });
        }
    }
}

async fn flow_reader(socket: Arc<UdpSocket>, relay_flow: Arc<AtomicU64>, live: watch::Receiver<Option<Arc<Live>>>) {
    let mut buf = vec![0u8; 65_536];
    loop {
        let Ok(n) = socket.recv(&mut buf).await else {
            // Port fermé côté Kyber (ICMP) : on attend un peu et on continue.
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        };
        let flow = relay_flow.load(Ordering::Relaxed);
        if flow == 0 {
            continue;
        }
        let media = live.borrow().as_ref().map(|l| l.media.clone());
        if let Some(m) = media {
            m.send(flow, &buf[..n]);
        }
    }
}

/// L'agent de la Station ; il s'arrête quand on le lâche.
pub struct Agent {
    shared: Arc<AgentShared>,
    task: AbortOnDrop,
    _gc: AbortOnDrop,
}

impl fmt::Debug for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent({} chez {})", self.id(), self.shared.cfg.relay)
    }
}

impl Agent {
    /// Lance l'agent (dans un runtime tokio) : il se connecte, se reconnecte
    /// de lui-même, et rend ce qui arrive du relais.
    pub fn start(cfg: AgentConfig) -> (Agent, mpsc::Receiver<Incoming>) {
        let (incoming, rx) = mpsc::channel(256);
        let (state, _) = watch::channel(AgentState::Connecting);
        let (live, _) = watch::channel(None);
        let shared = Arc::new(AgentShared {
            cfg,
            state,
            live,
            codes: Mutex::new(BTreeSet::new()),
            flows: Mutex::new(StationFlows::default()),
            incoming,
            active: ActiveCounter::default(),
        });
        let task = tokio::spawn(run(shared.clone()));
        let gc = tokio::spawn(collect_sockets(Arc::downgrade(&shared)));
        (Agent { shared, task: AbortOnDrop(task), _gc: AbortOnDrop(gc) }, rx)
    }

    /// L'identifiant de la Station.
    pub fn id(&self) -> &str {
        self.shared.cfg.identity.id()
    }

    pub fn relay(&self) -> &RelayAddr {
        &self.shared.cfg.relay
    }

    pub fn state(&self) -> AgentState {
        self.shared.state.borrow().clone()
    }

    /// Suit les changements d'état.
    pub fn subscribe(&self) -> watch::Receiver<AgentState> {
        self.shared.state.subscribe()
    }

    /// Aller-retour Station ↔ relais.
    pub fn rtt(&self) -> Option<Duration> {
        self.shared.current().and_then(|l| l.conn.rtt())
    }

    /// Le transport de la connexion en cours.
    pub fn transport(&self) -> Option<Transport> {
        self.shared.current().map(|l| l.transport)
    }

    /// Flux et flux UDP en cours.
    pub fn active(&self) -> usize {
        self.shared.active.count()
    }

    /// Réserve un code chez le relais : `preferred` s'il est libre (ou déjà
    /// à cette Station), sinon un autre. Gardé d'une reconnexion à l'autre.
    pub async fn reserve(&self, preferred: Option<&str>) -> Result<String, TunnelError> {
        let live = self.shared.wait_live(Duration::from_secs(5)).await?;
        let req = live.next_req.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        live.pending.lock().unwrap().insert(req, tx);
        let _ = live.ctl.send(Msg::Reserve { req, preferred: preferred.map(str::to_string) });
        match timeout(Duration::from_secs(5), rx).await {
            Ok(Ok(Msg::Reserved { code, .. })) => {
                self.shared.codes.lock().unwrap().insert(code.clone());
                Ok(code)
            }
            Ok(Ok(Msg::Error(ErrorMsg { code, message, .. }))) => Err(TunnelError::Refused(code, message)),
            Ok(_) => Err(TunnelError::Closed),
            Err(_) => Err(TunnelError::Timeout),
        }
    }

    /// Libère un code (partage arrêté, « Nouveau code »).
    pub fn release(&self, code: &str) {
        self.shared.codes.lock().unwrap().remove(code);
        if let Some(live) = self.shared.current() {
            let _ = live.ctl.send(Msg::Release { code: code.to_string() });
        }
    }

    /// Les codes tenus chez le relais.
    pub fn codes(&self) -> Vec<String> {
        self.shared.codes.lock().unwrap().iter().cloned().collect()
    }

    /// Ferme les flux UDP de ce client (autorisation retirée).
    pub fn close_client(&self, client: &str) {
        let live = self.shared.current();
        let mut flows = self.shared.flows.lock().unwrap();
        let keys: Vec<FlowKey> = flows.by_key.keys().filter(|(c, _)| c == client).cloned().collect();
        for key in keys {
            if let Some(s) = flows.by_key.remove(&key) {
                let flow = s.relay_flow.load(Ordering::Relaxed);
                if flow != 0 {
                    flows.by_relay.remove(&flow);
                    if let Some(l) = &live {
                        let _ = l.ctl.send(Msg::FlowClose { flow });
                    }
                }
            }
        }
    }

    /// Arrête l'agent tout de suite.
    pub fn stop(&self) {
        self.task.0.abort();
        if let Some(l) = self.shared.live.send_replace(None) {
            l.conn.close("arrêt");
        }
        self.shared.flows.lock().unwrap().orphan_all();
        self.shared.state.send_replace(AgentState::Stopped);
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn collect_sockets(shared: Weak<AgentShared>) {
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tick.tick().await;
        let Some(shared) = shared.upgrade() else { return };
        shared.flows.lock().unwrap().by_key.retain(|_, s| s.orphan_since.is_none_or(|t| t.elapsed() < FLOW_SOCKET_KEEP));
    }
}

fn dial_config(shared: &AgentShared) -> DialConfig {
    let id = &shared.cfg.identity;
    DialConfig {
        relay: shared.cfg.relay.clone(),
        roots: shared.cfg.roots.clone(),
        auth: Some(id.client_auth()),
        keep_alive: STATION_KEEP_ALIVE,
        hello: Hello {
            v: crate::PROTOCOL_VERSION,
            role: Role::Station,
            station: None,
            token: None,
            admin_fp: Some(shared.cfg.admin_fp.clone()),
            sig: Some(id.sign_admin_fp(&shared.cfg.admin_fp)),
            codes: shared.codes.lock().unwrap().iter().cloned().collect(),
            version: Some(shared.cfg.version.clone()),
        },
        with_media: true,
        force_tcp: shared.cfg.force_tcp,
    }
}

enum End {
    Lost,
    Replaced,
    Upgrade(Box<Session>),
}

async fn run(shared: Arc<AgentShared>) {
    let mut attempt = 0u32;
    let mut next: Option<Session> = None;
    loop {
        let result = match next.take() {
            Some(s) => Ok(s),
            None => dial(&dial_config(&shared)).await,
        };
        match result {
            Ok(session) => {
                attempt = 0;
                match serve(&shared, session).await {
                    End::Upgrade(s) => {
                        next = Some(*s);
                        continue;
                    }
                    End::Replaced => {
                        log::warn!("relais : connexion de la Station remplacée par une autre");
                        shared.state.send_replace(AgentState::Unreachable { reason: "replaced".into() });
                    }
                    End::Lost => {
                        shared.state.send_replace(AgentState::Unreachable { reason: "connexion perdue".into() });
                    }
                }
            }
            Err(e) => {
                log::info!("relais {} : {e}", shared.cfg.relay);
                let permanent = matches!(e, DialError::Refused(ErrorCode::Banned | ErrorCode::Version, _));
                let reason = match &e {
                    DialError::Unreachable(why) => why.clone(),
                    DialError::Refused(code, _) => code.as_str().to_string(),
                };
                if permanent {
                    shared.state.send_replace(AgentState::Stopped);
                    return;
                }
                shared.state.send_replace(AgentState::Unreachable { reason });
            }
        }
        tokio::time::sleep(backoff(attempt)).await;
        attempt = attempt.saturating_add(1);
    }
}

async fn serve(shared: &Arc<AgentShared>, session: Session) -> End {
    let Session { conn, ctl_tx, mut ctl_rx, media, welcome, transport, _endpoint, _writer } = session;
    let Some((media_tx, media_rx)) = media else { return End::Lost };
    // Les codes que le relais n'a pas gardés pendant la coupure sont perdus :
    // Admin en réserve un autre quand l'état repasse à « joint ».
    {
        let kept: BTreeSet<String> = welcome.codes.iter().cloned().collect();
        shared.codes.lock().unwrap().retain(|c| kept.contains(c));
    }
    let live = Arc::new(Live {
        conn: conn.clone(),
        ctl: ctl_tx.clone(),
        media: media_tx,
        transport,
        next_req: AtomicU32::new(1),
        pending: Mutex::new(HashMap::new()),
    });
    shared.live.send_replace(Some(live.clone()));
    shared.state.send_replace(AgentState::Joined { transport });
    log::info!("relais {} joint en {transport} (Station {})", shared.cfg.relay, welcome.id);
    let _media_task = AbortOnDrop(tokio::spawn(media_loop(shared.clone(), media_rx)));
    let _streams = AbortOnDrop(tokio::spawn(accept_streams(shared.clone(), conn.clone())));
    let pinger = (transport == Transport::Tcp).then(|| Pinger::start(conn.clone(), ctl_tx.clone()));
    // Sur le repli TCP, QUIC est retenté toutes les 5 minutes, au repos.
    let (upgrade_tx, mut upgrade_rx) = oneshot::channel::<Session>();
    let _upgrader = (transport == Transport::Tcp).then(|| AbortOnDrop(tokio::spawn(retry_quic(shared.clone(), upgrade_tx))));
    let mut upgrade_open = transport == Transport::Tcp;
    let end = loop {
        let msg = tokio::select! {
            m = read_msg(&mut ctl_rx) => m,
            s = &mut upgrade_rx, if upgrade_open => match s {
                Ok(s) => break End::Upgrade(Box::new(s)),
                Err(_) => {
                    upgrade_open = false;
                    continue;
                }
            },
        };
        let Ok(Some(msg)) = msg else { break End::Lost };
        match msg {
            Msg::FlowOpen { flow, client, key, ip } => {
                let req = FlowRequest {
                    flow,
                    client,
                    key,
                    ip,
                    shared: shared.clone(),
                    live: Arc::downgrade(&live),
                    answered: false,
                };
                // File pleine : la demande est lâchée, donc refusée.
                let _ = shared.incoming.try_send(Incoming::Flow(req));
            }
            Msg::FlowClose { flow } => shared.flows.lock().unwrap().orphan(flow),
            Msg::ClientGone { client } => {
                {
                    let mut flows = shared.flows.lock().unwrap();
                    let gone: Vec<u64> = flows.by_relay.iter().filter(|(_, (c, _))| *c == client).map(|(f, _)| *f).collect();
                    for f in gone {
                        flows.orphan(f);
                    }
                }
                let _ = shared.incoming.try_send(Incoming::ClientGone { client });
            }
            Msg::Reserved { req, .. } | Msg::Error(ErrorMsg { req: Some(req), .. }) => {
                let waiter = live.pending.lock().unwrap().remove(&req);
                if let Some(w) = waiter {
                    let _ = w.send(msg);
                }
            }
            Msg::Error(ErrorMsg { req: None, code, message }) => {
                log::info!("relais : {} {}", code.as_str(), message.unwrap_or_default());
                if code == ErrorCode::Replaced {
                    break End::Replaced;
                }
            }
            Msg::Ping { n } => {
                let _ = ctl_tx.send(Msg::Pong { n });
            }
            Msg::Pong { n } => {
                if let Some(p) = &pinger {
                    p.pong(n);
                }
            }
            _ => {}
        }
    };
    shared.live.send_replace(None);
    shared.flows.lock().unwrap().orphan_all();
    conn.close("fin");
    end
}

async fn retry_quic(shared: Arc<AgentShared>, upgrade: oneshot::Sender<Session>) {
    loop {
        tokio::time::sleep(QUIC_RETRY_EVERY).await;
        if shared.active.count() > 0 {
            continue;
        }
        if let Ok(s) = dial_quic_only(&dial_config(&shared)).await {
            let _ = upgrade.send(s);
            return;
        }
    }
}

async fn accept_streams(shared: Arc<AgentShared>, conn: Conn) {
    while let Some(mut stream) = conn.accept().await {
        let shared = shared.clone();
        tokio::spawn(async move {
            match timeout(HEADER_TIMEOUT, read_msg(&mut stream)).await {
                Ok(Ok(Some(Msg::Stream(header)))) => {
                    let stream = stream.counted(&shared.active);
                    if shared.incoming.try_send(Incoming::Stream { header, stream }).is_err() {
                        log::debug!("relais : flux lâché (Admin ne suit pas)");
                    }
                }
                _ => stream.reset(),
            }
        });
    }
}

async fn media_loop(shared: Arc<AgentShared>, mut rx: MediaReceiver) {
    while let Some((flow, payload)) = rx.recv().await {
        let socket = {
            let flows = shared.flows.lock().unwrap();
            flows.by_relay.get(&flow).and_then(|k| flows.by_key.get(k)).map(|s| s.socket.clone())
        };
        if let Some(s) = socket {
            let _ = s.try_send(&payload);
        }
    }
}
