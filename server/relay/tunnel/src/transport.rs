//! Les deux transports du tunnel sous une même forme (docs/relais.md) :
//!
//! - [`Conn`] : une connexion qui porte des flux, QUIC (`kn-ctl/1`) ou yamux
//!   sur TLS (`kn-tcp/1`) ;
//! - [`RelayStream`] : un flux, lisible et inscriptible comme une socket TCP ;
//! - [`MediaSender`] et [`MediaReceiver`] : les paquets UDP relayés, en
//!   datagrammes QUIC (`kn-media/1`, avec la règle de taille) ou en
//!   enregistrements sur un flux yamux.

use std::fmt;
use std::future::poll_fn;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufWriter, ReadBuf};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};

use crate::frame::{decode_datagram, encode_datagram, encode_record, read_record};

/// Un paquet de 1200 octets au plus passe toujours, en datagramme ou en flux
/// de secours ; plus grand, il est jeté s'il ne tient pas.
pub const SMALL_PACKET: usize = 1200;

/// Le transport d'une connexion au relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// QUIC sur UDP (`kn-ctl/1` et `kn-media/1`).
    Quic,
    /// Repli TLS sur TCP (`kn-tcp/1`), multiplexé par yamux.
    Tcp,
}

impl Transport {
    pub fn as_str(self) -> &'static str {
        match self {
            Transport::Quic => "quic",
            Transport::Tcp => "tcp",
        }
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Une tâche annulée quand on la lâche.
#[derive(Debug)]
pub struct AbortOnDrop(pub JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Compte les flux et les flux UDP en cours (pour savoir si une connexion
/// est au repos).
#[derive(Debug, Clone, Default)]
pub struct ActiveCounter(Arc<AtomicUsize>);

impl ActiveCounter {
    pub fn guard(&self) -> ActiveGuard {
        self.0.fetch_add(1, Ordering::Relaxed);
        ActiveGuard(self.0.clone())
    }

    pub fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct ActiveGuard(Arc<AtomicUsize>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

// --- Flux ----------------------------------------------------------------

enum StreamInner {
    Quic { send: quinn::SendStream, recv: quinn::RecvStream },
    Yamux(Compat<yamux::Stream>),
}

/// Un flux du tunnel : une connexion TCP relayée, ou le flux des messages.
pub struct RelayStream {
    inner: StreamInner,
    _guard: Option<ActiveGuard>,
}

impl fmt::Debug for RelayStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            StreamInner::Quic { send, .. } => write!(f, "RelayStream(quic {})", send.id()),
            StreamInner::Yamux(_) => f.write_str("RelayStream(yamux)"),
        }
    }
}

impl RelayStream {
    pub fn quic(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        RelayStream { inner: StreamInner::Quic { send, recv }, _guard: None }
    }

    pub fn yamux(stream: yamux::Stream) -> Self {
        RelayStream { inner: StreamInner::Yamux(stream.compat()), _guard: None }
    }

    /// Compté dans `counter` tant qu'il vit.
    pub fn counted(mut self, counter: &ActiveCounter) -> Self {
        self._guard = Some(counter.guard());
        self
    }

    /// Coupe le flux dans les deux sens (pas une fermeture propre).
    pub fn reset(&mut self) {
        match &mut self.inner {
            StreamInner::Quic { send, recv } => {
                let _ = send.reset(quinn::VarInt::from_u32(1));
                let _ = recv.stop(quinn::VarInt::from_u32(1));
            }
            // yamux n'a pas de coupure franche : le flux se ferme quand on le lâche.
            StreamInner::Yamux(_) => {}
        }
    }
}

impl AsyncRead for RelayStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().inner {
            StreamInner::Quic { recv, .. } => AsyncRead::poll_read(Pin::new(recv), cx, buf),
            StreamInner::Yamux(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for RelayStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match &mut self.get_mut().inner {
            StreamInner::Quic { send, .. } => AsyncWrite::poll_write(Pin::new(send), cx, buf),
            StreamInner::Yamux(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().inner {
            StreamInner::Quic { send, .. } => AsyncWrite::poll_flush(Pin::new(send), cx),
            StreamInner::Yamux(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.get_mut().inner {
            StreamInner::Quic { send, .. } => AsyncWrite::poll_shutdown(Pin::new(send), cx),
            StreamInner::Yamux(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

// --- yamux ---------------------------------------------------------------

type OpenReply = oneshot::Sender<Result<yamux::Stream, yamux::ConnectionError>>;

struct YamuxShared {
    open_tx: mpsc::Sender<OpenReply>,
    inbound: tokio::sync::Mutex<mpsc::Receiver<yamux::Stream>>,
    done: watch::Receiver<bool>,
    remote: SocketAddr,
    rtt_us: AtomicU64,
    task: AbortOnDrop,
}

/// Une connexion yamux, menée par sa propre tâche.
#[derive(Clone)]
pub struct YamuxHandle(Arc<YamuxShared>);

/// Configuration yamux du tunnel : 256 flux, fenêtres réglées d'elles-mêmes.
pub fn yamux_config() -> yamux::Config {
    let mut cfg = yamux::Config::default();
    cfg.set_max_connection_receive_window(Some(256 * 1024 * 1024));
    cfg.set_max_num_streams(256);
    cfg
}

impl YamuxHandle {
    /// Lance yamux sur une connexion (TLS sur TCP). Le bout qui se connecte
    /// au relais est `Client`, le relais `Server`.
    pub fn start<T>(io: T, mode: yamux::Mode, remote: SocketAddr) -> YamuxHandle
    where
        T: futures_util::io::AsyncRead + futures_util::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (open_tx, open_rx) = mpsc::channel::<OpenReply>(32);
        let (in_tx, in_rx) = mpsc::channel(64);
        let (done_tx, done_rx) = watch::channel(false);
        let conn = yamux::Connection::new(io, yamux_config(), mode);
        let task = tokio::spawn(async move {
            drive(conn, open_rx, in_tx).await;
            let _ = done_tx.send(true);
        });
        YamuxHandle(Arc::new(YamuxShared {
            open_tx,
            inbound: tokio::sync::Mutex::new(in_rx),
            done: done_rx,
            remote,
            rtt_us: AtomicU64::new(0),
            task: AbortOnDrop(task),
        }))
    }

    pub async fn open(&self) -> io::Result<RelayStream> {
        let (tx, rx) = oneshot::channel();
        self.0.open_tx.send(tx).await.map_err(|_| closed())?;
        match rx.await {
            Ok(Ok(s)) => Ok(RelayStream::yamux(s)),
            Ok(Err(e)) => Err(io::Error::new(io::ErrorKind::ConnectionAborted, e.to_string())),
            Err(_) => Err(closed()),
        }
    }

    pub async fn accept(&self) -> Option<RelayStream> {
        self.0.inbound.lock().await.recv().await.map(RelayStream::yamux)
    }

    pub async fn closed(&self) {
        let mut done = self.0.done.clone();
        let _ = done.wait_for(|d| *d).await;
    }

    pub fn close(&self) {
        self.0.task.0.abort();
    }

    pub fn is_closed(&self) -> bool {
        *self.0.done.borrow() || self.0.task.0.is_finished()
    }
}

async fn drive<T>(mut conn: yamux::Connection<T>, mut open_rx: mpsc::Receiver<OpenReply>, in_tx: mpsc::Sender<yamux::Stream>)
where
    T: futures_util::io::AsyncRead + futures_util::io::AsyncWrite + Unpin,
{
    let mut waiting: std::collections::VecDeque<OpenReply> = Default::default();
    let mut handles_gone = false;
    poll_fn(|cx| {
        if !handles_gone {
            loop {
                match open_rx.poll_recv(cx) {
                    Poll::Ready(Some(r)) => waiting.push_back(r),
                    Poll::Ready(None) => {
                        handles_gone = true;
                        break;
                    }
                    Poll::Pending => break,
                }
            }
        }
        while !waiting.is_empty() {
            match conn.poll_new_outbound(cx) {
                Poll::Ready(Ok(s)) => {
                    let _ = waiting.pop_front().expect("une demande attend").send(Ok(s));
                }
                Poll::Ready(Err(e)) => {
                    for w in waiting.drain(..) {
                        let _ = w.send(Err(yamux::ConnectionError::Closed));
                    }
                    log::debug!("yamux : ouverture impossible ({e})");
                    return Poll::Ready(());
                }
                Poll::Pending => break,
            }
        }
        loop {
            match conn.poll_next_inbound(cx) {
                Poll::Ready(Some(Ok(s))) => {
                    // Trop de flux en attente d'être pris : celui-ci est refusé.
                    if in_tx.try_send(s).is_err() {
                        log::debug!("yamux : flux entrant refusé (file pleine)");
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    log::debug!("yamux : connexion perdue ({e})");
                    return Poll::Ready(());
                }
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => break,
            }
        }
        Poll::Pending
    })
    .await
}

fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "connexion au relais fermée")
}

// --- Connexion -----------------------------------------------------------

/// Une connexion au relais qui porte des flux.
#[derive(Clone)]
pub enum Conn {
    Quic(quinn::Connection),
    Yamux(YamuxHandle),
}

impl fmt::Debug for Conn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Conn({} {})", self.transport(), self.remote_address())
    }
}

impl Conn {
    pub fn transport(&self) -> Transport {
        match self {
            Conn::Quic(_) => Transport::Quic,
            Conn::Yamux(_) => Transport::Tcp,
        }
    }

    /// Ouvre un flux bidirectionnel.
    pub async fn open(&self) -> io::Result<RelayStream> {
        match self {
            Conn::Quic(c) => {
                let (send, recv) = c.open_bi().await.map_err(|e| io::Error::new(io::ErrorKind::ConnectionAborted, e))?;
                Ok(RelayStream::quic(send, recv))
            }
            Conn::Yamux(y) => y.open().await,
        }
    }

    /// Le prochain flux ouvert par l'autre bout ; `None` quand la connexion est fermée.
    pub async fn accept(&self) -> Option<RelayStream> {
        match self {
            Conn::Quic(c) => c.accept_bi().await.ok().map(|(s, r)| RelayStream::quic(s, r)),
            Conn::Yamux(y) => y.accept().await,
        }
    }

    /// Aller-retour mesuré (QUIC), ou noté par [`Conn::set_rtt`] (repli TCP).
    pub fn rtt(&self) -> Option<Duration> {
        match self {
            Conn::Quic(c) => Some(c.rtt()),
            Conn::Yamux(y) => match y.0.rtt_us.load(Ordering::Relaxed) {
                0 => None,
                us => Some(Duration::from_micros(us)),
            },
        }
    }

    /// Note l'aller-retour mesuré par `ping` (repli TCP seulement).
    pub fn set_rtt(&self, rtt: Duration) {
        if let Conn::Yamux(y) = self {
            y.0.rtt_us.store(rtt.as_micros().max(1) as u64, Ordering::Relaxed);
        }
    }

    pub fn remote_address(&self) -> SocketAddr {
        match self {
            Conn::Quic(c) => c.remote_address(),
            Conn::Yamux(y) => y.0.remote,
        }
    }

    pub fn close(&self, reason: &str) {
        match self {
            Conn::Quic(c) => c.close(quinn::VarInt::from_u32(0), reason.as_bytes()),
            Conn::Yamux(y) => y.close(),
        }
    }

    /// Attend la fin de la connexion.
    pub async fn closed(&self) {
        match self {
            Conn::Quic(c) => {
                c.closed().await;
            }
            Conn::Yamux(y) => y.closed().await,
        }
    }

    pub fn is_closed(&self) -> bool {
        match self {
            Conn::Quic(c) => c.close_reason().is_some(),
            Conn::Yamux(y) => y.is_closed(),
        }
    }
}

// --- Paquets UDP ---------------------------------------------------------

/// Ce qu'est devenu un paquet envoyé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    /// Parti en datagramme (ou en enregistrement sur le repli TCP).
    Datagram,
    /// Trop grand pour un datagramme, parti dans un flux de secours.
    Fallback,
    /// Jeté.
    Dropped(DropReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// Plus de 1200 octets et plus grand que ce que le chemin laisse passer.
    TooLarge,
    /// File d'envoi pleine (repli TCP).
    Full,
    /// Connexion fermée.
    Closed,
    /// L'autre bout ne prend pas les datagrammes.
    Unsupported,
}

impl DropReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DropReason::TooLarge => "too_large",
            DropReason::Full => "full",
            DropReason::Closed => "closed",
            DropReason::Unsupported => "unsupported",
        }
    }
}

#[derive(Clone)]
enum SendInner {
    Quic(quinn::Connection),
    Records(mpsc::Sender<Bytes>),
}

/// L'envoi des paquets UDP relayés.
#[derive(Clone)]
pub struct MediaSender(SendInner);

impl fmt::Debug for MediaSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            SendInner::Quic(_) => f.write_str("MediaSender(quic)"),
            SendInner::Records(_) => f.write_str("MediaSender(tcp)"),
        }
    }
}

impl MediaSender {
    pub fn quic(conn: quinn::Connection) -> Self {
        MediaSender(SendInner::Quic(conn))
    }

    /// Envoie un paquet sur un flux.
    pub fn send(&self, flow: u64, payload: &[u8]) -> Sent {
        self.send_datagram(encode_datagram(flow, payload), payload.len())
    }

    /// Envoie un datagramme déjà formé (`[flux][paquet]`) ; `payload_len` est
    /// la taille du paquet seul, pour la règle de taille.
    pub fn send_datagram(&self, datagram: Bytes, payload_len: usize) -> Sent {
        match &self.0 {
            SendInner::Quic(conn) => match conn.send_datagram(datagram.clone()) {
                Ok(()) => Sent::Datagram,
                Err(quinn::SendDatagramError::TooLarge) if payload_len <= SMALL_PACKET => {
                    let conn = conn.clone();
                    tokio::spawn(async move {
                        if let Ok(mut s) = conn.open_uni().await {
                            let _ = s.write_all(&datagram).await;
                            let _ = s.finish();
                        }
                    });
                    Sent::Fallback
                }
                Err(quinn::SendDatagramError::TooLarge) => Sent::Dropped(DropReason::TooLarge),
                Err(quinn::SendDatagramError::ConnectionLost(_)) => Sent::Dropped(DropReason::Closed),
                Err(_) => Sent::Dropped(DropReason::Unsupported),
            },
            SendInner::Records(tx) => match encode_record(&datagram) {
                None => Sent::Dropped(DropReason::TooLarge),
                Some(record) => match tx.try_send(record) {
                    Ok(()) => Sent::Datagram,
                    Err(mpsc::error::TrySendError::Full(_)) => Sent::Dropped(DropReason::Full),
                    Err(mpsc::error::TrySendError::Closed(_)) => Sent::Dropped(DropReason::Closed),
                },
            },
        }
    }

    /// Plus grand datagramme que le chemin laisse passer en ce moment.
    pub fn max_datagram_size(&self) -> Option<usize> {
        match &self.0 {
            SendInner::Quic(conn) => conn.max_datagram_size(),
            SendInner::Records(_) => Some(u16::MAX as usize),
        }
    }

    pub fn is_closed(&self) -> bool {
        match &self.0 {
            SendInner::Quic(conn) => conn.close_reason().is_some(),
            SendInner::Records(tx) => tx.is_closed(),
        }
    }
}

enum RecvInner {
    Quic { conn: quinn::Connection, uni: mpsc::Receiver<Bytes>, uni_open: bool, _task: AbortOnDrop },
    Records { rx: mpsc::Receiver<Bytes>, _task: AbortOnDrop },
}

/// La réception des paquets UDP relayés.
pub struct MediaReceiver(RecvInner);

impl MediaReceiver {
    /// Datagrammes de `kn-media/1`, et paquets des flux de secours.
    pub fn quic(conn: quinn::Connection) -> Self {
        let (tx, rx) = mpsc::channel::<Bytes>(256);
        let c = conn.clone();
        let task = tokio::spawn(async move {
            while let Ok(mut r) = c.accept_uni().await {
                let tx = tx.clone();
                tokio::spawn(async move {
                    // Un paquet de secours fait 1200 octets au plus, plus son numéro de flux.
                    if let Ok(b) = r.read_to_end(2048).await {
                        let _ = tx.try_send(Bytes::from(b));
                    }
                });
            }
        });
        MediaReceiver(RecvInner::Quic { conn, uni: rx, uni_open: true, _task: AbortOnDrop(task) })
    }

    /// Le prochain paquet : son numéro de flux et son contenu ; `None` à la fin.
    pub async fn recv(&mut self) -> Option<(u64, Bytes)> {
        loop {
            let datagram = match &mut self.0 {
                RecvInner::Quic { conn, uni, uni_open, .. } => {
                    if *uni_open {
                        tokio::select! {
                            biased;
                            d = conn.read_datagram() => d.ok()?,
                            u = uni.recv() => match u {
                                Some(b) => b,
                                None => {
                                    *uni_open = false;
                                    continue;
                                }
                            },
                        }
                    } else {
                        conn.read_datagram().await.ok()?
                    }
                }
                RecvInner::Records { rx, .. } => rx.recv().await?,
            };
            if let Some(d) = decode_datagram(&datagram) {
                return Some(d);
            }
        }
    }
}

/// Les paquets UDP sur un flux du repli TCP : enregistrements
/// `[longueur][flux][paquet]`, une tâche pour écrire, une pour lire.
pub fn media_over_stream(stream: RelayStream) -> (MediaSender, MediaReceiver) {
    let (read, write) = tokio::io::split(stream);
    let (out_tx, mut out_rx) = mpsc::channel::<Bytes>(1024);
    let (in_tx, in_rx) = mpsc::channel::<Bytes>(1024);
    let writer = tokio::spawn(async move {
        let mut w = BufWriter::new(write);
        while let Some(first) = out_rx.recv().await {
            if w.write_all(&first).await.is_err() {
                return;
            }
            while let Ok(more) = out_rx.try_recv() {
                if w.write_all(&more).await.is_err() {
                    return;
                }
            }
            if w.flush().await.is_err() {
                return;
            }
        }
    });
    let reader = tokio::spawn(async move {
        let _writer = AbortOnDrop(writer);
        let mut read = read;
        while let Ok(Some(record)) = read_record(&mut read).await {
            // File pleine : le paquet est perdu, comme en UDP.
            let _ = in_tx.try_send(record);
        }
    });
    (MediaSender(SendInner::Records(out_tx)), MediaReceiver(RecvInner::Records { rx: in_rx, _task: AbortOnDrop(reader) }))
}
