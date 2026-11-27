//! Accueil au premier lancement et incitation au compte KyberNow, côté interface (sans le cœur ni SDL) ; textes et
//! règles : docs/link-ui.md, « Accueil et compte KyberNow » ; logique commune : `apps/core/src/welcome.rs`, reprise
//! telle quelle (`crate::welcome_rules`), l'état gardé dans `AppSettings::welcome` (kybernow.json).
//!
//! - **Accueil en trois écrans** (`Screen::Welcome`) : au premier lancement, et une fois après la mise à jour qui
//!   l'apporte, sans compte (`WelcomeState::should_show`) ; « Passer », « Plus tard » ou la fin le marquent vu.
//!   Rejouable : Réglages de l'app › Comptes › « Revoir l'accueil ». Écran 2 : au bureau « Connecter un compte » (la
//!   page du relais, comme toujours ; réussie, l'écran 3 est passé) et « Créer un compte » (la page d'inscription dans
//!   le navigateur par `xdg-open`, puis l'avis) ; au Deck, au salon et sur KyberNow Link OS, le code de la connexion
//!   par code et son QR code dans une carte à droite, « Continuer sans compte » en bouton. Écran 3 : les ordinateurs
//!   vus sur le réseau (« Ajouter »), « Rejoindre avec un code », le lien de la Station.
//! - **Sans compte, ensuite** : « Connecter un compte » en haut de l'accueil ; la carte du compte en fin de rangée des
//!   ordinateurs (« Fermer », puis revenue après 30 jours « Ne plus afficher ») ; le bandeau après un appairage ; la
//!   fenêtre « Hors de chez vous ? » quand « Se connecter » ou « Réessayer » vise un ordinateur appairé hors ligne (ni sur ce réseau,
//!   ni par le relais), une fois par ordinateur et par jour.
//! - Au Deck, au salon et sur Link OS (connexion par code), « Créer un compte » n'est pas proposé sur la carte ni dans
//!   la fenêtre : pas de navigateur sous la main ; l'écran du code dit « Pas encore de compte ? Créez-le sur cette
//!   page. », et la page du code le propose.

pub use crate::welcome_rules::{sign_up_url, WelcomeState, STATION_DOWNLOAD_URL};

use crate::app::{Action, App, Nav, Online, Row, RowKind, Screen, Setting, Target};
use crate::i18n::{english, tr, Lang};
use crate::model::{now_secs, Host};

/// La fenêtre « Hors de chez vous ? » : l'ordinateur, son nom, et ce que voulait le bouton principal (« Pas
/// maintenant » le fait quand même : réveiller, réessayer).
#[derive(Clone, Debug, PartialEq)]
pub struct AwayHint {
    pub host: String,
    pub name: String,
    pub then: Target,
}

/// Ce que l'interface montre de l'accueil et du compte.
#[derive(Clone, Debug, Default)]
pub struct WelcomeUi {
    /// L'écran de l'accueil en trois écrans (1 à 3).
    pub step: u8,
    /// « Hors de chez vous ? », ouverte par-dessus l'accueil.
    pub away: Option<AwayHint>,
    /// Le bandeau après l'appairage de cet ordinateur (jusqu'à « Plus tard » ou une connexion au compte).
    pub paired: Option<String>,
}

/// « BIENVENUE · {n} / 3 ».
pub fn step_label(n: u8) -> String {
    format!("{} · {n} / 3", tr("Bienvenue", "Welcome"))
}

pub fn title(n: u8) -> &'static str {
    match n {
        1 => tr("Votre ordinateur, sur tous vos appareils", "Your computer, on all your devices"),
        2 => tr("Avec un compte, tout vous suit", "With an account, everything follows you"),
        _ => tr("Ajoutez votre ordinateur", "Add your computer"),
    }
}

pub fn text(n: u8) -> &'static str {
    match n {
        1 => tr(
            "Jouez, travaillez et retrouvez votre bureau depuis un autre écran, comme si vous étiez devant.",
            "Play, work and get back to your desktop from another screen, as if you were sitting in front of it.",
        ),
        2 => tr("Un compte KyberNow, et vos ordinateurs vous attendent partout.", "A KyberNow account, and your computers wait for you everywhere."),
        _ => tr(
            "Installez KyberNow Station sur l'ordinateur à rejoindre : sur le même réseau, il apparaît ici tout seul. Un ami vous a donné un code ? Entrez-le.",
            "Install KyberNow Station on the computer you want to reach: on the same network, it shows up here by itself. A friend gave you a code? Enter it.",
        ),
    }
}

/// Les quatre avantages de l'écran 2 : étiquette, texte.
pub fn perks() -> [(&'static str, &'static str); 4] {
    [
        (tr("Hors de chez vous", "Away from home"), tr("Rejoignez votre ordinateur de n'importe où, sans rien régler sur votre box.", "Reach your computer from anywhere, with nothing to set up on your router.")),
        (tr("Tous vos appareils", "All your devices"), tr("Vos ordinateurs apparaissent d'eux-mêmes partout où vous vous connectez.", "Your computers show up by themselves wherever you sign in.")),
        (tr("Partager et prêter", "Share and lend"), tr("Invitez un ami à jouer, prêtez votre PC, ouvrez-le à la famille ou à votre équipe.", "Invite a friend to play, lend your PC, open it to your family or your team.")),
        (tr("Vos réglages", "Your settings"), tr("Profil, format et débit vous suivent d'un appareil à l'autre.", "Profile, format and bitrate follow you from one device to another.")),
    ]
}

/// « La page de {relais} s'ouvre : votre mot de passe ne passe jamais par l'app. »
pub fn relay_help(relay: &str) -> String {
    if english() { format!("{relay}'s page opens: your password never goes through the app.") } else { format!("La page de {relay} s'ouvre : votre mot de passe ne passe jamais par l'app.") }
}

pub fn connect_label() -> &'static str {
    tr("Connecter un compte", "Connect an account")
}

pub fn create_label() -> &'static str {
    tr("Créer un compte", "Create an account")
}

/// L'avis après « Créer un compte ».
pub fn sign_up_notice() -> &'static str {
    tr("Créez votre compte dans le navigateur, puis revenez ici : « Connecter un compte ».", "Create your account in the browser, then come back here: “Connect an account”.")
}

/// Sous le code, au salon.
pub fn no_account_yet() -> &'static str {
    tr("Pas encore de compte ? Créez-le sur cette page.", "No account yet? Create it on this page.")
}

/// Le texte de « Hors de chez vous ? ».
pub fn away_text(name: &str) -> String {
    if english() {
        format!("{name} isn't on this network. To reach it from where you are, connect a KyberNow account: your computer follows you everywhere, with nothing to set up on your router.")
    } else {
        format!("{name} n'est pas sur ce réseau. Pour le rejoindre d'où vous êtes, connectez un compte KyberNow : votre ordinateur vous suit partout, sans rien régler sur votre box.")
    }
}

/// Le bandeau après un appairage.
pub fn paired_text() -> &'static str {
    tr("Retrouvez-le aussi hors de chez vous et sur vos autres appareils : connectez un compte KyberNow.", "Find it away from home and on your other devices too: connect a KyberNow account.")
}

/// « Revoir l'accueil », dans Réglages de l'app › Comptes.
pub fn again_row() -> Row {
    Row { setting: Setting::WelcomeAgain, label: tr("Revoir l'accueil", "Show the welcome again").into(), hint: String::new(), kind: RowKind::Action(String::new()), enabled: true, tag: None }
}

impl App {
    /// L'écran 2 de l'accueil montre le code de la connexion par code (Deck, salon, Link OS).
    pub fn welcome_code(&self) -> bool {
        self.screen == Screen::Welcome && self.welcome.step == 2 && self.sign_in_by_code()
    }

    /// L'accueil, ou la première fois (et une fois après la mise à jour, sans compte) l'accueil en trois écrans.
    pub fn home_or_welcome(&mut self) {
        if self.store.app.welcome.should_show(self.has_accounts()) {
            self.open_welcome();
        } else {
            self.go(Screen::Home);
        }
    }

    /// L'accueil en trois écrans, depuis le premier (premier lancement, « Revoir l'accueil »).
    pub fn open_welcome(&mut self) {
        self.welcome.step = 1;
        self.welcome.away = None;
        self.go(Screen::Welcome);
    }

    /// Le focus en arrivant sur un écran de l'accueil.
    pub fn welcome_focus(&self) -> Target {
        match self.welcome.step {
            1 => Target::WelcomeNext,
            2 if self.sign_in_by_code() => Target::WelcomeNoAccount,
            2 => Target::WelcomeSignIn,
            _ if !self.new_found().is_empty() => Target::WelcomeFound(0),
            _ => Target::WelcomeJoin,
        }
    }

    /// Change d'écran ; au salon, l'écran 2 demande un code en arrivant et l'abandonne en partant.
    fn welcome_step(&mut self, step: u8) -> Vec<Action> {
        let mut acts = self.welcome_leave_code();
        self.welcome.step = step.clamp(1, 3);
        self.go(Screen::Welcome);
        if self.welcome_code() {
            self.accounts.flow = crate::accounts::AccountFlow { attempt: self.accounts.flow.attempt + 1, back_to: Some(Screen::Home), ..Default::default() };
            acts.extend(self.account_activate(Target::AccountNewCode).unwrap_or_default());
            self.focus = Target::WelcomeNoAccount;
        }
        acts
    }

    /// Le code de l'écran 2 est abandonné (on part de l'écran) : son attente s'arrête, une réponse tardive est écartée.
    fn welcome_leave_code(&mut self) -> Vec<Action> {
        if !self.welcome_code() {
            return Vec::new();
        }
        self.accounts.flow = crate::accounts::AccountFlow { attempt: self.accounts.flow.attempt + 1, ..Default::default() };
        vec![Action::AccountCancel]
    }

    /// Fin de l'accueil (« Passer », « Plus tard », la fin, un ordinateur ajouté…) : vu, l'accueil de l'app.
    fn welcome_finish(&mut self) -> Vec<Action> {
        let mut acts = self.welcome_leave_code();
        self.store.app.welcome.welcome_done();
        self.go(Screen::Home);
        acts.push(Action::Save);
        acts
    }

    /// Retour (B, Échap) : l'écran précédent ; le premier ne recule pas.
    pub fn welcome_back(&mut self) -> Vec<Action> {
        if self.welcome.step > 1 {
            let step = self.welcome.step - 1;
            return self.welcome_step(step);
        }
        Vec::new()
    }

    /// « Créer un compte » : la page d'inscription du relais public dans le navigateur, puis l'avis.
    fn sign_up(&mut self) -> Vec<Action> {
        self.toast(sign_up_notice());
        vec![Action::OpenUrl(sign_up_url("", Lang::current()))]
    }

    /// La carte du compte, en fin de rangée des ordinateurs : sans compte, pas fermée (ou revenue), et pas sur
    /// l'accueil vide (qui a déjà « Connecter un compte »).
    pub fn account_card_visible(&self) -> bool {
        let empty = self.store.hosts.is_empty() && self.store.borrowed.is_empty() && self.store.ps_consoles.is_empty();
        !empty && self.store.app.welcome.card_visible(self.has_accounts(), now_secs())
    }

    /// Le bouton du compte en haut de l'accueil : sans compte.
    pub fn account_button_visible(&self) -> bool {
        self.screen == Screen::Home && !self.has_accounts()
    }

    /// Le bandeau après un appairage, sous les boutons de cet ordinateur.
    /// Un seul bandeau à la fois sous « Se connecter » : celui de la mise à jour de la Station passe d'abord ; le
    /// bandeau du compte attend qu'il soit parti.
    pub fn paired_banner(&self, host: &str) -> bool {
        !self.has_accounts() && self.welcome.paired.as_deref() == Some(host) && self.home_banner(host).is_none()
    }

    /// Le bandeau après un appairage a été vu pour de bon (« Plus tard », « Connecter un compte », ou « Se connecter »
    /// pendant qu'il se voyait) : compté une fois pour cet ordinateur (`take_paired_hint`), jamais avant de s'être
    /// montré. Rend vrai s'il faut l'enregistrer.
    fn paired_seen(&mut self) -> bool {
        let Some(id) = self.welcome.paired.take() else { return false };
        let has = self.has_accounts();
        self.store.app.welcome.take_paired_hint(has, &id)
    }

    /// Un ordinateur appairé (Admin a accepté le code) : gardé, choisi, l'accueil ; sans compte et pas encore vu pour
    /// cet ordinateur, le bandeau du compte attend sous ses boutons (`paired_banner`).
    pub fn paired(&mut self, host: Host) -> String {
        let name = host.name.clone();
        let id = self.store.put(host);
        self.store.selected = Some(id.clone());
        if !self.has_accounts() && !self.store.app.welcome.paired_hint.contains(&id) {
            self.welcome.paired = Some(id.clone());
        }
        self.toast(format!("{name} {}", tr("est ajouté", "added")));
        self.go(Screen::Home);
        id
    }

    /// Le bouton principal de l'accueil vise un ordinateur appairé hors ligne, sans compte : « Hors de chez vous ? »,
    /// une fois par ordinateur et par jour.
    /// Jamais sur « Réveiller l'ordinateur » : un ordinateur qu'on réveille est chez soi (comme Android et Windows).
    fn away_check(&mut self, then: Target) -> Option<Vec<Action>> {
        if then == Target::Wake || self.screen != Screen::Home || self.welcome.away.is_some() || self.has_accounts() {
            return None;
        }
        let h = self.selected()?;
        let (id, name) = (h.id.clone(), h.name.clone());
        let offline = self.live.get(&id).is_some_and(|l| l.online == Online::Offline);
        if !offline || self.store.account_host(&id).is_some() || self.host_lent(&id) {
            return None;
        }
        if !self.store.app.welcome.take_away_hint(false, &id, now_secs()) {
            return None;
        }
        self.welcome.away = Some(AwayHint { host: id, name, then });
        self.focus = Target::AwaySignIn;
        Some(vec![Action::Save])
    }

    /// La manette et le clavier pendant « Hors de chez vous ? » (`None` : elle n'est pas ouverte).
    pub fn welcome_nav(&mut self, n: Nav) -> Option<Vec<Action>> {
        self.welcome.away.as_ref()?;
        Some(match n {
            Nav::Ok => {
                let t = self.focus;
                self.activate(t)
            }
            Nav::Back => self.activate(Target::AwayLater),
            Nav::Up | Nav::Down | Nav::Left | Nav::Right => {
                self.move_focus(n);
                Vec::new()
            }
            _ => Vec::new(),
        })
    }

    /// Ce que font les éléments de l'accueil et de l'incitation au compte (`None` : une autre cible).
    pub fn welcome_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        Some(match t {
            Target::Connect | Target::Wake | Target::Retry => {
                if let Some(acts) = self.away_check(t) {
                    return Some(acts);
                }
                // Le bandeau après un appairage se voyait : il compte pour vu, et le bouton fait ce qu'il fait.
                let shown = self.screen == Screen::Home && self.selected().is_some_and(|h| self.paired_banner(&h.id));
                if shown && self.paired_seen() {
                    let mut acts = self.activate(t);
                    acts.push(Action::Save);
                    return Some(acts);
                }
                return None;
            }
            Target::WelcomeSkip => self.welcome_finish(),
            Target::WelcomeNext => self.welcome_step(self.welcome.step + 1),
            Target::WelcomeNoAccount => self.welcome_step(3),
            Target::WelcomeSignIn => {
                self.accounts.flow.back_to = None;
                self.account_sign_in(Screen::Welcome)
            }
            Target::WelcomeSignUp => self.sign_up(),
            Target::WelcomeFound(i) => {
                let mut acts = self.welcome_finish();
                acts.extend(self.activate(Target::Found(i)));
                acts
            }
            Target::WelcomeJoin => {
                let mut acts = self.welcome_finish();
                acts.extend(self.activate(Target::JoinCard));
                acts
            }
            Target::WelcomeDownload => vec![Action::OpenUrl(STATION_DOWNLOAD_URL.into())],
            Target::AccountTop | Target::CardSignIn => {
                self.accounts.flow.back_to = None;
                self.account_sign_in(Screen::Home)
            }
            Target::CardSignUp | Target::AwaySignUp => {
                self.welcome.away = None;
                self.go(Screen::Home);
                self.sign_up()
            }
            Target::CardClose => {
                self.store.app.welcome.close_card(now_secs());
                self.go(Screen::Home);
                vec![Action::Save]
            }
            Target::PairedSignIn => {
                self.paired_seen();
                self.accounts.flow.back_to = None;
                let mut acts = self.account_sign_in(Screen::Home);
                acts.push(Action::Save);
                acts
            }
            Target::PairedLater => {
                self.paired_seen();
                self.focus = Target::Connect;
                vec![Action::Save]
            }
            Target::AwaySignIn => {
                self.welcome.away = None;
                self.accounts.flow.back_to = None;
                self.account_sign_in(Screen::Home)
            }
            // « Pas maintenant » : ce que voulait le bouton principal (réessayer) se fait quand même.
            Target::AwayLater => {
                let Some(a) = self.welcome.away.take() else { return Some(Vec::new()) };
                self.focus = a.then;
                self.activate(a.then)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{HostLive, Layout};
    use crate::model::{KnAccount, KnUser, Store};

    fn host(id: &str) -> Host {
        crate::preview::host(id, "Tour du salon", "192.168.1.42")
    }

    fn lea() -> KnAccount {
        KnAccount { relay: "share.kybernow.app".into(), token: "t".into(), account: KnUser { id: "acc_lea".into(), name: "Léa".into(), email: "lea@exemple.org".into() }, ..Default::default() }
    }

    #[test]
    fn premier_lancement_au_bureau() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.home_or_welcome();
        assert_eq!((app.screen, app.welcome.step, app.focus), (Screen::Welcome, 1, Target::WelcomeNext));
        app.activate(Target::WelcomeNext);
        assert_eq!((app.welcome.step, app.focus), (2, Target::WelcomeSignIn));
        // « Créer un compte » : le navigateur, puis l'avis ; on reste sur l'écran 2.
        let acts = app.activate(Target::WelcomeSignUp);
        assert_eq!(acts, vec![Action::OpenUrl("https://share.kybernow.app/compte/inscription?lang=fr".into())]);
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(sign_up_notice()));
        assert_eq!(app.screen, Screen::Welcome);
        // « Connecter un compte » puis « Annuler » dans la page du relais : retour à l'écran 2.
        let acts = app.activate(Target::WelcomeSignIn);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountBrowser { attempt, relay: String::new() }]);
        assert_eq!(app.screen, Screen::Account);
        app.account_signed_in(attempt, Err(crate::accounts::SignInFail::Cancelled));
        assert_eq!((app.screen, app.welcome.step), (Screen::Welcome, 2));
        // Réussie : l'écran 3 est passé, l'accueil est vu.
        app.activate(Target::WelcomeSignIn);
        let attempt = app.accounts.flow.attempt;
        app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.store.app.welcome.should_show(false));
        // L'état est gardé dans le fichier.
        let json = serde_json::to_value(&app.store.app).unwrap();
        assert_eq!(json["welcome"]["welcome_seen"], 1);
    }

    #[test]
    fn passer_et_ecran_trois() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.home_or_welcome();
        assert_eq!(app.activate(Target::WelcomeSkip), vec![Action::Save]);
        assert_eq!(app.screen, Screen::Home);
        app.home_or_welcome();
        assert_eq!(app.screen, Screen::Home, "une seule fois");
        // Rejoué depuis les réglages : jusqu'à l'écran 3, « Rejoindre avec un code ».
        app.open_settings(true, Screen::Home);
        app.account_choose(Setting::WelcomeAgain);
        assert_eq!((app.screen, app.welcome.step), (Screen::Welcome, 1));
        app.activate(Target::WelcomeNext);
        app.activate(Target::WelcomeNoAccount);
        assert_eq!((app.welcome.step, app.focus), (3, Target::WelcomeJoin));
        assert_eq!(app.activate(Target::WelcomeDownload), vec![Action::OpenUrl(STATION_DOWNLOAD_URL.into())]);
        app.activate(Target::WelcomeJoin);
        assert_eq!(app.screen, Screen::Join);
        // Avec un compte : jamais.
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.put_account(lea());
        app.home_or_welcome();
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn ecran_deux_au_salon_par_code() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Tv);
        app.launch_done();
        assert_eq!(app.screen, Screen::Welcome);
        let acts = app.activate(Target::WelcomeNext);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: String::new() }]);
        assert_eq!((app.screen, app.focus), (Screen::Welcome, Target::WelcomeNoAccount));
        app.account_code(attempt, Ok(crate::preview::kn_code()));
        assert!(app.accounts.flow.code.is_some(), "le code s'affiche sur l'écran 2");
        // Refusé : « Nouveau code » redemande, sans quitter l'écran.
        app.account_signed_in(attempt, Err(crate::accounts::SignInFail::Denied));
        let acts = app.activate(Target::AccountNewCode);
        assert!(matches!(&acts[..], [Action::AccountCode { .. }]));
        assert_eq!(app.screen, Screen::Welcome);
        // « Continuer sans compte » : le code est abandonné, l'écran 3.
        let acts = app.activate(Target::WelcomeNoAccount);
        assert_eq!(acts, vec![Action::AccountCancel]);
        assert_eq!(app.welcome.step, 3);
        // Retour sur l'écran 2 : un nouveau code ; approuvé : l'accueil, vu.
        let acts = app.back();
        assert!(matches!(&acts[..], [Action::AccountCode { .. }]));
        let attempt = app.accounts.flow.attempt;
        app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.store.app.welcome.should_show(false));
    }

    #[test]
    fn carte_du_compte() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        assert!(!app.account_card_visible(), "accueil vide : pas de carte");
        app.store.hosts.push(host("a"));
        assert!(app.account_card_visible());
        app.activate(Target::CardClose);
        assert!(!app.account_card_visible());
        app.store.app.welcome.card_closed_at -= crate::welcome_rules::CARD_BACK_AFTER_SECS;
        assert!(app.account_card_visible() && app.store.app.welcome.card_close_is_final());
        app.activate(Target::CardClose);
        assert!(!app.account_card_visible(), "« Ne plus afficher »");
        let acts = app.activate(Target::CardSignIn);
        assert!(matches!(&acts[..], [Action::AccountBrowser { .. }]));
    }

    #[test]
    fn hors_de_chez_vous_et_apres_un_appairage() {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.app.welcome.welcome_done();
        let id = app.paired(host("a"));
        assert!(app.paired_banner(&id));
        // Un seul bandeau à la fois : la mise à jour de la Station passe d'abord, le bandeau du compte attend sans
        // être compté comme vu.
        app.live.entry(id.clone()).or_default().offer.banner = crate::station_offer::OfferBanner::Update("0.11.3".into());
        assert!(!app.paired_banner(&id));
        assert!(app.store.app.welcome.paired_hint.is_empty(), "pas encore vu");
        app.live.entry(id.clone()).or_default().offer.banner = crate::station_offer::OfferBanner::None;
        assert!(app.paired_banner(&id), "la mise à jour partie, il se montre");
        assert_eq!(app.activate(Target::PairedLater), vec![Action::Save]);
        assert!(!app.paired_banner(&id));
        assert_eq!(app.store.app.welcome.paired_hint, vec![id.clone()]);
        app.go(Screen::Home);
        // Hors ligne, sans réveil possible : « Réessayer » ouvre d'abord la fenêtre.
        app.live.insert(id.clone(), HostLive { online: Online::Offline, ..Default::default() });
        // « Réveiller » : jamais (un ordinateur qu'on réveille est chez soi).
        assert_eq!(app.activate(Target::Wake), vec![Action::Wake(id.clone())]);
        assert!(app.welcome.away.is_none());
        app.waking = None;
        assert_eq!(app.activate(Target::Retry), vec![Action::Save]);
        assert_eq!(app.welcome.away.as_ref().map(|a| a.name.as_str()), Some("Tour du salon"));
        assert_eq!(away_text("PC"), "PC n'est pas sur ce réseau. Pour le rejoindre d'où vous êtes, connectez un compte KyberNow : votre ordinateur vous suit partout, sans rien régler sur votre box.");
        // « Pas maintenant » : la fenêtre se ferme, et l'app réessaie.
        assert_eq!(app.nav(Nav::Back), vec![Action::Poll]);
        assert!(app.welcome.away.is_none());
        // Le même jour : plus de fenêtre.
        app.live.insert(id.clone(), HostLive { online: Online::Offline, ..Default::default() });
        assert_eq!(app.activate(Target::Retry), vec![Action::Poll]);
        // En ligne : jamais.
        let mut app = App::new(Store::default(), Layout::Desktop);
        app.store.hosts.push(host("b"));
        app.go(Screen::Home);
        app.live.insert("b".into(), HostLive { online: Online::Online, ..Default::default() });
        assert!(matches!(&app.activate(Target::Connect)[..], [Action::Connect { .. }]));
    }
}
