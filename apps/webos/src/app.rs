//! L'état de l'interface et ce que font les touches : sans rien du
//! téléviseur (ni SDL, ni cœur), pour que les écrans se dessinent aussi sur
//! l'ordinateur de développement (captures, `scripts/preview.sh`).
//!
//! Chaque élément qu'on peut choisir est une `Target` ; les écrans les
//! inscrivent avec leur rectangle à chaque dessin. Les flèches vont à la plus
//! proche dans leur direction (navigation spatiale, comme au focus des
//! téléviseurs), le pointeur de la Magic Remote à celle qu'il survole. Les
//! choix rendent des `Action`, que le pilote du téléviseur (`device`) exécute.

use std::collections::HashMap;
use std::time::Instant;

use tiny_skia::Pixmap;

use crate::i18n::tr;
use crate::model::{Host, Profile, Store};

/// Un rectangle à l'écran (1920 × 1080).
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

/// Une touche de navigation : télécommande, manette (menu), clavier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    /// Bouton Y / △ de la manette, ou le bouton … de la télécommande : options.
    Options,
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
    /// Mode que l'écran virtuel prendra (`3840 × 2160 à 120 Hz`).
    pub ready: Option<String>,
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

/// Le réseau du téléviseur.
#[derive(Clone, Debug, Default)]
pub struct NetInfo {
    pub kind: NetKind,
    /// Vitesse du lien en Mbit/s (Ethernet : 100 sur les LG).
    pub link_mbps: Option<u32>,
}

impl NetInfo {
    /// Plafond de débit de la session : la prise Ethernet des LG s'arrête à
    /// 100 Mbit/s (environ 93 utiles), le décodeur à 95 (moonlight-tv).
    pub fn cap_mbps(&self) -> u32 {
        match self.kind {
            NetKind::Ethernet if self.link_mbps.unwrap_or(100) <= 100 => 80,
            _ => 95,
        }
    }
}

/// Ce que sait faire ce téléviseur.
#[derive(Clone, Debug)]
pub struct TvInfo {
    pub model: String,
    pub webos: String,
    pub panel: (u32, u32),
    pub refresh: u32,
    /// Le décodeur du téléviseur (NDL) est là.
    pub decoder: bool,
    pub av1: bool,
    /// Sortie multicanal de NDL : 0 inconnue, 1 impossible, 2 pas d'appareil
    /// multicanal branché, 3 Sortie son numérique pas en Pass-Through, 4 prête.
    pub multichannel: u8,
}

impl Default for TvInfo {
    fn default() -> Self {
        Self { model: "LG".into(), webos: String::new(), panel: (3840, 2160), refresh: 120, decoder: true, av1: true, multichannel: 0 }
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
    pub codec: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Resume,
    Profile(Profile),
    RemoteMouse,
    PadMouse,
    Keys,
    Stats,
    Gamepads,
    Settings,
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

/// L'interface de la session.
#[derive(Clone, Debug)]
pub struct SessionUi {
    pub host: String,
    pub name: String,
    pub profile: Profile,
    pub phase: Phase,
    pub menu: bool,
    pub keys: bool,
    pub stats: Option<Stats>,
    pub show_stats: bool,
    pub remote_mouse: bool,
    pub pad_mouse: bool,
    /// Capsule passagère (manette branchée, mode changé…).
    pub notice: Option<(String, Instant)>,
    /// Rappeler le mode jeu de la télé (dans le menu).
    pub game_hint: bool,
    pub started: Instant,
}

/// Étapes de l'ajout d'un ordinateur.
#[derive(Clone, Debug, PartialEq)]
pub enum AddStep {
    /// Le code à six chiffres, présenté à chaque Station trouvée.
    Code,
    /// Adresse et port d'Admin, au pavé.
    Address,
    /// Empreinte du certificat à comparer avec celle d'Admin.
    Verify { address: String, port: u16, fingerprint: String },
}

#[derive(Clone, Debug)]
pub struct AddState {
    pub step: AddStep,
    pub code: String,
    /// Stations à qui le code a été présenté : (nom, adresse, accepté ?).
    pub asked: Vec<(String, String)>,
    pub address: String,
    pub message: Option<String>,
    pub busy: bool,
}

/// Une ligne de réglage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Profile,
    Resolution,
    Fps,
    Codec,
    Bitrate,
    GameHint,
    Audio,
    Xbox360,
    Combo,
    BackMenu,
    Reconnect,
    Reliable,
    Remove,
    Language,
    /// Ouvre l'écran de test des manettes.
    Pads,
}

/// Une rubrique des réglages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Picture,
    Sound,
    /// Manettes et Magic Remote, réglages de l'ordinateur.
    Controllers,
    Network,
    Account,
    /// Réglages globaux : langue, téléviseur, réseau du téléviseur, version.
    Tv,
    /// Réglages globaux : manettes branchées et leur test.
    Gamepads,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Picture => tr("Image", "Picture"),
            Section::Sound => tr("Son", "Sound"),
            Section::Controllers => tr("Manettes et Magic Remote", "Controllers and Magic Remote"),
            Section::Network => tr("Réseau", "Network"),
            Section::Account => tr("Compte", "Account"),
            Section::Tv => tr("Ce téléviseur", "This TV"),
            Section::Gamepads => tr("Manettes", "Controllers"),
        }
    }
}

/// Réglages de l'ordinateur choisi.
pub const HOST_SECTIONS: [Section; 5] = [Section::Picture, Section::Sound, Section::Controllers, Section::Network, Section::Account];
/// Réglages globaux (de l'app sur ce téléviseur), sans ordinateur.
pub const APP_SECTIONS: [Section; 2] = [Section::Tv, Section::Gamepads];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
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
    /// Réglages globaux (bouton de l'accueil).
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
    Pad(usize),
    PadTest(u8),
    Menu(MenuItem),
    Shortcut(usize),
    CancelConnect,
    HintDismiss,
    Back,
}

/// Ce que l'app demande au pilote du téléviseur.
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
    RemoteMouse(bool),
    PadMouse(bool),
    ReleaseAll,
    Disconnect,
    CancelConnect,
    Language,
    Quit,
}

pub struct App {
    pub store: Store,
    pub screen: Screen,
    pub focus: Target,
    /// Les éléments choisissables du dernier dessin.
    pub hits: Vec<(Target, Rect)>,
    /// Pointeur de la Magic Remote (hors session), s'il est montré.
    pub pointer: Option<(f32, f32)>,
    pub live: HashMap<String, HostLive>,
    /// Image de chaque ordinateur (dernière session, sinon aperçu d'Admin).
    pub images: HashMap<String, Pixmap>,
    pub found: Vec<Found>,
    pub tv: TvInfo,
    pub net: NetInfo,
    pub pads: Vec<PadInfo>,
    pub add: AddState,
    pub section: usize,
    /// Écran des réglages : globaux (vrai) ou de l'ordinateur choisi.
    pub settings_global: bool,
    pub pad_selected: usize,
    pub session: Option<SessionUi>,
    pub toast: Option<(String, Instant)>,
    pub version: &'static str,
    /// L'écran d'où l'on vient (Réglages et Manettes ouverts depuis la session).
    pub back_to: Screen,
}

impl App {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            screen: Screen::Home,
            focus: Target::Connect,
            hits: Vec::new(),
            pointer: None,
            live: HashMap::new(),
            images: HashMap::new(),
            found: Vec::new(),
            tv: TvInfo::default(),
            net: NetInfo::default(),
            pads: Vec::new(),
            add: AddState { step: AddStep::Code, code: String::new(), asked: Vec::new(), address: String::new(), message: None, busy: false },
            section: 0,
            settings_global: false,
            pad_selected: 0,
            session: None,
            toast: None,
            version: env!("CARGO_PKG_VERSION"),
            back_to: Screen::Home,
        }
    }

    /// L'ordinateur choisi sur l'accueil.
    pub fn selected(&self) -> Option<&Host> {
        let id = self.store.selected.as_deref();
        id.and_then(|id| self.store.host(id)).or_else(|| self.store.hosts.first())
    }


    /// Les Stations trouvées qui ne sont pas déjà ajoutées.
    pub fn new_found(&self) -> Vec<&Found> {
        self.found.iter().filter(|f| !self.store.hosts.iter().any(|h| h.address == f.address && h.port == f.port)).collect()
    }

    /// Les rubriques de l'écran des réglages ouvert.
    pub fn sections(&self) -> &'static [Section] {
        if self.settings_global { &APP_SECTIONS } else { &HOST_SECTIONS }
    }

    /// La rubrique ouverte.
    pub fn current_section(&self) -> Section {
        let s = self.sections();
        s[self.section.min(s.len() - 1)]
    }

    /// Ouvre les réglages : globaux, ou ceux de l'ordinateur choisi.
    pub fn open_settings(&mut self, global: bool, from: Screen) {
        self.settings_global = global;
        self.section = 0;
        self.back_to = from;
        self.go(Screen::Settings);
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    pub fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.focus = match screen {
            Screen::Home => {
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
                AddStep::Address => Target::Key('1'),
                AddStep::Verify { .. } => Target::VerifyOk,
            },
            Screen::Settings => Target::Section(self.section),
            Screen::Gamepads => Target::Pad(self.pad_selected.min(self.pads.len().saturating_sub(1))),
            Screen::Session => Target::Menu(MenuItem::Resume),
        };
    }

    /// Pointeur de la Magic Remote déplacé : le focus suit ce qu'il survole.
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        self.pointer = Some((x, y));
        if let Some((t, _)) = self.hits.iter().rev().find(|(_, r)| r.contains(x, y)) {
            if self.focus != *t {
                self.focus = *t;
            }
        }
        true
    }

    /// Clic du pointeur : choisit ce qui est dessous.
    pub fn pointer_click(&mut self, x: f32, y: f32) -> Vec<Action> {
        if let Some((t, _)) = self.hits.iter().rev().find(|(_, r)| r.contains(x, y)).copied() {
            self.focus = t;
            return self.activate(t);
        }
        Vec::new()
    }

    /// Une touche de navigation.
    pub fn nav(&mut self, n: Nav) -> Vec<Action> {
        self.pointer = None;
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
                Vec::new()
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
            Nav::Left | Nav::Right if matches!(self.focus, Target::Setting(_)) => {
                let Target::Setting(s) = self.focus else { return Vec::new() };
                self.adjust(s, if n == Nav::Right { 1 } else { -1 })
            }
            Nav::Left if matches!(self.focus, Target::Setting(_) | Target::Pad(_)) => Vec::new(),
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
            // Dans la bonne direction, et franchement (pas un voisin de la même rangée).
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
            // Les éléments alignés passent devant ; l'écart de travers compte plus que la distance.
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
            Screen::Home => vec![Action::Quit],
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
                if self.screen == Screen::Settings && matches!(self.focus, Target::Setting(_)) {
                    self.focus = Target::Section(self.section);
                    return Vec::new();
                }
                let to = self.back_to;
                self.back_to = Screen::Home;
                if to == Screen::Session && self.session.is_some() {
                    let item = if self.screen == Screen::Settings { MenuItem::Settings } else { MenuItem::Gamepads };
                    self.screen = Screen::Session;
                    self.focus = Target::Menu(item);
                } else if to == Screen::Settings && self.screen == Screen::Gamepads {
                    self.back_to = Screen::Home;
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

    /// Ouvre le menu de session (bouton Retour, combinaison de la manette).
    pub fn open_menu(&mut self) -> Vec<Action> {
        if let Some(s) = self.session.as_mut()
            && !s.menu
            && matches!(s.phase, Phase::Streaming | Phase::Reconnecting { .. })
        {
            s.menu = true;
            s.keys = false;
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
                Vec::new()
            }
            Target::Key(c) => {
                if self.add.address.len() < 40 {
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
                if let Some((t, _)) = self.hits.iter().find(|(t, _)| matches!(t, Target::Setting(_))) {
                    self.focus = *t;
                }
                Vec::new()
            }
            Target::Setting(s) => self.adjust(s, 1),
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
            Target::Shortcut(i) => {
                let keys = SHORTCUTS[i].2.to_vec();
                if let Some(s) = self.session.as_mut() {
                    s.keys = false;
                    s.menu = false;
                }
                vec![Action::Shortcut(keys)]
            }
            Target::CancelConnect => vec![Action::CancelConnect],
            Target::HintDismiss => {
                let Some(id) = self.session.as_ref().map(|s| s.host.clone()) else { return Vec::new() };
                if let Some(s) = self.session.as_mut() {
                    s.game_hint = false;
                }
                if let Some(h) = self.store.host_mut(&id) {
                    h.settings.game_mode_hint = false;
                }
                self.focus = Target::Menu(MenuItem::Resume);
                vec![Action::Save]
            }
            Target::Back => self.back(),
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
                vec![Action::SetProfile(p)]
            }
            MenuItem::RemoteMouse => {
                s.remote_mouse = !s.remote_mouse;
                s.menu = false;
                vec![Action::RemoteMouse(s.remote_mouse)]
            }
            MenuItem::PadMouse => {
                s.pad_mouse = !s.pad_mouse;
                s.menu = false;
                vec![Action::PadMouse(s.pad_mouse)]
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
            MenuItem::Gamepads => {
                self.back_to = Screen::Session;
                self.go(Screen::Gamepads);
                Vec::new()
            }
            MenuItem::Settings => {
                self.open_settings(false, Screen::Session);
                Vec::new()
            }
            MenuItem::Disconnect => vec![Action::Disconnect],
        }
    }

    /// Change un réglage (gauche / droite, ou OK pour la valeur suivante).
    fn adjust(&mut self, s: Setting, dir: i32) -> Vec<Action> {
        if s == Setting::Pads {
            self.back_to = Screen::Settings;
            self.go(Screen::Gamepads);
            return Vec::new();
        }
        if s == Setting::Language {
            let order = ["auto", "fr", "en"];
            let i = order.iter().position(|v| *v == self.store.app.language).unwrap_or(0);
            self.store.app.language = order[cycle(i, order.len(), dir)].into();
            return vec![Action::Language, Action::Save];
        }
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
        if s == Setting::Remove {
            self.store.remove(&id);
            self.toast(tr("Ordinateur retiré de ce téléviseur", "Computer removed from this TV"));
            self.go(Screen::Home);
            return vec![Action::Save];
        }
        let panel_h = self.tv.panel.1;
        let Some(h) = self.store.host_mut(&id) else { return Vec::new() };
        let st = &mut h.settings;
        match s {
            Setting::Profile => {
                let i = Profile::ALL.iter().position(|p| *p == st.profile).unwrap_or(0);
                st.profile = Profile::ALL[cycle(i, 3, dir)];
            }
            Setting::Resolution => {
                let mut opts = vec![0u32, 2160, 1440, 1080, 720];
                opts.retain(|v| *v == 0 || *v <= panel_h.max(1080));
                let i = opts.iter().position(|v| *v == st.resolution).unwrap_or(0);
                st.resolution = opts[cycle(i, opts.len(), dir)];
            }
            Setting::Fps => {
                let opts = [0u32, 30, 60, 120];
                let i = opts.iter().position(|v| *v == st.fps).unwrap_or(0);
                st.fps = opts[cycle(i, opts.len(), dir)];
            }
            Setting::Codec => {
                let opts = ["auto", "hevc", "h264", "av1"];
                let i = opts.iter().position(|v| *v == st.codec).unwrap_or(0);
                st.codec = opts[cycle(i, opts.len(), dir)].into();
            }
            Setting::Bitrate => {
                let opts = [0u32, 20, 30, 40, 50, 60, 70, 80, 95];
                let i = opts.iter().position(|v| *v == st.mbps).unwrap_or(0);
                st.mbps = opts[cycle(i, opts.len(), dir)];
            }
            Setting::Audio => {
                let opts = ["auto", "stereo", "surround", "computer"];
                let i = opts.iter().position(|v| *v == st.audio).unwrap_or(0);
                st.audio = opts[cycle(i, opts.len(), dir)].into();
            }
            Setting::GameHint => st.game_mode_hint = !st.game_mode_hint,
            Setting::Xbox360 => st.xbox360 = !st.xbox360,
            Setting::Combo => st.combo = !st.combo,
            Setting::BackMenu => st.back_opens_menu = !st.back_opens_menu,
            Setting::Reconnect => st.reconnect = !st.reconnect,
            Setting::Reliable => st.reliable = !st.reliable,
            Setting::Remove | Setting::Language | Setting::Pads => {}
        }
        vec![Action::Save]
    }

    /// Le mode que la session demandera : définition et fréquence.
    pub fn session_mode(&self, h: &Host) -> (u32, u32, u32) {
        let (pw, ph) = self.tv.panel;
        let height = if h.settings.resolution == 0 { ph } else { h.settings.resolution.min(ph.max(720)) };
        let width = (height as u64 * pw as u64 / ph.max(1) as u64) as u32 & !1;
        let fps = if h.settings.fps == 0 { self.tv.refresh.max(60) } else { h.settings.fps };
        (width, height, fps)
    }

    /// Le format choisi pour un ordinateur : HEVC d'office (AV1 seulement
    /// jusqu'à 60 i/s : les LG le montrent à 60 au-delà).
    pub fn codec_for(&self, h: &Host) -> &'static str {
        let (_, _, fps) = self.session_mode(h);
        match h.settings.codec.as_str() {
            "h264" => "h264",
            "av1" if self.tv.av1 && h.av1 && fps <= 60 => "av1",
            _ => "hevc",
        }
    }
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
    fn spatial_navigation() {
        let mut app = App::new(Store::default());
        app.hits = vec![
            (Target::Connect, Rect::new(100.0, 100.0, 200.0, 80.0)),
            (Target::Chip(Profile::Jeu), Rect::new(340.0, 110.0, 100.0, 60.0)),
            (Target::AddCard, Rect::new(100.0, 600.0, 200.0, 200.0)),
        ];
        app.focus = Target::Connect;
        app.nav(Nav::Right);
        assert_eq!(app.focus, Target::Chip(Profile::Jeu));
        app.nav(Nav::Down);
        assert_eq!(app.focus, Target::AddCard);
        app.nav(Nav::Up);
        assert!(matches!(app.focus, Target::Connect | Target::Chip(_)));
    }

    #[test]
    fn mode_follows_panel() {
        let mut app = App::new(Store::default());
        app.tv.panel = (3840, 2160);
        app.tv.refresh = 120;
        let mut h = Host {
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
        };
        assert_eq!(app.session_mode(&h), (3840, 2160, 120));
        assert_eq!(app.codec_for(&h), "hevc");
        h.settings.fps = 30;
        h.settings.resolution = 1080;
        assert_eq!(app.session_mode(&h), (1920, 1080, 30));
        h.settings.codec = "av1".into();
        assert_eq!(app.codec_for(&h), "av1");
        h.settings.fps = 120;
        assert_eq!(app.codec_for(&h), "hevc");
    }
}
