//! L'app et le registre des sessions du cœur (docs/plusieurs-ordinateurs.md).
//!
//! « Qui est active » et « Garder le son » ont une seule source : `kybernow_core::focus`. L'app lui
//! dit seulement ce qui se passe dans ses fenêtres : une session s'ouvre (`register`, quatre au
//! plus), une fenêtre passe au premier plan (`activate` : `FOCUS_GAINED`, « Afficher », « Ordinateur
//! suivant », la session qu'on vient d'ouvrir), le bouton du haut-parleur (`set_keep_sound`), une
//! session se termine (`unregister`). À chaque changement le cœur applique lui-même à toutes les
//! sessions : le clavier, la souris, les manettes et le presse-papiers n'atteignent que l'active
//! (`Session::set_active` : touches et clics relâchés, manettes au repos, souris relative coupée,
//! presse-papiers en pause, vibrations ignorées) et le son des autres est coupé, sauf « Garder le
//! son » (`Session::set_muted`, fondu de 20 ms).
//!
//! Ce qui reste ici, côté app : recopier la réponse du cœur dans l'interface (`sync` : la session
//! active de `App`, l'état du bouton « Garder le son »), régler l'état global de SDL d'après la
//! session active (souris relative, saisie de texte, manette en souris ; l'économiseur d'écran est
//! coupé tant qu'une session au moins tourne), et donner à chaque session un presse-papiers de
//! l'appareil frais avant qu'elle reprenne la main (`sync_clipboards`).

use std::ffi::c_int;

use kybernow_core::focus::{self, FocusId, Full};
use kybernow_core::session::Session;

use super::pads::Pads;
use super::{sdl, Live, Lives};
use crate::app::{App, SessionId};

// Les deux comptes de quatre (l'interface et le registre) sont les mêmes.
const _: () = assert!(crate::app::MAX_SESSIONS == focus::MAX_SESSIONS);

/// Ce que `sync` a déjà appliqué à SDL : la session active et le nombre de sessions.
#[derive(Default)]
pub struct Applied {
    active: Option<SessionId>,
    sessions: usize,
}

/// Une session s'ouvre : elle est inscrite au registre et devient active, comme sa fenêtre qui
/// s'ouvre par-dessus. `Full` au-delà de quatre.
pub fn register(session: &Session, name: &str) -> Result<FocusId, Full> {
    focus::global().register(session, name)
}

/// La fenêtre de la session `id` passe au premier plan : le cœur en fait la session active (et
/// l'interface la recopie tout de suite). Une session qui n'est plus au registre (finie sur une
/// erreur, son message encore affiché) n'est active que pour l'interface : on peut la fermer.
pub fn activate(app: &mut App, lives: &Lives, id: SessionId) {
    if let Some(l) = lives.get(id)
        && focus::global().active() != Some(l.focus)
    {
        sync_clipboards(lives);
        focus::global().activate(l.focus);
    }
    app.activate_session(id);
}

/// « Ordinateur suivant » : la session ouverte après l'active, dans l'ordre d'ouverture (le cœur).
pub fn next(lives: &Lives) -> Option<SessionId> {
    focus::global().next().and_then(|f| lives.by_focus(f)).map(|l| l.id)
}

/// « Garder le son » : la session reste audible sans être active.
pub fn set_keep_sound(live: &Live, keep: bool) {
    focus::global().set_keep_sound(live.focus, keep);
}

/// La session sort du registre (« Se déconnecter », fermeture de sa fenêtre, fin) : celle qui avait la
/// main juste avant la reprend. Sans effet pour une session qui n'y est plus.
pub fn unregister(lives: &Lives, live: &Live) {
    sync_clipboards(lives);
    focus::global().unregister(live.focus);
}

/// Le texte de l'appareil recopié dans le presse-papiers de chaque session (une seule lecture de SDL).
/// Le pont du cœur de la session qui reprend la main l'annonce à son ordinateur : il doit être frais.
pub fn sync_clipboards(lives: &Lives) {
    let mut text: Option<Option<String>> = None;
    for l in lives.iter() {
        if let Some(c) = &l.handle.shared.clipboard {
            c.system_text(text.get_or_insert_with(sdl::clipboard).clone());
        }
    }
}

/// À chaque tour de la boucle : la réponse du cœur dans l'interface, puis l'état de SDL d'après la
/// session active quand elle a changé. Rend vrai quand l'interface doit se redessiner.
pub fn sync(app: &mut App, lives: &Lives, pads: &mut Pads, applied: &mut Applied) -> bool {
    let core = focus::global();
    // « Garder le son » : l'état du bouton est celui du cœur.
    for l in lives.iter() {
        let keep = core.keep_sound(l.focus);
        if let Some(s) = app.session_by_mut(l.id)
            && s.keep_sound != keep
        {
            s.keep_sound = keep;
        }
    }
    // Qui est active : le cœur a le dernier mot (la session qui reprend la main quand l'active se
    // termine, un changement venu de lui). Une session finie sur une erreur, qui n'est plus au
    // registre, reste active pour l'interface tant que sa fenêtre a le clavier.
    let from_core = core.active().and_then(|f| lives.by_focus(f)).map(|l| l.id);
    let own_is_live = app.active.is_some_and(|a| lives.get(a).is_some());
    if let Some(c) = from_core
        && (own_is_live || app.active.is_none())
        && app.active != Some(c)
    {
        app.activate_session(c);
    }
    let now = (app.active_id(), app.sessions.len());
    if now == (applied.active, applied.sessions) {
        return false;
    }
    (applied.active, applied.sessions) = now;
    apply_sdl(app, lives, pads);
    true
}

/// L'état global de SDL, d'après la session active : souris relative, saisie de texte, manette en
/// souris ; l'économiseur d'écran est coupé tant qu'une session au moins tourne.
fn apply_sdl(app: &App, lives: &Lives, pads: &mut Pads) {
    let active = app.session();
    unsafe {
        sdl::SDL_SetRelativeMouseMode(active.is_some_and(|s| s.relative) as c_int);
        if app.sessions.is_empty() {
            sdl::SDL_ShowCursor(1);
            sdl::SDL_SetCursor(sdl::SDL_GetDefaultCursor());
            sdl::SDL_EnableScreenSaver();
            sdl::SDL_StopTextInput();
        } else {
            sdl::SDL_DisableScreenSaver();
            if active.is_some_and(|s| lives.get(s.id).is_some_and(|l| l.typing)) {
                sdl::SDL_StartTextInput();
            } else {
                sdl::SDL_StopTextInput();
            }
        }
    }
    pads.set_mouse(active.is_some_and(|s| s.pad_mouse));
}
