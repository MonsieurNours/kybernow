//! Fenêtre de mise à jour (icône de la barre des menus) : ce qu'Admin sait de
//! la dernière version, puis l'installation, suivie jusqu'au retour de la
//! Station dans la nouvelle version.
//!
//! Admin fait tout le travail, comme sous Windows : recherche sur GitHub,
//! manifeste signé, téléchargement vérifié, puis son applicateur Mac
//! (`server/admin/src/update/mac.rs`) remplace l'app et relance la Station,
//! ou remet l'ancienne si la nouvelle ne répond pas. Cette fenêtre lui parle
//! par ses routes locales (`/api/local/update…`), avec le jeton local et son
//! certificat épinglé, comme l'icône.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::commands::{Emit, Event, UpdatePhase};
use super::http::Client;

fn client() -> Result<(Client, u16, String)> {
    let record = super::read_record().context("KyberNow Station n'est pas installée")?;
    let fingerprint = record.console_fingerprint.context("certificat d'Admin inconnu (Admin n'a pas encore démarré)")?;
    let token = std::fs::read_to_string(super::admin_dir().join("local-token")).map(|t| t.trim().to_string()).unwrap_or_default();
    Ok((Client::pinned(&fingerprint)?, record.console_port, token))
}

fn call(method: &str, path: &str, body: Option<&[u8]>, timeout: Duration) -> Result<Value> {
    let (client, port, token) = client()?;
    let reply = client.send(method, port, path, &[("x-kyber-local", &token)], body, timeout)?;
    let v: Value = serde_json::from_slice(&reply.body).unwrap_or_default();
    if reply.status != 200 {
        bail!("{}", v["error"].as_str().or(v["message"].as_str()).unwrap_or("KyberNow Admin a refusé"));
    }
    Ok(v)
}

/// Points des notes de version (listes Markdown), sans leur balisage.
fn bullets(notes: &str) -> Vec<String> {
    notes
        .lines()
        .filter_map(|l| l.trim().strip_prefix("- ").or_else(|| l.trim().strip_prefix("* ")))
        .map(|l| l.replace("**", "").replace('`', "").trim().to_string())
        .filter(|l| !l.is_empty())
        .take(4)
        .collect()
}

fn info_from(status: &Value) -> Event {
    let current = status["current"].as_str().map(str::to_string).unwrap_or_else(|| crate::VERSION.into());
    let available = status["available"].as_bool() == Some(true);
    let version = available.then(|| status["latest"].as_str().map(str::to_string)).flatten();
    let refusal = (available && status["installable"].as_bool() != Some(true))
        .then(|| status["verify_error"].as_str().or(status["install_error"].as_str()).map(str::to_string))
        .flatten();
    Event::UpdateInfo {
        current,
        version,
        notes: status["notes"].as_str().map(bullets).unwrap_or_default(),
        page: status["page"].as_str().map(str::to_string),
        session: status["session"].as_str().map(str::to_string),
        refusal,
    }
}

/// Recherche la dernière version (GitHub, par Admin), puis l'état.
pub fn info() -> Event {
    let _ = call("POST", "/api/local/update/check", None, Duration::from_secs(20));
    match call("GET", "/api/local/update", None, Duration::from_secs(5)) {
        Ok(status) => info_from(&status),
        Err(e) => Event::UpdateFailed { message: format!("{e:#}") },
    }
}

/// Version d'Admin qui répond (sans compte : `/api/session`), `None` s'il est arrêté.
fn running_version() -> Option<String> {
    call("GET", "/api/session", None, Duration::from_secs(3)).ok().and_then(|v| v["version"].as_str().map(str::to_string))
}

/// Lance l'installation et la suit : téléchargement et vérification par
/// Admin, arrêt de la Station, retour dans la nouvelle version (3 minutes au
/// plus), sinon la raison écrite par l'applicateur (`update/resultat.json`).
pub fn start(emit: &Emit) {
    let progress = |phase, percent| emit(Event::UpdateProgress { phase, percent });
    progress(UpdatePhase::Download, 5.0);
    let before = running_version();
    let version = match call("POST", "/api/local/update/install", Some(br#"{"force":true}"#), Duration::from_secs(600)) {
        Ok(v) => v["version"].as_str().unwrap_or_default().to_string(),
        Err(e) => return emit(Event::UpdateFailed { message: format!("{e:#}") }),
    };
    progress(UpdatePhase::Install, 55.0);
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut down = false;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(2));
        match running_version() {
            None => {
                if !down {
                    down = true;
                    progress(UpdatePhase::Restart, 75.0);
                }
            }
            Some(v) if v == version => return emit(Event::UpdateDone { version }),
            Some(v) if down && Some(&v) == before.as_ref() => {
                // Revenue dans l'ancienne version : l'applicateur a remis la précédente.
                return emit(Event::UpdateFailed { message: failure().unwrap_or_else(|| format!("La Station est revenue en version {v}.")) });
            }
            Some(_) => {}
        }
    }
    emit(Event::UpdateFailed { message: failure().unwrap_or_else(|| "La Station n'est pas revenue en 3 minutes : voyez ~/Library/Logs/KyberNow/admin.log.".into()) })
}

/// Raison d'un échec, écrite par l'applicateur.
fn failure() -> Option<String> {
    let v: Value = serde_json::from_slice(&std::fs::read(super::admin_dir().join("update").join("resultat.json")).ok()?).ok()?;
    (v["ok"].as_bool() == Some(false)).then(|| v["message"].as_str().map(str::to_string)).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_become_bullets() {
        let notes = "## 1.0.1\n\n- **Écrans** du Mac éteints plus vite\n* Son `7.1` vers le téléviseur\nTexte libre\n";
        assert_eq!(bullets(notes), vec!["Écrans du Mac éteints plus vite", "Son 7.1 vers le téléviseur"]);
    }

    #[test]
    fn a_refused_version_says_why() {
        let status = serde_json::json!({ "current": "1.0.0", "available": true, "latest": "1.0.1", "installable": false, "verify_error": "réinstallez", "session": "iPhone" });
        match info_from(&status) {
            Event::UpdateInfo { version, refusal, session, .. } => {
                assert_eq!(version.as_deref(), Some("1.0.1"));
                assert_eq!(refusal.as_deref(), Some("réinstallez"));
                assert_eq!(session.as_deref(), Some("iPhone"));
            }
            other => panic!("{other:?}"),
        }
    }
}
