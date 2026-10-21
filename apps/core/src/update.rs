//! Mises à jour d'une app : la dernière version publiée sur GitHub, sur son
//! canal (stable, ou bêta : aussi les préversions), vérifiée par le manifeste
//! signé de la version (`kybernow-release.json` et sa signature minisign),
//! comme la console (`server/admin/src/update`), avec la même clé publique.
//!
//! Rien n'est proposé sans signature valide, et le fichier téléchargé doit
//! avoir exactement la taille et l'empreinte SHA-256 du manifeste.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::i18n::{self, Lang};

/// Message d'erreur dans la langue de l'app.
fn t(key: &str, params: &[(&str, &str)]) -> String {
    i18n::tr(i18n::current(), key, params)
}

pub const REPO: &str = "MonsieurNours/kybernow";
const MANIFEST: &str = "kybernow-release.json";
const SIGNATURE: &str = "kybernow-release.json.minisig";
/// Clé publique de publication (minisign), identifiant D63B24FFFC7E79D1.
const RELEASE_KEY: &str = include_str!("../../../server/admin/src/update/kybernow-release.pub");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FileEntry {
    sha256: String,
    size: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    version: String,
    channel: Channel,
    files: BTreeMap<String, FileEntry>,
}

/// Une version plus récente, vérifiée, à installer.
#[derive(Debug, Clone, Serialize)]
pub struct Available {
    pub version: String,
    pub channel: Channel,
    /// Notes de version dans la langue de l'app (voir [`Available::localize`]).
    pub notes: String,
    /// Texte complet de la release (français, `<!-- en -->`, anglais).
    #[serde(skip)]
    pub body: String,
    pub page: String,
    pub file: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

pub fn valid_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
}

/// « 0.10.2 » > « 0.9.9 ».
pub fn newer(candidate: &str, than: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.trim_start_matches('v').split(['.', '-']).map_while(|p| p.parse().ok()).collect() };
    parse(candidate) > parse(than)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent(&format!("kybernow-app/{}", env!("CARGO_PKG_VERSION")))
        .build()
}

fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let resp = agent().get(url).call().map_err(|e| t("update.download_of", &[("url", url), ("error", &e.to_string())]))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| t("update.download_of", &[("url", url), ("error", &e.to_string())]))?;
    Ok(buf)
}

fn verify(json: &[u8], minisig: &str) -> Result<Manifest, String> {
    let key = minisign_verify::PublicKey::decode(RELEASE_KEY.trim()).map_err(|e| t("update.bad_key", &[("error", &e.to_string())]))?;
    let sig = minisign_verify::Signature::decode(minisig.trim()).map_err(|e| t("update.bad_signature", &[("error", &e.to_string())]))?;
    key.verify(json, &sig, false).map_err(|e| match e {
        minisign_verify::Error::UnexpectedKeyId => t("update.other_key", &[]),
        _ => t("update.invalid_signature", &[]),
    })?;
    let m: Manifest = serde_json::from_slice(json).map_err(|e| t("update.bad_manifest", &[("error", &e.to_string())]))?;
    if !valid_version(&m.version) {
        return Err(t("update.bad_version", &[("version", &m.version)]));
    }
    // Le commentaire de confiance est signé lui aussi.
    let expected = format!("kybernow {} {}", m.version, m.channel.as_str());
    if sig.trusted_comment() != expected {
        return Err(t("update.bad_comment", &[("comment", sig.trusted_comment())]));
    }
    Ok(m)
}

/// La version la plus récente du canal, si elle est plus récente que
/// `current` et publie `file` ; vérifiée par le manifeste signé.
pub fn check(current: &str, channel: Channel, file: &str) -> Result<Option<Available>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=30");
    let resp = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| t("update.github_down", &[("error", &e.to_string())]))?;
    let mut body = Vec::new();
    resp.into_reader()
        .take(16 << 20)
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    let list: Vec<serde_json::Value> = serde_json::from_slice(&body).map_err(|e| t("update.github_invalid", &[("error", &e.to_string())]))?;
    let version_of = |r: &serde_json::Value| r["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    let has_file = |r: &serde_json::Value| r["assets"].as_array().is_some_and(|a| a.iter().any(|x| x["name"] == file));
    // La plus récente qui publie l'installateur de cette plateforme (une
    // version sans lui, serveur seulement par exemple, ne masque pas la précédente).
    let best = list
        .iter()
        .filter(|r| r["draft"].as_bool() != Some(true))
        .filter(|r| channel == Channel::Beta || r["prerelease"].as_bool() != Some(true))
        .filter(|r| valid_version(&version_of(r)))
        .filter(|r| has_file(r))
        .reduce(|best, r| if newer(&version_of(r), &version_of(best)) { r } else { best });
    let Some(release) = best else { return Ok(None) };
    let version = version_of(release);
    if !newer(&version, current) {
        return Ok(None);
    }
    let asset = |name: &str| {
        release["assets"].as_array().and_then(|a| {
            a.iter().find(|x| x["name"] == name).and_then(|x| x["browser_download_url"].as_str()).map(String::from)
        })
    };
    let (Some(murl), Some(surl)) = (asset(MANIFEST), asset(SIGNATURE)) else {
        return Err(t("update.no_manifest", &[("version", &version)]));
    };
    let Some(furl) = asset(file) else {
        // Version sans app pour cette plateforme : rien à proposer.
        return Ok(None);
    };
    let manifest = verify(&get(&murl, 256 << 10)?, &String::from_utf8_lossy(&get(&surl, 16 << 10)?))?;
    if manifest.version != version {
        return Err(t("update.version_mismatch", &[("manifest", &manifest.version), ("version", &version)]));
    }
    if manifest.channel == Channel::Beta && channel == Channel::Stable {
        return Ok(None);
    }
    let entry = manifest.files.get(file).ok_or_else(|| t("update.file_missing", &[("file", file), ("version", &version)]))?;
    let body = release["body"].as_str().unwrap_or("").to_string();
    Ok(Some(Available {
        version,
        channel: manifest.channel,
        notes: i18n::release_notes(&body, i18n::current()),
        body,
        page: release["html_url"].as_str().unwrap_or("").to_string(),
        file: file.to_string(),
        url: furl,
        sha256: entry.sha256.to_lowercase(),
        size: entry.size,
    }))
}

impl Available {
    /// Notes de version reprises dans `lang` (la langue de l'app a changé).
    pub fn localize(&mut self, lang: Lang) {
        self.notes = i18n::release_notes(&self.body, lang);
    }
}

/// Télécharge le fichier dans `dest`, jamais plus que la taille du
/// manifeste, et vérifie taille et empreinte ; `progress` reçoit les octets reçus.
pub fn download(a: &Available, dest: &Path, progress: impl Fn(u64)) -> Result<(), String> {
    let resp = agent().get(&a.url).call().map_err(|e| t("update.download", &[("error", &e.to_string())]))?;
    let mut reader = resp.into_reader().take(a.size + 1);
    let mut out = std::fs::File::create(dest).map_err(|e| format!("{} : {e}", dest.display()))?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buf = vec![0u8; 256 << 10];
    loop {
        let n = reader.read(&mut buf).map_err(|e| t("update.download", &[("error", &e.to_string())]))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        total += n as u64;
        progress(total);
    }
    out.sync_all().map_err(|e| e.to_string())?;
    drop(out);
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if total != a.size || digest != a.sha256 {
        let _ = std::fs::remove_file(dest);
        return Err(t("update.tampered", &[]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("0.10.0", "0.9.9"));
        assert!(!newer("0.2.8", "0.2.8"));
        assert!(valid_version("0.2.9"));
        assert!(!valid_version("0.2.9-beta"));
    }

    #[test]
    fn signature_refusee() {
        assert!(verify(b"{}", "nimporte quoi").is_err());
    }
}

#[cfg(test)]
mod live {
    /// Contre les vraies versions publiées (réseau) : `cargo test -p kybernow-core -- --ignored`.
    #[test]
    #[ignore]
    fn derniere_version_signee() {
        let a = super::check("0.0.1", super::Channel::Stable, "KyberNow-macOS.zip").unwrap().unwrap();
        assert!(super::valid_version(&a.version));
        assert_eq!(a.sha256.len(), 64);
        assert!(super::check(&a.version, super::Channel::Stable, "KyberNow-macOS.zip").unwrap().is_none());
    }
}
