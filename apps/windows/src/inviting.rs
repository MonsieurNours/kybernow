//! Invitations (docs/invitations.md, textes de docs/link-ui.md « Invitations »),
//! ce que l'app en tire sans rien de Windows, pour se tester sur le Mac :
//! l'ordinateur de l'accueil que l'invitation met à jour (même empreinte d'Admin)
//! plutôt que d'en ajouter un second, le nom de l'ordinateur ajouté, et les
//! réponses de `invite_redeem` telles que l'interface les lit (`api.ts`). Les
//! appels au cœur et l'ordinateur gardé sont dans `ui/invite.rs`.

use kybernow_core::i18n::Lang;
use kybernow_core::invite::{InviteError, Invited};
use serde::Serialize;

use crate::sharing::Known;

/// L'ordinateur de l'accueil qui est celui de l'invitation : le même Admin
/// (même empreinte) d'abord, où qu'il soit ; sinon, comme pour un ordinateur
/// ajouté par code, la même adresse et le même port. Il prend le nouvel accès au
/// lieu d'être ajouté deux fois.
pub fn same_computer<'a>(saved: impl IntoIterator<Item = &'a Known>, invited: &Invited) -> Option<usize> {
    let saved: Vec<&Known> = saved.into_iter().collect();
    let fingerprint = invited.fingerprint.as_deref().map(str::trim).filter(|f| !f.is_empty());
    if let Some(fp) = fingerprint
        && let Some(i) = saved.iter().position(|k| k.fingerprint.as_deref().is_some_and(|f| f.trim().eq_ignore_ascii_case(fp)))
    {
        return Some(i);
    }
    saved.iter().position(|k| k.host.eq_ignore_ascii_case(&invited.address) && k.port == invited.port)
}

/// Le nom de l'ordinateur ajouté : celui que dit la Station (le cœur reprend
/// sinon celui du code), à défaut son adresse.
pub fn name_of(invited: &Invited) -> String {
    match invited.hostname.trim() {
        "" => invited.address.clone(),
        name => name.to_string(),
    }
}

/// Réponse de `invite_redeem` : l'ordinateur ajouté (ou mis à jour), que
/// l'accueil sélectionne avec l'avis « {ordinateur} est ajouté. ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InviteAdded {
    pub id: String,
    pub name: String,
}

/// Refus de `invite_redeem`, montré sous le champ : `kind` (`bad_password`,
/// `no_invite`, `full`, `too_many`, `access_off`, `other`) et le texte du cœur
/// dans la langue de l'app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InviteRefusal {
    pub kind: &'static str,
    pub message: String,
}

impl InviteRefusal {
    /// `computer` : le nom de l'ordinateur (accès hors de chez vous coupé).
    pub fn of(e: &InviteError, lang: Lang, computer: &str) -> Self {
        Self { kind: e.kind(), message: e.text(lang, computer) }
    }

    /// Un refus de l'app elle-même (ticket inconnu, mot de passe pas rangé).
    pub fn other(message: String) -> Self {
        Self { kind: "other", message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(host: &str, port: u16, fp: Option<&str>) -> Known {
        Known { host: host.into(), port, fingerprint: fp.map(str::to_string) }
    }

    fn invited(address: &str, port: u16, fp: Option<&str>) -> Invited {
        Invited { hostname: "PC du salon".into(), address: address.into(), port, fingerprint: fp.map(str::to_string), username: "PORTABLE-ab12".into(), password: "secret".into(), ..Invited::default() }
    }

    #[test]
    fn meme_admin_meme_ordinateur() {
        let saved = [known("192.168.1.20", 47990, Some("sha256:bb")), known("10.0.0.5", 47990, Some("sha256:AA"))];
        // Même empreinte, ailleurs (ajouté de loin, adresse de son réseau) : c'est lui.
        assert_eq!(same_computer(&saved, &invited("192.168.50.2", 9443, Some("sha256:aa"))), Some(1));
        // Sans empreinte connue : la même adresse et le même port.
        assert_eq!(same_computer(&saved, &invited("192.168.1.20", 47990, None)), Some(0));
        // L'empreinte passe avant l'adresse.
        assert_eq!(same_computer(&saved, &invited("192.168.1.20", 47990, Some("sha256:aa"))), Some(1));
        // Ni l'un ni l'autre : un ordinateur de plus.
        assert_eq!(same_computer(&saved, &invited("192.168.1.30", 47990, Some("sha256:cc"))), None);
        assert_eq!(same_computer(&saved, &invited("192.168.1.20", 9443, Some(""))), None);
    }

    #[test]
    fn nom_de_l_ordinateur() {
        assert_eq!(name_of(&invited("10.0.0.5", 47990, None)), "PC du salon");
        let mut i = invited("10.0.0.5", 47990, None);
        i.hostname = " ".into();
        assert_eq!(name_of(&i), "10.0.0.5");
    }

    /// Les réponses de `invite_redeem` sont celles de `api.ts` (`InviteAdded`, `InviteRefusal`).
    #[test]
    fn reponses_d_ajouter() {
        let added = InviteAdded { id: "a1b2".into(), name: "PC du salon".into() };
        assert_eq!(serde_json::to_value(&added).unwrap(), serde_json::json!({ "id": "a1b2", "name": "PC du salon" }));
        let r = InviteRefusal::of(&InviteError::BadPassword, Lang::Fr, "PC du salon");
        assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({ "kind": "bad_password", "message": "Mot de passe refusé." }));
        let r = InviteRefusal::of(&InviteError::AccessOff, Lang::Fr, "PC du salon");
        assert_eq!((r.kind, r.message.as_str()), ("access_off", "PC du salon n'est pas joignable hors de chez vous (accès coupé dans Admin)."));
        assert_eq!(InviteRefusal::of(&InviteError::TooManyTries { seconds: Some(61) }, Lang::En, "").message, "Too many attempts: try again in 2 min.");
        let api = include_str!("../ui/src/lib/api.ts");
        for name in ["kind: 'invite'", "'bad_password'", "'no_invite'", "'full'", "'too_many'", "'access_off'"] {
            assert!(api.contains(name), "{name}");
        }
    }
}
