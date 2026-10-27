//! Liens `kybernow://` des notifications : `kybernow://connect?id=…`,
//! `kybernow://open[?id=…]`, `kybernow://station?id=…`,
//! `kybernow://station-update?id=…`. Windows les rend parfois avec une barre
//! avant le « ? » (`kybernow://connect/?id=…`). Le lien d'un partage
//! (`kybernow://join?host=…&port=…&fp=…&code=…`, KyberNow Share) est une
//! demande à part ([`is_join`]). Les autres liens (`kybernow://adresse:port?…`,
//! ceux d'Admin) ne sont pas des actions. Sans rien de Windows : testé sur le Mac.

/// L'action d'un lien de notification et l'ordinateur qu'il désigne.
pub fn action(link: &str) -> Option<(&str, Option<String>)> {
    let rest = link.strip_prefix("kybernow://")?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = path.trim_end_matches('/');
    if !["connect", "open", "station", "station-update"].contains(&path) {
        return None;
    }
    let id = query
        .split('&')
        .find_map(|p| p.strip_prefix("id="))
        .map(|v| v.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>())
        .filter(|v| !v.is_empty());
    Some((path, id))
}

/// Le lien d'un partage (`kybernow://join?…`) : « Rejoindre avec un code ».
/// Windows le rend parfois avec une barre avant le « ? ». Ce que le lien dit
/// (adresse, empreinte, code) est lu par `kybernow_core::share::parse_link`.
pub fn is_join(link: &str) -> bool {
    const SCHEME: &str = "kybernow://";
    let link = link.trim();
    if !link.get(..SCHEME.len()).is_some_and(|p| p.eq_ignore_ascii_case(SCHEME)) {
        return false;
    }
    let rest = &link[SCHEME.len()..];
    let path = rest.split_once('?').map_or(rest, |(p, _)| p).trim_end_matches('/');
    path.eq_ignore_ascii_case("join")
}

#[cfg(test)]
mod tests {
    use super::{action, is_join};

    #[test]
    fn liens() {
        assert_eq!(action("kybernow://connect?id=a1b2"), Some(("connect", Some("a1b2".into()))));
        // Comme Windows les rend aux boutons des notifications.
        assert_eq!(action("kybernow://station-update/?id=e55a1a7e11e40001"), Some(("station-update", Some("e55a1a7e11e40001".into()))));
        assert_eq!(action("kybernow://station/?id=x9"), Some(("station", Some("x9".into()))));
        assert_eq!(action("kybernow://open"), Some(("open", None)));
        assert_eq!(action("kybernow://open/"), Some(("open", None)));
        assert_eq!(action("kybernow://open?id=../../x"), Some(("open", Some("x".into()))));
        // Un lien d'Admin n'est pas une action.
        assert_eq!(action("kybernow://192.168.1.100:31544?name=Bureau&fp=sha256:aa"), None);
        assert_eq!(action("https://exemple.fr"), None);
        // Le lien d'un partage n'est pas une action de notification.
        assert_eq!(action("kybernow://join?host=192.168.1.100&port=9443&code=123456"), None);
    }

    #[test]
    fn lien_de_partage() {
        assert!(is_join("kybernow://join?host=192.168.1.100&port=9443&fp=sha256:aa&code=123456"));
        assert!(is_join("kybernow://join/?host=192.168.1.100&port=9443&code=123456"));
        assert!(is_join("  KYBERNOW://Join?code=1  "));
        // Illisible, mais bien un lien de partage : l'app le dit à l'ouverture.
        assert!(is_join("kybernow://join"));
        assert!(!is_join("kybernow://joint?x=1"));
        assert!(!is_join("kybernow://192.168.1.100:31544?name=Bureau&fp=sha256:aa"));
        assert!(!is_join("kybernow://connect?id=a1"));
        assert!(!is_join("https://exemple.fr/join"));
        assert!(!is_join(""));
    }
}
