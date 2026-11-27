//! Accueil des apps Link et incitation au compte KyberNow (docs/link-ui.md,
//! « Accueil et compte KyberNow ») : les règles du cœur (`welcome`) pour
//! Kotlin. L'app garde l'état (`WelcomeState`, en JSON) dans ses préférences ;
//! chaque appel le reçoit, applique une règle et rend le nouvel état avec la
//! réponse : `{"state": {…}, "value": bool}`. Les règles pures sont essayées
//! sur toutes les cibles (`cargo test -p kybernow-android`) ; les fonctions JNI
//! seulement sur Android.

use kybernow_core::i18n::Lang;
use kybernow_core::welcome::{self, WelcomeState};

/// Applique `op` à l'état `state_json` (vide ou illisible : un état neuf, comme
/// au premier lancement) et rend `{"state", "value"}`. `op` :
/// `should_show`, `done`, `card_visible`, `card_close_is_final`, `close_card`,
/// `take_paired_hint`, `take_away_hint` ; `computer` : l'identifiant de
/// l'ordinateur (bandeau après un appairage, « Hors de chez vous ? ») ; `now`
/// en secondes depuis 1970. Une opération inconnue : `value` faux, état
/// inchangé.
pub fn step(state_json: &str, op: &str, has_account: bool, computer: &str, now: u64) -> String {
    let mut s: WelcomeState = serde_json::from_str(state_json.trim()).unwrap_or_default();
    let value = match op {
        "should_show" => s.should_show(has_account),
        "done" => {
            s.welcome_done();
            true
        }
        "card_visible" => s.card_visible(has_account, now),
        "card_close_is_final" => s.card_close_is_final(),
        "close_card" => {
            s.close_card(now);
            true
        }
        "take_paired_hint" => s.take_paired_hint(has_account, computer),
        "take_away_hint" => s.take_away_hint(has_account, computer, now),
        _ => false,
    };
    serde_json::json!({ "state": s, "value": value }).to_string()
}

/// La page d'inscription du relais (`relay` vide : le relais public), à ouvrir
/// dans le navigateur de l'appareil.
pub fn sign_up_url(relay: &str, lang: Lang) -> String {
    welcome::sign_up_url(relay, lang)
}

#[cfg(target_os = "android")]
mod jni_exports {
    use jni::objects::{JClass, JString};
    use jni::sys::{jboolean, jlong, jstring};
    use kybernow_core::i18n;

    use crate::exports::{give, string, Env};

    /// `Native.welcome` : voir [`super::step`].
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_welcome(
        mut env: Env,
        _: JClass,
        state: JString,
        op: JString,
        has_account: jboolean,
        computer: JString,
        now: jlong,
    ) -> jstring {
        kybernow_core::ffi_guard!(null => {
            let state = string(&mut env, &state);
            let op = string(&mut env, &op);
            let computer = string(&mut env, &computer);
            let s = super::step(&state, &op, has_account != 0, &computer, now.max(0) as u64);
            give(&mut env, &s)
        })
    }

    /// `Native.welcomeSignUpUrl` : la page d'inscription, dans la langue de l'app.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_welcomeSignUpUrl(mut env: Env, _: JClass, relay: JString) -> jstring {
        kybernow_core::ffi_guard!(null => {
            let relay = string(&mut env, &relay);
            let s = super::sign_up_url(&relay, i18n::current());
            give(&mut env, &s)
        })
    }

    /// `Native.welcomeStationUrl` : la page de téléchargement de KyberNow Station.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_welcomeStationUrl(mut env: Env, _: JClass) -> jstring {
        kybernow_core::ffi_guard!(null => {
            give(&mut env, kybernow_core::welcome::STATION_DOWNLOAD_URL)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn run(state: &str, op: &str, has_account: bool, computer: &str, now: u64) -> (String, bool) {
        let v: Value = serde_json::from_str(&step(state, op, has_account, computer, now)).unwrap();
        (v["state"].to_string(), v["value"].as_bool().unwrap())
    }

    #[test]
    fn accueil_une_fois_puis_carte_et_moments() {
        let t = 1_800_000_000;
        // Premier lancement : état vide.
        let (s, show) = run("", "should_show", false, "", t);
        assert!(show);
        let (s, _) = run(&s, "done", false, "", t);
        assert!(!run(&s, "should_show", false, "", t).1);
        // La carte : visible, fermée, revenue après 30 jours avec « Ne plus afficher ».
        assert!(run(&s, "card_visible", false, "", t).1);
        assert!(!run(&s, "card_visible", true, "", t).1);
        let (s, _) = run(&s, "close_card", false, "", t);
        assert!(!run(&s, "card_visible", false, "", t + 3600).1);
        assert!(run(&s, "card_visible", false, "", t + welcome::CARD_BACK_AFTER_SECS).1);
        assert!(run(&s, "card_close_is_final", false, "", t).1);
        // Le bandeau après un appairage : une fois par ordinateur.
        let (s, first) = run(&s, "take_paired_hint", false, "pc1", t);
        assert!(first);
        assert!(!run(&s, "take_paired_hint", false, "pc1", t).1);
        // « Hors de chez vous ? » : une fois par jour.
        let (s, away) = run(&s, "take_away_hint", false, "pc1", t);
        assert!(away);
        assert!(!run(&s, "take_away_hint", false, "pc1", t + 60).1);
        // Une opération inconnue ne change rien.
        let (same, v) = run(&s, "autre", false, "", t);
        assert!(!v);
        assert_eq!(same, s);
    }

    #[test]
    fn etat_illisible_comme_au_premier_lancement() {
        assert!(run("pas du json", "should_show", false, "", 0).1);
        assert!(run("{}", "should_show", false, "", 0).1);
    }

    #[test]
    fn inscription() {
        assert_eq!(sign_up_url("", Lang::En), "https://share.kybernow.app/compte/inscription?lang=en");
    }
}
