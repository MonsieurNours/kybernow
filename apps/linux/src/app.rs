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
use std::time::Instant;

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

/// Ce que sait faire cet appareil (« Cet ordinateur », « Prêt à jouer »).
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

    /// Le H.264 décodé par la puce s'arrête à 1080p (60 Hz sur Pi 4, 30 Hz sur Pi 3).
    pub fn h264_hw_hz(&self) -> u32 {
        if self.generation >= 4 { 60 } else { 30 }
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

    /// Le nom du décodeur matériel, pour les étiquettes : « VA-API », « décodeur du Pi ».
    pub fn hw_name(&self) -> &'static str {
        if self.pi.is_some() { tr("décodeur du Pi", "Pi decoder") } else { "VA-API" }
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
    Ending,
    Ended { error: Option<String> },
}

/// Mesures de la dernière seconde.
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
}

/// L'interface de la session.
#[derive(Clone, Debug)]
pub struct SessionUi {
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
    /// Résumé de la session (« Jeu · 1280 × 800 · 90 Hz · HEVC · 30 Mbit/s »).
    pub summary: String,
    /// Déconnexion demandée : l'accueil revient en fondu sous « Déconnexion… ».
    pub ending_since: Option<Instant>,
    /// Première image : les moitiés de l'accueil s'écartent (passage).
    pub opened_at: Option<Instant>,
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
    /// (Réglages de l'ordinateur › Compte) : même champ et même pavé, en
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
    Controllers,
    Inputs,
    Network,
    Account,
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
            Section::Controllers => tr("Manettes", "Controllers"),
            Section::Inputs => tr("Souris et clavier", "Mouse and keyboard"),
            Section::Network => tr("Transport", "Transport"),
            Section::Account => tr("Compte", "Account"),
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

pub const HOST_SECTIONS: [Section; 7] = [Section::Profile, Section::Picture, Section::Sound, Section::Controllers, Section::Inputs, Section::Network, Section::Account];
pub const APP_SECTIONS: [Section; 2] = [Section::App, Section::Gamepads];

/// La forme d'une ligne de réglage.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// Valeurs au choix (segments sur le bureau, ‹ valeur › à la manette) ;
    /// `tags` : une étiquette par valeur, s'il y en a (« décodeur du Pi », « processeur »).
    Choice { options: Vec<String>, current: usize, disabled: Vec<bool>, tags: Vec<Option<Tag>> },
    Toggle(bool),
    /// Ouvre un écran ou lance quelque chose.
    Action(String),
    /// Retirer l'ordinateur.
    Danger(String),
    /// Information seule.
    Info(String),
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
        Some(Tag { text: tr("processeur", "CPU").into(), kind: TagKind::Cpu })
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
    Host(usize),
    Found(usize),
    AddCard,
    Wake,
    HostSettings,
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

/// Ce que l'app demande au pilote.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Connect { host: String, profile: Profile },
    Save,
    /// Présenter le code aux Stations trouvées (et à `address`, si donnée).
    StartPairing,
    StopPairing,
    PairAddress { address: String, port: u16, fingerprint: String },
    Probe { address: String, port: u16 },
    Wake(String),
    Rumble { pad: usize, low: u16, high: u16, triggers: bool },
    Shortcut(Vec<u16>),
    SetProfile(Profile),
    PadMouse(bool),
    Mute(bool),
    Light(bool),
    /// Souris des jeux ou du bureau (barre, Ctrl+Alt+Maj+M).
    ToggleMouse,
    ToggleFullscreen,
    /// Clavier de Steam (`steam://open/keyboard`), ou saisie de texte de SDL.
    OpenKeyboard,
    ReleaseAll,
    Disconnect,
    CancelConnect,
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
    pub session: Option<SessionUi>,
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
}

impl App {
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
            session: None,
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
        self.go(Screen::Settings);
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

    /// L'adresse MAC de l'ordinateur est en cours de saisie.
    pub fn typing_mac(&self) -> bool {
        self.screen == Screen::Add && self.add.step == AddStep::Mac
    }

    /// Le mot de passe d'un réseau Wi-Fi est en cours de saisie.
    pub fn typing_password(&self) -> bool {
        self.screen == Screen::Setup && self.setup.step == SetupStep::Network && self.setup.wifi_pick.is_some()
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
        if let Some(s) = self.session.as_mut() {
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
        match n {
            Nav::Ok => {
                let t = self.focus;
                self.activate(t)
            }
            Nav::Back => self.back(),
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
                        if let Some(host) = self.store.host_mut(&h.0) {
                            host.settings.profile = p;
                        }
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
                if self.screen == Screen::Add && matches!(self.add.step, AddStep::Address | AddStep::Mac) || self.typing_password() {
                    return self.activate(Target::Key(c));
                }
                Vec::new()
            }
            Nav::Erase => {
                if self.screen == Screen::Add && matches!(self.add.step, AddStep::Address | AddStep::Mac) || self.typing_password() {
                    return self.activate(Target::KeyErase);
                }
                Vec::new()
            }
            Nav::Left | Nav::Right if matches!(self.focus, Target::Setting(_)) && self.layout.console() => {
                let Target::Setting(s) = self.focus else { return Vec::new() };
                self.adjust(s, if n == Nav::Right { 1 } else { -1 })
            }
            Nav::Left | Nav::Right if self.screen == Screen::Home && matches!(self.focus, Target::Connect) && self.layout.console() && self.store.hosts.len() > 1 => {
                // Accueil à la manette : ‹ › changent d'ordinateur.
                let i = self.selected_index().unwrap_or(0);
                let j = cycle(i, self.store.hosts.len(), if n == Nav::Right { 1 } else { -1 });
                self.store.selected = Some(self.store.hosts[j].id.clone());
                vec![Action::Save]
            }
            _ => {
                self.move_focus(n);
                Vec::new()
            }
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
                if to == Screen::Session && self.session.is_some() {
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
                if to == Screen::Session && self.session.is_some() {
                    self.screen = Screen::Session;
                    self.focus = Target::Menu(MenuItem::Gamepads);
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
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        match s.phase {
            Phase::Preparing | Phase::Connecting | Phase::FirstImages => vec![Action::CancelConnect],
            Phase::Ended { .. } => {
                self.session = None;
                self.go(Screen::Home);
                Vec::new()
            }
            _ => {
                if s.keys {
                    s.keys = false;
                    self.focus = Target::Menu(MenuItem::Keys);
                    return Vec::new();
                }
                s.menu = !s.menu;
                if s.menu {
                    self.focus = Target::Menu(MenuItem::Resume);
                    vec![Action::ReleaseAll]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Ouvre le menu de session (combinaison de la manette, toucher en haut).
    pub fn open_menu(&mut self) -> Vec<Action> {
        if let Some(s) = self.session.as_mut()
            && !s.menu
            && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. })
        {
            s.menu = true;
            s.keys = false;
            s.bar = false;
            s.bar_menu = None;
            self.screen = Screen::Session;
            self.focus = Target::Menu(MenuItem::Resume);
            return vec![Action::ReleaseAll];
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
                if let Some(h) = self.store.host_mut(&id) {
                    h.settings.profile = p;
                }
                vec![Action::Save]
            }
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
            Target::AddByAddress => {
                self.add.step = AddStep::Address;
                self.add.message = None;
                self.go(Screen::Add);
                vec![Action::OpenKeyboard]
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
            Target::Wake => self.selected().map(|h| vec![Action::Wake(h.id.clone())]).unwrap_or_default(),
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
                if let Some(s) = self.session.as_mut() {
                    s.keys = false;
                    s.menu = false;
                    s.bar_menu = None;
                }
                vec![Action::Shortcut(keys)]
            }
            Target::CancelConnect => vec![Action::CancelConnect],
            Target::Back => self.back(),
            Target::WinMinimize => vec![Action::Minimize],
            Target::WinMaximize => vec![Action::Maximize],
            Target::WinClose => vec![Action::Quit],
            Target::BarProfile => {
                if let Some(s) = self.session.as_mut() {
                    s.bar_menu = if s.bar_menu == Some(BarMenu::Profile) { None } else { Some(BarMenu::Profile) };
                }
                Vec::new()
            }
            Target::BarKeys => {
                if let Some(s) = self.session.as_mut() {
                    s.bar_menu = if s.bar_menu == Some(BarMenu::Keys) { None } else { Some(BarMenu::Keys) };
                }
                Vec::new()
            }
            Target::BarPick(p) => {
                if let Some(s) = self.session.as_mut() {
                    s.profile = p;
                    s.bar_menu = None;
                }
                self.remember_profile(p);
                vec![Action::SetProfile(p), Action::Save]
            }
            Target::BarMouse => vec![Action::ToggleMouse],
            Target::BarFullscreen => vec![Action::ToggleFullscreen],
            Target::BarQuit => vec![Action::Disconnect],
            Target::UpdateNotice => match &self.update {
                UpdateState::Available { page, .. } => vec![Action::OpenUrl(page.clone())],
                _ => Vec::new(),
            },
        }
    }

    /// Le profil choisi en session est gardé pour cet ordinateur.
    fn remember_profile(&mut self, p: Profile) {
        if let Some(id) = self.session.as_ref().map(|s| s.host.clone())
            && let Some(h) = self.store.host_mut(&id)
        {
            h.settings.profile = p;
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
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        match item {
            MenuItem::Resume => {
                s.menu = false;
                Vec::new()
            }
            MenuItem::Profile(p) => {
                s.profile = p;
                s.menu = false;
                self.remember_profile(p);
                vec![Action::SetProfile(p), Action::Save]
            }
            MenuItem::Sound => {
                s.muted = !s.muted;
                vec![Action::Mute(s.muted)]
            }
            MenuItem::PadMouse => {
                s.pad_mouse = !s.pad_mouse;
                s.menu = false;
                vec![Action::PadMouse(s.pad_mouse)]
            }
            MenuItem::Keyboard => {
                s.menu = false;
                vec![Action::OpenKeyboard]
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
                vec![Action::Light(s.light)]
            }
            MenuItem::Gamepads => {
                self.back_to = Screen::Session;
                self.go(Screen::Gamepads);
                Vec::new()
            }
            MenuItem::Disconnect => vec![Action::Disconnect],
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
        if section == Section::App {
            let lang = ["auto", "fr", "en"].iter().position(|v| *v == self.store.app.language).unwrap_or(0);
            let iface = ["auto", "desktop", "handheld", "tv"].iter().position(|v| *v == self.store.app.interface).unwrap_or(0);
            let update = match &self.update {
                UpdateState::Unknown => String::new(),
                UpdateState::Checking => tr("Recherche…", "Checking…").into(),
                UpdateState::UpToDate => format!("{} {}", tr("À jour :", "Up to date:"), self.version),
                UpdateState::Available { version, .. } => format!("{} {version} {}", tr("Version", "Version"), tr("disponible", "available")),
                UpdateState::Failed(e) => e.clone(),
            };
            return vec![
                choice(
                    Setting::Interface,
                    tr("Interface", "Interface"),
                    tr("Auto : console sous gamescope (mode Jeu de SteamOS), bureau ailleurs", "Auto: console under gamescope (SteamOS Gaming Mode), desktop elsewhere").into(),
                    vec!["Auto".into(), tr("Bureau", "Desktop").into(), tr("Portable", "Handheld").into(), tr("Salon", "Living room").into()],
                    iface,
                    vec![false; 4],
                ),
                choice(Setting::Language, tr("Langue", "Language"), tr("Auto : celle du système", "Auto: the system's").into(), vec!["Auto".into(), "Français".into(), "English".into()], lang, vec![false; 3]),
                toggle(Setting::KeepImage, tr("Garder la dernière image", "Keep the last picture"), tr("L'accueil montre la dernière image de chaque session", "The home screen shows the last picture of each session").into(), self.store.app.keep_last_image),
                choice(
                    Setting::UpdateChannel,
                    tr("Versions proposées", "Offered versions"),
                    if flatpak() {
                        tr("Le Flatpak ne se met pas à jour seul : l'app prévient quand une version est publiée", "The Flatpak does not update itself: the app tells you when a version is published").into()
                    } else {
                        tr("Le paquet .deb ne se met pas à jour seul : l'app prévient quand une version est publiée", "The .deb package does not update itself: the app tells you when a version is published").into()
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
                tr("Aucune manette vue : branchez-la ou associez-la en Bluetooth", "No controller seen: plug it in or pair it over Bluetooth").to_string()
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
            Section::Profile => vec![choice(
                Setting::Profile,
                tr("Profil de la session", "Session profile"),
                tr("Jeu : latence la plus basse · Bureau : texte net · Max : la meilleure image", "Game: lowest latency · Desktop: sharp text · Max: best picture").into(),
                vec![tr("Jeu", "Game").into(), tr("Bureau", "Desktop").into(), "Max".into()],
                Profile::ALL.iter().position(|p| *p == st.profile).unwrap_or(0),
                vec![false; 3],
            )],
            Section::Picture => {
                let (_, dh) = m.display;
                let pi = m.pi.as_ref();
                let res = [0u32, 2160, 1440, 1080, 800, 720];
                let res_labels: Vec<String> = res.iter().map(|r| if *r == 0 { format!("{} · {}", tr("Comme cet écran", "Like this screen"), Machine::height_label(dh)) } else { Machine::height_label(*r) }).collect();
                // Au-delà de ce que la sortie affiche (Pi 3 : 1080p), grisé.
                let res_off: Vec<bool> = res.iter().map(|r| *r > m.max_height()).collect();
                let height = if st.resolution == 0 { dh } else { st.resolution };
                let fps = [0u32, 30, 60, 90, 120, 144];
                let fps_labels: Vec<String> = fps.iter().map(|f| if *f == 0 { format!("{} · {} Hz", tr("Écran", "Screen"), m.refresh) } else { format!("{f} Hz") }).collect();
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
                    (Some(_), _) => tr("Pas de fréquence variable sur la sortie HDMI du Pi · Immédiat : chaque image dès qu'elle est décodée", "No variable refresh on the Pi's HDMI output · Immediate: each frame as soon as it is decoded"),
                    (None, Some(true)) => tr("Fréquence variable : l'écran suit la cadence du flux · Immédiat : déchirure permise", "Variable refresh: the screen follows the stream's pace · Immediate: tearing allowed"),
                    (None, Some(false)) => tr("Cet écran n'annonce pas la fréquence variable · Immédiat : chaque image dès qu'elle est décodée", "This screen does not announce variable refresh · Immediate: each frame as soon as it is decoded"),
                    (None, None) => tr("Fréquence variable : quand l'écran et le compositeur la permettent", "Variable refresh: when the screen and compositor allow it"),
                };
                let bitrates = BITRATES;
                let br_labels: Vec<String> = bitrates.iter().map(|b| if *b == 0 { "Auto".into() } else { format!("{b} Mbit/s") }).collect();
                let mut res_row = choice(Setting::Resolution, tr("Écran", "Screen"), picture_hints(m).0, res_labels, res.iter().position(|v| *v == st.resolution).unwrap_or(0), res_off);
                if pi.is_none() {
                    res_row.label = tr("Écran de l'ordinateur", "Computer's screen").into();
                }
                let mut fps_row = choice(Setting::Fps, tr("Fréquence", "Refresh rate"), picture_hints(m).1, fps_labels, fps.iter().position(|v| *v == st.fps).unwrap_or(0), fps_off);
                if let RowKind::Choice { tags, .. } = &mut fps_row.kind {
                    *tags = fps_tags;
                }
                let mut codec_row = choice(Setting::Codec, tr("Format", "Format"), picture_hints(m).2, vec!["Auto".into(), "HEVC".into(), "H.264".into(), "AV1".into()], codecs.iter().position(|v| *v == st.codec).unwrap_or(0), codec_off);
                if let RowKind::Choice { tags, .. } = &mut codec_row.kind {
                    *tags = codec_tags;
                }
                let mut rows = vec![
                    res_row,
                    fps_row,
                    codec_row,
                    Row {
                        setting: Setting::Yuv444,
                        label: tr("Couleurs 4:4:4", "4:4:4 colour").into(),
                        hint: yuv444_hint(m, hevc).into(),
                        kind: RowKind::Toggle(st.yuv444 && hevc),
                        enabled: hevc,
                        tag: if hevc { who(m.hw_hevc444) } else { None },
                    },
                    choice(
                        Setting::Display,
                        tr("Affichage", "Display"),
                        vrr_hint.into(),
                        vec!["Auto".into(), tr("Synchronisé", "Synchronised").into(), tr("Immédiat", "Immediate").into(), tr("Fréquence variable", "Variable refresh").into()],
                        display.iter().position(|v| *v == st.display).unwrap_or(0),
                        vec![false, false, false, m.vrr == Some(false) || pi.is_some()],
                    ),
                    choice(Setting::Bitrate, tr("Débit", "Bitrate"), tr("Auto : proportionné aux pixels par seconde du profil", "Auto: in proportion to the profile's pixels per second").into(), br_labels, bitrates.iter().position(|v| *v == st.mbps).unwrap_or(0), vec![false; bitrates.len()]),
                    toggle(Setting::VirtualDisplay, tr("Écran virtuel", "Virtual display"), tr("Créé par la Station au début de la session ; sinon, les écrans de l'ordinateur tels quels", "Created by the Station when the session starts; otherwise the computer's screens as they are").into(), st.virtual_display),
                ];
                if m.battery.is_some() {
                    rows.push(toggle(Setting::BatteryLight, tr("Alléger sur batterie", "Lighter on battery"), tr("Sur batterie : 60 images par seconde au plus, débit réduit", "On battery: 60 frames per second at most, lower bitrate").into(), st.battery_light));
                }
                rows
            }
            Section::Sound => {
                let audio = ["auto", "stereo", "5.1", "7.1", "computer"];
                let off: Vec<bool> = audio.iter().map(|a| (*a == "5.1" && m.audio_channels < 6) || (*a == "7.1" && m.audio_channels < 8)).collect();
                vec![choice(
                    Setting::Audio,
                    tr("Son", "Sound"),
                    format!("{} {} : {} {}", tr("Auto suit la sortie", "Auto follows the output"), m.audio_name, m.audio_channels, tr("voies", "channels")),
                    vec!["Auto".into(), tr("Stéréo", "Stereo").into(), "5.1".into(), "7.1".into(), tr("Sur l'ordinateur", "On the computer").into()],
                    audio.iter().position(|v| *v == st.audio).unwrap_or(0),
                    off,
                )]
            }
            Section::Controllers => vec![
                choice(
                    Setting::Xbox360,
                    tr("Manette vue par l'ordinateur", "Gamepad seen by the computer"),
                    tr("Xbox 360 : reconnue par tous les jeux · Fidèle : le modèle branché", "Xbox 360: recognised by every game · Faithful: the actual model").into(),
                    vec!["Xbox 360".into(), tr("Fidèle", "Faithful").into()],
                    (!st.xbox360) as usize,
                    vec![false; 2],
                ),
                toggle(Setting::Combo, tr("L1 + R1 + Start + Select", "L1 + R1 + Start + Select"), tr("Tenus ensemble : ouvrent le menu de session", "Held together: open the session menu").into(), st.combo),
                Row {
                    setting: Setting::Pads,
                    label: tr("Tester les manettes", "Test the controllers").into(),
                    hint: if self.pads.is_empty() { tr("Aucune manette vue", "No controller seen").into() } else { self.pads.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(" · ") },
                    kind: RowKind::Action(format!("{}", self.pads.len())),
                    enabled: true,
                    tag: None,
                },
            ],
            Section::Inputs => vec![
                choice(
                    Setting::Mouse,
                    tr("Souris", "Mouse"),
                    tr("Auto : souris des jeux quand l'ordinateur cache son pointeur dans un jeu en plein écran", "Auto: game mouse when the computer hides its pointer in a full-screen game").into(),
                    vec!["Auto".into(), tr("Bureau", "Desktop").into(), tr("Jeu", "Game").into()],
                    ["auto", "desktop", "game"].iter().position(|v| *v == st.mouse).unwrap_or(0),
                    vec![false; 3],
                ),
                toggle(Setting::Clipboard, tr("Presse-papiers", "Clipboard"), tr("Texte dans les deux sens", "Text both ways").into(), st.clipboard),
            ],
            Section::Network => vec![
                choice(
                    Setting::Reliable,
                    tr("Transport", "Transport"),
                    tr("Rapide : pertes possibles, image clé redemandée · Fiable : tout arrive, plus de retard", "Fast: losses possible, keyframe requested again · Reliable: everything arrives, more delay").into(),
                    vec![tr("Rapide", "Fast").into(), tr("Fiable", "Reliable").into()],
                    st.reliable as usize,
                    vec![false; 2],
                ),
                toggle(Setting::Reconnect, tr("Reconnexion automatique", "Automatic reconnection"), tr("Quand le réseau coupe en cours de session", "When the network drops during a session").into(), st.reconnect),
            ],
            Section::Account => vec![
                Row {
                    setting: Setting::Remove,
                    label: tr("Compte", "Account").into(),
                    hint: format!("{} · {}", h.user, h.fingerprint.as_deref().map(short_fp).unwrap_or_default()),
                    kind: RowKind::Info(format!("{}:{}", h.address, h.port)),
                    enabled: true,
                    tag: None,
                },
                Row {
                    setting: Setting::WakeMac,
                    label: tr("Réveil par le réseau", "Wake on LAN").into(),
                    hint: match &h.mac {
                        Some(m) => format!("{m} · {}", tr("« Réveiller l'ordinateur » apparaît sur l'accueil quand il est hors ligne", "“Wake the computer” appears on the home screen when it is offline")),
                        None => tr("L'adresse MAC de la carte réseau de l'ordinateur : « Réveiller l'ordinateur » apparaît sur l'accueil quand il est hors ligne", "The computer's network card MAC address: “Wake the computer” appears on the home screen when it is offline").into(),
                    },
                    kind: RowKind::Action(if h.mac.is_some() { tr("Changer", "Change") } else { tr("Saisir", "Enter") }.into()),
                    enabled: true,
                    tag: None,
                },
                Row {
                    setting: Setting::Remove,
                    label: tr("Retirer l'ordinateur", "Remove the computer").into(),
                    hint: tr("Son compte sur cet appareil se retire aussi dans Admin › Appareils Link", "Its account on this device is also removed in Admin › Link devices").into(),
                    kind: RowKind::Danger(tr("Retirer", "Remove").into()),
                    enabled: true,
                    tag: None,
                },
            ],
            _ => Vec::new(),
        }
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
            return vec![Action::OpenKeyboard];
        }
        if s == Setting::Remove {
            self.store.remove(&id);
            self.toast(tr("Ordinateur retiré de cet appareil", "Computer removed from this device"));
            self.go(Screen::Home);
            return vec![Action::Save];
        }
        let Some(h) = self.store.host_mut(&id) else { return Vec::new() };
        let st = &mut h.settings;
        let pick = |opts: &[&str], i: usize| opts[i.min(opts.len() - 1)].to_string();
        match s {
            Setting::Profile => st.profile = Profile::ALL[i.min(2)],
            Setting::Resolution => st.resolution = [0u32, 2160, 1440, 1080, 800, 720][i.min(5)],
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
        let (pw, ph) = self.machine.display;
        let (pw, ph) = (pw.max(640), ph.max(480));
        let height = if h.settings.resolution == 0 { ph } else { h.settings.resolution };
        let width = ((height as u64 * pw as u64 / ph as u64) as u32) & !1;
        let mut fps = if h.settings.fps == 0 { self.machine.refresh.max(30) } else { h.settings.fps };
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
        let screen = tr("Écran virtuel créé par la Station au début de la session", "Virtual display created by the Station when the session starts").to_string();
        let fps = tr("Images par seconde demandées à l'ordinateur", "Frames per second asked of the computer").to_string();
        let codec = if m.vaapi && m.hw_any() { format!("{} · {}", tr("Ceux que la carte décode par VA-API", "Those the GPU decodes through VA-API"), decoders_label(m)) } else { tr("Pas de décodage par la carte : le processeur décode H.264 et HEVC", "No GPU decoding: the CPU decodes H.264 and HEVC").into() };
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

/// Ce que dit la ligne 4:4:4 : qui décode, et ce qu'il en coûte.
fn yuv444_hint(m: &Machine, hevc: bool) -> &'static str {
    if !hevc {
        tr("En HEVC seulement", "HEVC only")
    } else if m.pi.is_some() {
        tr("Texte plus net. Sur ce Pi, le décodeur ne les prend pas : le processeur décode, 1080p au plus.", "Sharper text. On this Pi the decoder does not take them: the CPU decodes, 1080p at most.")
    } else if m.hw_hevc444 {
        tr("Texte net : la carte décode le 4:4:4, l'app convertit les couleurs (plus de débit)", "Sharp text: the GPU decodes 4:4:4, the app converts the colours (more bitrate)")
    } else {
        tr("Texte net : le processeur décode le 4:4:4 (exigeant au-delà du 1440p)", "Sharp text: the CPU decodes 4:4:4 (demanding above 1440p)")
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
        assert_eq!(app.choose(Setting::WakeMac, usize::MAX), vec![Action::OpenKeyboard]);
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
}
