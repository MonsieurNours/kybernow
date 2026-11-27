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
//! - **Réglages synchronisés** (`crate::synced`, apps/core/README.md, « Réglages synchronisés ») : la mémoire des envois
//!   est celle du cœur (`account::AccountSync`, gardée par l'interface dans le même JSON) ; `refresh` envoie aussi les
//!   changements en attente puis lit les réglages du compte (`put_settings`, `fetch_settings`, `AccountSync::fetched`) ;
//!   `push`, après chaque réglage gardé, pose les clés changées sur ce qui attend (`to_send`, `unsent`) et les envoie ; `before_session` relit juste avant une session sur un ordinateur
//!   d'un compte, 1,5 s au plus (au-delà, la session part avec les réglages locaux et la lecture s'applique en arrivant).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use kybernow_core::account::{self, AccessParams, Account, AccountComputer, AccountError, AccountSync, Paired, StationAccess, SyncedSettings, SyncedValues};
use kybernow_core::console::Console;
use kybernow_core::relay::RelayRoute;

use super::Event;
use crate::accounts::{SignInFail, TvCodeUi};
use crate::app::App;
use crate::model::{Host, KnAccount, KnComputer, Profile};
use crate::synced::{SyncKeys, SyncMemory};

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
    /// Réglages synchronisés d'un compte : les envois faits, puis la lecture (`None` : pas lue) ; `connect` : la
    /// session qui attendait cette lecture (`before_session`).
    Settings { key: String, sent: Vec<Sent>, fetched: Option<Result<Vec<SyncedSettings>, AccountError>>, connect: Option<(String, Profile)> },
    /// La lecture d'avant une session tarde : la session part avec les réglages locaux.
    ConnectNow { host: String, profile: Profile },
}

/// Un envoi de réglages fait : les valeurs envoyées, et ce que le relais garde désormais.
pub struct Sent {
    pub station: String,
    pub values: SyncedValues,
    pub result: Result<SyncedSettings, AccountError>,
}

/// Un envoi à faire : ce qui attend pour cet ordinateur, posé sur les dernières valeurs lues ou envoyées.
struct Push {
    account: String,
    station: String,
    values: SyncedValues,
    last: Option<SyncedSettings>,
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
    // Réglages synchronisés : les envois en attente, puis la lecture, compte par compte.
    if app.sync_on() {
        for key in app.store.accounts.iter().map(|a| a.key()) {
            settings_job(app, &key, tx, None);
        }
    }
}

// MARK: - Réglages synchronisés

/// Les ordinateurs dont un envoi est en cours (compte, Station) : un seul à la fois, le suivant part à sa réponse.
static IN_FLIGHT: Mutex<Option<HashSet<(String, String)>>> = Mutex::new(None);
/// Les ordinateurs dont la session attend la lecture d'avant session ; ceux dont la lecture est faite (la session part
/// sans relire).
static WAITING: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static READY: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Ce qu'une session attend au plus de la lecture des réglages.
const BEFORE_SESSION: std::time::Duration = std::time::Duration::from_millis(1500);

fn set_insert(set: &Mutex<Option<HashSet<String>>>, v: &str) -> bool {
    set.lock().unwrap().get_or_insert_with(HashSet::new).insert(v.to_string())
}

fn set_remove(set: &Mutex<Option<HashSet<String>>>, v: &str) -> bool {
    set.lock().unwrap().as_mut().is_some_and(|s| s.remove(v))
}

/// L'envoi de cet ordinateur a sa réponse.
fn done(account: &str, station: &str) {
    if let Some(f) = IN_FLIGHT.lock().unwrap().as_mut() {
        f.remove(&(account.to_string(), station.to_string()));
    }
}

fn keys_to_core(k: &SyncKeys) -> Option<SyncedValues> {
    serde_json::to_value(k).ok().and_then(|v| serde_json::from_value(v).ok())
}

fn keys_from_core(v: &SyncedValues) -> SyncKeys {
    serde_json::to_value(v).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

/// La mémoire des réglages synchronisés d'un compte, celle du cœur (même JSON que `SyncMemory`).
fn memory(app: &App, key: &str) -> AccountSync {
    serde_json::to_value(app.store.sync_memory(key)).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

/// La mémoire changée par le cœur, gardée par l'interface.
fn keep(app: &mut App, key: &str, sync: &AccountSync) {
    match serde_json::to_value(sync).ok().and_then(|v| serde_json::from_value::<SyncMemory>(v).ok()) {
        Some(m) => app.store.set_sync_memory(key, m),
        None => log::error!("comptes : mémoire des réglages synchronisés illisible"),
    }
}

/// L'envoi de ce qui attend pour un ordinateur (rien : `None`).
fn push_for(sync: &AccountSync, account: &str, station: &str) -> Option<Push> {
    let values = sync.pending.get(station).filter(|v| !v.is_empty())?.clone();
    Some(Push { account: account.to_string(), station: station.to_string(), values, last: sync.last(station).cloned() })
}

/// Le débit du profil au mode de la session (Mbit/s) : un réglage personnalisé dont le débit est « Auto » l'envoie.
fn auto_mbps(app: &App, h: &Host) -> u32 {
    let s = super::stream_settings(app, h, h.settings.profile, false);
    let (w, hh, fps) = app.session_mode(h);
    s.bitrate(w, hh, fps) / 1_000_000
}

/// Un envoi (`PUT /v1/settings/<station>`), posé sur les dernières valeurs lues.
fn put(core: &Account, p: &Push) -> Sent {
    Sent { station: p.station.clone(), values: p.values.clone(), result: account::put_settings(core, &p.station, p.last.as_ref(), &p.values) }
}

/// Les envois en attente d'un compte, puis sa lecture, sur un fil ; `connect` : la session qui attend.
fn settings_job(app: &App, key: &str, tx: &Sender<Event>, connect: Option<(String, Profile, Arc<AtomicBool>)>) {
    let Some(core) = app.store.account(key).and_then(to_core) else { return };
    let sync = memory(app, key);
    let pushes: Vec<Push> = sync
        .pending
        .keys()
        .filter_map(|station| push_for(&sync, key, station))
        .filter(|p| IN_FLIGHT.lock().unwrap().get_or_insert_with(HashSet::new).insert((p.account.clone(), p.station.clone())))
        .collect();
    let (key, tx) = (key.to_string(), tx.clone());
    std::thread::Builder::new()
        .name("account-settings".into())
        .spawn(move || {
            // Ce que le relais n'a pas reçu part avant de lire ce qui arrive.
            let sent: Vec<Sent> = pushes.iter().map(|p| put(&core, p)).collect();
            let fetched = account::fetch_settings(&core);
            if let Err(e) = &fetched {
                log::info!("comptes : réglages de {} non lus ({})", core.account.id, e.kind());
            }
            // La session qui attend part avec cette lecture, sauf si elle est déjà partie sans elle.
            let connect = connect.and_then(|(host, profile, claimed)| (!claimed.swap(true, Ordering::SeqCst)).then_some((host, profile)));
            let _ = tx.send(Event::Account(AccountEvent::Settings { key, sent, fetched: Some(fetched), connect }));
        })
        .ok();
}

/// Après un réglage gardé : les clés changées des ordinateurs des comptes, posées sur ce qui attend encore (et toutes
/// pour un ordinateur que le relais n'a jamais reçu : `AccountSync::to_send`), gardées comme en attente (`unsent`)
/// puis envoyées (un envoi à la fois par ordinateur ; le suivant part à la réponse du précédent).
pub fn push(app: &mut App, tx: &Sender<Event>) {
    let changes = app.sync_changes(&auto_mbps);
    if changes.is_empty() {
        return;
    }
    let mut pushes = Vec::new();
    for c in changes {
        let (Some(change), Some(full)) = (keys_to_core(&c.keys), keys_to_core(&c.full)) else { continue };
        let mut sync = memory(app, &c.account);
        let values = sync.to_send(&c.station, &change, || full);
        sync.unsent(&c.station, &values);
        keep(app, &c.account, &sync);
        pushes.extend(push_for(&sync, &c.account, &c.station));
    }
    app.store.save();
    send_pushes(app, pushes, tx);
}

fn send_pushes(app: &App, pushes: Vec<Push>, tx: &Sender<Event>) {
    for p in pushes {
        if !IN_FLIGHT.lock().unwrap().get_or_insert_with(HashSet::new).insert((p.account.clone(), p.station.clone())) {
            continue;
        }
        let Some(core) = app.store.account(&p.account).and_then(to_core) else {
            done(&p.account, &p.station);
            continue;
        };
        let tx = tx.clone();
        std::thread::Builder::new()
            .name("account-settings-put".into())
            .spawn(move || {
                let sent = put(&core, &p);
                let _ = tx.send(Event::Account(AccountEvent::Settings { key: p.account.clone(), sent: vec![sent], fetched: None, connect: None }));
            })
            .ok();
    }
}

/// Juste avant une session sur un ordinateur d'un compte : la lecture des réglages, 1,5 s au plus ; la session part à
/// sa réponse (`AccountEvent::Settings`) ou au bout de l'attente (`ConnectNow`). Rend vrai quand la session attend.
pub fn before_session(app: &App, tx: &Sender<Event>, host: &str, profile: Profile) -> bool {
    if set_remove(&READY, host) || !app.sync_on() {
        return false;
    }
    let Some((key, _)) = app.store.sync_target(host) else { return false };
    // Déjà en attente (deux appuis) : la même lecture sert.
    if !set_insert(&WAITING, host) {
        return true;
    }
    let claimed = Arc::new(AtomicBool::new(false));
    settings_job(app, &key, tx, Some((host.to_string(), profile, claimed.clone())));
    let (tx, host) = (tx.clone(), host.to_string());
    std::thread::Builder::new()
        .name("account-settings-wait".into())
        .spawn(move || {
            std::thread::sleep(BEFORE_SESSION);
            if !claimed.swap(true, Ordering::SeqCst) {
                log::info!("comptes : réglages pas encore lus, la session part avec les réglages de l'appareil");
                let _ = tx.send(Event::Account(AccountEvent::ConnectNow { host, profile }));
            }
        })
        .ok();
    true
}

/// La session qui attendait part : avec le profil de l'ordinateur tel qu'il est maintenant (reçu, peut-être).
fn connect_now(app: &App, host: String, profile: Profile, actions: &mut Vec<crate::app::Action>) {
    set_remove(&WAITING, &host);
    set_insert(&READY, &host);
    let profile = app.store.host(&host).map_or(profile, |h| h.settings.profile);
    actions.push(crate::app::Action::Connect { host, profile });
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
pub fn handle(e: AccountEvent, app: &mut App, actions: &mut Vec<crate::app::Action>, tx: &Sender<Event>) -> bool {
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
        AccountEvent::Settings { key, sent, fetched, connect } => {
            let mut save = false;
            let mut again = Vec::new();
            for s in sent {
                done(&key, &s.station);
                let mut sync = memory(app, &key);
                match s.result {
                    Ok(kept) => {
                        // Changé encore pendant l'envoi : ce qui n'est pas parti reste à envoyer, et part.
                        let mut rest = sync.pending.get(&s.station).cloned().unwrap_or_default();
                        rest.remove_sent(&s.values);
                        sync.sent(kept);
                        if !rest.is_empty() {
                            sync.unsent(&s.station, &rest);
                        }
                        again.extend(push_for(&sync, &key, &s.station));
                    }
                    // L'ordinateur n'est plus au compte : ses réglages gardés sont oubliés (les locaux restent).
                    Err(AccountError::UnknownComputer) => sync.forget(&s.station),
                    Err(AccountError::SignedOut) => {
                        actions.extend(app.account_signed_out(&key, AccountError::SignedOut.text(lang())));
                        continue;
                    }
                    // Relais muet : le changement attend le rafraîchissement suivant.
                    Err(AccountError::Unreachable(_)) => continue,
                    // Relais d'avant les réglages synchronisés (sans rien dire), trop gros, refusé : pas d'autre essai.
                    Err(e) => {
                        log::info!("comptes : réglages de {} pas gardés par le relais ({})", s.station, e.kind());
                        sync.refused(&s.station, &s.values);
                    }
                }
                keep(app, &key, &sync);
                save = true;
            }
            // Ce qui est plus récent que ce qui est appliqué passe dans les réglages des ordinateurs (une clé encore en
            // attente d'envoi garde la valeur de l'appareil). La session d'appareil fermée est dite par la liste des
            // ordinateurs ; les autres refus, sans rien dire.
            if let Some(Ok(list)) = fetched
                && app.sync_on()
                && app.store.account(&key).is_some()
            {
                let mut sync = memory(app, &key);
                let before = sync.clone();
                let fresh = sync.fetched(list);
                keep(app, &key, &sync);
                save |= sync != before;
                for (station, values) in fresh {
                    save |= app.sync_apply(&key, &station, &keys_from_core(&values));
                }
            }
            if save {
                actions.push(crate::app::Action::Save);
            }
            send_pushes(app, again, tx);
            if let Some((host, profile)) = connect {
                connect_now(app, host, profile, actions);
            }
            false
        }
        AccountEvent::ConnectNow { host, profile } => {
            connect_now(app, host, profile, actions);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La mémoire de l'interface est celle du cœur, au même JSON : rien ne se perd d'un côté à l'autre (LINAPP-40).
    #[test]
    fn memoire_au_json_du_coeur() {
        let mut sync = AccountSync::default();
        let list = vec![SyncedSettings { station: "abcdefghijklmnopqrstuvwxyz".into(), updated_at: 120, values: serde_json::from_str(r#"{"hdr":true,"inconnue":[1,2]}"#).unwrap() }];
        sync.fetched(list);
        sync.unsent("abcdefghijklmnopqrstuvwxyz", &SyncedValues { profile: Some(None), codec: Some("hevc".into()), mbps: Some(80), immersive: Some("never".into()), ..Default::default() });
        let ours: SyncMemory = serde_json::from_value(serde_json::to_value(&sync).unwrap()).unwrap();
        let back: AccountSync = serde_json::from_value(serde_json::to_value(&ours).unwrap()).unwrap();
        assert_eq!(back, sync);
        // Les clés : `SyncKeys` est le JSON de `SyncedValues`, « Personnalisé » (`null`) compris.
        let v = SyncedValues { profile: Some(None), yuv444: Some(true), hdr: Some(false), pen: Some("mouse".into()), ..Default::default() };
        assert_eq!(keys_to_core(&keys_from_core(&v)), Some(v));
    }
}
