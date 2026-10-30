//! Liens `kybernow://` des notifications : `kybernow://connect?id=…`,
//! `kybernow://open[?id=…]`, `kybernow://share?id=…` (une demande à rejoindre : le
//! panneau de « Partager » de cette session), `kybernow://lend?id=…` (une demande
//! d'emprunt : le panneau de « Prêter l'ordinateur… »), `kybernow://station?id=…`,
//! `kybernow://station-update?id=…`. Windows les rend parfois avec une barre
//! avant le « ? » (`kybernow://connect/?id=…`). Le lien d'un partage
//! (`kybernow://join?host=…&port=…&fp=…&code=…`, KyberNow Share, avec
//! `&relay=…&sid=…` quand l'hôte est joignable hors de chez lui) est une
//! demande à part ([`is_join`]), comme le lien d'invitation d'un compte d'ami
//! (`kybernow://borrow?host=…&port=…&fp=…&login=…`, prêt de l'ordinateur,
//! [`is_borrow`]). Les autres liens (`kybernow://adresse:port?…`,
//! ceux d'Admin) ne sont pas des actions. Sans rien de Windows : testé sur le Mac.

/// L'action d'un lien de notification et l'ordinateur qu'il désigne.
pub fn action(link: &str) -> Option<(&str, Option<String>)> {
    let rest = link.strip_prefix("kybernow://")?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let path = path.trim_end_matches('/');
    if !["connect", "open", "share", "lend", "station", "station-update"].contains(&path) {
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
/// (adresse, empreinte, code, et le relais avec l'identifiant de la Station
/// hors de chez soi) est lu par `kybernow_core::share::parse_link`.
pub fn is_join(link: &str) -> bool {
    is_path(link, "join")
}

/// Le lien d'invitation d'un compte d'ami (`kybernow://borrow?…`, prêt de
/// l'ordinateur) : l'app ajoute « Ordinateur de {hôte} » à son accueil. Ce que le
/// lien dit est lu par `kybernow_core::lend::parse_invite`.
pub fn is_borrow(link: &str) -> bool {
    is_path(link, "borrow")
}

fn is_path(link: &str, name: &str) -> bool {
    const SCHEME: &str = "kybernow://";
    let link = link.trim();
    if !link.get(..SCHEME.len()).is_some_and(|p| p.eq_ignore_ascii_case(SCHEME)) {
        return false;
    }
    let rest = &link[SCHEME.len()..];
    let path = rest.split_once('?').map_or(rest, |(p, _)| p).trim_end_matches('/');
    path.eq_ignore_ascii_case(name)
}

#[cfg(test)]
mod tests {
    use super::{action, is_borrow, is_join};

    #[test]
    fn liens() {
        assert_eq!(action("kybernow://connect?id=a1b2"), Some(("connect", Some("a1b2".into()))));
        // Comme Windows les rend aux boutons des notifications.
        assert_eq!(action("kybernow://station-update/?id=e55a1a7e11e40001"), Some(("station-update", Some("e55a1a7e11e40001".into()))));
        assert_eq!(action("kybernow://station/?id=x9"), Some(("station", Some("x9".into()))));
        // Une demande à rejoindre : le panneau de « Partager » de cette session (pas le lien d'un partage).
        assert_eq!(action("kybernow://share?id=e55a1a7e11e40001"), Some(("share", Some("e55a1a7e11e40001".into()))));
        assert_eq!(action("kybernow://share/?id=x9"), Some(("share", Some("x9".into()))));
        assert!(!is_join("kybernow://share?id=x9"));
        // Une demande d'emprunt : le panneau de « Prêter l'ordinateur… » de cette session.
        assert_eq!(action("kybernow://lend?id=x9"), Some(("lend", Some("x9".into()))));
        assert!(!is_borrow("kybernow://lend?id=x9"));
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
        // Hors de chez soi : le relais et l'identifiant de la Station (le relais garde son `#sha256:…`, encodé).
        assert!(is_join("kybernow://join?host=192.168.1.100&port=9443&fp=sha256%3Aaa&code=123456&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz"));
        // Illisible, mais bien un lien de partage : l'app le dit à l'ouverture.
        assert!(is_join("kybernow://join"));
        assert!(!is_join("kybernow://joint?x=1"));
        assert!(!is_join("kybernow://192.168.1.100:31544?name=Bureau&fp=sha256:aa"));
        assert!(!is_join("kybernow://connect?id=a1"));
        assert!(!is_join("https://exemple.fr/join"));
        assert!(!is_join(""));
    }

    #[test]
    fn lien_d_invitation() {
        assert!(is_borrow("kybernow://borrow?host=192.168.1.100&port=9443&fp=sha256:aa&login=lea"));
        assert!(is_borrow("kybernow://borrow/?host=192.168.1.100&port=9443&login=lea"));
        assert!(!is_join("kybernow://borrow?host=192.168.1.100&port=9443&login=lea"));
        assert!(!is_borrow("kybernow://join?code=123456"));
        // Ni une action de notification.
        assert_eq!(action("kybernow://borrow?host=a&port=1&login=lea"), None);
    }
}
