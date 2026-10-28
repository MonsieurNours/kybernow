//! Le cœur du relais : les Stations et les clients connectés, leurs messages,
//! les flux TCP et UDP qu'il fait suivre, les codes (docs/relais.md, « Le
//! tunnel »). Le relais ne fait suivre que vers une Station connectée, et
//! seulement ce qu'un client a demandé pour la Station qu'il a nommée ; c'est
//! Admin qui autorise.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};
use std::time::{Duration, Instant};

use base64::Engine as _;
use bytes::Bytes;
use kybernow_tunnel::frame::{read_msg, write_msg};
use kybernow_tunnel::id;
use kybernow_tunnel::lookup::{self, CodeLookup};
use kybernow_tunnel::msg::{ErrorCode, Hello, Msg, Role, Service, StreamHeader, StreamKind, Welcome};
use kybernow_tunnel::transport::{Conn, DropReason, MediaReceiver, MediaSender, RelayStream, Sent, Transport, media_over_stream};
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf};
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::certs::CertStore;
use crate::codes::Codes;
use crate::config::Config;
use crate::limits::{Banlist, Bucket, Buckets, LookupLimiter};
use crate::metrics::{Dg, Dir, Gauges, Leg, Metrics};

/// Version du relais, donnée dans `welcome` et `/v1/health`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Délai pour `hello`, l'en-tête d'un flux, la connexion des paquets.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);
/// Délai pour qu'Admin accepte ou refuse un flux UDP.
const FLOW_TIMEOUT: Duration = Duration::from_secs(10);
/// Repli TCP : sans message (les bouts envoient `ping` toutes les 5 s) pendant ce temps, coupure.
const TCP_SILENCE: Duration = Duration::from_secs(35);
/// Stations demandées dans un `status`, au plus.
const MAX_STATUS: usize = 16;
/// Messages en attente d'envoi vers un bout, au plus : un bout qui ne lit pas
/// ses réponses (`pong`, `presence`…) est coupé au lieu de faire grossir la
/// mémoire du relais.
const CTL_QUEUE: usize = 1024;

pub struct StationPeer {
    pub id: String,
    conn: Conn,
    ctl: Ctl,
    pub ip: IpAddr,
    admin_fp: String,
    sig: String,
    station_key: String,
    media: RwLock<Option<(u64, MediaSender)>>,
    next_flow: AtomicU64,
    /// Flux de la Station → (client, flux du client).
    flows: Mutex<HashMap<u64, (Weak<ClientPeer>, u64)>>,
    pending: Mutex<HashMap<u64, Pending>>,
}

struct Pending {
    client: Weak<ClientPeer>,
    req: u32,
    cflow: u64,
}

pub struct ClientPeer {
    pub id: String,
    conn: Conn,
    ctl: Ctl,
    pub ip: IpAddr,
    pub station: Option<String>,
    media: RwLock<Option<(u64, MediaSender)>>,
    next_flow: AtomicU64,
    /// Flux du client → (Station, flux de la Station).
    flows: Mutex<HashMap<u64, (Weak<StationPeer>, u64)>>,
}

pub struct Hub {
    pub cfg: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub banlist: Arc<Banlist>,
    pub certs: Arc<CertStore>,
    stations: Mutex<HashMap<String, Arc<StationPeer>>>,
    clients: Mutex<HashMap<String, Arc<ClientPeer>>>,
    codes: Mutex<Codes>,
    lookups: Mutex<LookupLimiter>,
    /// Connexions de Station par minute, par adresse.
    station_hellos: Mutex<Buckets<IpAddr>>,
    web_clients: AtomicUsize,
    next_media: AtomicU64,
    cap: Option<Mutex<Bucket>>,
    /// Port écrit dans les liens (réglé une fois le port lié).
    pub link_port: AtomicU16,
}

/// Une connexion de navigateur comptée tant qu'elle vit.
pub struct WebGuard(Arc<Hub>);

impl Drop for WebGuard {
    fn drop(&mut self) {
        self.0.web_clients.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Les messages vers un bout : une file bornée ([`CTL_QUEUE`]) ; pleine, le
/// bout ne lit plus ce qu'on lui envoie et sa connexion est coupée.
#[derive(Clone)]
struct Ctl {
    tx: mpsc::Sender<Msg>,
    conn: Conn,
}

impl Ctl {
    /// Faux si la connexion est finie, ou vient d'être coupée (file pleine).
    fn send(&self, msg: Msg) -> bool {
        match self.tx.try_send(msg) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::debug!("messages non lus : connexion coupée");
                self.conn.close("messages non lus");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }
}

fn start_writer<W: AsyncWrite + Unpin + Send + 'static>(mut w: W, conn: Conn) -> (Ctl, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<Msg>(CTL_QUEUE);
    let task = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if write_msg(&mut w, &m).await.is_err() {
                break;
            }
        }
    });
    (Ctl { tx, conn }, task)
}

/// Refuse une connexion : le message part, puis la connexion se ferme.
async fn refuse(conn: &Conn, tx: Ctl, writer: tokio::task::JoinHandle<()>, code: ErrorCode, why: &str) {
    let _ = tx.send(Msg::error(None, code, Some(why.to_string())));
    drop(tx);
    let _ = timeout(Duration::from_secs(1), writer).await;
    conn.close(why);
}

impl Hub {
    pub fn new(cfg: Arc<Config>, certs: Arc<CertStore>, banlist: Arc<Banlist>) -> Arc<Hub> {
        let now = Instant::now();
        let cap = (cfg.global_cap_mbps > 0).then(|| {
            let bytes_per_sec = (cfg.global_cap_mbps * 1_000_000 / 8).min(u64::from(u32::MAX)) as u32;
            Mutex::new(Bucket::new(bytes_per_sec, Duration::from_secs(1), now))
        });
        Arc::new(Hub {
            lookups: Mutex::new(LookupLimiter::new(&cfg.limits, now)),
            station_hellos: Mutex::new(Buckets::new(cfg.limits.stations_per_minute_per_ip, Duration::from_secs(60))),
            link_port: AtomicU16::new(cfg.listen.port()),
            cfg,
            metrics: Arc::new(Metrics::default()),
            banlist,
            certs,
            stations: Mutex::new(HashMap::new()),
            clients: Mutex::new(HashMap::new()),
            codes: Mutex::new(Codes::default()),
            web_clients: AtomicUsize::new(0),
            next_media: AtomicU64::new(1),
            cap,
        })
    }

    pub fn station(&self, id: &str) -> Option<Arc<StationPeer>> {
        self.stations.lock().unwrap().get(id).cloned()
    }

    fn client(&self, id: &str) -> Option<Arc<ClientPeer>> {
        self.clients.lock().unwrap().get(id).cloned()
    }

    pub fn web_guard(self: &Arc<Self>) -> WebGuard {
        self.web_clients.fetch_add(1, Ordering::Relaxed);
        WebGuard(self.clone())
    }

    pub fn gauges(&self) -> Gauges {
        let stations = self.stations.lock().unwrap();
        let clients = self.clients.lock().unwrap();
        let avg = |rtts: Vec<Duration>| (!rtts.is_empty()).then(|| rtts.iter().map(Duration::as_secs_f64).sum::<f64>() / rtts.len() as f64);
        Gauges {
            stations_quic: stations.values().filter(|s| s.conn.transport() == Transport::Quic).count(),
            stations_tcp: stations.values().filter(|s| s.conn.transport() == Transport::Tcp).count(),
            clients_app: clients.len(),
            clients_web: self.web_clients.load(Ordering::Relaxed),
            sessions: clients.values().filter(|c| !c.flows.lock().unwrap().is_empty()).count(),
            codes: self.codes.lock().unwrap().active(Instant::now()),
            rtt_station: avg(stations.values().filter_map(|s| s.conn.rtt()).collect()),
            rtt_client: avg(clients.values().filter_map(|c| c.conn.rtt()).collect()),
            cert_expiry: self.certs.expires_in(),
        }
    }

    /// Ménage périodique : seaux de recherche, codes tombés.
    pub fn sweep(&self) {
        let now = Instant::now();
        self.lookups.lock().unwrap().sweep(now);
        self.station_hellos.lock().unwrap().sweep(now);
        self.codes.lock().unwrap().sweep(now);
    }

    // --- Codes ---------------------------------------------------------------

    /// `GET /v1/codes/{code}` : `Err` = trop d'essais (réessayer dans ce temps).
    pub fn lookup_code(&self, code: &str, ip: IpAddr) -> Result<Option<CodeLookup>, Duration> {
        if let Err((wait, limit)) = self.lookups.lock().unwrap().check(ip, Instant::now()) {
            self.metrics.limit_hit(limit);
            self.metrics.lookup("limited");
            return Err(wait);
        }
        let station = self.codes.lock().unwrap().lookup(code, Instant::now());
        let found = station.map(|id| match self.station(&id) {
            Some(s) => CodeLookup {
                station: id,
                station_key: s.station_key.clone(),
                admin_fp: s.admin_fp.clone(),
                sig: s.sig.clone(),
                online: true,
            },
            // Station partie depuis moins de 120 s : on connaît encore son identifiant.
            None => CodeLookup { station: id, station_key: String::new(), admin_fp: String::new(), sig: String::new(), online: false },
        });
        self.metrics.lookup(if found.is_some() { "found" } else { "unknown" });
        Ok(found)
    }

    // --- Connexions des bouts ----------------------------------------------

    /// Sert une connexion de messages (`kn-ctl/1`, ou repli `kn-tcp/1`).
    /// `spki` : la clé du certificat client, s'il y en a un.
    pub async fn serve_ctl(self: Arc<Self>, conn: Conn, spki: Option<Vec<u8>>, ip: IpAddr) {
        let Ok(Some(ctl)) = timeout(HELLO_TIMEOUT, conn.accept()).await else {
            conn.close("pas de flux de messages");
            return;
        };
        let (mut read, write) = tokio::io::split(ctl);
        let hello = match timeout(HELLO_TIMEOUT, read_msg(&mut read)).await {
            Ok(Ok(Some(Msg::Hello(h)))) => h,
            _ => {
                conn.close("pas de hello");
                return;
            }
        };
        let (tx, writer) = start_writer(write, conn.clone());
        if hello.v == 0 {
            return refuse(&conn, tx, writer, ErrorCode::Version, "version").await;
        }
        match hello.role {
            Role::Station => self.station_session(conn, read, tx, writer, hello, spki, ip).await,
            Role::Client => self.client_session(conn, read, tx, writer, hello, ip).await,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn station_session(
        self: Arc<Self>,
        conn: Conn,
        mut read: ReadHalf<RelayStream>,
        tx: Ctl,
        writer: tokio::task::JoinHandle<()>,
        hello: Hello,
        spki: Option<Vec<u8>>,
        ip: IpAddr,
    ) {
        if self.station_hellos.lock().unwrap().take(&ip, Instant::now()).is_err() {
            self.metrics.limit_hit("stations_per_minute_ip");
            return refuse(&conn, tx, writer, ErrorCode::TooMany, "trop de connexions de Station pour cette adresse").await;
        }
        let Some(spki) = spki else {
            return refuse(&conn, tx, writer, ErrorCode::BadRequest, "certificat client manquant").await;
        };
        let (Some(admin_fp), Some(sig)) = (hello.admin_fp.clone(), hello.sig.clone()) else {
            return refuse(&conn, tx, writer, ErrorCode::BadRequest, "empreinte d'Admin manquante").await;
        };
        let sig_ok = base64::engine::general_purpose::STANDARD.decode(&sig).is_ok_and(|s| lookup::verify_sig(&spki, &admin_fp, &s));
        if !sig_ok {
            return refuse(&conn, tx, writer, ErrorCode::BadRequest, "signature de l'empreinte").await;
        }
        let id = id::station_id(&spki);
        let others = self.stations.lock().unwrap().values().filter(|s| s.ip == ip && s.id != id).count();
        if others >= self.cfg.limits.stations_per_ip {
            self.metrics.limit_hit("stations_per_ip");
            return refuse(&conn, tx, writer, ErrorCode::TooMany, "trop de Stations pour cette adresse").await;
        }
        let peer = Arc::new(StationPeer {
            id: id.clone(),
            conn: conn.clone(),
            ctl: tx.clone(),
            ip,
            admin_fp,
            sig,
            station_key: base64::engine::general_purpose::STANDARD.encode(&spki),
            media: RwLock::new(None),
            next_flow: AtomicU64::new(1),
            flows: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
        });
        let old = self.stations.lock().unwrap().insert(id.clone(), peer.clone());
        if let Some(old) = old {
            let _ = old.ctl.send(Msg::error(None, ErrorCode::Replaced, None));
            old.conn.close("remplacée");
            self.drop_station_flows(&old);
        }
        let kept = self.codes.lock().unwrap().station_back(&id, &hello.codes, self.cfg.limits.codes_per_station, Instant::now());
        let _ = tx.send(Msg::Welcome(Welcome { v: 1, id: id.clone(), relay: VERSION.into(), online: None, codes: kept }));
        tracing::info!(station = %id, ip = %ip, transport = %conn.transport(), version = hello.version.as_deref().unwrap_or("?"), "Station connectée");
        self.notify_station(&id, true);
        // Les flux se prennent à part : une lecture de message interrompue
        // perdrait le début d'une trame.
        let acceptor = {
            let (hub, peer, conn) = (self.clone(), peer.clone(), conn.clone());
            tokio::spawn(async move {
                while let Some(s) = conn.accept().await {
                    tokio::spawn(hub.clone().station_stream(peer.clone(), s));
                }
            })
        };
        let transport = conn.transport();
        let pinger = pinger(transport, tx.clone());
        while let Some(msg) = read_with_watchdog(&mut read, transport).await {
            self.station_msg(&peer, msg);
        }
        acceptor.abort();
        if let Some(p) = pinger {
            p.abort();
        }
        self.station_gone(&peer);
        conn.close("fin");
        drop(writer);
    }

    fn station_msg(&self, peer: &Arc<StationPeer>, msg: Msg) {
        match msg {
            Msg::Reserve { req, preferred } => {
                let code = self.codes.lock().unwrap().reserve(&peer.id, preferred.as_deref(), self.cfg.limits.codes_per_station, Instant::now());
                let _ = peer.ctl.send(match code {
                    Some(code) => Msg::Reserved { req, code },
                    None => Msg::error(Some(req), ErrorCode::NoCode, None),
                });
            }
            Msg::Release { code } => self.codes.lock().unwrap().release(&peer.id, &code),
            Msg::FlowAccept { flow } => {
                let pending = peer.pending.lock().unwrap().remove(&flow);
                let Some(p) = pending else { return };
                match p.client.upgrade() {
                    Some(client) if self.is_current_client(&client) => {
                        client.flows.lock().unwrap().insert(p.cflow, (Arc::downgrade(peer), flow));
                        peer.flows.lock().unwrap().insert(flow, (Arc::downgrade(&client), p.cflow));
                        let _ = client.ctl.send(Msg::FlowReady { req: p.req, flow: p.cflow });
                    }
                    _ => {
                        let _ = peer.ctl.send(Msg::FlowClose { flow });
                    }
                }
            }
            Msg::FlowRefuse { flow, reason } => {
                let pending = peer.pending.lock().unwrap().remove(&flow);
                if let Some(client) = pending.as_ref().and_then(|p| p.client.upgrade()) {
                    let _ = client.ctl.send(Msg::FlowRefused { req: pending.expect("demande").req, reason });
                }
            }
            Msg::FlowClose { flow } => {
                let link = peer.flows.lock().unwrap().remove(&flow);
                if let Some((client, cflow)) = link
                    && let Some(client) = client.upgrade()
                {
                    client.flows.lock().unwrap().remove(&cflow);
                    let _ = client.ctl.send(Msg::FlowClose { flow: cflow });
                }
            }
            Msg::Ping { n } => {
                let _ = peer.ctl.send(Msg::Pong { n });
            }
            Msg::Pong { n } => pong(&peer.conn, n),
            _ => {}
        }
    }

    /// Un flux ouvert par la Station : seulement la connexion des paquets du repli TCP.
    async fn station_stream(self: Arc<Self>, peer: Arc<StationPeer>, mut s: RelayStream) {
        match timeout(HELLO_TIMEOUT, read_msg(&mut s)).await {
            Ok(Ok(Some(Msg::Media { .. }))) => {
                let (tx, rx) = media_over_stream(s);
                self.station_media(peer, tx, rx).await;
            }
            _ => s.reset(),
        }
    }

    fn is_current_station(&self, peer: &Arc<StationPeer>) -> bool {
        self.stations.lock().unwrap().get(&peer.id).is_some_and(|p| Arc::ptr_eq(p, peer))
    }

    fn is_current_client(&self, peer: &Arc<ClientPeer>) -> bool {
        self.clients.lock().unwrap().get(&peer.id).is_some_and(|p| Arc::ptr_eq(p, peer))
    }

    /// Prévient les clients de cette Station qu'elle part ou revient.
    fn notify_station(&self, id: &str, online: bool) {
        let clients: Vec<Arc<ClientPeer>> = self.clients.lock().unwrap().values().filter(|c| c.station.as_deref() == Some(id)).cloned().collect();
        for c in clients {
            let _ = c.ctl.send(Msg::Station { online });
        }
    }

    /// Oublie les flux d'une Station (partie ou remplacée) des deux côtés.
    fn drop_station_flows(&self, peer: &StationPeer) {
        let flows: Vec<(Weak<ClientPeer>, u64)> = peer.flows.lock().unwrap().drain().map(|(_, v)| v).collect();
        for (client, cflow) in flows {
            if let Some(c) = client.upgrade() {
                c.flows.lock().unwrap().remove(&cflow);
            }
        }
        let pending: Vec<Pending> = peer.pending.lock().unwrap().drain().map(|(_, p)| p).collect();
        for p in pending {
            if let Some(c) = p.client.upgrade() {
                let _ = c.ctl.send(Msg::FlowRefused { req: p.req, reason: "offline".into() });
            }
        }
    }

    fn station_gone(&self, peer: &Arc<StationPeer>) {
        let current = {
            let mut map = self.stations.lock().unwrap();
            let current = map.get(&peer.id).is_some_and(|p| Arc::ptr_eq(p, peer));
            if current {
                map.remove(&peer.id);
            }
            current
        };
        self.drop_station_flows(peer);
        if current {
            self.codes.lock().unwrap().station_left(&peer.id, Instant::now());
            self.notify_station(&peer.id, false);
            tracing::info!(station = %peer.id, "Station partie");
        }
    }

    async fn client_session(
        self: Arc<Self>,
        conn: Conn,
        mut read: ReadHalf<RelayStream>,
        tx: Ctl,
        writer: tokio::task::JoinHandle<()>,
        hello: Hello,
        ip: IpAddr,
    ) {
        let Some(token) = hello.token.filter(|t| id::is_token(t)) else {
            return refuse(&conn, tx, writer, ErrorCode::BadRequest, "jeton").await;
        };
        if hello.station.as_deref().is_some_and(|s| !id::is_station_id(s)) {
            return refuse(&conn, tx, writer, ErrorCode::BadRequest, "identifiant de Station").await;
        }
        let id = id::client_id(&token);
        if let Some(station) = &hello.station {
            let others = self.clients.lock().unwrap().values().filter(|c| c.station.as_ref() == Some(station) && c.id != id).count();
            if others >= self.cfg.limits.clients_per_station {
                self.metrics.limit_hit("clients_per_station");
                return refuse(&conn, tx, writer, ErrorCode::TooMany, "trop de clients pour cette Station").await;
            }
        }
        let peer = Arc::new(ClientPeer {
            id: id.clone(),
            conn: conn.clone(),
            ctl: tx.clone(),
            ip,
            station: hello.station.clone(),
            media: RwLock::new(None),
            next_flow: AtomicU64::new(1),
            flows: Mutex::new(HashMap::new()),
        });
        let old = self.clients.lock().unwrap().insert(id.clone(), peer.clone());
        if let Some(old) = old {
            let _ = old.ctl.send(Msg::error(None, ErrorCode::Replaced, None));
            old.conn.close("remplacé");
            self.drop_client_flows(&old);
        }
        let online = hello.station.as_deref().map(|s| self.station(s).is_some());
        let _ = tx.send(Msg::Welcome(Welcome { v: 1, id: id.clone(), relay: VERSION.into(), online, codes: vec![] }));
        tracing::debug!(client = %id, "client connecté");
        // L'aller-retour de la Station, toutes les 5 s, pour la ligne « Chemin ».
        let stats = hello.station.clone().map(|station| {
            let (hub, ctl) = (self.clone(), tx.clone());
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(5));
                loop {
                    tick.tick().await;
                    let rtt = hub.station(&station).and_then(|s| s.conn.rtt());
                    if let Some(rtt) = rtt
                        && !ctl.send(Msg::Stats { rtt_station_ms: Some(rtt.as_millis().min(u128::from(u32::MAX)) as u32) })
                    {
                        return;
                    }
                }
            })
        });
        let acceptor = {
            let (hub, peer, conn) = (self.clone(), peer.clone(), conn.clone());
            tokio::spawn(async move {
                while let Some(s) = conn.accept().await {
                    tokio::spawn(hub.clone().client_stream(peer.clone(), s));
                }
            })
        };
        let transport = conn.transport();
        let pinger = pinger(transport, tx.clone());
        while let Some(msg) = read_with_watchdog(&mut read, transport).await {
            self.client_msg(&peer, msg);
        }
        acceptor.abort();
        if let Some(p) = pinger {
            p.abort();
        }
        if let Some(s) = stats {
            s.abort();
        }
        self.client_gone(&peer);
        conn.close("fin");
        drop(writer);
    }

    fn client_msg(&self, peer: &Arc<ClientPeer>, msg: Msg) {
        match msg {
            Msg::OpenFlow { req, key } => {
                let Some(station) = peer.station.as_deref().and_then(|s| self.station(s)) else {
                    let _ = peer.ctl.send(Msg::FlowRefused { req, reason: "offline".into() });
                    return;
                };
                let pending_mine =
                    station.pending.lock().unwrap().values().filter(|p| p.client.upgrade().is_some_and(|c| Arc::ptr_eq(&c, peer))).count();
                if peer.flows.lock().unwrap().len() + pending_mine >= self.cfg.limits.flows_per_client {
                    self.metrics.limit_hit("flows_per_client");
                    let _ = peer.ctl.send(Msg::FlowRefused { req, reason: "too_many".into() });
                    return;
                }
                let sflow = station.next_flow.fetch_add(1, Ordering::Relaxed);
                let cflow = peer.next_flow.fetch_add(1, Ordering::Relaxed);
                station.pending.lock().unwrap().insert(sflow, Pending { client: Arc::downgrade(peer), req, cflow });
                let _ = station.ctl.send(Msg::FlowOpen { flow: sflow, client: peer.id.clone(), key, ip: peer.ip });
                let (st, client) = (Arc::downgrade(&station), Arc::downgrade(peer));
                tokio::spawn(async move {
                    tokio::time::sleep(FLOW_TIMEOUT).await;
                    let late = st.upgrade().and_then(|s| s.pending.lock().unwrap().remove(&sflow));
                    if let (Some(p), Some(c)) = (late, client.upgrade()) {
                        let _ = c.ctl.send(Msg::FlowRefused { req: p.req, reason: "timeout".into() });
                    }
                });
            }
            Msg::FlowClose { flow } => {
                let link = peer.flows.lock().unwrap().remove(&flow);
                if let Some((station, sflow)) = link
                    && let Some(station) = station.upgrade()
                {
                    station.flows.lock().unwrap().remove(&sflow);
                    let _ = station.ctl.send(Msg::FlowClose { flow: sflow });
                }
            }
            Msg::Status { req, stations } => {
                let online = stations.iter().take(MAX_STATUS).map(|s| self.station(s).is_some()).collect();
                let _ = peer.ctl.send(Msg::Presence { req, online });
            }
            Msg::Ping { n } => {
                let _ = peer.ctl.send(Msg::Pong { n });
            }
            Msg::Pong { n } => pong(&peer.conn, n),
            _ => {}
        }
    }

    /// Un flux ouvert par une app : une connexion TCP vers sa Station, ou la
    /// connexion des paquets du repli TCP.
    async fn client_stream(self: Arc<Self>, peer: Arc<ClientPeer>, mut s: RelayStream) {
        match timeout(HELLO_TIMEOUT, read_msg(&mut s)).await {
            Ok(Ok(Some(Msg::Open { service }))) => {
                let station = peer.station.as_deref().and_then(|id| self.station(id));
                let Some(station) = station else { return s.reset() };
                let header = StreamHeader { v: 1, kind: service.kind(), client: peer.id.clone(), ip: peer.ip, host: None };
                let Some(mut to_station) = open_to_station(&station, header).await else { return s.reset() };
                let (up, down) = tokio::io::copy_bidirectional(&mut s, &mut to_station).await.unwrap_or((0, 0));
                self.metrics.add_bytes(Leg::Stream, Dir::ToStation, up);
                self.metrics.add_bytes(Leg::Stream, Dir::ToClient, down);
                if service == Service::KyberTcp {
                    tracing::debug!(client = %peer.id, up, down, "flux Kyber terminé");
                }
            }
            Ok(Ok(Some(Msg::Media { token }))) if token.as_deref().map(id::client_id).as_deref() == Some(peer.id.as_str()) => {
                let (tx, rx) = media_over_stream(s);
                self.client_media(peer, tx, rx).await;
            }
            _ => s.reset(),
        }
    }

    fn drop_client_flows(&self, peer: &ClientPeer) {
        let flows: Vec<(Weak<StationPeer>, u64)> = peer.flows.lock().unwrap().drain().map(|(_, v)| v).collect();
        for (station, sflow) in flows {
            if let Some(s) = station.upgrade() {
                s.flows.lock().unwrap().remove(&sflow);
            }
        }
    }

    fn client_gone(&self, peer: &Arc<ClientPeer>) {
        let current = {
            let mut map = self.clients.lock().unwrap();
            let current = map.get(&peer.id).is_some_and(|p| Arc::ptr_eq(p, peer));
            if current {
                map.remove(&peer.id);
            }
            current
        };
        self.drop_client_flows(peer);
        if current && let Some(station) = peer.station.as_deref().and_then(|s| self.station(s)) {
            station.pending.lock().unwrap().retain(|_, p| p.client.upgrade().is_some_and(|c| !Arc::ptr_eq(&c, peer)));
            let _ = station.ctl.send(Msg::ClientGone { client: peer.id.clone() });
        }
    }

    // --- Paquets -------------------------------------------------------------

    /// La connexion des paquets (`kn-media/1`) : un flux unidirectionnel
    /// `{"t":"media"}` d'abord, puis les datagrammes.
    pub async fn serve_media_quic(self: Arc<Self>, conn: quinn::Connection, spki: Option<Vec<u8>>) {
        let hello = match timeout(HELLO_TIMEOUT, conn.accept_uni()).await {
            Ok(Ok(mut uni)) => timeout(HELLO_TIMEOUT, read_msg(&mut uni)).await.ok().and_then(Result::ok).flatten(),
            _ => None,
        };
        let Some(Msg::Media { token }) = hello else {
            conn.close(0u32.into(), b"pas de media");
            return;
        };
        let (tx, rx) = (MediaSender::quic(conn.clone()), MediaReceiver::quic(conn.clone()));
        match (spki, token) {
            (Some(spki), _) => match self.station(&id::station_id(&spki)) {
                Some(peer) => self.station_media(peer, tx, rx).await,
                None => conn.close(0u32.into(), b"Station inconnue"),
            },
            (None, Some(token)) if id::is_token(&token) => match self.client(&id::client_id(&token)) {
                Some(peer) => self.client_media(peer, tx, rx).await,
                None => conn.close(0u32.into(), b"client inconnu"),
            },
            _ => conn.close(0u32.into(), b"media sans identite"),
        }
    }

    async fn station_media(&self, peer: Arc<StationPeer>, tx: MediaSender, mut rx: MediaReceiver) {
        let n = self.next_media.fetch_add(1, Ordering::Relaxed);
        *peer.media.write().unwrap() = Some((n, tx));
        while let Some((flow, payload)) = rx.recv().await {
            if !self.is_current_station(&peer) {
                break;
            }
            self.route_from_station(&peer, flow, payload);
        }
        let mut m = peer.media.write().unwrap();
        if m.as_ref().is_some_and(|(id, _)| *id == n) {
            *m = None;
        }
    }

    async fn client_media(&self, peer: Arc<ClientPeer>, tx: MediaSender, mut rx: MediaReceiver) {
        let n = self.next_media.fetch_add(1, Ordering::Relaxed);
        *peer.media.write().unwrap() = Some((n, tx));
        while let Some((flow, payload)) = rx.recv().await {
            if !self.is_current_client(&peer) {
                break;
            }
            self.route_from_client(&peer, flow, payload);
        }
        let mut m = peer.media.write().unwrap();
        if m.as_ref().is_some_and(|(id, _)| *id == n) {
            *m = None;
        }
    }

    fn capped(&self, len: usize) -> bool {
        self.cap.as_ref().is_some_and(|b| b.lock().unwrap().take_n(len as f64, Instant::now()).is_err())
    }

    fn deliver(&self, media: Option<MediaSender>, flow: u64, payload: &Bytes, dir: Dir) {
        let Some(m) = media else { return self.metrics.datagram(Dg::NoRoute) };
        if self.capped(payload.len()) {
            self.metrics.limit_hit("global_cap");
            return self.metrics.datagram(Dg::Capped);
        }
        let r = match m.send(flow, payload) {
            Sent::Datagram => Dg::Forwarded,
            Sent::Fallback => Dg::Fallback,
            Sent::Dropped(DropReason::TooLarge) => Dg::TooLarge,
            Sent::Dropped(DropReason::Full) => Dg::Full,
            Sent::Dropped(DropReason::Closed) => Dg::Closed,
            Sent::Dropped(DropReason::Unsupported) => Dg::Unsupported,
        };
        if matches!(r, Dg::Forwarded | Dg::Fallback) {
            self.metrics.add_bytes(Leg::Media, dir, payload.len() as u64);
        }
        self.metrics.datagram(r);
    }

    fn route_from_client(&self, peer: &ClientPeer, flow: u64, payload: Bytes) {
        let link = peer.flows.lock().unwrap().get(&flow).cloned();
        let target = link.and_then(|(s, sflow)| Some((s.upgrade()?, sflow)));
        let Some((station, sflow)) = target else { return self.metrics.datagram(Dg::NoRoute) };
        let media = station.media.read().unwrap().as_ref().map(|(_, m)| m.clone());
        self.deliver(media, sflow, &payload, Dir::ToStation);
    }

    fn route_from_station(&self, peer: &StationPeer, flow: u64, payload: Bytes) {
        let link = peer.flows.lock().unwrap().get(&flow).cloned();
        let target = link.and_then(|(c, cflow)| Some((c.upgrade()?, cflow)));
        let Some((client, cflow)) = target else { return self.metrics.datagram(Dg::NoRoute) };
        let media = client.media.read().unwrap().as_ref().map(|(_, m)| m.clone());
        self.deliver(media, cflow, &payload, Dir::ToClient);
    }

    // --- Navigateur ----------------------------------------------------------

    /// Un flux `admin_http` vers la Station, pour une connexion de navigateur.
    pub async fn open_web_stream(&self, station: &str, browser: &str, ip: IpAddr, host: &str) -> Option<RelayStream> {
        let station = self.station(station)?;
        let header = StreamHeader { v: 1, kind: StreamKind::AdminHttp, client: browser.into(), ip, host: Some(host.into()) };
        open_to_station(&station, header).await
    }
}

/// Un flux vers la Station, en-tête écrit ; `None` si la Station ne le
/// prend pas à temps (ses flux tous pris, connexion perdue) : la tâche qui
/// l'attend ne reste pas suspendue.
async fn open_to_station(station: &StationPeer, header: StreamHeader) -> Option<RelayStream> {
    timeout(HELLO_TIMEOUT, async {
        let mut s = station.conn.open().await.ok()?;
        write_msg(&mut s, &Msg::Stream(header)).await.ok()?;
        Some(s)
    })
    .await
    .ok()
    .flatten()
}

/// Origine des `ping` du relais (`n` en microsecondes depuis elle).
static EPOCH: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

fn micros() -> u64 {
    EPOCH.elapsed().as_micros() as u64
}

/// Repli TCP : QUIC mesure l'aller-retour lui-même, yamux non ; le relais
/// envoie un `ping` toutes les 5 s et note l'aller-retour au `pong`.
fn pinger(transport: Transport, ctl: Ctl) -> Option<tokio::task::JoinHandle<()>> {
    (transport == Transport::Tcp).then(|| {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                if !ctl.send(Msg::Ping { n: micros() }) {
                    return;
                }
            }
        })
    })
}

fn pong(conn: &Conn, n: u64) {
    let now = micros();
    if n <= now {
        conn.set_rtt(Duration::from_micros(now - n));
    }
}

/// Le prochain message ; sur le repli TCP, `None` après 35 s de silence.
async fn read_with_watchdog<R: AsyncRead + Unpin>(read: &mut R, transport: Transport) -> Option<Msg> {
    let r = match transport {
        Transport::Quic => read_msg(read).await,
        Transport::Tcp => match timeout(TCP_SILENCE, read_msg(read)).await {
            Ok(r) => r,
            Err(_) => return None,
        },
    };
    r.ok().flatten()
}
