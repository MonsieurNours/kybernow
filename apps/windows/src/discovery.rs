//! Ordinateurs annoncés sur le réseau local : annonce Bonjour/mDNS
//! `_kybernow._tcp` de la console (TXT `name`, `fp`, `kyber`). Celles marquées
//! `kyber=0` (console sans serveur Kyber derrière) ne sont pas proposées.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;

const SERVICE: &str = "_kybernow._tcp.local.";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub fingerprint: Option<String>,
}

#[derive(Default)]
pub struct Discovery {
    pub found: Mutex<BTreeMap<String, Found>>,
}

impl Discovery {
    pub fn list(&self) -> Vec<Found> {
        self.found.lock().unwrap().values().cloned().collect()
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
                let mut map = state.found.lock().unwrap();
                let before = map.clone();
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let txt = |k: &str| info.get_property_val_str(k).map(str::to_owned);
                        if txt("kyber").as_deref() == Some("0") {
                            continue;
                        }
                        // Une adresse IPv4 de préférence (VPN et IPv6 locales à part).
                        // Les adresses IPv6 locales au lien (`fe80::…%12`) ne font pas une URL.
                        let mut addrs: Vec<String> =
                            info.get_addresses().iter().map(|a| a.to_string()).filter(|a| !a.contains('%')).collect();
                        addrs.sort_by_key(|a| a.contains(':'));
                        let Some(host) = addrs.into_iter().next() else { continue };
                        let fullname = info.get_fullname().to_string();
                        let instance = fullname.trim_end_matches(SERVICE).trim_end_matches('.').to_string();
                        map.insert(
                            fullname.clone(),
                            Found {
                                id: fullname,
                                name: txt("name").unwrap_or(instance),
                                host,
                                port: info.get_port(),
                                fingerprint: txt("fp"),
                            },
                        );
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
