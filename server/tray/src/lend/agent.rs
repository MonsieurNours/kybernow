//! Agent de console du prêt : `kybernow-tray.exe --console` (`docs/pret.md`).
//!
//! Admin le lance dans la session Windows de l'utilisateur, avec son compte,
//! quand un prêt commence (même si l'icône est fermée). Pas d'icône, pas le
//! verrou de l'icône : le sien (`Local\KyberNow.Lend`), pour qu'il n'y ait
//! qu'un agent à la fois.
//!
//! Deux fils :
//! - le **pilote** relève `GET /api/local/lend/console` chaque seconde et suit
//!   `phase` ([`Machine`]) : mise en place (cache, crochets, fenêtres réduites,
//!   barre des tâches cachée, Steam), lanceur, `ready`, `beat`, remise en état,
//!   `ended` ; toutes les 500 ms pendant le prêt : barre des tâches, curseur,
//!   fenêtres « Ouvrir », fenêtres du prêt ramenées sur l'écran de l'ami ;
//! - le **fil des fenêtres** (boucle de `tao`) : le lanceur (plein écran sur
//!   l'écran virtuel), le cache (au-dessus de tout, sur chaque écran du PC),
//!   les manettes (XInput) dans le lanceur, le lanceur gardé devant, Entrée
//!   et Échap du clavier du PC pour le cache.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::monitor::MonitorHandle;
use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
use tao::window::{Fullscreen, Window, WindowBuilder};
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};
use windows_sys::Win32::UI::WindowsAndMessaging::GetClipCursor;
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows};

use super::store::{self, Saved};
use super::system::{self, Born, Hooks};
use super::{App, Cover, CoverKey, Display, Machine, Notice, Pad, Phase, Reply, Step, View, base64, clock_at, cover_key, cover_text, left, notice_text, steam, stick};
use crate::console::{self, Client};
use crate::i18n::{self, Lang, tr};

/// Argument de la ligne de commande.
pub const ARG: &str = "--console";
/// Un seul agent à la fois dans la session ; l'icône le prend aussi pour réparer.
const MUTEX: &str = r"Local\KyberNow.Lend";

const LAUNCHER: &str = include_str!("launcher.html");
const COVER: &str = include_str!("cover.html");
const LOGO: &str = include_str!("../../resources/kybernow-share-dark.svg");

const ROUTE: &str = "/api/local/lend/console";

fn state_path() -> Option<PathBuf> {
    system::dir().map(|d| d.join("state.json"))
}

// --- Messages entre les fils ----------------------------------------------------

/// D'où vient un message de page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Launcher,
    Cover,
}

enum Ev {
    Ui(Ui),
    /// Entrée ou Échap du clavier du PC (crochet).
    Key(CoverKey),
    Page(Page, String),
}

/// Ce que le pilote demande au fil des fenêtres.
enum Ui {
    /// Le prêt tel qu'Admin le décrit (à chaque relevé).
    Status(Status),
    /// Le cache sur les écrans du PC.
    Covers,
    /// Le lanceur sur l'écran de l'ami (créé ou déplacé) ; vrai s'il y est.
    Launcher(Display, Vec<Tile>, Sender<bool>),
    Tiles(Vec<Tile>),
    /// Lanceur et cache fermés.
    Close(Sender<()>),
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
struct Status {
    lang: Lang,
    borrower: String,
    host: String,
    until: Option<i64>,
    notice: Option<Notice>,
    display: Option<Display>,
}

impl Status {
    fn of(v: &View) -> Status {
        Status { lang: v.lang, borrower: v.borrower.clone(), host: v.host.clone(), until: v.until, notice: v.notice.clone(), display: v.display.clone() }
    }
}

/// Une tuile du lanceur : l'app, et son icône en `data:` (la page n'a pas le jeton local).
#[derive(Debug, Clone, PartialEq)]
struct Tile {
    id: String,
    name: String,
    icon: Option<String>,
}

fn send(proxy: &EventLoopProxy<Ev>, ui: Ui) {
    let _ = proxy.send_event(Ev::Ui(ui));
}

/// Un appel à Admin hors des fils de l'agent (lanceur, cache) : il ne les retient pas.
fn post_later(route: &'static str, body: Value) {
    std::thread::spawn(move || {
        let ok = console::port().is_some_and(|port| {
            Client::new().local("POST", port, &format!("{ROUTE}/{route}"), Some(&body), Duration::from_secs(10)).is_ok_and(|r| r.status < 300)
        });
        if !ok {
            system::log(&format!("{route} : Admin n'a pas répondu"));
        }
    });
}

// --- Entrée -----------------------------------------------------------------------

pub fn run() -> i32 {
    // Un agent relancé par Admin attend que le précédent ait fini.
    let Some(_lock) = system::lock_named(MUTEX, 15_000) else {
        system::log("un autre agent tient déjà le prêt");
        return 2;
    };
    system::log("agent de console : départ");
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    {
        let proxy = proxy.clone();
        std::thread::spawn(move || {
            Pilot::new(proxy.clone()).run();
            send(&proxy, Ui::Exit);
        });
    }
    let data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(r"KyberNow\lend-webview");
    let mut screens = Screens::new(proxy, WebContext::new(Some(data)));
    event_loop.run(move |event, target, flow| match event {
        Event::NewEvents(StartCause::Init) | Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
            screens.tick(target);
            *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
        }
        Event::UserEvent(Ev::Ui(Ui::Exit)) => {
            system::log("agent de console : fin");
            *flow = ControlFlow::Exit;
        }
        Event::UserEvent(Ev::Ui(ui)) => screens.on_ui(ui, target),
        Event::UserEvent(Ev::Key(key)) => screens.on_key(key),
        Event::UserEvent(Ev::Page(page, message)) => screens.on_page(page, &message),
        // Alt+F4 ne ferme ni le lanceur ni le cache.
        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {}
        _ => {}
    })
}

/// Au démarrage de l'icône : un état de prêt laissé sur le disque, sans
/// agent pour s'en occuper ni prêt en cours chez Admin (agent tombé, PC
/// redémarré en plein prêt) : Steam, les fenêtres, la barre des tâches et le
/// curseur sont remis comme avant.
pub fn repair_at_start() {
    let Some(path) = state_path() else { return };
    let Some(saved) = store::load(&path) else { return };
    let Some(lock) = system::lock_named(MUTEX, 0) else { return };
    let client = Client::new();
    let reply = console::port().and_then(|p| client.local("GET", p, ROUTE, None, Duration::from_secs(5)).ok());
    let lending = reply
        .filter(|r| r.status == 200)
        .and_then(|r| serde_json::from_slice::<Value>(&r.body).ok())
        .is_some_and(|v| Phase::parse(v["phase"].as_str().unwrap_or_default()) != Phase::Idle);
    // Un prêt en cours : Admin relance l'agent, qui reprendra cet état.
    if !lending {
        system::log("icône : remise en état d'un prêt mal fini");
        finish(Some(&path), saved);
    }
    system::release(lock);
}

/// Remise en état depuis l'état gardé : Steam, fenêtres, barre des tâches,
/// curseur. L'état est effacé, sauf la part de Steam qui n'a pas pu être
/// remise (réessayée au prochain démarrage de l'icône). Vrai si Steam est remis.
fn finish(path: Option<&std::path::Path>, mut saved: Saved) -> bool {
    let steam_ok = match &saved.steam {
        None => true,
        Some(s) => steam::restore(s).inspect_err(|e| system::log(&format!("Steam pas remis : {e:#}"))).is_ok(),
    };
    // Après un redémarrage, les numéros de fenêtres ne valent plus rien.
    if (saved.boot - system::boot_time()).abs() < 120 {
        system::restore(&saved.windows);
    }
    system::taskbars(true);
    system::clip(None);
    match path {
        Some(path) if steam_ok => store::clear(path),
        Some(path) => {
            saved.windows.clear();
            if let Err(e) = store::save(path, &saved) {
                system::log(&format!("état du prêt : {e}"));
            }
        }
        None => {}
    }
    steam_ok
}

// --- Le pilote ----------------------------------------------------------------------

struct Pilot {
    proxy: EventLoopProxy<Ev>,
    client: Client,
    path: Option<PathBuf>,
    saved: Option<Saved>,
    hooks: Option<Hooks>,
    born: Option<Born>,
    /// Fenêtres « Ouvrir » déjà là au début du prêt (laissées).
    dialogs: HashSet<isize>,
    /// L'écran de l'ami, une fois le lanceur dessus.
    display: Option<Display>,
    apps: Vec<App>,
    icons: HashMap<String, Option<String>>,
    steam_restored: bool,
}

impl Pilot {
    fn new(proxy: EventLoopProxy<Ev>) -> Pilot {
        let path = state_path();
        let saved = path.as_deref().and_then(store::load);
        Pilot {
            proxy,
            client: Client::new(),
            path,
            saved,
            hooks: None,
            born: None,
            dialogs: HashSet::new(),
            display: None,
            apps: Vec::new(),
            icons: HashMap::new(),
            steam_restored: true,
        }
    }

    fn run(mut self) {
        let mut machine = Machine::new(self.saved.is_some());
        let mut polled: Option<Instant> = None;
        loop {
            let now = Instant::now();
            if polled.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(1)) {
                polled = Some(now);
                let reply = self.poll();
                let view = match &reply {
                    Reply::View(v) => Some(&**v),
                    _ => None,
                };
                if let Some(v) = view {
                    self.follow(v);
                }
                for step in machine.next(&reply, now) {
                    match (step, view) {
                        (Step::Begin, Some(v)) => {
                            if let Err(e) = self.begin(v) {
                                system::log(&format!("mise en place : {e:#}"));
                                machine.retry(Step::Begin);
                            }
                        }
                        (Step::Show, Some(v)) => {
                            if !self.show(v) {
                                machine.retry(Step::Show);
                            }
                        }
                        (Step::Ready, _) => {
                            if !self.post("ready", json!({})) {
                                machine.retry(Step::Ready);
                            }
                        }
                        (Step::Beat, _) => {
                            if !self.post("beat", json!({})) {
                                machine.retry(Step::Beat);
                            }
                        }
                        (Step::TearDown, _) => self.tear_down(),
                        (Step::Ended, _) => {
                            if !self.post("ended", json!({ "steam_restored": self.steam_restored })) {
                                machine.retry(Step::Ended);
                            }
                        }
                        (Step::Lock, _) => {
                            system::log("Admin ne répond plus : session verrouillée");
                            system::lock_workstation();
                        }
                        (Step::Exit, _) => return,
                        _ => {}
                    }
                }
            }
            if machine.begun() {
                self.keep_up();
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    fn poll(&mut self) -> Reply {
        // Certificat d'Admin régénéré (réinstallation) : client refait.
        if console::fingerprint() != self.client.fingerprint {
            self.client = Client::new();
        }
        let Some(port) = console::port() else { return Reply::Silent };
        match self.client.local("GET", port, ROUTE, None, Duration::from_secs(5)) {
            Ok(r) if r.status == 200 => serde_json::from_slice::<Value>(&r.body)
                .ok()
                .and_then(|v| View::parse(&v, i18n::lang()))
                .map_or(Reply::Silent, |v| Reply::View(Box::new(v))),
            Ok(r) if r.status == 404 => Reply::Gone,
            Ok(r) => {
                system::log(&format!("relevé : {}", r.status));
                Reply::Silent
            }
            Err(_) => Reply::Silent,
        }
    }

    fn post(&self, route: &str, body: Value) -> bool {
        let Some(port) = console::port() else { return false };
        match self.client.local("POST", port, &format!("{ROUTE}/{route}"), Some(&body), Duration::from_secs(10)) {
            Ok(r) if r.status < 300 => true,
            Ok(r) => {
                system::log(&format!("{route} : {}", r.status));
                false
            }
            Err(e) => {
                system::log(&format!("{route} : {e:#}"));
                false
            }
        }
    }

    /// À chaque relevé : l'état pour le lanceur et le cache, et le lanceur
    /// suit l'écran de l'ami et la liste des apps.
    fn follow(&mut self, v: &View) {
        send(&self.proxy, Ui::Status(Status::of(v)));
        if self.display.is_none() {
            return;
        }
        if let Some(d) = &v.display
            && self.display.as_ref() != Some(d)
        {
            let (tx, _rx) = mpsc::channel();
            let tiles = self.tiles(&v.apps);
            send(&self.proxy, Ui::Launcher(d.clone(), tiles, tx));
            self.display = Some(d.clone());
        }
        if v.apps != self.apps {
            let tiles = self.tiles(&v.apps);
            send(&self.proxy, Ui::Tiles(tiles));
            self.apps = v.apps.clone();
        }
    }

    fn persist(&self, saved: &Saved) {
        if let Some(path) = &self.path
            && let Err(e) = store::save(path, saved)
        {
            system::log(&format!("état du prêt : {e}"));
        }
    }

    /// Mise en place, sans l'écran de l'ami. Agent relancé en plein prêt : il
    /// reprend l'état gardé (fenêtres déjà notées, Steam déjà basculé).
    fn begin(&mut self, v: &View) -> Result<()> {
        if let Some(old) = self.saved.take_if(|s| s.lend != v.lend) {
            system::log("état d'un autre prêt : remise en état d'abord");
            finish(self.path.as_deref(), old);
        }
        let resume = self.saved.is_some();
        system::log(&format!("mise en place du prêt {}{}", v.lend, if resume { " (reprise)" } else { "" }));
        send(&self.proxy, Ui::Covers);
        if self.hooks.is_none() {
            let proxy = Mutex::new(self.proxy.clone());
            self.hooks = Some(Hooks::start(move |key| {
                let _ = proxy.lock().unwrap().send_event(Ev::Key(key));
            })?);
        }
        let mut saved = self.saved.take().unwrap_or_else(|| Saved {
            lend: v.lend.clone(),
            started: system::unix_now(),
            boot: system::boot_time(),
            windows: Vec::new(),
            steam: None,
        });
        if resume {
            system::minimize_again(&saved.windows);
        } else {
            saved.windows = system::minimize_all();
        }
        system::taskbars(false);
        self.dialogs = system::file_dialogs();
        self.born = Some(Born::new(saved.started));
        self.persist(&saved);
        self.saved = Some(saved.clone());

        // Steam : une fois par prêt (pas à la reprise).
        if saved.steam.is_some() {
            return Ok(());
        }
        let Some(exe) = steam::exe(v.steam_exe.as_deref()) else { return Ok(()) };
        if v.steam_friend {
            let path = self.path.clone();
            let mut on_disk = saved.clone();
            let switched = steam::switch(&exe, &mut |s| {
                on_disk.steam = Some(s.clone());
                path.as_deref().map_or(Ok(()), |p| store::save(p, &on_disk))
            });
            match switched {
                Ok(s) => saved.steam = Some(s),
                Err(e) => {
                    // Ce qui a pu être noté avant l'échec sera remis à la fin.
                    self.saved = self.path.as_deref().and_then(store::load).or(Some(saved));
                    return Err(e).context("Steam n'a pas changé de compte");
                }
            }
        } else {
            saved.steam = Some(steam::host(&exe));
        }
        self.persist(&saved);
        self.saved = Some(saved);
        Ok(())
    }

    /// Le lanceur sur l'écran de l'ami, le curseur dessus.
    fn show(&mut self, v: &View) -> bool {
        let Some(d) = v.display.clone() else { return false };
        let tiles = self.tiles(&v.apps);
        let (tx, rx) = mpsc::channel();
        send(&self.proxy, Ui::Launcher(d.clone(), tiles, tx));
        if !rx.recv_timeout(Duration::from_secs(15)).unwrap_or(false) {
            system::log("lanceur : écran de l'ami introuvable");
            return false;
        }
        system::clip(Some(&d));
        system::center_cursor(&d);
        self.display = Some(d);
        self.apps = v.apps.clone();
        true
    }

    /// Les tuiles, avec les icônes lues chez Admin (une fois chacune).
    fn tiles(&mut self, apps: &[App]) -> Vec<Tile> {
        apps.iter()
            .map(|a| {
                let icon = a.icon.as_ref().and_then(|path| {
                    if !self.icons.contains_key(path) {
                        let fetched = self.icon(path);
                        self.icons.insert(path.clone(), fetched);
                    }
                    self.icons[path].clone()
                });
                Tile { id: a.id.clone(), name: a.name.clone(), icon }
            })
            .collect()
    }

    fn icon(&self, path: &str) -> Option<String> {
        let port = console::port()?;
        let reply = self.client.local("GET", port, path, None, Duration::from_secs(5)).ok().filter(|r| r.status == 200)?;
        Some(format!("data:image/png;base64,{}", base64(&reply.body)))
    }

    /// Toutes les 500 ms pendant le prêt.
    fn keep_up(&mut self) {
        system::taskbars(false);
        if let Some(d) = &self.display {
            // Un jeu qui garde le curseur dans sa fenêtre (sur l'écran de l'ami) le garde.
            let mut current: RECT = unsafe { std::mem::zeroed() };
            let inside = unsafe { GetClipCursor(&mut current) } != 0
                && d.contains(current.left, current.top)
                && d.contains(current.right - 1, current.bottom - 1)
                && (current.right - current.left, current.bottom - current.top) != (d.width as i32, d.height as i32);
            if !inside {
                system::clip(Some(d));
            }
            if let Some(born) = self.born.as_mut() {
                system::keep_on(d, born);
            }
        }
        system::close_file_dialogs(&self.dialogs);
    }

    /// Fin du prêt : apps fermées (10 s au plus), Steam, fenêtres, barre des
    /// tâches, curseur, crochets, lanceur et cache.
    fn tear_down(&mut self) {
        system::log("remise en état");
        if let Some(saved) = &self.saved {
            let mut born = self.born.take().unwrap_or_else(|| Born::new(saved.started));
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut left = system::close_loan_windows(&mut born);
            while left > 0 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(250));
                left = system::loan_windows(&mut born).len();
            }
            if left > 0 {
                system::log(&format!("{left} fenêtre(s) du prêt encore ouvertes : Admin les fermera"));
            }
        }
        self.steam_restored = match self.saved.take() {
            Some(saved) => finish(self.path.as_deref(), saved),
            None => {
                system::taskbars(true);
                system::clip(None);
                true
            }
        };
        // Le clavier et la souris du PC reviennent juste avant que le cache parte.
        self.hooks = None;
        let (tx, rx) = mpsc::channel();
        send(&self.proxy, Ui::Close(tx));
        let _ = rx.recv_timeout(Duration::from_secs(5));
        self.display = None;
        self.born = None;
    }
}

// --- Les fenêtres -------------------------------------------------------------------

fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let (body, kind): (&'static [u8], &str) = match request.uri().path() {
        "/fonts/archivo.woff2" => (crate::panel::ARCHIVO, "font/woff2"),
        "/fonts/martian.woff2" => (crate::panel::MARTIAN, "font/woff2"),
        "/logo.svg" => (LOGO.as_bytes(), "image/svg+xml"),
        "/cover" => (COVER.as_bytes(), "text/html; charset=utf-8"),
        _ => (LAUNCHER.as_bytes(), "text/html; charset=utf-8"),
    };
    Response::builder().header(CONTENT_TYPE, kind).body(Cow::Borrowed(body)).unwrap()
}

struct Surface {
    window: Window,
    webview: WebView,
    ready: bool,
}

impl Surface {
    fn hwnd(&self) -> HWND {
        self.window.hwnd() as HWND
    }

    fn eval(&self, script: &str) {
        if self.ready {
            let _ = self.webview.evaluate_script(script);
        }
    }
}

/// Une fenêtre plein écran sur un écran : le lanceur (activable, pas
/// au-dessus des jeux) ou un cache (au-dessus de tout, jamais activé).
fn surface(target: &EventLoopWindowTarget<Ev>, context: &mut WebContext, proxy: &EventLoopProxy<Ev>, monitor: MonitorHandle, page: Page) -> Result<Surface> {
    let cover = page == Page::Cover;
    let window = WindowBuilder::new()
        .with_title("KyberNow Share")
        .with_decorations(false)
        .with_resizable(false)
        .with_skip_taskbar(true)
        .with_always_on_top(cover)
        .with_focused(!cover)
        .with_focusable(!cover)
        .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
        .build(target)
        .context("fenêtre")?;
    let proxy = Mutex::new(proxy.clone());
    let webview = WebViewBuilder::new_with_web_context(context)
        .with_custom_protocol("kn".into(), |_id, request| serve(request))
        .with_ipc_handler(move |req: Request<String>| {
            let _ = proxy.lock().unwrap().send_event(Ev::Page(page, req.body().clone()));
        })
        .with_navigation_handler(|url| url.starts_with("http://kn.localhost/"))
        .with_background_color((5, 7, 10, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_browser_accelerator_keys(false)
        .with_default_context_menus(false)
        .with_focused(!cover)
        .with_url(if cover { "http://kn.localhost/cover" } else { "http://kn.localhost/launcher" })
        .build(&window)
        .context("vue web (WebView2)")?;
    Ok(Surface { window, webview, ready: false })
}

/// L'écran (de `tao`) qui est celui de l'ami : son nom GDI, sinon sa position.
fn friend_screen(m: &MonitorHandle, d: &Display) -> bool {
    let named = !d.gdi_name.is_empty() && m.name().is_some_and(|n| n.eq_ignore_ascii_case(&d.gdi_name));
    let p = m.position();
    named || (p.x == d.x && p.y == d.y)
}

/// Un écran tel qu'il est : un changement de définition refait son cache.
fn screen_key(m: &MonitorHandle) -> String {
    let (p, s) = (m.position(), m.size());
    format!("{}@{},{} {}x{}", m.name().unwrap_or_default(), p.x, p.y, s.width, s.height)
}

/// Les boutons de toutes les manettes réunis (la croix et le stick gauche).
fn read_pads() -> u16 {
    let mut all = 0;
    for i in 0..4 {
        let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
        if unsafe { XInputGetState(i, &mut state) } == 0 {
            all |= state.Gamepad.wButtons | stick(state.Gamepad.sThumbLX, state.Gamepad.sThumbLY);
        }
    }
    all
}

struct Screens {
    proxy: EventLoopProxy<Ev>,
    context: WebContext,
    launcher: Option<Surface>,
    covers: Vec<(String, Surface)>,
    covers_on: bool,
    status: Option<Status>,
    tiles: Vec<Tile>,
    cover: Cover,
    asked: Option<Instant>,
    /// « Ouverture de Steam… » après une tuile, 10 s au plus.
    opening: Option<(String, Instant)>,
    pad: Pad,
    pushed_launcher: String,
    pushed_cover: String,
    focus_at: Instant,
    second_at: Instant,
}

impl Screens {
    fn new(proxy: EventLoopProxy<Ev>, context: WebContext) -> Screens {
        let now = Instant::now();
        Screens {
            proxy,
            context,
            launcher: None,
            covers: Vec::new(),
            covers_on: false,
            status: None,
            tiles: Vec::new(),
            cover: Cover::Idle,
            asked: None,
            opening: None,
            pad: Pad::default(),
            pushed_launcher: String::new(),
            pushed_cover: String::new(),
            focus_at: now,
            second_at: now,
        }
    }

    fn display(&self) -> Option<&Display> {
        self.status.as_ref().and_then(|s| s.display.as_ref())
    }

    fn on_ui(&mut self, ui: Ui, target: &EventLoopWindowTarget<Ev>) {
        match ui {
            Ui::Status(status) => {
                let moved = self.display() != status.display.as_ref();
                self.status = Some(status);
                if moved {
                    self.sync_covers(target);
                }
                self.push();
            }
            Ui::Covers => {
                self.covers_on = true;
                self.sync_covers(target);
            }
            Ui::Launcher(display, tiles, ack) => {
                let ok = self.place_launcher(&display, target);
                if let Some(status) = self.status.as_mut() {
                    status.display = Some(display);
                }
                self.sync_covers(target);
                self.set_tiles(tiles);
                let _ = ack.send(ok);
            }
            Ui::Tiles(tiles) => self.set_tiles(tiles),
            Ui::Close(ack) => {
                self.launcher = None;
                self.covers.clear();
                self.covers_on = false;
                self.cover = Cover::Idle;
                let _ = ack.send(());
            }
            Ui::Exit => {}
        }
    }

    fn place_launcher(&mut self, display: &Display, target: &EventLoopWindowTarget<Ev>) -> bool {
        let Some(monitor) = target.available_monitors().find(|m| friend_screen(m, display)) else { return false };
        if let Some(launcher) = &self.launcher {
            launcher.window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
            return true;
        }
        match surface(target, &mut self.context, &self.proxy, monitor, Page::Launcher) {
            Ok(s) => {
                system::bring_to_front(s.hwnd());
                self.launcher = Some(s);
                self.pushed_launcher.clear();
                true
            }
            Err(e) => {
                system::log(&format!("lanceur : {e:#}"));
                false
            }
        }
    }

    /// Un cache sur chaque écran du PC (tous, sauf celui de l'ami).
    fn sync_covers(&mut self, target: &EventLoopWindowTarget<Ev>) {
        if !self.covers_on {
            return;
        }
        let display = self.display().cloned();
        let wanted: Vec<MonitorHandle> = target.available_monitors().filter(|m| display.as_ref().is_none_or(|d| !friend_screen(m, d))).collect();
        let keys: Vec<String> = wanted.iter().map(screen_key).collect();
        let before = self.covers.len();
        self.covers.retain(|(k, _)| keys.contains(k));
        let mut added = false;
        for (monitor, key) in wanted.into_iter().zip(keys) {
            if self.covers.iter().any(|(k, _)| *k == key) {
                continue;
            }
            match surface(target, &mut self.context, &self.proxy, monitor, Page::Cover) {
                Ok(s) => {
                    self.covers.push((key, s));
                    added = true;
                }
                Err(e) => system::log(&format!("cache : {e:#}")),
            }
        }
        if added || self.covers.len() != before {
            self.pushed_cover.clear();
        }
    }

    fn set_tiles(&mut self, tiles: Vec<Tile>) {
        self.tiles = tiles;
        self.push_tiles();
    }

    fn push_tiles(&self) {
        if let Some(l) = &self.launcher {
            let tiles: Vec<Value> = self.tiles.iter().map(|t| json!({ "id": t.id, "name": t.name, "icon": t.icon })).collect();
            l.eval(&format!("window.__knTiles && window.__knTiles({})", Value::from(tiles)));
        }
    }

    /// Les textes du lanceur et du cache, poussés quand ils changent.
    fn push(&mut self) {
        let Some(s) = self.status.clone() else { return };
        let now = system::unix_now();
        let lang = s.lang;
        let notice_left = s.notice.as_ref().map(|n| n.at - now);
        if let Some(l) = &self.launcher {
            let texts = json!({
                "lang": lang.code(),
                "lentBy": tr(lang, "lend.lentBy", &[("host", &s.host)]),
                "left": match s.until {
                    Some(until) => left(lang, until - now),
                    None => tr(lang, "lend.untilTakeBack", &[("host", &s.host)]),
                },
                "notice": s.notice.as_ref().and_then(|n| notice_text(lang, &n.reason, &s.host)),
                "giveBack": tr(lang, "lend.giveBack", &[]),
                "confirm": tr(lang, "lend.giveBackConfirm", &[("host", &s.host)]),
                "cancel": tr(lang, "lend.cancel", &[]),
                "opening": self.opening.as_ref().map(|(name, _)| tr(lang, "lend.opening", &[("app", name)])),
                "noApps": tr(lang, "lend.noApps", &[]),
            })
            .to_string();
            if l.ready && texts != self.pushed_launcher {
                l.eval(&format!("window.__knTexts && window.__knTexts({texts})"));
                self.pushed_launcher = texts;
            }
        }
        let until = s.until.and_then(|u| clock_at(lang, u));
        let c = cover_text(lang, &s.borrower, until.as_deref(), notice_left, self.cover);
        let texts = json!({
            "title": c.title,
            "until": c.until,
            "line": c.line,
            "keys": c.keys,
            "asking": self.cover == Cover::Asking && notice_left.is_none(),
            "notice": notice_left.is_some(),
        })
        .to_string();
        if texts != self.pushed_cover && self.covers.iter().all(|(_, c)| c.ready) {
            for (_, cover) in &self.covers {
                cover.eval(&format!("window.__knCover && window.__knCover({texts})"));
            }
            self.pushed_cover = texts;
        }
    }

    fn on_page(&mut self, page: Page, message: &str) {
        let Ok(m) = serde_json::from_str::<Value>(message) else { return };
        match (page, m["cmd"].as_str().unwrap_or_default()) {
            (Page::Launcher, "ready") => {
                if let Some(l) = self.launcher.as_mut() {
                    l.ready = true;
                    let _ = l.webview.focus();
                }
                self.pushed_launcher.clear();
                self.push_tiles();
                self.push();
            }
            (Page::Cover, "ready") => {
                // La page ne dit pas laquelle : toutes celles qui ont fini de charger.
                for (_, cover) in &mut self.covers {
                    cover.ready = true;
                }
                self.pushed_cover.clear();
                self.push();
            }
            (Page::Launcher, "launch") => {
                let Some(id) = m["id"].as_str() else { return };
                let Some(tile) = self.tiles.iter().find(|t| t.id == id) else { return };
                self.opening = Some((tile.name.clone(), Instant::now()));
                // L'app ouverte par Admin pourra passer devant le lanceur.
                system::allow_foreground();
                post_later("launch", json!({ "id": id }));
                self.push();
            }
            (Page::Launcher, "giveBack") => post_later("give-back", json!({})),
            _ => {}
        }
    }

    fn on_key(&mut self, key: CoverKey) {
        if !self.covers_on {
            return;
        }
        let notice = self.status.as_ref().is_some_and(|s| s.notice.is_some());
        let (next, take_back) = cover_key(self.cover, key, notice);
        self.cover = next;
        self.asked = (next == Cover::Asking).then(Instant::now);
        if let Some(now) = take_back {
            system::log(if now { "reprise tout de suite (écran du PC)" } else { "reprise demandée (écran du PC)" });
            post_later("take-back", json!({ "now": now }));
        }
        self.push();
    }

    /// Toutes les 50 ms.
    fn tick(&mut self, target: &EventLoopWindowTarget<Ev>) {
        let now = Instant::now();
        // Les manettes, seulement quand le lanceur est devant.
        match &self.launcher {
            Some(l) if l.ready && system::foreground() == l.hwnd() => {
                for action in self.pad.next(read_pads(), now) {
                    l.eval(&format!("window.__knPad && window.__knPad('{action}')"));
                }
            }
            _ => self.pad.reset(),
        }
        if now.duration_since(self.focus_at) >= Duration::from_millis(250) {
            self.focus_at = now;
            self.guard_focus();
        }
        if now.duration_since(self.second_at) >= Duration::from_secs(1) {
            self.second_at = now;
            self.sync_covers(target);
            for (_, cover) in &self.covers {
                system::topmost(cover.hwnd());
            }
            // La question du cache se retire seule au bout de 30 s.
            if self.asked.is_some_and(|t| now.duration_since(t) > Duration::from_secs(30)) {
                self.asked = None;
                self.cover = Cover::Idle;
            }
            if self.opening.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > Duration::from_secs(10)) {
                self.opening = None;
            }
            self.push();
        }
    }

    /// Le lanceur revient devant quand plus rien de l'ami ne l'est (une app
    /// fermée, une fenêtre partie sur un écran du PC, le bureau).
    fn guard_focus(&mut self) {
        let (Some(l), Some(d)) = (&self.launcher, self.display()) else { return };
        let fg = system::foreground();
        if fg == l.hwnd() {
            return;
        }
        if system::acceptable_foreground(fg, d) {
            // L'app est devant : « Ouverture de… » n'a plus lieu d'être.
            self.opening = None;
            return;
        }
        system::bring_to_front(l.hwnd());
        let _ = l.webview.focus();
    }
}
