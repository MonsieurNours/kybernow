//! Les invitations pour l'app Android (docs/invitations.md) : ajouter un
//! ordinateur de loin avec le code et le mot de passe d'une invitation, sans
//! accord sur place. « Rejoindre avec un code » reconnaît le code (code 33 de
//! `shareJoinAnywhere` et `shareJoinLink`, `value` : l'`InviteCode`) ;
//! `inviteRedeem` envoie le mot de passe. Réponses en JSON comme les autres
//! appels : `{"code": 0, "value": …}` ou `{"code": …, "error": "…", "kind":
//! "…"}` ; refus (`InviteError::code`) : 60 mot de passe refusé, 61 code qui
//! ne sert plus, 62 invitation complète, 63 trop d'essais, 64 accès hors de
//! chez vous coupé, 65 le reste (Admin injoignable, relais muet…).

use jni::objects::{JClass, JString};
use jni::sys::jstring;

use kybernow_core::console::DeviceInfo;
use kybernow_core::i18n;
use kybernow_core::invite::{self, InviteCode, InviteError, Invited};

use crate::exports::{give, string, Env};

fn invite_envelope(r: Result<Invited, InviteError>, computer: &str) -> String {
    match r {
        Ok(v) => serde_json::json!({ "code": 0, "value": v }).to_string(),
        Err(e) => serde_json::json!({ "code": e.code(), "error": e.text(i18n::current(), computer), "kind": e.kind() }).to_string(),
    }
}

fn bad(e: impl std::fmt::Display) -> String {
    serde_json::json!({ "code": 1, "error": e.to_string() }).to_string()
}

/// « Ajouter l'ordinateur » (comme `kn_invite_redeem`) : `code` la `value`
/// du code 33 de « Rejoindre » (`{code, target, route, computer}`),
/// `password` celui de l'invitation (tirets, espaces et majuscules ne
/// comptent pas), `device` `{id, name, kind, os, app}`. `value` : `{hostname,
/// address, port, fingerprint, username, password, relay, invite_name,
/// expires, via_relay}`, à garder comme un ordinateur appairé. Bloquant (15 s
/// au plus), hors du fil principal.
#[unsafe(no_mangle)]
pub extern "system" fn Java_fr_leslignesclaires_kybernow_core_Native_inviteRedeem(mut env: Env, _: JClass, code: JString, password: JString, device: JString) -> jstring {
    kybernow_core::ffi_guard!(null => {
        let (code, password, device) = (string(&mut env, &code), string(&mut env, &password), string(&mut env, &device));
        let s = match (serde_json::from_str::<InviteCode>(&code), serde_json::from_str::<DeviceInfo>(&device)) {
            (Ok(c), Ok(d)) => invite_envelope(invite::redeem(&c, &password, &d), &c.computer),
            (Err(e), _) => bad(e),
            (_, Err(e)) => bad(e),
        };
        give(&mut env, &s)
    })
}
