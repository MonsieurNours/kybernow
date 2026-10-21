//! Icône, panneau, menu et boucle d'événements.
//!
//! Clic gauche : le panneau (`panel.rs`), dessiné comme le reste de KyberNow.
//! Clic droit : le menu de Windows, qui sert aussi de secours si WebView2 manque.
//! Un fil surveille les services et la console toutes les deux secondes ; les
//! actions longues (démarrage, arrêt, invite UAC) tournent dans leur propre fil.
//! Le fil principal ne fait qu'afficher : il relit l'état deux fois par seconde.

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
use crate::icons::{Icons, Look};
use crate::panel::Panel;
use crate::services::{self, ADMIN, KYBER, State};
use crate::{password, startup};

const TITLE: &str = "KyberNow";

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
    let port = console::port().ok_or_else(|| anyhow::anyhow!("Le port de la console est introuvable : KyberNow est-il installé ?"))?;
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

#[derive(Default)]
struct Shared {
    snapshot: Option<Snapshot>,
    /// Action en cours, affichée à la place de l'état du serveur.
    busy: Option<&'static str>,
    /// Code d'initialisation après « Réinitialiser le mot de passe de la console ».
    setup_code: Option<String>,
}

type SharedState = Arc<Mutex<Shared>>;

fn watch(shared: SharedState, wake: Receiver<()>) {
    let client = Client::new();
    loop {
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
        (Some(label), _) => label,
        (_, State::Running) => "Serveur en marche",
        (_, State::Starting) => "Démarrage du serveur…",
        (_, State::Stopping) => "Arrêt du serveur…",
        (_, State::Stopped) => "Serveur arrêté",
        (_, State::Missing) => "Serveur non installé",
        (_, State::Unknown) => "État du serveur inconnu",
    };
    let live = s.console.as_ref().and_then(|c| c.live.as_ref()).and_then(|l| l["name"].as_str()).map(str::to_owned);
    let session = match (s.kyber, s.admin, &s.console) {
        (State::Running, State::Running, Some(_)) if live.is_some() => format!("{} est connecté", live.unwrap_or_default()),
        (State::Running, State::Running, Some(LocalState { session: true, user: Some(user), .. })) => format!("Session en cours ({user})"),
        (State::Running, State::Running, Some(LocalState { session: true, user: None, .. })) => "Session en cours".into(),
        (State::Running, State::Running, Some(_)) => "Aucune session en cours".into(),
        (State::Running, State::Running, None) => "La console ne répond pas".into(),
        (State::Running, _, _) => "Console arrêtée".into(),
        (State::Stopped, _, _) => "Vos appareils ne peuvent pas se connecter".into(),
        (State::Missing, _, _) => "Réinstallez KyberNow".into(),
        _ => "Patientez…".into(),
    };
    let in_session = s.console.as_ref().is_some_and(|c| c.session || c.live.is_some());
    let look = match s.kyber {
        _ if busy.is_some() => Look::Busy,
        State::Starting | State::Stopping => Look::Busy,
        State::Running if in_session => Look::Session,
        State::Running => Look::Running,
        _ => Look::Stopped,
    };
    (status.to_string(), session, look)
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

/// L'état tel que le panneau l'affiche.
fn panel_state(s: &Snapshot, busy: Option<&str>, setup_code: Option<&str>) -> Value {
    json!({
        "service": state_name(s.kyber),
        "adminMissing": s.admin == State::Missing,
        "busy": busy,
        "startWithComputer": s.start_with_computer,
        "setupCode": setup_code,
        "version": env!("CARGO_PKG_VERSION"),
        "console": s.console.as_ref().map(|c| json!({
            "pairRequests": c.pair_requests,
            "session": c.session,
            "user": c.user,
            "update": c.update,
            "version": c.version,
            "hostname": c.hostname,
            "address": c.address,
            "live": c.live,
            "last": c.last,
        })),
    })
}

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
    status: MenuItem,
    session: MenuItem,
    open: MenuItem,
    updates: MenuItem,
    toggle: MenuItem,
    restart: MenuItem,
    start_with_computer: CheckMenuItem,
    reset: MenuItem,
    quit: MenuItem,
    shared: SharedState,
    wake: Sender<()>,
    shown: Option<(Snapshot, Option<&'static str>, Option<String>)>,
    look: Option<Look>,
    /// Demandes d'accès déjà signalées (le panneau s'ouvre pour les nouvelles).
    seen_requests: std::collections::HashSet<String>,
    done: bool,
}

pub fn run() -> Result<()> {
    // Une seule icône par session ; une seconde ouverture (menu Démarrer) ouvre la console.
    let name = wide(r"Local\KyberNow.Tray");
    let mutex = unsafe { CreateMutexW(null(), 1, name.as_ptr()) };
    if !mutex.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return open_console();
    }

    let shared: SharedState = Default::default();
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

    let status = MenuItem::new("KyberNow", false, None);
    let session = MenuItem::new("Patientez…", false, None);
    let open = MenuItem::new("Ouvrir la console", true, None);
    let updates = MenuItem::new("Mises à jour…", true, None);
    let toggle = MenuItem::new("Arrêter le serveur", false, None);
    let restart = MenuItem::new("Redémarrer le serveur", false, None);
    let start_with_computer = CheckMenuItem::new("Démarrer avec l'ordinateur", false, false, None);
    let reset = MenuItem::new("Réinitialiser le mot de passe de la console…", true, None);
    let quit = MenuItem::new("Fermer l'icône", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &session,
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
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(panel.is_none())
        .with_tooltip(TITLE)
        .with_icon(icons.get(Look::Stopped)?)
        .build()?;

    let mut app = App {
        tray,
        icons,
        panel,
        status,
        session,
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
    fn snapshot(&self) -> (Option<Snapshot>, Option<&'static str>, Option<String>) {
        let s = self.shared.lock().unwrap();
        (s.snapshot.clone(), s.busy, s.setup_code.clone())
    }

    fn refresh(&mut self) {
        let (Some(snap), busy, code) = self.snapshot() else { return };
        if self.shown.as_ref().is_some_and(|(s, b, c)| *s == snap && *b == busy && *c == code) {
            return;
        }
        let (status, session, look) = describe(&snap, busy);
        let idle = busy.is_none();
        self.status.set_text(&status);
        self.session.set_text(&session);
        self.toggle.set_text(if snap.kyber.is_up() { "Arrêter le serveur" } else { "Démarrer le serveur" });
        self.toggle.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.restart.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.start_with_computer.set_checked(snap.start_with_computer == Some(true));
        self.start_with_computer.set_enabled(idle && snap.start_with_computer.is_some());
        self.reset.set_enabled(idle && snap.admin != State::Missing);
        self.open.set_enabled(snap.admin != State::Missing);
        let update = snap.console.as_ref().and_then(|c| c.update.clone());
        self.updates.set_text(match &update {
            Some(v) => format!("Installer la version {v}…"),
            None => "Mises à jour…".to_string(),
        });
        self.updates.set_enabled(snap.admin == State::Running);
        let _ = self.tray.set_tooltip(Some(format!("{TITLE}\n{status}\n{session}")));
        if self.look != Some(look) {
            if let Ok(icon) = self.icons.get(look) {
                let _ = self.tray.set_icon(Some(icon));
                self.look = Some(look);
            }
        }
        if let Some(panel) = &self.panel {
            panel.push(&panel_state(&snap, busy, code.as_deref()).to_string());
        }
        // Un appareil demande l'accès : le panneau s'ouvre de lui-même pour
        // que la personne devant l'ordinateur le voie.
        let requests: std::collections::HashSet<String> = snap
            .console
            .as_ref()
            .map(|c| c.pair_requests.iter().filter_map(|r| r["id"].as_str().map(str::to_owned)).collect())
            .unwrap_or_default();
        let fresh = requests.iter().any(|id| !self.seen_requests.contains(id));
        self.seen_requests = requests;
        self.shown = Some((snap, busy, code));
        if fresh {
            if let (Some(panel), Some(rect)) = (self.panel.as_mut(), self.tray.rect()) {
                if !panel.is_visible() {
                    panel.toggle(rect.position, (rect.size.width, rect.size.height));
                }
            }
        }
    }

    /// Lance une action longue dans son propre fil.
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

    fn in_session(&self) -> bool {
        let (snap, _, _) = self.snapshot();
        snap.and_then(|s| s.console).is_some_and(|c| c.session || c.live.is_some())
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
        if id == *self.updates.id() {
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
            // Le code d'appairage ne passe plus par l'icône : il se compare dans la console.
            "openPairing" => {
                self.hide_panel();
                if let Err(e) = open_console_at("acces") {
                    alert(&format!("{e:#}"));
                }
            }
            "endSession" => self.end_session(),
            "pairAnswer" => {
                if let Some(id) = message["id"].as_str() {
                    self.pair_answer(id.to_string(), message["accept"].as_bool().unwrap_or(false));
                }
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
        let (Some(snap), _, _) = self.snapshot() else { return };
        if snap.admin == State::Running {
            if let Err(e) = open_console() {
                alert(&format!("{e:#}"));
            }
        } else {
            self.act("Démarrage de la console…", |_| {
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
        self.act(if accept { "Accès donné à l'appareil…" } else { "Accès refusé…" }, move |_| {
            let port = console::port().ok_or_else(|| anyhow::anyhow!("Le port de la console est introuvable."))?;
            Client::new().pair_answer(port, &id, accept)
        });
    }

    fn end_session(&mut self) {
        if !confirm("Mettre fin à la session en cours ? L'appareil connecté sera déconnecté.") {
            return;
        }
        self.act("Fin de la session…", |_| {
            let port = console::port().ok_or_else(|| anyhow::anyhow!("Le port de la console est introuvable."))?;
            Client::new().end_session(port)
        });
    }

    fn toggle_server(&mut self) {
        let (Some(snap), _, _) = self.snapshot() else { return };
        if snap.kyber.is_up() {
            if self.in_session() && !confirm("Un appareil est connecté. Arrêter le serveur coupera sa session.\n\nArrêter le serveur ?") {
                return;
            }
            self.act("Arrêt du serveur…", |_| services::stop_server());
        } else {
            self.act("Démarrage du serveur…", |_| services::start_server());
        }
    }

    fn restart_server(&mut self) {
        if self.in_session() && !confirm("Un appareil est connecté. Redémarrer le serveur coupera sa session.\n\nRedémarrer le serveur ?") {
            return;
        }
        self.act("Redémarrage du serveur…", |_| services::restart_server());
    }

    fn switch_start_with_computer(&mut self) {
        let (Some(snap), _, _) = self.snapshot() else { return };
        let on = snap.start_with_computer != Some(true);
        self.act("Réglage du démarrage…", move |_| startup::set(on).map(|_| ()));
    }

    /// Efface le mot de passe de la console (invite UAC), montre le code et ouvre la console.
    fn reset_password(&mut self) {
        if !confirm(
            "Réinitialiser le mot de passe de la console ?\n\nVous en choisirez un nouveau avec un code d'initialisation. \
             Vos appareils et leurs comptes ne changent pas.",
        ) {
            return;
        }
        self.act("Réinitialisation du mot de passe…", |shared| {
            if let Some(code) = password::reset()? {
                shared.lock().unwrap().setup_code = Some(code);
                open_console()?;
            }
            Ok(())
        });
    }

    fn quit(&mut self) {
        let (Some(snap), _, _) = self.snapshot() else {
            self.done = true;
            return;
        };
        if !snap.kyber.is_up() {
            self.done = true;
            return;
        }
        match ask_quit(self.in_session()) {
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

fn ask_quit(in_session: bool) -> QuitChoice {
    let title = wide(TITLE);
    let instruction = wide("Fermer l'icône KyberNow ?");
    let content = wide(if in_session {
        "Un appareil est connecté. Le serveur peut rester en marche : la session continue, \
         et vous pourrez rouvrir l'icône depuis le menu Démarrer."
    } else {
        "Le serveur peut rester en marche : vos appareils pourront toujours se connecter, \
         et vous pourrez rouvrir l'icône depuis le menu Démarrer."
    });
    let keep = wide("Laisser le serveur en marche");
    let stop = wide("Arrêter aussi le serveur");
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
        return if confirm("Fermer l'icône KyberNow et arrêter aussi le serveur ?\n\nNon : le serveur reste en marche.") {
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
