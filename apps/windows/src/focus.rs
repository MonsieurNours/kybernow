//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : l'app et
//! le registre des sessions du cœur (`kybernow_core::focus`), seule source de
//! « qui est active ». L'app lui dit seulement quelle fenêtre de session passe
//! au premier plan (`activate`), « Garder le son » (`set_keep_sound`), et lui
//! demande l'ordinateur suivant (`next`, Ctrl+Alt+Maj+O) ; le cœur applique
//! lui-même à chaque session « active » (entrées, manettes, presse-papiers)
//! et « son coupé » (fondu de 20 ms), sauf « Garder le son ».
//!
//! Ce module ne fait que relayer, et prévenir l'app d'un changement (le menu
//! de l'icône, dont le haut vaut pour la session active).

use std::sync::OnceLock;

use kybernow_core::focus::{self as core, FocusId, Full};
use kybernow_core::session::Session;

/// Quatre ordinateurs au plus en même temps.
pub const MAX_SESSIONS: usize = core::MAX_SESSIONS;

/// L'app veut savoir quand l'active ou « Garder le son » change.
static LISTENER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

pub fn on_change(f: impl Fn() + Send + Sync + 'static) {
    let _ = LISTENER.set(Box::new(f));
}

fn changed() {
    if let Some(f) = LISTENER.get() {
        f();
    }
}

/// Une session de plus (`name` : son ordinateur, pour le journal) ; elle
/// devient active, comme sa fenêtre qui s'ouvre par-dessus. [`Full`] au-delà
/// de quatre.
pub fn register(session: &Session, name: &str) -> Result<FocusId, Full> {
    let r = core::global().register(session, name);
    changed();
    r
}

/// Fin de la session : si elle avait la main, celle qui l'avait juste avant
/// la reprend.
pub fn unregister(id: FocusId) {
    core::global().unregister(id);
    changed();
}

/// Une fenêtre de cette session passe au premier plan.
pub fn activate(id: FocusId) {
    if core::global().active() == Some(id) {
        return;
    }
    core::global().activate(id);
    changed();
}

/// « Garder le son » de la barre ou du menu de l'icône.
pub fn set_keep_sound(id: FocusId, on: bool) {
    core::global().set_keep_sound(id, on);
    changed();
}

pub fn keep_sound(id: FocusId) -> bool {
    core::global().keep_sound(id)
}

pub fn active() -> Option<FocusId> {
    core::global().active()
}

/// « Ordinateur suivant » : la session ouverte après l'active.
pub fn next() -> Option<FocusId> {
    core::global().next()
}

/// Sessions inscrites.
pub fn count() -> usize {
    core::global().count()
}
