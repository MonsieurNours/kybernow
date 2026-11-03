//! Les messages du tunnel (docs/relais.md, « Trames et messages ») : un objet
//! JSON par trame, dont le champ `t` donne le type. Un type inconnu devient
//! [`Msg::Unknown`] et un champ inconnu est ignoré : on ajoute sans casser.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Qui se présente au relais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Une Station (Admin), reconnue à son certificat client.
    Station,
    /// Une app (ou une connexion qui ne sert qu'à `status`).
    Client,
}

/// Ce qu'une app demande en ouvrant un flux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Service {
    /// Admin, en TLS de bout en bout (le tunnel ne voit que des octets).
    AdminTls,
    /// Kyber en TCP : contrôle HTTPS de kycontroller, WebSocket.
    KyberTcp,
    /// Une console PlayStation du réseau de la Station, en TCP (session,
    /// canal de contrôle, association) : `to`, `host_id` et `port` disent
    /// laquelle et où (docs/relais.md, « Consoles PlayStation »). Un relais
    /// d'avant les consoles ne connaît pas ce service et coupe le flux.
    ConsoleTcp,
}

impl Service {
    pub fn kind(self) -> StreamKind {
        match self {
            Service::AdminTls => StreamKind::AdminTls,
            Service::KyberTcp => StreamKind::KyberTcp,
            Service::ConsoleTcp => StreamKind::ConsoleTcp,
        }
    }
}

/// Vers quoi va un flux UDP ou une connexion TCP relayée (`to`) : Kyber
/// (ou absent, comme avant les consoles) ou une console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum To {
    Kyber,
    Console,
}

/// Une console PlayStation vue par la Station : son identifiant (`host-id`)
/// et le port voulu. La Station ne joint que l'adresse qu'elle a trouvée
/// elle-même pour cet identifiant, jamais une adresse donnée par l'app.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConsoleTarget {
    pub host_id: String,
    pub port: u16,
}

impl ConsoleTarget {
    pub fn new(host_id: impl Into<String>, port: u16) -> Self {
        ConsoleTarget { host_id: host_id.into(), port }
    }

    /// Les champs `to`, `host_id`, `port` d'un message.
    pub fn fields(target: Option<&ConsoleTarget>) -> (Option<To>, Option<String>, Option<u16>) {
        match target {
            Some(t) => (Some(To::Console), Some(t.host_id.clone()), Some(t.port)),
            None => (None, None, None),
        }
    }

    /// La console d'un message (`to: "console"` avec `host_id` et `port`) ;
    /// `Err` pour un `to: "console"` incomplet (à refuser, jamais à prendre pour Kyber).
    #[allow(clippy::result_unit_err)]
    pub fn from_fields(to: Option<To>, host_id: &Option<String>, port: Option<u16>) -> Result<Option<ConsoleTarget>, ()> {
        match (to, host_id, port) {
            (None | Some(To::Kyber), _, _) => Ok(None),
            (Some(To::Console), Some(h), Some(p)) if !h.is_empty() && p != 0 => Ok(Some(ConsoleTarget { host_id: h.clone(), port: p })),
            (Some(To::Console), _, _) => Err(()),
        }
    }
}

/// Ce que la Station reçoit dans l'en-tête d'un flux (écrit par le relais).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    /// Une app vers Admin, TLS de bout en bout.
    AdminTls,
    /// Un navigateur vers Admin, déchiffré par le relais : HTTP/1.1 en clair.
    AdminHttp,
    /// kyclient vers Kyber, en TCP.
    KyberTcp,
    /// Une app vers une console PlayStation du réseau de la Station, en TCP.
    ConsoleTcp,
}

impl StreamKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamKind::AdminTls => "admin_tls",
            StreamKind::AdminHttp => "admin_http",
            StreamKind::KyberTcp => "kyber_tcp",
            StreamKind::ConsoleTcp => "console_tcp",
        }
    }
}

/// Codes d'erreur du relais (`error.code`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Version,
    BadRequest,
    UnknownStation,
    TooMany,
    Banned,
    Replaced,
    NoCode,
    NotBound,
    #[serde(other)]
    Other,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Version => "version",
            ErrorCode::BadRequest => "bad_request",
            ErrorCode::UnknownStation => "unknown_station",
            ErrorCode::TooMany => "too_many",
            ErrorCode::Banned => "banned",
            ErrorCode::Replaced => "replaced",
            ErrorCode::NoCode => "no_code",
            ErrorCode::NotBound => "not_bound",
            ErrorCode::Other => "other",
        }
    }
}

/// `hello` : le premier message d'un bout, sur le flux des messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub v: u32,
    pub role: Role,
    /// App : la Station voulue (absente pour une connexion qui ne sert qu'à `status`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    /// App : le jeton du tunnel, d'où le relais tire l'identifiant du client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Station : l'empreinte d'Admin (`sha256:aa:bb:…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_fp: Option<String>,
    /// Station : signature Ed25519 de `admin_fp` (voir [`crate::lookup::sig_message`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    /// Station : les codes à garder, après une reconnexion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codes: Vec<String>,
    /// Version du logiciel (Admin, app), pour les journaux du relais.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// `welcome` : la réponse du relais à `hello`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub v: u32,
    /// L'identifiant de la Station, ou celui du client.
    pub id: String,
    /// La version du relais.
    #[serde(default)]
    pub relay: String,
    /// App : la Station est-elle connectée au relais.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online: Option<bool>,
    /// Station : les codes de `hello` que le relais a gardés.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codes: Vec<String>,
}

/// `error` : un refus du relais, réponse à une demande (`req`) ou non.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorMsg {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub req: Option<u32>,
    pub code: ErrorCode,
    /// Pour les journaux, jamais affiché tel quel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// L'en-tête d'un flux vers la Station : écrit par le relais seul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamHeader {
    pub v: u32,
    pub kind: StreamKind,
    /// L'identifiant du client (app ou connexion du navigateur).
    pub client: String,
    /// Son adresse publique, vue par le relais.
    pub ip: IpAddr,
    /// Navigateur : le nom demandé (`<id>.share.kybernow.app`).
    #[serde(default)]
    pub host: Option<String>,
    /// `console_tcp` : `"console"`, la console (`host_id`) et son port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<To>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

impl StreamHeader {
    /// L'en-tête d'un flux vers Admin ou Kyber (sans console).
    pub fn new(kind: StreamKind, client: impl Into<String>, ip: IpAddr, host: Option<String>) -> Self {
        StreamHeader { v: 1, kind, client: client.into(), ip, host, to: None, host_id: None, port: None }
    }

    /// L'en-tête d'un flux vers une console (`console_tcp`).
    pub fn console_tcp(client: impl Into<String>, ip: IpAddr, target: &ConsoleTarget) -> Self {
        let (to, host_id, port) = ConsoleTarget::fields(Some(target));
        StreamHeader { to, host_id, port, ..StreamHeader::new(StreamKind::ConsoleTcp, client, ip, None) }
    }

    /// La console d'un flux `console_tcp` ; `None` sinon, ou incomplète.
    pub fn console(&self) -> Option<ConsoleTarget> {
        ConsoleTarget::from_fields(self.to, &self.host_id, self.port).ok().flatten()
    }
}

/// Tous les messages, sur le flux des messages et en tête des autres flux.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Msg {
    Hello(Hello),
    Welcome(Welcome),
    Error(ErrorMsg),
    Reserve {
        req: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preferred: Option<String>,
    },
    Reserved {
        req: u32,
        code: String,
    },
    Release {
        code: String,
    },
    OpenFlow {
        req: u32,
        key: u32,
        /// Vers une console (`"console"`, avec `host_id` et `port`) ; absent : Kyber.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<To>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
    FlowOpen {
        flow: u64,
        client: String,
        key: u32,
        ip: IpAddr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<To>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
    FlowAccept {
        flow: u64,
    },
    FlowRefuse {
        flow: u64,
        #[serde(default)]
        reason: String,
    },
    FlowReady {
        req: u32,
        flow: u64,
    },
    FlowRefused {
        req: u32,
        #[serde(default)]
        reason: String,
    },
    FlowClose {
        flow: u64,
    },
    ClientGone {
        client: String,
    },
    Station {
        online: bool,
    },
    Stats {
        #[serde(default)]
        rtt_station_ms: Option<u32>,
    },
    Status {
        req: u32,
        stations: Vec<String>,
    },
    Presence {
        req: u32,
        online: Vec<bool>,
    },
    Ping {
        n: u64,
    },
    Pong {
        n: u64,
    },
    /// En tête d'un flux ouvert par une app : le service voulu ; pour
    /// `console_tcp`, la console (`to`, `host_id`, `port`).
    Open {
        service: Service,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to: Option<To>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        host_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
    /// En tête d'un flux ouvert par le relais vers la Station.
    Stream(StreamHeader),
    /// En tête de la connexion des paquets (`kn-media/1`, ou deuxième flux du repli).
    Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Un type que cette version ne connaît pas : ignoré.
    #[serde(other)]
    Unknown,
}

impl Msg {
    pub fn error(req: Option<u32>, code: ErrorCode, message: impl Into<Option<String>>) -> Msg {
        Msg::Error(ErrorMsg { req, code, message: message.into() })
    }

    /// `open` vers Admin ou Kyber.
    pub fn open(service: Service) -> Msg {
        Msg::Open { service, to: None, host_id: None, port: None }
    }

    /// `open` vers une console (`console_tcp`).
    pub fn open_console(target: &ConsoleTarget) -> Msg {
        let (to, host_id, port) = ConsoleTarget::fields(Some(target));
        Msg::Open { service: Service::ConsoleTcp, to, host_id, port }
    }

    /// `open_flow` vers Kyber (`None`) ou une console.
    pub fn open_flow(req: u32, key: u32, target: Option<&ConsoleTarget>) -> Msg {
        let (to, host_id, port) = ConsoleTarget::fields(target);
        Msg::OpenFlow { req, key, to, host_id, port }
    }

    /// Le type, pour les journaux.
    pub fn kind(&self) -> &'static str {
        match self {
            Msg::Hello(_) => "hello",
            Msg::Welcome(_) => "welcome",
            Msg::Error(_) => "error",
            Msg::Reserve { .. } => "reserve",
            Msg::Reserved { .. } => "reserved",
            Msg::Release { .. } => "release",
            Msg::OpenFlow { .. } => "open_flow",
            Msg::FlowOpen { .. } => "flow_open",
            Msg::FlowAccept { .. } => "flow_accept",
            Msg::FlowRefuse { .. } => "flow_refuse",
            Msg::FlowReady { .. } => "flow_ready",
            Msg::FlowRefused { .. } => "flow_refused",
            Msg::FlowClose { .. } => "flow_close",
            Msg::ClientGone { .. } => "client_gone",
            Msg::Station { .. } => "station",
            Msg::Stats { .. } => "stats",
            Msg::Status { .. } => "status",
            Msg::Presence { .. } => "presence",
            Msg::Ping { .. } => "ping",
            Msg::Pong { .. } => "pong",
            Msg::Open { .. } => "open",
            Msg::Stream(_) => "stream",
            Msg::Media { .. } => "media",
            Msg::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_du_contrat() {
        let m = Msg::open_flow(3, 1, None);
        assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"t":"open_flow","req":3,"key":1}"#);
        let h = Msg::Stream(StreamHeader::new(StreamKind::AdminTls, "abcdefghijklmnop", "203.0.113.5".parse().unwrap(), None));
        assert_eq!(
            serde_json::to_string(&h).unwrap(),
            r#"{"t":"stream","v":1,"kind":"admin_tls","client":"abcdefghijklmnop","ip":"203.0.113.5","host":null}"#
        );
        let o = Msg::open(Service::KyberTcp);
        assert_eq!(serde_json::to_string(&o).unwrap(), r#"{"t":"open","service":"kyber_tcp"}"#);
        let e = Msg::error(Some(2), ErrorCode::TooMany, None);
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"t":"error","req":2,"code":"too_many"}"#);
    }

    #[test]
    fn flux_vers_une_console() {
        let c = ConsoleTarget::new("C0151B2DAE08", 9296);
        let m = Msg::open_flow(4, 2, Some(&c));
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"t":"open_flow","req":4,"key":2,"to":"console","host_id":"C0151B2DAE08","port":9296}"#
        );
        let o = Msg::open_console(&ConsoleTarget::new("C0151B2DAE08", 9295));
        assert_eq!(
            serde_json::to_string(&o).unwrap(),
            r#"{"t":"open","service":"console_tcp","to":"console","host_id":"C0151B2DAE08","port":9295}"#
        );
        // `to: "console"` incomplet : refusé, jamais pris pour Kyber.
        assert_eq!(ConsoleTarget::from_fields(Some(To::Console), &None, Some(9296)), Err(()));
        assert_eq!(ConsoleTarget::from_fields(Some(To::Console), &Some("A".into()), Some(0)), Err(()));
        assert_eq!(ConsoleTarget::from_fields(None, &Some("A".into()), Some(9296)), Ok(None));
        assert_eq!(ConsoleTarget::from_fields(Some(To::Console), &Some("A".into()), Some(9296)), Ok(Some(ConsoleTarget::new("A", 9296))));
        // Un en-tête de flux vers une console, lu par la Station.
        let h: Msg = serde_json::from_str(
            r#"{"t":"stream","v":1,"kind":"console_tcp","client":"c","ip":"203.0.113.5","host":null,"to":"console","host_id":"A","port":9295}"#,
        )
        .unwrap();
        let Msg::Stream(h) = h else { panic!() };
        assert_eq!(h.console(), Some(ConsoleTarget::new("A", 9295)));
        assert_eq!(StreamHeader::console_tcp("c", "203.0.113.5".parse().unwrap(), &ConsoleTarget::new("A", 9295)), h);
        // Une app d'avant les consoles ouvre Kyber, sans champ de plus.
        let old: Msg = serde_json::from_str(r#"{"t":"open_flow","req":1,"key":1}"#).unwrap();
        assert!(matches!(old, Msg::OpenFlow { to: None, host_id: None, port: None, .. }));
    }

    #[test]
    fn type_et_champs_inconnus_ignores() {
        let m: Msg = serde_json::from_str(r#"{"t":"futur","x":1}"#).unwrap();
        assert_eq!(m, Msg::Unknown);
        let m: Msg = serde_json::from_str(r#"{"t":"ping","n":4,"plus":"tard"}"#).unwrap();
        assert_eq!(m, Msg::Ping { n: 4 });
        let e: Msg = serde_json::from_str(r#"{"t":"error","code":"nouveau"}"#).unwrap();
        assert!(matches!(e, Msg::Error(ErrorMsg { code: ErrorCode::Other, .. })));
    }

    #[test]
    fn hello_de_la_station_et_de_l_app() {
        let h: Msg = serde_json::from_str(
            r#"{"t":"hello","v":1,"role":"station","admin_fp":"sha256:aa","sig":"c2ln","codes":["123456"],"version":"0.8.0"}"#,
        )
        .unwrap();
        let Msg::Hello(h) = h else { panic!() };
        assert_eq!(h.role, Role::Station);
        assert_eq!(h.codes, vec!["123456".to_string()]);
        let c = Msg::Hello(Hello {
            v: 1,
            role: Role::Client,
            station: Some("s".into()),
            token: Some("t".into()),
            admin_fp: None,
            sig: None,
            codes: vec![],
            version: None,
        });
        assert_eq!(serde_json::to_string(&c).unwrap(), r#"{"t":"hello","v":1,"role":"client","station":"s","token":"t"}"#);
    }
}
