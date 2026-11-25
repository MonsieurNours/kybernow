//! KyberNow Station pour Linux (docs/station-linux.md).
//!
//! Sous Linux, Admin est soit une vraie Station (le service systemd de
//! l'utilisateur `kybernow-kyber.service` est installé : paquet .deb ou .rpm,
//! ou `server/linux/scripts/run-station.sh`), soit une machine de
//! développement, où tout reste simulé. [`is_station`] le dit, une fois pour
//! toutes au démarrage.
//!
//! Comme sur Mac, tout tourne dans la session de l'utilisateur devant
//! l'écran (services systemd `--user`) : c'est là que vivent le compositeur
//! (GNOME), PipeWire et le bus de la session. Kyber y est piloté par
//! `systemctl --user` ([`Systemd`]).

use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Context, Result};

use crate::service::{ServiceControl, ServiceState};

/// Service systemd (de l'utilisateur) du serveur Kyber.
pub const KYBER_UNIT: &str = "kybernow-kyber.service";
/// Service systemd (de l'utilisateur) d'Admin.
pub const ADMIN_UNIT: &str = "kybernow-admin.service";
/// Nom du service de Kyber, s'il n'est pas `kybernow-kyber.service`
/// (Station d'essai : `run-station.sh` le pose sous un autre nom).
pub const UNIT_ENV: &str = "KYBERNOW_KYBER_UNIT";

static STATION: OnceLock<Option<String>> = OnceLock::new();

/// Le service de Kyber, si cette machine est une Station.
pub fn kyber_unit() -> Option<&'static str> {
    STATION
        .get_or_init(|| {
            if !cfg!(target_os = "linux") || cfg!(test) {
                return None;
            }
            let unit = std::env::var(UNIT_ENV).ok().filter(|u| !u.is_empty()).unwrap_or_else(|| KYBER_UNIT.into());
            // Installé par le paquet (/usr/lib/systemd/user) ou par
            // l'utilisateur (~/.config/systemd/user) : systemd le connaît.
            let out = Command::new("systemctl").args(["--user", "cat", &unit]).output().ok()?;
            out.status.success().then_some(unit)
        })
        .as_deref()
}

/// Cette machine est une Station Linux (Kyber installé), pas une machine de
/// développement.
pub fn is_station() -> bool {
    kyber_unit().is_some()
}

fn systemctl(args: &[&str]) -> Result<std::process::Output> {
    Command::new("systemctl").arg("--user").args(args).output().context("systemctl --user")
}

/// `ActiveState` de `systemctl show`, lu tel quel.
pub(crate) fn parse_state(active: &str, load: &str) -> ServiceState {
    match (active.trim(), load.trim()) {
        (_, "not-found") => ServiceState::NotInstalled,
        ("active", _) | ("reloading", _) => ServiceState::Running,
        ("inactive", _) | ("failed", _) => ServiceState::Stopped,
        ("activating", _) => ServiceState::Starting,
        ("deactivating", _) => ServiceState::Stopping,
        _ => ServiceState::Unknown,
    }
}

/// Kyber en service systemd de l'utilisateur.
pub struct Systemd {
    pub unit: String,
}

impl ServiceControl for Systemd {
    fn state(&self) -> Result<ServiceState> {
        let out = systemctl(&["show", "--property=ActiveState,LoadState", "--value", &self.unit])?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let (load, active) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
        // `--value` garde l'ordre des propriétés de systemd (LoadState d'abord).
        Ok(parse_state(active, load))
    }

    fn start(&self) -> Result<()> {
        let out = systemctl(&["start", "--no-block", &self.unit])?;
        anyhow::ensure!(out.status.success(), "systemctl : {}", String::from_utf8_lossy(&out.stderr).trim());
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let out = systemctl(&["stop", &self.unit])?;
        anyhow::ensure!(out.status.success(), "systemctl : {}", String::from_utf8_lossy(&out.stderr).trim());
        Ok(())
    }
}

/// Redémarre Kyber puis Admin (lui-même) par systemd, après un délai : la
/// réponse à l'appareil part d'abord.
pub fn restart_station(grace: std::time::Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(grace);
        let kyber = kyber_unit().unwrap_or(KYBER_UNIT).to_string();
        let admin = std::env::var("KYBERNOW_ADMIN_UNIT").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| ADMIN_UNIT.into());
        for unit in [kyber, admin] {
            tracing::info!("systemctl --user restart {unit}");
            let _ = systemctl(&["restart", "--no-block", &unit]);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::parse_state;
    use crate::service::ServiceState;

    #[test]
    fn reads_the_systemd_state() {
        assert_eq!(parse_state("active", "loaded"), ServiceState::Running);
        assert_eq!(parse_state("inactive", "loaded"), ServiceState::Stopped);
        assert_eq!(parse_state("failed", "loaded"), ServiceState::Stopped);
        assert_eq!(parse_state("activating", "loaded"), ServiceState::Starting);
        assert_eq!(parse_state("deactivating", "loaded"), ServiceState::Stopping);
        assert_eq!(parse_state("inactive", "not-found"), ServiceState::NotInstalled);
        assert_eq!(parse_state("", ""), ServiceState::Unknown);
    }
}
