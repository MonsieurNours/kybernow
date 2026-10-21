//! Manifeste signé d'une version (`kybernow-release.json` et sa signature
//! minisign `kybernow-release.json.minisig`), écrit par
//! `scripts/sign-manifest.sh` au moment de la publication.
//!
//! La clé publique est compilée dans la console (`kybernow-release.pub`) :
//! un paquet n'est installé que si son manifeste est signé par elle, annonce
//! la version publiée sur GitHub, un canal que suit cet ordinateur, et donne
//! l'empreinte et la taille exactes du paquet téléchargé.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MANIFEST: &str = "kybernow-release.json";
pub const SIGNATURE: &str = "kybernow-release.json.minisig";
/// Clé publique de publication (format minisign), identifiant D63B24FFFC7E79D1.
pub const RELEASE_KEY: &str = include_str!("kybernow-release.pub");

/// Canal de mises à jour suivi par l'ordinateur (réglage `update_channel`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Versions publiées, seulement.
    #[default]
    Stable,
    /// Aussi les préversions (prereleases GitHub, manifeste en `beta`).
    Beta,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }

    /// Une version de ce canal peut-elle s'installer sur un ordinateur qui suit `setting` ?
    pub fn accepted_by(self, setting: Channel) -> bool {
        setting == Channel::Beta || self == Channel::Stable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub channel: Channel,
    pub published: String,
    pub min_installer_version: String,
    #[serde(default)]
    pub requires_setup: bool,
    pub files: BTreeMap<String, FileEntry>,
}

/// Pourquoi une version signée ne s'installe pas depuis la console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Il faut repasser par `KyberNow-Setup.exe` (version trop ancienne ici,
    /// ou paquet qui l'exige).
    Setup(String),
    /// Paquet refusé (canal, version, fichier absent).
    Rejected(String),
}

impl Refusal {
    pub fn message(&self) -> &str {
        match self {
            Refusal::Setup(m) | Refusal::Rejected(m) => m,
        }
    }
}

/// Vérifie la signature du manifeste avec `public_key` (texte d'un fichier
/// `.pub` de minisign) et le lit. Rien n'est lu d'un manifeste mal signé.
pub fn verify(json: &[u8], minisig: &str, public_key: &str) -> Result<Manifest> {
    let key = minisign_verify::PublicKey::decode(public_key.trim()).map_err(|e| anyhow::anyhow!("clé publique de publication illisible : {e}"))?;
    let signature = minisign_verify::Signature::decode(minisig.trim()).map_err(|e| anyhow::anyhow!("signature du manifeste illisible : {e}"))?;
    key.verify(json, &signature, false).map_err(|e| match e {
        minisign_verify::Error::UnexpectedKeyId => anyhow::anyhow!("manifeste signé par une autre clé que celle de KyberNow"),
        _ => anyhow::anyhow!("signature du manifeste invalide : version falsifiée ou abîmée"),
    })?;
    let manifest: Manifest = serde_json::from_slice(json).context("manifeste signé mais illisible")?;
    if !super::valid_version(&manifest.version) || !super::valid_version(&manifest.min_installer_version) {
        bail!("numéros de version inattendus dans le manifeste ({}, {})", manifest.version, manifest.min_installer_version);
    }
    // Le commentaire de confiance est signé lui aussi : il doit dire la même chose.
    let expected = format!("kybernow {} {}", manifest.version, manifest.channel.as_str());
    if signature.trusted_comment() != expected {
        bail!("commentaire de signature inattendu : « {} » au lieu de « {expected} »", signature.trusted_comment());
    }
    Ok(manifest)
}

/// Vérifie la signature avec la clé de KyberNow compilée dans la console.
pub fn verify_release(json: &[u8], minisig: &str) -> Result<Manifest> {
    verify(json, minisig, RELEASE_KEY)
}

impl Manifest {
    /// La version publiée (`tag_name`) peut-elle s'installer ici, depuis la
    /// console, sur le canal `channel`, par-dessus `installed` ?
    pub fn check(&self, tag_version: &str, channel: Channel, installed: &str) -> Result<(), Refusal> {
        if self.version != tag_version {
            return Err(Refusal::Rejected(format!(
                "le manifeste signé annonce la version {} mais GitHub publie {tag_version}",
                self.version
            )));
        }
        if !self.channel.accepted_by(channel) {
            return Err(Refusal::Rejected(format!(
                "version {} du canal {} : cet ordinateur suit le canal {}",
                self.version,
                self.channel.as_str(),
                channel.as_str()
            )));
        }
        if super::newer(&self.min_installer_version, installed) {
            return Err(Refusal::Setup(format!(
                "la version {} demande au moins la {} : réinstallez avec KyberNow-Setup.exe",
                self.version, self.min_installer_version
            )));
        }
        if self.requires_setup {
            return Err(Refusal::Setup(format!("la version {} s'installe avec KyberNow-Setup.exe : réinstallez avec lui", self.version)));
        }
        if !self.files.contains_key(super::PACKAGE) {
            return Err(Refusal::Rejected(format!("le manifeste de la version {} ne contient pas de paquet de mise à jour", self.version)));
        }
        Ok(())
    }

    /// Empreinte et taille attendues d'un fichier publié.
    pub fn file(&self, name: &str) -> Result<&FileEntry> {
        self.files.get(name).with_context(|| format!("{name} absent du manifeste signé"))
    }

    /// Le fichier téléchargé est-il exactement celui du manifeste ?
    pub fn check_file(&self, name: &str, path: &Path) -> Result<()> {
        let entry = self.file(name)?;
        let size = std::fs::metadata(path).with_context(|| format!("lecture de {}", path.display()))?.len();
        if size != entry.size {
            bail!("{name} : {size} octets au lieu de {} : téléchargement abîmé ou falsifié", entry.size);
        }
        let actual = sha256_file(path)?;
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            bail!("empreinte de {name} différente du manifeste signé : téléchargement abîmé ou falsifié");
        }
        Ok(())
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Paire de clés minisign créée pour le test (clé publique au format `.pub`).
    pub struct TestKey {
        pub pk: minisign::PublicKey,
        sk: minisign::SecretKey,
    }

    impl TestKey {
        pub fn new() -> Self {
            let minisign::KeyPair { pk, sk } = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
            Self { pk, sk }
        }

        pub fn public(&self) -> String {
            self.pk.to_box().unwrap().into_string()
        }

        pub fn sign(&self, data: &[u8], trusted_comment: &str) -> String {
            minisign::sign(Some(&self.pk), &self.sk, std::io::Cursor::new(data), Some(trusted_comment), None)
                .unwrap()
                .into_string()
        }
    }

    pub fn sample(version: &str, channel: Channel) -> Manifest {
        let mut files = BTreeMap::new();
        files.insert("kybernow-update.zip".to_string(), FileEntry { sha256: "ab".repeat(32), size: 42 });
        files.insert("KyberNow-Setup.exe".to_string(), FileEntry { sha256: "cd".repeat(32), size: 7 });
        Manifest {
            version: version.into(),
            channel,
            published: "2026-10-21T08:00:00Z".into(),
            min_installer_version: "0.2.0".into(),
            requires_setup: false,
            files,
        }
    }

    pub fn signed(key: &TestKey, manifest: &Manifest) -> (Vec<u8>, String) {
        let json = serde_json::to_vec_pretty(manifest).unwrap();
        let sig = key.sign(&json, &format!("kybernow {} {}", manifest.version, manifest.channel.as_str()));
        (json, sig)
    }

    #[test]
    fn the_shipped_public_key_is_readable() {
        let key = minisign_verify::PublicKey::decode(RELEASE_KEY.trim()).unwrap();
        drop(key);
        assert!(RELEASE_KEY.contains("D63B24FFFC7E79D1"));
    }

    #[test]
    fn manifest_reads_the_published_format() {
        // Tel qu'écrit par scripts/sign-manifest.sh.
        let text = r#"{
  "version": "0.2.6",
  "channel": "beta",
  "published": "2026-10-21T07:41:24Z",
  "min_installer_version": "0.2.5",
  "requires_setup": false,
  "files": {
    "KyberNow-Setup.exe": { "sha256": "cd47d6ce8c25e5c92422bfc6e63ae848644460075916584426db7355a2d128a1", "size": 4 },
    "kybernow-update.zip": { "sha256": "257a4c075a4c2721970ffc7ef0380d0bf7329185de8f34ff9a6ac8336561c34c", "size": 4 }
  }
}"#;
        let m: Manifest = serde_json::from_str(text).unwrap();
        assert_eq!(m.channel, Channel::Beta);
        assert_eq!(m.files["kybernow-update.zip"].size, 4);
        let back: Manifest = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        assert_eq!(serde_json::to_value(Channel::Stable).unwrap(), "stable");
        assert!(serde_json::from_str::<Channel>("\"nightly\"").is_err());
    }

    #[test]
    fn a_good_signature_is_accepted() {
        let key = TestKey::new();
        let (json, sig) = signed(&key, &sample("0.2.7", Channel::Stable));
        let m = verify(&json, &sig, &key.public()).unwrap();
        assert_eq!(m.version, "0.2.7");
    }

    #[test]
    fn tampering_and_other_keys_are_refused() {
        let key = TestKey::new();
        let manifest = sample("0.2.7", Channel::Stable);
        let (json, sig) = signed(&key, &manifest);
        // Une empreinte changée après la signature.
        let forged = String::from_utf8(json.clone()).unwrap().replace(&"ab".repeat(32), &"ef".repeat(32));
        let e = verify(forged.as_bytes(), &sig, &key.public()).unwrap_err().to_string();
        assert!(e.contains("invalide"), "{e}");
        // Signé par une autre clé.
        let other = TestKey::new();
        let (_, other_sig) = signed(&other, &manifest);
        let e = verify(&json, &other_sig, &key.public()).unwrap_err().to_string();
        assert!(e.contains("autre clé"), "{e}");
        // Et refusé par la clé de KyberNow.
        assert!(verify_release(&json, &sig).is_err());
        // Commentaire de confiance qui ne correspond pas au manifeste.
        let sig = key.sign(&json, "kybernow 0.2.7 beta");
        let e = verify(&json, &sig, &key.public()).unwrap_err().to_string();
        assert!(e.contains("commentaire"), "{e}");
        // Signature absente ou tronquée.
        assert!(verify(&json, "", &key.public()).is_err());
    }

    #[test]
    fn compatibility_checks() {
        let m = sample("0.2.7", Channel::Stable);
        assert_eq!(m.check("0.2.7", Channel::Stable, "0.2.6"), Ok(()));
        assert!(matches!(m.check("0.2.8", Channel::Stable, "0.2.6"), Err(Refusal::Rejected(_))));

        let beta = sample("0.2.7", Channel::Beta);
        assert!(matches!(beta.check("0.2.7", Channel::Stable, "0.2.6"), Err(Refusal::Rejected(_))));
        assert_eq!(beta.check("0.2.7", Channel::Beta, "0.2.6"), Ok(()));
        assert_eq!(m.check("0.2.7", Channel::Beta, "0.2.6"), Ok(()));

        let mut old = sample("0.3.0", Channel::Stable);
        old.min_installer_version = "0.2.10".into();
        match old.check("0.3.0", Channel::Stable, "0.2.9") {
            Err(Refusal::Setup(msg)) => assert!(msg.contains("KyberNow-Setup.exe"), "{msg}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(old.check("0.3.0", Channel::Stable, "0.2.10"), Ok(()));

        let mut setup = sample("0.3.0", Channel::Stable);
        setup.requires_setup = true;
        assert!(matches!(setup.check("0.3.0", Channel::Stable, "0.2.9"), Err(Refusal::Setup(_))));

        let mut empty = sample("0.3.0", Channel::Stable);
        empty.files.remove("kybernow-update.zip");
        assert!(matches!(empty.check("0.3.0", Channel::Stable, "0.2.9"), Err(Refusal::Rejected(_))));
    }

    #[test]
    fn downloaded_file_must_match_size_and_hash() {
        let dir = std::env::temp_dir().join(format!("kybernow-manifest-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("kybernow-update.zip");
        std::fs::write(&zip, b"paquet").unwrap();
        let mut m = sample("0.2.7", Channel::Stable);
        m.files.insert("kybernow-update.zip".into(), FileEntry { sha256: sha256_file(&zip).unwrap().to_uppercase(), size: 6 });
        m.check_file("kybernow-update.zip", &zip).unwrap();
        std::fs::write(&zip, b"paquex").unwrap();
        assert!(m.check_file("kybernow-update.zip", &zip).unwrap_err().to_string().contains("empreinte"));
        std::fs::write(&zip, b"paquet!").unwrap();
        assert!(m.check_file("kybernow-update.zip", &zip).unwrap_err().to_string().contains("octets"));
        assert!(m.check_file("update.ps1", &zip).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
