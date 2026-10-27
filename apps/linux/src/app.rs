//! L'état de l'interface et ce que font les touches, sans SDL ni cœur : les
//! écrans se dessinent aussi sans l'app (captures, `kybernow --preview`).
//!
//! Trois interfaces, d'après les maquettes (https://claude.ai/artifact/MgBJNBJ4WEMFvVB6Ujfxhc) :
//! **bureau** (fenêtre, souris et clavier), **portable** (Steam Deck et
//! consoles sous gamescope, 1280 × 800, à la manette) et **salon** (SteamOS
//! au téléviseur, à trois mètres). Chaque élément qu'on peut choisir est une
//! `Target` ; les écrans les inscrivent avec leur rectangle à chaque dessin.
//! Les flèches vont au plus proche dans leur direction (navigation spatiale),
//! la souris à ce qu'elle survole. Les choix rendent des `Action`, que le
//! pilote (`device`) exécute.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::i18n::tr;
use crate::model::{Host, Profile, Store};

/// Un rectangle, en points de l'interface (ceux des maquettes).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// L'interface, d'après l'écran et le compositeur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Fenêtre de bureau, souris et clavier (maquettes « Bureau »).
    Desktop,
    /// Steam Deck et consoles portables, à la manette (maquettes « Steam Deck »).
    Handheld,
    /// SteamOS au téléviseur, à la manette, à trois mètres (maquettes « salon »).
    Tv,
}

impl Layout {
    pub fn console(self) -> bool {
        self != Layout::Desktop
    }
}

/// Une touche de navigation : manette, clavier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    /// Y : réglages de l'ordinateur.
    Options,
    /// X : ajouter un ordinateur.
    Add,
    /// Sélection (⧉) : « Rejoindre avec un code » (KyberNow Share).
    Join,
    /// L1 / R1 : profil précédent, suivant (accueil) ; rubrique (réglages).
    PrevTab,
    NextTab,
    /// Start (☰) : réglages de l'app.
    Menu,
    Char(char),
    Erase,
}

/// État d'un ordinateur vu sur le réseau (sondage d'Admin).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Online {
    #[default]
    Unknown,
    Online,
    Offline,
}

#[derive(Clone, Debug, Default)]
pub struct HostLive {
    pub online: Online,
    /// La Station a un écran virtuel (`None` : pas encore su).
    pub virtual_display: Option<bool>,
    /// La Station, depuis sa dernière réponse (`None` : jamais répondu).
    pub station: Option<StationLive>,
    /// Le compte de cet appareil a été créé par l'appairage par code (Admin
    /// 0.5.0 et suivantes ; `None` : Admin ne le dit pas).
    pub own_device: Option<bool>,
    /// Écrans que la Station sait créer (`virtual_display.max_screens` de
    /// `GET /api/stream/info`, multi-écrans) ; `None` : pas encore su, ou une
    /// Station antérieure, qui n'en crée qu'un.
    pub max_screens: Option<usize>,
    /// KyberNow Share : ce que « Partager » peut faire avec cet ordinateur
    /// (`Info::share`) ; `None` : pas encore su.
    pub share: Option<ShareAvail>,
}

/// Multi-écrans (docs/multi-ecrans.md) : ce que la session prendra pour
/// l'ordinateur choisi, calculé par la couche appareil avec le cœur (le même
/// calcul que la session : réglage « Écrans », profil, définition sur mesure,
/// écran virtuel, écrans de l'appareil et ce que la Station sait créer).
/// L'accueil et la ligne « Écrans » des réglages le lisent.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreensPlan {
    /// Écrans pris : 1 (« Cet écran »), ou au moins 2.
    pub count: usize,
    /// Le réglage « Écrans » se change ; sinon il est inactif, sur « Cet écran ».
    pub enabled: bool,
    /// La valeur que les réglages montrent : 0 Auto, 1 Cet écran, 2 Tous les écrans.
    pub shown: usize,
}

impl Default for ScreensPlan {
    fn default() -> Self {
        Self { count: 1, enabled: true, shown: 0 }
    }
}

/// La Station d'un ordinateur, d'après `GET /api/stream/info` (cœur :
/// `Info::station`, absent avant la 0.5.0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationLive {
    /// Version de la Station (celle d'Admin), vide si elle ne la dit pas.
    pub version: String,
    /// La Station se redémarre et se met à jour depuis un appareil (0.5.0 et suivantes).
    pub supported: bool,
    /// Une version plus récente est publiée.
    pub available: bool,
    pub update_version: Option<String>,
    /// Elle s'installe depuis un appareil.
    pub installable: bool,
    /// Elle s'installe sur l'ordinateur, avec l'installateur.
    pub requires_setup: bool,
    pub installing: bool,
    pub restarting: bool,
}

impl StationLive {
    /// Une mise à jour à lancer depuis cet appareil : sa version.
    pub fn installable_version(&self) -> Option<&str> {
        (self.supported && self.available && self.installable && !self.requires_setup).then(|| self.update_version.as_deref().unwrap_or(""))
    }
}

/// Redémarrer ou mettre à jour la Station.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationKind {
    Restart,
    Update,
}

/// Où en est un redémarrage ou une mise à jour de la Station.
#[derive(Clone, Debug, PartialEq)]
pub enum StationStep {
    /// La demande est partie (une mise à jour : Admin télécharge et vérifie d'abord).
    Asking,
    /// Admin a accepté : la Station s'arrête puis revient ; l'app la sonde
    /// toutes les 2 s. `target` : la version attendue (mise à jour).
    Waiting { since: Instant, target: Option<String>, saw_down: bool },
    Done(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StationOp {
    pub host: String,
    pub kind: StationKind,
    pub step: StationStep,
}

impl StationOp {
    pub fn busy(&self) -> bool {
        matches!(self.step, StationStep::Asking | StationStep::Waiting { .. })
    }
}

/// Un refus d'Admin, pour l'interface (cœur : `StationError`).
#[derive(Clone, Debug, PartialEq)]
pub enum StationRefusal {
    /// Un appareil est en session (son nom, si Admin le donne).
    InSession(Option<String>),
    TooSoon,
    RequiresSetup,
    /// Station 0.4.9 ou plus ancienne.
    Unsupported,
    /// Le reste, avec le message d'Admin ou du cœur.
    Other(String),
}

/// Une confirmation par-dessus les réglages.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirm {
    /// Retirer l'ordinateur de cet appareil.
    Remove { host: String, name: String },
    /// Un appareil est en session : recommencer avec `force` coupe sa session.
    Force { host: String, kind: StationKind, device: String },
}

/// Une Station trouvée sur le réseau (mDNS `_kybernow._tcp`), pas encore ajoutée.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NetKind {
    #[default]
    Unknown,
    Ethernet,
    Wifi,
}

/// Le réseau de cet appareil.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetInfo {
    pub kind: NetKind,
    /// Vitesse du lien en Mbit/s (Ethernet), ou débit du Wi-Fi.
    pub link_mbps: Option<u32>,
    /// Bande du Wi-Fi (« 5 GHz »).
    pub band: Option<String>,
}

/// Ce que sait faire cet appareil (Réglages de l'app › « Cet appareil »).
#[derive(Clone, Debug)]
pub struct Machine {
    /// « Fedora Linux 42 », « SteamOS 3.7 ».
    pub os: String,
    /// Bureau : « GNOME 48, Wayland », « gamescope ».
    pub desktop: String,
    /// « Steam Deck OLED », nom du produit (DMI), ou vide.
    pub model: String,
    /// La carte graphique (pilote VA-API ou DRM).
    pub gpu: String,
    /// Décodés par la carte graphique (VA-API).
    pub hw_h264: bool,
    pub hw_hevc: bool,
    pub hw_av1: bool,
    /// HEVC 4:4:4 décodé par la carte (Intel récentes) ; sinon le processeur.
    pub hw_hevc444: bool,
    /// VA-API présent.
    pub vaapi: bool,
    /// Écran : définition et fréquence (celles de la fenêtre).
    pub display: (u32, u32),
    pub refresh: u32,
    pub display_name: String,
    /// L'écran branché annonce la fréquence variable (DRM `vrr_capable`) ;
    /// `None` : on ne sait pas (pas de DRM lisible).
    pub vrr: Option<bool>,
    /// Sortie son : voies, nom et fréquence (Hz, 0 : inconnue).
    pub audio_channels: u32,
    pub audio_name: String,
    pub audio_rate: u32,
    /// Batterie : pourcentage, sur secteur ou non.
    pub battery: Option<(u32, bool)>,
    /// Un Raspberry Pi (arbre des périphériques), avec ce qu'il décode par V4L2.
    pub pi: Option<Pi>,
    /// Température du processeur (°C), lue toutes les quelques secondes (Pi).
    pub temp_c: Option<f32>,
    /// Modes de l'écran branché, lus par DRM : fréquence la plus haute en
    /// 1080p, et le plus grand mode (largeur, hauteur, fréquence) ; 0 : inconnus.
    pub max_hz_1080p: u32,
    pub max_mode: (u32, u32, u32),
}

/// Un Raspberry Pi : modèle court, génération (famille de la puce), mémoire,
/// décodeurs V4L2 présents (`/sys/class/video4linux`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pi {
    /// « Raspberry Pi 5 », « Raspberry Pi 400 », « Compute Module 5 », « Raspberry Pi Zero 2 W ».
    pub model: String,
    /// 5 : BCM2712 (Pi 5, 500, CM5) ; 4 : BCM2711 (Pi 4, 400, CM4) ; 3 : BCM2837 (Pi 3, Zero 2 W) ; 0 : plus ancien ou inconnu.
    pub generation: u8,
    /// Mémoire vive, en Go (arrondie).
    pub ram_gb: u32,
    /// Décodeur H.264 `bcm2835-codec` (Pi 3 et 4 ; absent du Pi 5).
    pub v4l2_h264: bool,
    /// Décodeur HEVC `rpivid` (Pi 4 et 5), API V4L2 « request ».
    pub v4l2_hevc: bool,
}

impl Pi {
    /// La plus grande hauteur d'image que la sortie HDMI du Pi affiche : 4K
    /// (Pi 4 et 5, HDMI 2.0, 60 Hz) ; 1080p pour les plus anciens (HDMI 1.4).
    pub fn max_height(&self) -> u32 {
        if self.generation >= 4 { 2160 } else { 1080 }
    }

    /// La fréquence la plus haute que le Pi sort à cette hauteur : 4K et 1440p
    /// à 60 Hz ; 1080p à 120 Hz sur les Pi 4 et 5 (bande passante HDMI 2.0, pas
    /// encore vérifié sur une carte), 60 Hz avant.
    pub fn max_hz(&self, height: u32) -> u32 {
        if height <= 1080 && self.generation >= 4 { 120 } else { 60 }
    }

    /// « HDMI 2.0 » / « HDMI 1.4 ».
    pub fn hdmi(&self) -> &'static str {
        if self.generation >= 4 { "HDMI 2.0" } else { "HDMI 1.4" }
    }

    /// Le nom de la carte graphique de la puce : « VideoCore VII ».
    pub fn gpu(&self) -> &'static str {
        match self.generation {
            5 => "VideoCore VII",
            4 => "VideoCore VI",
            _ => "VideoCore IV",
        }
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            os: "Linux".into(),
            desktop: String::new(),
            model: String::new(),
            gpu: String::new(),
            hw_h264: true,
            hw_hevc: true,
            hw_av1: false,
            hw_hevc444: false,
            vaapi: true,
            display: (1280, 800),
            refresh: 60,
            display_name: String::new(),
            vrr: None,
            audio_channels: 2,
            audio_name: "PipeWire".into(),
            audio_rate: 0,
            battery: None,
            pi: None,
            temp_c: None,
            max_hz_1080p: 0,
            max_mode: (0, 0, 0),
        }
    }
}

impl Machine {
    pub fn on_battery(&self) -> bool {
        self.battery.is_some_and(|(_, ac)| !ac)
    }

    /// Les formats décodés, pour les profils : H.264 et HEVC par la carte ou
    /// le processeur ; AV1 par la carte seulement (FFmpeg est compilé sans
    /// décodeur AV1 logiciel).
    pub fn decodes(&self, codec: &str) -> bool {
        match codec {
            "h264" => true,
            // HEVC : par la carte, sinon par le processeur.
            "hevc" => true,
            "av1" => self.hw_av1,
            _ => true,
        }
    }

    /// Un décodeur matériel existe (VA-API, ou V4L2 sur le Pi).
    pub fn hw_any(&self) -> bool {
        self.hw_h264 || self.hw_hevc || self.hw_av1
    }

    /// Le nom du décodeur matériel, pour les étiquettes : « carte graphique », « décodeur du Pi ».
    pub fn hw_name(&self) -> &'static str {
        if self.pi.is_some() { tr("décodeur du Pi", "Pi decoder") } else { tr("carte graphique", "graphics card") }
    }

    /// La plus grande hauteur d'image que cet appareil affiche (Pi : sa sortie HDMI).
    pub fn max_height(&self) -> u32 {
        self.pi.as_ref().map(|p| p.max_height()).unwrap_or(u32::MAX)
    }

    /// La fréquence la plus haute possible pour une hauteur d'image : sur le
    /// Pi, celle de sa sortie, bornée par ce que l'écran annonce en 1080p
    /// (DRM) ; ailleurs, celle de l'écran (au moins 60).
    pub fn max_hz(&self, height: u32) -> u32 {
        match &self.pi {
            Some(p) => {
                let hz = p.max_hz(height);
                if height <= 1080 && self.max_hz_1080p > 0 { hz.min(self.max_hz_1080p.max(self.refresh)) } else { hz.min(self.refresh.max(60)) }
            }
            None => {
                let mut hz = self.refresh.max(60);
                if height <= 1080 {
                    hz = hz.max(self.max_hz_1080p);
                }
                hz
            }
        }
    }

    /// « 1080p », « 4K », « 800p ».
    pub fn height_label(h: u32) -> String {
        if h >= 2160 { "4K".into() } else { format!("{h}p") }
    }
}

/// Une manette branchée (SDL GameController).
#[derive(Clone, Debug, Default)]
pub struct PadInfo {
    /// Joueur (0 à 3).
    pub slot: usize,
    pub name: String,
    pub rumble: bool,
    pub trigger_rumble: bool,
    /// Boutons tenus (bits dans l'ordre de SDL : A, B, X, Y, Back, Guide,
    /// Start, sticks, L1, R1, croix).
    pub buttons: u32,
    /// Sticks et gâchettes (−32768 à 32767).
    pub axes: [i16; 6],
    pub battery: Option<String>,
    /// Moteurs en marche (essai), pour les jauges : 0 à 1.
    pub motors: (f32, f32),
}

/// Étape d'une session, vue par l'interface.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Preparing,
    Connecting,
    FirstImages,
    Streaming,
    Reconnecting { attempt: u32, max: u32 },
    /// Dernier recours de la connexion : le cœur redémarre la Station, puis
    /// reprend du début (« La Station redémarre… »).
    StationRestarting,
    Ending,
    Ended { error: Option<String> },
}

/// Statistiques de la dernière seconde (barre, menu de session).
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub latency_ms: Option<f64>,
    pub fps: f64,
    pub mbps: f64,
    pub lost: u32,
    pub mode: String,
    pub decoder: String,
    pub audio_ms: Option<f64>,
    /// Latences des dernières secondes (courbe de la barre).
    pub history: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Resume,
    Profile(Profile),
    Sound,
    PadMouse,
    Keyboard,
    Keys,
    Stats,
    Light,
    Gamepads,
    /// KyberNow Share : « Partager » (hôte).
    Share,
    /// KyberNow Share, invité : « Qualité de l'image » (la tuile montre la qualité reçue ; ouvre les cinq choix).
    Quality,
    Disconnect,
}

/// Raccourcis du menu Touches (scancodes de Windows, comme sur Android).
pub const SHORTCUTS: [(&str, &str, &[u16]); 10] = [
    ("Ctrl + Alt + Suppr", "Ctrl + Alt + Del", &[0x1D, 0x38, 0xE053]),
    ("Alt + Tab", "Alt + Tab", &[0x38, 0x0F]),
    ("Touche Windows", "Windows key", &[0xE05B]),
    ("Windows + D : bureau", "Windows + D: desktop", &[0xE05B, 0x20]),
    ("Alt + F4", "Alt + F4", &[0x38, 0x3E]),
    ("Échap", "Esc", &[0x01]),
    ("F11 : plein écran", "F11: full screen", &[0x57]),
    ("Gestionnaire des tâches", "Task Manager", &[0x1D, 0x2A, 0x01]),
    ("Impr. écran", "Print Screen", &[0xE037]),
    ("Windows + L : verrouiller", "Windows + L: lock", &[0xE05B, 0x26]),
];

/// Menu déroulant de la barre de session (bureau).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMenu {
    Profile,
    Keys,
    /// Les autres ordinateurs (plusieurs ordinateurs à la fois, docs/plusieurs-ordinateurs.md).
    Computers,
    /// KyberNow Share : le panneau de l'hôte (code, QR code, invités).
    Share,
    /// KyberNow Share, invité : « Qualité de l'image » (les cinq choix).
    Quality,
}

// MARK: - KyberNow Share (docs/kybernow-share.md, textes : docs/link-ui.md, « Partage »)

/// Le rôle d'un invité, que la Station impose : miroir de `kybernow_core::share::ShareRole` (l'interface seule,
/// celle des captures et des tests sur Mac, n'a pas le cœur). Deux droits indépendants, les manettes et le clavier
/// et la souris, que l'hôte donne ou reprend chacun sans toucher à l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShareRole {
    /// Aucun droit : voit et entend, n'envoie rien (le rôle d'arrivée).
    #[default]
    Spectator,
    /// Les manettes : elles deviennent la manette suivante de Windows.
    Player,
    /// Le clavier et la souris, sans les manettes.
    Kbm,
    /// Les manettes, et le clavier et la souris.
    PlayerKbm,
}

impl ShareRole {
    /// Le rôle de ces deux droits.
    pub fn of(pads: bool, kbm: bool) -> Self {
        match (pads, kbm) {
            (false, false) => ShareRole::Spectator,
            (true, false) => ShareRole::Player,
            (false, true) => ShareRole::Kbm,
            (true, true) => ShareRole::PlayerKbm,
        }
    }

    /// Les manettes de l'invité partent vers l'ordinateur.
    pub fn pads(self) -> bool {
        matches!(self, ShareRole::Player | ShareRole::PlayerKbm)
    }

    /// Le clavier et la souris de l'invité partent vers l'ordinateur.
    pub fn kbm(self) -> bool {
        matches!(self, ShareRole::Kbm | ShareRole::PlayerKbm)
    }

    /// Un droit au moins : l'invité a une chaîne d'entrées, et la barre le dit « Joueur ».
    pub fn plays(self) -> bool {
        self.pads() || self.kbm()
    }

    /// Le rôle quand l'hôte donne (`true`) ou reprend (`false`) les manettes, le clavier et la souris tels quels.
    pub fn with_pads(self, on: bool) -> Self {
        Self::of(on, self.kbm())
    }

    /// Le rôle quand l'hôte donne ou reprend le clavier et la souris, les manettes telles quelles.
    pub fn with_kbm(self, on: bool) -> Self {
        Self::of(self.pads(), on)
    }

    /// « Spectateur », « Joueur » : la barre réduite de l'invité.
    pub fn label(self) -> &'static str {
        if self.plays() { tr("Joueur", "Player") } else { tr("Spectateur", "Spectator") }
    }

    /// « Spectateur », « Joueur », « Clavier et souris », « Joueur, clavier et souris » : la liste de l'hôte.
    pub fn long_label(self) -> &'static str {
        match self {
            ShareRole::Spectator => tr("Spectateur", "Spectator"),
            ShareRole::Player => tr("Joueur", "Player"),
            ShareRole::Kbm => tr("Clavier et souris", "Keyboard and mouse"),
            ShareRole::PlayerKbm => tr("Joueur, clavier et souris", "Player, keyboard and mouse"),
        }
    }

    /// L'avis bref d'un rôle changé par l'hôte (docs/link-ui.md). Les deux droits changés d'un coup : un seul
    /// avis, le droit donné avant le droit repris, les manettes avant le clavier et la souris. Plus aucun droit :
    /// « Vous regardez de nouveau. » à la place de l'avis de reprise. Rien au départ (`previous` nul) ni sans
    /// changement.
    pub fn notice(self, previous: Option<ShareRole>) -> Option<&'static str> {
        let before = previous?;
        if self.pads() && !before.pads() {
            Some(tr("Vous pouvez jouer à la manette.", "You can play with a controller."))
        } else if self.kbm() && !before.kbm() {
            Some(tr("Vous avez le clavier et la souris.", "You have the keyboard and mouse."))
        } else if !self.plays() && before.plays() {
            Some(tr("Vous regardez de nouveau.", "You are watching again."))
        } else if !self.pads() && before.pads() {
            Some(tr("Les manettes sont reprises.", "The controllers were taken back."))
        } else if !self.kbm() && before.kbm() {
            Some(tr("Le clavier et la souris sont repris.", "The keyboard and mouse were taken back."))
        } else {
            None
        }
    }
}

/// La « Qualité de l'image » qu'un invité demande : miroir de `kybernow_core::share::ShareQuality` (l'interface
/// seule, celle des captures et des tests sur Mac, n'a pas le cœur). L'image reçue n'est jamais au-dessus de
/// celle de l'hôte ; la Station décide et le dit (`ShareGuest::quality`, `GuestUi::image`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShareQuality {
    /// D'après l'écran de l'appareil et ce que son décodeur suit : proposé.
    #[default]
    Auto,
    /// « Comme l'hôte ».
    Host,
    /// « 4K · 60 Hz ».
    P2160_60,
    /// « 1080p · 60 Hz ».
    P1080_60,
    /// « 720p · 30 Hz ».
    P720_30,
}

impl ShareQuality {
    /// Dans l'ordre du choix.
    pub const ALL: [ShareQuality; 5] = [Self::Auto, Self::Host, Self::P2160_60, Self::P1080_60, Self::P720_30];

    /// Le nom retenu dans `kybernow.json` : celui de l'échange avec Admin (`auto`, `host`, `2160p60`, `1080p60`, `720p30`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Host => "host",
            Self::P2160_60 => "2160p60",
            Self::P1080_60 => "1080p60",
            Self::P720_30 => "720p30",
        }
    }

    /// Un nom inconnu (fichier plus récent ou écrit à la main) se lit « Auto ».
    pub fn parse(s: &str) -> Self {
        Self::ALL.into_iter().find(|q| q.as_str() == s.trim()).unwrap_or_default()
    }

    /// « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » (docs/link-ui.md, « Qualité de l'image »).
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => tr("Auto", "Auto"),
            Self::Host => tr("Comme l'hôte", "Same as host"),
            Self::P2160_60 => "4K · 60 Hz",
            Self::P1080_60 => "1080p · 60 Hz",
            Self::P720_30 => "720p · 30 Hz",
        }
    }
}

/// Ce que « Partager » peut faire avec un ordinateur (`Info::share`) : miroir de
/// `kybernow_core::share::ShareAvailability`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareAvail {
    /// Une Station antérieure à la 0.8.0.
    Unsupported,
    /// Le partage est coupé sur l'ordinateur (`multi_client` de Kyber).
    Disabled,
    Available,
}

impl ShareAvail {
    /// Le texte de « Partager » inactif ; rien quand le partage est disponible.
    pub fn text(self) -> Option<&'static str> {
        match self {
            ShareAvail::Unsupported => Some(tr("Le partage demande KyberNow Station 0.8.0 ou plus récente.", "Sharing needs KyberNow Station 0.8.0 or later.")),
            ShareAvail::Disabled => Some(tr("Le partage est coupé sur cet ordinateur (Admin › Appareils Link).", "Sharing is turned off on this computer (Admin › Link devices).")),
            ShareAvail::Available => None,
        }
    }
}

/// Un invité du partage de l'hôte (`ShareGuest` du cœur).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ShareGuest {
    pub id: String,
    pub name: String,
    pub role: ShareRole,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    pub app: String,
    /// Son arrivée (secondes depuis 1970), telle que la Station la donne.
    pub since: Option<i64>,
    /// La qualité de l'image qu'il reçoit (« 720p · 30 Hz »), seulement quand c'est une image à part, pas celle
    /// de l'hôte, et que Kyber en sait la taille : sinon `None` (une image encore inconnue compte comme celle de
    /// l'hôte). L'hôte la voit sans la changer.
    pub quality: Option<String>,
}

impl ShareGuest {
    /// « Léa · Spectateur », « Léa · Joueur », « Léa · Joueur, clavier et souris ».
    pub fn line(&self) -> String {
        format!("{} · {}", self.name, self.role.long_label())
    }

    /// « iPhone ou iPad · 720p · 30 Hz · depuis 4 min » : son app, sa qualité d'image si elle n'est pas celle de
    /// l'hôte, et depuis quand il est là (`now` : secondes depuis 1970). `since` est en secondes Unix, d'après
    /// l'horloge de la Station : la durée n'est jamais négative (horloges décalées). Sans arrivée lisible, sans
    /// la durée.
    pub fn meta(&self, now: i64) -> String {
        let mut parts = vec![app_label(&self.app)];
        parts.extend(self.quality.clone());
        if let Some(t) = self.since.filter(|t| *t > 0) {
            parts.push(format!("{} {}", tr("depuis", "for"), duration_label(now.saturating_sub(t).max(0) as u64)));
        }
        parts.join(" · ")
    }
}

/// L'app d'un invité (champ `app`) : Mac, Windows, Linux, « iPhone ou iPad », Apple TV, Android,
/// « Téléviseur LG », « Navigateur » ; une app inconnue (Station plus récente) se dit telle quelle.
pub fn app_label(app: &str) -> String {
    match app {
        "mac" => "Mac".into(),
        "windows" => "Windows".into(),
        "linux" => "Linux".into(),
        "ios" => tr("iPhone ou iPad", "iPhone or iPad").into(),
        "tvos" => "Apple TV".into(),
        "android" => "Android".into(),
        "webos" => tr("Téléviseur LG", "LG TV").into(),
        "web" => tr("Navigateur", "Browser").into(),
        other => other.into(),
    }
}

/// « moins d'une minute », « 3 min », « 1 h 12 » (docs/link-ui.md) : la durée d'un invité.
pub fn duration_label(seconds: u64) -> String {
    let m = seconds / 60;
    if m < 1 {
        tr("moins d'une minute", "under a minute").into()
    } else if m < 60 {
        format!("{m} min")
    } else {
        format!("{} h {:02}", m / 60, m % 60)
    }
}

/// Le partage ouvert d'une session d'hôte (`ShareState` du cœur) : le code à six chiffres, le lien des
/// apps (`kybernow://join?…`, celui du QR code), l'adresse du client web (« Copier le lien ») et les
/// invités. Quatre au plus.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ShareView {
    pub code: String,
    pub link: String,
    pub web: String,
    pub guests: Vec<ShareGuest>,
}

/// Les invités qu'un partage reçoit au plus.
pub const MAX_GUESTS: usize = 4;

/// « 2 invités », « 1 invité », « 0 invité » ; “2 guests”, “1 guest” : la pastille de la barre.
pub fn guests_label(n: usize) -> String {
    if crate::i18n::english() {
        format!("{n} {}", if n == 1 { "guest" } else { "guests" })
    } else {
        format!("{n} {}", if n >= 2 { "invités" } else { "invité" })
    }
}

/// Le sous-titre de la tuile « Partager » du menu, et si la tuile est allumée (le partage est ouvert) : « 2 invités »
/// dès qu'il y a un invité ; sinon, le partage ouvert, son code (« 123 456 ») ; sinon « Code et QR code ».
pub fn share_tile(share: Option<&ShareView>) -> (String, bool) {
    match share {
        Some(v) if !v.guests.is_empty() => (guests_label(v.guests.len()), true),
        Some(v) => (format_code(&v.code), true),
        None => (tr("Code et QR code", "Code and QR code").to_string(), false),
    }
}

/// « 123 456 » : le code d'un partage (six chiffres, groupés par trois), tel quel s'il n'en a pas six.
pub fn format_code(code: &str) -> String {
    if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) { format!("{} {}", &code[..3], &code[3..]) } else { code.to_string() }
}

/// « Invité sur Atelier », “Guest on Atelier” : le titre d'une session d'invité.
pub fn guest_title(computer: &str) -> String {
    format!("{} {computer}", tr("Invité sur", "Guest on"))
}

/// Un des deux boutons de la ligne d'un invité (hors « Retirer ») : un droit, donné ou repris sans toucher à
/// l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareMove {
    /// « Donner » ou « Reprendre les manettes » : `SharePads(i)`.
    Pads,
    /// « Donner » ou « Reprendre le clavier et la souris » : `ShareKbm(i)`.
    Kbm,
}

impl ShareMove {
    /// Les deux boutons, dans l'ordre de la ligne : les manettes, puis le clavier et la souris.
    pub const ALL: [ShareMove; 2] = [ShareMove::Pads, ShareMove::Kbm];

    /// Ce que dit le bouton pour un invité de ce rôle : donner le droit qu'il n'a pas, reprendre celui qu'il a.
    pub fn label(self, role: ShareRole) -> &'static str {
        match (self, self.held(role)) {
            (ShareMove::Pads, false) => tr("Donner les manettes", "Give controllers"),
            (ShareMove::Pads, true) => tr("Reprendre les manettes", "Take back controllers"),
            (ShareMove::Kbm, false) => tr("Donner le clavier et la souris", "Give keyboard and mouse"),
            (ShareMove::Kbm, true) => tr("Reprendre le clavier et la souris", "Take back keyboard and mouse"),
        }
    }

    /// L'invité a ce droit.
    pub fn held(self, role: ShareRole) -> bool {
        match self {
            ShareMove::Pads => role.pads(),
            ShareMove::Kbm => role.kbm(),
        }
    }

    /// Le rôle que demande le bouton : ce droit basculé, l'autre tel quel.
    pub fn toggled(self, role: ShareRole) -> ShareRole {
        match self {
            ShareMove::Pads => role.with_pads(!role.pads()),
            ShareMove::Kbm => role.with_kbm(!role.kbm()),
        }
    }

    /// La cible du bouton pour l'invité `i`.
    pub fn target(self, i: usize) -> Target {
        match self {
            ShareMove::Pads => Target::SharePads(i),
            ShareMove::Kbm => Target::ShareKbm(i),
        }
    }
}

/// Ce que fait l'hôte dans le panneau de son partage.
#[derive(Clone, Debug, PartialEq)]
pub enum ShareOp {
    /// « Partager » : ouvre le partage de l'écran principal.
    Open,
    /// « Nouveau code » : les invités déjà là restent.
    NewCode,
    /// Un droit donné ou repris (« Donner les manettes », « Reprendre le clavier et la souris »…) : le rôle voulu.
    Role(String, ShareRole),
    /// « Retirer ».
    Kick(String),
    /// « Arrêter le partage » : tout le monde sort.
    Close,
}

/// Ce que l'invité tient de sa session (une session d'invité n'a pas d'ordinateur dans la liste de l'appareil).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestUi {
    /// Le nom de l'ordinateur qui partage.
    pub computer: String,
    pub role: ShareRole,
    /// La « Qualité de l'image » choisie (celle de « Rejoindre », puis celle de la barre).
    pub quality: ShareQuality,
    /// La qualité reçue (« 1080p · 60 Hz »), telle que `Platform::on_share_image` la donne ; `None` tant que
    /// Kyber n'en sait pas la taille : la barre ne la montre alors pas.
    pub image: Option<String>,
}

impl GuestUi {
    /// Ce que montrent la puce de la barre et la tuile du menu : la qualité reçue quand on la connaît, sinon le
    /// choix (« Auto »).
    pub fn shown(&self) -> &str {
        self.image.as_deref().unwrap_or(self.quality.label())
    }
}

/// Le champ que la saisie de « Rejoindre un partage » remplit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinField {
    /// Le code à six chiffres.
    Code,
    /// « Votre prénom ».
    Name,
}

/// L'écran « Rejoindre un partage » : le code, le prénom (proposé : le nom de l'appareil, retenu), la
/// « Qualité de l'image » (« Auto » proposé, retenue aussi), la recherche en cours et ce que le cœur a répondu.
#[derive(Clone, Debug)]
pub struct JoinState {
    /// Les chiffres tapés, six au plus.
    pub code: String,
    pub name: String,
    /// « Qualité de l'image » : sous le prénom, cinq choix (`Target::JoinQuality`).
    pub quality: ShareQuality,
    pub field: JoinField,
    /// Majuscule du pavé des lettres (manette), pour la touche suivante.
    pub shift: bool,
    /// « Recherche du partage… » : le code est parti aux ordinateurs du réseau.
    pub busy: bool,
    /// Le refus du cœur (« Aucun partage avec ce code sur le réseau. »…).
    pub message: Option<String>,
    /// Numéro de la recherche : une réponse d'une recherche abandonnée n'ouvre rien.
    pub attempt: u32,
    pub started: Instant,
}

impl JoinState {
    pub fn new() -> Self {
        Self { code: String::new(), name: String::new(), quality: ShareQuality::Auto, field: JoinField::Code, shift: false, busy: false, message: None, attempt: 0, started: Instant::now() }
    }

    /// Six chiffres et un prénom : « Rejoindre » peut partir.
    pub fn ready(&self) -> bool {
        self.code.len() == 6 && !self.name.trim().is_empty()
    }
}

impl Default for JoinState {
    fn default() -> Self {
        Self::new()
    }
}

/// Les prénoms ne dépassent pas cette longueur.
pub const NAME_MAX: usize = 24;

/// Identifiant d'une session de l'app : il croît à chaque session ouverte et ne se réutilise
/// pas (un événement qui arrive après la fin d'une session ne vise jamais la suivante).
pub type SessionId = u32;

/// Ordinateurs en session en même temps, au plus (docs/plusieurs-ordinateurs.md).
pub const MAX_SESSIONS: usize = 4;

/// L'interface d'une session. L'app en tient une liste (`App::sessions`, dans l'ordre
/// d'ouverture) et une session active (`App::session()`), celle de la fenêtre au premier plan.
#[derive(Clone, Debug)]
pub struct SessionUi {
    pub id: SessionId,
    pub host: String,
    pub name: String,
    pub profile: Profile,
    pub phase: Phase,
    /// Menu de session (manette, tuiles).
    pub menu: bool,
    pub keys: bool,
    pub stats: Option<Stats>,
    pub show_stats: bool,
    pub pad_mouse: bool,
    pub muted: bool,
    pub light: bool,
    /// Souris des jeux (relative) en ce moment.
    pub relative: bool,
    /// Barre de session (bureau) montrée, et son menu ouvert.
    pub bar: bool,
    pub bar_menu: Option<BarMenu>,
    /// Capsule passagère (manette branchée, mode changé…).
    pub notice: Option<(String, Instant)>,
    pub started: Instant,
    /// Déconnexion demandée : l'accueil revient en fondu sous « Déconnexion… ».
    pub ending_since: Option<Instant>,
    /// La première image : l'image seule à partir d'elle ; sans elle, une
    /// fin sur erreur est un échec de la connexion.
    pub opened_at: Option<Instant>,
    /// Écrans pris par la session (multi-écrans), tels que le cœur les a
    /// annoncés : 1 tant qu'il ne l'a pas fait. Le même compte partout : les
    /// statistiques de la barre, la dernière session (docs/link-ui.md,
    /// « Statistiques »), comme `screenCount` sur Mac ; les écrans que
    /// l'ordinateur n'a pas pu créer n'y sont pas.
    pub screens: usize,
    /// Écrans attendus : ceux du plan du cœur au départ, au changement de
    /// profil ou d'écrans de l'appareil, puis ceux que le cœur annonce (pris et
    /// non créés : une fenêtre chacun). « Préparation des écrans » en dépend.
    pub expected_screens: usize,
    /// « Garder le son » (bouton du haut-parleur de la barre) : la session reste audible
    /// même quand elle n'est pas active. Éteint par défaut, pour la session en cours.
    pub keep_sound: bool,
    /// La fenêtre de la session est en plein écran (« Plein écran » de la barre).
    pub fullscreen: bool,
    /// KyberNow Share, session d'hôte : le partage ouvert (`None` : pas de partage), le panneau montré
    /// (à la manette ; au bureau, c'est le menu `BarMenu::Share` de la barre), une demande à Admin en cours,
    /// l'adresse du client web copiée (réussie ou non : le panneau le dit un instant), et ce qu'Admin a refusé.
    pub share: Option<ShareView>,
    pub share_panel: bool,
    pub share_busy: bool,
    pub share_copied: Option<(Instant, bool)>,
    pub share_error: Option<String>,
    /// « Partager » choisi avant la première réponse d'Admin (`Info::share` pas encore connue) : le panneau
    /// attend cette réponse, puis ouvre le partage ou dit pourquoi il ne peut pas (`App::share_known`).
    pub share_wait: bool,
    /// Quand le partage s'est ouvert, et quand l'hôte l'a arrêté : un signe de vie parti avant ne le défait pas
    /// (`None` : un partage fermé) ni ne le rouvre (comme l'app Mac).
    pub share_opened: Option<Instant>,
    pub share_closed: Option<Instant>,
    /// KyberNow Share, session d'invité : l'ordinateur qui partage et le rôle donné par l'hôte.
    pub guest: Option<GuestUi>,
}

impl SessionUi {
    /// Une session qui démarre : la connexion commence.
    pub fn new(id: SessionId, host: &str, name: &str, profile: Profile, light: bool, relative: bool, expected_screens: usize) -> Self {
        Self {
            id,
            host: host.into(),
            name: name.into(),
            profile,
            phase: Phase::Preparing,
            menu: false,
            keys: false,
            stats: None,
            show_stats: false,
            pad_mouse: false,
            muted: false,
            light,
            relative,
            bar: false,
            bar_menu: None,
            notice: None,
            started: Instant::now(),
            ending_since: None,
            opened_at: None,
            screens: 1,
            expected_screens,
            keep_sound: false,
            fullscreen: false,
            share: None,
            share_panel: false,
            share_busy: false,
            share_copied: None,
            share_error: None,
            share_wait: false,
            share_opened: None,
            share_closed: None,
            guest: None,
        }
    }

    /// Une session d'invité d'un partage (« Invité sur Atelier ») : arrivé en spectateur, jamais dans la
    /// liste des ordinateurs (`key` : une clé propre à la session, que le magasin ne connaît pas).
    pub fn guest(id: SessionId, key: &str, computer: &str, role: ShareRole) -> Self {
        let mut s = Self::new(id, key, &guest_title(computer), Profile::Jeu, false, false, 1);
        s.guest = Some(GuestUi { computer: computer.into(), role, quality: ShareQuality::Auto, image: None });
        s
    }

    /// Le rôle de l'invité ; `None` pour une session d'hôte.
    pub fn role(&self) -> Option<ShareRole> {
        self.guest.as_ref().map(|g| g.role)
    }

    /// Clavier et souris vont à l'ordinateur : une session d'hôte, ou un joueur à qui ils sont donnés.
    pub fn keyboard_mouse(&self) -> bool {
        self.guest.as_ref().is_none_or(|g| g.role.kbm())
    }

    /// Un panneau du partage est montré par-dessus l'image.
    pub fn share_overlay(&self) -> bool {
        self.share_panel || self.bar_menu == Some(BarMenu::Share)
    }

    /// La session est en cours : pas finie. Une session finie sur une erreur reste dans la liste avec son
    /// message, jusqu'à « Fermer », mais le cœur l'a désinscrite : elle ne compte plus (limite de quatre,
    /// « Garder le son », « Afficher » du menu « Ordinateurs »).
    pub fn running(&self) -> bool {
        !matches!(self.phase, Phase::Ended { .. })
    }

    /// La session est à plusieurs écrans (une fenêtre plein écran chacun) : pas de « Plein
    /// écran ». Comme `multiScreen` sur Mac.
    pub fn multi(&self) -> bool {
        self.screens >= 2 || self.expected_screens >= 2
    }
}

/// Étapes de l'ajout d'un ordinateur.
#[derive(Clone, Debug, PartialEq)]
pub enum AddStep {
    /// Le code à six chiffres, présenté à chaque Station trouvée.
    Code,
    /// Adresse et port d'Admin, au clavier ou au pavé.
    Address,
    /// Empreinte du certificat à comparer avec celle d'Admin.
    Verify { address: String, port: u16, fingerprint: String },
    /// L'adresse MAC de l'ordinateur choisi, pour le réveil par le réseau
    /// (Réglages de l'ordinateur › Réseau) : même champ et même pavé, en
    /// hexadécimal.
    Mac,
}

#[derive(Clone, Debug)]
pub struct AddState {
    pub step: AddStep,
    pub code: String,
    /// Stations à qui le code a été présenté : (nom, adresse).
    pub asked: Vec<(String, String)>,
    pub address: String,
    /// L'adresse MAC en cours de saisie (`AddStep::Mac`).
    pub mac: String,
    pub message: Option<String>,
    pub busy: bool,
    pub started: Instant,
}

/// Une ligne de réglage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Profile,
    Resolution,
    /// Écrans pris par la session : Auto, Cet écran, Tous les écrans (bureau seulement).
    Displays,
    Fps,
    Codec,
    Yuv444,
    Display,
    Bitrate,
    VirtualDisplay,
    BatteryLight,
    Audio,
    Xbox360,
    Combo,
    Pads,
    Mouse,
    Clipboard,
    Reliable,
    Reconnect,
    /// L'adresse MAC de l'ordinateur (réveil par le réseau).
    WakeMac,
    Remove,
    /// Station : ouvrir Admin dans le navigateur, redémarrer, mettre à jour.
    StationAdmin,
    StationRestart,
    StationUpdate,
    /// Une ligne d'information (Statistiques, Compte) : rien à choisir.
    Info,
    Language,
    Interface,
    UpdateChannel,
    CheckUpdate,
    KeepImage,
    /// KyberNow Link OS (réglages de l'appareil).
    OsUsage,
    OsWifi,
    OsBtPair,
    OsHdmi,
    OsCec,
    OsAutoUpdate,
    OsSsh,
    OsReboot,
    OsPoweroff,
    OsUpdateApply,
    OsUpdateLater,
    OsRollback,
    OsStandby,
    OsInfo,
}

/// Une rubrique des réglages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Profile,
    Picture,
    Sound,
    /// Manettes, souris, clavier, presse-papiers.
    Controls,
    Network,
    /// La dernière session et ce que la Station dit de l'ordinateur.
    Statistics,
    /// Admin, redémarrer, mettre à jour, compte, retirer.
    Station,
    /// Réglages de l'app.
    App,
    Gamepads,
    /// KyberNow Link OS : réglages de l'appareil.
    OsUsage,
    OsNetwork,
    OsControllers,
    OsScreen,
    OsSound,
    OsUpdate,
    OsLanguage,
    OsAdvanced,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Profile => tr("Profil", "Profile"),
            Section::Picture => tr("Image", "Picture"),
            Section::Sound => tr("Son", "Sound"),
            Section::Controls => tr("Commandes", "Controls"),
            Section::Network => tr("Réseau", "Network"),
            Section::Statistics => tr("Statistiques", "Statistics"),
            Section::Station => "Station",
            Section::App => tr("KyberNow Link", "KyberNow Link"),
            Section::Gamepads => tr("Manettes", "Controllers"),
            Section::OsUsage => tr("Usage", "Use"),
            Section::OsNetwork => tr("Réseau", "Network"),
            Section::OsControllers => tr("Manettes", "Controllers"),
            Section::OsScreen => tr("Écran", "Screen"),
            Section::OsSound => tr("Son", "Sound"),
            Section::OsUpdate => tr("Mise à jour", "Update"),
            Section::OsLanguage => tr("Langue", "Language"),
            Section::OsAdvanced => tr("Avancé", "Advanced"),
        }
    }
}

/// Les rubriques des réglages de l'appareil (KyberNow Link OS), dans l'ordre de la maquette.
pub const OS_SECTIONS: [Section; 8] = [Section::OsUsage, Section::OsNetwork, Section::OsControllers, Section::OsScreen, Section::OsSound, Section::OsUpdate, Section::OsLanguage, Section::OsAdvanced];

/// Ce que l'app demande à KyberNow Link OS.
#[derive(Clone, Debug, PartialEq)]
pub enum OsAction {
    /// Bluetooth à l'écoute : les manettes qui se présentent sont acceptées.
    BtListen(bool),
    WifiScan,
    WifiConnect { ssid: String, password: String },
    /// Le premier démarrage est fini (usage choisi, interface appliquée).
    SetupDone,
    Status,
    /// Télécharger l'image publiée, puis l'écrire dans l'autre emplacement.
    UpdateDownload,
    UpdateApply,
    Rollback,
    Ssh(bool),
    Reboot,
    Poweroff,
    /// Veille (vrai) ou réveil : le téléviseur suit par HDMI-CEC si le réglage le permet.
    Standby(bool),
    /// HDMI-CEC allumé (le nom annoncé au téléviseur) ou coupé (l'adresse CEC rendue).
    Cec(bool),
    /// Mode HDMI pour le prochain démarrage.
    Hdmi(String),
}

/// Débits proposés (Mbit/s, 0 : celui du profil) : jusqu'à 500, quand
/// l'ordinateur, le réseau et le décodeur le tiennent.
pub const BITRATES: [u32; 14] = [0, 10, 20, 30, 40, 50, 60, 80, 100, 150, 200, 300, 400, 500];

/// Les rubriques des réglages d'un ordinateur (docs/link-ui.md) : le profil en
/// tête, puis les six rubriques communes à toutes les apps Link.
pub const HOST_SECTIONS: [Section; 7] = [Section::Profile, Section::Picture, Section::Sound, Section::Controls, Section::Network, Section::Statistics, Section::Station];
pub const APP_SECTIONS: [Section; 2] = [Section::App, Section::Gamepads];

/// La forme d'une ligne de réglage.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// Valeurs au choix (segments sur le bureau, ‹ valeur › à la manette) ;
    /// `tags` : une étiquette par valeur, s'il y en a (« décodeur du Pi », « processeur »).
    Choice { options: Vec<String>, current: usize, disabled: Vec<bool>, tags: Vec<Option<Tag>> },
    Toggle(bool),
    /// Ouvre un écran ou lance quelque chose (la valeur à droite, « › »).
    Action(String),
    /// Un bouton qui dit ce qu'il fait (« Redémarrer la Station ») : sur le
    /// bureau, le bouton puis l'aide ; à la manette, son texte en libellé.
    Button { text: String, style: ButtonStyle },
    /// Information seule.
    Info(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Ghost,
    Primary,
    /// Retirer l'ordinateur.
    Danger,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub setting: Setting,
    pub label: String,
    pub hint: String,
    pub kind: RowKind,
    pub enabled: bool,
    /// Étiquette au bout de l'aide (« processeur » sur la ligne 4:4:4).
    pub tag: Option<Tag>,
}

/// Une étiquette sur une valeur : qui fait le travail (la carte ou le
/// décodeur du Pi, le processeur) ou une précision (« 1080p »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub text: String,
    pub kind: TagKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    /// Décodeur matériel (Spectre).
    Hw,
    /// Processeur (ambre).
    Cpu,
    /// Précision neutre.
    Note,
}

impl Tag {
    pub fn hw(text: &str) -> Option<Tag> {
        Some(Tag { text: text.into(), kind: TagKind::Hw })
    }
    pub fn cpu() -> Option<Tag> {
        Some(Tag { text: tr("processeur", "processor").into(), kind: TagKind::Cpu })
    }
    pub fn note(text: &str) -> Option<Tag> {
        Some(Tag { text: text.into(), kind: TagKind::Note })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Lancement animé (interfaces console, et KyberNow Link OS).
    Launch,
    Home,
    Add,
    /// KyberNow Share : « Rejoindre un partage » (code et prénom).
    Join,
    Settings,
    Gamepads,
    Session,
    /// KyberNow Link OS : premier démarrage (manette, langue, réseau, usage, ordinateur).
    Setup,
    /// KyberNow Link OS : réglages de l'appareil.
    Device,
    /// KyberNow Link OS : veille (écran éteint ; une touche réveille).
    Standby,
}

/// KyberNow Link OS : les étapes du premier démarrage, dans l'ordre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupStep {
    Controller,
    Language,
    Network,
    Usage,
    Computer,
}

impl SetupStep {
    pub const ALL: [SetupStep; 5] = [SetupStep::Controller, SetupStep::Language, SetupStep::Network, SetupStep::Usage, SetupStep::Computer];
}

/// Une manette vue par Bluetooth (association, connexion).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

/// Un réseau Wi-Fi vu par NetworkManager.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WifiNet {
    pub ssid: String,
    /// 0 à 100.
    pub signal: u8,
    pub secured: bool,
    pub active: bool,
}

/// Le premier démarrage (et les écrans Manette et Réseau rouverts depuis les
/// réglages de l'appareil : `active` faux).
#[derive(Clone, Debug)]
pub struct SetupState {
    /// Le premier démarrage complet (sinon une étape seule, depuis les réglages).
    pub active: bool,
    pub step: SetupStep,
    pub bt: Vec<BtDevice>,
    pub bt_listening: bool,
    pub wifi: Vec<WifiNet>,
    pub wifi_scanning: bool,
    /// Réseau choisi (mot de passe en cours de saisie).
    pub wifi_pick: Option<usize>,
    pub password: String,
    pub shift: bool,
    pub net_message: Option<String>,
    pub net_busy: bool,
    /// 0 : English, 1 : Français.
    pub lang: usize,
    /// 0 : salon, 1 : bureau.
    pub usage: usize,
}

impl Default for SetupState {
    fn default() -> Self {
        Self { active: false, step: SetupStep::Controller, bt: Vec::new(), bt_listening: false, wifi: Vec::new(), wifi_scanning: false, wifi_pick: None, password: String::new(), shift: false, net_message: None, net_busy: false, lang: 1, usage: 0 }
    }
}

/// Ce que l'app sait de KyberNow Link OS (`kybernow-os-update status`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OsInfo {
    pub version: String,
    /// « A » ou « B », et l'autre.
    pub slot: String,
    pub other: String,
    pub other_version: String,
    /// Le tryboot A/B existe (Pi 4 et 5).
    pub ab: bool,
    /// Un essai est en cours (version qui attend d'être inscrite).
    pub pending: String,
    pub tryboot: bool,
    pub ssh: bool,
    pub built: String,
}

/// La mise à jour de l'OS : téléchargement de l'image, puis écriture.
#[derive(Clone, Debug, Default)]
pub struct OsUpdate {
    /// (reçu, total) pendant le téléchargement.
    pub downloading: Option<(u64, u64)>,
    /// L'image téléchargée et vérifiée, prête à écrire.
    pub ready: Option<std::path::PathBuf>,
    pub applying: bool,
    pub error: Option<String>,
}

/// Ce qu'on peut choisir.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Connect,
    Chip(Profile),
    /// « Personnalisé » sur l'accueil : ouvre les réglages de l'ordinateur.
    CustomChip,
    Host(usize),
    Found(usize),
    AddCard,
    /// « Rejoindre avec un code » sur l'accueil.
    JoinCard,
    Wake,
    /// Hors ligne, sans adresse MAC : sonder l'ordinateur à nouveau.
    Retry,
    HostSettings,
    /// Les deux boutons d'une confirmation : faire, annuler.
    ConfirmYes,
    ConfirmNo,
    /// Réglages de l'app.
    AppSettings,
    AddFound(usize),
    AddByAddress,
    Key(char),
    KeyErase,
    KeyOk,
    VerifyOk,
    VerifyNo,
    Section(usize),
    Setting(Setting),
    /// Une valeur précise d'un réglage (segments du bureau).
    Choice(Setting, usize),
    Pad(usize),
    PadTest(u8),
    Menu(MenuItem),
    Shortcut(usize),
    CancelConnect,
    Back,
    /// Fenêtre (bureau) : réduire, agrandir, fermer.
    WinMinimize,
    WinMaximize,
    WinClose,
    /// Barre de session (bureau).
    BarProfile,
    BarPick(Profile),
    BarKeys,
    BarShortcut(usize),
    BarMouse,
    BarFullscreen,
    BarQuit,
    /// Plusieurs ordinateurs : le menu « Ordinateurs » de la barre, l'un des ordinateurs
    /// (indice dans `Store::hosts`), et le bouton du haut-parleur (« Garder le son »).
    BarComputers,
    BarComputer(usize),
    BarSound,
    /// KyberNow Share, invité : les champs de « Rejoindre un partage » et le bouton « Rejoindre ».
    JoinCode,
    JoinName,
    JoinGo,
    /// KyberNow Share, invité : « Qualité de l'image », à « Rejoindre » (les cinq choix sous le prénom).
    JoinQuality(ShareQuality),
    /// La puce de la barre d'un invité (la qualité reçue) et son menu ; un choix de ce menu, ou du panneau à la
    /// manette.
    BarQuality,
    QualityPick(ShareQuality),
    /// KyberNow Share, hôte : « Partager » dans la barre ; le panneau (copier le lien, les deux droits d'un
    /// invité : manettes, clavier et souris, retirer ; nouveau code, arrêter, fermer).
    BarShare,
    ShareCopy,
    SharePads(usize),
    ShareKbm(usize),
    ShareKick(usize),
    ShareNewCode,
    ShareStop,
    ShareClose,
    /// Une mise à jour est publiée : sa page.
    UpdateNotice,
    /// KyberNow Link OS : premier démarrage.
    SetupContinue,
    SetupSkip,
    SetupLang(usize),
    SetupUsage(usize),
    SetupWifi(usize),
    SetupRescan,
    KeyShift,
    /// KyberNow Link OS : rubrique des réglages de l'appareil.
    DeviceSection(usize),
}

/// Ce que l'app demande au pilote. Une action de session porte l'identifiant de sa session :
/// elle vise celle-là même si la session active a changé entre-temps (une session qui se
/// termine au même instant ne fait pas déconnecter la suivante).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Connect { host: String, profile: Profile },
    /// Plusieurs ordinateurs : la fenêtre de cette session passe devant et devient active
    /// (« Afficher », « Se connecter » sur un ordinateur déjà en session).
    ShowSession(SessionId),
    /// « Ordinateur suivant » : la fenêtre de la session suivante, dans l'ordre d'ouverture.
    NextSession,
    Save,
    /// Présenter le code aux Stations trouvées (et à `address`, si donnée).
    StartPairing,
    StopPairing,
    PairAddress { address: String, port: u16, fingerprint: String },
    Probe { address: String, port: u16 },
    Wake(String),
    /// Sonder les ordinateurs maintenant (Réessayer).
    Poll,
    /// Redémarrer ou mettre à jour la Station de cet ordinateur ; `force` :
    /// même si un appareil est en session.
    Station { host: String, kind: StationKind, force: bool },
    /// KyberNow Share, invité : le code part aux ordinateurs du réseau (`share::join_checked`), avec la
    /// « Qualité de l'image » choisie (pour « Auto », l'écran de la fenêtre et ce que le décodeur suit).
    JoinShare { code: String, name: String, quality: ShareQuality },
    /// KyberNow Share, invité : la « Qualité de l'image » changée en session (`Session::set_share_quality`,
    /// sans rien relancer de la session).
    ShareQuality(SessionId, ShareQuality),
    /// KyberNow Share, hôte : une demande à Admin pour le partage de cette session.
    Share(SessionId, ShareOp),
    /// Copier ce texte (le lien du partage) dans le presse-papiers du système.
    CopyText(String),
    /// La saisie de texte de SDL, sans le clavier de Steam (« Rejoindre un partage »).
    TextInput,
    Rumble { pad: usize, low: u16, high: u16, triggers: bool },
    Shortcut(SessionId, Vec<u16>),
    SetProfile(SessionId, Profile),
    PadMouse(SessionId, bool),
    /// Le son coupé depuis le menu de session.
    Mute(SessionId, bool),
    /// « Garder le son » : la session reste audible quand elle n'est pas active.
    KeepSound(SessionId, bool),
    Light(SessionId, bool),
    /// Souris des jeux ou du bureau (barre, Ctrl+Alt+Maj+M).
    ToggleMouse(SessionId),
    /// Plein écran de la fenêtre d'une session ; `None` : la fenêtre de l'app, hors session.
    ToggleFullscreen(Option<SessionId>),
    /// Clavier de Steam (`steam://open/keyboard`), ou saisie de texte de SDL ; la session
    /// dont c'est le clavier, `None` pour l'ajout d'un ordinateur.
    OpenKeyboard(Option<SessionId>),
    ReleaseAll(SessionId),
    Disconnect(SessionId),
    CancelConnect(SessionId),
    Language,
    Interface,
    CheckUpdate,
    OpenUrl(String),
    Minimize,
    Maximize,
    Quit,
    /// KyberNow Link OS.
    Os(OsAction),
}

/// Où en est la recherche de mise à jour.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateState {
    #[default]
    Unknown,
    Checking,
    UpToDate,
    /// Plus récente, signée : version, canal (`stable`, `beta`), page de la
    /// version ; le fichier publié pour cette installation (adresse,
    /// empreinte, taille), que l'OS télécharge.
    Available { version: String, channel: String, page: String, file: String, url: String, sha256: String, size: u64 },
    Failed(String),
}

pub struct App {
    pub store: Store,
    pub layout: Layout,
    pub screen: Screen,
    pub focus: Target,
    /// Les éléments choisissables du dernier dessin.
    pub hits: Vec<(Target, Rect)>,
    /// Pointeur (souris, pavé tactile), en points de l'interface.
    pub pointer: Option<(f32, f32)>,
    /// Le focus se voit : navigation à la manette ou au clavier depuis le
    /// dernier mouvement de souris (bureau).
    pub keyboard_focus: bool,
    pub live: HashMap<String, HostLive>,
    /// Image de chaque ordinateur (dernière session, sinon aperçu d'Admin).
    pub images: HashMap<String, Pixmap>,
    /// D'où vient l'image : dernière session (vrai) ou aperçu d'Admin.
    pub image_is_last: HashMap<String, bool>,
    pub found: Vec<Found>,
    pub machine: Machine,
    pub net: NetInfo,
    pub pads: Vec<PadInfo>,
    pub add: AddState,
    pub section: usize,
    /// Écran des réglages : de l'app (vrai) ou de l'ordinateur choisi.
    pub settings_global: bool,
    /// Défilement des réglages (bureau), en points.
    pub scroll: f32,
    pub pad_selected: usize,
    /// Les sessions en cours, dans l'ordre d'ouverture (une seule sur le Steam Deck, au salon
    /// et sur KyberNow Link OS : `multi_sessions`), et celle de la fenêtre au premier plan.
    /// `App::session()` rend la session active : la barre, le menu et les raccourcis la visent.
    /// Qui est active, c'est le registre du cœur (`kybernow_core::focus`) qui le dit : la couche
    /// appareil recopie ici sa réponse (`device::focus::sync`), l'interface ne la décide pas.
    pub sessions: Vec<SessionUi>,
    pub active: Option<SessionId>,
    /// Les sessions déjà actives, la plus récente en dernier : qui reprend la main quand la
    /// session active se termine, tant que le cœur n'a pas répondu (comme lui, `focus::unregister`).
    history: Vec<SessionId>,
    next_session: SessionId,
    pub toast: Option<(String, Instant)>,
    pub version: &'static str,
    pub update: UpdateState,
    /// L'écran d'où l'on vient (Réglages et Manettes ouverts depuis la session).
    pub back_to: Screen,
    /// Début du lancement animé, de l'arrivée d'un écran.
    pub launched: Instant,
    pub screen_since: Instant,
    /// Fenêtre plein écran, agrandie (bureau).
    pub fullscreen: bool,
    pub maximized: bool,
    /// Moins d'animations (`KYBERNOW_REDUCE_MOTION=1`).
    pub reduce_motion: bool,
    /// Le nom de cet appareil pour Admin (« Steam Deck », nom de la machine).
    pub device_name: String,
    /// KyberNow Link OS : ce que l'app sait de l'OS (`None` ailleurs).
    pub os: Option<OsInfo>,
    pub setup: SetupState,
    /// Rubrique des réglages de l'appareil.
    pub device_section: usize,
    pub os_update: OsUpdate,
    /// Au lancement sur l'OS : ce qui se prépare encore (libellé, avancement 0 à 1) ;
    /// la scène du lancement tient tant que c'est là.
    pub os_loading: Option<(String, f32)>,
    /// Veille : écran éteint.
    pub standby: bool,
    /// Une confirmation ouverte par-dessus les réglages.
    pub confirm: Option<Confirm>,
    /// Le dernier redémarrage ou la dernière mise à jour de la Station demandés.
    pub station_op: Option<StationOp>,
    /// Le réveil envoyé à un ordinateur, et quand : l'accueil dit « Réveil de
    /// l'ordinateur… » et l'app le sonde toutes les 2 s, 2 min au plus.
    pub waking: Option<(String, Instant)>,
    /// Multi-écrans : ce que la session prendra pour l'ordinateur choisi
    /// (rempli par la couche appareil à chaque tour de boucle).
    pub screens_plan: ScreensPlan,
    /// KyberNow Share : l'écran « Rejoindre un partage ».
    pub join: JoinState,
}

impl App {
    /// L'app prend tous les écrans de l'appareil : interface de bureau, hors
    /// KyberNow Link OS (le Steam Deck, le salon et l'OS restent sur un écran).
    pub fn multi_screens(&self) -> bool {
        self.layout == Layout::Desktop && !self.is_os()
    }

    /// L'app tient plusieurs ordinateurs à la fois, une fenêtre chacun : interface de bureau,
    /// hors KyberNow Link OS (le Steam Deck, le salon et l'OS gardent une session), la même
    /// garde que `multi_screens` (docs/plusieurs-ordinateurs.md).
    pub fn multi_sessions(&self) -> bool {
        self.multi_screens()
    }

    // MARK: - Les sessions

    /// La session active : celle de la fenêtre au premier plan.
    pub fn session(&self) -> Option<&SessionUi> {
        self.session_by(self.active?)
    }

    pub fn session_mut(&mut self) -> Option<&mut SessionUi> {
        Self::active_in(&mut self.sessions, self.active)
    }

    /// La session active, en n'empruntant que la liste : le reste de l'app (`focus`, `screen`)
    /// reste libre pendant qu'on la modifie.
    fn active_in(sessions: &mut [SessionUi], active: Option<SessionId>) -> Option<&mut SessionUi> {
        let id = active?;
        sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn session_by(&self, id: SessionId) -> Option<&SessionUi> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn session_by_mut(&mut self, id: SessionId) -> Option<&mut SessionUi> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// La session de cet ordinateur, s'il est déjà en session (jamais deux fois le même) : une session
    /// finie sur une erreur n'y est plus, on peut s'y reconnecter.
    pub fn session_of_host(&self, host: &str) -> Option<&SessionUi> {
        self.sessions.iter().find(|s| s.host == host && s.running())
    }

    /// Les sessions en cours (voir `SessionUi::running`) : celles que compte le registre du cœur.
    pub fn running_sessions(&self) -> usize {
        self.sessions.iter().filter(|s| s.running()).count()
    }

    /// L'identifiant de la session active.
    pub fn active_id(&self) -> Option<SessionId> {
        self.session().map(|s| s.id)
    }

    /// Quatre ordinateurs sont déjà en session : « Se connecter » est inactif. Les sessions finies sur une
    /// erreur, dont le message attend « Fermer », ne comptent pas.
    pub fn sessions_full(&self) -> bool {
        self.running_sessions() >= MAX_SESSIONS
    }

    /// Un identifiant pour la session qui va s'ouvrir.
    pub fn new_session_id(&mut self) -> SessionId {
        let id = self.next_session;
        self.next_session += 1;
        id
    }

    /// Une session de plus : elle est ajoutée à la fin (ordre d'ouverture) et devient active.
    pub fn open_session(&mut self, ui: SessionUi) {
        let id = ui.id;
        self.sessions.push(ui);
        self.activate_session(id);
    }

    /// Les captures d'écran et les essais : cette session seule, et active (ou aucune).
    pub fn set_session(&mut self, ui: Option<SessionUi>) {
        self.sessions.clear();
        self.history.clear();
        self.active = None;
        if let Some(ui) = ui {
            self.open_session(ui);
        }
    }

    /// La session `id` devient la session active (sa fenêtre est au premier plan). Rend vrai
    /// si la session active a changé. La barre de celle qu'on quitte se referme ; un écran
    /// ouvert par-dessus la session (les manettes) revient à la session.
    pub fn activate_session(&mut self, id: SessionId) -> bool {
        if self.session_by(id).is_none() || self.active == Some(id) {
            return false;
        }
        if let Some(prev) = self.session_mut() {
            prev.bar = false;
            prev.bar_menu = None;
        }
        self.active = Some(id);
        self.history.retain(|h| *h != id);
        self.history.push(id);
        if self.screen != Screen::Session {
            self.screen = Screen::Session;
            self.back_to = Screen::Home;
        }
        self.focus = Target::Menu(MenuItem::Resume);
        true
    }

    /// La session `id` est finie et sa fenêtre fermée : elle sort de la liste. Si c'était
    /// l'active, la dernière qui l'a été reprend la main (sinon la première). Rend vrai si la
    /// session active a changé.
    pub fn close_session(&mut self, id: SessionId) -> bool {
        self.sessions.retain(|s| s.id != id);
        self.history.retain(|h| *h != id);
        if self.active.is_some_and(|a| self.session_by(a).is_some()) {
            return false;
        }
        self.active = self.history.last().copied().or_else(|| self.sessions.first().map(|s| s.id));
        if self.active.is_some() && self.screen != Screen::Session {
            self.screen = Screen::Session;
            self.back_to = Screen::Home;
        }
        true
    }

    /// L'écran virtuel de la Station de cet ordinateur est utilisable : pilote en
    /// place, déclenchement actif (`virtual_display.available` de `GET
    /// /api/stream/info`) ; inconnu tant qu'elle n'a pas répondu : oui.
    pub fn virtual_display_available(&self, h: &Host) -> bool {
        self.live.get(&h.id).and_then(|l| l.virtual_display) != Some(false)
    }

    /// L'écran virtuel compte pour cette session : demandé (réglage de l'ordinateur)
    /// **et** utilisable sur la Station, la règle du cœur (`Screens::choice`).
    pub fn virtual_display_usable(&self, h: &Host) -> bool {
        h.settings.virtual_display && self.virtual_display_available(h)
    }

    pub fn new(store: Store, layout: Layout) -> Self {
        Self {
            store,
            layout,
            screen: if layout.console() { Screen::Launch } else { Screen::Home },
            focus: Target::Connect,
            hits: Vec::new(),
            pointer: None,
            keyboard_focus: layout.console(),
            live: HashMap::new(),
            images: HashMap::new(),
            image_is_last: HashMap::new(),
            found: Vec::new(),
            machine: Machine::default(),
            net: NetInfo::default(),
            pads: Vec::new(),
            add: AddState {
                step: AddStep::Code,
                code: String::new(),
                asked: Vec::new(),
                address: String::new(),
                mac: String::new(),
                message: None,
                busy: false,
                started: Instant::now(),
            },
            section: 0,
            settings_global: false,
            scroll: 0.0,
            pad_selected: 0,
            sessions: Vec::new(),
            active: None,
            history: Vec::new(),
            next_session: 1,
            toast: None,
            version: env!("CARGO_PKG_VERSION"),
            update: UpdateState::Unknown,
            back_to: Screen::Home,
            launched: Instant::now(),
            screen_since: Instant::now(),
            fullscreen: false,
            maximized: false,
            reduce_motion: false,
            device_name: String::new(),
            os: None,
            setup: SetupState::default(),
            device_section: 0,
            os_update: OsUpdate::default(),
            os_loading: None,
            standby: false,
            confirm: None,
            station_op: None,
            waking: None,
            screens_plan: ScreensPlan::default(),
            join: JoinState::new(),
        }
    }

    /// KyberNow Link OS.
    pub fn is_os(&self) -> bool {
        self.os.is_some()
    }

    /// Sur l'OS : le premier démarrage, depuis sa première étape.
    pub fn start_setup(&mut self) {
        self.setup = SetupState { active: true, lang: if crate::i18n::english() { 0 } else { 1 }, ..SetupState::default() };
        self.go(Screen::Setup);
    }

    /// Un écran du premier démarrage seul (Manette, Réseau), depuis les réglages de l'appareil.
    pub fn open_setup_step(&mut self, step: SetupStep) {
        self.setup.active = false;
        self.setup.step = step;
        self.setup.wifi_pick = None;
        self.setup.password.clear();
        self.setup.net_message = None;
        self.back_to = Screen::Device;
        self.go(Screen::Setup);
    }

    pub fn open_device_settings(&mut self, from: Screen) {
        self.back_to = from;
        self.scroll = 0.0;
        self.go(Screen::Device);
    }

    /// Les rubriques des réglages de l'appareil.
    pub fn device_section(&self) -> Section {
        OS_SECTIONS[self.device_section.min(OS_SECTIONS.len() - 1)]
    }

    /// L'ordinateur choisi sur l'accueil.
    pub fn selected(&self) -> Option<&Host> {
        let id = self.store.selected.as_deref();
        id.and_then(|id| self.store.host(id)).or_else(|| self.store.hosts.first())
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected()?.id.clone();
        self.store.hosts.iter().position(|h| h.id == id)
    }

    /// Les Stations trouvées qui ne sont pas déjà ajoutées.
    pub fn new_found(&self) -> Vec<&Found> {
        self.found.iter().filter(|f| !self.store.hosts.iter().any(|h| h.address == f.address && h.port == f.port)).collect()
    }

    /// Les rubriques de l'écran des réglages ouvert.
    pub fn sections(&self) -> &'static [Section] {
        if self.settings_global { &APP_SECTIONS } else { &HOST_SECTIONS }
    }

    pub fn current_section(&self) -> Section {
        let s = self.sections();
        s[self.section.min(s.len() - 1)]
    }

    /// La première rubrique de la colonne du bureau : le profil y est une
    /// rangée de boutons à part, les réglages commencent à Image.
    pub fn first_desktop_section(&self) -> usize {
        if self.layout == Layout::Desktop && !self.settings_global { 1 } else { 0 }
    }

    pub fn open_settings(&mut self, global: bool, from: Screen) {
        self.settings_global = global;
        self.section = self.first_desktop_section();
        self.scroll = 0.0;
        self.back_to = from;
        self.confirm = None;
        // Un redémarrage fini (ou raté) ne se raconte plus en rouvrant les réglages.
        if self.station_op.as_ref().is_some_and(|o| !o.busy()) {
            self.station_op = None;
        }
        self.go(Screen::Settings);
    }

    /// Ouvre une rubrique des réglages de l'ordinateur choisi.
    pub fn open_host_section(&mut self, section: Section, from: Screen) {
        self.open_settings(false, from);
        if let Some(i) = HOST_SECTIONS.iter().position(|s| *s == section) {
            self.section = i;
            self.focus = Target::Section(i);
            if self.layout == Layout::Desktop {
                self.focus = self.first_setting().map(Target::Setting).unwrap_or(Target::Back);
            }
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    pub fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.screen_since = Instant::now();
        self.focus = match screen {
            Screen::Launch | Screen::Home => {
                if self.store.hosts.is_empty() {
                    if self.new_found().is_empty() { Target::AddCard } else { Target::Found(0) }
                } else {
                    Target::Connect
                }
            }
            Screen::Add => match self.add.step {
                AddStep::Code => {
                    if self.new_found().is_empty() { Target::AddByAddress } else { Target::AddFound(0) }
                }
                AddStep::Address | AddStep::Mac => Target::KeyOk,
                AddStep::Verify { .. } => Target::VerifyOk,
            },
            Screen::Join => match (self.layout.console(), self.join.field) {
                (false, JoinField::Code) => Target::JoinCode,
                (false, JoinField::Name) => Target::JoinName,
                (true, JoinField::Code) => Target::Key('1'),
                (true, JoinField::Name) => Target::Key('q'),
            },
            Screen::Settings => {
                if self.layout == Layout::Desktop {
                    self.first_setting().map(Target::Setting).unwrap_or(Target::Back)
                } else {
                    Target::Section(self.section)
                }
            }
            Screen::Gamepads => Target::Pad(self.pad_selected.min(self.pads.len().saturating_sub(1))),
            Screen::Session => Target::Menu(MenuItem::Resume),
            Screen::Setup => match self.setup.step {
                SetupStep::Controller => Target::SetupContinue,
                SetupStep::Language => Target::SetupLang(self.setup.lang),
                SetupStep::Network => {
                    if self.setup.wifi_pick.is_some() {
                        Target::KeyOk
                    } else if self.net.kind == NetKind::Ethernet {
                        Target::SetupContinue
                    } else if self.setup.wifi.is_empty() {
                        Target::SetupRescan
                    } else {
                        Target::SetupWifi(0)
                    }
                }
                SetupStep::Usage => Target::SetupUsage(self.setup.usage),
                SetupStep::Computer => Target::SetupContinue,
            },
            Screen::Device => Target::DeviceSection(self.device_section),
            Screen::Standby => Target::Back,
        };
    }

    fn first_setting(&self) -> Option<Setting> {
        self.rows(self.current_section()).into_iter().find(|r| r.enabled && !matches!(r.kind, RowKind::Info(_))).map(|r| r.setting)
    }

    fn first_device_setting(&self) -> Option<Setting> {
        let s = self.device_section();
        if s == Section::OsUpdate {
            return Some(if self.os.as_ref().is_some_and(|o| !o.pending.is_empty()) || matches!(self.update, UpdateState::Available { .. }) { Setting::OsUpdateApply } else { Setting::OsAutoUpdate });
        }
        self.rows(s).into_iter().find(|r| r.enabled && !matches!(r.kind, RowKind::Info(_))).map(|r| r.setting)
    }

    /// Le focus est sur une valeur au choix ou un interrupteur (‹ › les changent).
    fn focus_adjustable(&self) -> bool {
        let Target::Setting(s) = self.focus else { return false };
        if s == Setting::OsAutoUpdate {
            return true;
        }
        let secs: &[Section] = if self.screen == Screen::Device { &OS_SECTIONS } else { self.sections() };
        secs.iter().flat_map(|sec| self.rows(*sec)).any(|r| r.setting == s && matches!(r.kind, RowKind::Choice { .. } | RowKind::Toggle(_)))
    }

    /// Choisit un profil pour un ordinateur : ce qui s'en écartait (format,
    /// débit, 4:4:4 en Jeu, transport fiable hors Bureau) revient au profil.
    pub fn set_profile(&mut self, id: &str, p: Profile) {
        if let Some(h) = self.store.host_mut(id) {
            let st = &mut h.settings;
            st.profile = p;
            st.codec = "auto".into();
            st.mbps = 0;
            if p == Profile::Jeu {
                st.yuv444 = false;
            }
            if p != Profile::Bureau {
                st.reliable = false;
            }
        }
    }

    /// L'adresse MAC de l'ordinateur est en cours de saisie.
    pub fn typing_mac(&self) -> bool {
        self.screen == Screen::Add && self.add.step == AddStep::Mac
    }

    /// Le mot de passe d'un réseau Wi-Fi est en cours de saisie.
    pub fn typing_password(&self) -> bool {
        self.screen == Screen::Setup && self.setup.step == SetupStep::Network && self.setup.wifi_pick.is_some()
    }

    /// « Rejoindre un partage » : le code ou le prénom est en cours de saisie.
    pub fn typing_join(&self) -> bool {
        self.screen == Screen::Join
    }

    /// L'étape suivante du premier démarrage (ou la fin de l'étape seule).
    fn setup_next(&mut self) -> Vec<Action> {
        let mut acts = Vec::new();
        if !self.setup.active {
            if self.setup.step == SetupStep::Controller {
                acts.push(Action::Os(OsAction::BtListen(false)));
            }
            self.open_device_settings(Screen::Home);
            return acts;
        }
        match self.setup.step {
            SetupStep::Controller => {
                acts.push(Action::Os(OsAction::BtListen(false)));
                self.setup.step = SetupStep::Language;
            }
            SetupStep::Language => {
                self.store.app.language = if self.setup.lang == 0 { "en".into() } else { "fr".into() };
                acts.push(Action::Language);
                acts.push(Action::Save);
                self.setup.step = SetupStep::Network;
                if self.net.kind != NetKind::Ethernet {
                    self.setup.wifi_scanning = true;
                    acts.push(Action::Os(OsAction::WifiScan));
                }
            }
            SetupStep::Network => {
                self.setup.wifi_pick = None;
                self.setup.step = SetupStep::Usage;
            }
            SetupStep::Usage => {
                self.store.app.interface = if self.setup.usage == 0 { "tv".into() } else { "desktop".into() };
                acts.push(Action::Save);
                self.setup.step = SetupStep::Computer;
                // L'ordinateur : l'écran d'ajout par code, puis la fin.
                self.add.step = AddStep::Code;
                self.add.message = None;
                self.go(Screen::Add);
                acts.push(Action::StartPairing);
                return acts;
            }
            SetupStep::Computer => {
                return self.finish_setup();
            }
        }
        self.go(Screen::Setup);
        acts
    }

    /// Fin du premier démarrage : l'usage choisi s'applique, l'accueil.
    pub fn finish_setup(&mut self) -> Vec<Action> {
        self.setup.active = false;
        self.store.app.os_setup_done = true;
        self.go(Screen::Home);
        vec![Action::Save, Action::Interface, Action::Os(OsAction::SetupDone), Action::StopPairing]
    }

    /// Fin du lancement animé : l'accueil prend la main.
    pub fn launch_done(&mut self) {
        if self.screen == Screen::Launch {
            if self.is_os() && !self.store.app.os_setup_done {
                self.start_setup();
            } else {
                self.go(Screen::Home);
            }
        }
    }

    /// Souris déplacée : le focus suit ce qu'elle survole.
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let changed = self.pointer != Some((x, y));
        self.pointer = Some((x, y));
        if self.layout == Layout::Desktop {
            self.keyboard_focus = false;
        }
        if let Some((t, _)) = self.hits.iter().rev().find(|(_, r)| r.contains(x, y))
            && self.focus != *t
        {
            self.focus = *t;
            return true;
        }
        changed
    }

    /// Clic : choisit ce qui est dessous.
    pub fn pointer_click(&mut self, x: f32, y: f32) -> Vec<Action> {
        if let Some((t, _)) = self.hits.iter().rev().find(|(_, r)| r.contains(x, y)).copied() {
            self.focus = t;
            return self.activate(t);
        }
        // Un clic hors de la barre ferme son menu.
        if let Some(s) = self.session_mut() {
            s.bar_menu = None;
        }
        Vec::new()
    }

    /// Molette (réglages du bureau).
    pub fn wheel(&mut self, dy: f32) {
        if self.screen == Screen::Settings {
            self.scroll = (self.scroll - dy * 48.0).max(0.0);
        }
    }

    /// Une touche de navigation.
    pub fn nav(&mut self, n: Nav) -> Vec<Action> {
        self.keyboard_focus = true;
        if self.screen == Screen::Launch {
            if matches!(n, Nav::Ok | Nav::Back) {
                self.launch_done();
            }
            return Vec::new();
        }
        // Une confirmation ouverte : ses deux boutons, B l'annule.
        if self.confirm.is_some() {
            match n {
                Nav::Ok => {
                    let t = self.focus;
                    return self.activate(t);
                }
                Nav::Back => return self.activate(Target::ConfirmNo),
                Nav::Up | Nav::Down | Nav::Left | Nav::Right => self.move_focus(n),
                _ => {}
            }
            return Vec::new();
        }
        match n {
            // « Rejoindre un partage » : Entrée dans un champ passe au suivant, ou rejoint quand tout est donné.
            Nav::Ok if self.screen == Screen::Join && matches!(self.focus, Target::JoinCode | Target::JoinName) => {
                if self.join.ready() {
                    return self.activate(Target::JoinGo);
                }
                let (field, focus) = if self.focus == Target::JoinCode { (JoinField::Name, Target::JoinName) } else { (JoinField::Code, Target::JoinCode) };
                self.join.field = field;
                self.focus = focus;
                Vec::new()
            }
            Nav::Ok => {
                let t = self.focus;
                self.activate(t)
            }
            Nav::Back => self.back(),
            Nav::Join => {
                if self.screen == Screen::Home {
                    return self.activate(Target::JoinCard);
                }
                Vec::new()
            }
            Nav::Options => {
                if self.screen == Screen::Home && !self.store.hosts.is_empty() {
                    return self.activate(Target::HostSettings);
                }
                if self.screen == Screen::Add && self.add.step == AddStep::Code {
                    return self.activate(Target::AddByAddress);
                }
                Vec::new()
            }
            Nav::Add => {
                if self.screen == Screen::Home {
                    return self.activate(Target::AddCard);
                }
                Vec::new()
            }
            Nav::Menu => {
                if self.screen == Screen::Home {
                    // Sur l'OS, ☰ ouvre les réglages de l'appareil (la langue y est une rubrique).
                    if self.is_os() {
                        self.open_device_settings(Screen::Home);
                        return vec![Action::Os(OsAction::Status)];
                    }
                    return self.activate(Target::AppSettings);
                }
                Vec::new()
            }
            Nav::PrevTab | Nav::NextTab => {
                let dir = if n == Nav::NextTab { 1 } else { -1 };
                match self.screen {
                    Screen::Home => {
                        let Some(h) = self.selected().map(|h| (h.id.clone(), h.settings.profile)) else { return Vec::new() };
                        let i = Profile::ALL.iter().position(|p| *p == h.1).unwrap_or(0);
                        let p = Profile::ALL[cycle(i, 3, dir)];
                        self.set_profile(&h.0, p);
                        vec![Action::Save]
                    }
                    Screen::Device => {
                        self.device_section = cycle(self.device_section, OS_SECTIONS.len(), dir);
                        self.scroll = 0.0;
                        self.focus = Target::DeviceSection(self.device_section);
                        Vec::new()
                    }
                    Screen::Settings => {
                        let first = self.first_desktop_section();
                        let n = self.sections().len() - first;
                        self.section = first + cycle(self.section.saturating_sub(first), n, dir);
                        self.scroll = 0.0;
                        self.focus = Target::Section(self.section);
                        Vec::new()
                    }
                    _ => Vec::new(),
                }
            }
            Nav::Char(c) => {
                if self.screen == Screen::Add && matches!(self.add.step, AddStep::Address | AddStep::Mac) || self.typing_password() || self.typing_join() {
                    return self.activate(Target::Key(c));
                }
                Vec::new()
            }
            Nav::Erase => {
                if self.screen == Screen::Add && matches!(self.add.step, AddStep::Address | AddStep::Mac) || self.typing_password() || self.typing_join() {
                    return self.activate(Target::KeyErase);
                }
                Vec::new()
            }
            // ‹ › changent une valeur ou un interrupteur ; sur un bouton, ils déplacent le focus.
            Nav::Left | Nav::Right if self.layout.console() && self.focus_adjustable() => {
                let Target::Setting(s) = self.focus else { return Vec::new() };
                self.adjust(s, if n == Nav::Right { 1 } else { -1 })
            }
            Nav::Left | Nav::Right if self.screen == Screen::Home && matches!(self.focus, Target::Connect | Target::Wake | Target::Retry) && self.layout.console() && self.store.hosts.len() > 1 => {
                // Accueil à la manette : ‹ › changent d'ordinateur.
                let i = self.selected_index().unwrap_or(0);
                let j = cycle(i, self.store.hosts.len(), if n == Nav::Right { 1 } else { -1 });
                self.store.selected = Some(self.store.hosts[j].id.clone());
                vec![Action::Save]
            }
            _ => {
                self.move_focus(n);
                self.follow_join_field();
                Vec::new()
            }
        }
    }

    /// « Rejoindre un partage » : le champ que le focus atteint avec les flèches devient celui où l'on tape
    /// (au pavé, le pavé change : chiffres pour le code, lettres pour le prénom).
    fn follow_join_field(&mut self) {
        if self.screen != Screen::Join {
            return;
        }
        match self.focus {
            Target::JoinCode => self.join.field = JoinField::Code,
            Target::JoinName => self.join.field = JoinField::Name,
            _ => {}
        }
    }

    /// Navigation spatiale : l'élément le plus proche dans la direction.
    fn move_focus(&mut self, n: Nav) {
        let Some(cur) = self.hits.iter().find(|(t, _)| *t == self.focus).map(|(_, r)| *r) else {
            if let Some((t, _)) = self.hits.first() {
                self.focus = *t;
            }
            return;
        };
        let (cx, cy) = cur.center();
        let mut best: Option<(f32, Target)> = None;
        for (t, r) in &self.hits {
            if *t == self.focus {
                continue;
            }
            let (x, y) = r.center();
            let (dx, dy) = (x - cx, y - cy);
            let (along, across, overlap) = match n {
                Nav::Right => (r.x - (cur.x + cur.w) + 1.0, dy, r.y < cur.y + cur.h && r.y + r.h > cur.y),
                Nav::Left => (cur.x - (r.x + r.w) + 1.0, dy, r.y < cur.y + cur.h && r.y + r.h > cur.y),
                Nav::Down => (r.y - (cur.y + cur.h) + 1.0, dx, r.x < cur.x + cur.w && r.x + r.w > cur.x),
                Nav::Up => (cur.y - (r.y + r.h) + 1.0, dx, r.x < cur.x + cur.w && r.x + r.w > cur.x),
                _ => return,
            };
            if along <= 0.0 {
                continue;
            }
            let score = along + across.abs() * if overlap { 0.3 } else { 2.0 } + if overlap { 0.0 } else { 400.0 };
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, *t));
            }
        }
        if let Some((_, t)) = best {
            self.focus = t;
        }
    }

    pub fn back(&mut self) -> Vec<Action> {
        match self.screen {
            Screen::Launch => {
                self.launch_done();
                Vec::new()
            }
            Screen::Home => {
                if self.layout.console() {
                    vec![Action::Quit]
                } else {
                    Vec::new()
                }
            }
            Screen::Add => match self.add.step {
                AddStep::Code if self.setup.active => self.finish_setup(),
                AddStep::Code => {
                    self.go(Screen::Home);
                    vec![Action::StopPairing]
                }
                AddStep::Mac => {
                    self.leave_mac();
                    Vec::new()
                }
                _ => {
                    self.add.step = AddStep::Code;
                    self.add.message = None;
                    self.go(Screen::Add);
                    Vec::new()
                }
            },
            // Une recherche en cours est abandonnée : sa réponse n'ouvrira rien (elle repart de la Station).
            Screen::Join => {
                self.join.attempt += 1;
                self.join.busy = false;
                self.go(Screen::Home);
                Vec::new()
            }
            Screen::Standby => {
                self.standby = false;
                self.go(Screen::Home);
                vec![Action::Os(OsAction::Standby(false))]
            }
            Screen::Setup => {
                // Un mot de passe en cours : revenir à la liste ; une étape seule : les réglages ;
                // le premier démarrage : l'étape précédente (la première ne recule pas).
                if self.setup.wifi_pick.is_some() {
                    self.setup.wifi_pick = None;
                    self.setup.net_message = None;
                    self.go(Screen::Setup);
                    return Vec::new();
                }
                if !self.setup.active {
                    let stop = if self.setup.step == SetupStep::Controller { vec![Action::Os(OsAction::BtListen(false))] } else { Vec::new() };
                    self.open_device_settings(Screen::Home);
                    return stop;
                }
                let i = SetupStep::ALL.iter().position(|s| *s == self.setup.step).unwrap_or(0);
                if i > 0 {
                    self.setup.step = SetupStep::ALL[i - 1];
                    let mut acts = Vec::new();
                    if self.setup.step == SetupStep::Controller {
                        acts.push(Action::Os(OsAction::BtListen(true)));
                    }
                    self.go(Screen::Setup);
                    return acts;
                }
                Vec::new()
            }
            Screen::Device => {
                if matches!(self.focus, Target::Setting(_) | Target::Choice(..)) {
                    self.focus = Target::DeviceSection(self.device_section);
                    return Vec::new();
                }
                let to = self.back_to;
                self.back_to = Screen::Home;
                if to == Screen::Session && self.session().is_some() {
                    self.screen = Screen::Session;
                    self.focus = Target::Menu(MenuItem::Resume);
                } else {
                    self.go(Screen::Home);
                }
                Vec::new()
            }
            Screen::Settings | Screen::Gamepads => {
                if self.screen == Screen::Settings && self.layout.console() && matches!(self.focus, Target::Setting(_) | Target::Choice(..)) {
                    self.focus = Target::Section(self.section);
                    return Vec::new();
                }
                let to = self.back_to;
                self.back_to = Screen::Home;
                if to == Screen::Session && self.session().is_some() {
                    self.screen = Screen::Session;
                    // Un invité à qui l'hôte a repris les manettes n'a plus la tuile : le focus revient sur « Reprendre ».
                    let pads = self.session().and_then(|s| s.role()).is_none_or(|r| r.pads());
                    self.focus = Target::Menu(if pads { MenuItem::Gamepads } else { MenuItem::Resume });
                } else if to == Screen::Settings && self.screen == Screen::Gamepads {
                    self.screen = Screen::Settings;
                    self.focus = Target::Setting(Setting::Pads);
                } else {
                    self.go(Screen::Home);
                }
                Vec::new()
            }
            Screen::Session => self.session_back(),
        }
    }

    /// Retour en session : ouvre ou ferme le menu.
    pub fn session_back(&mut self) -> Vec<Action> {
        let Some(s) = Self::active_in(&mut self.sessions, self.active) else { return Vec::new() };
        let id = s.id;
        match s.phase {
            Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages => vec![Action::CancelConnect(id)],
            Phase::Ended { .. } => {
                // La session finie sur une erreur se ferme ; la dernière ramène l'accueil.
                self.close_session(id);
                if self.sessions.is_empty() {
                    self.go(Screen::Home);
                }
                Vec::new()
            }
            _ => {
                if s.keys {
                    s.keys = false;
                    self.focus = Target::Menu(MenuItem::Keys);
                    return Vec::new();
                }
                // Le panneau du partage se referme sur l'image (il s'ouvre depuis le menu, ou la barre).
                if s.share_overlay() {
                    // Le panneau de la qualité de l'image d'un invité (à la manette) revient au menu d'où il s'ouvre.
                    let quality = s.share_panel && s.guest.is_some();
                    s.share_panel = false;
                    if s.bar_menu == Some(BarMenu::Share) {
                        s.bar_menu = None;
                    }
                    if quality {
                        s.menu = true;
                        self.focus = Target::Menu(MenuItem::Quality);
                    }
                    return Vec::new();
                }
                s.menu = !s.menu;
                if s.menu {
                    self.focus = Target::Menu(MenuItem::Resume);
                    vec![Action::ReleaseAll(id)]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Ouvre le menu de session (combinaison de la manette, toucher en haut).
    pub fn open_menu(&mut self) -> Vec<Action> {
        if let Some(s) = Self::active_in(&mut self.sessions, self.active)
            && !s.menu
            && !s.share_panel
            && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. })
        {
            let id = s.id;
            s.menu = true;
            s.keys = false;
            s.bar = false;
            s.bar_menu = None;
            self.screen = Screen::Session;
            self.focus = Target::Menu(MenuItem::Resume);
            return vec![Action::ReleaseAll(id)];
        }
        Vec::new()
    }

    pub fn activate(&mut self, t: Target) -> Vec<Action> {
        match t {
            Target::Connect => {
                let Some(h) = self.selected() else { return Vec::new() };
                let profile = h.settings.profile;
                vec![Action::Connect { host: h.id.clone(), profile }]
            }
            Target::Chip(p) => {
                let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
                self.set_profile(&id, p);
                vec![Action::Save]
            }
            Target::CustomChip => {
                self.open_settings(false, Screen::Home);
                Vec::new()
            }
            Target::Retry => {
                if let Some(id) = self.selected().map(|h| h.id.clone()) {
                    self.live.entry(id).or_default().online = Online::Unknown;
                }
                vec![Action::Poll]
            }
            Target::ConfirmNo => {
                self.close_confirm();
                Vec::new()
            }
            Target::ConfirmYes => self.confirmed(),
            Target::Host(i) => {
                let Some(h) = self.store.hosts.get(i) else { return Vec::new() };
                let id = h.id.clone();
                if self.store.selected.as_deref() == Some(&id) {
                    self.focus = Target::Connect;
                    return Vec::new();
                }
                self.store.selected = Some(id);
                vec![Action::Save]
            }
            Target::Found(i) | Target::AddFound(i) => {
                let Some(f) = self.new_found().get(i).map(|f| (*f).clone()) else { return Vec::new() };
                self.start_add();
                self.add.message = Some(format!("{} {}", tr("Code présenté à", "Code shown to"), f.name));
                vec![Action::StartPairing]
            }
            Target::AddCard => {
                self.start_add();
                vec![Action::StartPairing]
            }
            Target::JoinCard => self.start_join(),
            Target::JoinCode => {
                self.join.field = JoinField::Code;
                Vec::new()
            }
            Target::JoinName => {
                self.join.field = JoinField::Name;
                Vec::new()
            }
            Target::JoinGo => self.join_go(),
            // « Qualité de l'image », sous le prénom : un des cinq choix (retenu au « Rejoindre »).
            Target::JoinQuality(q) => {
                if !self.join.busy {
                    self.join.quality = q;
                    self.join.message = None;
                }
                Vec::new()
            }
            Target::BarShare => self.share_button(false),
            // « Copier le lien » copie l'adresse du client web (`web`) ; le QR code porte le lien des apps.
            Target::ShareCopy => match self.session().and_then(|s| s.share.as_ref().map(|v| (s.id, v.web.clone()))) {
                Some((id, web)) => {
                    if let Some(s) = self.session_by_mut(id) {
                        s.share_copied = Some((Instant::now(), true));
                    }
                    vec![Action::CopyText(web)]
                }
                None => Vec::new(),
            },
            Target::SharePads(i) => self.share_guest_move(i, ShareMove::Pads),
            Target::ShareKbm(i) => self.share_guest_move(i, ShareMove::Kbm),
            Target::ShareKick(i) => {
                let Some((id, guest)) = self.session().and_then(|s| s.share.as_ref().and_then(|v| v.guests.get(i)).map(|g| (s.id, g.id.clone()))) else { return Vec::new() };
                self.share_op(id, ShareOp::Kick(guest))
            }
            Target::ShareNewCode => match self.active_id() {
                Some(id) => self.share_op(id, ShareOp::NewCode),
                None => Vec::new(),
            },
            Target::ShareStop => match self.active_id() {
                Some(id) => self.share_op(id, ShareOp::Close),
                None => Vec::new(),
            },
            Target::ShareClose => {
                if let Some(s) = self.session_mut() {
                    s.share_panel = false;
                    if s.bar_menu == Some(BarMenu::Share) {
                        s.bar_menu = None;
                    }
                }
                Vec::new()
            }
            Target::AddByAddress => {
                self.add.step = AddStep::Address;
                self.add.message = None;
                self.go(Screen::Add);
                vec![Action::OpenKeyboard(None)]
            }
            Target::Key(c) if self.typing_join() => {
                self.join_type(c);
                Vec::new()
            }
            Target::KeyErase if self.typing_join() => {
                if !self.join.busy {
                    match self.join.field {
                        JoinField::Code => self.join.code.pop(),
                        JoinField::Name => self.join.name.pop(),
                    };
                    self.join.message = None;
                }
                Vec::new()
            }
            Target::KeyShift if self.typing_join() => {
                self.join.shift = !self.join.shift;
                Vec::new()
            }
            Target::Key(c) => {
                if self.typing_password() {
                    if self.setup.password.chars().count() < 63 {
                        let c = if self.setup.shift { c.to_uppercase().next().unwrap_or(c) } else { c };
                        self.setup.password.push(c);
                        self.setup.shift = false;
                    }
                } else if self.typing_mac() {
                    if (c.is_ascii_hexdigit() || matches!(c, ':' | '-')) && self.add.mac.len() < 17 {
                        self.add.mac.push(if c == '-' { ':' } else { c.to_ascii_uppercase() });
                        self.add.message = None;
                    }
                } else if self.add.address.len() < 60 {
                    self.add.address.push(c);
                }
                Vec::new()
            }
            Target::KeyErase => {
                if self.typing_password() {
                    self.setup.password.pop();
                } else if self.typing_mac() {
                    self.add.mac.pop();
                } else {
                    self.add.address.pop();
                }
                Vec::new()
            }
            Target::KeyShift => {
                self.setup.shift = !self.setup.shift;
                Vec::new()
            }
            Target::KeyOk if self.typing_password() => {
                let Some(i) = self.setup.wifi_pick else { return Vec::new() };
                let Some(n) = self.setup.wifi.get(i) else { return Vec::new() };
                let ssid = n.ssid.clone();
                self.setup.net_busy = true;
                self.setup.net_message = Some(format!("{} {ssid}…", tr("Connexion à", "Connecting to")));
                self.focus = Target::KeyOk;
                vec![Action::Os(OsAction::WifiConnect { ssid, password: self.setup.password.clone() })]
            }
            Target::KeyOk if self.typing_mac() => {
                // Douze chiffres hexadécimaux, séparés ou non ; vide : plus de réveil.
                let hex: String = self.add.mac.chars().filter(char::is_ascii_hexdigit).collect();
                if !hex.is_empty() && hex.len() != 12 {
                    self.add.message = Some(tr("Six paires de chiffres hexadécimaux, par exemple 3C:7C:3F:12:AB:CD", "Six pairs of hexadecimal digits, for example 3C:7C:3F:12:AB:CD").into());
                    return Vec::new();
                }
                let mac = (!hex.is_empty()).then(|| hex.as_bytes().chunks(2).map(|p| String::from_utf8_lossy(p).into_owned()).collect::<Vec<_>>().join(":"));
                let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
                if let Some(h) = self.store.host_mut(&id) {
                    h.mac = mac;
                }
                self.leave_mac();
                vec![Action::Save]
            }
            Target::SetupContinue => self.setup_next(),
            Target::SetupSkip => {
                // Le réseau : passer (Ethernet plus tard, ou le Wi-Fi depuis les réglages).
                if self.setup.step == SetupStep::Network {
                    self.setup.wifi_pick = None;
                    return self.setup_next();
                }
                Vec::new()
            }
            // Une carte (langue, usage) : la choisir, c'est continuer (A « Choisir »).
            Target::SetupLang(i) => {
                self.setup.lang = i;
                self.setup_next()
            }
            Target::SetupUsage(i) => {
                self.setup.usage = i;
                self.setup_next()
            }
            Target::SetupWifi(i) => {
                let Some(n) = self.setup.wifi.get(i).cloned() else { return Vec::new() };
                if n.active {
                    return self.setup_next();
                }
                self.setup.password.clear();
                self.setup.shift = false;
                self.setup.net_message = None;
                if n.secured {
                    self.setup.wifi_pick = Some(i);
                    self.focus = Target::Key('a');
                    Vec::new()
                } else {
                    self.setup.net_busy = true;
                    self.setup.net_message = Some(format!("{} {}…", tr("Connexion à", "Connecting to"), n.ssid));
                    vec![Action::Os(OsAction::WifiConnect { ssid: n.ssid, password: String::new() })]
                }
            }
            Target::SetupRescan => {
                self.setup.wifi_scanning = true;
                self.setup.net_message = None;
                vec![Action::Os(OsAction::WifiScan)]
            }
            Target::DeviceSection(i) => {
                self.device_section = i.min(OS_SECTIONS.len() - 1);
                self.scroll = 0.0;
                if let Some(s) = self.first_device_setting() {
                    self.focus = Target::Setting(s);
                }
                Vec::new()
            }
            Target::KeyOk => {
                let a = self.add.address.trim().to_string();
                match parse_address(&a) {
                    Some((address, port)) => {
                        self.add.busy = true;
                        self.add.message = Some(tr("Connexion à Admin…", "Reaching Admin…").into());
                        vec![Action::Probe { address, port }]
                    }
                    None => {
                        self.add.message = Some(tr("Adresse et port d'Admin, par exemple 192.168.1.20:24811", "Admin address and port, for example 192.168.1.20:24811").into());
                        Vec::new()
                    }
                }
            }
            Target::VerifyOk => {
                let AddStep::Verify { address, port, fingerprint } = self.add.step.clone() else { return Vec::new() };
                self.add.step = AddStep::Code;
                self.add.code = crate::model::pair_code();
                self.add.started = Instant::now();
                self.add.message = Some(format!("{} {address}", tr("Code présenté à", "Code shown to")));
                self.go(Screen::Add);
                vec![Action::PairAddress { address, port, fingerprint }]
            }
            Target::VerifyNo => {
                self.add.step = AddStep::Address;
                self.go(Screen::Add);
                Vec::new()
            }
            Target::Wake => {
                let Some(id) = self.selected().filter(|h| h.can_wake()).map(|h| h.id.clone()) else { return Vec::new() };
                self.waking = Some((id.clone(), Instant::now()));
                vec![Action::Wake(id)]
            }
            Target::HostSettings => {
                if self.selected().is_none() {
                    return Vec::new();
                }
                self.open_settings(false, Screen::Home);
                Vec::new()
            }
            Target::AppSettings => {
                if self.is_os() {
                    self.open_device_settings(Screen::Home);
                    return vec![Action::Os(OsAction::Status)];
                }
                self.open_settings(true, Screen::Home);
                Vec::new()
            }
            Target::Section(i) => {
                self.section = i;
                self.scroll = 0.0;
                if self.layout.console()
                    && let Some(s) = self.first_setting()
                {
                    self.focus = Target::Setting(s);
                }
                Vec::new()
            }
            Target::Setting(s) => self.adjust(s, 1),
            Target::Choice(s, i) => self.choose(s, i),
            Target::Pad(i) => {
                self.pad_selected = i;
                Vec::new()
            }
            Target::PadTest(m) => {
                let pad = self.pad_selected;
                let triggers = m == 3;
                let (low, high) = match m {
                    0 => (0xC000, 0),
                    1 => (0, 0xC000),
                    _ => (0xC000, 0xC000),
                };
                vec![Action::Rumble { pad, low, high, triggers }]
            }
            Target::Menu(item) => self.menu(item),
            Target::Shortcut(i) | Target::BarShortcut(i) => {
                let keys = SHORTCUTS[i].2.to_vec();
                let Some(s) = self.session_mut() else { return Vec::new() };
                s.keys = false;
                s.menu = false;
                s.bar_menu = None;
                vec![Action::Shortcut(s.id, keys)]
            }
            Target::CancelConnect => self.active_id().map(|id| vec![Action::CancelConnect(id)]).unwrap_or_default(),
            Target::Back => self.back(),
            Target::WinMinimize => vec![Action::Minimize],
            Target::WinMaximize => vec![Action::Maximize],
            Target::WinClose => vec![Action::Quit],
            Target::BarProfile => self.toggle_bar_menu(BarMenu::Profile),
            Target::BarKeys => self.toggle_bar_menu(BarMenu::Keys),
            Target::BarComputers => self.toggle_bar_menu(BarMenu::Computers),
            Target::BarQuality => self.toggle_bar_menu(BarMenu::Quality),
            Target::QualityPick(q) => self.quality_pick(q),
            Target::BarComputer(i) => self.bar_computer(i),
            Target::BarPick(p) => {
                let Some(s) = self.session_mut() else { return Vec::new() };
                s.profile = p;
                s.bar_menu = None;
                let id = s.id;
                self.remember_profile(p);
                vec![Action::SetProfile(id, p), Action::Save]
            }
            Target::BarMouse => self.active_id().map(|id| vec![Action::ToggleMouse(id)]).unwrap_or_default(),
            Target::BarFullscreen => self.active_id().map(|id| vec![Action::ToggleFullscreen(Some(id))]).unwrap_or_default(),
            Target::BarSound => {
                let Some(s) = self.session_mut() else { return Vec::new() };
                s.keep_sound = !s.keep_sound;
                vec![Action::KeepSound(s.id, s.keep_sound)]
            }
            Target::BarQuit => self.active_id().map(|id| vec![Action::Disconnect(id)]).unwrap_or_default(),
            Target::UpdateNotice => match &self.update {
                UpdateState::Available { page, .. } => vec![Action::OpenUrl(page.clone())],
                _ => Vec::new(),
            },
        }
    }

    /// Le profil choisi en session est gardé pour cet ordinateur.
    fn remember_profile(&mut self, p: Profile) {
        if let Some(id) = self.session().map(|s| s.host.clone()) {
            self.set_profile(&id, p);
        }
    }

    /// Ouvre un menu déroulant de la barre, ou le ferme s'il est déjà ouvert.
    fn toggle_bar_menu(&mut self, menu: BarMenu) -> Vec<Action> {
        if let Some(s) = self.session_mut() {
            s.bar_menu = if s.bar_menu == Some(menu) { None } else { Some(menu) };
        }
        Vec::new()
    }

    /// Un ordinateur du menu « Ordinateurs » de la barre. Déjà en session : sa fenêtre passe
    /// devant. Hors ligne : il se réveille (ou se sonde à nouveau) comme sur l'accueil. Sinon
    /// une session de plus s'ouvre, dans une fenêtre, tant qu'il y a de la place.
    fn bar_computer(&mut self, i: usize) -> Vec<Action> {
        let Some(h) = self.store.hosts.get(i) else { return Vec::new() };
        let (id, profile, can_wake) = (h.id.clone(), h.settings.profile, h.can_wake());
        // Une ligne inactive (réveil en cours, quatre sessions) laisse le menu ouvert.
        let offline = self.live.get(&id).is_some_and(|l| l.online == Online::Offline);
        let known = self.session_of_host(&id).map(|s| s.id);
        if known.is_none() && ((offline && can_wake && self.is_waking(&id)) || (!offline && self.sessions_full())) {
            return Vec::new();
        }
        if let Some(s) = self.session_mut() {
            s.bar_menu = None;
        }
        if let Some(session) = known {
            return vec![Action::ShowSession(session)];
        }
        if offline {
            if !can_wake {
                self.live.entry(id).or_default().online = Online::Unknown;
                return vec![Action::Poll];
            }
            self.waking = Some((id.clone(), Instant::now()));
            return vec![Action::Wake(id)];
        }
        vec![Action::Connect { host: id, profile }]
    }

    // MARK: - KyberNow Share

    /// « Rejoindre avec un code » : l'écran du code, du prénom (proposé : celui de la dernière fois, sinon le nom de
    /// cet appareil) et de la « Qualité de l'image » (celle de la dernière fois, sinon « Auto »).
    fn start_join(&mut self) -> Vec<Action> {
        let kept = self.store.app.share_name.trim();
        let name: String = if kept.is_empty() { self.device_name.trim() } else { kept }.chars().take(NAME_MAX).collect();
        let quality = ShareQuality::parse(&self.store.app.share_quality);
        self.join = JoinState { name, quality, attempt: self.join.attempt + 1, ..JoinState::new() };
        self.go(Screen::Join);
        vec![Action::TextInput]
    }

    /// Une lettre ou un chiffre pour le champ de « Rejoindre un partage » : les chiffres du code (six au
    /// plus), le prénom (un caractère imprimable, `NAME_MAX` au plus). À la manette, le sixième chiffre
    /// porte le focus sur « Rejoindre ».
    fn join_type(&mut self, c: char) {
        if self.join.busy {
            return;
        }
        match self.join.field {
            JoinField::Code => {
                if c.is_ascii_digit() && self.join.code.len() < 6 {
                    self.join.code.push(c);
                    self.join.message = None;
                    if self.join.code.len() == 6 && self.layout.console() && self.join.ready() {
                        self.focus = Target::JoinGo;
                    }
                }
            }
            JoinField::Name => {
                if !c.is_control() && self.join.name.chars().count() < NAME_MAX {
                    let c = if self.join.shift { c.to_uppercase().next().unwrap_or(c) } else { c };
                    self.join.name.push(c);
                    self.join.shift = false;
                    self.join.message = None;
                }
            }
        }
    }

    /// « Rejoindre » : le code part aux ordinateurs du réseau ; sans les six chiffres ou sans prénom, le
    /// focus va au champ qui manque. Le prénom est retenu pour la fois suivante.
    fn join_go(&mut self) -> Vec<Action> {
        if self.join.busy {
            return Vec::new();
        }
        if !self.join.ready() {
            let field = if self.join.code.len() < 6 { JoinField::Code } else { JoinField::Name };
            self.join.field = field;
            self.focus = match (self.layout.console(), field) {
                (false, JoinField::Code) => Target::JoinCode,
                (false, JoinField::Name) => Target::JoinName,
                (true, JoinField::Code) => Target::Key('1'),
                (true, JoinField::Name) => Target::Key('q'),
            };
            return Vec::new();
        }
        self.join.busy = true;
        self.join.message = None;
        self.join.started = Instant::now();
        let name = self.join.name.trim().to_string();
        self.store.app.share_name = name.clone();
        let quality = self.join.quality;
        self.store.app.share_quality = quality.as_str().into();
        vec![Action::Save, Action::JoinShare { code: self.join.code.clone(), name, quality }]
    }

    /// Le cœur n'a trouvé aucun partage (ou l'a refusé) : le texte du cœur, sous les champs.
    pub fn join_failed(&mut self, attempt: u32, message: String) {
        if self.screen != Screen::Join || self.join.attempt != attempt || !self.join.busy {
            return;
        }
        self.join.busy = false;
        self.join.message = Some(message);
    }

    /// La recherche `attempt` est toujours celle de l'écran : sa réponse ouvre la session.
    pub fn join_current(&self, attempt: u32) -> bool {
        self.screen == Screen::Join && self.join.attempt == attempt && self.join.busy
    }

    /// Ce que « Partager » peut faire avec l'ordinateur de cette session (`None` : pas encore su).
    pub fn share_avail(&self, s: &SessionUi) -> Option<ShareAvail> {
        self.live.get(&s.host).and_then(|l| l.share)
    }

    /// « Partager » : le bouton de la barre (`menu` faux : à la souris) ou la tuile du menu (`menu` : à la
    /// manette ou au toucher). Il ouvre le partage de l'écran principal (une demande à Admin) et son panneau,
    /// ou, déjà ouvert, le panneau. Partage impossible (Station trop ancienne, partage coupé) : à la souris le
    /// bouton est inactif (estompé, la raison est son infobulle) ; à la manette la tuile se choisit et ouvre le
    /// panneau réduit à la raison. Tant qu'Admin n'a pas dit ce que l'ordinateur permet, le panneau attend sa
    /// réponse (`share_known`) au lieu de laisser un bouton muet.
    fn share_button(&mut self, menu: bool) -> Vec<Action> {
        let Some(s) = self.session() else { return Vec::new() };
        if s.guest.is_some() {
            return Vec::new();
        }
        let (id, open, shown) = (s.id, s.share.is_some(), s.share_overlay());
        let avail = self.share_avail(s);
        let blocked = avail.is_some_and(|a| a != ShareAvail::Available);
        if blocked && !menu && !shown {
            return Vec::new();
        }
        // Un panneau par-dessus l'image à la manette (et sur une interface de console) ; au bureau, à la souris,
        // le menu de la barre.
        let overlay = menu || self.layout.console();
        let Some(s) = self.session_mut() else { return Vec::new() };
        if shown {
            s.share_panel = false;
            s.bar_menu = None;
            return Vec::new();
        }
        s.share_error = None;
        s.bar_menu = None;
        if overlay {
            s.share_panel = true;
        } else {
            s.bar_menu = Some(BarMenu::Share);
        }
        let mut acts = if overlay { vec![Action::ReleaseAll(id)] } else { Vec::new() };
        let focus = if open { Target::ShareNewCode } else { Target::ShareClose };
        if !open && !blocked {
            s.share_busy = true;
            if avail.is_some() {
                acts.push(Action::Share(id, ShareOp::Open));
            } else {
                s.share_wait = true;
                acts.push(Action::Poll);
            }
        }
        self.focus = focus;
        acts
    }

    /// Admin a répondu à un sondage de l'ordinateur `host` : les sessions dont le panneau de « Partager »
    /// attendait cette réponse ouvrent le partage, ou disent pourquoi elles ne le peuvent pas (le panneau se
    /// réduit à la raison). Sans réponse (ordinateur injoignable), la demande part quand même : le refus qui
    /// revient dit pourquoi. Un panneau refermé entre-temps n'ouvre rien.
    pub fn share_known(&mut self, host: &str) -> Vec<Action> {
        let avail = self.live.get(host).and_then(|l| l.share);
        let mut acts = Vec::new();
        for s in self.sessions.iter_mut().filter(|s| s.host == host && s.share_wait) {
            s.share_wait = false;
            if !s.share_overlay() || avail.is_some_and(|a| a != ShareAvail::Available) {
                s.share_busy = false;
            } else {
                acts.push(Action::Share(s.id, ShareOp::Open));
            }
        }
        acts
    }

    /// Une demande de l'hôte à Admin : « Nouveau code », « Retirer », « Arrêter le partage », un rôle.
    fn share_op(&mut self, id: SessionId, op: ShareOp) -> Vec<Action> {
        let Some(s) = self.session_by_mut(id) else { return Vec::new() };
        if s.share_busy {
            return Vec::new();
        }
        s.share_busy = true;
        s.share_error = None;
        vec![Action::Share(id, op)]
    }

    /// Un des deux boutons de l'invité `i` : son droit basculé (donné ou repris), l'autre tel quel. Le rôle voulu
    /// se calcule d'après celui qui est montré : `share_op` ne laisse partir qu'une demande à la fois, jusqu'à la
    /// réponse d'Admin, sinon deux clics rapides sur les deux droits s'annuleraient.
    fn share_guest_move(&mut self, i: usize, m: ShareMove) -> Vec<Action> {
        let Some((id, guest)) = self.session().and_then(|s| s.share.as_ref().and_then(|v| v.guests.get(i)).map(|g| (s.id, g.clone()))) else { return Vec::new() };
        self.share_op(id, ShareOp::Role(guest.id, m.toggled(guest.role)))
    }

    /// La réponse d'Admin à une demande de l'hôte : le partage tel qu'il est maintenant (`None` : fermé),
    /// ou ce qu'Admin a refusé.
    pub fn share_reply(&mut self, id: SessionId, r: Result<Option<ShareView>, String>) {
        let (console, active) = (self.layout.console(), self.active);
        let Some(s) = self.session_by_mut(id) else { return };
        s.share_busy = false;
        let focus_panel = s.share_overlay();
        match r {
            Ok(view) => {
                let first = s.share.is_none() && view.is_some();
                if view.is_none() {
                    s.share_panel = false;
                    if s.bar_menu == Some(BarMenu::Share) {
                        s.bar_menu = None;
                    }
                    s.share_opened = None;
                    s.share_closed = Some(Instant::now());
                } else if first {
                    s.share_opened = Some(Instant::now());
                    s.share_closed = None;
                }
                s.share = view;
                s.share_error = None;
                if first && focus_panel && console && active == Some(id) {
                    self.focus = Target::ShareNewCode;
                }
            }
            Err(e) => s.share_error = Some(e),
        }
    }

    /// Le signe de vie de l'hôte porte la liste des invités (`Platform::on_guests`). Un signe de vie parti avant
    /// une réponse d'Admin ne la défait pas : ni un partage qu'on vient d'ouvrir (fermé pendant 8 s au plus), ni
    /// un partage qu'on vient d'arrêter (rouvert pendant 3 s au plus), comme l'app Mac.
    pub fn share_guests(&mut self, id: SessionId, view: Option<ShareView>) {
        let Some(s) = self.session_by_mut(id) else { return };
        match &view {
            None if s.share_busy || s.share_opened.is_some_and(|t| t.elapsed() < Duration::from_secs(8)) => return,
            Some(_) if s.share_closed.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) => return,
            _ => {}
        }
        if view.is_none() {
            s.share_panel = false;
            if s.bar_menu == Some(BarMenu::Share) {
                s.bar_menu = None;
            }
            s.share_opened = None;
        } else if s.share.is_none() {
            s.share_opened = Some(Instant::now());
        }
        s.share = view;
    }

    /// Un choix de « Qualité de l'image » en session (menu de la barre, ou panneau à la manette) : retenu pour la
    /// fois suivante, et demandé à la Station (l'image de l'invité seulement est relancée, jamais celle de l'hôte).
    /// Le menu ou le panneau se ferme ; choisir la qualité déjà en place ne demande rien.
    fn quality_pick(&mut self, q: ShareQuality) -> Vec<Action> {
        let Some(s) = self.session_mut() else { return Vec::new() };
        let id = s.id;
        let Some(g) = s.guest.as_mut() else { return Vec::new() };
        let changed = g.quality != q;
        g.quality = q;
        s.bar_menu = None;
        s.share_panel = false;
        if !changed {
            return Vec::new();
        }
        self.store.app.share_quality = q.as_str().into();
        vec![Action::ShareQuality(id, q), Action::Save]
    }

    /// L'image que l'invité reçoit (`Platform::on_share_image`, au départ puis à chaque changement) : sa qualité,
    /// que la barre et le menu montrent quand Kyber en sait la taille (`label` nul sinon), et l'avis bref de la
    /// Station s'il y en a un (« Image allégée pour suivre. »).
    pub fn share_image(&mut self, id: SessionId, label: Option<String>, notice: Option<String>) {
        let Some(s) = self.session_by_mut(id) else { return };
        let Some(g) = s.guest.as_mut() else { return };
        g.image = label;
        if let Some(text) = notice {
            s.notice = Some((text, Instant::now()));
        }
    }

    /// Le rôle de l'invité a changé (au départ, `previous` nul ; puis par l'hôte) : l'avis bref, et ce que
    /// le rôle n'ouvre plus se ferme (touches, souris, menus).
    pub fn share_role_changed(&mut self, id: SessionId, role: ShareRole, previous: Option<ShareRole>) {
        let active = self.active;
        let Some(s) = self.session_by_mut(id) else { return };
        let Some(g) = s.guest.as_mut() else { return };
        g.role = role;
        if let Some(text) = role.notice(previous) {
            s.notice = Some((text.into(), Instant::now()));
        }
        // La souris des jeux se rend dans la boucle de l'appareil (`session_tick`), qui seule règle SDL.
        if !role.kbm() {
            s.keys = false;
            if s.bar_menu == Some(BarMenu::Keys) {
                s.bar_menu = None;
            }
            if s.pad_mouse {
                s.pad_mouse = false;
            }
        }
        if s.menu && active == Some(id) {
            self.focus = Target::Menu(MenuItem::Resume);
        }
    }

    // MARK: - Confirmations et Station

    /// Ferme la confirmation ; le focus revient sur ce qui l'a ouverte.
    fn close_confirm(&mut self) {
        let back = match self.confirm.take() {
            Some(Confirm::Remove { .. }) => Setting::Remove,
            Some(Confirm::Force { kind: StationKind::Update, .. }) => Setting::StationUpdate,
            _ => Setting::StationRestart,
        };
        self.focus = Target::Setting(back);
    }

    fn open_confirm(&mut self, c: Confirm) {
        self.confirm = Some(c);
        // Annuler a le focus : deux appuis sur A ne retirent rien par mégarde.
        self.focus = Target::ConfirmNo;
    }

    fn confirmed(&mut self) -> Vec<Action> {
        match self.confirm.take() {
            Some(Confirm::Remove { host, name }) => {
                self.store.remove(&host);
                self.live.remove(&host);
                self.toast(format!("{name} {}", tr("est retiré de cet appareil", "is removed from this device")));
                self.go(Screen::Home);
                vec![Action::Save]
            }
            Some(Confirm::Force { host, kind, .. }) => {
                self.focus = Target::Setting(if kind == StationKind::Update { Setting::StationUpdate } else { Setting::StationRestart });
                self.start_station(host, kind, true)
            }
            None => Vec::new(),
        }
    }

    /// La Station de l'ordinateur choisi, si elle a répondu.
    pub fn station(&self, id: &str) -> Option<&StationLive> {
        self.live.get(id).and_then(|l| l.station.as_ref())
    }

    /// Redémarrer ou mettre à jour la Station se peut : elle répond, elle le
    /// connaît, rien n'est déjà en cours.
    pub fn station_ready(&self, id: &str) -> bool {
        let online = self.live.get(id).is_some_and(|l| l.online == Online::Online);
        let busy = self.station_op.as_ref().is_some_and(|o| o.busy());
        online && !busy && self.station(id).is_some_and(|s| s.supported && !s.installing && !s.restarting)
    }

    fn start_station(&mut self, host: String, kind: StationKind, force: bool) -> Vec<Action> {
        self.station_op = Some(StationOp { host: host.clone(), kind, step: StationStep::Asking });
        vec![Action::Station { host, kind, force }]
    }

    /// La réponse d'Admin à un redémarrage ou une mise à jour : accepté (la
    /// version visée), ou refusé.
    pub fn station_reply(&mut self, host: &str, kind: StationKind, r: Result<Option<String>, StationRefusal>) {
        if self.station_op.as_ref().is_none_or(|o| o.host != host || o.kind != kind) {
            return;
        }
        let step = match r {
            Ok(version) => {
                let target = version.filter(|v| !v.is_empty()).or_else(|| self.station(host).and_then(|s| s.update_version.clone()));
                StationStep::Waiting { since: Instant::now(), target: if kind == StationKind::Update { target } else { None }, saw_down: false }
            }
            Err(StationRefusal::InSession(device)) => {
                self.station_op = None;
                let device = device.filter(|d| !d.is_empty()).unwrap_or_else(|| tr("Un appareil", "A device").into());
                self.open_confirm(Confirm::Force { host: host.into(), kind, device });
                return;
            }
            Err(StationRefusal::TooSoon) => StationStep::Failed(tr("La Station vient de redémarrer. Réessayez dans une minute.", "The Station has just restarted. Try again in a minute.").into()),
            Err(StationRefusal::RequiresSetup) => {
                let v = self.station(host).and_then(|s| s.update_version.clone()).unwrap_or_default();
                StationStep::Failed(requires_setup_text(&v))
            }
            Err(StationRefusal::Unsupported) => StationStep::Failed(too_old_text().into()),
            Err(StationRefusal::Other(m)) => StationStep::Failed(m),
        };
        if let Some(o) = self.station_op.as_mut() {
            o.step = step;
        }
    }

    /// Un réveil est en cours pour cet ordinateur (2 min au plus).
    pub fn is_waking(&self, id: &str) -> bool {
        self.waking.as_ref().is_some_and(|(h, t)| h == id && t.elapsed().as_secs() < 120)
    }

    /// L'ordinateur qu'on réveille, sondé toutes les 2 s.
    pub fn waking_host(&self) -> Option<&str> {
        self.waking.as_ref().filter(|(_, t)| t.elapsed().as_secs() < 120).map(|(h, _)| h.as_str())
    }

    /// L'ordinateur dont l'app attend le retour de la Station (sondé toutes les 2 s).
    pub fn station_waiting(&self) -> Option<&str> {
        self.station_op.as_ref().filter(|o| matches!(o.step, StationStep::Waiting { .. })).map(|o| o.host.as_str())
    }

    /// Une réponse (ou un silence) de la Station attendue : revenue, à la
    /// bonne version pour une mise à jour ; trop longtemps absente.
    pub fn station_progress(&mut self, id: &str) {
        let online = self.live.get(id).is_some_and(|l| l.online == Online::Online);
        let st = self.station(id).cloned();
        let Some(op) = self.station_op.as_mut().filter(|o| o.host == id) else { return };
        let StationStep::Waiting { since, target, saw_down } = &mut op.step else { return };
        let elapsed = since.elapsed().as_secs();
        let busy = st.as_ref().is_none_or(|s| s.restarting || s.installing);
        if !online || busy {
            *saw_down = true;
        }
        let late = if op.kind == StationKind::Update { 600 } else { 180 };
        let step = match (op.kind, online && !busy, st) {
            (StationKind::Restart, true, _) if *saw_down || elapsed >= 20 => Some(StationStep::Done(tr("La Station a redémarré.", "The Station has restarted.").into())),
            (StationKind::Update, true, Some(s)) if target.as_deref().is_some_and(|t| t == s.version) || (target.is_none() && *saw_down) => {
                Some(StationStep::Done(format!("{} {} {}", tr("KyberNow Station", "KyberNow Station"), s.version, tr("est en place.", "is installed."))))
            }
            (StationKind::Update, true, Some(_)) if *saw_down && elapsed >= 120 => Some(StationStep::Failed(tr("La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus.", "The update did not complete. Open Admin to find out more.").into())),
            _ if elapsed >= late => Some(StationStep::Failed(tr("La Station tarde à revenir. Ouvrez Admin pour en savoir plus.", "The Station is taking a while to come back. Open Admin to find out more.").into())),
            _ => None,
        };
        if let Some(s) = step {
            op.step = s;
        }
    }

    fn start_add(&mut self) {
        self.add = AddState {
            step: AddStep::Code,
            code: crate::model::pair_code(),
            asked: Vec::new(),
            address: self.add.address.clone(),
            mac: String::new(),
            message: None,
            busy: false,
            started: Instant::now(),
        };
        self.go(Screen::Add);
    }

    /// Fin de la saisie de l'adresse MAC : retour aux réglages de l'ordinateur, sur sa ligne.
    fn leave_mac(&mut self) {
        self.add.step = AddStep::Code;
        self.add.message = None;
        self.screen = Screen::Settings;
        self.screen_since = Instant::now();
        self.focus = Target::Setting(Setting::WakeMac);
    }

    fn menu(&mut self, item: MenuItem) -> Vec<Action> {
        let Some(s) = Self::active_in(&mut self.sessions, self.active) else { return Vec::new() };
        let id = s.id;
        match item {
            MenuItem::Resume => {
                s.menu = false;
                Vec::new()
            }
            MenuItem::Profile(p) => {
                s.profile = p;
                s.menu = false;
                self.remember_profile(p);
                vec![Action::SetProfile(id, p), Action::Save]
            }
            MenuItem::Sound => {
                s.muted = !s.muted;
                vec![Action::Mute(id, s.muted)]
            }
            MenuItem::PadMouse => {
                s.pad_mouse = !s.pad_mouse;
                s.menu = false;
                vec![Action::PadMouse(id, s.pad_mouse)]
            }
            MenuItem::Keyboard => {
                s.menu = false;
                vec![Action::OpenKeyboard(Some(id))]
            }
            MenuItem::Keys => {
                s.keys = true;
                self.focus = Target::Shortcut(0);
                Vec::new()
            }
            MenuItem::Stats => {
                s.show_stats = !s.show_stats;
                s.menu = false;
                Vec::new()
            }
            MenuItem::Light => {
                s.light = !s.light;
                s.menu = false;
                vec![Action::Light(id, s.light)]
            }
            MenuItem::Gamepads => {
                self.back_to = Screen::Session;
                self.go(Screen::Gamepads);
                Vec::new()
            }
            // « Partager » : le menu se ferme, le panneau du partage s'ouvre sur l'image.
            MenuItem::Share => {
                s.menu = false;
                self.share_button(true)
            }
            // « Qualité de l'image » (invité) : le menu se ferme, les cinq choix s'ouvrent sur l'image ; B revient au menu.
            MenuItem::Quality => {
                let Some(q) = s.guest.as_ref().map(|g| g.quality) else { return Vec::new() };
                s.menu = false;
                s.share_panel = true;
                self.focus = Target::QualityPick(q);
                vec![Action::ReleaseAll(id)]
            }
            MenuItem::Disconnect => vec![Action::Disconnect(id)],
        }
    }

    // MARK: - Réglages

    /// Les lignes d'une rubrique, pour les trois interfaces.
    pub fn rows(&self, section: Section) -> Vec<Row> {
        let m = &self.machine;
        let choice = |setting, label: &str, hint: String, options: Vec<String>, current: usize, disabled: Vec<bool>| Row {
            setting,
            label: label.to_string(),
            hint,
            kind: RowKind::Choice { options, current, disabled, tags: Vec::new() },
            enabled: true,
            tag: None,
        };
        let toggle = |setting, label: &str, hint: String, on: bool| Row { setting, label: label.to_string(), hint, kind: RowKind::Toggle(on), enabled: true, tag: None };
        let info = |label: &str, hint: String, value: String| Row { setting: Setting::Info, label: label.to_string(), hint, kind: RowKind::Info(value), enabled: true, tag: None };
        let button = |setting, text: &str, hint: String, style: ButtonStyle, enabled: bool| Row { setting, label: text.to_string(), hint, kind: RowKind::Button { text: text.to_string(), style }, enabled, tag: None };
        if section == Section::App {
            let lang = ["auto", "fr", "en"].iter().position(|v| *v == self.store.app.language).unwrap_or(0);
            let iface = ["auto", "desktop", "handheld", "tv"].iter().position(|v| *v == self.store.app.interface).unwrap_or(0);
            let update = match &self.update {
                UpdateState::Unknown => String::new(),
                UpdateState::Checking => tr("Recherche…", "Checking…").into(),
                UpdateState::UpToDate => tr("À jour", "Up to date").into(),
                UpdateState::Available { version, .. } => format!("{} {version} {}", tr("Version", "Version"), tr("disponible", "available")),
                UpdateState::Failed(e) => e.clone(),
            };
            return vec![
                choice(
                    Setting::Interface,
                    tr("Interface", "Interface"),
                    tr("Auto : console en mode Jeu de SteamOS, bureau ailleurs.", "Auto: console in SteamOS Gaming Mode, desktop elsewhere.").into(),
                    vec!["Auto".into(), tr("Bureau", "Desktop").into(), tr("Portable", "Handheld").into(), tr("Salon", "Living room").into()],
                    iface,
                    vec![false; 4],
                ),
                choice(Setting::Language, tr("Langue", "Language"), tr("Auto : celle du système.", "Auto: the system's.").into(), vec!["Auto".into(), "Français".into(), "English".into()], lang, vec![false; 3]),
                toggle(Setting::KeepImage, tr("Garder la dernière image", "Keep the last image"), tr("L'accueil montre la dernière image de chaque session, pour tous les ordinateurs.", "The home screen shows the last image of each session, for every computer.").into(), self.store.app.keep_last_image),
                choice(
                    Setting::UpdateChannel,
                    tr("Versions proposées", "Offered versions"),
                    if flatpak() {
                        tr("Le Flatpak ne se met pas à jour seul : l'app prévient quand une version est publiée.", "The Flatpak does not update itself: the app tells you when a version is published.").into()
                    } else {
                        tr("Le paquet .deb ne se met pas à jour seul : l'app prévient quand une version est publiée.", "The .deb package does not update itself: the app tells you when a version is published.").into()
                    },
                    vec![tr("Stables", "Stable").into(), tr("Aussi les bêtas", "Betas too").into()],
                    self.store.app.beta as usize,
                    vec![false; 2],
                ),
                Row { setting: Setting::CheckUpdate, label: format!("KyberNow Link {}", self.version), hint: update, kind: RowKind::Action(tr("Chercher", "Check").into()), enabled: true, tag: None },
            ];
        }
        if section == Section::Gamepads {
            let n = self.pads.len();
            let hint = if n == 0 {
                tr("Aucune manette : branchez-la, ou associez-la en Bluetooth.", "No controller: plug it in, or pair it over Bluetooth.").to_string()
            } else {
                self.pads.iter().map(|p| format!("{}{} {}", tr("J", "P"), p.slot + 1, p.name)).collect::<Vec<_>>().join(" · ")
            };
            return vec![Row { setting: Setting::Pads, label: tr("Tester les manettes", "Test the controllers").into(), hint, kind: RowKind::Action(format!("{n}")), enabled: true, tag: None }];
        }
        if let Some(rows) = self.os_rows(section) {
            return rows;
        }
        let Some(h) = self.selected() else { return Vec::new() };
        let st = &h.settings;
        match section {
            Section::Profile => {
                let custom = st.custom();
                let mut options: Vec<String> = Profile::ALL.iter().map(|p| crate::ui::profile_label(*p).to_string()).collect();
                let mut disabled = vec![false; 3];
                // « Personnalisé » se lit, ne se choisit pas : il vient des réglages.
                if custom {
                    options.push(tr("Personnalisé", "Custom").into());
                    disabled.push(true);
                }
                let current = if custom { 3 } else { Profile::ALL.iter().position(|p| *p == st.profile).unwrap_or(0) };
                vec![choice(Setting::Profile, tr("Profil", "Profile"), profile_phrase((!custom).then_some(st.profile)).into(), options, current, disabled)]
            }
            Section::Picture => {
                let (_, dh) = m.display;
                let pi = m.pi.as_ref();
                let res = [0u32, 2160, 1440, 1080, 800, 720];
                let res_labels: Vec<String> = res.iter().map(|r| if *r == 0 { format!("{} · {}", tr("Comme cet écran", "Match this display"), Machine::height_label(dh)) } else { Machine::height_label(*r) }).collect();
                // Au-delà de ce que la sortie affiche (Pi 3 : 1080p), grisé.
                let res_off: Vec<bool> = res.iter().map(|r| *r > m.max_height()).collect();
                let height = if st.resolution == 0 { dh } else { st.resolution };
                let fps = [0u32, 30, 60, 90, 120, 144];
                let fps_labels: Vec<String> = fps.iter().map(|f| if *f == 0 { format!("{} · {} Hz", tr("Comme cet écran", "Match this display"), m.refresh) } else { format!("{f} Hz") }).collect();
                let fps_off: Vec<bool> = fps.iter().map(|f| *f != 0 && *f > m.max_hz(height)).collect();
                // Sur le Pi, 120 Hz n'existe qu'en 1080p : l'étiquette le dit.
                let fps_tags: Vec<Option<Tag>> = fps.iter().map(|f| if pi.is_some_and(|p| p.generation >= 4) && *f == 120 { Tag::note("1080p") } else { None }).collect();
                let codecs = ["auto", "hevc", "h264", "av1"];
                let hevc = self.codec_for(h) == "hevc";
                let codec_off: Vec<bool> = codecs.iter().map(|c| *c != "auto" && !m.decodes(c)).collect();
                let who = |hw: bool| if hw { Tag::hw(m.hw_name()) } else { Tag::cpu() };
                let codec_tags = vec![None, who(m.hw_hevc), who(m.hw_h264), if m.hw_av1 { Tag::hw(m.hw_name()) } else { None }];
                let display = ["auto", "vsync", "immediate", "vrr"];
                let vrr_hint = match (pi, m.vrr) {
                    (Some(_), _) => tr("La sortie HDMI du Pi n'a pas de fréquence variable. Immédiat : chaque image dès qu'elle est décodée.", "The Pi's HDMI output has no variable refresh. Immediate: each frame as soon as it is decoded."),
                    (None, Some(true)) => tr("Fréquence variable : l'écran suit le rythme des images. Immédiat : l'image peut se déchirer.", "Variable refresh: the screen follows the pace of the frames. Immediate: the picture may tear."),
                    (None, Some(false)) => tr("Cet écran n'annonce pas la fréquence variable. Immédiat : chaque image dès qu'elle est décodée.", "This screen does not announce variable refresh. Immediate: each frame as soon as it is decoded."),
                    (None, None) => tr("Fréquence variable : quand l'écran et le système la permettent.", "Variable refresh: when the screen and the system allow it."),
                };
                let br_labels: Vec<String> = BITRATES.iter().map(|b| if *b == 0 { "Auto".into() } else { format!("{b} Mbit/s") }).collect();
                let (res_hint, fps_hint, codec_hint) = picture_hints(m);
                let res_row = choice(Setting::Resolution, tr("Définition", "Resolution"), res_hint, res_labels, res.iter().position(|v| *v == st.resolution).unwrap_or(0), res_off);
                let mut fps_row = choice(Setting::Fps, tr("Fréquence", "Refresh rate"), fps_hint, fps_labels, fps.iter().position(|v| *v == st.fps).unwrap_or(0), fps_off);
                if let RowKind::Choice { tags, .. } = &mut fps_row.kind {
                    *tags = fps_tags;
                }
                let mut codec_row = choice(Setting::Codec, tr("Format vidéo", "Video format"), codec_hint, vec!["Auto".into(), "HEVC".into(), "H.264".into(), "AV1".into()], codecs.iter().position(|v| *v == st.codec).unwrap_or(0), codec_off);
                if let RowKind::Choice { tags, .. } = &mut codec_row.kind {
                    *tags = codec_tags;
                }
                let mut rows = vec![
                    res_row,
                    fps_row,
                    toggle(
                        Setting::VirtualDisplay,
                        tr("Écran virtuel", "Virtual display"),
                        tr(
                            "L'ordinateur crée un écran à la taille de cet appareil pendant la session ; les siens s'éteignent, puis reviennent à la fin.",
                            "The computer creates a display sized for this device during the session; its own displays switch off, then come back at the end.",
                        )
                        .into(),
                        st.virtual_display,
                    ),
                    codec_row,
                    Row {
                        setting: Setting::Yuv444,
                        label: tr("Texte net (4:4:4)", "Sharp text (4:4:4)").into(),
                        hint: yuv444_hint(m, hevc),
                        kind: RowKind::Toggle(st.yuv444 && hevc),
                        enabled: hevc,
                        tag: if hevc { who(m.hw_hevc444) } else { None },
                    },
                    choice(Setting::Bitrate, tr("Débit", "Bitrate"), tr("Auto : il suit la définition, la fréquence et le profil.", "Auto: it follows the resolution, the refresh rate and the profile.").into(), br_labels, BITRATES.iter().position(|v| *v == st.mbps).unwrap_or(0), vec![false; BITRATES.len()]),
                    choice(
                        Setting::Display,
                        tr("Affichage des images", "Frame display"),
                        vrr_hint.into(),
                        vec!["Auto".into(), tr("Synchronisé", "Synced").into(), tr("Immédiat", "Immediate").into(), tr("Fréquence variable", "Variable refresh").into()],
                        display.iter().position(|v| *v == st.display).unwrap_or(0),
                        vec![false, false, false, m.vrr == Some(false) || pi.is_some()],
                    ),
                ];
                // Écrans, sous Définition (multi-écrans : le bureau seulement).
                if self.multi_screens() {
                    rows.insert(1, self.displays_row(h));
                }
                if m.battery.is_some() {
                    rows.push(toggle(Setting::BatteryLight, tr("Alléger sur batterie", "Lighter on battery"), tr("Sur batterie : 60 images par seconde au plus, débit réduit.", "On battery: 60 frames per second at most, lower bitrate.").into(), st.battery_light));
                }
                rows
            }
            Section::Sound => {
                let audio = ["auto", "stereo", "5.1", "7.1", "computer"];
                let off: Vec<bool> = audio.iter().map(|a| (*a == "5.1" && m.audio_channels < 6) || (*a == "7.1" && m.audio_channels < 8)).collect();
                vec![choice(
                    Setting::Audio,
                    tr("Son", "Sound"),
                    format!("{} ({}, {} {}).", tr("Auto suit la sortie de cet appareil", "Auto follows this device's output"), m.audio_name, m.audio_channels, tr("voies", "channels")),
                    vec!["Auto".into(), tr("Stéréo", "Stereo").into(), "5.1".into(), "7.1".into(), tr("Sur l'ordinateur", "On the computer").into()],
                    audio.iter().position(|v| *v == st.audio).unwrap_or(0),
                    off,
                )]
            }
            Section::Controls => vec![
                choice(
                    Setting::Xbox360,
                    tr("Manette vue par l'ordinateur", "Controller seen by the computer"),
                    tr("Xbox 360 : tous les jeux la reconnaissent. Fidèle : le modèle branché.", "Xbox 360: every game recognises it. As is: the model you plugged in.").into(),
                    vec!["Xbox 360".into(), tr("Fidèle", "As is").into()],
                    (!st.xbox360) as usize,
                    vec![false; 2],
                ),
                toggle(Setting::Combo, "L1 + R1 + Start + Select", tr("Tenus ensemble, ils ouvrent le menu de session.", "Held together, they open the session menu.").into(), st.combo),
                choice(
                    Setting::Mouse,
                    tr("Souris", "Mouse"),
                    tr("Auto : souris des jeux quand un jeu en plein écran cache le pointeur de l'ordinateur.", "Auto: game mouse when a full-screen game hides the computer's pointer.").into(),
                    vec!["Auto".into(), tr("Souris du bureau", "Desktop mouse").into(), tr("Souris des jeux", "Game mouse").into()],
                    ["auto", "desktop", "game"].iter().position(|v| *v == st.mouse).unwrap_or(0),
                    vec![false; 3],
                ),
                toggle(Setting::Clipboard, tr("Presse-papiers partagé", "Shared clipboard"), tr("Le texte copié passe dans les deux sens.", "Copied text goes both ways.").into(), st.clipboard),
            ],
            Section::Network => vec![
                choice(
                    Setting::Reliable,
                    tr("Transport de l'image", "Picture transport"),
                    tr("Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai.", "Fast suits your home network and gaming. Reliable resends what gets lost, at the cost of a little delay.").into(),
                    vec![tr("Rapide", "Fast").into(), tr("Fiable", "Reliable").into()],
                    st.reliable as usize,
                    vec![false; 2],
                ),
                toggle(Setting::Reconnect, tr("Reconnexion automatique", "Reconnect automatically"), tr("Quand le réseau coupe pendant une session.", "When the network drops during a session.").into(), st.reconnect),
                {
                    // La carte apprise d'Admin (Ethernet d'abord), sinon la MAC saisie.
                    let help = tr("Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système).", "Turns the computer on remotely when it is asleep. It must also be allowed on the computer (Admin › System).");
                    let (value, hint) = match (h.wake.first(), &h.mac) {
                        (Some(card), _) => (tr("Prêt", "Ready"), format!("{}. {help}", card.mac)),
                        (None, Some(m)) => (tr("Prêt", "Ready"), format!("{m} ({}). {help}", tr("saisie", "entered by hand"))),
                        (None, None) => ("", format!("{} {help}", tr("Pas encore d'adresse : connectez-vous une fois à cet ordinateur.", "No address yet: connect to this computer once."))),
                    };
                    info(tr("Réveil par le réseau", "Wake on LAN"), hint, value.into())
                },
                Row {
                    setting: Setting::WakeMac,
                    label: tr("Adresse MAC à la main", "MAC address by hand").into(),
                    hint: tr("Réglage avancé, si la Station ne donne pas la carte réseau de l'ordinateur.", "Advanced setting, if the Station does not give the computer's network card.").into(),
                    kind: RowKind::Action(match &h.mac {
                        Some(m) => m.clone(),
                        None => tr("Saisir", "Enter").into(),
                    }),
                    enabled: true,
                    tag: None,
                },
            ],
            Section::Statistics => {
                let dash = || "—".to_string();
                vec![
                    info(tr("Système", "System"), String::new(), h.os.clone().unwrap_or_else(dash)),
                    info(tr("Carte graphique", "Graphics card"), String::new(), h.gpu.clone().unwrap_or_else(dash)),
                    info(tr("Adresse", "Address"), String::new(), format!("{}:{}", bracketed(&h.address), h.port)),
                    info(tr("Empreinte", "Fingerprint"), String::new(), h.fingerprint.as_deref().map(short_fp).unwrap_or_else(dash)),
                ]
            }
            Section::Station => {
                let ready = self.station_ready(&h.id);
                let mut rows = Vec::new();
                // Admin : un lien au bureau ; à la manette et sur KyberNow Link OS, un QR code (ui/station.rs).
                if !self.admin_by_qr() {
                    rows.push(button(
                        Setting::StationAdmin,
                        tr("Ouvrir Admin", "Open Admin"),
                        tr(
                            "Admin s'ouvre dans votre navigateur et demande son mot de passe. Le navigateur peut signaler le certificat : la Station signe elle-même sa connexion, c'est normal.",
                            "Admin opens in your browser and asks for its password. The browser may warn about the certificate: the Station signs its own connection, this is expected.",
                        )
                        .into(),
                        ButtonStyle::Ghost,
                        true,
                    ));
                }
                rows.push(button(Setting::StationRestart, tr("Redémarrer la Station", "Restart the Station"), tr("Environ 30 secondes. Une session en cours sera coupée.", "About 30 seconds. A session in progress will be cut off.").into(), ButtonStyle::Ghost, ready));
                if let Some(v) = self.station(&h.id).and_then(|s| s.installable_version()) {
                    rows.push(button(
                        Setting::StationUpdate,
                        tr("Mettre à jour la Station", "Update the Station"),
                        format!("{} {v}. {}", tr("Version", "Version"), tr("Environ une minute. Une session en cours sera coupée.", "About a minute. A session in progress will be cut off.")),
                        ButtonStyle::Primary,
                        ready,
                    ));
                }
                // L'ajout par code donne à cet appareil son propre compte ; Admin 0.5.0 le confirme (`own_device`).
                let own = self.live.get(&h.id).and_then(|l| l.own_device).unwrap_or(true);
                let account_hint = if own { tr("Accès propre à cet appareil", "This device's own access") } else { "" };
                rows.push(info(tr("Compte", "Account"), account_hint.into(), h.user.clone()));
                rows.push(button(Setting::Remove, tr("Retirer cet ordinateur", "Remove this computer"), tr("Cet appareil l'oublie. Son accès se retire dans Admin, Appareils Link.", "This device forgets it. Its access is removed in Admin, Link devices.").into(), ButtonStyle::Danger, true));
                rows
            }
            _ => Vec::new(),
        }
    }

    /// La ligne « Écrans » de la rubrique Image (docs/link-ui.md) : Auto, Cet
    /// écran, Tous les écrans ; inactive, sur « Cet écran », avec une définition
    /// sur mesure ou sans écran virtuel (`screens_plan`, du cœur). L'aide : celle
    /// d'Auto quand la ligne est active, celle de la définition sur mesure quand
    /// elle l'est ; sans écran virtuel, la ligne grisée n'a pas d'aide (comme sur
    /// Mac : elle se suffit, l'écran virtuel a sa propre ligne).
    fn displays_row(&self, h: &Host) -> Row {
        let plan = &self.screens_plan;
        let hint = if h.settings.resolution != 0 {
            tr("Avec une définition sur mesure, la session prend un seul écran.", "With a custom resolution, the session takes a single display.")
        } else if plan.enabled {
            tr(
                "Auto : tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et en Personnalisé. Chaque écran reçoit un écran de l'ordinateur, placé de la même façon.",
                "Auto: all of this device's displays in Desktop and Max, one in Game and Custom. Each display gets a display on the computer, laid out the same way.",
            )
        } else {
            ""
        };
        Row {
            setting: Setting::Displays,
            label: tr("Écrans", "Displays").into(),
            hint: hint.into(),
            kind: RowKind::Choice {
                options: vec!["Auto".into(), tr("Cet écran", "This display").into(), tr("Tous les écrans", "All displays").into()],
                current: plan.shown.min(2),
                // Inactive : la valeur montrée, « Cet écran », ne se change pas ; les autres sont barrées.
                disabled: (0..3).map(|i| !plan.enabled && i != plan.shown.min(2)).collect(),
                tags: Vec::new(),
            },
            enabled: plan.enabled,
            tag: None,
        }
    }

    /// Admin s'ouvre par un QR code (manette, KyberNow Link OS) plutôt que
    /// dans un navigateur de cet appareil : en mode Jeu de SteamOS
    /// (gamescope), un navigateur s'ouvre mal ; Link OS n'en a pas.
    pub fn admin_by_qr(&self) -> bool {
        self.layout.console() || self.is_os()
    }

    /// Les lignes des réglages de l'appareil (KyberNow Link OS) ; `None` pour
    /// les autres rubriques. La mise à jour a son propre panneau (`ui/device.rs`).
    fn os_rows(&self, section: Section) -> Option<Vec<Row>> {
        let choice = |setting, label: &str, hint: String, options: Vec<String>, current: usize, disabled: Vec<bool>| Row { setting, label: label.to_string(), hint, kind: RowKind::Choice { options, current, disabled, tags: Vec::new() }, enabled: true, tag: None };
        let toggle = |setting, label: &str, hint: String, on: bool| Row { setting, label: label.to_string(), hint, kind: RowKind::Toggle(on), enabled: true, tag: None };
        let action = |setting, label: &str, hint: String, verb: &str| Row { setting, label: label.to_string(), hint, kind: RowKind::Action(verb.to_string()), enabled: true, tag: None };
        let info = |label: &str, hint: String, value: String| Row { setting: Setting::OsInfo, label: label.to_string(), hint, kind: RowKind::Info(value), enabled: true, tag: None };
        let a = &self.store.app;
        let m = &self.machine;
        let rows = match section {
            Section::OsUsage => vec![choice(
                Setting::OsUsage,
                tr("Usage de ce Raspberry Pi", "What this Raspberry Pi is for"),
                tr("Salon : au téléviseur, à la manette · Bureau : écran, clavier et souris · Start tenu trois secondes bascule", "Living room: on a TV, with a controller · Desktop: screen, keyboard and mouse · Start held three seconds switches").into(),
                vec![tr("Salon", "Living room").into(), tr("Bureau", "Desktop").into()],
                (a.interface == "desktop") as usize,
                vec![false; 2],
            )],
            Section::OsNetwork => {
                let state = match self.net.kind {
                    NetKind::Ethernet => match self.net.link_mbps {
                        Some(mb) => format!("Ethernet · {}", crate::ui::gbit_label(mb)),
                        None => "Ethernet".into(),
                    },
                    NetKind::Wifi => match &self.net.band {
                        Some(b) => format!("Wi-Fi · {b}"),
                        None => "Wi-Fi".into(),
                    },
                    NetKind::Unknown => tr("Pas de réseau", "No network").into(),
                };
                vec![
                    info(tr("Liaison", "Link"), tr("Ethernet dès qu'un câble est branché ; le Wi-Fi sinon", "Ethernet as soon as a cable is plugged in; Wi-Fi otherwise").into(), state),
                    action(Setting::OsWifi, tr("Réseau Wi-Fi", "Wi-Fi network"), tr("La liste des réseaux, le mot de passe à la manette", "The list of networks, the password with the controller").into(), tr("Choisir", "Choose")),
                ]
            }
            Section::OsControllers => {
                let mut v: Vec<Row> = self.pads.iter().map(|p| info(&format!("{} {}", tr("Joueur", "Player"), p.slot + 1), p.battery.clone().unwrap_or_default(), p.name.clone())).collect();
                if v.is_empty() {
                    v.push(info(tr("Aucune manette", "No controller"), tr("Branchez-la en USB, ou associez-la", "Plug it in over USB, or pair it").into(), String::new()));
                }
                v.push(action(Setting::OsBtPair, tr("Associer une manette", "Pair a controller"), tr("Bluetooth à l'écoute : la manette en mode association est acceptée sans code", "Bluetooth listening: a controller in pairing mode is accepted without a code").into(), tr("Associer", "Pair")));
                v.push(action(Setting::Pads, tr("Tester les manettes", "Test the controllers"), tr("Boutons, sticks, vibrations", "Buttons, sticks, rumble").into(), &format!("{}", self.pads.len())));
                v
            }
            Section::OsScreen => {
                let modes = ["auto", "2160p60", "1080p120", "1080p60"];
                let labels = vec!["Auto".into(), "4K · 60 Hz".into(), "1080p · 120 Hz".into(), "1080p · 60 Hz".into()];
                let off = vec![false, m.max_height() < 2160, m.max_hz(1080) < 120, false];
                vec![
                    choice(Setting::OsHdmi, tr("Sortie HDMI", "HDMI output"), tr("Au prochain démarrage · Auto : le mode que le téléviseur préfère", "At the next boot · Auto: the mode the TV prefers").into(), labels, modes.iter().position(|v| *v == a.os_hdmi).unwrap_or(0), off),
                    toggle(Setting::OsCec, "HDMI-CEC", tr("Nom annoncé, télécommande, veille et réveil du téléviseur avec l'appareil", "Name announced, remote, TV standby and wake with the device").into(), a.os_cec),
                    info(tr("Écran", "Screen"), m.display_name.clone(), format!("{} × {} · {} Hz", m.display.0, m.display.1, m.refresh)),
                ]
            }
            Section::OsSound => vec![
                info(tr("Sortie", "Output"), tr("La première prise HDMI de la puce", "The chip's first HDMI socket").into(), format!("HDMI · {} {}", m.audio_channels, tr("voies", "channels"))),
                info(tr("Par ordinateur", "Per computer"), tr("Stéréo, 5.1 ou 7.1 : Réglages de l'ordinateur › Son", "Stereo, 5.1 or 7.1: Computer settings › Sound").into(), String::new()),
            ],
            Section::OsUpdate => Vec::new(),
            Section::OsLanguage => {
                let lang = ["auto", "fr", "en"].iter().position(|v| *v == a.language).unwrap_or(0);
                vec![choice(Setting::Language, tr("Langue", "Language"), tr("Auto : celle du système", "Auto: the system's").into(), vec!["Auto".into(), "Français".into(), "English".into()], lang, vec![false; 3])]
            }
            Section::OsAdvanced => {
                let o = self.os.clone().unwrap_or_default();
                vec![
                    toggle(Setting::OsSsh, "SSH", tr("Compte kybernow, par clé : kybernow-ssh.pub déposé sur la carte", "kybernow account, by key: kybernow-ssh.pub dropped on the card").into(), o.ssh),
                    action(Setting::OsReboot, tr("Redémarrer", "Restart"), tr("Le Raspberry Pi redémarre sur le système en cours", "The Raspberry Pi restarts on the current system").into(), tr("Redémarrer", "Restart")),
                    action(Setting::OsPoweroff, tr("Éteindre", "Shut down"), tr("Arrêt complet ; le bouton du Pi 5 rallume", "Full shutdown; the Pi 5's button turns it back on").into(), tr("Éteindre", "Shut down")),
                    info("KyberNow Link OS", format!("{} · {} {}", o.built, tr("emplacement", "slot"), o.slot), o.version),
                ]
            }
            _ => return None,
        };
        Some(rows)
    }

    /// Change un réglage (gauche / droite, ou OK pour la valeur suivante).
    fn adjust(&mut self, s: Setting, dir: i32) -> Vec<Action> {
        let secs: &[Section] = if self.screen == Screen::Device { &OS_SECTIONS } else { self.sections() };
        let row = secs.iter().flat_map(|sec| self.rows(*sec)).find(|r| r.setting == s && !matches!(r.kind, RowKind::Info(_)));
        if let Some(row) = row
            && let RowKind::Choice { options, current, disabled, .. } = &row.kind
        {
            // La valeur suivante qui n'est pas indisponible.
            let n = options.len();
            let mut i = *current;
            for _ in 0..n {
                i = cycle(i, n, dir);
                if !disabled.get(i).copied().unwrap_or(false) {
                    break;
                }
            }
            return self.choose(s, i);
        }
        self.choose(s, usize::MAX)
    }

    /// Choisit la valeur `i` d'un réglage (`usize::MAX` : basculer, ou l'action).
    fn choose(&mut self, s: Setting, i: usize) -> Vec<Action> {
        match s {
            Setting::Pads => {
                self.back_to = Screen::Settings;
                self.go(Screen::Gamepads);
                return Vec::new();
            }
            Setting::CheckUpdate => return vec![Action::CheckUpdate],
            Setting::UpdateChannel => {
                self.store.app.beta = if i == usize::MAX { !self.store.app.beta } else { i == 1 };
                return vec![Action::Save, Action::CheckUpdate];
            }
            Setting::Language => {
                let order = ["auto", "fr", "en"];
                self.store.app.language = order[i.min(2)].into();
                return vec![Action::Language, Action::Save];
            }
            Setting::Interface => {
                let order = ["auto", "desktop", "handheld", "tv"];
                self.store.app.interface = order[i.min(3)].into();
                return vec![Action::Interface, Action::Save];
            }
            Setting::KeepImage => {
                self.store.app.keep_last_image = !self.store.app.keep_last_image;
                return vec![Action::Save];
            }
            // KyberNow Link OS.
            Setting::OsUsage => {
                let desktop = if i == usize::MAX { self.store.app.interface != "desktop" } else { i == 1 };
                self.store.app.interface = if desktop { "desktop".into() } else { "tv".into() };
                return vec![Action::Save, Action::Interface];
            }
            Setting::OsHdmi => {
                let modes = ["auto", "2160p60", "1080p120", "1080p60"];
                let cur = modes.iter().position(|m| *m == self.store.app.os_hdmi).unwrap_or(0);
                let i = if i == usize::MAX { (cur + 1) % modes.len() } else { i.min(modes.len() - 1) };
                self.store.app.os_hdmi = modes[i].into();
                return vec![Action::Save, Action::Os(OsAction::Hdmi(modes[i].into()))];
            }
            Setting::OsCec => {
                self.store.app.os_cec = !self.store.app.os_cec;
                return vec![Action::Save, Action::Os(OsAction::Cec(self.store.app.os_cec))];
            }
            Setting::OsAutoUpdate => {
                self.store.app.os_auto_update = !self.store.app.os_auto_update;
                return vec![Action::Save];
            }
            Setting::OsSsh => {
                let on = !self.os.as_ref().is_some_and(|o| o.ssh);
                if let Some(o) = self.os.as_mut() {
                    o.ssh = on;
                }
                return vec![Action::Os(OsAction::Ssh(on))];
            }
            Setting::OsReboot => return vec![Action::Os(OsAction::Reboot)],
            Setting::OsPoweroff => return vec![Action::Os(OsAction::Poweroff)],
            Setting::OsStandby => {
                self.standby = true;
                self.go(Screen::Standby);
                return vec![Action::Os(OsAction::Standby(true))];
            }
            Setting::OsUpdateApply => {
                if self.os_update.ready.is_some() {
                    self.os_update.applying = true;
                    return vec![Action::Os(OsAction::UpdateApply)];
                }
                if matches!(self.update, UpdateState::Available { .. }) && self.os_update.downloading.is_none() {
                    self.os_update.error = None;
                    self.os_update.downloading = Some((0, 0));
                    return vec![Action::Os(OsAction::UpdateDownload)];
                }
                return Vec::new();
            }
            Setting::OsUpdateLater => {
                // Rien ne s'installe sans « Redémarrer sur la … » (Réglages de l'appareil › Mise à jour).
                self.toast(tr("La mise à jour attend ici : rien ne s'installe sans vous", "The update waits here: nothing installs without you"));
                return Vec::new();
            }
            Setting::OsRollback => return vec![Action::Os(OsAction::Rollback)],
            Setting::OsWifi => {
                self.open_setup_step(SetupStep::Network);
                self.setup.wifi_scanning = true;
                return vec![Action::Os(OsAction::WifiScan)];
            }
            Setting::OsBtPair => {
                self.open_setup_step(SetupStep::Controller);
                return vec![Action::Os(OsAction::BtListen(true))];
            }
            Setting::OsInfo => return Vec::new(),
            _ => {}
        }
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
        if s == Setting::WakeMac {
            self.add.mac = self.store.host(&id).and_then(|h| h.mac.clone()).unwrap_or_default();
            self.add.step = AddStep::Mac;
            self.add.message = None;
            self.add.busy = false;
            self.go(Screen::Add);
            return vec![Action::OpenKeyboard(None)];
        }
        match s {
            Setting::Remove => {
                let name = self.store.host(&id).map(|h| h.name.clone()).unwrap_or_default();
                self.open_confirm(Confirm::Remove { host: id, name });
                return Vec::new();
            }
            Setting::StationAdmin => {
                let Some(h) = self.store.host(&id) else { return Vec::new() };
                return vec![Action::OpenUrl(admin_url(&h.address, h.port))];
            }
            Setting::StationRestart => {
                return if self.station_ready(&id) { self.start_station(id, StationKind::Restart, false) } else { Vec::new() };
            }
            Setting::StationUpdate => {
                let installable = self.station(&id).and_then(|s| s.installable_version()).is_some();
                return if installable && self.station_ready(&id) { self.start_station(id, StationKind::Update, false) } else { Vec::new() };
            }
            Setting::Info => return Vec::new(),
            Setting::Profile => {
                // Valeur suivante (‹ ›, A) ; « Personnalisé » (la quatrième) ne se choisit pas.
                if i < Profile::ALL.len() {
                    self.set_profile(&id, Profile::ALL[i]);
                    return vec![Action::Save];
                }
                return Vec::new();
            }
            _ => {}
        }
        let enabled = self.screens_plan.enabled;
        let Some(h) = self.store.host_mut(&id) else { return Vec::new() };
        let st = &mut h.settings;
        let pick = |opts: &[&str], i: usize| opts[i.min(opts.len() - 1)].to_string();
        match s {
            Setting::Resolution => st.resolution = [0u32, 2160, 1440, 1080, 800, 720][i.min(5)],
            // Inactif (définition sur mesure, sans écran virtuel) : rien à changer.
            // Ne rend pas le profil « Personnalisé » : `custom()` ne le regarde pas.
            Setting::Displays if enabled => st.displays = pick(&["auto", "this", "all"], i),
            Setting::Fps => st.fps = [0u32, 30, 60, 90, 120, 144][i.min(5)],
            Setting::Codec => st.codec = pick(&["auto", "hevc", "h264", "av1"], i),
            Setting::Display => st.display = pick(&["auto", "vsync", "immediate", "vrr"], i),
            Setting::Bitrate => st.mbps = BITRATES[i.min(BITRATES.len() - 1)],
            Setting::Audio => st.audio = pick(&["auto", "stereo", "5.1", "7.1", "computer"], i),
            Setting::Mouse => st.mouse = pick(&["auto", "desktop", "game"], i),
            Setting::Xbox360 => st.xbox360 = if i == usize::MAX { !st.xbox360 } else { i == 0 },
            Setting::Reliable => st.reliable = if i == usize::MAX { !st.reliable } else { i == 1 },
            Setting::VirtualDisplay => st.virtual_display = !st.virtual_display,
            Setting::BatteryLight => st.battery_light = !st.battery_light,
            Setting::Combo => st.combo = !st.combo,
            Setting::Clipboard => st.clipboard = !st.clipboard,
            Setting::Reconnect => st.reconnect = !st.reconnect,
            Setting::Yuv444 => st.yuv444 = !st.yuv444,
            _ => {}
        }
        vec![Action::Save]
    }

    // MARK: - Session

    /// Le mode que la session demandera : définition et fréquence (« comme
    /// cet écran », ou la hauteur choisie, à la largeur de cet écran).
    pub fn session_mode(&self, h: &Host) -> (u32, u32, u32) {
        self.mode_for(h, self.machine.display, self.machine.refresh)
    }

    /// Le mode pour une surface de `display` pixels rafraîchie à `refresh` Hz : l'écran de
    /// l'appareil (`session_mode`), ou la fenêtre d'une session de plus (plusieurs ordinateurs :
    /// l'écran de l'ordinateur prend la taille de sa fenêtre, `screens::extra_session_window`).
    /// « Comme cet écran » : cette surface ; une hauteur choisie : à sa largeur.
    pub fn mode_for(&self, h: &Host, display: (u32, u32), refresh: u32) -> (u32, u32, u32) {
        let (pw, ph) = display;
        let (pw, ph) = (pw.max(640), ph.max(480));
        let height = if h.settings.resolution == 0 { ph } else { h.settings.resolution };
        let width = ((height as u64 * pw as u64 / ph as u64) as u32) & !1;
        let mut fps = if h.settings.fps == 0 { refresh.max(30) } else { h.settings.fps };
        if h.settings.battery_light && self.machine.on_battery() {
            fps = fps.min(60);
        }
        (width, height, fps)
    }

    /// Le format choisi : celui demandé s'il est décodé (AV1 : si l'ordinateur
    /// l'encode aussi), sinon HEVC, que la carte ou le processeur décode.
    pub fn codec_for(&self, h: &Host) -> &'static str {
        let m = &self.machine;
        let hevc_ok = m.decodes("hevc");
        match h.settings.codec.as_str() {
            "h264" => "h264",
            "av1" if m.decodes("av1") && h.av1 => "av1",
            "hevc" if hevc_ok => "hevc",
            _ if hevc_ok => "hevc",
            _ => "h264",
        }
    }
}

/// Les aides des lignes Écran, Fréquence et Format, selon la machine : sur le
/// Pi, ce que sa sortie HDMI et ses décodeurs permettent ; ailleurs, VA-API.
fn picture_hints(m: &Machine) -> (String, String, String) {
    let Some(p) = &m.pi else {
        let screen = tr("La définition de l'image que l'ordinateur envoie.", "The resolution of the picture the computer sends.").to_string();
        let fps = tr("Les images par seconde demandées à l'ordinateur.", "The frames per second asked of the computer.").to_string();
        let codec = if m.vaapi && m.hw_any() {
            format!("{} {}. {}", tr("La carte graphique de cet appareil décode :", "This device's graphics card decodes:"), decoders_label(m), tr("Sans elle, le processeur décode H.264 et HEVC.", "Without it, the processor decodes H.264 and HEVC."))
        } else {
            tr("La carte graphique de cet appareil ne décode rien : le processeur décode H.264 et HEVC.", "This device's graphics card decodes nothing: the processor decodes H.264 and HEVC.").into()
        };
        return (screen, fps, codec);
    };
    let screen = match p.generation {
        5 => tr("4K seulement en HEVC, décodé par le Pi. Au-delà de 1080p, le H.264 passe par le processeur et saccade.", "4K in HEVC only, decoded by the Pi. Above 1080p, H.264 goes through the CPU and stutters.").to_string(),
        4 => tr("4K en HEVC, 1080p en H.264 : les deux par le décodeur du Pi.", "4K in HEVC, 1080p in H.264: both by the Pi's decoder.").to_string(),
        _ => tr("1080p au plus (HDMI 1.4), en H.264 par le décodeur du Pi.", "1080p at most (HDMI 1.4), in H.264 by the Pi's decoder.").to_string(),
    };
    let fps = if p.generation >= 4 {
        if m.max_hz_1080p >= 120 {
            format!(
                "{} {} {}",
                tr("L'écran annonce 1080p à", "The screen announces 1080p at"),
                format!("{} Hz ;", m.max_hz_1080p),
                tr("le Pi ne sort pas la 4K au-delà de 60 Hz. Pas de fréquence variable sur cette sortie.", "the Pi does not output 4K above 60 Hz. No variable refresh on this output.")
            )
        } else {
            tr("Le Pi sort la 4K à 60 Hz au plus, et le 1080p à 120 Hz si l'écran l'annonce. Pas de fréquence variable sur cette sortie.", "The Pi outputs 4K at 60 Hz at most, and 1080p at 120 Hz when the screen announces it. No variable refresh on this output.").to_string()
        }
    } else {
        tr("60 Hz au plus sur la sortie HDMI de ce Pi ; 30 Hz en H.264 décodé par la puce.", "60 Hz at most on this Pi's HDMI output; 30 Hz for H.264 decoded by the chip.").to_string()
    };
    let codec = match p.generation {
        5 => tr("HEVC : jusqu'à 4K à 60 Hz, en 4:2:0. H.264 : 1080p à 60 Hz au plus, par les quatre cœurs. AV1 : rien ne le décode ici.", "HEVC: up to 4K at 60 Hz, 4:2:0. H.264: 1080p at 60 Hz at most, on the four cores. AV1: nothing decodes it here.").to_string(),
        4 => tr("HEVC jusqu'à 4K à 60 Hz et H.264 jusqu'à 1080p à 60 Hz, tous deux par le décodeur du Pi. AV1 : rien ne le décode ici.", "HEVC up to 4K at 60 Hz and H.264 up to 1080p at 60 Hz, both by the Pi's decoder. AV1: nothing decodes it here.").to_string(),
        _ => tr("H.264 par le décodeur du Pi, 1080p à 30 Hz au plus. HEVC par le processeur : 720p. AV1 : non.", "H.264 by the Pi's decoder, 1080p at 30 Hz at most. HEVC on the CPU: 720p. AV1: no.").to_string(),
    };
    (screen, fps, codec)
}

/// Ce que dit la ligne 4:4:4 : ce qu'elle apporte, et sur le Pi ce qu'il en coûte.
fn yuv444_hint(m: &Machine, hevc: bool) -> String {
    let base = tr("Couleurs complètes : le texte reste net, au prix d'un débit plus élevé.", "Full colour: text stays sharp, at the cost of a higher bitrate.");
    if !hevc {
        format!("{base} {}", tr("En HEVC seulement.", "HEVC only."))
    } else if m.pi.is_some() {
        format!("{base} {}", tr("Sur ce Pi, le processeur décode : 1080p au plus.", "On this Pi, the processor decodes: 1080p at most."))
    } else {
        base.into()
    }
}

/// « La version 0.5.1 s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe. »
pub fn requires_setup_text(v: &str) -> String {
    format!("{} {v} {}", tr("La version", "Version"), tr("s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe.", "installs on the computer, with KyberNow-Station-Setup.exe."))
}

/// Une Station 0.4.9 ou plus ancienne : ni redémarrage ni mise à jour depuis un appareil.
pub fn too_old_text() -> &'static str {
    tr("Pour redémarrer ou mettre à jour la Station depuis cet appareil, mettez-la d'abord à jour depuis Admin.", "To restart or update the Station from this device, first update it from Admin.")
}

/// La phrase d'un profil (`None` : Personnalisé).
pub fn profile_phrase(p: Option<Profile>) -> &'static str {
    match p {
        Some(Profile::Jeu) => tr("La réactivité d'abord, pour jouer.", "Responsiveness first, for gaming."),
        Some(Profile::Bureau) => tr("Un texte net, pour travailler.", "Sharp text, for work."),
        Some(Profile::Max) => tr("La meilleure image que cet appareil affiche.", "The best picture this device can show."),
        None => tr("Vos propres réglages.", "Your own settings."),
    }
}

/// Une adresse IPv6 entre crochets (« [fe80::1] »), les autres telles quelles.
pub fn bracketed(address: &str) -> String {
    let a = address.trim_start_matches('[').trim_end_matches(']');
    if a.contains(':') { format!("[{a}]") } else { a.to_string() }
}

/// L'adresse d'Admin à ouvrir dans un navigateur ou à mettre dans le QR code :
/// celle du cœur avec l'app ; sans lui (captures), la même, recalculée.
pub fn admin_url(address: &str, port: u16) -> String {
    #[cfg(feature = "device")]
    {
        kybernow_core::console::admin_url(address, port)
    }
    #[cfg(not(feature = "device"))]
    {
        format!("https://{}:{port}/", bracketed(address))
    }
}

fn decoders_label(m: &Machine) -> String {
    let mut v = Vec::new();
    if m.hw_h264 {
        v.push("H.264");
    }
    if m.hw_hevc {
        v.push("HEVC");
    }
    if m.hw_av1 {
        v.push("AV1");
    }
    if v.is_empty() { tr("aucun", "none").into() } else { v.join(", ") }
}

/// L'app tourne dans le bac à sable Flatpak (sinon : paquet .deb ou binaire).
pub fn flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// « sha256:7C:3A:91:E0:… » → « 7c3a · 91e0 · 4b2d · a81f ».
pub fn short_fp(fp: &str) -> String {
    let hex: String = fp.trim_start_matches("sha256:").chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_lowercase();
    let groups: Vec<&str> = (0..4).filter_map(|i| hex.get(i * 4..i * 4 + 4)).collect();
    groups.join(" · ")
}

fn cycle(i: usize, n: usize, dir: i32) -> usize {
    ((i as i32 + dir).rem_euclid(n as i32)) as usize
}

/// `192.168.1.20:24811` → adresse et port.
pub fn parse_address(s: &str) -> Option<(String, u16)> {
    let (a, p) = s.rsplit_once(':')?;
    let port: u16 = p.parse().ok()?;
    let a = a.trim().trim_start_matches('[').trim_end_matches(']');
    if a.is_empty() || port == 0 {
        return None;
    }
    Some((a.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address() {
        assert_eq!(parse_address("192.168.1.20:24811"), Some(("192.168.1.20".into(), 24811)));
        assert_eq!(parse_address("192.168.1.20"), None);
        assert_eq!(parse_address(":24811"), None);
    }

    #[test]
    fn fingerprint() {
        assert_eq!(short_fp("sha256:7C:3A:91:E0:4B:2D:A8:1F:00"), "7c3a · 91e0 · 4b2d · a81f");
    }

    #[test]
    fn spatial_navigation() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.screen = Screen::Home;
        app.hits = vec![
            (Target::Connect, Rect::new(100.0, 100.0, 200.0, 80.0)),
            (Target::Chip(Profile::Jeu), Rect::new(340.0, 110.0, 100.0, 60.0)),
            (Target::AddCard, Rect::new(100.0, 600.0, 200.0, 200.0)),
        ];
        app.focus = Target::Chip(Profile::Jeu);
        app.nav(Nav::Left);
        assert_eq!(app.focus, Target::Connect);
        app.nav(Nav::Down);
        assert_eq!(app.focus, Target::AddCard);
    }

    fn host() -> Host {
        Host {
            id: "a".into(),
            name: "A".into(),
            address: "x".into(),
            port: 1,
            fingerprint: None,
            user: String::new(),
            password: String::new(),
            os: None,
            gpu: None,
            av1: true,
            mac: None,
            wake: Vec::new(),
            settings: Default::default(),
            last: None,
        }
    }

    #[test]
    fn mode_follows_screen() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.machine.display = (1280, 800);
        app.machine.refresh = 90;
        let mut h = host();
        assert_eq!(app.session_mode(&h), (1280, 800, 90));
        app.machine.battery = Some((40, false));
        assert_eq!(app.session_mode(&h), (1280, 800, 60));
        h.settings.resolution = 1080;
        h.settings.fps = 120;
        h.settings.battery_light = false;
        assert_eq!(app.session_mode(&h), (1728, 1080, 120));
    }

    #[test]
    fn codec_follows_decoders() {
        let mut app = App::new(Store::default(), Layout::Desktop);
        let mut h = host();
        app.machine.hw_av1 = false;
        h.settings.codec = "av1".into();
        assert_eq!(app.codec_for(&h), "hevc");
        app.machine.hw_av1 = true;
        assert_eq!(app.codec_for(&h), "av1");
        // Sans HEVC par la carte, le processeur le décode.
        app.machine.hw_hevc = false;
        h.settings.codec = "auto".into();
        assert_eq!(app.codec_for(&h), "hevc");
        h.settings.codec = "h264".into();
        assert_eq!(app.codec_for(&h), "h264");
    }

    #[test]
    fn wake_mac() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(host());
        app.open_settings(false, Screen::Home);
        assert_eq!(app.choose(Setting::WakeMac, usize::MAX), vec![Action::OpenKeyboard(None)]);
        assert!(app.typing_mac());
        // Minuscules, tirets, caractères hors hexadécimal : remis en forme.
        for c in "3c-7c-3fz12ab".chars() {
            app.nav(Nav::Char(c));
        }
        app.activate(Target::KeyOk);
        assert!(app.add.message.is_some(), "dix chiffres : refusé");
        for c in "cd".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!(app.activate(Target::KeyOk), vec![Action::Save]);
        assert_eq!(app.selected().unwrap().mac.as_deref(), Some("3C:7C:3F:12:AB:CD"));
        assert_eq!((app.screen, app.focus), (Screen::Settings, Target::Setting(Setting::WakeMac)));
        // Vide : plus de réveil.
        app.choose(Setting::WakeMac, usize::MAX);
        while !app.add.mac.is_empty() {
            app.nav(Nav::Erase);
        }
        app.activate(Target::KeyOk);
        assert_eq!(app.selected().unwrap().mac, None);
    }

    #[test]
    fn settings_choices_skip_disabled() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(host());
        app.machine.hw_av1 = false;
        app.settings_global = false;
        app.section = 1; // Image
        app.adjust(Setting::Codec, 1); // auto → hevc
        assert_eq!(app.selected().unwrap().settings.codec, "hevc");
        app.adjust(Setting::Codec, 1); // → h264
        app.adjust(Setting::Codec, 1); // av1 indisponible → auto
        assert_eq!(app.selected().unwrap().settings.codec, "auto");
    }

    fn online(app: &mut App, id: &str, station: Option<StationLive>) {
        let l = app.live.entry(id.into()).or_default();
        l.online = Online::Online;
        l.station = station;
    }

    #[test]
    fn remove_asks_first() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(host());
        app.open_host_section(Section::Station, Screen::Home);
        app.focus = Target::Setting(Setting::Remove);
        // ‹ › ne déclenchent pas un bouton.
        app.hits = vec![(Target::Setting(Setting::Remove), Rect::new(0.0, 0.0, 10.0, 10.0))];
        app.nav(Nav::Right);
        assert!(app.confirm.is_none() && app.store.hosts.len() == 1);
        assert!(app.nav(Nav::Ok).is_empty());
        assert!(matches!(app.confirm, Some(Confirm::Remove { .. })));
        assert_eq!(app.focus, Target::ConfirmNo);
        app.nav(Nav::Back);
        assert!(app.confirm.is_none() && app.store.hosts.len() == 1);
        app.activate(Target::Setting(Setting::Remove));
        assert_eq!(app.activate(Target::ConfirmYes), vec![Action::Save]);
        assert!(app.store.hosts.is_empty());
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn station_restart_flow() {
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.hosts.push(host());
        // Station ancienne : rien ne part.
        online(&mut app, "a", Some(StationLive { version: "0.4.9".into(), ..Default::default() }));
        assert!(app.activate(Target::Setting(Setting::StationRestart)).is_empty());
        online(&mut app, "a", Some(StationLive { version: "0.5.0".into(), supported: true, ..Default::default() }));
        let acts = app.activate(Target::Setting(Setting::StationRestart));
        assert_eq!(acts, vec![Action::Station { host: "a".into(), kind: StationKind::Restart, force: false }]);
        // Un appareil est en session : confirmation, puis « quand même ».
        app.station_reply("a", StationKind::Restart, Err(StationRefusal::InSession(Some("iPad".into()))));
        assert!(matches!(&app.confirm, Some(Confirm::Force { device, .. }) if device == "iPad"));
        let acts = app.activate(Target::ConfirmYes);
        assert_eq!(acts, vec![Action::Station { host: "a".into(), kind: StationKind::Restart, force: true }]);
        app.station_reply("a", StationKind::Restart, Ok(None));
        assert_eq!(app.station_waiting(), Some("a"));
        // Partie, puis revenue.
        app.live.get_mut("a").unwrap().online = Online::Offline;
        app.station_progress("a");
        online(&mut app, "a", Some(StationLive { version: "0.5.0".into(), supported: true, ..Default::default() }));
        app.station_progress("a");
        assert!(matches!(app.station_op.as_ref().map(|o| &o.step), Some(StationStep::Done(_))));
        assert_eq!(app.station_waiting(), None);
    }

    #[test]
    fn station_update_waits_for_version() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(host());
        let mut st = StationLive { version: "0.5.0".into(), supported: true, available: true, installable: true, update_version: Some("0.5.1".into()), ..Default::default() };
        online(&mut app, "a", Some(st.clone()));
        let acts = app.activate(Target::Setting(Setting::StationUpdate));
        assert_eq!(acts, vec![Action::Station { host: "a".into(), kind: StationKind::Update, force: false }]);
        app.station_reply("a", StationKind::Update, Ok(Some("0.5.1".into())));
        st.installing = true;
        online(&mut app, "a", Some(st.clone()));
        app.station_progress("a");
        assert_eq!(app.station_waiting(), Some("a"));
        st = StationLive { version: "0.5.1".into(), supported: true, ..Default::default() };
        online(&mut app, "a", Some(st));
        app.station_progress("a");
        assert!(matches!(app.station_op.as_ref().map(|o| &o.step), Some(StationStep::Done(m)) if m.contains("0.5.1")));
        // Trop tôt : le texte du contrat.
        app.station_op = None;
        online(&mut app, "a", Some(StationLive { version: "0.5.1".into(), supported: true, ..Default::default() }));
        app.activate(Target::Setting(Setting::StationRestart));
        app.station_reply("a", StationKind::Restart, Err(StationRefusal::TooSoon));
        assert!(matches!(app.station_op.as_ref().map(|o| &o.step), Some(StationStep::Failed(_))));
    }

    #[test]
    fn custom_profile() {
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.hosts.push(host());
        assert!(!app.selected().unwrap().settings.custom());
        app.store.hosts[0].settings.mbps = 80;
        assert!(app.selected().unwrap().settings.custom());
        // Choisir un profil y revient.
        app.activate(Target::Chip(Profile::Max));
        let st = &app.selected().unwrap().settings;
        assert!(!st.custom() && st.profile == Profile::Max && st.mbps == 0);
    }

    #[test]
    fn last_session_of_0_4() {
        // Un fichier écrit par la 0.4.x : ni fréquence ni format à part.
        let old: crate::model::LastSession = serde_json::from_str(r#"{"median":6.8,"p95":9.4,"seconds":4320,"mode":"1920 × 1200 · HEVC","mbps":38.0,"at":1}"#).unwrap();
        assert_eq!((old.hz, old.codec.as_str()), (0, ""));
        assert_eq!(old.transmitted(), "1920 × 1200 · HEVC · 38 Mbit/s");
        let new = crate::model::LastSession { mode: "2560 × 1440".into(), hz: 120, codec: "HEVC".into(), mbps: 80.0, ..Default::default() };
        assert_eq!(new.transmitted(), "2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s");
    }

    #[test]
    fn admin_address() {
        assert_eq!(admin_url("192.168.1.20", 24811), "https://192.168.1.20:24811/");
        assert_eq!(admin_url("fe80::1", 24811), "https://[fe80::1]:24811/");
        assert!(qrcodegen::QrCode::encode_text(&admin_url("192.168.1.20", 24811), qrcodegen::QrCodeEcc::Medium).is_ok());
    }

    #[test]
    fn wake_or_retry() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(host());
        app.live.entry("a".into()).or_default().online = Online::Offline;
        // Ni carte apprise ni MAC saisie : rien à envoyer.
        assert!(app.activate(Target::Wake).is_empty());
        app.store.hosts[0].wake.push(crate::model::WakeCard { mac: "3C:7C:3F:12:AB:CD".into(), ..Default::default() });
        assert_eq!(app.activate(Target::Wake), vec![Action::Wake("a".into())]);
        assert!(app.is_waking("a"));
        assert_eq!(app.waking_host(), Some("a"));
        // Pendant la connexion, le redémarrage de la Station s'annule comme le reste.
        let id = app.new_session_id();
        let mut s = SessionUi::new(id, "a", "A", Profile::Jeu, false, false, 1);
        s.phase = Phase::StationRestarting;
        app.open_session(s);
        assert_eq!(app.session_back(), vec![Action::CancelConnect(id)]);
    }

    /// Les lignes de la rubrique Image : « Écrans » juste sous « Définition »,
    /// au bureau seulement (le Deck, le salon et l'OS restent sur un écran).
    #[test]
    fn displays_row_follows_the_core() {
        let labels = |app: &App| app.rows(Section::Picture).iter().map(|r| r.label.clone()).collect::<Vec<_>>();
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.hosts.push(host());
        crate::i18n::force_english(false);
        let l = labels(&app);
        assert_eq!(&l[..3], ["Définition", "Écrans", "Fréquence"], "{l:?}");
        // Actif : les trois valeurs, Auto d'abord ; Auto par défaut.
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert!(row.enabled);
        let RowKind::Choice { options, current, disabled, .. } = &row.kind else { panic!("pas des valeurs au choix") };
        assert_eq!(options, &["Auto", "Cet écran", "Tous les écrans"]);
        assert_eq!((*current, disabled.iter().any(|d| *d)), (0, false));
        assert!(row.hint.starts_with("Auto : tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et en Personnalisé."), "{}", row.hint);
        // Choisir une valeur la garde, sans rendre le profil « Personnalisé ».
        app.activate(Target::Choice(Setting::Displays, 2));
        assert_eq!(app.selected().unwrap().settings.displays, "all");
        assert!(!app.selected().unwrap().settings.custom());
        app.activate(Target::Choice(Setting::Displays, 1));
        assert_eq!(app.selected().unwrap().settings.displays, "this");
        // Inactif (le cœur le dit : définition sur mesure, sans écran virtuel) : la valeur montrée, rien à changer.
        app.screens_plan = ScreensPlan { count: 1, enabled: false, shown: 1 };
        app.store.hosts[0].settings.resolution = 1080;
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert!(!row.enabled);
        assert_eq!(row.hint, "Avec une définition sur mesure, la session prend un seul écran.");
        let RowKind::Choice { current, disabled, .. } = &row.kind else { panic!() };
        assert_eq!((*current, disabled.clone()), (1, vec![true, false, true]));
        // Inactif faute d'écran virtuel (réglage coupé, Station sans pilote) : la ligne grisée sur
        // « Cet écran », sans aide (pas celle d'Auto, qui ne vaut pas), comme sur Mac.
        app.store.hosts[0].settings.resolution = 0;
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert!(!row.enabled);
        assert_eq!(row.hint, "", "pas d'aide quand la ligne est inactive sans écran virtuel");
        let RowKind::Choice { current, disabled, .. } = &row.kind else { panic!() };
        assert_eq!((*current, disabled.clone()), (1, vec![true, false, true]));
        // Une définition sur mesure garde son aide, écran virtuel ou non.
        app.store.hosts[0].settings.resolution = 1080;
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert_eq!(row.hint, "Avec une définition sur mesure, la session prend un seul écran.");
        app.store.hosts[0].settings.resolution = 0;
        // Actif de nouveau : l'aide d'Auto.
        app.screens_plan = ScreensPlan::default();
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert!(row.enabled && row.hint.starts_with("Auto : tous les écrans"));
        app.screens_plan = ScreensPlan { count: 1, enabled: false, shown: 1 };
        app.store.hosts[0].settings.resolution = 1080;
        app.store.hosts[0].settings.displays = "all".into();
        app.activate(Target::Choice(Setting::Displays, 0));
        assert_eq!(app.selected().unwrap().settings.displays, "all", "inactif : le réglage ne bouge pas");
        // Anglais.
        crate::i18n::force_english(true);
        let row = app.rows(Section::Picture).into_iter().find(|r| r.setting == Setting::Displays).unwrap();
        assert_eq!((row.label.as_str(), row.hint.as_str()), ("Displays", "With a custom resolution, the session takes a single display."));
        crate::i18n::force_english(false);
        // Les autres interfaces n'ont pas la ligne.
        for layout in [Layout::Handheld, Layout::Tv] {
            let mut app = App::new(Store::default(), layout);
            app.store.hosts.push(host());
            assert!(!app.rows(Section::Picture).iter().any(|r| r.setting == Setting::Displays));
        }
    }

    #[test]
    fn last_session_with_several_displays() {
        crate::i18n::force_english(false);
        let one = crate::model::LastSession { mode: "3840 × 2160".into(), hz: 160, codec: "HEVC".into(), mbps: 120.0, screens: 1, ..Default::default() };
        assert_eq!(one.transmitted(), "3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s");
        let three = crate::model::LastSession { screens: 3, ..one.clone() };
        assert_eq!(three.transmitted(), "3 écrans · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s");
        crate::i18n::force_english(true);
        assert_eq!(three.transmitted(), "3 displays · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s");
        crate::i18n::force_english(false);
        // Un fichier d'avant le multi-écrans : un seul écran.
        let old: crate::model::LastSession = serde_json::from_str(r#"{"median":6.8,"p95":9.4,"seconds":4320,"mode":"1920 × 1200","hz":120,"codec":"HEVC","mbps":38.0,"at":1}"#).unwrap();
        assert_eq!(old.screens, 0);
        assert_eq!(old.transmitted(), "1920 × 1200 · 120 Hz · HEVC · 38 Mbit/s");
    }

    // MARK: - Plusieurs ordinateurs (docs/plusieurs-ordinateurs.md)

    /// Un bureau avec cinq ordinateurs (a à e) ; chacun en ligne.
    fn desk() -> App {
        let mut app = App::new(Store::default(), Layout::Desktop);
        for id in ["a", "b", "c", "d", "e"] {
            app.store.hosts.push(Host { id: id.into(), name: id.to_uppercase(), ..host() });
            app.live.entry(id.into()).or_default().online = Online::Online;
        }
        app
    }

    fn open(app: &mut App, host: &str) -> SessionId {
        let id = app.new_session_id();
        let mut s = SessionUi::new(id, host, &host.to_uppercase(), Profile::Bureau, false, false, 1);
        s.phase = Phase::Streaming;
        app.open_session(s);
        id
    }

    /// « Garder le son » (bouton du haut-parleur) s'allume et s'éteint pour la session de la barre et
    /// l'annonce avec son identifiant ; « Couper le son » du menu de session est un autre choix. Qui est
    /// active, et qui s'entend, c'est le registre du cœur (`device::focus`).
    #[test]
    fn keep_sound_belongs_to_the_session_of_the_bar() {
        let mut app = desk();
        let (a, b) = (open(&mut app, "a"), open(&mut app, "b"));
        assert_eq!(app.active, Some(b), "la session ouverte devient active");
        assert_eq!(app.activate(Target::BarSound), vec![Action::KeepSound(b, true)]);
        app.activate_session(a);
        assert!(app.session_by(b).unwrap().keep_sound && !app.session_by(a).unwrap().keep_sound);
        assert_eq!(app.activate(Target::BarSound), vec![Action::KeepSound(a, true)]);
        assert_eq!(app.activate(Target::BarSound), vec![Action::KeepSound(a, false)]);
        assert_eq!(app.activate(Target::Menu(MenuItem::Sound)), vec![Action::Mute(a, true)]);
        assert!(app.session_by(a).unwrap().muted && !app.session_by(a).unwrap().keep_sound);
    }

    /// Le mode d'une session de plus est la surface de sa fenêtre, à la fréquence de son écran ;
    /// une définition choisie garde la largeur de la fenêtre ; sur batterie, « Alléger » plafonne à 60.
    #[test]
    fn an_extra_session_takes_its_window_size() {
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.machine.display = (3840, 2160);
        app.machine.refresh = 144;
        let mut h = host();
        // La fenêtre : deux tiers d'un écran 4K, en pixels.
        assert_eq!(app.mode_for(&h, (2560, 1440), 144), (2560, 1440, 144));
        assert_eq!(app.session_mode(&h), (3840, 2160, 144), "l'écran de l'appareil n'a pas changé");
        h.settings.resolution = 1080;
        assert_eq!(app.mode_for(&h, (2560, 1440), 144), (1920, 1080, 144));
        h.settings.fps = 90;
        assert_eq!(app.mode_for(&h, (2560, 1440), 144).2, 90);
        h.settings.fps = 0;
        app.machine.battery = Some((50, false));
        assert_eq!(app.mode_for(&h, (2560, 1440), 144).2, 60);
    }

    /// Quand la session active se termine, celle qui l'a été en dernier reprend la main (comme
    /// dans le registre du cœur, qui a le dernier mot) ; la dernière fermée laisse l'app sans
    /// session active.
    #[test]
    fn the_last_active_session_takes_over() {
        let mut app = desk();
        let (a, b, c) = (open(&mut app, "a"), open(&mut app, "b"), open(&mut app, "c"));
        app.activate_session(a);
        app.activate_session(c);
        // L'ordre des activations : b, a, c. c se termine : a reprend la main.
        assert!(app.close_session(c));
        assert_eq!(app.active, Some(a));
        // Une session qui n'est pas active se termine : la session active ne change pas.
        assert!(!app.close_session(b));
        assert_eq!(app.active, Some(a));
        assert!(app.close_session(a));
        assert_eq!((app.active, app.sessions.len()), (None, 0));
        // Un identifiant ne sert qu'une fois : le suivant n'est jamais celui d'une session finie.
        let d = open(&mut app, "d");
        assert!(d > c);
        // Activer une session qui n'existe plus ne change rien.
        assert!(!app.activate_session(a) && app.active == Some(d));
    }

    /// La barre d'une session qu'on quitte se ferme ; un écran ouvert par-dessus la session (les
    /// manettes) revient à la session de la fenêtre qui prend la main.
    #[test]
    fn switching_closes_the_bar_and_brings_back_the_session() {
        let mut app = desk();
        let (a, b) = (open(&mut app, "a"), open(&mut app, "b"));
        app.activate_session(a);
        {
            let s = app.session_mut().unwrap();
            s.bar = true;
            s.bar_menu = Some(BarMenu::Computers);
        }
        app.screen = Screen::Gamepads;
        app.back_to = Screen::Session;
        assert!(app.activate_session(b));
        assert!(app.session_by(a).is_some_and(|s| !s.bar && s.bar_menu.is_none()));
        assert_eq!((app.screen, app.back_to), (Screen::Session, Screen::Home));
    }

    /// Le menu « Ordinateurs » : un clic ouvre une session de plus (en ligne), réveille (hors ligne), ou
    /// remet la fenêtre devant (déjà en session) ; quatre ordinateurs au plus ; le même jamais deux fois.
    #[test]
    fn the_computers_menu_connects_wakes_or_shows() {
        let mut app = desk();
        app.store.hosts[2].wake.push(crate::model::WakeCard { mac: "3C:7C:3F:12:AB:CD".into(), ..Default::default() });
        let a = open(&mut app, "a");
        // En ligne, pas encore en session : une session de plus, avec le profil de l'ordinateur.
        app.session_mut().unwrap().bar_menu = Some(BarMenu::Computers);
        assert_eq!(app.activate(Target::BarComputer(1)), vec![Action::Connect { host: "b".into(), profile: Profile::Jeu }]);
        assert!(app.session().unwrap().bar_menu.is_none(), "le menu se referme");
        let b = open(&mut app, "b");
        // Déjà en session : sa fenêtre passe devant, sans nouvelle session.
        assert_eq!(app.activate(Target::BarComputer(0)), vec![Action::ShowSession(a)]);
        assert_eq!(app.activate(Target::BarComputer(1)), vec![Action::ShowSession(b)]);
        // Hors ligne avec une carte apprise : le réveil, comme l'accueil ; pendant le réveil, rien de plus.
        app.live.get_mut("c").unwrap().online = Online::Offline;
        assert_eq!(app.activate(Target::BarComputer(2)), vec![Action::Wake("c".into())]);
        assert!(app.is_waking("c"));
        app.session_mut().unwrap().bar_menu = Some(BarMenu::Computers);
        assert!(app.activate(Target::BarComputer(2)).is_empty());
        assert_eq!(app.session().unwrap().bar_menu, Some(BarMenu::Computers), "une ligne inactive laisse le menu ouvert");
        // Hors ligne sans adresse : « Réessayer ».
        app.live.get_mut("d").unwrap().online = Online::Offline;
        assert_eq!(app.activate(Target::BarComputer(3)), vec![Action::Poll]);
        // Quatre au plus : le quatrième s'ouvre, le cinquième est refusé (« Se connecter » inactif).
        app.live.get_mut("c").unwrap().online = Online::Online;
        app.live.get_mut("d").unwrap().online = Online::Online;
        app.waking = None;
        open(&mut app, "c");
        assert_eq!(app.activate(Target::BarComputer(4)), vec![Action::Connect { host: "e".into(), profile: Profile::Jeu }]);
        open(&mut app, "d");
        assert!(app.sessions_full());
        app.session_mut().unwrap().bar_menu = Some(BarMenu::Computers);
        assert!(app.activate(Target::BarComputer(4)).is_empty());
        assert_eq!(app.session().unwrap().bar_menu, Some(BarMenu::Computers));
        // Les sessions déjà ouvertes s'affichent toujours.
        assert_eq!(app.activate(Target::BarComputer(0)), vec![Action::ShowSession(a)]);
    }

    /// Une session finie sur une erreur garde son message dans sa fenêtre jusqu'à « Fermer », mais le cœur l'a
    /// désinscrite : elle ne compte plus pour la limite de quatre, ni pour « Afficher » (on peut se reconnecter à
    /// cet ordinateur), ni pour le seuil de « Garder le son » (deux sessions en cours).
    #[test]
    fn a_failed_session_no_longer_counts() {
        let mut app = desk();
        let ids: Vec<SessionId> = ["a", "b", "c", "d"].iter().map(|h| open(&mut app, h)).collect();
        assert!(app.sessions_full() && app.running_sessions() == 4);
        assert!(app.activate(Target::BarComputer(4)).is_empty(), "quatre en cours : « Se connecter » est inactif");
        app.session_by_mut(ids[3]).unwrap().phase = Phase::Ended { error: Some("Un autre appareil a pris la main".into()) };
        assert!(!app.sessions_full() && app.running_sessions() == 3 && app.sessions.len() == 4);
        assert!(app.session_of_host("d").is_none() && app.session_of_host("c").is_some());
        assert_eq!(app.activate(Target::BarComputer(4)), vec![Action::Connect { host: "e".into(), profile: Profile::Jeu }]);
        // On peut se reconnecter à l'ordinateur dont la session a échoué ; le message n'est pas « Afficher ».
        app.activate_session(ids[0]);
        assert_eq!(app.activate(Target::BarComputer(3)), vec![Action::Connect { host: "d".into(), profile: Profile::Jeu }]);
        // Deux sessions en cours, dont une finie : une seule compte, pas de « Garder le son ».
        let mut app = desk();
        let (a, b) = (open(&mut app, "a"), open(&mut app, "b"));
        assert_eq!(app.running_sessions(), 2);
        app.session_by_mut(a).unwrap().phase = Phase::Ended { error: Some("coupée".into()) };
        assert_eq!((app.running_sessions(), app.active), (1, Some(b)));
        // Un identifiant ne sert jamais deux fois : la session finie garde le sien tant qu'elle est dans la liste.
        assert!(app.session_by(a).is_some_and(|s| !s.running()));
    }

    /// Le Steam Deck, le salon et KyberNow Link OS gardent une session ; le bureau en tient plusieurs.
    #[test]
    fn only_the_desktop_keeps_several_computers() {
        assert!(App::new(Store::default(), Layout::Desktop).multi_sessions());
        assert!(!App::new(Store::default(), Layout::Handheld).multi_sessions());
        assert!(!App::new(Store::default(), Layout::Tv).multi_sessions());
        let mut os = App::new(Store::default(), Layout::Desktop);
        os.os = Some(OsInfo::default());
        assert!(!os.multi_sessions(), "KyberNow Link OS en usage bureau : une session");
    }

    /// Les actions de session portent leur session : une action partie d'une session qui n'est plus
    /// l'active (ou qui n'existe plus) ne vise jamais la suivante.
    #[test]
    fn session_actions_name_their_session() {
        let mut app = desk();
        let (a, b) = (open(&mut app, "a"), open(&mut app, "b"));
        assert_eq!(app.activate(Target::BarQuit), vec![Action::Disconnect(b)]);
        app.activate_session(a);
        assert_eq!(app.activate(Target::BarQuit), vec![Action::Disconnect(a)]);
        assert_eq!(app.activate(Target::BarMouse), vec![Action::ToggleMouse(a)]);
        assert_eq!(app.activate(Target::BarFullscreen), vec![Action::ToggleFullscreen(Some(a))]);
        assert_eq!(app.activate(Target::BarPick(Profile::Max)), vec![Action::SetProfile(a, Profile::Max), Action::Save]);
        assert_eq!(app.activate(Target::Shortcut(0)).first(), Some(&Action::Shortcut(a, SHORTCUTS[0].2.to_vec())));
        assert_eq!(app.activate(Target::Menu(MenuItem::Sound)), vec![Action::Mute(a, true)]);
        // Une session finie sur une erreur : « Retour » la ferme, la dernière ramène l'accueil.
        app.session_by_mut(b).unwrap().phase = Phase::Ended { error: Some("coupée".into()) };
        app.activate_session(b);
        assert!(app.session_back().is_empty());
        assert_eq!((app.active, app.screen), (Some(a), Screen::Session), "une autre session reste : pas d'accueil");
        app.session_mut().unwrap().phase = Phase::Ended { error: Some("coupée".into()) };
        assert!(app.session_back().is_empty());
        assert_eq!((app.active, app.screen, app.sessions.len()), (None, Screen::Home, 0));
    }

    // MARK: - KyberNow Share (docs/kybernow-share.md)

    /// « Rejoindre avec un code » : l'écran s'ouvre sur le prénom proposé (le nom de l'appareil, puis celui de la
    /// fois d'avant), les chiffres se tapent (six au plus, rien d'autre), « Rejoindre » ne part qu'avec six
    /// chiffres et un prénom, et retient le prénom ; le refus du cœur s'affiche sous les champs ; B abandonne la
    /// recherche.
    #[test]
    fn joining_a_share_by_code() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.device_name = "fedora-bureau".into();
        app.go(Screen::Home);
        assert_eq!(app.activate(Target::JoinCard), vec![Action::TextInput]);
        assert_eq!((app.screen, app.join.name.as_str(), app.join.field, app.focus), (Screen::Join, "fedora-bureau", JoinField::Code, Target::JoinCode));
        // Les chiffres du code ; une lettre n'y entre pas, le prénom la prend.
        for c in "48a29 17x3".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!(app.join.code, "482917");
        // « Rejoindre » sans prénom : le focus va au prénom, rien ne part.
        app.join.name.clear();
        assert!(app.activate(Target::JoinGo).is_empty());
        assert_eq!((app.join.field, app.focus, app.join.busy), (JoinField::Name, Target::JoinName, false));
        for c in "Léa".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!(app.join.name, "Léa");
        app.nav(Nav::Erase);
        app.nav(Nav::Char('a'));
        // Entrée dans le prénom, avec tout donné : « Rejoindre ».
        let acts = app.nav(Nav::Ok);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "Léa".into(), quality: ShareQuality::Auto }]);
        assert!(app.join.busy && app.store.app.share_name == "Léa");
        assert!(app.activate(Target::JoinGo).is_empty(), "une recherche à la fois");
        // Le refus du cœur, pour cette recherche seulement.
        let attempt = app.join.attempt;
        app.join_failed(attempt + 1, "autre".into());
        assert!(app.join.busy && app.join.message.is_none());
        app.join_failed(attempt, "Aucun partage avec ce code sur le réseau.".into());
        assert!(!app.join.busy && app.join.message.as_deref() == Some("Aucun partage avec ce code sur le réseau."));
        // Taper efface le message ; B abandonne : l'accueil, et la recherche en vol n'ouvre plus rien.
        app.nav(Nav::Erase);
        app.join.busy = true;
        assert!(app.join_current(attempt));
        app.nav(Nav::Back);
        assert!(app.screen == Screen::Home && !app.join_current(attempt));
        // La fois suivante : le prénom retenu.
        app.activate(Target::JoinCard);
        assert_eq!(app.join.name, "Léa");
    }

    /// « Qualité de l'image » à « Rejoindre » : « Auto » est proposé, les cinq choix se prennent (clavier, pavé, souris),
    /// le choix est retenu avec le prénom (`kybernow.json`) et revient la fois suivante ; il part avec « Rejoindre » ;
    /// pendant la recherche, il ne change plus ; un nom inconnu du fichier se lit « Auto ».
    #[test]
    fn joining_a_share_with_a_picture_quality() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.device_name = "fedora-bureau".into();
        app.go(Screen::Home);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::Auto, "« Auto » proposé");
        assert_eq!(ShareQuality::ALL.map(ShareQuality::label), ["Auto", "Comme l'hôte", "4K · 60 Hz", "1080p · 60 Hz", "720p · 30 Hz"]);
        crate::i18n::force_english(true);
        assert_eq!(ShareQuality::ALL.map(ShareQuality::label), ["Auto", "Same as host", "4K · 60 Hz", "1080p · 60 Hz", "720p · 30 Hz"]);
        crate::i18n::force_english(false);
        assert_eq!(ShareQuality::ALL.map(ShareQuality::as_str), ["auto", "host", "2160p60", "1080p60", "720p30"], "les noms de l'échange avec Admin, dans l'ordre des choix");
        for q in ShareQuality::ALL {
            assert!(app.activate(Target::JoinQuality(q)).is_empty());
            assert_eq!(app.join.quality, q);
            assert_eq!(ShareQuality::parse(q.as_str()), q, "le nom retenu se relit");
        }
        assert_eq!((ShareQuality::parse(""), ShareQuality::parse("4k240")), (ShareQuality::Auto, ShareQuality::Auto));
        // Le choix part avec le code et le prénom, et il est retenu.
        app.activate(Target::JoinQuality(ShareQuality::P720_30));
        for c in "482917".chars() {
            app.nav(Nav::Char(c));
        }
        let acts = app.activate(Target::JoinGo);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "fedora-bureau".into(), quality: ShareQuality::P720_30 }]);
        assert_eq!(app.store.app.share_quality, "720p30");
        // Pendant la recherche, le choix ne bouge plus.
        app.activate(Target::JoinQuality(ShareQuality::Host));
        assert_eq!(app.join.quality, ShareQuality::P720_30);
        // La fois suivante : le choix retenu, et un fichier à la main avec un nom inconnu : « Auto ».
        app.nav(Nav::Back);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::P720_30);
        app.nav(Nav::Back);
        app.store.app.share_quality = "4k240".into();
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::Auto);
    }

    /// « 4K · 60 Hz » à « Rejoindre » : placé entre « Comme l'hôte » et « 1080p · 60 Hz », il part avec le code
    /// (`2160p60` dans `kybernow.json`) et revient la fois suivante.
    #[test]
    fn joining_a_share_in_4k() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.device_name = "fedora-bureau".into();
        app.go(Screen::Home);
        app.activate(Target::JoinCard);
        assert_eq!(ShareQuality::ALL.iter().position(|q| *q == ShareQuality::P2160_60), Some(2), "après « Comme l'hôte »");
        assert!(app.activate(Target::JoinQuality(ShareQuality::P2160_60)).is_empty());
        assert_eq!(app.join.quality, ShareQuality::P2160_60);
        for c in "482917".chars() {
            app.nav(Nav::Char(c));
        }
        let acts = app.activate(Target::JoinGo);
        assert_eq!(acts, vec![Action::Save, Action::JoinShare { code: "482917".into(), name: "fedora-bureau".into(), quality: ShareQuality::P2160_60 }]);
        assert_eq!(app.store.app.share_quality, "2160p60");
        app.nav(Nav::Back);
        app.activate(Target::JoinCard);
        assert_eq!(app.join.quality, ShareQuality::P2160_60, "retenu pour la fois suivante");
        assert_eq!(ShareQuality::parse("2160p60"), ShareQuality::P2160_60);
    }

    /// Au pavé, les flèches vont du prénom aux cinq choix de « Qualité de l'image », puis à « Rejoindre » : rien n'est
    /// atteint sans eux.
    #[test]
    fn picture_quality_is_reached_with_the_pad() {
        crate::i18n::force_english(false);
        for (layout, size) in [(Layout::Handheld, (1280, 800)), (Layout::Tv, (1920, 1080)), (Layout::Desktop, (1280, 800))] {
            let mut app = App::new(Store::default(), layout);
            app.go(Screen::Home);
            app.activate(Target::JoinCard);
            let mut cv = crate::ui::canvas::Canvas::new();
            cv.resize(size.0, size.1, 1.0);
            app.hits = crate::ui::draw(&app, &mut cv);
            let quality: Vec<Rect> = ShareQuality::ALL.iter().map(|q| app.hits.iter().find(|(t, _)| *t == Target::JoinQuality(*q)).expect("un choix").1).collect();
            assert!(quality.windows(2).all(|w| w[0].x + w[0].w <= w[1].x + 0.5 && (w[0].y - w[1].y).abs() < 0.5), "{layout:?} : cinq choix sur une ligne");
            let name = app.hits.iter().find(|(t, _)| *t == Target::JoinName).unwrap().1;
            let go = app.hits.iter().find(|(t, _)| *t == Target::JoinGo).unwrap().1;
            assert!(name.y + name.h <= quality[0].y && quality[0].y + quality[0].h <= go.y, "{layout:?} : sous le prénom, au-dessus de « Rejoindre »");
            app.focus = Target::JoinName;
            app.nav(Nav::Down);
            assert!(matches!(app.focus, Target::JoinQuality(_)), "{layout:?} : {:?}", app.focus);
            app.focus = Target::JoinQuality(ShareQuality::Auto);
            app.nav(Nav::Right);
            assert_eq!(app.focus, Target::JoinQuality(ShareQuality::Host), "{layout:?}");
            app.nav(Nav::Ok);
            assert_eq!(app.join.quality, ShareQuality::Host, "{layout:?} : Entrée choisit");
            app.nav(Nav::Right);
            assert_eq!(app.focus, Target::JoinQuality(ShareQuality::P2160_60), "{layout:?} : « 4K · 60 Hz » après « Comme l'hôte »");
            app.nav(Nav::Right);
            assert_eq!(app.focus, Target::JoinQuality(ShareQuality::P1080_60), "{layout:?}");
            app.nav(Nav::Left);
            app.nav(Nav::Ok);
            assert_eq!(app.join.quality, ShareQuality::P2160_60, "{layout:?}");
            app.nav(Nav::Down);
            assert_eq!(app.focus, Target::JoinGo, "{layout:?}");
        }
    }

    /// La qualité de l'image en session : la barre et la tuile montrent la qualité reçue (le choix tant que Kyber ne la
    /// donne pas) ; un choix part à la Station, est retenu, ferme le menu ; le même choix ne demande rien ; à la manette,
    /// la tuile ouvre les cinq choix et B revient au menu ; l'avis de la Station s'affiche en capsule.
    #[test]
    fn picture_quality_in_a_guest_session() {
        crate::i18n::force_english(false);
        let mut app = desk();
        let id = app.new_session_id();
        app.open_session(SessionUi::guest(id, "share:g1", "Atelier", ShareRole::Spectator));
        app.session_mut().unwrap().phase = Phase::Streaming;
        let shown = |app: &App| app.session().unwrap().guest.as_ref().unwrap().shown().to_string();
        assert_eq!(shown(&app), "Auto", "la qualité reçue n'est pas connue : le choix");
        app.share_image(id, Some("1080p · 60 Hz".into()), None);
        assert_eq!(shown(&app), "1080p · 60 Hz");
        assert!(app.session().unwrap().notice.is_none());
        // Le menu de la barre : s'ouvre et se ferme à la puce ; un choix part à la Station et se retient.
        assert!(app.activate(Target::BarQuality).is_empty());
        assert_eq!(app.session().unwrap().bar_menu, Some(BarMenu::Quality));
        let acts = app.activate(Target::QualityPick(ShareQuality::P720_30));
        assert_eq!(acts, vec![Action::ShareQuality(id, ShareQuality::P720_30), Action::Save]);
        assert_eq!((app.session().unwrap().bar_menu, app.store.app.share_quality.as_str()), (None, "720p30"));
        assert_eq!(app.session().unwrap().guest.as_ref().unwrap().quality, ShareQuality::P720_30);
        // La Station répond : la qualité reçue ; l'avis bref arrive avec.
        app.share_image(id, Some("720p · 30 Hz".into()), Some("Picture lightened to keep up.".into()));
        assert_eq!(shown(&app), "720p · 30 Hz");
        assert_eq!(app.session().unwrap().notice.as_ref().map(|n| n.0.as_str()), Some("Picture lightened to keep up."));
        // « 4K · 60 Hz » en cours de session : il part à la Station, est retenu, et la qualité reçue le dit.
        app.activate(Target::BarQuality);
        let acts = app.activate(Target::QualityPick(ShareQuality::P2160_60));
        assert_eq!(acts, vec![Action::ShareQuality(id, ShareQuality::P2160_60), Action::Save]);
        assert_eq!(app.store.app.share_quality, "2160p60");
        app.share_image(id, Some("4K · 60 Hz".into()), None);
        assert_eq!(shown(&app), "4K · 60 Hz");
        app.activate(Target::BarQuality);
        app.activate(Target::QualityPick(ShareQuality::P720_30));
        app.share_image(id, Some("720p · 30 Hz".into()), None);
        // Kyber ne sait plus la taille : plus de qualité reçue, le choix se montre.
        app.share_image(id, None, None);
        assert!(app.session().unwrap().guest.as_ref().unwrap().image.is_none());
        assert_eq!(shown(&app), "720p · 30 Hz");
        app.activate(Target::BarQuality);
        assert!(app.activate(Target::QualityPick(ShareQuality::P720_30)).is_empty(), "le même choix : rien à demander");
        assert!(app.session().unwrap().bar_menu.is_none());
        // Une session d'hôte n'a pas ce choix.
        open(&mut app, "a");
        assert_ne!(app.active, Some(id));
        assert!(app.activate(Target::QualityPick(ShareQuality::Host)).is_empty());
        app.activate_session(id);
        // À la manette : la tuile du menu ouvre les cinq choix sur l'image ; B revient au menu ; un choix revient à l'image.
        app.layout = Layout::Handheld;
        app.session_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Quality)), vec![Action::ReleaseAll(id)]);
        let s = app.session().unwrap();
        assert!(s.share_panel && !s.menu);
        assert_eq!(app.focus, Target::QualityPick(ShareQuality::P720_30), "le focus sur le choix en place");
        assert!(app.session_back().is_empty());
        let s = app.session().unwrap();
        assert!(!s.share_panel && s.menu, "B revient au menu");
        assert_eq!(app.focus, Target::Menu(MenuItem::Quality));
        app.activate(Target::Menu(MenuItem::Quality));
        let acts = app.activate(Target::QualityPick(ShareQuality::Host));
        assert_eq!(acts, vec![Action::ShareQuality(id, ShareQuality::Host), Action::Save]);
        let s = app.session().unwrap();
        assert!(!s.share_panel && !s.menu, "le choix revient à l'image");
        // Un événement d'une session qui n'existe plus ne fait rien.
        app.share_image(id + 99, Some("1080p · 60 Hz".into()), Some("x".into()));
    }

    /// Le panneau de l'hôte dit la qualité d'un invité entre son app et la durée, seulement quand c'est une image à part.
    #[test]
    fn a_guest_line_tells_its_picture_quality() {
        crate::i18n::force_english(false);
        let g = ShareGuest { id: "g".into(), name: "Léa".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(1_000), quality: Some("720p · 30 Hz".into()) };
        assert_eq!(g.meta(1_000 + 4 * 60), "iPhone ou iPad · 720p · 30 Hz · depuis 4 min");
        assert_eq!(ShareGuest { quality: None, ..g.clone() }.meta(1_000 + 4 * 60), "iPhone ou iPad · depuis 4 min", "l'image de l'hôte : rien");
        assert_eq!(ShareGuest { since: None, ..g.clone() }.meta(5), "iPhone ou iPad · 720p · 30 Hz");
        crate::i18n::force_english(true);
        assert_eq!(g.meta(1_000 + 4 * 60), "iPhone or iPad · 720p · 30 Hz · for 4 min");
        crate::i18n::force_english(false);
    }

    /// Au pavé (Deck, salon, Link OS) : les chiffres du code puis, le sixième tapé, le focus va à « Rejoindre » ; le
    /// pavé des lettres sert au prénom (majuscule d'une touche) ; le Select (⧉) ouvre l'écran depuis l'accueil.
    #[test]
    fn joining_a_share_on_a_pad() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.device_name = "Steam Deck".into();
        app.go(Screen::Home);
        assert_eq!(app.nav(Nav::Join), vec![Action::TextInput]);
        assert_eq!((app.screen, app.focus), (Screen::Join, Target::Key('1')));
        for c in "123456".chars() {
            app.activate(Target::Key(c));
        }
        assert_eq!((app.join.code.as_str(), app.focus), ("123456", Target::JoinGo));
        // Le prénom : le champ prend le pavé des lettres ; la majuscule ne vaut que pour la touche suivante.
        app.activate(Target::JoinName);
        assert_eq!(app.join.field, JoinField::Name);
        app.join.name.clear();
        app.activate(Target::KeyShift);
        app.activate(Target::Key('é'));
        app.activate(Target::Key('m'));
        assert_eq!(app.join.name, "Ém");
        let long = "x".repeat(40);
        for c in long.chars() {
            app.activate(Target::Key(c));
        }
        assert_eq!(app.join.name.chars().count(), NAME_MAX);
        app.activate(Target::KeyErase);
        assert_eq!(app.join.name.chars().count(), NAME_MAX - 1);
        // Hors de l'accueil, Select ne fait rien.
        assert!(app.nav(Nav::Join).is_empty());
    }

    /// Une Station qui ne partage pas : à la souris « Partager » est inactif (rien ne part, sa raison est l'infobulle) ; à
    /// la manette la tuile se choisit et ouvre le panneau réduit à la raison ; avant la première réponse d'Admin, le
    /// panneau attend cette réponse (jamais un bouton muet). Avec un partage possible, il ouvre le panneau et demande
    /// l'ouverture à Admin, une fois ; le panneau déjà ouvert se referme au second clic.
    #[test]
    fn share_button_of_the_host() {
        crate::i18n::force_english(false);
        let mut app = desk();
        let a = open(&mut app, "a");
        // Sans réponse d'Admin : le panneau s'ouvre et attend, Admin est sondé de nouveau.
        assert_eq!(app.activate(Target::BarShare), vec![Action::Poll]);
        let s = app.session().unwrap();
        assert!(s.share_wait && s.share_busy && s.bar_menu == Some(BarMenu::Share));
        // Ordinateur injoignable (rien de neuf) : la demande part quand même, son refus dira pourquoi.
        assert_eq!(app.share_known("a"), vec![Action::Share(a, ShareOp::Open)]);
        assert!(!app.session().unwrap().share_wait && app.session().unwrap().share_busy);
        app.share_reply(a, Err("Admin injoignable".into()));
        assert_eq!(app.session().unwrap().share_error.as_deref(), Some("Admin injoignable"));
        // Plus rien n'attend : une autre réponse n'ouvre rien.
        assert!(app.share_known("a").is_empty());
        // Le panneau refermé pendant l'attente n'ouvre rien non plus.
        assert!(app.activate(Target::BarShare).is_empty() && app.session().unwrap().bar_menu.is_none());
        assert_eq!(app.activate(Target::BarShare), vec![Action::Poll]);
        assert!(app.activate(Target::BarShare).is_empty());
        assert!(app.session().unwrap().share_wait && app.session().unwrap().bar_menu.is_none());
        assert!(app.share_known("a").is_empty() && !app.session().unwrap().share_busy);
        // La réponse dit que le partage est impossible : le panneau se réduit à la raison, rien ne part.
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Unsupported);
        let s = app.session_mut().unwrap();
        (s.share_wait, s.share_busy, s.bar_menu) = (true, true, Some(BarMenu::Share));
        assert!(app.share_known("a").is_empty());
        assert!(!app.session().unwrap().share_busy && app.session().unwrap().bar_menu == Some(BarMenu::Share));
        app.session_mut().unwrap().bar_menu = None;
        // À la souris, « Partager » impossible est inactif : ni panneau, ni capsule.
        assert!(app.activate(Target::BarShare).is_empty());
        let s = app.session().unwrap();
        assert!(s.bar_menu.is_none() && s.notice.is_none() && !s.share_busy);
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Disabled);
        assert!(app.activate(Target::BarShare).is_empty());
        assert!(app.session().unwrap().bar_menu.is_none() && app.session().unwrap().notice.is_none());
        // À la manette (la tuile du menu), il se choisit : le panneau s'ouvre, réduit à la raison ; B le referme.
        app.session_mut().unwrap().menu = true;
        assert_eq!(app.activate(Target::Menu(MenuItem::Share)), vec![Action::ReleaseAll(a)]);
        let s = app.session().unwrap();
        assert!(s.share_panel && !s.menu && !s.share_busy && s.share.is_none() && s.bar_menu.is_none());
        assert!(app.session_back().is_empty() && !app.session().unwrap().share_panel);
        assert!(!app.session().unwrap().menu, "B ferme le panneau, il ne rouvre pas le menu");
        // Disponible : le menu de la barre s'ouvre, Admin ouvre le partage.
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        assert_eq!(app.activate(Target::BarShare), vec![Action::Share(a, ShareOp::Open)]);
        let s = app.session().unwrap();
        assert!(s.share_busy && s.bar_menu == Some(BarMenu::Share));
        // Une demande à la fois.
        assert!(app.activate(Target::ShareNewCode).is_empty());
        let view = ShareView { code: "482917".into(), link: "kybernow://join?x".into(), web: "https://h:1/link/?join=482917".into(), guests: vec![ShareGuest { id: "g1".into(), name: "Léa".into(), role: ShareRole::Spectator, app: "ios".into(), since: Some(1), quality: None }] };
        app.share_reply(a, Ok(Some(view.clone())));
        assert!(!app.session().unwrap().share_busy && app.session().unwrap().share.is_some());
        // Ses invités : deux droits indépendants, chacun donné ou repris sans toucher à l'autre, puis « Retirer ».
        let with = |role| ShareView { guests: vec![ShareGuest { role, ..view.guests[0].clone() }], ..view.clone() };
        let ask = |app: &mut App, t: Target, role: ShareRole| {
            assert_eq!(app.activate(t), vec![Action::Share(a, ShareOp::Role("g1".into(), role))], "{t:?}");
            app.share_reply(a, Ok(Some(with(role))));
        };
        // Une seule demande à la fois, jusqu'à la réponse d'Admin : le rôle voulu se calcule d'après celui qui est
        // montré, deux clics rapides sur les deux droits s'annuleraient sinon (Spectator → Kbm écraserait Player).
        assert_eq!(app.activate(Target::SharePads(0)), vec![Action::Share(a, ShareOp::Role("g1".into(), ShareRole::Player))]);
        assert!(app.activate(Target::ShareKbm(0)).is_empty() && app.activate(Target::SharePads(0)).is_empty() && app.activate(Target::ShareKick(0)).is_empty());
        app.share_reply(a, Ok(Some(with(ShareRole::Player)))); // les manettes données
        ask(&mut app, Target::ShareKbm(0), ShareRole::PlayerKbm); // puis le clavier et la souris, les manettes gardées
        ask(&mut app, Target::SharePads(0), ShareRole::Kbm); // les manettes reprises, le clavier et la souris gardés
        ask(&mut app, Target::SharePads(0), ShareRole::PlayerKbm); // rendues
        ask(&mut app, Target::ShareKbm(0), ShareRole::Player); // le clavier et la souris repris, les manettes gardées
        ask(&mut app, Target::SharePads(0), ShareRole::Spectator); // plus aucun droit
        ask(&mut app, Target::ShareKbm(0), ShareRole::Kbm); // le clavier et la souris seuls, sans passer par les manettes
        ask(&mut app, Target::ShareKbm(0), ShareRole::Spectator);
        app.share_reply(a, Ok(Some(view.clone())));
        assert_eq!(app.activate(Target::ShareKick(0)), vec![Action::Share(a, ShareOp::Kick("g1".into()))]);
        app.share_reply(a, Ok(Some(ShareView { guests: Vec::new(), ..view.clone() })));
        assert_eq!(app.activate(Target::ShareNewCode), vec![Action::Share(a, ShareOp::NewCode)]);
        app.share_reply(a, Ok(Some(ShareView { code: "111222".into(), ..view.clone() })));
        // « Copier le lien » : l'adresse du client web, jamais le lien des apps.
        assert_eq!(app.activate(Target::ShareCopy), vec![Action::CopyText("https://h:1/link/?join=482917".into())]);
        // Le panneau se ferme et se rouvre sans redemander l'ouverture ; « Arrêter le partage » le ferme.
        assert!(app.activate(Target::BarShare).is_empty() && app.session().unwrap().bar_menu.is_none());
        assert!(app.activate(Target::BarShare).is_empty() && app.session().unwrap().bar_menu == Some(BarMenu::Share));
        assert_eq!(app.activate(Target::ShareStop), vec![Action::Share(a, ShareOp::Close)]);
        app.share_reply(a, Ok(None));
        assert!(app.session().unwrap().share.is_none() && app.session().unwrap().bar_menu.is_none());
        // Un refus d'Admin reste dans le panneau.
        assert!(!app.activate(Target::BarShare).is_empty());
        app.share_reply(a, Err("Aucun partage en cours.".into()));
        assert_eq!(app.session().unwrap().share_error.as_deref(), Some("Aucun partage en cours."));
    }

    /// À la manette, « Partager » est une tuile du menu : le panneau prend l'image, B le referme ; la liste des
    /// invités du signe de vie le met à jour ; un signe de vie parti avant l'ouverture ne la défait pas.
    #[test]
    fn share_panel_on_a_pad() {
        let mut app = App::new(Store::default(), Layout::Handheld);
        app.store.hosts.push(Host { id: "a".into(), name: "A".into(), ..host() });
        app.live.entry("a".into()).or_default().online = Online::Online;
        app.live.get_mut("a").unwrap().share = Some(ShareAvail::Available);
        let a = open(&mut app, "a");
        app.session_mut().unwrap().menu = true;
        let acts = app.activate(Target::Menu(MenuItem::Share));
        assert_eq!(acts, vec![Action::ReleaseAll(a), Action::Share(a, ShareOp::Open)]);
        let s = app.session().unwrap();
        assert!(s.share_panel && !s.menu && s.share_busy);
        app.share_guests(a, None);
        assert!(app.session().unwrap().share_panel, "le signe de vie d'avant l'ouverture");
        app.share_reply(a, Ok(Some(ShareView { code: "123456".into(), ..Default::default() })));
        assert_eq!(app.focus, Target::ShareNewCode);
        app.share_guests(a, Some(ShareView { code: "123456".into(), guests: vec![ShareGuest { id: "g".into(), name: "Tom".into(), role: ShareRole::Spectator, app: "web".into(), since: None, quality: None }], ..Default::default() }));
        assert_eq!(app.session().unwrap().share.as_ref().map(|v| v.guests.len()), Some(1));
        // B referme le panneau ; l'image reprend (pas le menu).
        assert!(app.session_back().is_empty());
        let s = app.session().unwrap();
        assert!(!s.share_panel && !s.menu);
        // Un signe de vie parti avant l'ouverture, arrivé juste après : il ne la défait pas (8 s).
        app.session_mut().unwrap().share_panel = true;
        app.share_guests(a, None);
        assert!(app.session().unwrap().share_panel && app.session().unwrap().share.is_some());
        // Le partage fermé par ailleurs (Admin), longtemps après son ouverture : le panneau se ferme.
        app.session_mut().unwrap().share_opened = Some(Instant::now() - Duration::from_secs(9));
        app.share_guests(a, None);
        assert!(!app.session().unwrap().share_panel && app.session().unwrap().share.is_none());
        // « Arrêter le partage » : un signe de vie parti avant ne le rouvre pas (3 s), un plus tardif oui.
        let view = ShareView { code: "123456".into(), ..Default::default() };
        app.share_reply(a, Ok(Some(view.clone())));
        app.share_reply(a, Ok(None));
        app.share_guests(a, Some(view.clone()));
        assert!(app.session().unwrap().share.is_none(), "la réponse d'avant l'arrêt ne rouvre rien");
        app.session_mut().unwrap().share_closed = Some(Instant::now() - Duration::from_secs(4));
        app.share_guests(a, Some(view));
        assert!(app.session().unwrap().share.is_some());
    }

    /// Une session d'invité : titrée « Invité sur … », jamais dans la liste des ordinateurs, elle compte pour la limite de
    /// quatre ; le clavier et la souris seulement avec ce rôle ; l'avis du rôle changé ; ni profil ni « Partager ».
    #[test]
    fn a_guest_session() {
        crate::i18n::force_english(false);
        let mut app = desk();
        let id = app.new_session_id();
        app.open_session(SessionUi::guest(id, "share:g1", "Atelier", ShareRole::Spectator));
        let s = app.session().unwrap();
        assert_eq!((s.name.as_str(), s.role(), s.keyboard_mouse()), ("Invité sur Atelier", Some(ShareRole::Spectator), false));
        assert!(app.store.hosts.iter().all(|h| h.id != s.host) && app.session_of_host("a").is_none());
        // Elle compte : trois autres ordinateurs la remplissent.
        for h in ["a", "b", "c"] {
            open(&mut app, h);
        }
        assert!(app.sessions_full());
        app.activate_session(id);
        // Pas de « Partager » ni de profil pour un invité.
        assert!(app.activate(Target::BarShare).is_empty() && app.session().unwrap().bar_menu.is_none());
        // Le rôle change : l'avis, et ce que le rôle n'ouvre plus se ferme.
        let notice = |app: &App| app.session().unwrap().notice.as_ref().map(|n| n.0.clone());
        app.share_role_changed(id, ShareRole::Player, Some(ShareRole::Spectator));
        assert_eq!(notice(&app).as_deref(), Some("Vous pouvez jouer à la manette."));
        assert!(!app.session().unwrap().keyboard_mouse());
        // Le clavier et la souris donnés par-dessus les manettes : un avis pour le droit donné seulement.
        app.share_role_changed(id, ShareRole::PlayerKbm, Some(ShareRole::Player));
        assert_eq!(notice(&app).as_deref(), Some("Vous avez le clavier et la souris."));
        assert!(app.session().unwrap().keyboard_mouse());
        app.session_mut().unwrap().keys = true;
        app.session_mut().unwrap().relative = true;
        app.session_mut().unwrap().pad_mouse = true;
        // Les manettes reprises : le clavier et la souris restent ouverts (touches, manette en souris).
        app.share_role_changed(id, ShareRole::Kbm, Some(ShareRole::PlayerKbm));
        let s = app.session().unwrap();
        assert_eq!(notice(&app).as_deref(), Some("Les manettes sont reprises."));
        assert!(s.keys && s.pad_mouse && s.keyboard_mouse());
        // Les manettes rendues, puis le clavier et la souris repris : ce qu'ils ouvraient se ferme.
        app.share_role_changed(id, ShareRole::PlayerKbm, Some(ShareRole::Kbm));
        assert_eq!(notice(&app).as_deref(), Some("Vous pouvez jouer à la manette."));
        app.share_role_changed(id, ShareRole::Player, Some(ShareRole::PlayerKbm));
        let s = app.session().unwrap();
        assert_eq!(notice(&app).as_deref(), Some("Le clavier et la souris sont repris."));
        assert!(!s.keys && !s.pad_mouse && !s.keyboard_mouse());
        // Plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de reprise.
        app.share_role_changed(id, ShareRole::Spectator, Some(ShareRole::Player));
        assert_eq!(notice(&app).as_deref(), Some("Vous regardez de nouveau."));
        // Les quatre rôles, de l'un à l'autre : un seul avis ; les deux droits changés d'un coup, le droit donné
        // avant le droit repris, les manettes avant le clavier et la souris ; plus aucun droit : « Vous regardez
        // de nouveau. » à la place de l'avis de reprise.
        use ShareRole::{Kbm, Player, PlayerKbm, Spectator};
        let said = |to: ShareRole, from: ShareRole| to.notice(Some(from));
        assert_eq!(said(Player, Spectator), Some("Vous pouvez jouer à la manette."));
        assert_eq!(said(Kbm, Spectator), Some("Vous avez le clavier et la souris."));
        assert_eq!(said(PlayerKbm, Spectator), Some("Vous pouvez jouer à la manette."), "deux droits donnés : les manettes d'abord");
        assert_eq!(said(PlayerKbm, Kbm), Some("Vous pouvez jouer à la manette."));
        assert_eq!(said(PlayerKbm, Player), Some("Vous avez le clavier et la souris."));
        assert_eq!(said(Kbm, Player), Some("Vous avez le clavier et la souris."), "un droit donné, un repris : le donné d'abord");
        assert_eq!(said(Player, Kbm), Some("Vous pouvez jouer à la manette."), "un droit donné, un repris : le donné d'abord");
        assert_eq!(said(Spectator, Player), Some("Vous regardez de nouveau."));
        assert_eq!(said(Spectator, Kbm), Some("Vous regardez de nouveau."));
        assert_eq!(said(Spectator, PlayerKbm), Some("Vous regardez de nouveau."), "deux droits repris : plus aucun, un seul avis");
        assert_eq!(said(Kbm, PlayerKbm), Some("Les manettes sont reprises."));
        assert_eq!(said(Player, PlayerKbm), Some("Le clavier et la souris sont repris."));
        // Le rôle de départ (avant la connexion) et un rôle inchangé n'ont pas d'avis.
        assert_eq!(Spectator.notice(None), None);
        for r in [Spectator, Player, Kbm, PlayerKbm] {
            assert_eq!(r.notice(Some(r)), None);
        }
        crate::i18n::force_english(true);
        assert_eq!(said(Player, Spectator), Some("You can play with a controller."));
        assert_eq!(said(Kbm, Spectator), Some("You have the keyboard and mouse."));
        assert_eq!(said(Spectator, Player), Some("You are watching again."));
        assert_eq!(said(Kbm, PlayerKbm), Some("The controllers were taken back."));
        assert_eq!(said(Player, PlayerKbm), Some("The keyboard and mouse were taken back."));
        crate::i18n::force_english(false);
        // Les deux droits, mots pour mots : « Joueur » dès qu'il y en a un.
        assert_eq!([Spectator, Player, Kbm, PlayerKbm].map(|r| (r.label(), r.pads(), r.kbm(), r.plays())), [("Spectateur", false, false, false), ("Joueur", true, false, true), ("Joueur", false, true, true), ("Joueur", true, true, true)]);
        for (pads, kbm) in [(false, false), (true, false), (false, true), (true, true)] {
            let r = ShareRole::of(pads, kbm);
            assert_eq!((r.pads(), r.kbm()), (pads, kbm));
            for on in [false, true] {
                assert_eq!((r.with_pads(on).pads(), r.with_pads(on).kbm()), (on, kbm), "{r:?} : les manettes, le clavier et la souris tels quels");
                assert_eq!((r.with_kbm(on).pads(), r.with_kbm(on).kbm()), (pads, on), "{r:?} : le clavier et la souris, les manettes telles quelles");
            }
        }
        // Quitter le partage : « Se déconnecter », pour sa session.
        assert_eq!(app.activate(Target::BarQuit), vec![Action::Disconnect(id)]);
    }

    /// Les mots du panneau : durée, app, pastille, code.
    #[test]
    fn share_words() {
        crate::i18n::force_english(false);
        assert_eq!(duration_label(0), "moins d'une minute");
        assert_eq!(duration_label(59), "moins d'une minute");
        assert_eq!(duration_label(60), "1 min");
        assert_eq!(duration_label(3 * 60 + 40), "3 min");
        assert_eq!(duration_label(59 * 60 + 59), "59 min");
        assert_eq!(duration_label(3600), "1 h 00");
        assert_eq!(duration_label(3600 + 12 * 60), "1 h 12");
        assert_eq!(duration_label(2 * 3600 + 5 * 60), "2 h 05");
        assert_eq!(app_label("ios"), "iPhone ou iPad");
        assert_eq!(app_label("webos"), "Téléviseur LG");
        assert_eq!(app_label("web"), "Navigateur");
        assert_eq!(app_label("tvos"), "Apple TV");
        assert_eq!(app_label("plan9"), "plan9");
        assert_eq!((guests_label(0), guests_label(1), guests_label(2)), ("0 invité".to_string(), "1 invité".to_string(), "2 invités".to_string()));
        assert_eq!(format_code("482917"), "482 917");
        assert_eq!(format_code("12"), "12");
        // Le sous-titre de la tuile « Partager » : les invités, sinon le code, sinon « Code et QR code ».
        let view = |n: usize| ShareView { code: "482917".into(), guests: (0..n).map(|i| ShareGuest { id: format!("g{i}"), name: "Léa".into(), ..Default::default() }).collect(), ..Default::default() };
        assert_eq!(share_tile(None), ("Code et QR code".to_string(), false));
        assert_eq!(share_tile(Some(&view(0))), ("482 917".to_string(), true));
        assert_eq!(share_tile(Some(&view(1))), ("1 invité".to_string(), true));
        assert_eq!(share_tile(Some(&view(3))), ("3 invités".to_string(), true));
        let g = ShareGuest { id: "g".into(), name: "Léa".into(), role: ShareRole::PlayerKbm, app: "ios".into(), since: Some(1_000), quality: None };
        assert_eq!(g.line(), "Léa · Joueur, clavier et souris");
        assert_eq!(g.meta(1_000 + 4 * 60), "iPhone ou iPad · depuis 4 min");
        assert_eq!(g.meta(1_000 + 20), "iPhone ou iPad · depuis moins d'une minute");
        // Une horloge de l'appareil en retard sur celle de la Station : jamais de durée négative.
        assert_eq!(g.meta(1_000 - 90), "iPhone ou iPad · depuis moins d'une minute");
        // Rien de lisible : l'app seule.
        assert_eq!(ShareGuest { since: None, ..g.clone() }.meta(5), "iPhone ou iPad");
        assert_eq!(ShareGuest { since: Some(0), ..g }.meta(5), "iPhone ou iPad");
        crate::i18n::force_english(true);
        assert_eq!(
            (duration_label(20), duration_label(3 * 60), duration_label(3600 + 12 * 60), guests_label(1), guests_label(3), app_label("webos")),
            ("under a minute".to_string(), "3 min".to_string(), "1 h 12".to_string(), "1 guest".to_string(), "3 guests".to_string(), "LG TV".to_string())
        );
        assert_eq!(ShareGuest { id: "g".into(), name: "Léa".into(), role: ShareRole::Player, app: "web".into(), since: Some(100), quality: None }.meta(100 + 20), "Browser · for under a minute");
        assert_eq!(
            [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm].map(|r| r.long_label()),
            ["Spectator", "Player", "Keyboard and mouse", "Player, keyboard and mouse"]
        );
        // Actions sur un invité : deux droits indépendants, chacun donné ou repris, jamais « Faire jouer » ni « Faire regarder ».
        let labels = |r: ShareRole| ShareMove::ALL.map(|m| m.label(r));
        assert_eq!(labels(ShareRole::Spectator), ["Give controllers", "Give keyboard and mouse"]);
        assert_eq!(labels(ShareRole::Player), ["Take back controllers", "Give keyboard and mouse"]);
        assert_eq!(labels(ShareRole::Kbm), ["Give controllers", "Take back keyboard and mouse"]);
        assert_eq!(labels(ShareRole::PlayerKbm), ["Take back controllers", "Take back keyboard and mouse"]);
        crate::i18n::force_english(false);
        assert_eq!(labels(ShareRole::Spectator), ["Donner les manettes", "Donner le clavier et la souris"]);
        assert_eq!(labels(ShareRole::Player), ["Reprendre les manettes", "Donner le clavier et la souris"]);
        assert_eq!(labels(ShareRole::Kbm), ["Donner les manettes", "Reprendre le clavier et la souris"]);
        assert_eq!(labels(ShareRole::PlayerKbm), ["Reprendre les manettes", "Reprendre le clavier et la souris"]);
        // Les libellés d'un invité : « Léa · Spectateur », « · Joueur », « · Clavier et souris », « · Joueur, clavier et souris ».
        let lines = [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm].map(|role| ShareGuest { name: "Léa".into(), role, ..Default::default() }.line());
        assert_eq!(lines, ["Léa · Spectateur", "Léa · Joueur", "Léa · Clavier et souris", "Léa · Joueur, clavier et souris"]);
        crate::i18n::force_english(false);
    }
}
