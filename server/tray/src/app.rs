//! Menu, état affiché et boucle de messages.
//!
//! Un fil surveille les services et la console toutes les deux secondes ; les
//! actions longues (démarrage, arrêt, invite UAC) tournent dans leur propre fil.
//! Le fil principal ne fait qu'afficher : il relit l'état deux fois par seconde.

use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Controls::{
    TASKDIALOG_BUTTON, TASKDIALOGCONFIG, TDCBF_CANCEL_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION, TaskDialogIndirect,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, IDYES, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO, MSG,
    MessageBoxW, SW_SHOWNORMAL, SetTimer, TranslateMessage, WM_TIMER,
};

use crate::console::{self, Client, LocalState};
use crate::icons::{Icons, Look};
use crate::services::{self, ADMIN, KYBER, State};
use crate::startup;

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
    let port = console::port().ok_or_else(|| anyhow::anyhow!("Le port de la console est introuvable : KyberNow est-il installé ?"))?;
    open_url(&console::url(port));
    Ok(())
}

/// Ce que l'icône sait de l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

type SharedState = Arc<Mutex<Shared>>;

fn watch(shared: SharedState, wake: Receiver<()>) {
    let client = Client::new();
    loop {
        let snapshot = poll(&client);
        shared.lock().unwrap().snapshot = Some(snapshot);
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
    let session = match (s.kyber, s.admin, &s.console) {
        (State::Running, State::Running, Some(LocalState { session: true, user: Some(user) })) => format!("Session en cours ({user})"),
        (State::Running, State::Running, Some(LocalState { session: true, user: None })) => "Session en cours".into(),
        (State::Running, State::Running, Some(_)) => "Aucune session en cours".into(),
        (State::Running, State::Running, None) => "La console ne répond pas".into(),
        (State::Running, _, _) => "Console arrêtée".into(),
        (State::Stopped, _, _) => "Vos appareils ne peuvent pas se connecter".into(),
        (State::Missing, _, _) => "Réinstallez KyberNow".into(),
        _ => "Patientez…".into(),
    };
    let in_session = s.console.as_ref().is_some_and(|c| c.session);
    let look = match s.kyber {
        _ if busy.is_some() => Look::Busy,
        State::Starting | State::Stopping => Look::Busy,
        State::Running if in_session => Look::Session,
        State::Running => Look::Running,
        _ => Look::Stopped,
    };
    (status.to_string(), session, look)
}

struct App {
    tray: TrayIcon,
    icons: Icons,
    status: MenuItem,
    session: MenuItem,
    open: MenuItem,
    toggle: MenuItem,
    restart: MenuItem,
    start_with_computer: CheckMenuItem,
    quit: MenuItem,
    shared: SharedState,
    wake: Sender<()>,
    shown: Option<(Snapshot, Option<&'static str>)>,
    look: Option<Look>,
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

    let status = MenuItem::new("KyberNow", false, None);
    let session = MenuItem::new("Patientez…", false, None);
    let open = MenuItem::new("Ouvrir la console", true, None);
    let toggle = MenuItem::new("Arrêter le serveur", false, None);
    let restart = MenuItem::new("Redémarrer le serveur", false, None);
    let start_with_computer = CheckMenuItem::new("Démarrer avec l'ordinateur", false, false, None);
    let quit = MenuItem::new("Quitter", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &session,
        &PredefinedMenuItem::separator(),
        &open,
        &PredefinedMenuItem::separator(),
        &toggle,
        &restart,
        &PredefinedMenuItem::separator(),
        &start_with_computer,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    // Clics et survols : le menu s'ouvre de lui-même, le reste ne sert pas.
    // Sans gestionnaire, ces événements s'accumuleraient dans une file jamais lue.
    TrayIconEvent::set_event_handler(Some(|_| {}));

    let icons = Icons::load()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_tooltip(TITLE)
        .with_icon(icons.get(Look::Stopped)?)
        .build()?;

    let mut app = App {
        tray,
        icons,
        status,
        session,
        open,
        toggle,
        restart,
        start_with_computer,
        quit,
        shared,
        wake,
        shown: None,
        look: None,
        done: false,
    };

    unsafe { SetTimer(null_mut(), 0, 500, None) };
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while !app.done {
        let r = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
        if r == 0 || r == -1 {
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            app.on_menu(event);
        }
        if msg.message == WM_TIMER {
            app.refresh();
        }
    }
    Ok(())
}

impl App {
    fn snapshot(&self) -> (Option<Snapshot>, Option<&'static str>) {
        let s = self.shared.lock().unwrap();
        (s.snapshot.clone(), s.busy)
    }

    fn refresh(&mut self) {
        let (Some(snap), busy) = self.snapshot() else { return };
        if self.shown.as_ref().is_some_and(|(s, b)| *s == snap && *b == busy) {
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
        self.open.set_enabled(snap.admin != State::Missing);
        let _ = self.tray.set_tooltip(Some(format!("{TITLE}\n{status}\n{session}")));
        if self.look != Some(look) {
            if let Ok(icon) = self.icons.get(look) {
                let _ = self.tray.set_icon(Some(icon));
                self.look = Some(look);
            }
        }
        self.shown = Some((snap, busy));
    }

    /// Lance une action longue dans son propre fil.
    fn act(&mut self, label: &'static str, job: impl FnOnce() -> Result<()> + Send + 'static) {
        {
            let mut s = self.shared.lock().unwrap();
            if s.busy.is_some() {
                return;
            }
            s.busy = Some(label);
        }
        let (shared, wake) = (self.shared.clone(), self.wake.clone());
        std::thread::spawn(move || {
            let result = job();
            shared.lock().unwrap().busy = None;
            let _ = wake.send(());
            if let Err(e) = result {
                alert(&format!("{e:#}"));
            }
        });
        self.refresh();
    }

    fn in_session(&self) -> bool {
        let (snap, _) = self.snapshot();
        snap.and_then(|s| s.console).is_some_and(|c| c.session)
    }

    fn on_menu(&mut self, event: MenuEvent) {
        let (snap, _) = self.snapshot();
        let Some(snap) = snap else { return };
        let id = event.id;
        if id == *self.open.id() {
            if snap.admin == State::Running {
                if let Err(e) = open_console() {
                    alert(&format!("{e:#}"));
                }
            } else {
                self.act("Démarrage de la console…", || {
                    services::start(ADMIN)?;
                    services::wait(ADMIN, State::Running, Duration::from_secs(30))?;
                    std::thread::sleep(Duration::from_secs(1));
                    open_console()
                });
            }
        } else if id == *self.toggle.id() {
            if snap.kyber.is_up() {
                if self.in_session() && !confirm("Un appareil est connecté. Arrêter le serveur coupera sa session.\n\nArrêter le serveur ?") {
                    return;
                }
                self.act("Arrêt du serveur…", services::stop_server);
            } else {
                self.act("Démarrage du serveur…", services::start_server);
            }
        } else if id == *self.restart.id() {
            if self.in_session() && !confirm("Un appareil est connecté. Redémarrer le serveur coupera sa session.\n\nRedémarrer le serveur ?") {
                return;
            }
            self.act("Redémarrage du serveur…", services::restart_server);
        } else if id == *self.start_with_computer.id() {
            let on = snap.start_with_computer != Some(true);
            // Le menu a déjà coché ou décoché la case : on la remet selon l'état réel.
            self.shown = None;
            self.act("Réglage du démarrage…", move || startup::set(on).map(|_| ()));
        } else if id == *self.quit.id() {
            self.on_quit(&snap);
        }
    }

    fn on_quit(&mut self, snap: &Snapshot) {
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
