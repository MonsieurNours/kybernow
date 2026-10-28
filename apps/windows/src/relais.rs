//! Le relais de KyberNow côté app Windows, ce qui ne dépend de rien de Windows
//! (docs/relais.md, textes de docs/link-ui.md, « Hors de chez vous ») pour se
//! tester sur le Mac : la route d'un ordinateur par son relais, les relais à
//! interroger pour un code tapé, la présence au relais (accueil, « Hors de chez
//! vous ») et son rythme, le chemin d'une session tel que la barre et les
//! statistiques le disent, et les textes des refus avec le nom de l'ordinateur
//! (choisis d'après l'erreur typée du cœur, jamais d'après son texte).
//! Les appels, les fils et les fenêtres sont dans `ui.rs`, `ui/share.rs` et `host.rs`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use kybernow_core::i18n::Lang;
use kybernow_core::relay::{self, RelayAddr, RelayError, RelayInfo, RelayMode, RelayRoute, RouteInfo, RouteTransport};
use serde::Serialize;

/// La route d'un ordinateur par son relais (`info.relay` gardé avec lui, et
/// l'empreinte d'Admin que l'app connaît) : `None` sans relais réglé chez lui,
/// ou sans empreinte (jamais de premier contact par le relais).
pub fn route_of(relay: Option<&RelayInfo>, fingerprint: Option<&str>) -> Option<RelayRoute> {
    relay?.route(fingerprint?)
}

/// Les relais où chercher le code d'un partage en plus du relais public : ceux
/// des ordinateurs que l'app connaît (`info.relay.server`), sans doublon ; une
/// adresse illisible est écartée. Le cœur (`share::join_anywhere`) interroge
/// toujours le relais public lui-même : le code d'un ami y part même quand
/// l'ordinateur de l'utilisateur en utilise un autre.
pub fn join_relays<'a>(servers: impl IntoIterator<Item = &'a str>) -> Vec<RelayAddr> {
    let mut out = Vec::new();
    for server in servers {
        if let Ok(addr) = server.trim().parse::<RelayAddr>()
            && !out.contains(&addr)
        {
            out.push(addr);
        }
    }
    out
}

/// `KYBERNOW_RELAY=off` : le relais ne sert à rien (ni l'accueil, ni « Rejoindre »).
pub fn off() -> bool {
    RelayMode::current() == RelayMode::Off
}

// MARK: - Présence

/// La présence au relais se redemande toutes les 30 s au plus par ordinateur
/// (chaque appel ouvre une connexion brève).
pub const PRESENCE_EVERY: Duration = Duration::from_secs(30);

/// Ce que l'app sait de la présence de chaque ordinateur au relais, et quand
/// elle l'a demandée.
#[derive(Debug, Default)]
pub struct Presence {
    seen: HashMap<String, (Instant, Option<bool>)>,
}

/// La réponse du cache : à redemander au relais, ou la dernière connue
/// (`None` : le relais n'a pas répondu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    Due,
    Known(Option<bool>),
}

impl Presence {
    /// `station` : l'identifiant de l'ordinateur chez le relais.
    pub fn ask(&self, station: &str, now: Instant) -> Ask {
        match self.seen.get(station) {
            Some((at, present)) if now.saturating_duration_since(*at) < PRESENCE_EVERY => Ask::Known(*present),
            _ => Ask::Due,
        }
    }

    pub fn record(&mut self, station: &str, now: Instant, present: Option<bool>) {
        self.seen.insert(station.to_string(), (now, present));
    }

    /// Un ordinateur retiré de l'app, ou sans relais : rien à garder.
    pub fn forget(&mut self, station: &str) {
        self.seen.remove(station);
    }
}

/// L'ordinateur est-il joignable par le relais ? Bloquant. `None` : le relais
/// ne répond pas (l'ordinateur reste « Hors ligne »).
pub fn present(route: &RelayRoute) -> Option<bool> {
    let addr = route.relay_addr().ok()?;
    match relay::presence(&addr, std::slice::from_ref(&route.station)) {
        Ok(list) => list.first().copied(),
        Err(e) => {
            log::info!("présence au relais : {} ({})", e.kind(), e);
            None
        }
    }
}

// MARK: - Chemin d'une session

/// Le chemin d'une session, tel que l'interface le montre (événement
/// `session-route`, et dans `bar-shown`) : la puce « Par le relais » de la
/// barre (`via_relay`), la ligne « Chemin » (`label`) et les allers-retours.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouteView {
    /// `direct`, `relay`, `relay_tcp`.
    pub transport: &'static str,
    /// « Direct », « Par le relais », « Par le relais (TCP) ».
    pub label: String,
    pub via_relay: bool,
    /// Allers-retours en ms : appareil ↔ relais, relais ↔ Station.
    pub rtt_client: Option<f64>,
    pub rtt_station: Option<f64>,
}

impl RouteView {
    pub fn new(route: &RouteInfo, lang: Lang) -> Self {
        RouteView {
            transport: match route.transport {
                RouteTransport::Direct => "direct",
                RouteTransport::Relay => "relay",
                RouteTransport::RelayTcp => "relay_tcp",
            },
            label: route.transport.label(lang),
            via_relay: route.via_relay(),
            rtt_client: route.rtt_client,
            rtt_station: route.rtt_station,
        }
    }
}

// MARK: - Refus

/// Le texte de link-ui (« Hors de chez vous ») d'un échec de session par le
/// relais, avec le nom de l'ordinateur :
///
/// - l'accès est coupé dans Admin : « {nom} n'est pas joignable hors de chez vous
///   (accès coupé dans Admin). » ;
/// - l'ordinateur n'est pas connecté au relais : « {nom} n'est pas joignable :
///   l'ordinateur est éteint ou en veille. » ;
/// - le relais ne répond pas : « Le relais ne répond pas. ».
///
/// `relay` est l'erreur typée que le cœur a donnée en fin de session
/// (`Platform::on_relay_error`) ; `error`, la raison de la fin qu'il a donnée
/// (le même texte, avec le nom qu'il connaît). Sans erreur du relais, la raison
/// reste telle quelle : son texte ne se lit jamais pour la reconnaître.
pub fn failure_text(relay: Option<&RelayError>, error: Option<String>, name: &str, lang: Lang) -> Option<String> {
    match (relay, error) {
        (Some(e), Some(_)) => Some(e.text(lang, Some(name))),
        (_, error) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::relay::DEFAULT_RELAY;

    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

    #[test]
    fn route_d_un_ordinateur() {
        let info = RelayInfo { server: DEFAULT_RELAY.into(), id: STATION.into(), remote_access: true };
        let route = route_of(Some(&info), Some("sha256:aa")).expect("route");
        assert_eq!((route.server.as_str(), route.station.as_str(), route.admin_fp.as_str()), (DEFAULT_RELAY, STATION, "sha256:aa"));
        // Sans relais chez lui, ou sans empreinte : jamais de premier contact.
        assert_eq!(route_of(None, Some("sha256:aa")), None);
        assert_eq!(route_of(Some(&info), None), None);
        assert_eq!(route_of(Some(&info), Some("  ")), None);
        // Un identifiant illisible n'en fait pas une route.
        assert_eq!(route_of(Some(&RelayInfo { id: "court".into(), ..info }), Some("sha256:aa")), None);
    }

    #[test]
    fn relais_d_un_code() {
        // Aucun ordinateur : rien à ajouter, le cœur interroge seul le relais public.
        assert_eq!(join_relays([]), Vec::<RelayAddr>::new());
        // Ceux des ordinateurs, sans doublon (« share.kybernow.app » et « …:443 » sont le même relais, un autre
        // port reste écrit), les adresses illisibles écartées.
        let relays = join_relays(["share.kybernow.app", "relais.exemple.org:9443", "relais.exemple.org:9443", "share.kybernow.app:443", "a b", ""]);
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], RelayAddr::public());
        assert_eq!((relays[1].host.as_str(), relays[1].port), ("relais.exemple.org", 9443));
        assert_eq!(relays.iter().map(ToString::to_string).collect::<Vec<_>>(), ["share.kybernow.app", "relais.exemple.org:9443"]);
    }

    #[test]
    fn presence_au_plus_toutes_les_30_s() {
        let mut p = Presence::default();
        let t0 = Instant::now();
        assert_eq!(p.ask(STATION, t0), Ask::Due, "rien de connu : à demander");
        p.record(STATION, t0, Some(true));
        assert_eq!(p.ask(STATION, t0 + Duration::from_secs(5)), Ask::Known(Some(true)));
        assert_eq!(p.ask(STATION, t0 + Duration::from_secs(29)), Ask::Known(Some(true)));
        assert_eq!(p.ask(STATION, t0 + PRESENCE_EVERY), Ask::Due, "30 s : redemandée");
        // Un relais muet se redemande au même rythme : pas à chaque relevé de l'accueil.
        p.record(STATION, t0 + PRESENCE_EVERY, None);
        assert_eq!(p.ask(STATION, t0 + PRESENCE_EVERY + Duration::from_secs(8)), Ask::Known(None));
        // Chaque ordinateur a sa réponse.
        assert_eq!(p.ask("autre", t0), Ask::Due);
        p.forget(STATION);
        assert_eq!(p.ask(STATION, t0 + PRESENCE_EVERY + Duration::from_secs(8)), Ask::Due);
    }

    #[test]
    fn chemin_de_la_session() {
        let direct = RouteView::new(&RouteInfo::direct(), Lang::Fr);
        assert_eq!((direct.transport, direct.label.as_str(), direct.via_relay), ("direct", "Direct", false));
        let relay = RouteInfo { transport: RouteTransport::Relay, relay: Some(DEFAULT_RELAY.into()), rtt_client: Some(12.5), rtt_station: Some(1.0) };
        let v = RouteView::new(&relay, Lang::Fr);
        assert_eq!((v.transport, v.label.as_str(), v.via_relay, v.rtt_client, v.rtt_station), ("relay", "Par le relais", true, Some(12.5), Some(1.0)));
        // Le repli TCP : la ligne « Chemin » le dit, la puce reste « Par le relais ».
        let tcp = RouteView::new(&RouteInfo { transport: RouteTransport::RelayTcp, ..relay }, Lang::En);
        assert_eq!((tcp.transport, tcp.label.as_str(), tcp.via_relay), ("relay_tcp", "Via the relay (TCP)", true));
        assert_eq!(serde_json::to_value(&tcp).unwrap()["transport"], "relay_tcp");
    }

    #[test]
    fn refus_avec_le_nom_de_l_ordinateur() {
        let text = |e: RelayError, lang| failure_text(Some(&e), Some("raison du cœur".into()), "Atelier", lang);
        // L'accès coupé dans Admin et l'ordinateur absent du relais : choisis par l'erreur typée, avec le nom.
        assert_eq!(text(RelayError::AccessOff, Lang::Fr).as_deref(), Some("Atelier n'est pas joignable hors de chez vous (accès coupé dans Admin)."));
        assert_eq!(text(RelayError::AccessOff, Lang::En).as_deref(), Some("Atelier cannot be reached away from home (access turned off in Admin)."));
        assert_eq!(text(RelayError::StationOffline, Lang::Fr).as_deref(), Some("Atelier n'est pas joignable : l'ordinateur est éteint ou en veille."));
        assert_eq!(text(RelayError::StationOffline, Lang::En).as_deref(), Some("Atelier cannot be reached: the computer is off or asleep."));
        // Le relais muet garde son texte.
        assert_eq!(text(RelayError::Unreachable("x".into()), Lang::Fr).as_deref(), Some("Le relais ne répond pas."));
        // Sans erreur du relais, la raison reste telle quelle, même si elle ressemble à un refus : le texte ne se lit pas.
        let looks_like = RelayError::StationOffline.text(Lang::Fr, None);
        assert_eq!(failure_text(None, Some(looks_like.clone()), "Atelier", Lang::Fr), Some(looks_like));
        assert_eq!(failure_text(None, Some("Mot de passe refusé".into()), "Atelier", Lang::Fr).as_deref(), Some("Mot de passe refusé"));
        // Une session qui n'a pas fini en erreur n'en a pas.
        assert_eq!(failure_text(Some(&RelayError::AccessOff), None, "Atelier", Lang::Fr), None);
        assert_eq!(failure_text(None, None, "Atelier", Lang::Fr), None);
    }

    /// Les codes des ponts : 30 relais muet, 31 ordinateur absent du relais, 32 accès coupé dans Admin.
    #[test]
    fn codes_du_relais() {
        assert_eq!((RelayError::Unreachable("x".into()).code(), RelayError::StationOffline.code(), RelayError::AccessOff.code()), (30, 31, 32));
        assert_eq!(RelayError::from_code(32), Some(RelayError::AccessOff));
        assert_eq!(RelayError::AccessOff.kind(), "access_off");
    }

    /// Les trois libellés du chemin des catalogues de l'interface sont ceux du cœur.
    #[test]
    fn libelles_du_chemin() {
        for (lang, ui) in [(Lang::Fr, include_str!("../ui/src/lib/i18n/fr.json")), (Lang::En, include_str!("../ui/src/lib/i18n/en.json"))] {
            let ui: serde_json::Value = serde_json::from_str(ui).unwrap();
            for (key, transport) in [("path_direct", RouteTransport::Direct), ("path_relay", RouteTransport::Relay), ("path_relay_tcp", RouteTransport::RelayTcp)] {
                assert_eq!(ui["relay"][key].as_str(), Some(transport.label(lang).as_str()), "{lang:?} {key}");
            }
        }
    }
}
