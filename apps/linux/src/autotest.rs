//! Le profil Auto dans l'interface (docs/link-ui.md, « Profil Auto et débit adaptatif » ; contrat :
//! docs/profil-auto.md) : le panneau du test (ses étapes, le résumé, l'échec), la carte « Débit », le bandeau
//! « Autre réseau que celui du test ». Les textes sont ceux du catalogue du cœur (`autotest.*`), mot pour mot ; ceux
//! que le test donne en cours de route (étapes, mesures, choix et raisons) arrivent déjà traduits par le cœur
//! (`device/autotest.rs`). Sans SDL ni cœur : les captures s'en servent.

pub use kybernow_ui::autotest::{
    adaptive, adaptive_help, again, apply, cancel, chosen_by, other_network, other_network_text, phrase, retry, subtitle, summary, title,
    unreachable, waiting_steps, Choice, Step, StepState, BANNER_SECONDS,
};

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
