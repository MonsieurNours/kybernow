//! Les liens `kybernow://` au lancement (docs/pret.md, docs/kybernow-share.md). webOS lance une app native avec ses
//! paramètres de lancement en JSON dans le premier argument : un lien d'invitation (`kybernow://borrow?…`) ou de
//! partage (`kybernow://join?…`) y est lu sous `target`, `contentTarget`, `link` ou `url` (par exemple
//! `ares-launch fr.leslignesclaires.kybernow -p '{"target":"kybernow://borrow?…"}'`). Un téléviseur n'a ni navigateur
//! ni appareil photo pour ouvrir un lien de lui-même : le chemin habituel reste « Ajouter un ordinateur » › « Avec un
//! compte d'ami ». L'app déjà ouverte n'est pas relancée avec un nouveau lien (webOS la remet devant sans le passer).

use kybernow_core::{lend, share};

use crate::app::{InviteUi, JoinLinkUi, LinkUi};

/// Les clés des paramètres de lancement qui peuvent porter le lien.
const KEYS: [&str; 4] = ["target", "contentTarget", "link", "url"];

fn is_link(s: &str) -> bool {
    s.trim().to_ascii_lowercase().starts_with("kybernow://")
}

/// Le lien des paramètres de lancement (le JSON du premier argument, ou un lien passé tel quel) ; `None` sans lien.
pub fn from_args(args: &[String]) -> Option<String> {
    for a in args.iter().skip(1) {
        if is_link(a) {
            return Some(a.trim().to_string());
        }
        let Ok(serde_json::Value::Object(params)) = serde_json::from_str::<serde_json::Value>(a) else { continue };
        if let Some(l) = KEYS.iter().filter_map(|k| params.get(*k).and_then(|v| v.as_str())).find(|v| is_link(v)) {
            return Some(l.trim().to_string());
        }
    }
    None
}

/// Ce que l'app fait d'un lien : une invitation, un partage, ou une invitation illisible ; `None` pour le reste.
pub fn parse(url: &str) -> Option<LinkUi> {
    if let Some(i) = lend::parse_invite(url) {
        return Some(LinkUi::Invite(InviteUi { address: i.host, port: i.port, fingerprint: i.fingerprint, login: i.login, relay: i.relay, sid: i.sid, name: i.name }));
    }
    if url.trim().to_ascii_lowercase().starts_with("kybernow://borrow") {
        return Some(LinkUi::BadInvite);
    }
    let l = share::parse_link(url)?;
    Some(LinkUi::Join(JoinLinkUi { address: l.host, port: l.port, fingerprint: l.fingerprint, code: l.code, relay: l.relay, sid: l.sid }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_from_the_launch_parameters() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_args(&args(&["kybernow-webos", r#"{"@system_native_app":true}"#])), None);
        let l = from_args(&args(&["kybernow-webos", r#"{"@system_native_app":true,"target":"kybernow://borrow?host=h&port=1&login=lea&name=Hugo"}"#])).unwrap();
        match parse(&l) {
            Some(LinkUi::Invite(i)) => assert_eq!((i.login.as_str(), i.name.as_deref()), ("lea", Some("Hugo"))),
            other => panic!("invitation attendue : {other:?}"),
        }
        assert_eq!(from_args(&args(&["kybernow-webos", r#"{"contentTarget":"kybernow://join?host=h&port=1&code=482917"}"#])).as_deref(), Some("kybernow://join?host=h&port=1&code=482917"));
        assert_eq!(parse("kybernow://borrow?host=h"), Some(LinkUi::BadInvite));
    }
}
