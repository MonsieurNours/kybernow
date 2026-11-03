//! Consoles PlayStation, côté interface (sans le cœur ni SDL : les captures se font sur Mac aussi).
//! Contrat : docs/playstation.md ; textes : docs/link-ui.md, « Consoles PlayStation ».
//!
//! - **Accueil** : une carte par console associée (`Target::Console`), après les ordinateurs ; les consoles du
//!   réseau pas encore associées (« Sur le réseau », `Target::PsFound`). Choisie, la console prend le haut de
//!   l'accueil : son état, son nom, son modèle, le jeu en cours, « Jouer » / « Réveiller et jouer » /
//!   « Associer », la puce du compte (plusieurs comptes), « Auto » ou « Personnalisé », « Réglages ».
//! - **Ajout** : « PlayStation » dans « Ajouter un ordinateur ou une console » (`AddStep::Ps`) : les comptes, les
//!   consoles du réseau, « Envoyer à un appareil » (bureau), « Recevoir d'un autre appareil » ; puis les étapes de
//!   l'association (compte, saisie à la main, code à 8 chiffres).
//! - **Compte** : sur le bureau, la page de connexion de Sony s'ouvre dans le navigateur ; on colle ensuite
//!   l'adresse où Sony a mené (pas de fenêtre web dans l'app). Au salon, sur le Steam Deck et sur KyberNow Link
//!   OS : « Recevoir d'un autre appareil » ou la saisie à la main.
//! - **Réglages** de la console : les mêmes écrans que ceux d'un ordinateur (`Section::PsPicture`,
//!   `Section::PsControls`).
//! - **Partie** : une session de l'app (`SessionUi::console`) : la même connexion, la même barre, les mêmes
//!   statistiques et la même fin que pour un ordinateur, avec ce qui a du sens pour une console.

use std::collections::HashMap;
use std::time::Instant;

use crate::app::{Action, AddStep, App, Confirm, Layout, Phase, Row, RowKind, Screen, Section, SessionId, Setting, Target};
use crate::i18n::{english, tr};
use crate::model::{PsConsole, PsSettings};

/// `Store::selected` d'une console : `ps:<host_id>`.
pub const SELECTED: &str = "ps:";

/// L'état d'une console, vu de cet appareil.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PsState {
    /// Pas encore de réponse (« Recherche… »).
    #[default]
    Unknown,
    Ready,
    Standby,
    Offline,
    /// Absente du réseau, vue par une Station joignable par le relais (« Hors de chez vous »).
    Away,
}

impl PsState {
    pub fn label(self) -> &'static str {
        match self {
            PsState::Ready => tr("Allumée", "On"),
            PsState::Standby => tr("En veille", "In rest mode"),
            PsState::Offline => tr("Hors ligne", "Offline"),
            PsState::Away => tr("Hors de chez vous", "Away from home"),
            PsState::Unknown => tr("Recherche…", "Searching…"),
        }
    }
}

/// Une console qui a répondu à la recherche (réseau, adresse retenue, « Par adresse »).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PsFound {
    pub host_id: String,
    pub name: String,
    /// `PS5` ou `PS4`.
    pub model: String,
    pub state: PsState,
    pub address: String,
    /// La cible de l'association (`ps5_v1`…).
    pub target: String,
    pub running_app: Option<String>,
}

impl PsFound {
    /// La console telle que l'association la garde.
    pub fn console(&self) -> PsConsole {
        PsConsole {
            host_id: self.host_id.to_ascii_uppercase(),
            name: self.name.clone(),
            model: self.model.clone(),
            target: (!self.target.is_empty()).then(|| self.target.clone()),
            addresses: Vec::new(),
            ..Default::default()
        }
    }
}

/// Ce que l'accueil sait d'une console associée.
#[derive(Clone, Debug, Default)]
pub struct PsLive {
    pub state: PsState,
    /// L'adresse où elle a répondu (réseau, VPN).
    pub address: Option<String>,
    pub running_app: Option<String>,
    /// Le réveil envoyé, et quand : « Réveil de la console… », puis la partie quand elle est allumée.
    pub waking: Option<Instant>,
}

/// Un appareil du réseau qui attend un envoi (« Envoyer à un appareil »).
#[derive(Clone, Debug, PartialEq)]
pub struct PsWaiting {
    pub name: String,
    /// `macos`, `tvos`, `webos`, `linux`…
    pub platform: String,
    /// `ip:port`.
    pub addr: String,
}

impl PsWaiting {
    /// « {nom} · Apple TV », « {nom} · LG »…
    pub fn label(&self) -> String {
        let kind = match self.platform.as_str() {
            "macos" => "Mac",
            "ios" => "iPhone",
            "ipados" => "iPad",
            "tvos" => "Apple TV",
            "windows" => "PC",
            "linux" => "Linux",
            "steamdeck" => "Steam Deck",
            "raspberrypi" => "Raspberry Pi",
            "android" => "Android",
            "androidtv" => "Android TV",
            "webos" => "LG",
            other => other,
        };
        format!("{} · {kind}", self.name)
    }
}

/// Les étapes de l'onglet PlayStation de la fenêtre d'ajout.
#[derive(Clone, Debug, PartialEq)]
pub enum PsStep {
    /// Les comptes, les consoles du réseau, « Envoyer à un appareil », « Recevoir d'un autre appareil ».
    Tab,
    /// « Avec quel compte ? » (deux comptes ou plus pas encore associés à cette console).
    WhichAccount { console: String },
    /// Ajouter un compte : se connecter à PSN (bureau), recevoir d'un autre appareil, saisir à la main. `console` :
    /// l'association qui attend ce compte.
    Account { console: Option<String> },
    /// Bureau : la page de Sony est ouverte dans le navigateur ; l'adresse où elle mène est collée ici.
    Paste { console: Option<String> },
    /// « Identifiant du compte » et « Nom ».
    ByHand { console: Option<String> },
    /// Le code à 8 chiffres de la console.
    Code { console: String, account: String },
    /// Renommer un compte.
    Rename { account: String },
    /// « Envoyer à un appareil » : le code à 6 chiffres affiché par l'appareil choisi.
    Send { name: String, addr: String },
    /// « Recevoir d'un autre appareil » : le code de cet appareil, en grand.
    Receive,
}

/// L'onglet PlayStation et ses étapes.
#[derive(Clone, Debug)]
pub struct PsAdd {
    pub step: PsStep,
    /// Les champs de l'étape (code, adresse collée, identifiant et nom, nouveau nom, code d'envoi) ; `field` : celui
    /// où l'on tape.
    pub fields: [String; 2],
    pub field: usize,
    pub message: Option<String>,
    /// Ce qui a réussi (« Comptes et consoles envoyés à {nom}. », « {n} compte(s) et {n} console(s) reçus. »).
    pub done: Option<String>,
    pub busy: bool,
    pub started: Instant,
    /// Les appareils qui attendent un envoi, et s'ils ont été cherchés au moins une fois.
    pub devices: Vec<PsWaiting>,
    pub devices_searched: bool,
    /// « Recevoir d'un autre appareil » : le code de cet appareil ; expiré au bout de 5 minutes.
    pub receive_code: Option<String>,
    pub receive_expired: bool,
    /// Le pavé des lettres en majuscules (manette).
    pub shift: bool,
}

impl Default for PsAdd {
    fn default() -> Self {
        Self {
            step: PsStep::Tab,
            fields: [String::new(), String::new()],
            field: 0,
            message: None,
            done: None,
            busy: false,
            started: Instant::now(),
            devices: Vec::new(),
            devices_searched: false,
            receive_code: None,
            receive_expired: false,
            shift: false,
        }
    }
}

/// Ce que la partie montre en plus d'une session d'ordinateur.
#[derive(Clone, Debug, Default)]
pub struct PsSessionUi {
    pub host_id: String,
    /// La console a ouvert son clavier : le texte de son champ (le bandeau en bas de l'image).
    pub keyboard: Option<String>,
    /// La console ne peut pas diffuser ce qu'elle affiche (contenu protégé).
    pub protected: bool,
    /// Une manette est branchée : clavier et souris ne tiennent plus lieu de manette.
    pub pads: bool,
    /// Statistiques de la console (une fois par seconde) : définition, format, débit demandé, reçu ; perdues et
    /// réparées depuis le début.
    pub width: u32,
    pub height: u32,
    pub hevc: bool,
    pub max_fps: u32,
    pub target_kbps: u32,
    pub fps: f32,
    pub mbps: f32,
    pub lost: u64,
    pub recovered: u64,
    /// La souris capturée, et depuis quand (« Ctrl+Alt+Maj+M pour rendre la souris » un instant après).
    pub captured_at: Option<Instant>,
    /// La partie passe par une Station et le relais.
    pub relayed: bool,
    /// Réveil de la console par le relais : « Réveil de la console… » à l'étape de la connexion.
    pub waking: bool,
}

/// Ce que l'app tient des consoles.
#[derive(Clone, Debug, Default)]
pub struct PsUi {
    pub found: Vec<PsFound>,
    pub live: HashMap<String, PsLive>,
    pub add: PsAdd,
    /// Réglages › Commandes : l'action dont on attend la touche (« Appuyez sur une touche ou cliquez… »).
    pub capture: Option<usize>,
    /// Le nom de chaque touche du clavier de l'appareil (usage HID → lettre de sa disposition), donné par SDL.
    pub key_names: HashMap<u16, String>,
    /// Le choix du compte de la console choisie est ouvert (la puce de l'accueil).
    pub account_menu: bool,
    /// Le message de l'accueil pour la console choisie (« La console ne répond pas. ») : console, texte.
    pub message: Option<(String, String)>,
    /// Les adresses données « Par adresse » où une console a répondu : retenues avec elle.
    pub probed: Vec<String>,
}

// MARK: - Disposition du clavier et de la souris (miroir de `playstation::keymouse::Layout`)

/// Ce qui déclenche une action : une touche (usage HID) ou un bouton de la souris (0 gauche, 1 droit, 2 milieu,
/// 3 et 4 latéraux). Même JSON que le cœur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsInput {
    Key(u16),
    Mouse(u8),
}

/// Les actions de la manette, dans l'ordre du cœur (`Action::ALL`), avec leur nom JSON et leurs libellés.
pub const ACTIONS: [(&str, &str, &str); 26] = [
    ("left_stick_up", "Stick gauche vers le haut", "Left stick up"),
    ("left_stick_down", "Stick gauche vers le bas", "Left stick down"),
    ("left_stick_left", "Stick gauche vers la gauche", "Left stick left"),
    ("left_stick_right", "Stick gauche vers la droite", "Left stick right"),
    ("cross", "Croix", "Cross"),
    ("circle", "Rond", "Circle"),
    ("square", "Carré", "Square"),
    ("triangle", "Triangle", "Triangle"),
    ("l1", "L1", "L1"),
    ("r1", "R1", "R1"),
    ("l2", "L2", "L2"),
    ("r2", "R2", "R2"),
    ("l3", "L3", "L3"),
    ("r3", "R3", "R3"),
    ("dpad_up", "Croix directionnelle vers le haut", "D-pad up"),
    ("dpad_down", "Croix directionnelle vers le bas", "D-pad down"),
    ("dpad_left", "Croix directionnelle vers la gauche", "D-pad left"),
    ("dpad_right", "Croix directionnelle vers la droite", "D-pad right"),
    ("options", "Options", "Options"),
    ("create", "Create", "Create"),
    ("ps", "Bouton PS", "PS button"),
    ("touchpad", "Pavé tactile", "Touchpad"),
    ("right_stick_up", "Stick droit vers le haut", "Right stick up"),
    ("right_stick_down", "Stick droit vers le bas", "Right stick down"),
    ("right_stick_left", "Stick droit vers la gauche", "Right stick left"),
    ("right_stick_right", "Stick droit vers la droite", "Right stick right"),
];

/// La disposition d'une console (JSON stable du cœur).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PsLayout {
    pub bindings: Vec<(PsInput, String)>,
    #[serde(default = "yes")]
    pub mouse_stick: bool,
    #[serde(default = "one")]
    pub sensitivity: f32,
    #[serde(default = "curve")]
    pub curve: f32,
    #[serde(default = "deadzone")]
    pub deadzone: f32,
    #[serde(default)]
    pub invert_y: bool,
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}
fn curve() -> f32 {
    0.8
}
fn deadzone() -> f32 {
    0.2
}

impl Default for PsLayout {
    /// La disposition par défaut du cœur (`keymouse::Layout::default`, vérifiée par un test de la couche appareil).
    fn default() -> Self {
        let k = |u: u16, a: &str| (PsInput::Key(u), a.to_string());
        let m = |b: u8, a: &str| (PsInput::Mouse(b), a.to_string());
        PsLayout {
            bindings: vec![
                k(0x1a, "left_stick_up"),
                k(0x16, "left_stick_down"),
                k(0x04, "left_stick_left"),
                k(0x07, "left_stick_right"),
                k(0x2c, "cross"),
                k(0x28, "cross"),
                k(0x06, "circle"),
                k(0x2a, "circle"),
                k(0x15, "square"),
                k(0x09, "triangle"),
                k(0x14, "l1"),
                k(0x08, "r1"),
                m(1, "l2"),
                m(0, "r2"),
                k(0xe1, "l3"),
                k(0x19, "r3"),
                m(2, "r3"),
                k(0x52, "dpad_up"),
                k(0x51, "dpad_down"),
                k(0x50, "dpad_left"),
                k(0x4f, "dpad_right"),
                k(0x29, "options"),
                k(0x31, "create"),
                k(0x35, "ps"),
                k(0x2b, "touchpad"),
            ],
            mouse_stick: true,
            sensitivity: 1.0,
            curve: curve(),
            deadzone: deadzone(),
            invert_y: false,
        }
    }
}

impl PsLayout {
    pub fn of(c: &PsConsole) -> Self {
        c.layout.as_ref().and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Les touches d'une action.
    pub fn inputs(&self, action: &str) -> Vec<PsInput> {
        self.bindings.iter().filter(|(_, a)| a == action).map(|(i, _)| *i).collect()
    }

    /// Une touche de plus pour cette action ; prise ailleurs, elle y est retirée.
    pub fn bind(&mut self, input: PsInput, action: &str) {
        self.bindings.retain(|(i, _)| *i != input);
        self.bindings.push((input, action.to_string()));
    }

    /// L'action n'a plus de touche.
    pub fn clear(&mut self, action: &str) {
        self.bindings.retain(|(_, a)| a != action);
    }
}

/// Sensibilités proposées pour la souris (de 0,2 à 5, 1 par défaut).
pub const SENSITIVITIES: [f32; 11] = [0.2, 0.3, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 5.0];
/// Courbes : douce, normale, vive.
pub const CURVES: [f32; 3] = [1.2, 1.0, 0.8];
/// Débits proposés (kbit/s ; 0 : Auto), de 2 à 100 Mbit/s.
pub const PS_BITRATES: [u32; 12] = [0, 2_000, 5_000, 10_000, 15_000, 20_000, 30_000, 40_000, 50_000, 60_000, 80_000, 100_000];

/// Le nom d'une touche, par défaut (QWERTY américain) ; l'app montre celui du clavier de l'appareil (`key_names`).
pub fn default_key_name(u: u16) -> String {
    match u {
        0x04..=0x1d => ((b'A' + (u - 0x04) as u8) as char).to_string(),
        0x1e..=0x26 => ((b'1' + (u - 0x1e) as u8) as char).to_string(),
        0x27 => "0".into(),
        0x28 => tr("Entrée", "Return").into(),
        0x29 => tr("Échap", "Escape").into(),
        0x2a => tr("Effacement", "Backspace").into(),
        0x2b => tr("Tab", "Tab").into(),
        0x2c => tr("Espace", "Space").into(),
        0x2d => "-".into(),
        0x2e => "=".into(),
        0x2f => "[".into(),
        0x30 => "]".into(),
        0x31 => "\\".into(),
        0x33 => ";".into(),
        0x34 => "'".into(),
        0x35 => "`".into(),
        0x36 => ",".into(),
        0x37 => ".".into(),
        0x38 => "/".into(),
        0x39 => tr("Verr. maj.", "Caps Lock").into(),
        0x3a..=0x45 => format!("F{}", u - 0x39),
        0x4f => "→".into(),
        0x50 => "←".into(),
        0x51 => "↓".into(),
        0x52 => "↑".into(),
        0xe0 => tr("Ctrl gauche", "Left Ctrl").into(),
        0xe1 => tr("Maj gauche", "Left Shift").into(),
        0xe2 => tr("Alt gauche", "Left Alt").into(),
        0xe4 => tr("Ctrl droit", "Right Ctrl").into(),
        0xe5 => tr("Maj droite", "Right Shift").into(),
        0xe6 => tr("Alt droite", "Right Alt").into(),
        _ => format!("#{u:02X}"),
    }
}

/// « Clic gauche », « Bouton 4 »…
pub fn mouse_name(b: u8) -> String {
    match b {
        0 => tr("Clic gauche", "Left click").into(),
        1 => tr("Clic droit", "Right click").into(),
        2 => tr("Clic du milieu", "Middle click").into(),
        n => format!("{} {}", tr("Bouton", "Button"), n + 1),
    }
}

/// « Retirer le compte {nom} et ses {n} consoles ? » (une : « … et sa console ? » ; aucune : « Retirer le compte
/// {nom} ? »).
pub fn remove_account_question(name: &str, n: usize) -> String {
    match (english(), n) {
        (true, 0) => format!("Remove the account {name}?"),
        (true, 1) => format!("Remove the account {name} and its console?"),
        (true, n) => format!("Remove the account {name} and its {n} consoles?"),
        (false, 0) => format!("Retirer le compte {name} ?"),
        (false, 1) => format!("Retirer le compte {name} et sa console ?"),
        (false, n) => format!("Retirer le compte {name} et ses {n} consoles ?"),
    }
}

/// « {n} console(s) » ou « Aucune console ».
pub fn consoles_label(n: usize) -> String {
    match n {
        0 => tr("Aucune console", "No console").into(),
        1 => tr("1 console", "1 console").into(),
        n => format!("{n} consoles"),
    }
}

/// « {n} compte(s) et {n} console(s) reçus. »
pub fn received_label(accounts: usize, consoles: usize) -> String {
    let s = |n: usize| if n > 1 { "s" } else { "" };
    if english() {
        format!("{accounts} account{} and {consoles} console{} received.", s(accounts), s(consoles))
    } else {
        format!("{accounts} compte{} et {consoles} console{} reçu{}.", s(accounts), s(consoles), if accounts + consoles > 1 { "s" } else { "" })
    }
}

/// Le code à 6 chiffres en deux groupes : « 482 913 ».
pub fn spaced_code(code: &str) -> String {
    if code.len() == 6 { format!("{} {}", &code[..3], &code[3..]) } else { code.to_string() }
}

/// Les textes des fins d'une partie (docs/link-ui.md, « Session »).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsEnd {
    Stopped,
    InUse,
    NotPaired,
    Unreachable,
    Remote,
}

impl PsEnd {
    pub fn text(self) -> Option<&'static str> {
        match self {
            PsEnd::Stopped => None,
            PsEnd::InUse => Some(tr("La console est déjà jouée à distance par un autre appareil.", "The console is already being played remotely by another device.")),
            PsEnd::NotPaired => Some(tr("La console ne reconnaît plus cet appareil : associez-la de nouveau.", "The console no longer recognizes this device: pair it again.")),
            PsEnd::Unreachable => Some(tr("La console ne répond pas.", "The console isn't answering.")),
            PsEnd::Remote => Some(tr("La console a terminé la partie.", "The console ended the session.")),
        }
    }
}

/// Les codes d'échec de l'association (`kn_ps_regist` du pont Apple, mêmes valeurs).
pub fn regist_error_text(code: &str) -> &'static str {
    match code {
        "no_answer" => tr("La console ne répond pas : gardez l'écran « Associer un appareil » ouvert.", "The console isn't answering: keep the “Link Device” screen open."),
        "wrong_code" => tr("Code refusé : il change toutes les 5 minutes.", "Code refused: it changes every 5 minutes."),
        "account" => tr("La console refuse ce compte.", "The console refuses this account."),
        _ => tr("Console injoignable.", "Console unreachable."),
    }
}

/// Ce que l'app demande à la couche appareil pour les consoles.
#[derive(Clone, Debug, PartialEq)]
pub enum PsAction {
    /// Jouer sur la console (réveil d'abord si elle est en veille ; par une Station hors de chez soi).
    Play { host_id: String },
    /// Le compte choisi, puis le code : l'association (`target` : celui de la recherche ; `address` : là où elle a
    /// répondu).
    Pair { console: PsConsole, address: String, account: String, pin: u32 },
    /// Chercher les consoles tout de suite (« Réessayer »).
    Search,
    /// Bureau : la page de connexion de Sony, dans le navigateur.
    SignIn,
    /// L'adresse de retour de Sony, collée : l'identifiant et le pseudo du compte (deux requêtes HTTPS).
    ReadAccount { redirect: String, console: Option<String> },
    /// « Recevoir d'un autre appareil » : écouter (vrai) ou arrêter.
    Receive(bool),
    Send { addr: String, name: String, code: String },
    /// Mettre la console en veille (barre de la partie).
    Rest(SessionId),
    /// La disposition, les intensités ou le plein écran de la console ont changé : la partie en cours les prend.
    Changed { host_id: String },
}

impl crate::app::SessionUi {
    /// Une partie sur une console montre quelque chose par-dessus l'image : contenu protégé, clavier de la
    /// console, aide du clavier et de la souris (bureau, sans manette).
    pub fn console_overlay(&self, layout: Layout) -> bool {
        let Some(ps) = &self.console else { return false };
        ps.protected || ps.keyboard.is_some() || (layout == Layout::Desktop && !ps.pads && !self.bar && ps.captured_at.is_none_or(|t| t.elapsed().as_secs_f32() < 4.2))
    }
}

impl App {
    // MARK: - Choix

    /// La console choisie à l'accueil.
    pub fn selected_console(&self) -> Option<&PsConsole> {
        if let Some(c) = self.store.selected.as_deref().and_then(|s| s.strip_prefix(SELECTED)).and_then(|id| self.store.console(id)) {
            return Some(c);
        }
        // Sans ordinateur (ni choisi, ni gardé) : la première console.
        let chosen = self.store.selected.as_deref().is_some_and(|s| self.store.host(s).is_some() || self.store.borrowed(s).is_some());
        if !chosen && self.store.hosts.is_empty() {
            return self.store.ps_consoles.first();
        }
        None
    }

    /// Les consoles du réseau pas encore associées (avec aucun compte).
    pub fn new_ps_found(&self) -> Vec<&PsFound> {
        self.ps.found.iter().filter(|f| self.store.console(&f.host_id).is_none()).collect()
    }

    pub fn ps_live(&self, host_id: &str) -> PsLive {
        self.ps.live.get(&host_id.to_ascii_uppercase()).cloned().unwrap_or_default()
    }

    /// L'état d'une console associée : celui de la recherche, sinon « Hors de chez vous » si une Station joignable
    /// par le relais la voit, sinon « Hors ligne » une fois la recherche faite.
    pub fn ps_state(&self, host_id: &str) -> PsState {
        self.ps_live(host_id).state
    }

    /// Le compte choisi pour jouer sur cette console (son pseudo), et s'il y en a plusieurs.
    pub fn ps_account_of(&self, c: &PsConsole) -> (String, bool) {
        let name = c.pairing().and_then(|p| self.store.ps_account(&p.account)).map(|a| a.label()).unwrap_or_default();
        (name, c.pairings.len() > 1)
    }

    /// Ce que fait le bouton principal de la console : cible, libellé, symbole ▶.
    pub fn ps_main_action(&self, c: &PsConsole) -> (Target, &'static str, bool) {
        let live = self.ps_live(&c.host_id);
        if c.pairings.is_empty() {
            return (Target::PsPair, tr("Associer", "Pair"), false);
        }
        if live.waking.is_some() {
            return (Target::PsPlay, tr("Réveil de la console…", "Waking the console…"), false);
        }
        match live.state {
            PsState::Standby => (Target::PsPlay, tr("Réveiller et jouer", "Wake and play"), false),
            // Hors ligne : une recherche de plus (comme l'app Mac).
            PsState::Offline => (Target::PsRetry, tr("Réessayer", "Try again"), false),
            _ => (Target::PsPlay, tr("Jouer", "Play"), true),
        }
    }

    // MARK: - Résultats de la couche appareil

    /// Les consoles qui ont répondu (réseau, adresses retenues) : leur état ; les consoles associées qui n'ont pas
    /// répondu sont « Hors ligne » (ou « Hors de chez vous » quand une Station joignable les voit).
    pub fn ps_found(&mut self, found: Vec<PsFound>, away: &[String]) {
        for c in self.store.ps_consoles.clone() {
            let id = c.host_id.to_ascii_uppercase();
            let live = self.ps.live.entry(id.clone()).or_default();
            match found.iter().find(|f| f.host_id.eq_ignore_ascii_case(&id)) {
                Some(f) => {
                    live.state = f.state;
                    live.address = Some(f.address.clone());
                    live.running_app = f.running_app.clone();
                    // Allumée pendant un réveil : la partie part (la couche appareil la lance).
                    if f.state == PsState::Ready {
                        live.waking = None;
                    }
                }
                None if live.waking.is_some() => {}
                None => {
                    live.state = if away.iter().any(|a| a.eq_ignore_ascii_case(&id)) { PsState::Away } else { PsState::Offline };
                    live.running_app = None;
                }
            }
            // Le nom donné sur la console change : l'app le suit.
            if let Some(f) = found.iter().find(|f| f.host_id.eq_ignore_ascii_case(&id))
                && !f.name.is_empty()
                && f.name != c.name
                && let Some(cm) = self.store.console_mut(&id)
            {
                cm.name = f.name.clone();
            }
        }
        self.ps.found = found;
    }

    /// Un réveil qui dure plus de 40 s : la console ne répond pas.
    pub fn ps_tick(&mut self) -> bool {
        let mut changed = false;
        let ids: Vec<String> = self.ps.live.iter().filter(|(_, l)| l.waking.is_some_and(|t| t.elapsed().as_secs() > 40)).map(|(k, _)| k.clone()).collect();
        for id in ids {
            if let Some(l) = self.ps.live.get_mut(&id) {
                l.waking = None;
            }
            self.ps.message = Some((id, tr("La console ne répond pas.", "The console isn't answering.").into()));
            changed = true;
        }
        if self.ps.add.step == PsStep::Receive && !self.ps.add.receive_expired && self.ps.add.started.elapsed().as_secs() >= 300 {
            self.ps.add.receive_expired = true;
            changed = true;
        }
        changed
    }

    /// Le compte lu par la connexion Sony (ou reçu, ou saisi) : gardé, même si l'association échoue ensuite ; puis
    /// le code si une console attend.
    pub fn ps_account_read(&mut self, r: Result<(String, Option<String>), String>, console: Option<String>) -> Vec<Action> {
        self.ps.add.busy = false;
        match r {
            Err(_) => {
                self.ps.add.message = Some(tr("Identifiant du compte non lu : réessayez, ou saisissez-le à la main.", "Couldn't read the account ID: try again, or enter it by hand.").into());
                Vec::new()
            }
            Ok((psn_id, name)) => self.ps_account_ready(&psn_id, name, console),
        }
    }

    /// Un compte prêt (lu, saisi) : gardé, puis le code de la console qui l'attend, sinon l'onglet.
    fn ps_account_ready(&mut self, psn_id: &str, name: Option<String>, console: Option<String>) -> Vec<Action> {
        let (id, existed) = self.store.put_ps_account(psn_id, name, None);
        if existed {
            self.toast(tr("Ce compte est déjà là.", "This account is already here."));
        }
        match console {
            Some(c) => self.ps_open_step(PsStep::Code { console: c, account: id }),
            None => self.ps_open_step(PsStep::Tab),
        }
        vec![Action::Save]
    }

    /// L'association a répondu : gardée (`Ok`), ou le texte de l'échec.
    pub fn ps_paired(&mut self, console: PsConsole, account: &str, r: Result<serde_json::Value, String>) -> Vec<Action> {
        self.ps.add.busy = false;
        match r {
            Ok(registered) => {
                let host_id = console.host_id.to_ascii_uppercase();
                let name = console.name.clone();
                self.store.put_pairing(console, account, registered);
                self.store.selected = Some(format!("{SELECTED}{host_id}"));
                self.ps.live.entry(host_id).or_default();
                self.toast(format!("{name} {}", tr("est associée", "is paired")));
                self.go(Screen::Home);
                vec![Action::Save]
            }
            Err(code) => {
                self.ps.add.message = Some(regist_error_text(&code).into());
                Vec::new()
            }
        }
    }

    /// « Par adresse » : une console a répondu à cette adresse (son association s'ouvre ; l'adresse est retenue),
    /// ou rien (`None` : l'ajout d'un ordinateur continue comme avant).
    pub fn ps_probed(&mut self, address: &str, found: Option<PsFound>) -> Vec<Action> {
        let Some(f) = found else { return Vec::new() };
        let host_id = f.host_id.to_ascii_uppercase();
        if !self.ps.probed.iter().any(|a| a == address) {
            self.ps.probed.push(address.to_string());
        }
        if let Some(c) = self.store.console_mut(&host_id)
            && !c.addresses.iter().any(|a| a == address)
        {
            c.addresses.push(address.to_string());
        }
        if !self.ps.found.iter().any(|x| x.host_id.eq_ignore_ascii_case(&host_id)) {
            self.ps.found.push(f);
        }
        self.add.busy = false;
        self.add.message = None;
        self.ps_pair(&host_id, Some(address.to_string()));
        vec![Action::Save]
    }

    /// Reçu d'un autre appareil : « {n} compte(s) et {n} console(s) reçus. ».
    pub fn ps_received(&mut self, accounts: usize, consoles: usize) -> Vec<Action> {
        self.ps.add.done = Some(received_label(accounts, consoles));
        self.ps.add.receive_code = None;
        vec![Action::Save, Action::Ps(PsAction::Receive(false))]
    }

    /// Un envoi fini : fait, code faux, appareil qui ne répond plus.
    pub fn ps_sent(&mut self, name: &str, r: Result<(), bool>) {
        self.ps.add.busy = false;
        match r {
            Ok(()) => {
                self.ps.add.done = Some(if english() { format!("Accounts and consoles sent to {name}.") } else { format!("Comptes et consoles envoyés à {name}.") });
                self.ps_open_step(PsStep::Tab);
            }
            Err(true) => self.ps.add.message = Some(if english() { format!("Code refused: check it on {name}.") } else { format!("Code refusé : vérifiez-le sur {name}.") }),
            Err(false) => self.ps.add.message = Some(if english() { format!("{name} isn't answering any more.") } else { format!("{name} ne répond plus.") }),
        }
    }

    // MARK: - Onglet PlayStation

    /// Ouvre une étape de l'onglet PlayStation.
    pub fn ps_open_step(&mut self, step: PsStep) {
        let keep_done = matches!(step, PsStep::Tab);
        let done = if keep_done { self.ps.add.done.take() } else { None };
        self.ps.add = PsAdd { step: step.clone(), devices: std::mem::take(&mut self.ps.add.devices), devices_searched: self.ps.add.devices_searched, done, receive_code: self.ps.add.receive_code.take(), ..PsAdd::default() };
        if let PsStep::Rename { account } = &step {
            self.ps.add.fields[0] = self.store.ps_account(account).and_then(|a| a.name.clone()).unwrap_or_default();
        }
        self.add.step = AddStep::Ps(step);
        self.go(Screen::Add);
    }

    /// Associer cette console : le compte d'abord (étape sautée s'il n'y en a qu'un possible), puis le code.
    pub fn ps_pair(&mut self, host_id: &str, address: Option<String>) {
        let host_id = host_id.to_ascii_uppercase();
        if let Some(a) = address {
            self.ps.live.entry(host_id.clone()).or_default().address = Some(a);
        }
        let paired: Vec<String> = self.store.console(&host_id).map(|c| c.pairings.iter().map(|p| p.account.clone()).collect()).unwrap_or_default();
        let free: Vec<String> = self.store.ps_accounts.iter().filter(|a| !paired.contains(&a.id)).map(|a| a.id.clone()).collect();
        match free.len() {
            0 => self.ps_open_step(PsStep::Account { console: Some(host_id) }),
            1 => self.ps_open_step(PsStep::Code { console: host_id, account: free[0].clone() }),
            _ => self.ps_open_step(PsStep::WhichAccount { console: host_id }),
        }
    }

    /// Les comptes qui ne sont pas encore associés à cette console.
    pub fn ps_free_accounts(&self, host_id: &str) -> Vec<&crate::model::PsAccount> {
        let paired: Vec<&str> = self.store.console(host_id).map(|c| c.pairings.iter().map(|p| p.account.as_str()).collect()).unwrap_or_default();
        self.store.ps_accounts.iter().filter(|a| !paired.contains(&a.id.as_str())).collect()
    }

    /// Le champ où l'on tape dans l'étape ouverte.
    pub fn typing_ps(&self) -> bool {
        self.screen == Screen::Add && matches!(&self.add.step, AddStep::Ps(s) if matches!(s, PsStep::Code { .. } | PsStep::Paste { .. } | PsStep::ByHand { .. } | PsStep::Rename { .. } | PsStep::Send { .. }))
    }

    fn ps_step(&self) -> Option<PsStep> {
        match &self.add.step {
            AddStep::Ps(s) if self.screen == Screen::Add => Some(s.clone()),
            _ => None,
        }
    }

    /// Une lettre tapée dans le champ de l'étape (les codes : des chiffres seulement).
    fn ps_type(&mut self, c: char) {
        let Some(step) = self.ps_step() else { return };
        let i = self.ps.add.field.min(1);
        let f = &self.ps.add.fields[i];
        let ok = match step {
            PsStep::Code { .. } => c.is_ascii_digit() && f.len() < 8,
            PsStep::Send { .. } => c.is_ascii_digit() && f.len() < 6,
            PsStep::Paste { .. } => !c.is_control() && f.len() < 4096,
            PsStep::ByHand { .. } if i == 0 => (c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')) && f.len() < 64,
            _ => !c.is_control() && f.chars().count() < 40,
        };
        let c = if self.ps.add.shift && matches!(step, PsStep::ByHand { .. } | PsStep::Rename { .. }) { c.to_ascii_uppercase() } else { c };
        let f = &mut self.ps.add.fields[i];
        if ok {
            f.push(c);
            self.ps.add.message = None;
        }
    }

    /// Du texte collé (l'adresse de Sony).
    pub fn ps_paste(&mut self, text: &str) {
        for c in text.trim().chars() {
            self.ps_type(c);
        }
    }

    fn ps_erase(&mut self) {
        let i = self.ps.add.field.min(1);
        self.ps.add.fields[i].pop();
        self.ps.add.message = None;
    }

    /// « Continuer », « Associer », « Envoyer », « Enregistrer » : selon l'étape.
    fn ps_go(&mut self) -> Vec<Action> {
        let Some(step) = self.ps_step() else { return Vec::new() };
        if self.ps.add.busy {
            return Vec::new();
        }
        let [a, b] = self.ps.add.fields.clone();
        match step {
            PsStep::Code { console, account } => {
                let Ok(pin) = a.parse::<u32>() else { return Vec::new() };
                if a.len() != 8 {
                    return Vec::new();
                }
                let found = self.ps.found.iter().find(|f| f.host_id.eq_ignore_ascii_case(&console)).cloned();
                let stored = self.store.console(&console).cloned();
                let address = self.ps_live(&console).address.or_else(|| found.as_ref().map(|f| f.address.clone())).or_else(|| stored.as_ref().and_then(|c| c.addresses.first().cloned()));
                let Some(address) = address else {
                    self.ps.add.message = Some(tr("Console injoignable.", "Console unreachable.").into());
                    return Vec::new();
                };
                let mut c = found.map(|f| f.console()).or(stored).unwrap_or_default();
                // L'adresse donnée « Par adresse » (un VPN) est retenue ; celle du réseau se retrouve seule.
                if self.ps.probed.contains(&address) && !c.addresses.contains(&address) {
                    c.addresses.push(address.clone());
                }
                self.ps.add.busy = true;
                self.ps.add.message = None;
                vec![Action::Ps(PsAction::Pair { console: c, address, account, pin })]
            }
            PsStep::Paste { console } => {
                if a.trim().is_empty() {
                    return Vec::new();
                }
                self.ps.add.busy = true;
                self.ps.add.message = None;
                vec![Action::Ps(PsAction::ReadAccount { redirect: a.trim().to_string(), console })]
            }
            PsStep::ByHand { console } => {
                let id = a.trim().to_string();
                if !valid_psn_id(&id) {
                    self.ps.add.message = Some(tr("Identifiant du compte non lu : réessayez, ou saisissez-le à la main.", "Couldn't read the account ID: try again, or enter it by hand.").into());
                    return Vec::new();
                }
                let name = (!b.trim().is_empty()).then(|| b.trim().to_string());
                self.ps_account_ready(&id, name, console)
            }
            PsStep::Rename { account } => {
                if let Some(acc) = self.store.ps_accounts.iter_mut().find(|x| x.id == account) {
                    acc.name = (!a.trim().is_empty()).then(|| a.trim().to_string());
                }
                self.ps_open_step(PsStep::Tab);
                vec![Action::Save]
            }
            PsStep::Send { name, addr } => {
                if a.len() != 6 {
                    return Vec::new();
                }
                self.ps.add.busy = true;
                self.ps.add.message = None;
                vec![Action::Ps(PsAction::Send { addr, name, code: a })]
            }
            _ => Vec::new(),
        }
    }

    // MARK: - Navigation

    /// Ce que fait une touche de navigation dans les écrans des consoles ; `None` : la navigation habituelle.
    pub fn ps_nav(&mut self, n: crate::app::Nav) -> Option<Vec<Action>> {
        use crate::app::Nav;
        // Réglages › Commandes : la touche attendue (Échap annule, Effacement retire les touches).
        if self.screen == Screen::Settings && self.ps.capture.is_some() {
            match n {
                Nav::Back => self.ps.capture = None,
                Nav::Erase => {
                    if let Some(i) = self.ps.capture.take() {
                        self.ps_edit_layout(|l| l.clear(ACTIONS[i].0));
                        return Some(self.ps_changed());
                    }
                }
                _ => {}
            }
            return Some(Vec::new());
        }
        if self.typing_ps() {
            match n {
                Nav::Char(c) => {
                    self.ps_type(c);
                    return Some(Vec::new());
                }
                Nav::Erase => {
                    self.ps_erase();
                    return Some(Vec::new());
                }
                Nav::Ok if matches!(self.focus, Target::PsField(_)) => return Some(self.ps_go()),
                _ => {}
            }
        }
        // Le choix du compte ouvert sur l'accueil : B le referme.
        if self.screen == Screen::Home && self.ps.account_menu && n == Nav::Back {
            self.ps.account_menu = false;
            self.focus = Target::PsPlay;
            return Some(Vec::new());
        }
        if self.screen == Screen::Home && self.selected_console().is_some() {
            match n {
                Nav::Options => return Some(self.activate(Target::PsSettings)),
                Nav::PrevTab | Nav::NextTab => return Some(Vec::new()),
                _ => {}
            }
        }
        None
    }

    /// Retour dans l'onglet PlayStation : l'étape précédente.
    pub fn ps_back(&mut self) -> Option<Vec<Action>> {
        let step = self.ps_step()?;
        let acts = match step {
            PsStep::Tab => {
                self.add.step = AddStep::Code;
                self.add.message = None;
                self.go(Screen::Add);
                return Some(vec![Action::Ps(PsAction::Receive(false))]);
            }
            PsStep::Receive => {
                self.ps_open_step(PsStep::Tab);
                vec![Action::Ps(PsAction::Receive(false))]
            }
            PsStep::Paste { console } | PsStep::ByHand { console } => {
                self.ps_open_step(PsStep::Account { console });
                Vec::new()
            }
            _ => {
                self.ps_open_step(PsStep::Tab);
                Vec::new()
            }
        };
        Some(acts)
    }

    /// Les cibles des consoles ; `None` : pas une cible des consoles.
    pub fn ps_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        let acts = match t {
            Target::Console(i) => {
                let id = self.store.ps_consoles.get(i)?.host_id.clone();
                let key = format!("{SELECTED}{id}");
                self.ps.account_menu = false;
                if self.store.selected.as_deref() == Some(&key) {
                    self.focus = Target::PsPlay;
                    return Some(Vec::new());
                }
                self.store.selected = Some(key);
                vec![Action::Save]
            }
            Target::PsFound(i) | Target::PsTabFound(i) => {
                let f = self.new_ps_found().get(i).map(|f| (*f).clone()).or_else(|| if t == Target::PsTabFound(i) { self.ps.found.get(i).cloned() } else { None })?;
                self.ps_pair(&f.host_id, Some(f.address.clone()));
                Vec::new()
            }
            // « Associer avec un autre compte » d'une console de l'onglet.
            Target::PsPairOther(i) => {
                let f = self.ps.found.get(i)?.clone();
                self.ps_pair(&f.host_id, Some(f.address.clone()));
                Vec::new()
            }
            Target::PsPlay => {
                let c = self.selected_console()?.clone();
                if c.pairings.is_empty() {
                    return Some(self.activate(Target::PsPair));
                }
                if self.ps_live(&c.host_id).waking.is_some() {
                    return Some(Vec::new());
                }
                self.ps.message = None;
                if self.ps_state(&c.host_id) == PsState::Standby {
                    self.ps.live.entry(c.host_id.to_ascii_uppercase()).or_default().waking = Some(Instant::now());
                }
                vec![Action::Ps(PsAction::Play { host_id: c.host_id })]
            }
            Target::PsRetry => {
                let c = self.selected_console()?.host_id.to_ascii_uppercase();
                self.ps.live.entry(c).or_default().state = PsState::Unknown;
                self.ps.message = None;
                self.focus = Target::PsPlay;
                vec![Action::Ps(PsAction::Search)]
            }
            Target::PsPair => {
                let c = self.selected_console()?.clone();
                let addr = self.ps_live(&c.host_id).address;
                self.ps_pair(&c.host_id, addr);
                Vec::new()
            }
            Target::PsAccountChip => {
                self.ps.account_menu = !self.ps.account_menu;
                if self.ps.account_menu {
                    self.focus = Target::PsAccountPick(0);
                }
                Vec::new()
            }
            Target::PsAccountPick(i) => {
                let c = self.selected_console()?.clone();
                let account = c.pairings.get(i)?.account.clone();
                if let Some(cm) = self.store.console_mut(&c.host_id) {
                    cm.account = Some(account);
                }
                self.ps.account_menu = false;
                self.focus = Target::PsPlay;
                vec![Action::Save]
            }
            Target::PsSettings | Target::PsCustomChip => {
                self.selected_console()?;
                self.open_settings(false, Screen::Home);
                Vec::new()
            }
            Target::AddPs => {
                self.ps_open_step(PsStep::Tab);
                vec![Action::StopPairing]
            }
            Target::PsAddAccount => {
                self.ps_open_step(PsStep::Account { console: None });
                Vec::new()
            }
            Target::PsAccountRename(i) => {
                let id = self.store.ps_accounts.get(i)?.id.clone();
                self.ps_open_step(PsStep::Rename { account: id });
                vec![Action::TextInput]
            }
            Target::PsAccountRemove(i) => {
                let a = self.store.ps_accounts.get(i)?.clone();
                let consoles = self.store.consoles_of(&a.id);
                self.confirm = Some(Confirm::RemovePsAccount { account: a.id.clone(), name: a.label(), consoles });
                self.focus = Target::ConfirmNo;
                Vec::new()
            }
            Target::PsWhich(i) => {
                let PsStep::WhichAccount { console } = self.ps_step()? else { return Some(Vec::new()) };
                let id = self.ps_free_accounts(&console).get(i)?.id.clone();
                self.ps_open_step(PsStep::Code { console, account: id });
                vec![Action::TextInput]
            }
            Target::PsWhichNew => {
                let PsStep::WhichAccount { console } = self.ps_step()? else { return Some(Vec::new()) };
                self.ps_open_step(PsStep::Account { console: Some(console) });
                Vec::new()
            }
            Target::PsSignIn => {
                let console = match self.ps_step()? {
                    PsStep::Account { console } => console,
                    _ => None,
                };
                self.ps_open_step(PsStep::Paste { console });
                vec![Action::Ps(PsAction::SignIn), Action::TextInput]
            }
            Target::PsByHand => {
                let console = match self.ps_step()? {
                    PsStep::Account { console } | PsStep::Paste { console } => console,
                    _ => None,
                };
                self.ps_open_step(PsStep::ByHand { console });
                vec![Action::TextInput]
            }
            Target::PsReceive => {
                self.ps_open_step(PsStep::Receive);
                self.ps.add.done = None;
                vec![Action::Ps(PsAction::Receive(true))]
            }
            Target::PsNewCode => {
                self.ps_open_step(PsStep::Receive);
                vec![Action::Ps(PsAction::Receive(false)), Action::Ps(PsAction::Receive(true))]
            }
            Target::PsSendTo(i) => {
                let d = self.ps.add.devices.get(i)?.clone();
                self.ps_open_step(PsStep::Send { name: d.name.clone(), addr: d.addr });
                vec![Action::TextInput]
            }
            Target::PsField(i) => {
                self.ps.add.field = i as usize;
                vec![Action::TextInput]
            }
            Target::PsGo => self.ps_go(),
            Target::Key(c) if self.typing_ps() => {
                self.ps_type(c);
                Vec::new()
            }
            Target::KeyErase if self.typing_ps() => {
                self.ps_erase();
                Vec::new()
            }
            Target::KeyOk if self.typing_ps() => self.ps_go(),
            Target::KeyShift if self.typing_ps() => {
                self.ps.add.shift = !self.ps.add.shift;
                Vec::new()
            }
            Target::BarPsStats => {
                let s = self.session_mut()?;
                s.show_stats = !s.show_stats;
                Vec::new()
            }
            Target::BarPsMute => {
                let s = self.session_mut()?;
                s.muted = !s.muted;
                vec![Action::Mute(s.id, s.muted)]
            }
            Target::BarPsSettings => {
                let host = self.session()?.console.as_ref()?.host_id.clone();
                if let Some(s) = self.session_mut() {
                    s.bar = false;
                    s.menu = false;
                }
                self.store.selected = Some(format!("{SELECTED}{host}"));
                self.open_settings(false, Screen::Session);
                Vec::new()
            }
            Target::BarPsRest => {
                let s = self.session_mut()?;
                s.bar = false;
                s.menu = false;
                vec![Action::Ps(PsAction::Rest(s.id))]
            }
            _ => return None,
        };
        Some(acts)
    }

    /// Supprimer un compte (sa confirmation acceptée).
    pub fn ps_remove_account(&mut self, account: &str) -> Vec<Action> {
        self.store.remove_ps_account(account);
        self.ps_open_step(PsStep::Tab);
        vec![Action::Save]
    }

    // MARK: - Réglages de la console

    /// Les rubriques des réglages d'une console : « Image », « Commandes ».
    pub fn ps_sections(&self) -> &'static [Section] {
        &PS_SECTIONS
    }

    /// Les lignes d'une rubrique des réglages de la console choisie.
    pub fn ps_rows(&self, section: Section) -> Vec<Row> {
        let Some(c) = self.selected_console() else { return Vec::new() };
        let st = &c.settings;
        let desktop = self.layout == Layout::Desktop;
        let choice = |setting, label: &str, hint: String, options: Vec<String>, current: usize, enabled: bool| Row { setting, label: label.into(), hint, kind: RowKind::Choice { disabled: vec![false; options.len()], options, current, tags: Vec::new() }, enabled, tag: None };
        let toggle = |setting, label: &str, hint: String, on: bool| Row { setting, label: label.into(), hint, kind: RowKind::Toggle(on), enabled: true, tag: None };
        let pct = |v: u8| if v == 0 { tr("Coupé", "Off").to_string() } else { format!("{v} %") };
        let intensities: Vec<String> = (0..=10).map(|i| pct(i * 10)).collect();
        match section {
            Section::PsPicture => {
                let res = ["auto", "p1080", "p720", "p540", "p360"].iter().position(|r| *r == st.resolution).unwrap_or(0);
                let bitrate = PS_BITRATES.iter().position(|b| Some(*b) == st.bitrate_kbps).unwrap_or(0);
                let codec = ["auto", "h264", "h265"].iter().position(|v| *v == st.codec).unwrap_or(0);
                let hevc = self.machine.decodes("hevc");
                let mut rows = vec![
                    choice(
                        Setting::PsResolution,
                        tr("Définition", "Resolution"),
                        if res == 0 { tr("La plus haute que la console accepte : 1080p.", "The highest the console accepts: 1080p.").into() } else { String::new() },
                        vec!["Auto".into(), "1080p".into(), "720p".into(), "540p".into(), "360p".into()],
                        res,
                        true,
                    ),
                    choice(Setting::PsFps, tr("Images par seconde", "Frame rate"), String::new(), vec![tr("60 i/s", "60 fps").into(), tr("30 i/s", "30 fps").into()], if st.fps == 30 { 1 } else { 0 }, true),
                    choice(
                        Setting::PsBitrate,
                        tr("Débit", "Bitrate"),
                        if bitrate == 0 { tr("Suit la définition.", "Follows the resolution.").into() } else { String::new() },
                        PS_BITRATES.iter().map(|b| if *b == 0 { "Auto".to_string() } else { format!("{} Mbit/s", b / 1000) }).collect(),
                        bitrate,
                        true,
                    ),
                ];
                let codec_hint = if c.ps4() {
                    tr("La PS4 n'envoie que du H.264.", "The PS4 only sends H.264.").to_string()
                } else {
                    match codec {
                        0 if hevc => tr("H.265 si cet appareil le lit.", "H.265 if this device can play it.").into(),
                        0 => tr("H.265 si cet appareil le lit : ici, H.264.", "H.265 if this device can play it: here, H.264.").into(),
                        _ => String::new(),
                    }
                };
                rows.push(choice(Setting::PsCodec, tr("Format", "Format"), codec_hint, vec!["Auto".into(), "H.264".into(), "H.265".into()], if c.ps4() { 1 } else { codec }, !c.ps4()));
                // La partie occupe tout l'écran au salon, sur le Deck et sur KyberNow Link OS.
                if desktop {
                    rows.push(toggle(Setting::PsFullscreen, tr("Ouvrir la partie en plein écran", "Open the game in full screen"), String::new(), st.fullscreen));
                }
                rows
            }
            Section::PsControls => {
                let mut rows = Vec::new();
                // Clavier et souris : sur le bureau (au salon, sur le Deck et sur KyberNow Link OS, la manette joue).
                if desktop {
                    let layout = PsLayout::of(c);
                    for (i, (action, fr, en)) in ACTIONS.iter().enumerate() {
                        let value = if self.ps.capture == Some(i) {
                            tr("Appuyez sur une touche ou cliquez…", "Press a key or click…").to_string()
                        } else {
                            let names: Vec<String> = layout.inputs(action).iter().map(|inp| self.ps_input_name(*inp)).collect();
                            if names.is_empty() { tr("Aucune", "None").to_string() } else { names.join(", ") }
                        };
                        let hint = if i == 0 { tr("Le clavier et la souris tiennent lieu de manette. Une manette branchée les remplace.", "The keyboard and mouse stand in for a controller. A connected controller replaces them.").to_string() } else { String::new() };
                        rows.push(Row { setting: Setting::PsBind(i as u8), label: tr(fr, en).into(), hint, kind: RowKind::Action(value), enabled: true, tag: None });
                    }
                    rows.push(toggle(Setting::PsMouseStick, tr("La souris tient le stick droit", "The mouse holds the right stick"), String::new(), layout.mouse_stick));
                    let sens = SENSITIVITIES.iter().position(|s| (*s - layout.sensitivity).abs() < 0.01).unwrap_or(4);
                    rows.push(choice(Setting::PsSensitivity, tr("Sensibilité", "Sensitivity"), String::new(), SENSITIVITIES.iter().map(|s| crate::i18n::decimal(*s as f64, if s.fract() == 0.0 { 0 } else if (s * 10.0).fract() == 0.0 { 1 } else { 2 })).collect(), sens, layout.mouse_stick));
                    let curve = CURVES.iter().position(|v| (*v - layout.curve).abs() < 0.01).unwrap_or(2);
                    rows.push(choice(Setting::PsCurve, tr("Réactivité au départ", "Starting response"), String::new(), vec![tr("Douce", "Soft").into(), tr("Normale", "Normal").into(), tr("Vive", "Brisk").into()], curve, layout.mouse_stick));
                    let dz = ((layout.deadzone * 100.0 / 5.0).round() as usize).min(10);
                    rows.push(choice(Setting::PsDeadzone, tr("Inclinaison minimale", "Minimum tilt"), tr("Pour passer la zone morte des jeux.", "To get past games' dead zone.").into(), (0..=10).map(|i| format!("{} %", i * 5)).collect(), dz, layout.mouse_stick));
                    rows.push(toggle(Setting::PsInvertY, tr("Inverser l'axe vertical", "Invert vertical axis"), String::new(), layout.invert_y));
                    rows.push(Row { setting: Setting::PsDefaultLayout, label: tr("Disposition par défaut", "Default layout").into(), hint: String::new(), kind: RowKind::Button { text: tr("Disposition par défaut", "Default layout").into(), style: crate::app::ButtonStyle::Ghost }, enabled: c.layout.is_some(), tag: None });
                }
                let hint = tr("Pour la DualSense. Les vibrations HD passent par la manette branchée en USB.", "For the DualSense. HD haptics go through the controller plugged in over USB.").to_string();
                rows.push(choice(Setting::PsRumble, tr("Vibrations", "Rumble"), hint, intensities.clone(), (st.rumble.min(100) / 10) as usize, true));
                rows.push(choice(Setting::PsTriggers, tr("Gâchettes adaptatives", "Adaptive triggers"), String::new(), intensities, (st.triggers.min(100) / 10) as usize, true));
                rows
            }
            _ => Vec::new(),
        }
    }

    /// Le nom d'une touche ou d'un bouton de la souris.
    pub fn ps_input_name(&self, i: PsInput) -> String {
        match i {
            PsInput::Key(u) => self.ps.key_names.get(&u).cloned().unwrap_or_else(|| default_key_name(u)),
            PsInput::Mouse(b) => mouse_name(b),
        }
    }

    fn ps_edit_layout(&mut self, f: impl FnOnce(&mut PsLayout)) {
        let Some(id) = self.selected_console().map(|c| c.host_id.clone()) else { return };
        let Some(c) = self.store.console_mut(&id) else { return };
        let mut l = PsLayout::of(c);
        f(&mut l);
        c.layout = Some(l.to_value());
    }

    fn ps_edit(&mut self, f: impl FnOnce(&mut PsSettings)) {
        let Some(id) = self.selected_console().map(|c| c.host_id.clone()) else { return };
        if let Some(c) = self.store.console_mut(&id) {
            f(&mut c.settings);
        }
    }

    /// Ce qui a changé vaut tout de suite pour la partie en cours (disposition, intensités).
    fn ps_changed(&mut self) -> Vec<Action> {
        let host_id = self.selected_console().map(|c| c.host_id.clone()).unwrap_or_default();
        vec![Action::Save, Action::Ps(PsAction::Changed { host_id })]
    }

    /// Choisit la valeur `i` d'un réglage de la console (`usize::MAX` : basculer, ou l'action).
    pub fn ps_choose(&mut self, s: Setting, i: usize) -> Option<Vec<Action>> {
        let pick = |n: usize| if i == usize::MAX { 0 } else { i.min(n - 1) };
        match s {
            Setting::PsResolution => self.ps_edit(|st| st.resolution = ["auto", "p1080", "p720", "p540", "p360"][pick(5)].into()),
            Setting::PsFps => self.ps_edit(|st| st.fps = if pick(2) == 1 { 30 } else { 60 }),
            Setting::PsBitrate => self.ps_edit(|st| st.bitrate_kbps = Some(PS_BITRATES[pick(PS_BITRATES.len())]).filter(|b| *b > 0)),
            Setting::PsCodec => {
                if self.selected_console().is_some_and(|c| c.ps4()) {
                    return Some(Vec::new());
                }
                self.ps_edit(|st| st.codec = ["auto", "h264", "h265"][pick(3)].into());
            }
            Setting::PsFullscreen => self.ps_edit(|st| st.fullscreen = !st.fullscreen),
            Setting::PsRumble => self.ps_edit(|st| st.rumble = (pick(11) * 10) as u8),
            Setting::PsTriggers => self.ps_edit(|st| st.triggers = (pick(11) * 10) as u8),
            Setting::PsBind(a) => {
                self.ps.capture = if self.ps.capture == Some(a as usize) { None } else { Some(a as usize) };
                return Some(Vec::new());
            }
            Setting::PsMouseStick => self.ps_edit_layout(|l| l.mouse_stick = !l.mouse_stick),
            Setting::PsSensitivity => self.ps_edit_layout(|l| l.sensitivity = SENSITIVITIES[if i == usize::MAX { 4 } else { i.min(SENSITIVITIES.len() - 1) }]),
            Setting::PsCurve => self.ps_edit_layout(|l| l.curve = CURVES[if i == usize::MAX { 2 } else { i.min(2) }]),
            Setting::PsDeadzone => self.ps_edit_layout(|l| l.deadzone = (if i == usize::MAX { 4 } else { i.min(10) }) as f32 * 0.05),
            Setting::PsInvertY => self.ps_edit_layout(|l| l.invert_y = !l.invert_y),
            Setting::PsDefaultLayout => {
                let id = self.selected_console()?.host_id.clone();
                if let Some(c) = self.store.console_mut(&id) {
                    c.layout = None;
                }
            }
            _ => return None,
        }
        Some(self.ps_changed())
    }

    /// Réglages › Commandes : la touche ou le bouton de la souris attendu (`capture`).
    pub fn ps_capture_input(&mut self, input: PsInput) -> Vec<Action> {
        let Some(i) = self.ps.capture.take() else { return Vec::new() };
        self.ps_edit_layout(|l| l.bind(input, ACTIONS[i].0));
        self.ps_changed()
    }

    // MARK: - Partie

    /// La partie s'ouvre : une session de l'app, la même connexion que pour un ordinateur.
    pub fn ps_open_session(&mut self, host_id: &str, relayed: bool) -> Option<SessionId> {
        let c = self.store.console(host_id)?.clone();
        let id = self.new_session_id();
        let mut ui = crate::app::SessionUi::new(id, &format!("{SELECTED}{}", c.host_id), &c.name, crate::model::Profile::Jeu, false, false, 1);
        ui.console = Some(PsSessionUi { host_id: c.host_id.clone(), relayed, ..Default::default() });
        ui.fullscreen = self.layout.console() || c.settings.fullscreen;
        if let Some(l) = self.ps.live.get_mut(&c.host_id.to_ascii_uppercase()) {
            l.waking = None;
        }
        self.open_session(ui);
        self.screen = Screen::Session;
        self.focus = Target::CancelConnect;
        Some(id)
    }

    /// La fin d'une partie : son texte (rien pour une fin voulue).
    pub fn ps_ended(&mut self, id: SessionId, end: PsEnd) {
        let Some(s) = self.session_by_mut(id) else { return };
        if let Some(text) = end.text() {
            s.phase = Phase::Ended { error: Some(text.to_string()) };
        } else if matches!(s.phase, Phase::Ending) {
            // La déconnexion se voit jusqu'au bout (la fin voulue revient après l'animation).
            if s.ending_since.is_some_and(|t| t.elapsed().as_millis() < 800) {
                return;
            }
            self.close_session(id);
            if self.sessions.is_empty() {
                self.go(Screen::Home);
            }
        } else {
            s.phase = Phase::Ended { error: None };
            self.close_session(id);
            if self.sessions.is_empty() {
                self.go(Screen::Home);
            }
        }
    }
}

/// Les rubriques des réglages d'une console.
pub const PS_SECTIONS: [Section; 2] = [Section::PsPicture, Section::PsControls];

/// Un identifiant de compte PSN saisi à la main : du base64 de 8 octets (ce que donnent chiaki-ng et les autres
/// apps), avec ou sans `=` final.
pub fn valid_psn_id(s: &str) -> bool {
    let t = s.trim().trim_end_matches('=');
    (t.len() == 11 || t.len() == 10) && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PsConsole, Store};

    fn console(id: &str, accounts: &[&str]) -> PsConsole {
        PsConsole {
            host_id: id.into(),
            name: format!("PS5 {id}"),
            model: "PS5".into(),
            pairings: accounts.iter().map(|a| crate::model::PsPairing { account: (*a).into(), registered: serde_json::json!({"mac": id}) }).collect(),
            account: accounts.first().map(|a| a.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn layout_json_matches_the_core() {
        let l = PsLayout::default();
        let v = l.to_value();
        assert_eq!(v["bindings"][0], serde_json::json!([{ "key": 0x1a }, "left_stick_up"]));
        assert_eq!(v["bindings"][12], serde_json::json!([{ "mouse": 1 }, "l2"]));
        let back: PsLayout = serde_json::from_value(v).unwrap();
        assert_eq!(back, l);
        // Une touche prise ailleurs y est retirée.
        let mut l = PsLayout::default();
        l.bind(PsInput::Key(0x1a), "cross");
        assert!(l.inputs("left_stick_up").is_empty());
        assert!(l.inputs("cross").contains(&PsInput::Key(0x1a)));
        l.clear("cross");
        assert!(l.inputs("cross").is_empty());
    }

    #[test]
    fn accounts_and_pairings() {
        let mut s = Store::default();
        let (a, existed) = s.put_ps_account("AAAAAAAAAAA=", Some("Lea".into()), None);
        assert!(!existed);
        let (a2, existed) = s.put_ps_account("AAAAAAAAAAA=", Some("Léa".into()), None);
        assert!(existed && a == a2);
        assert_eq!(s.ps_accounts[0].name.as_deref(), Some("Léa"), "le pseudo suit la dernière connexion");
        let (b, _) = s.put_ps_account("BBBBBBBBBBB=", None, Some("fixed".into()));
        assert_eq!(b, "fixed", "l'identifiant d'un autre appareil est repris quand il est libre");
        s.put_pairing(console("ab12", &[]), &a, serde_json::json!({}));
        s.put_pairing(console("AB12", &[]), &b, serde_json::json!({}));
        assert_eq!(s.ps_consoles.len(), 1);
        assert_eq!(s.ps_consoles[0].pairings.len(), 2);
        assert_eq!(s.ps_consoles[0].account.as_deref(), Some(a.as_str()), "le premier compte reste le choix");
        assert_eq!(s.consoles_of(&a), 1);
        s.selected = Some("ps:AB12".into());
        s.remove_ps_account(&a);
        assert_eq!(s.ps_consoles[0].account.as_deref(), Some(b.as_str()));
        s.forget_pairing("ab12", &b);
        assert!(s.ps_consoles.is_empty(), "plus d'association : la console part");
        assert_eq!(s.selected, None);
    }

    #[test]
    fn labels() {
        crate::i18n::force_english(false);
        assert_eq!(remove_account_question("Léa", 0), "Retirer le compte Léa ?");
        assert_eq!(remove_account_question("Léa", 1), "Retirer le compte Léa et sa console ?");
        assert_eq!(remove_account_question("Léa", 3), "Retirer le compte Léa et ses 3 consoles ?");
        assert_eq!(received_label(1, 1), "1 compte et 1 console reçus.");
        assert_eq!(received_label(1, 0), "1 compte et 0 console reçu.");
        assert_eq!(received_label(2, 3), "2 comptes et 3 consoles reçus.");
        assert_eq!(spaced_code("482913"), "482 913");
        assert_eq!(PsWaiting { name: "Salon".into(), platform: "tvos".into(), addr: String::new() }.label(), "Salon · Apple TV");
        crate::i18n::force_english(true);
        assert_eq!(remove_account_question("Lea", 2), "Remove the account Lea and its 2 consoles?");
        assert_eq!(received_label(2, 1), "2 accounts and 1 console received.");
    }

    #[test]
    fn psn_id_by_hand() {
        assert!(valid_psn_id("AbCdEfGhIjk="));
        assert!(valid_psn_id(" AbCdEfGhIjk "));
        assert!(!valid_psn_id("123"));
        assert!(!valid_psn_id("AbCdEfGh!jk="));
    }

    #[test]
    fn settings_custom_and_bitrate() {
        let mut st = PsSettings::default();
        assert!(!st.custom());
        st.hdr = true;
        st.fullscreen = false;
        st.rumble = 30;
        assert!(!st.custom(), "HDR, plein écran et intensités ne rendent pas « Personnalisé »");
        st.resolution = "p720".into();
        assert!(st.custom());
        assert_eq!(st.bitrate(), 10_000);
        st.bitrate_kbps = Some(250_000);
        assert_eq!(st.bitrate(), 100_000);
        // Le JSON est celui du cœur : mêmes noms, mêmes valeurs.
        let v = serde_json::to_value(PsSettings::default()).unwrap();
        assert_eq!(v["resolution"], "auto");
        assert_eq!(v["codec"], "auto");
        assert_eq!(v["fps"], 60);
        assert!(v["bitrate_kbps"].is_null());
    }
}
