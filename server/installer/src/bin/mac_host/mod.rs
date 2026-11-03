//! Fenêtres de KyberNow Station pour Mac (`ui/src/mac`) : barre de titre
//! transparente, feux de macOS posés sur la colonne de la maquette, et
//! le moteur de `kybernow::mac` derrière.
//!
//! Quatre usages, choisis par `--mode` (ou la clé `KyberNowMode` du paquet
//! qui contient l'exécutable : l'app « Désinstaller » du .dmg) :
//! `install` (par défaut, l'app « Installer KyberNow Station »), `update`,
//! `uninstall` et `permissions` (ouverts par l'icône de la barre des menus).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use kybernow::i18n::{self, t};
use kybernow::mac::checks::MacInfo;
use kybernow::mac::commands::{Command, Emit, Event};
use kybernow::mac::{self, permissions};
use tao::dpi::LogicalSize;
use tao::event::{Event as WinEvent, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;
use wry::http::Request;

enum UserEvent {
    Js(String),
    Drag,
    Close,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Install,
    Update,
    Uninstall,
    Permissions,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Install => "install",
            Mode::Update => "update",
            Mode::Uninstall => "uninstall",
            Mode::Permissions => "permissions",
        }
    }

    fn parse(s: &str) -> Option<Mode> {
        Some(match s {
            "install" => Mode::Install,
            "update" => Mode::Update,
            "uninstall" => Mode::Uninstall,
            "permissions" => Mode::Permissions,
            _ => return None,
        })
    }

    /// Taille de la maquette.
    fn size(self) -> (f64, f64) {
        match self {
            Mode::Install => (960.0, 640.0),
            Mode::Update => (760.0, 520.0),
            Mode::Uninstall => (760.0, 580.0),
            Mode::Permissions => (760.0, 560.0),
        }
    }
}

/// Paquet `.app` qui contient cet exécutable.
fn own_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let contents = exe.parent()?.parent()?;
    (contents.file_name()? == "Contents").then(|| contents.parent().map(PathBuf::from)).flatten()
}

fn mode() -> Mode {
    if let Some(m) = super::arg("--mode").and_then(|m| Mode::parse(&m.to_string_lossy())) {
        return m;
    }
    // L'app « Désinstaller » du .dmg : même exécutable, `KyberNowMode` dans son Info.plist.
    if let Some(bundle) = own_bundle()
        && let Ok(plist) = std::fs::read_to_string(bundle.join("Contents/Info.plist"))
        && let Some(rest) = plist.split("<key>KyberNowMode</key>").nth(1)
        && let Some(value) = rest.split("<string>").nth(1).and_then(|v| v.split("</string>").next())
        && let Some(m) = Mode::parse(value.trim())
    {
        return m;
    }
    Mode::Install
}

/// L'app à installer : `--payload`, sinon `KyberNow Station.app` dans les
/// ressources de l'installateur.
fn payload() -> PathBuf {
    super::arg("--payload")
        .or_else(|| own_bundle().map(|b| b.join("Contents/Resources").join(mac::APP_NAME)))
        .unwrap_or_else(|| PathBuf::from(mac::APP_NAME))
}

fn title(mode: Mode) -> String {
    t(&format!("mac.window.{}", mode.name()), &[])
}

/// Menus de l'app : le nom (À propos, Masquer, Quitter) et Édition, sans
/// lequel copier-coller ne marche pas dans les champs.
fn menus(mode: Mode) -> muda::Menu {
    use muda::{Menu, PredefinedMenuItem, Submenu};
    let menu = Menu::new();
    let app = Submenu::new(title(mode), true);
    let _ = app.append_items(&[
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::close_window(None),
    ]);
    let edit = Submenu::new(t("mac.window.edit", &[]), true);
    let _ = edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);
    let _ = menu.append_items(&[&app, &edit]);
    menu
}

pub fn main() -> wry::Result<()> {
    let mode = mode();
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let (w, h) = mode.size();
    let window = WindowBuilder::new()
        .with_title(title(mode))
        .with_inner_size(LogicalSize::new(w, h))
        .with_resizable(false)
        .with_minimizable(mode == Mode::Install)
        .with_titlebar_transparent(true)
        .with_title_hidden(true)
        .with_fullsize_content_view(true)
        .build(&event_loop)
        .expect("création de la fenêtre");
    let menu = menus(mode);
    menu.init_for_nsapp();

    let (tx, rx) = mpsc::channel::<String>();
    // Pendant l'installation ou la désinstallation, la fenêtre ne se ferme pas.
    let busy = Arc::new(AtomicBool::new(false));
    spawn_backend(rx, proxy.clone(), busy.clone());

    let webview = WebViewBuilder::new()
        .with_custom_protocol("kn".into(), |_id, request| super::serve(request))
        .with_initialization_script(format!(
            "window.__knLang = {:?}; window.__knPlatform = 'macos'; window.__knMode = {:?};",
            i18n::lang().code(),
            mode.name()
        ))
        .with_ipc_handler(move |req: Request<String>| {
            let _ = tx.send(req.body().clone());
        })
        .with_background_color((5, 7, 10, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_url("kn://localhost/")
        .build(&window)?;
    place_lights(&window);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _ = &menu;
        match event {
            WinEvent::UserEvent(UserEvent::Js(json)) => {
                let _ = webview.evaluate_script(&format!("window.__knReceive && window.__knReceive({json})"));
            }
            WinEvent::UserEvent(UserEvent::Drag) => {
                let _ = window.drag_window();
            }
            WinEvent::UserEvent(UserEvent::Close) => *control_flow = ControlFlow::Exit,
            WinEvent::WindowEvent { event: WindowEvent::Focused(_) | WindowEvent::Resized(_), .. } => place_lights(&window),
            WinEvent::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if !busy.load(Ordering::SeqCst) {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

/// Feux de la fenêtre à 20 points du haut et de la gauche, sur la colonne
/// (maquette). tao sait le faire, mais seulement quand sa propre vue se
/// dessine, et la vue web la recouvre : on le refait ici, à l'ouverture et
/// quand macOS les remet en place (focus).
fn place_lights(window: &tao::window::Window) {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use tao::platform::macos::WindowExtMacOS;
    const X: f64 = 20.0;
    const Y: f64 = 20.0;
    let ns_window = window.ns_window() as *const NSWindow;
    let Some(ns_window) = (unsafe { ns_window.as_ref() }) else { return };
    let buttons = [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton].map(|b| ns_window.standardWindowButton(b));
    let [Some(close), Some(mini), Some(zoom)] = buttons else { return };
    let Some(bar) = (unsafe { close.superview() }).and_then(|v| unsafe { v.superview() }) else { return };
    let close_frame = close.frame();
    let height = close_frame.size.height + Y;
    let mut frame = bar.frame();
    frame.size.height = height;
    frame.origin.y = ns_window.frame().size.height - height;
    bar.setFrame(frame);
    let gap = mini.frame().origin.x - close_frame.origin.x;
    // Repère de la barre de titre : y depuis le bas (vue non retournée).
    let Some(row) = (unsafe { close.superview() }) else { return };
    let row_height = row.frame().size.height;
    let y = if row.isFlipped() { Y } else { row_height - Y - close_frame.size.height };
    for (i, button) in [&close, &mini, &zoom].into_iter().enumerate() {
        let mut origin = button.frame().origin;
        origin.x = X + i as f64 * gap;
        origin.y = y;
        button.setFrameOrigin(origin);
    }
}

fn spawn_backend(rx: mpsc::Receiver<String>, proxy: EventLoopProxy<UserEvent>, busy: Arc<AtomicBool>) {
    let emit: Emit = {
        let proxy = Mutex::new(proxy.clone());
        Arc::new(move |event: Event| {
            if let Ok(json) = serde_json::to_string(&event) {
                let _ = proxy.lock().unwrap().send_event(UserEvent::Js(json));
            }
        })
    };
    let info: Arc<Mutex<MacInfo>> = Arc::default();
    // Génération de la surveillance des autorisations : une seule boucle à la fois.
    let watching = Arc::new(AtomicU64::new(0));

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
                    std::thread::spawn(move || *info.lock().unwrap() = mac::checks::run(&emit));
                }
                Command::ValidatePorts { stream, console } => emit(Event::PortsValidated {
                    stream_error: kybernow::ports::validate(stream, console, &[]),
                    console_error: kybernow::ports::validate(console, stream, &[]),
                }),
                Command::Install(request) => {
                    if busy.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    let (emit, info, busy) = (emit.clone(), info.clone(), busy.clone());
                    std::thread::spawn(move || {
                        let info = info.lock().unwrap().clone();
                        let _ = mac::install::run(&request, &info, &payload(), &emit);
                        busy.store(false, Ordering::SeqCst);
                    });
                }
                Command::WatchPermissions { on } => {
                    let generation = watching.fetch_add(1, Ordering::SeqCst) + 1;
                    if on {
                        let (emit, watching) = (emit.clone(), watching.clone());
                        std::thread::spawn(move || {
                            while watching.load(Ordering::SeqCst) == generation {
                                match permissions::check(&mac::app_path()) {
                                    Ok(event) => emit(event),
                                    Err(e) => eprintln!("autorisations illisibles : {e:#}"),
                                }
                                std::thread::sleep(Duration::from_millis(1500));
                            }
                        });
                    }
                }
                Command::RequestPermission { which } => {
                    if let Err(e) = permissions::request(&mac::app_path(), which) {
                        eprintln!("autorisation non demandée : {e:#}");
                    }
                }
                Command::PermissionsDone => permissions::apply(),
                Command::UpdateCheck => {
                    let emit = emit.clone();
                    std::thread::spawn(move || emit(mac::update::info()));
                }
                Command::UpdateStart => {
                    if busy.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    let (emit, busy) = (emit.clone(), busy.clone());
                    std::thread::spawn(move || {
                        mac::update::start(&emit);
                        busy.store(false, Ordering::SeqCst);
                    });
                }
                Command::Uninstall { keep_settings } => {
                    if busy.swap(true, Ordering::SeqCst) {
                        continue;
                    }
                    let (emit, busy) = (emit.clone(), busy.clone());
                    std::thread::spawn(move || {
                        match mac::uninstall::run(keep_settings) {
                            Ok(()) => emit(Event::UninstallDone),
                            Err(e) => {
                                let message = if mac::root::is_cancelled(&e) { t("mac.uninstall.cancelled", &[]) } else { format!("{e:#}") };
                                emit(Event::UninstallFailed { message });
                            }
                        }
                        busy.store(false, Ordering::SeqCst);
                    });
                }
                Command::OpenUrl { url } => {
                    if url.starts_with("https://") || url.starts_with("http://") {
                        let _ = std::process::Command::new("/usr/bin/open").arg(&url).spawn();
                    }
                }
                Command::Copy { text } => {
                    use std::io::Write;
                    if let Ok(mut child) = std::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).spawn() {
                        if let Some(stdin) = child.stdin.as_mut() {
                            let _ = stdin.write_all(text.as_bytes());
                        }
                        let _ = child.wait();
                    }
                }
                Command::Drag => {
                    let _ = proxy.send_event(UserEvent::Drag);
                }
                Command::Close => {
                    if !busy.load(Ordering::SeqCst) {
                        let _ = proxy.send_event(UserEvent::Close);
                    }
                }
            }
        }
    });
}
