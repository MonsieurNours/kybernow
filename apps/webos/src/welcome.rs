//! Accueil et compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow ») sur le téléviseur.
//!
//! - **Accueil** au premier lancement, et une fois après la mise à jour pour qui n'a pas de compte
//!   (`WelcomeState::should_show`) : trois écrans à la télécommande (`Screen::Welcome`). L'écran 2 montre à droite le
//!   QR code et le code de la connexion par code (comme l'écran des comptes) : approuvée, l'écran 3 est passé. Pas de
//!   navigateur sur le téléviseur : « Créer un compte » est la ligne « Pas encore de compte ? Créez-le sur cette
//!   page. » de la carte du code. L'écran 3 : les ordinateurs vus sur le réseau, « Rejoindre avec un code », la page
//!   de téléchargement de la Station (son adresse et son QR code). Rejouable : Réglages de l'app › Comptes, « Revoir
//!   l'accueil ».
//! - **Sans compte, ensuite** : « Connecter un compte » en haut de l'accueil ; la carte du compte en fin de rangée
//!   (`card_visible`, « Fermer » puis « Ne plus afficher ») ; « Hors de chez vous ? » quand « Se connecter » vise un
//!   ordinateur appairé absent du réseau (`take_away_hint`) ; le bandeau après un appairage (`take_paired_hint`).
//!
//! Les règles sont celles de `kybernow_rules::welcome` (la caisse des règles, sans le cœur : l'interface se compile
//! aussi pour les captures) ; `WelcomeState` est gardé dans `kybernow.json` (`Store::welcome`).

pub use kybernow_rules::welcome::{WelcomeState, STATION_DOWNLOAD_URL};

use crate::accounts::AccountFlow;
use crate::app::{Action, App, Dialog, Screen, Target};
use crate::i18n::{english, tr};

/// Ce que l'interface garde de l'accueil le temps de l'app (le reste est dans `WelcomeState`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WelcomeUi {
    /// L'écran de l'accueil : 1, 2 ou 3.
    pub step: u8,
    /// L'ordinateur sous lequel le bandeau « après un appairage » est montré.
    pub paired: Option<String>,
}

/// Au plus deux ordinateurs du réseau à l'écran 3 (avec les cartes « Un code » et « Pas encore installé »).
pub const FOUND_MAX: usize = 2;

// MARK: - Textes (docs/link-ui.md)

pub fn connect_account() -> &'static str {
    tr("Connecter un compte", "Connect an account")
}

pub fn welcome_again() -> &'static str {
    tr("Revoir l'accueil", "Show the welcome again")
}

/// « BIENVENUE · {n} / 3 ».
pub fn step_label(n: u8) -> String {
    format!("{} · {n} / 3", tr("BIENVENUE", "WELCOME"))
}

pub fn later() -> &'static str {
    tr("Plus tard", "Later")
}

pub fn skip() -> &'static str {
    tr("Passer", "Skip")
}

/// La croix de la carte du compte : « Fermer », puis, revenue, « Ne plus afficher ».
pub fn card_close_label(last: bool) -> &'static str {
    if last { tr("Ne plus afficher", "Don't show again") } else { tr("Fermer", "Close") }
}

pub fn away_title() -> &'static str {
    tr("Hors de chez vous ?", "Away from home?")
}

pub fn away_text(name: &str) -> String {
    if english() {
        format!("{name} isn't on this network. To reach it from where you are, connect a KyberNow account: your computer follows you everywhere, with nothing to set up on your router.")
    } else {
        format!("{name} n'est pas sur ce réseau. Pour le rejoindre d'où vous êtes, connectez un compte KyberNow : votre ordinateur vous suit partout, sans rien régler sur votre box.")
    }
}

pub fn paired_text() -> &'static str {
    tr(
        "Retrouvez-le aussi hors de chez vous et sur vos autres appareils : connectez un compte KyberNow.",
        "Find it away from home and on your other devices too: connect a KyberNow account.",
    )
}

/// Sur la carte du code (téléviseur) : « Créer un compte ».
pub fn no_account_yet() -> &'static str {
    tr("Pas encore de compte ? Créez-le sur cette page.", "No account yet? Create it on this page.")
}

/// L'adresse de la page de téléchargement, telle qu'écrite (le lien de l'écran 3).
pub fn download_label() -> &'static str {
    "www.kybernow.app"
}

impl App {
    fn has_account(&self) -> bool {
        !self.store.accounts.is_empty()
    }

    /// Au lancement : l'accueil, s'il est à montrer (premier lancement, ou la mise à jour qui l'apporte, sans compte).
    pub fn welcome_at_launch(&mut self) -> Vec<Action> {
        if self.store.welcome.should_show(self.has_account()) { self.open_welcome() } else { Vec::new() }
    }

    /// L'accueil, à son premier écran (aussi « Revoir l'accueil »).
    pub fn open_welcome(&mut self) -> Vec<Action> {
        self.dialog = None;
        self.welcome.step = 1;
        self.go(Screen::Welcome);
        Vec::new()
    }

    /// L'écran 2 montre la carte du code : une connexion part, comme « Connecter un compte » (relais public).
    pub fn welcome_code_shown(&self) -> bool {
        self.screen == Screen::Welcome && self.welcome.step == 2
    }

    /// Le focus en arrivant sur un écran de l'accueil.
    pub fn welcome_focus(&self) -> Target {
        match self.welcome.step {
            3 if !self.new_found().is_empty() => Target::Found(0),
            3 => Target::JoinCard,
            _ => Target::WelcomeNext,
        }
    }

    /// Passe à l'écran `step` ; l'écran 2 demande un code, le quitter l'abandonne.
    fn welcome_step(&mut self, step: u8) -> Vec<Action> {
        let mut acts = Vec::new();
        if self.welcome.step == 2 && step != 2 {
            self.accounts.flow = AccountFlow { attempt: self.accounts.flow.attempt + 1, ..AccountFlow::default() };
            acts.push(Action::AccountCancel);
        }
        self.welcome.step = step;
        if step == 2 {
            acts.extend(self.welcome_code());
        }
        self.go(Screen::Welcome);
        acts
    }

    /// Un code pour la carte de l'écran 2 (et « Nouveau code » après un refus).
    fn welcome_code(&mut self) -> Vec<Action> {
        let attempt = self.accounts.flow.attempt + 1;
        self.accounts.flow = AccountFlow { attempt, busy: true, back_to: Some(Screen::Home), ..AccountFlow::default() };
        vec![Action::AccountCode { attempt, relay: String::new() }]
    }

    /// La fin de l'accueil (« Passer », « Plus tard », un choix de l'écran 3, un compte connecté) : vu, gardé.
    pub fn welcome_finish(&mut self) -> Vec<Action> {
        let mut acts = Vec::new();
        if self.welcome.step == 2 {
            self.accounts.flow = AccountFlow { attempt: self.accounts.flow.attempt + 1, ..AccountFlow::default() };
            acts.push(Action::AccountCancel);
        }
        self.welcome.step = 0;
        self.store.welcome.welcome_done();
        self.go(Screen::Home);
        acts.push(Action::Save);
        acts
    }

    /// Retour : l'écran d'avant ; au premier, comme « Passer ».
    pub fn welcome_back(&mut self) -> Vec<Action> {
        match self.welcome.step {
            0 | 1 => self.welcome_finish(),
            n => self.welcome_step(n - 1),
        }
    }

    /// La carte du compte, en fin de rangée de l'accueil : sans compte, avec des ordinateurs à l'accueil (sans
    /// ordinateur, l'accueil vide dit déjà le compte), pas fermée (ou revenue après 30 jours).
    pub fn account_card_shown(&self) -> bool {
        let home = self.selected().is_some() || self.selected_borrowed().is_some() || self.selected_console().is_some();
        home && self.store.welcome.card_visible(self.has_account(), crate::model::now_secs())
    }

    /// Le bouton « Connecter un compte » en haut de l'accueil : sans compte, avec des ordinateurs à l'accueil.
    pub fn account_button_shown(&self) -> bool {
        !self.has_account() && (self.selected().is_some() || self.selected_borrowed().is_some() || self.selected_console().is_some())
    }

    /// Le bandeau « après un appairage » sous l'ordinateur `id` (sans compte). Un seul bandeau à la fois sous la
    /// rangée « Se connecter » : celui de la mise à jour de la Station passe d'abord, celui-ci attend.
    pub fn paired_banner(&self, id: &str) -> bool {
        !self.has_account() && self.welcome.paired.as_deref() == Some(id) && self.home_banner(id).is_none()
    }

    /// Un ordinateur vient d'être appairé (par le code) : le bandeau, une fois par ordinateur, sans compte. Il n'est
    /// compté comme vu (`take_paired_hint`) que quand il s'affiche (`paired_banner_seen`).
    pub fn after_pairing(&mut self, id: &str) {
        if !self.has_account() && !self.store.welcome.paired_hint.iter().any(|c| c == id) {
            self.welcome.paired = Some(id.to_string());
        }
    }

    /// Avant chaque dessin : le bandeau après un appairage est à l'écran (l'accueil, son ordinateur choisi, pas de
    /// bandeau de mise à jour) : il est compté comme vu, une fois. Rend vrai quand l'état est à garder.
    pub fn paired_banner_seen(&mut self) -> bool {
        let Some(id) = self.welcome.paired.clone() else { return false };
        let shown = self.screen == Screen::Home && self.selected().is_some_and(|h| h.id == id) && self.paired_banner(&id);
        let has = self.has_account();
        shown && self.store.welcome.take_paired_hint(has, &id)
    }

    /// « Se connecter » sur un ordinateur appairé hors ligne (le bouton dit alors « Réessayer »), sans compte : « Hors de
    /// chez vous ? », une fois par ordinateur et par jour au plus (`None` : le sondage de toujours). Pas sur
    /// « Réveiller l'ordinateur » : un ordinateur qu'on réveille est chez soi (comme l'app Android).
    pub fn away_hint(&mut self, id: &str) -> Option<Vec<Action>> {
        if self.has_account() || self.store.account_host(id).is_some() {
            return None;
        }
        let host = self.store.host(id)?;
        if host.can_wake() {
            return None;
        }
        let name = host.name.clone();
        if !self.store.welcome.take_away_hint(false, id, crate::model::now_secs()) {
            return None;
        }
        self.dialog = Some(Dialog::Away { host: id.to_string(), name });
        self.focus = Target::DialogOk;
        Some(vec![Action::Save])
    }

    /// Ce que font les éléments de l'accueil, de la carte du compte et du bandeau (`None` : une autre cible).
    pub fn welcome_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        if self.screen == Screen::Welcome {
            return Some(match t {
                Target::WelcomeSkip => self.welcome_finish(),
                Target::WelcomeNext if self.welcome.step >= 3 => self.welcome_finish(),
                Target::WelcomeNext => {
                    let next = self.welcome.step.max(1) + 1;
                    self.welcome_step(next)
                }
                Target::AccountNewCode => self.welcome_code(),
                // Écran 3 : ajouter un ordinateur du réseau, rejoindre avec un code ; l'accueil est fini.
                Target::Found(_) | Target::JoinCard => {
                    let mut acts = self.welcome_finish();
                    acts.extend(self.activate(t));
                    acts
                }
                _ => Vec::new(),
            });
        }
        Some(match t {
            Target::WelcomeCardConnect | Target::WelcomePairedConnect => {
                self.welcome.paired = None;
                self.account_sign_in(Screen::Home)
            }
            Target::WelcomeCardClose => {
                self.store.welcome.close_card(crate::model::now_secs());
                self.focus = Target::JoinCard;
                vec![Action::Save]
            }
            Target::WelcomePairedLater => {
                self.welcome.paired = None;
                self.focus = Target::Connect;
                Vec::new()
            }
            // « Pas maintenant » : le réveil ou le sondage de toujours.
            Target::DialogCancel if matches!(self.dialog, Some(Dialog::Away { .. })) => {
                self.dialog = None;
                self.focus = Target::Connect;
                self.activate(Target::Connect)
            }
            _ => return None,
        })
    }

    /// Une connexion par code a abouti : sur l'accueil, l'écran 3 est passé (les ordinateurs du compte arrivent).
    pub fn welcome_signed_in(&mut self) -> bool {
        if self.screen != Screen::Welcome {
            return false;
        }
        self.welcome.step = 0;
        self.store.welcome.welcome_done();
        self.go(Screen::Home);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::{SignInFail, TvCodeUi};
    use crate::app::{Found, HostLive, Nav, Online};
    use crate::model::{Host, KnAccount, KnUser, Store};

    fn tv() -> App {
        crate::i18n::force_english(false);
        App::new(Store::default())
    }

    fn host(id: &str) -> Host {
        Host {
            id: id.into(),
            name: "Atelier".into(),
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
            relay: None,
            settings: Default::default(),
            last: None,
        }
    }

    fn lea() -> KnAccount {
        KnAccount { relay: "share.kybernow.app".into(), token: "t".into(), account: KnUser { id: "acc_lea".into(), name: "Léa".into(), email: "lea@exemple.org".into() }, ..Default::default() }
    }

    #[test]
    fn three_screens_then_done() {
        let mut app = tv();
        assert!(app.welcome_at_launch().is_empty());
        assert_eq!((app.screen, app.welcome.step, app.focus), (Screen::Welcome, 1, Target::WelcomeNext));
        // Écran 2 : un code part.
        let acts = app.activate(Target::WelcomeNext);
        let attempt = app.accounts.flow.attempt;
        assert_eq!(acts, vec![Action::AccountCode { attempt, relay: String::new() }]);
        assert!(app.welcome_code_shown());
        let code = TvCodeUi { code: "KQ7M-4XPA".into(), url_label: "share.kybernow.app/tv".into(), url_complete: "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA".into(), relay: "share.kybernow.app".into(), expires_at: 1_600 };
        app.account_code(attempt, Ok(code));
        assert!(app.accounts.flow.code.is_some(), "le code de l'écran 2");
        app.account_signed_in(attempt, Err(SignInFail::Denied));
        assert!(app.accounts.flow.denied && app.screen == Screen::Welcome);
        let acts = app.activate(Target::AccountNewCode);
        assert!(matches!(&acts[..], [Action::AccountCode { .. }]) && app.screen == Screen::Welcome, "nouveau code, sur l'accueil");
        // « Continuer sans compte » : le code est abandonné.
        let acts = app.activate(Target::WelcomeNext);
        assert_eq!(acts, vec![Action::AccountCancel]);
        assert_eq!(app.welcome.step, 3);
        // Retour : l'écran 2, un nouveau code.
        assert!(matches!(&app.nav(Nav::Back)[..], [Action::AccountCode { .. }]));
        app.activate(Target::WelcomeNext);
        let acts = app.activate(Target::WelcomeSkip);
        assert!(acts.contains(&Action::Save));
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.store.welcome.should_show(false));
        assert!(app.welcome_at_launch().is_empty(), "une fois");
        // « Revoir l'accueil ».
        app.open_settings(true, Screen::Home);
        app.adjust_for_preview(crate::app::Setting::WelcomeAgain);
        assert_eq!((app.screen, app.welcome.step), (Screen::Welcome, 1));
    }

    #[test]
    fn signed_in_on_screen_two_skips_screen_three() {
        let mut app = tv();
        app.welcome_at_launch();
        app.activate(Target::WelcomeNext);
        let attempt = app.accounts.flow.attempt;
        let acts = app.account_signed_in(attempt, Ok(lea()));
        assert_eq!(acts, vec![Action::Save, Action::AccountsRefresh]);
        assert_eq!(app.screen, Screen::Home);
        assert!(!app.store.welcome.should_show(false));
        // Avec un compte : ni accueil, ni bouton, ni carte.
        let mut again = tv();
        again.store.put_account(lea());
        assert!(again.welcome_at_launch().is_empty() && again.screen == Screen::Home);
    }

    #[test]
    fn screen_three_adds_or_joins() {
        let mut app = tv();
        app.found.push(Found { name: "Salon-PC".into(), address: "192.168.1.42".into(), port: 31250, fingerprint: None });
        app.welcome_at_launch();
        app.activate(Target::WelcomeNext);
        app.activate(Target::WelcomeNext);
        assert_eq!(app.focus, Target::Found(0));
        let acts = app.activate(Target::Found(0));
        assert!(acts.contains(&Action::StartPairing) && acts.contains(&Action::Save));
        assert_eq!(app.screen, Screen::Add);
        assert!(!app.store.welcome.should_show(false));
    }

    #[test]
    fn card_button_and_banner_without_account() {
        let mut app = tv();
        app.store.welcome.welcome_done();
        assert!(!app.account_card_shown() && !app.account_button_shown(), "accueil vide : il dit déjà le compte");
        app.store.hosts.push(host("a"));
        assert!(app.account_card_shown() && app.account_button_shown());
        app.activate(Target::WelcomeCardClose);
        assert!(!app.account_card_shown(), "fermée : 30 jours");
        app.store.welcome.card_closed_at -= kybernow_rules::welcome::CARD_BACK_AFTER_SECS;
        assert!(app.account_card_shown() && app.store.welcome.card_close_is_final());
        app.activate(Target::WelcomeCardClose);
        assert!(!app.account_card_shown(), "ne plus afficher");
        // Après un appairage : une fois.
        app.after_pairing("a");
        assert!(app.paired_banner("a"));
        app.go(Screen::Home);
        assert!(app.paired_banner_seen(), "à l'écran : compté comme vu");
        assert!(!app.paired_banner_seen(), "une fois");
        app.activate(Target::WelcomePairedLater);
        assert!(!app.paired_banner("a"));
        app.after_pairing("a");
        assert!(!app.paired_banner("a"), "une fois par ordinateur");
        app.store.put_account(lea());
        assert!(!app.account_button_shown());
    }

    /// Un seul bandeau sous « Se connecter » : la mise à jour de la Station d'abord ; le bandeau après un appairage
    /// attend, sans être compté comme vu, puis s'affiche quand elle n'est plus là. Le bouton et la carte restent.
    #[test]
    fn station_update_banner_goes_first() {
        use crate::station_offer::OfferBanner;
        let mut app = tv();
        app.store.welcome.welcome_done();
        app.store.hosts.push(host("a"));
        app.live.insert("a".into(), HostLive { online: Online::Online, ..Default::default() });
        app.live.get_mut("a").unwrap().offer.banner = OfferBanner::Update("0.11.3".into());
        app.after_pairing("a");
        app.go(Screen::Home);
        assert!(app.home_banner("a").is_some());
        assert!(!app.paired_banner("a"), "la mise à jour d'abord");
        assert!(!app.paired_banner_seen() && app.store.welcome.paired_hint.is_empty(), "pas compté comme vu");
        assert!(app.account_button_shown() && app.account_card_shown(), "ce ne sont pas des bandeaux");
        // La mise à jour faite ou remise à plus tard : le bandeau du compte.
        app.live.get_mut("a").unwrap().offer.banner = OfferBanner::None;
        assert!(app.paired_banner("a"));
        assert!(app.paired_banner_seen());
        assert_eq!(app.store.welcome.paired_hint, vec!["a".to_string()]);
    }

    #[test]
    fn away_from_home_once_a_day() {
        let mut app = tv();
        app.store.hosts.push(host("a"));
        app.live.insert("a".into(), HostLive { online: Online::Offline, ..Default::default() });
        let acts = app.activate(Target::Connect);
        assert_eq!(acts, vec![Action::Save]);
        assert_eq!(app.dialog, Some(Dialog::Away { host: "a".into(), name: "Atelier".into() }));
        // « Pas maintenant » : le sondage de toujours.
        let acts = app.activate(Target::DialogCancel);
        assert_eq!(acts, vec![Action::Poll("a".into())]);
        assert!(app.dialog.is_none());
        app.live.insert("a".into(), HostLive { online: Online::Offline, ..Default::default() });
        assert_eq!(app.activate(Target::Connect), vec![Action::Poll("a".into())], "une fois par jour");
        // « Connecter un compte » : l'écran du code.
        app.store.welcome.away_hint.clear();
        app.live.insert("a".into(), HostLive { online: Online::Offline, ..Default::default() });
        app.activate(Target::Connect);
        let acts = app.activate(Target::DialogOk);
        assert!(matches!(&acts[..], [Action::AccountCode { .. }]));
        assert_eq!(app.screen, Screen::Account);
        // « Réveiller l'ordinateur » : un ordinateur qu'on réveille est chez soi, pas de fenêtre.
        let mut app = tv();
        let mut h = host("a");
        h.wake = vec![crate::model::WakeTarget { mac: "3c:7c:3f:12:ab:cd".into(), ..Default::default() }];
        app.store.hosts.push(h);
        app.live.insert("a".into(), HostLive { online: Online::Offline, ..Default::default() });
        assert_eq!(app.activate(Target::Connect), vec![Action::Wake("a".into())]);
        assert!(app.dialog.is_none() && app.store.welcome.away_hint.is_empty());
    }

    #[test]
    fn state_is_kept_in_the_settings_file() {
        let mut store = Store::default();
        store.welcome.welcome_done();
        store.welcome.close_card(1_800_000_000);
        let json = serde_json::to_string(&store).unwrap();
        assert!(json.contains("\"welcome\":{\"welcome_seen\":1"));
        let back: Store = serde_json::from_str(&json).unwrap();
        assert_eq!(back.welcome, store.welcome);
        // Un fichier d'avant : l'accueil est à montrer, une fois.
        let old: Store = serde_json::from_str("{\"hosts\":[]}").unwrap();
        assert!(old.welcome.should_show(false));
    }
}
