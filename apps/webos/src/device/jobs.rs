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

use kybernow_core::console::{Console, ConsoleError, DeviceInfo, Info, PairStatus, StationError};
use kybernow_core::profile::Capabilities;
use kybernow_core::relay::{self, RelayAddr, RelayMode};
use kybernow_core::share::{self, JoinRequest, Joining, PendingJoin, ShareAvailability, ShareTarget};
use kybernow_core::wake::WakeTarget;

use super::Event;
use crate::app::{Found, Polled, Problem, ShareAvail, ShareOp, ShareView, StationKnown, StationLive, StationOp, StationRefusal};
use crate::model::{new_id, Host, HostRelay, HostSettings};

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
/// comme pour l'appairage) **et** aux relais (le relais par défaut, et ceux des ordinateurs du téléviseur) ; une
/// Station du réseau qui l'accepte en 1,5 s est prise, sinon celle que donne le relais (`share::join_anywhere`, 8 s
/// au plus). Le format de son image est vérifié avec ce que décode ce téléviseur. Hors de chez soi l'hôte accepte
/// chaque invité : `Event::SharePending` dit que la demande attend, puis la réponse (l'invité entre, « L'hôte a
/// refusé. », « L'hôte n'a pas répondu. ») revient avec le numéro de la recherche. Une recherche abandonnée n'ouvre
/// rien et retire sa demande.
pub fn share_join(targets: Vec<ShareTarget>, relays: Vec<RelayAddr>, request: JoinRequest, caps: Capabilities, attempt: u32, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("share-join".into())
        .spawn(move || match share::join_anywhere(&targets, &relays, &request, Some(caps)) {
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
