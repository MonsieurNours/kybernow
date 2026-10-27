//! Console d'administration : son adresse, et l'état qu'elle donne à l'ordinateur
//! local (`GET /api/local/state` : serveur, session en cours, demandes d'accès,
//! partage), et le partage d'un écran du PC (`/api/local/displays`,
//! `/api/local/share…`, voir `docs/kybernow-share.md`).
//!
//! Les routes `/api/local/*` exigent, en plus de l'adresse locale, le jeton
//! que la console tire à son démarrage et à chaque ouverture de session, et
//! qu'un agent copie dans le profil du compte (`%LOCALAPPDATA%\KyberNow\local-token`,
//! lisible par ce compte et les administrateurs) : l'icône l'envoie dans `x-kyber-local`.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::i18n::t;
use crate::{http, share};

fn env_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(fallback))
}

/// `C:\Program Files\KyberNow\kybernow.json`, écrit par l'installateur et
/// complété par la console, sans secret et lisible par tous (modifiable par
/// les administrateurs seulement). L'icône ne lit rien dans
/// `C:\ProgramData\KyberNow` : ce dossier (où `kyber-admin.toml` garde
/// l'empreinte du mot de passe) est réservé à SYSTEM et aux administrateurs.
/// Relu à chaque fois : une réinstallation peut changer le port ou le certificat.
fn record() -> Option<Value> {
    let json = env_dir("ProgramFiles", r"C:\Program Files").join(r"KyberNow\kybernow.json");
    serde_json::from_slice(&std::fs::read(json).ok()?).ok()
}

/// Port de la console (`console_port`).
pub fn port() -> Option<u16> {
    record()?["console_port"].as_u64().and_then(|p| u16::try_from(p).ok())
}

/// Empreinte du certificat de la console (`console_fingerprint`, « sha256:… »),
/// écrite par `kyber-admin init` et par la console à chaque démarrage. Sans
/// elle (installation d'avant 0.2.6 dont la console n'a pas encore redémarré),
/// l'icône ne parle pas à la console.
pub fn fingerprint() -> Option<String> {
    record()?["console_fingerprint"].as_str().map(str::to_owned).filter(|f| !f.is_empty())
}

pub fn url(port: u16) -> String {
    format!("https://localhost:{port}/")
}

/// En-tête du jeton des routes locales.
const LOCAL_HEADER: &str = "x-kyber-local";

/// Jeton des routes locales, copié par la console dans le profil du compte.
pub fn local_token_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join(r"KyberNow\local-token"))
}

/// Relu à chaque appel : la console le renouvelle à chaque ouverture de
/// session et à chaque démarrage (un 403 veut dire qu'il a changé).
fn local_token() -> String {
    local_token_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).unwrap_or_default()
}

/// Ce que la console dit de la session en cours.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalState {
    pub session: bool,
    pub user: Option<String>,
    /// Version plus récente publiée sur GitHub, si la console en a vu une.
    pub update: Option<String>,
    pub version: Option<String>,
    pub hostname: Option<String>,
    pub address: Option<String>,
    /// Session d'une app KyberNow en cours (nom de l'appareil, début, latence).
    pub live: Option<Value>,
    /// Dernière session terminée.
    pub last: Option<Value>,
    /// Appareils qui demandent l'accès par code (id, nom, type ; jamais le
    /// code, qui se compare dans la console).
    pub pair_requests: Vec<Value>,
    /// Accès d'appareils que Kyber ne connaît pas encore : il redémarre de
    /// lui-même dès la fin de la session en cours (compte, appareil, donné ou retiré).
    pub pending_access: Vec<Value>,
    /// Partage ouvert (KyberNow Share), depuis un appareil ou depuis le PC :
    /// origine, code, liens, écran, invités.
    pub share: Option<Value>,
    /// Faux si le partage est coupé (`multi_client` de Kyber), ou si la
    /// console ne le connaît pas (avant la 0.8.0).
    pub share_enabled: bool,
    /// Relais de KyberNow (`server`, `remote_access`, `state`, `reason`),
    /// `None` pour une console qui ne le connaît pas.
    pub relay: Option<Value>,
}

impl LocalState {
    /// Nombre d'invités du partage ouvert (les demandes n'en sont pas).
    pub fn guests(&self) -> usize {
        self.share.as_ref().map_or(0, |s| share::guests(s).count())
    }

    /// Nombre de demandes en attente dans le partage ouvert.
    pub fn requests(&self) -> usize {
        self.share.as_ref().map_or(0, |s| share::requests(s).count())
    }

    /// Prénoms des invités du partage ouvert.
    pub fn guest_names(&self) -> Vec<String> {
        let open = self.share.as_ref();
        open.into_iter().flat_map(share::guests).filter_map(|g| g["name"].as_str().map(str::to_owned)).collect()
    }

    /// Un appareil est en session (le partage d'un écran du PC attend sa fin).
    pub fn device_session(&self) -> bool {
        self.session || self.live.is_some()
    }
}

/// Client épinglé sur le certificat de la console. Sans empreinte connue,
/// aucun appel n'aboutit.
pub struct Client {
    /// Empreinte sur laquelle le client est construit (relue par `watch`).
    pub fingerprint: Option<String>,
    http: Option<http::Client>,
}

impl Client {
    pub fn new() -> Self {
        let fingerprint = fingerprint();
        let http = fingerprint.as_deref().and_then(|fp| match http::Client::pinned(fp) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("certificat de la console : {e:#}");
                None
            }
        });
        Client { fingerprint, http }
    }

    fn get(&self, port: u16, path: &str, headers: &[(&str, &str)]) -> Option<http::Reply> {
        self.http.as_ref()?.call("GET", port, path, headers, Duration::from_secs(3)).ok()
    }

    fn post(&self, port: u16, path: &str, timeout: Duration) -> anyhow::Result<http::Reply> {
        let Some(http) = self.http.as_ref() else {
            anyhow::bail!(t("tray.errors.certificateUnknown", &[]));
        };
        let token = local_token();
        http.call("POST", port, path, &[("x-kyber-admin", "1"), (LOCAL_HEADER, &token)], timeout)
    }

    /// La console a-t-elle un mot de passe ? (`GET /api/session`, sans compte)
    pub fn configured(&self, port: u16) -> Option<bool> {
        let reply = self.get(port, "/api/session", &[])?;
        let body: Value = serde_json::from_slice(&reply.body).ok()?;
        body["configured"].as_bool()
    }

    pub fn local_state(&self, port: u16) -> Option<LocalState> {
        let token = local_token();
        let reply = self.get(port, "/api/local/state", &[(LOCAL_HEADER, &token)])?;
        if reply.status != 200 {
            return None;
        }
        let body: Value = serde_json::from_slice(&reply.body).ok()?;
        let text = |key: &str| body[key].as_str().map(str::to_owned);
        let some = |v: &Value| (!v.is_null()).then(|| v.clone());
        Some(LocalState {
            session: body["session"].as_bool().unwrap_or(false),
            user: text("user"),
            update: text("update_available"),
            version: text("version"),
            hostname: text("hostname"),
            address: text("address"),
            live: some(&body["activity"]["live"]),
            last: some(&body["activity"]["last"]),
            pair_requests: body["pair_requests"].as_array().cloned().unwrap_or_default(),
            pending_access: body["pending_access"].as_array().cloned().unwrap_or_default(),
            share: some(&body["share"]).filter(|s| s["open"].as_bool() != Some(false)),
            share_enabled: body["share_enabled"].as_bool().unwrap_or(false),
            relay: some(&body["relay"]),
        })
    }

    /// Une route locale du partage, avec le jeton local ; la réponse en JSON.
    /// Un refus d'Admin (partage coupé, appareil en session…) donne son
    /// message, déjà dans la langue de l'icône.
    fn share_call(&self, method: &str, port: u16, path: &str, body: Option<Value>) -> anyhow::Result<Value> {
        let failed = |error: &dyn std::fmt::Display| anyhow::anyhow!(t("tray.errors.shareFailed", &[("error", error)]));
        let Some(http) = self.http.as_ref() else {
            anyhow::bail!(t("tray.errors.certificateUnknown", &[]));
        };
        let token = local_token();
        let body = body.map(|b| b.to_string());
        let reply = http
            .send(method, port, path, &[("x-kyber-admin", "1"), (LOCAL_HEADER, &token)], body.as_deref().map(str::as_bytes), Duration::from_secs(15))
            .map_err(|e| failed(&format!("{e:#}")))?;
        let value = serde_json::from_slice::<Value>(&reply.body).unwrap_or(Value::Null);
        if reply.status >= 400 {
            return Err(match value["error"].as_str() {
                Some(message) => anyhow::anyhow!(message.to_string()),
                None => failed(&describe(&reply)),
            });
        }
        Ok(value)
    }

    /// Écrans du PC, l'écran principal d'abord (`id`, `label` traduit, définition).
    pub fn displays(&self, port: u16) -> anyhow::Result<Vec<Value>> {
        Ok(self.share_call("GET", port, "/api/local/displays", None)?.as_array().cloned().unwrap_or_default())
    }

    /// Ouvre le partage d'un écran du PC ; la vue du partage.
    pub fn share_open(&self, port: u16, display: Value) -> anyhow::Result<Value> {
        self.share_call("POST", port, "/api/local/share/open", Some(serde_json::json!({ "display": display })))
    }

    /// Nouveau code (les invités déjà là restent).
    pub fn share_code(&self, port: u16) -> anyhow::Result<Value> {
        self.share_call("POST", port, "/api/local/share/code", None)
    }

    /// Droits d'un invité : `spectator`, `player` (manettes), `kbm` (clavier et
    /// souris) ou `player_kbm` (les deux).
    pub fn share_role(&self, port: u16, guest: &str, role: &str) -> anyhow::Result<Value> {
        self.share_call("POST", port, &format!("/api/local/share/guest/{}", path_part(guest)), Some(serde_json::json!({ "role": role })))
    }

    /// Accepte ou refuse une demande (un invité venu par le relais) ; la vue
    /// du partage. Corps `{}` : rien à dire de plus que l'identifiant.
    pub fn share_answer(&self, port: u16, guest: &str, accept: bool) -> anyhow::Result<Value> {
        let action = if accept { "accept" } else { "refuse" };
        let path = format!("/api/local/share/guest/{}/{action}", path_part(guest));
        self.share_call("POST", port, &path, Some(serde_json::json!({})))
    }

    /// Retire un invité du partage.
    pub fn share_remove(&self, port: u16, guest: &str) -> anyhow::Result<Value> {
        self.share_call("DELETE", port, &format!("/api/local/share/guest/{}", path_part(guest)), None)
    }

    /// Arrête le partage : tout le monde sort.
    pub fn share_stop(&self, port: u16) -> anyhow::Result<()> {
        self.share_call("DELETE", port, "/api/local/share", None).map(|_| ())
    }

    /// Met fin à la session en cours (route réservée à l'ordinateur lui-même).
    pub fn end_session(&self, port: u16) -> anyhow::Result<()> {
        let reply = self.post(port, "/api/local/end", Duration::from_secs(10)).map_err(|e| anyhow::anyhow!(t("tray.errors.endSessionFailed", &[("error", &format!("{e:#}"))])))?;
        if reply.status >= 400 {
            anyhow::bail!(t("tray.errors.endSessionFailed", &[("error", &describe(&reply))]));
        }
        Ok(())
    }

    /// Accepte ou refuse une demande d'accès par code (route réservée à l'ordinateur lui-même).
    pub fn pair_answer(&self, port: u16, id: &str, accept: bool) -> anyhow::Result<()> {
        let action = if accept { "accept" } else { "refuse" };
        let reply = self.post(port, &format!("/api/local/pair/{id}/{action}"), Duration::from_secs(15)).map_err(|e| anyhow::anyhow!(t("tray.errors.pairFailed", &[("error", &format!("{e:#}"))])))?;
        match reply.status {
            s if s < 400 => Ok(()),
            410 => anyhow::bail!(t("tray.errors.pairExpired", &[])),
            _ => anyhow::bail!(t("tray.errors.pairFailed", &[("error", &describe(&reply))])),
        }
    }
}

/// Identifiant d'invité dans un chemin : lettres, chiffres, `-` et `_` tels
/// quels, le reste encodé.
fn path_part(id: &str) -> String {
    id.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// « 403 : jeton local absent ou périmé », d'après le JSON d'erreur de la console.
fn describe(reply: &http::Reply) -> String {
    let message = serde_json::from_slice::<Value>(&reply.body).ok().and_then(|v| v["error"].as_str().map(str::to_owned));
    match message {
        Some(m) => format!("{} : {m}", reply.status),
        None => reply.status.to_string(),
    }
}

/// Code d'initialisation écrit par la console quand son mot de passe a été
/// effacé (lisible en administrateur seulement).
pub fn data_setup_code_path() -> PathBuf {
    env_dir("ProgramData", r"C:\ProgramData").join(r"KyberNow\setup-code.txt")
}

/// Copie du code laissée par la partie élevée de « Réinitialiser le mot de
/// passe » dans le profil du compte (`%LOCALAPPDATA%\KyberNow\setup-code.txt`).
pub fn handoff_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join(r"KyberNow\setup-code.txt"))
}

pub fn read_code(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|c| c.trim().to_string()).filter(|c| !c.is_empty())
}

/// Code d'initialisation pour l'icône ordinaire : la copie laissée par la
/// partie élevée (lue puis effacée), sinon celui de ProgramData si ce compte
/// peut le lire.
pub fn take_setup_code() -> Option<String> {
    if let Some(path) = handoff_path()
        && let Some(code) = read_code(&path)
    {
        let _ = std::fs::remove_file(&path);
        return Some(code);
    }
    read_code(&data_setup_code_path())
}
