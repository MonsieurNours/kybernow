//! KyberNow Station pour Mac (docs/station-macos.md).
//!
//! Sur un Mac, Admin est soit une vraie Station (Kyber installé en
//! LaunchAgent : `KYBERNOW_KYBER_PLIST`, sinon
//! `/Library/LaunchAgents/fr.leslignesclaires.kybernow.station.kyber.plist`),
//! soit le Mac de développement, où tout reste simulé (tests, interface).
//! [`is_station`] le dit, une fois pour toutes au démarrage.
//!
//! Sur une vraie Station, Kyber est piloté par launchd ([`Launchd`]) dans la
//! session de l'utilisateur devant l'écran (domaine `gui/<uid>`), et il n'y a
//! pas encore d'écran virtuel ni de sortie son virtuelle (jalons 3 et 4).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Service launchd du serveur Kyber (`server/macos/scripts/run-station.sh`).
pub const KYBER_LABEL: &str = "fr.leslignesclaires.kybernow.station.kyber";
/// Chemin du plist du LaunchAgent de Kyber, s'il n'est pas à sa place habituelle.
pub const PLIST_ENV: &str = "KYBERNOW_KYBER_PLIST";

static STATION: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Plist du LaunchAgent de Kyber, si ce Mac est une Station.
pub fn kyber_plist() -> Option<&'static Path> {
    STATION
        .get_or_init(|| {
            if !cfg!(target_os = "macos") {
                return None;
            }
            let path = std::env::var_os(PLIST_ENV)
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(format!("/Library/LaunchAgents/{KYBER_LABEL}.plist")));
            path.is_file().then_some(path)
        })
        .as_deref()
}

/// Ce Mac est une Station (Kyber installé), pas un Mac de développement.
pub fn is_station() -> bool {
    kyber_plist().is_some()
}

/// Adresse de ce Mac à donner à l'appareil `peer` pour l'image (`kyber_host`
/// de `prepare`) : celle de la route principale (`tls::lan_ip`, l'Ethernet
/// avant le Wi-Fi), si `peer` est sur son réseau et l'appelle par une autre
/// adresse. Un Mac branché en Ethernet et en Wi-Fi au même réseau garde sinon
/// l'image sur l'adresse que l'appareil a retenue à l'appairage, souvent le
/// Wi-Fi. Hors Station Mac, ou appareil d'un autre réseau (relais, VPN,
/// Internet) : rien, l'adresse d'Admin reste celle de l'image.
pub fn preferred_host(peer: std::net::IpAddr, called: Option<std::net::IpAddr>) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if !is_station() {
            return None;
        }
        let lan: std::net::Ipv4Addr = crate::tls::lan_ip()?.parse().ok()?;
        let ifconfig = std::process::Command::new("/sbin/ifconfig").output().ok()?;
        preferred_among(&String::from_utf8_lossy(&ifconfig.stdout), lan, peer, called)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (peer, called);
        None
    }
}

/// `lan` si `peer` est sur son sous-réseau (masque lu dans `ifconfig`) et
/// n'appelle pas déjà cette adresse.
fn preferred_among(
    ifconfig: &str,
    lan: std::net::Ipv4Addr,
    peer: std::net::IpAddr,
    called: Option<std::net::IpAddr>,
) -> Option<String> {
    let std::net::IpAddr::V4(peer) = peer.to_canonical() else { return None };
    if called.map(|c| c.to_canonical()) == Some(std::net::IpAddr::V4(lan)) || peer == lan {
        return None;
    }
    let mask = ifconfig.lines().find_map(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        (words.first() == Some(&"inet") && words.get(1) == Some(&lan.to_string().as_str()))
            .then(|| words.iter().position(|w| *w == "netmask").and_then(|i| words.get(i + 1)))
            .flatten()
            .and_then(|m| u32::from_str_radix(m.trim_start_matches("0x"), 16).ok())
    })?;
    (mask != 0 && u32::from(peer) & mask == u32::from(lan) & mask).then(|| lan.to_string())
}

#[cfg(target_os = "macos")]
pub use launchd::Launchd;

#[cfg(target_os = "macos")]
mod launchd {
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::process::Command;

    use anyhow::{Context, Result};

    use super::KYBER_LABEL;
    use crate::service::{ServiceControl, ServiceState};

    /// Kyber en LaunchAgent, dans la session de l'utilisateur devant l'écran.
    pub struct Launchd {
        pub plist: PathBuf,
    }

    /// Compte de la session affichée à l'écran (propriétaire de /dev/console).
    fn console_uid() -> Result<u32> {
        Ok(std::fs::metadata("/dev/console").context("/dev/console")?.uid())
    }

    fn domain() -> Result<String> {
        Ok(format!("gui/{}", console_uid()?))
    }

    fn launchctl(args: &[&str]) -> Result<std::process::Output> {
        Command::new("/bin/launchctl").args(args).output().context("launchctl")
    }

    /// `state = …` de `launchctl print`, lu tel quel.
    pub(super) fn parse_state(print: &str) -> ServiceState {
        let state = print.lines().map(str::trim).find_map(|l| l.strip_prefix("state = ")).unwrap_or("");
        match state {
            "running" => ServiceState::Running,
            "not running" | "exited" => ServiceState::Stopped,
            "spawn scheduled" | "xpcproxy" => ServiceState::Starting,
            _ => ServiceState::Unknown,
        }
    }

    impl ServiceControl for Launchd {
        fn state(&self) -> Result<ServiceState> {
            let out = launchctl(&["print", &format!("{}/{KYBER_LABEL}", domain()?)])?;
            if !out.status.success() {
                // Pas chargé : arrêté (le plist est là) ou absent.
                return Ok(if self.plist.is_file() { ServiceState::Stopped } else { ServiceState::NotInstalled });
            }
            Ok(parse_state(&String::from_utf8_lossy(&out.stdout)))
        }

        fn start(&self) -> Result<()> {
            let domain = domain()?;
            let target = format!("{domain}/{KYBER_LABEL}");
            // Chargé : on le lance ; sinon on le charge (RunAtLoad le lance).
            let out = if launchctl(&["print", &target])?.status.success() {
                launchctl(&["kickstart", &target])?
            } else {
                launchctl(&["bootstrap", &domain, &self.plist.to_string_lossy()])?
            };
            anyhow::ensure!(out.status.success(), "launchctl : {}", String::from_utf8_lossy(&out.stderr).trim());
            Ok(())
        }

        fn stop(&self) -> Result<()> {
            let target = format!("{}/{KYBER_LABEL}", domain()?);
            if !launchctl(&["print", &target])?.status.success() {
                return Ok(());
            }
            let out = launchctl(&["bootout", &target])?;
            anyhow::ensure!(out.status.success(), "launchctl : {}", String::from_utf8_lossy(&out.stderr).trim());
            Ok(())
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::launchd::parse_state;
    use crate::service::ServiceState;

    #[test]
    fn reads_the_launchctl_state() {
        let print = "gui/501/x = {\n\tactive count = 1\n\tstate = running\n\tpid = 93324\n}";
        assert_eq!(parse_state(print), ServiceState::Running);
        assert_eq!(parse_state("\tstate = not running\n"), ServiceState::Stopped);
        assert_eq!(parse_state("\tstate = spawn scheduled\n"), ServiceState::Starting);
        assert_eq!(parse_state(""), ServiceState::Unknown);
    }
}

#[cfg(test)]
mod preferred_tests {
    use super::preferred_among;

    const IFCONFIG: &str = "en0: flags=8863<UP>\n\tinet 192.168.1.49 netmask 0xffffff00 broadcast 192.168.1.255\n\
en8: flags=8863<UP>\n\tinet 192.168.1.105 netmask 0xffffff00 broadcast 192.168.1.255\n";

    #[test]
    fn ethernet_for_a_device_of_the_same_network() {
        let lan = "192.168.1.105".parse().unwrap();
        let pc = "192.168.1.100".parse().unwrap();
        let wifi = Some("192.168.1.49".parse().unwrap());
        assert_eq!(preferred_among(IFCONFIG, lan, pc, wifi).as_deref(), Some("192.168.1.105"));
        // Adresse IPv4 mappée (écoute double pile).
        let mapped = "::ffff:192.168.1.100".parse().unwrap();
        assert_eq!(preferred_among(IFCONFIG, lan, mapped, wifi).as_deref(), Some("192.168.1.105"));
        // Déjà appelée par l'Ethernet : rien à changer.
        assert_eq!(preferred_among(IFCONFIG, lan, pc, Some("192.168.1.105".parse().unwrap())), None);
        // Autre réseau (VPN, Internet, relais local) : rien.
        assert_eq!(preferred_among(IFCONFIG, lan, "100.64.0.3".parse().unwrap(), wifi), None);
        assert_eq!(preferred_among(IFCONFIG, lan, "127.0.0.1".parse().unwrap(), wifi), None);
        assert_eq!(preferred_among(IFCONFIG, lan, "fe80::1".parse().unwrap(), wifi), None);
        // Adresse principale introuvable dans ifconfig : rien.
        assert_eq!(preferred_among(IFCONFIG, "10.0.0.2".parse().unwrap(), pc, wifi), None);
    }
}
