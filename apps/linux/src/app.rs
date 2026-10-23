//! L'état de l'interface et ce que font les touches, sans SDL ni cœur : les
//! écrans se dessinent aussi sans l'app (captures, `scripts/preview.sh`).
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
    /// VA-API présent.
    pub vaapi: bool,
    /// Écran : définition et fréquence (celles de la fenêtre).
    pub display: (u32, u32),
    pub refresh: u32,
    pub display_name: String,
    /// L'écran branché annonce la fréquence variable (DRM `vrr_capable`) ;
    /// `None` : on ne sait pas (pas de DRM lisible).
    pub vrr: Option<bool>,
    /// Sortie son : voies et nom.
    pub audio_channels: u32,
    pub audio_name: String,
    /// Batterie : pourcentage, sur secteur ou non.
    pub battery: Option<(u32, bool)>,
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
            vaapi: true,
            display: (1280, 800),
            refresh: 60,
            display_name: String::new(),
            vrr: None,
            audio_channels: 2,
            audio_name: "PipeWire".into(),
            battery: None,
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
}

#[derive(Clone, Debug)]
pub struct AddState {
    pub step: AddStep,
    pub code: String,
    /// Stations à qui le code a été présenté : (nom, adresse).
    pub asked: Vec<(String, String)>,
    pub address: String,
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
    Remove,
    Language,
    Interface,
    UpdateChannel,
    CheckUpdate,
    KeepImage,
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
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Profile => tr("Profil", "Profile"),
            Section::Picture => tr("Image", "Picture"),
            Section::Sound => tr("Son", "Sound"),
            Section::Controllers => tr("Manettes", "Controllers"),
            Section::Inputs => tr("Souris et clavier", "Mouse and keyboard"),
            Section::Network => tr("Réseau", "Network"),
            Section::Account => tr("Compte", "Account"),
            Section::App => tr("KyberNow Link", "KyberNow Link"),
            Section::Gamepads => tr("Manettes", "Controllers"),
        }
    }
}

pub const HOST_SECTIONS: [Section; 7] = [Section::Profile, Section::Picture, Section::Sound, Section::Controllers, Section::Inputs, Section::Network, Section::Account];
pub const APP_SECTIONS: [Section; 2] = [Section::App, Section::Gamepads];

/// La forme d'une ligne de réglage.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// Valeurs au choix (segments sur le bureau, ‹ valeur › à la manette).
    Choice { options: Vec<String>, current: usize, disabled: Vec<bool> },
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Lancement animé (interfaces console).
    Launch,
    Home,
    Add,
    Settings,
    Gamepads,
    Session,
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
}

/// Où en est la recherche de mise à jour.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum UpdateState {
    #[default]
    Unknown,
    Checking,
    UpToDate,
    /// Plus récente, signée : version, canal (`stable`, `beta`), page de la version.
    Available { version: String, channel: String, page: String },
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
    /// Le bureau réduit les animations (GNOME, KDE).
    pub reduce_motion: bool,
    /// Le nom de cet appareil pour Admin (« Steam Deck », nom de la machine).
    pub device_name: String,
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
        }
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

    pub fn open_settings(&mut self, global: bool, from: Screen) {
        self.settings_global = global;
        self.section = 0;
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
                AddStep::Address => Target::KeyOk,
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
        };
    }

    fn first_setting(&self) -> Option<Setting> {
        self.rows(self.current_section()).into_iter().find(|r| r.enabled && !matches!(r.kind, RowKind::Info(_))).map(|r| r.setting)
    }

    /// Fin du lancement animé : l'accueil prend la main.
    pub fn launch_done(&mut self) {
        if self.screen == Screen::Launch {
            self.go(Screen::Home);
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
                    Screen::Settings => {
                        let n = self.sections().len();
                        self.section = cycle(self.section, n, dir);
                        self.scroll = 0.0;
                        self.focus = Target::Section(self.section);
                        Vec::new()
                    }
                    _ => Vec::new(),
                }
            }
            Nav::Char(c) => {
                if self.screen == Screen::Add && self.add.step == AddStep::Address {
                    return self.activate(Target::Key(c));
                }
                Vec::new()
            }
            Nav::Erase => {
                if self.screen == Screen::Add && self.add.step == AddStep::Address {
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

    fn back(&mut self) -> Vec<Action> {
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
                AddStep::Code => {
                    self.go(Screen::Home);
                    vec![Action::StopPairing]
                }
                _ => {
                    self.add.step = AddStep::Code;
                    self.add.message = None;
                    self.go(Screen::Add);
                    Vec::new()
                }
            },
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
                if self.add.address.len() < 60 {
                    self.add.address.push(c);
                }
                Vec::new()
            }
            Target::KeyErase => {
                self.add.address.pop();
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
            message: None,
            busy: false,
            started: Instant::now(),
        };
        self.go(Screen::Add);
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
            kind: RowKind::Choice { options, current, disabled },
            enabled: true,
        };
        let toggle = |setting, label: &str, hint: String, on: bool| Row { setting, label: label.to_string(), hint, kind: RowKind::Toggle(on), enabled: true };
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
                    tr("Le Flatpak ne se met pas à jour seul : l'app prévient quand une version est publiée", "The Flatpak does not update itself: the app tells you when a version is published").into(),
                    vec![tr("Stables", "Stable").into(), tr("Aussi les bêtas", "Betas too").into()],
                    self.store.app.beta as usize,
                    vec![false; 2],
                ),
                Row { setting: Setting::CheckUpdate, label: format!("KyberNow Link {}", self.version), hint: update, kind: RowKind::Action(tr("Chercher", "Check").into()), enabled: true },
            ];
        }
        if section == Section::Gamepads {
            let n = self.pads.len();
            let hint = if n == 0 {
                tr("Aucune manette vue : branchez-la ou associez-la en Bluetooth", "No controller seen: plug it in or pair it over Bluetooth").to_string()
            } else {
                self.pads.iter().map(|p| format!("{}{} {}", tr("J", "P"), p.slot + 1, p.name)).collect::<Vec<_>>().join(" · ")
            };
            return vec![Row { setting: Setting::Pads, label: tr("Tester les manettes", "Test the controllers").into(), hint, kind: RowKind::Action(format!("{n}")), enabled: true }];
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
                let (dw, dh) = m.display;
                let res = [0u32, 2160, 1440, 1080, 800, 720];
                let res_labels: Vec<String> = res
                    .iter()
                    .map(|r| match r {
                        0 => format!("{} · {dw} × {dh}", tr("Comme cet écran", "Like this screen")),
                        2160 => "4K".into(),
                        r => format!("{r}p"),
                    })
                    .collect();
                let fps = [0u32, 30, 60, 90, 120, 144];
                let fps_labels: Vec<String> = fps.iter().map(|f| if *f == 0 { format!("{} · {} Hz", tr("Écran", "Screen"), m.refresh) } else { format!("{f}") }).collect();
                let fps_off: Vec<bool> = fps.iter().map(|f| *f > m.refresh.max(60) && *f != 0).collect();
                let codecs = ["auto", "hevc", "h264", "av1"];
                let codec_off: Vec<bool> = codecs.iter().map(|c| *c != "auto" && !m.decodes(c)).collect();
                let display = ["auto", "vsync", "immediate", "vrr"];
                let vrr_hint = match m.vrr {
                    Some(true) => tr("Fréquence variable : l'écran suit la cadence du flux · Immédiat : déchirure permise", "Variable refresh: the screen follows the stream's pace · Immediate: tearing allowed"),
                    Some(false) => tr("Cet écran n'annonce pas la fréquence variable · Immédiat : chaque image dès qu'elle est décodée", "This screen does not announce variable refresh · Immediate: each frame as soon as it is decoded"),
                    None => tr("Fréquence variable : quand l'écran et le compositeur la permettent", "Variable refresh: when the screen and compositor allow it"),
                };
                let bitrates = [0u32, 10, 20, 30, 40, 50, 60, 80, 100, 150];
                let br_labels: Vec<String> = bitrates.iter().map(|b| if *b == 0 { "Auto".into() } else { format!("{b} Mbit/s") }).collect();
                let mut rows = vec![
                    choice(
                        Setting::Resolution,
                        tr("Écran de l'ordinateur", "Computer's screen"),
                        tr("Écran virtuel créé par la Station au début de la session", "Virtual display created by the Station when the session starts").into(),
                        res_labels,
                        res.iter().position(|v| *v == st.resolution).unwrap_or(0),
                        vec![false; res.len()],
                    ),
                    choice(Setting::Fps, tr("Fréquence", "Refresh rate"), tr("Images par seconde demandées à l'ordinateur", "Frames per second asked of the computer").into(), fps_labels, fps.iter().position(|v| *v == st.fps).unwrap_or(0), fps_off),
                    choice(
                        Setting::Codec,
                        tr("Format", "Format"),
                        if m.vaapi && (m.hw_h264 || m.hw_hevc || m.hw_av1) { format!("{} · {}", tr("Ceux que la carte décode par VA-API", "Those the GPU decodes through VA-API"), decoders_label(m)) } else { tr("Pas de décodage par la carte : le processeur décode H.264 et HEVC", "No GPU decoding: the CPU decodes H.264 and HEVC").into() },
                        vec!["Auto".into(), "HEVC".into(), "H.264".into(), "AV1".into()],
                        codecs.iter().position(|v| *v == st.codec).unwrap_or(0),
                        codec_off,
                    ),
                    Row {
                        setting: Setting::Yuv444,
                        label: tr("Couleurs 4:4:4", "4:4:4 colour").into(),
                        hint: tr("Pas encore sur Linux : l'image est en 4:2:0", "Not on Linux yet: the picture is 4:2:0").into(),
                        kind: RowKind::Toggle(false),
                        enabled: false,
                    },
                    choice(
                        Setting::Display,
                        tr("Affichage", "Display"),
                        vrr_hint.into(),
                        vec!["Auto".into(), tr("Synchronisé", "Synchronised").into(), tr("Immédiat", "Immediate").into(), tr("Fréquence variable", "Variable refresh").into()],
                        display.iter().position(|v| *v == st.display).unwrap_or(0),
                        vec![false, false, false, m.vrr == Some(false)],
                    ),
                    choice(Setting::Bitrate, tr("Débit", "Bitrate"), tr("Auto : selon les pixels par seconde du profil", "Auto: from the profile's pixels per second").into(), br_labels, bitrates.iter().position(|v| *v == st.mbps).unwrap_or(0), vec![false; bitrates.len()]),
                    toggle(Setting::VirtualDisplay, tr("Écran virtuel", "Virtual display"), tr("Sinon, les écrans de l'ordinateur tels quels", "Otherwise the computer's screens as they are").into(), st.virtual_display),
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
                    tr("Manette vue par l'ordinateur", "Controller seen by the computer"),
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
                },
                Row {
                    setting: Setting::Remove,
                    label: tr("Retirer l'ordinateur", "Remove the computer").into(),
                    hint: tr("Son compte sur cet appareil se retire aussi dans Admin › Appareils Link", "Its account on this device is also removed in Admin › Link devices").into(),
                    kind: RowKind::Danger(tr("Retirer", "Remove").into()),
                    enabled: true,
                },
            ],
            Section::App | Section::Gamepads => Vec::new(),
        }
    }

    /// Change un réglage (gauche / droite, ou OK pour la valeur suivante).
    fn adjust(&mut self, s: Setting, dir: i32) -> Vec<Action> {
        let row = self.sections().iter().flat_map(|sec| self.rows(*sec)).find(|r| r.setting == s && !matches!(r.kind, RowKind::Info(_)));
        if let Some(row) = row
            && let RowKind::Choice { options, current, disabled } = &row.kind
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
            Setting::Yuv444 => return Vec::new(),
            _ => {}
        }
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
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
            Setting::Bitrate => st.mbps = [0u32, 10, 20, 30, 40, 50, 60, 80, 100, 150][i.min(9)],
            Setting::Audio => st.audio = pick(&["auto", "stereo", "5.1", "7.1", "computer"], i),
            Setting::Mouse => st.mouse = pick(&["auto", "desktop", "game"], i),
            Setting::Xbox360 => st.xbox360 = if i == usize::MAX { !st.xbox360 } else { i == 0 },
            Setting::Reliable => st.reliable = if i == usize::MAX { !st.reliable } else { i == 1 },
            Setting::VirtualDisplay => st.virtual_display = !st.virtual_display,
            Setting::BatteryLight => st.battery_light = !st.battery_light,
            Setting::Combo => st.combo = !st.combo,
            Setting::Clipboard => st.clipboard = !st.clipboard,
            Setting::Reconnect => st.reconnect = !st.reconnect,
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

    /// Le format choisi : celui demandé s'il est décodé, sinon HEVC (ou H.264
    /// quand la carte ne décode pas HEVC).
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
