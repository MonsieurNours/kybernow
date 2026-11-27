//! Profil Auto (docs/profil-auto.md ; textes et écrans : docs/link-ui.md,
//! « Profil Auto et débit adaptatif ») côté Windows.
//!
//! - **Le test** ([`autotest_run`]) : `autotest::run` du cœur, bloquant
//!   (une dizaine de secondes, sur le fil de la commande), avec la console de
//!   l'ordinateur que l'app utilise déjà (par le relais quand il n'est
//!   joignable que par lui), l'écran « Cet écran », ce que la carte décode, le
//!   choix « HDR », le réseau du moment (nom du Wi-Fi, « Ethernet ») et le
//!   câble ; l'étape « Décodage de cet appareil » passe les extraits au
//!   décodeur des sessions, dans un processus à part
//!   (`autotest_decode::ChildDecoder`). Chaque `Progress` part à l'accueil
//!   (`autotest-progress`) ; « Annuler » ([`autotest_cancel`]) arrête le test
//!   entre deux mesures. La réponse : le résultat et ses choix
//!   (`AutoResult::choices`), ou le texte de l'échec.
//! - **Appliquer** ([`autotest_apply`], [`apply`]) : le résultat est gardé
//!   pour l'ordinateur (`Computer::auto`, `computers.json`), le profil devient
//!   Auto (retouches effacées), la définition et la fréquence du test vont aux
//!   réglages de définition (« Comme cet écran » si ce sont celles de cet
//!   écran, sinon « Sur mesure ») ; pendant sa session, appliqué tout de suite.
//! - **Choisir Auto** ([`choose`]) : avec un test gardé, ses réglages tout de
//!   suite ; sans test, le panneau du test (à la fin de la session si
//!   l'ordinateur est en session : [`later`], repris par `show_home_if_idle`,
//!   et dit dans un avis : « Le test se fera à la fin de la session. »).
//! - **En session** ([`Check`]) : Auto, et un réseau (ou le relais) différent
//!   de celui du test : le bandeau « Autre réseau que celui du test ».

use crate::sync::Locked;
use std::sync::atomic::AtomicBool;

use kybernow_core::autotest::{self as core_auto, AutoError, AutoResult, Choice, Device, Progress, Step, StepProgress, StepState};
use kybernow_core::console::Mode;

use super::*;
use crate::autotest_decode::ChildDecoder;

/// Les tests en cours (un par ordinateur) et leur « Annuler ».
static RUNNING: Mutex<Vec<(String, Arc<AtomicBool>)>> = Mutex::new(Vec::new());
/// Le panneau du test à ouvrir à la fin de la session de cet ordinateur
/// (« Refaire le test » du bandeau, Auto choisi dans la barre sans test).
static LATER: Mutex<Option<String>> = Mutex::new(None);

/// Ce que fait le choix d'Auto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Chosen {
    /// Le test gardé s'applique.
    Applied,
    /// Pas de test gardé : le panneau du test s'ouvre.
    Test,
    /// Pas de test gardé, l'ordinateur est en session : le panneau s'ouvrira à sa fin.
    Later,
}

/// Auto choisi (accueil, réglages, barre, menu de l'icône).
pub fn choose(app: &AppHandle, id: &str) -> Result<Chosen, String> {
    let state = app.state::<UiState>();
    let kept = state.with_store(|s| s.get(id).map(|c| c.auto.is_some())).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    if kept {
        apply(app, id, None)?;
        Ok(Chosen::Applied)
    } else if state.in_session(id) {
        later(id);
        Ok(Chosen::Later)
    } else {
        Ok(Chosen::Test)
    }
}

/// Le panneau du test s'ouvrira à la fin de la session de `id`.
pub fn later(id: &str) {
    log::info!("profil Auto : test à la fin de la session");
    *LATER.locked() = Some(id.to_string());
}

/// [`later`], dit dans un avis (capsule de la barre, sinon notification de
/// Windows) : « Le test se fera à la fin de la session. ». Pour l'accueil et le
/// menu de l'icône ; la barre le dit elle-même (`Bar.svelte`).
pub fn later_told(app: &AppHandle, id: &str) {
    later(id);
    let name = app.state::<UiState>().with_session(id, |r| r.name.clone()).unwrap_or_default();
    super::share::notice(app, id, &name, &t("session.autotest_later", &[]));
}

/// La session est finie, l'accueil revient : le panneau du test attendu.
pub fn take_later() -> Option<String> {
    LATER.locked().take()
}

/// Le profil Auto pour `id` avec `result` (sinon le test gardé) : gardé,
/// retouches effacées, définition et fréquence du test posées ; appliqué à sa
/// session s'il en a une.
pub fn apply(app: &AppHandle, id: &str, result: Option<AutoResult>) -> Result<Computer, String> {
    let state = app.state::<UiState>();
    let screen = system::screen_mode();
    let (c, before) = state.with_store(|s| {
        let c = s.get_mut(id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        let r = result.or_else(|| c.auto.clone()).ok_or_else(|| t("errors.unknown_profile", &[]))?;
        let before = c.settings.clone();
        let mode = Mode { width: r.width, height: r.height, refresh: r.refresh };
        c.settings.profile = "auto".into();
        c.settings.codec = None;
        c.settings.yuv444 = None;
        c.settings.mbps = None;
        c.settings.reliable = None;
        if mode == screen {
            c.settings.custom = None;
            c.settings.max_4k = false;
        } else {
            c.settings.custom = Some(mode);
        }
        log::info!(
            "profil Auto appliqué : {} {}{}, {}×{} à {} Hz, {} Mbit/s, {}",
            c.name,
            r.codec.label(),
            if r.yuv444 { " 4:4:4" } else { "" },
            r.width,
            r.height,
            r.refresh,
            r.mbps,
            if r.reliable { "fiable" } else { "rapide" }
        );
        // Plafond de l'encodeur : recalculé seulement pour un nouveau
        // résultat (« Appliquer » à la fin d'un test). Le résultat déjà gardé
        // remis (choisir Auto, profil synchronisé) n'y touche pas : un plafond
        // trouvé par une session après le test est plus récent.
        if is_new_result(c.auto.as_ref(), &r) {
            let next = r.next_encoder_max(c.encoder_max);
            if next != c.encoder_max {
                log::info!("plafond de l'encodeur de {} : {:?} → {:?} Mbit/s par écran", c.name, c.encoder_max, next);
            }
            c.encoder_max = next;
        }
        c.auto = Some(r);
        let c = c.clone();
        s.save();
        Ok::<_, String>((c, before))
    })?;
    // Réglages synchronisés : `{profile: "auto"}` part au relais du compte.
    synced::changed(app, id, &before);
    emit_computers(app);
    apply_to_session(app, id, &c)?;
    Ok(c)
}

/// `result` est-il un autre test que celui gardé (`kept`) ? Reconnu à sa
/// date et à sa Station : le résultat gardé repasse par l'interface (JSON),
/// ses mesures décimales peuvent y bouger au dernier chiffre.
fn is_new_result(kept: Option<&AutoResult>, result: &AutoResult) -> bool {
    kept.is_none_or(|k| k.at != result.at || k.station != result.station)
}

/// Le réseau du test et celui de la session (`Check::differs`).
pub struct Check {
    result: AutoResult,
    /// Le réseau de ce PC au lancement (nom du Wi-Fi, « Ethernet »).
    network: Option<String>,
}

impl Check {
    /// Autre réseau que celui du test (`relayed` : la session passe par le relais).
    pub fn differs(&self, relayed: bool) -> bool {
        let now = if relayed { Some("relay") } else { self.network.as_deref() };
        !self.result.same_network(now)
    }
}

/// Au lancement d'une session : le profil Auto (sans retouche) et son test.
pub fn check(c: &Computer, host: &str) -> Option<Check> {
    let s = &c.settings;
    let retouched = s.codec.is_some() || s.yuv444.is_some() || s.mbps.is_some() || s.reliable.is_some();
    if s.profile != "auto" || retouched {
        return None;
    }
    let result = c.auto.clone()?;
    let network = (!host.is_empty()).then(|| system::network_name(host)).flatten();
    Some(Check { result, network })
}

/// `autotest-progress` : l'avancement du test de l'ordinateur `id`.
#[derive(Clone, Serialize)]
struct ProgressEvent<'a> {
    id: &'a str,
    progress: &'a Progress,
}

/// La fin du test : le résultat et le résumé (`choices`), « Annuler »
/// (`cancelled`), ou l'échec (`error`, « Le test n'a pas pu joindre {nom}. »).
#[derive(Serialize)]
pub struct Outcome {
    result: Option<AutoResult>,
    choices: Vec<Choice>,
    error: Option<String>,
    cancelled: bool,
}

/// Le panneau du test : le test de l'ordinateur `id`, jusqu'au bout.
#[tauri::command(async)]
pub fn autotest_run(app: AppHandle, id: String) -> Result<Outcome, String> {
    let state = app.state::<UiState>();
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    // Un ordinateur d'un compte : son adresse sur le réseau, d'après la découverte.
    let located = account::locate(&app, &c);
    let host = located.as_ref().unwrap_or(&c).host.clone();
    let console = Arc::new(console_of(&app, &id)?);
    let pc = {
        let cached = state.this_pc.locked().clone();
        match cached {
            Some(pc) => pc,
            None => {
                let pc = pc::this_pc();
                *state.this_pc.locked() = Some(pc.clone());
                pc
            }
        }
    };
    let device = Device {
        screen: system::screen_mode(),
        caps: capabilities(&pc),
        // Le HDR ne sert que sur un écran HDR, avec une carte qui le décode.
        hdr: c.settings.hdr_on() && pc.hdr && home_screen_hdr().0,
        network: (!host.is_empty()).then(|| system::network_name(&host)).flatten(),
        wired: !host.is_empty() && system::wired_to(&host),
    };
    log::info!(
        "profil Auto : test de {} ({}×{} à {} Hz, {:?}, réseau {:?}, {})",
        c.name,
        device.screen.width,
        device.screen.height,
        device.screen.refresh,
        device.caps,
        device.network,
        if device.wired { "câble" } else { "sans câble" }
    );
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = RUNNING.locked();
        for (other, flag) in running.iter().filter(|(other, _)| *other == id) {
            log::info!("profil Auto : test précédent de {other} arrêté");
            flag.store(true, Ordering::Relaxed);
        }
        running.retain(|(other, _)| *other != id);
        running.push((id.clone(), cancel.clone()));
    }
    let lang = i18n::lang();
    let mut decoder = ChildDecoder::default();
    let (a, pid) = (app.clone(), id.clone());
    let mut progress = move |p: &Progress| {
        let _ = a.emit_to("main", "autotest-progress", ProgressEvent { id: &pid, progress: p });
    };
    // Les étapes tout de suite, la liaison en cours (le cœur donne la
    // suite après la première réponse de l'ordinateur).
    progress(&Progress {
        steps: Step::ALL
            .iter()
            .map(|&step| StepProgress {
                step,
                title: step.title(lang),
                state: if step == Step::Link { StepState::Running } else { StepState::Waiting },
            })
            .collect(),
    });
    let started = Instant::now();
    let out = core_auto::run(console, &device, &mut decoder, &mut progress, &cancel, lang);
    drop(decoder);
    RUNNING.locked().retain(|(_, flag)| !Arc::ptr_eq(flag, &cancel));
    let secs = started.elapsed().as_secs_f64();
    Ok(match out {
        Ok(r) => {
            log::info!("profil Auto : test fini en {secs:.1} s : {:?}", r.measures);
            Outcome { choices: r.choices(lang), result: Some(r), error: None, cancelled: false }
        }
        Err(AutoError::Cancelled) => {
            log::info!("profil Auto : test arrêté après {secs:.1} s");
            Outcome { result: None, choices: Vec::new(), error: None, cancelled: true }
        }
        Err(e) => {
            log::warn!("profil Auto : test en échec après {secs:.1} s : {e:?}");
            Outcome { result: None, choices: Vec::new(), error: Some(e.text(lang, &c.name)), cancelled: false }
        }
    })
}

/// « Annuler » du panneau : le test s'arrête (entre deux mesures), le profil
/// d'avant reste.
#[tauri::command(async)]
pub fn autotest_cancel(id: String) {
    for (other, flag) in RUNNING.locked().iter() {
        if *other == id {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

/// « Appliquer » : le profil Auto avec ce résultat ; rend les réglages de
/// l'ordinateur tels qu'ils sont enregistrés (l'écran des réglages les reprend).
#[tauri::command(async)]
pub fn autotest_apply(app: AppHandle, id: String, result: AutoResult) -> Result<Settings, String> {
    apply(&app, &id, Some(result)).map(|c| c.settings)
}

/// « Refaire le test » pendant la session : le panneau à la fin de la session.
/// `told` : l'avis « Le test se fera à la fin de la session. » est à montrer
/// (depuis l'accueil) ; la barre, elle, le montre dans sa capsule.
#[tauri::command(async)]
pub fn autotest_later(app: AppHandle, id: String, told: Option<bool>) {
    if told == Some(true) {
        later_told(&app, &id);
    } else {
        later(&id);
    }
}
