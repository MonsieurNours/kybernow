//! Le panneau du test du profil Auto (docs/link-ui.md, « Profil Auto et débit
//! adaptatif » ; contrat : docs/profil-auto.md) : ses étapes, son résumé et
//! les textes qui l'entourent. Les étapes et les choix arrivent déjà mis en
//! mots par le cœur (couche appareil) ; avant le premier avancement, les
//! titres des étapes et le relais viennent de son catalogue.

use crate::i18n::{core_text, english, tr};

/// Où en est une étape du test (`StepState` du cœur).
#[derive(Clone, Debug, PartialEq)]
pub enum StepState {
    Waiting,
    /// En cours : la roue.
    Running,
    /// Cochée, avec sa mesure (« 2,1 ms · gigue 0,4 ms »).
    Done(String),
    /// Passée, avec sa raison (« Mettez à jour Atelier pour mesurer le débit. »).
    Skipped(String),
}

/// Une étape, telle que le panneau la montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub title: String,
    pub state: StepState,
}

/// Une ligne du résumé (`Choice` du cœur) : le réglage, sa valeur, sa raison.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub title: String,
    pub value: String,
    pub reason: String,
}

/// « Profil Auto ».
pub fn title() -> &'static str {
    tr("Profil Auto", "Auto profile")
}

/// La phrase du profil Auto.
pub fn phrase() -> &'static str {
    tr("Les meilleurs réglages pour ce réseau et cet appareil.", "The best settings for this network and this device.")
}

/// Les cinq étapes en attente, avant le premier avancement du cœur :
/// « Liaison », « Latence au repos », « Débit vers cet appareil »,
/// « Décodage de cet appareil », « Encodeur de l'ordinateur » (catalogue du
/// cœur, `autotest.step.*`).
pub fn waiting_steps() -> Vec<Step> {
    ["link", "latency", "bandwidth", "decode", "encoder"]
        .into_iter()
        .map(|k| Step { title: core_text(&format!("autotest.step.{k}"), &[]), state: StepState::Waiting })
        .collect()
}

/// « Le test n'a pas pu joindre {nom}. » (catalogue du cœur).
pub fn unreachable(name: &str) -> String {
    core_text("autotest.unreachable", &[("name", name)])
}

/// Le réseau d'un test, à montrer : le relais (`relay`) a son nom (« Par le
/// relais », catalogue du cœur) ; les autres tels quels (« Ethernet », le nom
/// du Wi-Fi).
pub fn network_label(network: &str) -> String {
    if network == "relay" { core_text("autotest.link.relay", &[]) } else { network.into() }
}

/// « Choisi par le test du 26 novembre (Ethernet). » ; sans réseau connu,
/// sans parenthèses.
pub fn chosen_by(at: u64, network: Option<&str>) -> String {
    let date = crate::time::date_label(at);
    let network = network.map(network_label);
    match (english(), network) {
        (true, Some(n)) => format!("Chosen by the test of {date} ({n})."),
        (true, None) => format!("Chosen by the test of {date}."),
        (false, Some(n)) => format!("Choisi par le test du {date} ({n})."),
        (false, None) => format!("Choisi par le test du {date}."),
    }
}

/// « Débit adaptatif ».
pub fn adaptive() -> &'static str {
    tr("Débit adaptatif", "Adaptive bitrate")
}

/// L'aide de « Débit adaptatif » ; en PyroWave, son complément.
pub fn adaptive_help(pyrowave: bool) -> String {
    let base = tr(
        "Baisse le débit quand le réseau sature, puis le remonte. Coupé, le débit ne bouge jamais.",
        "Lowers the bitrate when the network is saturated, then raises it again. Off, the bitrate never moves.",
    );
    if pyrowave {
        format!("{base} {}", tr("En PyroWave, il est en pause\u{202F}: la définition baisse à la place.", "With PyroWave it pauses: the resolution goes down instead."))
    } else {
        base.into()
    }
}

pub fn summary() -> &'static str {
    tr("Réglages proposés", "Suggested settings")
}

pub fn apply() -> &'static str {
    tr("Appliquer", "Apply")
}

pub fn cancel() -> &'static str {
    tr("Annuler", "Cancel")
}

pub fn again() -> &'static str {
    tr("Refaire le test", "Run the test again")
}

pub fn retry() -> &'static str {
    tr("Réessayer", "Try again")
}

/// Le bandeau du début de session : son titre et son texte.
pub fn other_network() -> &'static str {
    tr("Autre réseau que celui du test", "Different network from the test")
}

pub fn other_network_text() -> &'static str {
    tr("Le débit adaptatif s'en charge.", "Adaptive bitrate will handle it.")
}

/// Le bandeau part seul au bout de 8 s (ou à la croix).
pub const BANNER_SECONDS: u64 = 8;

pub fn subtitle() -> &'static str {
    tr("KyberNow mesure votre réseau et cet appareil, puis propose les meilleurs réglages.", "KyberNow measures your network and this device, then suggests the best settings.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choisi_par_le_test() {
        crate::i18n::force_english(false);
        let at = 1_795_000_000;
        let fr = chosen_by(at, Some("Ethernet"));
        assert!(fr.starts_with("Choisi par le test du ") && fr.ends_with(" (Ethernet)."), "{fr}");
        assert!(chosen_by(at, Some("relay")).ends_with("(Par le relais)."));
        assert!(!chosen_by(at, None).contains('('));
        assert_eq!(waiting_steps()[0], Step { title: "Liaison".into(), state: StepState::Waiting });
        assert_eq!(unreachable("Atelier"), "Le test n'a pas pu joindre Atelier.");
        crate::i18n::force_english(true);
        let en = chosen_by(at, Some("Maison"));
        assert!(en.starts_with("Chosen by the test of ") && en.ends_with(" (Maison)."), "{en}");
        assert_eq!(network_label("relay"), "Through the relay");
    }
}
