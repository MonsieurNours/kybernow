//! « Envoyer le journal » par QR code (Apple TV ; docs/link-ui.md, « Réglages
//! de l'app », et docs/relais.md, « Dépôt d'un journal ») : le pont C de
//! `kybernow_core::journal` (compression, chiffrement `KNJ1`, dépôt sur le
//! relais par son agent HTTPS, empreinte épinglée comprise ; suivi et
//! retrait). L'app rassemble l'en-tête et le journal (`AppJournal.swift`).

use std::ffi::c_char;

use kybernow_core::journal::{self, DepositError, Remote};

use crate::{KN_ERROR, KN_OK, give_to, text};

/// Le relais n'a pas répondu (réseau, nom, certificat).
pub const KN_JOURNAL_UNREACHABLE: i32 = 90;
/// Relais d'avant le dépôt des journaux (`404` sur `POST /v1/journal`).
pub const KN_JOURNAL_OLD_RELAY: i32 = 91;
/// Trop de dépôts depuis cette adresse (`429`), ou relais plein (`503`).
pub const KN_JOURNAL_TOO_MANY: i32 = 92;
/// Le dépôt n'est plus là : ouvert puis effacé, expiré (une heure), retiré.
pub const KN_JOURNAL_GONE: i32 = 93;
/// États d'un dépôt (`kn_journal_state`).
pub const KN_JOURNAL_WAITING: i32 = 0;
pub const KN_JOURNAL_OPENED: i32 = 1;

/// Le code C d'un refus du cœur.
fn refused(e: &DepositError) -> i32 {
    match e {
        DepositError::Unreachable(_) => KN_JOURNAL_UNREACHABLE,
        DepositError::OldRelay => KN_JOURNAL_OLD_RELAY,
        DepositError::TooMany => KN_JOURNAL_TOO_MANY,
        DepositError::Failed(_) => KN_ERROR,
    }
}

/// L'état C d'un dépôt.
fn state_code(r: Remote) -> i32 {
    match r {
        Remote::Waiting => KN_JOURNAL_WAITING,
        Remote::Opened => KN_JOURNAL_OPENED,
        Remote::Gone => KN_JOURNAL_GONE,
        Remote::Unknown => KN_JOURNAL_UNREACHABLE,
    }
}

/// Dépose le journal sur le relais (`relay` : celui du compte KyberNow de
/// l'appareil ; vide : le relais public). `header` : appareil, système,
/// version, fins précédentes ; `body` : le journal de l'app, du plus ancien
/// au plus récent. `out` : `{relay, id, owner, url, expires_in}` (`url` :
/// l'adresse du QR code, clé comprise ; `owner` : le jeton pour suivre et
/// retirer le dépôt). Refus : `KN_JOURNAL_*`, `KN_ERROR` (le détail dans
/// `error`). Bloquant (30 s au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_upload(
    relay_addr: *const c_char,
    header: *const c_char,
    body: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    kybernow_core::ffi_guard!(KN_ERROR => {
        match journal::deposit(&text(relay_addr), &text(header), &text(body)) {
            Ok(d) => {
                give_to(out, d.to_json());
                KN_OK
            }
            Err(e) => {
                give_to(error, e.detail());
                refused(&e)
            }
        }
    })
}

/// Où en est un dépôt (l'écran du QR code le relit toutes les 2 s) :
/// `KN_JOURNAL_WAITING`, `KN_JOURNAL_OPENED` (le téléphone l'a ouvert),
/// `KN_JOURNAL_GONE` (expiré), `KN_JOURNAL_UNREACHABLE`. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_state(relay_addr: *const c_char, id: *const c_char, owner: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(KN_JOURNAL_UNREACHABLE => {
        state_code(journal::state(&text(relay_addr), &text(id), &text(owner)))
    })
}

/// « Terminé » : retire un dépôt pas encore ouvert (sans effet sinon).
/// `KN_OK`, ou `KN_JOURNAL_UNREACHABLE`. Bloquant.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_journal_delete(relay_addr: *const c_char, id: *const c_char, owner: *const c_char) -> i32 {
    kybernow_core::ffi_guard!(KN_JOURNAL_UNREACHABLE => {
        if journal::discard(&text(relay_addr), &text(id), &text(owner)) { KN_OK } else { KN_JOURNAL_UNREACHABLE }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les codes C sont ceux du cœur (`DepositError::code`, `Remote::code`) :
    /// Android les reçoit du cœur, Swift d'ici.
    #[test]
    fn codes_comme_le_coeur() {
        for e in [DepositError::Unreachable(String::new()), DepositError::OldRelay, DepositError::TooMany, DepositError::Failed(String::new())] {
            assert_eq!(refused(&e), e.code());
        }
        for r in [Remote::Waiting, Remote::Opened, Remote::Gone, Remote::Unknown] {
            assert_eq!(state_code(r), r.code());
        }
    }
}
