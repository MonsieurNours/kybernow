//! Ce que l'app garde : les ordinateurs (adresse d'Admin, empreinte, compte),
//! leurs réglages, la dernière session, l'identité de ce téléviseur et les
//! réglages de l'app. Un fichier JSON dans le dossier de l'app (`Store::dir` :
//! `/media/developer/temp/kybernow` en mode développeur).
//!
//! Les mots de passe sont dans ce fichier, en clair : webOS n'a pas de
//! trousseau accessible aux apps natives. Le compte est propre à cet appareil
//! (accordé par le code à six chiffres) et se retire dans Admin.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Profils, dans le vocabulaire du cœur (`Profile::parse`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    #[default]
    Jeu,
    Bureau,
    Max,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Jeu, Profile::Bureau, Profile::Max];

    pub fn id(self) -> &'static str {
        match self {
            Profile::Jeu => "jeu",
            Profile::Bureau => "bureau",
            Profile::Max => "max",
        }
    }

    /// Transport fiable d'office : Bureau, comme `StreamSettings::for_profile`
    /// du cœur (l'interface se compile sans lui, pour les captures).
    pub fn reliable(self) -> bool {
        self == Profile::Bureau
    }
}

/// Réglages d'un ordinateur, comme Réglages › par ordinateur sur les autres apps.
/// Le profil donne le format, le débit et le transport ; en choisir un autre
/// à la main (`codec`, `mbps`, `transport`) rend le profil « Personnalisé ».
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HostSettings {
    pub profile: Profile,
    /// 0 : comme ce téléviseur ; sinon hauteur (2160, 1440, 1080, 720).
    pub resolution: u32,
    /// Images par seconde : 30, 60, 120 (0 : la fréquence de la dalle).
    pub fps: u32,
    /// `auto`, `hevc`, `h264`, `av1`.
    pub codec: String,
    /// 0 : celui du profil ; plafonné dans tous les cas par le réseau
    /// (`NetInfo::cap_mbps` : 80 Mbit/s en Ethernet à 100 Mbit/s, 95 sinon).
    pub mbps: u32,
    /// `auto` (stéréo), `stereo`, `dolby` (5.1 en E-AC-3), `surround` (5.1 en
    /// PCM), `computer` (le son reste sur l'ordinateur).
    pub audio: String,
    /// Toutes les manettes vues comme des Xbox 360 par l'ordinateur.
    pub xbox360: bool,
    /// L1 + R1 + Start + Select ouvrent le menu.
    pub combo: bool,
    /// Bouton Retour de la Magic Remote : ouvre le menu (sinon Échap).
    pub back_opens_menu: bool,
    /// Sans effet : pas de presse-papiers sur webOS (la session le coupe).
    pub clipboard: bool,
    pub reconnect: bool,
    /// Transport de l'image : `""` celui du profil, `fast` ou `reliable`.
    pub transport: String,
    /// Copie de `transport == "reliable"` : le seul champ que lisent les
    /// versions 0.4.x (« Transport fiable » forcé).
    pub reliable: bool,
    /// Rappeler, dans le menu de session, de passer la télé en Optimiseur de jeu.
    pub game_mode_hint: bool,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            profile: Profile::Jeu,
            resolution: 0,
            fps: 0,
            codec: "auto".into(),
            mbps: 0,
            audio: "auto".into(),
            xbox360: true,
            combo: true,
            back_opens_menu: true,
            clipboard: true,
            reconnect: true,
            transport: String::new(),
            reliable: false,
            game_mode_hint: true,
        }
    }
}

impl HostSettings {
    /// Les réglages s'écartent du profil : « Personnalisé ».
    pub fn is_custom(&self) -> bool {
        self.codec != "auto" || self.mbps != 0 || !self.transport.is_empty()
    }

    /// Choisit un profil : format, débit et transport reviennent aux siens.
    pub fn apply(&mut self, p: Profile) {
        self.profile = p;
        self.codec = "auto".into();
        self.mbps = 0;
        self.set_transport("");
    }

    /// Transport fiable pour la session : le choix, sinon celui du profil.
    pub fn reliable_transport(&self) -> bool {
        match self.transport.as_str() {
            "reliable" => true,
            "fast" => false,
            _ => self.profile.reliable(),
        }
    }

    pub fn set_transport(&mut self, t: &str) {
        self.transport = t.into();
        self.reliable = t == "reliable";
    }
}

/// La dernière session (au moins 3 s) : latence médiane et 95e centile (ms),
/// durée (s), date (secondes Unix), définition, fréquence, format et débit
/// moyen. Les fichiers de la 0.4.x n'ont que `mode` (« 3840 × 2160 · HEVC »)
/// à la place de la définition et du format : les champs nouveaux sont
/// facultatifs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct LastSession {
    pub median: f64,
    pub p95: f64,
    pub seconds: u64,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub mbps: f64,
    pub at: u64,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// Fréquence de l'écran de la session (Hz).
    #[serde(default)]
    pub refresh: u32,
    /// `HEVC`, `AV1`, `H.264`.
    #[serde(default)]
    pub codec: String,
}

/// Un ordinateur équipé de KyberNow Station.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Host {
    pub id: String,
    pub name: String,
    /// Adresse et port d'Admin.
    pub address: String,
    pub port: u16,
    /// Empreinte du certificat d'Admin (`sha256:…`), épinglée.
    pub fingerprint: Option<String>,
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub gpu: Option<String>,
    #[serde(default)]
    pub av1: bool,
    /// Adresse matérielle pour le réveil, écrite à la main (aucun écran ne
    /// la saisit au téléviseur ; lue si un fichier en porte une).
    #[serde(default)]
    pub mac: Option<String>,
    /// Les cartes réseau de l'ordinateur pour le réveil (Wake-on-LAN), telles
    /// qu'Admin les donne (`wake` d'`info`) : gardées au dernier sondage
    /// réussi qui en donne, pour le réveiller une fois éteint.
    #[serde(default)]
    pub wake: Vec<WakeTarget>,
    #[serde(default)]
    pub settings: HostSettings,
    #[serde(default)]
    pub last: Option<LastSession>,
}

impl Host {
    /// L'app sait réveiller cet ordinateur : des cartes données par Admin, ou
    /// une adresse MAC écrite à la main.
    pub fn can_wake(&self) -> bool {
        !self.wake.is_empty() || self.mac.as_deref().is_some_and(|m| !m.trim().is_empty())
    }

    /// L'adresse MAC montrée dans les réglages : celle de la première carte
    /// (Ethernet d'abord, dans l'ordre d'Admin), sinon celle écrite à la main.
    pub fn wake_mac(&self) -> Option<String> {
        self.wake.first().map(|t| t.mac.clone()).or_else(|| self.mac.clone().filter(|m| !m.trim().is_empty()))
    }
}

/// Une carte réseau de l'ordinateur à réveiller, au format JSON de
/// `kybernow_core::wake::WakeTarget` (le même dans `kybernow.json` que dans
/// la réponse d'Admin) ; l'interface se compile sans le cœur, pour les captures.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WakeTarget {
    pub mac: String,
    #[serde(default)]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub broadcast: Option<String>,
    /// `ethernet` ou `wifi` (vide : inconnu).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub armed: Option<bool>,
    #[serde(default)]
    pub name: String,
}

/// Réglages de l'app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// `auto`, `fr`, `en`.
    pub language: String,
    /// Mises à jour : aussi les bêtas (sinon les versions stables seules).
    pub beta: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { language: "auto".into(), beta: false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    /// Identifiant de ce téléviseur auprès d'Admin, tiré une fois.
    pub device_id: String,
    pub hosts: Vec<Host>,
    /// Dernier ordinateur choisi sur l'accueil.
    pub selected: Option<String>,
    pub app: AppSettings,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Store {
    /// Le dossier de l'app, choisi une fois : le premier où l'on peut
    /// écrire parmi `$HOME/conf`, `/media/developer/temp/kybernow` et
    /// `/tmp/kybernow`. En mode développeur, `HOME` vaut `/media/developer`,
    /// qui n'est pas ouvert à l'utilisateur de l'app ; `temp` l'est, survit au
    /// redémarrage et se lit en ssh (le `/tmp` de l'app est vidé au redémarrage
    /// et invisible en ssh). Ce qui était dans `/tmp/kybernow` y est recopié.
    pub fn dir() -> PathBuf {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let tmp = PathBuf::from("/tmp/kybernow");
            let mut candidates = Vec::new();
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home).join("conf"));
            }
            candidates.push(PathBuf::from("/media/developer/temp/kybernow"));
            candidates.push(tmp.clone());
            let d = candidates.into_iter().find(|d| writable(d)).unwrap_or_else(|| tmp.clone());
            if d != tmp && !d.join("kybernow.json").exists() && tmp.join("kybernow.json").exists() {
                let _ = std::fs::copy(tmp.join("kybernow.json"), d.join("kybernow.json"));
            }
            d
        })
        .clone()
    }

    pub fn load() -> Self {
        let path = Self::dir().join("kybernow.json");
        let mut s: Store = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if s.device_id.is_empty() {
            s.device_id = new_id();
        }
        // Fichier de la 0.4.x : « Transport fiable » forcé devient le choix
        // « Fiable », sauf en Bureau, où il l'est déjà.
        for h in &mut s.hosts {
            if h.settings.reliable && h.settings.transport.is_empty() {
                let t = if h.settings.profile.reliable() { "" } else { "reliable" };
                h.settings.set_transport(t);
            }
        }
        s.path = Some(path);
        s.save();
        s
    }

    /// Écrit le fichier (par un fichier temporaire renommé : jamais à moitié écrit).
    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let Ok(bytes) = serde_json::to_vec_pretty(self) else { return };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    pub fn host(&self, id: &str) -> Option<&Host> {
        self.hosts.iter().find(|h| h.id == id)
    }

    pub fn host_mut(&mut self, id: &str) -> Option<&mut Host> {
        self.hosts.iter_mut().find(|h| h.id == id)
    }

    /// Ajoute un ordinateur, ou met à jour celui de même adresse.
    pub fn put(&mut self, host: Host) -> String {
        if let Some(h) = self.hosts.iter_mut().find(|h| h.address == host.address && h.port == host.port) {
            let id = h.id.clone();
            let settings = h.settings.clone();
            let last = h.last.clone();
            let wake = if host.wake.is_empty() { h.wake.clone() } else { host.wake.clone() };
            *h = Host { id: id.clone(), settings, last, wake, ..host };
            return id;
        }
        let id = host.id.clone();
        self.hosts.push(host);
        id
    }

    pub fn remove(&mut self, id: &str) {
        self.hosts.retain(|h| h.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }

    /// Aperçu d'Admin gardé pour l'accueil (un fichier par ordinateur).
    pub fn image_path(id: &str) -> PathBuf {
        let d = Self::dir().join("images");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{id}.jpg"))
    }
}

/// On peut créer le dossier et y écrire.
fn writable(d: &std::path::Path) -> bool {
    let probe = d.join(".probe");
    let ok = std::fs::create_dir_all(d).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

/// Un identifiant aléatoire au format UUID v4.
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        b.copy_from_slice(&t.to_le_bytes());
    }
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Un code à six chiffres pour l'ajout par code (accepté dans Admin).
pub fn pair_code() -> String {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    format!("{:06}", u32::from_le_bytes(b) % 1_000_000)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
