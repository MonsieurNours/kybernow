//! Le micro de l'appareil dans l'interface (docs/micro.md, docs/link-ui.md « Micro ») : la puce « Micro » de la
//! barre, la tuile du menu, l'icône près de l'indicateur de la session, le réglage « Micro » de la rubrique Son et
//! ceux des Réglages de l'app. L'état et ses textes viennent du cœur (`micro::MicUi`, tenu par `device::mic` et
//! donné en `MicView`) ; restent ici ce qui est propre à l'app : le raccourci, les libellés des réglages, l'avis du
//! système qui ne donne pas le micro. Sans SDL ni cœur : les captures s'en servent.

use serde::Deserialize;

use crate::i18n::tr;

/// Le micro d'une session tel que l'interface le montre : la vue de `micro::MicUi` du cœur (`ui_json`, donnée par le
/// pilote à chaque changement, comme l'enregistrement), textes de link-ui compris : la puce (« Micro », « Micro
/// coupé »), son infobulle (l'état et le raccourci, ou la raison du refus), la tuile (« Envoyé », « Coupé », « Envoi… »
/// tant que l'ordinateur n'a pas répondu). L'interface ne décide de rien : l'appui part au pilote (`Action::SetMic`),
/// qui tient le `MicUi` du cœur.
#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct MicView {
    /// Allumé par l'utilisateur.
    pub on: bool,
    /// `off`, `waiting`, `on`, `refused`.
    pub state: String,
    /// La puce est allumée (allumé, pas refusé).
    pub lit: bool,
    /// Le son part : l'icône près de l'indicateur de la session.
    pub sent: bool,
    /// La raison quand l'ordinateur ne le prend pas : la puce est estompée.
    pub reason: Option<String>,
    pub label: String,
    pub tooltip: String,
    pub tile: String,
    /// La puce est montrée (`MicUi::shown` : pas un invité que l'hôte ne laisse pas parler).
    pub shown: bool,
}

/// Le raccourci de la puce (Ctrl+Alt+Maj + V).
pub fn shortcut() -> &'static str {
    tr("Ctrl+Alt+Maj+V", "Ctrl+Alt+Shift+V")
}

/// « Micro ».
pub fn label_on() -> &'static str {
    tr("Micro", "Microphone")
}

/// L'avis quand l'appareil ne donne pas son micro (le micro reste coupé).
pub fn no_access() -> &'static str {
    tr("KyberNow Link n'a pas accès au micro. Autorisez-le dans les Réglages.", "KyberNow Link can't access the microphone. Allow it in Settings.")
}

/// L'aide du réglage « Micro » de la rubrique Son.
pub fn setting_help() -> &'static str {
    tr(
        "Envoie le micro de cet appareil à l'ordinateur pendant la session, pour parler dans un jeu, un appel ou une visio.",
        "Sends this device's microphone to the computer during the session, to talk in a game, a call or a video meeting.",
    )
}

/// Réglages de l'app : « Micro » (« Celui du système » puis les micros de l'appareil).
pub fn app_setting() -> &'static str {
    tr("Micro", "Microphone")
}

pub fn system_default() -> &'static str {
    tr("Celui du système", "System default")
}

/// Réglages de l'app : « Réduire l'écho et le bruit ».
pub fn echo() -> &'static str {
    tr("Réduire l'écho et le bruit", "Reduce echo and noise")
}

pub fn echo_help() -> &'static str {
    tr(
        "Le traitement voix de l'appareil retire le son de l'ordinateur et le bruit autour de vous. Coupez-le pour un micro de studio.",
        "The device's voice processing removes the computer's sound and the noise around you. Turn it off for a studio microphone.",
    )
}

/// Le micro choisi dans les Réglages de l'app parmi ceux de l'appareil : son indice dans « Celui du système » + la
/// liste (0 : celui du système, aussi quand le micro choisi est débranché ; le choix reste gardé).
pub fn device_index(chosen: &str, mics: &[String]) -> usize {
    if chosen.is_empty() {
        return 0;
    }
    mics.iter().position(|m| m == chosen).map_or(0, |i| i + 1)
}

/// Le micro à ouvrir : `None` pour celui du système (aucun choisi, ou le choisi débranché).
pub fn device_to_open<'a>(chosen: &'a str, mics: &[String]) -> Option<&'a str> {
    (!chosen.is_empty() && mics.iter().any(|m| m == chosen)).then_some(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vue_du_coeur() {
        // Le JSON de `MicUi::ui_json` du cœur (et `shown`, ajouté par le pilote).
        let v: MicView = serde_json::from_str(r#"{"on":true,"state":"waiting","lit":true,"sent":false,"reason":null,"label":"Micro","tooltip":"Micro : envoyé à l'ordinateur (Ctrl+Alt+Maj+V)","tile":"Envoi…","shown":true}"#).unwrap();
        assert_eq!((v.on, v.lit, v.sent, v.tile.as_str(), v.reason), (true, true, false, "Envoi…", None));
    }

    #[test]
    fn micro_choisi_debranche() {
        let mics = vec!["USB".to_string(), "Casque".to_string()];
        assert_eq!(device_index("", &mics), 0);
        assert_eq!(device_index("Casque", &mics), 2);
        assert_eq!(device_index("Ancien", &mics), 0);
        assert_eq!(device_to_open("Ancien", &mics), None);
        assert_eq!(device_to_open("USB", &mics), Some("USB"));
        assert_eq!(device_to_open("", &mics), None);
    }
}
