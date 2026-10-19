//! Certificat HTTPS auto-signé de l'interface, généré au premier lancement.

use std::fs;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn ensure_certificate(data_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let dir = data_dir.join("tls");
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    if cert.exists() && key.exists() {
        return Ok((cert, key));
    }
    fs::create_dir_all(&dir)?;

    let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    if let Ok(host) = hostname::get() {
        let host = host.to_string_lossy().to_lowercase();
        names.push(format!("{host}.local"));
        names.push(host);
    }
    if let Some(ip) = lan_ip() {
        names.push(ip);
    }
    let generated = rcgen::generate_simple_self_signed(names.clone())
        .context("génération du certificat")?;
    fs::write(&cert, generated.cert.pem())?;
    fs::write(&key, generated.key_pair.serialize_pem())?;
    tracing::info!("certificat auto-signé créé pour {}", names.join(", "));
    Ok((cert, key))
}

/// Empreinte SHA-256 du certificat, au format « sha256:aa:bb:… » : les apps
/// la comparent à celle du certificat présenté à la connexion.
pub fn fingerprint(cert: &Path) -> Option<String> {
    use rustls::pki_types::{CertificateDer, pem::PemObject};
    use sha2::{Digest, Sha256};
    let der = CertificateDer::from_pem_file(cert).ok()?;
    let digest = Sha256::digest(der.as_ref());
    let hex: Vec<String> = digest.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("sha256:{}", hex.join(":")))
}

/// Adresse IP locale utilisée pour sortir vers le réseau (aucun paquet n'est envoyé).
pub fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}
