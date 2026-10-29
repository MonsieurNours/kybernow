//! Icône, panneau, menu et boucle d'événements.
//!
//! Clic gauche : le panneau (`panel.rs`), dessiné comme le reste de KyberNow.
//! Clic droit : le menu de Windows, qui sert aussi de secours si WebView2 manque.
//! Le partage d'un écran du PC (KyberNow Share) part de l'un ou de l'autre :
//! le choix de l'écran et le partage ouvert vivent dans le panneau. Une demande
//! pour le rejoindre (un invité venu par le relais) ouvre le panneau sur le partage.
//! Le prêt de l'ordinateur (`docs/pret.md`) de même : « Prêter l'ordinateur… »
//! (le code, dans le panneau), les demandes des amis, « Reprendre l'ordinateur ».
//! Un fil surveille les services et la console toutes les deux secondes ; les
//! actions longues (démarrage, arrêt, invite UAC) tournent dans leur propre fil.
//! Le fil principal ne fait qu'afficher : il relit l'état deux fois par seconde.

use std::collections::HashSet;
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::{Value, json};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDCBF_CANCEL_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION, TaskDialogIndirect,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, MessageBoxW, SW_SHOWNORMAL,
};

use crate::console::{self, Client, LocalState};
use crate::i18n::{lang, list, t, tn};
use crate::icons::{Icons, Look};
use crate::panel::Panel;
use crate::services::{self, ADMIN, KYBER, State};
use crate::lend::host as lend;
use crate::{password, qr, share, startup};

/// Nom affiché : infobulle, boîtes de message, première ligne du menu.
pub const TITLE: &str = "KyberNow Station";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Message d'erreur, depuis n'importe quel fil.
pub fn alert(text: &str) {
    let (text, title) = (wide(text), wide(TITLE));
    unsafe { MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING | MB_SETFOREGROUND | MB_TOPMOST) };
}

fn confirm(text: &str) -> bool {
    let (text, title) = (wide(text), wide(TITLE));
    unsafe { MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), MB_YESNO | MB_ICONWARNING | MB_SETFOREGROUND) == IDYES }
}

fn open_url(url: &str) {
    let (verb, url) = (wide("open"), wide(url));
    unsafe { ShellExecuteW(null_mut(), verb.as_ptr(), url.as_ptr(), null(), null(), SW_SHOWNORMAL) };
}

fn open_console() -> Result<()> {
    open_console_at("")
}

/// Une page de la console (« updates »…), la page d'accueil si vide.
fn open_console_at(page: &str) -> Result<()> {
    let port = console::port().ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissing", &[])))?;
    let url = console::url(port);
    open_url(&if page.is_empty() { url } else { format!("{url}#/{page}") });
    Ok(())
}

/// Ce que l'icône sait de l'ordinateur.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    kyber: State,
    admin: State,
    /// `None` : la console ne répond pas.
    console: Option<LocalState>,
    start_with_computer: Option<bool>,
}

fn poll(client: &Client) -> Snapshot {
    let kyber = services::state(KYBER);
    let admin = services::state(ADMIN);
    let console = if admin == State::Running { console::port().and_then(|p| client.local_state(p)) } else { None };
    Snapshot { kyber, admin, console, start_with_computer: startup::enabled() }
}

/// Le partage vu du panneau, en plus de ce qu'en dit la console : les écrans
/// à choisir, une action en cours, le dernier refus d'Admin.
#[derive(Debug, Clone, Default, PartialEq)]
struct ShareUi {
    /// Écrans du PC, lus à l'ouverture du choix (`None` : en lecture).
    displays: Option<Vec<Value>>,
    busy: bool,
    /// Refus d'Admin (partage coupé, appareil en session…), dans sa langue.
    error: Option<String>,
}

/// Le prêt vu du panneau, en plus de la vue d'Admin : une action en cours,
/// le dernier refus d'Admin.
#[derive(Debug, Clone, Default, PartialEq)]
struct LendUi {
    busy: bool,
    error: Option<String>,
}

/// L'avertissement du prêt (« Votre ami joue dans votre session Windows… »)
/// a été montré : noté dans le profil, il ne revient plus.
fn lend_warned_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| std::path::PathBuf::from(d).join(r"KyberNow\lend\warned"))
}

/// Ce que donne une action du partage.
enum ShareChange {
    Displays(Vec<Value>),
    /// Le partage après l'action (ouverture, nouveau code, droits, invité retiré).
    View(Value),
    Closed,
}

/// Qui occupe l'ordinateur, pour les confirmations (arrêter, redémarrer, quitter).
enum Occupied {
    Free,
    /// Un appareil est en session (avec ou sans invités).
    Device,
    /// Un partage lancé depuis le PC, et les prénoms de ses invités.
    Guests(Vec<String>),
}

#[derive(Default)]
struct Shared {
    snapshot: Option<Snapshot>,
    /// Action en cours (clé de `tray.busy`), affichée à la place de l'état du serveur.
    busy: Option<&'static str>,
    /// Code d'initialisation après « Réinitialiser le mot de passe de la console ».
    setup_code: Option<String>,
    share_ui: ShareUi,
    lend_ui: LendUi,
    lend_warned: bool,
}

/// Le partage a changé (réponse d'une action) : montré sans attendre le prochain relevé.
fn set_share(s: &mut Shared, view: Option<Value>) {
    let view = view.filter(|v| v["open"].as_bool() != Some(false));
    if let Some(console) = s.snapshot.as_mut().and_then(|snap| snap.console.as_mut()) {
        console.share = view;
    }
}

/// Le prêt a changé (réponse d'une action) : montré sans attendre le prochain relevé.
fn set_lend(s: &mut Shared, view: Value) {
    if let Some(console) = s.snapshot.as_mut().and_then(|snap| snap.console.as_mut()) {
        console.lend = Some(view);
    }
}

type SharedState = Arc<Mutex<Shared>>;

fn watch(shared: SharedState, wake: Receiver<()>) {
    let mut client = Client::new();
    loop {
        // Certificat de la console régénéré ou noté depuis (kybernow.json) : client refait.
        if console::fingerprint() != client.fingerprint {
            client = Client::new();
        }
        let snapshot = poll(&client);
        // Le mot de passe a été choisi : le code ne sert plus.
        let showing_code = shared.lock().unwrap().setup_code.is_some();
        let configured = showing_code
            && snapshot.admin == State::Running
            && console::port().and_then(|p| client.configured(p)) == Some(true);
        {
            let mut s = shared.lock().unwrap();
            if configured {
                s.setup_code = None;
            }
            s.snapshot = Some(snapshot);
        }
        match wake.recv_timeout(Duration::from_secs(2)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Textes et apparence pour un état donné.
fn describe(s: &Snapshot, busy: Option<&str>) -> (String, String, Look) {
    let status = match (busy, s.kyber) {
        (Some(key), _) => t(key, &[]),
        (_, State::Running) => t("tray.status.running", &[]),
        (_, State::Starting) => t("tray.status.starting", &[]),
        (_, State::Stopping) => t("tray.status.stopping", &[]),
        (_, State::Stopped) => t("tray.status.stopped", &[]),
        (_, State::Missing) => t("tray.status.missing", &[]),
        (_, State::Unknown) => t("tray.status.unknown", &[]),
    };
    let live = s.console.as_ref().and_then(|c| c.live.as_ref()).and_then(|l| l["name"].as_str()).map(str::to_owned);
    let has_live = live.is_some();
    let session = match (s.kyber, s.admin, &s.console) {
        (State::Running, State::Running, Some(_)) if live.is_some() => t("tray.session.connected", &[("name", &live.unwrap_or_default())]),
        (State::Running, State::Running, Some(LocalState { session: true, user: Some(user), .. })) => t("tray.session.userSession", &[("user", user)]),
        (State::Running, State::Running, Some(LocalState { session: true, user: None, .. })) => t("tray.session.session", &[]),
        (State::Running, State::Running, Some(_)) => t("tray.session.none", &[]),
        (State::Running, State::Running, None) => t("tray.session.consoleSilent", &[]),
        (State::Running, _, _) => t("tray.session.consoleStopped", &[]),
        (State::Stopped, _, _) => t("tray.session.cannotConnect", &[]),
        (State::Missing, _, _) => t("tray.session.reinstall", &[]),
        _ => t("tray.session.wait", &[]),
    };
    // Partage ouvert : l'écran partagé depuis le PC, ou les invités de l'appareil
    // hôte ; ses demandes à côté (« 2 invités · 1 demande »), seulement s'il y en a.
    let (guests, requests) = s.console.as_ref().map_or((0, 0), |c| (c.guests(), c.requests()));
    let counts: Vec<String> =
        [(guests > 0).then(|| tn("tray.guests", guests)), (requests > 0).then(|| tn("tray.requests", requests))].into_iter().flatten().collect();
    let session = match s.console.as_ref().and_then(|c| c.share.as_ref()) {
        Some(share) if s.kyber == State::Running && !has_live && share["origin"] == "local" => {
            let base = t("tray.session.sharing", &[("n", &share["display"]["index"])]);
            std::iter::once(base).chain(counts).collect::<Vec<_>>().join(" · ")
        }
        Some(_) if s.kyber == State::Running && !counts.is_empty() => format!("{session} · {}", counts.join(" · ")),
        _ => session,
    };
    // Ordinateur prêté : « Prêté à Léa jusqu'à 18 h », « Prêté à Léa · Reprise dans 1:42 ».
    let lent = s.console.as_ref().and_then(|c| c.lend.as_ref()).filter(|l| lend::active(l));
    let session = match lent {
        Some(l) if s.kyber == State::Running => {
            let until = l["borrower"]["until"].as_i64().and_then(|u| crate::lend::clock_at(lang(), u));
            lend::status_line(lang(), l, until.as_deref(), crate::lend::unix_now()).unwrap_or(session)
        }
        _ => session,
    };
    let in_session = s.console.as_ref().is_some_and(|c| c.device_session() || c.guests() > 0) || lent.is_some();
    let look = match s.kyber {
        _ if busy.is_some() => Look::Busy,
        State::Starting | State::Stopping => Look::Busy,
        State::Running if in_session => Look::Session,
        State::Running => Look::Running,
        _ => Look::Stopped,
    };
    (status, session, look)
}

fn state_name(s: State) -> &'static str {
    match s {
        State::Running => "running",
        State::Starting => "starting",
        State::Stopping => "stopping",
        State::Stopped => "stopped",
        State::Missing => "missing",
        State::Unknown => "unknown",
    }
}

/// Le partage tel que le panneau l'affiche, avec le QR code de son lien.
fn share_for_panel(share: &Value) -> Value {
    let mut share = share.clone();
    share["qr"] = share["link"].as_str().and_then(qr::svg).map_or(Value::Null, Value::from);
    share
}

/// Le prêt tel que le panneau l'affiche : le QR code du lien du code, et
/// l'heure de fin déjà écrite (« 18 h »).
fn lend_for_panel(view: &Value) -> Value {
    let mut view = view.clone();
    if let Some(link) = view["code"]["link"].as_str() {
        view["code"]["qr"] = qr::svg(link).map_or(Value::Null, Value::from);
    }
    if view["borrower"].is_object() {
        view["borrower"]["untilText"] = view["borrower"]["until"].as_i64().and_then(|u| crate::lend::clock_at(lang(), u)).map_or(Value::Null, Value::from);
    }
    view
}

/// L'état tel que le panneau l'affiche.
fn panel_state(s: &Snapshot, busy: Option<&str>, setup_code: Option<&str>, share_ui: &ShareUi, lend_ui: &LendUi, lend_warned: bool) -> Value {
    json!({
        "service": state_name(s.kyber),
        "adminMissing": s.admin == State::Missing,
        "busy": busy.map(|key| t(key, &[])),
        "startWithComputer": s.start_with_computer,
        "setupCode": setup_code,
        "version": env!("CARGO_PKG_VERSION"),
        "console": s.console.as_ref().map(|c| json!({
            "pairRequests": c.pair_requests,
            "pendingAccess": c.pending_access,
            "session": c.session,
            "user": c.user,
            "update": c.update,
            "version": c.version,
            "hostname": c.hostname,
            "address": c.address,
            "live": c.live,
            "last": c.last,
            "share": c.share.as_ref().map(share_for_panel),
            "shareEnabled": c.share_enabled,
            "lend": c.lend.as_ref().map(lend_for_panel),
            "relay": c.relay,
        })),
        "shareUi": {
            "displays": share_ui.displays,
            "busy": share_ui.busy,
            "error": share_ui.error,
        },
        "lendUi": {
            "busy": lend_ui.busy,
            "error": lend_ui.error,
            "warned": lend_warned,
        },
    })
}

/// Ce que l'icône a affiché : l'état, l'action en cours, le code
/// d'initialisation, le partage et le prêt vus du panneau, la ligne de la session.
type Shown = (Snapshot, Option<&'static str>, Option<String>, ShareUi, LendUi, String);

/// Événements de l'icône, du menu et du panneau, ramenés dans la boucle.
pub enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Panel(String),
}

struct App {
    tray: TrayIcon,
    icons: Icons,
    panel: Option<Panel>,
    menu: Menu,
    status: MenuItem,
    session: MenuItem,
    /// « Relais joint », « Relais injoignable (…) » sous la session, seulement
    /// quand le relais compte (`relay_shown` : la ligne est dans le menu).
    relay: MenuItem,
    relay_shown: bool,
    /// « Partager un écran… », ou « Arrêter le partage » quand un partage est ouvert.
    share: MenuItem,
    /// « Prêter l'ordinateur… », ou pendant un prêt « Reprendre l'ordinateur »
    /// (« Reprendre tout de suite » pendant le préavis).
    lend: MenuItem,
    open: MenuItem,
    updates: MenuItem,
    toggle: MenuItem,
    restart: MenuItem,
    start_with_computer: CheckMenuItem,
    reset: MenuItem,
    quit: MenuItem,
    shared: SharedState,
    wake: Sender<()>,
    /// Ce qui est affiché ; la dernière chaîne : la ligne du prêt (son décompte
    /// change chaque seconde sans que rien d'autre ne change).
    shown: Option<Shown>,
    look: Option<Look>,
    /// Demandes d'accès déjà signalées (le panneau s'ouvre pour les nouvelles).
    seen_requests: HashSet<String>,
    /// Demandes pour rejoindre le partage déjà signalées (de même).
    seen_asks: HashSet<String>,
    /// Demandes d'emprunt (et préavis d'un créneau) déjà signalés (de même).
    seen_lend: HashSet<String>,
    done: bool,
}

pub fn run() -> Result<()> {
    // Une seule icône par session ; une seconde ouverture (menu Démarrer) ouvre la console.
    let name = wide(r"Local\KyberNow.Tray");
    let mutex = unsafe { CreateMutexW(null(), 1, name.as_ptr()) };
    if !mutex.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return open_console();
    }

    // Un prêt mal fini (agent tombé, PC redémarré en plein prêt) : remis en état.
    std::thread::spawn(crate::lend::agent::repair_at_start);

    let shared: SharedState = Arc::new(Mutex::new(Shared {
        lend_warned: lend_warned_path().is_some_and(|p| p.exists()),
        ..Default::default()
    }));
    let (wake, wake_rx) = mpsc::channel();
    {
        let shared = shared.clone();
        std::thread::spawn(move || watch(shared, wake_rx));
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    {
        let proxy = Mutex::new(proxy.clone());
        TrayIconEvent::set_event_handler(Some(move |e| {
            let _ = proxy.lock().unwrap().send_event(UserEvent::Tray(e));
        }));
    }
    {
        let proxy = Mutex::new(proxy.clone());
        MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.lock().unwrap().send_event(UserEvent::Menu(e));
        }));
    }

    // Sans WebView2, pas de panneau : le menu s'ouvre aussi au clic gauche.
    let panel = match Panel::new(&event_loop, proxy.clone()) {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("panneau indisponible : {e:#}");
            None
        }
    };

    let status = MenuItem::new(TITLE, false, None);
    let session = MenuItem::new(t("tray.session.wait", &[]), false, None);
    let relay = MenuItem::new("", false, None);
    let share = MenuItem::new(t("tray.menu.share", &[]), false, None);
    let lend = MenuItem::new(t("tray.menu.lend", &[]), false, None);
    let open = MenuItem::new(t("tray.menu.openConsole", &[]), true, None);
    let updates = MenuItem::new(t("tray.menu.updates", &[]), true, None);
    let toggle = MenuItem::new(t("tray.menu.stopServer", &[]), false, None);
    let restart = MenuItem::new(t("tray.menu.restartServer", &[]), false, None);
    let start_with_computer = CheckMenuItem::new(t("tray.menu.startWithComputer", &[]), false, false, None);
    let reset = MenuItem::new(t("tray.menu.resetPassword", &[]), true, None);
    let quit = MenuItem::new(t("tray.menu.quit", &[]), true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &session,
        &PredefinedMenuItem::separator(),
        &share,
        &lend,
        &PredefinedMenuItem::separator(),
        &open,
        &updates,
        &PredefinedMenuItem::separator(),
        &toggle,
        &restart,
        &PredefinedMenuItem::separator(),
        &start_with_computer,
        &reset,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let icons = Icons::load()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu.clone()))
        .with_menu_on_left_click(panel.is_none())
        .with_tooltip(TITLE)
        .with_icon(icons.get(Look::Stopped)?)
        .build()?;

    let mut app = App {
        tray,
        icons,
        panel,
        menu,
        status,
        session,
        relay,
        relay_shown: false,
        share,
        lend,
        open,
        updates,
        toggle,
        restart,
        start_with_computer,
        reset,
        quit,
        shared,
        wake,
        shown: None,
        look: None,
        seen_requests: Default::default(),
        seen_asks: Default::default(),
        seen_lend: Default::default(),
        done: false,
    };

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                app.refresh();
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(500));
            }
            Event::UserEvent(UserEvent::Tray(e)) => app.on_tray(e),
            Event::UserEvent(UserEvent::Menu(e)) => app.on_menu(e),
            Event::UserEvent(UserEvent::Panel(json)) => app.on_panel(&json),
            Event::WindowEvent { window_id, event: WindowEvent::Focused(false), .. }
                if app.panel.as_ref().is_some_and(|p| p.id() == window_id) =>
            {
                if let Some(p) = app.panel.as_mut() {
                    p.hide();
                }
            }
            _ => {}
        }
        if app.done {
            *control_flow = ControlFlow::Exit;
        }
    });
}

impl App {
    fn snapshot(&self) -> (Option<Snapshot>, Option<&'static str>, Option<String>, ShareUi) {
        let s = self.shared.lock().unwrap();
        (s.snapshot.clone(), s.busy, s.setup_code.clone(), s.share_ui.clone())
    }

    fn lend_ui(&self) -> (LendUi, bool) {
        let s = self.shared.lock().unwrap();
        (s.lend_ui.clone(), s.lend_warned)
    }

    fn refresh(&mut self) {
        if let Some(p) = self.panel.as_mut() {
            p.tick();
        }
        let (Some(snap), busy, code, share_ui) = self.snapshot() else { return };
        let (lend_ui, lend_warned) = self.lend_ui();
        let (status, session, look) = describe(&snap, busy);
        if self.shown.as_ref().is_some_and(|(s, b, c, u, l, line)| *s == snap && *b == busy && *c == code && *u == share_ui && *l == lend_ui && *line == session) {
            return;
        }
        let idle = busy.is_none();
        self.status.set_text(&status);
        self.session.set_text(&session);
        self.toggle.set_text(t(if snap.kyber.is_up() { "tray.menu.stopServer" } else { "tray.menu.startServer" }, &[]));
        self.toggle.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.restart.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.start_with_computer.set_checked(snap.start_with_computer == Some(true));
        self.start_with_computer.set_enabled(idle && snap.start_with_computer.is_some());
        self.reset.set_enabled(idle && snap.admin != State::Missing);
        self.open.set_enabled(snap.admin != State::Missing);
        let update = snap.console.as_ref().and_then(|c| c.update.clone());
        self.updates.set_text(match &update {
            Some(v) => t("tray.menu.installVersion", &[("version", v)]),
            None => t("tray.menu.updates", &[]),
        });
        self.updates.set_enabled(snap.admin == State::Running);
        // Partager un écran du PC : pas pendant la session d'un appareil (c'est lui
        // qui partage), ni quand le partage est coupé ; un partage ouvert s'arrête d'ici.
        let console = snap.console.as_ref().filter(|_| snap.kyber == State::Running);
        let share_open = console.is_some_and(|c| c.share.is_some());
        self.share.set_text(t(if share_open { "tray.menu.stopShare" } else { "tray.menu.share" }, &[]));
        self.share.set_enabled(
            idle && !share_ui.busy && console.is_some_and(|c| c.share.is_some() || (c.share_enabled && !c.device_session())),
        );
        // Prêter l'ordinateur : quand le prêt est activé dans Admin ; pendant un
        // prêt, le reprendre d'ici (tout de suite pendant le préavis).
        let lend_view = console.and_then(|c| c.lend.as_ref());
        let lent = lend_view.is_some_and(lend::active);
        let notice = lend_view.and_then(lend::notice_end).is_some();
        self.lend.set_text(t(
            match (lent, notice) {
                (true, true) => "tray.menu.takeBackNow",
                (true, false) => "tray.menu.takeBack",
                _ => "tray.menu.lend",
            },
            &[],
        ));
        self.lend.set_enabled(idle && !lend_ui.busy && lend_view.is_some_and(|l| lent || l["enabled"] == true));
        // L'état du relais, sous la session, quand il compte : joint, ou injoignable.
        let relay = console.and_then(|c| c.relay.as_ref()).and_then(|r| share::relay_line(lang(), r));
        if let Some(text) = &relay {
            self.relay.set_text(text);
        }
        if relay.is_some() != self.relay_shown {
            let changed = if relay.is_some() { self.menu.insert(&self.relay, 2) } else { self.menu.remove(&self.relay) };
            self.relay_shown = if changed.is_ok() { relay.is_some() } else { self.relay_shown };
        }
        let _ = self.tray.set_tooltip(Some(format!("{TITLE}\n{status}\n{session}")));
        if self.look != Some(look) {
            if let Ok(icon) = self.icons.get(look) {
                let _ = self.tray.set_icon(Some(icon));
                self.look = Some(look);
            }
        }
        // Une demande nouvelle, une fois chacune : le panneau s'ouvre de lui-même
        // pour que la personne devant l'ordinateur la voie. Un appareil demande
        // l'accès : l'accueil ; un invité venu par le relais demande à rejoindre
        // le partage lancé depuis le PC : la page du partage.
        let requests: HashSet<String> = snap
            .console
            .as_ref()
            .map(|c| c.pair_requests.iter().filter_map(|r| r["id"].as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let asks: HashSet<String> =
            console.and_then(|c| c.share.as_ref()).map(|s| share::local_request_ids(s).into_iter().collect()).unwrap_or_default();
        // Un ami veut emprunter l'ordinateur, ou son créneau commence : la page du prêt.
        let lend_asks: HashSet<String> = lend_view.map(|l| lend::request_ids(l).into_iter().chain(lend::slot_start(l)).collect()).unwrap_or_default();
        let fresh = requests.iter().any(|id| !self.seen_requests.contains(id));
        let fresh_ask = asks.iter().any(|id| !self.seen_asks.contains(id));
        let fresh_lend = lend_asks.iter().any(|id| !self.seen_lend.contains(id));
        self.seen_requests = requests;
        self.seen_asks = asks;
        self.seen_lend = lend_asks;
        if fresh || fresh_ask || fresh_lend {
            self.reveal_panel();
        }
        // Panneau ouvert (et à l'instant pour une demande) : l'état à jour, puis la page.
        if let Some(panel) = self.panel.as_mut() {
            panel.push(&panel_state(&snap, busy, code.as_deref(), &share_ui, &lend_ui, lend_warned).to_string());
            if fresh_lend {
                panel.show_page("lend");
            } else if fresh_ask {
                panel.show_page("share");
            }
        }
        self.shown = Some((snap, busy, code, share_ui, lend_ui, session));
    }

    /// Ouvre le panneau contre l'icône s'il est fermé.
    fn reveal_panel(&mut self) {
        if let (Some(panel), Some(rect)) = (self.panel.as_mut(), self.tray.rect())
            && !panel.is_visible()
        {
            panel.toggle(rect.position, (rect.size.width, rect.size.height));
        }
    }

    /// Lance une action longue dans son propre fil (`label` : clé de `tray.busy`).
    fn act(&mut self, label: &'static str, job: impl FnOnce(&SharedState) -> Result<()> + Send + 'static) {
        {
            let mut s = self.shared.lock().unwrap();
            if s.busy.is_some() {
                return;
            }
            s.busy = Some(label);
        }
        let (shared, wake) = (self.shared.clone(), self.wake.clone());
        std::thread::spawn(move || {
            let result = job(&shared);
            shared.lock().unwrap().busy = None;
            let _ = wake.send(());
            if let Err(e) = result {
                alert(&format!("{e:#}"));
            }
        });
        self.refresh();
    }

    /// Une action du partage dans son propre fil, sans bloquer l'icône : le
    /// panneau montre l'attente, puis le partage à jour, ou le refus d'Admin
    /// (dans le panneau s'il est ouvert, sinon dans une boîte de message).
    fn share_act(&mut self, job: impl FnOnce(&Client, u16) -> Result<ShareChange> + Send + 'static) {
        {
            let mut s = self.shared.lock().unwrap();
            if s.share_ui.busy {
                return;
            }
            s.share_ui.busy = true;
            s.share_ui.error = None;
        }
        let inline = self.panel.as_ref().is_some_and(Panel::is_visible);
        let (shared, wake) = (self.shared.clone(), self.wake.clone());
        std::thread::spawn(move || {
            let result = console::port()
                .ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissingShort", &[])))
                .and_then(|port| job(&Client::new(), port));
            let mut failure = None;
            {
                let mut s = shared.lock().unwrap();
                s.share_ui.busy = false;
                match result {
                    Ok(ShareChange::Displays(list)) => s.share_ui.displays = Some(list),
                    Ok(ShareChange::View(view)) => set_share(&mut s, Some(view)),
                    Ok(ShareChange::Closed) => set_share(&mut s, None),
                    Err(e) if inline => s.share_ui.error = Some(format!("{e:#}")),
                    Err(e) => failure = Some(format!("{e:#}")),
                }
            }
            let _ = wake.send(());
            if let Some(text) = failure {
                alert(&text);
            }
        });
        self.refresh();
    }

    /// Une action du prêt dans son propre fil, comme [`App::share_act`] ; la
    /// vue du prêt qu'elle rend est montrée aussitôt. `warned` : l'action vaut
    /// lecture de l'avertissement (un code créé, une demande acceptée).
    fn lend_act(&mut self, warned: bool, job: impl FnOnce(&Client, u16) -> Result<Value> + Send + 'static) {
        {
            let mut s = self.shared.lock().unwrap();
            if s.lend_ui.busy {
                return;
            }
            s.lend_ui.busy = true;
            s.lend_ui.error = None;
        }
        let inline = self.panel.as_ref().is_some_and(Panel::is_visible);
        let (shared, wake) = (self.shared.clone(), self.wake.clone());
        std::thread::spawn(move || {
            let result = console::port()
                .ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissingShort", &[])))
                .and_then(|port| job(&Client::new(), port));
            let mut failure = None;
            {
                let mut s = shared.lock().unwrap();
                s.lend_ui.busy = false;
                match result {
                    Ok(view) => {
                        set_lend(&mut s, view);
                        if warned && !s.lend_warned {
                            s.lend_warned = true;
                            if let Some(path) = lend_warned_path() {
                                let _ = path.parent().map(std::fs::create_dir_all);
                                let _ = std::fs::write(path, b"");
                            }
                        }
                    }
                    Err(e) if inline => s.lend_ui.error = Some(format!("{e:#}")),
                    Err(e) => failure = Some(format!("{e:#}")),
                }
            }
            let _ = wake.send(());
            if let Some(text) = failure {
                alert(&text);
            }
        });
        self.refresh();
    }

    /// « Partager un écran… » du menu : le panneau s'ouvre sur le choix de
    /// l'écran (sans panneau, l'accueil d'Admin, qui a le même choix).
    fn open_share_choice(&mut self) {
        self.open_page("choose", "ensemble");
    }

    /// Une page du panneau, ouvert s'il est fermé ; sans panneau, la page
    /// d'Admin qui fait la même chose.
    fn open_page(&mut self, page: &'static str, console_page: &str) {
        match (self.panel.as_mut(), self.tray.rect()) {
            (Some(panel), Some(rect)) => {
                if !panel.is_visible() && panel.toggle(rect.position, (rect.size.width, rect.size.height)) {
                    self.shown = None;
                    let _ = self.wake.send(());
                }
                panel.show_page(page);
                self.refresh();
            }
            _ => {
                if let Err(e) = open_console_at(console_page) {
                    alert(&format!("{e:#}"));
                }
            }
        }
    }

    fn lend_view(&self) -> Option<Value> {
        let (snap, _, _, _) = self.snapshot();
        snap.and_then(|s| s.console).and_then(|c| c.lend)
    }

    fn share_open(&self) -> bool {
        let (snap, _, _, _) = self.snapshot();
        snap.and_then(|s| s.console).is_some_and(|c| c.share.is_some())
    }

    fn occupied(&self) -> Occupied {
        let (snap, _, _, _) = self.snapshot();
        match snap.and_then(|s| s.console) {
            Some(c) if c.device_session() => Occupied::Device,
            Some(c) if c.guests() > 0 => Occupied::Guests(c.guest_names()),
            _ => Occupied::Free,
        }
    }

    fn hide_panel(&mut self) {
        if let Some(p) = self.panel.as_mut() {
            p.hide();
        }
    }

    fn on_tray(&mut self, event: TrayIconEvent) {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
            if let Some(p) = self.panel.as_mut() {
                if p.toggle(rect.position, (rect.size.width, rect.size.height)) {
                    // État à jour dès l'ouverture.
                    self.shown = None;
                    let _ = self.wake.send(());
                    self.refresh();
                }
            }
        }
    }

    fn on_menu(&mut self, event: MenuEvent) {
        let id = event.id;
        if id == *self.share.id() {
            if self.share_open() {
                self.share_act(|c, port| c.share_stop(port).map(|()| ShareChange::Closed));
            } else {
                self.open_share_choice();
            }
        } else if id == *self.lend.id() {
            // Pendant un prêt : le reprendre (tout de suite pendant le préavis) ;
            // sinon, le panneau du code.
            match self.lend_view().filter(lend::active) {
                Some(l) => {
                    let now = lend::notice_end(&l).is_some();
                    self.lend_act(false, move |c, port| c.lend_take_back(port, now));
                }
                None => self.open_page("lend", "pret"),
            }
        } else if id == *self.updates.id() {
            self.open_updates();
        } else if id == *self.open.id() {
            self.open_console();
        } else if id == *self.toggle.id() {
            self.toggle_server();
        } else if id == *self.restart.id() {
            self.restart_server();
        } else if id == *self.start_with_computer.id() {
            // Le menu a déjà coché ou décoché la case : on la remet selon l'état réel.
            self.shown = None;
            self.switch_start_with_computer();
        } else if id == *self.reset.id() {
            self.reset_password();
        } else if id == *self.quit.id() {
            self.quit();
        }
    }

    fn on_panel(&mut self, json: &str) {
        let Ok(message) = serde_json::from_str::<Value>(json) else { return };
        match message["cmd"].as_str().unwrap_or_default() {
            "ready" => {
                if let Some(p) = self.panel.as_mut() {
                    p.mark_ready();
                }
                self.shown = None;
                self.refresh();
            }
            "size" => {
                if let (Some(p), Some(h)) = (self.panel.as_mut(), message["height"].as_f64()) {
                    p.resize(h);
                }
            }
            "hide" => self.hide_panel(),
            "openConsole" => {
                self.hide_panel();
                self.open_console();
            }
            "openUpdates" => {
                self.hide_panel();
                self.open_updates();
            }
            // Le code d'appairage ne passe pas par l'icône : il se compare dans la console.
            "openPairing" => {
                self.hide_panel();
                if let Err(e) = open_console_at("appareils") {
                    alert(&format!("{e:#}"));
                }
            }
            "endSession" => self.end_session(),
            "pairAnswer" => {
                if let Some(id) = message["id"].as_str() {
                    self.pair_answer(id.to_string(), message["accept"].as_bool().unwrap_or(false));
                }
            }
            // Partage d'un écran du PC (KyberNow Share).
            "shareChoose" => {
                self.shared.lock().unwrap().share_ui.displays = None;
                self.share_act(|c, port| c.displays(port).map(ShareChange::Displays));
            }
            // L'écran choisi, par sa place dans la liste lue pour le choix (son
            // identifiant passe tel quel, sans détour par un nombre JavaScript).
            "shareOpen" => {
                let display = message["index"].as_u64().and_then(|i| {
                    let s = self.shared.lock().unwrap();
                    s.share_ui.displays.as_ref()?.get(i as usize).map(|d| d["id"].clone())
                });
                if let Some(display) = display {
                    self.share_act(move |c, port| c.share_open(port, display).map(ShareChange::View));
                }
            }
            "shareCode" => self.share_act(|c, port| c.share_code(port).map(ShareChange::View)),
            "shareRole" => {
                if let (Some(id), Some(role)) = (message["id"].as_str(), message["role"].as_str()) {
                    let (id, role) = (id.to_string(), role.to_string());
                    self.share_act(move |c, port| c.share_role(port, &id, &role).map(ShareChange::View));
                }
            }
            "shareRemove" => {
                if let Some(id) = message["id"].as_str() {
                    let id = id.to_string();
                    self.share_act(move |c, port| c.share_remove(port, &id).map(ShareChange::View));
                }
            }
            // Une demande (un invité venu par le relais) : accepter ou refuser.
            "shareAnswer" => {
                if let Some(id) = message["id"].as_str() {
                    let (id, accept) = (id.to_string(), message["accept"].as_bool() == Some(true));
                    self.share_act(move |c, port| c.share_answer(port, &id, accept).map(ShareChange::View));
                }
            }
            "shareStop" => self.share_act(|c, port| c.share_stop(port).map(|()| ShareChange::Closed)),
            "shareDismiss" => {
                self.shared.lock().unwrap().share_ui.error = None;
                self.refresh();
            }
            // Prêt de l'ordinateur : le code, les demandes, la reprise.
            "lendCode" => {
                if let Some(duration) = lend::duration(&message["duration"]) {
                    let steam = lend::steam(&message["steam"]);
                    self.lend_act(true, move |c, port| c.lend_code(port, duration, steam));
                }
            }
            "lendCancel" => self.lend_act(false, |c, port| c.lend_cancel_code(port)),
            "lendAnswer" => {
                if let Some(id) = message["id"].as_str() {
                    let (id, accept, steam) = (id.to_string(), message["accept"].as_bool() == Some(true), lend::steam(&message["steam"]));
                    self.lend_act(accept, move |c, port| c.lend_answer(port, &id, accept, steam));
                }
            }
            "lendTakeBack" => {
                let now = message["now"].as_bool() == Some(true);
                self.lend_act(false, move |c, port| c.lend_take_back(port, now));
            }
            "lendDismiss" => {
                self.shared.lock().unwrap().lend_ui.error = None;
                self.refresh();
            }
            "restart" => self.restart_server(),
            "toggleServer" => self.toggle_server(),
            "startWithComputer" => self.switch_start_with_computer(),
            "resetPassword" => self.reset_password(),
            "quit" => {
                self.hide_panel();
                self.quit();
            }
            _ => {}
        }
    }

    fn open_console(&mut self) {
        let (Some(snap), _, _, _) = self.snapshot() else { return };
        if snap.admin == State::Running {
            if let Err(e) = open_console() {
                alert(&format!("{e:#}"));
            }
        } else {
            self.act("tray.busy.startingConsole", |_| {
                services::start(ADMIN)?;
                services::wait(ADMIN, State::Running, Duration::from_secs(30))?;
                std::thread::sleep(Duration::from_secs(1));
                open_console()
            });
        }
    }

    fn open_updates(&mut self) {
        if let Err(e) = open_console_at("systeme") {
            alert(&format!("{e:#}"));
        }
    }

    fn pair_answer(&mut self, id: String, accept: bool) {
        self.act(if accept { "tray.busy.granting" } else { "tray.busy.refusing" }, move |_| {
            let port = console::port().ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissingShort", &[])))?;
            Client::new().pair_answer(port, &id, accept)
        });
    }

    fn end_session(&mut self) {
        if !confirm(&t("tray.confirm.endSession", &[])) {
            return;
        }
        self.act("tray.busy.endingSession", |_| {
            let port = console::port().ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissingShort", &[])))?;
            Client::new().end_session(port)
        });
    }

    fn toggle_server(&mut self) {
        let (Some(snap), _, _, _) = self.snapshot() else { return };
        if snap.kyber.is_up() {
            let sure = match self.occupied() {
                Occupied::Free => true,
                Occupied::Device => confirm(&t("tray.confirm.stopInSession", &[])),
                Occupied::Guests(names) => confirm(&t("tray.confirm.stopSharing", &[("names", &list(&names))])),
            };
            if !sure {
                return;
            }
            self.act("tray.busy.stopping", |_| services::stop_server());
        } else {
            self.act("tray.busy.starting", |_| services::start_server());
        }
    }

    fn restart_server(&mut self) {
        let sure = match self.occupied() {
            Occupied::Free => true,
            Occupied::Device => confirm(&t("tray.confirm.restartInSession", &[])),
            Occupied::Guests(names) => confirm(&t("tray.confirm.restartSharing", &[("names", &list(&names))])),
        };
        if !sure {
            return;
        }
        self.act("tray.busy.restarting", |_| services::restart_server());
    }

    fn switch_start_with_computer(&mut self) {
        let (Some(snap), _, _, _) = self.snapshot() else { return };
        let on = snap.start_with_computer != Some(true);
        self.act("tray.busy.startup", move |_| startup::set(on).map(|_| ()));
    }

    /// Efface le mot de passe de la console (invite UAC), montre le code et ouvre la console.
    fn reset_password(&mut self) {
        if !confirm(&t("tray.confirm.resetPassword", &[])) {
            return;
        }
        self.act("tray.busy.resetting", |shared| {
            if let Some(code) = password::reset()? {
                shared.lock().unwrap().setup_code = Some(code);
                open_console()?;
            }
            Ok(())
        });
    }

    fn quit(&mut self) {
        let (Some(snap), _, _, _) = self.snapshot() else {
            self.done = true;
            return;
        };
        if !snap.kyber.is_up() {
            self.done = true;
            return;
        }
        match ask_quit(self.occupied()) {
            QuitChoice::KeepServer => self.done = true,
            QuitChoice::StopServer => {
                if let Err(e) = services::stop(KYBER) {
                    alert(&format!("{e:#}"));
                }
                self.done = true;
            }
            QuitChoice::Cancel => {}
        }
    }
}

enum QuitChoice {
    KeepServer,
    StopServer,
    Cancel,
}

const KEEP: i32 = 100;
const STOP: i32 = 101;

fn ask_quit(occupied: Occupied) -> QuitChoice {
    let title = wide(TITLE);
    let instruction = wide(&t("tray.quit.instruction", &[]));
    let content = wide(&match occupied {
        Occupied::Free => t("tray.quit.idle", &[]),
        Occupied::Device => t("tray.quit.inSession", &[]),
        Occupied::Guests(names) => t("tray.quit.sharing", &[("names", &list(&names))]),
    });
    let keep = wide(&t("tray.quit.keep", &[]));
    let stop = wide(&t("tray.quit.stop", &[]));
    let buttons = [
        TASKDIALOG_BUTTON { nButtonID: KEEP, pszButtonText: keep.as_ptr() },
        TASKDIALOG_BUTTON { nButtonID: STOP, pszButtonText: stop.as_ptr() },
    ];
    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = size_of::<TASKDIALOGCONFIG>() as u32;
    config.hInstance = unsafe { GetModuleHandleW(null()) };
    config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION;
    config.dwCommonButtons = TDCBF_CANCEL_BUTTON;
    config.pszWindowTitle = title.as_ptr();
    config.Anonymous1.pszMainIcon = 1 as *const u16; // icône de l'exécutable (ressource 1)
    config.pszMainInstruction = instruction.as_ptr();
    config.pszContent = content.as_ptr();
    config.cButtons = buttons.len() as u32;
    config.pButtons = buttons.as_ptr();
    config.nDefaultButton = KEEP;
    let mut pressed = 0i32;
    let hr = unsafe { TaskDialogIndirect(&config, &mut pressed, null_mut(), null_mut()) };
    if hr < 0 {
        // Sans boîte de dialogue moderne : la question simple.
        return if confirm(&t("tray.quit.fallback", &[])) {
            QuitChoice::StopServer
        } else {
            QuitChoice::KeepServer
        };
    }
    match pressed {
        KEEP => QuitChoice::KeepServer,
        STOP => QuitChoice::StopServer,
        _ => QuitChoice::Cancel,
    }
}
