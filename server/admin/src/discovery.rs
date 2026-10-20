//! Annonce de la console sur le réseau local (Bonjour / mDNS), pour que les
//! apps KyberNow trouvent l'ordinateur toutes seules.
//!
//! Service `_kybernow._tcp`, au port de la console, avec dans ses données :
//! `v` (version du format), `name` (nom de l'ordinateur), `fp` (empreinte
//! SHA-256 du certificat de la console, que l'app vérifie à la connexion) et
//! `kyber` (`1` : un serveur Kyber est installé derrière la console ; `0` :
//! console seule, une session n'y démarrerait pas, les apps ne la proposent
//! pas). Une annonce sans `kyber` vient d'une console d'une version précédente :
//! les apps la proposent.

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::service::{ServiceControl, ServiceState};

pub const SERVICE_TYPE: &str = "_kybernow._tcp.local.";

/// Un serveur Kyber utilisable derrière la console : sa configuration est là
/// et son service Windows existe (le service simulé du Mac de développement
/// ne compte pas). `forced` : `--announce-kyber`, pour le développement.
pub fn kyber_ready(forced: bool, config_found: bool, service: &dyn ServiceControl) -> bool {
    forced || (config_found && !service.is_simulated() && !matches!(service.state(), Ok(ServiceState::NotInstalled)))
}

/// Démarre l'annonce ; elle dure tant que le démon renvoyé est gardé.
pub fn announce(port: u16, fingerprint: Option<&str>, kyber: bool) -> Option<ServiceDaemon> {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            return None;
        }
    };
    let name = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "KyberNow".into());
    let host = format!("{}.local.", name.to_lowercase());
    let mut properties = vec![("v", "1".to_string()), ("name", name.clone()), ("kyber", if kyber { "1" } else { "0" }.to_string())];
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
            tracing::info!("annoncé sur le réseau local (Bonjour) : {name}, port {port}, kyber={}", u8::from(kyber));
            Some(daemon)
        }
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn console_sans_kyber() {
        let simulated = crate::service::Simulated::default();
        assert!(!kyber_ready(false, true, &simulated), "le service simulé n'est pas un serveur Kyber");
        assert!(kyber_ready(true, false, &simulated), "--announce-kyber force l'annonce");
    }
}
