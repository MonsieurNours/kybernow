//! Le prêt vu par l'hôte dans l'icône (la **vue du prêt** de `docs/pret.md`,
//! `lend` de `/api/local/state`) : l'état de l'ordinateur, les demandes, la
//! ligne du menu. Sans Windows, pour les essais sur le Mac.

use serde_json::Value;

use crate::i18n::{Lang, tr};

/// Un prêt en cours (ou qui se prépare, ou qui finit) : l'ordinateur est à l'ami.
pub fn active(lend: &Value) -> bool {
    matches!(lend["state"].as_str(), Some("preparing" | "lent" | "ending")) && lend["borrower"].is_object()
}

/// Prénom de l'ami qui a l'ordinateur.
pub fn borrower(lend: &Value) -> Option<&str> {
    lend["borrower"]["name"].as_str()
}

/// Fin du préavis (secondes Unix) quand l'ordinateur revient bientôt à l'hôte.
pub fn notice_end(lend: &Value) -> Option<i64> {
    let notice = &lend["notice"];
    match notice["reason"].as_str()? {
        "taken_back" | "time_up" | "slot_end" => notice["at"].as_i64(),
        _ => None,
    }
}

/// Préavis d'un créneau qui commence : « Créneau de Léa : l'ordinateur lui
/// est prêté dans 2 min. » ; sa clé (pour n'ouvrir le panneau qu'une fois).
pub fn slot_start(lend: &Value) -> Option<String> {
    let notice = &lend["notice"];
    (notice["reason"] == "slot_start").then(|| format!("slot:{}", notice["at"]))
}

/// Identifiants des demandes en attente.
pub fn request_ids(lend: &Value) -> Vec<String> {
    lend["requests"].as_array().into_iter().flatten().filter_map(|r| r["id"].as_str().map(str::to_owned)).collect()
}

/// Ligne de l'ordinateur dans le menu pendant un prêt : « Prêté à Léa
/// jusqu'à 18 h », et pendant le préavis « Prêté à Léa · Reprise dans 1:42 ».
/// `until` : l'heure de fin déjà écrite (« 18 h »).
pub fn status_line(lang: Lang, lend: &Value, until: Option<&str>, now: i64) -> Option<String> {
    if !active(lend) {
        return None;
    }
    let name = borrower(lend).unwrap_or_default();
    if let Some(at) = notice_end(lend) {
        let base = tr(lang, "tray.session.lentTo", &[("name", &name)]);
        let left = tr(lang, "tray.session.takingBackIn", &[("left", &super::countdown(at - now))]);
        return Some(format!("{base} · {left}"));
    }
    Some(match until {
        Some(time) => tr(lang, "tray.session.lentToUntil", &[("name", &name), ("time", &time)]),
        None => tr(lang, "tray.session.lentTo", &[("name", &name)]),
    })
}

/// Durées proposées pour un code (`duration` de `POST /api/local/lend/code`),
/// `None` : jusqu'à ce que l'hôte reprenne.
pub fn duration(value: &Value) -> Option<Option<u64>> {
    match value {
        Value::Null => Some(None),
        v => v.as_u64().filter(|d| [3600, 7200, 14400].contains(d)).map(Some),
    }
}

/// `friend` ou `host`, `friend` par défaut.
pub fn steam(value: &Value) -> &'static str {
    if value == "host" { "host" } else { "friend" }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn lend(state: &str, notice: Value) -> Value {
        json!({
            "enabled": true, "state": state,
            "borrower": if state == "free" { Value::Null } else { json!({"name": "Léa", "kind": "code", "app": "ios", "via": "relay", "since": 1, "until": 1793180000, "steam": "friend", "connected": true}) },
            "notice": notice,
            "requests": [{"id": "r1", "name": "Max", "kind": "friend", "app": "web", "via": "lan", "since": 2}, {"name": "sans id"}],
            "code": null,
        })
    }

    #[test]
    fn etat_et_ligne() {
        let now = 1_793_170_000;
        assert!(!active(&lend("free", Value::Null)));
        assert!(active(&lend("lent", Value::Null)));
        assert_eq!(request_ids(&lend("free", Value::Null)), ["r1"]);
        assert_eq!(status_line(Lang::Fr, &lend("free", Value::Null), None, now), None);
        assert_eq!(status_line(Lang::Fr, &lend("lent", Value::Null), Some("18 h"), now).as_deref(), Some("Prêté à Léa jusqu'à 18 h"));
        assert_eq!(status_line(Lang::En, &lend("lent", Value::Null), None, now).as_deref(), Some("Lent to Léa"));
        let notice = json!({"reason": "taken_back", "at": now + 102});
        assert_eq!(status_line(Lang::Fr, &lend("lent", notice.clone()), Some("18 h"), now).as_deref(), Some("Prêté à Léa · Reprise dans 1:42"));
        assert_eq!(status_line(Lang::En, &lend("lent", notice), None, now).as_deref(), Some("Lent to Léa · Taking back in 1:42"));
        let slot = json!({"reason": "slot_start", "at": now + 120});
        assert_eq!(notice_end(&lend("free", slot.clone())), None);
        assert_eq!(slot_start(&lend("free", slot)).as_deref(), Some("slot:1793170120"));
        assert_eq!(slot_start(&lend("free", Value::Null)), None);
    }

    #[test]
    fn choix_du_code() {
        assert_eq!(duration(&json!(7200)), Some(Some(7200)));
        assert_eq!(duration(&Value::Null), Some(None));
        assert_eq!(duration(&json!(60)), None);
        assert_eq!(steam(&json!("host")), "host");
        assert_eq!(steam(&json!("autre")), "friend");
    }
}
