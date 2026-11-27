//! Accueil des apps Link et incitation au compte KyberNow (docs/link-ui.md,
//! « Accueil et compte KyberNow ») : quand montrer l'accueil, quand montrer
//! la carte du compte, et l'adresse de l'inscription. L'état est gardé par
//! chaque app (`WelcomeState`, en JSON) ; les règles sont ici, pour être les
//! mêmes partout.

use serde::{Deserialize, Serialize};

/// Version de l'accueil : une app qui a montré une version plus ancienne (ou
/// aucune) le montre une fois, sans compte.
pub const WELCOME_VERSION: u32 = 1;

/// Après une première fermeture, la carte du compte revient au bout de 30 jours.
pub const CARD_BACK_AFTER_SECS: u64 = 30 * 24 * 3600;

/// Le moment « hors de chez vous » : une fois par ordinateur et par jour au plus.
pub const AWAY_HINT_EVERY_SECS: u64 = 24 * 3600;

/// Ce que l'app garde (une fois pour l'app, pas par ordinateur, sauf les listes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WelcomeState {
    /// La dernière version de l'accueil montrée (0 : jamais).
    pub welcome_seen: u32,
    /// Fermetures de la carte du compte : 0, 1 (revient après 30 jours), 2 (« Ne plus afficher »).
    pub card_closed: u32,
    /// Date de la dernière fermeture (secondes depuis 1970).
    pub card_closed_at: u64,
    /// Ordinateurs dont le bandeau « après un appairage » a déjà été montré.
    pub paired_hint: Vec<String>,
    /// Dernier moment « hors de chez vous » montré, par ordinateur.
    pub away_hint: Vec<(String, u64)>,
}

impl WelcomeState {
    /// Montrer l'accueil : jamais montré dans cette version, et sans compte.
    /// (Au premier lancement, il n'y a pas de compte.)
    pub fn should_show(&self, has_account: bool) -> bool {
        !has_account && self.welcome_seen < WELCOME_VERSION
    }

    /// L'accueil a été vu (fini, passé ou « Plus tard »).
    pub fn welcome_done(&mut self) {
        self.welcome_seen = WELCOME_VERSION;
    }

    /// La carte du compte à l'accueil : sans compte, pas fermée, ou revenue
    /// après 30 jours ; jamais après « Ne plus afficher ».
    pub fn card_visible(&self, has_account: bool, now: u64) -> bool {
        if has_account {
            return false;
        }
        match self.card_closed {
            0 => true,
            1 => now.saturating_sub(self.card_closed_at) >= CARD_BACK_AFTER_SECS,
            _ => false,
        }
    }

    /// La croix de la carte : « Fermer » la première fois, « Ne plus afficher » ensuite.
    pub fn card_close_is_final(&self) -> bool {
        self.card_closed >= 1
    }

    /// La carte est fermée (la croix).
    pub fn close_card(&mut self, now: u64) {
        self.card_closed = (self.card_closed + 1).min(2);
        self.card_closed_at = now;
    }

    /// Le bandeau après l'appairage de `computer` (sans compte) : une fois.
    pub fn take_paired_hint(&mut self, has_account: bool, computer: &str) -> bool {
        if has_account || self.paired_hint.iter().any(|c| c == computer) {
            return false;
        }
        self.paired_hint.push(computer.to_string());
        true
    }

    /// La fenêtre « Hors de chez vous ? » pour `computer` (sans compte) : une
    /// fois par jour au plus.
    pub fn take_away_hint(&mut self, has_account: bool, computer: &str, now: u64) -> bool {
        if has_account {
            return false;
        }
        match self.away_hint.iter_mut().find(|(c, _)| c == computer) {
            Some((_, at)) if now.saturating_sub(*at) < AWAY_HINT_EVERY_SECS => false,
            Some((_, at)) => {
                *at = now;
                true
            }
            None => {
                self.away_hint.push((computer.to_string(), now));
                true
            }
        }
    }
}

/// La page d'inscription du relais, à ouvrir dans le navigateur de
/// l'appareil (l'app ne crée pas de compte : règle 5.1.1(v) de l'App Store).
/// `relay` vide : le relais public.
pub fn sign_up_url(relay: &str, lang: crate::i18n::Lang) -> String {
    // Sous la forme du relais (sans empreinte épinglée ni port 443, COEUR-40).
    let host = relay.trim().trim_start_matches("https://").trim_end_matches('/');
    let host = if host.is_empty() { "share.kybernow.app".to_string() } else { crate::account::relay_label(host) };
    format!("https://{host}/compte/inscription?lang={}", lang.code())
}

/// La page de téléchargement de KyberNow Station (écran 3 de l'accueil).
pub const STATION_DOWNLOAD_URL: &str = "https://www.kybernow.app/#telecharger";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn accueil_une_fois_sans_compte() {
        let mut s = WelcomeState::default();
        assert!(s.should_show(false), "premier lancement");
        assert!(!s.should_show(true), "avec un compte : jamais");
        s.welcome_done();
        assert!(!s.should_show(false), "déjà vu");
        let json = serde_json::to_string(&s).unwrap();
        let back: WelcomeState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // État d'une app d'avant : champs absents, accueil à montrer.
        let old: WelcomeState = serde_json::from_str("{}").unwrap();
        assert!(old.should_show(false));
    }

    #[test]
    fn carte_revient_apres_trente_jours_puis_ne_plus_afficher() {
        let mut s = WelcomeState::default();
        let t0 = 1_800_000_000;
        assert!(s.card_visible(false, t0));
        assert!(!s.card_visible(true, t0));
        assert!(!s.card_close_is_final());
        s.close_card(t0);
        assert!(!s.card_visible(false, t0 + 29 * 24 * 3600));
        assert!(s.card_visible(false, t0 + CARD_BACK_AFTER_SECS));
        assert!(s.card_close_is_final(), "revenue : « Ne plus afficher »");
        s.close_card(t0 + CARD_BACK_AFTER_SECS);
        assert!(!s.card_visible(false, t0 + 365 * 24 * 3600), "pour de bon");
    }

    #[test]
    fn bandeau_apres_appairage_et_moment_hors_de_chez_vous() {
        let mut s = WelcomeState::default();
        assert!(s.take_paired_hint(false, "pc1"));
        assert!(!s.take_paired_hint(false, "pc1"), "une fois par ordinateur");
        assert!(s.take_paired_hint(false, "pc2"));
        assert!(!s.take_paired_hint(true, "pc3"), "avec un compte : jamais");
        let t = 1_800_000_000;
        assert!(s.take_away_hint(false, "pc1", t));
        assert!(!s.take_away_hint(false, "pc1", t + 3600), "une fois par jour");
        assert!(s.take_away_hint(false, "pc1", t + AWAY_HINT_EVERY_SECS));
        assert!(!s.take_away_hint(true, "pc2", t));
    }

    #[test]
    fn adresse_de_l_inscription() {
        assert_eq!(sign_up_url("", Lang::Fr), "https://share.kybernow.app/compte/inscription?lang=fr");
        assert_eq!(sign_up_url("https://relais.exemple.org:9443/", Lang::En), "https://relais.exemple.org:9443/compte/inscription?lang=en");
        // Relais épinglé : l'empreinte n'entre pas dans l'adresse (COEUR-40).
        assert_eq!(sign_up_url("relais.exemple.org:9443#sha256:abcd", Lang::Fr), "https://relais.exemple.org:9443/compte/inscription?lang=fr");
    }
}
