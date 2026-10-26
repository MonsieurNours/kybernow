//! Le fond de la connexion sur les écrans de la session autres que l'écran
//! principal (docs/link-ui.md, « Plusieurs écrans ») : une fenêtre plein écran
//! sans texte, de la couleur de l'écran de connexion, jusqu'à la première
//! image de cet écran ; pour un écran que l'ordinateur n'a pas pu créer, la
//! même fenêtre avec une seule ligne, qui reste jusqu'à la fin. L'écran de
//! connexion lui-même (les étapes) est le passage de l'accueil, sur l'écran
//! principal (`ui.rs`).
//!
//! Ce sont des fenêtres Tauri (`fond.html`, dessinées comme le reste de
//! l'interface : même police pour la ligne). Elles ne prennent jamais le
//! clavier ni le premier plan.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Un écran de ce PC, en pixels du bureau de Windows.
#[derive(Debug, Clone)]
pub struct Target {
    pub key: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Ce que dit la page d'un fond : la ligne d'un écran que l'ordinateur n'a pas
/// pu créer, ou rien.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FondState {
    pub not_created: bool,
}

struct Fond {
    key: String,
    label: String,
    state: FondState,
}

pub struct Fonds {
    app: AppHandle,
    open: Mutex<Vec<Fond>>,
    seq: Mutex<u32>,
}

impl Fonds {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self { app, open: Mutex::new(Vec::new()), seq: Mutex::new(0) })
    }

    /// Le fond de la connexion sur chacun de ces écrans (sans texte), s'il
    /// n'y en a pas déjà un.
    pub fn show(self: &Arc<Self>, targets: &[Target]) {
        for t in targets {
            self.ensure(t, false);
        }
    }

    /// L'ordinateur n'a pas pu créer l'écran : le fond y garde sa ligne.
    pub fn not_created(self: &Arc<Self>, target: &Target) {
        self.ensure(target, true);
    }

    /// La page d'un fond la demande à son ouverture.
    pub fn state_of(&self, label: &str) -> FondState {
        self.open.lock().unwrap().iter().find(|f| f.label == label).map_or(FondState { not_created: false }, |f| f.state.clone())
    }

    /// Le fond de cet écran s'efface (son image est là).
    pub fn hide(self: &Arc<Self>, key: &str) {
        let label = {
            let mut open = self.open.lock().unwrap();
            let Some(i) = open.iter().position(|f| f.key == key) else { return };
            open.remove(i).label
        };
        self.destroy(label);
    }

    /// Ne garde que les fonds de ces écrans (les autres ne sont plus pris par la session).
    pub fn keep_only(self: &Arc<Self>, keys: &[String]) {
        let gone: Vec<String> = {
            let mut open = self.open.lock().unwrap();
            let mut gone = Vec::new();
            open.retain(|f| {
                let keep = keys.contains(&f.key);
                if !keep {
                    gone.push(f.label.clone());
                }
                keep
            });
            gone
        };
        for label in gone {
            self.destroy(label);
        }
    }

    pub fn close_all(self: &Arc<Self>) {
        let all: Vec<String> = self.open.lock().unwrap().drain(..).map(|f| f.label).collect();
        for label in all {
            self.destroy(label);
        }
    }

    fn ensure(self: &Arc<Self>, target: &Target, not_created: bool) {
        let state = FondState { not_created };
        let (label, create) = {
            let mut open = self.open.lock().unwrap();
            if let Some(f) = open.iter_mut().find(|f| f.key == target.key) {
                if f.state == state {
                    return;
                }
                f.state = state.clone();
                (f.label.clone(), false)
            } else {
                let mut seq = self.seq.lock().unwrap();
                *seq += 1;
                let label = format!("fond-{}", *seq);
                open.push(Fond { key: target.key.clone(), label: label.clone(), state: state.clone() });
                (label, true)
            }
        };
        if !create {
            let _ = self.app.emit_to(label.as_str(), "fond-state", state);
            return;
        }
        let app = self.app.clone();
        let target = target.clone();
        // Sur le fil de Tauri : la création d'une fenêtre n'y est pas bloquante.
        let _ = self.app.clone().run_on_main_thread(move || build(&app, &label, &target));
    }

    fn destroy(&self, label: String) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.destroy();
            }
        });
    }
}

/// Place la fenêtre sur son écran, en pixels réels (l'échelle de l'écran où elle arrive).
fn place(w: &tauri::WebviewWindow, t: &Target) {
    let _ = w.set_position(tauri::PhysicalPosition::new(t.x, t.y));
    let _ = w.set_size(tauri::PhysicalSize::new(t.width, t.height));
}

fn build(app: &AppHandle, label: &str, t: &Target) {
    // Même langue que les autres fenêtres, dès le premier affichage.
    let lang_script = format!("window.__KN_LANG = '{}';", crate::i18n::lang().code());
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("fond.html".into()))
        .initialization_script(&lang_script)
        .title(crate::shell::DISPLAY_NAME)
        .inner_size(320.0, 200.0)
        .decorations(false)
        .resizable(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        // Le fond de l'écran de connexion, avant même que la page soit là.
        .background_color(Color(5, 7, 10, 255))
        .build();
    let window = match built {
        Ok(w) => w,
        Err(e) => {
            log::warn!("fond de la connexion sur {} : {e}", t.key);
            return;
        }
    };
    place(&window, t);
    let _ = window.show();
    log::info!("fond de la connexion : écran {} ({}×{})", t.key, t.width, t.height);
    // Windows règle la taille d'une fenêtre à l'échelle de l'écran où elle
    // arrive : une seconde fois, une fois posée.
    let w = window.clone();
    let t = t.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        place(&w, &t);
    });
}
