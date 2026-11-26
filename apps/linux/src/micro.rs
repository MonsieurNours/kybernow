//! Le micro de l'appareil dans l'interface (docs/micro.md, docs/link-ui.md « Micro ») : la puce « Micro » de la
//! barre, la tuile du menu, l'icône près de l'indicateur de la session, le réglage « Micro » de la rubrique Son et
//! ceux des Réglages de l'app. Les textes sont ceux de link-ui, mot pour mot. Sans SDL ni cœur : les captures s'en
//! servent ; `device::mic` capte et traduit l'état du cœur (`kybernow_core::micro::MicState`) en celui-ci.

use crate::i18n::tr;

/// Pourquoi l'ordinateur ne prend pas le micro (`MicRefusal` du cœur).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicRefusal {
    /// Invité d'un partage que l'hôte ne laisse pas parler.
    NotAllowed,
    /// Pas de micro virtuel sur l'ordinateur.
    NoDevice,
    /// La Station ne peut pas le prendre.
    Unsupported,
    /// Station d'avant le micro.
    OldStation,
}

impl MicRefusal {
    pub fn text(self) -> &'static str {
        match self {
            MicRefusal::NotAllowed => tr("L'hôte ne laisse pas les invités parler.", "The host isn't letting guests speak."),
            MicRefusal::NoDevice => tr("L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son.", "The computer has no virtual microphone: see Admin › Picture and sound."),
            MicRefusal::Unsupported => tr("Cet ordinateur ne reçoit pas le micro.", "This computer can't receive the microphone."),
            MicRefusal::OldStation => tr("Mettez à jour la Station pour envoyer le micro.", "Update the Station to send your microphone."),
        }
    }
}

/// Ce que dit le cœur (`Platform::on_microphone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MicState {
    /// Coupé : rien n'est capté.
    #[default]
    Off,
    /// Allumé, la réponse de l'ordinateur pas encore venue.
    Waiting,
    /// Allumé et pris : le son part.
    On,
    /// L'ordinateur ne le prend pas.
    Refused(MicRefusal),
}

/// « Micro » d'une session : le choix (retenu pour cet ordinateur), l'état du cœur, et l'avis « Micro envoyé à
/// {nom}. » attendu (allumé par l'utilisateur, l'état pas encore passé à « On »).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MicUi {
    pub on: bool,
    pub state: MicState,
    pub announce: bool,
}

impl MicUi {
    pub fn new(on: bool) -> Self {
        Self { on, state: MicState::Off, announce: false }
    }

    /// La raison quand l'ordinateur ne le prend pas : la puce est estompée.
    pub fn refused(&self) -> Option<MicRefusal> {
        match self.state {
            MicState::Refused(r) => Some(r),
            _ => None,
        }
    }

    /// La puce est allumée : allumé et pas refusé (pris, ou la réponse attendue).
    pub fn lit(&self) -> bool {
        self.on && self.refused().is_none()
    }

    /// Le son part vers l'ordinateur : l'icône près de l'indicateur de la session.
    pub fn sent(&self) -> bool {
        self.on && self.state == MicState::On
    }

    /// L'infobulle de la puce : la raison quand l'ordinateur ne le prend pas, sinon l'état et le raccourci.
    pub fn tooltip(&self) -> String {
        if let Some(r) = self.refused() {
            return r.text().into();
        }
        if self.lit() {
            format!("{} ({})", tr("Micro : envoyé à l'ordinateur", "Microphone: sent to the computer"), shortcut())
        } else {
            format!("{} ({})", tr("Micro coupé", "Microphone off"), shortcut())
        }
    }

    /// Le sous-titre de la tuile « Micro » : « Envoyé », « Coupé », « Envoi… » avant la réponse.
    pub fn tile_sub(&self) -> &'static str {
        match (self.lit(), self.state) {
            (true, MicState::On) => tr("Envoyé", "On"),
            (true, _) => tr("Envoi…", "Connecting…"),
            (false, _) => tr("Coupé", "Off"),
        }
    }
}

/// Le raccourci de la puce (Ctrl+Alt+Maj + V).
pub fn shortcut() -> &'static str {
    tr("Ctrl+Alt+Maj+V", "Ctrl+Alt+Shift+V")
}

/// « Micro ».
pub fn label_on() -> &'static str {
    tr("Micro", "Microphone")
}

/// L'avis bref à l'allumage.
pub fn sent_notice(name: &str) -> String {
    if crate::i18n::english() { format!("Microphone sent to {name}.") } else { format!("Micro envoyé à {name}.") }
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
    fn puce_et_tuile() {
        crate::i18n::force_english(false);
        let mut m = MicUi::new(false);
        assert_eq!((m.tile_sub(), m.lit(), m.sent()), ("Coupé", false, false));
        assert_eq!(m.tooltip(), "Micro coupé (Ctrl+Alt+Maj+V)");
        m.on = true;
        m.state = MicState::Waiting;
        assert_eq!((m.tile_sub(), m.lit(), m.sent()), ("Envoi…", true, false));
        m.state = MicState::On;
        assert_eq!((m.tile_sub(), m.sent()), ("Envoyé", true));
        assert_eq!(m.tooltip(), "Micro : envoyé à l'ordinateur (Ctrl+Alt+Maj+V)");
        // Refusé : estompé, la raison en infobulle, coupé dans la tuile.
        m.state = MicState::Refused(MicRefusal::OldStation);
        assert_eq!((m.lit(), m.tile_sub()), (false, "Coupé"));
        assert_eq!(m.tooltip(), "Mettez à jour la Station pour envoyer le micro.");
        crate::i18n::force_english(true);
        assert_eq!(sent_notice("Atelier"), "Microphone sent to Atelier.");
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
