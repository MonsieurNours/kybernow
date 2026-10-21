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

    /// « donner l'accès à Apple TV et retirer l'accès de iPad ».
    pub fn reason(entries: &[Entry]) -> String {
        let names = |grant: bool| entries.iter().filter(|e| e.grant == grant).map(|e| e.device.as_str()).collect::<Vec<_>>();
        let mut parts = Vec::new();
        let granted = names(true);
        if !granted.is_empty() {
            parts.push(format!("donner l'accès à {}", join(&granted)));
        }
        let revoked = names(false);
        if !revoked.is_empty() {
            parts.push(format!("retirer l'accès de {}", join(&revoked)));
        }
        if parts.is_empty() { "appliquer les comptes".into() } else { parts.join(" et ") }
    }
}

/// « A », « A et B », « A, B et C ».
fn join(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).into(),
        [rest @ .., last] => format!("{} et {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(PendingAccess::reason(&[e("Salon", true)]), "donner l'accès à Salon");
        assert_eq!(
            PendingAccess::reason(&[e("Salon", true), e("Bureau", true), e("Chambre", true), e("iPad", false)]),
            "donner l'accès à Salon, Bureau et Chambre et retirer l'accès de iPad"
        );
        assert_eq!(PendingAccess::reason(&[]), "appliquer les comptes");
    }
}
