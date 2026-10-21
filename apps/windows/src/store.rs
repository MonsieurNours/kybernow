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
    pub vsync: bool,
    /// `auto`, `bureau` ou `jeu`.
    pub mouse: String,
    /// `xbox360` ou `fidele`.
    pub gamepad: String,
    pub clipboard: bool,
    /// `auto`, `2`, `6`, `8` ou `0` (son laissé sur l'ordinateur).
    pub audio: String,
    pub virtual_display: bool,
    /// Sur batterie : `alleger` (30 Hz au plus) ou `garder`.
    pub battery: String,
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
            vsync: true,
            mouse: "auto".into(),
            gamepad: "xbox360".into(),
            clipboard: true,
            audio: "auto".into(),
            virtual_display: true,
            battery: "alleger".into(),
        }
    }
}

/// Dernière session, pour l'accueil.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastSession {
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub seconds: u64,
    /// « Bureau · HEVC 4:4:4 ».
    pub label: String,
    /// Fin, en secondes depuis 1970.
    pub at: u64,
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

pub fn delete_password(id: &str) {
    let target = target(id);
    unsafe {
        let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None);
    }
}
