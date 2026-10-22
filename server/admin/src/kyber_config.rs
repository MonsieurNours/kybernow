//! Lecture et écriture de `kyber_config.toml`.
//!
//! Le fichier est édité avec `toml_edit` pour conserver les commentaires et la
//! mise en forme du modèle fourni par Kyber. Chaque écriture est atomique et
//! précédée d'une sauvegarde horodatée.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

use crate::i18n::Msg;
use crate::schema::{self, Field};

const MAX_BACKUPS: usize = 30;
/// Clé HS256 des jetons du client web, dans le dossier de données (voir
/// `set_jwt_secret_file`).
pub const JWT_KEY_FILE: &str = "kyber-jwt.key";
/// Identifiant et empreinte présents dans le modèle Kyber (« Default password »).
const DEFAULT_USER: &str = "Default user";
const DEFAULT_PASSWORD_HASH: &str =
    "c2bb5d1edbff196e3a879ec0a59aeb3952822266909b5ba07b45072d20a05446";

pub struct KyberConfig {
    path: PathBuf,
    backup_dir: PathBuf,
    /// Une seule lecture-modification-écriture à la fois : deux changements
    /// simultanés (un appairage accepté depuis la console et depuis l'icône,
    /// un réglage appliqué pendant un appairage) ne s'écrasent plus.
    write: Mutex<()>,
    /// Comptes lus la dernière fois, avec la date du fichier : les appels des
    /// appareils (signal de vie chaque seconde) ne relisent pas le TOML à chaque fois.
    logins: Mutex<Option<(SystemTime, u64, Vec<(String, String)>)>>,
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
        Self { path, backup_dir, write: Mutex::new(()), logins: Mutex::new(None) }
    }

    /// Comptes Kyber (identifiant, empreinte), relus seulement si le fichier a changé.
    fn cached_logins(&self) -> Result<Vec<(String, String)>> {
        let meta = fs::metadata(&self.path).with_context(|| format!("lecture de {}", self.path.display()))?;
        let stamp = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
        if let Some((modified, len, logins)) = self.logins.lock().unwrap().as_ref() {
            if (*modified, *len) == stamp {
                return Ok(logins.clone());
            }
        }
        let logins = read_logins(&self.load()?);
        *self.logins.lock().unwrap() = Some((stamp.0, stamp.1, logins.clone()));
        Ok(logins)
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
        let _write = self.write.lock().unwrap();
        let fields = schema::fields();
        let mut doc = self.load()?;
        for (id, value) in changes {
            let field = fields
                .iter()
                .find(|f| f.id == id)
                .ok_or_else(|| Msg::new("config.unknownSetting").with("id", id.as_str()))?;
            match value {
                Some(v) => {
                    field.validate(v)?;
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

    /// Clé HS256 des jetons JWT acceptés par Kyber, posée par `init` : dans
    /// le fichier `key = { file = … }` (depuis 0.2.6), ou en clair
    /// (`key = { plain = … }`, installations précédentes, déplacée au
    /// démarrage suivant de la console).
    pub fn jwt_secret(&self) -> Option<String> {
        let doc = self.load().ok()?;
        let jwt = table(&doc, &["kycontroller", "auth", "jwt"])?;
        if jwt.get("enabled").and_then(|v| v.as_bool()) == Some(false) {
            return None;
        }
        if jwt.get("algorithm").and_then(|v| v.as_str()).is_some_and(|a| a != "HS256") {
            return None;
        }
        let key = jwt.get("key")?.as_table_like()?;
        if let Some(file) = key.get("file").and_then(|v| v.as_str()) {
            // Kyber lit le fichier tel quel, blancs de début et de fin retirés.
            return fs::read_to_string(file).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        }
        key.get("plain")?.as_str().map(str::to_string).filter(|s| !s.is_empty())
    }

    /// Clé HS256 écrite dans `file` (dans le dossier de données, réservé à
    /// SYSTEM et aux Administrateurs, que Kyber lit puisqu'il tourne en
    /// SYSTEM) et désignée par `key = { file = … }` : `kyber_config.toml`,
    /// sous Program Files, ne contient plus la clé.
    pub fn set_jwt_secret_file(&self, file: &Path, secret: &str) -> Result<()> {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension("key.tmp");
        fs::write(&tmp, secret).with_context(|| format!("écriture de {}", tmp.display()))?;
        fs::rename(&tmp, file).with_context(|| format!("remplacement de {}", file.display()))?;
        let _write = self.write.lock().unwrap();
        let mut doc = self.load()?;
        write_jwt_key(&mut doc, "file", &file.display().to_string())?;
        self.save(&doc.to_string())
    }

    /// Installation d'avant 0.2.6 : la clé en clair dans `kyber_config.toml`
    /// part dans `file`, même valeur (Kyber, déjà lancé, la garde en mémoire
    /// jusqu'à son redémarrage, où il la relira dans le fichier). Vrai si la
    /// clé a été déplacée.
    pub fn move_jwt_secret_to_file(&self, file: &Path) -> Result<bool> {
        let doc = self.load()?;
        let Some(plain) = table(&doc, &["kycontroller", "auth", "jwt"])
            .and_then(|jwt| jwt.get("key")?.as_table_like()?.get("plain")?.as_str().map(str::to_string))
            .filter(|s| !s.is_empty())
        else {
            return Ok(false);
        };
        drop(doc);
        self.set_jwt_secret_file(file, &plain)?;
        Ok(true)
    }

    /// Certificat TLS de kycontroller (`[kycontroller] tls_cert`, sinon
    /// `kybertest_cert.pem` à côté de `kyber_config.toml`, ce que `cert-gen.exe`
    /// produit à l'installation).
    pub fn tls_cert_path(&self) -> PathBuf {
        self.kyber_path("tls_cert", "kybertest_cert.pem")
    }

    /// Clé privée du certificat de kycontroller.
    pub fn tls_key_path(&self) -> PathBuf {
        self.kyber_path("tls_key", "kybertest_key.pem")
    }

    fn kyber_path(&self, key: &str, default: &str) -> PathBuf {
        let dir = self.path.parent().map(Path::to_path_buf).unwrap_or_default();
        let configured = self.load().ok().and_then(|doc| {
            table(&doc, &["kycontroller"])?.get(key)?.as_str().map(PathBuf::from)
        });
        match configured {
            Some(p) if p.is_absolute() => p,
            Some(p) => dir.join(p),
            None => dir.join(default),
        }
    }

    /// Réserve à SYSTEM et aux Administrateurs ce que Kyber garde sous
    /// Program Files et qui ne doit pas être lu par les autres comptes :
    /// `kyber_config.toml` (empreintes des mots de passe des comptes), le
    /// certificat et la clé de kycontroller, et `avant-maj\` (copie d'une
    /// version précédente) s'il existe. Kyber tourne en SYSTEM : rien ne
    /// change pour lui. Une mise à jour remplace ces fichiers (ils reprennent
    /// alors les droits de Program Files) : la console repose les droits à
    /// chaque démarrage, et à chaque écriture de `kyber_config.toml`.
    pub fn secure_secrets(&self) {
        if !cfg!(windows) {
            return; // rien à poser hors Windows (développement)
        }
        for file in [self.path.clone(), self.tls_cert_path(), self.tls_key_path()] {
            if !file.exists() {
                continue;
            }
            match crate::update::acl::secure_file(&file) {
                Ok(()) => tracing::info!("droits de {} : SYSTEM et Administrateurs seulement", file.display()),
                Err(e) => tracing::warn!("droits de {} non posés : {e:#}", file.display()),
            }
        }
        if let Some(previous) = self.path.parent().map(|d| d.join("avant-maj")).filter(|d| d.is_dir()) {
            match crate::update::acl::secure_dir(&previous) {
                Ok(_) => tracing::info!("droits de {} : SYSTEM et Administrateurs seulement", previous.display()),
                Err(e) => tracing::warn!("droits de {} non posés : {e:#}", previous.display()),
            }
        }
    }

    /// Vérifie un identifiant et un mot de passe Kyber (connexion Basic).
    pub fn verify_login(&self, username: &str, password: &str) -> bool {
        let Ok(logins) = self.cached_logins() else { return false };
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        logins.iter().any(|(u, h)| {
            // Comparaison à temps constant des empreintes.
            u == username && h.len() == hash.len() && h.bytes().zip(hash.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
        })
    }

    /// Ajoute un utilisateur, ou change son mot de passe s'il existe déjà.
    pub fn upsert_user(&self, username: &str, password: &str) -> Result<()> {
        validate_username(username)?;
        if password.chars().count() < 8 {
            bail!(Msg::new("config.passwordLength"));
        }
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        let _write = self.write.lock().unwrap();
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
            bail!(Msg::new("config.passwordLength"));
        }
        let hash = hex::encode(Sha256::digest(password.as_bytes()));
        let _write = self.write.lock().unwrap();
        let mut doc = self.load()?;
        write_logins(&mut doc, &[(username.to_string(), hash)])?;
        self.save(&doc.to_string())
    }

    /// Clé JWT en clair dans `kyber_config.toml` (tests ; les installations
    /// passent par `set_jwt_secret_file`).
    #[cfg(test)]
    pub fn set_jwt_secret(&self, secret: &str) -> Result<()> {
        let _write = self.write.lock().unwrap();
        let mut doc = self.load()?;
        write_jwt_key(&mut doc, "plain", secret)?;
        self.save(&doc.to_string())
    }

    pub fn delete_user(&self, username: &str) -> Result<()> {
        let _write = self.write.lock().unwrap();
        let mut doc = self.load()?;
        let mut logins = read_logins(&doc);
        let before = logins.len();
        logins.retain(|(u, _)| u != username);
        if logins.len() == before {
            bail!(Msg::new("config.unknownUser").with("username", username));
        }
        write_logins(&mut doc, &logins)?;
        self.save(&doc.to_string())
    }

    /// Remplace le fichier entier (éditeur avancé), après vérification de la syntaxe.
    pub fn write_raw(&self, text: &str) -> Result<()> {
        text.parse::<DocumentMut>()
            .map_err(|e| Msg::new("config.invalidToml").with("error", e.to_string()))?;
        let _write = self.write.lock().unwrap();
        self.save(text)
    }

    fn save(&self, text: &str) -> Result<()> {
        self.backup()?;
        let tmp = self.path.with_extension("toml.kyber-admin-tmp");
        fs::write(&tmp, text).with_context(|| format!("écriture de {}", tmp.display()))?;
        // Le fichier neuf hérite des droits de Program Files (lisible par tous) :
        // réservé à SYSTEM et aux Administrateurs avant de prendre la place.
        if let Err(e) = crate::update::acl::secure_file(&tmp) {
            tracing::warn!("droits de {} non posés : {e:#}", tmp.display());
        }
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
        // Deux sauvegardes dans la même milliseconde : suffixe `_01`, `_02`… pour
        // ne pas écraser la précédente. `_` se classe après `.`, donc le tri par
        // nom ci-dessous garde l'ordre chronologique pour la rotation.
        let mut dest = self.backup_dir.join(format!("kyber_config-{stamp}.toml"));
        let mut n = 1;
        while dest.exists() {
            dest = self.backup_dir.join(format!("kyber_config-{stamp}_{n:02}.toml"));
            n += 1;
        }
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

/// `[kycontroller.auth.jwt]` : `algorithm = "HS256"`, `key = { <kind> = value }`.
fn write_jwt_key(doc: &mut DocumentMut, kind: &str, value: &str) -> Result<()> {
    let mut key = InlineTable::new();
    key.insert(kind, value.into());
    let table = table_mut(doc, &["kycontroller", "auth", "jwt"])?;
    table.insert("algorithm", toml_edit::value("HS256"));
    table.insert("key", toml_edit::value(key));
    Ok(())
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
        bail!(Msg::new("config.emptyUsername"));
    }
    if username.contains(':') || username.chars().any(char::is_control) {
        bail!(Msg::new("config.badUsername"));
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

    #[test]
    fn jwt_secret_lives_in_a_file() {
        let (d, cfg) = setup();
        let file = d.0.join("data").join("kyber-jwt.key");
        cfg.set_jwt_secret_file(&file, "clef-dans-un-fichier").unwrap();
        let text = cfg.read_raw().unwrap();
        assert!(!text.contains("clef-dans-un-fichier"), "{text}");
        assert!(text.contains("key = { file = "), "{text}");
        assert_eq!(fs::read_to_string(&file).unwrap(), "clef-dans-un-fichier");
        assert_eq!(cfg.jwt_secret().as_deref(), Some("clef-dans-un-fichier"));
        // Déjà dans un fichier : rien à déplacer.
        assert!(!cfg.move_jwt_secret_to_file(&file).unwrap());

        // Installation précédente : clé en clair, déplacée au démarrage.
        cfg.set_jwt_secret("en-clair").unwrap();
        assert!(cfg.move_jwt_secret_to_file(&file).unwrap());
        assert!(!cfg.read_raw().unwrap().contains("en-clair"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "en-clair");
        assert_eq!(cfg.jwt_secret().as_deref(), Some("en-clair"));
    }

    #[test]
    fn kyber_certificate_paths() {
        let (d, cfg) = setup();
        assert_eq!(cfg.tls_cert_path(), d.0.join("kybertest_cert.pem"));
        assert_eq!(cfg.tls_key_path(), d.0.join("kybertest_key.pem"));
        cfg.write_raw(&SAMPLE.replace("#port = 8080", "#port = 8080\ntls_cert = \"certs/mine.pem\"\ntls_key = \"/abs/key.pem\"")).unwrap();
        assert_eq!(cfg.tls_cert_path(), d.0.join("certs").join("mine.pem"));
        assert_eq!(cfg.tls_key_path(), PathBuf::from("/abs/key.pem"));
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
