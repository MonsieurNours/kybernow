//! Ce qui se fait en arrière-plan, sur des fils à part : sondage d'Admin
//! (état, système, Station, aperçu), redémarrage et mise à jour de la
//! Station, découverte des Stations (mDNS `_kybernow._tcp`), ajout par code
//! ou par adresse, recherche de mises à jour ; et le réveil
//! (Wake-on-LAN), envoyé tout de suite. Les résultats reviennent à la boucle
//! principale par le canal des événements.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Info, Mode, PairStatus, StationError};
use kybernow_core::lend;
use kybernow_core::profile::Capabilities;
use kybernow_core::relay::{self, RelayAddr, RelayMode, RelayRoute};
use kybernow_core::share::{self, JoinRequest, Joining, PendingJoin, ShareAvailability, ShareTarget};
use kybernow_core::wake::WakeTarget;

use super::Event;
use crate::app::{BorrowSource, Found, LendAvail, LendHostView, LendLive, LendOp, LendSlotUi, LendState, LendSteam, Polled, Problem, ShareAvail, ShareOp, ShareView, StationKnown, StationLive, StationOp, StationRefusal};
use crate::model::{new_id, Borrowed, Host, HostRelay, HostSettings};

/// Sonde un ordinateur : en ligne ou non, son système, sa Station, et son
/// aperçu s'il en manque un.
pub fn poll(host: Host, want_preview: bool, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
        match c.info() {
            Ok(info) => {
                let station = match &info.station {
                    Some(st) => StationKnown::Info(StationLive {
                        version: st.version.clone(),
                        update_available: st.update.available,
                        update_version: st.update.version.clone(),
                        installable: st.update.installable,
                        requires_setup: st.update.requires_setup,
                        installing: st.update.installing,
                        restarting: st.restarting,
                    }),
                    None => StationKnown::Old(info.admin_version.clone()),
                };
                // Prêt de l'ordinateur : ce que « Prêter l'ordinateur… » peut faire, et, le prêt activé, la vue du prêt.
                let lend_avail = match info.lend {
                    None => LendAvail::Unsupported,
                    Some(l) if l.available && l.enabled => LendAvail::Available,
                    Some(l) if !l.available => LendAvail::Impossible,
                    Some(_) => LendAvail::Off,
                };
                let lend_view = if lend_avail == LendAvail::Available { c.lend_state().ok().map(|v| lend_view(&v)) } else { None };
                let polled = Polled {
                    online: true,
                    away: false,
                    relay: info.relay.as_ref().map(host_relay),
                    os: (!info.os.is_empty()).then(|| info.os.clone()),
                    gpu: info.gpu.clone(),
                    av1: info.av1,
                    virtual_display: Some(info.virtual_display.available),
                    station,
                    problem: None,
                    wake: wake_targets(&info),
                    own_device: Some(info.account.own_device),
                    share: Some(share_avail(&info)),
                    lend: Some(lend_avail),
                    lend_view,
                };
                let _ = tx.send(Event::Polled { id: host.id.clone(), polled });
                if want_preview
                    && let Ok(bytes) = c.preview(960)
                    && !bytes.is_empty()
                {
                    let _ = tx.send(Event::Preview { id: host.id.clone(), jpeg: bytes });
                }
            }
            Err(e) => {
                log::info!("{} : hors ligne ({e})", host.name);
                let problem = match &e {
                    ConsoleError::CertificateChanged { .. } => Some(Problem::Reinstalled),
                    ConsoleError::Http { status: 401, .. } => Some(Problem::Refused),
                    _ => None,
                };
                // Le réseau ne répond pas : l'ordinateur est peut-être joignable par le relais (« Hors de chez vous »).
                let away = away_from_home(&host, &e);
                let _ = tx.send(Event::Polled { id: host.id.clone(), polled: Polled { problem, away, ..Default::default() } });
            }
        }
    });
}

/// Le relais d'un ordinateur tel qu'Admin le donne (`info.relay`), à garder avec lui.
fn host_relay(r: &relay::RelayInfo) -> HostRelay {
    HostRelay { server: r.server.clone(), id: r.id.clone(), remote_access: r.remote_access }
}

/// La présence au relais se redemande toutes les 30 s au plus par ordinateur : chaque appel ouvre une connexion brève,
/// et l'accueil sonde toutes les 8 s.
const PRESENCE_EVERY: Duration = Duration::from_secs(30);

/// Ce que le relais a dit de chaque ordinateur (son identifiant chez lui), et quand.
static PRESENCE: std::sync::Mutex<Option<HashMap<String, (std::time::Instant, bool)>>> = std::sync::Mutex::new(None);

/// L'ordinateur n'a pas répondu sur le réseau : est-il joignable par le relais ? C'est le cas quand sa Station a donné
/// un relais avec « Accès hors de chez vous » actif, que l'empreinte d'Admin est connue (jamais de premier contact par
/// le relais), que `KYBERNOW_RELAY` n'est pas `off`, que l'échec est un silence du réseau (Admin qui répond, même par un
/// refus ou un autre certificat, n'est pas « hors de chez vous ») et que le relais dit l'ordinateur connecté. Un relais
/// muet : « Hors ligne », comme avant.
fn away_from_home(host: &Host, error: &ConsoleError) -> bool {
    if RelayMode::current() == RelayMode::Off || !matches!(error, ConsoleError::Network(_)) || !host.relay.as_ref().is_some_and(|r| r.remote_access) {
        return false;
    }
    let Some(route) = super::session::relay_route(host) else { return false };
    let Ok(addr) = route.relay_addr() else { return false };
    let now = std::time::Instant::now();
    if let Some((at, present)) = PRESENCE.lock().unwrap().as_ref().and_then(|m| m.get(&route.station)).copied()
        && now.duration_since(at) < PRESENCE_EVERY
    {
        return present;
    }
    let present = match relay::presence(&addr, std::slice::from_ref(&route.station)) {
        Ok(list) => list.first().copied().unwrap_or(false),
        Err(e) => {
            log::info!("{} : présence au relais impossible ({e})", host.name);
            false
        }
    };
    PRESENCE.lock().unwrap().get_or_insert_with(HashMap::new).insert(route.station, (now, present));
    present
}

/// Ce que « Partager » peut faire avec cet ordinateur (`Info::share`, le texte est celui de link-ui).
fn share_avail(info: &Info) -> ShareAvail {
    match ShareAvailability::of(info) {
        ShareAvailability::Unsupported => ShareAvail::Unsupported,
        ShareAvailability::Disabled => ShareAvail::Disabled,
        ShareAvailability::Available => ShareAvail::Available,
    }
}

/// Les cartes réseau pour le réveil, telles qu'Admin les donne.
fn wake_targets(info: &Info) -> Vec<crate::model::WakeTarget> {
    info.wake
        .iter()
        .map(|t| crate::model::WakeTarget { mac: t.mac.clone(), ipv4: t.ipv4.clone(), broadcast: t.broadcast.clone(), kind: t.kind.clone(), armed: t.armed, name: t.name.clone() })
        .collect()
}

/// Redémarre ou met à jour la Station (rubrique Station des réglages) ; la
/// réponse revient par `Event::Station`. Une mise à jour attend qu'Admin ait
/// téléchargé et vérifié le paquet (quelques minutes au plus).
pub fn station(host: Host, op: StationOp, force: bool, device_id: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
        let r = match op {
            StationOp::Restart => c.station_restart(Some(&device_id), force).map(|_| None),
            StationOp::Update => c.station_update(Some(&device_id), force).map(Some),
        };
        let result = r.map_err(|e| {
            log::warn!("station : {} refusé par {} ({e})", if op == StationOp::Restart { "redémarrage" } else { "mise à jour" }, host.name);
            match e {
                StationError::InSession { device, .. } => StationRefusal::InSession(device),
                StationError::TooSoon { .. } => StationRefusal::TooSoon,
                StationError::Unsupported => StationRefusal::Unsupported,
                StationError::RequiresSetup { .. } => StationRefusal::RequiresSetup,
                e => StationRefusal::Other(e.to_string()),
            }
        });
        if result.is_ok() {
            log::info!("station : {} accepté par {}", if op == StationOp::Restart { "redémarrage" } else { "mise à jour" }, host.name);
        }
        let _ = tx.send(Event::Station { id: host.id.clone(), op, result });
    });
}

/// Découverte des Stations sur le réseau (annonce Bonjour d'Admin).
pub fn discover(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let daemon = match mdns_sd::ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => {
                log::warn!("découverte : mDNS impossible ({e})");
                return;
            }
        };
        let rx = match daemon.browse("_kybernow._tcp.local.") {
            Ok(rx) => rx,
            Err(e) => {
                log::warn!("découverte : recherche impossible ({e})");
                return;
            }
        };
        let mut found: HashMap<String, Found> = HashMap::new();
        while let Ok(ev) = rx.recv() {
            match ev {
                mdns_sd::ServiceEvent::ServiceResolved(info) => {
                    let Some(addr) = info.get_addresses().iter().find(|a| a.is_ipv4()).or_else(|| info.get_addresses().iter().next()) else { continue };
                    let name = info.get_property_val_str("name").map(str::to_string).unwrap_or_else(|| {
                        info.get_fullname().split("._kybernow").next().unwrap_or("Station").to_string()
                    });
                    let fp = info.get_property_val_str("fp").map(|s| if s.starts_with("sha256:") { s.to_string() } else { format!("sha256:{s}") });
                    let f = Found { name, address: addr.to_string(), port: info.get_port(), fingerprint: fp };
                    found.insert(info.get_fullname().to_string(), f);
                    let _ = tx.send(Event::Found(found.values().cloned().collect()));
                }
                mdns_sd::ServiceEvent::ServiceRemoved(_, full) => {
                    if found.remove(&full).is_some() {
                        let _ = tx.send(Event::Found(found.values().cloned().collect()));
                    }
                }
                _ => {}
            }
        }
    });
}

/// Premier contact par adresse : l'empreinte du certificat d'Admin.
pub fn probe(address: String, port: u16, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let c = Console::new(&address, port, "", "", None);
        let _ = c.probe();
        let r = c.fingerprint_seen().ok_or_else(|| crate::i18n::tr("Admin ne répond pas à cette adresse", "Admin does not answer at this address").to_string());
        let _ = tx.send(Event::Probed { address, port, fingerprint: r });
    });
}

/// Une Station à qui présenter le code.
#[derive(Clone)]
pub struct Target {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub fingerprint: Option<String>,
}

/// Présente le code aux Stations et attend qu'Admin l'accepte (ou le
/// refuse) ; s'arrête quand `cancel` passe à vrai.
pub fn pair(targets: Vec<Target>, code: String, device: DeviceInfo, cancel: Arc<AtomicBool>, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let mut pending = Vec::new();
        for t in targets {
            let c = Console::new(&t.address, t.port, "", "", t.fingerprint.clone());
            match c.pair_request(&code, &device) {
                Ok(id) => {
                    let _ = tx.send(Event::PairAsked { name: t.name.clone(), address: t.address.clone() });
                    pending.push((t, id));
                }
                Err(e) => log::warn!("code : {} refuse la demande ({e})", t.address),
            }
        }
        if pending.is_empty() {
            let _ = tx.send(Event::PairFailed(crate::i18n::tr("Aucune Station n'a reçu le code : est-elle à jour, sur ce réseau ?", "No Station got the code: is it up to date, on this network?").into()));
            return;
        }
        while !pending.is_empty() && !cancel.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(2));
            let mut i = 0;
            while i < pending.len() {
                let (t, id) = &pending[i];
                let c = Console::new(&t.address, t.port, "", "", t.fingerprint.clone());
                match c.pair_status(id) {
                    Ok(PairStatus::Accepted { username, password }) => {
                        let fp = t.fingerprint.clone().or_else(|| c.fingerprint_seen());
                        let mut host = Host {
                            id: new_id(),
                            name: t.name.clone(),
                            address: t.address.clone(),
                            port: t.port,
                            fingerprint: fp.clone(),
                            user: username,
                            password,
                            os: None,
                            gpu: None,
                            av1: false,
                            mac: None,
                            wake: Vec::new(),
                            relay: None,
                            settings: HostSettings::default(),
                            last: None,
                        };
                        let c = Console::new(&host.address, host.port, &host.user, &host.password, fp);
                        if let Ok(info) = c.info() {
                            if !info.hostname.is_empty() {
                                host.name = info.hostname.clone();
                            }
                            host.os = (!info.os.is_empty()).then(|| info.os.clone());
                            host.gpu = info.gpu.clone();
                            host.av1 = info.av1;
                            host.wake = wake_targets(&info);
                        }
                        let _ = tx.send(Event::PairAccepted(host));
                        return;
                    }
                    Ok(PairStatus::Refused) => {
                        pending.remove(i);
                        let _ = tx.send(Event::PairFailed(crate::i18n::tr("Code refusé dans Admin", "Code refused in Admin").into()));
                        continue;
                    }
                    Ok(PairStatus::Expired) => {
                        pending.remove(i);
                        let _ = tx.send(Event::PairFailed(crate::i18n::tr("Code expiré : recommencez", "Code expired: try again").into()));
                        continue;
                    }
                    _ => {}
                }
                i += 1;
            }
        }
    });
}

/// Réveille un ordinateur (Wake-on-LAN) par le cœur : ses cartes données par
/// Admin, ou l'adresse MAC écrite à la main ; le message d'erreur sinon.
pub fn wake(host: &Host) -> Result<(), String> {
    let mut targets: Vec<WakeTarget> = host
        .wake
        .iter()
        .map(|t| WakeTarget { mac: t.mac.clone(), ipv4: t.ipv4.clone(), broadcast: t.broadcast.clone(), kind: t.kind.clone(), armed: t.armed, name: t.name.clone() })
        .collect();
    if targets.is_empty()
        && let Some(t) = host.mac.as_deref().and_then(|m| WakeTarget::from_mac(m, Some(&host.address)))
    {
        targets.push(t);
    }
    match kybernow_core::wake::send(&targets) {
        Ok(n) => {
            log::info!("réveil : {n} paquet(s) envoyé(s) à {}", host.name);
            Ok(())
        }
        Err(e) => {
            log::warn!("réveil : {} ({e})", host.name);
            Err(e.to_string())
        }
    }
}

/// Nom du paquet de l'app des téléviseurs LG dans chaque version publiée.
pub const IPK: &str = "KyberNow-Link-webOS.ipk";

/// La dernière version du canal qui publie le paquet webOS, plus récente que
/// celle-ci et vérifiée par le manifeste signé (le cœur, comme sur Android).
/// L'app ne s'installe pas elle-même (mode développeur) : elle le signale.
pub fn check_update(beta: bool, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("update".into())
        .spawn(move || {
            let channel = if beta { kybernow_core::update::Channel::Beta } else { kybernow_core::update::Channel::Stable };
            let r = kybernow_core::update::check(env!("CARGO_PKG_VERSION"), channel, &[IPK]);
            let _ = tx.send(Event::Update(r));
        })
        .ok();
}

/// La demande d'un invité venu par le relais, en attente de l'accord de l'hôte : de quoi la retirer (« Annuler »).
struct JoinWait {
    attempt: u32,
    pending: PendingJoin,
    cancel: Arc<AtomicBool>,
}

/// L'attente en cours (une seule : « Rejoindre » est un écran).
static JOIN_WAIT: std::sync::Mutex<Option<JoinWait>> = std::sync::Mutex::new(None);

/// KyberNow Share, invité : le code part à tous ces ordinateurs à la fois (les Stations trouvées sur le réseau,
/// comme pour l'appairage) **et** aux relais (le relais public, `share.kybernow.app`, que le cœur interroge
/// toujours, et ceux des ordinateurs du téléviseur, `relays`) ; une Station du réseau qui l'accepte en 1,5 s est
/// prise, sinon celle que donne le relais (`share::join_anywhere`, 8 s au plus). `KYBERNOW_RELAY=off` : le réseau
/// seulement. Le format de son image est vérifié avec ce que décode ce téléviseur. Hors de chez soi l'hôte accepte
/// chaque invité : `Event::SharePending` dit que la demande attend, puis la réponse (l'invité entre, « L'hôte a
/// refusé. », « L'hôte n'a pas répondu. ») revient avec le numéro de la recherche. Une recherche abandonnée n'ouvre
/// rien et retire sa demande.
///
/// Un lien de partage (`link`) : le code part à sa seule Station, puis à son relais (`share::join_link`).
pub fn share_join(targets: Vec<ShareTarget>, relays: Vec<RelayAddr>, link: Option<share::JoinLink>, request: JoinRequest, caps: Capabilities, attempt: u32, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("share-join".into())
        .spawn(move || match join(&targets, &relays, link.as_ref(), &request, caps) {
            Ok(Joining::Joined(found)) => {
                let _ = tx.send(Event::ShareJoined { attempt, result: Ok(found) });
            }
            Ok(Joining::Pending(pending)) => share_wait(pending, caps, attempt, &tx),
            Err(e) => {
                let _ = tx.send(Event::ShareJoined { attempt, result: Err(e) });
            }
        })
        .ok();
}

/// Le code d'un partage aux Stations du réseau et aux relais (`share::join_anywhere`, qui ajoute lui-même le relais
/// public) ; `KYBERNOW_RELAY=off` : aux Stations du réseau seulement (`share::join_checked`), jamais à un relais.
fn join(targets: &[ShareTarget], relays: &[RelayAddr], link: Option<&share::JoinLink>, request: &JoinRequest, caps: Capabilities) -> Result<Joining, share::ShareError> {
    if RelayMode::current() == RelayMode::Off {
        let only = link.map(|l| vec![l.target()]);
        return share::join_checked(only.as_deref().unwrap_or(targets), request, caps).map(Joining::Joined);
    }
    match link {
        Some(l) => share::join_link(l, request, Some(caps)),
        None => share::join_anywhere(targets, relays, request, Some(caps)),
    }
}

/// L'attente de l'accord de l'hôte : `share::wait` tient son appel 25 s, on le rappelle tant que l'hôte n'a pas répondu
/// et que la demande n'est pas annulée.
fn share_wait(pending: PendingJoin, caps: Capabilities, attempt: u32, tx: &Sender<Event>) {
    let cancel = Arc::new(AtomicBool::new(false));
    *JOIN_WAIT.lock().unwrap() = Some(JoinWait { attempt, pending: pending.clone(), cancel: cancel.clone() });
    let _ = tx.send(Event::SharePending { attempt });
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match share::wait(&pending, Some(caps)) {
            Ok(None) => continue,
            Ok(Some(found)) => break Ok(found),
            Err(e) => break Err(e),
        }
    };
    {
        let mut wait = JOIN_WAIT.lock().unwrap();
        if wait.as_ref().is_some_and(|w| w.attempt == attempt) {
            *wait = None;
        }
    }
    // Annulée entre-temps : l'invité qui vient d'entrer repart (la boucle principale le fait pour une recherche abandonnée).
    let _ = tx.send(Event::ShareJoined { attempt, result });
}

/// « Annuler » (ou Retour) pendant l'attente de l'accord de l'hôte : la demande est retirée chez l'hôte, l'attente s'arrête.
pub fn share_cancel() {
    let Some(w) = JOIN_WAIT.lock().unwrap().take() else { return };
    w.cancel.store(true, Ordering::Relaxed);
    std::thread::Builder::new().name("share-cancel".into()).spawn(move || share::cancel(&w.pending)).ok();
}

/// L'invité repart avant d'avoir ouvert sa session (recherche abandonnée) : la Station le sait, il ne reste
/// pas dans la liste de l'hôte.
pub fn share_leave(found: share::Found) {
    std::thread::Builder::new().name("share-leave".into()).spawn(move || share::leave(&found)).ok();
}

/// KyberNow Share, hôte : une demande à Admin avec le compte de l'appareil en session (« Partager »,
/// « Nouveau code », un rôle, « Retirer », « Arrêter le partage »), puis le partage tel qu'il est maintenant.
pub fn share_host(host: Host, op: ShareOp, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("share-host".into())
        .spawn(move || {
            let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
            let result = share_run(&c, &op);
            if let Err(e) = &result {
                log::warn!("partage : {e}");
            }
            let _ = tx.send(Event::ShareHost(result));
        })
        .ok();
}

fn share_run(c: &Console, op: &ShareOp) -> Result<Option<ShareView>, String> {
    let lang = kybernow_core::i18n::current();
    let text = |e: ConsoleError| e.text(lang);
    match op {
        ShareOp::Open => {
            c.share_open().map_err(text)?;
        }
        ShareOp::NewCode => {
            c.share_code().map_err(text)?;
        }
        ShareOp::Role(id, role) => c.share_guest(id, super::session::core_role(*role)).map_err(text)?,
        ShareOp::Kick(id) => c.share_kick(id).map_err(text)?,
        ShareOp::Accept(id) => {
            c.share_accept(id).map_err(text)?;
        }
        ShareOp::Refuse(id) => {
            c.share_refuse(id).map_err(text)?;
        }
        ShareOp::Close => {
            c.share_close().map_err(text)?;
            return Ok(None);
        }
    }
    Ok(super::session::share_view(&c.share_state().map_err(text)?))
}

// MARK: - Prêt de l'ordinateur (docs/pret.md, cœur : `kybernow_core::lend`)

/// L'emprunt en attente de l'accord de l'hôte : de quoi le retirer (« Annuler »).
struct BorrowWait {
    source: BorrowSource,
    pending: lend::PendingBorrow,
    cancel: Arc<AtomicBool>,
}

/// L'attente en cours (une seule : l'emprunt part d'un écran à la fois).
static BORROW_WAIT: std::sync::Mutex<Option<BorrowWait>> = std::sync::Mutex::new(None);

/// Le dernier emprunt demandé (la Station, le chemin, la demande) : « Revenir » le refait tel quel.
type LastBorrow = (ShareTarget, Option<RelayRoute>, lend::BorrowRequest);
static LAST_BORROW: std::sync::Mutex<Option<LastBorrow>> = std::sync::Mutex::new(None);

/// La demande d'emprunt de cet appareil : le prénom, l'identifiant du téléviseur, `webos`, l'écran (la Station fait
/// l'écran virtuel à sa taille) et les formats que l'appareil décode.
pub fn borrow_request(credentials: lend::LendCredentials, name: String, device: String, screen: (u32, u32, u32), av1: bool) -> lend::BorrowRequest {
    let mut codecs = vec!["hevc".to_string(), "h264".to_string()];
    if av1 {
        codecs.push("av1".into());
    }
    lend::BorrowRequest { credentials, name, device, app: "webos".into(), screen: Mode { width: screen.0, height: screen.1, refresh: screen.2 }, codecs }
}

/// Emprunter (`lend::borrow`, 30 s au plus) : le prêt démarre (`Event::Borrowed`), ou l'hôte doit accepter
/// (`Event::BorrowPending` « En attente de {hôte}… », puis `lend::wait` rappelé tant qu'il n'a pas répondu). Le texte d'un
/// refus est celui du cœur, avec le prénom de l'hôte quand on le connaît.
pub fn borrow(source: BorrowSource, target: ShareTarget, route: Option<RelayRoute>, request: lend::BorrowRequest, tx: Sender<Event>) {
    *LAST_BORROW.lock().unwrap() = Some((target.clone(), route.clone(), request.clone()));
    std::thread::Builder::new()
        .name("lend-borrow".into())
        .spawn(move || {
            let lang = kybernow_core::i18n::current();
            match lend::borrow(&target, route.as_ref(), &request) {
                Ok(lend::Borrowing::Ready(loan)) => {
                    let _ = tx.send(Event::Borrowed { source, result: Ok(Box::new(loan)) });
                }
                Ok(lend::Borrowing::Pending(pending)) => borrow_wait(source, pending, &tx),
                Err(e) => {
                    log::info!("prêt : emprunt refusé ({})", e.kind());
                    let _ = tx.send(Event::Borrowed { source, result: Err(e.text(lang, "")) });
                }
            }
        })
        .ok();
}

/// L'attente de l'accord de l'hôte : `lend::wait` tient son appel 30 s, rappelé tant que l'hôte n'a pas répondu et que
/// la demande n'est pas annulée.
fn borrow_wait(source: BorrowSource, pending: lend::PendingBorrow, tx: &Sender<Event>) {
    let cancel = Arc::new(AtomicBool::new(false));
    *BORROW_WAIT.lock().unwrap() = Some(BorrowWait { source: source.clone(), pending: pending.clone(), cancel: cancel.clone() });
    let _ = tx.send(Event::BorrowPending { source: source.clone(), host: pending.host.clone() });
    let lang = kybernow_core::i18n::current();
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match lend::wait(&pending) {
            Ok(None) => continue,
            Ok(Some(loan)) => break Ok(Box::new(loan)),
            Err(e) => break Err(e.text(lang, &pending.host)),
        }
    };
    {
        let mut wait = BORROW_WAIT.lock().unwrap();
        if wait.as_ref().is_some_and(|w| w.source == source) {
            *wait = None;
        }
    }
    let _ = tx.send(Event::Borrowed { source, result });
}

/// « Annuler » (ou retour) pendant l'attente de l'accord de l'hôte : la demande est retirée, l'attente s'arrête.
pub fn borrow_cancel() {
    let Some(w) = BORROW_WAIT.lock().unwrap().take() else { return };
    w.cancel.store(true, Ordering::Relaxed);
    std::thread::Builder::new().name("lend-cancel".into()).spawn(move || lend::cancel(&w.pending)).ok();
}

/// « Revenir » : le dernier emprunt, refait (15 min après une fin sans le prêt, la Station le reconnaît).
pub fn come_back(source: BorrowSource, tx: Sender<Event>) {
    let last = LAST_BORROW.lock().unwrap().clone();
    match last {
        Some((target, route, request)) => borrow(source, target, route, request, tx),
        None => {
            let _ = tx.send(Event::Borrowed { source, result: Err(lend::LendError::NoCode.text(kybernow_core::i18n::current(), "")) });
        }
    }
}

/// Un emprunt accordé qui n'ouvre rien (l'écran a changé, plus de place pour une session de plus) : l'ordinateur est
/// rendu tout de suite, l'hôte le retrouve.
pub fn borrow_leave(loan: lend::Loan) {
    std::thread::Builder::new()
        .name("lend-leave".into())
        .spawn(move || {
            if let Err(e) = lend::leave(&loan) {
                log::warn!("prêt : ordinateur pas rendu ({})", e.kind());
            }
        })
        .ok();
}

/// L'état de l'ordinateur d'un ami, pour l'interface (`lend::status`).
pub fn lend_live(s: &lend::LendStatus) -> LendLive {
    let state = match s.state {
        lend::LendAvailability::Free => LendState::Free,
        lend::LendAvailability::Busy => LendState::Busy,
        lend::LendAvailability::Lent if s.mine => LendState::Mine,
        lend::LendAvailability::Lent => LendState::Lent,
        lend::LendAvailability::Locked => LendState::Locked,
        lend::LendAvailability::Off => LendState::Off,
    };
    let slot = s.slot.as_ref().and_then(|sl| sl.next.map(|p| LendSlotUi { now: sl.now, start: p.start, end: p.end }));
    LendLive { state: Some(state), slot }
}

/// Le chemin par le relais d'un ordinateur emprunté (son lien d'invitation) : le relais, l'identifiant de la Station
/// et l'empreinte d'Admin épinglée ; `None` sans l'un d'eux, ou avec `KYBERNOW_RELAY=off`. Le cœur essaie le réseau
/// d'abord (1,5 s), puis le relais.
pub fn borrowed_route(relay: Option<&str>, sid: Option<&str>, fingerprint: Option<&str>) -> Option<RelayRoute> {
    if RelayMode::current() == RelayMode::Off {
        return None;
    }
    let route = RelayRoute { server: relay?.to_string(), station: sid?.to_string(), admin_fp: fingerprint?.to_string() };
    route.relay_addr().ok().map(|_| route)
}

/// Sonde l'ordinateur d'un ami (la carte « Ordinateur de {hôte} ») : son état et son créneau ; « Hors ligne » quand sa
/// Station ne répond pas. Un compte refusé (mot de passe changé) : l'état reste inconnu, « Emprunter » dira pourquoi.
pub fn lend_poll(b: Borrowed, password: String, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("lend-status".into())
        .spawn(move || {
            let target = ShareTarget { host: b.address.clone(), port: b.port, fingerprint: b.fingerprint.clone() };
            let route = borrowed_route(b.relay.as_deref(), b.sid.as_deref(), b.fingerprint.as_deref());
            let live = match lend::status(&target, route.as_ref(), &b.login, &password) {
                Ok(s) => lend_live(&s),
                Err(lend::LendError::Other(ConsoleError::Network(_))) => LendLive { state: Some(LendState::Offline), slot: None },
                Err(e) => {
                    log::info!("prêt : état de {} inconnu ({})", b.computer, e.kind());
                    LendLive::default()
                }
            };
            let _ = tx.send(Event::LendStatus { id: b.id.clone(), live });
        })
        .ok();
}

/// « Avec un compte d'ami » : l'identifiant et le mot de passe présentés à la Station (`lend::status`), qui dit le prénom
/// de l'hôte, le nom de l'ordinateur et son état ; l'ordinateur emprunté est alors prêt à garder. Un lien d'invitation
/// donne aussi le relais de la Station (`relay`, `sid`), gardé avec l'ordinateur.
#[allow(clippy::too_many_arguments)]
pub fn friend_add(address: String, port: u16, fingerprint: Option<String>, relay: Option<String>, sid: Option<String>, login: String, password: String, remember: bool, editing: Option<String>, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("lend-friend".into())
        .spawn(move || {
            let lang = kybernow_core::i18n::current();
            let target = ShareTarget { host: address.clone(), port, fingerprint: fingerprint.clone() };
            let route = borrowed_route(relay.as_deref(), sid.as_deref(), fingerprint.as_deref());
            let result = lend::status(&target, route.as_ref(), &login, &password).map_err(|e| e.text(lang, "")).map(|s| {
                let b = Borrowed {
                    id: editing.clone().unwrap_or_else(|| format!("lend:{}", new_id())),
                    host: s.host.clone(),
                    computer: s.computer.clone(),
                    address: address.clone(),
                    port,
                    fingerprint: fingerprint.clone(),
                    relay: relay.clone(),
                    sid: sid.clone(),
                    login: login.clone(),
                    password: if remember { password.clone() } else { String::new() },
                    remember,
                };
                Box::new((b, lend_live(&s), password.clone()))
            });
            let _ = tx.send(Event::FriendAdded(result));
        })
        .ok();
}

/// La vue du prêt pour l'interface (`LendView` du cœur).
pub fn lend_view(v: &lend::LendView) -> LendHostView {
    use crate::app::{LendCodeUi, LendHostState, LendNoticeKind, LendRequestUi};
    let steam = |s: lend::LendSteam| if s == lend::LendSteam::Host { LendSteam::Host } else { LendSteam::Friend };
    LendHostView {
        enabled: v.enabled,
        state: match v.state {
            lend::LendHostState::Free => LendHostState::Free,
            lend::LendHostState::Preparing => LendHostState::Preparing,
            lend::LendHostState::Lent => LendHostState::Lent,
            lend::LendHostState::Ending => LendHostState::Ending,
        },
        borrower: v.borrower.as_ref().map(|b| (b.name.clone(), b.until)),
        notice: v.notice.map(|n| {
            let kind = match n.reason {
                lend::LendNoticeReason::TakenBack => LendNoticeKind::TakenBack,
                lend::LendNoticeReason::TimeUp => LendNoticeKind::TimeUp,
                lend::LendNoticeReason::SlotEnd => LendNoticeKind::SlotEnd,
                lend::LendNoticeReason::SlotStart => LendNoticeKind::SlotStart,
            };
            (kind, n.at)
        }),
        requests: v.requests.iter().map(|r| LendRequestUi { id: r.id.clone(), name: r.name.clone(), app: r.app.clone(), via_relay: r.via == share::ShareVia::Relay, steam: steam(r.steam), slot: r.slot }).collect(),
        // Le relais du code comme celui du partage ; le moment de sa préparation est posé par l'app
        // (`LendHostView::keep_relay_since`).
        code: v.code.as_ref().map(|c| LendCodeUi { code: c.code.clone(), link: c.link.clone(), web: c.web.clone(), duration: c.duration, steam: steam(c.steam), relay: super::session::relay_of(c.relay), relay_since: None }),
    }
}

/// Le prêt de l'ordinateur, côté hôte : une demande à Admin avec le compte de l'appareil (« Créer un code », « Annuler
/// le code », « Accepter », « Refuser », « Reprendre l'ordinateur »), puis la vue du prêt telle qu'elle est.
fn lend_run(host: &Host, op: &LendOp) -> Result<LendHostView, String> {
    let lang = kybernow_core::i18n::current();
    let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
    let core_steam = |s: LendSteam| if s == LendSteam::Host { lend::LendSteam::Host } else { lend::LendSteam::Friend };
    let view = match op {
        LendOp::Code(duration, steam) => c.lend_code(*duration, core_steam(*steam)),
        LendOp::CancelCode => c.lend_cancel_code(),
        LendOp::Answer(id, accept, steam) => c.lend_answer(id, *accept, core_steam(*steam)),
        LendOp::TakeBack(now) => c.lend_take_back(*now),
    };
    view.map(|v| lend_view(&v)).map_err(|e| {
        log::warn!("prêt : {}", e.text(kybernow_core::i18n::Lang::Fr));
        e.text(lang)
    })
}

/// En session : la demande du prêt, puis `Event::LendHost`.
pub fn lend_host(host: Host, op: LendOp, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("lend-host".into())
        .spawn(move || {
            let result = lend_run(&host, &op);
            let _ = tx.send(Event::LendHost(result));
        })
        .ok();
}

/// Sur l'accueil (« Reprendre l'ordinateur ») : la demande, puis `Event::LendHome`.
pub fn lend_home(host: Host, op: LendOp, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("lend-home".into())
        .spawn(move || {
            let result = lend_run(&host, &op);
            let _ = tx.send(Event::LendHome { host: host.id.clone(), result });
        })
        .ok();
}
