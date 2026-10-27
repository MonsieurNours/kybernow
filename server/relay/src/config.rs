//! Réglages du relais : un fichier TOML facultatif (`/config/relay.toml`),
//! puis les variables d'environnement `KN_RELAY_*`, qui passent avant.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Le nom du relais ; les Stations sont servies sous `<id>.<domain>`.
    pub domain: String,
    /// Adresse d'écoute, en TCP et en UDP sur le même port.
    pub listen: SocketAddr,
    /// Port écrit dans les liens (`/j/{code}`), si la box le change.
    pub public_port: Option<u16>,
    /// Certificat (chaîne complète, PEM) et clé de `<domain>` et `*.<domain>`.
    pub cert: PathBuf,
    pub key: PathBuf,
    /// Liste de bannissement (une adresse ou un réseau par ligne), relue à chaud.
    pub banlist: Option<PathBuf>,
    /// Avertit dans le journal au-delà de ce débit sortant (0 : jamais).
    pub egress_alert_mbps: u64,
    /// Plafond global des paquets relayés (0 : aucun plafond).
    pub global_cap_mbps: u64,
    pub metrics: MetricsConfig,
    pub limits: Limits,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    /// Métriques Prometheus et `/healthz` (`None` : coupées).
    pub listen: Option<SocketAddr>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        MetricsConfig { listen: Some("127.0.0.1:9464".parse().expect("adresse")) }
    }
}

/// Les limites (docs/relais.md, « Limites et protections du relais »).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub lookups_per_minute: u32,
    pub lookups_per_hour: u32,
    pub lookups_per_minute_subnet: u32,
    pub lookups_per_minute_total: u32,
    pub quic_per_ip: usize,
    pub quic_handshakes_per_minute_per_ip: u32,
    pub quic_total: usize,
    /// Au-delà de ce nombre de négociations par seconde, preuve d'adresse (Retry).
    pub retry_above_per_second: u32,
    pub tcp_per_ip: usize,
    pub stations_per_ip: usize,
    pub clients_per_station: usize,
    pub flows_per_client: usize,
    pub codes_per_station: usize,
    pub header_timeout_secs: u64,
    pub max_header_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            lookups_per_minute: 10,
            lookups_per_hour: 60,
            lookups_per_minute_subnet: 30,
            lookups_per_minute_total: 600,
            quic_per_ip: 16,
            quic_handshakes_per_minute_per_ip: 60,
            quic_total: 4096,
            retry_above_per_second: 100,
            tcp_per_ip: 32,
            stations_per_ip: 4,
            clients_per_station: 8,
            flows_per_client: 4,
            codes_per_station: 4,
            header_timeout_secs: 10,
            max_header_bytes: 16 * 1024,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let domain = "share.kybernow.app".to_string();
        Config {
            cert: PathBuf::from(format!("/certs/certificates/{domain}.crt")),
            key: PathBuf::from(format!("/certs/certificates/{domain}.key")),
            domain,
            listen: "0.0.0.0:8443".parse().expect("adresse"),
            public_port: None,
            banlist: None,
            egress_alert_mbps: 0,
            global_cap_mbps: 0,
            metrics: MetricsConfig::default(),
            limits: Limits::default(),
        }
    }
}

impl Config {
    /// Le fichier (s'il existe), puis l'environnement.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let mut cfg = match path {
            Some(p) if p.exists() => {
                let text = std::fs::read_to_string(p).with_context(|| format!("lecture de {}", p.display()))?;
                toml::from_str(&text).with_context(|| format!("{} illisible", p.display()))?
            }
            _ => Config::default(),
        };
        cfg.apply_env(|k| std::env::var(k).ok())?;
        cfg.check()?;
        Ok(cfg)
    }

    /// `KN_RELAY_DOMAIN`, `KN_RELAY_LISTEN`, `KN_RELAY_PUBLIC_PORT`,
    /// `KN_RELAY_CERT`, `KN_RELAY_KEY`, `KN_RELAY_BANLIST`, `KN_RELAY_METRICS`
    /// (adresse, ou `off`), `KN_RELAY_EGRESS_ALERT_MBPS`, `KN_RELAY_GLOBAL_CAP_MBPS`.
    pub fn apply_env(&mut self, get: impl Fn(&str) -> Option<String>) -> Result<()> {
        let get = |k: &str| get(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if let Some(d) = get("KN_RELAY_DOMAIN") {
            let default_cert = Config { domain: self.domain.clone(), ..Config::default() };
            let follow = self.cert == Config::default().cert || self.cert == default_cert.cert;
            self.domain = d.to_ascii_lowercase();
            if follow {
                self.cert = PathBuf::from(format!("/certs/certificates/{}.crt", self.domain));
                self.key = PathBuf::from(format!("/certs/certificates/{}.key", self.domain));
            }
        }
        if let Some(v) = get("KN_RELAY_LISTEN") {
            self.listen = v.parse().with_context(|| format!("KN_RELAY_LISTEN : {v}"))?;
        }
        if let Some(v) = get("KN_RELAY_PUBLIC_PORT") {
            self.public_port = Some(v.parse().with_context(|| format!("KN_RELAY_PUBLIC_PORT : {v}"))?);
        }
        if let Some(v) = get("KN_RELAY_CERT") {
            self.cert = v.into();
        }
        if let Some(v) = get("KN_RELAY_KEY") {
            self.key = v.into();
        }
        if let Some(v) = get("KN_RELAY_BANLIST") {
            self.banlist = Some(v.into());
        }
        if let Some(v) = get("KN_RELAY_METRICS") {
            self.metrics.listen = if v == "off" { None } else { Some(v.parse().with_context(|| format!("KN_RELAY_METRICS : {v}"))?) };
        }
        if let Some(v) = get("KN_RELAY_EGRESS_ALERT_MBPS") {
            self.egress_alert_mbps = v.parse().context("KN_RELAY_EGRESS_ALERT_MBPS")?;
        }
        if let Some(v) = get("KN_RELAY_GLOBAL_CAP_MBPS") {
            self.global_cap_mbps = v.parse().context("KN_RELAY_GLOBAL_CAP_MBPS")?;
        }
        Ok(())
    }

    fn check(&self) -> Result<()> {
        let ok = !self.domain.is_empty()
            && self.domain.split('.').all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        anyhow::ensure!(ok, "domaine invalide : {}", self.domain);
        Ok(())
    }

    /// Le port des liens.
    pub fn link_port(&self, bound: u16) -> u16 {
        self.public_port.unwrap_or(bound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fichier_puis_environnement() {
        let mut cfg: Config = toml::from_str(
            r#"
            domain = "relais.exemple.org"
            [limits]
            clients_per_station = 3
            "#,
        )
        .unwrap();
        assert_eq!(cfg.limits.clients_per_station, 3);
        assert_eq!(cfg.limits.flows_per_client, 4);
        cfg.apply_env(|k| match k {
            "KN_RELAY_LISTEN" => Some("127.0.0.1:9443".into()),
            "KN_RELAY_METRICS" => Some("off".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(cfg.listen.port(), 9443);
        assert!(cfg.metrics.listen.is_none());
        assert!(toml::from_str::<Config>("inconnu = 1").is_err());
    }

    #[test]
    fn le_certificat_suit_le_domaine() {
        let mut cfg = Config::default();
        cfg.apply_env(|k| (k == "KN_RELAY_DOMAIN").then(|| "Relais.Exemple.org".to_string())).unwrap();
        assert_eq!(cfg.domain, "relais.exemple.org");
        assert_eq!(cfg.cert, PathBuf::from("/certs/certificates/relais.exemple.org.crt"));
        assert!(cfg.check().is_ok());
        assert_eq!(cfg.link_port(8443), 8443);
    }
}
