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
use kybernow_core::wake::WakeTarget;

use super::Event;
use crate::app::{Found, Polled, Problem, StationKnown, StationLive, StationOp, StationRefusal};
use crate::model::{new_id, Host, HostSettings};

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
                    os: (!info.os.is_empty()).then(|| info.os.clone()),
                    gpu: info.gpu.clone(),
                    av1: info.av1,
                    virtual_display: Some(info.virtual_display.available),
                    station,
                    problem: None,
                    wake: wake_targets(&info),
                    own_device: Some(info.account.own_device),
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
                let _ = tx.send(Event::Polled { id: host.id.clone(), polled: Polled { problem, ..Default::default() } });
            }
        }
    });
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
