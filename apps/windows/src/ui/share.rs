//! KyberNow Share côté Windows (docs/kybernow-share.md, textes de
//! docs/link-ui.md « Partage »), dans l'app :
//!
//! - **Invité** : « Rejoindre avec un code » (écran du code et du prénom,
//!   menu de l'icône, lien `kybernow://join`). `share_join` envoie le code aux
//!   Stations du réseau **et en même temps** au relais (`share::join_anywhere`, ou
//!   `share::join_link` pour un lien : le réseau d'abord, puis le relais
//!   que le lien désigne) ; hors de chez soi, l'hôte accepte chaque invité :
//!   « En attente de l'accord de l'hôte… » (`share_wait`, un appel tenu 25 s que
//!   l'interface rappelle, et `share_cancel` : « Annuler »). Puis
//!   `share_connect` ouvre la session d'invité (`SessionParams::guest`) dans sa
//!   fenêtre, titrée « Invité sur {nom} ». Une session d'invité est une session
//!   comme les autres pour le registre du cœur et la table des sessions (elle
//!   compte parmi les quatre), mais son identifiant est `guest-N`, jamais un
//!   ordinateur de l'app : rien n'est écrit dans `computers.json`, ni profil
//!   ni réglages, et sa fin vient du partage (texte du cœur, jamais « Reprendre
//!   la main »). Sa « Qualité de l'image » est choisie à « Rejoindre » (« Auto »
//!   envoie l'écran de ce PC et ce que sa carte décode) et changée dans la
//!   barre (`share_quality`, sans relancer la session) ; l'image reçue et son
//!   avis arrivent par `HostEvents::share_image`.
//! - **Hôte** : « Partager » dans la barre de la session. Les appels à Admin
//!   (ouvrir, nouveau code, rôle d'un invité, retirer, arrêter, accepter ou refuser
//!   une demande venue par le relais) passent par le
//!   compte de l'appareil (`console_of`, par le relais quand la session y passe),
//!   seulement pour une session d'hôte ;
//!   le partage et ses invités arrivent aussi avec le signe de vie de la session
//!   (`HostEvents::guests`).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_SHOWNA, ShowWindow};

use kybernow_core::console::{Console, ConsoleError, Mode};
use kybernow_core::profile::{Profile, StreamSettings};
use kybernow_core::screens::DisplaysSetting;
use kybernow_core::session::SessionParams;
use kybernow_core::share::{self, Found, JoinRequest, Joining, PendingJoin, ShareAvailability, ShareGuest, ShareOpened, ShareQuality, ShareRole, ShareState, ShareTarget};

use super::*;
use crate::relais;
use crate::sharing::{self, JoinOutcome, Joined, Known, ShareView, WaitOutcome};

/// L'ordinateur qui partage, vu par une session d'invité.
pub struct Guest {
    /// Son nom (« Atelier ») : le menu de l'icône dit « Invité sur Atelier ».
    pub computer: String,
    /// {hôte} des textes : le prénom de l'hôte, ou le nom de l'ordinateur.
    pub host: String,
    /// « Invité sur {nom} » : le titre de ses fenêtres et de sa connexion.
    pub title: String,
    pub role: Mutex<ShareRole>,
    /// La « Qualité de l'image » demandée : celle de « Rejoindre », puis celle
    /// que dit la Station (`ShareImage::quality`) ou que l'invité choisit.
    pub quality: Mutex<ShareQuality>,
    /// La qualité reçue (« 1080p · 60 Hz »), quand Kyber en connaît la taille.
    pub image: Mutex<Option<String>>,
}

/// Un partage rejoint, en attente de sa session (`share_connect`).
pub struct Pending {
    id: String,
    session: u64,
    found: Found,
    guest: Arc<Guest>,
}

/// Une demande par le relais qui attend l'accord de l'hôte (« En attente de
/// l'accord de l'hôte… ») : de quoi la suivre (`share_wait`), l'annuler
/// (`share_cancel`) et ouvrir la session d'invité quand l'hôte accepte.
pub struct Wait {
    id: u64,
    pending: PendingJoin,
    quality: ShareQuality,
    /// « Annuler » est passé : ce que `share_wait` apprend ensuite est jeté, et un
    /// accord arrivé entre-temps est rendu (`share::leave`).
    cancelled: Arc<AtomicBool>,
}

/// Le numéro des demandes en attente (`share_join` → `share_wait`).
static WAITS: AtomicU64 = AtomicU64::new(1);

/// `share-role` : le rôle de l'invité de la session `id` (au départ, puis
/// chaque changement par l'hôte).
#[derive(Clone, Serialize)]
pub struct RoleEvent {
    pub id: String,
    pub role: &'static str,
}

/// `share-image` : l'image que reçoit l'invité de la session `id` (la qualité
/// demandée, et la reçue quand Kyber la connaît).
#[derive(Clone, Serialize)]
pub struct ImageEvent {
    pub id: String,
    pub quality: &'static str,
    pub label: Option<String>,
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

/// Les relais où chercher le code en plus du relais public (que le cœur interroge
/// toujours) : ceux des ordinateurs de l'app (`info.relay.server`, `relais::join_relays`).
fn join_relays(state: &UiState) -> Vec<kybernow_core::relay::RelayAddr> {
    let servers: Vec<String> = state.with_store(|s| s.computers.iter().filter_map(|c| c.relay.as_ref().map(|r| r.server.clone())).collect());
    relais::join_relays(servers.iter().map(String::as_str))
}

/// Ce que propose « Rejoindre » : le prénom d'avant, sinon le nom de ce PC, et
/// la « Qualité de l'image » d'avant, sinon « Auto ».
#[derive(Serialize)]
pub struct Defaults {
    name: String,
    quality: &'static str,
}

#[tauri::command(async)]
pub fn share_defaults(app: AppHandle) -> Defaults {
    let state = app.state::<UiState>();
    let (remembered, quality) = {
        let p = state.prefs.lock().unwrap();
        (p.share_name.clone(), p.share_quality.unwrap_or_default())
    };
    Defaults { name: sharing::proposed_name(remembered.as_deref(), &system::device_info().name), quality: quality.as_str() }
}

/// Le code d'un lien de partage (`kybernow://join?…`), pour l'écran de
/// « Rejoindre » ouvert par le lien.
#[tauri::command(async)]
pub fn share_link_code(link: String) -> Result<String, String> {
    share::parse_link(&link).map(|l| l.code).ok_or_else(|| t("errors.share_link", &[]))
}

/// « Rejoindre » : le code part aux Stations du réseau **et en même temps** au
/// relais public (`share.kybernow.app`, sans port : le 443) et à ceux des
/// ordinateurs de l'app (`share::join_anywhere`) ;
/// avec un lien, la Station du lien sur le réseau d'abord, puis le relais qu'il
/// désigne (`share::join_link`). `KYBERNOW_RELAY=off` : le réseau seulement. La
/// « Qualité de l'image » choisie (`auto`, `host`, `2160p60`, `1080p60`,
/// `720p30` ; « Auto » ajoute l'écran de ce PC et ce que sa carte décode) part
/// avec le code.
///
/// Rend `joined` (la session à ouvrir, `share_connect`) quand une Station du
/// réseau a répondu, `pending` quand une Station vue par le relais attend
/// l'accord de l'hôte (`share_wait`), ou le texte du refus (celui du cœur :
/// « Aucun partage avec ce code sur le réseau. », « Ce partage est complet… »,
/// « Trop d'essais… », « Le relais ne répond pas. », « Trop de demandes en
/// attente… »). Le format de l'image de l'hôte n'en est plus un : un format que
/// ce PC ne lit pas lui vaut une image à part. Bloque 8 s au plus, sans compter
/// l'attente de l'hôte.
#[tauri::command(async)]
pub fn share_join(app: AppHandle, code: String, name: String, quality: String, link: Option<String>) -> Result<JoinOutcome, String> {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::Relaxed) {
        return Err(t("errors.session_not_ready", &[]));
    }
    // Une session d'invité compte parmi les quatre.
    if state.full() {
        return Err(t("errors.four_at_most", &[]));
    }
    let code = share::normalize_code(&code).ok_or_else(|| t("errors.share_code", &[]))?;
    let link = match link.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        Some(l) => Some(share::parse_link(l).ok_or_else(|| t("errors.share_link", &[]))?),
        None => None,
    };
    let device = system::device_info();
    let name = match sharing::clean_name(&name) {
        n if n.is_empty() => sharing::proposed_name(None, &device.name),
        n => n,
    };
    let quality = ShareQuality::parse(&quality).ok_or_else(|| t("errors.unknown_quality", &[]))?;
    let pc = this_pc_of(&state);
    let caps = capabilities(&pc);
    // « Auto » : l'écran de ce PC, tel qu'il est maintenant, et ce que sa carte décode.
    let auto = sharing::join_auto(quality, system::screen_mode(), caps);
    let request = JoinRequest { code, name: name.clone(), device: device.id.clone(), app: "windows".into(), quality, auto, ..JoinRequest::default() };
    log::info!(
        "partage : code envoyé{}, image {}{}",
        if relais::off() { " au réseau seulement (KYBERNOW_RELAY=off)" } else { " au réseau et au relais" },
        quality.as_str(),
        request.auto.as_ref().map(|a| format!(" ({}×{} à {} Hz)", a.width, a.height, a.fps)).unwrap_or_default()
    );
    let joining = if relais::off() {
        // Le réseau seulement : la Station du lien, ou celles que l'app connaît.
        let targets = match &link {
            Some(l) => vec![l.target()],
            None => join_targets(&state),
        };
        share::join_checked(&targets, &request, caps).map(Joining::Joined)
    } else {
        match &link {
            Some(l) => share::join_link(l, &request, Some(caps)),
            None => share::join_anywhere(&join_targets(&state), &join_relays(&state), &request, Some(caps)),
        }
    }
    .map_err(|e| {
        log::info!("partage : refusé ({})", e.kind());
        e.text(i18n::lang())
    })?;
    // Le prénom et la qualité de l'image sont retenus pour la fois suivante.
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.share_name = Some(name.clone());
        p.share_quality = Some(quality);
        p.clone()
    };
    save_prefs(&prefs);

    match joining {
        Joining::Joined(found) => Ok(JoinOutcome::Joined(open_found(&state, found, quality))),
        Joining::Pending(pending) => {
            // Hors de chez soi : l'hôte accepte chaque invité. Une demande déjà en
            // attente (une autre ouverture de « Rejoindre ») est retirée.
            let wait = WAITS.fetch_add(1, Ordering::Relaxed);
            log::info!("partage : demande en attente de l'accord de l'hôte (n°{wait}, par le relais)");
            let old = state.join_wait.lock().unwrap().replace(Wait { id: wait, pending, quality, cancelled: Arc::new(AtomicBool::new(false)) });
            if let Some(old) = old {
                old.cancelled.store(true, Ordering::Relaxed);
                std::thread::spawn(move || share::cancel(&old.pending));
            }
            Ok(JoinOutcome::Pending { wait })
        }
    }
}

/// Le partage trouvé devient la session d'invité à ouvrir (`share_connect`) :
/// le passage de l'accueil montre « Invité sur {nom} » dès maintenant. Un partage
/// rejoint dont la session ne s'est jamais ouverte repart.
fn open_found(state: &UiState, found: Found, quality: ShareQuality) -> Joined {
    let j = &found.joined;
    let host = j.host_name().to_string();
    let computer = if j.computer.trim().is_empty() { host.clone() } else { j.computer.clone() };
    let title = t("share.guest_on", &[("name", &computer)]);
    let session = window::new_app_id();
    let id = format!("guest-{session}");
    log::info!(
        "partage : rejoint, chez {computer} (partage de {host}), {:?}{}",
        j.role,
        if j.route.is_some() { ", par le relais" } else { "" }
    );
    let guest = Arc::new(Guest {
        computer,
        host: host.clone(),
        title: title.clone(),
        role: Mutex::new(j.role),
        quality: Mutex::new(quality),
        image: Mutex::new(j.image.as_ref().and_then(|i| i.label())),
    });
    let joined = Joined { id: id.clone(), title, host, role: j.role.as_str(), fingerprint: found.target.fingerprint.clone() };
    let old = state.pending_guest.lock().unwrap().replace(Pending { id, session, found, guest });
    if let Some(old) = old {
        std::thread::spawn(move || share::leave(&old.found));
    }
    joined
}

/// « En attente de l'accord de l'hôte… » : un appel tenu 25 s au plus que
/// l'interface rappelle (`waiting`) jusqu'à la réponse de l'hôte : accepté
/// (`joined`, la session s'ouvre ensuite par `share_connect`, par le relais), ou
/// le refus en toutes lettres : « L'hôte a refusé. », « L'hôte n'a pas répondu. »
/// (120 s sans réponse), « Le relais ne répond pas. ».
#[tauri::command(async)]
pub fn share_wait(app: AppHandle, wait: u64) -> Result<WaitOutcome, String> {
    let state = app.state::<UiState>();
    let (pending, quality, cancelled) = match state.join_wait.lock().unwrap().as_ref() {
        Some(w) if w.id == wait => (w.pending.clone(), w.quality, w.cancelled.clone()),
        // « Annuler » est déjà passé.
        _ => return Ok(WaitOutcome::Cancelled),
    };
    let pc = this_pc_of(&state);
    let answer = share::wait(&pending, Some(capabilities(&pc)));
    // « Annuler » est passé pendant l'attente : un accord arrivé entre-temps est rendu.
    if cancelled.load(Ordering::Relaxed) {
        if let Ok(Some(found)) = answer {
            std::thread::spawn(move || share::leave(&found));
        }
        return Ok(WaitOutcome::Cancelled);
    }
    let finished = |state: &UiState| {
        let mut w = state.join_wait.lock().unwrap();
        if w.as_ref().is_some_and(|w| w.id == wait) {
            w.take();
        }
    };
    match answer {
        Ok(None) => Ok(WaitOutcome::Waiting),
        Ok(Some(found)) => {
            finished(&state);
            Ok(WaitOutcome::Joined(open_found(&state, found, quality)))
        }
        Err(e) => {
            finished(&state);
            log::info!("partage : demande finie ({})", e.kind());
            Err(e.text(i18n::lang()))
        }
    }
}

/// « Annuler » : la demande en attente est retirée chez l'hôte (`share::cancel`).
#[tauri::command(async)]
pub fn share_cancel(app: AppHandle, wait: u64) {
    cancel_wait(&app.state::<UiState>(), Some(wait));
}

/// Retire la demande en attente (celle-ci, ou n'importe laquelle sans numéro :
/// l'app se ferme) ; bloque le temps de prévenir le relais (6 s au plus).
pub fn cancel_wait(state: &UiState, only: Option<u64>) {
    let taken = {
        let mut w = state.join_wait.lock().unwrap();
        if w.as_ref().is_some_and(|w| only.is_none_or(|id| w.id == id)) { w.take() } else { None }
    };
    if let Some(w) = taken {
        log::info!("partage : demande annulée (n°{})", w.id);
        w.cancelled.store(true, Ordering::Relaxed);
        share::cancel(&w.pending);
    }
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
    // Un invité n'a pas de partage à lui (toujours vide), mais le même fil que les hôtes.
    let share = Arc::new(Mutex::new(None));
    // Par où passe sa session (par le relais quand il est venu de loin).
    let route = Arc::new(Mutex::new(None));
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
            share: share.clone(),
            route: route.clone(),
        }),
    );
    running.push(Running { id: id.clone(), name: guest.title.clone(), host, bar, sent, stats, guest: Some(guest), share, route });
    drop(running);
    drop(_launching);
    // « Quitter » pendant le lancement : elle s'arrête tout de suite, l'app part à sa fin.
    if state.quitting.load(Ordering::Relaxed) {
        end_instance(&state, session, "Quitter pendant le lancement");
    }
    emit_sessions(&app);
    Ok(ConnectInfo { screens: 1, windowed: !first })
}

/// « Qualité de l'image » depuis la barre d'un invité : dite à la Station sans
/// relancer la session (l'image de l'invité seulement repart, celle de l'hôte
/// jamais) et retenue pour la fois suivante. L'image reçue revient par
/// `HostEvents::share_image` (événement `share-image`). Refus : la session n'est
/// pas celle d'un invité, ou pas encore prête.
#[tauri::command(async)]
pub fn share_quality(app: AppHandle, id: String, quality: String) -> Result<(), String> {
    let quality = ShareQuality::parse(&quality).ok_or_else(|| t("errors.unknown_quality", &[]))?;
    let state = app.state::<UiState>();
    let Some(guest) = state.with_session(&id, |r| r.guest.clone()).flatten() else {
        return Err(t("errors.no_session", &[]));
    };
    if !state.with_session(&id, |r| r.host.set_share_quality(quality)).unwrap_or(false) {
        return Err(t("errors.session_not_ready", &[]));
    }
    log::info!("[{}] qualité de l'image demandée : {}", guest.title, quality.as_str());
    *guest.quality.lock().unwrap() = quality;
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.share_quality = Some(quality);
        p.clone()
    };
    save_prefs(&prefs);
    Ok(())
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
    // Copié d'abord : le verrou du partage n'est pas tenu pendant que `view` lit
    // `live` (les deux ne se tiennent jamais ensemble).
    let current = share.lock().unwrap().clone();
    let share = view(app, id, current.as_ref());
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
        (Err(_), Some(o)) => Ok(ShareState { open: true, code: o.code.clone(), link: o.link.clone(), web: o.web.clone(), display: o.display.clone(), guests, ..ShareState::default() }),
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

/// « Donner » ou « Reprendre les manettes », « Donner » ou « Reprendre le clavier
/// et la souris » : le rôle d'un invité, que la barre déduit de celui qu'elle
/// montre en changeant un seul droit (`spectator`, `player`, `kbm`, `player_kbm`).
#[tauri::command(async)]
pub fn share_guest(app: AppHandle, id: String, guest: String, role: String) -> Result<ShareView, String> {
    let role = ShareRole::parse(&role).ok_or_else(|| t("errors.share_code", &[]))?;
    let console = host_console(&app, &id)?;
    lift(console.share_guest(&guest, role))?;
    log::info!("[{id}] invité {guest} : {role:?}");
    let state = state_after(&console, None, Vec::new())?;
    Ok(publish(&app, &id, Some(state)))
}

/// « Accepter » la demande d'un invité venu par le relais : il devient spectateur
/// (sa session s'ouvre chez lui), et le panneau le passe de « Demandes » à « Invités ».
/// Admin refuse si les quatre places sont prises (« Ce partage est complet… »).
#[tauri::command(async)]
pub fn share_accept(app: AppHandle, id: String, guest: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    let state = lift(console.share_accept(&guest))?;
    log::info!("[{id}] demande de {guest} acceptée");
    Ok(publish(&app, &id, Some(state)))
}

/// « Refuser » la demande d'un invité venu par le relais : elle disparaît de
/// « Demandes » ; son appareil ne redemande pas avant 10 minutes.
#[tauri::command(async)]
pub fn share_refuse(app: AppHandle, id: String, guest: String) -> Result<ShareView, String> {
    let console = host_console(&app, &id)?;
    let state = lift(console.share_refuse(&guest))?;
    log::info!("[{id}] demande de {guest} refusée");
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
/// Il entre dans l'historique de Windows (Win+V) : on le copie pour le coller.
#[tauri::command(async)]
pub fn copy_text(text: String) -> Result<(), String> {
    use kybernow_core::input::Pasteboard;
    let board = crate::pasteboard::WinPasteboard;
    board.set_plain_text(&text);
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
/// Appelé depuis le fil du cœur (signe de vie de l'invité) : tout se décide sur
/// le fil de Tauri, comme `top_edge`, pour que le cœur n'attende ni la fenêtre
/// de la barre (`is_visible` attend le fil de Tauri) ni les verrous de l'app.
pub fn notice(app: &AppHandle, id: &str, title: &str, text: &str) {
    notice_with(app, id, title, text, false);
}

/// L'avis d'une demande à rejoindre arrivée pendant la session d'hôte `id`
/// (« {prénom} demande à rejoindre. ») : comme les autres avis, dans la barre quand
/// l'app est devant, sinon dans une notification de Windows ; le choisir (un clic sur
/// l'avis de la barre, sur la notification) ouvre le panneau de « Partager », où
/// « Accepter » et « Refuser » attendent. `title` : le nom de l'ordinateur.
pub fn request_notice(app: &AppHandle, id: &str, title: &str, name: &str) {
    notice_with(app, id, title, &t("share.notice_request", &[("name", name)]), true);
}

/// `request` : l'avis est celui d'une demande, que l'on peut choisir pour ouvrir le panneau.
/// La barre déjà ouverte sur cette session décide elle-même de le montrer (`share-request`) :
/// son panneau de « Partager » ouvert montre déjà la demande, l'avis ne le referme pas.
fn notice_with(app: &AppHandle, id: &str, title: &str, text: &str, request: bool) {
    let (a, id, title, text) = (app.clone(), id.to_string(), title.to_string(), text.to_string());
    let _ = app.run_on_main_thread(move || {
        let state = a.state::<UiState>();
        let bar_shown = a.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
        let bar_here = bar_shown && state.bar_session.lock().unwrap().as_deref() == Some(id.as_str());
        let bar_elsewhere = bar_shown && !bar_here;
        if shell::app_in_front() && !bar_elsewhere {
            if request && bar_here {
                let _ = a.emit_to("bar", "share-request", ShareRequestEvent { id, text });
                return;
            }
            let hwnd = state.with_session(&id, |r| r.host.bar_window()).unwrap_or(0);
            show_bar_notice(&a, &id, hwnd, true, Some(text), request);
        } else {
            let launch = if request { format!("kybernow://share?id={id}") } else { "kybernow://open".to_string() };
            shell::notify_to(&launch, &title, &text, None);
        }
    });
}

/// Le choix d'une notification de demande (`kybernow://share?id=…`) : la fenêtre de cette
/// session passe devant et la barre s'ouvre sur le panneau de « Partager ». Faux quand
/// cette session n'existe plus (la demande est restée dans le centre de notifications) :
/// l'accueil revient alors, comme pour les autres liens.
pub fn open_panel(app: &AppHandle, id: &str) -> bool {
    if app.state::<UiState>().with_session(id, |_| ()).is_none() {
        return false;
    }
    let (a, id) = (app.clone(), id.to_string());
    let _ = app.run_on_main_thread(move || {
        let state = a.state::<UiState>();
        let Some(hwnd) = state.with_session(&id, |r| r.host.bar_window()) else { return };
        // La session passe devant (sa fenêtre prend le premier plan), puis la barre se montre sur elle.
        state.with_session(&id, |r| r.host.focus(hwnd));
        show_bar(&a, &id, hwnd, true);
        let _ = a.emit_to("bar", "share-open-panel", id);
    });
    true
}
