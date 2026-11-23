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
    /// Port public annoncé, si la box en publie un autre que le port
    /// d'écoute : liens (`/j/{code}`) et origines des Stations
    /// (`https://<id>.<domaine>`, sans port au 443).
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
    pub accounts: AccountsConfig,
}

/// Les comptes KyberNow (docs/comptes.md, « Réglages »), coupés par défaut.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountsConfig {
    pub enabled: bool,
    /// La base SQLite (dans un volume en écriture).
    pub database: PathBuf,
    /// Le fichier de la clé secrète (chiffrement des secrets TOTP, signature
    /// des cookies) : créé au premier démarrage s'il n'existe pas.
    pub secret_key: PathBuf,
    /// L'adresse publique des pages, pour les liens des e-mails
    /// (`https://<domaine>[:port public]` par défaut).
    pub public_url: Option<String>,
    /// L'envoi des e-mails ; sans lui, ni vérification ni mot de passe
    /// oublié, et l'administrateur du relais crée les comptes.
    pub smtp: Option<SmtpConfig>,
}

impl Default for AccountsConfig {
    fn default() -> Self {
        AccountsConfig {
            enabled: false,
            database: PathBuf::from("/data/accounts.db"),
            secret_key: PathBuf::from("/data/secret.key"),
            public_url: None,
            smtp: None,
        }
    }
}

/// Le serveur SMTP des e-mails des comptes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmtpConfig {
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    /// STARTTLS (port 587) ; `false` : TLS dès la connexion (port 465).
    #[serde(default = "default_true")]
    pub starttls: bool,
    pub user: Option<String>,
    /// Le mot de passe, dans un fichier à part (un secret Docker).
    pub password_file: Option<PathBuf>,
    /// L'expéditeur : `KyberNow <comptes@exemple.org>`.
    pub from: String,
}

fn default_smtp_port() -> u16 {
    587
}

fn default_true() -> bool {
    true
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
    /// Connexions de Station acceptées par minute et par adresse (une
    /// Station se reconnecte rarement ; sans cette limite, des clés jetables
    /// en rafale garderaient des codes 120 s chacune).
    pub stations_per_minute_per_ip: u32,
    pub clients_per_station: usize,
    /// Clients d'une même Station depuis une même adresse (un /64 en IPv6).
    pub clients_per_station_per_ip: usize,
    /// Un client d'une Station sans flux ni connexion relayée pendant ce
    /// temps est renvoyé : sa place se libère.
    pub client_idle_secs: u64,
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
            stations_per_minute_per_ip: 20,
            clients_per_station: 8,
            clients_per_station_per_ip: 2,
            client_idle_secs: 30,
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
            accounts: AccountsConfig::default(),
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
    /// (adresse, ou `off`), `KN_RELAY_EGRESS_ALERT_MBPS`, `KN_RELAY_GLOBAL_CAP_MBPS` ;
    /// les comptes : `KN_RELAY_ACCOUNTS` (`on` ou `off`), `KN_RELAY_ACCOUNTS_DATABASE`,
    /// `KN_RELAY_ACCOUNTS_SECRET_KEY`, `KN_RELAY_ACCOUNTS_PUBLIC_URL`, `KN_RELAY_SMTP_HOST`,
    /// `KN_RELAY_SMTP_PORT`, `KN_RELAY_SMTP_STARTTLS`, `KN_RELAY_SMTP_USER`,
    /// `KN_RELAY_SMTP_PASSWORD_FILE`, `KN_RELAY_SMTP_FROM`.
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
        let a = &mut self.accounts;
        if let Some(v) = get("KN_RELAY_ACCOUNTS") {
            a.enabled = parse_switch(&v).with_context(|| format!("KN_RELAY_ACCOUNTS : {v}"))?;
        }
        if let Some(v) = get("KN_RELAY_ACCOUNTS_DATABASE") {
            a.database = v.into();
        }
        if let Some(v) = get("KN_RELAY_ACCOUNTS_SECRET_KEY") {
            a.secret_key = v.into();
        }
        if let Some(v) = get("KN_RELAY_ACCOUNTS_PUBLIC_URL") {
            a.public_url = Some(v);
        }
        if let Some(host) = get("KN_RELAY_SMTP_HOST") {
            let smtp = a.smtp.get_or_insert_with(|| SmtpConfig {
                host: String::new(),
                port: default_smtp_port(),
                starttls: true,
                user: None,
                password_file: None,
                from: String::new(),
            });
            smtp.host = host;
        }
        if let Some(smtp) = a.smtp.as_mut() {
            if let Some(v) = get("KN_RELAY_SMTP_PORT") {
                smtp.port = v.parse().with_context(|| format!("KN_RELAY_SMTP_PORT : {v}"))?;
            }
            if let Some(v) = get("KN_RELAY_SMTP_STARTTLS") {
                smtp.starttls = parse_switch(&v).with_context(|| format!("KN_RELAY_SMTP_STARTTLS : {v}"))?;
            }
            if let Some(v) = get("KN_RELAY_SMTP_USER") {
                smtp.user = Some(v);
            }
            if let Some(v) = get("KN_RELAY_SMTP_PASSWORD_FILE") {
                smtp.password_file = Some(v.into());
            }
            if let Some(v) = get("KN_RELAY_SMTP_FROM") {
                smtp.from = v;
            }
        }
        Ok(())
    }

    fn check(&self) -> Result<()> {
        let ok = !self.domain.is_empty()
            && self.domain.split('.').all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        anyhow::ensure!(ok, "domaine invalide : {}", self.domain);
        anyhow::ensure!(self.public_port != Some(0), "port public invalide : 0");
        if let Some(url) = &self.accounts.public_url {
            anyhow::ensure!(url.starts_with("https://") && url.len() > 8 && !url.ends_with('/'), "[accounts] public_url : https://… sans / final ({url})");
        }
        if let Some(smtp) = &self.accounts.smtp {
            anyhow::ensure!(!smtp.host.is_empty(), "[accounts.smtp] host manque");
            anyhow::ensure!(smtp.from.contains('@'), "[accounts.smtp] from manque (KyberNow <comptes@exemple.org>)");
        }
        Ok(())
    }

    /// L'adresse publique des pages : `public_url`, ou `https://<domaine>[:port]`.
    pub fn public_url(&self, link_port: u16) -> String {
        match &self.accounts.public_url {
            Some(url) => url.clone(),
            None if link_port == 443 => format!("https://{}", self.domain),
            None => format!("https://{}:{link_port}", self.domain),
        }
    }

    /// Le port des liens.
    pub fn link_port(&self, bound: u16) -> u16 {
        self.public_port.unwrap_or(bound)
    }
}

/// `on`/`off`, `true`/`false`, `yes`/`no`, `1`/`0`.
fn parse_switch(v: &str) -> Result<bool> {
    match v.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Ok(true),
        "off" | "false" | "no" | "0" => Ok(false),
        _ => anyhow::bail!("on ou off attendu"),
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

    /// Le 443 dehors renvoyé vers le 8443 de la machine (le relais public) :
    /// les liens annoncent le 443 ; une variable vide (`RELAY_PUBLIC_PORT=`
    /// du compose) laisse le port d'écoute.
    #[test]
    fn port_public_annonce() {
        let mut cfg = Config::default();
        cfg.apply_env(|k| (k == "KN_RELAY_PUBLIC_PORT").then(|| "443".to_string())).unwrap();
        assert_eq!((cfg.listen.port(), cfg.link_port(cfg.listen.port())), (8443, 443));
        let mut cfg = Config::default();
        cfg.apply_env(|k| (k == "KN_RELAY_PUBLIC_PORT").then(String::new)).unwrap();
        assert_eq!(cfg.link_port(8443), 8443);
        let mut cfg = Config::default();
        assert!(cfg.apply_env(|k| (k == "KN_RELAY_PUBLIC_PORT").then(|| "0".to_string())).and_then(|_| cfg.check()).is_err());
    }

    #[test]
    fn comptes_coupes_par_defaut() {
        let cfg = Config::default();
        assert!(!cfg.accounts.enabled && cfg.accounts.smtp.is_none());
        assert_eq!(cfg.public_url(8443), "https://share.kybernow.app:8443");
        assert_eq!(cfg.public_url(443), "https://share.kybernow.app");
        let mut cfg: Config = toml::from_str(
            r#"
            [accounts]
            enabled = true
            database = "/data/a.db"
            public_url = "https://relais.exemple.org"
            [accounts.smtp]
            host = "smtp.exemple.org"
            user = "relais"
            password_file = "/run/secrets/smtp"
            from = "KyberNow <comptes@exemple.org>"
            "#,
        )
        .unwrap();
        let smtp = cfg.accounts.smtp.clone().unwrap();
        assert_eq!((smtp.port, smtp.starttls), (587, true));
        assert_eq!(cfg.public_url(8443), "https://relais.exemple.org");
        assert!(cfg.check().is_ok());
        cfg.apply_env(|k| match k {
            "KN_RELAY_ACCOUNTS" => Some("off".into()),
            "KN_RELAY_SMTP_PORT" => Some("465".into()),
            "KN_RELAY_SMTP_STARTTLS" => Some("no".into()),
            _ => None,
        })
        .unwrap();
        let smtp = cfg.accounts.smtp.clone().unwrap();
        assert!(!cfg.accounts.enabled);
        assert_eq!((smtp.port, smtp.starttls), (465, false));
        // Le SMTP par l'environnement seul.
        let mut cfg = Config::default();
        cfg.apply_env(|k| match k {
            "KN_RELAY_ACCOUNTS" => Some("on".into()),
            "KN_RELAY_SMTP_HOST" => Some("smtp.exemple.org".into()),
            _ => None,
        })
        .unwrap();
        assert!(cfg.accounts.enabled);
        assert!(cfg.check().is_err(), "l'expéditeur manque");
        assert!(toml::from_str::<Config>("[accounts]\npublic_url = \"http://x\"").unwrap().check().is_err());
        assert!(cfg.apply_env(|k| (k == "KN_RELAY_ACCOUNTS").then(|| "peut-être".into())).is_err());
    }
}
