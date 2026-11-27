//! Comptes KyberNow (docs/comptes.md, lot 3 ; textes de docs/link-ui.md
//! « Comptes KyberNow »), ce que l'app en tire sans rien de Windows, pour se
//! tester sur le Mac : l'identifiant chez l'app d'un ordinateur d'un compte,
//! le nom d'un compte tel qu'on le montre, la liste des ordinateurs d'un
//! relevé (un relais muet garde ce qu'on savait, une session d'appareil
//! fermée oublie le compte), les pastilles d'origine de l'accueil, et ce que
//! l'interface lit (`api.ts`). Les appels au cœur, le gestionnaire
//! d'identification et l'accueil sont dans `ui/account.rs` et `store.rs`.

use std::collections::HashMap;

use kybernow_core::account::{self, Account, AccountComputer, AccountError, Paired};
use kybernow_core::i18n::Lang;
use serde::Serialize;

/// L'identifiant chez l'app d'un ordinateur d'un compte qui n'est pas appairé
/// sur ce PC : le même pour le même relais et la même Station, d'un relevé à
/// l'autre (ses réglages, sa dernière session et son image restent), seize
/// signes hexadécimaux comme ceux de `store::new_id` (FNV-1a sur 64 bits).
pub fn computer_id(relay: &str, station: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in relay.bytes().chain(std::iter::once(b'|')).chain(station.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Le compte tel qu'on le nomme (« Connecté à {compte}. », « Se déconnecter
/// de {compte} ? ») : le nom, sinon l'e-mail, sinon l'identifiant.
pub fn account_name(a: &Account) -> String {
    [&a.account.name, &a.account.email, &a.account.id].into_iter().map(|s| s.trim()).find(|s| !s.is_empty()).unwrap_or_default().to_string()
}

/// Des comptes sur plusieurs relais : la pastille dit alors lequel
/// (« Compte · {relais} »).
pub fn several_relays(accounts: &[Account]) -> bool {
    accounts.iter().any(|a| a.relay != accounts[0].relay)
}

/// Un compte dans les réglages de l'app (« Comptes ») : jamais son jeton.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AccountView {
    /// `Account::key` : « Gérer le compte », « Se déconnecter ».
    pub key: String,
    /// Le nom, sinon l'e-mail (titre de la ligne, confirmation, avis).
    pub name: String,
    pub email: String,
    /// Le relais tel qu'on le montre (`share.kybernow.app`).
    pub relay: String,
}

impl AccountView {
    pub fn of(a: &Account) -> Self {
        let s = a.summary();
        Self { key: s.key, name: account_name(a), email: s.account.email, relay: s.relay_label }
    }
}

/// La fin d'une connexion par la fenêtre du système (événement
/// `account-sign-in`) : connecté (l'avis « Connecté à {compte}. »), refusée
/// (le texte du cœur), ou annulée (rien : l'accueil reste tel quel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignInOutcome {
    SignedIn { account: String },
    Failed { message: String },
    Cancelled,
}

impl SignInOutcome {
    pub fn of(r: &Result<Account, AccountError>, lang: Lang) -> Self {
        match r {
            Ok(a) => Self::SignedIn { account: account_name(a) },
            Err(AccountError::Cancelled) => Self::Cancelled,
            Err(e) => Self::Failed { message: e.text(lang) },
        }
    }
}

/// Un relevé des comptes (`account::computers` pour chacun, `key` le compte) :
/// la nouvelle liste, et les comptes dont la session d'appareil est fermée
/// (à oublier). Un relais muet ou une réponse inattendue gardent les
/// ordinateurs déjà connus de ce compte (l'accueil ne se vide pas faute de
/// réseau) ; les empreintes d'Admin déjà vues restent
/// (`account::remember_fingerprints`).
pub fn gather(known: &[AccountComputer], fetched: Vec<(String, Result<Vec<AccountComputer>, AccountError>)>) -> (Vec<AccountComputer>, Vec<String>) {
    let mut fresh = Vec::new();
    let mut signed_out = Vec::new();
    for (key, r) in fetched {
        match r {
            Ok(list) => fresh.extend(list),
            Err(AccountError::SignedOut) => signed_out.push(key),
            Err(_) => fresh.extend(known.iter().filter(|c| c.account == key).cloned()),
        }
    }
    account::remember_fingerprints(&mut fresh, known);
    (fresh, signed_out)
}

/// Les pastilles d'origine de l'accueil, par identifiant d'ordinateur,
/// seulement quand un compte est connecté : un ordinateur d'un compte, son
/// origine (« Compte », « Compte · {relais} », l'organisation, « Ouvert par
/// {prénom} ») ; un appairé que le compte a aussi, celle du compte ; un
/// appairé seul, « Appairé ».
pub fn origins(accounts: &[Account], paired: &[Paired], from_accounts: &[(String, AccountComputer)], list: &[AccountComputer], lang: Lang) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if accounts.is_empty() {
        return out;
    }
    let several = several_relays(accounts);
    let linked = account::merge(paired, list).linked;
    for p in paired {
        let origin = match linked.iter().find(|(id, _)| id == &p.id) {
            Some((_, c)) => c.origin(lang, several),
            None => account::origin_paired(lang),
        };
        out.insert(p.id.clone(), origin);
    }
    for (id, c) in from_accounts {
        out.insert(id.clone(), c.origin(lang, several));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::account::{AccountDevice, AccountUser, Owner};

    fn compte(relay: &str, id: &str, name: &str, email: &str) -> Account {
        Account {
            relay: relay.into(),
            token: "jeton".into(),
            account: AccountUser { id: id.into(), name: name.into(), email: email.into() },
            device: AccountDevice { id: "dev_1".into(), name: "PORTABLE".into() },
        }
    }

    fn ordinateur(account: &Account, station: &str, fp: Option<&str>) -> AccountComputer {
        AccountComputer {
            account: account.key(),
            relay: account.relay.clone(),
            station: station.into(),
            name: format!("PC {station}"),
            rights: vec!["session".into()],
            owner: Owner { kind: "person".into(), id: account.account.id.clone(), name: account.account.name.clone() },
            mine: true,
            online: true,
            admin_fp: fp.map(str::to_string),
            attached_at: 0,
        }
    }

    #[test]
    fn identifiant_stable() {
        let a = computer_id("share.kybernow.app", "abcdefghijklmnopqrstuvwxyz");
        assert_eq!(a, computer_id("share.kybernow.app", "abcdefghijklmnopqrstuvwxyz"));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        // Deux relais ont leurs propres Stations.
        assert_ne!(a, computer_id("relais.exemple.org:9443", "abcdefghijklmnopqrstuvwxyz"));
        assert_ne!(a, computer_id("share.kybernow.app", "bbcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn nom_du_compte() {
        assert_eq!(account_name(&compte("share.kybernow.app", "acc_1", "Léa Martin", "lea@exemple.fr")), "Léa Martin");
        assert_eq!(account_name(&compte("share.kybernow.app", "acc_1", " ", "lea@exemple.fr")), "lea@exemple.fr");
        assert_eq!(account_name(&compte("share.kybernow.app", "acc_1", "", "")), "acc_1");
        let v = AccountView::of(&compte("relais.exemple.org:9443#sha256:aa", "acc_1", "", "lea@exemple.fr"));
        assert_eq!((v.name.as_str(), v.relay.as_str()), ("lea@exemple.fr", "relais.exemple.org:9443"));
        // Jamais le jeton dans ce que l'interface lit.
        assert!(!serde_json::to_string(&v).unwrap().contains("jeton"));
    }

    #[test]
    fn issue_de_la_connexion() {
        let a = compte("share.kybernow.app", "acc_1", "Léa", "");
        assert_eq!(serde_json::to_value(SignInOutcome::of(&Ok(a), Lang::Fr)).unwrap(), serde_json::json!({ "kind": "signed_in", "account": "Léa" }));
        assert_eq!(serde_json::to_value(SignInOutcome::of(&Err(AccountError::Cancelled), Lang::Fr)).unwrap(), serde_json::json!({ "kind": "cancelled" }));
        assert_eq!(
            serde_json::to_value(SignInOutcome::of(&Err(AccountError::BadCode), Lang::Fr)).unwrap(),
            serde_json::json!({ "kind": "failed", "message": "La connexion n'a pas abouti\u{202f}: réessayez." })
        );
        // `api.ts` dit les mêmes noms.
        let api = include_str!("../ui/src/lib/api.ts");
        for name in ["kind: 'signed_in'", "kind: 'failed'", "kind: 'cancelled'"] {
            assert!(api.contains(name), "{name}");
        }
    }

    #[test]
    fn releve() {
        let lea = compte("share.kybernow.app", "acc_1", "Léa", "");
        let tom = compte("relais.exemple.org:9443", "acc_2", "Tom", "");
        let parti = compte("share.kybernow.app", "acc_3", "Ana", "");
        let known = vec![ordinateur(&lea, "s1", Some("sha256:aa")), ordinateur(&tom, "s2", Some("sha256:bb")), ordinateur(&parti, "s3", None)];
        let (fresh, signed_out) = gather(
            &known,
            vec![
                // Station déconnectée : le relais ne donne plus son empreinte, gardée.
                (lea.key(), Ok(vec![ordinateur(&lea, "s1", None), ordinateur(&lea, "s4", None)])),
                // Relais muet : ce qu'on savait reste.
                (tom.key(), Err(AccountError::Unreachable("délai".into()))),
                (parti.key(), Err(AccountError::SignedOut)),
            ],
        );
        assert_eq!(signed_out, vec![parti.key()]);
        let stations: Vec<&str> = fresh.iter().map(|c| c.station.as_str()).collect();
        assert_eq!(stations, ["s1", "s4", "s2"]);
        assert_eq!(fresh[0].admin_fp.as_deref(), Some("sha256:aa"));
        assert_eq!(fresh[2].admin_fp.as_deref(), Some("sha256:bb"));
    }

    #[test]
    fn pastilles() {
        let lea = compte("share.kybernow.app", "acc_1", "Léa", "");
        let list = vec![ordinateur(&lea, "s1", Some("sha256:aa")), ordinateur(&lea, "s2", None)];
        let paired = vec![
            Paired { id: "p1".into(), fingerprint: Some("SHA256:AA".into()), station: None },
            Paired { id: "p2".into(), fingerprint: Some("sha256:cc".into()), station: None },
        ];
        let from_accounts = vec![(computer_id(&lea.relay, "s2"), list[1].clone())];
        // Sans compte : aucune pastille.
        assert!(origins(&[], &paired, &[], &list, Lang::Fr).is_empty());
        let o = origins(std::slice::from_ref(&lea), &paired, &from_accounts, &list, Lang::Fr);
        assert_eq!(o["p1"], "Compte");
        assert_eq!(o["p2"], "Appairé");
        assert_eq!(o[&computer_id(&lea.relay, "s2")], "Compte");
        // Des comptes sur deux relais : le relais dans la pastille.
        let tom = compte("relais.exemple.org:9443", "acc_2", "Tom", "");
        let o = origins(&[lea.clone(), tom], &paired, &from_accounts, &list, Lang::En);
        assert_eq!(o["p1"], "Account · share.kybernow.app");
        assert!(several_relays(&[lea.clone(), compte("relais.exemple.org:9443", "acc_9", "", "")]));
        assert!(!several_relays(&[lea.clone(), compte("share.kybernow.app", "acc_9", "", "")]));
    }
}
