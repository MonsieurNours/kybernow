//! Le paquet de « Envoyer à un appareil » (docs/playstation.md, « Le
//! paquet ») et ses règles de fusion (WINAPP-54) : le même pour toutes les
//! apps, transporté chiffré par [`super::handoff`]. Avant, l'app Windows
//! l'avait en types vérifiés et l'app Linux l'écrivait à la main, sans
//! vérifier l'identifiant PSN.
//!
//! Le cœur lit et écrit le paquet, vérifie les identifiants et dit comment
//! rapprocher ses comptes, ses consoles et ses Stations de ceux de
//! l'appareil ; l'app range le résultat dans son magasin (fichier, trousseau).

use std::collections::BTreeMap;

use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Le paquet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    /// Le paquet de cette version (1).
    #[serde(default = "one")]
    pub version: u32,
    /// Le nom de l'appareil qui envoie.
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub accounts: Vec<BundleAccount>,
    #[serde(default)]
    pub consoles: Vec<BundleConsole>,
}

fn one() -> u32 {
    1
}

/// Un compte PSN : son identifiant chez l'appareil qui envoie, son nom, son
/// identifiant PSN (8 octets en base64).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleAccount {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Toléré : `account`, `account_id` (proposition de l'app LG).
    #[serde(alias = "account", alias = "account_id")]
    pub psn_id: String,
}

/// Une console : ses comptes associés et leur association, ses adresses
/// retenues, les Stations qui la voient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleConsole {
    pub host_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Toléré : une seule adresse en texte (reprise dans `addresses` par
    /// [`Bundle::parse`]).
    #[serde(default, skip_serializing)]
    pub address: Option<String>,
    #[serde(default)]
    pub accounts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Compte → JSON de l'association (`regist::Registered`), en texte.
    /// Toléré : l'objet lui-même, ou un tableau `[{"account","registered"}]`.
    #[serde(default, deserialize_with = "pairings_any")]
    pub pairings: BTreeMap<String, String>,
    /// Toléré : les seuls identifiants, en texte.
    #[serde(default, deserialize_with = "stations_any")]
    pub stations: Vec<BundleStation>,
}

/// Une Station qui voit la console, telle que l'appareil qui envoie la
/// connaît : reliée à un ordinateur de l'appareil qui reçoit par son
/// identifiant, l'empreinte d'Admin ou l'adresse ([`BundleStation::matches`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BundleStation {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub fingerprint: Option<String>,
}

impl BundleStation {
    /// C'est cet ordinateur de l'appareil : même identifiant, même
    /// empreinte d'Admin (la règle commune), ou même adresse.
    pub fn matches(&self, id: &str, fingerprint: Option<&str>, address: &str) -> bool {
        (!self.id.is_empty() && self.id == id)
            || kybernow_rules::fingerprint::same_opt(self.fingerprint.as_deref(), fingerprint)
            || (!self.address.is_empty() && self.address.eq_ignore_ascii_case(address))
    }
}

fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => Some(v.to_string()),
        _ => None,
    }
}

fn pairings_any<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    let mut out = BTreeMap::new();
    match Value::deserialize(d)? {
        Value::Object(map) => {
            for (k, r) in map {
                if let Some(t) = as_text(&r) {
                    out.insert(k, t);
                }
            }
        }
        Value::Array(list) => {
            for e in list {
                if let (Some(a), Some(r)) = (e.get("account").and_then(Value::as_str), e.get("registered").and_then(as_text)) {
                    out.insert(a.to_string(), r);
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

fn stations_any<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<BundleStation>, D::Error> {
    let Value::Array(list) = Value::deserialize(d)? else { return Ok(Vec::new()) };
    Ok(list
        .into_iter()
        .filter_map(|e| match e {
            Value::String(id) => Some(BundleStation { id, name: String::new(), address: String::new(), fingerprint: None }),
            other => serde_json::from_value(other).ok(),
        })
        .collect())
}

/// Un identifiant PSN : 8 octets en base64 (12 caractères), sans espaces
/// autour ; `None` s'il n'en est pas un.
pub fn valid_psn_id(text: &str) -> Option<String> {
    let t = text.trim();
    let bytes = base64::engine::general_purpose::STANDARD.decode(t).ok()?;
    (bytes.len() == 8).then(|| t.to_string())
}

/// Un identifiant de compte neuf (UUID v4, en majuscules comme `uuidString`).
pub fn new_account_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

impl Bundle {
    /// Lit un paquet reçu, mis en forme : comptes sans identifiant PSN
    /// valable retirés, `host_id` et modèle en majuscules, consoles sans
    /// `host_id` retirées, `address` reprise dans `addresses` (sans double ni
    /// vide), associations de comptes inconnus du paquet gardées (l'app les
    /// ignore). `None` si ce n'est pas un paquet.
    pub fn parse(text: &str) -> Option<Bundle> {
        let mut b: Bundle = serde_json::from_str(text).ok()?;
        b.accounts.retain_mut(|a| match valid_psn_id(&a.psn_id) {
            Some(id) => {
                a.psn_id = id;
                true
            }
            None => false,
        });
        b.consoles.retain(|c| !c.host_id.trim().is_empty());
        for c in &mut b.consoles {
            c.host_id = c.host_id.trim().to_ascii_uppercase();
            c.model = c.model.to_ascii_uppercase();
            if let Some(a) = c.address.take() {
                c.addresses.push(a);
            }
            let mut seen = Vec::new();
            c.addresses.retain(|a| !a.trim().is_empty() && !seen.contains(a) && {
                seen.push(a.clone());
                true
            });
        }
        Some(b)
    }

    /// Le paquet à envoyer (JSON).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Les comptes du paquet rapprochés de ceux de l'appareil (`local` :
    /// identifiant, identifiant PSN) : même identifiant PSN → le compte de
    /// l'appareil ; sinon un nouveau compte, sous l'identifiant du paquet
    /// (en majuscules) s'il est libre, sinon un neuf.
    pub fn match_accounts(&self, local: &[(String, String)]) -> Vec<AccountMatch> {
        let mut taken: Vec<String> = local.iter().map(|(id, _)| id.to_ascii_uppercase()).collect();
        self.accounts
            .iter()
            .map(|a| match local.iter().find(|(_, psn)| psn.trim() == a.psn_id) {
                Some((id, _)) => AccountMatch { from: a.id.clone(), id: id.clone(), new: false },
                None => {
                    let wanted = a.id.trim().to_ascii_uppercase();
                    let id = if !wanted.is_empty() && !taken.contains(&wanted) { wanted } else { new_account_id() };
                    taken.push(id.clone());
                    AccountMatch { from: a.id.clone(), id, new: true }
                }
            })
            .collect()
    }
}

/// Un compte du paquet chez l'appareil qui reçoit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountMatch {
    /// Son identifiant dans le paquet (clé de `pairings`).
    pub from: String,
    /// Son identifiant chez l'appareil.
    pub id: String,
    /// À créer chez l'appareil (identifiant PSN à garder).
    pub new: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const PSN: &str = "AAECAwQFBgc=";

    #[test]
    fn paquet_commun() {
        let text = format!(
            r#"{{"from":"Mac","accounts":[{{"id":"a1","name":"Léa","psn_id":"{PSN}"}},{{"id":"a2","account":"pas-un-id"}}],
            "consoles":[{{"host_id":"ab12","model":"ps5","address":"192.168.1.30","addresses":["192.168.1.30",""],
              "pairings":[{{"account":"a1","registered":{{"key":"x"}}}}],"stations":["st1",{{"id":"st2","fingerprint":"sha256:aa"}}]}},
              {{"host_id":" "}}]}}"#
        );
        let b = Bundle::parse(&text).unwrap();
        assert_eq!(b.version, 1);
        assert_eq!(b.accounts.len(), 1, "identifiant PSN illisible : retiré");
        let c = &b.consoles[0];
        assert_eq!((c.host_id.as_str(), c.model.as_str(), c.addresses.clone()), ("AB12", "PS5", vec!["192.168.1.30".to_string()]));
        assert_eq!(c.pairings.get("a1").map(String::as_str), Some(r#"{"key":"x"}"#));
        assert_eq!(c.stations.len(), 2);
        assert_eq!(b.consoles.len(), 1, "console sans host_id : retirée");
        // Aller-retour.
        assert_eq!(Bundle::parse(&b.to_json()).unwrap(), b);
        assert!(Bundle::parse("pas un paquet").is_none());
        // Stations : par identifiant, empreinte (règle commune) ou adresse.
        let s = &c.stations[1];
        assert!(s.matches("st2", None, "") && s.matches("x", Some("SHA256:AA"), "") && !s.matches("x", Some("sha256:bb"), ""));
        assert!(BundleStation { id: String::new(), name: String::new(), address: "pc.local".into(), fingerprint: None }.matches("y", None, "PC.local"));
    }

    #[test]
    fn comptes_rapproches() {
        assert_eq!(valid_psn_id(&format!(" {PSN} ")).as_deref(), Some(PSN));
        assert_eq!(valid_psn_id("AAECAw=="), None);
        let b = Bundle {
            accounts: vec![
                BundleAccount { id: "a1".into(), name: None, psn_id: PSN.into() },
                BundleAccount { id: "local-1".into(), name: None, psn_id: "BwYFBAMCAQA=".into() },
                BundleAccount { id: "a3".into(), name: None, psn_id: "AQEBAQEBAQE=".into() },
            ],
            ..Default::default()
        };
        let local = vec![("LOCAL-0".to_string(), PSN.to_string()), ("LOCAL-1".to_string(), "x".to_string())];
        let m = b.match_accounts(&local);
        assert_eq!(m[0], AccountMatch { from: "a1".into(), id: "LOCAL-0".into(), new: false }, "même identifiant PSN");
        assert!(m[1].new && m[1].id != "LOCAL-1" && m[1].id.len() == 36, "identifiant pris : un neuf");
        assert_eq!(m[2], AccountMatch { from: "a3".into(), id: "A3".into(), new: true });
    }
}
