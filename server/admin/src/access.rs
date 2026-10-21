//! Accès en attente : comptes d'appareils (appairage par code) écrits dans la
//! configuration de Kyber, mais que Kyber en marche ne connaît pas encore.
//!
//! Kyber ne lit ses comptes qu'à son démarrage. Quand un appareil reçoit (ou
//! perd) son accès pendant la session d'un autre, la console ne peut pas
//! redémarrer Kyber tout de suite sans couper cette session : le compte est
//! noté ici, et Kyber redémarre dès qu'il redevient libre (voir
//! `api::supervise_access`).

use std::sync::Mutex;

use serde::Serialize;
use serde_json::json;

use crate::i18n::Msg;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    /// Compte Kyber de l'appareil.
    pub account: String,
    /// Nom de l'appareil.
    pub device: String,
    /// Vrai : accès donné ; faux : accès retiré.
    pub grant: bool,
}

#[derive(Default)]
pub struct PendingAccess(Mutex<Vec<Entry>>);

impl PendingAccess {
    /// Note un accès donné ou retiré (remplace ce qui attendait pour ce compte).
    pub fn add(&self, account: &str, device: &str, grant: bool) {
        let mut entries = self.0.lock().unwrap();
        entries.retain(|e| e.account != account);
        entries.push(Entry { account: account.into(), device: device.into(), grant });
    }

    /// Oublie un compte (supprimé avant que Kyber l'ait jamais connu).
    pub fn forget(&self, account: &str) {
        self.0.lock().unwrap().retain(|e| e.account != account);
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }

    /// Ce compte a-t-il reçu un accès que Kyber en marche ne connaît pas encore ?
    pub fn waits_for(&self, account: &str) -> bool {
        self.0.lock().unwrap().iter().any(|e| e.grant && e.account == account)
    }

    pub fn snapshot(&self) -> Vec<Entry> {
        self.0.lock().unwrap().clone()
    }

    /// Kyber a redémarré : il connaît tout ce qui attendait au moment de
    /// `taken` (ce qui est arrivé depuis attend encore).
    pub fn applied(&self, taken: &[Entry]) {
        self.0.lock().unwrap().retain(|e| !taken.contains(e));
    }

    /// « donner l'accès à Apple TV et retirer l'accès de iPad », écrit à
    /// l'affichage dans la langue de qui lit.
    pub fn reason(entries: &[Entry]) -> Msg {
        let names = |grant: bool| entries.iter().filter(|e| e.grant == grant).map(|e| e.device.clone()).collect::<Vec<_>>();
        let part = |key: &str, names: Vec<String>| (!names.is_empty()).then(|| Msg::new(key).with("names", json!({ "list": names })));
        match (part("access.grant", names(true)), part("access.revoke", names(false))) {
            (Some(grant), Some(revoke)) => Msg::new("access.both").with_msg("grant", grant).with_msg("revoke", revoke),
            (Some(one), None) | (None, Some(one)) => one,
            (None, None) => Msg::new("access.apply"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn grant_waits_until_applied() {
        let p = PendingAccess::default();
        p.add("apple-tv", "Apple TV", true);
        assert!(p.waits_for("apple-tv"));
        assert!(!p.waits_for("autre"));
        let taken = p.snapshot();
        p.add("ipad", "iPad", true);
        p.applied(&taken);
        assert!(!p.waits_for("apple-tv"));
        // Arrivé pendant le redémarrage : attend le suivant.
        assert!(p.waits_for("ipad"));
    }

    #[test]
    fn revoke_replaces_grant_and_does_not_wait() {
        let p = PendingAccess::default();
        p.add("apple-tv", "Apple TV", true);
        p.add("apple-tv", "Apple TV", false);
        assert_eq!(p.snapshot().len(), 1);
        assert!(!p.waits_for("apple-tv"));
        assert!(!p.is_empty());
        p.forget("apple-tv");
        assert!(p.is_empty());
    }

    #[test]
    fn reason_names_devices() {
        let e = |d: &str, grant| Entry { account: d.to_lowercase(), device: d.into(), grant };
        assert_eq!(PendingAccess::reason(&[e("Salon", true)]).to_string(), "donner l'accès à Salon");
        let both = PendingAccess::reason(&[e("Salon", true), e("Bureau", true), e("Chambre", true), e("iPad", false)]);
        assert_eq!(both.to_string(), "donner l'accès à Salon, Bureau et Chambre et retirer l'accès de iPad");
        assert_eq!(both.render(Lang::En), "give access to Salon, Bureau and Chambre and remove access from iPad");
        assert_eq!(PendingAccess::reason(&[]).to_string(), "appliquer les comptes");
    }
}
