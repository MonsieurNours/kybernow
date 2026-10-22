//! Interface de l'installateur : une fenêtre sans bordure avec une vue
//! WebView2 qui affiche l'app Svelte de `ui/dist` (embarquée par build.rs).
//!
//! Le JavaScript envoie des commandes via `window.ipc.postMessage(json)` ;
//! l'installateur répond en appelant `window.__knReceive(event)`.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("l'interface de l'installateur ne tourne que sous Windows.");
}

#[cfg(windows)]
fn main() -> wry::Result<()> {
    ui::main()
}

#[cfg(windows)]
mod ui {
    use std::borrow::Cow;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::Duration;

    use knsetup::events::{Command, Emit, Event};
    use knsetup::i18n::{lang, tr};
    use knsetup::{checks, install, sys};
    use tao::dpi::LogicalSize;
    use tao::event::{Event as WinEvent, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
    use tao::platform::windows::{WindowBuilderExtWindows, WindowExtWindows};
    use tao::window::WindowBuilder;
    use wry::http::{Request, Response, header::CONTENT_TYPE};
    use wry::{WebContext, WebViewBuilder};

    include!(concat!(env!("OUT_DIR"), "/assets.rs"));

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

    fn mime(path: &str) -> &'static str {
        match path.rsplit('.').next().unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "svg" => "image/svg+xml",
            "woff2" => "font/woff2",
            "png" => "image/png",
            "json" => "application/json",
            _ => "application/octet-stream",
        }
    }

    fn serve(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
        let path = request.uri().path().trim_start_matches('/');
        let path = if path.is_empty() { "index.html" } else { path };
        let found = ASSETS.iter().find(|(p, _)| *p == path).or_else(|| ASSETS.iter().find(|(p, _)| *p == "index.html"));
        match found {
            Some((p, data)) => Response::builder().header(CONTENT_TYPE, mime(p)).body(Cow::Borrowed(*data)).unwrap(),
            None => Response::builder()
                .status(404)
                .body(Cow::Borrowed(b"Interface non compilee : npm run build dans apps/windows/setup/ui" as &[u8]))
                .unwrap(),
        }
    }

    pub fn main() -> wry::Result<()> {
        let _ = std::env::set_current_dir(std::env::temp_dir());
        sys::init_log("setup-ui");
        let payload = arg("--payload");
        let webview_data = arg("--webview-data").unwrap_or_else(|| std::env::temp_dir().join("KyberNow-Link-Setup-WebView"));

        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let proxy = event_loop.create_proxy();
        // L'ombre de tao réserve autour de la fenêtre une bande de cadre que
        // Windows peint en clair : on la fait nous-mêmes, sans bande (frame).
        let window = WindowBuilder::new()
            .with_title(tr(lang(), "setup.windowTitle", &[]))
            .with_inner_size(LogicalSize::new(1000.0, 640.0))
            .with_resizable(false)
            .with_decorations(false)
            .with_undecorated_shadow(false)
            .build(&event_loop)
            .expect("création de la fenêtre");
        frame(window.hwnd() as _);

        let (tx, rx) = mpsc::channel::<String>();
        // Vrai pendant l'installation : la fenêtre refuse alors de se fermer.
        let installing = Arc::new(Mutex::new(false));
        spawn_backend(rx, proxy, payload, installing.clone());

        let mut context = WebContext::new(Some(webview_data));
        let webview = WebViewBuilder::new_with_web_context(&mut context)
            .with_custom_protocol("kn".into(), |_id, request| serve(request))
            .with_ipc_handler(move |req: Request<String>| {
                let _ = tx.send(req.body().clone());
            })
            .with_background_color((5, 7, 10, 255))
            .with_devtools(cfg!(debug_assertions))
            // La langue de Windows, passée à l'interface (l'installateur n'a pas de réglage).
            .with_url(format!("http://kn.localhost/?lang={}", lang().code()))
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
                WinEvent::UserEvent(UserEvent::Close) => *control_flow = ControlFlow::Exit,
                // Alt+F4, barre des tâches : ignoré pendant l'installation.
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
    fn spawn_backend(rx: mpsc::Receiver<String>, proxy: EventLoopProxy<UserEvent>, payload: Option<PathBuf>, installing: Arc<Mutex<bool>>) {
        let emit: Emit = {
            let proxy = Mutex::new(proxy.clone());
            Arc::new(move |event: Event| {
                if let Ok(json) = serde_json::to_string(&event) {
                    let _ = proxy.lock().unwrap().send_event(UserEvent::Js(json));
                }
            })
        };

        std::thread::spawn(move || {
            for message in rx {
                let command: Command = match serde_json::from_str(&message) {
                    Ok(c) => c,
                    Err(e) => {
                        sys::log(&format!("commande illisible : {e} ({message})"));
                        continue;
                    }
                };
                match command {
                    Command::RunChecks => {
                        let emit = emit.clone();
                        std::thread::spawn(move || checks::run(&emit));
                    }
                    Command::Install { desktop } => {
                        let mut busy = installing.lock().unwrap();
                        if *busy {
                            continue;
                        }
                        *busy = true;
                        let (emit, installing, payload) = (emit.clone(), installing.clone(), payload.clone().unwrap_or_default());
                        std::thread::spawn(move || {
                            let _ = install::run_install(&payload, desktop, Duration::from_secs(3), &emit);
                            *installing.lock().unwrap() = false;
                        });
                    }
                    Command::Launch => {
                        if let Err(e) = sys::launch_app(&[]) {
                            sys::log(&format!("ouverture de l'app : {e:#}"));
                        }
                        let _ = proxy.send_event(UserEvent::Close);
                    }
                    Command::OpenUrl { url } => sys::open_url(&url),
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
    /// l'ombre de la fenêtre (une marge d'un pixel étendue dans la zone cliente).
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
}
