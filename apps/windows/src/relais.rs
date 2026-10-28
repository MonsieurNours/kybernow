//! Le relais de KyberNow côté app Windows, ce qui ne dépend de rien de Windows
//! (docs/relais.md, textes de docs/link-ui.md, « Hors de chez vous ») pour se
//! tester sur le Mac : la route d'un ordinateur par son relais, les relais à
//! interroger pour un code tapé, la présence au relais (accueil, « Hors de chez
//! vous ») et son rythme, le chemin d'une session tel que la barre et les
//! statistiques le disent, et les textes des refus avec le nom de l'ordinateur.
//! Les appels, les fils et les fenêtres sont dans `ui.rs`, `ui/share.rs` et `host.rs`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use kybernow_core::console::ConsoleError;
use kybernow_core::i18n::Lang;
use kybernow_core::relay::{self, RelayAddr, RelayError, RelayInfo, RelayMode, RelayRoute, RouteInfo, RouteTransport};
use serde::Serialize;

/// Textes : clé du catalogue de l'app (`src/i18n/`) et paramètres.
pub type Tr<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// La route d'un ordinateur par son relais (`info.relay` gardé avec lui, et
/// l'empreinte d'Admin que l'app connaît) : `None` sans relais réglé chez lui,
/// ou sans empreinte (jamais de premier contact par le relais).
pub fn route_of(relay: Option<&RelayInfo>, fingerprint: Option<&str>) -> Option<RelayRoute> {
    relay?.route(fingerprint?)
}

/// Les relais où chercher le code d'un partage : le relais public d'abord, puis
/// ceux des ordinateurs que l'app connaît (`info.relay.server`), sans doublon ;
/// une adresse illisible est écartée. Le code d'un ami part ainsi au relais
/// public même quand l'ordinateur de l'utilisateur en utilise un autre.
pub fn join_relays<'a>(servers: impl IntoIterator<Item = &'a str>) -> Vec<RelayAddr> {
    let mut out = vec![RelayAddr::public()];
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

/// Le message qu'Admin donne quand « Accès hors de chez vous » est coupé
/// (`code: remote_access_off`), dans la langue demandée par l'app.
fn admin_access_off(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => "L'accès hors de chez vous est coupé dans Admin.",
        Lang::En => "Access away from home is turned off in Admin.",
    }
}

/// Le texte de link-ui (« Hors de chez vous ») d'un échec de session par le
/// relais, avec le nom de l'ordinateur que le cœur ne connaît pas :
///
/// - l'accès est coupé dans Admin : « {nom} n'est pas joignable hors de chez vous
///   (accès coupé dans Admin). » ;
/// - l'ordinateur n'est pas connecté au relais : « {nom} n'est pas joignable :
///   l'ordinateur est éteint ou en veille. »
///
/// `None` : l'échec est autre chose, son texte reste tel quel. `error` est le
/// texte que le cœur a donné (celui de la langue de l'app).
pub fn refusal(error: &str, name: &str, lang: Lang, tr: Tr) -> Option<String> {
    let access_off = [Lang::Fr, Lang::En].into_iter().any(|l| {
        let known = ConsoleError::Http { status: 403, message: admin_access_off(l).to_string() };
        error == known.text(l)
    });
    if access_off {
        return Some(tr("relay.access_off", &[("name", name)]));
    }
    let offline = [Lang::Fr, Lang::En].into_iter().any(|l| error == RelayError::StationOffline.text(l, None));
    offline.then(|| RelayError::StationOffline.text(lang, Some(name)))
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
        // Le relais public d'abord, même sans ordinateur.
        assert_eq!(join_relays([]), vec![RelayAddr::public()]);
        // Puis ceux des ordinateurs, sans doublon (le port se complète), les adresses illisibles écartées.
        let relays = join_relays(["share.kybernow.app", "relais.exemple.org:9443", "relais.exemple.org:9443", "a b", ""]);
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], RelayAddr::public());
        assert_eq!((relays[1].host.as_str(), relays[1].port), ("relais.exemple.org", 9443));
        // Le relais d'un ordinateur n'écarte pas le relais public.
        assert_eq!(join_relays(["relais.exemple.org:9443"])[0], RelayAddr::public());
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

    fn tr(key: &str, params: &[(&str, &str)]) -> String {
        assert_eq!(key, "relay.access_off");
        format!("{} n'est pas joignable hors de chez vous (accès coupé dans Admin).", params[0].1)
    }

    #[test]
    fn refus_avec_le_nom_de_l_ordinateur() {
        // Ce que le cœur donne d'un ordinateur absent du relais : le texte sans nom.
        let offline = RelayError::StationOffline.text(Lang::Fr, None);
        assert_eq!(
            refusal(&offline, "Atelier", Lang::Fr, &tr).as_deref(),
            Some("Atelier n'est pas joignable : l'ordinateur est éteint ou en veille.")
        );
        let en = RelayError::StationOffline.text(Lang::En, None);
        assert_eq!(refusal(&en, "Atelier", Lang::En, &tr).as_deref(), Some("Atelier cannot be reached: the computer is off or asleep."));
        // L'accès coupé dans Admin : `403` avec le message d'Admin, dit par le cœur « Admin : … (403) ».
        let off = ConsoleError::Http { status: 403, message: admin_access_off(Lang::Fr).into() }.to_string();
        assert!(refusal(&off, "Atelier", Lang::Fr, &tr).is_some_and(|t| t.starts_with("Atelier n'est pas joignable hors de chez vous")));
        // Un autre échec garde son texte (relais muet, mot de passe refusé, empreinte changée…).
        assert_eq!(refusal(&RelayError::Unreachable("x".into()).text(Lang::Fr, None), "Atelier", Lang::Fr, &tr), None);
        assert_eq!(refusal(&ConsoleError::Http { status: 401, message: "compte".into() }.text(Lang::Fr), "Atelier", Lang::Fr, &tr), None);
        assert_eq!(refusal("autre chose", "Atelier", Lang::Fr, &tr), None);
    }

    /// Le message d'Admin que l'app reconnaît est celui de son catalogue.
    #[test]
    fn message_d_admin_pour_l_acces_coupe() {
        for (lang, admin) in [(Lang::Fr, include_str!("../../../server/admin/src/i18n/fr.json")), (Lang::En, include_str!("../../../server/admin/src/i18n/en.json"))] {
            let admin: serde_json::Value = serde_json::from_str(admin).unwrap();
            assert_eq!(admin["relay"]["accessOff"].as_str(), Some(admin_access_off(lang)), "{lang:?}");
        }
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
