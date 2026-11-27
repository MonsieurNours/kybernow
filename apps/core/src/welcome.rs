//! Accueil des apps Link et incitation au compte KyberNow (docs/link-ui.md,
//! « Accueil et compte KyberNow ») : les règles de `kybernow_rules::welcome`
//! (la caisse des règles, dont les interfaces Linux et LG dépendent sans le
//! cœur), et l'adresse de l'inscription dans la langue du cœur.

pub use kybernow_rules::welcome::{AWAY_HINT_EVERY_SECS, CARD_BACK_AFTER_SECS, STATION_DOWNLOAD_URL, WELCOME_VERSION, WelcomeState};

/// La page d'inscription du relais, à ouvrir dans le navigateur de
/// l'appareil (`rules::welcome::sign_up_url`). `relay` vide : le relais public.
pub fn sign_up_url(relay: &str, lang: crate::i18n::Lang) -> String {
    kybernow_rules::welcome::sign_up_url(relay, lang.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn adresse_de_l_inscription_dans_la_langue() {
        assert_eq!(sign_up_url("", Lang::Fr), "https://share.kybernow.app/compte/inscription?lang=fr");
        assert_eq!(sign_up_url("relais.exemple.org:9443#sha256:abcd", Lang::En), "https://relais.exemple.org:9443/compte/inscription?lang=en");
        // L'état se garde en JSON (fonctionnalité `serde` de la caisse, prise par le cœur).
        let old: WelcomeState = serde_json::from_str("{}").unwrap();
        assert!(old.should_show(false));
    }
}
