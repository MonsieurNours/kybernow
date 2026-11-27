//! Icône de KyberNow Station près de l'horloge, sous Linux (docs/station-linux.md).
//!
//! Une icône AppIndicator (StatusNotifierItem) : KDE Plasma l'affiche de
//! lui-même, GNOME avec l'extension AppIndicator (installée et activée par
//! Ubuntu ; `gnome-shell-extension-appindicator` ailleurs). Un clic, quel
//! qu'il soit, ouvre le menu : le panneau de Windows n'a pas de place ici,
//! ses pages s'ouvrent dans Admin (partage, prêt, mises à jour). Mêmes menus
//! et textes que sous Windows (`tray.*`), sans « Démarrer avec l'ordinateur »
//! (les services de la Station démarrent avec la session de l'utilisateur).
//!
//! Lancée par le service systemd de l'utilisateur `kybernow-tray.service`
//! (lié à sa session graphique) ; Kyber et Admin sont pilotés par
//! `systemctl --user`, sans droits particuliers.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::console::{self, Client};
use crate::i18n::{lang, list, t};
use crate::icons::Icons;
use crate::lend::host as lend;
use crate::share;
use crate::status::{Look, Snapshot, State, describe};

pub const TITLE: &str = "KyberNow Station";
/// Services systemd de l'utilisateur (`server/linux/packaging`).
pub const KYBER: &str = "kybernow-kyber.service";
pub const ADMIN: &str = "kybernow-admin.service";
/// Admin installé par le paquet.
const ADMIN_EXE: &str = "/usr/lib/kybernow-station/bin/kyber-admin";

/// Dossier de la Station de cet utilisateur (`~/.local/share/kybernow/station`).
pub fn station_dir() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(data.join("kybernow/station"))
}

// ---------------------------------------------------------------------------
// Services (systemctl --user)

fn systemctl(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("systemctl").arg("--user").args(args).output()
}

/// `LoadState` puis `ActiveState`, tels que les donne `systemctl show --value`.
pub fn parse_state(load: &str, active: &str) -> State {
    match (load.trim(), active.trim()) {
        ("not-found", _) => State::Missing,
        (_, "active" | "reloading") => State::Running,
        (_, "inactive" | "failed") => State::Stopped,
        (_, "activating") => State::Starting,
        (_, "deactivating") => State::Stopping,
        _ => State::Unknown,
    }
}

pub fn state(unit: &str) -> State {
    let Ok(out) = systemctl(&["show", "--property=LoadState,ActiveState", "--value", unit]) else { return State::Unknown };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    parse_state(lines.next().unwrap_or(""), lines.next().unwrap_or(""))
}

fn run(start: bool, unit: &str) -> Result<()> {
    let out = systemctl(&[if start { "start" } else { "stop" }, unit])?;
    if !out.status.success() {
        let key = if start { "tray.errors.serviceStartFailed" } else { "tray.errors.serviceStopFailed" };
        bail!(t(key, &[("service", &unit), ("error", &String::from_utf8_lossy(&out.stderr).trim())]));
    }
    Ok(())
}

fn wait(unit: &str, target: State, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let s = state(unit);
        if s == target {
            return Ok(());
        }
        if target == State::Running && s == State::Stopped {
            bail!(t("tray.errors.serviceStoppedAtStart", &[("service", &unit)]));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    bail!(t("tray.errors.serviceTimeout", &[("service", &unit), ("seconds", &timeout.as_secs())]))
}

/// Démarre Admin (s'il est arrêté) et Kyber, comme sous Windows.
fn start_server() -> Result<()> {
    if state(ADMIN) == State::Stopped {
        run(true, ADMIN)?;
    }
    run(true, KYBER)?;
    wait(KYBER, State::Running, Duration::from_secs(30))
}

/// Arrête Kyber ; Admin reste disponible.
fn stop_server() -> Result<()> {
    run(false, KYBER)?;
    wait(KYBER, State::Stopped, Duration::from_secs(30))
}

/// « Redémarrer la Station » : demandé à Admin, comme sous Windows (STWIN-45) ;
/// Admin injoignable : systemd relance Kyber puis Admin, comme le ferait Admin.
fn restart_server() -> Result<()> {
    let by_admin = match crate::console::port() {
        Some(port) => crate::console::Client::new().station_restart(port)?,
        None => false,
    };
    if by_admin {
        return crate::status::after_restart(&state, ADMIN, KYBER);
    }
    for unit in [KYBER, ADMIN] {
        let out = systemctl(&["restart", unit])?;
        if !out.status.success() {
            bail!(t("tray.errors.serviceStartFailed", &[("service", &unit), ("error", &String::from_utf8_lossy(&out.stderr).trim())]));
        }
    }
    wait(KYBER, State::Running, Duration::from_secs(30))?;
    wait(ADMIN, State::Running, Duration::from_secs(30))
}

/// « Réinitialiser le mot de passe d'Admin » : Admin tourne avec le compte de
/// l'utilisateur, l'icône le fait elle-même, sans droits particuliers.
/// Renvoie le code d'initialisation écrit par Admin à son redémarrage.
fn reset_password() -> Result<String> {
    let data = station_dir().map(|d| d.join("admin")).ok_or_else(|| anyhow::anyhow!("HOME"))?;
    let code_file = data.join("setup-code.txt");
    run(false, ADMIN)?;
    wait(ADMIN, State::Stopped, Duration::from_secs(30))?;
    let status = Command::new(ADMIN_EXE).arg("--data-dir").arg(&data).arg("reset-password").status();
    // Un ancien code ne doit pas être pris pour celui d'Admin qui redémarre.
    let _ = std::fs::remove_file(&code_file);
    run(true, ADMIN)?;
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => bail!(t("tray.errors.resetCommandFailed", &[("status", &s)])),
        Err(e) => bail!(t("tray.errors.resetFailed", &[("error", &e)])),
    }
    wait(ADMIN, State::Running, Duration::from_secs(30))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Some(code) = console::read_code(&code_file) {
            return Ok(code);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    bail!(t("tray.linux.noSetupCode", &[("path", &code_file.display())]))
}

// ---------------------------------------------------------------------------
// Boîtes de dialogue : zenity (GNOME), kdialog (KDE), sinon une notification.

fn has(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

pub fn alert(text: &str) {
    let shown = if has("kdialog") && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.contains("KDE")) {
        Command::new("kdialog").args(["--title", TITLE, "--sorry", text]).status().is_ok()
    } else if has("zenity") {
        Command::new("zenity").args(["--warning", "--title", TITLE, "--no-wrap", "--text", text]).status().is_ok()
    } else if has("notify-send") {
        Command::new("notify-send").args(["--app-name", TITLE, TITLE, text]).status().is_ok()
    } else {
        false
    };
    if !shown {
        eprintln!("{text}");
    }
}

/// Oui ou non ; sans boîte de dialogue possible, oui (le clic dans le menu vaut accord).
fn confirm(text: &str) -> bool {
    if has("kdialog") && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.contains("KDE")) {
        return Command::new("kdialog").args(["--title", TITLE, "--warningyesno", text]).status().is_ok_and(|s| s.success());
    }
    if has("zenity") {
        return Command::new("zenity").args(["--question", "--title", TITLE, "--no-wrap", "--text", text]).status().is_ok_and(|s| s.success());
    }
    true
}

fn notify(text: &str) {
    if has("notify-send") {
        let _ = Command::new("notify-send").args(["--app-name", TITLE, "--icon", "dialog-information", TITLE, text]).spawn();
    }
}

fn open_url(url: &str) {
    if let Err(e) = Command::new("xdg-open").arg(url).spawn() {
        alert(&format!("xdg-open : {e}"));
    }
}

/// Une page d'Admin (« systeme », « ensemble », « pret »…), l'accueil si vide.
fn open_console_at(page: &str) -> Result<()> {
    let port = console::port().ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissing", &[])))?;
    let url = console::url(port);
    open_url(&if page.is_empty() { url } else { format!("{url}#/{page}") });
    Ok(())
}

// ---------------------------------------------------------------------------
// L'icône

#[derive(Default)]
struct Shared {
    snapshot: Option<Snapshot>,
    busy: Option<&'static str>,
}

type SharedState = Arc<Mutex<Shared>>;

fn poll(client: &Client) -> Snapshot {
    let kyber = state(KYBER);
    let admin = state(ADMIN);
    let console = if admin == State::Running { console::port().and_then(|p| client.local_state(p)) } else { None };
    Snapshot { kyber, admin, console, start_with_computer: None }
}

fn watch(shared: SharedState, wake: Receiver<()>) {
    let mut client = Client::new();
    loop {
        if console::fingerprint() != client.fingerprint {
            client = Client::new();
        }
        let snapshot = poll(&client);
        shared.lock().unwrap().snapshot = Some(snapshot);
        match wake.recv_timeout(Duration::from_secs(2)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

enum UserEvent {
    Menu(MenuEvent),
}

struct App {
    tray: TrayIcon,
    icons: Icons,
    menu: Menu,
    status: MenuItem,
    session: MenuItem,
    relay: MenuItem,
    relay_shown: bool,
    share: MenuItem,
    lend: MenuItem,
    open: MenuItem,
    updates: MenuItem,
    toggle: MenuItem,
    restart: MenuItem,
    reset: MenuItem,
    quit: MenuItem,
    shared: SharedState,
    wake: Sender<()>,
    shown: Option<(Snapshot, Option<&'static str>, String)>,
    look: Option<Look>,
    /// Demandes déjà signalées (une notification pour chaque nouvelle).
    seen: HashSet<String>,
    done: bool,
}

/// Une seule icône par session : un verrou dans le dossier d'exécution.
fn single_instance() -> Option<std::fs::File> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("kybernow-tray.lock")).ok()?;
    use std::os::fd::AsRawFd;
    (unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0).then_some(file)
}

pub fn run_app() -> Result<()> {
    let Some(_lock) = single_instance() else {
        // Déjà là : ouvrir Admin, comme une seconde ouverture sous Windows.
        return open_console_at("");
    };
    // tray-icon et tao passent par GTK, qui doit être prêt avant l'icône.
    let shared: SharedState = Arc::new(Mutex::new(Shared::default()));
    let (wake, wake_rx) = mpsc::channel();
    {
        let shared = shared.clone();
        std::thread::spawn(move || watch(shared, wake_rx));
    }
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    {
        let proxy = Mutex::new(event_loop.create_proxy());
        MenuEvent::set_event_handler(Some(move |e| {
            let _ = proxy.lock().unwrap().send_event(UserEvent::Menu(e));
        }));
    }
    let status = MenuItem::new(TITLE, false, None);
    let session = MenuItem::new(t("tray.session.wait", &[]), false, None);
    let relay = MenuItem::new("", false, None);
    let share = MenuItem::new(t("tray.menu.share", &[]), false, None);
    let lend = MenuItem::new(t("tray.menu.lend", &[]), false, None);
    let open = MenuItem::new(t("tray.menu.openConsole", &[]), true, None);
    let updates = MenuItem::new(t("tray.menu.updates", &[]), true, None);
    let toggle = MenuItem::new(t("tray.menu.stopServer", &[]), false, None);
    let restart = MenuItem::new(t("tray.menu.restartServer", &[]), false, None);
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
        &reset,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;
    let icons = Icons::load()?;
    let tray = TrayIconBuilder::new()
        .with_id("kybernow-station")
        .with_menu(Box::new(menu.clone()))
        .with_title(TITLE)
        .with_tooltip(TITLE)
        .with_icon(icons.get(Look::Stopped)?)
        .build()?;
    let mut app = App {
        tray,
        icons,
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
        reset,
        quit,
        shared,
        wake,
        shown: None,
        look: None,
        seen: HashSet::new(),
        done: false,
    };
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                app.refresh();
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(500));
            }
            Event::UserEvent(UserEvent::Menu(e)) => app.on_menu(e),
            _ => {}
        }
        if app.done {
            *control_flow = ControlFlow::Exit;
        }
    });
}

/// Demandes qui attendent la personne devant l'ordinateur : appareils qui
/// demandent l'accès, invités du partage, amis qui veulent emprunter.
fn pending(snap: &Snapshot) -> HashSet<String> {
    let Some(c) = snap.console.as_ref() else { return HashSet::new() };
    let pair = c.pair_requests.iter().filter_map(|r| r["id"].as_str().map(|id| format!("pair:{id}")));
    let asks = c.share.as_ref().map(share::local_request_ids).unwrap_or_default().into_iter().map(|id| format!("share:{id}"));
    let lend = c.lend.as_ref().map(|l| lend::request_ids(l).into_iter().chain(lend::slot_start(l)).collect::<Vec<_>>()).unwrap_or_default();
    let remote = c.remote_admin.iter().map(|n| format!("remote:{n}"));
    pair.chain(asks).chain(lend.into_iter().map(|id| format!("lend:{id}"))).chain(remote).collect()
}

impl App {
    fn snapshot(&self) -> (Option<Snapshot>, Option<&'static str>) {
        let s = self.shared.lock().unwrap();
        (s.snapshot.clone(), s.busy)
    }

    fn refresh(&mut self) {
        let (Some(snap), busy) = self.snapshot() else { return };
        let (status, session, look) = describe(&snap, busy);
        if self.shown.as_ref().is_some_and(|(s, b, line)| *s == snap && *b == busy && *line == session) {
            return;
        }
        let idle = busy.is_none();
        self.status.set_text(&status);
        self.session.set_text(&session);
        self.toggle.set_text(t(if snap.kyber.is_up() { "tray.menu.stopServer" } else { "tray.menu.startServer" }, &[]));
        self.toggle.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.restart.set_enabled(idle && matches!(snap.kyber, State::Running | State::Stopped));
        self.reset.set_enabled(idle && snap.admin != State::Missing);
        self.open.set_enabled(snap.admin != State::Missing);
        let update = snap.console.as_ref().and_then(|c| c.update.clone());
        self.updates.set_text(match &update {
            Some(v) => t("tray.menu.installVersion", &[("version", v)]),
            None => t("tray.menu.updates", &[]),
        });
        self.updates.set_enabled(snap.admin == State::Running);
        let console = snap.console.as_ref().filter(|_| snap.kyber == State::Running);
        let share_open = console.is_some_and(|c| c.share.is_some());
        self.share.set_text(t(if share_open { "tray.menu.stopShare" } else { "tray.menu.share" }, &[]));
        self.share.set_enabled(idle && console.is_some_and(|c| c.share.is_some() || (c.share_enabled && !c.device_session())));
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
        self.lend.set_enabled(idle && lend_view.is_some_and(|l| lent || l["enabled"] == true));
        let relay = console.and_then(|c| c.relay.as_ref()).and_then(|r| share::relay_line(lang(), r));
        if let Some(text) = &relay {
            self.relay.set_text(text);
        }
        if relay.is_some() != self.relay_shown {
            let changed = if relay.is_some() { self.menu.insert(&self.relay, 2) } else { self.menu.remove(&self.relay) };
            self.relay_shown = if changed.is_ok() { relay.is_some() } else { self.relay_shown };
        }
        // AppIndicator n'a pas d'infobulle : l'état est dans le titre (KDE l'affiche au survol).
        self.tray.set_title(Some(format!("{TITLE} · {status} · {session}")));
        if self.look != Some(look)
            && let Ok(icon) = self.icons.get(look)
        {
            let _ = self.tray.set_icon(Some(icon));
            self.look = Some(look);
        }
        // Une demande nouvelle : une notification (le panneau de Windows s'ouvrirait).
        let now = pending(&snap);
        if now.iter().any(|id| !self.seen.contains(id)) {
            notify(&t("tray.linux.attention", &[]));
        }
        self.seen = now;
        self.shown = Some((snap, busy, session));
    }

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

    /// Une action du partage ou du prêt auprès d'Admin, dans son propre fil ;
    /// son refus dans une boîte de dialogue.
    fn background(&self, job: impl FnOnce(&Client, u16) -> Result<()> + Send + 'static) {
        let wake = self.wake.clone();
        std::thread::spawn(move || {
            let result = console::port()
                .ok_or_else(|| anyhow::anyhow!(t("tray.consolePortMissingShort", &[])))
                .and_then(|port| job(&Client::new(), port));
            let _ = wake.send(());
            if let Err(e) = result {
                alert(&format!("{e:#}"));
            }
        });
    }

    /// Qui occupe l'ordinateur : un appareil, ou les invités d'un partage.
    fn occupied(&self) -> (bool, Vec<String>) {
        let (snap, _) = self.snapshot();
        match snap.and_then(|s| s.console) {
            Some(c) if c.device_session() => (true, vec![]),
            Some(c) if c.guests() > 0 => (false, c.guest_names()),
            _ => (false, vec![]),
        }
    }

    fn on_menu(&mut self, event: MenuEvent) {
        let id = event.id;
        let page = |page: &str| {
            if let Err(e) = open_console_at(page) {
                alert(&format!("{e:#}"));
            }
        };
        if id == *self.share.id() {
            let open = self.snapshot().0.and_then(|s| s.console).is_some_and(|c| c.share.is_some());
            if open {
                self.background(|c, port| c.share_stop(port));
            } else {
                page("ensemble");
            }
        } else if id == *self.lend.id() {
            match self.snapshot().0.and_then(|s| s.console).and_then(|c| c.lend).filter(lend::active) {
                Some(l) => {
                    let now = lend::notice_end(&l).is_some();
                    self.background(move |c, port| c.lend_take_back(port, now).map(|_| ()));
                }
                None => page("pret"),
            }
        } else if id == *self.updates.id() {
            page("systeme");
        } else if id == *self.open.id() {
            page("");
        } else if id == *self.toggle.id() {
            let up = self.snapshot().0.is_some_and(|s| s.kyber.is_up());
            if up {
                let sure = match self.occupied() {
                    (true, _) => confirm(&t("tray.confirm.stopInSession", &[])),
                    (false, names) if !names.is_empty() => confirm(&t("tray.confirm.stopSharing", &[("names", &list(&names))])),
                    _ => true,
                };
                if sure {
                    self.act("tray.busy.stopping", stop_server);
                }
            } else {
                self.act("tray.busy.starting", start_server);
            }
        } else if id == *self.restart.id() {
            let sure = match self.occupied() {
                (true, _) => confirm(&t("tray.confirm.restartInSession", &[])),
                (false, names) if !names.is_empty() => confirm(&t("tray.confirm.restartSharing", &[("names", &list(&names))])),
                _ => true,
            };
            if sure {
                self.act("tray.busy.restarting", restart_server);
            }
        } else if id == *self.reset.id() {
            if confirm(&t("tray.confirm.resetPassword", &[])) {
                self.act("tray.busy.resetting", || {
                    let code = reset_password()?;
                    open_console_at("")?;
                    alert(&t("tray.linux.setupCode", &[("code", &code)]));
                    Ok(())
                });
            }
        } else if id == *self.quit.id() {
            self.done = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_systemd_state() {
        assert_eq!(parse_state("loaded", "active"), State::Running);
        assert_eq!(parse_state("loaded", "failed"), State::Stopped);
        assert_eq!(parse_state("loaded", "activating"), State::Starting);
        assert_eq!(parse_state("loaded", "deactivating"), State::Stopping);
        assert_eq!(parse_state("not-found", "inactive"), State::Missing);
        assert_eq!(parse_state("", ""), State::Unknown);
    }
}
