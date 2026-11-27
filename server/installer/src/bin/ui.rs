//! Interface de l'installateur : une fenêtre sans bordure avec une vue web
//! (WebView2 sous Windows) qui affiche l'app Svelte de `ui/dist`.
//!
//! Le JavaScript envoie des commandes via `window.ipc.postMessage(json)` ;
//! l'installateur répond en appelant `window.__knReceive(event)`.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};

use kybernow::account::{self, AccountCommand, AccountEvent, Coordinator, Navigation, ViewCommand};
use kybernow::checks::SystemInfo;
use kybernow::events::{Command, Emit, Event, TaskStatus};
use kybernow::i18n::{self, t};
use rust_embed::Embed;
use tao::dpi::LogicalSize;
use tao::event::{Event as WinEvent, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{NewWindowResponse, WebContext, WebView, WebViewBuilder};

#[derive(Embed)]
#[folder = "ui/dist/"]
#[allow_missing = true]
struct Assets;

#[cfg_attr(target_os = "macos", allow(dead_code))]
enum UserEvent {
    Js(String),
    Drag,
    Minimize,
    Close,
    /// La page du relais (étape « Compte KyberNow »), dans le cadre donné.
    AccountView(ViewCommand),
    /// La page du relais va ailleurs sur le relais (« Créer un compte »).
    AccountRedirect(String),
}

/// Ouvre une adresse web dans le navigateur par défaut.
pub(crate) fn open_in_browser(url: &str) {
    kybernow::sys::open_url(url);
}

/// La page du relais dans la fenêtre de l'installateur : une vue enfant posée
/// sur le cadre de l'interface, qui ne quitte pas le relais (tout autre lien
/// part dans le navigateur). `builder` porte le contexte de la fenêtre
/// (profil de WebView2 sous Windows).
pub(crate) fn account_view<'a>(
    builder: WebViewBuilder<'a>,
    window: &'a tao::window::Window,
    url: &str,
    bounds: wry::Rect,
    signup: bool,
    redirect: impl Fn(String) + 'static,
) -> wry::Result<WebView> {
    let origin = account::origin_of(url).unwrap_or_default();
    let signup = Arc::new(AtomicBool::new(signup));
    builder
        .with_url(url)
        .with_bounds(bounds)
        .with_background_color((5, 7, 10, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_navigation_handler(move |to| match account::view_navigation(&to, &origin, &signup) {
            Navigation::Allow => true,
            Navigation::External => {
                open_in_browser(&to);
                false
            }
            Navigation::Block => false,
            Navigation::Redirect(next) => {
                redirect(next);
                false
            }
        })
        .with_new_window_req_handler(|to, _| {
            open_in_browser(&to);
            NewWindowResponse::Deny
        })
        .build_as_child(window)
}

/// Le cadre de la page du relais, en points de la fenêtre.
pub(crate) fn view_bounds(x: f64, y: f64, width: f64, height: f64) -> wry::Rect {
    use tao::dpi::{LogicalPosition, LogicalSize};
    wry::Rect { position: LogicalPosition::new(x, y).into(), size: LogicalSize::new(width.max(1.0), height.max(1.0)).into() }
}

pub(crate) fn arg(name: &str) -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from)
}

pub(crate) fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(file) => Response::builder()
            .header(CONTENT_TYPE, mime_guess::from_path(path).first_or_octet_stream().as_ref())
            .body(Cow::Owned(file.data.into_owned()))
            .unwrap(),
        None => Response::builder()
            .status(404)
            .body(Cow::Borrowed(b"Interface non compilee : npm run build dans server/installer/ui" as &[u8]))
            .unwrap(),
    }
}

#[cfg(target_os = "macos")]
#[path = "mac_host/mod.rs"]
mod mac_host;

fn main() -> wry::Result<()> {
    #[cfg(target_os = "macos")]
    {
        mac_host::main()
    }
    #[cfg(not(target_os = "macos"))]
    {
        windows_main()
    }
}

/// Installateur Windows (et son déroulé simulé hors Windows et Mac).
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn windows_main() -> wry::Result<()> {
    let payload = arg("--payload");
    let webview_data = arg("--webview-data").unwrap_or_else(|| std::env::temp_dir().join("KyberNow-WebView"));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let builder = WindowBuilder::new()
        .with_title(t("ui.windowTitle", &[]))
        .with_inner_size(LogicalSize::new(1100.0, 700.0))
        .with_resizable(false)
        .with_decorations(false);
    // L'ombre de tao réserve autour de la fenêtre une bande de cadre que
    // Windows peint en clair : on la fait nous-mêmes, sans bande (frame).
    #[cfg(windows)]
    let builder = {
        use tao::platform::windows::WindowBuilderExtWindows;
        builder.with_undecorated_shadow(false)
    };
    let window = builder.build(&event_loop).expect("création de la fenêtre");
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        frame(window.hwnd() as _);
    }

    let (tx, rx) = mpsc::channel::<String>();
    // Vrai pendant l'installation : la fenêtre refuse alors de se fermer.
    let installing = Arc::new(Mutex::new(false));
    spawn_backend(rx, proxy.clone(), payload, installing.clone());

    let mut context = WebContext::new(Some(webview_data));
    let url = if cfg!(windows) { "http://kn.localhost/" } else { "kn://localhost/" };
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_custom_protocol("kn".into(), |_id, request| serve(request))
        // Langue de Windows, lue par l'interface avant son démarrage.
        .with_initialization_script(format!("window.__knLang = {:?};", i18n::lang().code()))
        .with_ipc_handler(move |req: Request<String>| {
            let _ = tx.send(req.body().clone());
        })
        .with_background_color((5, 7, 10, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_url(url)
        .build(&window)?;

    // La page du relais (étape « Compte KyberNow »), créée à la demande.
    let mut relay_view: Option<(String, WebView)> = None;
    let redirect_proxy = proxy.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            WinEvent::UserEvent(UserEvent::Js(json)) => {
                let _ = webview.evaluate_script(&format!("window.__knReceive && window.__knReceive({json})"));
            }
            WinEvent::UserEvent(UserEvent::AccountView(ViewCommand::AccountView { url, x, y, width, height, signup })) => {
                let bounds = view_bounds(x, y, width, height);
                match &relay_view {
                    Some((shown, view)) if *shown == url => {
                        let _ = view.set_bounds(bounds);
                        let _ = view.set_visible(true);
                    }
                    _ => {
                        relay_view = None;
                        let p = redirect_proxy.clone();
                        let builder = WebViewBuilder::new_with_web_context(&mut context);
                        match account_view(builder, &window, &url, bounds, signup, move |to| {
                            let _ = p.send_event(UserEvent::AccountRedirect(to));
                        }) {
                            Ok(view) => {
                                let _ = view.focus();
                                relay_view = Some((url, view));
                            }
                            Err(e) => eprintln!("page du relais : {e}"),
                        }
                    }
                }
            }
            WinEvent::UserEvent(UserEvent::AccountView(ViewCommand::AccountViewHide)) => relay_view = None,
            WinEvent::UserEvent(UserEvent::AccountRedirect(to)) => {
                if let Some((_, view)) = &relay_view {
                    let _ = view.load_url(&to);
                }
            }
            WinEvent::UserEvent(UserEvent::Drag) => {
                let _ = window.drag_window();
            }
            WinEvent::UserEvent(UserEvent::Minimize) => window.set_minimized(true),
            WinEvent::UserEvent(UserEvent::Close) => *control_flow = ControlFlow::Exit,
            // Alt+F4, barre des tâches : ignoré pendant l'installation, qui
            // laisserait sinon l'ordinateur à moitié installé.
            WinEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if !*installing.lock().unwrap() {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// Fil de travail : reçoit les commandes de l'interface et exécute les tâches.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn spawn_backend(rx: mpsc::Receiver<String>, proxy: EventLoopProxy<UserEvent>, payload: Option<PathBuf>, installing: Arc<Mutex<bool>>) {
    let to_page = {
        let proxy = Mutex::new(proxy.clone());
        Arc::new(move |json: String| {
            let _ = proxy.lock().unwrap().send_event(UserEvent::Js(json));
        })
    };
    // Étape « Compte KyberNow » : le rattachement part quand Admin a démarré.
    let accounts = {
        let to_page = to_page.clone();
        Coordinator::new(Arc::new(move |event: AccountEvent| {
            if let Ok(json) = serde_json::to_string(&event) {
                to_page(json);
            }
        }))
    };
    let emit: Emit = {
        let accounts = accounts.clone();
        Arc::new(move |event: Event| {
            if matches!(&event, Event::Task { id, status: TaskStatus::Done } if id == "admin") || matches!(event, Event::Done { .. }) {
                accounts.admin_ready();
            }
            if let Ok(json) = serde_json::to_string(&event) {
                to_page(json);
            }
        })
    };
    let info: Arc<Mutex<SystemInfo>> = Arc::default();

    std::thread::spawn(move || {
        for message in rx {
            if let Ok(command) = serde_json::from_str::<AccountCommand>(&message) {
                accounts.handle(command);
                continue;
            }
            if let Ok(view) = serde_json::from_str::<ViewCommand>(&message) {
                let _ = proxy.send_event(UserEvent::AccountView(view));
                continue;
            }
            let command: Command = match serde_json::from_str(&message) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("commande illisible : {e} ({message})");
                    continue;
                }
            };
            match command {
                Command::RunChecks => {
                    let (emit, info) = (emit.clone(), info.clone());
                    std::thread::spawn(move || {
                        let result = kybernow::checks::run(&emit);
                        *info.lock().unwrap() = result;
                    });
                }
                Command::ValidatePorts { stream, console } => {
                    let excluded = kybernow::ports::excluded_ranges();
                    emit(Event::PortsValidated {
                        stream_error: kybernow::ports::validate(stream, console, &excluded),
                        console_error: kybernow::ports::validate(console, stream, &excluded),
                    });
                }
                Command::Install(request) => {
                    let mut busy = installing.lock().unwrap();
                    if *busy {
                        continue;
                    }
                    *busy = true;
                    let (emit, info, installing, payload) = (emit.clone(), info.clone(), installing.clone(), payload.clone());
                    std::thread::spawn(move || {
                        let mut info = info.lock().unwrap().clone();
                        info.stream_port = request.stream_port;
                        info.console_port = request.console_port;
                        let payload = payload.unwrap_or_default();
                        if let Ok(need_reboot) = kybernow::install::run_install(&request, &info, &payload, &emit) {
                            emit(Event::Done {
                                lan_ip: info.lan_ip.clone().or_else(kybernow::checks::lan_ip),
                                stream_port: request.stream_port,
                                console_port: request.console_port,
                                need_reboot,
                            });
                        }
                        *installing.lock().unwrap() = false;
                    });
                }
                Command::OpenUrl { url } => open_in_browser(&url),
                Command::Reboot => {
                    if cfg!(windows) {
                        let _ = kybernow::sys::run("shutdown", &["/r", "/t", "5", "/c", &t("ui.rebootReason", &[])]);
                    }
                    let _ = proxy.send_event(UserEvent::Close);
                }
                Command::Minimize => {
                    let _ = proxy.send_event(UserEvent::Minimize);
                }
                Command::Drag => {
                    let _ = proxy.send_event(UserEvent::Drag);
                }
                Command::Close => {
                    if !*installing.lock().unwrap() {
                        let _ = proxy.send_event(UserEvent::Close);
                    }
                }
            }
        }
    });
}

/// Habillage de Windows 11 sans cadre : coins arrondis, liseré sombre, et
/// l'ombre de la fenêtre (une marge d'un pixel étendue dans la zone cliente,
/// recouverte par la vue web).
#[cfg(windows)]
fn frame(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
    use windows_sys::Win32::UI::Controls::MARGINS;
    let corner: u32 = 2; // DWMWCP_ROUND
    let border: u32 = 0x0042_372B; // #2B3742 (COLORREF : 0x00BBGGRR)
    let dark: u32 = 1;
    let margins = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 0, cyBottomHeight: 1 };
    unsafe {
        DwmSetWindowAttribute(hwnd, 20 /* DWMWA_USE_IMMERSIVE_DARK_MODE */, (&dark as *const u32).cast(), 4);
        DwmSetWindowAttribute(hwnd, 33 /* DWMWA_WINDOW_CORNER_PREFERENCE */, (&corner as *const u32).cast(), 4);
        DwmSetWindowAttribute(hwnd, 34 /* DWMWA_BORDER_COLOR */, (&border as *const u32).cast(), 4);
        DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}
