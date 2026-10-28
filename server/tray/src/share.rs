//! Le partage vu par l'icône (KyberNow Share, `docs/relais.md`) : ses invités,
//! ses demandes (invités venus par le relais, `pending`, pas encore acceptés :
//! ils ne comptent pas dans les invités) et l'état du relais. Sans Windows,
//! pour que les essais tournent aussi sur le Mac.

use serde_json::Value;

use crate::i18n::{Lang, tr};

fn pending(guest: &Value) -> bool {
    guest["pending"].as_bool() == Some(true)
}

fn all(share: &Value) -> impl Iterator<Item = &Value> {
    share["guests"].as_array().into_iter().flatten()
}

/// Invités acceptés du partage.
pub fn guests(share: &Value) -> impl Iterator<Item = &Value> {
    all(share).filter(|g| !pending(g))
}

/// Demandes en attente de l'accord de l'hôte.
pub fn requests(share: &Value) -> impl Iterator<Item = &Value> {
    all(share).filter(|g| pending(g))
}

/// Identifiants des demandes d'un partage lancé depuis le PC : celles que le
/// panneau annonce (celles d'un partage ouvert par un appareil, l'appareil
/// les montre ; l'ordinateur est alors l'écran de sa session).
pub fn local_request_ids(share: &Value) -> Vec<String> {
    if share["origin"] != "local" {
        return Vec::new();
    }
    requests(share).filter_map(|g| g["id"].as_str().map(str::to_owned)).collect()
}

/// Ligne du relais dans le menu (`relay` de `/api/local/state`) : « Relais
/// joint », « Relais injoignable ({raison}) » ; rien sans relais, quand rien
/// ne le demande (`off`) et pendant la connexion.
pub fn relay_line(lang: Lang, relay: &Value) -> Option<String> {
    if relay["server"].as_str().is_some_and(str::is_empty) {
        return None;
    }
    match relay["state"].as_str()? {
        "joined" => Some(tr(lang, "tray.relay.joined", &[])),
        "unreachable" => Some(match relay["reason"].as_str().map(str::trim).filter(|r| !r.is_empty()) {
            Some(reason) => tr(lang, "tray.relay.unreachable", &[("reason", &reason)]),
            None => tr(lang, "tray.relay.unreachableBare", &[]),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn share(origin: &str) -> Value {
        json!({
            "origin": origin,
            "guests": [
                { "id": "a", "name": "Léa", "pending": false, "via": "lan" },
                { "id": "b", "name": "Max", "pending": true, "via": "relay" },
                { "id": "c", "name": "Zoé", "via": "relay" },
            ],
        })
    }

    #[test]
    fn demandes_a_part_des_invites() {
        let s = share("local");
        let names = |it: &mut dyn Iterator<Item = &Value>| it.map(|g| g["name"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        // Sans `pending` (Station d'avant le relais) : un invité.
        assert_eq!(names(&mut guests(&s)), ["Léa", "Zoé"]);
        assert_eq!(names(&mut requests(&s)), ["Max"]);
        assert_eq!(local_request_ids(&s), ["b"]);
        assert!(local_request_ids(&share("host")).is_empty());
        assert_eq!(guests(&json!({})).count(), 0);
    }

    #[test]
    fn ligne_du_relais() {
        let relay = |state: &str, reason: Value| json!({ "server": "share.kybernow.app", "remote_access": false, "state": state, "reason": reason });
        assert_eq!(relay_line(Lang::Fr, &relay("joined", Value::Null)).as_deref(), Some("Relais joint"));
        assert_eq!(relay_line(Lang::En, &relay("joined", Value::Null)).as_deref(), Some("Relay reached"));
        assert_eq!(relay_line(Lang::Fr, &relay("unreachable", json!("délai dépassé"))).as_deref(), Some("Relais injoignable (délai dépassé)"));
        assert_eq!(relay_line(Lang::En, &relay("unreachable", Value::Null)).as_deref(), Some("Relay unreachable"));
        assert_eq!(relay_line(Lang::Fr, &relay("connecting", Value::Null)), None);
        assert_eq!(relay_line(Lang::Fr, &relay("off", Value::Null)), None);
        assert_eq!(relay_line(Lang::Fr, &json!({ "server": "", "state": "unreachable" })), None);
        assert_eq!(relay_line(Lang::Fr, &Value::Null), None);
    }
}
