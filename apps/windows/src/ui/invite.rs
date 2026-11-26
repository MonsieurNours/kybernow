//! Invitations côté Windows (docs/invitations.md, textes de docs/link-ui.md
//! « Invitations ») : ajouter un ordinateur de loin, sans que personne ait à
//! l'accepter dessus.
//!
//! « Rejoindre avec un code » tombe sur un code d'invitation
//! (`ShareError::Invite` du cœur) : `share_join` rend `invite`, un ticket et le
//! nom de l'ordinateur ; l'interface demande le mot de passe (« Ajouter
//! {ordinateur} ») et `invite_redeem` ajoute l'ordinateur comme un ordinateur
//! appairé par code : son compte propre (mot de passe au gestionnaire
//! d'identifiants de Windows), l'adresse et le port d'Admin, l'empreinte
//! épinglée, et son relais (`info.relay`) pour l'ouvrir hors de chez soi. Le
//! même Admin déjà sur l'accueil (même empreinte) prend le nouvel accès au lieu
//! d'être ajouté deux fois. Le reste (même ordinateur, nom, réponses) est dans
//! `inviting.rs`.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Manager};

use kybernow_core::invite::{self, InviteCode, Invited};

use super::*;
use crate::inviting::{self, InviteAdded, InviteRefusal};
use crate::sharing::Known;

/// Ce que l'app tient des invitations, à côté de `UiState`.
#[derive(Default)]
pub struct InviteState {
    /// Un code d'invitation trouvé par « Rejoindre avec un code », en attente
    /// de son mot de passe (`invite_redeem`). Gardé après un refus : on
    /// réessaie avec un autre mot de passe sans retaper le code.
    ticket: Mutex<Option<Ticket>>,
}

struct Ticket {
    id: u64,
    code: InviteCode,
}

/// Le numéro des tickets (`share_join` → `invite_redeem`).
static SEQ: AtomicU64 = AtomicU64::new(1);

/// « Rejoindre avec un code » est tombé sur un code d'invitation : le code, la
/// Station et le chemin sont gardés ; l'interface demande le mot de passe et
/// ajoute l'ordinateur par `invite_redeem` (`ticket`). Rend le ticket et le nom
/// de l'ordinateur (vide quand la Station ne le dit pas).
pub fn keep_ticket(state: &UiState, code: InviteCode) -> (u64, String) {
    let id = SEQ.fetch_add(1, Ordering::Relaxed);
    log::info!("partage : le code est un code d'invitation{}", if code.route.is_some() { " (par le relais)" } else { "" });
    let computer = code.computer.trim().to_string();
    *state.invite.ticket.lock().unwrap() = Some(Ticket { id, code });
    (id, computer)
}

/// « Ajouter l'ordinateur » : le mot de passe de l'invitation part à la Station
/// par le chemin de « Rejoindre » (le réseau, ou le relais de bout en bout,
/// Admin épinglé) ; bloque 15 s au plus (`invite::redeem`), sur un fil de
/// Tauri. Rend l'ordinateur ajouté (ou mis à jour), que l'accueil sélectionne
/// avec l'avis « {ordinateur} est ajouté. » ; sinon le refus, montré sous le
/// champ : « Mot de passe refusé. », « Ce code ne sert plus. », « Cette
/// invitation a déjà ajouté tous ses appareils. », « Trop d'essais : réessayez
/// dans {n} min. », l'accès hors de chez vous coupé, ou le texte de l'erreur du
/// cœur.
#[tauri::command(async)]
pub fn invite_redeem(app: AppHandle, ticket: u64, password: String) -> Result<InviteAdded, InviteRefusal> {
    let state = app.state::<UiState>();
    let code = match state.invite.ticket.lock().unwrap().as_ref() {
        Some(tk) if tk.id == ticket => tk.code.clone(),
        _ => return Err(InviteRefusal::other(t("errors.no_session", &[]))),
    };
    let invited = invite::redeem(&code, &password, &system::device_info()).map_err(|e| {
        log::info!("invitation : refusée ({})", e.kind());
        InviteRefusal::of(&e, i18n::lang(), &code.computer)
    })?;
    // Le code a servi : il n'est plus gardé.
    {
        let mut tk = state.invite.ticket.lock().unwrap();
        if tk.as_ref().is_some_and(|x| x.id == ticket) {
            *tk = None;
        }
    }
    keep(&app, &invited)
}

/// Les ordinateurs de l'app, tels que `inviting::same_computer` les compare.
fn known(s: &Store) -> Vec<Known> {
    s.computers.iter().map(|c| Known { host: c.host.clone(), port: c.port, fingerprint: c.fingerprint.clone() }).collect()
}

/// Garde l'ordinateur ajouté comme un ordinateur appairé par code (`by_code`) :
/// le compte au gestionnaire d'identifiants d'abord (rien n'est gardé s'il n'y
/// entre pas), puis `computers.json`. Le même Admin déjà sur l'accueil garde son
/// identifiant, son nom, ses réglages et sa dernière session ; il prend
/// l'adresse, le compte, l'empreinte et le relais nouveaux.
fn keep(app: &AppHandle, invited: &Invited) -> Result<InviteAdded, InviteRefusal> {
    let state = app.state::<UiState>();
    let old = state.with_store(|s| inviting::same_computer(&known(s), invited).and_then(|i| s.computers.get(i).cloned()));
    let mut c = Computer {
        id: old.as_ref().map(|o| o.id.clone()).unwrap_or_else(store::new_id),
        name: old.as_ref().map(|o| o.name.clone()).unwrap_or_else(|| inviting::name_of(invited)),
        host: invited.address.clone(),
        port: invited.port,
        user: invited.username.clone(),
        fingerprint: invited.fingerprint.clone(),
        os: old.as_ref().map(|o| o.os.clone()).unwrap_or_default(),
        gpu: old.as_ref().and_then(|o| o.gpu.clone()),
        settings: Settings::default(),
        last: None,
        by_code: true,
        wake: Vec::new(),
        station_notified: None,
        relay: invited.relay.clone(),
        account: None,
        auto: None,
    };
    if !store::set_password(&c.id, &c.user, &invited.password) {
        return Err(InviteRefusal::other(t("errors.password_store", &[])));
    }
    // Ce que l'ordinateur dit de lui (système, carte graphique, cartes réseau pour
    // le réveil), par le chemin qui l'a ajouté ; sans réponse, le relevé suivant
    // le dira (« Hors de chez vous » quand seul le relais le tient).
    let reachable = !invited.via_relay || relais::route_of(c.relay.as_ref(), c.fingerprint.as_deref()).is_some();
    let info = if reachable { console_via(&c, invited.via_relay).info().ok() } else { None };
    if let Some(i) = &info {
        if !i.os.is_empty() {
            c.os = i.os.clone();
        }
        if i.gpu.is_some() {
            c.gpu = i.gpu.clone();
        }
        c.wake = i.wake.clone();
        if i.relay.is_some() {
            c.relay = i.relay.clone();
        }
    }
    let id = state.with_store(|s| {
        let at = inviting::same_computer(&known(s), invited);
        s.put(c.clone(), at)
    });
    log::info!(
        "invitation : {} {}{}{}",
        c.name,
        if old.is_some() { "prend le nouvel accès" } else { "ajouté" },
        if invited.via_relay { ", par le relais" } else { "" },
        if invited.invite_name.is_empty() { String::new() } else { format!(" (invitation « {} »)", invited.invite_name) }
    );
    let online = info.is_some();
    {
        let mut live = state.live.lock().unwrap();
        let l = live.entry(id.clone()).or_default();
        if online {
            l.online = Some(true);
            l.away = invited.via_relay;
            l.info = info;
        } else {
            l.online = None;
            l.away = false;
        }
    }
    emit_computers(app);
    if !online {
        poll_soon(app);
    }
    Ok(InviteAdded { id, name: c.name })
}
