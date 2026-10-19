//! Annonce de la console sur le réseau local (Bonjour / mDNS), pour que les
//! apps KyberNow trouvent l'ordinateur toutes seules.
//!
//! Service `_kybernow._tcp`, au port de la console, avec dans ses données :
//! `v` (version du format), `name` (nom de l'ordinateur) et `fp` (empreinte
//! SHA-256 du certificat de la console, que l'app vérifie à la connexion).

use mdns_sd::{ServiceDaemon, ServiceInfo};

pub const SERVICE_TYPE: &str = "_kybernow._tcp.local.";

/// Démarre l'annonce ; elle dure tant que le démon renvoyé est gardé.
pub fn announce(port: u16, fingerprint: Option<&str>) -> Option<ServiceDaemon> {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            return None;
        }
    };
    let name = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "KyberNow".into());
    let host = format!("{}.local.", name.to_lowercase());
    let mut properties = vec![("v", "1".to_string()), ("name", name.clone())];
    if let Some(fp) = fingerprint {
        properties.push(("fp", fp.to_string()));
    }
    let info = match ServiceInfo::new(SERVICE_TYPE, &name, &host, "", port, &properties[..]) {
        Ok(i) => i.enable_addr_auto(),
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            return None;
        }
    };
    match daemon.register(info) {
        Ok(()) => {
            tracing::info!("annoncé sur le réseau local (Bonjour) : {name}, port {port}");
            Some(daemon)
        }
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            None
        }
    }
}
