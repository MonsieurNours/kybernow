//! Accueil au premier lancement et incitation au compte KyberNow (docs/link-ui.md,
//! « Accueil et compte KyberNow ») : l'état gardé dans `app.json`
//! (`Prefs::welcome`, le `WelcomeState` du cœur), lu et changé par l'interface.
//!
//! - **Accueil en trois écrans** : `welcome_state` dit s'il faut le montrer
//!   (premier lancement, ou une fois après la mise à jour qui l'apporte, sans
//!   compte) ; « Passer », « Plus tard », « Continuer sans compte » jusqu'au bout
//!   ou un compte connecté : `welcome_done`. « Revoir l'accueil » (réglages de
//!   l'app) le rouvre sans rien changer ici.
//! - **« Créer un compte »** : `welcome_sign_up` ouvre la page d'inscription du
//!   relais dans le navigateur par défaut (l'app ne crée pas de compte).
//! - **Carte du compte** : `welcome_close_card` (croix « Fermer », puis « Ne plus
//!   afficher » quand elle est revenue après 30 jours).
//! - **Bandeau après un appairage** (`welcome_paired_hint`) et **« Hors de chez
//!   vous ? »** (`welcome_away_hint`) : une fois par ordinateur, une fois par jour
//!   au plus ; la règle est celle du cœur.
//! - **« PAS ENCORE INSTALLÉ »** (écran 3) : `welcome_station_download`.

use kybernow_core::welcome::{self as rules, WelcomeState};

use super::*;
use crate::welcome::{self as view, WelcomeView};

/// Un compte KyberNow est connecté sur ce PC.
fn has_account() -> bool {
    !account::list().is_empty()
}

/// `f` sur l'état gardé, puis `app.json` enregistré.
fn with_state<T>(app: &AppHandle, f: impl FnOnce(&mut WelcomeState) -> T) -> T {
    let state = app.state::<UiState>();
    update_prefs(&state, |p| f(&mut p.welcome))
}

fn current(app: &AppHandle) -> WelcomeView {
    let state = app.state::<UiState>().prefs.lock().unwrap().welcome.clone();
    view::view(&state, has_account(), view::now())
}

/// Ce que l'accueil lit : l'accueil en trois écrans à montrer, la carte du compte.
#[tauri::command(async)]
pub fn welcome_state(app: AppHandle) -> WelcomeView {
    current(&app)
}

/// L'accueil a été vu (fini, passé ou « Plus tard ») : il ne revient plus seul.
#[tauri::command(async)]
pub fn welcome_done(app: AppHandle) -> WelcomeView {
    with_state(&app, WelcomeState::welcome_done);
    log::info!("accueil : vu");
    current(&app)
}

/// La croix de la carte du compte : fermée (elle revient dans 30 jours), ou pour de bon.
#[tauri::command(async)]
pub fn welcome_close_card(app: AppHandle) -> WelcomeView {
    let now = view::now();
    with_state(&app, |s| s.close_card(now));
    current(&app)
}

/// Le bandeau après l'appairage de cet ordinateur (sans compte) : vrai une seule fois. `station_banner` : le
/// bandeau de la mise à jour de sa Station est là ; il passe d'abord, et celui-ci n'est pas compté comme montré.
#[tauri::command(async)]
pub fn welcome_paired_hint(app: AppHandle, id: String, station_banner: bool) -> bool {
    let has = has_account();
    with_state(&app, |s| view::take_paired_hint(s, has, &id, station_banner))
}

/// La fenêtre « Hors de chez vous ? » pour cet ordinateur (sans compte) : une fois par jour au plus.
#[tauri::command(async)]
pub fn welcome_away_hint(app: AppHandle, id: String) -> bool {
    let has = has_account();
    let now = view::now();
    with_state(&app, |s| s.take_away_hint(has, &id, now))
}

/// « Créer un compte » : la page d'inscription du relais public, dans le navigateur.
#[tauri::command(async)]
pub fn welcome_sign_up(app: AppHandle) -> Result<(), String> {
    let url = rules::sign_up_url(&current(&app).relay, i18n::lang());
    shell::open_url(&url)?;
    log::info!("accueil : inscription ouverte dans le navigateur");
    Ok(())
}

/// « PAS ENCORE INSTALLÉ » : la page de téléchargement de KyberNow Station.
#[tauri::command(async)]
pub fn welcome_station_download() -> Result<(), String> {
    shell::open_url(rules::STATION_DOWNLOAD_URL)
}
