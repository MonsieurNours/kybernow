//! KyberNow Share côté Windows (docs/kybernow-share.md, textes de
//! docs/link-ui.md « Partage »), dans l'app :
//!
//! - **Invité** : « Rejoindre avec un code » (écran du code et du prénom,
//!   menu de l'icône, lien `kybernow://join`). `share_join` envoie le code aux
//!   Stations du réseau (`kybernow_core::share::join_checked`) puis
//!   `share_connect` ouvre la session d'invité (`SessionParams::guest`) dans sa
//!   fenêtre, titrée « Invité sur {nom} ». Une session d'invité est une session
//!   comme les autres pour le registre du cœur et la table des sessions (elle
//!   compte parmi les quatre), mais son identifiant est `guest-N`, jamais un
//!   ordinateur de l'app : rien n'est écrit dans `computers.json`, ni profil
//!   ni réglages, et sa fin vient du partage (texte du cœur, jamais « Reprendre
//!   la main »).
//! - **Hôte** : « Partager » dans la barre de la session. Les appels à Admin
//!   (ouvrir, nouveau code, rôle d'un invité, retirer, arrêter) passent par le
//!   compte de l'appareil (`console_of`), seulement pour une session d'hôte ;
//!   le partage et ses invités arrivent aussi avec le signe de vie de la session
//!   (`HostEvents::guests`).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNA, ShowWindow};

use kybernow_core::console::{Console, ConsoleError, Mode};
use kybernow_core::profile::{Profile, StreamSettings};
use kybernow_core::screens::DisplaysSetting;
use kybernow_core::session::SessionParams;
use kybernow_core::share::{self, Found, JoinRequest, ShareAvailability, ShareGuest, ShareOpened, ShareRole, ShareState, ShareTarget};

use super::*;
use crate::sharing::{self, Known, ShareView};

/// L'ordinateur qui partage, vu par une session d'invité.
pub struct Guest {
    /// Son nom (« Atelier ») : le menu de l'icône dit « Invité sur Atelier ».
    pub computer: String,
    /// {hôte} des textes : le prénom de l'hôte, ou le nom de l'ordinateur.
    pub host: String,
    /// « Invité sur {nom} » : le titre de ses fenêtres et de sa connexion.
    pub title: String,
    pub role: Mutex<ShareRole>,
}

/// Un partage rejoint, en attente de sa session (`share_connect`).
pub struct Pending {
    id: String,
    session: u64,
    found: Found,
    guest: Arc<Guest>,
}

/// Réponse de `share_join` : de quoi montrer la connexion (le passage de
/// l'accueil) avant d'ouvrir la session.
#[derive(Serialize)]
pub struct Joined {
    /// `guest-N` : la session d'invité dans tous les événements.
    id: String,
    /// « Invité sur {nom} ».
    title: String,
    host: String,
    role: &'static str,
    /// Teinte du passage : l'empreinte d'Admin de l'ordinateur qui partage.
    fingerprint: Option<String>,
}

/// `share-role` : le rôle de l'invité de la session `id` (au départ, puis
/// chaque changement par l'hôte).
#[derive(Clone, Serialize)]
pub struct RoleEvent {
    pub id: String,
    pub role: &'static str,
}

/// `share-state` : le panneau de l'hôte de la session `id`.
#[derive(Clone, Serialize)]
struct ShareEvent {
    id: String,
    share: ShareView,
}

/// Capacités de ce PC, sondées une fois (comme `connect`).
fn this_pc_of(state: &UiState) -> ThisPc {
    let cached = state.this_pc.lock().unwrap().clone();
    match cached {
        Some(pc) => pc,
        None => {
            let pc = pc::this_pc();
            *state.this_pc.lock().unwrap() = Some(pc.clone());
            pc
        }
    }
}

/// Les Stations à qui envoyer le code : celles annoncées sur le réseau, puis
/// celles des ordinateurs de l'app, sans ce PC (`sharing::targets`).
fn join_targets(state: &UiState) -> Vec<ShareTarget> {
    let discovered: Vec<Known> = state
        .discovery
        .get()
        .map(|d| d.list())
        .unwrap_or_default()
        .into_iter()
        .map(|f| Known { host: f.host, port: f.port, fingerprint: f.fingerprint })
        .collect();
    let saved: Vec<Known> =
        state.with_store(|s| s.computers.iter().map(|c| Known { host: c.host.clone(), port: c.port, fingerprint: c.fingerprint.clone() }).collect());
    sharing::targets(discovered, saved, system::is_this_pc)
}

/// Le prénom proposé à « Rejoindre » : celui d'avant, sinon le nom de ce PC.
#[derive(Serialize)]
pub struct Defaults {
    name: String,
}

#[tauri::command(async)]
pub fn share_defaults(app: AppHandle) -> Defaults {
    let remembered = app.state::<UiState>().prefs.lock().unwrap().share_name.clone();
    Defaults { name: sharing::proposed_name(remembered.as_deref(), &system::device_info().name) }
}

/// Le code d'un lien de partage (`kybernow://join?…`), pour l'écran de
/// « Rejoindre » ouvert par le lien.
#[tauri::command(async)]
pub fn share_link_code(link: String) -> Result<String, String> {
    share::parse_link(&link).map(|l| l.code).ok_or_else(|| t("errors.share_link", &[]))
}

/// « Rejoindre » : le code part aux Stations du réseau (ou à celle du lien),
/// celle qui partage répond, et l'app vérifie qu'elle sait lire son image.
/// Rend la session à ouvrir (`share_connect`), ou le texte du refus (celui du
/// cœur : « Aucun partage avec ce code sur le réseau. », « Ce partage est
/// complet… », « Trop d'essais… », « Cet appareil ne lit pas l'image… »).
/// Bloque 6 s au plus.
#[tauri::command(async)]
pub fn share_join(app: AppHandle, code: String, name: String, link: Option<String>) -> Result<Joined, String> {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::Relaxed) {
        return Err(t("errors.session_not_ready", &[]));
    }
    // Une session d'invité compte parmi les quatre.
    if state.full() {
        return Err(t("errors.four_at_most", &[]));
    }
    let code = share::normalize_code(&code).ok_or_else(|| t("errors.share_code", &[]))?;
    let targets = match link.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => vec![share::parse_link(l).ok_or_else(|| t("errors.share_link", &[]))?.target()],
        None => join_targets(&state),
    };
    let device = system::device_info();
    let name = match sharing::clean_name(&name) {
        n if n.is_empty() => sharing::proposed_name(None, &device.name),
        n => n,
    };
    let pc = this_pc_of(&state);
    let request = JoinRequest { code, name: name.clone(), device: device.id.clone(), app: "windows".into() };
    log::info!("partage : code envoyé à {} Station(s)", targets.len());
    let found = share::join_checked(&targets, &request, capabilities(&pc)).map_err(|e| {
        log::info!("partage : refusé ({})", e.kind());
        e.text(i18n::lang())
    })?;
    // Le prénom est retenu pour la fois suivante.
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.share_name = Some(name);
        p.clone()
    };
    save_prefs(&prefs);

    let j = &found.joined;
    let host = j.host_name().to_string();
    let computer = if j.computer.trim().is_empty() { host.clone() } else { j.computer.clone() };
    let title = t("share.guest_on", &[("name", &computer)]);
    let session = window::new_app_id();
    let id = format!("guest-{session}");
    log::info!("partage : rejoint, chez {computer} (partage de {host}), {:?}", j.role);
    let guest = Arc::new(Guest { computer, host: host.clone(), title: title.clone(), role: Mutex::new(j.role) });
    let joined = Joined { id: id.clone(), title, host, role: j.role.as_str(), fingerprint: found.target.fingerprint.clone() };
    // Un partage rejoint dont la session ne s'est jamais ouverte repart.
    let old = state.pending_guest.lock().unwrap().replace(Pending { id, session, found, guest });
    if let Some(old) = old {
        std::thread::spawn(move || share::leave(&old.found));
    }
    Ok(joined)
}

/// La session d'invité d'un partage rejoint (`share_join`) : la première
/// s'ouvre comme celle d'un ordinateur (passage de l'accueil, plein écran) ;
/// pendant une autre session, elle s'ouvre dans une fenêtre avec sa propre
/// fenêtre de connexion (plusieurs ordinateurs). Un seul écran : celui de
/// l'hôte du partage, que l'invité ne change jamais.
#[tauri::command(async)]
pub fn share_connect(app: AppHandle, id: String) -> Result<ConnectInfo, String> {
    let state = app.state::<UiState>();
    let pending = {
        let mut p = state.pending_guest.lock().unwrap();
        match p.take() {
            Some(x) if x.id == id => x,
            other => {
                *p = other;
                return Err(t("errors.no_session", &[]));
            }
        }
    };
    let leave = |found: &Found| {
        let found = found.clone();
        std::thread::spawn(move || share::leave(&found));
    };
    // L'app se ferme : rien de nouveau ne démarre.
    if state.quitting.load(Ordering::Relaxed) {
        leave(&pending.found);
        return Ok(ConnectInfo { screens: 0, windowed: true });
    }
    // Une place parmi les quatre (comme `connect`).
    let first = {
        let sessions = state.sessions.lock().unwrap();
        let mut launching = state.launching.lock().unwrap();
        if sessions.len() + launching.len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS {
            drop(launching);
            drop(sessions);
            leave(&pending.found);
            return Err(t("errors.four_at_most", &[]));
        }
        let first = sessions.is_empty() && launching.is_empty();
        launching.push(id.clone());
        first
    };
    let _launching = Launching { app: &app, id: id.clone() };
    let Pending { session, found, guest, .. } = pending;

    let pc = this_pc_of(&state);
    let caps = capabilities(&pc);
    // Le profil de l'app (transport rapide) : le format est celui de l'hôte.
    let settings = StreamSettings::for_profile(Profile::Jeu, caps);
    let params = SessionParams::guest(&found, system::device_info(), settings.clone(), caps, audio::default_output_channels());
    let d = found.joined.display;
    let mode = if d.width > 0 && d.height > 0 { Mode { width: d.width, height: d.height, refresh: d.refresh.max(1) } } else { Mode { width: 1920, height: 1080, refresh: 60 } };
    // Un seul écran de ce PC, « Cet écran » ; sans écran virtuel ni écrans de
    // l'ordinateur à créer : le partage montre un écran (celui de l'hôte).
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
    log::info!("session d'invité chez {} : {}×{} à {} Hz", guest.computer, mode.width, mode.height, mode.refresh);
    let bar = Arc::new(Mutex::new(BarLabel { label: String::new(), profile: String::new() }));
    let sent = Arc::new(Mutex::new(Sent { settings, mode, screens: 1, pixel_rate: 0.0, virtual_display: false }));
    let stats = Arc::new(Mutex::new(None));
    let spot = Arc::new(Mutex::new(Spot::default()));
    let spec = layout.spec;
    let this_screen = layout.this_screen.clone();
    // Sa connexion : une fenêtre à sa place pendant une autre session, titrée comme elle.
    let connexion = Arc::new(Connexion { session, name: guest.title.clone(), ..Default::default() });
    {
        let mut view = connexion.view.lock().unwrap();
        view.id = id.clone();
        view.phase = "preparing";
        view.title = Some(guest.title.clone());
        view.fingerprint = found.target.fingerprint.clone();
        view.guest = true;
    }
    if let Some(f) = &follow {
        log::info!("session d'invité : fenêtre de {}×{} sur {} (plusieurs ordinateurs)", f.client.0, f.client.1, f.monitor);
        open_connexion(&app, &connexion, f.at, (f.client.0 as u32, f.client.1 as u32), &guest.title);
    }
    // Tenu jusqu'à l'inscription de la session : une fin très rapide (dans
    // `ended`) ne peut pas passer avant.
    let mut running = state.sessions.lock().unwrap();
    let host = SessionHost::start_as(
        session,
        guest.title.clone(),
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
            guest: Some(guest.clone()),
            share: Arc::new(Mutex::new(None)),
        }),
    );
    running.push(Running { id: id.clone(), name: guest.title.clone(), host, bar, sent, stats, guest: Some(guest), share: Arc::new(Mutex::new(None)) });
    drop(running);
    drop(_launching);
    // « Quitter » pendant le lancement : elle s'arrête tout de suite, l'app part à sa fin.
    if state.quitting.load(Ordering::Relaxed) {
        end_instance(&state, session, "Quitter pendant le lancement");
    }
    emit_sessions(&app);
    Ok(ConnectInfo { screens: 1, windowed: !first })
}

// MARK: - Hôte

/// Ce que « Partager » peut faire avec cet ordinateur, d'après ce qu'il a dit de
/// lui (`info.share`) ; `None` tant qu'il n'a rien dit.
fn availability(app: &AppHandle, id: &str) -> Option<ShareAvailability> {
    let state = app.state::<UiState>();
    let live = state.live.lock().unwrap();
    live.get(id).and_then(|l| l.info.as_ref()).map(ShareAvailability::of)
}

/// Le panneau de l'hôte de la session `id`, pour la barre.
pub fn view(app: &AppHandle, id: &str, state: Option<&ShareState>) -> ShareView {
    ShareView::new(state, availability(app, id), i18n::lang())
}

/// Le partage de la session `id` a changé : la barre en est prévenue.
pub fn emit_state(app: &AppHandle, id: &str, share: &Mutex<Option<ShareState>>) {
    let share = view(app, id, share.lock().unwrap().as_ref());
    let _ = app.emit_to("bar", "share-state", ShareEvent { id: id.to_string(), share });
}

/// Garde le partage de la session `id` et le dit à la barre ; rend le panneau.
fn publish(app: &AppHandle, id: &str, state: Option<ShareState>) -> ShareView {
    let shown = state.filter(|s| s.open);
    if let Some(cell) = app.state::<UiState>().with_session(id, |r| r.share.clone()) {
        *cell.lock().unwrap() = shown.clone();
    }
    let share = view(app, id, shown.as_ref());
    let _ = app.emit_to("bar", "share-state", ShareEvent { id: id.to_string(), share: share.clone() });
    share
}

/// Admin de l'ordinateur d'une session d'hôte (compte de l'appareil) : jamais
/// pour la session d'un invité, ni sans session.
fn host_console(app: &AppHandle, id: &str) -> Result<Console, String> {
    let is_host = app.state::<UiState>().with_session(id, |r| r.guest.is_none()).unwrap_or(false);
    if !is_host {
        return Err(t("errors.no_session", &[]));
    }
    console_of(app, id)
}

fn lift<T>(r: Result<T, ConsoleError>) -> Result<T, String> {
    r.map_err(|e| e.text(i18n::lang()))
}

/// Le partage tel qu'Admin le dit maintenant ; à défaut (Admin ne répond
/// pas), ce que la dernière action a rendu, avec ces invités.
fn state_after(console: &Console, opened: Option<&ShareOpened>, guests: Vec<ShareGuest>) -> Result<ShareState, String> {
    match (lift(console.share_state()), opened) {
        (Ok(s), _) => Ok(s),
        (Err(_), Some(o)) => Ok(ShareState { open: true, code: o.code.clone(), link: o.link.clone(), web: o.web.clone(), display: o.display.clone(), guests }),
        (Err(e), None) => Err(e),
    }
}

fn current_guests(app: &AppHandle, id: &str) -> Vec<ShareGuest> {
    app.state::<UiState>()
        .with_session(id, |r| r.share.lock().unwrap().as_ref().map(|s| s.guests.clone()))
        .flatten()
        .unwrap_or_default()
}

/// « Partager » : ouvre le partage de l'écran principal de la session (code,
/// lien, QR code). Admin refuse si l'ordinateur est trop ancien ou si le
/// partage y est coupé : le texte du cœur (docs/link-ui.md).
#[tauri::command(async)]
pub fn share_open(app: AppHandle, id: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    // Ce que l'ordinateur annonce maintenant (la session n'a pas relevé `info` depuis son départ).
    let info = lift(console.info())?;
    app.state::<UiState>().live.lock().unwrap().entry(id.clone()).or_default().info = Some(info.clone());
    if let Some(text) = ShareAvailability::of(&info).text(i18n::lang()) {
        publish(&app, &id, None);
        return Err(text);
    }
    let opened = lift(console.share_open())?;
    log::info!("[{id}] partage ouvert{}", opened.display.as_ref().map(|d| format!(" ({})", d.label)).unwrap_or_default());
    let state = state_after(&console, Some(&opened), Vec::new())?;
    Ok(publish(&app, &id, Some(state)))
}

/// « Nouveau code » : les invités déjà là restent.
#[tauri::command(async)]
pub fn share_code(app: AppHandle, id: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    let opened = lift(console.share_code())?;
    let state = state_after(&console, Some(&opened), current_guests(&app, &id))?;
    Ok(publish(&app, &id, Some(state)))
}

/// « Faire jouer », « Faire regarder », « Donner le clavier et la souris »,
/// « Reprendre le clavier et la souris » : le rôle d'un invité
/// (`spectator`, `player`, `player_kbm`).
#[tauri::command(async)]
pub fn share_guest(app: AppHandle, id: String, guest: String, role: String) -> Result<ShareView, String> {
    let role = ShareRole::parse(&role).ok_or_else(|| t("errors.share_code", &[]))?;
    let console = host_console(&app, &id)?;
    lift(console.share_guest(&guest, role))?;
    log::info!("[{id}] invité {guest} : {role:?}");
    let state = state_after(&console, None, Vec::new())?;
    Ok(publish(&app, &id, Some(state)))
}

/// « Retirer » un invité du partage.
#[tauri::command(async)]
pub fn share_kick(app: AppHandle, id: String, guest: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    lift(console.share_kick(&guest))?;
    log::info!("[{id}] invité {guest} retiré");
    let state = state_after(&console, None, Vec::new())?;
    Ok(publish(&app, &id, Some(state)))
}

/// « Arrêter le partage » : tout le monde sort.
#[tauri::command(async)]
pub fn share_close(app: AppHandle, id: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    lift(console.share_close())?;
    log::info!("[{id}] partage arrêté");
    Ok(publish(&app, &id, None))
}

/// « Copier le lien » : le texte part dans le presse-papiers de Windows (le
/// presse-papiers de l'interface demande un geste et peut être refusé). Vrai
/// refus (« Copie impossible… ») si le presse-papiers n'a pas pris le texte.
#[tauri::command(async)]
pub fn copy_text(text: String) -> Result<(), String> {
    use kybernow_core::input::Pasteboard;
    let board = crate::pasteboard::WinPasteboard;
    board.set_text(&text);
    if board.text().as_deref() == Some(text.as_str()) { Ok(()) } else { Err(String::new()) }
}

// MARK: - Avis bref

/// Montre une fenêtre sans lui donner le clavier : le clavier et les manettes
/// de la session en cours ne s'interrompent pas.
pub fn show_without_focus(bar: &tauri::WebviewWindow) {
    match bar.hwnd() {
        Ok(h) => {
            let raw = h.0 as isize;
            unsafe {
                let _ = ShowWindow(HWND(raw as *mut _), SW_SHOWNA);
            }
        }
        Err(_) => {
            let _ = bar.show();
        }
    }
}

/// Un avis bref pour l'invité de la session `id` (« Vous pouvez jouer. ») :
/// dans la barre de sa fenêtre quand l'app est devant (sans prendre le clavier
/// à la session, la barre repart d'elle-même), sinon dans une notification de
/// Windows (les notifications ne s'affichent pas par-dessus un jeu en plein
/// écran, d'où la barre). La barre sert déjà une autre session : la notification.
pub fn notice(app: &AppHandle, id: &str, title: &str, text: &str) {
    let state = app.state::<UiState>();
    let bar_shown = app.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
    let bar_elsewhere = bar_shown && state.bar_session.lock().unwrap().as_deref() != Some(id);
    if shell::app_in_front() && !bar_elsewhere {
        let (a, id, text) = (app.clone(), id.to_string(), text.to_string());
        let _ = app.run_on_main_thread(move || {
            let hwnd = a.state::<UiState>().with_session(&id, |r| r.host.bar_window()).unwrap_or(0);
            show_bar_with(&a, &id, hwnd, true, Some(text));
        });
    } else {
        shell::notify_to("kybernow://open", title, text, None);
    }
}
