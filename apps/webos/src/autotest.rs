//! Profil Auto, côté interface (docs/link-ui.md, « Profil Auto et débit adaptatif » ; contrat :
//! docs/profil-auto.md) : le panneau du test, « Appliquer », le bandeau « Autre réseau que celui du test » de la
//! session. Le test lui-même est mené par le cœur (`autotest::run`, `src/device/autotest.rs`) : l'interface se
//! compile sans lui (captures), et reçoit ses étapes et son résumé déjà mis en mots, dans la langue de l'app.

use std::time::Instant;

use crate::app::{Action, App, Phase, Screen, Target};
use crate::i18n::tr;
use crate::model::Profile;

/// Le bandeau « Autre réseau que celui du test » : 8 s, ou la croix (`kybernow_ui::autotest::BANNER_SECONDS`).
pub const BANNER_SECONDS: f32 = kybernow_ui::autotest::BANNER_SECONDS as f32;

pub use kybernow_ui::autotest::{
    adaptive, adaptive_help, again, apply, cancel, chosen_by, other_network, other_network_text, phrase, retry, subtitle, summary, title, waiting_steps,
    Choice, Step, StepState,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AutoPhase {
    Running,
    /// « Réglages proposés » : le résultat (`AutoResult` du cœur, en JSON, gardé tel quel par « Appliquer ») et ses
    /// lignes.
    Done { result: serde_json::Value, choices: Vec<Choice> },
    /// « Le test n'a pas pu joindre {nom}. » (`AutoError::text` du cœur).
    Failed(String),
}

/// Le panneau du test ouvert pour un ordinateur, et d'où il a été ouvert (l'écran et l'élément qui y reprend le focus).
#[derive(Clone, Debug, PartialEq)]
pub struct AutoTestUi {
    pub host: String,
    pub name: String,
    pub attempt: u32,
    pub steps: Vec<Step>,
    pub phase: AutoPhase,
    pub from: Screen,
    pub from_focus: Target,
}

/// Ce que l'app rend au cœur pour un extrait décodé par NDL (`SampleDecoder`) : NDL n'a pas de décodage sans
/// affichage ; une mesure qui atteint au moins la fréquence de la dalle est bornée par l'affichage, pas par le
/// décodeur : `None` (le cœur suppose alors que le format matériel tient). Sous la fréquence de la dalle, la mesure.
pub fn decode_rate(measured: Option<f64>, panel_refresh: u32) -> Option<f64> {
    let fps = measured?;
    // 5 % de tolérance : une mesure qui oscille juste sous la fréquence de la
    // dalle (118 i/s à 120 Hz) est aussi bornée par l'affichage.
    if panel_refresh > 0 && fps >= f64::from(panel_refresh) * 0.95 { None } else { Some(fps) }
}

impl App {
    /// Le focus à l'ouverture du panneau, selon où en est le test.
    pub fn auto_focus(&self) -> Target {
        match self.auto.as_ref().map(|a| &a.phase) {
            Some(AutoPhase::Done { .. }) => Target::AutoApply,
            Some(AutoPhase::Failed(_)) => Target::AutoRetry,
            _ => Target::AutoCancel,
        }
    }

    /// Auto choisi (accueil, réglages) : ses réglages tout de suite s'ils sont gardés, sinon le panneau du test.
    pub fn choose_auto(&mut self, id: &str) -> Vec<Action> {
        let (panel, refresh) = (self.tv.panel, self.tv.refresh);
        let Some(h) = self.store.host_mut(id) else { return Vec::new() };
        match h.settings.auto.clone() {
            Some(result) if h.settings.auto_brief().is_some() => {
                h.settings.apply_auto(result, panel, refresh);
                vec![Action::Save]
            }
            _ => self.open_auto_test(id),
        }
    }

    /// Ouvre le panneau du test pour cet ordinateur et lance le test (un nouveau numéro : la réponse d'un test
    /// abandonné n'y entre pas).
    pub fn open_auto_test(&mut self, id: &str) -> Vec<Action> {
        let Some(name) = self.store.host(id).map(|h| h.name.clone()) else { return Vec::new() };
        // En session (réglages ouverts depuis son menu), le décodeur du téléviseur montre l'image : le test s'ouvrira
        // à la fin de la session.
        if self.session.is_some() {
            self.auto_after = Some(id.into());
            self.toast(tr("Le test se fera à la fin de la session.", "The test will run when the session ends.").to_string());
            return Vec::new();
        }
        let mut actions = Vec::new();
        if self.auto.as_ref().is_some_and(|a| a.phase == AutoPhase::Running) {
            actions.push(Action::AutoCancel);
        }
        self.auto_attempt += 1;
        let (from, from_focus) = match self.auto.take() {
            // « Refaire le test » dans le panneau : on revient toujours là d'où il s'était ouvert.
            Some(a) => (a.from, a.from_focus),
            None if self.screen == Screen::Settings => (Screen::Settings, self.focus),
            None => (Screen::Home, if self.focus == Target::AutoRetest { Target::Chip(Profile::Auto) } else { self.focus }),
        };
        self.auto = Some(AutoTestUi { host: id.into(), name, attempt: self.auto_attempt, steps: waiting_steps(), phase: AutoPhase::Running, from, from_focus });
        self.go(Screen::AutoTest);
        actions.push(Action::AutoTest { host: id.into(), attempt: self.auto_attempt });
        actions
    }

    /// L'avancement du test (`Progress` du cœur, mis en mots).
    pub fn auto_progress(&mut self, attempt: u32, steps: Vec<Step>) -> bool {
        let Some(a) = self.auto.as_mut().filter(|a| a.attempt == attempt && a.phase == AutoPhase::Running) else { return false };
        a.steps = steps;
        true
    }

    /// La fin du test : le résumé, ou l'échec. Un test arrêté (« Annuler ») ne revient pas ici.
    pub fn auto_finished(&mut self, attempt: u32, result: Result<(serde_json::Value, Vec<Choice>), String>) -> bool {
        let Some(a) = self.auto.as_mut().filter(|a| a.attempt == attempt && a.phase == AutoPhase::Running) else { return false };
        a.phase = match result {
            Ok((result, choices)) => AutoPhase::Done { result, choices },
            Err(text) => AutoPhase::Failed(text),
        };
        if self.screen == Screen::AutoTest {
            self.focus = self.auto_focus();
        }
        true
    }

    /// Ferme le panneau : retour là d'où il s'était ouvert.
    fn auto_close(&mut self) {
        let Some(a) = self.auto.take() else { return };
        self.screen = a.from;
        self.focus = a.from_focus;
    }

    /// Les boutons du panneau, de « Refaire le test » des réglages et du bandeau de la session.
    pub fn auto_activate(&mut self, t: Target) -> Vec<Action> {
        match t {
            Target::AutoApply => {
                let Some(a) = self.auto.clone() else { return Vec::new() };
                let AutoPhase::Done { result, .. } = a.phase else { return Vec::new() };
                let (panel, refresh) = (self.tv.panel, self.tv.refresh);
                if let Some(h) = self.store.host_mut(&a.host) {
                    h.settings.apply_new_auto(result, panel, refresh);
                }
                self.auto_close();
                vec![Action::Save]
            }
            Target::AutoCancel => {
                let running = self.auto.as_ref().is_some_and(|a| a.phase == AutoPhase::Running);
                self.auto_close();
                if running { vec![Action::AutoCancel] } else { Vec::new() }
            }
            Target::AutoAgain | Target::AutoRetry => {
                let Some(id) = self.auto.as_ref().map(|a| a.host.clone()) else { return Vec::new() };
                self.open_auto_test(&id)
            }
            // « Refaire le test » des réglages de l'ordinateur.
            Target::AutoRetest => {
                let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
                self.open_auto_test(&id)
            }
            // Le bandeau de la session : le test ne peut pas se faire pendant la session sur le téléviseur (son
            // décodeur la montre) ; son panneau s'ouvre à la fin de la session.
            Target::AutoBannerAgain => {
                if let Some(s) = self.session.as_mut() {
                    s.auto_banner = None;
                    self.auto_after = Some(s.host.clone());
                }
                Vec::new()
            }
            Target::AutoBannerClose => {
                if let Some(s) = self.session.as_mut() {
                    s.auto_banner = None;
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// À la première image d'une session en Auto : le bandeau « Autre réseau que celui du test » si le réseau (ou le
    /// relais) n'est pas celui du test. Une fois par session, jamais pour un visiteur ni une console.
    pub fn auto_banner_check(&mut self) {
        let current = self.net.name.clone();
        let Some(s) = self.session.as_mut() else { return };
        if s.auto_checked || !s.streamed || s.phase != Phase::Streaming {
            return;
        }
        s.auto_checked = true;
        if s.profile != Profile::Auto || s.visitor() || s.ps.is_some() {
            return;
        }
        let network = if s.via_relay() { Some("relay".to_string()) } else { current };
        let Some(brief) = self.store.host(&s.host).and_then(|h| h.settings.auto_brief()) else { return };
        if !brief.same_network(network.as_deref()) {
            s.auto_banner = Some(Instant::now());
        }
    }

    /// Le bandeau est-il montré (8 s au plus, ni menu ni panneau par-dessus) ?
    pub fn auto_banner_visible(&self) -> bool {
        self.session.as_ref().is_some_and(|s| {
            s.auto_banner.is_some_and(|t| t.elapsed().as_secs_f32() < BANNER_SECONDS) && !s.menu && !s.keys && !s.share_panel && !s.lend_panel && !s.watch_panel && s.phase == Phase::Streaming
        })
    }

    /// La session finie : le panneau du test demandé pendant elle s'ouvre (« Refaire le test » du bandeau, Auto choisi
    /// sans résultat dans le menu).
    pub fn auto_after_session(&mut self) -> Vec<Action> {
        if self.session.is_some() || self.screen != Screen::Home {
            return Vec::new();
        }
        let Some(id) = self.auto_after.take() else { return Vec::new() };
        if self.store.host(&id).is_none() {
            return Vec::new();
        }
        self.store.selected = Some(id.clone());
        self.open_auto_test(&id)
    }

    /// La session a dû redescendre sous l'encodeur de l'ordinateur (`Notice::EncoderCeiling` du cœur) : ce débit
    /// devient le plafond de cet ordinateur, pour les sessions suivantes, sans rien dire à l'écran. Jamais pour une
    /// session sur l'ordinateur de quelqu'un d'autre (invité, emprunt) ni une console. `true` : à enregistrer.
    pub fn encoder_ceiling(&mut self, mbps: u32) -> bool {
        let Some(s) = self.session.as_ref().filter(|s| !s.visitor() && s.ps.is_none()) else { return false };
        let host = s.host.clone();
        let Some(h) = self.store.host_mut(&host) else { return false };
        if mbps == 0 || h.settings.encoder_max == Some(mbps) {
            return false;
        }
        h.settings.encoder_max = Some(mbps);
        true
    }

    /// Les tuiles des profils du menu de session : Auto sans résultat gardé dit qu'il viendra à la prochaine session.
    pub fn auto_tile_sub(&self, host: &str) -> &'static str {
        if self.store.host(host).is_some_and(|h| h.settings.auto_brief().is_some()) {
            tr("ce réseau, cet appareil", "this network, this device")
        } else {
            tr("prochaine session", "next session")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{MenuItem, SessionUi, Setting};
    use crate::model::{Host, HostSettings, Profile, Store};

    fn host() -> Host {
        Host {
            id: "a".into(),
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
            settings: HostSettings::default(),
            last: None,
        }
    }

    /// Un résultat comme le cœur l'écrit (`AutoResult`, docs/profil-auto.md).
    fn result(network: &str, height: u32, refresh: u32, reliable: bool) -> serde_json::Value {
        serde_json::json!({
            "at": 1_795_000_000u64,
            "network": network,
            "measures": { "link": "cable", "latency": { "min": 1.0, "median": 1.4, "jitter": 0.3 }, "bandwidth": { "mbps": 93.0, "loaded_delta": 2.0 }, "decode": [{ "codec": "hevc", "yuv444": false, "fps": 240.0 }] },
            "codec": "hevc", "yuv444": false, "width": height * 16 / 9, "height": height, "refresh": refresh,
            "mbps": 45, "mode_mbps": 70, "reliable": reliable,
            "reasons": { "codec": "best_format", "mode": "matches_display", "yuv444": "yuv444_not_decoded", "bitrate": "measured", "transport": "steady" },
            "station": "Atelier"
        })
    }

    fn app() -> App {
        crate::i18n::force_english(false);
        let mut app = App::new(Store::default());
        app.store.hosts.push(host());
        app.store.selected = Some("a".into());
        app.tv.panel = (3840, 2160);
        app.tv.refresh = 120;
        app
    }

    fn done(app: &mut App, r: serde_json::Value) {
        let attempt = app.auto.as_ref().unwrap().attempt;
        let choice = Choice { title: "Format vidéo".into(), value: "HEVC".into(), reason: "le meilleur format que cet appareil décode".into() };
        assert!(app.auto_finished(attempt, Ok((r, vec![choice]))));
    }

    /// Auto sans résultat gardé : le panneau du test s'ouvre et le test part ; « Annuler » garde le profil d'avant.
    #[test]
    fn auto_sans_resultat_ouvre_le_test() {
        let mut app = app();
        app.store.host_mut("a").unwrap().settings.apply(Profile::Bureau);
        let acts = app.activate(Target::Chip(Profile::Auto));
        assert_eq!(acts, vec![Action::AutoTest { host: "a".into(), attempt: 1 }]);
        assert_eq!(app.screen, Screen::AutoTest);
        assert_eq!(app.focus, Target::AutoCancel);
        assert_eq!(app.auto.as_ref().unwrap().steps.len(), 5);
        assert_eq!(app.activate(Target::AutoCancel), vec![Action::AutoCancel]);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.store.host("a").unwrap().settings.profile, Profile::Bureau);
        assert!(app.store.host("a").unwrap().settings.auto.is_none());
    }

    /// L'avancement d'un test abandonné n'entre pas dans le panneau d'un nouveau.
    #[test]
    fn avancement_d_un_autre_test_ignore() {
        let mut app = app();
        app.activate(Target::Chip(Profile::Auto));
        let acts = app.activate(Target::AutoAgain);
        assert_eq!(acts, vec![Action::AutoCancel, Action::AutoTest { host: "a".into(), attempt: 2 }]);
        let step = Step { title: "Liaison".into(), state: StepState::Done("Câble".into()) };
        assert!(!app.auto_progress(1, vec![step.clone()]));
        assert!(app.auto_progress(2, vec![step.clone()]));
        assert_eq!(app.auto.as_ref().unwrap().steps, vec![step]);
        assert!(!app.auto_finished(1, Err("x".into())));
        // L'échec : « Réessayer » a le focus.
        assert!(app.auto_finished(2, Err("Le test n'a pas pu joindre Atelier.".into())));
        assert_eq!(app.focus, Target::AutoRetry);
        assert_eq!(app.activate(Target::AutoRetry), vec![Action::AutoTest { host: "a".into(), attempt: 3 }]);
    }

    /// « Appliquer » : Auto devient le profil, avec le transport du test, la définition et la fréquence ramenées aux
    /// réglages de l'app ; ensuite, choisir Auto l'applique sans test.
    #[test]
    fn appliquer_puis_choisir_auto_sans_test() {
        let mut app = app();
        app.activate(Target::Chip(Profile::Auto));
        done(&mut app, result("Ethernet", 1440, 60, true));
        assert_eq!(app.focus, Target::AutoApply);
        assert_eq!(app.activate(Target::AutoApply), vec![Action::Save]);
        assert_eq!(app.screen, Screen::Home);
        let st = &app.store.host("a").unwrap().settings;
        assert_eq!(st.profile, Profile::Auto);
        assert!(!st.is_custom() && st.reliable_transport() && st.adaptive);
        assert_eq!((st.resolution, st.fps), (1440, 60));
        assert_eq!(st.auto_brief().unwrap().network.as_deref(), Some("Ethernet"));
        // L'écran entier à sa fréquence : « Comme cet écran », la fréquence de la dalle.
        app.activate(Target::AutoRetest);
        done(&mut app, result("Ethernet", 2160, 120, false));
        app.activate(Target::AutoApply);
        let st = &app.store.host("a").unwrap().settings;
        assert_eq!((st.resolution, st.fps), (0, 0));
        assert!(!st.reliable_transport());
        // Un autre profil, puis Auto de nouveau : ses réglages tout de suite.
        app.activate(Target::Chip(Profile::Jeu));
        assert_eq!(app.activate(Target::Chip(Profile::Auto)), vec![Action::Save]);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.store.host("a").unwrap().settings.profile, Profile::Auto);
    }

    /// Le plafond de l'encodeur : « Appliquer » un nouveau résultat le recalcule (`next_encoder_max`), remettre Auto
    /// avec le résultat gardé n'y touche pas ; une session le garde pour son ordinateur ; il est dans le fichier.
    #[test]
    fn plafond_de_l_encodeur() {
        let with_encoder = |mbps: u32, limited: bool| {
            let mut r = result("Ethernet", 2160, 120, false);
            r["measures"]["encoder"] = serde_json::json!({ "codec": "hevc", "yuv444": false, "width": 3840, "height": 2160, "refresh": 120, "mbps": mbps, "limited": limited });
            r
        };
        let mut app = app();
        app.activate(Target::Chip(Profile::Auto));
        done(&mut app, with_encoder(440, true));
        app.activate(Target::AutoApply);
        assert_eq!(app.store.host("a").unwrap().settings.encoder_max, Some(440));
        // Une session redescend : 390 gardé.
        let mut s = SessionUi::new("a", "Atelier", Profile::Auto, false);
        s.phase = Phase::Streaming;
        app.session = Some(s);
        assert!(app.encoder_ceiling(390));
        assert!(!app.encoder_ceiling(390), "rien de neuf");
        app.session = None;
        // Auto remis avec le résultat gardé : le plafond de la session reste.
        app.activate(Target::Chip(Profile::Jeu));
        app.activate(Target::Chip(Profile::Auto));
        assert_eq!(app.store.host("a").unwrap().settings.encoder_max, Some(390));
        // Un nouveau test accepte 290 du premier coup : 390 reste (au-dessus) ; un plafond plus bas serait retiré.
        app.activate(Target::AutoRetest);
        done(&mut app, with_encoder(290, false));
        app.activate(Target::AutoApply);
        assert_eq!(app.store.host("a").unwrap().settings.encoder_max, Some(390));
        app.store.host_mut("a").unwrap().settings.encoder_max = Some(200);
        app.activate(Target::AutoRetest);
        done(&mut app, with_encoder(290, false));
        app.activate(Target::AutoApply);
        assert_eq!(app.store.host("a").unwrap().settings.encoder_max, None);
        // Sans mesure (Station d'avant) : celui d'avant.
        app.store.host_mut("a").unwrap().settings.encoder_max = Some(200);
        app.activate(Target::AutoRetest);
        done(&mut app, result("Ethernet", 2160, 120, false));
        app.activate(Target::AutoApply);
        assert_eq!(app.store.host("a").unwrap().settings.encoder_max, Some(200));
        // Gardé dans le fichier, hors profil ; absent d'un fichier d'avant.
        let st = app.store.host("a").unwrap().settings.clone();
        assert!(!st.is_custom());
        let back: HostSettings = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
        assert_eq!(back.encoder_max, Some(200));
        let old: HostSettings = serde_json::from_str(r#"{"profile":"max"}"#).unwrap();
        assert_eq!(old.encoder_max, None);
        // Sans session, rien.
        assert!(!app.encoder_ceiling(100));
    }

    /// Le débit adaptatif se coupe sans faire passer en « Personnalisé » ; il est gardé dans le fichier.
    #[test]
    fn debit_adaptatif_hors_profil() {
        let mut app = app();
        app.adjust_for_preview(Setting::Adaptive);
        let st = &app.store.host("a").unwrap().settings;
        assert!(!st.adaptive && !st.is_custom());
        let json = serde_json::to_string(st).unwrap();
        let back: HostSettings = serde_json::from_str(&json).unwrap();
        assert!(!back.adaptive);
        // Un fichier d'avant : allumé.
        let old: HostSettings = serde_json::from_str(r#"{"profile":"max"}"#).unwrap();
        assert!(old.adaptive && old.auto.is_none());
    }

    /// Le bandeau « Autre réseau que celui du test » : Auto, réseau autre que celui du test, une fois par session ;
    /// « Refaire le test » ouvre le panneau à la fin de la session.
    #[test]
    fn bandeau_autre_reseau() {
        let mut app = app();
        app.activate(Target::Chip(Profile::Auto));
        done(&mut app, result("Maison", 2160, 120, false));
        app.activate(Target::AutoApply);
        let start = |app: &mut App| {
            let mut s = SessionUi::new("a", "Atelier", Profile::Auto, false);
            s.phase = Phase::Streaming;
            s.streamed = true;
            app.session = Some(s);
            app.screen = Screen::Session;
            app.auto_banner_check();
        };
        app.net.name = Some("Maison".into());
        start(&mut app);
        assert!(!app.auto_banner_visible(), "même réseau");
        app.net.name = Some("Ethernet".into());
        start(&mut app);
        assert!(app.auto_banner_visible());
        app.activate(Target::AutoBannerClose);
        assert!(!app.auto_banner_visible());
        app.auto_banner_check();
        assert!(!app.auto_banner_visible(), "une fois par session");
        // « Refaire le test » : après la session.
        start(&mut app);
        assert!(app.activate(Target::AutoBannerAgain).is_empty());
        assert_eq!(app.auto_after.as_deref(), Some("a"));
        assert!(app.auto_after_session().is_empty(), "pas pendant la session");
        app.session = None;
        app.go(Screen::Home);
        assert_eq!(app.auto_after_session(), vec![Action::AutoTest { host: "a".into(), attempt: 2 }]);
        assert_eq!(app.screen, Screen::AutoTest);
        // Un autre profil : pas de bandeau.
        app.auto = None;
        let mut s = SessionUi::new("a", "Atelier", Profile::Jeu, false);
        s.phase = Phase::Streaming;
        s.streamed = true;
        app.session = Some(s);
        app.auto_banner_check();
        assert!(!app.auto_banner_visible());
    }

    /// Dans le menu de session, Auto sans résultat : le test à la fin de la session (le décodeur montre l'image).
    #[test]
    fn auto_dans_le_menu_sans_resultat() {
        let mut app = app();
        let mut s = SessionUi::new("a", "Atelier", Profile::Jeu, false);
        s.phase = Phase::Streaming;
        s.menu = true;
        app.session = Some(s);
        app.screen = Screen::Session;
        assert!(app.activate(Target::Menu(MenuItem::Profile(Profile::Auto))).is_empty());
        assert_eq!(app.auto_after.as_deref(), Some("a"));
        assert_eq!(app.session.as_ref().unwrap().profile, Profile::Jeu);
        assert_eq!(app.auto_tile_sub("a"), "prochaine session");
        // Les réglages ouverts depuis la session : pareil, avec l'avis.
        app.auto_after = None;
        assert!(app.choose_auto("a").is_empty());
        assert_eq!(app.auto_after.as_deref(), Some("a"));
        assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Le test se fera à la fin de la session."));
    }

    /// Une mesure de NDL bornée par l'affichage (au moins la fréquence de la dalle) n'est pas rendue au cœur.
    #[test]
    fn mesure_bornee_par_la_dalle() {
        assert_eq!(decode_rate(Some(120.0), 120), None);
        assert_eq!(decode_rate(Some(131.5), 120), None);
        assert_eq!(decode_rate(Some(60.0), 60), None);
        assert_eq!(decode_rate(Some(118.0), 120), None, "5 % de tolérance");
        assert_eq!(decode_rate(Some(113.0), 120), Some(113.0));
        assert_eq!(decode_rate(Some(60.0), 120), Some(60.0), "AV1 montré à 60 sur une dalle à 120 Hz");
        assert_eq!(decode_rate(None, 120), None, "format refusé");
    }
}
