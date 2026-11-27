//! Ordinateurs annoncés sur le réseau local : annonce Bonjour/mDNS
//! `_kybernow._tcp` de la console (TXT `name`, `fp`, `kyber`). L'app garde sa
//! bibliothèque mDNS ; l'annonce se lit par la règle du cœur
//! (`kybernow_core::discovery::found`, WINAPP-53, la même pour toutes les
//! apps) : `kyber=0` (console sans serveur Kyber derrière) et port nul
//! écartés, une IPv4 hors de 169.254/16 d'abord, jamais une IPv6 locale au
//! lien, l'empreinte mise en forme (`sha256:aa:bb:…`).

use crate::sync::Locked;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent};

pub use kybernow_core::discovery::Found;
use kybernow_core::discovery::{SERVICE, found};

#[derive(Default)]
pub struct Discovery {
    pub found: Mutex<BTreeMap<String, Found>>,
}

impl Discovery {
    pub fn list(&self) -> Vec<Found> {
        self.found.locked().values().cloned().collect()
    }
}

/// Écoute le réseau ; `changed` est appelé à chaque apparition ou départ.
pub fn start(changed: impl Fn(Vec<Found>) + Send + 'static) -> Arc<Discovery> {
    let d = Arc::new(Discovery::default());
    let mdns = match ServiceDaemon::new() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("découverte : {e}");
            return d;
        }
    };
    let rx = match mdns.browse(SERVICE) {
        Ok(rx) => rx,
        Err(e) => {
            log::warn!("découverte : {e}");
            return d;
        }
    };
    let state = d.clone();
    std::thread::Builder::new()
        .name("kybernow-discovery".into())
        .spawn(move || {
            let _daemon = mdns;
            while let Ok(event) = rx.recv() {
                let mut map = state.found.locked();
                let before = map.clone();
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let fullname = info.get_fullname().to_string();
                        let addresses: Vec<IpAddr> = info.get_addresses().iter().map(|a| a.to_ip_addr()).collect();
                        let txt = |k: &str| info.get_property_val_str(k).map(str::to_owned);
                        match found(&fullname, info.get_port(), &addresses, txt) {
                            Some(f) => {
                                map.insert(fullname, f);
                            }
                            // Plus une Station à proposer (Kyber arrêté, plus d'adresse) : retirée.
                            None => {
                                map.remove(&fullname);
                            }
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        map.remove(&fullname);
                    }
                    _ => {}
                }
                if *map != before {
                    let list = map.values().cloned().collect();
                    drop(map);
                    changed(list);
                }
            }
        })
        .expect("fil de découverte");
    d
}
