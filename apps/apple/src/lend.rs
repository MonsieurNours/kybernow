//! Le prêt de l'ordinateur pour les apps Apple (docs/pret.md) : l'ami
//! (statut de l'ordinateur, emprunt par un code ou un compte d'ami, attente
//! de l'accord de l'hôte, départ, lien d'invitation) et l'hôte (la vue du
//! prêt, le code de séance, les demandes, « Reprendre l'ordinateur »). La
//! session d'emprunt passe par `borrower` dans `kn_session_start`.

use std::ffi::c_char;

use kybernow_core::i18n;
use kybernow_core::lend::{self, BorrowRequest, LendAvailability, LendError, LendNotice, LendSteam, Loan, PendingBorrow};
use kybernow_core::relay::RelayRoute;
use kybernow_core::share::ShareTarget;

use crate::{KN_ERROR, KN_OK, KnConsole, console_result, failure, give, give_to, optional, text};

/// Refus de l'emprunt (`error` : le texte à montrer, docs/link-ui.md) :
/// identifiant ou mot de passe refusé, code qui ne sert plus, ordinateur
/// déjà prêté, verrouillé, prêt coupé, trop d'essais, l'hôte a refusé ou n'a
/// pas répondu (`kn_lend_wait`), Station sans prêt (avant 0.9.0) ;
/// `KN_LEND_WAITING` : `kn_lend_wait` toujours en attente (rappeler).
pub const KN_LEND_BAD_CREDENTIALS: i32 = 40;
pub const KN_LEND_NO_CODE: i32 = 41;
pub const KN_LEND_LENT: i32 = 42;
pub const KN_LEND_LOCKED: i32 = 43;
pub const KN_LEND_OFF: i32 = 44;
pub const KN_LEND_TOO_MANY: i32 = 45;
pub const KN_LEND_REFUSED: i32 = 46;
pub const KN_LEND_EXPIRED: i32 = 47;
pub const KN_LEND_UNSUPPORTED: i32 = 48;
pub const KN_LEND_WAITING: i32 = 49;

/// La fin d'un emprunt (`on_lend_end`) : repris par l'hôte, durée passée,
/// fin du créneau, rendu (fin voulue : « Vous avez rendu l'ordinateur. »),
/// arrêté (« Le prêt s'est arrêté. »).
pub const KN_LEND_END_TAKEN_BACK: i32 = 0;
pub const KN_LEND_END_TIME_UP: i32 = 1;
pub const KN_LEND_END_SLOT_END: i32 = 2;
pub const KN_LEND_END_LEFT: i32 = 3;
pub const KN_LEND_END_STOPPED: i32 = 4;

/// Le compte Steam du prêt : celui de l'ami (« Son compte Steam »), celui de
/// l'hôte (« Votre compte Steam »).
pub const KN_LEND_STEAM_FRIEND: i32 = 0;
pub const KN_LEND_STEAM_HOST: i32 = 1;

fn lend_failure(e: LendError, host: &str, error: *mut *mut c_char) -> i32 {
    let message = e.text(i18n::current(), host);
    match e {
        LendError::Other(e) => failure(e, error),
        e => {
            give_to(error, message);
            e.code().unwrap_or(KN_ERROR)
        }
    }
}

fn parsed<T: serde::de::DeserializeOwned>(json: *const c_char, error: *mut *mut c_char) -> Option<T> {
    match serde_json::from_str(&text(json)) {
        Ok(v) => Some(v),
        Err(e) => {
            give_to(error, e.to_string());
            None
        }
    }
}

fn route(route_json: *const c_char) -> Option<RelayRoute> {
    optional(route_json).and_then(|r| serde_json::from_str::<RelayRoute>(&r).ok())
}

/// L'état de l'ordinateur d'un ami (la carte « Ordinateur de {hôte} ») :
/// `target_json` `{host, port, fingerprint}` (celui de l'invitation),
/// `route_json` sa route par le relais ou nul ; `out` : `{state, host,
/// computer, slot}` (`state` : `free`, `busy`, `lent`, `locked`, `off`).
/// Bloquant.
///
/// # Safety
/// Chaînes C valables (ou nulles pour `route_json`) ; `out`, `error` valables ou nuls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_status(
    target_json: *const c_char,
    route_json: *const c_char,
    login: *const c_char,
    password: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let Some(target) = parsed::<ShareTarget>(target_json, error) else { return KN_ERROR };
    match lend::status(&target, route(route_json).as_ref(), &text(login), &text(password)) {
        Ok(s) => {
            give_to(out, serde_json::to_string(&s).unwrap_or_default());
            KN_OK
        }
        Err(e) => lend_failure(e, "", error),
    }
}

/// « Emprunter » : `request_json` `{"code"}` ou `{"login","password"}`, avec
/// `name`, `device`, `app`, `screen` `{width, height, refresh}` (l'écran de
/// l'appareil), `codecs`. Un code de « Rejoindre » qui est un code de prêt
/// (`KN_SHARE_LEND`) donne `target` et `route` dans son `out`. `out` :
/// `{"ready": {target, borrowed}}` (à donner à `kn_session_start` :
/// `console_*` de `target`, `borrower.borrowed`) ou `{"pending": {…}}`
/// (« En attente de {hôte}… » : `kn_lend_wait`). Bloquant (30 s au plus, le
/// temps du mode console).
///
/// # Safety
/// Comme `kn_lend_status`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_borrow(target_json: *const c_char, route_json: *const c_char, request_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(target) = parsed::<ShareTarget>(target_json, error) else { return KN_ERROR };
    let Some(request) = parsed::<BorrowRequest>(request_json, error) else { return KN_ERROR };
    match lend::borrow(&target, route(route_json).as_ref(), &request) {
        Ok(b) => {
            give_to(out, serde_json::to_string(&b).unwrap_or_default());
            KN_OK
        }
        Err(e) => lend_failure(e, "", error),
    }
}

/// L'hôte a-t-il répondu (`pending_json` : `pending` de `kn_lend_borrow`) ?
/// Tenu 30 s : `KN_OK` et `out` = `{target, borrowed}` (accepté),
/// `KN_LEND_WAITING` (rappeler), `KN_LEND_REFUSED`, `KN_LEND_EXPIRED`
/// (« {hôte} a refusé. », « {hôte} n'a pas répondu. »). Bloquant.
///
/// # Safety
/// Comme `kn_lend_status`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_wait(pending_json: *const c_char, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(pending) = parsed::<PendingBorrow>(pending_json, error) else { return KN_ERROR };
    match lend::wait(&pending) {
        Ok(Some(loan)) => {
            give_to(out, serde_json::to_string(&loan).unwrap_or_default());
            KN_OK
        }
        Ok(None) => KN_LEND_WAITING,
        Err(e) => lend_failure(e, &pending.host, error),
    }
}

/// « Annuler » une demande en attente.
///
/// # Safety
/// `pending_json` : chaîne C valable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_cancel(pending_json: *const c_char) -> i32 {
    let Ok(pending) = serde_json::from_str::<PendingBorrow>(&text(pending_json)) else { return KN_ERROR };
    lend::cancel(&pending);
    KN_OK
}

/// « Rendre l'ordinateur » hors d'une session (`loan_json` : `{target,
/// borrowed}`) ; en session : `kn_session_give_back`. Bloquant.
///
/// # Safety
/// `loan_json` : chaîne C valable ; `error` valable ou nul.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_leave(loan_json: *const c_char, error: *mut *mut c_char) -> i32 {
    let Some(loan) = parsed::<Loan>(loan_json, error) else { return KN_ERROR };
    match lend::leave(&loan) {
        Ok(()) => KN_OK,
        Err(e) => lend_failure(e, "", error),
    }
}

/// Lit un lien d'invitation (`kybernow://borrow?host&port&fp&login[&relay&sid]`,
/// ou `https://…/link/?borrow=<identifiant>`, avec `&name=` facultatif) :
/// `{host, port, fingerprint, login, relay, sid, name, route}` (`name` : le
/// prénom de l'hôte ou nul ; `route` : la route par le relais, ou nulle) ;
/// ce JSON sert aussi de `target_json`. Nul si ce n'en est pas un.
///
/// # Safety
/// `url` : chaîne C valable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_parse_invite(url: *const c_char) -> *mut c_char {
    match lend::parse_invite(&text(url)) {
        Some(invite) => {
            let mut v = serde_json::to_value(&invite).unwrap_or_default();
            v["route"] = serde_json::to_value(invite.relay_route()).unwrap_or_default();
            give(v.to_string())
        }
        None => std::ptr::null_mut(),
    }
}

/// L'état de la carte « Ordinateur de {hôte} » (`free`, `busy`, `lent`,
/// `locked`, `off`) : « Disponible », « Occupé »… ; nul pour un état inconnu.
///
/// # Safety
/// `state` : chaîne C valable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_availability_label(state: *const c_char) -> *mut c_char {
    match LendAvailability::parse(&text(state)) {
        Some(a) => give(a.label(i18n::current())),
        None => std::ptr::null_mut(),
    }
}

/// Le texte d'un préavis (`notice_json` : `{reason, at}`) ; `name` : le
/// prénom de l'hôte chez l'ami, celui de l'ami pour `slot_start` chez l'hôte.
///
/// # Safety
/// Chaînes C valables (`name` nul : sans nom).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_lend_notice_text(notice_json: *const c_char, name: *const c_char) -> *mut c_char {
    match serde_json::from_str::<LendNotice>(&text(notice_json)) {
        Ok(n) => give(n.text(i18n::current(), &text(name))),
        Err(_) => std::ptr::null_mut(),
    }
}

// L'hôte : sur la `KnConsole` de l'ordinateur (appareil appairé). `out` :
// la vue du prêt (`enabled`, `state`, `borrower`, `notice`, `requests`,
// `code`), comme le champ `lend` de `on_lend`. Bloquants.

/// # Safety
/// `c` : console valable ; `out`, `error` valables ou nuls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_lend_state(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.lend_state(), out, error)
}

/// « Créer un code » : `duration` en secondes (0 ou moins : « Jusqu'à ce
/// que je reprenne »), `steam` `KN_LEND_STEAM_*`.
///
/// # Safety
/// Comme `kn_console_lend_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_lend_code(c: *const KnConsole, duration: i64, steam: i32, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    let duration = (duration > 0).then_some(duration as u64);
    console_result(c.0.lend_code(duration, LendSteam::from_code(steam)), out, error)
}

/// « Annuler le code ».
///
/// # Safety
/// Comme `kn_console_lend_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_lend_cancel_code(c: *const KnConsole, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.lend_cancel_code(), out, error)
}

/// « Accepter » (`accept`, avec `steam`) ou « Refuser » une demande
/// (`requests[].id` de la vue).
///
/// # Safety
/// Comme `kn_console_lend_state` ; `request` : chaîne C valable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_lend_answer(c: *const KnConsole, request: *const c_char, accept: bool, steam: i32, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.lend_answer(&text(request), accept, LendSteam::from_code(steam)), out, error)
}

/// « Reprendre l'ordinateur » (préavis de 2 min) ou, `now`, « Reprendre
/// tout de suite ».
///
/// # Safety
/// Comme `kn_console_lend_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_console_lend_take_back(c: *const KnConsole, now: bool, out: *mut *mut c_char, error: *mut *mut c_char) -> i32 {
    let Some(c) = (unsafe { c.as_ref() }) else { return KN_ERROR };
    console_result(c.0.lend_take_back(now), out, error)
}
