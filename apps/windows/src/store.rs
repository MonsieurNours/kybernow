//! Les ordinateurs de l'app : liste et réglages dans
//! `%APPDATA%\KyberNow\computers.json`, mots de passe dans le gestionnaire
//! d'identifiants de Windows (`KyberNow:<id>`).

use std::path::PathBuf;

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
    /// `jeu`, `bureau`, `max` ; `perso` dès qu'un réglage s'écarte du profil.
    pub profile: String,
    pub codec: Option<String>,
    pub yuv444: Option<bool>,
    pub mbps: Option<u32>,
    pub reliable: Option<bool>,
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: "jeu".into(),
            codec: None,
            yuv444: None,
            mbps: None,
            reliable: None,
            max_4k: false,
            custom: None,
            displays: "auto".into(),
            vsync: true,
            mouse: "auto".into(),
            gamepad: "xbox360".into(),
            clipboard: true,
            audio: "auto".into(),
            virtual_display: true,
            battery: "alleger".into(),
            keep_last_image: true,
            auto_reconnect: true,
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
}

pub struct Store {
    path: PathBuf,
    pub computers: Vec<Computer>,
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
        Self { path, computers }
    }

    pub fn save(&self) {
        use std::io::Write;
        let tmp = self.path.with_extension("json.tmp");
        let Ok(text) = serde_json::to_string_pretty(&self.computers) else { return };
        // Écrit et posé sur le disque avant de remplacer l'ancien.
        let written = std::fs::File::create(&tmp).and_then(|mut f| {
            f.write_all(text.as_bytes())?;
            f.sync_all()
        });
        match written {
            Ok(()) => {
                if let Err(e) = std::fs::rename(&tmp, &self.path) {
                    log::error!("computers.json : {e}");
                }
            }
            Err(e) => log::error!("computers.json : {e}"),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Computer> {
        self.computers.iter().find(|c| c.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Computer> {
        self.computers.iter_mut().find(|c| c.id == id)
    }

    /// Ajoute ou remplace (même adresse et même port).
    pub fn upsert(&mut self, mut c: Computer) -> String {
        let id = if let Some(old) = self.computers.iter_mut().find(|o| o.host == c.host && o.port == c.port) {
            c.id = old.id.clone();
            c.settings = old.settings.clone();
            c.last = old.last.take();
            // Une Station plus ancienne ne donne pas ses cartes : on garde les connues.
            if c.wake.is_empty() {
                c.wake = std::mem::take(&mut old.wake);
            }
            c.station_notified = old.station_notified.take();
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
