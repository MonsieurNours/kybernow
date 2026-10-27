//! Le tunnel d'une app vers une Station, par le relais (docs/relais.md,
//! « Cœur et apps ») : une connexion tenue (reconnexion d'elle-même), des
//! flux vers Admin et Kyber, des flux UDP, et des points d'écoute sur la
//! boucle locale pour que la console et kyclient marchent tels quels :
//!
//! - [`Tunnel::listen_admin`] : `127.0.0.1:A` en TCP, chaque connexion devient
//!   un flux `admin_tls` (TLS de bout en bout, empreinte d'Admin épinglée) ;
//! - [`Tunnel::listen_kyber`] : `127.0.0.1:L` en TCP **et** en UDP sur le même
//!   numéro ; TCP devient `kyber_tcp`, chaque source UDP un flux UDP.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use rustls::RootCertStore;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::timeout;

use crate::addr::RelayAddr;
use crate::dial::{DialConfig, DialError, Pinger, Session, backoff, dial};
pub use crate::error::TunnelError;
use crate::frame::{read_msg, write_msg};
use crate::id;
use crate::msg::{ErrorCode, ErrorMsg, Hello, Msg, Role, Service};
use crate::quic::CLIENT_KEEP_ALIVE;
use crate::transport::{AbortOnDrop, ActiveCounter, Conn, MediaReceiver, MediaSender, RelayStream, Transport};

/// Paquets gardés pour un flux UDP pas encore prêt (le premier paquet QUIC
/// de kyclient arrive avant `flow_ready`).
const QUEUE_BEFORE_READY: usize = 64;
/// Délai pour qu'un flux soit accepté ou refusé par la Station.
const FLOW_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai d'attente de la connexion quand elle se refait.
const LIVE_TIMEOUT: Duration = Duration::from_secs(10);
/// Une source UDP muette depuis ce temps libère son flux.
const SOURCE_IDLE: Duration = Duration::from_secs(60);
/// Flux UDP par client, au plus (limite du relais).
const MAX_FLOWS: usize = 4;

/// Le tunnel à ouvrir.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub relay: RelayAddr,
    /// L'identifiant de la Station.
    pub station: String,
    /// Autorités à la place de celles du web (essais).
    pub roots: Option<Arc<RootCertStore>>,
    /// Toujours le repli TCP (essais).
    pub force_tcp: bool,
    /// Version de l'app, pour les journaux du relais.
    pub version: Option<String>,
}

impl ClientConfig {
    pub fn new(relay: RelayAddr, station: impl Into<String>) -> Self {
        ClientConfig { relay, station: station.into(), roots: None, force_tcp: false, version: None }
    }
}

/// L'état du tunnel, pour l'indicateur « Par le relais » et la ligne « Chemin ».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelStats {
    /// Connecté au relais en ce moment.
    pub connected: bool,
    pub transport: Option<Transport>,
    /// Aller-retour app ↔ relais.
    pub rtt_client: Option<Duration>,
    /// Aller-retour relais ↔ Station (message `stats` du relais).
    pub rtt_station: Option<Duration>,
    pub station_online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FlowState {
    Pending,
    Ready,
    Refused(String),
}

struct FlowEntry {
    relay_flow: Option<u64>,
    /// Une demande `open_flow` est partie sur la connexion en cours.
    requested: bool,
    to_local: Option<mpsc::Sender<Bytes>>,
    queued: VecDeque<Bytes>,
    state: watch::Sender<FlowState>,
}

#[derive(Default)]
struct Flows {
    by_key: HashMap<u32, FlowEntry>,
    by_relay: HashMap<u64, u32>,
}

impl Flows {
    /// La connexion (ou la Station) est partie : plus aucun numéro du relais.
    fn unbind_all(&mut self) {
        self.by_relay.clear();
        for e in self.by_key.values_mut() {
            e.relay_flow = None;
            e.requested = false;
            if *e.state.borrow() == FlowState::Ready {
                e.state.send_replace(FlowState::Pending);
            }
        }
    }
}

struct Live {
    conn: Conn,
    ctl: mpsc::UnboundedSender<Msg>,
    media: MediaSender,
    transport: Transport,
    station_online: AtomicBool,
    rtt_station_ms: AtomicU32,
    next_req: AtomicU32,
    /// `open_flow` en attente : numéro de demande → clé du flux.
    open_reqs: Mutex<HashMap<u32, u32>>,
    /// Autres demandes (`status`).
    pending: Mutex<HashMap<u32, oneshot::Sender<Msg>>>,
}

impl Live {
    fn req(&self) -> u32 {
        self.next_req.fetch_add(1, Ordering::Relaxed)
    }
}

struct Shared {
    cfg: ClientConfig,
    token: String,
    client_id: String,
    live: watch::Sender<Option<Arc<Live>>>,
    flows: Mutex<Flows>,
    active: ActiveCounter,
    last_error: Mutex<Option<TunnelError>>,
}

impl Shared {
    fn current(&self) -> Option<Arc<Live>> {
        self.live.borrow().clone()
    }

    async fn wait_live(&self, within: Duration) -> Result<Arc<Live>, TunnelError> {
        let mut rx = self.live.subscribe();
        match timeout(within, rx.wait_for(|l| l.is_some())).await {
            Ok(Ok(l)) => Ok(l.clone().expect("connexion présente")),
            Ok(Err(_)) => Err(TunnelError::Closed),
            Err(_) => Err(self.last_error.lock().unwrap().clone().unwrap_or(TunnelError::Timeout)),
        }
    }

    /// Demande au relais d'ouvrir le flux de cette clé.
    fn request_flow(&self, live: &Live, key: u32) {
        {
            let mut flows = self.flows.lock().unwrap();
            match flows.by_key.get_mut(&key) {
                Some(e) if !e.requested => e.requested = true,
                _ => return,
            }
        }
        let req = live.req();
        live.open_reqs.lock().unwrap().insert(req, key);
        let _ = live.ctl.send(Msg::OpenFlow { req, key });
    }

    fn reopen_flows(&self, live: &Live) {
        let keys: Vec<u32> = {
            let flows = self.flows.lock().unwrap();
            flows.by_key.iter().filter(|(_, e)| matches!(*e.state.borrow(), FlowState::Pending)).map(|(k, _)| *k).collect()
        };
        for key in keys {
            self.request_flow(live, key);
        }
    }

    fn flow_ready(&self, live: &Live, key: u32, flow: u64) {
        let mut flows = self.flows.lock().unwrap();
        let Some(e) = flows.by_key.get_mut(&key) else {
            // Le flux a été lâché entre-temps.
            let _ = live.ctl.send(Msg::FlowClose { flow });
            return;
        };
        e.requested = false;
        let old = e.relay_flow.replace(flow);
        for p in e.queued.drain(..) {
            live.media.send(flow, &p);
        }
        e.state.send_replace(FlowState::Ready);
        flows.by_relay.insert(flow, key);
        if let Some(old) = old.filter(|o| *o != flow) {
            flows.by_relay.remove(&old);
            let _ = live.ctl.send(Msg::FlowClose { flow: old });
        }
    }

    fn flow_refused(&self, key: u32, reason: String) {
        let mut flows = self.flows.lock().unwrap();
        if let Some(e) = flows.by_key.get_mut(&key) {
            e.relay_flow = None;
            e.requested = false;
            e.queued.clear();
            e.to_local = None;
            e.state.send_replace(FlowState::Refused(reason));
        }
    }
}

/// Le tunnel d'une app vers une Station. Se clone à peu de frais ; il se
/// ferme quand le dernier clone (et le dernier point d'écoute) est lâché.
#[derive(Clone)]
pub struct Tunnel(Arc<TunnelInner>);

struct TunnelInner {
    shared: Arc<Shared>,
    _task: AbortOnDrop,
}

impl fmt::Debug for Tunnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tunnel({} → {})", self.0.shared.cfg.relay, self.0.shared.cfg.station)
    }
}

impl Tunnel {
    /// Ouvre le tunnel et attend la première réponse du relais. Le tunnel
    /// s'ouvre même si la Station n'est pas là (`stats().station_online`) :
    /// les flux le diront ([`TunnelError::StationOffline`]). Une première
    /// connexion impossible rend l'erreur ; ensuite, le tunnel se reconnecte
    /// de lui-même. À appeler dans un runtime tokio.
    pub async fn open(cfg: ClientConfig) -> Result<Tunnel, TunnelError> {
        let token = id::new_token();
        let client_id = id::client_id(&token);
        let (live, _) = watch::channel(None);
        let shared = Arc::new(Shared {
            cfg,
            token,
            client_id,
            live,
            flows: Mutex::new(Flows::default()),
            active: ActiveCounter::default(),
            last_error: Mutex::new(None),
        });
        let (first_tx, first_rx) = oneshot::channel();
        let task = tokio::spawn(run(shared.clone(), first_tx));
        let tunnel = Tunnel(Arc::new(TunnelInner { shared, _task: AbortOnDrop(task) }));
        match first_rx.await {
            Ok(Ok(())) => Ok(tunnel),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(TunnelError::Closed),
        }
    }

    fn shared(&self) -> &Arc<Shared> {
        &self.0.shared
    }

    /// L'identifiant de ce client chez le relais et la Station.
    pub fn client_id(&self) -> &str {
        &self.shared().client_id
    }

    pub fn station(&self) -> &str {
        &self.shared().cfg.station
    }

    pub fn relay(&self) -> &RelayAddr {
        &self.shared().cfg.relay
    }

    pub fn stats(&self) -> TunnelStats {
        match self.shared().current() {
            None => TunnelStats::default(),
            Some(l) => TunnelStats {
                connected: true,
                transport: Some(l.transport),
                rtt_client: l.conn.rtt(),
                rtt_station: match l.rtt_station_ms.load(Ordering::Relaxed) {
                    0 => None,
                    ms => Some(Duration::from_millis(u64::from(ms))),
                },
                station_online: l.station_online.load(Ordering::Relaxed),
            },
        }
    }

    pub fn station_online(&self) -> bool {
        self.shared().current().is_some_and(|l| l.station_online.load(Ordering::Relaxed))
    }

    /// Attend que la Station soit connectée au relais.
    pub async fn wait_station(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        loop {
            if self.station_online() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Flux et flux UDP en cours (le cœur ferme le tunnel au repos).
    pub fn active(&self) -> usize {
        self.shared().active.count()
    }

    /// Ouvre un flux vers Admin (`admin_tls`) ou Kyber (`kyber_tcp`).
    pub async fn open_stream(&self, service: Service) -> Result<RelayStream, TunnelError> {
        let live = self.shared().wait_live(LIVE_TIMEOUT).await?;
        if !live.station_online.load(Ordering::Relaxed) {
            return Err(TunnelError::StationOffline);
        }
        let mut s = live.conn.open().await.map_err(|e| TunnelError::Io(e.to_string()))?;
        write_msg(&mut s, &Msg::Open { service }).await.map_err(|e| TunnelError::Io(e.to_string()))?;
        Ok(s.counted(&self.shared().active))
    }

    /// Un flux UDP vers Kyber, de clé `key` (stable tant que la source vit) :
    /// les paquets envoyés avant que la Station l'accepte sont gardés.
    pub fn flow(&self, key: u32) -> Flow {
        let shared = self.shared().clone();
        let (to_local, rx) = mpsc::channel(1024);
        let (state, state_rx) = watch::channel(FlowState::Pending);
        shared.flows.lock().unwrap().by_key.insert(
            key,
            FlowEntry { relay_flow: None, requested: false, to_local: Some(to_local), queued: VecDeque::new(), state },
        );
        if let Some(live) = shared.current()
            && live.station_online.load(Ordering::Relaxed)
        {
            shared.request_flow(&live, key);
        }
        Flow {
            sender: FlowSender { key, shared: shared.clone() },
            receiver: FlowReceiver { key, shared, rx, state: state_rx, _guard: self.shared().active.guard() },
        }
    }

    /// [`Tunnel::flow`], puis attend que la Station l'accepte.
    pub async fn open_flow(&self, key: u32) -> Result<Flow, TunnelError> {
        let mut flow = self.flow(key);
        flow.receiver.ready().await?;
        Ok(flow)
    }

    /// Demande au relais quelles Stations sont connectées (16 au plus).
    pub async fn presence(&self, stations: &[String]) -> Result<Vec<bool>, TunnelError> {
        let live = self.shared().wait_live(LIVE_TIMEOUT).await?;
        ask_presence(&live, stations).await
    }

    /// Point d'écoute vers Admin : `127.0.0.1:A` en TCP.
    pub async fn listen_admin(&self) -> io::Result<LocalListener> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let task = tokio::spawn(accept_tcp(listener, self.clone(), Service::AdminTls));
        Ok(LocalListener { addr, _tasks: vec![AbortOnDrop(task)] })
    }

    /// Point d'écoute vers Kyber : `127.0.0.1:L` en TCP et en UDP, même
    /// numéro (kyclient annonce ce port à Kyber, qui le lui rend pour QUIC).
    pub async fn listen_kyber(&self) -> io::Result<LocalListener> {
        let (tcp, udp) = bind_pair()?;
        let addr = tcp.local_addr()?;
        let tcp = TcpListener::from_std(tcp)?;
        let udp = Arc::new(UdpSocket::from_std(udp)?);
        let t1 = tokio::spawn(accept_tcp(tcp, self.clone(), Service::KyberTcp));
        let t2 = tokio::spawn(bridge_udp(udp, self.clone()));
        Ok(LocalListener { addr, _tasks: vec![AbortOnDrop(t1), AbortOnDrop(t2)] })
    }

    /// Ferme la connexion au relais tout de suite (les clones ne servent plus).
    pub fn close(&self) {
        self.0._task.0.abort();
        if let Some(l) = self.shared().live.send_replace(None) {
            l.conn.close("fermé");
        }
    }
}

/// Un point d'écoute sur la boucle locale ; il se ferme quand on le lâche.
#[derive(Debug)]
pub struct LocalListener {
    addr: SocketAddr,
    _tasks: Vec<AbortOnDrop>,
}

impl LocalListener {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }
}

/// Un flux UDP : l'envoi et la réception.
pub struct Flow {
    pub sender: FlowSender,
    pub receiver: FlowReceiver,
}

impl Flow {
    pub fn send(&self, payload: &[u8]) -> bool {
        self.sender.send(payload)
    }

    pub async fn recv(&mut self) -> Option<Bytes> {
        self.receiver.recv().await
    }

    pub fn into_split(self) -> (FlowSender, FlowReceiver) {
        (self.sender, self.receiver)
    }
}

/// L'envoi d'un flux UDP.
#[derive(Clone)]
pub struct FlowSender {
    key: u32,
    shared: Arc<Shared>,
}

impl FlowSender {
    /// Envoie un paquet ; gardé tant que le flux n'est pas prêt (64 au plus).
    /// Faux si le flux est refusé ou fermé.
    pub fn send(&self, payload: &[u8]) -> bool {
        let live = self.shared.current();
        let mut flows = self.shared.flows.lock().unwrap();
        let Some(e) = flows.by_key.get_mut(&self.key) else { return false };
        let refused = matches!(*e.state.borrow(), FlowState::Refused(_));
        match (refused, e.relay_flow, live) {
            (true, _, _) => false,
            (_, Some(flow), Some(live)) => {
                live.media.send(flow, payload);
                true
            }
            _ => {
                if e.queued.len() >= QUEUE_BEFORE_READY {
                    e.queued.pop_front();
                }
                e.queued.push_back(Bytes::copy_from_slice(payload));
                true
            }
        }
    }
}

/// La réception d'un flux UDP ; le lâcher ferme le flux.
pub struct FlowReceiver {
    key: u32,
    shared: Arc<Shared>,
    rx: mpsc::Receiver<Bytes>,
    state: watch::Receiver<FlowState>,
    _guard: crate::transport::ActiveGuard,
}

impl FlowReceiver {
    /// Le prochain paquet de Kyber ; `None` quand la Station ferme ou refuse le flux.
    pub async fn recv(&mut self) -> Option<Bytes> {
        self.rx.recv().await
    }

    /// Attend que la Station accepte le flux.
    pub async fn ready(&mut self) -> Result<(), TunnelError> {
        let state = timeout(FLOW_TIMEOUT, self.state.wait_for(|s| *s != FlowState::Pending)).await;
        match state {
            Err(_) => {
                if self.shared.current().is_some_and(|l| !l.station_online.load(Ordering::Relaxed)) {
                    Err(TunnelError::StationOffline)
                } else {
                    Err(TunnelError::Timeout)
                }
            }
            Ok(Err(_)) => Err(TunnelError::Closed),
            Ok(Ok(s)) => match &*s {
                FlowState::Ready => Ok(()),
                FlowState::Refused(why) => Err(TunnelError::FlowRefused(why.clone())),
                FlowState::Pending => unreachable!(),
            },
        }
    }
}

impl Drop for FlowReceiver {
    fn drop(&mut self) {
        let removed = self.shared.flows.lock().unwrap().by_key.remove(&self.key);
        if let Some(flow) = removed.and_then(|e| e.relay_flow) {
            self.shared.flows.lock().unwrap().by_relay.remove(&flow);
            if let Some(live) = self.shared.current() {
                let _ = live.ctl.send(Msg::FlowClose { flow });
            }
        }
    }
}

async fn ask_presence(live: &Live, stations: &[String]) -> Result<Vec<bool>, TunnelError> {
    let req = live.req();
    let (tx, rx) = oneshot::channel();
    live.pending.lock().unwrap().insert(req, tx);
    let _ = live.ctl.send(Msg::Status { req, stations: stations.to_vec() });
    match timeout(Duration::from_secs(5), rx).await {
        Ok(Ok(Msg::Presence { online, .. })) => Ok(online),
        Ok(Ok(Msg::Error(ErrorMsg { code, message, .. }))) => Err(TunnelError::Refused(code, message)),
        Ok(_) => Err(TunnelError::Closed),
        Err(_) => Err(TunnelError::Timeout),
    }
}

/// Demande au relais quelles Stations sont connectées, sans tunnel (accueil).
pub async fn presence(relay: &RelayAddr, roots: Option<Arc<RootCertStore>>, stations: &[String]) -> Result<Vec<bool>, TunnelError> {
    let cfg = DialConfig {
        relay: relay.clone(),
        roots,
        auth: None,
        keep_alive: CLIENT_KEEP_ALIVE,
        hello: Hello {
            v: crate::PROTOCOL_VERSION,
            role: Role::Client,
            station: None,
            token: Some(id::new_token()),
            admin_fp: None,
            sig: None,
            codes: vec![],
            version: None,
        },
        with_media: false,
        force_tcp: false,
    };
    let mut session = dial(&cfg).await?;
    let req = 1;
    let _ = session.ctl_tx.send(Msg::Status { req, stations: stations.to_vec() });
    let answer = timeout(Duration::from_secs(5), async {
        loop {
            match read_msg(&mut session.ctl_rx).await {
                Ok(Some(Msg::Presence { req: r, online })) if r == req => return Ok(online),
                Ok(Some(Msg::Error(ErrorMsg { code, message, .. }))) => return Err(TunnelError::Refused(code, message)),
                Ok(Some(_)) => continue,
                _ => return Err(TunnelError::Closed),
            }
        }
    })
    .await
    .unwrap_or(Err(TunnelError::Timeout));
    session.close("présence");
    answer
}

async fn run(shared: Arc<Shared>, first: oneshot::Sender<Result<(), TunnelError>>) {
    let mut first = Some(first);
    let mut attempt = 0u32;
    loop {
        let cfg = DialConfig {
            relay: shared.cfg.relay.clone(),
            roots: shared.cfg.roots.clone(),
            auth: None,
            keep_alive: CLIENT_KEEP_ALIVE,
            hello: Hello {
                v: crate::PROTOCOL_VERSION,
                role: Role::Client,
                station: Some(shared.cfg.station.clone()),
                token: Some(shared.token.clone()),
                admin_fp: None,
                sig: None,
                codes: vec![],
                version: shared.cfg.version.clone(),
            },
            with_media: true,
            force_tcp: shared.cfg.force_tcp,
        };
        match dial(&cfg).await {
            Ok(session) => {
                attempt = 0;
                *shared.last_error.lock().unwrap() = None;
                if let Some(f) = first.take() {
                    let _ = f.send(Ok(()));
                }
                let replaced = serve(&shared, session).await;
                if replaced {
                    log::info!("tunnel vers {} : remplacé par une autre connexion", shared.cfg.station);
                    return;
                }
            }
            Err(e) => {
                let err = TunnelError::from(e.clone());
                if let Some(f) = first.take() {
                    let _ = f.send(Err(err));
                    return;
                }
                let permanent =
                    matches!(e, DialError::Refused(ErrorCode::Banned | ErrorCode::Version | ErrorCode::BadRequest, _));
                log::info!("tunnel vers {} : {e}", shared.cfg.station);
                *shared.last_error.lock().unwrap() = Some(err);
                if permanent {
                    return;
                }
            }
        }
        tokio::time::sleep(backoff(attempt)).await;
        attempt = attempt.saturating_add(1);
    }
}

/// Sert une connexion jusqu'à sa fin ; vrai si le relais l'a remplacée.
async fn serve(shared: &Arc<Shared>, session: Session) -> bool {
    let Session { conn, ctl_tx, mut ctl_rx, media, welcome, transport, _endpoint, _writer } = session;
    let Some((media_tx, media_rx)) = media else { return false };
    let live = Arc::new(Live {
        conn: conn.clone(),
        ctl: ctl_tx.clone(),
        media: media_tx,
        transport,
        station_online: AtomicBool::new(welcome.online.unwrap_or(false)),
        rtt_station_ms: AtomicU32::new(0),
        next_req: AtomicU32::new(1),
        open_reqs: Mutex::new(HashMap::new()),
        pending: Mutex::new(HashMap::new()),
    });
    shared.live.send_replace(Some(live.clone()));
    if live.station_online.load(Ordering::Relaxed) {
        shared.reopen_flows(&live);
    }
    let _media_task = AbortOnDrop(tokio::spawn(media_loop(shared.clone(), media_rx)));
    let pinger = (transport == Transport::Tcp).then(|| Pinger::start(conn.clone(), ctl_tx.clone()));
    let mut replaced = false;
    while let Ok(Some(msg)) = read_msg(&mut ctl_rx).await {
        match msg {
            Msg::FlowReady { req, flow } => {
                let key = live.open_reqs.lock().unwrap().remove(&req);
                if let Some(key) = key {
                    shared.flow_ready(&live, key, flow);
                }
            }
            Msg::FlowRefused { req, reason } => {
                let key = live.open_reqs.lock().unwrap().remove(&req);
                if let Some(key) = key {
                    shared.flow_refused(key, reason);
                }
            }
            Msg::FlowClose { flow } => {
                let key = shared.flows.lock().unwrap().by_relay.remove(&flow);
                if let Some(key) = key {
                    shared.flow_refused(key, "closed".into());
                }
            }
            Msg::Station { online } => {
                live.station_online.store(online, Ordering::Relaxed);
                if online {
                    shared.reopen_flows(&live);
                } else {
                    live.rtt_station_ms.store(0, Ordering::Relaxed);
                    shared.flows.lock().unwrap().unbind_all();
                }
            }
            Msg::Stats { rtt_station_ms } => {
                // 0 veut dire « pas encore mesuré » : une mesure de 0 ms devient 1.
                let ms = rtt_station_ms.map_or(0, |ms| ms.max(1));
                live.rtt_station_ms.store(ms, Ordering::Relaxed);
            }
            Msg::Presence { req, .. } => {
                let waiter = live.pending.lock().unwrap().remove(&req);
                if let Some(w) = waiter {
                    let _ = w.send(msg);
                }
            }
            Msg::Error(ErrorMsg { req: Some(req), code, ref message }) => {
                let key = live.open_reqs.lock().unwrap().remove(&req);
                if let Some(key) = key {
                    shared.flow_refused(key, message.clone().unwrap_or_else(|| code.as_str().into()));
                } else if let Some(w) = live.pending.lock().unwrap().remove(&req) {
                    let _ = w.send(msg);
                }
            }
            Msg::Error(ErrorMsg { req: None, code, message }) => {
                log::info!("relais : {} {}", code.as_str(), message.unwrap_or_default());
                if code == ErrorCode::Replaced {
                    replaced = true;
                    break;
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
    }
    shared.live.send_replace(None);
    shared.flows.lock().unwrap().unbind_all();
    conn.close("fin");
    replaced
}

async fn media_loop(shared: Arc<Shared>, mut rx: MediaReceiver) {
    while let Some((flow, payload)) = rx.recv().await {
        let flows = shared.flows.lock().unwrap();
        let Some(key) = flows.by_relay.get(&flow) else { continue };
        if let Some(tx) = flows.by_key.get(key).and_then(|e| e.to_local.as_ref()) {
            // File pleine : perdu, comme en UDP.
            let _ = tx.try_send(payload);
        }
    }
}

async fn accept_tcp(listener: TcpListener, tunnel: Tunnel, service: Service) {
    while let Ok((mut tcp, _)) = listener.accept().await {
        let tunnel = tunnel.clone();
        tokio::spawn(async move {
            let _ = tcp.set_nodelay(true);
            match tunnel.open_stream(service).await {
                Ok(mut s) => {
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut s).await;
                }
                Err(e) => log::info!("tunnel : flux {service:?} impossible ({e})"),
            }
        });
    }
}

/// TCP puis UDP sur le même numéro, sur la boucle locale (on recommence si
/// le numéro est déjà pris en UDP).
fn bind_pair() -> io::Result<(std::net::TcpListener, std::net::UdpSocket)> {
    for _ in 0..32 {
        let tcp = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = tcp.local_addr()?.port();
        if let Ok(udp) = std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, port)) {
            tcp.set_nonblocking(true)?;
            udp.set_nonblocking(true)?;
            return Ok((tcp, udp));
        }
    }
    Err(io::Error::new(io::ErrorKind::AddrInUse, "pas de port libre en TCP et en UDP"))
}

struct Source {
    sender: FlowSender,
    last: Instant,
    /// Flux refusé ou fermé par la Station : la source se tait jusque-là,
    /// puis redemande un flux (sans marteler la Station à chaque paquet).
    dead_until: Option<Instant>,
    _reader: AbortOnDrop,
}

/// Silence d'une source dont le flux a été refusé.
const REFUSED_PAUSE: Duration = Duration::from_secs(5);

/// Chaque source UDP (un socket de kyclient) devient un flux du tunnel.
async fn bridge_udp(udp: Arc<UdpSocket>, tunnel: Tunnel) {
    let mut sources: HashMap<SocketAddr, Source> = HashMap::new();
    let mut next_key = 1u32;
    let mut buf = vec![0u8; 65_536];
    let mut sweep = tokio::time::interval(Duration::from_secs(10));
    loop {
        tokio::select! {
            r = udp.recv_from(&mut buf) => {
                let Ok((n, from)) = r else { return };
                let now = Instant::now();
                if sources.get(&from).is_some_and(|s| s.dead_until.is_some_and(|t| now >= t)) {
                    sources.remove(&from);
                }
                if !sources.contains_key(&from) {
                    if sources.len() >= MAX_FLOWS {
                        // La plus ancienne source laisse sa place.
                        if let Some(oldest) = sources.iter().min_by_key(|(_, s)| s.last).map(|(a, _)| *a) {
                            sources.remove(&oldest);
                        }
                    }
                    let (sender, mut receiver) = tunnel.flow(next_key).into_split();
                    next_key = next_key.wrapping_add(1).max(1);
                    let udp = udp.clone();
                    let reader = tokio::spawn(async move {
                        while let Some(p) = receiver.recv().await {
                            let _ = udp.send_to(&p, from).await;
                        }
                    });
                    sources.insert(from, Source { sender, last: now, dead_until: None, _reader: AbortOnDrop(reader) });
                }
                let source = sources.get_mut(&from).expect("source présente");
                source.last = now;
                if source.dead_until.is_none() && !source.sender.send(&buf[..n]) {
                    source.dead_until = Some(now + REFUSED_PAUSE);
                }
            }
            _ = sweep.tick() => {
                sources.retain(|_, s| s.last.elapsed() < SOURCE_IDLE);
            }
        }
    }
}
