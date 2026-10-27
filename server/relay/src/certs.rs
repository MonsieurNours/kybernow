//! Le certificat du relais (`<domain>` et `*.<domain>`, renouvelé par lego) :
//! relu quand le fichier change, sans redémarrer, pour TLS sur TCP comme
//! pour QUIC ; sa date d'expiration pour les métriques.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

/// Le certificat en cours, interchangeable à chaud.
#[derive(Debug)]
pub struct CertStore {
    cert: PathBuf,
    key: PathBuf,
    current: RwLock<Option<Arc<CertifiedKey>>>,
    stamp: Mutex<Option<(SystemTime, SystemTime)>>,
    expiry: AtomicI64,
}

impl ResolvesServerCert for CertStore {
    fn resolve(&self, _hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        self.current.read().unwrap().clone()
    }
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

impl CertStore {
    /// Prépare le magasin et lit le certificat s'il est déjà là (au premier
    /// lancement, lego ne l'a peut-être pas encore obtenu : le relais attend).
    pub fn new(cert: PathBuf, key: PathBuf) -> Arc<CertStore> {
        let store = Arc::new(CertStore { cert, key, current: RwLock::new(None), stamp: Mutex::new(None), expiry: AtomicI64::new(0) });
        match store.reload_if_changed() {
            Ok(true) => {}
            Ok(false) => tracing::warn!("certificat {} absent : en attente de lego", store.cert.display()),
            Err(e) => tracing::warn!("certificat illisible : {e:#}"),
        }
        store
    }

    /// Relit le certificat si l'un des deux fichiers a changé ; vrai s'il a été pris.
    pub fn reload_if_changed(&self) -> Result<bool> {
        let (Some(c), Some(k)) = (mtime(&self.cert), mtime(&self.key)) else { return Ok(false) };
        if *self.stamp.lock().unwrap() == Some((c, k)) {
            return Ok(false);
        }
        let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(&self.cert)
            .with_context(|| format!("lecture de {}", self.cert.display()))?
            .collect::<Result<_, _>>()
            .with_context(|| format!("{} illisible", self.cert.display()))?;
        anyhow::ensure!(!certs.is_empty(), "{} : aucun certificat", self.cert.display());
        let key = PrivateKeyDer::from_pem_file(&self.key).with_context(|| format!("{} illisible", self.key.display()))?;
        let signer = rustls::crypto::ring::sign::any_supported_type(&key).context("clé non prise en charge")?;
        let expiry = not_after(&certs[0]).unwrap_or(0);
        *self.current.write().unwrap() = Some(Arc::new(CertifiedKey::new(certs, signer)));
        *self.stamp.lock().unwrap() = Some((c, k));
        self.expiry.store(expiry, Ordering::Relaxed);
        tracing::info!("certificat {} pris (expire dans {} jours)", self.cert.display(), (expiry - unix_now()) / 86_400);
        Ok(true)
    }

    pub fn is_ready(&self) -> bool {
        self.current.read().unwrap().is_some()
    }

    /// Secondes avant l'expiration (négatif : expiré ; `None` : pas de certificat).
    pub fn expires_in(&self) -> Option<i64> {
        match self.expiry.load(Ordering::Relaxed) {
            0 => None,
            t => Some(t - unix_now()),
        }
    }

    /// Relit le certificat toutes les 30 s s'il a changé.
    pub fn watch(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let store = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(30));
            loop {
                tick.tick().await;
                if let Err(e) = store.reload_if_changed() {
                    tracing::warn!("certificat non relu : {e:#}");
                }
            }
        })
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Un élément DER : (étiquette, contenu, reste).
fn der(input: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, mut rest) = rest.split_first()?;
    let len = if first < 0x80 {
        first as usize
    } else {
        let n = (first & 0x7f) as usize;
        if n == 0 || n > 4 || rest.len() < n {
            return None;
        }
        let len = rest[..n].iter().fold(0usize, |acc, b| (acc << 8) | *b as usize);
        rest = &rest[n..];
        len
    };
    (rest.len() >= len).then(|| (tag, &rest[..len], &rest[len..]))
}

/// La fin de validité d'un certificat, en secondes Unix.
pub fn not_after(cert: &[u8]) -> Option<i64> {
    let (_, cert, _) = der(cert)?;
    let (_, tbs, _) = der(cert)?;
    let mut rest = tbs;
    let (tag, _, r) = der(rest)?;
    if tag == 0xa0 {
        rest = r; // version
    }
    let (_, _, r) = der(rest)?; // numéro de série
    let (_, _, r) = der(r)?; // algorithme de signature
    let (_, _, r) = der(r)?; // émetteur
    let (_, validity, _) = der(r)?;
    let (_, _, r) = der(validity)?; // début
    let (tag, time, _) = der(r)?;
    parse_time(tag, std::str::from_utf8(time).ok()?)
}

fn parse_time(tag: u8, s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (year, rest) = match tag {
        0x17 if s.len() >= 12 => {
            let y: i64 = s[..2].parse().ok()?;
            (if y >= 50 { 1900 + y } else { 2000 + y }, &s[2..])
        }
        0x18 if s.len() >= 14 => (s[..4].parse().ok()?, &s[4..]),
        _ => return None,
    };
    let n = |i: usize| -> Option<i64> { rest.get(i..i + 2)?.parse().ok() };
    let (month, day, hour, min, sec) = (n(0)?, n(2)?, n(4)?, n(6)?, n(8)?);
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + min * 60 + sec)
}

/// Jours depuis le 1er janvier 1970 (algorithme de Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(parse_time(0x17, "260131120000Z"), Some(1_769_860_800));
        assert_eq!(parse_time(0x18, "20491231235959Z"), Some(2_524_607_999));
    }

    #[test]
    fn expiration_d_un_certificat() {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(vec!["share.test".into()]).unwrap();
        params.not_after = rcgen::date_time_ymd(2031, 3, 4);
        let cert = params.self_signed(&key).unwrap();
        assert_eq!(not_after(cert.der()), Some(days_from_civil(2031, 3, 4) * 86_400));
    }

    #[test]
    fn rechargement_a_chaud() {
        let dir = std::env::temp_dir().join(format!("kn-relay-certs-{}", kybernow_tunnel::id::random_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (c, k) = (dir.join("c.pem"), dir.join("k.pem"));
        let store = CertStore::new(c.clone(), k.clone());
        assert!(!store.is_ready());
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(vec!["share.test".into()]).unwrap().self_signed(&key).unwrap();
        std::fs::write(&c, cert.pem()).unwrap();
        std::fs::write(&k, key.serialize_pem()).unwrap();
        assert!(store.reload_if_changed().unwrap());
        assert!(store.is_ready());
        assert!(!store.reload_if_changed().unwrap(), "rien n'a changé");
        assert!(store.expires_in().unwrap() > 0);
        let _ = std::fs::remove_dir_all(dir);
    }
}
