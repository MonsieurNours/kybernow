//! Interface de l'installateur : une fenêtre sans bordure avec une vue web
//! (WebView2 sous Windows) qui affiche l'app Svelte de `ui/dist`.
//!
//! Le JavaScript envoie des commandes via `window.ipc.postMessage(json)` ;
//! l'installateur répond en appelant `window.__knReceive(event)`.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use kybernow::checks::SystemInfo;
use kybernow::events::{Command, Emit, Event};
use rust_embed::Embed;
use tao::dpi::LogicalSize;
use tao::event::{Event as WinEvent, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::http::{Request, Response, header::CONTENT_TYPE};
use wry::{WebContext, WebViewBuilder};

#[derive(Embed)]
#[folder = "ui/dist/"]
#[allow_missing = true]
struct Assets;

enum UserEvent {
    Js(String),
    Drag,
    Minimize,
    Close,
}

fn arg(name: &str) -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from)
}

fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
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

fn main() -> wry::Result<()> {
    let payload = arg("--payload");
    let webview_data = arg("--webview-data").unwrap_or_else(|| std::env::temp_dir().join("KyberNow-WebView"));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let builder = WindowBuilder::new()
        .with_title("Installation de KyberNow")
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
    spawn_backend(rx, proxy.clone(), payload);

    let mut context = WebContext::new(Some(webview_data));
    let url = if cfg!(windows) { "http://kn.localhost/" } else { "kn://localhost/" };
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_custom_protocol("kn".into(), |_id, request| serve(request))
        .with_ipc_handler(move |req: Request<String>| {
            let _ = tx.send(req.body().clone());
        })
        .with_background_color((5, 7, 10, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_url(url)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            WinEvent::UserEvent(UserEvent::Js(json)) => {
                let _ = webview.evaluate_script(&format!("window.__knReceive && window.__knReceive({json})"));
            }
            WinEvent::UserEvent(UserEvent::Drag) => {
                let _ = window.drag_window();
            }
            WinEvent::UserEvent(UserEvent::Minimize) => window.set_minimized(true),
            WinEvent::UserEvent(UserEvent::Close)
            | WinEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

/// Fil de travail : reçoit les commandes de l'interface et exécute les tâches.
fn spawn_backend(rx: mpsc::Receiver<String>, proxy: EventLoopProxy<UserEvent>, payload: Option<PathBuf>) {
    let emit: Emit = {
        let proxy = Mutex::new(proxy.clone());
        Arc::new(move |event: Event| {
            if let Ok(json) = serde_json::to_string(&event) {
                let _ = proxy.lock().unwrap().send_event(UserEvent::Js(json));
            }
        })
    };
    let info: Arc<Mutex<SystemInfo>> = Arc::default();
    let installing = Arc::new(Mutex::new(false));

    std::thread::spawn(move || {
        for message in rx {
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
                Command::OpenUrl { url } => {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        let opener = if cfg!(windows) { ("cmd", vec!["/C", "start", "", url.as_str()]) } else { ("open", vec![url.as_str()]) };
                        let _ = std::process::Command::new(opener.0).args(opener.1).spawn();
                    }
                }
                Command::Reboot => {
                    if cfg!(windows) {
                        let _ = kybernow::sys::run("shutdown", &["/r", "/t", "5", "/c", "Redémarrage pour terminer l'installation de KyberNow"]);
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
