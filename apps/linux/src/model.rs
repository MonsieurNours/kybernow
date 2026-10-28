//! Ce que l'app garde : les ordinateurs (adresse d'Admin, empreinte, compte),
//! leurs réglages, la dernière session, l'identité de cet appareil et les
//! réglages de l'app. Un fichier JSON dans le dossier de configuration de
//! l'utilisateur (`Store::dir` : `$XDG_CONFIG_HOME/kybernow`, soit
//! `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow` en Flatpak).
//!
//! Les mots de passe sont dans ce fichier, lisible par l'utilisateur seul
//! (droits 0600) : en mode Jeu de SteamOS, aucun trousseau n'est déverrouillé.
//! Le compte est propre à cet appareil (accordé par le code à six chiffres)
//! et se retire dans Admin.

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
}

/// Réglages d'un ordinateur, comme Réglages › par ordinateur sur les autres apps.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HostSettings {
    pub profile: Profile,
    /// 0 : comme cet écran ; sinon hauteur (2160, 1440, 1080, 800, 720).
    pub resolution: u32,
    /// Écrans pris par la session (multi-écrans, docs/multi-ecrans.md) : `auto`
    /// (tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et
    /// en Personnalisé), `this` (cet écran), `all` (tous les écrans). Gardé
    /// comme le cœur (`DisplaysSetting`) ; ne rend jamais le profil « Personnalisé ».
    pub displays: String,
    /// Images par seconde : 30, 60, 90, 120, 144 (0 : celle de l'écran).
    pub fps: u32,
    /// `auto`, `hevc`, `h264`, `av1`.
    pub codec: String,
    /// 0 : celui du profil ; sinon en Mbit/s.
    pub mbps: u32,
    /// Affichage : `auto` (fréquence variable si l'écran la permet, sinon
    /// synchronisé), `vsync`, `immediate`, `vrr`.
    pub display: String,
    /// `auto`, `stereo`, `5.1`, `7.1`, `computer` (le son reste sur l'ordinateur).
    pub audio: String,
    /// Souris : `auto`, `desktop`, `game`.
    pub mouse: String,
    /// Toutes les manettes vues comme des Xbox 360 par l'ordinateur.
    pub xbox360: bool,
    /// L1 + R1 + Start + Select ouvrent le menu.
    pub combo: bool,
    pub clipboard: bool,
    pub reconnect: bool,
    /// Transport fiable même en profil Jeu (Wi-Fi chargé).
    pub reliable: bool,
    /// Sur batterie : 60 i/s et débit réduit au plus.
    pub battery_light: bool,
    /// Écran virtuel de la Station (sinon les écrans de l'ordinateur tels quels).
    pub virtual_display: bool,
    /// Couleurs 4:4:4 (texte net) en HEVC : en option, la carte (VA-API) ou le
    /// processeur décode, l'app convertit en RVB.
    pub yuv444: bool,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            profile: Profile::Jeu,
            resolution: 0,
            displays: "auto".into(),
            fps: 0,
            codec: "auto".into(),
            mbps: 0,
            display: "auto".into(),
            audio: "auto".into(),
            mouse: "auto".into(),
            xbox360: true,
            combo: true,
            clipboard: true,
            reconnect: true,
            reliable: false,
            battery_light: true,
            virtual_display: true,
            yuv444: false,
        }
    }
}

impl HostSettings {
    /// Les réglages s'écartent du profil (« Personnalisé ») : format ou débit
    /// choisis, 4:4:4 en Jeu (Bureau et Max le prennent quand il est permis),
    /// transport fiable hors Bureau (Bureau l'est déjà).
    pub fn custom(&self) -> bool {
        self.codec != "auto" || self.mbps != 0 || (self.yuv444 && self.profile == Profile::Jeu) || (self.reliable && self.profile != Profile::Bureau)
    }
}

/// La dernière session d'au moins 3 s (Réglages de l'ordinateur ›
/// Statistiques) : latence médiane et 95e centile (ms, 0 : non mesurée),
/// durée (s), date, définition, fréquence, format et débit moyen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct LastSession {
    pub median: f64,
    pub p95: f64,
    pub seconds: u64,
    /// « 1920 × 1200 » ; les fichiers de la 0.4.x y mettent aussi le format
    /// (« 1920 × 1200 · HEVC ») et n'ont ni `hz` ni `codec`.
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub mbps: f64,
    pub at: u64,
    /// Fréquence (Hz), 0 : inconnue.
    #[serde(default)]
    pub hz: u32,
    /// « HEVC », « H.264 », « AV1 » ; vide : inconnu.
    #[serde(default)]
    pub codec: String,
    /// Écrans pris par la session (0 ou 1 : un seul) ; avec plusieurs, la
    /// définition et la fréquence sont celles de l'écran principal, le débit
    /// le total.
    #[serde(default)]
    pub screens: u32,
    /// Le chemin de la session : `direct`, `relay` (par le relais) ou `relay_tcp`
    /// (par le relais, repli TCP) ; vide : direct, ou une session d'avant le relais
    /// (le `RouteInfo` du cœur, ramené à son transport).
    #[serde(default)]
    pub route: String,
}

impl LastSession {
    /// Ce qui a été transmis : « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » ; avec
    /// plusieurs écrans, leur nombre d'abord : « 3 écrans · 3840 × 2160 · 160 Hz
    /// · HEVC · 120 Mbit/s » (docs/link-ui.md, « Statistiques »).
    pub fn transmitted(&self) -> String {
        let mut v: Vec<String> = Vec::new();
        if self.screens >= 2 {
            v.push(format!("{} {}", self.screens, crate::i18n::tr("écrans", "displays")));
        }
        if !self.mode.is_empty() {
            v.push(self.mode.clone());
        }
        if self.hz > 0 {
            v.push(format!("{} Hz", self.hz));
        }
        if !self.codec.is_empty() {
            v.push(self.codec.clone());
        }
        if self.mbps > 0.0 {
            v.push(format!("{:.0} Mbit/s", self.mbps));
        }
        v.join(" · ")
    }
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
    /// Adresse MAC saisie à la main (Réglages › Réseau, réglage avancé) :
    /// le réveil s'en sert quand la Station n'a pas donné ses cartes.
    #[serde(default)]
    pub mac: Option<String>,
    /// Les cartes réseau de l'ordinateur pour le réveil, telles qu'Admin les
    /// donne (`wake` de `GET /api/stream/info`, Station 0.5.0 et suivantes),
    /// gardées d'un `info` réussi à l'autre (une liste vide ne les efface pas).
    #[serde(default)]
    pub wake: Vec<WakeCard>,
    /// Le relais de l'ordinateur (`relay` de `GET /api/stream/info`), gardé d'un `info` réussi à l'autre : de quoi
    /// le joindre hors de chez soi (`Route::Auto`) et le dire « Hors de chez vous » sur l'accueil.
    #[serde(default)]
    pub relay: Option<HostRelay>,
    #[serde(default)]
    pub settings: HostSettings,
    #[serde(default)]
    pub last: Option<LastSession>,
}

/// Le relais d'un ordinateur, tel que la Station le donne (`RelayInfo` du cœur) : le relais (`hôte:port`, ou
/// `hôte:port#sha256:…` pour un relais épinglé), l'identifiant de la Station chez lui, et « Accès hors de chez
/// vous » (actif dans Admin ?). Sans lui, l'ordinateur ne se joint que sur le réseau.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct HostRelay {
    pub server: String,
    pub id: String,
    pub remote_access: bool,
}

/// Une carte réseau de l'ordinateur à réveiller, au JSON de `WakeTarget` du
/// cœur (`apps/core/src/wake.rs`) : l'app la garde telle qu'Admin la donne.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct WakeCard {
    /// `AA:BB:CC:DD:EE:FF`.
    pub mac: String,
    pub ipv4: Option<String>,
    pub broadcast: Option<String>,
    /// `ethernet`, `wifi`, ou vide.
    pub kind: String,
    pub armed: Option<bool>,
    pub name: String,
}

impl Host {
    /// L'app sait réveiller cet ordinateur : des cartes apprises d'Admin, ou une MAC saisie.
    pub fn can_wake(&self) -> bool {
        !self.wake.is_empty() || self.mac.is_some()
    }
}

/// Réglages de l'app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// `auto`, `fr`, `en`.
    pub language: String,
    /// Mises à jour : aussi les bêtas (sinon les versions stables seules).
    pub beta: bool,
    /// Interface : `auto` (console sous gamescope, bureau ailleurs),
    /// `desktop`, `handheld`, `tv`.
    pub interface: String,
    /// Garder la dernière image de chaque session pour l'accueil.
    pub keep_last_image: bool,
    /// KyberNow Link OS : le premier démarrage (manette, langue, réseau, usage,
    /// ordinateur) est fait.
    pub os_setup_done: bool,
    /// KyberNow Link OS : HDMI-CEC, tout ou rien : l'annonce du nom
    /// « KyberNow Link » au téléviseur au lancement (qui donne aussi à
    /// l'appareil son adresse CEC, sans laquelle la télécommande ne lui parle
    /// pas), la veille et le réveil du téléviseur avec l'appareil.
    pub os_cec: bool,
    /// KyberNow Link OS : télécharger les versions dès qu'elles sortent ;
    /// l'installation attend « Redémarrer sur la X ».
    pub os_auto_update: bool,
    /// KyberNow Link OS : mode de la sortie HDMI au démarrage : `auto`,
    /// `2160p60`, `1080p120`, `1080p60`.
    pub os_hdmi: String,
    /// KyberNow Share : le prénom donné en rejoignant un partage, retenu pour
    /// la fois suivante (vide : le nom de cet appareil est proposé). Avec la qualité de l'image ci-dessous, la
    /// seule trace d'un partage sur l'appareil : un invité n'est jamais ajouté aux ordinateurs.
    pub share_name: String,
    /// KyberNow Share : la « Qualité de l'image » choisie en rejoignant un partage ou dans la barre, retenue
    /// pour la fois suivante (`auto`, `host`, `2160p60`, `1080p60`, `720p30` ; vide ou inconnue : « Auto »).
    pub share_quality: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { language: "auto".into(), beta: false, interface: "auto".into(), keep_last_image: true, os_setup_done: false, os_cec: true, os_auto_update: true, os_hdmi: "auto".into(), share_name: String::new(), share_quality: String::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    /// Identifiant de cet appareil auprès d'Admin, tiré une fois.
    pub device_id: String,
    pub hosts: Vec<Host>,
    /// Dernier ordinateur choisi sur l'accueil.
    pub selected: Option<String>,
    pub app: AppSettings,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Store {
    /// Le dossier de l'app : `$XDG_CONFIG_HOME/kybernow` (Flatpak :
    /// `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow`), sinon
    /// `~/.config/kybernow`, sinon `/tmp/kybernow`.
    pub fn dir() -> PathBuf {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let mut candidates = Vec::new();
            if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                candidates.push(PathBuf::from(x).join("kybernow"));
            }
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home).join(".config/kybernow"));
            }
            candidates.push(PathBuf::from("/tmp/kybernow"));
            candidates.into_iter().find(|d| writable(d)).unwrap_or_else(|| PathBuf::from("/tmp/kybernow"))
        })
        .clone()
    }

    pub fn load() -> Self {
        let path = Self::dir().join("kybernow.json");
        let mut s: Store = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if s.device_id.is_empty() {
            s.device_id = new_id();
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
        // Lisible par l'utilisateur seul : le fichier porte les mots de passe.
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let written = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).and_then(|mut f| f.write_all(&bytes));
        if written.is_ok() {
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
            // Ajouté à nouveau : ses réglages, sa dernière session et de quoi le réveiller restent.
            let wake = if host.wake.is_empty() { h.wake.clone() } else { host.wake.clone() };
            let mac = host.mac.clone().or_else(|| h.mac.clone());
            let relay = host.relay.clone().or_else(|| h.relay.clone());
            *h = Host { id: id.clone(), settings, last, wake, mac, relay, ..host };
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

    /// Dossier des dernières images de session (une par ordinateur).
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
