//! Comptes KyberNow côté Windows (docs/comptes.md, lot 3 ; textes de
//! docs/link-ui.md « Comptes KyberNow »).
//!
//! - **Connexion** (« Se connecter à un compte KyberNow », « Ajouter un
//!   compte ») : `account_sign_in` ouvre la page de connexion du relais dans
//!   le navigateur par défaut (le mot de passe, la passkey et la double
//!   authentification s'y font, jamais dans l'app) et garde la demande en
//!   mémoire ; le relais revient à `kybernow://compte?code=…&etat=…`, lien
//!   inscrit par l'app elle-même (`shell::register`) et passé à l'app ouverte
//!   (`instance.rs`) ; [`callback`] échange le code contre la session
//!   d'appareil (`account::finish_sign_in`, sur un fil) et le dit par
//!   l'événement `account-sign-in`.
//! - **Comptes gardés** : chacun entier dans le gestionnaire d'identification
//!   (`store::save_account`, une cible par compte), lus une fois au lancement ;
//!   plusieurs comptes, sur plusieurs relais.
//! - **Ordinateurs des comptes** : relevés au lancement, au retour de la
//!   fenêtre (30 s au plus souvent) et après une connexion ([`sync`]) ; la
//!   liste entière est gardée dans `accounts.json` (empreintes d'Admin
//!   retenues), et ceux qui ne sont pas déjà appairés sur ce PC rejoignent
//!   `computers.json` avec `account` (un ordinateur une seule fois,
//!   `account::merge`) : mêmes cartes, mêmes réglages, même menu de l'icône.
//!   Admin y est appelé avec le jeton d'accès du relais (`Console::account`,
//!   un `StationAccess` gardé par ordinateur : un jeton toutes les 4 minutes,
//!   pas à chaque relevé), sur le réseau quand la découverte le trouve (même
//!   empreinte d'Admin), par le relais sinon.
//! - **Refus** : session d'appareil fermée (`AccountError::SignedOut`), le
//!   compte est oublié et l'accueil le dit (`account-signed-out`) ; ordinateur
//!   retiré du compte (`UnknownComputer`), il quitte l'accueil.
//! - **Se déconnecter** : `account::sign_out`, puis le compte est oublié même
//!   sans réponse du relais ; ses ordinateurs quittent l'accueil, les appairés
//!   restent.
//!
//! Les appels au relais sont bloquants (8 s au plus chacun) : sur les fils de
//! Tauri (`#[tauri::command(async)]`) ou sur un fil à part, jamais sur celui de
//! la fenêtre. Ce qui se teste sans Windows est dans `accounts.rs`.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

use kybernow_core::account::{self, AccessParams, Account, AccountComputer, AccountError, Paired, SignIn, StationAccess};
use kybernow_core::console::ConsoleError;
use kybernow_core::relay::{RelayError, RelayInfo};
use kybernow_core::session::Route;

use super::*;
use crate::accounts::{self as rules, AccountView, SignInOutcome};
use crate::discovery::Found;

/// Les comptes connectés, lus une fois dans le gestionnaire d'identification
/// (`None` : pas encore lus).
static ACCOUNTS: Mutex<Option<Vec<Account>>> = Mutex::new(None);
/// Les jetons d'accès, par compte et par Station (`<compte>#<station>`) : un
/// jeton sert 4 minutes à tous les appels de l'accueil vers cet ordinateur.
static ACCESS: Mutex<Option<HashMap<String, Arc<StationAccess>>>> = Mutex::new(None);
/// La connexion en cours par la fenêtre du système (en mémoire : une app
/// relancée entre-temps écarte le retour, il suffit de recommencer).
static SIGNING: Mutex<Option<SignIn>> = Mutex::new(None);
/// Un relevé à la fois ; le dernier, pour ne pas relever à chaque retour de la fenêtre.
static SYNCING: AtomicBool = AtomicBool::new(false);
static LAST_SYNC: Mutex<Option<Instant>> = Mutex::new(None);
/// Au retour de la fenêtre, un relevé au plus toutes les 30 s.
const SYNC_EVERY: Duration = Duration::from_secs(30);

// MARK: - Comptes gardés

/// Les comptes connectés sur ce PC (le gestionnaire d'identification, lu une fois).
pub fn list() -> Vec<Account> {
    ACCOUNTS.lock().unwrap().get_or_insert_with(store::accounts).clone()
}

fn account_of(key: &str) -> Option<Account> {
    list().into_iter().find(|a| a.key() == key)
}

/// Ce que l'interface montre des comptes (réglages de l'app, « Comptes »).
fn views() -> Vec<AccountView> {
    list().iter().map(AccountView::of).collect()
}

fn emit_accounts(app: &AppHandle) {
    let _ = app.emit_to("main", "accounts", views());
}

/// Le jeton d'accès de ce compte pour cette Station, gardé d'un appel à l'autre.
fn access(account: &Account, station: &str) -> Arc<StationAccess> {
    let key = format!("{}#{station}", account.key());
    let mut cache = ACCESS.lock().unwrap();
    let cache = cache.get_or_insert_with(HashMap::new);
    // Un compte reconnecté a un autre jeton d'appareil : un `StationAccess` neuf.
    if let Some(a) = cache.get(&key).filter(|a| a.account().token == account.token) {
        return a.clone();
    }
    let a = StationAccess::new(AccessParams { account: account.clone(), station: station.to_string() });
    cache.insert(key, a.clone());
    a
}

/// Oublie le compte : gestionnaire d'identification, mémoire, jetons d'accès,
/// et ses ordinateurs à l'accueil (les appairés restent). `told` : la session
/// de l'appareil a été fermée ailleurs, l'accueil le dit.
fn forget(app: &AppHandle, key: &str, told: bool) {
    store::delete_account(key);
    super::synced::forget_account(key);
    if let Some(list) = ACCOUNTS.lock().unwrap().as_mut() {
        list.retain(|a| a.key() != key);
    }
    if let Some(cache) = ACCESS.lock().unwrap().as_mut() {
        cache.retain(|k, _| !k.starts_with(&format!("{key}#")));
    }
    log::info!("comptes : {key} oublié{}", if told { " (session de l'appareil fermée)" } else { "" });
    let rest = app.state::<UiState>().with_store(|s| s.account_computers.iter().filter(|c| c.account != key).cloned().collect());
    apply(app, rest);
    if told {
        let _ = app.emit_to("main", "account-signed-out", AccountError::SignedOut.text(i18n::lang()));
    }
}

// MARK: - Connexion

/// « Se connecter à un compte KyberNow » (accueil, relais public) et « Ajouter
/// un compte » (réglages de l'app, le relais choisi, ou « Un autre relais… »
/// tapé) : la page de connexion du relais s'ouvre dans le navigateur, l'app
/// attend son retour (`kybernow://compte?…`, [`callback`]). Refus : « Adresse
/// du relais illisible. », ou l'ouverture du navigateur.
#[tauri::command(async)]
pub fn account_sign_in(relay: Option<String>) -> Result<(), String> {
    let lang = i18n::lang();
    let relay = account::normalize_relay(relay.as_deref().unwrap_or_default()).map_err(|e| e.text(lang))?;
    let s = account::sign_in(&relay, &system::device_info().name).map_err(|e| e.text(lang))?;
    shell::open_url(&s.url)?;
    log::info!("comptes : connexion ouverte dans le navigateur ({})", account::relay_label(&s.relay));
    *SIGNING.lock().unwrap() = Some(s);
    Ok(())
}

/// « Annuler » pendant « Connexion au compte… » (la page fermée sans revenir) :
/// un retour arrivé ensuite est écarté.
#[tauri::command(async)]
pub fn account_sign_in_cancel() {
    *SIGNING.lock().unwrap() = None;
}

/// Le relais revient à `kybernow://compte?code=…&etat=…` (ou `?erreur=annule`) :
/// le code est échangé contre la session d'appareil, sur un fil (8 s au plus).
/// Connecté : le compte est gardé, ses ordinateurs relevés, et l'accueil dit
/// « Connecté à {compte}. » ; « Annuler » sur la page : rien.
pub fn callback(app: &AppHandle, url: &str) {
    let Some(sign_in) = SIGNING.lock().unwrap().take() else {
        log::warn!("comptes : retour de la page de connexion sans connexion en cours, écarté");
        return;
    };
    let (app, url) = (app.clone(), url.to_string());
    std::thread::spawn(move || {
        let lang = i18n::lang();
        let r = account::finish_sign_in(&sign_in, &url);
        let outcome = match &r {
            Ok(a) if store::save_account(a) => {
                keep(&app, a.clone());
                SignInOutcome::of(&r, lang)
            }
            // Pas gardé : la session d'appareil ne servirait qu'à ce lancement.
            Ok(_) => SignInOutcome::Failed { message: t("errors.password_store", &[]) },
            Err(AccountError::Cancelled) => {
                log::info!("comptes : connexion annulée sur la page du relais");
                SignInOutcome::Cancelled
            }
            Err(e) => {
                log::info!("comptes : connexion refusée ({})", e.kind());
                // Le retour d'une demande plus ancienne (autre `etat`) : celle en cours attend toujours le sien.
                if *e == AccountError::BadCode {
                    SIGNING.lock().unwrap().get_or_insert(sign_in);
                }
                SignInOutcome::of(&r, lang)
            }
        };
        let signed_in = matches!(outcome, SignInOutcome::SignedIn { .. });
        let _ = app.emit_to("main", "account-sign-in", outcome);
        if signed_in {
            sync(&app);
        }
    });
}

/// Un compte connecté : il remplace le même compte déjà là (son ancienne
/// session d'appareil est fermée chez le relais, sans attendre).
fn keep(app: &AppHandle, a: Account) {
    let old = {
        let mut guard = ACCOUNTS.lock().unwrap();
        let list = guard.get_or_insert_with(store::accounts);
        let old = list.iter().position(|o| o.key() == a.key()).map(|i| list.remove(i));
        list.push(a.clone());
        list.sort_by_key(|x| x.key());
        old
    };
    if let Some(old) = old.filter(|o| o.token != a.token) {
        std::thread::spawn(move || {
            let _ = account::sign_out(&old);
        });
    }
    emit_accounts(app);
}

// MARK: - Réglages de l'app

/// Les comptes connectés (réglages de l'app, « Comptes »).
#[tauri::command(async)]
pub fn account_list() -> Vec<AccountView> {
    views()
}

/// « Gérer le compte » : la page du compte sur son relais, dans le navigateur.
#[tauri::command(async)]
pub fn account_manage(key: String) -> Result<(), String> {
    let a = account_of(&key).ok_or_else(|| AccountError::SignedOut.text(i18n::lang()))?;
    shell::open_url(&a.summary().console)
}

/// « Se déconnecter » (confirmé dans l'interface) : la session de l'appareil
/// est fermée chez le relais, puis le compte est oublié, même si le relais ne
/// répond pas. Ses ordinateurs quittent l'accueil ; les appairés restent.
#[tauri::command(async)]
pub fn account_sign_out(app: AppHandle, key: String) {
    let Some(a) = account_of(&key) else { return };
    if let Err(e) = account::sign_out(&a) {
        log::warn!("comptes : déconnexion de {key} sans réponse du relais ({}), oublié quand même", e.kind());
    }
    forget(&app, &key, false);
    emit_accounts(&app);
}

// MARK: - Ordinateurs des comptes

/// Relève les ordinateurs de chaque compte (bloquant : un appel au relais par
/// compte), les garde et met l'accueil à jour. Une session d'appareil fermée
/// oublie son compte ; un relais muet garde ce qu'on savait.
pub fn sync(app: &AppHandle) {
    if SYNCING.swap(true, Ordering::AcqRel) {
        return;
    }
    *LAST_SYNC.lock().unwrap() = Some(Instant::now());
    let accounts = list();
    let known = app.state::<UiState>().with_store(|s| s.account_computers.clone());
    let fetched = accounts.iter().map(|a| (a.key(), account::computers(a))).collect();
    let (fresh, signed_out) = rules::gather(&known, fetched);
    for key in &signed_out {
        forget(app, key, true);
    }
    apply(app, fresh);
    emit_accounts(app);
    // Les réglages synchronisés, en même temps que les ordinateurs.
    super::synced::fetch_all(app);
    SYNCING.store(false, Ordering::Release);
}

/// Un relevé sur un fil : au lancement (`force`), et au retour de la fenêtre
/// (30 s au plus souvent).
pub fn sync_soon(app: &AppHandle, force: bool) {
    let recent = LAST_SYNC.lock().unwrap().is_some_and(|t| t.elapsed() < SYNC_EVERY);
    if (!force && recent) || (!force && list().is_empty()) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || sync(&app));
}

/// Un ordinateur appairé tel que `account::merge` le compare.
pub(super) fn paired_of(c: &Computer) -> Paired {
    Paired { id: c.id.clone(), fingerprint: c.fingerprint.clone(), station: c.relay.as_ref().map(|r| r.id.clone()).filter(|s| !s.is_empty()) }
}

/// L'ordinateur que la découverte voit sur le réseau avec la même empreinte d'Admin.
fn seen_on_network(app: &AppHandle, fingerprint: Option<&str>) -> Option<Found> {
    let fp = fingerprint?;
    let found = app.state::<UiState>().discovery.get()?.list();
    found.into_iter().find(|f| f.fingerprint.as_deref().is_some_and(|x| rules::same_fingerprint(x, fp)))
}

/// Ce que le compte dit de l'ordinateur, sur son entrée de l'accueil : le nom
/// (sauf s'il a été renommé ici), l'empreinte, la route par le relais du compte.
fn update_entry(c: &mut Computer, ac: &AccountComputer) {
    let renamed = c.account.as_ref().is_some_and(|old| old.name != c.name);
    if !renamed && !ac.name.trim().is_empty() {
        c.name = ac.name.trim().to_string();
    }
    if ac.admin_fp.is_some() {
        c.fingerprint = ac.admin_fp.clone();
    }
    // La route par le relais du compte (`relais::route_of`, présence « Hors de chez vous »).
    c.relay = Some(RelayInfo { server: ac.relay.clone(), id: ac.station.clone(), remote_access: true });
    c.account = Some(ac.clone());
}

fn new_entry(id: String, ac: &AccountComputer) -> Computer {
    let mut c = Computer {
        id,
        name: if ac.name.trim().is_empty() { ac.station.clone() } else { ac.name.trim().to_string() },
        host: String::new(),
        port: 0,
        user: String::new(),
        fingerprint: ac.admin_fp.clone(),
        os: String::new(),
        gpu: None,
        settings: Settings::default(),
        last: None,
        by_code: false,
        wake: Vec::new(),
        station_notified: None,
        relay: None,
        account: None,
        auto: None,
        encoder_max: None,
    };
    update_entry(&mut c, ac);
    c
}

/// La nouvelle liste des ordinateurs des comptes : gardée (`accounts.json`), et
/// l'accueil suit — un ordinateur déjà appairé n'est pas ajouté une seconde
/// fois, un ordinateur qui n'est plus à un compte part (sauf pendant sa
/// session : au relevé suivant).
fn apply(app: &AppHandle, fresh: Vec<AccountComputer>) {
    let state = app.state::<UiState>();
    let keys: Vec<String> = list().iter().map(|a| a.key()).collect();
    // Un compte oublié pendant le relevé ne revient pas.
    let fresh: Vec<AccountComputer> = fresh.into_iter().filter(|c| keys.contains(&c.account)).collect();
    let in_session = state.session_ids();
    let removed: Vec<String> = state.with_store(|s| {
        s.account_computers = fresh.clone();
        s.save_account_computers();
        let paired: Vec<Paired> = s.computers.iter().filter(|c| c.account.is_none()).map(paired_of).collect();
        let merged = account::merge(&paired, &fresh);
        let wanted: Vec<(String, &AccountComputer)> = merged.added.iter().map(|ac| (rules::computer_id(&ac.relay, &ac.station), ac)).collect();
        let removed: Vec<String> = s
            .computers
            .iter()
            .filter(|c| c.account.is_some() && !wanted.iter().any(|(id, _)| id == &c.id) && !in_session.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();
        s.computers.retain(|c| !removed.contains(&c.id));
        for (id, ac) in wanted {
            match s.computers.iter().position(|c| c.id == id) {
                Some(i) => update_entry(&mut s.computers[i], ac),
                None => s.computers.push(new_entry(id, ac)),
            }
        }
        s.save();
        removed
    });
    for id in &removed {
        state.live.lock().unwrap().remove(id);
        snapshot::forget(id);
    }
    if !removed.is_empty() {
        log::info!("comptes : {} ordinateur(s) quittent l'accueil", removed.len());
    }
    emit_computers(app);
    poll_soon(app);
}

/// Les pastilles d'origine de l'accueil (seulement avec un compte connecté).
pub fn origins(computers: &[Computer], list_now: &[AccountComputer]) -> HashMap<String, String> {
    let accounts = list();
    if accounts.is_empty() {
        return HashMap::new();
    }
    let paired: Vec<Paired> = computers.iter().filter(|c| c.account.is_none()).map(paired_of).collect();
    let from_accounts: Vec<(String, AccountComputer)> = computers.iter().filter_map(|c| Some((c.id.clone(), c.account.clone()?))).collect();
    rules::origins(&accounts, &paired, &from_accounts, list_now, i18n::lang())
}

/// Admin d'un ordinateur d'un compte, avec le jeton d'accès du relais. Son
/// compte oublié entre-temps : sans jeton (Admin refuse, l'ordinateur part au
/// relevé suivant).
pub fn console(c: &Computer, ac: &AccountComputer) -> Console {
    match account_of(&ac.account) {
        Some(a) => Console::account(&c.host, c.port, access(&a, &ac.station), c.fingerprint.clone()),
        None => Console::new(&c.host, c.port, "", "", c.fingerprint.clone()),
    }
}

/// L'adresse sur le réseau d'un ordinateur d'un compte, d'après la découverte
/// (même empreinte d'Admin) : gardée avec lui quand elle change. Rend
/// l'ordinateur à jour, ou `None` si rien ne change.
pub fn locate(app: &AppHandle, c: &Computer) -> Option<Computer> {
    c.account.as_ref()?;
    let f = seen_on_network(app, c.fingerprint.as_deref())?;
    if f.host == c.host && f.port == c.port {
        return None;
    }
    log::info!("comptes : {} vu sur le réseau ({}:{})", c.name, f.host, f.port);
    app.state::<UiState>().with_store(|s| {
        let m = s.get_mut(&c.id)?;
        m.host = f.host.clone();
        m.port = f.port;
        let updated = m.clone();
        s.save();
        Some(updated)
    })
}

/// Un refus du relais pour un ordinateur d'un compte (jeton d'accès) : session
/// de l'appareil fermée, le compte est oublié ; ordinateur retiré du compte, il
/// quitte l'accueil. Rend vrai si l'accueil a changé.
pub fn on_error(app: &AppHandle, c: &Computer, e: &AccountError) -> bool {
    let Some(ac) = c.account.as_ref() else { return false };
    match e {
        AccountError::SignedOut => {
            forget(app, &ac.account, true);
            emit_accounts(app);
            true
        }
        AccountError::UnknownComputer => {
            log::info!("comptes : {} n'est plus au compte", c.name);
            let rest = app.state::<UiState>().with_store(|s| s.account_computers.iter().filter(|x| !(x.relay == ac.relay && x.station == ac.station)).cloned().collect());
            apply(app, rest);
            true
        }
        _ => false,
    }
}

/// Une erreur d'Admin pour un ordinateur d'un compte : [`on_error`] quand
/// c'est le jeton d'accès qui manque.
pub fn on_console_error(app: &AppHandle, c: &Computer, e: &ConsoleError) -> bool {
    match e {
        ConsoleError::Account(e) => on_error(app, c, e),
        _ => false,
    }
}

/// Ce qu'il faut pour ouvrir la session d'un ordinateur d'un compte : le
/// compte et la Station (`SessionParams::account`), son adresse et la route —
/// sur le réseau d'abord quand la découverte le voit (même empreinte d'Admin),
/// par le relais du compte sinon. Le jeton d'accès est demandé d'abord : les
/// refus du relais (session d'appareil fermée, ordinateur retiré du compte,
/// relais muet) se disent avant toute fenêtre.
pub fn session_params(app: &AppHandle, c: &Computer, ac: &AccountComputer) -> Result<(AccessParams, Route, String, u16), String> {
    let lang = i18n::lang();
    let account = account_of(&ac.account).ok_or_else(|| AccountError::SignedOut.text(lang))?;
    if let Err(e) = access(&account, &ac.station).token() {
        on_error(app, c, &e);
        return Err(e.text(lang));
    }
    let params = AccessParams::new(&account, ac);
    match seen_on_network(app, ac.admin_fp.as_deref().or(c.fingerprint.as_deref())) {
        Some(f) => Ok((params, Route::Auto(ac.route()), f.host, f.port)),
        None => {
            // Jamais vu connecté au relais : pas d'empreinte d'Admin, pas de route.
            let route = ac.route().ok_or_else(|| RelayError::StationOffline.text(lang, Some(c.name.as_str())))?;
            Ok((params, Route::Relay(route), c.host.clone(), c.port))
        }
    }
}

/// Ce qu'il faut pour « Regarder » un ordinateur d'un compte
/// (`share::watch_account`) : le jeton d'accès de son compte pour cette Station
/// et son Admin sur le réseau quand la découverte le voit (même empreinte
/// d'Admin) ; la route par le relais est celle du compte. Le jeton est demandé
/// d'abord, comme pour une session : les refus du relais se disent avant tout.
pub fn watch_params(app: &AppHandle, c: &Computer, ac: &AccountComputer) -> Result<(Arc<StationAccess>, Option<kybernow_core::share::ShareTarget>), String> {
    let lang = i18n::lang();
    let account = account_of(&ac.account).ok_or_else(|| AccountError::SignedOut.text(lang))?;
    let access = access(&account, &ac.station);
    if let Err(e) = access.token() {
        on_error(app, c, &e);
        return Err(e.text(lang));
    }
    let lan = seen_on_network(app, ac.admin_fp.as_deref().or(c.fingerprint.as_deref()))
        .map(|f| kybernow_core::share::ShareTarget { host: f.host, port: f.port, fingerprint: f.fingerprint });
    if lan.is_none() && ac.route().is_none() {
        // Jamais vu connecté au relais : pas d'empreinte d'Admin, pas de route.
        return Err(RelayError::StationOffline.text(lang, Some(c.name.as_str())));
    }
    Ok((access, lan))
}
