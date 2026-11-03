//! Identifiant du compte PSN, lu une seule fois par la connexion au compte
//! Sony, comme chiaki-ng (`scripts/psn-account-id.py`,
//! `gui/src/psnaccountid.cpp`). Le compte ne sert à rien d'autre : pas de
//! connexion par les serveurs de Sony pendant le jeu.
//!
//! 1. L'app ouvre [`LOGIN_URL`] dans une fenêtre web ; la personne se connecte
//!    sur la page de Sony.
//! 2. Sony renvoie vers [`REDIRECT_URL`]`?code=…` : l'app intercepte cette
//!    adresse (ou, dans le terminal, on la colle) et en tire le code
//!    ([`code_from_redirect`]).
//! 3. [`account_id`] échange le code contre un jeton, puis lit le `user_id`
//!    du compte : c'est l'identifiant ([`PsnId::from_user_id`]) ; la même
//!    réponse donne le pseudo (`online_id`), pour nommer le compte.
//!
//! Les identifiants de client sont ceux de l'app de lecture à distance de
//! Sony pour Windows, repris de chiaki-ng.

use base64::Engine;

use super::regist::PsnId;

const CLIENT_ID: &str = "ba495a24-818c-472b-b12d-ff231c1b5745";
const CLIENT_SECRET: &str = "mvaiZkRsAsI1IBkY";

/// Adresse vers laquelle Sony renvoie après la connexion, avec `?code=`.
pub const REDIRECT_URL: &str = "https://remoteplay.dl.playstation.net/remoteplay/redirect";

const TOKEN_URL: &str = "https://auth.api.sonyentertainmentnetwork.com/2.0/oauth/token";

/// Page de connexion au compte Sony.
pub const LOGIN_URL: &str = "https://auth.api.sonyentertainmentnetwork.com/2.0/oauth/authorize?service_entity=urn:service-entity:psn&response_type=code&client_id=ba495a24-818c-472b-b12d-ff231c1b5745&redirect_uri=https://remoteplay.dl.playstation.net/remoteplay/redirect&scope=psn:clientapp&request_locale=en_US&ui=pr&service_logo=ps&layout_type=popup&smcid=remoteplay&prompt=always&PlatformPrivacyWs1=minimal&";

/// Pourquoi l'identifiant n'a pas pu être lu.
#[derive(Debug)]
pub enum PsnError {
    /// L'adresse de retour ne contient pas de `code`.
    NoCode,
    /// Sony a refusé l'échange (code expiré, déjà utilisé…).
    Refused(String),
    Network(String),
}

impl std::fmt::Display for PsnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsnError::NoCode => write!(f, "l'adresse de retour de Sony ne contient pas de code"),
            PsnError::Refused(why) => write!(f, "Sony refuse : {why}"),
            PsnError::Network(why) => write!(f, "réseau : {why}"),
        }
    }
}

impl std::error::Error for PsnError {}

/// Vrai pour l'adresse de retour de Sony : l'app intercepte la navigation
/// vers elle au lieu de l'afficher.
pub fn is_redirect(url: &str) -> bool {
    url.starts_with(REDIRECT_URL)
}

/// Le `code` de l'adresse de retour.
pub fn code_from_redirect(url: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == "code" && !v.is_empty()).then(|| percent_decode(v))
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn basic_auth() -> String {
    let raw = format!("{CLIENT_ID}:{CLIENT_SECRET}");
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw))
}

/// Un compte lu chez Sony : son identifiant et son pseudo (s'il est donné).
#[derive(Debug, Clone)]
pub struct Account {
    pub id: PsnId,
    pub online_id: Option<String>,
}

/// Échange le code contre un jeton, puis lit l'identifiant du compte.
/// Deux requêtes HTTPS : à appeler hors du fil de l'interface.
pub fn account_id(code: &str) -> Result<PsnId, PsnError> {
    account(code).map(|a| a.id)
}

/// Comme [`account_id`], avec le pseudo du compte.
pub fn account(code: &str) -> Result<Account, PsnError> {
    let body = format!("grant_type=authorization_code&code={code}&redirect_uri={REDIRECT_URL}&");
    let token: serde_json::Value = match ureq::post(TOKEN_URL)
        .set("Authorization", &basic_auth())
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&body)
    {
        Ok(r) => r.into_json().map_err(|e| PsnError::Network(e.to_string()))?,
        Err(ureq::Error::Status(status, r)) => {
            return Err(PsnError::Refused(format!("HTTP {status} {}", r.into_string().unwrap_or_default())));
        }
        Err(e) => return Err(PsnError::Network(e.to_string())),
    };
    let access = token["access_token"]
        .as_str()
        .ok_or_else(|| PsnError::Refused("pas de jeton dans la réponse".into()))?;

    let info: serde_json::Value = match ureq::get(&format!("{TOKEN_URL}/{access}")).set("Authorization", &basic_auth()).call() {
        Ok(r) => r.into_json().map_err(|e| PsnError::Network(e.to_string()))?,
        Err(ureq::Error::Status(status, r)) => {
            return Err(PsnError::Refused(format!("HTTP {status} {}", r.into_string().unwrap_or_default())));
        }
        Err(e) => return Err(PsnError::Network(e.to_string())),
    };
    let user_id = match &info["user_id"] {
        serde_json::Value::String(s) => s.parse::<u64>().ok(),
        serde_json::Value::Number(n) => n.as_u64(),
        _ => None,
    }
    .ok_or_else(|| PsnError::Refused("pas de user_id dans la réponse".into()))?;
    let online_id = info["online_id"].as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Ok(Account { id: PsnId::from_user_id(user_id), online_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_from_redirect_url() {
        assert_eq!(
            code_from_redirect("https://remoteplay.dl.playstation.net/remoteplay/redirect?code=v3.AbCdEf&cid=1234"),
            Some("v3.AbCdEf".into())
        );
        assert_eq!(code_from_redirect(&format!("{REDIRECT_URL}?code=a%2Bb")), Some("a+b".into()));
        assert_eq!(code_from_redirect(&format!("{REDIRECT_URL}?error=access_denied")), None);
        assert_eq!(code_from_redirect(REDIRECT_URL), None);
        assert!(is_redirect(&format!("{REDIRECT_URL}?code=x")));
        assert!(!is_redirect(LOGIN_URL));
    }

    #[test]
    fn login_url_uses_redirect() {
        assert!(LOGIN_URL.contains(CLIENT_ID));
        assert!(LOGIN_URL.contains(&format!("redirect_uri={REDIRECT_URL}")));
    }
}
