//! Ce qui se fait en arrière-plan, sur des fils à part : sondage d'Admin
//! (état, système, aperçu), découverte des Stations (mDNS `_kybernow._tcp`),
//! ajout par code ou par adresse. Les résultats reviennent à la boucle
//! principale par le canal des événements.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::console::{Console, DeviceInfo, PairStatus};

use super::Event;
use crate::app::Found;
use crate::model::{new_id, Host, HostSettings};

/// Sonde un ordinateur : en ligne ou non, son système, et son aperçu s'il en manque un.
pub fn poll(host: Host, want_preview: bool, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
        match c.info() {
            Ok(info) => {
                let os = (!info.os.is_empty()).then(|| info.os.clone());
                let _ = tx.send(Event::HostInfo { id: host.id.clone(), online: true, os, gpu: info.gpu.clone(), av1: info.av1 });
                if want_preview
                    && let Ok(bytes) = c.preview(960)
                    && !bytes.is_empty()
                {
                    let _ = tx.send(Event::Preview { id: host.id.clone(), jpeg: bytes });
                }
            }
            Err(e) => {
                log::info!("{} : hors ligne ({e})", host.name);
                let _ = tx.send(Event::HostInfo { id: host.id.clone(), online: false, os: None, gpu: None, av1: false });
            }
        }
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

/// Réveille un ordinateur (Wake-on-LAN, paquet magique en diffusion).
pub fn wake(mac: &str) -> bool {
    let bytes: Vec<u8> = mac.split([':', '-']).filter_map(|p| u8::from_str_radix(p, 16).ok()).collect();
    if bytes.len() != 6 {
        return false;
    }
    let mut packet = vec![0xFFu8; 6];
    for _ in 0..16 {
        packet.extend_from_slice(&bytes);
    }
    let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") else { return false };
    let _ = sock.set_broadcast(true);
    sock.send_to(&packet, "255.255.255.255:9").is_ok()
}
