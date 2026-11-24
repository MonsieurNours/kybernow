//! Comptes KyberNow, côté interface (sans le cœur : les captures et les essais se font sans le téléviseur ni Kyber).
//! Contrat : docs/comptes.md (lot 3) ; textes : docs/link-ui.md, « Comptes KyberNow » (les erreurs et les pastilles
//! d'origine viennent du cœur, `kybernow_core::account`, par le pilote).
//!
//! - **Se connecter** : par un **code** (`Screen::Account`) : trois étapes, le code en chasse fixe, son QR code, « En
//!   attente de votre téléphone… Le code change dans {n} min. » ; le pilote attend l'approbation
//!   (`account::tv_wait`) et redemande un code échu de lui-même ; refusé, « Nouveau code ». Le relais est dit
//!   (« Relais : {relais} ») ; « Un autre relais… » ouvre le champ « Relais », au pavé de la télécommande.
//! - **Accueil** : sans ordinateur, « Retrouvez vos ordinateurs » et « Se connecter à un compte KyberNow » ; avec des
//!   comptes, leurs ordinateurs sont des ordinateurs de l'accueil comme les autres (`Store::account_hosts`), avec une
//!   pastille d'origine (`AccountsUi::origins`, calculée par le pilote avec `account::merge` et `origin`).
//! - **Réglages de l'app › Comptes** : chaque compte (nom, e-mail, relais), « Gérer le compte » (la page du compte en
//!   QR code), « Se déconnecter » (confirmé), « Ajouter un compte ». L'app ne crée ni ne supprime de compte.

use std::collections::HashMap;

use crate::app::{Action, App, Dialog, Nav, Screen, Setting, Target};
use crate::i18n::{english, tr};
use crate::model::{KnAccount, KnComputer};

/// Les étapes de l'écran des comptes (`Screen::Account`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AccountStep {
    /// Le code et son QR code.
    #[default]
    Code,
    /// « Un autre relais » : le champ « Relais », « Continuer », « Annuler ».
    OtherRelay,
    /// « Gérer le compte » : le QR code de la page du compte (clé du compte).
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
    /// Échéance (secondes Unix du téléviseur).
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
    /// Abandonnée (« Retour ») : rien à dire.
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
    pub denied: bool,
    /// Un code est demandé au relais.
    pub busy: bool,
    /// Un refus du relais (relais muet, trop d'essais…), à la place du code.
    pub message: Option<String>,
    /// « Un autre relais » : le champ, et le refus dessous.
    pub field: String,
    pub field_message: Option<String>,
    /// Le relais vient du champ : un refus y ramène.
    pub typed: bool,
    /// Le numéro de la demande en cours : une réponse plus ancienne est écartée.
    pub attempt: u32,
    /// L'écran d'où l'on vient (l'accueil, les réglages de l'app).
    pub back_to: Option<Screen>,
}

/// Les comptes vus par l'interface.
#[derive(Clone, Debug, Default)]
pub struct AccountsUi {
    pub flow: AccountFlow,
    /// La pastille d'origine de chaque ordinateur de l'accueil (`Host::id`), calculée par le pilote.
    pub origins: HashMap<String, String>,
}

/// « Connecté à {compte}. »
pub fn signed_in_notice(account: &str) -> String {
    if english() { format!("Signed in to {account}.") } else { format!("Connecté à {account}.") }
}

/// « Se déconnecter de {compte} ? »
pub fn sign_out_question(account: &str) -> String {
    if english() { format!("Sign out of {account}?") } else { format!("Se déconnecter de {account} ?") }
}

/// « Relais : {relais} ».
pub fn relay_line(relay: &str) -> String {
    format!("{} {}", tr("Relais :", "Relay:"), crate::model::relay_label(relay))
}

/// « En attente de votre téléphone… Le code change dans {n} min. »
pub fn waiting_line(minutes: u64) -> String {
    if english() { format!("Waiting for your phone… The code changes in {minutes} min.") } else { format!("En attente de votre téléphone… Le code change dans {minutes} min.") }
}

/// « ou ouvrez {adresse} et entrez le code, »
pub fn open_step(address: &str) -> String {
    if english() { format!("or open {address} and enter the code,") } else { format!("ou ouvrez {address} et entrez le code,") }
}

/// L'aide sous « Se connecter à un compte KyberNow » (un téléviseur : le code).
pub fn sign_in_help() -> &'static str {
    tr("Un code s'affiche : vous l'approuvez depuis votre téléphone.", "A code appears: you approve it from your phone.")
}

/// L'aide de la rubrique « Comptes ».
pub fn accounts_help() -> &'static str {
    tr("Créer un compte, changer son mot de passe ou le supprimer : sur la page du compte.", "Create an account, change its password or delete it: on the account's page.")
}

/// Le plus long relais qu'on tape.
const RELAY_MAX: usize = 200;

impl App {
    /// Le champ « Relais » attend des lettres.
    pub fn typing_relay(&self) -> bool {
        self.screen == Screen::Account && self.accounts.flow.step == AccountStep::OtherRelay
    }

    /// Un compte au moins est connecté : les pastilles d'origine se montrent.
    pub fn has_accounts(&self) -> bool {
        !self.store.accounts.is_empty()
    }

    /// La pastille d'origine d'un ordinateur de l'accueil, quand un compte est connecté.
    pub fn origin(&self, host: &str) -> Option<&str> {
        self.has_accounts().then(|| self.accounts.origins.get(host).map(String::as_str)).flatten().filter(|s| !s.is_empty())
    }

    /// « Se connecter à un compte KyberNow » (accueil vide, réglages) : l'écran du code, sur le relais public.
    pub fn account_sign_in(&mut self, from: Screen) -> Vec<Action> {
        self.accounts.flow = AccountFlow { attempt: self.accounts.flow.attempt + 1, back_to: Some(from), ..AccountFlow::default() };
        self.account_start(String::new(), false)
    }

    /// Un code part vers `relay` (vide : le relais public).
    fn account_start(&mut self, relay: String, typed: bool) -> Vec<Action> {
        let f = &mut self.accounts.flow;
        f.attempt += 1;
        f.relay = relay.clone();
        f.typed = typed;
        f.code = None;
        f.denied = false;
        f.message = None;
        f.field_message = None;
        f.busy = true;
        f.step = AccountStep::Code;
        let attempt = f.attempt;
        self.go(Screen::Account);
        vec![Action::AccountCode { attempt, relay }]
    }

    /// La réponse à la demande d'un code (et chaque nouveau code, quand le précédent est échu) ; un refus du relais.
    pub fn account_code(&mut self, attempt: u32, r: Result<TvCodeUi, String>) {
        let f = &mut self.accounts.flow;
        if f.attempt != attempt || self.screen != Screen::Account || f.step != AccountStep::Code {
            return;
        }
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

    /// La connexion a abouti ou non. Réussie, le compte est gardé (même après « Retour » : sa session d'appareil
    /// existe déjà), « Connecté à {compte}. », ses ordinateurs arrivent.
    pub fn account_signed_in(&mut self, attempt: u32, r: Result<KnAccount, SignInFail>) -> Vec<Action> {
        let current = self.screen == Screen::Account && self.accounts.flow.attempt == attempt && self.accounts.flow.step == AccountStep::Code;
        match r {
            Ok(a) => {
                self.toast(signed_in_notice(&a.label()));
                self.store.put_account(a);
                if current {
                    self.account_leave();
                }
                vec![Action::Save, Action::AccountsRefresh]
            }
            Err(_) if !current => Vec::new(),
            Err(SignInFail::Cancelled) => Vec::new(),
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

    /// La liste des ordinateurs d'un compte, arrivée du relais (empreintes déjà gardées par le pilote).
    pub fn account_computers(&mut self, key: &str, list: Vec<KnComputer>) {
        if self.store.account(key).is_none() {
            return;
        }
        self.store.account_computers.retain(|c| c.account != key);
        self.store.account_computers.extend(list);
    }

    /// La session d'appareil d'un compte est fermée : le compte est oublié, ses ordinateurs quittent l'accueil,
    /// `message` le dit (« Ce compte a été déconnecté de cet appareil : reconnectez-vous. », du cœur).
    pub fn account_signed_out(&mut self, key: &str, message: String) -> Vec<Action> {
        if self.store.account(key).is_none() {
            return Vec::new();
        }
        self.forget_account(key);
        self.toast(message);
        vec![Action::Save, Action::AccountsSync]
    }

    /// Un ordinateur du compte que le relais ne connaît plus : il quitte l'accueil, `message` le dit.
    pub fn account_unknown_computer(&mut self, host: &str, message: String) -> Vec<Action> {
        let Some(link) = self.store.account_host(host).cloned() else { return Vec::new() };
        self.store.account_computers.retain(|c| !(c.relay == link.relay && c.station == link.station));
        self.toast(message);
        vec![Action::Save, Action::AccountsSync]
    }

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
        if to == Screen::Settings {
            self.screen = Screen::Settings;
            self.focus = Target::Setting(Setting::AccountAdd);
        } else {
            self.go(Screen::Home);
        }
    }

    /// Retour sur l'écran des comptes.
    pub fn account_back(&mut self) -> Vec<Action> {
        match self.accounts.flow.step {
            // Retour au code (le précédent est abandonné : on en redemande un, au relais d'avant).
            AccountStep::OtherRelay => {
                let relay = if self.accounts.flow.typed { String::new() } else { self.accounts.flow.relay.clone() };
                self.account_start(relay, false)
            }
            AccountStep::Code => {
                self.account_leave();
                vec![Action::AccountCancel]
            }
            AccountStep::Manage(_) => {
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
            AccountStep::OtherRelay => Target::Key('1'),
            _ => Target::AccountBack,
        }
    }

    /// La télécommande et un clavier branché sur le champ « Relais » (`None` : rien de particulier).
    pub fn account_nav(&mut self, n: Nav) -> Option<Vec<Action>> {
        if !self.typing_relay() {
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

    /// « Continuer » (inactif tant que le champ est vide).
    fn relay_go(&mut self) -> Vec<Action> {
        let relay = self.accounts.flow.field.trim().to_string();
        if relay.is_empty() {
            return Vec::new();
        }
        self.account_start(relay, true)
    }

    /// « Un autre relais… » : le champ, vide ; l'attente du code en cours est abandonnée.
    fn other_relay(&mut self) -> Vec<Action> {
        let f = &mut self.accounts.flow;
        f.step = AccountStep::OtherRelay;
        f.attempt += 1;
        f.busy = false;
        f.field.clear();
        f.field_message = None;
        self.go(Screen::Account);
        vec![Action::AccountCancel]
    }

    /// Ce que font les éléments de l'écran des comptes et de l'accueil vide (`None` : une autre cible).
    pub fn account_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        Some(match t {
            Target::AccountSignIn => {
                let from = if self.screen == Screen::Settings { Screen::Settings } else { Screen::Home };
                self.account_sign_in(from)
            }
            Target::AccountOtherRelay => self.other_relay(),
            Target::AccountNewCode => {
                let relay = self.accounts.flow.relay.clone();
                self.account_start(relay, false)
            }
            Target::AccountBack | Target::AccountRelayCancel => self.account_back(),
            Target::AccountRelayField => Vec::new(),
            Target::AccountRelayGo => self.relay_go(),
            Target::KeyOk if self.typing_relay() => self.relay_go(),
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
    pub fn account_adjust(&mut self, s: Setting) -> Option<Vec<Action>> {
        Some(match s {
            Setting::AccountManage(i) => {
                let Some(a) = self.store.accounts.get(i as usize).cloned() else { return Some(Vec::new()) };
                // Pas de navigateur sur le téléviseur : la page du compte en QR code.
                self.accounts.flow = AccountFlow { step: AccountStep::Manage(a.key()), attempt: self.accounts.flow.attempt + 1, back_to: Some(Screen::Settings), ..AccountFlow::default() };
                self.go(Screen::Account);
                Vec::new()
            }
            Setting::AccountSignOut(i) => {
                if let Some(a) = self.store.accounts.get(i as usize) {
                    self.dialog = Some(Dialog::SignOut { account: a.key(), name: a.label() });
                    self.focus = Target::DialogCancel;
                }
                Vec::new()
            }
            // « Ajouter un compte » : le relais est dit sur l'écran du code, « Un autre relais… » en change.
            Setting::AccountAdd => self.account_sign_in(Screen::Settings),
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

    /// Le compte de la page « Gérer le compte ».
    pub fn managed_account(&self) -> Option<&KnAccount> {
        match &self.accounts.flow.step {
            AccountStep::Manage(key) => self.store.account(key),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{KnUser, Store};

    fn lea() -> KnAccount {
        KnAccount { relay: "share.kybernow.app".into(), token: "secret".into(), account: KnUser { id: "acc_lea".into(), name: "Léa Martin".into(), email: "lea@exemple.org".into() }, ..Default::default() }
    }

    fn computer(station: &str, name: &str, fp: &str) -> KnComputer {
        KnComputer { account: lea().key(), relay: "share.kybernow.app".into(), station: station.into(), name: name.into(), mine: true, admin_fp: Some(fp.into()), ..Default::default() }
    }

    fn tv() -> App {
        crate::i18n::force_english(false);
        App::new(Store::default())
    }

    #[test]
    fn signing_in_by_code() {
        let mut app = tv();
        app.go(Screen::Home);
        assert_eq!(app.focus, Target::AccountSignIn, "sans ordinateur, le compte d'abord");
        let acts = app.activate(Target::AccountSignIn);
        assert_eq!(app.screen, Screen::Account);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: String::new() }]);
        let code = TvCodeUi { code: "KQ7M-4XPA".into(), url_label: "share.kybernow.app/tv".into(), url_complete: "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA".into(), relay: "share.kybernow.app".into(), expires_at: 1_600 };
        app.account_code(attempt - 1, Ok(code.clone()));
        assert!(app.accounts.flow.code.is_none(), "une demande abandonnée est écartée");
        app.account_code(attempt, Ok(code.clone()));
        assert_eq!(code.minutes_left(1_000), 10);
        app.account_signed_in(attempt, Err(SignInFail::Denied));
        assert!(app.accounts.flow.denied && app.focus == Target::AccountNewCode);
        let acts = app.activate(Target::AccountNewCode);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: "share.kybernow.app".into() }]);
        let acts = app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(acts, vec![Action::Save, Action::AccountsRefresh]);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Connecté à Léa Martin."));
    }

    #[test]
    fn another_relay() {
        let mut app = tv();
        app.activate(Target::AccountSignIn);
        assert_eq!(app.activate(Target::AccountOtherRelay), vec![Action::AccountCancel]);
        assert!(app.typing_relay());
        for c in "Relais.Exemple.org:9443".chars() {
            app.nav(Nav::Char(c));
        }
        assert_eq!(app.accounts.flow.field, "relais.exemple.org:9443");
        let acts = app.activate(Target::AccountRelayGo);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: "relais.exemple.org:9443".into() }]);
        app.account_code(attempt, Err("Le relais ne répond pas.".into()));
        assert_eq!(app.accounts.flow.step, AccountStep::OtherRelay);
        assert_eq!(app.accounts.flow.field_message.as_deref(), Some("Le relais ne répond pas."));
        let acts = app.activate(Target::AccountBack);
        assert!(matches!(&acts[..], [Action::AccountCode { relay, .. }] if relay.is_empty()));
        assert_eq!(app.activate(Target::AccountBack), vec![Action::AccountCancel]);
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn computers_of_the_account() {
        let mut app = tv();
        app.store.put_account(lea());
        let a = computer("st_aaaa", "Tour du salon", "sha256:AA");
        app.account_computers(&lea().key(), vec![a.clone()]);
        app.set_account_hosts(std::slice::from_ref(&a), HashMap::new());
        let id = app.store.account_hosts[0].id.clone();
        assert!(app.store.host(&id).is_some_and(|h| h.user.is_empty() && h.fingerprint.as_deref() == Some("sha256:AA")));
        assert!(app.store.locate_account_hosts(&[("10.0.0.2".into(), 47990, Some("SHA256:aa".into()))]));
        assert_eq!(app.store.host(&id).map(|h| h.address.as_str()), Some("10.0.0.2"));
        assert_eq!(app.account_unknown_computer(&id, "x".into()), vec![Action::Save, Action::AccountsSync]);
        assert!(app.store.account_computers.is_empty());
        app.set_account_hosts(&[], HashMap::new());
        assert!(app.store.hosts.is_empty());
        app.account_signed_out(&lea().key(), "Ce compte a été déconnecté de cet appareil : reconnectez-vous.".into());
        assert!(app.store.accounts.is_empty());
    }

    #[test]
    fn manage_and_sign_out() {
        let mut app = tv();
        app.store.put_account(lea());
        app.open_settings(true, Screen::Home);
        app.account_adjust(Setting::AccountManage(0));
        assert_eq!(app.managed_account().map(|a| a.console()), Some("https://share.kybernow.app/compte".into()));
        app.activate(Target::AccountBack);
        assert_eq!(app.screen, Screen::Settings);
        app.account_adjust(Setting::AccountSignOut(0));
        assert_eq!(app.dialog, Some(Dialog::SignOut { account: lea().key(), name: "Léa Martin".into() }));
        let acts = app.activate(Target::DialogOk);
        assert!(matches!(&acts[..], [Action::AccountSignOut(a), Action::Save, Action::AccountsSync] if a.token == "secret"));
        assert!(app.store.accounts.is_empty());
        assert!(!format!("{:?}", lea()).contains("secret"));
    }
}
