//! La caméra de ce PC dans l'interface (docs/link-ui.md, « Caméra de
//! l'appareil ») : la puce « Caméra » de la barre et son menu, Ctrl+Alt+Maj+C,
//! les réponses de l'ordinateur (voyant, refus), la ligne des statistiques.
//! La capture et l'encodage sont dans `crate::camera`, le reste dans le cœur.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kybernow_core::camera::{self as core_camera, CameraEvent, CameraLink, CameraResolution, DeviceNotice};
use kybernow_core::i18n;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::UiState;
use crate::camera::{self, CameraChoice, CameraSender, Problem};

/// L'état de la caméra d'une session.
#[derive(Default)]
struct CamSession {
    offered: bool,
    /// 0 éteinte, 1 allumage en cours, 2 envoyée (voyant), 3 en pause.
    state: u8,
    sender: Option<CameraSender>,
    /// Le fil qui lit les réponses de l'ordinateur ; arrêté par `stop`.
    stop: Option<Arc<AtomicBool>>,
}

static SESSIONS: Mutex<Option<HashMap<String, CamSession>>> = Mutex::new(None);

fn with<T>(id: &str, f: impl FnOnce(&mut CamSession) -> T) -> T {
    let mut all = SESSIONS.lock().unwrap();
    f(all.get_or_insert_with(HashMap::new).entry(id.to_owned()).or_default())
}

/// Ce que la barre montre (évènement `bar-camera`, commande `session_camera_state`).
#[derive(Debug, Clone, Serialize)]
pub struct BarCamera {
    id: String,
    offered: bool,
    state: u8,
    cameras: Vec<CameraChoice>,
    current: Option<String>,
    /// La ligne « Caméra » des statistiques (« 1280 × 720 · 30 i/s · H.264 · 1,4 Mbit/s »).
    line: Option<String>,
}

/// La webcam choisie, gardée pour ce PC (pas par ordinateur).
fn choice_path() -> std::path::PathBuf {
    crate::system::camera_choice_path()
}

fn chosen() -> Option<String> {
    std::fs::read_to_string(choice_path()).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn link(app: &AppHandle, id: &str) -> Option<Arc<CameraLink>> {
    app.state::<UiState>().with_session(id, |r| r.host.camera_link()).flatten()
}

/// La liste des webcams, relue au plus toutes les 10 s : `MFEnumDeviceSources`
/// coûte des centaines de millisecondes avec des caméras USB ou virtuelles,
/// et la barre la demande à chaque fois qu'elle se montre.
static CAMERAS: Mutex<Option<(Instant, Vec<CameraChoice>)>> = Mutex::new(None);
const CAMERAS_FOR: Duration = Duration::from_secs(10);

fn cameras(fresh: bool) -> Vec<CameraChoice> {
    {
        let cached = CAMERAS.lock().unwrap();
        if let Some((at, list)) = cached.as_ref()
            && !fresh
            && at.elapsed() < CAMERAS_FOR
        {
            return list.clone();
        }
    }
    // Relue hors du verrou.
    let list = camera::cameras();
    *CAMERAS.lock().unwrap() = Some((Instant::now(), list.clone()));
    list
}

fn snapshot(app: &AppHandle, id: &str) -> BarCamera {
    let (offered, state) = with(id, |c| (c.offered, c.state));
    let cameras = if offered { cameras(false) } else { Vec::new() };
    let current = chosen().filter(|c| cameras.iter().any(|x| &x.id == c)).or_else(|| cameras.first().map(|c| c.id.clone()));
    let line = link(app, id).and_then(|l| core_camera::stats_line(&l.stats(), i18n::current()));
    BarCamera { id: id.to_owned(), offered, state, cameras, current, line }
}

fn emit(app: &AppHandle, id: &str) {
    let _ = app.emit("bar-camera", snapshot(app, id));
}

fn notice(app: &AppHandle, id: &str, text: &str) {
    let name = app.state::<UiState>().with_session(id, |r| r.name.clone()).unwrap_or_default();
    super::share::notice(app, id, &name, text);
}

/// L'ordinateur propose la caméra, ou plus (le cœur, `UiEvents::camera_offer`).
/// Le flux qui revient la relance de lui-même ; le réglage apparaît dans les
/// réglages de l'ordinateur.
pub fn offer(app: &AppHandle, id: &str, offered: bool) {
    with(id, |c| c.offered = offered);
    if offered {
        let state = app.state::<UiState>();
        state.with_store(|s| {
            if let Some(c) = s.get_mut(id)
                && !c.camera_offered
            {
                c.camera_offered = true;
                s.save();
            }
        });
        super::emit_computers(app);
    }
    emit(app, id);
}

/// La session finit : la caméra s'éteint.
pub fn ended(id: &str) {
    let removed = SESSIONS.lock().unwrap().get_or_insert_with(HashMap::new).remove(id);
    // Hors du verrou : l'arrêt de la webcam peut attendre (`CameraSender`).
    if let Some(c) = removed {
        if let Some(stop) = c.stop {
            stop.store(true, Ordering::Relaxed);
        }
        drop(c.sender);
    }
}

fn turn_off(app: &AppHandle, id: &str) {
    let (sender, stop) = with(id, |c| {
        c.state = 0;
        (c.sender.take(), c.stop.take())
    });
    if let Some(stop) = stop {
        stop.store(true, Ordering::Relaxed);
    }
    drop(sender);
    if let Some(l) = link(app, id) {
        l.stop();
    }
    emit(app, id);
}

fn turn_on(app: &AppHandle, id: &str, camera_id: Option<String>) {
    let Some(l) = link(app, id) else { return };
    // Une autre caméra pendant qu'elle part : la précédente s'arrête d'abord, START de nouveau.
    let old = with(id, |c| c.sender.take());
    drop(old);
    let res1080 = app.state::<UiState>().with_store(|s| s.get_mut(id).is_some_and(|c| c.settings.camera_1080));
    let resolution = if res1080 { CameraResolution::P1080 } else { CameraResolution::P720 };
    let device = std::env::var("COMPUTERNAME").unwrap_or_default();
    let (a, i) = (app.clone(), id.to_owned());
    // Appelé du fil de la caméra : la suite (qui arrête ce fil) part sur un autre.
    let problem: Arc<dyn Fn(Problem, String) + Send + Sync> = Arc::new(move |p, name| {
        let (a, i) = (a.clone(), i.clone());
        std::thread::spawn(move || handle_problem(&a, &i, p, name));
    });
    let sender = CameraSender::start(l.clone(), camera_id, resolution, device, problem);

    let stop = Arc::new(AtomicBool::new(false));
    let old_stop = with(id, |c| {
        c.sender = Some(sender);
        if c.state == 0 {
            c.state = 1;
        }
        c.stop.replace(stop.clone())
    });
    if let Some(s) = old_stop {
        s.store(true, Ordering::Relaxed);
    }
    emit(app, id);
    // Les réponses de l'ordinateur, quatre fois par seconde ; la ligne des statistiques chaque seconde.
    let (app, id) = (app.clone(), id.to_owned());
    std::thread::spawn(move || {
        let mut ticks = 0u32;
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(250));
            ticks += 1;
            let name = app.state::<UiState>().with_session(&id, |r| r.name.clone()).unwrap_or_default();
            for e in l.poll() {
                match &e {
                    CameraEvent::Started => {
                        with(&id, |c| c.state = 2);
                        emit(&app, &id);
                    }
                    CameraEvent::Busy { .. } | CameraEvent::Failed { .. } => {
                        if let Some(text) = core_camera::event_notice(&e, &name, i18n::current()) {
                            notice(&app, &id, &text);
                        }
                        turn_off(&app, &id);
                        return;
                    }
                    CameraEvent::KeyframeWanted => with(&id, |c| {
                        if let Some(s) = &c.sender {
                            s.force_keyframe();
                        }
                    }),
                    _ => {}
                }
            }
            if ticks % 4 == 0 {
                emit(&app, &id);
            }
        }
    });
}

/// Un problème de la webcam : l'avis ; débranchée, la suivante de la liste
/// prend la relève, sinon la caméra s'éteint.
fn handle_problem(app: &AppHandle, id: &str, p: Problem, name: String) {
    let which = match p {
        Problem::Denied => DeviceNotice::Denied,
        Problem::InUse => DeviceNotice::InUse,
        Problem::Unplugged => DeviceNotice::Unplugged,
        Problem::NoEncoder => DeviceNotice::NoEncoder,
    };
    notice(app, id, &core_camera::device_notice(which, &name, i18n::current()));
    let next = cameras(true).into_iter().find(|c| c.name != name);
    match (p, next) {
        (Problem::Unplugged, Some(next)) if with(id, |c| c.state != 0) => turn_on(app, id, Some(next.id)),
        _ => turn_off(app, id),
    }
}

/// Ctrl+Alt+Maj+C, la puce, le menu : l'allume ou l'éteint.
pub fn toggle(app: &AppHandle, id: &str) {
    let (offered, state) = with(id, |c| (c.offered, c.state));
    if !offered {
        return;
    }
    if state == 0 { turn_on(app, id, chosen()) } else { turn_off(app, id) }
}

/// Sur un fil de Tauri, jamais celui des fenêtres : la liste des webcams peut
/// être relue.
#[tauri::command(async)]
pub fn session_camera_state(app: AppHandle, id: String) -> BarCamera {
    snapshot(&app, &id)
}

/// « Allumer la caméra » / « Éteindre la caméra ».
#[tauri::command(async)]
pub fn session_camera(app: AppHandle, id: String, on: bool) -> BarCamera {
    let state = with(&id, |c| c.state);
    if on && state == 0 {
        turn_on(&app, &id, chosen());
    } else if !on && state != 0 {
        turn_off(&app, &id);
    }
    snapshot(&app, &id)
}

/// Une webcam de la liste : gardée, prise tout de suite si la caméra est allumée.
#[tauri::command(async)]
pub fn session_camera_pick(app: AppHandle, id: String, camera: String) -> BarCamera {
    let _ = std::fs::write(choice_path(), &camera);
    if with(&id, |c| c.state != 0) {
        turn_on(&app, &id, Some(camera));
    }
    snapshot(&app, &id)
}
