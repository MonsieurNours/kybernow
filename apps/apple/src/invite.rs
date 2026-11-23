//! Les invitations pour les apps Apple (docs/invitations.md ; textes dans
//! docs/link-ui.md, « Invitations ») : « Rejoindre avec un code » est tombé
//! sur un code d'invitation (`KN_SHARE_INVITE`, son `out` donne la Station
//! et le chemin) ; l'app demande le mot de passe de l'invitation et ajoute
//! l'ordinateur avec `kn_invite_redeem`, sans que personne ait à l'accepter.
//! L'ordinateur ajouté se garde comme un ordinateur appairé par code.

use std::ffi::c_char;

use kybernow_core::console::DeviceInfo;
use kybernow_core::i18n;
use kybernow_core::invite::{self, InviteCode, InviteError};

use crate::{KN_ERROR, KN_OK, give_to, text};

/// Refus de `kn_invite_redeem` (`InviteError::code` du cœur ; `error` : le
/// texte à montrer sous le champ, docs/link-ui.md) : mot de passe refusé,
/// code qui ne sert plus, invitation qui a déjà ajouté tous ses appareils,
/// trop d'essais (« Trop d'essais : réessayez dans {n} min. »), accès hors
/// de chez vous coupé dans Admin (avec le nom de l'ordinateur), le reste
/// (Admin injoignable, relais muet, réponse illisible : le texte du cœur).
pub const KN_INVITE_BAD_PASSWORD: i32 = 60;
pub const KN_INVITE_NO_INVITE: i32 = 61;
pub const KN_INVITE_FULL: i32 = 62;
pub const KN_INVITE_TOO_MANY: i32 = 63;
pub const KN_INVITE_ACCESS_OFF: i32 = 64;
pub const KN_INVITE_OTHER: i32 = 65;

fn invite_failure(e: InviteError, computer: &str, error: *mut *mut c_char) -> i32 {
    give_to(error, e.text(i18n::current(), computer));
    e.code()
}

/// « Ajouter l'ordinateur » : `invite_code_json` est l'`out` de
/// `KN_SHARE_INVITE` (`{code, target, route, computer}`), tel quel ;
/// `password` : le mot de passe de l'invitation, tel que tapé (majuscules,
/// espaces et tirets ne comptent pas) ; `device_json` : l'appareil
/// (`{id, name, kind, os, app}`, comme pour l'appairage). `out` :
/// `{hostname, address, port, fingerprint, username, password, relay,
/// invite_name, expires, via_relay}`, à garder comme un ordinateur appairé
/// (adresse et port d'Admin, empreinte épinglée, compte au trousseau,
/// `relay` comme `info.relay`). Refus : `KN_INVITE_*`, le texte dans
/// `error` ; `KN_ERROR` pour un JSON illisible. Bloquant (15 s au plus).
///
/// # Safety
/// Chaînes C valables ; `out`, `error` valables ou nuls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_invite_redeem(
    invite_code_json: *const c_char,
    password: *const c_char,
    device_json: *const c_char,
    out: *mut *mut c_char,
    error: *mut *mut c_char,
) -> i32 {
    let code: InviteCode = match serde_json::from_str(&text(invite_code_json)) {
        Ok(c) => c,
        Err(e) => {
            give_to(error, e.to_string());
            return KN_ERROR;
        }
    };
    let device: DeviceInfo = match serde_json::from_str(&text(device_json)) {
        Ok(d) => d,
        Err(e) => {
            give_to(error, e.to_string());
            return KN_ERROR;
        }
    };
    match invite::redeem(&code, &text(password), &device) {
        Ok(invited) => {
            give_to(out, serde_json::to_string(&invited).unwrap_or_default());
            KN_OK
        }
        Err(e) => invite_failure(e, &code.computer, error),
    }
}
