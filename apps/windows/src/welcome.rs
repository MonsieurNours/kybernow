//! Accueil au premier lancement et incitation au compte KyberNow, côté Windows
//! (docs/link-ui.md, « Accueil et compte KyberNow ») : ce que l'interface lit
//! (`api.ts`, `WelcomeView`), sans rien de Windows, pour se tester sur le Mac.
//! Les règles (quand montrer l'accueil, la carte, les bandeaux) sont celles du
//! cœur (`kybernow_core::welcome`) ; l'état (`WelcomeState`) est gardé avec
//! les autres réglages de l'app (`app.json`, `Prefs::welcome`) ; les commandes
//! sont dans `ui/welcome.rs`.

use kybernow_core::account;
use kybernow_core::welcome::WelcomeState;
use serde::Serialize;

/// Ce que l'accueil et l'écran d'accueil lisent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WelcomeView {
    /// Montrer l'accueil en trois écrans au lancement (`should_show`).
    pub show: bool,
    /// Un compte est connecté (pas de bouton ni de carte du compte).
    pub has_account: bool,
    /// La carte du compte, en fin de rangée des ordinateurs.
    pub card: bool,
    /// Sa croix dit « Ne plus afficher » (la carte est revenue après 30 jours).
    pub card_final: bool,
    /// Le relais de « Connecter un compte » et « Créer un compte », tel qu'on
    /// le montre (« La page de share.kybernow.app s'ouvre… »).
    pub relay: String,
}

/// La vue de l'état gardé, à l'instant `now` (secondes depuis 1970).
pub fn view(state: &WelcomeState, has_account: bool, now: u64) -> WelcomeView {
    WelcomeView {
        show: state.should_show(has_account),
        has_account,
        card: state.card_visible(has_account, now),
        card_final: state.card_close_is_final(),
        relay: account::relay_label(&account::normalize_relay("").unwrap_or_default()),
    }
}

/// Le bandeau après l'appairage de `computer` : un seul bandeau à la fois sous la rangée « Se connecter », la
/// mise à jour de la Station passe d'abord (`station_banner`, docs/link-ui.md). Tant qu'elle est là, il attend
/// sans être compté comme montré ; ensuite, la règle du cœur (une fois, sans compte).
pub fn take_paired_hint(state: &mut WelcomeState, has_account: bool, computer: &str, station_banner: bool) -> bool {
    !station_banner && state.take_paired_hint(has_account, computer)
}

/// Secondes depuis 1970.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::welcome::CARD_BACK_AFTER_SECS;

    #[test]
    fn premier_lancement_sans_compte() {
        let s = WelcomeState::default();
        let v = view(&s, false, 1_800_000_000);
        assert!(v.show && v.card && !v.card_final && !v.has_account);
        assert_eq!(v.relay, "share.kybernow.app");
        // Avec un compte : ni accueil, ni carte.
        let v = view(&s, true, 1_800_000_000);
        assert!(!v.show && !v.card && v.has_account);
    }

    #[test]
    fn carte_fermee_puis_revenue() {
        let mut s = WelcomeState::default();
        s.welcome_done();
        let t0 = 1_800_000_000;
        s.close_card(t0);
        let v = view(&s, false, t0 + 3600);
        assert!(!v.show && !v.card);
        let v = view(&s, false, t0 + CARD_BACK_AFTER_SECS);
        assert!(v.card && v.card_final, "revenue : « Ne plus afficher »");
    }

    #[test]
    fn bandeau_apres_appairage_apres_celui_de_la_station() {
        let mut s = WelcomeState::default();
        assert!(!take_paired_hint(&mut s, false, "pc1", true), "la mise à jour de la Station passe d'abord");
        assert!(!take_paired_hint(&mut s, false, "pc1", true));
        assert!(take_paired_hint(&mut s, false, "pc1", false), "pas compté comme montré pendant l'attente");
        assert!(!take_paired_hint(&mut s, false, "pc1", false), "une fois");
        assert!(!take_paired_hint(&mut s, true, "pc2", false), "avec un compte : jamais");
    }

    /// `app.json` d'une version d'avant l'accueil : pas de `welcome`, l'accueil se montre une fois.
    #[test]
    fn app_json_d_avant() {
        #[derive(serde::Deserialize, Default)]
        #[serde(default)]
        struct Prefs {
            welcome: WelcomeState,
        }
        let p: Prefs = serde_json::from_str(r#"{"channel":"stable","language":"auto","close_hint":true}"#).unwrap();
        assert!(view(&p.welcome, false, now()).show);
    }
}
