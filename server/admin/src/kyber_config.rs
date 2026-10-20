//! Lecture et écriture de `kyber_config.toml`.
//!
//! Le fichier est édité avec `toml_edit` pour conserver les commentaires et la
//! mise en forme du modèle fourni par Kyber. Chaque écriture est atomique et
//! précédée d'une sauvegarde horodatée.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

use crate::schema::{self, Field};

const MAX_BACKUPS: usize = 30;
/// Identifiant et empreinte présents dans le modèle Kyber (« Default password »).
const DEFAULT_USER: &str = "Default user";
const DEFAULT_PASSWORD_HASH: &str =
    "c2bb5d1edbff196e3a879ec0a59aeb3952822266909b5ba07b45072d20a05446";

pub struct KyberConfig {
    path: PathBuf,
    backup_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct FieldValue {
    pub id: &'static str,
    pub value: Value,
    /// `false` quand la clé est absente du fichier (Kyber applique sa valeur par défaut).
    pub explicit: bool,
}

#[derive(Debug, Serialize)]
pub struct User {
    pub username: String,
    /// Compte du modèle Kyber avec son mot de passe d'origine.
    pub insecure_default: bool,
}

impl KyberConfig {
    pub fn new(path: PathBuf, backup_dir: PathBuf) -> Self {
        Self { path, backup_dir }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_raw(&self) -> Result<String> {
        fs::read_to_string(&self.path)
            .with_context(|| format!("lecture de {}", self.path.display()))
    }

    fn load(&self) -> Result<DocumentMut> {
        self.read_raw()?
            .parse::<DocumentMut>()
            .context("kyber_config.toml n'est pas un TOML valide")
    }

    pub fn values(&self) -> Result<Vec<FieldValue>> {
        let doc = self.load()?;
        Ok(schema::fields()
            .iter()
            .map(|f| match read_field(&doc, f) {
                Some(value) => FieldValue { id: f.id, value, explicit: true },
                None => FieldValue { id: f.id, value: f.default.clone(), explicit: false },
            })
            .collect())
    }

    /// Applique des modifications : `Some(v)` écrit la valeur, `None` retire la clé
    /// pour revenir à la valeur par défaut de Kyber.
    pub fn apply(&self, changes: &BTreeMap<String, Option<Value>>) -> Result<()> {
        let fields = schema::fields();
        let mut doc = self.load()?;
        for (id, value) in changes {
            let field = fields
                .iter()
                .find(|f| f.id == id)
                .ok_or_else(|| anyhow!("réglage inconnu : {id}"))?;
            match value {
                Some(v) => {
                    field.validate(v).map_err(|e| anyhow!(e))?;
                    write_field(&mut doc, field, v)?;
                }
                None => remove_field(&mut doc, field),
            }
        }
        self.save(&doc.to_string())
    }

    pub fn users(&self) -> Result<Vec<User>> {
        let doc = self.load()?;
        Ok(read_logins(&doc)
            .into_iter()
            .map(|(username, hash)| User {
                insecure_default: username == DEFAULT_USER && hash == DEFAULT_PASSWORD_HASH,
                username,
            })
            .collect())
    }

    /// Port du serveur Kyber (TCP et UDP).
    pub fn port(&self) -> Option<u16> {
        self.values()
            .ok()?
            .into_iter()
            .find(|f| f.id == "network.port")
            .and_then(|f| f.value.as_u64())
            .and_then(|p| u16::try_from(p).ok())
    }

    /// Clé HS256 des jetons JWT acceptés par Kyber, posée par `init`.
    pub fn jwt_secret(&self) -> Option<String> {
        let doc = self.load().ok()?;
        let jwt = table(&doc, &["kycontroller", "auth", "jwt"])?;
        if jwt.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            return None;
        }
        if jwt.get("algorithm").and_then(|v| v.as_str()).is_some_and(|a| a != "HS256") {
            return None;
        }
        let key = jwt.get("key")?;
        let plain = key.as_table_like()?.get("plain")?;
        plain.as_str().map(str::to_string).filter(|s| !s.is_empty())
    }

    /// Vérifie un identifiant et un mot de passe Kyber (connexion Basic).
    pub fn verify_login(&self, username: &str, password: &str) -> bool {
        let Ok(doc) = self.load() else { return false };
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        read_logins(&doc).iter().any(|(u, h)| {
            // Comparaison à temps constant des empreintes.
            u == username && h.len() == hash.len() && h.bytes().zip(hash.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
        })
    }

    /// Ajoute un utilisateur, ou change son mot de passe s'il existe déjà.
    pub fn upsert_user(&self, username: &str, password: &str) -> Result<()> {
        validate_username(username)?;
        if password.chars().count() < 8 {
            bail!("le mot de passe doit faire au moins 8 caractères");
        }
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        let mut doc = self.load()?;
        let mut logins = read_logins(&doc);
        match logins.iter_mut().find(|(u, _)| u == username) {
            Some(entry) => entry.1 = hash,
            None => logins.push((username.to_string(), hash)),
        }
        write_logins(&mut doc, &logins)?;
        self.save(&doc.to_string())
    }

    /// Remplace tous les comptes par un seul (installation initiale).
    pub fn reset_users(&self, username: &str, password: &str) -> Result<()> {
        validate_username(username)?;
        if password.chars().count() < 8 {
            bail!("le mot de passe doit faire au moins 8 caractères");
        }
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        let mut doc = self.load()?;
        write_logins(&mut doc, &[(username.to_string(), hash)])?;
        self.save(&doc.to_string())
    }

    /// Remplace la clé de développement JWT de Kyber par un secret aléatoire.
    pub fn set_jwt_secret(&self, secret: &str) -> Result<()> {
        let mut doc = self.load()?;
        let mut key = InlineTable::new();
        key.insert("plain", secret.into());
        let table = table_mut(&mut doc, &["kycontroller", "auth", "jwt"])?;
        table.insert("algorithm", toml_edit::value("HS256"));
        table.insert("key", toml_edit::value(key));
        self.save(&doc.to_string())
    }

    pub fn delete_user(&self, username: &str) -> Result<()> {
        let mut doc = self.load()?;
        let mut logins = read_logins(&doc);
        let before = logins.len();
        logins.retain(|(u, _)| u != username);
        if logins.len() == before {
            bail!("utilisateur inconnu : {username}");
        }
        write_logins(&mut doc, &logins)?;
        self.save(&doc.to_string())
    }

    /// Remplace le fichier entier (éditeur avancé), après vérification de la syntaxe.
    pub fn write_raw(&self, text: &str) -> Result<()> {
        text.parse::<DocumentMut>()
            .map_err(|e| anyhow!("TOML invalide : {e}"))?;
        self.save(text)
    }

    fn save(&self, text: &str) -> Result<()> {
        self.backup()?;
        let tmp = self.path.with_extension("toml.kyber-admin-tmp");
        fs::write(&tmp, text).with_context(|| format!("écriture de {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("remplacement de {}", self.path.display()))?;
        tracing::info!("kyber_config.toml mis à jour");
        Ok(())
    }

    fn backup(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.backup_dir)?;
        let stamp = time::OffsetDateTime::now_utc()
            .format(time::macros::format_description!(
                "[year][month][day]-[hour][minute][second]-[subsecond digits:3]"
            ))
            .unwrap_or_default();
        let dest = self.backup_dir.join(format!("kyber_config-{stamp}.toml"));
        fs::copy(&self.path, &dest).with_context(|| format!("sauvegarde vers {}", dest.display()))?;

        let mut backups: Vec<_> = fs::read_dir(&self.backup_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        backups.sort();
        let excess = backups.len().saturating_sub(MAX_BACKUPS);
        for old in &backups[..excess] {
            let _ = fs::remove_file(old);
        }
        Ok(())
    }
}

fn table<'a>(doc: &'a DocumentMut, path: &[&str]) -> Option<&'a dyn toml_edit::TableLike> {
    let mut current: &dyn toml_edit::TableLike = doc.as_table();
    for key in path {
        current = current.get(key)?.as_table_like()?;
    }
    Some(current)
}

fn table_mut<'a>(doc: &'a mut DocumentMut, path: &[&str]) -> Result<&'a mut Table> {
    let mut current: &mut Table = doc.as_table_mut();
    for key in path {
        let item = current.entry(key).or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        });
        current = item
            .as_table_mut()
            .ok_or_else(|| anyhow!("[{}] n'est pas une table", path.join(".")))?;
    }
    Ok(current)
}

fn read_field(doc: &DocumentMut, field: &Field) -> Option<Value> {
    let value = table(doc, field.table)?.get(field.key)?.as_value()?;
    match value {
        toml_edit::Value::Boolean(b) => Some(Value::Bool(*b.value())),
        toml_edit::Value::Integer(i) => Some(Value::from(*i.value())),
        toml_edit::Value::String(s) => Some(Value::String(s.value().clone())),
        _ => None,
    }
}

fn write_field(doc: &mut DocumentMut, field: &Field, value: &Value) -> Result<()> {
    let item = match value {
        Value::Bool(b) => toml_edit::value(*b),
        Value::Number(n) => toml_edit::value(n.as_i64().ok_or_else(|| anyhow!("entier attendu"))?),
        Value::String(s) => toml_edit::value(s.as_str()),
        _ => bail!("type non pris en charge"),
    };
    let table = table_mut(doc, field.table)?;
    match table.get_mut(field.key).and_then(Item::as_value_mut) {
        // Conserve les commentaires de fin de ligne déjà présents.
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = item.into_value().expect("valeur simple");
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert(field.key, item);
        }
    }
    Ok(())
}

fn remove_field(doc: &mut DocumentMut, field: &Field) {
    if let Ok(table) = table_mut(doc, field.table) {
        table.remove(field.key);
    }
}

const LOGINS_PATH: &[&str] = &["kycontroller", "auth", "basic"];

fn read_logins(doc: &DocumentMut) -> Vec<(String, String)> {
    let Some(array) = table(doc, LOGINS_PATH)
        .and_then(|t| t.get("logins"))
        .and_then(Item::as_array)
    else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|v| {
            let t = v.as_inline_table()?;
            Some((
                t.get("username")?.as_str()?.to_string(),
                t.get("password")?.as_str()?.to_string(),
            ))
        })
        .collect()
}

fn write_logins(doc: &mut DocumentMut, logins: &[(String, String)]) -> Result<()> {
    let mut array = Array::new();
    for (username, hash) in logins {
        let mut entry = InlineTable::new();
        entry.insert("username", username.as_str().into());
        entry.insert("password", hash.as_str().into());
        array.push(entry);
    }
    for value in array.iter_mut() {
        value.decor_mut().set_prefix("\n    ");
    }
    array.set_trailing_comma(true);
    array.set_trailing("\n");

    let table = table_mut(doc, LOGINS_PATH)?;
    match table.get_mut("logins").and_then(Item::as_value_mut) {
        Some(existing) => {
            let decor = existing.decor().clone();
            *existing = toml_edit::Value::Array(array);
            *existing.decor_mut() = decor;
        }
        None => {
            table.insert("logins", toml_edit::value(array));
        }
    }
    Ok(())
}

fn validate_username(username: &str) -> Result<()> {
    // RFC 7617 : pas de « : » dans l'identifiant, pas de caractères de contrôle.
    if username.trim().is_empty() {
        bail!("l'identifiant est vide");
    }
    if username.contains(':') || username.chars().any(char::is_control) {
        bail!("l'identifiant ne peut contenir ni « : » ni caractère de contrôle");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[kyavserver]
# Configure the video encoder to use
#encoder = "nvenc"

[kycontroller]
#port = 8080

[kycontroller.auth.basic]
# Enable/Disable the backend.
enabled = true

# List of allowed login pairs.
logins = [
    { username = "Default user", password = "c2bb5d1edbff196e3a879ec0a59aeb3952822266909b5ba07b45072d20a05446" },
]
"#;

    fn setup() -> (tempdir::TempDir, KyberConfig) {
        let dir = tempdir::TempDir::new();
        let path = dir.0.join("kyber_config.toml");
        fs::write(&path, SAMPLE).unwrap();
        let cfg = KyberConfig::new(path, dir.0.join("backups"));
        (dir, cfg)
    }

    #[test]
    fn defaults_when_keys_are_commented_out() {
        let (_d, cfg) = setup();
        let values = cfg.values().unwrap();
        let port = values.iter().find(|v| v.id == "network.port").unwrap();
        assert_eq!(port.value, Value::from(8080));
        assert!(!port.explicit);
    }

    #[test]
    fn apply_keeps_comments_and_round_trips() {
        let (_d, cfg) = setup();
        let mut changes = BTreeMap::new();
        changes.insert("network.port".to_string(), Some(Value::from(9000)));
        changes.insert("video.encoder".to_string(), Some(Value::from("x264")));
        changes.insert("auth.jwt_enabled".to_string(), Some(Value::from(false)));
        cfg.apply(&changes).unwrap();

        let text = cfg.read_raw().unwrap();
        assert!(text.contains("# Configure the video encoder to use"));
        assert!(text.contains("port = 9000"));
        assert!(text.contains("encoder = \"x264\""));
        assert!(text.contains("[kycontroller.auth.jwt]"));
        let values = cfg.values().unwrap();
        let jwt = values.iter().find(|v| v.id == "auth.jwt_enabled").unwrap();
        assert_eq!(jwt.value, Value::from(false));

        changes.clear();
        changes.insert("network.port".to_string(), None);
        cfg.apply(&changes).unwrap();
        assert!(!cfg.read_raw().unwrap().contains("port = 9000"));
        assert!(cfg.backup_dir.read_dir().unwrap().count() >= 2);
    }

    #[test]
    fn rejects_invalid_values() {
        let (_d, cfg) = setup();
        let mut changes = BTreeMap::new();
        changes.insert("network.port".to_string(), Some(Value::from(70000)));
        assert!(cfg.apply(&changes).is_err());
        changes.clear();
        changes.insert("video.encoder".to_string(), Some(Value::from("h265")));
        assert!(cfg.apply(&changes).is_err());
    }

    #[test]
    fn manages_users() {
        let (_d, cfg) = setup();
        assert!(cfg.users().unwrap()[0].insecure_default);
        cfg.upsert_user("matthieu", "un-mot-de-passe").unwrap();
        cfg.delete_user("Default user").unwrap();
        let users = cfg.users().unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].username, "matthieu");
        let text = cfg.read_raw().unwrap();
        assert!(text.contains("# List of allowed login pairs."));
        assert!(text.contains(&hex::encode(Sha256::digest(b"un-mot-de-passe"))));
        assert!(cfg.upsert_user("a:b", "un-mot-de-passe").is_err());
        assert!(cfg.verify_login("matthieu", "un-mot-de-passe"));
        assert!(!cfg.verify_login("matthieu", "mauvais-mot-de-passe"));
        assert!(cfg.upsert_user("bob", "court").is_err());
    }

    #[test]
    fn initial_setup() {
        let (_d, cfg) = setup();
        cfg.reset_users("matthieu", "un-mot-de-passe").unwrap();
        cfg.set_jwt_secret("secret-de-test").unwrap();
        let users = cfg.users().unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].username, "matthieu");
        let text = cfg.read_raw().unwrap();
        assert!(text.contains("key = { plain = \"secret-de-test\" }"));
        assert!(text.parse::<DocumentMut>().is_ok());
        // Le client web signe ses jetons avec cette clé.
        assert_eq!(cfg.jwt_secret().as_deref(), Some("secret-de-test"));
    }

    /// Petit répertoire temporaire, sans dépendance supplémentaire.
    mod tempdir {
        pub struct TempDir(pub std::path::PathBuf);
        impl TempDir {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "kyber-admin-test-{}-{}",
                    std::process::id(),
                    rand::random::<u64>()
                ));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
        }
        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
