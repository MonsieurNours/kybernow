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
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

use crate::i18n::{english, tr};
use crate::model::{Host, Profile, Store, WakeTarget};

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
    /// Touche verte de la télécommande : « Rejoindre avec un code » (KyberNow Share).
    Join,
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

/// Ce qu'on sait d'un ordinateur depuis le dernier sondage.
#[derive(Clone, Debug, Default)]
pub struct HostLive {
    pub online: Online,
    /// La Station sait créer un écran virtuel (`None` : pas encore sondée).
    pub virtual_display: Option<bool>,
    /// Gardée d'un sondage à l'autre, même hors ligne.
    pub station: StationKnown,
    pub problem: Option<Problem>,
    /// Compte créé pour cet appareil par le code, d'après Admin (`None` :
    /// Admin ne le dit pas ; sur ce téléviseur, tout compte vient du code).
    pub own_device: Option<bool>,
    /// KyberNow Share : ce que « Partager » peut faire avec cet ordinateur (`Info::share`) ; `None` : pas
    /// encore su.
    pub share: Option<ShareAvail>,
}

/// La Station, pour la rubrique « Station » (`Info::station` du cœur).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationLive {
    pub version: String,
    /// Une version plus récente, signée par KyberNow, est publiée.
    pub update_available: bool,
    pub update_version: Option<String>,
    /// … et elle s'installe depuis un appareil.
    pub installable: bool,
    /// … ou elle s'installe avec l'installateur, sur l'ordinateur.
    pub requires_setup: bool,
    pub installing: bool,
    pub restarting: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum StationKnown {
    /// Pas encore sondée.
    #[default]
    Unknown,
    /// Station 0.4.9 ou plus ancienne (`info` sans `station`), avec la
    /// version d'Admin si elle la donne : ni redémarrage ni mise à jour d'ici.
    Old(Option<String>),
    Info(StationLive),
}

/// Ce qu'Admin refuse au sondage, dit sous le nom de l'ordinateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Compte refusé (401) : retiré dans Admin.
    Refused,
    /// Certificat changé : Station réinstallée.
    Reinstalled,
}

/// Un sondage d'Admin (`GET /api/stream/info`).
#[derive(Clone, Debug, Default)]
pub struct Polled {
    pub online: bool,
    pub os: Option<String>,
    pub gpu: Option<String>,
    pub av1: bool,
    pub virtual_display: Option<bool>,
    pub station: StationKnown,
    pub problem: Option<Problem>,
    /// Les cartes réseau pour le réveil (vide avec une Station 0.4.9).
    pub wake: Vec<WakeTarget>,
    pub own_device: Option<bool>,
    pub share: Option<ShareAvail>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StationOp {
    Restart,
    Update,
}

/// Refus d'un redémarrage ou d'une mise à jour (`StationError` du cœur).
#[derive(Clone, Debug, PartialEq)]
pub enum StationRefusal {
    /// Un appareil est en session (son nom, si Admin le donne).
    InSession(Option<String>),
    TooSoon,
    /// Station 0.4.9 ou plus ancienne.
    Unsupported,
    RequiresSetup,
    /// Le message d'Admin ou du cœur : rien à installer, déjà en cours, injoignable…
    Other(String),
}

/// Où en est un redémarrage ou une mise à jour de la Station.
#[derive(Clone, Debug, PartialEq)]
pub enum StationPhase {
    Idle,
    /// Demande envoyée. Pour une mise à jour, Admin télécharge et vérifie le
    /// paquet avant de répondre (quelques minutes au plus).
    Asking(StationOp),
    /// Acceptée : la Station s'arrête, puis revient (sondée toutes les 2 s) ;
    /// `down` : vue arrêtée ; `target` : la version attendue.
    Waiting { op: StationOp, since: Instant, down: bool, target: Option<String> },
}

/// La rubrique Station d'un ordinateur : l'opération en cours et le dernier message.
#[derive(Clone, Debug)]
pub struct StationUi {
    pub host: String,
    pub phase: StationPhase,
    pub message: Option<String>,
}

/// Fenêtre de confirmation, par-dessus les réglages.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialog {
    /// « Retirer {nom} de cet appareil ? »
    Remove { host: String, name: String },
    /// Un appareil est en session : redémarrer ou mettre à jour quand même.
    Force { host: String, op: StationOp, device: String },
}

/// Attente au plus du retour de la Station : redémarrage, mise à jour.
const RESTART_WAIT: u64 = 180;
const UPDATE_WAIT: u64 = 300;
/// Après un réveil, l'ordinateur est sondé toutes les 2 s, 2 min au plus.
pub const WAKE_WAIT: u64 = 120;

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
    /// 100 Mbit/s (environ 93 utiles), le décodeur à 95.
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
    /// Dernier recours de la connexion : le cœur redémarre la Station, puis
    /// reprend la connexion du début.
    StationRestarting,
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
    /// KyberNow Share : « Partager » (hôte).
    Share,
    /// KyberNow Share, invité : « Qualité de l'image » (la tuile montre la qualité reçue ; ouvre les cinq choix).
    Quality,
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
/// seule, celle des captures et des tests sur l'ordinateur, n'a pas le cœur). L'image reçue n'est jamais au-dessus
/// de celle de l'hôte ; la Station décide et le dit (`ShareGuest::quality`, `GuestUi::image`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShareQuality {
    /// D'après la sortie du téléviseur et ce que son décodeur suit : proposé.
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
    /// La « Qualité de l'image » choisie (celle de « Rejoindre », puis celle du menu).
    pub quality: ShareQuality,
    /// La qualité reçue (« 1080p · 60 Hz »), telle que `Platform::on_share_image` la donne ; `None` tant que
    /// Kyber n'en sait pas la taille : le menu ne la montre alors pas.
    pub image: Option<String>,
}

impl GuestUi {
    /// Ce que montre la tuile du menu : la qualité reçue quand on la connaît, sinon le choix (« Auto »).
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

/// L'interface de la session.
#[derive(Clone, Debug)]
pub struct SessionUi {
    pub host: String,
    pub name: String,
    pub profile: Profile,
    pub phase: Phase,
    /// L'image est arrivée au moins une fois : une fin d'erreur avant elle
    /// est un échec de la connexion.
    pub streamed: bool,
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
    /// KyberNow Share, session d'hôte : le partage ouvert (`None` : pas de partage), son panneau montré, une
    /// demande à Admin en cours, et ce qu'Admin a refusé.
    pub share: Option<ShareView>,
    pub share_panel: bool,
    pub share_busy: bool,
    pub share_error: Option<String>,
    /// « Partager » choisi avant la première réponse d'Admin (`Info::share` pas encore connue) : le panneau attend
    /// cette réponse, puis ouvre le partage ou dit pourquoi il ne peut pas (`App::share_known`).
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
    pub fn new(host: &str, name: &str, profile: Profile, game_hint: bool) -> Self {
        Self {
            host: host.into(),
            name: name.into(),
            profile,
            phase: Phase::Preparing,
            streamed: false,
            menu: false,
            keys: false,
            stats: None,
            show_stats: false,
            remote_mouse: false,
            pad_mouse: false,
            notice: None,
            game_hint,
            started: Instant::now(),
            share: None,
            share_panel: false,
            share_busy: false,
            share_error: None,
            share_wait: false,
            share_opened: None,
            share_closed: None,
            guest: None,
        }
    }

    /// Une session d'invité d'un partage (« Invité sur Atelier ») : arrivé en spectateur, jamais dans la
    /// liste des ordinateurs (`key` : une clé propre à la session, que le magasin ne connaît pas).
    pub fn guest(key: &str, computer: &str, role: ShareRole) -> Self {
        let mut s = Self::new(key, &guest_title(computer), Profile::Jeu, false);
        s.guest = Some(GuestUi { computer: computer.into(), role, quality: ShareQuality::Auto, image: None });
        s
    }

    /// Le rôle de l'invité ; `None` pour une session d'hôte.
    pub fn role(&self) -> Option<ShareRole> {
        self.guest.as_ref().map(|g| g.role)
    }

    /// Clavier, souris et Magic Remote en souris vont à l'ordinateur : une session d'hôte, ou un joueur à qui
    /// le clavier et la souris sont donnés.
    pub fn keyboard_mouse(&self) -> bool {
        self.guest.as_ref().is_none_or(|g| g.role.kbm())
    }
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
    /// Stations à qui le code a été présenté : (nom, adresse).
    pub asked: Vec<(String, String)>,
    pub address: String,
    pub message: Option<String>,
    pub busy: bool,
}

/// Une ligne de réglage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Resolution,
    Fps,
    Codec,
    Bitrate,
    GameHint,
    Audio,
    /// Manette vue par l'ordinateur : Xbox 360 ou fidèle.
    Gamepad,
    Combo,
    BackMenu,
    Transport,
    Reconnect,
    /// Redémarrer la Station, la mettre à jour.
    Restart,
    Update,
    Remove,
    Language,
    /// Mises à jour : stables, ou aussi les bêtas.
    UpdateChannel,
    /// Ouvre l'écran de test des manettes.
    Pads,
}

impl Setting {
    /// Une ligne qui agit (OK) au lieu de changer de valeur (gauche, droite).
    pub fn is_action(self) -> bool {
        matches!(self, Setting::Restart | Setting::Update | Setting::Remove | Setting::Pads)
    }
}

/// Une rubrique des réglages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Picture,
    Sound,
    /// Manettes et Magic Remote, vues par l'ordinateur.
    Controls,
    Network,
    /// Dernière session et cet ordinateur.
    Statistics,
    /// Admin, redémarrer, mettre à jour, compte, retirer.
    Station,
    /// Réglages de l'app : langue, téléviseur, réseau du téléviseur, version.
    Tv,
    /// Réglages de l'app : manettes branchées et leur test.
    Gamepads,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Picture => tr("Image", "Picture"),
            Section::Sound => tr("Son", "Sound"),
            Section::Controls => tr("Commandes", "Controls"),
            Section::Network => tr("Réseau", "Network"),
            Section::Statistics => tr("Statistiques", "Statistics"),
            Section::Station => "Station",
            Section::Tv => tr("Ce téléviseur", "This TV"),
            Section::Gamepads => tr("Manettes", "Controllers"),
        }
    }
}

/// Réglages de l'ordinateur choisi (docs/link-ui.md : six rubriques, dans cet ordre).
pub const HOST_SECTIONS: [Section; 6] = [Section::Picture, Section::Sound, Section::Controls, Section::Network, Section::Statistics, Section::Station];
/// Réglages de l'app sur ce téléviseur, même sans ordinateur.
pub const APP_SECTIONS: [Section; 2] = [Section::Tv, Section::Gamepads];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Home,
    Add,
    /// KyberNow Share : « Rejoindre un partage » (code et prénom).
    Join,
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
    /// « Rejoindre avec un code » sur l'accueil.
    JoinCard,
    HostSettings,
    /// Réglages de l'app (bouton de l'accueil).
    AppSettings,
    AddFound(usize),
    AddByAddress,
    Key(char),
    KeyErase,
    KeyOk,
    /// Majuscule du pavé des lettres (le prénom d'un invité).
    KeyShift,
    /// KyberNow Share, invité : les champs de « Rejoindre un partage » et le bouton « Rejoindre ».
    JoinCode,
    JoinName,
    JoinGo,
    /// KyberNow Share, invité : « Qualité de l'image », à « Rejoindre » (les cinq choix sous le prénom).
    JoinQuality(ShareQuality),
    /// Un des cinq choix du panneau « Qualité de l'image » d'une session d'invité.
    QualityPick(ShareQuality),
    /// KyberNow Share, hôte : le panneau du partage (les deux droits d'un invité : manettes, clavier et souris,
    /// retirer, nouveau code, arrêter, fermer).
    SharePads(usize),
    ShareKbm(usize),
    ShareKick(usize),
    ShareNewCode,
    ShareStop,
    ShareClose,
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
    /// Boutons de la fenêtre de confirmation : agir, annuler.
    DialogOk,
    DialogCancel,
    Back,
}

/// Ce que l'app demande au pilote du téléviseur.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Connect { host: String, profile: Profile },
    Save,
    /// Présenter le code aux Stations trouvées à qui il ne l'a pas encore été.
    StartPairing,
    StopPairing,
    PairAddress { address: String, port: u16, fingerprint: String },
    Probe { address: String, port: u16 },
    Wake(String),
    /// Sonder un ordinateur tout de suite.
    Poll(String),
    /// Redémarrer ou mettre à jour la Station ; `force` : même si un appareil
    /// est en session.
    Station { host: String, op: StationOp, force: bool },
    /// KyberNow Share, invité : le code part aux ordinateurs du réseau (`share::join_checked`), avec la
    /// « Qualité de l'image » choisie (pour « Auto », la sortie du téléviseur et ce que son décodeur suit).
    JoinShare { code: String, name: String, quality: ShareQuality },
    /// KyberNow Share, invité : la « Qualité de l'image » changée en session (`Session::set_share_quality`, sans
    /// rien relancer de la session).
    ShareQuality(ShareQuality),
    /// KyberNow Share, hôte : une demande à Admin pour le partage de la session.
    Share(ShareOp),
    Rumble { pad: usize, low: u16, high: u16, triggers: bool },
    Shortcut(Vec<u16>),
    SetProfile(Profile),
    RemoteMouse(bool),
    PadMouse(bool),
    ReleaseAll,
    Disconnect,
    CancelConnect,
    Language,
    /// Chercher une mise à jour maintenant.
    CheckUpdate,
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
    pub screen: Screen,
    pub focus: Target,
    /// Les éléments choisissables du dernier dessin.
    pub hits: Vec<(Target, Rect)>,
    /// Pointeur de la Magic Remote (hors session), s'il est montré.
    pub pointer: Option<(f32, f32)>,
    pub live: HashMap<String, HostLive>,
    /// Image de chaque ordinateur : son aperçu d'Admin.
    pub images: HashMap<String, Pixmap>,
    pub found: Vec<Found>,
    pub tv: TvInfo,
    pub net: NetInfo,
    pub pads: Vec<PadInfo>,
    pub add: AddState,
    pub section: usize,
    /// Écran des réglages : ceux de l'app (vrai) ou de l'ordinateur choisi.
    pub settings_global: bool,
    /// Confirmation ouverte par-dessus les réglages.
    pub dialog: Option<Dialog>,
    /// Redémarrage ou mise à jour de la Station, pour un ordinateur à la fois.
    pub station: Option<StationUi>,
    /// Réveil envoyé à cet ordinateur : « Réveil de l'ordinateur… » jusqu'à
    /// ce qu'il soit en ligne (`WAKE_WAIT` au plus).
    pub waking: Option<(String, Instant)>,
    pub pad_selected: usize,
    pub session: Option<SessionUi>,
    pub toast: Option<(String, Instant)>,
    pub version: &'static str,
    pub update: UpdateState,
    /// L'écran d'où l'on vient (Réglages et Manettes ouverts depuis la session).
    pub back_to: Screen,
    /// KyberNow Share : l'écran « Rejoindre un partage ».
    pub join: JoinState,
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
            dialog: None,
            station: None,
            waking: None,
            pad_selected: 0,
            session: None,
            toast: None,
            version: env!("CARGO_PKG_VERSION"),
            update: UpdateState::Unknown,
            back_to: Screen::Home,
            join: JoinState::new(),
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

    /// Ouvre les réglages : ceux de l'app, ou ceux de l'ordinateur choisi.
    pub fn open_settings(&mut self, global: bool, from: Screen) {
        self.settings_global = global;
        self.section = 0;
        self.dialog = None;
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
            Screen::Join => match self.join.field {
                JoinField::Code => Target::Key('1'),
                JoinField::Name => Target::Key('q'),
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
            // « Rejoindre un partage » : OK dans un champ passe au suivant, ou rejoint quand tout est donné.
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
            Nav::Options => {
                if self.screen == Screen::Home && !self.store.hosts.is_empty() {
                    return self.activate(Target::HostSettings);
                }
                Vec::new()
            }
            Nav::Join => {
                if self.screen == Screen::Home {
                    return self.activate(Target::JoinCard);
                }
                Vec::new()
            }
            Nav::Char(c) => {
                if self.screen == Screen::Add && self.add.step == AddStep::Address || self.screen == Screen::Join {
                    return self.activate(Target::Key(c));
                }
                Vec::new()
            }
            Nav::Erase => {
                if self.screen == Screen::Add && self.add.step == AddStep::Address || self.screen == Screen::Join {
                    return self.activate(Target::KeyErase);
                }
                Vec::new()
            }
            Nav::Left | Nav::Right if matches!(self.focus, Target::Setting(s) if !s.is_action()) => {
                let Target::Setting(s) = self.focus else { return Vec::new() };
                self.adjust(s, if n == Nav::Right { 1 } else { -1 })
            }
            // Une ligne qui agit : à gauche, les rubriques ; rien à droite.
            Nav::Right if matches!(self.focus, Target::Setting(_)) => Vec::new(),
            Nav::Left if matches!(self.focus, Target::Pad(_)) => Vec::new(),
            _ => {
                self.move_focus(n);
                self.follow_join_field();
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
            // Une recherche en cours est abandonnée : sa réponse n'ouvrira rien (elle repart de la Station).
            Screen::Join => {
                self.join.attempt += 1;
                self.join.busy = false;
                self.go(Screen::Home);
                Vec::new()
            }
            Screen::Settings | Screen::Gamepads => {
                if self.screen == Screen::Settings && self.dialog.is_some() {
                    self.close_dialog();
                    return Vec::new();
                }
                if self.screen == Screen::Settings && matches!(self.focus, Target::Setting(_)) {
                    self.focus = Target::Section(self.section);
                    return Vec::new();
                }
                let to = self.back_to;
                self.back_to = Screen::Home;
                if to == Screen::Session && self.session.is_some() {
                    // Un invité à qui l'hôte a repris les manettes n'a plus la tuile : le focus revient sur « Reprendre ».
                    let pads = self.session.as_ref().and_then(|s| s.role()).is_none_or(|r| r.pads());
                    let item = if self.screen == Screen::Settings { MenuItem::Settings } else if pads { MenuItem::Gamepads } else { MenuItem::Resume };
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
            Phase::Preparing | Phase::Connecting | Phase::StationRestarting | Phase::FirstImages => vec![Action::CancelConnect],
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
                // Le panneau du partage se referme sur l'image (il s'ouvre depuis le menu).
                if s.share_panel {
                    // Le panneau de la qualité de l'image d'un invité revient au menu d'où il s'ouvre.
                    let quality = s.guest.is_some();
                    s.share_panel = false;
                    if quality {
                        s.menu = true;
                        self.focus = Target::Menu(MenuItem::Quality);
                    }
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
            && !s.share_panel
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
            // Le bouton principal : se connecter ; hors ligne, réveiller
            // l'ordinateur (adresse MAC connue) ou le sonder de nouveau.
            Target::Connect => {
                let Some(h) = self.selected() else { return Vec::new() };
                let id = h.id.clone();
                if self.live.get(&id).is_some_and(|l| l.online == Online::Offline) {
                    if h.can_wake() {
                        self.waking = Some((id.clone(), Instant::now()));
                        return vec![Action::Wake(id)];
                    }
                    self.live.entry(id.clone()).or_default().online = Online::Unknown;
                    return vec![Action::Poll(id)];
                }
                let profile = h.settings.profile;
                vec![Action::Connect { host: id, profile }]
            }
            Target::Chip(p) => {
                let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
                if let Some(h) = self.store.host_mut(&id) {
                    h.settings.apply(p);
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
            Target::QualityPick(q) => self.quality_pick(q),
            Target::SharePads(i) => self.share_guest_move(i, ShareMove::Pads),
            Target::ShareKbm(i) => self.share_guest_move(i, ShareMove::Kbm),
            Target::ShareKick(i) => {
                let Some(guest) = self.session.as_ref().and_then(|s| s.share.as_ref()).and_then(|v| v.guests.get(i)).map(|g| g.id.clone()) else { return Vec::new() };
                self.share_op(ShareOp::Kick(guest))
            }
            Target::ShareNewCode => self.share_op(ShareOp::NewCode),
            Target::ShareStop => self.share_op(ShareOp::Close),
            Target::ShareClose => {
                if let Some(s) = self.session.as_mut() {
                    s.share_panel = false;
                }
                Vec::new()
            }
            Target::KeyShift => {
                self.join.shift = !self.join.shift;
                Vec::new()
            }
            Target::Key(c) if self.screen == Screen::Join => {
                self.join_type(c);
                Vec::new()
            }
            Target::KeyErase if self.screen == Screen::Join => {
                if !self.join.busy {
                    match self.join.field {
                        JoinField::Code => self.join.code.pop(),
                        JoinField::Name => self.join.name.pop(),
                    };
                    self.join.message = None;
                }
                Vec::new()
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
            Target::DialogOk => match self.dialog.take() {
                Some(Dialog::Remove { host, name }) => {
                    self.store.remove(&host);
                    if self.station.as_ref().is_some_and(|s| s.host == host) {
                        self.station = None;
                    }
                    self.toast(format!("{name} {}", tr("est retiré de ce téléviseur", "removed from this TV")));
                    self.go(Screen::Home);
                    vec![Action::Save]
                }
                Some(Dialog::Force { host, op, .. }) => self.station_ask(&host, op, true),
                None => Vec::new(),
            },
            Target::DialogCancel => {
                self.close_dialog();
                Vec::new()
            }
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

    // MARK: - KyberNow Share

    /// Le prénom proposé à l'invité : le nom de ce téléviseur pour Admin (« LG OLED83G2 »).
    pub fn device_name(&self) -> String {
        format!("LG {}", self.tv.model)
    }

    /// « Rejoindre avec un code » : l'écran du code, du prénom (proposé : celui de la dernière fois, sinon le nom de
    /// ce téléviseur) et de la « Qualité de l'image » (celle de la dernière fois, sinon « Auto »).
    fn start_join(&mut self) -> Vec<Action> {
        let kept = self.store.app.share_name.trim().to_string();
        let proposed = self.device_name();
        let name: String = if kept.is_empty() { proposed.trim() } else { kept.as_str() }.chars().take(NAME_MAX).collect();
        let quality = ShareQuality::parse(&self.store.app.share_quality);
        self.join = JoinState { name, quality, attempt: self.join.attempt + 1, ..JoinState::new() };
        self.go(Screen::Join);
        Vec::new()
    }

    /// Une lettre ou un chiffre pour le champ de « Rejoindre un partage » : les chiffres du code (six au
    /// plus), le prénom (un caractère imprimable, `NAME_MAX` au plus). Le sixième chiffre porte le focus
    /// sur « Rejoindre ».
    fn join_type(&mut self, c: char) {
        if self.join.busy {
            return;
        }
        match self.join.field {
            JoinField::Code => {
                if c.is_ascii_digit() && self.join.code.len() < 6 {
                    self.join.code.push(c);
                    self.join.message = None;
                    if self.join.code.len() == 6 && self.join.ready() {
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
            let (field, focus) = if self.join.code.len() < 6 { (JoinField::Code, Target::Key('1')) } else { (JoinField::Name, Target::Key('q')) };
            self.join.field = field;
            self.focus = focus;
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

    /// « Rejoindre un partage » : le champ que le focus atteint avec la croix devient celui où l'on tape
    /// (le pavé change : chiffres pour le code, lettres pour le prénom).
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

    /// Ce que « Partager » peut faire avec l'ordinateur de la session (`None` : pas encore su).
    pub fn share_avail(&self) -> Option<ShareAvail> {
        let s = self.session.as_ref()?;
        self.live.get(&s.host).and_then(|l| l.share)
    }

    /// « Partager » (tuile du menu de session) : il ouvre le partage de l'écran principal (une demande à Admin) et
    /// son panneau, ou, déjà ouvert, le panneau. Partage impossible (Station trop ancienne, partage coupé) : la
    /// tuile est estompée mais se choisit, et le panneau qu'elle ouvre est réduit à la raison. Tant qu'Admin n'a pas
    /// dit ce que l'ordinateur permet, le panneau attend sa réponse (`share_known`) au lieu de laisser une tuile muette.
    fn share_button(&mut self) -> Vec<Action> {
        let Some(s) = self.session.as_ref() else { return Vec::new() };
        if s.guest.is_some() {
            return Vec::new();
        }
        let (open, shown) = (s.share.is_some(), s.share_panel);
        let avail = self.share_avail();
        let blocked = avail.is_some_and(|a| a != ShareAvail::Available);
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        if shown {
            s.share_panel = false;
            return Vec::new();
        }
        s.share_error = None;
        s.share_panel = true;
        s.menu = false;
        let mut acts = vec![Action::ReleaseAll];
        let focus = if open { Target::ShareNewCode } else { Target::ShareClose };
        if !open && !blocked {
            s.share_busy = true;
            if avail.is_some() {
                acts.push(Action::Share(ShareOp::Open));
            } else {
                s.share_wait = true;
                acts.push(Action::Poll(s.host.clone()));
            }
        }
        self.focus = focus;
        acts
    }

    /// Admin a répondu à un sondage de l'ordinateur `host` : la session dont le panneau de « Partager » attendait
    /// cette réponse ouvre le partage, ou dit pourquoi elle ne le peut pas (le panneau se réduit à la raison).
    /// Sans réponse (ordinateur injoignable), la demande part quand même : le refus qui revient dit pourquoi. Un
    /// panneau refermé entre-temps n'ouvre rien.
    pub fn share_known(&mut self, host: &str) -> Vec<Action> {
        let avail = self.live.get(host).and_then(|l| l.share);
        let Some(s) = self.session.as_mut().filter(|s| s.host == host && s.share_wait) else { return Vec::new() };
        s.share_wait = false;
        if !s.share_panel || avail.is_some_and(|a| a != ShareAvail::Available) {
            s.share_busy = false;
            return Vec::new();
        }
        vec![Action::Share(ShareOp::Open)]
    }

    /// Une demande de l'hôte à Admin : « Nouveau code », « Retirer », « Arrêter le partage », un rôle.
    fn share_op(&mut self, op: ShareOp) -> Vec<Action> {
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        if s.share_busy {
            return Vec::new();
        }
        s.share_busy = true;
        s.share_error = None;
        vec![Action::Share(op)]
    }

    /// Un des deux boutons de l'invité `i` : son droit basculé (donné ou repris), l'autre tel quel. Le rôle voulu
    /// se calcule d'après celui qui est montré : `share_op` ne laisse partir qu'une demande à la fois, jusqu'à la
    /// réponse d'Admin, sinon deux clics rapides sur les deux droits s'annuleraient.
    fn share_guest_move(&mut self, i: usize, m: ShareMove) -> Vec<Action> {
        let Some(guest) = self.session.as_ref().and_then(|s| s.share.as_ref()).and_then(|v| v.guests.get(i)).cloned() else { return Vec::new() };
        self.share_op(ShareOp::Role(guest.id, m.toggled(guest.role)))
    }

    /// La réponse d'Admin à une demande de l'hôte : le partage tel qu'il est maintenant (`None` : fermé),
    /// ou ce qu'Admin a refusé.
    pub fn share_reply(&mut self, r: Result<Option<ShareView>, String>) {
        let Some(s) = self.session.as_mut() else { return };
        s.share_busy = false;
        let shown = s.share_panel;
        match r {
            Ok(view) => {
                let first = s.share.is_none() && view.is_some();
                if view.is_none() {
                    s.share_panel = false;
                    s.share_opened = None;
                    s.share_closed = Some(Instant::now());
                } else if first {
                    s.share_opened = Some(Instant::now());
                    s.share_closed = None;
                }
                s.share = view;
                s.share_error = None;
                if first && shown {
                    self.focus = Target::ShareNewCode;
                }
            }
            Err(e) => s.share_error = Some(e),
        }
    }

    /// Le signe de vie de l'hôte porte la liste des invités (`Platform::on_guests`). Un signe de vie parti avant
    /// une réponse d'Admin ne la défait pas : ni un partage qu'on vient d'ouvrir (fermé pendant 8 s au plus), ni
    /// un partage qu'on vient d'arrêter (rouvert pendant 3 s au plus), comme l'app Mac.
    pub fn share_guests(&mut self, view: Option<ShareView>) {
        let Some(s) = self.session.as_mut() else { return };
        match &view {
            None if s.share_busy || s.share_opened.is_some_and(|t| t.elapsed() < Duration::from_secs(8)) => return,
            Some(_) if s.share_closed.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) => return,
            _ => {}
        }
        if view.is_none() {
            s.share_panel = false;
            s.share_opened = None;
        } else if s.share.is_none() {
            s.share_opened = Some(Instant::now());
        }
        s.share = view;
    }

    /// Un choix de « Qualité de l'image » en session (panneau ouvert depuis le menu) : retenu pour la fois suivante,
    /// et demandé à la Station (l'image de l'invité seulement est relancée, jamais celle de l'hôte). Le panneau se
    /// ferme ; choisir la qualité déjà en place ne demande rien.
    fn quality_pick(&mut self, q: ShareQuality) -> Vec<Action> {
        let Some(s) = self.session.as_mut() else { return Vec::new() };
        let Some(g) = s.guest.as_mut() else { return Vec::new() };
        let changed = g.quality != q;
        g.quality = q;
        s.share_panel = false;
        if !changed {
            return Vec::new();
        }
        self.store.app.share_quality = q.as_str().into();
        vec![Action::ShareQuality(q), Action::Save]
    }

    /// L'image que l'invité reçoit (`Platform::on_share_image`, au départ puis à chaque changement) : sa qualité,
    /// que la tuile du menu montre quand Kyber en sait la taille (`label` nul sinon), et l'avis bref de la
    /// Station s'il y en a un (« Image allégée pour suivre. »).
    pub fn share_image(&mut self, label: Option<String>, notice: Option<String>) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(g) = s.guest.as_mut() else { return };
        g.image = label;
        if let Some(text) = notice {
            s.notice = Some((text, Instant::now()));
        }
    }

    /// Le rôle de l'invité a changé (au départ, `previous` nul ; puis par l'hôte) : l'avis bref, et ce que
    /// le rôle n'ouvre plus se ferme (touches, Magic Remote et manette en souris).
    pub fn share_role_changed(&mut self, role: ShareRole, previous: Option<ShareRole>) {
        let Some(s) = self.session.as_mut() else { return };
        let Some(g) = s.guest.as_mut() else { return };
        g.role = role;
        if let Some(text) = role.notice(previous) {
            s.notice = Some((text.into(), Instant::now()));
        }
        if !role.kbm() {
            s.keys = false;
            s.remote_mouse = false;
            s.pad_mouse = false;
        }
        if s.menu {
            self.focus = Target::Menu(MenuItem::Resume);
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
            // « Partager » : le menu se ferme, le panneau du partage s'ouvre sur l'image.
            MenuItem::Share => self.share_button(),
            // « Qualité de l'image » (invité) : le menu se ferme, les cinq choix s'ouvrent sur l'image ; Retour revient au menu.
            MenuItem::Quality => {
                let Some(q) = s.guest.as_ref().map(|g| g.quality) else { return Vec::new() };
                s.menu = false;
                s.share_panel = true;
                self.focus = Target::QualityPick(q);
                vec![Action::ReleaseAll]
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
        if s == Setting::UpdateChannel {
            self.store.app.beta = !self.store.app.beta;
            return vec![Action::Save, Action::CheckUpdate];
        }
        if s == Setting::Language {
            let order = ["auto", "fr", "en"];
            let i = order.iter().position(|v| *v == self.store.app.language).unwrap_or(0);
            self.store.app.language = order[cycle(i, order.len(), dir)].into();
            return vec![Action::Language, Action::Save];
        }
        let Some((id, name)) = self.selected().map(|h| (h.id.clone(), h.name.clone())) else { return Vec::new() };
        match s {
            Setting::Remove => {
                self.dialog = Some(Dialog::Remove { host: id, name });
                self.focus = Target::DialogCancel;
                return Vec::new();
            }
            Setting::Restart => return self.station_ask(&id, StationOp::Restart, false),
            Setting::Update => return self.station_ask(&id, StationOp::Update, false),
            _ => {}
        }
        let panel_h = self.tv.panel.1;
        let Some(h) = self.store.host_mut(&id) else { return Vec::new() };
        let st = &mut h.settings;
        match s {
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
                let opts = ["auto", "stereo", "dolby", "surround", "computer"];
                let i = opts.iter().position(|v| *v == st.audio).unwrap_or(0);
                st.audio = opts[cycle(i, opts.len(), dir)].into();
            }
            Setting::GameHint => st.game_mode_hint = !st.game_mode_hint,
            Setting::Gamepad => st.xbox360 = !st.xbox360,
            Setting::Combo => st.combo = !st.combo,
            Setting::BackMenu => st.back_opens_menu = !st.back_opens_menu,
            Setting::Reconnect => st.reconnect = !st.reconnect,
            // Rapide ou Fiable ; le choix du profil n'est pas un écart.
            Setting::Transport => {
                let reliable = !st.reliable_transport();
                let t = if reliable == st.profile.reliable() { "" } else if reliable { "reliable" } else { "fast" };
                st.set_transport(t);
            }
            Setting::Remove | Setting::Restart | Setting::Update | Setting::Language | Setting::Pads | Setting::UpdateChannel => {}
        }
        vec![Action::Save]
    }

    /// Ferme la confirmation ; le focus revient à la ligne qui l'a ouverte.
    fn close_dialog(&mut self) {
        self.focus = match self.dialog.take() {
            Some(Dialog::Force { op: StationOp::Restart, .. }) => Target::Setting(Setting::Restart),
            Some(Dialog::Force { op: StationOp::Update, .. }) => Target::Setting(Setting::Update),
            _ => Target::Setting(Setting::Remove),
        };
    }

    /// Demande un redémarrage ou une mise à jour de la Station.
    fn station_ask(&mut self, host: &str, op: StationOp, force: bool) -> Vec<Action> {
        if self.station.as_ref().is_some_and(|s| s.host == host && s.phase != StationPhase::Idle) {
            return Vec::new();
        }
        self.station = Some(StationUi { host: host.into(), phase: StationPhase::Asking(op), message: None });
        // Les lignes d'action laissent la place à l'état : le focus revient à la rubrique.
        if self.screen == Screen::Settings {
            self.focus = Target::Section(self.section);
        }
        vec![Action::Station { host: host.into(), op, force }]
    }

    /// La réponse d'Admin : acceptée (avec la version visée pour une mise à
    /// jour), ou refusée.
    pub fn station_answer(&mut self, host: &str, op: StationOp, r: Result<Option<String>, StationRefusal>) {
        let setup_version = match self.live.get(host).map(|l| &l.station) {
            Some(StationKnown::Info(st)) => st.update_version.clone(),
            _ => None,
        };
        let Some(ui) = self.station.as_mut().filter(|s| s.host == host) else { return };
        ui.phase = StationPhase::Idle;
        match r {
            Ok(target) => ui.phase = StationPhase::Waiting { op, since: Instant::now(), down: false, target: target.filter(|v| !v.is_empty()) },
            Err(StationRefusal::InSession(device)) => {
                let device = device.unwrap_or_else(|| tr("Un appareil", "A device").into());
                self.dialog = Some(Dialog::Force { host: host.into(), op, device });
                self.focus = Target::DialogCancel;
            }
            Err(StationRefusal::TooSoon) => ui.message = Some(tr("La Station vient de redémarrer. Réessayez dans une minute.", "The Station has just restarted. Try again in a minute.").into()),
            Err(StationRefusal::Unsupported) => ui.message = Some(station_too_old().into()),
            Err(StationRefusal::RequiresSetup) => ui.message = Some(setup_version.map(|v| setup_text(&v)).unwrap_or_else(|| tr("Cette version s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe.", "This version installs on the computer, with KyberNow-Station-Setup.exe.").into())),
            Err(StationRefusal::Other(m)) => ui.message = Some(m),
        }
    }

    /// L'ordinateur dont la Station redémarre ou se met à jour : sondé plus souvent.
    pub fn station_waiting(&self) -> Option<&str> {
        self.station.as_ref().filter(|s| matches!(s.phase, StationPhase::Waiting { .. })).map(|s| s.host.as_str())
    }

    /// L'ordinateur qu'on réveille, tant qu'on l'attend (`WAKE_WAIT` au plus).
    pub fn waking(&self) -> Option<&str> {
        self.waking.as_ref().filter(|(_, at)| at.elapsed().as_secs() < WAKE_WAIT).map(|(id, _)| id.as_str())
    }

    /// Les ordinateurs à sonder toutes les 2 s : Station qui revient, ordinateur qui se réveille.
    pub fn fast_poll(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.station_waiting().into_iter().chain(self.waking()).map(str::to_string).collect();
        ids.dedup();
        ids
    }

    /// Un sondage d'Admin : état, système, Station ; suit le retour de la
    /// Station après un redémarrage ou une mise à jour.
    pub fn polled(&mut self, id: &str, p: Polled) -> bool {
        let entry = self.live.entry(id.to_string()).or_default();
        entry.online = if p.online { Online::Online } else { Online::Offline };
        entry.problem = p.problem;
        if p.online {
            entry.virtual_display = p.virtual_display;
            entry.station = p.station.clone();
            entry.own_device = p.own_device;
            entry.share = p.share;
            // Réveillé : le bouton redevient « Se connecter ».
            if self.waking.as_ref().is_some_and(|(w, _)| w == id) {
                self.waking = None;
            }
        }
        let mut save = false;
        if p.online
            && let Some(h) = self.store.host_mut(id)
        {
            save = h.os != p.os || h.gpu != p.gpu || h.av1 != p.av1;
            h.os = p.os.or(h.os.take());
            h.gpu = p.gpu.or(h.gpu.take());
            h.av1 = p.av1;
            // Les cartes pour le réveil : les dernières données, gardées si Admin n'en donne plus.
            if !p.wake.is_empty() && h.wake != p.wake {
                h.wake = p.wake;
                save = true;
            }
        }
        let mut done: Option<String> = None;
        if let Some(ui) = self.station.as_mut().filter(|s| s.host == id)
            && let StationPhase::Waiting { op, since, down, target } = &mut ui.phase
        {
            if !p.online {
                *down = true;
            } else if let StationKnown::Info(st) = &p.station {
                match op {
                    StationOp::Restart if *down && !st.restarting => done = Some(tr("La Station a redémarré.", "The Station has restarted.").into()),
                    StationOp::Update if !st.installing && (target.as_deref() == Some(st.version.as_str()) || (*down && target.is_none())) => {
                        done = Some(if english() { format!("KyberNow Station {} is installed.", st.version) } else { format!("KyberNow Station {} est en place.", st.version) });
                    }
                    StationOp::Update if *down && !st.installing && !st.restarting => {
                        done = Some(tr("La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus.", "The update did not complete. Open Admin to find out more.").into());
                    }
                    _ => {}
                }
            }
            let wait = if *op == StationOp::Restart { RESTART_WAIT } else { UPDATE_WAIT };
            if done.is_none() && since.elapsed().as_secs() > wait {
                done = Some(tr("La Station tarde à revenir. Ouvrez Admin pour en savoir plus.", "The Station is taking a while to come back. Open Admin to find out more.").into());
            }
            if let Some(m) = &done {
                ui.phase = StationPhase::Idle;
                ui.message = Some(m.clone());
            }
        }
        if let Some(m) = done
            && self.screen != Screen::Settings
        {
            self.toast(m);
        }
        save
    }

    /// L'écran que la session créera ou utilisera, comme `session_mode` :
    /// docs/link-ui.md, Accueil.
    pub fn screen_line(&self, h: &Host) -> String {
        if self.live.get(&h.id).and_then(|l| l.virtual_display) == Some(false) {
            return tr("Les écrans de l'ordinateur, tels quels", "The computer's own displays, as they are").into();
        }
        let (w, hh, fps) = self.session_mode(h);
        let sized = h.settings.resolution == 0;
        match (english(), sized) {
            (false, true) => format!("Écran de {w} × {hh} à {fps} Hz, à la taille de cet écran"),
            (false, false) => format!("Écran de {w} × {hh} à {fps} Hz"),
            (true, true) => format!("{w} × {hh} display at {fps} Hz, sized for this screen"),
            (true, false) => format!("{w} × {hh} display at {fps} Hz"),
        }
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

/// « Station trop ancienne » : docs/link-ui.md, Station.
pub fn station_too_old() -> &'static str {
    tr(
        "Pour redémarrer ou mettre à jour la Station depuis cet appareil, mettez-la d'abord à jour depuis Admin.",
        "To restart or update the Station from this device, first update it from Admin.",
    )
}

/// Une version de la Station qui s'installe avec l'installateur.
pub fn setup_text(v: &str) -> String {
    if english() {
        format!("Version {v} installs on the computer, with KyberNow-Station-Setup.exe.")
    } else {
        format!("La version {v} s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe.")
    }
}

/// L'adresse d'Admin à ouvrir dans un navigateur (le QR code de la rubrique
/// Station) : celle du cœur sur le téléviseur.
#[cfg(feature = "device")]
pub use kybernow_core::console::admin_url;

/// Sans le cœur (captures sur l'ordinateur, `--no-default-features`, qui ne
/// compile pas les bibliothèques de Kyber) : la même règle, recopiée.
#[cfg(not(feature = "device"))]
pub fn admin_url(address: &str, port: u16) -> String {
    if address.contains(':') && !address.starts_with('[') { format!("https://[{address}]:{port}/") } else { format!("https://{address}:{port}/") }
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

    fn demo_host() -> Host {
        Host {
            id: "a".into(),
            name: "A".into(),
            address: "192.168.1.20".into(),
            port: 24811,
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

    fn info(version: &str) -> Polled {
        Polled { online: true, station: StationKnown::Info(StationLive { version: version.into(), ..Default::default() }), ..Default::default() }
    }

    #[test]
    fn admin_address() {
        assert_eq!(admin_url("192.168.1.20", 24811), "https://192.168.1.20:24811/");
        assert_eq!(admin_url("fe80::1", 9443), "https://[fe80::1]:9443/");
        assert_eq!(admin_url("[fe80::1]", 9443), "https://[fe80::1]:9443/");
    }

    #[test]
    fn custom_profile_and_transport() {
        let mut app = App::new(Store::default());
        app.store.hosts.push(demo_host());
        let h = &app.store.hosts[0].settings;
        assert!(!h.is_custom() && !h.reliable_transport());
        // Fiable en Jeu : un écart ; revenir à Rapide n'en est plus un.
        app.adjust(Setting::Transport, 1);
        let h = &app.store.hosts[0].settings;
        assert!(h.is_custom() && h.reliable_transport() && h.reliable);
        app.adjust(Setting::Transport, 1);
        assert!(!app.store.hosts[0].settings.is_custom());
        // Bureau : fiable d'office ; Rapide est un écart, effacé par le choix d'un profil.
        app.activate(Target::Chip(Profile::Bureau));
        assert!(app.store.hosts[0].settings.reliable_transport());
        app.adjust(Setting::Transport, 1);
        assert!(app.store.hosts[0].settings.is_custom() && !app.store.hosts[0].settings.reliable_transport());
        app.store.hosts[0].settings.mbps = 40;
        app.activate(Target::Chip(Profile::Max));
        let h = &app.store.hosts[0].settings;
        assert!(!h.is_custom() && h.profile == Profile::Max && h.mbps == 0);
    }

    #[test]
    fn screen_line_follows_the_session() {
        crate::i18n::set_english(false);
        let mut app = App::new(Store::default());
        app.tv.panel = (3840, 2160);
        app.tv.refresh = 120;
        let mut h = demo_host();
        assert_eq!(app.screen_line(&h), "Écran de 3840 × 2160 à 120 Hz, à la taille de cet écran");
        h.settings.resolution = 1080;
        h.settings.fps = 60;
        assert_eq!(app.screen_line(&h), "Écran de 1920 × 1080 à 60 Hz");
        app.live.insert("a".into(), HostLive { online: Online::Online, virtual_display: Some(false), ..Default::default() });
        assert_eq!(app.screen_line(&h), "Les écrans de l'ordinateur, tels quels");
    }

    #[test]
    fn station_restart_and_update() {
        crate::i18n::set_english(false);
        let mut app = App::new(Store::default());
        app.store.hosts.push(demo_host());
        app.screen = Screen::Settings;
        app.polled("a", info("0.5.0"));

        // Redémarrage : un appareil en session, puis « quand même ».
        let a = app.adjust(Setting::Restart, 1);
        assert_eq!(a, vec![Action::Station { host: "a".into(), op: StationOp::Restart, force: false }]);
        app.station_answer("a", StationOp::Restart, Err(StationRefusal::InSession(Some("Pixel 9".into()))));
        assert!(matches!(&app.dialog, Some(Dialog::Force { device, .. }) if device == "Pixel 9"));
        let a = app.activate(Target::DialogOk);
        assert_eq!(a, vec![Action::Station { host: "a".into(), op: StationOp::Restart, force: true }]);
        app.station_answer("a", StationOp::Restart, Ok(None));
        assert_eq!(app.station_waiting(), Some("a"));
        // Encore là (pas encore arrêtée), puis arrêtée, puis revenue.
        app.polled("a", info("0.5.0"));
        assert_eq!(app.station_waiting(), Some("a"));
        app.polled("a", Polled::default());
        app.polled("a", info("0.5.0"));
        assert_eq!(app.station_waiting(), None);
        assert_eq!(app.station.as_ref().and_then(|s| s.message.as_deref()), Some("La Station a redémarré."));

        // Mise à jour : finie quand la Station revient dans la version visée.
        app.adjust(Setting::Update, 1);
        app.station_answer("a", StationOp::Update, Ok(Some("0.5.1".into())));
        app.polled("a", Polled::default());
        app.polled("a", info("0.5.1"));
        assert_eq!(app.station.as_ref().and_then(|s| s.message.as_deref()), Some("KyberNow Station 0.5.1 est en place."));

        // Trop tôt, Station trop ancienne : un message, rien en cours.
        app.adjust(Setting::Restart, 1);
        app.station_answer("a", StationOp::Restart, Err(StationRefusal::TooSoon));
        assert!(app.station.as_ref().is_some_and(|s| s.phase == StationPhase::Idle && s.message.as_deref() == Some("La Station vient de redémarrer. Réessayez dans une minute.")));
        app.adjust(Setting::Restart, 1);
        app.station_answer("a", StationOp::Restart, Err(StationRefusal::Unsupported));
        assert_eq!(app.station.as_ref().and_then(|s| s.message.as_deref()), Some(station_too_old()));
    }

    #[test]
    fn wake_targets_kept_and_used() {
        let mut app = App::new(Store::default());
        app.store.hosts.push(demo_host());
        let target = WakeTarget { mac: "3c:7c:3f:12:ab:cd".into(), ..Default::default() };
        // Hors ligne sans cible : « Réessayer » sonde de nouveau.
        app.polled("a", Polled::default());
        assert_eq!(app.activate(Target::Connect), vec![Action::Poll("a".into())]);
        // Les cibles d'Admin sont gardées, même si un sondage suivant n'en donne pas.
        assert!(app.polled("a", Polled { online: true, wake: vec![target.clone()], ..Default::default() }));
        app.polled("a", Polled { online: true, ..Default::default() });
        assert_eq!(app.store.hosts[0].wake, vec![target]);
        // Hors ligne avec des cibles : réveil, puis sondé souvent jusqu'au retour.
        app.polled("a", Polled::default());
        assert_eq!(app.activate(Target::Connect), vec![Action::Wake("a".into())]);
        assert_eq!(app.fast_poll(), vec!["a".to_string()]);
        app.polled("a", Polled::default());
        assert_eq!(app.waking(), Some("a"));
        app.polled("a", Polled { online: true, ..Default::default() });
        assert!(app.waking().is_none() && app.fast_poll().is_empty());
    }

    #[test]
    fn remove_asks_first() {
        let mut app = App::new(Store::default());
        app.store.hosts.push(demo_host());
        app.screen = Screen::Settings;
        app.adjust(Setting::Remove, 1);
        assert_eq!(app.store.hosts.len(), 1);
        assert_eq!(app.focus, Target::DialogCancel);
        app.nav(Nav::Back);
        assert!(app.dialog.is_none() && app.store.hosts.len() == 1);
        assert_eq!(app.focus, Target::Setting(Setting::Remove));
        app.adjust(Setting::Remove, 1);
        app.activate(Target::DialogOk);
        assert!(app.store.hosts.is_empty() && app.screen == Screen::Home);
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
            wake: Vec::new(),
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
