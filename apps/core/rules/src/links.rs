//! Reconnaître un lien avant de le lire (COEUR-41) : un lien qui a la forme
//! d'une invitation mais ne se lit pas doit dire « Ce lien d'invitation est
//! illisible. » au lieu d'être ignoré.

/// Le texte a la forme d'un lien d'invitation à emprunter un ordinateur :
/// `kybernow://borrow…`, ou une page `…/link?…borrow=…` du client web (en
/// `https://`). Ne dit pas qu'il se lit (`lend::parse_invite` du cœur).
pub fn looks_like_invite(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    if t.starts_with("kybernow://borrow") {
        return true;
    }
    let Some(rest) = t.strip_prefix("https://") else { return false };
    let Some((path, query)) = rest.split_once('?') else { return false };
    path.contains("/link") && query.split('#').next().unwrap_or_default().split('&').any(|p| p.starts_with("borrow="))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liens_d_invitation() {
        assert!(looks_like_invite("kybernow://borrow?host=pc&port=9443&login=ami"));
        assert!(looks_like_invite(" KYBERNOW://BORROW?x "));
        assert!(looks_like_invite("https://192.168.1.20:9443/link/?borrow=ami&name=PC"));
        assert!(looks_like_invite("https://[fe80::1]:9443/link?name=PC&borrow="));
        assert!(!looks_like_invite("https://192.168.1.20:9443/link/?join=123456"));
        assert!(!looks_like_invite("http://pc/link?borrow=ami"));
        assert!(!looks_like_invite("https://pc/autre?borrow=ami"));
        assert!(!looks_like_invite("kybernow://join?code=123456"));
        assert!(!looks_like_invite(""));
    }
}
