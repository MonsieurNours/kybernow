//! Comptes KyberNow, côté interface (sans le cœur ni SDL : les captures et les essais se font sans Kyber).
//! Contrat : docs/comptes.md (lot 3) ; textes : docs/link-ui.md, « Comptes KyberNow » (les erreurs et les pastilles
//! d'origine viennent du cœur, `kybernow_core::account`, par le pilote).
//!
//! - **Se connecter** : au salon, sur le Steam Deck et sur KyberNow Link OS, par un **code** (`Screen::Account`,
//!   `AccountStep::Code`) : trois étapes, le code en chasse fixe, son QR code, « En attente de votre téléphone… Le
//!   code change dans {n} min. » ; le pilote attend l'approbation (`account::tv_wait`) et redemande un code échu de
//!   lui-même. Sur le bureau, par la **page du relais dans le navigateur** (`account::sign_in`) : le relais revient à
//!   `kybernow://compte?…`, que le gestionnaire de liens déjà déclaré (.desktop, Flatpak : `x-scheme-handler/kybernow`)
//!   donne à l'app ouverte (`device/link.rs`), comme les liens de partage et d'invitation ; l'écran dit « Connexion au
//!   compte… » en attendant. Le code reste le chemin du salon : en mode Jeu de SteamOS un navigateur s'ouvre mal, et
//!   Link OS n'en a pas.
//! - **Plusieurs relais** : « Ajouter un compte » demande d'abord le relais (`share.kybernow.app` proposé, « Un autre
//!   relais… ») ; l'écran du code le montre (« Relais : {relais} ») et propose d'en changer.
//! - **Accueil** : sans ordinateur, « Retrouvez vos ordinateurs » et « Connecter un compte » ; avec des
//!   comptes, leurs ordinateurs sont des ordinateurs de l'accueil comme les autres (`Store::account_hosts`), avec une
//!   pastille d'origine (`AccountsUi::origins`, calculée par le pilote avec `account::merge` et `origin`).
//! - **Réglages de l'app › Comptes** : chaque compte (nom, e-mail, relais), « Gérer le compte » (la page du compte :
//!   le navigateur sur le bureau, un QR code au salon), « Se déconnecter » (confirmé), « Ajouter un compte ». L'app ne
//!   crée ni ne supprime de compte.

use std::collections::HashMap;

use crate::app::{Action, App, ButtonStyle, Confirm, Nav, Row, RowKind, Screen, Setting, Target};
use crate::i18n::{english, tr};
use crate::model::{KnAccount, KnComputer};

/// Les étapes de l'écran des comptes (`Screen::Account`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AccountStep {
    /// Le code de téléviseur et son QR code (salon, Deck, Link OS).
    #[default]
    Code,
    /// « Ajouter un compte » sur le bureau : le relais d'abord (« share.kybernow.app », « Un autre relais… »).
    Relay,
    /// « Un autre relais » : le champ « Relais », « Continuer », « Annuler ».
    OtherRelay,
    /// Bureau : la page du relais est ouverte dans le navigateur, « Connexion au compte… ».
    Browser,
    /// « Gérer le compte » au salon : le QR code de la page du compte (clé du compte).
    Manage(String),
}

/// Un code de téléviseur à afficher (`account::TvCode` du cœur, sans le jeton de la demande).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TvCodeUi {
    /// `KQ7M-4XPA`.
    pub code: String,
    /// `share.kybernow.app/tv`, à écrire à l'écran.
    pub url_label: String,
    /// La page avec le code : le QR code.
    pub url_complete: String,
    /// Le relais (forme normale).
    pub relay: String,
    /// Échéance (secondes Unix de l'appareil).
    pub expires_at: u64,
}

impl TvCodeUi {
    /// Minutes avant l'échéance, arrondies au-dessus (au moins 1 tant qu'il vaut) : « Le code change dans {n} min. ».
    pub fn minutes_left(&self, now: u64) -> u64 {
        if now >= self.expires_at { 0 } else { (self.expires_at - now).div_ceil(60).max(1) }
    }
}

/// Pourquoi une connexion n'aboutit pas, vu de l'interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInFail {
    /// « Ce n'est pas moi » sur le téléphone : « Connexion refusée sur le téléphone. » et « Nouveau code ».
    Denied,
    /// Abandonnée (« Retour », « Annuler » dans la page du relais) : rien à dire.
    Cancelled,
    /// Le texte du cœur (`AccountError::text`).
    Message(String),
}

/// La connexion en cours (écran `Screen::Account`).
#[derive(Clone, Debug, Default)]
pub struct AccountFlow {
    pub step: AccountStep,
    /// Le relais choisi (forme normale, ou telle que tapée) ; vide : `share.kybernow.app`.
    pub relay: String,
    pub code: Option<TvCodeUi>,
    /// Le code a été refusé sur le téléphone.
    pub denied: bool,
    /// Un code est demandé au relais (ou la page du relais se prépare).
    pub busy: bool,
    /// Un refus du relais (relais muet, trop d'essais…), sous le code.
    pub message: Option<String>,
    /// « Un autre relais » : le champ, et le refus dessous.
    pub field: String,
    pub field_message: Option<String>,
    /// Le relais vient du champ : un refus y ramène.
    pub typed: bool,
    /// Le numéro de la demande en cours : une réponse plus ancienne est écartée.
    pub attempt: u32,
    /// L'écran d'où l'on vient (l'accueil, les réglages de l'app ou de l'appareil).
    pub back_to: Option<Screen>,
}

/// Les comptes vus par l'interface.
#[derive(Clone, Debug, Default)]
pub struct AccountsUi {
    pub flow: AccountFlow,
    /// La pastille d'origine de chaque ordinateur de l'accueil (`Host::id`), calculée par le pilote
    /// (`AccountComputer::origin`, `origin_paired`) ; vide sans compte connecté (pas de pastille).
    pub origins: HashMap<String, String>,
    /// Réglages synchronisés : ce qui a été lu, appliqué ou envoyé pour chaque ordinateur de l'accueil, la base des
    /// comparaisons qui disent quoi envoyer (`crate::synced`).
    pub sync_base: HashMap<String, crate::synced::Tracked>,
}

/// « Connecté à {compte}. »
pub fn signed_in_notice(account: &str) -> String {
    if english() { format!("Signed in to {account}.") } else { format!("Connecté à {account}.") }
}

/// « Se déconnecter de {compte} ? »
pub fn sign_out_question(account: &str) -> String {
    if english() { format!("Sign out of {account}?") } else { format!("Se déconnecter de {account} ?") }
}

/// « Relais : {relais} ».
pub fn relay_line(relay: &str) -> String {
    format!("{} {}", tr("Relais :", "Relay:"), crate::model::relay_label(relay))
}

/// « En attente de votre téléphone… Le code change dans {n} min. »
pub fn waiting_line(minutes: u64) -> String {
    if english() { format!("Waiting for your phone… The code changes in {minutes} min.") } else { format!("En attente de votre téléphone… Le code change dans {minutes} min.") }
}

/// « ou ouvrez {adresse} et entrez le code, »
pub fn open_step(address: &str) -> String {
    if english() { format!("or open {address} and enter the code,") } else { format!("ou ouvrez {address} et entrez le code,") }
}

/// L'aide sous « Connecter un compte » : la page du relais (bureau) ou le code (salon).
pub fn sign_in_help(by_code: bool) -> &'static str {
    if by_code {
        tr("Un code s'affiche : vous l'approuvez depuis votre téléphone.", "A code appears: you approve it from your phone.")
    } else {
        tr(
            "La page du relais s'ouvre : mot de passe, passkey ou double authentification s'y font, jamais dans l'app. Pas encore de compte : « Créer un compte » sur cette page.",
            "The relay's page opens: password, passkey or two-factor authentication happen there, never in the app. No account yet: “Create an account” on that page.",
        )
    }
}

/// Le plus long relais qu'on tape.
const RELAY_MAX: usize = 200;

impl App {
    /// La connexion se fait par un code (salon, Steam Deck, KyberNow Link OS), sinon par la page du relais dans le
    /// navigateur (bureau).
    pub fn sign_in_by_code(&self) -> bool {
        self.layout.console() || self.is_os()
    }

    /// Le champ « Relais » attend des lettres.
    pub fn typing_relay(&self) -> bool {
        self.screen == Screen::Account && self.accounts.flow.step == AccountStep::OtherRelay
    }

    /// Le code se montre : l'écran des comptes, ou l'écran 2 de l'accueil au salon (la carte du code à droite).
    pub fn code_screen(&self) -> bool {
        self.screen == Screen::Account || self.welcome_code()
    }

    /// Un compte au moins est connecté : les pastilles d'origine se montrent.
    pub fn has_accounts(&self) -> bool {
        !self.store.accounts.is_empty()
    }

    /// La pastille d'origine d'un ordinateur de l'accueil, quand un compte est connecté.
    pub fn origin(&self, host: &str) -> Option<&str> {
        self.has_accounts().then(|| self.accounts.origins.get(host).map(String::as_str)).flatten().filter(|s| !s.is_empty())
    }

    /// « Connecter un compte » (accueil vide, bouton et carte du compte, réglages) : un code au salon, la page du relais
    /// (`share.kybernow.app`) sur le bureau.
    pub fn account_sign_in(&mut self, from: Screen) -> Vec<Action> {
        let back_to = Some(self.accounts.flow.back_to.unwrap_or(from));
        self.accounts.flow = AccountFlow { attempt: self.accounts.flow.attempt + 1, back_to, ..AccountFlow::default() };
        self.account_start(String::new(), false)
    }

    /// « Ajouter un compte » (réglages) : au salon, l'écran du code (le relais y est dit, « Un autre relais… » en
    /// change) ; sur le bureau, le relais d'abord.
    pub fn account_add(&mut self) -> Vec<Action> {
        let from = self.screen;
        if self.sign_in_by_code() {
            return self.account_sign_in(from);
        }
        self.accounts.flow = AccountFlow { step: AccountStep::Relay, attempt: self.accounts.flow.attempt + 1, back_to: Some(from), ..AccountFlow::default() };
        self.go(Screen::Account);
        Vec::new()
    }

    /// La connexion part vers `relay` (vide : le relais public) : un code, ou la page du relais.
    fn account_start(&mut self, relay: String, typed: bool) -> Vec<Action> {
        let by_code = self.sign_in_by_code();
        let f = &mut self.accounts.flow;
        f.attempt += 1;
        f.relay = relay.clone();
        f.typed = typed;
        f.code = None;
        f.denied = false;
        f.message = None;
        f.field_message = None;
        f.busy = true;
        f.step = if by_code { AccountStep::Code } else { AccountStep::Browser };
        let attempt = f.attempt;
        // Au salon, l'écran 2 de l'accueil montre le code lui-même : on y reste.
        if !self.welcome_code() {
            self.go(Screen::Account);
        }
        if by_code { vec![Action::AccountCode { attempt, relay }] } else { vec![Action::AccountBrowser { attempt, relay }] }
    }

    /// La réponse à la demande d'un code (et chaque nouveau code, quand le précédent est échu) ; un refus du relais.
    pub fn account_code(&mut self, attempt: u32, r: Result<TvCodeUi, String>) {
        let f = &mut self.accounts.flow;
        if f.attempt != attempt || f.step != AccountStep::Code {
            return;
        }
        if !self.code_screen() {
            return;
        }
        let f = &mut self.accounts.flow;
        f.busy = false;
        match r {
            Ok(code) => {
                f.relay = code.relay.clone();
                f.code = Some(code);
                f.denied = false;
                f.message = None;
            }
            // Un relais tapé qui ne va pas : retour au champ, le refus dessous.
            Err(m) if f.typed => {
                f.step = AccountStep::OtherRelay;
                f.field_message = Some(m);
                self.go(Screen::Account);
            }
            Err(m) => {
                f.code = None;
                f.message = Some(m);
                self.focus = Target::AccountNewCode;
            }
        }
    }

    /// Bureau : la page du relais n'a pas pu s'ouvrir (adresse du relais illisible) : le refus.
    pub fn account_browser_failed(&mut self, attempt: u32, message: String) {
        let f = &mut self.accounts.flow;
        if f.attempt != attempt || self.screen != Screen::Account {
            return;
        }
        f.busy = false;
        if f.typed {
            f.step = AccountStep::OtherRelay;
            f.field_message = Some(message);
            self.go(Screen::Account);
        } else {
            f.message = Some(message);
        }
    }

    /// La connexion a abouti (code approuvé, retour de la page du relais) ou non. Réussie, le compte est gardé
    /// (même après « Retour » : sa session d'appareil existe déjà), « Connecté à {compte}. », ses ordinateurs arrivent.
    pub fn account_signed_in(&mut self, attempt: u32, r: Result<KnAccount, SignInFail>) -> Vec<Action> {
        let current = self.code_screen() && self.accounts.flow.attempt == attempt && matches!(self.accounts.flow.step, AccountStep::Code | AccountStep::Browser);
        match r {
            Ok(a) => {
                self.toast(signed_in_notice(&a.label()));
                self.store.put_account(a);
                // Depuis l'accueil en trois écrans : il est fini, son écran 3 est passé (les ordinateurs du compte
                // arrivent).
                let from_welcome = self.screen == Screen::Welcome || self.accounts.flow.back_to == Some(Screen::Welcome);
                if from_welcome {
                    self.store.app.welcome.welcome_done();
                    self.accounts.flow.back_to = Some(Screen::Home);
                }
                if current || (self.screen == Screen::Account && self.accounts.flow.step == AccountStep::Browser) {
                    self.account_leave();
                } else if self.screen == Screen::Welcome {
                    self.accounts.flow = AccountFlow { attempt: self.accounts.flow.attempt + 1, ..AccountFlow::default() };
                    self.go(Screen::Home);
                }
                vec![Action::Save, Action::AccountsRefresh]
            }
            Err(_) if !current => Vec::new(),
            Err(SignInFail::Cancelled) => {
                // Bureau : « Annuler » dans la page du relais, rien ne change (l'écran revient d'où il vient).
                if self.accounts.flow.step == AccountStep::Browser {
                    self.account_leave();
                }
                Vec::new()
            }
            Err(SignInFail::Denied) => {
                let f = &mut self.accounts.flow;
                f.busy = false;
                f.code = None;
                f.denied = true;
                f.message = None;
                self.focus = Target::AccountNewCode;
                Vec::new()
            }
            Err(SignInFail::Message(m)) => {
                let f = &mut self.accounts.flow;
                f.busy = false;
                f.code = None;
                f.message = Some(m);
                self.focus = Target::AccountNewCode;
                Vec::new()
            }
        }
    }

    /// La liste des ordinateurs d'un compte, arrivée du relais (empreintes déjà gardées par le pilote) : elle remplace
    /// la précédente. Le pilote refait ensuite l'accueil (`account::merge`).
    pub fn account_computers(&mut self, key: &str, list: Vec<KnComputer>) {
        if self.store.account(key).is_none() {
            return;
        }
        self.store.account_computers.retain(|c| c.account != key);
        self.store.account_computers.extend(list);
    }

    /// La session d'appareil d'un compte est fermée (page du compte, nouveau mot de passe) : le compte est oublié, ses
    /// ordinateurs quittent l'accueil, et `message` le dit (« Ce compte a été déconnecté de cet appareil :
    /// reconnectez-vous. », du cœur).
    pub fn account_signed_out(&mut self, key: &str, message: String) -> Vec<Action> {
        if self.store.account(key).is_none() {
            return Vec::new();
        }
        self.forget_account(key);
        self.toast(message);
        vec![Action::Save, Action::AccountsSync]
    }

    /// Un ordinateur du compte que le relais ne connaît plus (`unknown_station`) : il quitte l'accueil, `message` le
    /// dit (« Cet ordinateur n'est plus sur votre compte. »).
    pub fn account_unknown_computer(&mut self, host: &str, message: String) -> Vec<Action> {
        let Some(link) = self.store.account_host(host).cloned() else { return Vec::new() };
        self.store.account_computers.retain(|c| !(c.relay == link.relay && c.station == link.station));
        self.toast(message);
        vec![Action::Save, Action::AccountsSync]
    }

    /// Le compte et ses ordinateurs quittent l'appareil (ceux qui sont appairés restent).
    fn forget_account(&mut self, key: &str) {
        for id in self.store.forget_account(key) {
            self.live.remove(&id);
            self.images.remove(&id);
            self.accounts.origins.remove(&id);
        }
    }

    /// Les ordinateurs des comptes à l'accueil (`added` d'`account::merge`, par le pilote), et les pastilles.
    pub fn set_account_hosts(&mut self, added: &[KnComputer], origins: HashMap<String, String>) {
        for id in self.store.sync_account_hosts(added) {
            self.live.remove(&id);
            self.images.remove(&id);
        }
        self.accounts.origins = origins;
    }

    /// L'écran des comptes se referme : retour d'où l'on vient.
    fn account_leave(&mut self) {
        let to = self.accounts.flow.back_to.take().unwrap_or(Screen::Home);
        let attempt = self.accounts.flow.attempt + 1;
        self.accounts.flow = AccountFlow { attempt, ..AccountFlow::default() };
        match to {
            // Annulée depuis l'écran 2 de l'accueil : on y revient.
            Screen::Welcome => self.go(Screen::Welcome),
            Screen::Settings => {
                self.screen = Screen::Settings;
                self.screen_since = std::time::Instant::now();
                self.focus = Target::Setting(Setting::AccountAdd);
            }
            Screen::Device => {
                self.screen = Screen::Device;
                self.screen_since = std::time::Instant::now();
                self.focus = Target::Setting(Setting::AccountAdd);
            }
            _ => self.go(Screen::Home),
        }
    }

    /// Retour (B, Échap) sur l'écran des comptes.
    pub fn account_back(&mut self) -> Vec<Action> {
        match self.accounts.flow.step.clone() {
            AccountStep::OtherRelay => {
                // Retour au code (le précédent est abandonné : on en redemande un), ou au choix du relais.
                if self.sign_in_by_code() {
                    let relay = if self.accounts.flow.typed { String::new() } else { self.accounts.flow.relay.clone() };
                    self.account_start(relay, false)
                } else {
                    self.accounts.flow.step = AccountStep::Relay;
                    self.accounts.flow.field_message = None;
                    self.go(Screen::Account);
                    Vec::new()
                }
            }
            AccountStep::Code | AccountStep::Browser => {
                self.account_leave();
                vec![Action::AccountCancel]
            }
            AccountStep::Relay | AccountStep::Manage(_) => {
                self.account_leave();
                Vec::new()
            }
        }
    }

    /// Le focus en arrivant sur l'écran des comptes.
    pub fn account_focus(&self) -> Target {
        let f = &self.accounts.flow;
        match f.step {
            AccountStep::Code if f.denied || f.message.is_some() => Target::AccountNewCode,
            AccountStep::Relay => Target::AccountRelayDefault,
            AccountStep::OtherRelay if self.layout.console() => Target::Key('1'),
            AccountStep::OtherRelay => Target::AccountRelayField,
            _ => Target::AccountBack,
        }
    }

    /// La manette et le clavier sur l'écran des comptes (`None` : ailleurs, ou rien de particulier).
    pub fn account_nav(&mut self, n: Nav) -> Option<Vec<Action>> {
        if !self.typing_relay() || self.confirm.is_some() {
            return None;
        }
        match n {
            Nav::Char(c) => {
                self.relay_type(c);
                Some(Vec::new())
            }
            Nav::Erase => {
                self.accounts.flow.field.pop();
                self.accounts.flow.field_message = None;
                Some(Vec::new())
            }
            Nav::Ok if self.focus == Target::AccountRelayField => Some(self.relay_go()),
            _ => None,
        }
    }

    /// Une lettre pour le champ « Relais » : l'adresse d'un relais (lettres, chiffres, `.`, `:`, `-`, `[`, `]`, `#`).
    fn relay_type(&mut self, c: char) {
        let c = c.to_ascii_lowercase();
        let f = &mut self.accounts.flow;
        if (c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '[' | ']' | '#' | '_')) && f.field.chars().count() < RELAY_MAX {
            f.field.push(c);
            f.field_message = None;
        }
    }

    /// « Continuer » (inactif tant que le champ est vide) : la connexion part vers ce relais ; un refus ramène au champ.
    fn relay_go(&mut self) -> Vec<Action> {
        let relay = self.accounts.flow.field.trim().to_string();
        if relay.is_empty() {
            return Vec::new();
        }
        self.account_start(relay, true)
    }

    /// « Un autre relais… » : le champ, vide ; un code en attente est abandonné.
    fn other_relay(&mut self) -> Vec<Action> {
        let waiting = self.accounts.flow.step == AccountStep::Code;
        let f = &mut self.accounts.flow;
        f.step = AccountStep::OtherRelay;
        f.attempt += 1;
        f.busy = false;
        f.field.clear();
        f.field_message = None;
        self.go(Screen::Account);
        let mut acts = vec![Action::TextInput];
        if waiting {
            acts.insert(0, Action::AccountCancel);
        }
        acts
    }

    /// Ce que font les éléments de l'écran des comptes et de l'accueil vide (`None` : une autre cible).
    pub fn account_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        Some(match t {
            Target::AccountSignIn => {
                let from = if matches!(self.screen, Screen::Settings | Screen::Device) { self.screen } else { Screen::Home };
                self.accounts.flow.back_to = None;
                self.account_sign_in(from)
            }
            Target::AccountOtherRelay => self.other_relay(),
            Target::AccountNewCode => {
                let relay = self.accounts.flow.relay.clone();
                self.account_start(relay, false)
            }
            Target::AccountBack => self.account_back(),
            Target::AccountRelayDefault => self.account_start(String::new(), false),
            Target::AccountRelayField => Vec::new(),
            Target::AccountRelayGo => self.relay_go(),
            Target::AccountRelayCancel => self.account_back(),
            Target::Key(c) if self.typing_relay() => {
                self.relay_type(c);
                Vec::new()
            }
            Target::KeyErase if self.typing_relay() => {
                self.accounts.flow.field.pop();
                Vec::new()
            }
            _ => return None,
        })
    }

    /// Réglages › Comptes : « Gérer le compte », « Se déconnecter », « Ajouter un compte » (`None` : un autre réglage).
    pub fn account_choose(&mut self, s: Setting) -> Option<Vec<Action>> {
        Some(match s {
            Setting::AccountManage(i) => {
                let Some(a) = self.store.accounts.get(i as usize).cloned() else { return Some(Vec::new()) };
                // Au salon et sur Link OS, pas de navigateur : la page du compte en QR code.
                if self.admin_by_qr() {
                    let from = self.screen;
                    self.accounts.flow = AccountFlow { step: AccountStep::Manage(a.key()), attempt: self.accounts.flow.attempt + 1, back_to: Some(from), ..AccountFlow::default() };
                    self.go(Screen::Account);
                    Vec::new()
                } else {
                    vec![Action::OpenUrl(a.console())]
                }
            }
            Setting::AccountSignOut(i) => {
                if let Some(a) = self.store.accounts.get(i as usize) {
                    let c = Confirm::SignOut { account: a.key(), name: a.label() };
                    self.confirm = Some(c);
                    self.focus = Target::ConfirmNo;
                }
                Vec::new()
            }
            Setting::AccountAdd => self.account_add(),
            // « Revoir l'accueil » : ses trois écrans, depuis le premier.
            Setting::WelcomeAgain => {
                self.open_welcome();
                Vec::new()
            }
            // Rallumé : les réglages du relais sont relus (ce qui est plus récent s'applique).
            Setting::SyncSettings => {
                self.store.app.sync_settings = !self.store.app.sync_settings;
                if self.store.app.sync_settings { vec![Action::Save, Action::AccountsRefresh] } else { vec![Action::Save] }
            }
            _ => return None,
        })
    }

    /// « Se déconnecter » confirmé : la session d'appareil est fermée chez le relais (`account::sign_out`) et le
    /// compte oublié tout de suite, même si le relais ne répond pas.
    pub fn account_sign_out(&mut self, key: &str) -> Vec<Action> {
        let Some(a) = self.store.account(key).cloned() else { return Vec::new() };
        self.forget_account(key);
        self.focus = Target::Setting(if self.store.accounts.is_empty() { Setting::AccountAdd } else { Setting::AccountSignOut(0) });
        vec![Action::AccountSignOut(a), Action::Save, Action::AccountsSync]
    }

    /// Les lignes de Réglages › Comptes.
    pub fn account_rows(&self) -> Vec<Row> {
        let row = |setting, label: String, hint: String, kind| Row { setting, label, hint, kind, enabled: true, tag: None };
        let button = |setting, text: &str, style| row(setting, text.to_string(), String::new(), RowKind::Button { text: text.to_string(), style });
        let mut rows = Vec::new();
        if self.store.accounts.is_empty() {
            rows.push(row(Setting::Info, tr("Aucun compte.", "No account.").into(), String::new(), RowKind::Info(String::new())));
            let text = tr("Connecter un compte", "Connect an account");
            rows.push(row(Setting::AccountAdd, text.into(), sign_in_help(self.sign_in_by_code()).into(), RowKind::Button { text: text.into(), style: ButtonStyle::Primary }));
            rows.push(crate::welcome::again_row());
            return rows;
        }
        for (i, a) in self.store.accounts.iter().enumerate().take(u8::MAX as usize) {
            let email = if a.account.name.trim().is_empty() { String::new() } else { a.account.email.clone() };
            rows.push(row(Setting::Info, a.label(), email, RowKind::Info(a.relay_label())));
            rows.push(button(Setting::AccountManage(i as u8), tr("Gérer le compte", "Manage account"), ButtonStyle::Ghost));
            rows.push(button(Setting::AccountSignOut(i as u8), tr("Se déconnecter", "Sign out"), ButtonStyle::Danger));
        }
        // Réglages synchronisés : allumé par défaut, gardé sur cet appareil.
        rows.push(row(Setting::SyncSettings, sync_settings_title().into(), sync_settings_help().into(), RowKind::Toggle(self.store.app.sync_settings)));
        let text = tr("Ajouter un compte", "Add an account");
        rows.push(row(Setting::AccountAdd, text.into(), accounts_help().into(), RowKind::Button { text: text.into(), style: ButtonStyle::Ghost }));
        rows.push(crate::welcome::again_row());
        rows
    }

    /// Le compte de la page « Gérer le compte » (au salon).
    pub fn managed_account(&self) -> Option<&KnAccount> {
        match &self.accounts.flow.step {
            AccountStep::Manage(key) => self.store.account(key),
            _ => None,
        }
    }
}

/// « Synchroniser les réglages des ordinateurs » (`account::sync_settings_texts` du cœur, recopié : l'interface se
/// compile sans lui).
pub fn sync_settings_title() -> &'static str {
    tr("Synchroniser les réglages des ordinateurs", "Sync computer settings")
}

/// L'aide de l'interrupteur des réglages synchronisés.
pub fn sync_settings_help() -> &'static str {
    tr(
        "Le profil, le format, le débit, le HDR et le mode immersif de chaque ordinateur vous suivent sur vos appareils.",
        "Each computer's profile, format, bitrate, HDR and immersive mode follow you on your devices.",
    )
}

/// L'aide de la rubrique « Comptes ».
pub fn accounts_help() -> &'static str {
    tr("Créer un compte, changer son mot de passe ou le supprimer : sur la page du compte.", "Create an account, change its password or delete it: on the account's page.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Layout;
    use crate::model::{KnUser, Store};

    fn lea() -> KnAccount {
        KnAccount { relay: "share.kybernow.app".into(), token: "secret".into(), account: KnUser { id: "acc_lea".into(), name: "Léa Martin".into(), email: "lea@exemple.org".into() }, ..Default::default() }
    }

    fn computer(station: &str, name: &str, fp: &str) -> KnComputer {
        KnComputer { account: lea().key(), relay: "share.kybernow.app".into(), station: station.into(), name: name.into(), mine: true, admin_fp: Some(fp.into()), ..Default::default() }
    }

    fn tv() -> App {
        crate::i18n::force_english(false);
        App::new(Store::default(), Layout::Tv)
    }

    #[test]
    fn signing_in_by_code() {
        let mut app = tv();
        app.go(Screen::Home);
        let acts = app.activate(Target::AccountSignIn);
        assert_eq!(app.screen, Screen::Account);
        assert_eq!(app.accounts.flow.step, AccountStep::Code);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: String::new() }]);
        // Un code arrive ; un code d'une demande abandonnée est écarté.
        let code = TvCodeUi { code: "KQ7M-4XPA".into(), url_label: "share.kybernow.app/tv".into(), url_complete: "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA".into(), relay: "share.kybernow.app".into(), expires_at: 1_000 + 600 };
        app.account_code(attempt - 1, Ok(TvCodeUi { code: "AAAA-BBBB".into(), ..code.clone() }));
        assert!(app.accounts.flow.code.is_none());
        app.account_code(attempt, Ok(code.clone()));
        assert_eq!(app.accounts.flow.code.as_ref().map(|c| c.code.as_str()), Some("KQ7M-4XPA"));
        assert_eq!(code.minutes_left(1_000), 10);
        assert_eq!(code.minutes_left(1_000 + 541), 1);
        assert_eq!(waiting_line(10), "En attente de votre téléphone… Le code change dans 10 min.");
        // Refusé sur le téléphone : « Nouveau code ».
        app.account_signed_in(attempt, Err(SignInFail::Denied));
        assert!(app.accounts.flow.denied && app.focus == Target::AccountNewCode);
        let acts = app.activate(Target::AccountNewCode);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: "share.kybernow.app".into() }]);
        assert!(!app.accounts.flow.denied);
        // Approuvé : le compte est gardé, l'accueil revient, ses ordinateurs sont demandés.
        let acts = app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(acts, vec![Action::Save, Action::AccountsRefresh]);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.store.accounts.len(), 1);
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Connecté à Léa Martin."));
    }

    #[test]
    fn another_relay() {
        let mut app = tv();
        app.go(Screen::Home);
        app.activate(Target::AccountSignIn);
        let acts = app.activate(Target::AccountOtherRelay);
        assert_eq!(acts, vec![Action::AccountCancel, Action::TextInput]);
        assert!(app.typing_relay());
        assert_eq!(app.focus, Target::Key('1'));
        for c in "Relais.Exemple.org:9443".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!(app.accounts.flow.field, "relais.exemple.org:9443");
        app.focus = Target::AccountRelayField;
        let acts = app.nav(Nav::Ok);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: "relais.exemple.org:9443".into() }]);
        // Un relais sans comptes : retour au champ, le refus dessous.
        app.account_code(attempt, Err("Ce relais n'a pas de comptes KyberNow.".into()));
        assert_eq!(app.accounts.flow.step, AccountStep::OtherRelay);
        assert_eq!(app.accounts.flow.field_message.as_deref(), Some("Ce relais n'a pas de comptes KyberNow."));
        // Retour : un nouveau code du relais public.
        let acts = app.back();
        assert!(matches!(&acts[..], [Action::AccountCode { relay, .. }] if relay.is_empty()));
        // Retour encore : l'accueil, l'attente abandonnée.
        assert_eq!(app.back(), vec![Action::AccountCancel]);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn signing_in_on_the_desktop() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.go(Screen::Home);
        let acts = app.activate(Target::AccountSignIn);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountBrowser { attempt, relay: String::new() }]);
        assert_eq!(app.accounts.flow.step, AccountStep::Browser);
        // « Annuler » dans la page du relais : l'accueil, tel quel.
        app.account_signed_in(attempt, Err(SignInFail::Cancelled));
        assert_eq!(app.screen, Screen::Home);
        assert!(app.store.accounts.is_empty());
        // « Ajouter un compte » : le relais d'abord.
        app.open_settings(true, Screen::Home);
        let acts = app.account_choose(Setting::AccountAdd).unwrap();
        assert!(acts.is_empty());
        assert_eq!((app.screen, app.accounts.flow.step.clone()), (Screen::Account, AccountStep::Relay));
        let acts = app.activate(Target::AccountRelayDefault);
        assert!(matches!(&acts[..], [Action::AccountBrowser { relay, .. }] if relay.is_empty()));
        // Le retour de la page (lien `kybernow://compte`) arrive : connecté, retour aux réglages.
        let attempt = app.accounts.flow.attempt;
        app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(app.screen, Screen::Settings);
        assert_eq!(app.focus, Target::Setting(Setting::AccountAdd));
    }

    #[test]
    fn computers_of_the_account_on_the_home_screen() {
        let mut app = tv();
        app.store.put_account(lea());
        let a = computer("st_aaaa", "Tour du salon", "sha256:AA");
        let b = computer("st_bbbb", "Atelier", "sha256:BB");
        app.account_computers(&lea().key(), vec![a.clone(), b.clone()]);
        let origins: HashMap<String, String> = HashMap::new();
        app.set_account_hosts(&[a.clone(), b.clone()], origins);
        assert_eq!(app.store.hosts.len(), 2);
        assert!(app.store.hosts.iter().all(|h| h.user.is_empty() && h.password.is_empty() && h.address.is_empty()));
        let id = app.store.account_hosts[0].id.clone();
        // La même liste : mêmes identifiants (réglages et dernière session gardés).
        app.store.host_mut(&id).unwrap().settings.profile = crate::model::Profile::Max;
        app.set_account_hosts(&[a.clone(), b.clone()], HashMap::new());
        assert_eq!(app.store.host(&id).map(|h| h.settings.profile), Some(crate::model::Profile::Max));
        // Vu sur le réseau (même empreinte) : son adresse ; absent : plus d'adresse.
        assert!(app.store.locate_account_hosts(&[("192.168.1.20".into(), 47990, Some("sha256:aa".into()))]));
        assert_eq!(app.store.host(&id).map(|h| (h.address.as_str(), h.port)), Some(("192.168.1.20", 47990)));
        assert!(app.store.locate_account_hosts(&[]));
        assert_eq!(app.store.host(&id).map(|h| h.address.as_str()), Some(""));
        // L'ordinateur n'est plus sur le compte : il quitte l'accueil.
        let acts = app.account_unknown_computer(&id, "Cet ordinateur n'est plus sur votre compte.".into());
        assert_eq!(acts, vec![Action::Save, Action::AccountsSync]);
        assert_eq!(app.store.account_computers, vec![b.clone()]);
        app.set_account_hosts(std::slice::from_ref(&b), HashMap::new());
        assert!(app.store.host(&id).is_none());
        // La session d'appareil fermée : le compte et ses ordinateurs partent.
        let acts = app.account_signed_out(&lea().key(), "Ce compte a été déconnecté de cet appareil : reconnectez-vous.".into());
        assert_eq!(acts, vec![Action::Save, Action::AccountsSync]);
        assert!(app.store.accounts.is_empty() && app.store.hosts.is_empty() && app.store.account_hosts.is_empty());
    }

    #[test]
    fn the_accounts_section() {
        let mut app = tv();
        app.open_settings(true, Screen::Home);
        let rows = app.account_rows();
        assert_eq!(rows[0].label, "Aucun compte.");
        assert_eq!(rows[1].setting, Setting::AccountAdd);
        app.store.put_account(lea());
        app.store.put_account(KnAccount { relay: "relais.exemple.org:9443#sha256:abcd".into(), account: KnUser { id: "acc_2".into(), email: "hugo@exemple.org".into(), ..Default::default() }, ..Default::default() });
        let rows = app.account_rows();
        assert_eq!((rows[0].label.as_str(), rows[0].hint.as_str()), ("Léa Martin", "lea@exemple.org"));
        assert_eq!(rows[0].kind, RowKind::Info("share.kybernow.app".into()));
        assert_eq!((rows[3].label.as_str(), rows[3].kind.clone()), ("hugo@exemple.org", RowKind::Info("relais.exemple.org:9443".into())));
        assert_eq!(rows.iter().rev().nth(1).map(|r| r.setting), Some(Setting::AccountAdd));
        // « Gérer le compte » au salon : le QR code de la page du compte.
        app.account_choose(Setting::AccountManage(1));
        assert_eq!(app.managed_account().map(|a| a.console()), Some("https://relais.exemple.org:9443/compte".into()));
        app.back();
        assert_eq!(app.screen, Screen::Settings);
        // « Se déconnecter », confirmé.
        app.account_choose(Setting::AccountSignOut(0));
        assert_eq!(app.confirm, Some(Confirm::SignOut { account: lea().key(), name: "Léa Martin".into() }));
        let acts = app.activate(Target::ConfirmYes);
        assert!(matches!(&acts[..], [Action::AccountSignOut(a), Action::Save, Action::AccountsSync] if a.token == "secret"));
        assert_eq!(app.store.accounts.len(), 1);
        assert_eq!(sign_out_question("Léa Martin"), "Se déconnecter de Léa Martin ?");
    }

    #[test]
    fn origin_badges_only_with_an_account() {
        let mut app = tv();
        app.accounts.origins.insert("a".into(), "Appairé".into());
        assert_eq!(app.origin("a"), None);
        app.store.put_account(lea());
        assert_eq!(app.origin("a"), Some("Appairé"));
    }

    #[test]
    fn the_token_stays_out_of_the_logs() {
        assert!(!format!("{:?}", lea()).contains("secret"));
        let json = serde_json::to_value(lea()).unwrap();
        assert_eq!(json["token"], "secret", "gardé entier dans le fichier");
        assert_eq!(crate::model::relay_label("relais.exemple.org:443"), "relais.exemple.org");
    }
}
