//! Prêt de l'ordinateur (KyberNow Share, docs/pret.md, textes de docs/link-ui.md
//! « Prêt de l'ordinateur ») côté Windows, dans l'app :
//!
//! - **L'ami** : « Rejoindre avec un code » qui tombe sur un code de prêt
//!   (`share_join` rend `lend` et un ticket, `ShareError::Lend` du cœur),
//!   l'ordinateur d'un ami sur l'accueil (« Ordinateur de {hôte} », ajouté par le
//!   lien d'invitation `kybernow://borrow?…` ou « Ajouter un ordinateur » › « Avec
//!   un compte d'ami » : `lend_invite`, `lend_add`), gardé à part des ordinateurs
//!   appairés (`friends.json`, mot de passe au gestionnaire d'identifiants de
//!   Windows avec « Retenir le mot de passe »), son état (`lend::status`, relevé
//!   toutes les 15 s, `lend-friends`), « Emprunter » (`lend_borrow` : le prêt
//!   démarre, ou « En attente de {hôte}… » par `lend_wait`, `lend_cancel`), la
//!   session d'emprunt (`lend_connect`, `SessionParams::borrower`, identifiant
//!   `lend-N`, titrée « Ordinateur de {hôte} », profil Jeu, sans presse-papiers ni
//!   profil de la Station), « Rendre l'ordinateur » (`lend_give_back`), le préavis
//!   (`HostEvents::lend_notice`) et la fin (`HostEvents::lend_end`, carte de fin
//!   de l'accueil, « Revenir » : `lend_come_back`).
//! - **L'hôte** : « Prêter l'ordinateur… » dans la barre d'une session d'hôte
//!   (`lend_state`, `lend_code`, `lend_cancel_code`, `lend_answer`), les demandes
//!   (avis « {prénom} veut emprunter l'ordinateur. », signe de vie de la session :
//!   `HostEvents::lend`), et sur l'accueil de cet ordinateur « Prêté à {prénom} »
//!   avec « Reprendre l'ordinateur » (`lend_take_back`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use kybernow_core::console::{Console, Mode};
use kybernow_core::lend::{self, BorrowRequest, Borrowing, LendCode, LendCredentials, LendEnd, LendError, LendView, Loan, PendingBorrow};
use kybernow_core::profile::{Profile, StreamSettings};
use kybernow_core::relay::RelayRoute;
use kybernow_core::screens::DisplaysSetting;
use kybernow_core::session::SessionParams;
use kybernow_core::share::ShareTarget;

use super::*;
use crate::lending::{self, BorrowBar, BorrowOutcome, Friend, FriendView, LendPanel, LendWaitOutcome, LoanReady, NoticeView, Seen};

/// Ce que l'app tient du prêt, à côté de `UiState`.
#[derive(Default)]
pub struct LendState {
    /// Les ordinateurs empruntés (`friends.json`), lus une fois.
    friends: Mutex<Option<Vec<Friend>>>,
    /// Ce que l'app sait de chacun (carte de l'accueil).
    seen: Mutex<HashMap<String, Seen>>,
    /// Les mots de passe donnés sans « Retenir le mot de passe », le temps du lancement.
    passwords: Mutex<HashMap<String, String>>,
    /// Un code de prêt trouvé par « Rejoindre avec un code », en attente de `lend_borrow`.
    ticket: Mutex<Option<Ticket>>,
    /// Une demande qui attend l'accord de l'hôte (« En attente de {hôte}… »).
    wait: Mutex<Option<Wait>>,
    /// Un prêt accordé dont la session s'ouvre (`lend_connect`).
    pending: Mutex<Option<Pending>>,
    /// La dernière demande accordée, pour « Revenir » (15 min, `lend::COME_BACK`).
    last: Mutex<Option<Again>>,
    /// Relevé des ordinateurs empruntés demandé tout de suite.
    asked: AtomicBool,
}

struct Ticket {
    id: u64,
    code: LendCode,
    name: String,
}

struct Wait {
    id: u64,
    pending: PendingBorrow,
    again: Again,
    cancelled: Arc<AtomicBool>,
}

struct Pending {
    id: String,
    session: u64,
    loan: Loan,
}

#[derive(Clone)]
struct Again {
    target: ShareTarget,
    route: Option<RelayRoute>,
    request: BorrowRequest,
}

/// La session d'emprunt, vue par l'app : l'hôte, le titre de ses fenêtres, la
/// fin prévue, le préavis en cours et la fin du prêt.
pub struct Borrow {
    /// {hôte} des textes : le prénom de l'hôte, ou le nom de l'ordinateur.
    pub host: String,
    /// « Ordinateur de {hôte} ».
    pub title: String,
    pub until: Option<u64>,
    pub notice: Mutex<Option<NoticeView>>,
    /// La fin dite par le prêt (`HostEvents::lend_end`), juste avant celle de la session.
    pub end: Mutex<Option<(LendEnd, String)>>,
}

impl Borrow {
    pub fn bar(&self) -> BorrowBar {
        BorrowBar { host: self.host.clone(), until: self.until, notice: self.notice.lock().unwrap().clone() }
    }
}

/// Numéros des tickets et des demandes en attente.
static SEQ: AtomicU64 = AtomicU64::new(1);

fn friends_path() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow").join("friends.json")
}

impl LendState {
    fn with_friends<T>(&self, f: impl FnOnce(&mut Vec<Friend>) -> T) -> T {
        let mut list = self.friends.lock().unwrap();
        f(list.get_or_insert_with(|| lending::load_friends(&friends_path())))
    }

    fn save(&self) {
        let list = self.with_friends(|l| l.clone());
        if let Err(e) = lending::save_friends(&friends_path(), &list) {
            log::error!("friends.json : {e}");
        }
    }

    fn friend(&self, id: &str) -> Option<Friend> {
        self.with_friends(|l| l.iter().find(|f| f.id == id).cloned())
    }

    /// Le mot de passe de ce compte : donné depuis le lancement, sinon retenu.
    fn password(&self, f: &Friend) -> Option<String> {
        if let Some(p) = self.passwords.lock().unwrap().get(&f.id) {
            return Some(p.clone());
        }
        if f.remember { store::password(&f.credential()).filter(|p| !p.is_empty()) } else { None }
    }

    /// Le mot de passe donné : retenu au gestionnaire d'identifiants avec
    /// « Retenir le mot de passe », sinon gardé le temps du lancement (et effacé
    /// du gestionnaire).
    fn keep_password(&self, f: &Friend, password: &str, remember: bool) -> Result<(), String> {
        if remember {
            if !store::set_password(&f.credential(), &f.login, password) {
                return Err(t("errors.password_store", &[]));
            }
            self.passwords.lock().unwrap().remove(&f.id);
        } else {
            store::delete_password(&f.credential());
            self.passwords.lock().unwrap().insert(f.id.clone(), password.to_string());
        }
        Ok(())
    }
}

fn lang() -> kybernow_core::i18n::Lang {
    i18n::lang()
}

// MARK: - Ordinateurs empruntés

/// Les cartes « Ordinateur de {hôte} » de l'accueil.
fn views(state: &UiState) -> Vec<FriendView> {
    let list = state.lend.with_friends(|l| l.clone());
    let seen = state.lend.seen.lock().unwrap();
    list.iter().map(|f| FriendView::new(f, seen.get(&f.id).unwrap_or(&Seen::Unknown))).collect()
}

pub fn emit_friends(app: &AppHandle) {
    let views = views(&app.state::<UiState>());
    let _ = app.emit_to("main", "lend-friends", views);
}

#[tauri::command(async)]
pub fn lend_friends(app: AppHandle) -> Vec<FriendView> {
    views(&app.state::<UiState>())
}

/// Demande l'état de l'ordinateur d'un ami (`lend::status`) ; vrai s'il a changé.
fn refresh_friend(state: &UiState, f: &Friend) -> bool {
    let seen = match state.lend.password(f) {
        None => Seen::NoPassword,
        Some(password) => match lend::status(&f.target(), f.route().as_ref(), &f.login, &password) {
            Ok(s) => {
                let learned = state.lend.with_friends(|l| l.iter_mut().find(|x| x.id == f.id).is_some_and(|x| x.learn(&s)));
                if learned {
                    state.lend.save();
                }
                Seen::Status(s)
            }
            Err(LendError::BadCredentials) => Seen::Refused,
            Err(LendError::Unsupported) => Seen::Unsupported,
            // Trop d'essais : rien de nouveau.
            Err(LendError::TooMany { .. }) => return false,
            Err(e) => {
                log::debug!("prêt : {} ne répond pas ({})", f.owner_name(), e.kind());
                Seen::Offline
            }
        },
    };
    let mut all = state.lend.seen.lock().unwrap();
    all.insert(f.id.clone(), seen.clone()) != Some(seen)
}

/// Les ordinateurs empruntés, toutes les 15 s avec l'accueil affiché, toutes les
/// 60 s sans lui, et tout de suite quand on le demande (`ask_soon`).
pub fn poll_friends(app: AppHandle) {
    loop {
        let state = app.state::<UiState>();
        let list = state.lend.with_friends(|l| l.clone());
        let mut changed = false;
        for f in &list {
            changed |= refresh_friend(&state, f);
        }
        if changed {
            emit_friends(&app);
        }
        let shown = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(true);
        for _ in 0..if shown { 15 } else { 60 } {
            std::thread::sleep(Duration::from_secs(1));
            if state.lend.asked.swap(false, Ordering::Relaxed) {
                break;
            }
        }
    }
}

fn ask_soon(state: &UiState) {
    state.lend.asked.store(true, Ordering::Relaxed);
}

/// Le lien d'invitation, pour l'écran d'ajout : l'identifiant du compte et le
/// prénom de l'hôte qu'il porte (`name=` : « {hôte} vous prête son
/// ordinateur. ») ; sans lui (lien d'avant), l'écran s'intitule « Avec un compte
/// d'ami ». Illisible : « Ce lien d'invitation est illisible. »
#[derive(Serialize)]
pub struct InviteView {
    login: String,
    host: Option<String>,
    relay: bool,
}

#[tauri::command(async)]
pub fn lend_invite(link: String) -> Result<InviteView, String> {
    let invite = lend::parse_invite(&link).ok_or_else(|| t("errors.lend_link", &[]))?;
    Ok(InviteView { login: invite.login.clone(), host: lending::invite_name(&invite), relay: invite.relay_route().is_some() })
}

/// « Ajouter » : l'ordinateur d'un ami, par son lien d'invitation (`link`) ou
/// trouvé par « Ajouter un ordinateur » (`candidate` : réseau, lien d'Admin,
/// adresse), avec l'identifiant et le mot de passe de son compte d'ami, vérifiés
/// par `lend::status` (« Identifiant ou mot de passe refusé. »). Rend son
/// identifiant dans l'app.
#[tauri::command(async)]
pub fn lend_add(app: AppHandle, link: Option<String>, candidate: Option<Candidate>, login: String, password: String, remember: bool) -> Result<String, String> {
    let state = app.state::<UiState>();
    let login = login.trim().to_string();
    let mut f = match (link.as_deref().map(str::trim).filter(|l| !l.is_empty()), candidate) {
        (Some(l), _) => Friend::from_invite(store::new_id(), &lend::parse_invite(l).ok_or_else(|| t("errors.lend_link", &[]))?),
        (None, Some(c)) => Friend {
            id: store::new_id(),
            host: c.host,
            port: c.port,
            fingerprint: c.fingerprint,
            login: String::new(),
            relay: None,
            sid: None,
            owner: String::new(),
            computer: c.name.unwrap_or_default(),
            remember: false,
        },
        (None, None) => return Err(t("errors.lend_link", &[])),
    };
    if !login.is_empty() {
        f.login = login;
    }
    if f.login.is_empty() || password.is_empty() {
        return Err(lend::LendError::BadCredentials.text(lang(), ""));
    }
    if system::is_this_pc(&f.host) {
        return Err(t("errors.this_pc", &[]));
    }
    f.remember = remember;
    let status = lend::status(&f.target(), f.route().as_ref(), &f.login, &password).map_err(|e| e.text(lang(), ""))?;
    f.learn(&status);
    let id = state.lend.with_friends(|l| lending::upsert(l, f.clone()));
    let f = Friend { id: id.clone(), ..f };
    state.lend.keep_password(&f, &password, remember)?;
    state.lend.save();
    state.lend.seen.lock().unwrap().insert(id.clone(), Seen::Status(status));
    log::info!("prêt : ordinateur de {} ajouté{}", f.owner_name(), if f.route().is_some() { " (avec le relais)" } else { "" });
    emit_friends(&app);
    Ok(id)
}

/// « Retirer » l'ordinateur d'un ami de cet appareil (et son mot de passe).
#[tauri::command(async)]
pub fn lend_remove(app: AppHandle, id: String) {
    let state = app.state::<UiState>();
    let removed = state.lend.with_friends(|l| {
        let before = l.len();
        l.retain(|f| f.id != id);
        before != l.len()
    });
    if removed {
        state.lend.save();
    }
    store::delete_password(&format!("lend-{id}"));
    state.lend.passwords.lock().unwrap().remove(&id);
    state.lend.seen.lock().unwrap().remove(&id);
    emit_friends(&app);
}

/// L'état de l'ordinateur d'un ami tout de suite (carte choisie sur l'accueil).
#[tauri::command(async)]
pub fn lend_refresh(app: AppHandle, id: String) {
    let state = app.state::<UiState>();
    if let Some(f) = state.lend.friend(&id) {
        refresh_friend(&state, &f);
    }
    emit_friends(&app);
}

// MARK: - Emprunter

/// « Rejoindre avec un code » est tombé sur un code de prêt : le code, la Station
/// et le chemin sont gardés, l'interface emprunte ensuite par `lend_borrow`
/// (`ticket`), avec le passage « Préparation de l'ordinateur ».
pub fn keep_ticket(state: &UiState, code: LendCode, name: String) -> u64 {
    let id = SEQ.fetch_add(1, Ordering::Relaxed);
    log::info!("partage : le code est un code de prêt{}", if code.route.is_some() { " (par le relais)" } else { "" });
    *state.lend.ticket.lock().unwrap() = Some(Ticket { id, code, name });
    id
}

/// Le prénom envoyé avec la demande : celui de « Rejoindre », sinon le nom de ce PC.
fn borrower_name(state: &UiState) -> String {
    let remembered = state.prefs.lock().unwrap().share_name.clone();
    crate::sharing::proposed_name(remembered.as_deref(), &system::device_info().name)
}

/// « Emprunter » (l'ordinateur d'un ami, `friend` ; `password` et `remember`
/// quand le mot de passe n'est pas retenu) ou la suite de « Rejoindre avec un
/// code » (`ticket`). Bloque le temps que la Station passe en mode console (30 s
/// au plus) : le passage montre alors « Préparation de l'ordinateur ». Rend
/// `ready` (la session à ouvrir, `lend_connect`), `pending` (« En attente de
/// {hôte}… », `lend_wait`), ou le refus en toutes lettres (texte du cœur).
#[tauri::command(async)]
pub fn lend_borrow(app: AppHandle, friend: Option<String>, ticket: Option<u64>, password: Option<String>, remember: Option<bool>) -> Result<BorrowOutcome, String> {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::Relaxed) {
        return Err(t("errors.session_not_ready", &[]));
    }
    if state.full() {
        return Err(t("errors.four_at_most", &[]));
    }
    let device = system::device_info();
    let pc = super::share::this_pc_of(&state);
    let caps = capabilities(&pc);
    let (target, route, credentials, name, host) = match (friend, ticket) {
        (Some(id), _) => {
            let mut f = state.lend.friend(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
            let given = password.filter(|p| !p.is_empty());
            let password = match given {
                Some(p) => {
                    let remember = remember.unwrap_or(f.remember);
                    if remember != f.remember {
                        f.remember = remember;
                        state.lend.with_friends(|l| l.iter_mut().find(|x| x.id == f.id).map(|x| x.remember = remember));
                        state.lend.save();
                    }
                    state.lend.keep_password(&f, &p, remember)?;
                    p
                }
                None => state.lend.password(&f).ok_or_else(|| t("errors.lend_password", &[]))?,
            };
            let host = f.owner_name().to_string();
            (f.target(), f.route(), LendCredentials::Account { login: f.login.clone(), password }, borrower_name(&state), host)
        }
        (None, Some(n)) => {
            let tk = {
                let mut tk = state.lend.ticket.lock().unwrap();
                match tk.take() {
                    Some(x) if x.id == n => x,
                    other => {
                        *tk = other;
                        return Err(t("errors.no_session", &[]));
                    }
                }
            };
            (tk.code.target, tk.code.route, LendCredentials::Code { code: tk.code.code }, tk.name, String::new())
        }
        (None, None) => return Err(t("errors.no_session", &[])),
    };
    let request = BorrowRequest { credentials, name, device: device.id, app: "windows".into(), screen: system::screen_mode(), codecs: lending::codecs(caps.hevc, caps.av1) };
    let again = Again { target: target.clone(), route: route.clone(), request: request.clone() };
    let answer = lend::borrow(&target, route.as_ref(), &request).map_err(|e| {
        log::info!("prêt : refusé ({})", e.kind());
        e.text(lang(), &host)
    })?;
    ask_soon(&state);
    Ok(outcome(&state, answer, again))
}

fn outcome(state: &UiState, answer: Borrowing, again: Again) -> BorrowOutcome {
    match answer {
        Borrowing::Ready(loan) => BorrowOutcome::Ready(open_loan(state, loan, again)),
        Borrowing::Pending(pending) => {
            let id = SEQ.fetch_add(1, Ordering::Relaxed);
            let host = pending.host.clone();
            log::info!("prêt : demande en attente de l'accord de l'hôte (n°{id})");
            let old = state.lend.wait.lock().unwrap().replace(Wait { id, pending, again, cancelled: Arc::new(AtomicBool::new(false)) });
            if let Some(old) = old {
                old.cancelled.store(true, Ordering::Relaxed);
                std::thread::spawn(move || lend::cancel(&old.pending));
            }
            BorrowOutcome::Pending { wait: id, host }
        }
    }
}

/// Le prêt accordé devient la session à ouvrir (`lend_connect`) ; la demande est
/// gardée pour « Revenir ».
fn open_loan(state: &UiState, loan: Loan, again: Again) -> LoanReady {
    let host = loan.borrowed.host_name().to_string();
    let title = t("lend.computer_of", &[("host", &host)]);
    let session = window::new_app_id();
    let id = format!("lend-{session}");
    log::info!(
        "prêt : accordé, chez {} ({}){}",
        loan.borrowed.computer,
        loan.borrowed.until.map_or("sans fin".to_string(), |u| format!("jusqu'à {u}")),
        if loan.borrowed.route.is_some() { ", par le relais" } else { "" }
    );
    let ready = LoanReady { id: id.clone(), title, host, fingerprint: loan.target.fingerprint.clone() };
    *state.lend.last.lock().unwrap() = Some(again);
    *state.lend.pending.lock().unwrap() = Some(Pending { id, session, loan });
    ready
}

/// « En attente de {hôte}… » : un appel tenu 30 s au plus que l'interface
/// rappelle (`waiting`) jusqu'à la réponse : accepté (`ready`), ou le refus en
/// toutes lettres (« {hôte} a refusé. », « {hôte} n'a pas répondu. »).
#[tauri::command(async)]
pub fn lend_wait(app: AppHandle, wait: u64) -> Result<LendWaitOutcome, String> {
    let state = app.state::<UiState>();
    let (pending, again, cancelled) = match state.lend.wait.lock().unwrap().as_ref() {
        Some(w) if w.id == wait => (w.pending.clone(), w.again.clone(), w.cancelled.clone()),
        _ => return Ok(LendWaitOutcome::Cancelled),
    };
    let answer = lend::wait(&pending);
    if cancelled.load(Ordering::Relaxed) {
        // « Annuler » pendant l'attente : un accord arrivé entre-temps est rendu.
        if let Ok(Some(loan)) = answer {
            std::thread::spawn(move || {
                let _ = lend::leave(&loan);
            });
        }
        return Ok(LendWaitOutcome::Cancelled);
    }
    let finished = |state: &UiState| {
        let mut w = state.lend.wait.lock().unwrap();
        if w.as_ref().is_some_and(|w| w.id == wait) {
            w.take();
        }
    };
    match answer {
        Ok(None) => Ok(LendWaitOutcome::Waiting),
        Ok(Some(loan)) => {
            finished(&state);
            Ok(LendWaitOutcome::Ready(open_loan(&state, loan, again)))
        }
        Err(e) => {
            finished(&state);
            log::info!("prêt : demande finie ({})", e.kind());
            Err(e.text(lang(), &pending.host))
        }
    }
}

/// « Annuler » : la demande en attente est retirée chez l'hôte.
#[tauri::command(async)]
pub fn lend_cancel(app: AppHandle, wait: u64) {
    cancel_wait(&app.state::<UiState>(), Some(wait));
}

/// Retire la demande en attente (celle-ci, ou n'importe laquelle : l'app se ferme).
pub fn cancel_wait(state: &UiState, only: Option<u64>) {
    let taken = {
        let mut w = state.lend.wait.lock().unwrap();
        if w.as_ref().is_some_and(|w| only.is_none_or(|id| w.id == id)) { w.take() } else { None }
    };
    if let Some(w) = taken {
        log::info!("prêt : demande annulée (n°{})", w.id);
        w.cancelled.store(true, Ordering::Relaxed);
        lend::cancel(&w.pending);
    }
}

/// « Revenir » (carte de fin, fenêtre fermée ou réseau perdu) : la même demande,
/// pendant 15 min ; la Station rend le même prêt.
#[tauri::command(async)]
pub fn lend_come_back(app: AppHandle) -> Result<BorrowOutcome, String> {
    let state = app.state::<UiState>();
    if state.full() {
        return Err(t("errors.four_at_most", &[]));
    }
    let again = state.lend.last.lock().unwrap().clone().ok_or_else(|| t("errors.no_session", &[]))?;
    log::info!("prêt : retour demandé");
    let answer = lend::borrow(&again.target, again.route.as_ref(), &again.request).map_err(|e| e.text(lang(), ""))?;
    Ok(outcome(&state, answer, again))
}

/// La session d'emprunt du prêt accordé : la première s'ouvre comme celle d'un
/// ordinateur (passage de l'accueil, plein écran) ; pendant une autre session,
/// dans une fenêtre avec sa fenêtre de connexion. Un seul écran : l'écran virtuel
/// que la Station a fait à la taille de ce PC.
#[tauri::command(async)]
pub fn lend_connect(app: AppHandle, id: String) -> Result<ConnectInfo, String> {
    let state = app.state::<UiState>();
    let pending = {
        let mut p = state.lend.pending.lock().unwrap();
        match p.take() {
            Some(x) if x.id == id => x,
            other => {
                *p = other;
                return Err(t("errors.no_session", &[]));
            }
        }
    };
    // L'app se ferme : rien ne démarre (le prêt reste, l'ami peut revenir).
    if state.quitting.load(Ordering::Relaxed) {
        return Ok(ConnectInfo { screens: 0, windowed: true });
    }
    let first = {
        let sessions = state.sessions.lock().unwrap();
        let mut launching = state.launching.lock().unwrap();
        if sessions.len() + launching.len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS {
            return Err(t("errors.four_at_most", &[]));
        }
        let first = sessions.is_empty() && launching.is_empty();
        launching.push(id.clone());
        first
    };
    let _launching = Launching { app: &app, id: id.clone() };
    let Pending { session, loan, .. } = pending;
    let host_name = loan.borrowed.host_name().to_string();
    let title = t("lend.computer_of", &[("host", &host_name)]);
    let borrow = Arc::new(Borrow { host: host_name, title: title.clone(), until: loan.borrowed.until, notice: Mutex::new(None), end: Mutex::new(None) });

    let pc = super::share::this_pc_of(&state);
    let caps = capabilities(&pc);
    // Le profil Jeu (docs/pret.md) ; rien n'est changé sur l'ordinateur prêté.
    let settings = StreamSettings::for_profile(Profile::Jeu, caps);
    let params = SessionParams::borrower(&loan, system::device_info(), settings.clone(), audio::default_output_channels());
    let d = loan.borrowed.display;
    let mode = if d.width > 0 && d.height > 0 { Mode { width: d.width, height: d.height, refresh: d.refresh.max(1) } } else { system::screen_mode() };
    let spec = Spec {
        setting: DisplaysSetting::This,
        limit_4k: false,
        custom_resolution: false,
        max_screens: 1,
        lighten: false,
        virtual_display_available: false,
        capabilities: Some(caps),
    };
    let (layout, follow) = if first {
        (Layout::now(spec), None)
    } else {
        let (layout, follow) = follow_layout(&state, spec);
        (layout, Some(follow))
    };
    log::info!("session d'emprunt chez {} : {}×{} à {} Hz", loan.borrowed.computer, mode.width, mode.height, mode.refresh);
    let bar = Arc::new(Mutex::new(BarLabel { label: String::new(), profile: String::new() }));
    let sent = Arc::new(Mutex::new(Sent { settings, mode, screens: 1, pixel_rate: 0.0, virtual_display: false }));
    let stats = Arc::new(Mutex::new(None));
    let spot = Arc::new(Mutex::new(Spot::default()));
    let share = Arc::new(Mutex::new(None));
    let lend_view = Arc::new(Mutex::new(None));
    let route = Arc::new(Mutex::new(None));
    let spec = layout.spec;
    let this_screen = layout.this_screen.clone();
    let connexion = Arc::new(Connexion { session, name: title.clone(), ..Default::default() });
    {
        let mut view = connexion.view.lock().unwrap();
        view.id = id.clone();
        view.phase = "preparing";
        view.title = Some(title.clone());
        view.fingerprint = loan.target.fingerprint.clone();
        // La raison d'un échec se lit telle quelle, comme pour un invité.
        view.guest = true;
        view.borrow = true;
    }
    if let Some(f) = &follow {
        open_connexion(&app, &connexion, f.at, (f.client.0 as u32, f.client.1 as u32), &title);
    }
    let mut running = state.sessions.lock().unwrap();
    let host = SessionHost::start_as(
        session,
        title.clone(),
        params,
        HostOptions {
            vsync: store::Settings::default().vsync,
            hardware: true,
            fullscreen: first,
            mouse: None,
            window_size: match &follow {
                Some(f) => f.client,
                None => system::initial_window_size(mode),
            },
            at: follow.as_ref().map(|f| f.at),
            title_stats: false,
            layout,
        },
        Arc::new(UiEvents {
            app: app.clone(),
            id: id.clone(),
            sent: sent.clone(),
            stats: stats.clone(),
            spec,
            this_screen,
            spot,
            first,
            connexion,
            guest: None,
            borrow: Some(borrow.clone()),
            share: share.clone(),
            lend: lend_view.clone(),
            lend_seen: AtomicBool::new(false),
            route: route.clone(),
        }),
    );
    running.push(Running { id: id.clone(), name: title, host, bar, sent, stats, guest: None, borrow: Some(borrow), share, lend: lend_view, route });
    drop(running);
    drop(_launching);
    if state.quitting.load(Ordering::Relaxed) {
        end_instance(&state, session, "Quitter pendant le lancement");
    }
    emit_sessions(&app);
    Ok(ConnectInfo { screens: 1, windowed: !first })
}

/// « Rendre l'ordinateur » (après sa confirmation, dans la barre) : le prêt finit
/// pour de bon, puis la session, sans carte de retour.
#[tauri::command(async)]
pub fn lend_give_back(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    let is_borrow = state.with_session(&id, |r| r.borrow.is_some()).unwrap_or(false);
    if !is_borrow {
        return Err(t("errors.no_session", &[]));
    }
    log::info!("[{id}] « Rendre l'ordinateur »");
    if state.with_session(&id, |r| r.host.give_back()).unwrap_or(false) { Ok(()) } else { Err(t("errors.session_not_ready", &[])) }
}

// MARK: - L'hôte

/// Ce que « Prêter l'ordinateur… » peut faire avec cet ordinateur (`info.lend`).
fn availability(app: &AppHandle, id: &str) -> &'static str {
    let state = app.state::<UiState>();
    let live = state.live.lock().unwrap();
    let info = live.get(id).and_then(|l| l.info.as_ref());
    lending::availability(info.is_some(), info.and_then(|i| i.lend))
}

/// Le texte d'un préavis de créneau chez l'hôte (« Créneau de {prénom} :
/// l'ordinateur lui est prêté dans 2 min. »).
fn slot_text(view: &LendView) -> impl Fn(&kybernow_core::lend::LendNotice) -> String + '_ {
    move |n| n.text(lang(), &lending::slot_name(view))
}

/// Le panneau de l'hôte de la session (ou de l'ordinateur) `id`.
pub fn panel(app: &AppHandle, id: &str, view: Option<&LendView>) -> LendPanel {
    let a = availability(app, id);
    match view {
        Some(v) => LendPanel::new(Some(v), a, slot_text(v)),
        None => LendPanel::new(None, a, |_| String::new()),
    }
}

/// `lend-state` : le panneau de l'hôte de la session `id`.
#[derive(Clone, Serialize)]
struct LendEvent {
    id: String,
    lend: LendPanel,
}

/// Le prêt de l'ordinateur `id` a changé : la barre (s'il est en session) et
/// l'accueil (« Prêté à {prénom} ») en sont prévenus.
pub fn publish(app: &AppHandle, id: &str, view: Option<LendView>) -> LendPanel {
    let state = app.state::<UiState>();
    if let Some(cell) = state.with_session(id, |r| r.lend.clone()) {
        *cell.lock().unwrap() = view.clone();
    }
    let home_changed = {
        let mut live = state.live.lock().unwrap();
        match live.get_mut(id) {
            Some(l) if l.lend != view => {
                l.lend = view.clone();
                true
            }
            _ => false,
        }
    };
    let lend = panel(app, id, view.as_ref());
    let _ = app.emit_to("bar", "lend-state", LendEvent { id: id.to_string(), lend: lend.clone() });
    if home_changed {
        emit_computers(app);
    }
    lend
}

/// Admin de l'ordinateur d'une session d'hôte : jamais pour un invité ni pour
/// l'ami qui emprunte.
fn host_console(app: &AppHandle, id: &str) -> Result<Console, String> {
    let state = app.state::<UiState>();
    if state.with_session(id, |r| r.guest.is_some() || r.borrow.is_some()).unwrap_or(false) || state.with_store(|s| s.get(id).is_none()) {
        return Err(t("errors.no_session", &[]));
    }
    console_of(app, id)
}

fn lift<T>(r: Result<T, kybernow_core::console::ConsoleError>) -> Result<T, String> {
    r.map_err(|e| e.text(lang()))
}

/// « Prêter l'ordinateur… » s'ouvre : ce que l'ordinateur dit de lui (`info.lend`)
/// et le prêt tel qu'Admin le dit. Prêt coupé ou Station trop ancienne : le
/// panneau le dit (pas d'erreur).
#[tauri::command(async)]
pub fn lend_state(app: AppHandle, id: String) -> Result<LendPanel, String> {
    let console = host_console(&app, &id)?;
    let info = lift(console.info())?;
    let usable = info.lend.is_some_and(|l| l.available && l.enabled);
    app.state::<UiState>().live.lock().unwrap().entry(id.clone()).or_default().info = Some(info);
    if !usable {
        return Ok(publish(&app, &id, None));
    }
    let view = lift(console.lend_state())?;
    Ok(publish(&app, &id, Some(view)))
}

/// « Créer un code » : la durée (`duration` en secondes, `None` : « Jusqu'à ce
/// que je reprenne ») et le compte Steam (`friend`, `host`).
#[tauri::command(async)]
pub fn lend_code(app: AppHandle, id: String, duration: Option<u64>, steam: String) -> Result<LendPanel, String> {
    let console = host_console(&app, &id)?;
    let view = lift(console.lend_code(duration, lending::parse_steam(&steam)))?;
    log::info!("[{id}] code de prêt créé ({})", duration.map_or("sans fin".to_string(), |d| format!("{} min", d / 60)));
    Ok(publish(&app, &id, Some(view)))
}

/// « Annuler le code ».
#[tauri::command(async)]
pub fn lend_cancel_code(app: AppHandle, id: String) -> Result<LendPanel, String> {
    let console = host_console(&app, &id)?;
    let view = lift(console.lend_cancel_code())?;
    log::info!("[{id}] code de prêt annulé");
    Ok(publish(&app, &id, Some(view)))
}

/// « Accepter » (avec le compte Steam) ou « Refuser » une demande d'emprunt.
/// Accepter depuis sa session la ferme (« Votre session va se fermer. »).
#[tauri::command(async)]
pub fn lend_answer(app: AppHandle, id: String, request: String, accept: bool, steam: String) -> Result<LendPanel, String> {
    let console = host_console(&app, &id)?;
    let view = lift(console.lend_answer(&request, accept, lending::parse_steam(&steam)))?;
    log::info!("[{id}] demande d'emprunt {}", if accept { "acceptée" } else { "refusée" });
    Ok(publish(&app, &id, Some(view)))
}

/// « Reprendre l'ordinateur » (préavis de 2 min, `now` faux) ou « Reprendre tout
/// de suite », depuis l'accueil de cet ordinateur.
#[tauri::command(async)]
pub fn lend_take_back(app: AppHandle, id: String, now: bool) -> Result<LendPanel, String> {
    let console = host_console(&app, &id)?;
    let view = lift(console.lend_take_back(now))?;
    log::info!("[{id}] reprise de l'ordinateur{}", if now { " tout de suite" } else { " (préavis de 2 min)" });
    Ok(publish(&app, &id, Some(view)))
}

/// Le relevé d'un ordinateur (`refresh`) : le prêt en cours, quand le prêt y est
/// activé (`GET /api/lend` par le compte de l'appareil) ; `None` sinon.
pub fn home_view(console: &Console, info: &kybernow_core::console::Info) -> Option<LendView> {
    let l = info.lend?;
    if !(l.available && l.enabled) {
        return None;
    }
    console.lend_state().ok()
}

/// `lend-borrow` : la barre de la session d'emprunt `id` (préavis arrivé).
#[derive(Clone, Serialize)]
pub struct BorrowEvent {
    pub id: String,
    pub borrow: BorrowBar,
}

// MARK: - Événements des sessions

impl UiEvents {
    /// Session d'hôte : la vue du prêt (signe de vie). Le panneau suit ; une
    /// demande qui arrive est dite (« {prénom} veut emprunter l'ordinateur. », le
    /// choisir ouvre le panneau), comme le préavis d'un créneau.
    pub(super) fn on_lend_view(&self, view: Option<&LendView>) {
        if self.guest.is_some() || self.borrow.is_some() {
            return;
        }
        let after = view.cloned();
        let before = std::mem::replace(&mut *self.lend.lock().unwrap(), after.clone());
        let known = self.lend_seen.swap(true, Ordering::Relaxed);
        publish(&self.app, &self.id, after.clone());
        let arrived = lending::new_requests(before.as_ref(), after.as_ref(), known);
        // Pendant un créneau, la demande dit le texte du créneau (et le préavis ne le redit pas).
        let slot_said = arrived.first().is_some_and(|r| r.kind == kybernow_core::lend::LendKind::Slot);
        if let Some(r) = arrived.first() {
            log::info!("[{}] {} veut emprunter l'ordinateur{}", self.connexion.name, r.name, if slot_said { " (créneau)" } else { "" });
            let text = if slot_said {
                kybernow_core::lend::LendNotice { reason: kybernow_core::lend::LendNoticeReason::SlotStart, at: 0 }.text(lang(), &r.name)
            } else {
                t("lend.notice_request", &[("name", &r.name)])
            };
            super::share::panel_notice(&self.app, &self.id, &self.connexion.name, &text, "lend");
        }
        if let (Some(n), Some(v), false) = (lending::new_slot_notice(before.as_ref(), after.as_ref()), after.as_ref(), slot_said) {
            super::share::notice(&self.app, &self.id, &self.connexion.name, &n.text(lang(), &lending::slot_name(v)));
        }
    }

    /// Session d'emprunt : le préavis de 2 minutes (barre ou notification).
    pub(super) fn on_lend_notice(&self, notice: &kybernow_core::lend::LendNotice, text: &str) {
        let Some(b) = &self.borrow else { return };
        log::info!("[{}] préavis du prêt : {:?}", b.title, notice.reason);
        *b.notice.lock().unwrap() = Some(NoticeView::new(notice, text.to_string()));
        let _ = self.app.emit_to("bar", "lend-borrow", BorrowEvent { id: self.id.clone(), borrow: b.bar() });
        super::share::notice(&self.app, &self.id, &b.title, text);
    }

    /// Session d'emprunt : la fin du prêt, gardée pour la carte de fin.
    pub(super) fn on_lend_end(&self, end: LendEnd, text: &str) {
        let Some(b) = &self.borrow else { return };
        log::info!("[{}] fin du prêt ({end:?})", b.title);
        *b.end.lock().unwrap() = Some((end, text.to_string()));
    }
}
