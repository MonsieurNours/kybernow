//! Consoles PlayStation sur le téléviseur (docs/playstation.md ; textes : docs/link-ui.md, « Consoles
//! PlayStation ») : l'état de l'interface et ce que la télécommande y fait, sans le cœur (les captures de
//! l'interface se compilent sans lui). Le pilote (`device/playstation.rs`) fait le reste : recherche, association,
//! réveil, partie, envoi entre appareils.
//!
//! Ce qui change d'un appareil à écran et clavier : pas de connexion au compte Sony (pas de navigateur) ; un compte
//! arrive d'un autre appareil (« Recevoir d'un autre appareil », le code à 6 chiffres en grand) ou s'écrit à la main
//! au pavé ; le code de la console se tape au pavé des chiffres ; le clavier de la console s'écrit au pavé des
//! lettres.

use std::collections::HashMap;
use std::time::Instant;

use crate::app::{Action, App, MenuItem, Phase, Screen, SessionUi, Target};
use crate::i18n::{english, tr};
use crate::model::{PsConsole, PsSettings};

/// L'état d'une console, vu de l'accueil.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PsState {
    /// Pas encore de réponse depuis le lancement.
    #[default]
    Unknown,
    On,
    Standby,
    Offline,
    /// Ne répond pas en direct, mais une Station joignable par le relais la voit.
    Away,
}

impl PsState {
    pub fn label(self) -> &'static str {
        match self {
            PsState::On => tr("Allumée", "On"),
            PsState::Standby => tr("En veille", "In rest mode"),
            PsState::Offline => tr("Hors ligne", "Offline"),
            PsState::Unknown => tr("Recherche…", "Searching…"),
            PsState::Away => tr("Hors de chez vous", "Away from home"),
        }
    }

    pub fn lit(self) -> bool {
        matches!(self, PsState::On | PsState::Standby | PsState::Away)
    }
}

/// Une console qui a répondu à la recherche (sur le réseau, ou à une adresse retenue).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PsFound {
    pub address: String,
    /// En majuscules.
    pub host_id: String,
    pub name: String,
    /// « PS5 » ou « PS4 ».
    pub model: String,
    pub state: PsState,
    /// `TargetName` du cœur (pour l'association).
    pub target: String,
    pub running_app: Option<String>,
}

/// Ce qu'on sait d'une console associée.
#[derive(Clone, Debug, Default)]
pub struct PsLive {
    pub state: PsState,
    pub running_app: Option<String>,
    /// Quand elle a répondu pour la dernière fois.
    pub seen: Option<Instant>,
}

/// Les étapes de l'écran « PlayStation » de l'ajout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PsStep {
    /// Comptes, consoles sur ce réseau, envoyer à un appareil.
    #[default]
    Main,
    /// « Recevoir d'un autre appareil » : le code à 6 chiffres en grand, et « Saisir l'identifiant à la main ».
    Receive,
    /// « Saisir l'identifiant à la main » : identifiant et nom (ou seulement le nom : « Renommer »).
    Manual,
    /// « Avec quel compte ? »
    Choose,
    /// Le code à 8 chiffres de la console.
    Code,
    /// « Envoyer à un appareil » : l'appareil choisi, le code à 6 chiffres qu'il affiche.
    Send,
}

/// La console qu'on associe.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PsPairing {
    pub host_id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    pub target: String,
    /// Le compte choisi (`PsAccount::id`).
    pub account: Option<String>,
}

/// Un appareil du réseau qui attend un envoi.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsWaiting {
    pub name: String,
    pub platform: String,
    /// `ip:port`.
    pub address: String,
}

impl PsWaiting {
    /// « {nom} · Apple TV », « {nom} · LG »…
    pub fn line(&self) -> String {
        // Les libellés communs des plateformes (`platform` des réponses aux recherches).
        let p = match self.platform.as_str() {
            "tvos" => "Apple TV",
            "ios" => "iPhone",
            "ipados" => "iPad",
            "macos" => "Mac",
            "windows" => "PC",
            "linux" => "Linux",
            "steamdeck" => "Steam Deck",
            "raspberrypi" => "Raspberry Pi",
            "android" => "Android",
            "androidtv" => "Android TV",
            "webos" => "LG",
            other => other,
        };
        format!("{} · {p}", self.name)
    }
}

/// Le champ en cours de saisie à « Saisir l'identifiant à la main ».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ManualField {
    #[default]
    Id,
    Name,
}

/// L'écran « PlayStation » de l'ajout.
#[derive(Clone, Debug, Default)]
pub struct PsAdd {
    pub step: PsStep,
    /// La console qu'on associe (`Choose`, `Code`), et les comptes proposés à `Choose`.
    pub pairing: Option<PsPairing>,
    pub code: String,
    pub manual_id: String,
    pub manual_name: String,
    pub manual_field: ManualField,
    /// « Renommer » : le compte dont on change le nom (seul le champ « Nom »).
    pub rename: Option<String>,
    pub shift: bool,
    /// Le code à 6 chiffres affiché pour « Recevoir d'un autre appareil », et l'attente qui a expiré.
    pub receive_code: Option<String>,
    pub receive_expired: bool,
    /// Les appareils qui attendent un envoi, celui choisi et le code tapé.
    pub waiting: Vec<PsWaiting>,
    pub send_to: Option<PsWaiting>,
    pub send_code: String,
    /// Un avis (réussite) ou une erreur, sous l'étape.
    pub message: Option<(String, bool)>,
    /// Une demande en cours (association, envoi, identifiant vérifié).
    pub busy: bool,
    /// D'où l'on vient (Retour à la première étape y ramène).
    pub from: Option<Screen>,
}

/// Ce que l'on confirme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PsDialog {
    /// « Retirer le compte {nom}… » (`PsAccount::id`).
    RemoveAccount(String),
    /// « Retirer {nom} » : la console et ses associations (`host_id`).
    RemoveConsole(String),
}

/// Les lignes des réglages d'une console.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsSetting {
    Resolution,
    Fps,
    Bitrate,
    Codec,
    Hdr,
    Rumble,
    Triggers,
    Remove,
}

impl PsSetting {
    pub fn is_action(self) -> bool {
        self == PsSetting::Remove
    }
}

/// Les rubriques des réglages d'une console sur le téléviseur : Image et Commandes (« Console » : la retirer).
/// Pas de plein écran (toujours plein écran), pas d'agrandissement (le téléviseur agrandit lui-même), pas de
/// disposition du clavier et de la souris (la disposition par défaut du cœur, flèches et OK compris).
pub const PS_SECTIONS: [PsSection; 3] = [PsSection::Picture, PsSection::Controls, PsSection::Console];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsSection {
    Picture,
    Controls,
    Console,
}

impl PsSection {
    pub fn label(self) -> &'static str {
        match self {
            PsSection::Picture => tr("Image", "Picture"),
            PsSection::Controls => tr("Commandes", "Controls"),
            PsSection::Console => tr("Console", "Console"),
        }
    }
}

/// Ce qu'on choisit dans les écrans des consoles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PsTarget {
    /// Accueil : une console de la rangée (indice dans `PsStore::consoles`), une console trouvée pas encore associée
    /// (indice dans `PsUi::new_found`), le bouton principal, la puce du compte et ses choix, la puce HDR, la puce
    /// « Auto » ou « Personnalisé », l'engrenage.
    Card(usize),
    FoundCard(usize),
    Primary,
    AccountChip,
    AccountPick(usize),
    HdrChip,
    CustomChip,
    Gear,
    /// Ajout : le bouton « PlayStation » de l'écran d'ajout.
    OpenTab,
    /// Écran PlayStation : « Ajouter un compte », « Recevoir d'un autre appareil », « Saisir l'identifiant à la
    /// main », « Renommer » et « Retirer » d'un compte, « Associer » (et « Associer avec un autre compte ») d'une
    /// console du réseau, un compte de « Avec quel compte ? », les champs et boutons des étapes.
    AddAccount,
    Receive,
    Manual,
    Rename(usize),
    RemoveAccount(usize),
    Pair(usize),
    PairOther(usize),
    Choose(usize),
    ManualId,
    ManualName,
    ManualGo,
    PairGo,
    NewCode,
    SendTo(usize),
    SendGo,
    /// Réglages d'une console : une rubrique, une ligne.
    Section(usize),
    Setting(PsSetting),
    /// Confirmation.
    DialogOk,
    DialogCancel,
    /// Clavier de la console : valider, annuler.
    KeyboardAccept,
    KeyboardCancel,
}

/// Ce que l'interface demande au pilote pour les consoles.
#[derive(Clone, Debug, PartialEq)]
pub enum PsAction {
    /// Lancer une partie (la console est réveillée d'abord, ou jointe par une Station hors de chez soi).
    Play { host_id: String },
    /// Chercher une console à cette adresse (« Par adresse ») ; `then_station` : puis la Station à ce port.
    ProbeAddress { address: String, then_station: Option<u16> },
    /// Associer (`pin` : le code à 8 chiffres).
    Pair { pairing: PsPairing, psn_id: String, pin: u32 },
    /// Un compte saisi à la main : l'identifiant (base64, ou le numéro du compte) vérifié par le cœur.
    AddManual { id: String, name: String, then_pair: bool },
    /// « Recevoir d'un autre appareil » : commencer à attendre (nouveau code), arrêter.
    ReceiveStart,
    ReceiveStop,
    /// « Envoyer à un appareil » : chercher les appareils en attente, envoyer.
    Discover,
    Send { to: PsWaiting, code: String },
    /// Partie : couper le son, mettre la console en veille, les intensités de la DualSense, le clavier de la console.
    Mute(bool),
    Sleep,
    Intensities(u8, u8),
    KeyboardText(String),
    KeyboardAccept,
    KeyboardReject,
}

/// Les mesures d'une partie (une fois par seconde, `Play::stats`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsStats {
    pub width: u32,
    pub height: u32,
    pub fps_target: u32,
    pub hevc: bool,
    pub hdr: bool,
    pub target_kbps: u32,
    pub fps: f32,
    pub mbps: f32,
    /// Depuis le début de la partie.
    pub lost: u64,
    pub recovered: u64,
}

impl PsStats {
    /// « 1920 × 1080 · 60 i/s · H.265 · HDR · 15 Mbit/s demandés ».
    pub fn line1(&self) -> String {
        let codec = if self.hevc { "H.265" } else { "H.264" };
        let hdr = if self.hdr { " · HDR" } else { "" };
        let mbps = (self.target_kbps as f64 / 1000.0).round();
        if english() {
            format!("{} × {} · {} fps · {codec}{hdr} · {mbps} Mbit/s requested", self.width, self.height, self.fps_target)
        } else {
            format!("{} × {} · {} i/s · {codec}{hdr} · {mbps} Mbit/s demandés", self.width, self.height, self.fps_target)
        }
    }

    /// « Reçu : 59 i/s · 12,4 Mbit/s ».
    pub fn line2(&self) -> String {
        if english() {
            format!("Received: {} fps · {} Mbit/s", self.fps.round(), crate::i18n::decimal(self.mbps as f64, 1))
        } else {
            format!("Reçu : {} i/s · {} Mbit/s", self.fps.round(), crate::i18n::decimal(self.mbps as f64, 1))
        }
    }

    /// « Images perdues : {n} · réparées : {n} ».
    pub fn line3(&self) -> String {
        if english() { format!("Pictures lost: {} · repaired: {}", self.lost, self.recovered) } else { format!("Images perdues : {} · réparées : {}", self.lost, self.recovered) }
    }
}

/// Une partie sur une console, vue par l'interface (dans `SessionUi::ps`).
#[derive(Clone, Debug, Default)]
pub struct PsSessionUi {
    pub host_id: String,
    /// La console était en veille : « Réveil de la console » est la première étape de la connexion.
    pub waking: bool,
    /// « La console ne peut pas diffuser ce qu'elle affiche. »
    pub cant_display: bool,
    pub muted: bool,
    pub stats: Option<PsStats>,
    /// Le clavier de la console est ouvert : son texte.
    pub keyboard: Option<String>,
    pub keyboard_shift: bool,
}

/// Tout ce que l'interface sait des consoles.
#[derive(Clone, Debug, Default)]
pub struct PsUi {
    pub found: Vec<PsFound>,
    pub live: HashMap<String, PsLive>,
    pub add: PsAdd,
    /// Réveil envoyé depuis l'accueil (une partie qui démarre) : `host_id`.
    pub waking: Option<String>,
    /// Le choix du compte ouvert sous la puce « {pseudo} ».
    pub account_menu: bool,
    /// Réglages d'une console : laquelle, la rubrique, d'où l'on vient.
    pub settings: Option<String>,
    pub section: usize,
    pub settings_from: Option<Screen>,
    pub dialog: Option<PsDialog>,
}

/// « 1 console », « 3 consoles », « Aucune console ».
pub fn consoles_label(n: usize) -> String {
    match n {
        0 => tr("Aucune console", "No console").into(),
        1 => "1 console".into(),
        n => format!("{n} consoles"),
    }
}

/// « Réglages de {nom} ».
pub fn settings_for(name: &str) -> String {
    if english() { format!("Settings for {name}") } else { format!("Réglages {}{name}", crate::i18n::de(name)) }
}

/// Le texte d'une fin de partie (`code` : celui du pilote) ; `None` : arrêtée par l'app.
pub fn end_text(code: PsEnd) -> Option<String> {
    let t = match code {
        PsEnd::Stopped => return None,
        PsEnd::InUse => tr("La console est déjà jouée à distance par un autre appareil.", "The console is already being played remotely by another device."),
        PsEnd::NotPaired => tr("La console ne reconnaît plus cet appareil : associez-la de nouveau.", "The console no longer recognizes this device: pair it again."),
        PsEnd::Unreachable => tr("La console ne répond pas.", "The console isn't answering."),
        PsEnd::Remote => tr("La console a terminé la partie.", "The console ended the session."),
    };
    Some(t.into())
}

/// Pourquoi une partie finit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsEnd {
    Stopped,
    InUse,
    NotPaired,
    Unreachable,
    Remote,
}

/// Une erreur d'association, dite comme link-ui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairError {
    NoAnswer,
    WrongCode,
    Account,
    Unreachable,
}

impl PairError {
    pub fn text(self) -> &'static str {
        match self {
            PairError::NoAnswer => tr("La console ne répond pas : gardez l'écran « Associer un appareil » ouvert.", "The console isn't answering: keep the “Link Device” screen open."),
            PairError::WrongCode => tr("Code refusé : il change toutes les 5 minutes.", "Code refused: it changes every 5 minutes."),
            PairError::Account => tr("La console refuse ce compte.", "The console refuses this account."),
            PairError::Unreachable => tr("Console injoignable.", "Console unreachable."),
        }
    }
}

/// Ce qu'un envoi reçu a ajouté (comptes, consoles).
pub fn received_label(accounts: usize, consoles: usize) -> String {
    if english() {
        format!("{accounts} account(s) and {consoles} console(s) received.")
    } else {
        format!("{accounts} compte(s) et {consoles} console(s) reçus.")
    }
}

impl App {
    /// La console choisie sur l'accueil.
    pub fn selected_console(&self) -> Option<&PsConsole> {
        let id = self.store.selected.as_deref()?.strip_prefix("ps:")?;
        self.store.playstation.console(id)
    }

    /// Les consoles trouvées qui ne sont pas encore associées (avec aucun compte).
    pub fn ps_new_found(&self) -> Vec<&PsFound> {
        self.ps.found.iter().filter(|f| self.store.playstation.console(&f.host_id).is_none_or(|c| c.pairings.is_empty())).collect()
    }

    /// L'état d'une console : ce que la recherche en dit.
    pub fn ps_state(&self, host_id: &str) -> PsState {
        self.ps.live.get(host_id).map(|l| l.state).unwrap_or_default()
    }

    /// La puce « HDR » d'une console : PS5, écran et décodeur HDR du téléviseur, H.264 pas imposé.
    pub fn ps_hdr_available(&self, c: &PsConsole) -> bool {
        c.ps5() && self.tv.hdr && self.tv.hdr_decoder && c.settings.codec != "h264"
    }

    /// Les comptes qui ne sont pas encore associés à cette console.
    pub fn ps_candidates(&self, host_id: &str) -> Vec<String> {
        let c = self.store.playstation.console(host_id);
        self.store.playstation.accounts.iter().filter(|a| c.is_none_or(|c| !c.pairings.contains_key(&a.id))).map(|a| a.id.clone()).collect()
    }

    /// Ouvre l'écran « PlayStation » de l'ajout : sans compte, directement « Recevoir d'un autre appareil » (une télé
    /// n'a pas de navigateur pour la connexion Sony).
    pub fn ps_open(&mut self, from: Screen) -> Vec<Action> {
        self.ps.add = PsAdd { from: Some(from), ..Default::default() };
        self.screen = Screen::PlayStation;
        if self.store.playstation.accounts.is_empty() {
            return self.ps_step(PsStep::Receive);
        }
        self.ps_step(PsStep::Main)
    }

    /// Passe à une étape de l'écran « PlayStation » (l'attente d'un envoi ne vit que le temps de son étape).
    fn ps_step(&mut self, step: PsStep) -> Vec<Action> {
        let mut out = Vec::new();
        let was = self.ps.add.step;
        if was == PsStep::Receive && step != PsStep::Receive {
            out.push(Action::Ps(PsAction::ReceiveStop));
        }
        self.ps.add.step = step;
        self.ps.add.busy = false;
        self.ps.add.shift = false;
        self.focus = match step {
            PsStep::Main => {
                if self.store.playstation.accounts.is_empty() {
                    Target::Ps(PsTarget::AddAccount)
                } else if !self.ps.found.is_empty() {
                    Target::Ps(PsTarget::Pair(0))
                } else {
                    Target::Ps(PsTarget::AddAccount)
                }
            }
            PsStep::Receive => Target::Ps(PsTarget::Manual),
            PsStep::Manual => Target::Key('1'),
            PsStep::Choose => Target::Ps(PsTarget::Choose(0)),
            PsStep::Code | PsStep::Send => Target::Key('1'),
        };
        match step {
            PsStep::Receive => {
                self.ps.add.receive_code = None;
                self.ps.add.receive_expired = false;
                out.push(Action::Ps(PsAction::ReceiveStart));
            }
            PsStep::Main => out.push(Action::Ps(PsAction::Discover)),
            _ => {}
        }
        out
    }

    /// Commence l'association d'une console : le choix du compte s'il y en a deux ou plus, l'ajout d'un compte s'il
    /// n'y en a aucun, sinon directement le code.
    pub fn ps_start_pairing(&mut self, p: PsPairing) -> Vec<Action> {
        let mut out = Vec::new();
        if self.screen != Screen::PlayStation {
            self.ps.add = PsAdd { from: Some(self.screen), ..Default::default() };
            self.screen = Screen::PlayStation;
        }
        let candidates = self.ps_candidates(&p.host_id);
        self.ps.add.message = None;
        self.ps.add.code.clear();
        self.ps.add.pairing = Some(p);
        match candidates.len() {
            0 => out.extend(self.ps_step(PsStep::Receive)),
            1 => {
                if let Some(pp) = self.ps.add.pairing.as_mut() {
                    pp.account = Some(candidates[0].clone());
                }
                out.extend(self.ps_step(PsStep::Code));
            }
            _ => out.extend(self.ps_step(PsStep::Choose)),
        }
        out
    }

    /// La console associable d'une ligne de l'écran (une console trouvée, avec son adresse).
    fn ps_pairing_of(&self, f: &PsFound) -> PsPairing {
        PsPairing { host_id: f.host_id.clone(), name: f.name.clone(), model: f.model.clone(), address: f.address.clone(), target: f.target.clone(), account: None }
    }

    pub fn ps_activate(&mut self, t: PsTarget) -> Vec<Action> {
        match t {
            PsTarget::Card(i) => {
                let Some(key) = self.store.playstation.consoles.get(i).map(PsConsole::key) else { return Vec::new() };
                self.store.selected = Some(key);
                self.ps.account_menu = false;
                self.focus = Target::Ps(PsTarget::Primary);
                vec![Action::Save]
            }
            PsTarget::FoundCard(i) => {
                let Some(f) = self.ps_new_found().get(i).map(|f| (*f).clone()) else { return Vec::new() };
                let p = self.ps_pairing_of(&f);
                self.ps_start_pairing(p)
            }
            PsTarget::Primary => self.ps_primary(),
            PsTarget::AccountChip => {
                self.ps.account_menu = !self.ps.account_menu;
                if self.ps.account_menu {
                    self.focus = Target::Ps(PsTarget::AccountPick(0));
                }
                Vec::new()
            }
            PsTarget::AccountPick(i) => {
                let Some(c) = self.selected_console() else { return Vec::new() };
                let host_id = c.host_id.clone();
                let ids: Vec<String> = c.paired_accounts(&self.store.playstation.accounts).iter().map(|a| a.id.clone()).collect();
                let Some(id) = ids.get(i).cloned() else { return Vec::new() };
                if let Some(c) = self.store.playstation.console_mut(&host_id) {
                    c.account = Some(id);
                }
                self.ps.account_menu = false;
                self.focus = Target::Ps(PsTarget::AccountChip);
                vec![Action::Save]
            }
            PsTarget::HdrChip => {
                let Some(c) = self.selected_console().cloned() else { return Vec::new() };
                if !self.ps_hdr_available(&c) {
                    return Vec::new();
                }
                if let Some(c) = self.store.playstation.console_mut(&c.host_id) {
                    c.hdr = !c.hdr;
                }
                vec![Action::Save]
            }
            PsTarget::CustomChip | PsTarget::Gear => {
                let Some(id) = self.selected_console().map(|c| c.host_id.clone()) else { return Vec::new() };
                self.ps_open_settings(&id, Screen::Home);
                Vec::new()
            }
            PsTarget::OpenTab => self.ps_open(Screen::Add),
            PsTarget::AddAccount | PsTarget::Receive => self.ps_step(PsStep::Receive),
            PsTarget::Manual => {
                self.ps.add.rename = None;
                self.ps.add.manual_id.clear();
                self.ps.add.manual_name.clear();
                self.ps.add.manual_field = ManualField::Id;
                self.ps.add.message = None;
                self.ps_step(PsStep::Manual)
            }
            PsTarget::Rename(i) => {
                let Some(a) = self.store.playstation.accounts.get(i).cloned() else { return Vec::new() };
                self.ps.add.rename = Some(a.id);
                self.ps.add.manual_name = a.name.unwrap_or_default();
                self.ps.add.manual_field = ManualField::Name;
                self.ps.add.message = None;
                let out = self.ps_step(PsStep::Manual);
                self.focus = Target::Key('q');
                out
            }
            PsTarget::RemoveAccount(i) => {
                let Some(a) = self.store.playstation.accounts.get(i) else { return Vec::new() };
                self.ps.dialog = Some(PsDialog::RemoveAccount(a.id.clone()));
                self.focus = Target::Ps(PsTarget::DialogCancel);
                Vec::new()
            }
            PsTarget::Pair(i) | PsTarget::PairOther(i) => {
                let Some(f) = self.ps.found.get(i).cloned() else { return Vec::new() };
                let p = self.ps_pairing_of(&f);
                self.ps_start_pairing(p)
            }
            PsTarget::Choose(i) => {
                let Some(p) = self.ps.add.pairing.as_ref() else { return Vec::new() };
                let candidates = self.ps_candidates(&p.host_id);
                // Après les comptes : « Ajouter un compte ».
                match candidates.get(i) {
                    Some(id) => {
                        let id = id.clone();
                        if let Some(p) = self.ps.add.pairing.as_mut() {
                            p.account = Some(id);
                        }
                        self.ps.add.code.clear();
                        self.ps_step(PsStep::Code)
                    }
                    None => self.ps_step(PsStep::Receive),
                }
            }
            PsTarget::ManualId => {
                self.ps.add.manual_field = ManualField::Id;
                Vec::new()
            }
            PsTarget::ManualName => {
                self.ps.add.manual_field = ManualField::Name;
                Vec::new()
            }
            PsTarget::ManualGo => self.ps_manual_go(),
            PsTarget::PairGo => self.ps_pair_go(),
            PsTarget::NewCode => self.ps_step(PsStep::Receive),
            PsTarget::SendTo(i) => {
                let Some(w) = self.ps.add.waiting.get(i).cloned() else { return Vec::new() };
                self.ps.add.send_to = Some(w);
                self.ps.add.send_code.clear();
                self.ps.add.message = None;
                self.ps_step(PsStep::Send)
            }
            PsTarget::SendGo => {
                let Some(to) = self.ps.add.send_to.clone() else { return Vec::new() };
                if self.ps.add.send_code.len() != 6 || self.ps.add.busy {
                    return Vec::new();
                }
                self.ps.add.busy = true;
                self.ps.add.message = None;
                vec![Action::Ps(PsAction::Send { to, code: self.ps.add.send_code.clone() })]
            }
            PsTarget::Section(i) => {
                self.ps.section = i.min(PS_SECTIONS.len() - 1);
                self.focus = match PS_SECTIONS[self.ps.section] {
                    PsSection::Picture => Target::Ps(PsTarget::Setting(PsSetting::Resolution)),
                    PsSection::Controls => Target::Ps(PsTarget::Setting(PsSetting::Rumble)),
                    PsSection::Console => Target::Ps(PsTarget::Setting(PsSetting::Remove)),
                };
                Vec::new()
            }
            PsTarget::Setting(s) => self.ps_adjust(s, 1),
            PsTarget::DialogOk => self.ps_dialog_ok(),
            PsTarget::DialogCancel => {
                let d = self.ps.dialog.take();
                self.focus = match d {
                    Some(PsDialog::RemoveConsole(_)) => Target::Ps(PsTarget::Setting(PsSetting::Remove)),
                    _ => Target::Ps(PsTarget::AddAccount),
                };
                Vec::new()
            }
            PsTarget::KeyboardAccept => self.ps_keyboard_close(true),
            PsTarget::KeyboardCancel => self.ps_keyboard_close(false),
        }
    }

    /// Le bouton principal de l'accueil d'une console.
    fn ps_primary(&mut self) -> Vec<Action> {
        let Some(c) = self.selected_console().cloned() else { return Vec::new() };
        if self.session.is_some() {
            return Vec::new();
        }
        let state = self.ps_state(&c.host_id);
        if c.pairings.is_empty() || c.play_account(&self.store.playstation.accounts).is_none() {
            let f = self.ps.found.iter().find(|f| f.host_id == c.host_id).cloned();
            let p = PsPairing {
                host_id: c.host_id.clone(),
                name: c.name.clone(),
                model: c.model.clone(),
                address: f.as_ref().map(|f| f.address.clone()).or_else(|| c.addresses.first().cloned()).unwrap_or_default(),
                target: f.map(|f| f.target).unwrap_or_else(|| c.target.clone()),
                account: None,
            };
            return self.ps_start_pairing(p);
        }
        if matches!(state, PsState::Offline | PsState::Unknown) {
            // Une recherche de plus : l'état revient avec elle.
            self.ps.live.entry(c.host_id.clone()).or_default().state = PsState::Unknown;
            return vec![Action::Ps(PsAction::Discover)];
        }
        if !self.tv.decoder {
            self.toast(tr("Décodeur du téléviseur introuvable : webOS 5 (2020) au moins", "TV decoder not found: webOS 5 (2020) or later required"));
            return Vec::new();
        }
        let waking = state == PsState::Standby;
        let mut ui = SessionUi::new(&c.key(), &c.name, crate::model::Profile::Jeu, false);
        ui.ps = Some(PsSessionUi { host_id: c.host_id.clone(), waking, ..Default::default() });
        self.session = Some(ui);
        self.screen = Screen::Session;
        self.focus = Target::CancelConnect;
        if waking {
            self.ps.waking = Some(c.host_id.clone());
        }
        vec![Action::Ps(PsAction::Play { host_id: c.host_id })]
    }

    /// « Continuer » de la saisie à la main (ou « Renommer »).
    fn ps_manual_go(&mut self) -> Vec<Action> {
        let name = self.ps.add.manual_name.trim().to_string();
        if let Some(id) = self.ps.add.rename.take() {
            if let Some(a) = self.store.playstation.accounts.iter_mut().find(|a| a.id == id) {
                a.name = (!name.is_empty()).then_some(name);
            }
            let mut out = self.ps_step(PsStep::Main);
            out.push(Action::Save);
            return out;
        }
        let id = self.ps.add.manual_id.trim().to_string();
        if id.is_empty() || self.ps.add.busy {
            self.ps.add.manual_field = ManualField::Id;
            return Vec::new();
        }
        self.ps.add.busy = true;
        let then_pair = self.ps.add.pairing.is_some();
        vec![Action::Ps(PsAction::AddManual { id, name, then_pair })]
    }

    /// « Associer » : le code à 8 chiffres part à la console.
    fn ps_pair_go(&mut self) -> Vec<Action> {
        let Some(p) = self.ps.add.pairing.clone() else { return Vec::new() };
        let Some(psn) = p.account.as_ref().and_then(|id| self.store.playstation.account(id)).map(|a| a.psn_id.clone()) else { return Vec::new() };
        if self.ps.add.code.len() != 8 || self.ps.add.busy {
            return Vec::new();
        }
        let Ok(pin) = self.ps.add.code.parse::<u32>() else { return Vec::new() };
        self.ps.add.busy = true;
        self.ps.add.message = None;
        vec![Action::Ps(PsAction::Pair { pairing: p, psn_id: psn, pin })]
    }

    /// Une touche du pavé (chiffres, lettres) ou du clavier, dans l'écran PlayStation ou le clavier de la console.
    pub fn ps_key(&mut self, t: Target) -> Vec<Action> {
        // Le clavier de la console : chaque changement part en entier.
        if let Some(ps) = self.session.as_mut().and_then(|s| s.ps.as_mut())
            && let Some(text) = ps.keyboard.as_mut()
        {
            match t {
                Target::Key(c) => {
                    let c = if ps.keyboard_shift { c.to_uppercase().next().unwrap_or(c) } else { c };
                    if text.chars().count() < 200 {
                        text.push(c);
                    }
                    ps.keyboard_shift = false;
                }
                Target::KeySpace => text.push(' '),
                Target::KeyErase => {
                    text.pop();
                }
                Target::KeyShift => {
                    ps.keyboard_shift = !ps.keyboard_shift;
                    return Vec::new();
                }
                Target::KeyOk => return self.ps_keyboard_close(true),
                _ => return Vec::new(),
            }
            return vec![Action::Ps(PsAction::KeyboardText(text.clone()))];
        }
        let add = &mut self.ps.add;
        let digits_only = matches!(add.step, PsStep::Code | PsStep::Send);
        let field: Option<&mut String> = match add.step {
            PsStep::Code => Some(&mut add.code),
            PsStep::Send => Some(&mut add.send_code),
            PsStep::Manual => Some(match add.manual_field {
                ManualField::Id => &mut add.manual_id,
                ManualField::Name => &mut add.manual_name,
            }),
            _ => None,
        };
        let Some(field) = field else { return Vec::new() };
        let max = match add.step {
            PsStep::Code => 8,
            PsStep::Send => 6,
            _ => 40,
        };
        match t {
            Target::Key(c) => {
                if digits_only && !c.is_ascii_digit() {
                    return Vec::new();
                }
                let c = if add.shift { c.to_uppercase().next().unwrap_or(c) } else { c };
                if field.chars().count() < max {
                    field.push(c);
                }
                add.shift = false;
            }
            Target::KeySpace if add.step == PsStep::Manual && add.manual_field == ManualField::Name => {
                if field.chars().count() < max {
                    field.push(' ');
                }
            }
            Target::KeyErase => {
                field.pop();
            }
            Target::KeyShift => add.shift = !add.shift,
            Target::KeyOk => {
                return match add.step {
                    PsStep::Code => self.ps_pair_go(),
                    PsStep::Send => self.ps_activate(PsTarget::SendGo),
                    PsStep::Manual if add.manual_field == ManualField::Id && add.rename.is_none() => {
                        add.manual_field = ManualField::Name;
                        Vec::new()
                    }
                    _ => self.ps_manual_go(),
                };
            }
            _ => {}
        }
        Vec::new()
    }

    /// Le clavier de la console se ferme : validé (OK) ou annulé (Retour). Le panneau disparaît tout de suite.
    pub fn ps_keyboard_close(&mut self, accept: bool) -> Vec<Action> {
        let Some(ps) = self.session.as_mut().and_then(|s| s.ps.as_mut()) else { return Vec::new() };
        if ps.keyboard.take().is_none() {
            return Vec::new();
        }
        vec![Action::Ps(if accept { PsAction::KeyboardAccept } else { PsAction::KeyboardReject })]
    }

    /// Le clavier de la console s'écrit en ce moment (le pavé des lettres a la main).
    pub fn ps_keyboard_open(&self) -> bool {
        self.session.as_ref().and_then(|s| s.ps.as_ref()).is_some_and(|p| p.keyboard.is_some())
    }

    /// L'écran PlayStation attend une saisie au pavé.
    pub fn ps_typing(&self) -> bool {
        self.screen == Screen::PlayStation && matches!(self.ps.add.step, PsStep::Manual | PsStep::Code | PsStep::Send)
    }

    /// Retour dans les écrans des consoles.
    pub fn ps_back(&mut self) -> Vec<Action> {
        if self.ps.dialog.is_some() {
            return self.ps_activate(PsTarget::DialogCancel);
        }
        match self.screen {
            Screen::PlayStation => {
                let first = self.store.playstation.accounts.is_empty() && self.ps.add.step == PsStep::Receive;
                match self.ps.add.step {
                    PsStep::Main => self.ps_leave(),
                    _ if first => self.ps_leave(),
                    // Une association en cours : le code revient au choix du compte, ou à la liste.
                    PsStep::Code if self.ps.add.pairing.is_some() && self.ps_candidates(&self.ps.add.pairing.as_ref().unwrap().host_id).len() > 1 => self.ps_step(PsStep::Choose),
                    PsStep::Manual if self.ps.add.rename.is_none() => self.ps_step(PsStep::Receive),
                    _ => {
                        self.ps.add.pairing = None;
                        self.ps.add.message = None;
                        self.ps_step(PsStep::Main)
                    }
                }
            }
            Screen::PsSettings => {
                if matches!(self.focus, Target::Ps(PsTarget::Setting(_))) {
                    self.focus = Target::Ps(PsTarget::Section(self.ps.section));
                    return Vec::new();
                }
                let from = self.ps.settings_from.take().unwrap_or(Screen::Home);
                if from == Screen::Session && self.session.is_some() {
                    self.screen = Screen::Session;
                    self.focus = Target::Menu(MenuItem::Settings);
                } else {
                    self.go(Screen::Home);
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Quitte l'écran PlayStation vers l'écran d'où l'on vient (l'ajout, l'accueil).
    fn ps_leave(&mut self) -> Vec<Action> {
        let out = if self.ps.add.step == PsStep::Receive { vec![Action::Ps(PsAction::ReceiveStop)] } else { Vec::new() };
        let to = self.ps.add.from.take().unwrap_or(Screen::Home);
        self.ps.add.pairing = None;
        self.go(if to == Screen::Add { Screen::Add } else { Screen::Home });
        out
    }

    pub fn ps_open_settings(&mut self, host_id: &str, from: Screen) {
        self.ps.settings = Some(host_id.to_string());
        self.ps.section = 0;
        self.ps.settings_from = Some(from);
        self.ps.dialog = None;
        self.screen = Screen::PsSettings;
        self.focus = Target::Ps(PsTarget::Section(0));
    }

    /// La console dont les réglages sont ouverts.
    pub fn ps_settings_console(&self) -> Option<&PsConsole> {
        self.ps.settings.as_deref().and_then(|id| self.store.playstation.console(id))
    }

    /// Change un réglage de la console (gauche, droite ; OK : la valeur suivante).
    pub fn ps_adjust(&mut self, s: PsSetting, dir: i32) -> Vec<Action> {
        let Some(id) = self.ps.settings.clone() else { return Vec::new() };
        if s == PsSetting::Remove {
            self.ps.dialog = Some(PsDialog::RemoveConsole(id));
            self.focus = Target::Ps(PsTarget::DialogCancel);
            return Vec::new();
        }
        let hdr_screen = self.tv.hdr && self.tv.hdr_decoder;
        let in_play = self.session.as_ref().and_then(|x| x.ps.as_ref()).is_some_and(|p| p.host_id == id);
        let Some(c) = self.store.playstation.console_mut(&id) else { return Vec::new() };
        let ps5 = c.ps5();
        let st: &mut PsSettings = &mut c.settings;
        let mut out = vec![Action::Save];
        match s {
            PsSetting::Resolution => {
                let opts = ["auto", "p1080", "p720", "p540", "p360"];
                let i = opts.iter().position(|v| *v == st.resolution).unwrap_or(0);
                st.resolution = opts[cycle(i, opts.len(), dir)].into();
            }
            PsSetting::Fps => st.fps = if st.fps == 30 { 60 } else { 30 },
            PsSetting::Bitrate => {
                let opts: [Option<u32>; 9] = [None, Some(2_000), Some(5_000), Some(10_000), Some(15_000), Some(20_000), Some(30_000), Some(50_000), Some(100_000)];
                let i = opts.iter().position(|v| *v == st.bitrate_kbps).unwrap_or(0);
                st.bitrate_kbps = opts[cycle(i, opts.len(), dir)];
            }
            PsSetting::Codec => {
                if !ps5 {
                    return Vec::new();
                }
                let opts = ["auto", "h264", "h265"];
                let i = opts.iter().position(|v| *v == st.codec).unwrap_or(0);
                st.codec = opts[cycle(i, opts.len(), dir)].into();
            }
            PsSetting::Hdr => {
                if !ps5 || !hdr_screen {
                    return Vec::new();
                }
                c.hdr = !c.hdr;
            }
            PsSetting::Rumble | PsSetting::Triggers => {
                let v = if s == PsSetting::Rumble { &mut st.rumble } else { &mut st.triggers };
                let i = (*v as i32 / 10 + dir).clamp(0, 10);
                *v = (i * 10) as u8;
                if in_play {
                    out.push(Action::Ps(PsAction::Intensities(st.rumble, st.triggers)));
                }
            }
            PsSetting::Remove => {}
        }
        out
    }

    /// Le bouton qui agit de la confirmation.
    fn ps_dialog_ok(&mut self) -> Vec<Action> {
        match self.ps.dialog.take() {
            Some(PsDialog::RemoveAccount(id)) => {
                self.store.playstation.remove_account(&id);
                if self.selected_console().is_none() && self.store.selected.as_deref().is_some_and(|s| s.starts_with("ps:")) {
                    self.store.selected = None;
                }
                let mut out = if self.store.playstation.accounts.is_empty() { self.ps_step(PsStep::Receive) } else { self.ps_step(PsStep::Main) };
                out.push(Action::Save);
                out
            }
            Some(PsDialog::RemoveConsole(id)) => {
                let key = format!("ps:{}", id.to_ascii_uppercase());
                self.store.playstation.consoles.retain(|c| !c.host_id.eq_ignore_ascii_case(&id));
                if self.store.selected.as_deref() == Some(key.as_str()) {
                    self.store.selected = None;
                }
                self.ps.settings = None;
                self.ps.settings_from = None;
                self.go(Screen::Home);
                vec![Action::Save]
            }
            None => Vec::new(),
        }
    }

    /// Le texte de la confirmation et son bouton.
    pub fn ps_dialog_texts(&self) -> Option<(String, String, &'static str)> {
        match self.ps.dialog.as_ref()? {
            PsDialog::RemoveAccount(id) => {
                let a = self.store.playstation.account(id)?;
                let name = a.label();
                let n = self.store.playstation.consoles_of(id);
                let title = match (n, english()) {
                    (0, false) => format!("Retirer le compte {name} ?"),
                    (1, false) => format!("Retirer le compte {name} et sa console ?"),
                    (n, false) => format!("Retirer le compte {name} et ses {n} consoles ?"),
                    (0, true) => format!("Remove the account {name}?"),
                    (1, true) => format!("Remove the account {name} and its console?"),
                    (n, true) => format!("Remove the account {name} and its {n} consoles?"),
                };
                let body = tr(
                    "Ses associations sont effacées de cet appareil. Une console associée aussi avec un autre compte reste, avec cet autre compte.",
                    "Its pairings are erased from this device. A console also paired with another account stays, with that other account.",
                );
                Some((title, body.into(), tr("Retirer", "Remove")))
            }
            PsDialog::RemoveConsole(id) => {
                let c = self.store.playstation.console(id)?;
                let title = if english() { format!("Remove {}?", c.name) } else { format!("Retirer {} ?", c.name) };
                let body = tr(
                    "Ses associations sont effacées de ce téléviseur. Pour y jouer de nouveau, associez-la ou recevez-la d'un autre appareil.",
                    "Its pairings are erased from this TV. To play on it again, pair it or receive it from another device.",
                );
                Some((title, body.into(), tr("Retirer", "Remove")))
            }
        }
    }

    // MARK: - Ce que le pilote rapporte

    /// La recherche a répondu : les consoles trouvées, l'état des consoles associées (« Hors ligne » après deux
    /// recherches sans réponse ; « Hors de chez vous » si une Station qui la voit n'est joignable que par le relais).
    pub fn ps_found(&mut self, found: Vec<PsFound>) {
        let away: Vec<String> = self
            .store
            .playstation
            .consoles
            .iter()
            .filter(|c| c.stations.iter().any(|h| self.live.get(h).is_some_and(|l| l.online == crate::app::Online::Away)))
            .map(|c| c.host_id.clone())
            .collect();
        let now = Instant::now();
        let mut changed = false;
        for f in &found {
            let l = self.ps.live.entry(f.host_id.clone()).or_default();
            l.state = f.state;
            l.running_app = f.running_app.clone();
            l.seen = Some(now);
            // Une console associée garde son adresse et son nom à jour.
            if let Some(c) = self.store.playstation.console_mut(&f.host_id)
                && (c.addresses.first() != Some(&f.address) || c.name != f.name || c.target != f.target)
            {
                self.store.playstation.put_console(&f.host_id, &f.name, &f.model, &f.target, Some(&f.address));
                changed = true;
            }
        }
        for c in &self.store.playstation.consoles {
            if found.iter().any(|f| f.host_id == c.host_id) {
                continue;
            }
            let l = self.ps.live.entry(c.host_id.clone()).or_default();
            let quiet = l.seen.is_none_or(|t| now.duration_since(t).as_secs() > 6);
            if away.contains(&c.host_id) {
                l.state = PsState::Away;
            } else if quiet {
                l.state = PsState::Offline;
                l.running_app = None;
            }
        }
        self.ps.found = found;
        if changed {
            self.store.save();
        }
    }

    /// Une association a réussi : gardée pour ce compte ; l'accueil montre la console.
    pub fn ps_paired(&mut self, p: &PsPairing, registered: serde_json::Value) {
        let Some(account) = p.account.clone() else { return };
        let c = self.store.playstation.put_console(&p.host_id, &p.name, &p.model, &p.target, Some(&p.address));
        c.pairings.insert(account.clone(), registered);
        if c.account.is_none() {
            c.account = Some(account);
        }
        let key = c.key();
        let name = c.name.clone();
        self.store.selected = Some(key);
        self.store.save();
        self.ps.add.pairing = None;
        self.ps.add.busy = false;
        self.ps.add.from = None;
        self.go(Screen::Home);
        self.toast(if english() { format!("{name} is paired") } else { format!("{name} est associée") });
    }

    pub fn ps_pair_failed(&mut self, e: PairError) {
        self.ps.add.busy = false;
        self.ps.add.code.clear();
        self.ps.add.message = Some((e.text().into(), true));
    }

    /// Un compte saisi à la main, vérifié par le cœur (`psn_id` en base64), ou refusé.
    pub fn ps_manual_done(&mut self, r: Result<String, ()>, name: String, then_pair: bool) -> Vec<Action> {
        self.ps.add.busy = false;
        let Ok(psn_id) = r else {
            self.ps.add.message = Some((tr("Identifiant du compte illisible : le numéro du compte, ou son identifiant en base64.", "Unreadable account ID: the account number, or its ID in base64.").into(), true));
            self.ps.add.manual_field = ManualField::Id;
            return Vec::new();
        };
        let (id, existed) = self.store.playstation.put_account(&psn_id, Some(name).filter(|n| !n.trim().is_empty()));
        self.store.save();
        let mut out = vec![Action::Save];
        if then_pair && let Some(p) = self.ps.add.pairing.as_mut() {
            p.account = Some(id);
            self.ps.add.code.clear();
            out.extend(self.ps_step(PsStep::Code));
            if existed {
                self.ps.add.message = Some((tr("Ce compte est déjà là.", "This account is already here.").into(), false));
            }
            return out;
        }
        out.extend(self.ps_step(PsStep::Main));
        if existed {
            self.ps.add.message = Some((tr("Ce compte est déjà là.", "This account is already here.").into(), false));
        }
        out
    }

    /// Un envoi est arrivé : comptes et consoles fusionnés (même identifiant de compte ou même console, rien en
    /// double, les associations s'ajoutent).
    pub fn ps_received(&mut self, bundle: &str) -> Vec<Action> {
        let (accounts, consoles) = merge_bundle(&mut self.store.playstation, bundle, &self.store.hosts);
        self.store.save();
        let text = received_label(accounts, consoles);
        // Une association en cours (aucun compte au départ) continue avec le compte reçu.
        if let Some(host_id) = self.ps.add.pairing.as_ref().map(|p| p.host_id.clone())
            && self.store.playstation.console(&host_id).is_none_or(|c| c.pairings.is_empty())
            && !self.ps_candidates(&host_id).is_empty()
        {
            let p = self.ps.add.pairing.clone().unwrap();
            let mut out = self.ps_start_pairing(p);
            self.ps.add.message = Some((text, false));
            out.push(Action::Save);
            return out;
        }
        let mut out = self.ps_step(PsStep::Main);
        self.ps.add.message = Some((text, false));
        out.push(Action::Save);
        out
    }

    /// L'appareil qui envoie a reçu le paquet, ou non.
    pub fn ps_sent(&mut self, r: Result<(), bool>) {
        self.ps.add.busy = false;
        let name = self.ps.add.send_to.as_ref().map(|w| w.name.clone()).unwrap_or_default();
        match r {
            Ok(()) => {
                self.ps.add.message = Some((if english() { format!("Accounts and consoles sent to {name}.") } else { format!("Comptes et consoles envoyés à {name}.") }, false));
                self.ps.add.step = PsStep::Main;
                self.focus = Target::Ps(PsTarget::AddAccount);
            }
            Err(true) => {
                self.ps.add.send_code.clear();
                self.ps.add.message = Some((if english() { format!("Code refused: check it on {name}.") } else { format!("Code refusé : vérifiez-le sur {name}.") }, true));
            }
            Err(false) => self.ps.add.message = Some((if english() { format!("{name} isn't answering any more.") } else { format!("{name} ne répond plus.") }, true)),
        }
    }

    /// Le paquet que ce téléviseur envoie : ses comptes, leurs associations, ses consoles (format dans
    /// `merge_bundle`).
    pub fn ps_bundle(&self) -> String {
        bundle_of(&self.store.playstation, &self.store.hosts, &self.device_name())
    }

    /// Une partie : son état a changé.
    pub fn ps_phase(&mut self, phase: Phase) {
        let Some(s) = self.session.as_mut() else { return };
        if s.ps.is_none() {
            return;
        }
        if phase == Phase::Streaming && s.phase != Phase::Streaming {
            s.streamed = true;
            self.focus = Target::Menu(MenuItem::Resume);
        }
        if !matches!(phase, Phase::Preparing) {
            self.ps.waking = None;
        }
        s.phase = phase;
    }

    /// Une partie finit : la fin voulue revient à l'accueil ; une autre reste à l'écran avec son texte. La console
    /// qui ne reconnaît plus ce compte : cette association est effacée.
    pub fn ps_ended(&mut self, end: PsEnd) {
        self.ps.waking = None;
        let Some(s) = self.session.as_mut() else { return };
        let Some(ps) = s.ps.as_ref() else { return };
        let host_id = ps.host_id.clone();
        if end == PsEnd::NotPaired {
            let accounts = self.store.playstation.accounts.clone();
            if let Some(c) = self.store.playstation.console_mut(&host_id)
                && let Some(a) = c.play_account(&accounts).map(|a| a.id.clone())
            {
                c.pairings.remove(&a);
                if c.account.as_deref() == Some(a.as_str()) {
                    c.account = None;
                }
                self.store.save();
            }
        }
        match end_text(end) {
            None => {
                self.session = None;
                self.go(Screen::Home);
            }
            Some(error) => {
                if let Some(s) = self.session.as_mut() {
                    s.phase = Phase::Ended { error: Some(error) };
                    s.menu = false;
                }
                self.screen = Screen::Session;
                self.focus = Target::Back;
            }
        }
    }
}

/// Le pas suivant dans une liste, en boucle.
fn cycle(i: usize, n: usize, dir: i32) -> usize {
    ((i as i32 + dir).rem_euclid(n.max(1) as i32)) as usize
}

/// Le paquet « Envoyer à un appareil » (docs/playstation.md, « Plusieurs appareils », « Le paquet ») : le JSON de la
/// liste partagée, plus les secrets et les Stations, comme l'app Mac l'écrit :
///
/// ```json
/// {"from":"LG OLED83G2",
///  "accounts":[{"id":"<UUID>","name":"<pseudo>","psn_id":"<base64>"}],
///  "consoles":[{"host_id":"<HOSTID>","name":"…","model":"PS5","addresses":["…"],"accounts":["<UUID>"],"target":"ps5_v1",
///               "pairings":{"<UUID>":"<JSON Registered, en texte>"},
///               "stations":[{"id":"<id de l'ordinateur ici>","name":"…","address":"192.168.1.10","fingerprint":"…"}]}]}
/// ```
pub fn bundle_of(ps: &crate::model::PsStore, hosts: &[crate::model::Host], from: &str) -> String {
    use serde_json::json;
    let accounts: Vec<_> = ps.accounts.iter().map(|a| json!({ "id": a.id.to_ascii_uppercase(), "name": a.name, "psn_id": a.psn_id })).collect();
    let consoles: Vec<_> = ps
        .consoles
        .iter()
        .filter(|c| !c.pairings.is_empty())
        .map(|c| {
            let pairings: serde_json::Map<String, serde_json::Value> = c.pairings.iter().map(|(id, r)| (id.to_ascii_uppercase(), serde_json::Value::String(r.to_string()))).collect();
            let stations: Vec<_> = c
                .stations
                .iter()
                .filter_map(|id| hosts.iter().find(|h| &h.id == id))
                .map(|h| json!({ "id": h.id, "name": h.name, "address": h.address, "fingerprint": h.fingerprint }))
                .collect();
            json!({
                "host_id": c.host_id.to_ascii_uppercase(), "name": c.name, "model": c.model, "target": c.target,
                "addresses": c.addresses,
                "accounts": c.pairings.keys().map(|k| k.to_ascii_uppercase()).collect::<Vec<_>>(),
                "pairings": pairings,
                "stations": stations,
            })
        })
        .collect();
    json!({ "from": from, "accounts": accounts, "consoles": consoles }).to_string()
}

/// Fusionne un paquet reçu : un compte est le même s'il a le même identifiant PSN (son nom pris s'il n'en avait pas ;
/// nouveau : l'`id` du paquet s'il est libre), une console si elle a le même `host_id` ; les associations s'ajoutent
/// (une association déjà là pour ce compte et cette console reste). Les Stations qui voient la console sont reliées aux
/// ordinateurs de ce téléviseur par l'`id`, sinon l'empreinte, sinon l'adresse ; inconnues, ignorées. Lit aussi
/// quelques variantes (`account` ou `account_id` pour l'identifiant, `pairings` en tableau, association en objet,
/// `address` seule, Stations en texte). Rend le nombre de comptes et de consoles du paquet.
pub fn merge_bundle(ps: &mut crate::model::PsStore, bundle: &str, hosts: &[crate::model::Host]) -> (usize, usize) {
    use serde_json::Value;
    let Ok(v) = serde_json::from_str::<Value>(bundle) else { return (0, 0) };
    let text = |v: &Value, keys: &[&str]| keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str)).map(str::to_string);
    // Les comptes : uuid de l'envoyeur (en majuscules) → uuid ici.
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut accounts = 0;
    for a in v.get("accounts").and_then(Value::as_array).into_iter().flatten() {
        let Some(psn) = text(a, &["psn_id", "account", "account_id"]).filter(|s| !s.is_empty()) else { continue };
        let theirs = text(a, &["id"]).map(|s| s.to_ascii_uppercase());
        let name = text(a, &["name", "online_id"]);
        let id = match ps.accounts.iter_mut().find(|x| x.psn_id == psn) {
            Some(x) => {
                if x.name.as_deref().is_none_or(|n| n.trim().is_empty()) {
                    x.name = name.filter(|n| !n.trim().is_empty());
                }
                x.id.clone()
            }
            None => {
                let id = theirs.clone().filter(|t| !t.is_empty() && !ps.accounts.iter().any(|x| x.id.eq_ignore_ascii_case(t))).unwrap_or_else(crate::model::new_id);
                ps.accounts.push(crate::model::PsAccount { id: id.clone(), name: name.filter(|n| !n.trim().is_empty()), psn_id: psn });
                id
            }
        };
        if let Some(theirs) = theirs {
            ids.insert(theirs, id);
        }
        accounts += 1;
    }
    let registered = |r: &Value| -> Option<Value> {
        match r {
            Value::String(s) => serde_json::from_str(s).ok(),
            Value::Object(_) => Some(r.clone()),
            _ => None,
        }
    };
    let station_of = |s: &Value| -> Option<String> {
        if let Some(id) = s.as_str() {
            return hosts.iter().find(|h| h.id == id).map(|h| h.id.clone());
        }
        let id = s.get("id").and_then(Value::as_str);
        let fp = s.get("fingerprint").and_then(Value::as_str);
        let addr = s.get("address").and_then(Value::as_str);
        hosts
            .iter()
            .find(|h| id.is_some_and(|i| h.id.eq_ignore_ascii_case(i)))
            .or_else(|| hosts.iter().find(|h| fp.is_some_and(|f| h.fingerprint.as_deref() == Some(f))))
            .or_else(|| hosts.iter().find(|h| addr.is_some_and(|a| h.address == a)))
            .map(|h| h.id.clone())
    };
    let mut consoles = 0;
    for c in v.get("consoles").and_then(Value::as_array).into_iter().flatten() {
        let Some(host_id) = text(c, &["host_id"]).filter(|s| !s.is_empty()) else { continue };
        let name = text(c, &["name"]).unwrap_or_default();
        let model = text(c, &["model"]).unwrap_or_default();
        let target = text(c, &["target"]).unwrap_or_default();
        let mut addresses: Vec<String> = c.get("addresses").and_then(Value::as_array).into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect();
        if let Some(a) = text(c, &["address"]) {
            addresses.push(a);
        }
        let known = ps.console(&host_id).is_some();
        let first = addresses.first().cloned();
        // Une console déjà là garde son nom, son modèle et ses adresses à elle ; les adresses reçues s'ajoutent.
        let local = if known { ps.console_mut(&host_id).unwrap() } else { ps.put_console(&host_id, &name, &model, &target, first.as_deref()) };
        if local.target.is_empty() {
            local.target = target;
        }
        for a in &addresses {
            if !local.addresses.contains(a) && a != "127.0.0.1" {
                local.addresses.push(a.clone());
            }
        }
        for s in c.get("stations").and_then(Value::as_array).into_iter().flatten() {
            if let Some(id) = station_of(s)
                && !local.stations.contains(&id)
            {
                local.stations.push(id);
            }
        }
        let mut add = |theirs: &str, r: Option<Value>| {
            if let (Some(id), Some(r)) = (ids.get(&theirs.to_ascii_uppercase()), r) {
                local.pairings.entry(id.clone()).or_insert(r);
            }
        };
        match c.get("pairings") {
            Some(Value::Object(m)) => {
                for (theirs, r) in m {
                    add(theirs, registered(r));
                }
            }
            Some(Value::Array(list)) => {
                for p in list {
                    add(&text(p, &["account", "account_id"]).unwrap_or_default(), p.get("registered").and_then(registered));
                }
            }
            _ => {}
        }
        if local.account.is_none() {
            local.account = local.pairings.keys().next().cloned();
        }
        consoles += 1;
    }
    (accounts, consoles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PsStore;

    #[test]
    fn bundle_round_trip_and_merge() {
        let mut a = PsStore::default();
        let (acc, _) = a.put_account("AAAAAAAAAAE=", Some("Matt".into()));
        let c = a.put_console("abc123", "PS5-Salon", "ps5", "ps5_v1", Some("192.168.1.110"));
        c.pairings.insert(acc.clone(), serde_json::json!({"mac": "ABC123", "rp_key": "00"}));
        let bundle = bundle_of(&a, &[], "LG OLED83G2");
        assert!(bundle.contains("\"pairings\":{\"") && bundle.contains("\"from\":\"LG OLED83G2\""));

        // Un autre appareil qui connaît déjà le compte (autre uuid) : pas de doublon, l'association arrive.
        let mut b = PsStore::default();
        b.put_account("AAAAAAAAAAE=", None);
        let (n_acc, n_con) = merge_bundle(&mut b, &bundle, &[]);
        assert_eq!((n_acc, n_con), (1, 1));
        assert_eq!(b.accounts.len(), 1);
        assert_eq!(b.accounts[0].name.as_deref(), Some("Matt"));
        let c = b.console("ABC123").unwrap();
        assert_eq!(c.model, "PS5");
        assert_eq!(c.addresses, vec!["192.168.1.110".to_string()]);
        assert!(c.pairings.contains_key(&b.accounts[0].id));
        assert_eq!(c.account.as_deref(), Some(b.accounts[0].id.as_str()));
        // Deux fois : rien en double.
        merge_bundle(&mut b, &bundle, &[]);
        assert_eq!((b.accounts.len(), b.consoles.len()), (1, 1));
    }

    #[test]
    fn bundle_variants() {
        let bundle = r#"{"accounts":[{"id":"x","name":"Léa","account":"AAAAAAAAAAI="}],
            "consoles":[{"host_id":"F00","name":"PS4","model":"PS4","address":"10.0.0.5",
            "pairings":[{"account":"x","registered":"{\"mac\":\"F00\"}"}]}]}"#;
        let mut s = PsStore::default();
        assert_eq!(merge_bundle(&mut s, bundle, &[]), (1, 1));
        let c = s.console("f00").unwrap();
        assert!(!c.ps5());
        assert_eq!(c.pairings.len(), 1);
        assert_eq!(merge_bundle(&mut s, "pas du json", &[]), (0, 0));
    }

    #[test]
    fn bundle_from_the_mac() {
        // Le paquet tel que l'app Mac l'écrit (association en texte, Stations en objets, uuid en majuscules).
        let bundle = r#"{"from":"MacBook Pro de Léa",
            "accounts":[{"id":"5E1F0A9C-0000-4000-8000-000000000001","name":"Lea_42","psn_id":"AAAAAAAAAAM="}],
            "consoles":[{"host_id":"F8461C0A1B2C","name":"PS5 du salon","model":"PS5","addresses":["192.168.1.110"],
              "accounts":["5E1F0A9C-0000-4000-8000-000000000001"],"target":"ps5_v1",
              "pairings":{"5E1F0A9C-0000-4000-8000-000000000001":"{\"mac\":\"F8461C0A1B2C\",\"rp_key\":\"00\"}"},
              "stations":[{"id":"AB12","name":"PC du salon","address":"192.168.1.10","fingerprint":null}]}]}"#;
        let pc = crate::model::Host { id: "local-pc".into(), name: "PC".into(), address: "192.168.1.10".into(), port: 24811, fingerprint: None, user: String::new(), password: String::new(), os: None, gpu: None, av1: false, mac: None, wake: Vec::new(), relay: None, settings: Default::default(), last: None };
        let mut s = PsStore::default();
        assert_eq!(merge_bundle(&mut s, bundle, &[pc]), (1, 1));
        // L'uuid du paquet est libre : gardé.
        assert_eq!(s.accounts[0].id, "5E1F0A9C-0000-4000-8000-000000000001");
        let c = s.console("F8461C0A1B2C").unwrap();
        assert_eq!(c.pairings.values().next().unwrap()["mac"], "F8461C0A1B2C");
        assert_eq!(c.stations, vec!["local-pc".to_string()]);
    }

    #[test]
    fn removing_an_account() {
        let mut s = PsStore::default();
        let (a, _) = s.put_account("A", None);
        let (b, _) = s.put_account("B", None);
        s.put_console("C1", "Un", "PS5", "", None).pairings.insert(a.clone(), serde_json::json!({}));
        let c2 = s.put_console("C2", "Deux", "PS5", "", None);
        c2.pairings.insert(a.clone(), serde_json::json!({}));
        c2.pairings.insert(b.clone(), serde_json::json!({}));
        assert_eq!(s.consoles_of(&a), 2);
        assert_eq!(s.remove_account(&a), 2);
        // C1 n'avait que ce compte : retirée ; C2 reste avec B.
        assert_eq!(s.consoles.len(), 1);
        assert_eq!(s.consoles[0].paired_accounts(&s.accounts).len(), 1);
    }

    #[test]
    fn settings_json_matches_the_core() {
        // Les noms et valeurs de `PlaySettings` (apps/core/src/playstation/settings.rs).
        let s: PsSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, PsSettings::default());
        assert!(!s.is_custom());
        let s: PsSettings = serde_json::from_str(r#"{"resolution":"p720","bitrate_kbps":20000}"#).unwrap();
        assert!(s.is_custom());
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"resolution\":\"p720\"") && json.contains("\"codec\":\"auto\"") && json.contains("\"bitrate_kbps\":20000"));
    }
}
