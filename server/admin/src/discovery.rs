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
    let name = crate::system::computer_name().unwrap_or_else(|| "KyberNow".into());
    // Nom réseau pour l'adresse (sans espaces) ; le nom affiché peut en avoir.
    let host_name = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "kybernow".into());
    let host = format!("{}.local.", host_name.trim_end_matches(".local").to_lowercase());
    let mut properties = vec![("v", "1".to_string()), ("name", name.clone()), ("kyber", if kyber { "1" } else { "0" }.to_string())];
    if let Some(fp) = fingerprint {
        properties.push(("fp", fp.to_string()));
    }
    // Station Mac : une seule adresse, celle par laquelle le Mac répond (la
    // route principale : l'Ethernet avant le Wi-Fi). Un Mac branché deux fois
    // au même réseau répond depuis cette adresse-là ; une app qui en visait une
    // autre ne recevait pas l'image de Kyber (UDP, docs/station-macos.md).
    #[cfg(not(windows))]
    let single = crate::mac::is_station();
    #[cfg(windows)]
    let single = false;
    let make = |ip: Option<&str>| -> Option<ServiceInfo> {
        match ServiceInfo::new(SERVICE_TYPE, &name, &host, ip.unwrap_or(""), port, &properties[..]) {
            Ok(i) if ip.is_some() => Some(i),
            Ok(i) => Some(i.enable_addr_auto()),
            Err(e) => {
                tracing::warn!("annonce Bonjour impossible : {e}");
                None
            }
        }
    };
    let address = if single { crate::tls::lan_ip() } else { None };
    let info = make(address.as_deref())?;
    match daemon.register(info) {
        Ok(()) => {
            tracing::info!(
                "annoncé sur le réseau local (Bonjour) : {name}, port {port}, kyber={}{}",
                u8::from(kyber),
                address.as_deref().map(|a| format!(", adresse {a}")).unwrap_or_default()
            );
        }
        Err(e) => {
            tracing::warn!("annonce Bonjour impossible : {e}");
            return None;
        }
    }
    if single {
        // L'adresse suit la route principale (Ethernet débranché : le Wi-Fi).
        let (daemon, mut current) = (daemon.clone(), address);
        let make = move |ip: &str| ServiceInfo::new(SERVICE_TYPE, &name, &host, ip, port, &properties[..]).ok();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(15));
                let now = crate::tls::lan_ip();
                if now != current {
                    if let Some(info) = now.as_deref().and_then(&make)
                        && daemon.register(info).is_ok()
                    {
                        tracing::info!("annonce Bonjour : nouvelle adresse {}", now.as_deref().unwrap_or("?"));
                    }
                    current = now;
                }
            }
        });
    }
    Some(daemon)
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
