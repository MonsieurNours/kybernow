//! Ce qui se fait en arrière-plan, sur des fils à part : sondage d'Admin
//! (état, système, aperçu), découverte des Stations (mDNS `_kybernow._tcp`),
//! ajout par code ou par adresse. Les résultats reviennent à la boucle
//! principale par le canal des événements.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::console::{Console, DeviceInfo, Info, PairStatus, StationError};
use kybernow_core::wake::WakeTarget;

use super::Event;
use crate::app::{Found, StationKind, StationLive, StationRefusal};
use crate::model::{new_id, Host, HostSettings, WakeCard};

/// Sonde un ordinateur : en ligne ou non, son système, sa Station, et son
/// aperçu s'il en manque un.
pub fn poll(host: Host, want_preview: bool, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
        match c.info() {
            Ok(info) => {
                let os = (!info.os.is_empty()).then(|| info.os.clone());
                let station = station_live(&info);
                let _ = tx.send(Event::HostInfo {
                    id: host.id.clone(),
                    online: true,
                    os,
                    gpu: info.gpu.clone(),
                    av1: info.av1,
                    virtual_display: Some(info.virtual_display.available),
                    max_screens: Some(info.virtual_display.max_screens()),
                    station: Some(station),
                    wake: wake_cards(&info),
                    own_device: info.station.is_some().then_some(info.account.own_device),
                });
                if want_preview
                    && let Ok(bytes) = c.preview(960)
                    && !bytes.is_empty()
                {
                    let _ = tx.send(Event::Preview { id: host.id.clone(), jpeg: bytes });
                }
            }
            Err(e) => {
                log::info!("{} : hors ligne ({e})", host.name);
                let _ = tx.send(Event::HostInfo { id: host.id.clone(), online: false, os: None, gpu: None, av1: false, virtual_display: None, max_screens: None, station: None, wake: Vec::new(), own_device: None });
            }
        }
    });
}

/// La Station vue par l'interface : celle d'`Info::station` ; une Station
/// d'avant la 0.5.0 n'en dit rien (sa version est celle d'Admin).
fn station_live(info: &Info) -> StationLive {
    match &info.station {
        Some(s) => StationLive {
            version: s.version.clone(),
            supported: true,
            available: s.update.available,
            update_version: s.update.version.clone(),
            installable: s.update.installable,
            requires_setup: s.update.requires_setup,
            installing: s.update.installing,
            restarting: s.restarting,
        },
        None => StationLive { version: info.admin_version.clone().unwrap_or_default(), ..Default::default() },
    }
}

/// Redémarre ou met à jour la Station (`POST /api/stream/station/restart`,
/// `/update`) ; la réponse revient en `Event::StationReply`. Une mise à jour
/// attend qu'Admin ait téléchargé et vérifié la version (quelques minutes au plus).
pub fn station(host: Host, kind: StationKind, force: bool, device_id: String, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("station".into())
        .spawn(move || {
            let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
            let r = match kind {
                StationKind::Restart => c.station_restart(Some(&device_id), force).map(|_| None),
                StationKind::Update => c.station_update(Some(&device_id), force).map(Some),
            };
            let result = r.map_err(|e| {
                log::warn!("station : {} refusé ({e})", if kind == StationKind::Update { "mise à jour" } else { "redémarrage" });
                match e {
                    StationError::InSession { device, .. } => StationRefusal::InSession(device),
                    StationError::TooSoon { .. } => StationRefusal::TooSoon,
                    StationError::RequiresSetup { .. } => StationRefusal::RequiresSetup,
                    StationError::Unsupported => StationRefusal::Unsupported,
                    other => StationRefusal::Other(other.to_string()),
                }
            });
            let _ = tx.send(Event::StationReply { host: host.id.clone(), kind, result });
        })
        .ok();
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
                    let _ = tx.send(Event::PairAsked { name: t.name.clone() });
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
                            host.wake = wake_cards(&info);
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

/// Réveille un ordinateur par le réseau (le cœur, `kybernow_core::wake`) :
/// le paquet magique part vers chaque carte apprise d'Admin et vers la MAC
/// saisie à la main (avec l'adresse de l'ordinateur pour l'envoi direct).
/// La réponse revient en `Event::Woken` : le nombre de paquets partis, ou pourquoi rien n'est parti.
pub fn wake(host: Host, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("wake".into())
        .spawn(move || {
            let mut targets: Vec<WakeTarget> = host.wake.iter().map(core_target).collect();
            if let Some(t) = host.mac.as_deref().and_then(|m| WakeTarget::from_mac(m, Some(&host.address))) {
                targets.push(t);
            }
            let r = kybernow_core::wake::send(&targets).map_err(|e| e.to_string());
            match &r {
                Ok(n) => log::info!("réveil de {} : {n} paquet(s) envoyé(s)", host.name),
                Err(e) => log::warn!("réveil de {} : {e}", host.name),
            }
            let _ = tx.send(Event::Woken { id: host.id.clone(), result: r.map(|_| ()) });
        })
        .ok();
}

/// Une carte gardée (`WakeCard`, le JSON du cœur) vers celle du cœur.
fn core_target(c: &WakeCard) -> WakeTarget {
    WakeTarget { mac: c.mac.clone(), ipv4: c.ipv4.clone(), broadcast: c.broadcast.clone(), kind: c.kind.clone(), armed: c.armed, name: c.name.clone() }
}

/// Les cartes d'`info`, à garder pour l'ordinateur.
fn wake_cards(info: &Info) -> Vec<WakeCard> {
    info.wake.iter().map(|t| WakeCard { mac: t.mac.clone(), ipv4: t.ipv4.clone(), broadcast: t.broadcast.clone(), kind: t.kind.clone(), armed: t.armed, name: t.name.clone() }).collect()
}

/// Le paquet Flatpak de cette architecture dans chaque version publiée.
#[cfg(target_arch = "x86_64")]
pub const FLATPAK: &str = "KyberNow-Link-Linux-x86_64.flatpak";
#[cfg(not(target_arch = "x86_64"))]
pub const FLATPAK: &str = "KyberNow-Link-Linux-aarch64.flatpak";
/// Le paquet .deb du Raspberry Pi (arm64).
pub const DEB: &str = "KyberNow-Link-RaspberryPi-arm64.deb";

/// Le fichier que cette installation cherche dans une version publiée : le
/// Flatpak dans le bac à sable, le .deb hors de lui sur ARM (Raspberry Pi).
pub fn asset() -> &'static str {
    if super::os::is_os() {
        OS_IMAGE
    } else if !crate::app::flatpak() && cfg!(target_arch = "aarch64") {
        DEB
    } else {
        FLATPAK
    }
}
/// L'image de KyberNow Link OS (mise à jour A/B).
pub const OS_IMAGE: &str = "KyberNow-Link-OS.img.xz";

/// La dernière version trouvée, gardée pour la télécharger.
static AVAILABLE: std::sync::Mutex<Option<kybernow_core::update::Available>> = std::sync::Mutex::new(None);

pub fn remember(a: kybernow_core::update::Available) {
    *AVAILABLE.lock().unwrap() = Some(a);
}

/// KyberNow Link OS : télécharge l'image publiée dans le dossier des mises à
/// jour (partition de données), vérifiée par le cœur (taille, SHA-256 du
/// manifeste signé), puis la signale prête.
pub fn download_os(tx: Sender<Event>) {
    let Some(a) = AVAILABLE.lock().unwrap().clone() else {
        let _ = tx.send(Event::OsDownloaded(Err(crate::i18n::tr("Aucune version à télécharger", "No version to download").into())));
        return;
    };
    std::thread::Builder::new()
        .name("os-update".into())
        .spawn(move || {
            let dir = super::os::update_dir();
            let _ = std::fs::create_dir_all(&dir);
            let dest = dir.join(&a.file);
            let total = a.size;
            let tx2 = tx.clone();
            let last = std::sync::Mutex::new(std::time::Instant::now());
            let r = kybernow_core::update::download(&a, &dest, move |done| {
                let mut l = last.lock().unwrap();
                if l.elapsed() > std::time::Duration::from_millis(400) {
                    *l = std::time::Instant::now();
                    let _ = tx2.send(Event::OsDownload(done, total));
                }
            });
            let _ = tx.send(Event::OsDownloaded(r.map(|_| dest)));
        })
        .ok();
}

/// La dernière version du canal qui publie le paquet de cette installation,
/// plus récente que celle-ci et vérifiée par le manifeste signé (le cœur,
/// comme sur Android et webOS). Un Flatpak ou un .deb installé depuis un
/// fichier ne se met pas à jour seul : l'app le signale.
pub fn check_update(beta: bool, tx: Sender<Event>) {
    std::thread::Builder::new()
        .name("update".into())
        .spawn(move || {
            let channel = if beta { kybernow_core::update::Channel::Beta } else { kybernow_core::update::Channel::Stable };
            let r = kybernow_core::update::check(env!("CARGO_PKG_VERSION"), channel, &[asset()]);
            let _ = tx.send(Event::Update(r));
        })
        .ok();
}
