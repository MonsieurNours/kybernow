//! Le test du profil Auto (docs/profil-auto.md) : `autotest::run` du cœur, sur un fil à part (une dizaine de
//! secondes), vers la console d'Admin de l'ordinateur (celle de l'accueil : celle d'un compte, ou de l'appareil
//! appairé ; par le relais quand l'ordinateur est « Hors de chez vous »), avec ce que cet appareil est : l'écran de
//! la fenêtre, ce qu'il décode, le choix « HDR » de l'ordinateur, le réseau du moment (« Ethernet », le nom du Wi-Fi)
//! et le câble vers l'ordinateur (`system::wired_to`). Les extraits passent au décodeur de la session
//! (`video::decode_sample` : FFmpeg, VA-API ou les décodeurs V4L2 du Pi, PyroWave par Vulkan). L'avancement et le
//! résultat reviennent à l'interface traduits (`Event::AutoProgress`, `Event::AutoDone`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use kybernow_core::autotest::{self, AutoError, Device, Progress, Sample, StepState};
use kybernow_core::console::Mode;

use super::Event;
use crate::app::App;
use crate::autotest as ui;

/// L'arrêt du test en cours (« Annuler », ou un nouveau test).
static CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Arrête le test en cours, s'il y en a un (entre deux mesures : le cœur le voit au plus tard après l'extrait en
/// cours de décodage, 1 s).
pub fn cancel() {
    if let Some(flag) = CANCEL.lock().unwrap().take() {
        flag.store(true, Ordering::Relaxed);
        log::info!("profil Auto : test annulé");
    }
}

/// Lance le test vers cet ordinateur, sous le numéro `attempt`.
pub fn start(app: &App, host: &str, attempt: u32, tx: Sender<Event>) {
    let Some(h) = app.store.host(host).cloned() else { return };
    cancel();
    let flag = Arc::new(AtomicBool::new(false));
    *CANCEL.lock().unwrap() = Some(flag.clone());
    let relay = app.station_via_relay(&h.id);
    let console = Arc::new(super::accounts::admin(&h, relay));
    let m = app.machine.clone();
    let caps = super::session::guest_capabilities(&m);
    let hdr = h.settings.hdr && super::session::hdr_decodes(&m);
    let network = app.network_name();
    let (width, height) = m.display;
    let screen = Mode { width, height, refresh: m.refresh.max(30) };
    let lang = kybernow_core::i18n::current();
    log::info!("profil Auto : test vers {} ({}, écran {width} × {height} à {} Hz, réseau {:?})", h.name, if relay { "par le relais" } else { "réseau local" }, screen.refresh, network);
    let spawned = std::thread::Builder::new().name("kybernow-autotest".into()).spawn(move || {
        // Le câble vers l'ordinateur : l'interface qui y mène (sans adresse, un ordinateur joint par le relais seul).
        let wired = !relay && !h.address.is_empty() && super::system::wired_to(&h.address, h.port);
        let device = Device { screen, caps, hdr, network, wired };
        let mut decoder = |s: &Sample| {
            let fps = super::video::decode_sample(s, &m);
            let what = fps.map_or("pas décodé".to_string(), |f| format!("{f:.0} i/s"));
            log::info!("profil Auto : {} {}{} × {} : {what}", s.codec.label(), if s.yuv444 { "4:4:4 " } else { "" }, s.width, s.height);
            fps
        };
        let ptx = tx.clone();
        let mut progress = |p: &Progress| {
            let _ = ptx.send(Event::AutoProgress { attempt, steps: steps_of(p) });
        };
        let result = match autotest::run(console, &device, &mut decoder, &mut progress, &flag, lang) {
            Ok(r) => {
                log::info!("profil Auto : {} {}×{} à {} Hz, {} Mbit/s, 4:4:4 {}, {}", r.codec.label(), r.width, r.height, r.refresh, r.mbps, r.yuv444, if r.reliable { "fiable" } else { "rapide" });
                let choices = r.choices(lang).into_iter().map(|c| ui::Choice { title: c.title, value: c.value, reason: c.reason }).collect();
                match serde_json::to_value(&r) {
                    Ok(json) => Ok((choices, json)),
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(AutoError::Cancelled) => return,
            Err(e) => {
                log::warn!("profil Auto : {e:?}");
                Err(e.text(lang, &h.name))
            }
        };
        let _ = tx.send(Event::AutoDone { attempt, result });
    });
    if spawned.is_err() {
        log::warn!("profil Auto : fil du test impossible");
    }
}

/// Les étapes du cœur, pour l'interface.
fn steps_of(p: &Progress) -> Vec<ui::Step> {
    p.steps
        .iter()
        .map(|s| ui::Step {
            title: s.title.clone(),
            state: match &s.state {
                StepState::Waiting => ui::StepState::Waiting,
                StepState::Running => ui::StepState::Running,
                StepState::Done(t) => ui::StepState::Done(t.clone()),
                StepState::Skipped(t) => ui::StepState::Skipped(t.clone()),
            },
        })
        .collect()
}
