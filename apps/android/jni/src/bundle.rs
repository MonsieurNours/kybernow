//! « Recevoir d'un autre appareil » (docs/playstation.md, « Plusieurs
//! appareils ») : le paquet reçu, lu et rapproché de ce que garde l'appareil
//! par le cœur (`playstation::bundle` : formes tolérées, identifiants PSN
//! vérifiés, `match_accounts`, `BundleStation::matches`), mis à plat pour
//! Kotlin, qui le range dans son magasin (`PsNative.bundlePlan`). Pur :
//! essayé hors d'Android.

use kybernow_core::playstation::bundle::Bundle;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Local {
    id: String,
    psn_id: String,
}

#[derive(Deserialize)]
struct HostRef {
    id: String,
    #[serde(default)]
    fingerprint: Option<String>,
    #[serde(default)]
    address: String,
}

/// `text` : le paquet ; `local` : `[{id, psn_id}]`, les comptes de
/// l'appareil ; `hosts` : `[{id, fingerprint, address}]`, ses ordinateurs.
/// Rend `{"accounts": [{id, psn_id, name, new}], "consoles": [{host_id, name,
/// model, address, target, pairings: {compte ici: association}, hosts:
/// [ordinateurs]}]}` (les associations de comptes absents du paquet laissées
/// de côté), ou `None` si ce n'est pas un paquet.
pub fn plan(text: &str, local: &str, hosts: &str) -> Option<Value> {
    let bundle = Bundle::parse(text)?;
    let local: Vec<Local> = serde_json::from_str(local).unwrap_or_default();
    let hosts: Vec<HostRef> = serde_json::from_str(hosts).unwrap_or_default();
    let pairs: Vec<(String, String)> = local.into_iter().map(|l| (l.id, l.psn_id)).collect();
    let matches = bundle.match_accounts(&pairs);
    let accounts: Vec<Value> = bundle
        .accounts
        .iter()
        .zip(&matches)
        .map(|(a, m)| json!({ "id": m.id, "psn_id": a.psn_id, "name": a.name, "new": m.new }))
        .collect();
    let here = |from: &str| matches.iter().find(|m| m.from.eq_ignore_ascii_case(from)).map(|m| m.id.clone());
    let consoles: Vec<Value> = bundle
        .consoles
        .iter()
        .map(|c| {
            let pairings: serde_json::Map<String, Value> = c.pairings.iter().filter_map(|(k, r)| here(k).map(|id| (id, json!(r)))).collect();
            let matched: Vec<&str> = c
                .stations
                .iter()
                .filter_map(|s| {
                    // Le même identifiant d'abord, puis la règle commune (empreinte, adresse).
                    hosts
                        .iter()
                        .find(|h| !s.id.is_empty() && s.id == h.id)
                        .or_else(|| hosts.iter().find(|h| s.matches(&h.id, h.fingerprint.as_deref(), &h.address)))
                        .map(|h| h.id.as_str())
                })
                .collect();
            json!({
                "host_id": c.host_id,
                "name": c.name,
                "model": c.model,
                "address": c.addresses.first(),
                "target": c.target,
                "pairings": pairings,
                "hosts": matched,
            })
        })
        .collect();
    Some(json!({ "accounts": accounts, "consoles": consoles }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PSN: &str = "AAECAwQFBgc=";

    #[test]
    fn paquet_rapproche_de_l_appareil() {
        let text = format!(
            r#"{{"from":"Mac","accounts":[{{"id":"a1","name":"Léa","psn_id":"{PSN}"}},{{"id":"a2","account":"pas-un-id"}},{{"id":"a3","account_id":"AQIDBAUGBwg="}}],
            "consoles":[{{"host_id":"ab12","model":"ps5","address":"192.168.1.30",
              "pairings":[{{"account":"a1","registered":{{"key":"x"}}}},{{"account":"a2","registered":{{"key":"y"}}}}],
              "stations":["st1",{{"id":"autre","fingerprint":"SHA256:AA:BB"}},{{"id":"x","address":"192.168.1.40"}}]}}]}}"#
        );
        let local = format!(r#"[{{"id":"MOI","psn_id":"{PSN}"}}]"#);
        let hosts = r#"[{"id":"st1","fingerprint":null,"address":"192.168.1.20"},{"id":"pc2","fingerprint":"sha256:aa:bb","address":"192.168.1.21"},{"id":"pc3","address":"192.168.1.40"}]"#;
        let p = plan(&text, &local, hosts).unwrap();
        // a1 : le même identifiant PSN que le compte MOI ; a2 : identifiant illisible, retiré ; a3 : nouveau, sous son identifiant.
        let accounts = p["accounts"].as_array().unwrap();
        assert_eq!(accounts.len(), 2);
        assert_eq!((accounts[0]["id"].as_str(), accounts[0]["new"].as_bool()), (Some("MOI"), Some(false)));
        assert_eq!((accounts[1]["id"].as_str(), accounts[1]["new"].as_bool()), (Some("A3"), Some(true)));
        let c = &p["consoles"][0];
        assert_eq!((c["host_id"].as_str(), c["model"].as_str(), c["address"].as_str()), (Some("AB12"), Some("PS5"), Some("192.168.1.30")));
        // L'association de a1 sous le compte de l'appareil ; celle de a2 (compte absent) laissée de côté.
        assert_eq!(c["pairings"].as_object().unwrap().keys().collect::<Vec<_>>(), vec!["MOI"]);
        // Stations : par l'identifiant, par l'empreinte (règle commune), par l'adresse.
        assert_eq!(c["hosts"], json!(["st1", "pc2", "pc3"]));
        assert!(plan("pas un paquet", "[]", "[]").is_none());
    }
}
