//! Le profil Auto dans l'interface (docs/link-ui.md, « Profil Auto et débit adaptatif » ; contrat :
//! docs/profil-auto.md) : le panneau du test (ses étapes, le résumé, l'échec), la carte « Débit », le bandeau
//! « Autre réseau que celui du test ». Les textes sont ceux du catalogue du cœur (`autotest.*`), mot pour mot ; ceux
//! que le test donne en cours de route (étapes, mesures, choix et raisons) arrivent déjà traduits par le cœur
//! (`device/autotest.rs`). Sans SDL ni cœur : les captures s'en servent.

use crate::i18n::tr;

/// Où en est une étape du test.
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

/// Une ligne du résumé : le réglage, sa valeur, sa raison.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub title: String,
    pub value: String,
    pub reason: String,
}

/// Le panneau du test.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Le test tourne : ses étapes (cinq, celles du cœur).
    Running(Vec<Step>),
    /// « Réglages proposés » : les choix, et le résultat à garder si l'on applique (`AutoResult` du cœur, en JSON),
    /// avec le plafond de l'encodeur de l'ordinateur qui l'accompagne (`AutoResult::next_encoder_max` du plafond gardé).
    Done { choices: Vec<Choice>, result: serde_json::Value, encoder_max: Option<u32> },
    /// Le test n'a pas pu finir (« Le test n'a pas pu joindre {nom}. »).
    Failed(String),
}

/// Le panneau du test d'un ordinateur (par-dessus l'accueil ou les réglages).
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub host: String,
    pub name: String,
    /// Le numéro de ce test : une réponse d'un test annulé ou refait est ignorée.
    pub attempt: u32,
    pub state: State,
}

/// Les cinq étapes en attente, avant le premier avancement du cœur : « Liaison », « Latence au repos », « Débit
/// vers cet appareil », « Décodage de cet appareil », « Encodeur de l'ordinateur » (textes du catalogue du cœur,
/// `autotest.step.*`).
pub fn waiting_steps() -> Vec<Step> {
    [
        tr("Liaison", "Connection"),
        tr("Latence au repos", "Latency at rest"),
        tr("Débit vers cet appareil", "Bitrate to this device"),
        tr("Décodage de cet appareil", "Decoding on this device"),
        tr("Encodeur de l'ordinateur", "The computer's encoder"),
    ]
    .into_iter()
    .map(|t| Step { title: t.into(), state: StepState::Waiting })
    .collect()
}

pub fn title() -> &'static str {
    tr("Profil Auto", "Auto profile")
}

pub fn subtitle() -> &'static str {
    tr("KyberNow mesure votre réseau et cet appareil, puis propose les meilleurs réglages.", "KyberNow measures your network and this device, then suggests the best settings.")
}

/// La phrase du profil Auto.
pub fn phrase() -> &'static str {
    tr("Les meilleurs réglages pour ce réseau et cet appareil.", "The best settings for this network and this device.")
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

/// « Le test n'a pas pu joindre {nom}. » (échec sans réponse du cœur : le pilote n'a pas pu lancer le test).
#[cfg(any(test, not(feature = "device")))]
#[cfg_attr(all(test, feature = "device"), allow(dead_code))]
pub fn unreachable(name: &str) -> String {
    if crate::i18n::english() { format!("The test couldn't reach {name}.") } else { format!("Le test n'a pas pu joindre {name}.") }
}

/// « Débit adaptatif ».
pub fn adaptive() -> &'static str {
    tr("Débit adaptatif", "Adaptive bitrate")
}

/// L'aide de « Débit adaptatif », et en PyroWave son complément.
pub fn adaptive_help(pyrowave: bool) -> String {
    let base = tr(
        "Baisse le débit quand le réseau sature, puis le remonte. Coupé, le débit ne bouge jamais.",
        "Lowers the bitrate when the network is saturated, then raises it again. Off, the bitrate never moves.",
    );
    if pyrowave {
        format!("{base} {}", tr("En PyroWave, il est en pause : la définition baisse à la place.", "With PyroWave it pauses: the resolution goes down instead."))
    } else {
        base.into()
    }
}

/// Le réseau d'un test, à montrer : le relais (`relay`) a son nom (« Par le relais ») ; les autres tels quels
/// (« Ethernet », le nom du Wi-Fi).
pub fn network_label(network: &str) -> String {
    if network == "relay" { tr("Par le relais", "Via the relay").into() } else { network.into() }
}

/// « Choisi par le test du 26 novembre (Ethernet). » ; sans réseau connu, sans parenthèses.
pub fn chosen_by(at: u64, network: Option<&str>) -> String {
    let date = crate::app_time::date_label(at);
    let network = network.map(network_label);
    match (crate::i18n::english(), network) {
        (true, Some(n)) => format!("Chosen by the test of {date} ({n})."),
        (true, None) => format!("Chosen by the test of {date}."),
        (false, Some(n)) => format!("Choisi par le test du {date} ({n})."),
        (false, None) => format!("Choisi par le test du {date}."),
    }
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
