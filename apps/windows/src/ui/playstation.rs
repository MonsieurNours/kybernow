//! Consoles PlayStation dans l'app (docs/playstation.md ; textes :
//! docs/link-ui.md, « Consoles PlayStation ») : la recherche des consoles
//! tant que l'accueil est à l'écran, les comptes PlayStation Network (la
//! connexion de Sony dans sa propre fenêtre WebView2, ou la saisie à la
//! main), l'association par le code, la partie (réveil, hors de chez soi par
//! une Station et le relais, fenêtre de la partie : `ps_session.rs`), sa
//! barre (la même que celle d'une session d'ordinateur, réduite à ce qui a du
//! sens pour une console), les réglages de chaque console, et l'envoi des
//! comptes et des associations entre appareils (« Envoyer à un appareil »,
//! « Recevoir d'un autre appareil » : `playstation::handoff` du cœur).
//!
//! Le modèle (comptes, associations, état, paquet d'envoi) est dans
//! `consoles.rs`, sans rien de Windows ; les secrets vont au gestionnaire
//! d'identifiants (`KyberNow:ps-…`).

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use kybernow_core::input::PadKey;
use kybernow_core::playstation::discovery::{self, Model};
use kybernow_core::playstation::endpoint::Endpoint;
use kybernow_core::playstation::handoff;
use kybernow_core::playstation::keymouse::Layout;
use kybernow_core::playstation::play::{PlayEnd, PlayStats};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, RegistError, Registered, TargetName};
use kybernow_core::playstation::remote::{Remote, RemoteError};
use kybernow_core::playstation::settings::PlaySettings;
use kybernow_core::relay::RouteInfo;

use super::{UiState, console_via, emit_sessions, show_action, show_home_if_idle};
use crate::consoles::{self, Bundle, BundleStation, Consoles, Found, Secrets, StateView, Status};
use crate::i18n::t;
use crate::ps_session::{self, BarState, Command, PsSession};
use crate::{i18n, relais, store};

/// Ce que l'app tient des consoles (dans `UiState`).
#[derive(Default)]
pub struct PsState {
    consoles: Mutex<Consoles>,
    loaded: OnceLock<()>,
    /// Les parties en cours (une par console).
    sessions: Mutex<Vec<Arc<PsSession>>>,
    /// Pertes depuis le début de chaque partie (statistiques de la barre).
    totals: Mutex<std::collections::HashMap<String, (u64, u64)>>,
    /// Dernier `GET /api/consoles` de chaque Station (une fois par minute au plus).
    learned: Mutex<std::collections::HashMap<String, Instant>>,
    /// L'attente d'un envoi (« Recevoir d'un autre appareil ») : tenue par son fil.
    receiving: Mutex<Option<Receiving>>,
    /// La barre est pilotée par la manette (ouverte par la combinaison).
    pad_mode: AtomicBool,
    /// Les parties qui se lancent (réveil, relais), avant leur fenêtre ; et
    /// celles dont le lancement est annulé (« Annuler » du passage).
    launching: Mutex<Vec<String>>,
    cancelled: Mutex<Vec<String>>,
}

/// L'attente d'un envoi en cours : son fil la lâche quand `stop` passe à vrai.
struct Receiving {
    stop: Arc<AtomicBool>,
    port: u16,
}

/// Le gestionnaire d'identifiants de Windows (`KyberNow:ps-…`).
struct WinSecrets;

impl Secrets for WinSecrets {
    fn get(&self, key: &str) -> Option<String> {
        store::password(key)
    }
    fn set(&mut self, key: &str, value: &str) -> bool {
        store::set_password(key, "KyberNow", value)
    }
    fn delete(&mut self, key: &str) {
        store::delete_password(key)
    }
}

fn file_path() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow").join("playstation.json")
}

fn ps(app: &AppHandle) -> &PsState {
    &app.state::<UiState>().inner().ps
}

/// Au lancement : le fichier, les secrets relus, la recherche.
pub fn start(app: &AppHandle) {
    let p = ps(app);
    p.loaded.get_or_init(|| {
        let mut c = Consoles::load(&file_path());
        if c.check_secrets(&WinSecrets) {
            log::info!("PlayStation : associations sans secret écartées");
            c.save(&file_path());
        }
        log::info!("PlayStation : {} compte(s), {} console(s)", c.file.accounts.len(), c.file.consoles.len());
        *p.consoles.lock().unwrap() = c;
    });
    let a = app.clone();
    let _ = std::thread::Builder::new().name("ps-recherche".into()).spawn(move || search_loop(a));
}

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Consoles) -> T) -> T {
    f(&mut ps(app).consoles.lock().unwrap())
}

fn save(app: &AppHandle) {
    with(app, |c| c.save(&file_path()));
}

impl PsState {
    /// Une partie au moins est en cours.
    pub fn playing(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }
}

/// Les identifiants des parties en cours.
pub fn session_ids(app: &AppHandle) -> Vec<String> {
    ps(app).sessions.lock().unwrap().iter().map(|s| s.id().to_string()).collect()
}

fn session(app: &AppHandle, id: &str) -> Option<Arc<PsSession>> {
    ps(app).sessions.lock().unwrap().iter().find(|s| s.id() == id).cloned()
}

/// Une partie au moins est en cours.
pub fn playing(app: &AppHandle) -> bool {
    !ps(app).sessions.lock().unwrap().is_empty()
}

/// Les Stations joignables par le relais en ce moment (« Hors de chez vous »).
fn away_station(app: &AppHandle, station: &str) -> bool {
    if relais::off() {
        return false;
    }
    let state = app.state::<UiState>();
    let away = state.live.lock().unwrap().get(station).is_some_and(|l| l.away);
    away && state.with_store(|s| s.get(station).is_some_and(|c| relais::route_of(c.relay.as_ref(), c.fingerprint.as_deref()).is_some()))
}

fn view(app: &AppHandle) -> StateView {
    let sessions = session_ids(app);
    let away = |s: &str| away_station(app, s);
    let c = ps(app).consoles.lock().unwrap();
    c.view(Instant::now(), &away, &sessions)
}

pub fn emit_state(app: &AppHandle) {
    let v = view(app);
    let _ = app.emit_to("main", "ps-state", v);
}

fn found_of(c: &discovery::Console) -> Found {
    Found {
        host_id: c.host_id.to_ascii_uppercase(),
        name: c.name.clone(),
        model: c.model().name().to_string(),
        address: c.addr.to_string(),
        state: match c.state {
            discovery::State::Ready => "ready",
            discovery::State::Standby => "standby",
            discovery::State::Unknown => "unknown",
        }
        .into(),
        target: serde_json::to_value(TargetName::from_target(c.target())).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        running_app: c.running_app.clone(),
    }
}

/// Une recherche (diffusion, et en direct vers les adresses connues).
fn search_once(app: &AppHandle, wait: Duration) {
    let direct: Vec<Ipv4Addr> = with(app, |c| c.direct()).iter().filter_map(|a| a.parse().ok()).collect();
    let list: Vec<Found> = discovery::search(wait, &direct).unwrap_or_default().iter().map(found_of).collect();
    let changed = with(app, |c| c.record(list, Instant::now()));
    if changed {
        save(app);
    }
    emit_state(app);
}

/// Toutes les 3 s tant que l'accueil est à l'écran et qu'aucune partie ne
/// tourne (la console répond à la découverte sur le réseau du flux : une
/// image perdue toutes les 15 s pendant la recherche, essai du 03/11/2026),
/// ou qu'une console se réveille.
fn search_loop(app: AppHandle) {
    loop {
        let shown = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
        let waking = with(&app, |c| !c.waking.is_empty());
        if (shown && !playing(&app)) || waking {
            search_once(&app, Duration::from_millis(2000));
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Une Station sur le réseau (relevé des ordinateurs) : les consoles qu'elle
/// voit, une fois par minute au plus ; l'app retient, par console, les
/// Stations qui la voient (hors de chez soi).
pub fn learn(app: &AppHandle, station: &str, console: &kybernow_core::console::Console) {
    {
        let mut learned = ps(app).learned.lock().unwrap();
        if learned.get(station).is_some_and(|t| t.elapsed() < Duration::from_secs(60)) {
            return;
        }
        learned.insert(station.to_string(), Instant::now());
    }
    let Ok(list) = console.consoles() else { return };
    let ids: Vec<String> = list.iter().map(|c| c.host_id.clone()).collect();
    if with(app, |c| c.learn(station, &ids)) {
        log::info!("PlayStation : une Station voit {} console(s)", ids.len());
        save(app);
        emit_state(app);
    }
}

/// Un ordinateur retiré de l'app ne voit plus de console.
pub fn forget_station(app: &AppHandle, station: &str) {
    with(app, |c| c.forget_station(station));
    save(app);
}

// MARK: - Commandes : état et comptes

#[tauri::command]
pub fn ps_state(app: AppHandle) -> StateView {
    view(&app)
}

#[tauri::command(async)]
pub fn ps_search_now(app: AppHandle) -> StateView {
    search_once(&app, Duration::from_millis(2000));
    view(&app)
}

#[derive(Serialize)]
pub struct KeptAccount {
    account: consoles::AccountView,
    existed: bool,
}

fn kept_account(app: &AppHandle, psn_id: &str, name: Option<&str>) -> Result<KeptAccount, String> {
    let kept = with(app, |c| c.keep_account(&mut WinSecrets, psn_id, name));
    let Some((a, existed)) = kept else { return Err(t("errors.password_store", &[])) };
    save(app);
    emit_state(app);
    let consoles = with(app, |c| c.consoles_of(&a.id));
    log::info!("PlayStation : compte {}", if existed { "déjà là, mis à jour" } else { "gardé" });
    Ok(KeptAccount { account: consoles::AccountView { id: a.id, name: a.name, consoles }, existed })
}

/// « Se connecter à PlayStation Network » : la page de Sony dans sa propre
/// fenêtre (WebView2, rien de gardé une fois fermée). Son adresse de retour
/// n'est jamais chargée : l'identifiant et le pseudo en sont tirés (deux
/// requêtes HTTPS), puis le compte est gardé. `None` : fenêtre fermée sans
/// connexion.
#[tauri::command(async)]
pub fn ps_login(app: AppHandle) -> Result<Option<KeptAccount>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    // Déjà ouverte (deuxième clic) : elle passe devant, la première attente la garde.
    if let Some(old) = app.get_webview_window("psn-login") {
        let _ = old.set_focus();
        return Ok(None);
    }
    let url: tauri::Url = psn::LOGIN_URL.parse().map_err(|_| t("ps.account_unread", &[]))?;
    let caught = Arc::new(AtomicBool::new(false));
    let built = {
        let (tx_nav, caught) = (tx.clone(), caught.clone());
        let tx_close = tx.clone();
        let (w, h) = (520.0, 720.0);
        let builder = WebviewWindowBuilder::new(&app, "psn-login", WebviewUrl::External(url))
            .title(t("ps.sign_in_title", &[]))
            .inner_size(w, h)
            .center()
            .incognito(true)
            .on_navigation(move |u| {
                if !psn::is_redirect(u.as_str()) {
                    return true;
                }
                if !caught.swap(true, Ordering::SeqCst) {
                    let _ = tx_nav.send(Some(u.to_string()));
                }
                false
            });
        let built = builder.build();
        if let Ok(win) = &built {
            win.on_window_event(move |e| {
                if matches!(e, tauri::WindowEvent::Destroyed) {
                    let _ = tx_close.send(None);
                }
            });
        }
        built
    };
    let win = built.map_err(|e| {
        log::warn!("PlayStation : fenêtre de connexion de Sony impossible ({e})");
        t("ps.account_unread", &[])
    })?;
    drop(tx);
    let redirect = rx.recv().ok().flatten();
    let _ = win.destroy();
    let Some(redirect) = redirect else {
        log::info!("PlayStation : connexion de Sony fermée sans compte");
        return Ok(None);
    };
    let Some(code) = psn::code_from_redirect(&redirect) else {
        log::warn!("PlayStation : adresse de retour de Sony sans code");
        return Err(t("ps.account_unread", &[]));
    };
    match psn::account(&code) {
        Ok(a) => {
            let Some(id) = a.id.to_base64() else { return Err(t("ps.account_unread", &[])) };
            kept_account(&app, &id, a.online_id.as_deref()).map(Some)
        }
        Err(e) => {
            log::warn!("PlayStation : identifiant du compte non lu ({e})");
            Err(t("ps.account_unread", &[]))
        }
    }
}

/// « Saisir l'identifiant à la main ».
#[tauri::command]
pub fn ps_account_manual(app: AppHandle, psn_id: String, name: String) -> Result<KeptAccount, String> {
    let Some(id) = consoles::valid_psn_id(&psn_id) else { return Err(t("ps.bad_account_id", &[])) };
    kept_account(&app, &id, Some(&name))
}

#[tauri::command]
pub fn ps_account_rename(app: AppHandle, account: String, name: String) {
    if with(&app, |c| c.rename_account(&account, &name)) {
        save(&app);
        emit_state(&app);
    }
}

#[tauri::command]
pub fn ps_account_remove(app: AppHandle, account: String) {
    with(&app, |c| c.remove_account(&mut WinSecrets, &account));
    log::info!("PlayStation : compte retiré");
    save(&app);
    emit_state(&app);
}

#[tauri::command]
pub fn ps_choose_account(app: AppHandle, id: String, account: String) {
    let Some(host) = consoles::host_of(&id).map(str::to_string) else { return };
    with(&app, |c| {
        if let Some(k) = c.kept_mut(&host).filter(|k| k.accounts.contains(&account)) {
            k.account = Some(account.clone());
        }
    });
    save(&app);
    emit_state(&app);
}

// MARK: - Association

/// La cible à associer : celle de la recherche, sinon celle du modèle.
fn target_of(target: &str, ps5: bool) -> TargetName {
    serde_json::from_value(serde_json::Value::String(target.to_string()))
        .unwrap_or(if ps5 { TargetName::Ps5V1 } else { TargetName::Ps4V10 })
}

/// L'association par le code à 8 chiffres affiché par la console. Le compte
/// a déjà été gardé (même si l'association échoue ensuite).
#[tauri::command(async)]
pub fn ps_pair(app: AppHandle, host_id: String, address: String, target: String, account: String, pin: String) -> Result<String, String> {
    let Some(at) = Endpoint::parse(address.trim()) else { return Err(t("ps.unreachable", &[])) };
    let Some(psn_id) = WinSecrets.get(&consoles::account_secret(&account)).and_then(|s| PsnId::from_base64(&s)) else {
        return Err(t("ps.account_refused", &[]));
    };
    let digits: String = pin.chars().filter(char::is_ascii_digit).collect();
    let Ok(pin) = digits.parse::<u32>() else { return Err(t("ps.wrong_code", &[])) };
    let host_id = host_id.to_ascii_uppercase();
    let live = with(&app, |c| c.live(&host_id, Instant::now()).cloned());
    let kept = with(&app, |c| c.kept(&host_id).cloned());
    let model = live.as_ref().map(|f| f.model.clone()).or(kept.as_ref().map(|k| k.model.clone())).unwrap_or_else(|| "PS5".into());
    // Sans cible de la recherche (console gardée qui ne répond pas) : celle gardée.
    let target = if target.trim().is_empty() { kept.as_ref().map(|k| k.target_or_default()).unwrap_or_default() } else { target };
    let target = target_of(&target, model.eq_ignore_ascii_case("PS5"));
    log::info!("PlayStation : association demandée ({model})");
    let registered = regist::regist_at(&at, target.into(), &psn_id, pin).map_err(|e| {
        log::warn!("PlayStation : association refusée : {e}");
        match &e {
            RegistError::NoAnswer | RegistError::Refused => t("ps.no_answer", &[]),
            RegistError::Rejected { reason: Some(regist::REASON_REGIST_FAILED), .. } => t("ps.wrong_code", &[]),
            RegistError::Rejected { reason: Some(regist::REASON_INVALID_PSN_ID), .. } => t("ps.account_refused", &[]),
            _ => t("ps.unreachable", &[]),
        }
    })?;
    let json = serde_json::to_string(&registered).map_err(|e| e.to_string())?;
    let found = live.unwrap_or_else(|| Found {
        host_id: host_id.clone(),
        name: kept.as_ref().map(|k| k.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| registered.nickname.clone()),
        model: model.clone(),
        address: address.trim().to_string(),
        state: "unknown".into(),
        target: serde_json::to_value(target).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        running_app: None,
    });
    let mut found = found;
    if !address.trim().is_empty() && !address.trim().starts_with('{') {
        found.address = address.trim().to_string();
    }
    let host = registered.mac.to_ascii_uppercase();
    if !with(&app, |c| c.paired(&mut WinSecrets, &found, &host, &json, &account)) {
        return Err(t("errors.password_store", &[]));
    }
    log::info!("PlayStation : console associée");
    save(&app);
    emit_state(&app);
    Ok(consoles::console_id(&host))
}

/// « Par adresse » : une console à cette adresse (1,5 s, ports 9302 et 987).
/// Trouvée, l'adresse est retenue : la console y est cherchée en direct ensuite.
#[tauri::command(async)]
pub fn ps_probe_address(app: AppHandle, host: String) -> Option<Found> {
    let ip: Ipv4Addr = host.trim().parse().ok()?;
    let list = discovery::search_at(Duration::from_millis(1500), false, &[Endpoint::direct(ip)]).unwrap_or_default();
    let f = list.first().map(found_of)?;
    log::info!("PlayStation : une console répond à l'adresse saisie");
    with(&app, |c| c.remember(&ip.to_string(), f.clone(), Instant::now()));
    save(&app);
    emit_state(&app);
    Some(f)
}

// MARK: - Réglages

#[derive(Serialize)]
pub struct SettingsView {
    id: String,
    name: String,
    model: String,
    settings: PlaySettings,
    layout: Layout,
    /// Usage HID (en décimal) → nom de la touche dans la disposition de ce PC.
    key_names: std::collections::BTreeMap<String, String>,
}

/// Le nom d'une touche dans la disposition et la langue de ce PC.
fn key_name(usage: u16) -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyNameTextW;
    let lparam = crate::ps_keys::name_lparam(usage)?;
    let mut buf = [0u16; 64];
    let n = unsafe { GetKeyNameTextW(lparam, &mut buf) };
    (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
}

#[tauri::command]
pub fn ps_settings(app: AppHandle, id: String) -> Result<SettingsView, String> {
    let host = consoles::host_of(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    let k = with(&app, |c| c.kept(host).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    let key_names = crate::ps_keys::usages().filter_map(|u| key_name(u).map(|n| (u.to_string(), n))).collect();
    Ok(SettingsView { id, name: k.name.clone(), model: k.model.clone(), settings: k.settings(), layout: k.layout(), key_names })
}

#[tauri::command]
pub fn ps_save_settings(app: AppHandle, id: String, settings: PlaySettings) {
    let Some(host) = consoles::host_of(&id) else { return };
    let (rumble, triggers) = settings.intensities();
    with(&app, |c| {
        if let Some(k) = c.kept_mut(host) {
            k.settings = Some(settings);
        }
    });
    save(&app);
    emit_state(&app);
    // Les intensités de la manette suivent tout de suite dans une partie en cours.
    if let Some(s) = session(&app, &id) {
        s.set_intensities(rumble, triggers);
    }
}

#[tauri::command]
pub fn ps_save_layout(app: AppHandle, id: String, layout: Layout) {
    let Some(host) = consoles::host_of(&id) else { return };
    with(&app, |c| {
        if let Some(k) = c.kept_mut(host) {
            k.layout = Some(layout.clone());
        }
    });
    save(&app);
    if let Some(s) = session(&app, &id) {
        s.set_layout(layout);
    }
}

#[tauri::command]
pub fn ps_default_layout(app: AppHandle, id: String) -> Layout {
    if let Some(host) = consoles::host_of(&id) {
        with(&app, |c| {
            if let Some(k) = c.kept_mut(host) {
                k.layout = None;
            }
        });
        save(&app);
    }
    if let Some(s) = session(&app, &id) {
        s.set_layout(Layout::default());
    }
    Layout::default()
}

#[tauri::command]
pub fn ps_remove(app: AppHandle, id: String) {
    let Some(host) = consoles::host_of(&id) else { return };
    with(&app, |c| c.remove(&mut WinSecrets, host));
    log::info!("PlayStation : console retirée");
    save(&app);
    emit_state(&app);
}

/// Plein écran rejoint ou quitté pendant la partie : retenu pour la suivante.
fn remember_fullscreen(app: &AppHandle, host: &str, on: bool) {
    with(app, |c| {
        if let Some(k) = c.kept_mut(host) {
            let mut s = k.settings();
            s.fullscreen = on;
            k.settings = Some(s);
        }
    });
    save(app);
}

// MARK: - Partie

#[derive(Serialize)]
pub struct PlayInfo {
    windowed: bool,
}

#[derive(Clone, Serialize)]
struct PhaseEvent<'a> {
    id: &'a str,
    phase: &'static str,
    detail: Option<String>,
}

#[derive(Clone, Serialize)]
struct EndedEvent<'a> {
    id: &'a str,
    summary: Summary,
}

#[derive(Clone, Serialize)]
struct Summary {
    error: Option<String>,
    median_ms: Option<f64>,
    p95_ms: Option<f64>,
    seconds: u64,
}

fn phase(app: &AppHandle, id: &str, phase: &'static str) {
    let _ = app.emit("session-status", PhaseEvent { id, phase, detail: None });
}

fn ended_before(app: &AppHandle, id: &str, text: String) {
    log::info!("PlayStation : partie pas ouverte ({text})");
    let _ = app.emit("session-ended", EndedEvent { id, summary: Summary { error: Some(text), median_ms: None, p95_ms: None, seconds: 0 } });
}

/// Le texte d'une fin qui n'a pas été voulue ici.
fn end_text(end: &PlayEnd) -> Option<String> {
    match end {
        PlayEnd::Stopped => None,
        PlayEnd::InUse => Some(t("ps.end_in_use", &[])),
        PlayEnd::NotPaired => Some(t("ps.end_not_paired", &[])),
        PlayEnd::Unreachable(_) => Some(t("ps.end_unreachable", &[])),
        PlayEnd::Remote(_) => Some(t("ps.end_remote", &[])),
    }
}

/// La console se réveille : réveil envoyé, puis la console est cherchée à
/// `at` jusqu'à ce qu'elle se dise allumée (une minute au plus).
fn wake_and_wait(app: &AppHandle, id: &str, host: &str, at: &Endpoint, model: Model, registered: &Registered, broadcast_search: bool) -> bool {
    let Some(cred) = registered.wake_credential() else { return false };
    let sent = discovery::wake_at(at, model, cred).is_ok();
    log::info!("PlayStation : réveil {}", if sent { "envoyé" } else { "pas envoyé" });
    if !sent {
        return false;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if ps(app).cancelled.lock().unwrap().iter().any(|c| c == id) {
            return false;
        }
        if broadcast_search {
            search_once(app, Duration::from_millis(1500));
            if with(app, |c| c.live(host, Instant::now()).is_some_and(Found::ready)) {
                return true;
            }
        } else {
            let list = discovery::search_at(Duration::from_millis(1500), false, &[at.clone()]).unwrap_or_default();
            if list.iter().any(|c| c.host_id.eq_ignore_ascii_case(host) && c.state == discovery::State::Ready) {
                return true;
            }
        }
    }
    false
}

fn set_flag(app: &AppHandle, host: &str, waking: Option<bool>, reaching: Option<bool>) {
    with(app, |c| {
        if let Some(on) = waking {
            c.waking.retain(|h| h != host);
            if on {
                c.waking.push(host.to_string());
            }
        }
        if let Some(on) = reaching {
            c.reaching.retain(|h| h != host);
            if on {
                c.reaching.push(host.to_string());
            }
        }
    });
    emit_state(app);
}

/// « Jouer », « Réveiller et jouer », « Jouer » hors de chez soi. La partie
/// s'ouvre sur son propre fil ; la suite par `session-status`,
/// `session-shown`, `session-ended` (le passage de l'accueil).
#[tauri::command]
pub fn ps_play(app: AppHandle, id: String) -> Result<PlayInfo, String> {
    if let Some(s) = session(&app, &id) {
        s.command(Command::Focus);
        return Ok(PlayInfo { windowed: true });
    }
    let host = consoles::host_of(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?.to_ascii_uppercase();
    let (kept, chosen, live) = with(&app, |c| {
        let k = c.kept(&host).cloned();
        let chosen = k.as_ref().and_then(|k| c.chosen(k));
        (k, chosen, c.live(&host, Instant::now()).cloned())
    });
    let kept = kept.ok_or_else(|| t("errors.unknown_computer", &[]))?;
    let account = chosen.ok_or_else(|| t("ps.end_not_paired", &[]))?;
    let registered: Registered = WinSecrets
        .get(&consoles::pair_secret(&host, &account))
        .and_then(|j| serde_json::from_str(&j).ok())
        .ok_or_else(|| t("ps.end_not_paired", &[]))?;
    let away = live.is_none() && with(&app, |c| c.stations_of(&host).to_vec()).iter().any(|s| away_station(&app, s));
    {
        let mut launching = ps(&app).launching.lock().unwrap();
        if launching.contains(&id) {
            return Ok(PlayInfo { windowed: false });
        }
        launching.push(id.clone());
    }
    ps(&app).cancelled.lock().unwrap().retain(|c| c != &id);
    let (a, id2) = (app.clone(), id.clone());
    std::thread::Builder::new()
        .name("ps-lancement".into())
        .spawn(move || launch(a, id2, host, kept, account, registered, live, away))
        .map_err(|e| e.to_string())?;
    Ok(PlayInfo { windowed: false })
}

#[allow(clippy::too_many_arguments)]
fn launch(app: AppHandle, id: String, host: String, kept: consoles::Kept, account: String, registered: Registered, live: Option<Found>, away: bool) {
    launch_inner(&app, &id, host, kept, account, registered, live, away);
    ps(&app).launching.lock().unwrap().retain(|l| l != &id);
    ps(&app).cancelled.lock().unwrap().retain(|c| c != &id);
}

/// « Annuler » pendant le lancement : la partie ne s'ouvre pas, sans rien dire.
fn cancelled(app: &AppHandle, id: &str) -> bool {
    let c = ps(app).cancelled.lock().unwrap().iter().any(|c| c == id);
    if c {
        log::info!("PlayStation : lancement annulé");
        let _ = app.emit("session-ended", EndedEvent { id, summary: Summary { error: None, median_ms: None, p95_ms: None, seconds: 0 } });
    }
    c
}

#[allow(clippy::too_many_arguments)]
fn launch_inner(app: &AppHandle, id: &str, host: String, kept: consoles::Kept, account: String, registered: Registered, live: Option<Found>, away: bool) {
    let (app, id) = (app.clone(), id.to_string());
    let model = if kept.is_ps5() { Model::Ps5 } else { Model::Ps4 };
    let mut remote: Option<Remote> = None;
    let endpoint: Endpoint;
    if away {
        // Hors de chez soi : par une Station qui la voit, jointe par le relais.
        phase(&app, &id, "connecting");
        set_flag(&app, &host, None, Some(true));
        let stations = with(&app, |c| c.stations_of(&host).to_vec());
        let state = app.state::<UiState>();
        let station = stations.iter().filter(|s| away_station(&app, s)).find_map(|s| state.with_store(|st| st.get(s).cloned()));
        let Some(station) = station else {
            set_flag(&app, &host, None, Some(false));
            return ended_before(&app, &id, t("ps.end_unreachable", &[]));
        };
        let console = console_via(&station, true);
        match Remote::open(&console, &host) {
            Ok(r) => remote = Some(r),
            Err(RemoteError::Console(kybernow_core::console::ConsoleError::Relay(e))) => {
                set_flag(&app, &host, None, Some(false));
                log::info!("PlayStation : relais : {}", e.kind());
                return ended_before(&app, &id, e.text(i18n::lang(), Some(&station.name)));
            }
            Err(e) => {
                set_flag(&app, &host, None, Some(false));
                log::info!("PlayStation : console injoignable par la Station ({e})");
                return ended_before(&app, &id, t("ps.end_unreachable", &[]));
            }
        }
        set_flag(&app, &host, None, Some(false));
        if cancelled(&app, &id) {
            return;
        }
        let r = remote.as_ref().expect("remote");
        endpoint = r.endpoint();
        let ready = r.seen().is_some_and(|s| s.state == "ready");
        if !ready {
            phase(&app, &id, "preparing");
            set_flag(&app, &host, Some(true), None);
            let ok = wake_and_wait(&app, &id, &host, &endpoint, model, &registered, false);
            set_flag(&app, &host, Some(false), None);
            if cancelled(&app, &id) {
                return;
            }
            if !ok {
                return ended_before(&app, &id, t("ps.end_unreachable", &[]));
            }
        }
    } else {
        let address = live.as_ref().map(|f| f.address.clone()).unwrap_or_else(|| kept.address.clone());
        let Some(at) = address.parse::<Ipv4Addr>().ok().map(Endpoint::direct) else {
            return ended_before(&app, &id, t("ps.end_unreachable", &[]));
        };
        endpoint = at;
        if live.as_ref().is_some_and(|f| !f.ready()) {
            phase(&app, &id, "preparing");
            set_flag(&app, &host, Some(true), None);
            let ok = wake_and_wait(&app, &id, &host, &endpoint, model, &registered, true);
            set_flag(&app, &host, Some(false), None);
            if cancelled(&app, &id) {
                return;
            }
            if !ok {
                return ended_before(&app, &id, t("ps.end_unreachable", &[]));
            }
        }
    }
    if cancelled(&app, &id) {
        return;
    }
    phase(&app, &id, "connecting");
    let state = app.state::<UiState>();
    let hevc = state.this_pc.lock().unwrap().as_ref().is_none_or(|p| p.hevc);
    let mut settings = kept.settings();
    // Pas de HDR dans l'app Windows (l'image des sessions y est en SDR).
    settings.hdr = false;
    let near = app.get_webview_window("main").and_then(|w| {
        let p = w.outer_position().ok()?;
        let s = w.outer_size().ok()?;
        Some((p.x + s.width as i32 / 2, p.y + s.height as i32 / 2))
    });
    let start = ps_session::Start {
        id: id.clone(),
        name: kept.name.clone(),
        endpoint,
        registered,
        settings,
        layout: kept.layout(),
        hevc,
        ps5: kept.is_ps5(),
        remote,
        near,
    };
    let events = Arc::new(Events { app: app.clone(), id: id.clone(), host: host.clone(), account, name: kept.name.clone() });
    // Inscrite avant de démarrer : une fin très rapide (fenêtre impossible) la retire bien,
    // et « Annuler » la trouve dès cet instant.
    let session = Arc::new(PsSession::new(&start, events));
    ps(&app).sessions.lock().unwrap().push(session.clone());
    ps(&app).totals.lock().unwrap().insert(id.clone(), (0, 0));
    ps(&app).launching.lock().unwrap().retain(|l| l != &id);
    if ps(&app).cancelled.lock().unwrap().iter().any(|c| c == &id) {
        session.command(Command::Disconnect);
    }
    session.run(start);
    emit_sessions(&app);
    emit_state(&app);
}

/// Les nouvelles d'une partie, relayées à l'interface.
struct Events {
    app: AppHandle,
    id: String,
    host: String,
    account: String,
    name: String,
}

#[derive(Clone, Serialize)]
struct StatsEvent<'a> {
    id: &'a str,
    width: u32,
    height: u32,
    fps_asked: u32,
    hevc: bool,
    hdr: bool,
    target_kbps: u32,
    fps: f32,
    mbps: f32,
    lost: u64,
    recovered: u64,
}

/// La barre d'une partie (`PsBar` de l'interface).
#[derive(Clone, Serialize)]
pub struct BarView {
    id: String,
    name: String,
    pad: bool,
    captured: bool,
    muted: bool,
    fullscreen: bool,
    pad_mode: bool,
}

#[derive(Clone, Serialize)]
struct NavEvent<'a> {
    id: &'a str,
    key: &'static str,
}

#[derive(Clone, Serialize)]
struct RouteEvent<'a> {
    id: &'a str,
    route: relais::RouteView,
}

fn bar_view(app: &AppHandle, id: &str, name: &str, b: BarState) -> BarView {
    BarView {
        id: id.to_string(),
        name: name.to_string(),
        pad: b.pad,
        captured: b.captured,
        muted: b.muted,
        fullscreen: b.fullscreen,
        pad_mode: ps(app).pad_mode.load(Ordering::Relaxed),
    }
}

impl ps_session::Events for Events {
    fn phase(&self, p: &'static str) {
        phase(&self.app, &self.id, p);
    }

    fn shown(&self) {
        let _ = self.app.emit("session-shown", &self.id);
    }

    fn stats(&self, s: &PlayStats) {
        let (lost, recovered) = {
            let mut totals = ps(&self.app).totals.lock().unwrap();
            let e = totals.entry(self.id.clone()).or_default();
            e.0 += s.lost;
            e.1 += s.recovered;
            *e
        };
        let fps_asked = with(&self.app, |c| c.kept(&self.host).map(|k| k.settings().fps).unwrap_or(60));
        let _ = self.app.emit_to(
            "bar",
            "ps-stats",
            StatsEvent {
                id: &self.id,
                width: s.width,
                height: s.height,
                fps_asked: if fps_asked == 30 { 30 } else { 60 },
                hevc: s.hevc,
                hdr: s.hdr,
                target_kbps: s.target_kbps,
                fps: s.fps,
                mbps: s.mbps,
                lost,
                recovered,
            },
        );
    }

    fn bar(&self, b: BarState) {
        let _ = self.app.emit_to("bar", "ps-bar", bar_view(&self.app, &self.id, &self.name, b));
    }

    fn top_edge(&self, hwnd: isize) {
        let (a, id) = (self.app.clone(), self.id.clone());
        let _ = self.app.run_on_main_thread(move || show_bar(&a, &id, hwnd, false, false));
    }

    fn menu(&self, hwnd: isize) {
        let (a, id) = (self.app.clone(), self.id.clone());
        let _ = self.app.run_on_main_thread(move || {
            let state = a.state::<UiState>();
            let served = state.bar_session.lock().unwrap().as_deref() == Some(id.as_str());
            let shown = a.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
            if served && shown {
                super::bar_hide(a.clone(), Some(id), None);
            } else {
                show_bar(&a, &id, hwnd, true, true);
            }
        });
    }

    fn close_bar(&self) {
        let (a, id) = (self.app.clone(), self.id.clone());
        let _ = self.app.run_on_main_thread(move || super::bar_hide(a, Some(id), Some(false)));
    }

    fn navigate(&self, key: PadKey) {
        let key = match key {
            PadKey::Left => "left",
            PadKey::Right => "right",
            PadKey::Up => "up",
            PadKey::Down => "down",
            PadKey::Accept => "accept",
            PadKey::Back => "back",
        };
        let _ = self.app.emit_to("bar", "ps-bar-nav", NavEvent { id: &self.id, key });
    }

    fn fullscreen(&self, on: bool) {
        remember_fullscreen(&self.app, &self.host, on);
    }

    fn route(&self, r: &RouteInfo) {
        let _ = self.app.emit("session-route", RouteEvent { id: &self.id, route: relais::RouteView::new(r, i18n::lang()) });
    }

    fn ended(&self, end: Option<PlayEnd>, quiet: bool, seconds: u64) {
        let app = &self.app;
        ps(app).sessions.lock().unwrap().retain(|s| s.id() != self.id);
        ps(app).totals.lock().unwrap().remove(&self.id);
        // La console ne reconnaît plus ce compte : son association est effacée
        // (« Associer » revient si c'était la dernière).
        if end == Some(PlayEnd::NotPaired) {
            with(app, |c| c.unpair(&mut WinSecrets, &self.host, &self.account));
            save(app);
            log::info!("PlayStation : association effacée (la console ne la reconnaît plus)");
        }
        let error = if quiet { None } else { end.as_ref().and_then(end_text) };
        // La barre de cette partie s'en va avec elle.
        let state = app.state::<UiState>();
        if state.bar_session.lock().unwrap().as_deref() == Some(self.id.as_str()) {
            let (a, id) = (app.clone(), self.id.clone());
            let _ = app.run_on_main_thread(move || super::bar_hide(a, Some(id), Some(false)));
        }
        let _ = app.emit("session-ended", EndedEvent { id: &self.id, summary: Summary { error, median_ms: None, p95_ms: None, seconds } });
        emit_sessions(app);
        emit_state(app);
        show_home_if_idle(app);
        // « Quitter » pendant la partie : l'app part à sa fin.
        if app.state::<UiState>().quitting.load(Ordering::Relaxed) {
            super::exit_when_idle(app);
        }
    }
}

// MARK: - Barre de la partie

/// La barre de la partie (la même fenêtre que celle d'une session
/// d'ordinateur) : au pointeur contre le bord haut, ou par la combinaison de
/// la manette (`pad` : la manette la pilote). Ouverte, la console reçoit une
/// manette au repos, ni clavier ni souris.
fn show_bar(app: &AppHandle, id: &str, hwnd: isize, pinned: bool, pad: bool) {
    let Some(s) = session(app, id) else { return };
    let state = app.state::<UiState>();
    let Some(bar) = app.get_webview_window("bar") else { return };
    ps(app).pad_mode.store(pad, Ordering::Relaxed);
    let before = state.bar_session.lock().unwrap().replace(id.to_string());
    if let Some(prev) = before.filter(|p| p != id && consoles::host_of(p).is_some()) {
        if let Some(s) = session(app, &prev) {
            s.command(Command::BarClosed);
        }
    }
    super::bar_menu(app.clone(), false, None);
    let hwnd = if hwnd != 0 { hwnd } else { s.hwnd() };
    let (center, top, scale) = crate::window::bar_anchor(hwnd).unwrap_or((0, 0, bar.scale_factor().unwrap_or(1.0) as f32));
    let width = (super::BAR_WINDOW_WIDTH * scale as f64).round() as i32;
    let _ = bar.set_position(tauri::PhysicalPosition::new(center - width / 2, top));
    let _ = bar.set_always_on_top(true);
    let _ = bar.show();
    if pad {
        let _ = bar.set_focus();
    }
    crate::window::set_bar_session(None);
    s.command(Command::BarOpen);
    let route = s.route().map(|r| relais::RouteView::new(&r, i18n::lang()));
    let sessions = state.sessions.lock().unwrap().len() + ps(app).sessions.lock().unwrap().len();
    let shown = super::console_bar_shown(id, s.name(), pinned, sessions, route, bar_view(app, id, s.name(), s.bar_state()));
    let _ = app.emit_to("bar", "bar-shown", shown);
}

/// La barre se cache (ou sert une autre session) : la console reçoit de
/// nouveau la manette ; `focus` : la fenêtre de la partie reprend le clavier.
pub fn bar_hidden(app: &AppHandle, id: &str, focus: bool) {
    ps(app).pad_mode.store(false, Ordering::Relaxed);
    if let Some(s) = session(app, id) {
        s.command(Command::BarClosed);
        if focus {
            s.command(Command::Focus);
        }
    }
}

/// `session_command` pour une partie (`ps-…`) : vrai si c'en est une.
pub fn command(app: &AppHandle, id: &str, command: &str) -> bool {
    if consoles::host_of(id).is_none() {
        return false;
    }
    let Some(s) = session(app, id) else {
        if command == "disconnect" {
            // Pendant le réveil ou la jonction par le relais : le lancement s'arrête.
            if ps(app).launching.lock().unwrap().iter().any(|l| l == id) {
                ps(app).cancelled.lock().unwrap().push(id.to_string());
                log::info!("PlayStation : « Annuler » pendant le lancement");
            } else {
                log::info!("PlayStation : « Annuler » : plus de partie à arrêter");
            }
        }
        return true;
    };
    match command {
        "disconnect" => s.command(Command::Disconnect),
        "mute" => s.command(Command::Mute),
        "mouse" => s.command(Command::Mouse),
        "fullscreen" => s.command(Command::Fullscreen),
        "rest" => s.command(Command::Rest),
        // « Réglages de {nom} » : l'accueil passe devant, sur les réglages de la console.
        "settings" => {
            let (a, id) = (app.clone(), id.to_string());
            let _ = app.run_on_main_thread(move || {
                super::bar_hide(a.clone(), Some(id.clone()), Some(false));
                show_action(&a, super::Action::PsSettings { id });
            });
        }
        _ => {}
    }
    true
}

/// « Afficher la session » d'une partie : vrai si c'en est une.
pub fn show(app: &AppHandle, id: &str) -> Option<bool> {
    consoles::host_of(id)?;
    Some(session(app, id).map(|s| s.command(Command::Focus)).is_some())
}

/// Quitter l'app : chaque partie s'arrête.
pub fn stop_all(app: &AppHandle) {
    for s in ps(app).sessions.lock().unwrap().iter() {
        s.command(Command::Disconnect);
    }
}

// MARK: - Envoi entre appareils

#[derive(Serialize)]
pub struct WaitingView {
    name: String,
    platform: String,
    address: String,
}

fn device_name(app: &AppHandle) -> String {
    let state = app.state::<UiState>();
    let name = state.this_pc.lock().unwrap().as_ref().map(|p| p.name.clone()).unwrap_or_default();
    if name.trim().is_empty() { std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into()) } else { name }
}

/// Les appareils du réseau qui attendent un envoi (2 s), sans celui-ci.
#[tauri::command(async)]
pub fn ps_handoff_discover(app: AppHandle) -> Vec<WaitingView> {
    let mine = ps(&app).receiving.lock().unwrap().as_ref().map(|r| r.port);
    handoff::discover(Duration::from_millis(2000), &[])
        .unwrap_or_default()
        .into_iter()
        .filter(|w| Some(w.addr.port()) != mine)
        .map(|w| WaitingView { name: w.name, platform: w.platform, address: w.addr.to_string() })
        .collect()
}

/// « Envoyer » : tout part d'un coup (comptes, associations, consoles et les
/// Stations qui les voient), chiffré par le code tapé.
#[tauri::command(async)]
pub fn ps_handoff_send(app: AppHandle, address: String, name: String, code: String) -> Result<String, String> {
    let Ok(to) = address.parse::<SocketAddr>() else { return Err(t("ps.send_unreachable", &[("name", &name)])) };
    let state = app.state::<UiState>();
    let computers = state.with_store(|s| s.computers.clone());
    let station = |id: &str| {
        computers.iter().find(|c| c.id == id).map(|c| BundleStation { id: c.id.clone(), name: c.name.clone(), address: c.host.clone(), fingerprint: c.fingerprint.clone() })
    };
    let bundle = with(&app, |c| c.bundle(&device_name(&app), &WinSecrets, &station));
    let json = serde_json::to_string(&bundle).map_err(|e| e.to_string())?;
    let digits: String = code.chars().filter(char::is_ascii_digit).collect();
    log::info!("PlayStation : envoi de {} compte(s) et {} console(s) à un appareil", bundle.accounts.len(), bundle.consoles.len());
    match handoff::send(to, &digits, &json) {
        Ok(()) => Ok(t("ps.sent", &[("name", &name)])),
        Err(handoff::SendError::Refused) => Err(t("ps.send_refused", &[("name", &name)])),
        Err(handoff::SendError::Unreachable(d)) => {
            log::warn!("PlayStation : envoi impossible ({d})");
            Err(t("ps.send_unreachable", &[("name", &name)]))
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum HandoffEvent {
    Received { text: String },
    NewCode { code: String },
    Expired,
}

/// « 2 comptes et 1 console reçus. »
fn received_text(accounts: usize, consoles: usize) -> String {
    let n = |count: usize, one: &str, other: &str| if count == 1 { t(one, &[]) } else { t(other, &[("n", &count.to_string())]) };
    t(
        "ps.received",
        &[("accounts", &n(accounts, "ps.n_accounts_one", "ps.n_accounts_other")), ("consoles", &n(consoles, "ps.n_consoles_one", "ps.n_consoles_other"))],
    )
}

/// « Recevoir d'un autre appareil » : commence (`on`) ou arrête l'attente ;
/// rend le code à 6 chiffres. La suite par `ps-handoff`.
#[tauri::command(async)]
pub fn ps_handoff_receive(app: AppHandle, on: bool) -> Result<Option<String>, String> {
    let p = ps(&app);
    if let Some(old) = p.receiving.lock().unwrap().take() {
        old.stop.store(true, Ordering::Relaxed);
    }
    if !on {
        return Ok(None);
    }
    let receiver = handoff::Receiver::start(&device_name(&app), "windows").map_err(|e| {
        log::warn!("PlayStation : attente d'un envoi impossible ({e})");
        t("ps.receive_failed", &[])
    })?;
    let code = receiver.code();
    let stop = Arc::new(AtomicBool::new(false));
    *p.receiving.lock().unwrap() = Some(Receiving { stop: stop.clone(), port: receiver.port() });
    log::info!("PlayStation : attente d'un envoi");
    let a = app.clone();
    let _ = std::thread::Builder::new().name("ps-reception".into()).spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match receiver.next(Duration::from_millis(500)) {
                Some(handoff::Event::Received(json)) => {
                    let Ok(bundle) = serde_json::from_str::<Bundle>(&json) else {
                        log::warn!("PlayStation : paquet reçu illisible");
                        continue;
                    };
                    let (accounts, consoles) = merge(&a, &bundle);
                    log::info!("PlayStation : reçu {accounts} compte(s) et {consoles} console(s)");
                    let _ = a.emit_to("main", "ps-handoff", HandoffEvent::Received { text: received_text(accounts, consoles) });
                }
                Some(handoff::Event::NewCode(code)) => {
                    let _ = a.emit_to("main", "ps-handoff", HandoffEvent::NewCode { code });
                }
                Some(handoff::Event::Expired) => {
                    let _ = a.emit_to("main", "ps-handoff", HandoffEvent::Expired);
                    break;
                }
                None => {}
            }
        }
        receiver.stop();
        let mut r = ps(&a).receiving.lock().unwrap();
        if r.as_ref().is_some_and(|r| Arc::ptr_eq(&r.stop, &stop)) {
            *r = None;
        }
    });
    Ok(Some(code))
}

/// Un paquet reçu, fusionné ; ses Stations reliées aux ordinateurs de l'app
/// par l'identifiant, sinon l'empreinte, sinon l'adresse.
fn merge(app: &AppHandle, bundle: &Bundle) -> (usize, usize) {
    let state = app.state::<UiState>();
    let computers = state.with_store(|s| s.computers.clone());
    let station = |s: &BundleStation| {
        computers
            .iter()
            .find(|c| c.id == s.id)
            .or_else(|| s.fingerprint.as_ref().and_then(|fp| computers.iter().find(|c| c.fingerprint.as_deref() == Some(fp.as_str()))))
            .or_else(|| (!s.address.is_empty()).then(|| computers.iter().find(|c| c.host == s.address)).flatten())
            .map(|c| c.id.clone())
    };
    let counts = with(app, |c| c.merge(&mut WinSecrets, bundle, &station));
    save(app);
    emit_state(app);
    counts
}

/// Pour le menu de l'icône et la fermeture : l'état d'une console en partie.
#[allow(dead_code)]
pub fn status_of(app: &AppHandle, id: &str) -> Option<Status> {
    view(app).consoles.into_iter().find(|c| c.id == id).map(|c| c.status)
}
