//! Les ordinateurs de l'app : liste et réglages dans
//! `%APPDATA%\KyberNow\computers.json`, mots de passe dans le gestionnaire
//! d'identifiants de Windows (`KyberNow:<id>`).
//!
//! Comptes KyberNow (docs/comptes.md, lot 3) : chaque compte connecté est
//! gardé entier (JSON, jeton de la session d'appareil compris) dans le même
//! gestionnaire, sous `KyberNowCompte:<relais>|<identifiant du compte>` (une
//! cible par compte, hors du filtre `KyberNow:*` des mots de passe) ; la
//! dernière liste des ordinateurs des comptes, sans rien de secret, dans
//! `%APPDATA%\KyberNow\accounts.json`.

use std::path::PathBuf;

use kybernow_core::account::{Account, AccountComputer};
use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Security::Credentials::{
    CRED_FLAGS, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};
use windows::core::{PCWSTR, PWSTR};

/// Réglages d'un ordinateur (voir l'écran Réglages).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `jeu`, `bureau`, `max`, `auto` (profil Auto, avec le résultat du test
    /// gardé dans `Computer::auto`) ; « Personnalisé » dès qu'un réglage
    /// s'écarte du profil (retouches ci-dessous).
    pub profile: String,
    pub codec: Option<String>,
    pub yuv444: Option<bool>,
    pub mbps: Option<u32>,
    pub reliable: Option<bool>,
    /// « Débit adaptatif » de la carte « Débit » (docs/profil-auto.md) :
    /// allumé par défaut (absent des fichiers d'avant), hors du profil (le
    /// couper ne rend pas « Personnalisé »).
    pub adaptive: bool,
    pub max_4k: bool,
    /// Écran virtuel sur mesure (définition et fréquence) ; sinon comme cet écran.
    pub custom: Option<kybernow_core::console::Mode>,
    /// Multi-écrans, réglage « Écrans » : `auto`, `this` (Cet écran) ou `all`
    /// (Tous les écrans), comme `kybernow_core::screens::DisplaysSetting`.
    /// Absent des fichiers d'avant le multi-écrans : `auto`.
    pub displays: String,
    pub vsync: bool,
    /// `auto`, `bureau` ou `jeu`.
    pub mouse: String,
    /// `xbox360` ou `fidele`.
    pub gamepad: String,
    pub clipboard: bool,
    /// « Définition de la caméra » (docs/webcam.md) : 1080p, sinon 720p ;
    /// hors profil (comme le HDR).
    pub camera_1080: bool,
    /// « Ctrl agit comme ⌘ », face à une Station Mac seulement ; `None` :
    /// jamais changé, allumé (`keymap::control_as_command` du cœur).
    pub ctrl_command: Option<bool>,
    /// Mode immersif (docs/link-ui.md) : `fullscreen` (Alt+Tab, la touche ⊞… vont à l'ordinateur en plein
    /// écran, la règle d'avant ce réglage), `always` (aussi dans une fenêtre) ou `never`.
    pub immersive: String,
    /// `auto`, `2`, `6`, `8` ou `0` (son laissé sur l'ordinateur).
    pub audio: String,
    pub virtual_display: bool,
    /// Sur batterie : `alleger` ou `garder`. Le réglage vit dans les réglages
    /// de l'app (`Prefs::battery`, il vaut pour ce PC) ; celui-ci ne sert qu'à
    /// reprendre le choix quand `app.json` n'en a pas encore.
    pub battery: String,
    /// Garder la dernière image de la session comme fond sur l'accueil.
    pub keep_last_image: bool,
    /// Reconnexion automatique quand le réseau décroche pendant la session.
    pub auto_reconnect: bool,
    /// Le choix « HDR » pour cet ordinateur (docs/hdr.md) ; `None` : jamais
    /// changé, allumé (le HDR est automatique sur un écran HDR).
    pub hdr: Option<bool>,
    /// « Micro » pour cet ordinateur (docs/micro.md) : coupé à la première
    /// session, puis le dernier choix (barre, Ctrl+Alt+Maj+V, rubrique Son).
    pub microphone: bool,
}

impl Settings {
    /// Le choix « HDR » (allumé tant qu'il n'a pas été changé).
    pub fn hdr_on(&self) -> bool {
        self.hdr.unwrap_or(true)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: "jeu".into(),
            codec: None,
            yuv444: None,
            mbps: None,
            reliable: None,
            adaptive: true,
            max_4k: false,
            custom: None,
            displays: "auto".into(),
            vsync: true,
            mouse: "auto".into(),
            gamepad: "xbox360".into(),
            clipboard: true,
            camera_1080: false,
            ctrl_command: None,
            immersive: "fullscreen".into(),
            audio: "auto".into(),
            virtual_display: true,
            battery: "alleger".into(),
            keep_last_image: true,
            auto_reconnect: true,
            hdr: None,
            microphone: false,
        }
    }
}

/// Dernière session d'au moins 3 s (rubrique Statistiques des réglages de
/// l'ordinateur). Les champs de ce qui a été transmis sont absents des
/// fichiers écrits par la 0.4.x, qui gardaient `label` à la place.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LastSession {
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub seconds: u64,
    /// Fin, en secondes depuis 1970.
    pub at: u64,
    /// Définition et fréquence de l'image transmise (multi-écrans : celles de
    /// l'écran principal).
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub refresh: Option<u32>,
    /// Nombre d'écrans de la session (absent : un seul, comme avant le
    /// multi-écrans).
    pub screens: Option<u32>,
    /// Format vidéo (« HEVC », « H.264 », « AV1 »).
    pub codec: Option<String>,
    /// Débit demandé à l'ordinateur, en Mbit/s (multi-écrans : le total).
    pub mbps: Option<u32>,
    /// Par où est passée la session (`on_route` du cœur, à sa fin) : la ligne
    /// « Chemin » des statistiques (« Direct », « Par le relais »). Absent des
    /// sessions d'avant le relais.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<kybernow_core::relay::RouteInfo>,
    /// Un écran au moins a été en HDR (« HDR » après le format).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hdr: bool,
    /// Profil et format en français (« Bureau · HEVC 4:4:4 ») : seulement
    /// dans les fichiers de la 0.4.x, montré tant qu'une session ne l'a pas
    /// remplacé.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Computer {
    pub id: String,
    pub name: String,
    pub host: String,
    /// Port de la console.
    pub port: u16,
    pub user: String,
    pub fingerprint: Option<String>,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub gpu: Option<String>,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub last: Option<LastSession>,
    /// Ajouté avec un code : l'ordinateur a donné à ce PC son propre compte.
    #[serde(default)]
    pub by_code: bool,
    /// Ses cartes réseau pour le réveiller une fois éteint (`wake` du dernier
    /// `info` qui en donnait, Station 0.5.0 et suivantes) ; absentes des
    /// fichiers écrits par la 0.4.x.
    #[serde(default)]
    pub wake: Vec<kybernow_core::wake::WakeTarget>,
    /// Dernière version de la Station annoncée par une notification (une
    /// fois par version et par ordinateur).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_notified: Option<String>,
    /// L'ordinateur a déjà proposé la caméra (docs/webcam.md) : « Définition
    /// de la caméra » apparaît dans ses réglages.
    #[serde(default)]
    pub camera_offered: bool,
    /// Son relais (`info.relay` du dernier `info` : relais, identifiant chez lui,
    /// « Accès hors de chez vous » actif) : l'app ouvre l'ordinateur par le
    /// relais quand elle ne le trouve pas sur le réseau (docs/relais.md).
    /// Absent d'une Station antérieure ou sans relais réglé.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<kybernow_core::relay::RelayInfo>,
    /// Un ordinateur d'un compte KyberNow qui n'est pas appairé sur ce PC
    /// (docs/comptes.md) : tel que le relais le donne (`/v1/computers`). Il est
    /// ouvert avec le jeton d'accès du relais (ni compte ni mot de passe :
    /// `user` est vide), sur le réseau quand la découverte le trouve (même
    /// empreinte d'Admin), par le relais sinon ; il quitte l'accueil avec son
    /// compte. Absent : un ordinateur appairé, ajouté par code ou par une
    /// invitation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountComputer>,
    /// Profil Auto (docs/profil-auto.md) : le dernier test appliqué pour cet
    /// ordinateur sur ce PC (`AutoResult` du cœur, sa date, son réseau, ses
    /// mesures et ses réglages). Choisir Auto sans lui ouvre le test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto: Option<kybernow_core::autotest::AutoResult>,
    /// Plafond de l'encodeur de cet ordinateur, en Mbit/s par écran
    /// (docs/profil-auto.md, « L'encodeur de l'ordinateur ») : la session ne
    /// demande jamais plus en HEVC, AV1 ou H.264 (`StreamSettings::encoder_max`).
    /// Trouvé par le test Auto (« Appliquer » : `AutoResult::next_encoder_max`)
    /// ou par une session qui a dû redescendre (`Notice::EncoderCeiling`).
    /// Hors du profil et propre à ce PC : jamais synchronisé. Absent : aucun.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoder_max: Option<u32>,
    /// Mise à jour de la Station proposée à l'accueil (docs/link-ui.md « Mise à
    /// jour des Stations ») : « Plus tard » (version et date) et la dernière
    /// vérification demandée (`station_offer::Memory` du cœur). Absent : rien
    /// de remis ni de demandé.
    #[serde(default, skip_serializing_if = "is_default")]
    pub station_offer: kybernow_core::station_offer::Memory,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// `accounts.json` : la dernière liste des ordinateurs des comptes, appairés
/// sur ce PC compris (pour l'accueil, un ordinateur une seule fois), gardée
/// d'un lancement à l'autre avec les empreintes d'Admin déjà vues
/// (`account::remember_fingerprints` : le relais ne les donne que pour une
/// Station connectée). Rien de secret.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct AccountsFile {
    computers: Vec<AccountComputer>,
}

pub struct Store {
    path: PathBuf,
    pub computers: Vec<Computer>,
    /// La dernière liste des ordinateurs des comptes connectés (`accounts.json`).
    pub account_computers: Vec<AccountComputer>,
}

impl Store {
    pub fn load() -> Self {
        let dir = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("computers.json");
        let computers = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(list) => list,
                Err(e) => {
                    // Illisible : mis de côté plutôt qu'écrasé au prochain enregistrement.
                    let aside = path.with_extension(format!(
                        "json.{}.illisible",
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
                    ));
                    log::error!("computers.json illisible ({e}) : mis de côté dans {}", aside.display());
                    let _ = std::fs::rename(&path, &aside);
                    Vec::new()
                }
            },
            Err(_) => Vec::new(),
        };
        // Illisible : la liste revient de toute façon au prochain relevé des comptes.
        let account_computers = std::fs::read_to_string(accounts_path(&path))
            .ok()
            .and_then(|t| serde_json::from_str::<AccountsFile>(&t).ok())
            .map(|f| f.computers)
            .unwrap_or_default();
        Self { path, computers, account_computers }
    }

    /// Garde la dernière liste des ordinateurs des comptes (`accounts.json`).
    pub fn save_account_computers(&self) {
        let file = AccountsFile { computers: self.account_computers.clone() };
        let Ok(text) = serde_json::to_string_pretty(&file) else { return };
        write_atomic(&accounts_path(&self.path), &text);
    }

    pub fn save(&self) {
        let Ok(text) = serde_json::to_string_pretty(&self.computers) else { return };
        write_atomic(&self.path, &text);
    }

    pub fn get(&self, id: &str) -> Option<&Computer> {
        self.computers.iter().find(|c| c.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Computer> {
        self.computers.iter_mut().find(|c| c.id == id)
    }

    /// Ajoute ou remplace (même adresse et même port).
    pub fn upsert(&mut self, c: Computer) -> String {
        let at = self.computers.iter().position(|o| o.host == c.host && o.port == c.port);
        self.put(c, at)
    }

    /// Remplace l'ordinateur à la place `at` (il garde son identifiant, ses
    /// réglages, sa dernière session…), ou ajoute `c` sans place. Une invitation
    /// choisit elle-même l'ordinateur qu'elle remplace : le même Admin
    /// (`inviting::same_computer`).
    pub fn put(&mut self, mut c: Computer, at: Option<usize>) -> String {
        let id = if let Some(old) = at.and_then(|i| self.computers.get_mut(i)) {
            c.id = old.id.clone();
            c.settings = old.settings.clone();
            c.last = old.last.take();
            // Une Station plus ancienne ne donne pas ses cartes : on garde les connues.
            if c.wake.is_empty() {
                c.wake = std::mem::take(&mut old.wake);
            }
            c.station_notified = old.station_notified.take();
            c.station_offer = std::mem::take(&mut old.station_offer);
            // Une Station qui ne dit pas son relais (ajout par une adresse que le
            // relais ne connaît pas encore) garde celui qu'on connaît.
            if c.relay.is_none() {
                c.relay = old.relay.take();
            }
            *old = c;
            old.id.clone()
        } else {
            let id = c.id.clone();
            self.computers.push(c);
            id
        };
        self.save();
        id
    }

    pub fn remove(&mut self, id: &str) {
        self.computers.retain(|c| c.id != id);
        self.save();
        delete_password(id);
    }
}

/// `accounts.json`, à côté de `computers.json`.
fn accounts_path(computers: &std::path::Path) -> PathBuf {
    computers.with_file_name("accounts.json")
}

/// Écrit `text` dans `path`, posé sur le disque avant de remplacer l'ancien.
fn write_atomic(path: &std::path::Path, text: &str) {
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let written = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(text.as_bytes())?;
        f.sync_all()
    });
    match written {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp, path) {
                log::error!("{name} : {e}");
            }
        }
        Err(e) => log::error!("{name} : {e}"),
    }
}

pub fn new_id() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::getrandom(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn target(id: &str) -> Vec<u16> {
    wide(&format!("KyberNow:{id}"))
}

pub fn set_password(id: &str, user: &str, password: &str) -> bool {
    let mut target = target(id);
    let mut user = wide(user);
    let blob: Vec<u8> = password.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let cred = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user.as_mut_ptr()),
        LastWritten: FILETIME::default(),
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0).is_ok() }
}

pub fn password(id: &str) -> Option<String> {
    let target = target(id);
    let mut p: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut p).ok()?;
        let c = &*p;
        let s = if c.CredentialBlob.is_null() || c.CredentialBlobSize == 0 {
            String::new()
        } else {
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            let units: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
            String::from_utf16_lossy(&units)
        };
        CredFree(p as *const _);
        Some(s)
    }
}

/// Efface tous les mots de passe de KyberNow (désinstallation).
pub fn delete_all_passwords() -> usize {
    use windows::Win32::Security::Credentials::{CRED_ENUMERATE_FLAGS, CredEnumerateW};
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let filter: Vec<u16> = "KyberNow:*".encode_utf16().chain(Some(0)).collect();
    unsafe {
        if CredEnumerateW(PCWSTR(filter.as_ptr()), Some(CRED_ENUMERATE_FLAGS(0)), &mut count, &mut list).is_err() {
            return 0;
        }
        let creds = std::slice::from_raw_parts(list, count as usize);
        let targets: Vec<PCWSTR> = creds.iter().map(|c| PCWSTR((**c).TargetName.0)).collect();
        let mut n = 0;
        for t in targets {
            if CredDeleteW(t, CRED_TYPE_GENERIC, None).is_ok() {
                n += 1;
            }
        }
        CredFree(list as *const _);
        n
    }
}

pub fn delete_password(id: &str) {
    let target = target(id);
    unsafe {
        let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None);
    }
}

// MARK: - Comptes KyberNow

/// Préfixe des comptes dans le gestionnaire d'identifiants : hors du filtre
/// `KyberNow:*` de [`delete_all_passwords`] (les comptes ont le leur,
/// [`delete_all_accounts`]).
const ACCOUNT_PREFIX: &str = "KyberNowCompte:";
/// Taille la plus grande d'un secret du gestionnaire (`CRED_MAX_CREDENTIAL_BLOB_SIZE`).
const MAX_BLOB: usize = 5 * 512;

fn account_target(key: &str) -> Vec<u16> {
    wide(&format!("{ACCOUNT_PREFIX}{key}"))
}

/// Garde le compte entier (JSON en UTF-8, jeton compris) sous sa cible
/// (`Account::key`), en remplaçant celui qui y était.
pub fn save_account(account: &Account) -> bool {
    let Ok(json) = serde_json::to_vec(account) else { return false };
    if json.len() > MAX_BLOB {
        log::error!("comptes : {} trop long pour le gestionnaire d'identifiants ({} octets)", account.key(), json.len());
        return false;
    }
    let mut target = account_target(&account.key());
    // Le nom montré par le gestionnaire d'identifiants : l'e-mail, sinon l'identifiant.
    let shown = if account.account.email.is_empty() { &account.account.id } else { &account.account.email };
    let mut user = wide(shown);
    let cred = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: json.len() as u32,
        CredentialBlob: json.as_ptr() as *mut u8,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user.as_mut_ptr()),
        LastWritten: FILETIME::default(),
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0).is_ok() }
}

/// Les comptes gardés, dans l'ordre de leur cible. Un compte illisible est
/// écarté (et dit au journal, sans son contenu).
pub fn accounts() -> Vec<Account> {
    use windows::Win32::Security::Credentials::{CRED_ENUMERATE_FLAGS, CredEnumerateW};
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let filter = wide(&format!("{ACCOUNT_PREFIX}*"));
    let mut out = Vec::new();
    unsafe {
        // Aucun compte : ERROR_NOT_FOUND.
        if CredEnumerateW(PCWSTR(filter.as_ptr()), Some(CRED_ENUMERATE_FLAGS(0)), &mut count, &mut list).is_err() {
            return out;
        }
        for c in std::slice::from_raw_parts(list, count as usize) {
            let c = &**c;
            if c.CredentialBlob.is_null() || c.CredentialBlobSize == 0 {
                continue;
            }
            let bytes = std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize);
            match serde_json::from_slice::<Account>(bytes) {
                Ok(a) if !a.token.is_empty() => out.push(a),
                Ok(_) => log::warn!("comptes : un compte gardé sans jeton, écarté"),
                Err(e) => log::warn!("comptes : un compte gardé illisible, écarté ({e})"),
            }
        }
        CredFree(list as *const _);
    }
    out.sort_by_key(|a| a.key());
    out
}

/// Oublie le compte (`Account::key`).
pub fn delete_account(key: &str) {
    let target = account_target(key);
    unsafe {
        let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None);
    }
}

/// Oublie tous les comptes (désinstallation), lisibles ou non. Rend leur nombre.
pub fn delete_all_accounts() -> usize {
    use windows::Win32::Security::Credentials::{CRED_ENUMERATE_FLAGS, CredEnumerateW};
    let mut count = 0u32;
    let mut list: *mut *mut CREDENTIALW = std::ptr::null_mut();
    let filter = wide(&format!("{ACCOUNT_PREFIX}*"));
    unsafe {
        if CredEnumerateW(PCWSTR(filter.as_ptr()), Some(CRED_ENUMERATE_FLAGS(0)), &mut count, &mut list).is_err() {
            return 0;
        }
        let creds = std::slice::from_raw_parts(list, count as usize);
        let targets: Vec<PCWSTR> = creds.iter().map(|c| PCWSTR((**c).TargetName.0)).collect();
        let mut n = 0;
        for t in targets {
            if CredDeleteW(t, CRED_TYPE_GENERIC, None).is_ok() {
                n += 1;
            }
        }
        CredFree(list as *const _);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::Computer;

    #[test]
    fn plafond_de_l_encodeur_dans_computers_json() {
        // Fichier d'avant le plafond : aucun, et rien n'est écrit sans lui.
        let old = r#"{"id":"a","name":"Atelier","host":"192.168.1.10","port":31544,"user":"link","fingerprint":null}"#;
        let mut c: Computer = serde_json::from_str(old).unwrap();
        assert_eq!(c.encoder_max, None);
        assert!(serde_json::to_value(&c).unwrap().get("encoder_max").is_none());
        // Gardé d'un lancement à l'autre.
        c.encoder_max = Some(150);
        let back: Computer = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.encoder_max, Some(150));
    }
}
