//! Comptes KyberNow, côté appareil (docs/comptes.md, lot 3) : ce qui appelle le cœur (`kybernow_core::account`) sur
//! des fils à part, et ce qui fait de l'accueil la somme des ordinateurs appairés et de ceux des comptes.
//!
//! - **Garde** : les comptes sont dans `Store::accounts` (le fichier de l'app, 0600 : en mode Jeu de SteamOS et sur
//!   KyberNow Link OS aucun trousseau n'est déverrouillé, c'est là que sont déjà les mots de passe des ordinateurs et
//!   des prêts) ; le JSON est celui d'`account::Account`, recopié par serde (`to_core`, `from_core`).
//! - **Connexion** : par un code (`tv_code`, puis `tv_wait` sur un fil, avec un `AtomicBool` pour « Retour » ; un code
//!   échu est remplacé de lui-même) ; sur le bureau, par la page du relais (`sign_in` dans le navigateur, le retour
//!   `kybernow://compte` par les liens, `finish_sign_in`).
//! - **Accueil** : `refresh` redemande les ordinateurs de chaque compte (au lancement, après une connexion, toutes les
//!   5 minutes) ; `sync` refait l'accueil (`merge` avec les appairés, pastilles `origin` / `origin_paired`) et le
//!   registre des ordinateurs des comptes (`console`, `access`), par lequel passent les sondes, la Station, le partage,
//!   le prêt et la session.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use kybernow_core::account::{self, AccessParams, Account, AccountComputer, AccountError, Paired, StationAccess};
use kybernow_core::console::Console;
use kybernow_core::relay::RelayRoute;

use super::Event;
use crate::accounts::{SignInFail, TvCodeUi};
use crate::app::App;
use crate::model::{Host, KnAccount, KnComputer};

/// Ce que le pilote renvoie à la boucle.
pub enum AccountEvent {
    /// Un code à afficher (et chaque code suivant, quand le précédent est échu), ou le refus du relais.
    Code { attempt: u32, result: Result<TvCodeUi, String> },
    /// La connexion a abouti, ou non.
    SignedIn { attempt: u32, result: Result<KnAccount, SignInFail> },
    /// Bureau : la page du relais n'a pas pu s'ouvrir.
    BrowserFailed { attempt: u32, message: String },
    /// Les ordinateurs d'un compte (empreintes d'Admin déjà gardées), ou la session de l'appareil fermée.
    Computers { key: String, result: Result<Vec<KnComputer>, AccountError> },
}

pub fn to_core(a: &KnAccount) -> Option<Account> {
    serde_json::to_value(a).ok().and_then(|v| serde_json::from_value(v).ok())
}

fn from_core(a: &Account) -> Option<KnAccount> {
    serde_json::to_value(a).ok().and_then(|v| serde_json::from_value(v).ok())
}

fn computer_to_core(c: &KnComputer) -> Option<AccountComputer> {
    serde_json::to_value(c).ok().and_then(|v| serde_json::from_value(v).ok())
}

fn computer_from_core(c: &AccountComputer) -> Option<KnComputer> {
    serde_json::to_value(c).ok().and_then(|v| serde_json::from_value(v).ok())
}

fn lang() -> kybernow_core::i18n::Lang {
    kybernow_core::i18n::current()
}

// MARK: - Connexion

/// L'attente en cours (un code, un seul à la fois) : « Retour » ou un autre relais la coupe.
static CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Coupe l'attente d'un code en cours.
pub fn cancel() {
    if let Some(c) = CANCEL.lock().unwrap().take() {
        c.store(true, Ordering::Relaxed);
    }
}

fn code_ui(c: &account::TvCode) -> TvCodeUi {
    TvCodeUi { code: c.code.clone(), url_label: c.url_label.clone(), url_complete: c.url_complete.clone(), relay: c.relay.clone(), expires_at: c.issued_at + c.expires }
}

/// Un code pour ce relais, et l'attente de son approbation (`tv_wait`) : un code échu est remplacé de lui-même, un
/// relais muet un moment ne coupe rien (le cœur redemande) ; refusé, annulé, ou approuvé : la réponse.
pub fn sign_in_by_code(attempt: u32, relay: String, device_name: String, tx: Sender<Event>) {
    cancel();
    let flag = Arc::new(AtomicBool::new(false));
    *CANCEL.lock().unwrap() = Some(flag.clone());
    std::thread::Builder::new()
        .name("account-code".into())
        .spawn(move || {
            let send = |e: AccountEvent| {
                let _ = tx.send(Event::Account(e));
            };
            // Une adresse tapée se lit d'abord (« Adresse du relais illisible. »).
            let relay = match account::normalize_relay(&relay) {
                Ok(r) => r,
                Err(e) => return send(AccountEvent::Code { attempt, result: Err(e.text(lang())) }),
            };
            loop {
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                let code = match account::tv_code(&relay, &device_name) {
                    Ok(c) => c,
                    Err(e) => {
                        log::info!("comptes : pas de code ({})", e.kind());
                        return send(AccountEvent::Code { attempt, result: Err(e.text(lang())) });
                    }
                };
                send(AccountEvent::Code { attempt, result: Ok(code_ui(&code)) });
                match account::tv_wait(&code, &flag) {
                    Ok(a) => {
                        let result = from_core(&a).ok_or_else(|| SignInFail::Message(AccountError::Other("session illisible".into()).text(lang())));
                        return send(AccountEvent::SignedIn { attempt, result });
                    }
                    // Échu : un nouveau code s'affiche de lui-même.
                    Err(AccountError::Expired) => continue,
                    Err(AccountError::Cancelled) => return,
                    Err(AccountError::Denied) => return send(AccountEvent::SignedIn { attempt, result: Err(SignInFail::Denied) }),
                    Err(e) => return send(AccountEvent::SignedIn { attempt, result: Err(SignInFail::Message(e.text(lang()))) }),
                }
            }
        })
        .ok();
}

/// Bureau : la connexion par la page du relais en cours (son `etat`), gardée pour le retour `kybernow://compte`.
static SIGN_IN: Mutex<Option<(u32, account::SignIn)>> = Mutex::new(None);

/// Bureau : la page de connexion du relais s'ouvre dans le navigateur (`xdg-open`, le portail sous Flatpak).
pub fn sign_in_by_browser(attempt: u32, relay: String, device_name: String, tx: Sender<Event>) {
    let s = match account::sign_in(&relay, &device_name) {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(Event::Account(AccountEvent::BrowserFailed { attempt, message: e.text(lang()) }));
            return;
        }
    };
    // L'adresse porte l'`etat` de la demande : elle ne va pas au journal.
    match std::process::Command::new("xdg-open").arg(&s.url).spawn() {
        Ok(_) => log::info!("comptes : page de connexion de {} ouverte", s.relay),
        Err(e) => {
            log::warn!("comptes : navigateur impossible ({e})");
            let _ = tx.send(Event::Account(AccountEvent::BrowserFailed { attempt, message: AccountError::Other(e.to_string()).text(lang()) }));
            return;
        }
    }
    *SIGN_IN.lock().unwrap() = Some((attempt, s));
}

/// Le retour de la page du relais (`kybernow://compte?…`) : le code s'échange contre la session d'appareil. Sans
/// connexion en cours (l'app relancée entre-temps), « La connexion n'a pas abouti : réessayez. ».
pub fn callback(url: String, tx: Sender<Event>) {
    let Some((attempt, s)) = SIGN_IN.lock().unwrap().take() else {
        log::info!("comptes : retour de la page du relais sans connexion en cours");
        let _ = tx.send(Event::Account(AccountEvent::SignedIn { attempt: 0, result: Err(SignInFail::Message(AccountError::BadCode.text(lang()))) }));
        return;
    };
    std::thread::Builder::new()
        .name("account-callback".into())
        .spawn(move || {
            let result = match account::finish_sign_in(&s, &url) {
                Ok(a) => from_core(&a).ok_or_else(|| SignInFail::Message(AccountError::Other("session illisible".into()).text(lang()))),
                Err(AccountError::Cancelled) => Err(SignInFail::Cancelled),
                Err(e) => Err(SignInFail::Message(e.text(lang()))),
            };
            let _ = tx.send(Event::Account(AccountEvent::SignedIn { attempt, result }));
        })
        .ok();
}

/// « Se déconnecter » : la session d'appareil fermée chez le relais ; le compte est déjà oublié de l'app, même si le
/// relais ne répond pas.
pub fn sign_out(a: KnAccount) {
    let Some(a) = to_core(&a) else { return };
    std::thread::Builder::new()
        .name("account-sign-out".into())
        .spawn(move || {
            if let Err(e) = account::sign_out(&a) {
                log::info!("comptes : déconnexion sans réponse du relais ({}), compte oublié quand même", e.kind());
            }
        })
        .ok();
}

// MARK: - Ordinateurs des comptes

/// Redemande les ordinateurs de chaque compte (`GET /v1/computers`) ; les empreintes d'Admin déjà vues restent
/// (`remember_fingerprints`).
pub fn refresh(app: &App, tx: &Sender<Event>) {
    for a in &app.store.accounts {
        let Some(core) = to_core(a) else { continue };
        let key = a.key();
        let known: Vec<AccountComputer> = app.store.account_computers.iter().filter(|c| c.account == key).filter_map(computer_to_core).collect();
        let tx = tx.clone();
        std::thread::Builder::new()
            .name("account-computers".into())
            .spawn(move || {
                let result = account::computers(&core).map(|mut list| {
                    account::remember_fingerprints(&mut list, &known);
                    list.iter().filter_map(computer_from_core).collect()
                });
                if let Err(e) = &result {
                    log::info!("comptes : ordinateurs de {} non reçus ({})", core.account.id, e.kind());
                }
                let _ = tx.send(Event::Account(AccountEvent::Computers { key, result }));
            })
            .ok();
    }
}

/// Un ordinateur d'un compte, tel que le pilote le joint : le jeton d'accès (gardé 4 minutes, partagé par les sondes,
/// la Station, le partage et le prêt) et sa route par le relais du compte.
struct Entry {
    params: AccessParams,
    access: Arc<StationAccess>,
    route: Option<RelayRoute>,
}

static REGISTRY: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

/// Refait l'accueil d'après les comptes et leurs ordinateurs gardés : un ordinateur une fois (`merge` : un appairé
/// qui est aussi dans un compte reste l'appairé, avec la pastille du compte), les pastilles (seulement avec un compte
/// connecté ; `Compte · {relais}` quand les comptes sont sur plusieurs relais), et le registre.
pub fn sync(app: &mut App) {
    let l = lang();
    let computers: Vec<AccountComputer> = app.store.account_computers.iter().filter(|c| app.store.account(&c.account).is_some()).filter_map(computer_to_core).collect();
    // Les appairés : ceux qui ne viennent pas d'un compte, avec leur empreinte et leur Station chez le relais.
    let paired: Vec<Paired> = app.store.hosts.iter().filter(|h| app.store.account_host(&h.id).is_none()).map(|h| Paired { id: h.id.clone(), fingerprint: h.fingerprint.clone(), station: h.relay.as_ref().map(|r| r.id.clone()).filter(|s| !s.is_empty()) }).collect();
    let merged = account::merge(&paired, &computers);
    let mut relays: Vec<&str> = app.store.accounts.iter().map(|a| a.relay.as_str()).collect();
    relays.sort();
    relays.dedup();
    let several = relays.len() > 1;
    let added: Vec<KnComputer> = merged.added.iter().filter_map(computer_from_core).collect();
    // Les pastilles des appairés d'abord ; celles des ordinateurs des comptes une fois leurs identifiants connus.
    let mut origins: HashMap<String, String> = HashMap::new();
    if !app.store.accounts.is_empty() {
        for p in &paired {
            let badge = match merged.linked.iter().find(|(id, _)| *id == p.id) {
                Some((_, c)) => c.origin(l, several),
                None => account::origin_paired(l),
            };
            origins.insert(p.id.clone(), badge);
        }
    }
    app.set_account_hosts(&added, HashMap::new());
    let mut registry = REGISTRY.lock().unwrap();
    let old = registry.take().unwrap_or_default();
    let mut fresh = HashMap::new();
    for link in app.store.account_hosts.clone() {
        let Some(c) = merged.added.iter().find(|c| c.relay == link.relay && c.station == link.station) else { continue };
        if !app.store.accounts.is_empty() {
            origins.insert(link.id.clone(), c.origin(l, several));
        }
        let Some(a) = app.store.account(&c.account).and_then(to_core) else { continue };
        let params = AccessParams::new(&a, c);
        // Le même compte, la même Station : le jeton gardé sert encore.
        let access = match old.get(&link.id) {
            Some(e) if e.params == params => e.access.clone(),
            _ => StationAccess::new(params.clone()),
        };
        fresh.insert(link.id.clone(), Entry { params, access, route: c.route() });
    }
    *registry = Some(fresh);
    drop(registry);
    app.accounts.origins = origins;
    // L'adresse sur le réseau, d'après la découverte.
    locate(app);
}

/// La découverte a changé : un ordinateur d'un compte annoncé avec la même empreinte d'Admin prend son adresse.
pub fn locate(app: &mut App) {
    let found: Vec<(String, u16, Option<String>)> = app.found.iter().map(|f| (f.address.clone(), f.port, f.fingerprint.clone())).collect();
    if app.store.locate_account_hosts(&found) {
        app.store.save();
    }
}

/// Les paramètres de session d'un ordinateur d'un compte, et sa route par le relais ; `None` : un ordinateur appairé.
pub fn access(host: &str) -> Option<(AccessParams, Option<RelayRoute>)> {
    REGISTRY.lock().unwrap().as_ref()?.get(host).map(|e| (e.params.clone(), e.route.clone()))
}

/// Regarder : le jeton d'accès d'un ordinateur d'un compte et sa route par le relais (`share::watch_account`) ;
/// `None` : un ordinateur appairé.
pub fn watch_access(host: &str) -> Option<(Arc<StationAccess>, Option<RelayRoute>)> {
    REGISTRY.lock().unwrap().as_ref()?.get(host).map(|e| (e.access.clone(), e.route.clone()))
}

/// La console d'Admin d'un ordinateur d'un compte (jeton d'accès du relais, Admin épinglé sur l'empreinte du compte),
/// sur le réseau quand la découverte l'y voit, sinon par le relais ; `None` : un ordinateur appairé.
pub fn console(host: &Host) -> Option<Console> {
    let registry = REGISTRY.lock().unwrap();
    let e = registry.as_ref()?.get(&host.id)?;
    let c = Console::account(&host.address, host.port, e.access.clone(), host.fingerprint.clone());
    Some(if host.address.is_empty() { c.with_relay(e.route.clone()) } else { c })
}

/// La console d'Admin d'un ordinateur de l'accueil, par le relais quand `relay` (« Hors de chez vous ») : celle d'un
/// compte (le relais du compte), ou celle de l'appareil appairé (le relais de sa Station).
pub fn admin(host: &Host, relay: bool) -> Console {
    if let Some(e) = REGISTRY.lock().unwrap().as_ref().and_then(|r| r.get(&host.id)) {
        let c = Console::account(&host.address, host.port, e.access.clone(), host.fingerprint.clone());
        return if relay || host.address.is_empty() { c.with_relay(e.route.clone()) } else { c };
    }
    let c = Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone());
    if relay { c.with_relay(super::session::relay_route(host)) } else { c }
}

/// La console d'Admin d'un ordinateur de l'accueil : celle d'un compte, ou celle de l'appareil appairé.
pub fn console_of(host: &Host) -> Console {
    console(host).unwrap_or_else(|| Console::new(&host.address, host.port, &host.user, &host.password, host.fingerprint.clone()))
}

/// Une session qui finit sur une erreur d'un ordinateur d'un compte : la session de l'appareil fermée (le compte est
/// oublié) ou un ordinateur qui n'est plus au compte (il quitte l'accueil). Le texte de la session est celui du cœur
/// (`ConsoleError::Account`).
pub fn session_failed(app: &mut App, host: &str, error: &str) -> Vec<crate::app::Action> {
    let Some(link) = app.store.account_host(host).cloned() else { return Vec::new() };
    let l = lang();
    if error == AccountError::SignedOut.text(l) {
        return app.account_signed_out(&link.account, AccountError::SignedOut.text(l));
    }
    if error == AccountError::UnknownComputer.text(l) {
        return app.account_unknown_computer(host, AccountError::UnknownComputer.text(l));
    }
    Vec::new()
}

/// Ce que la boucle fait d'une réponse du pilote ; rend vrai quand l'accueil est à refaire (`sync`).
pub fn handle(e: AccountEvent, app: &mut App, actions: &mut Vec<crate::app::Action>) -> bool {
    match e {
        AccountEvent::Code { attempt, result } => {
            app.account_code(attempt, result);
            false
        }
        AccountEvent::BrowserFailed { attempt, message } => {
            app.account_browser_failed(attempt, message);
            false
        }
        AccountEvent::SignedIn { attempt, result } => {
            // Un retour de la page du relais sans connexion en cours (attempt 0) : le dire en avis.
            if attempt == 0
                && let Err(SignInFail::Message(m)) = &result
            {
                app.toast(m.clone());
                return false;
            }
            actions.extend(app.account_signed_in(attempt, result));
            false
        }
        AccountEvent::Computers { key, result } => match result {
            Ok(list) => {
                app.account_computers(&key, list);
                actions.push(crate::app::Action::Save);
                true
            }
            Err(AccountError::SignedOut) => {
                actions.extend(app.account_signed_out(&key, AccountError::SignedOut.text(lang())));
                false
            }
            // Relais muet, autre refus : la liste gardée reste.
            Err(_) => false,
        },
    }
}
