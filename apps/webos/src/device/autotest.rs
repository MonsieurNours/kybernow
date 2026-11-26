//! Profil Auto, côté téléviseur (docs/profil-auto.md) : le test des conditions du cœur (`autotest::run`) sur un fil
//! à part, avec ce que le téléviseur sait de lui (sa dalle, ce que NDL décode, le HDR choisi pour l'ordinateur, le
//! réseau du moment, câble ou non) et la console d'Admin que l'app emploie déjà (relais compris). Le décodage passe
//! par NDL, comme une session (`ndl::bench`). L'avancement et la fin reviennent à la boucle principale, mis en mots
//! dans la langue de l'app.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kybernow_core::autotest::{self, Device, Progress, Sample};
use kybernow_core::console::Mode;
use kybernow_core::profile::{Capabilities, Codec};

use super::{ndl, system, Event};
use crate::app::{App, NetKind, Online};
use crate::autotest::{Choice, Step, StepState};

/// Ce que le test renvoie à la boucle principale.
pub enum AutoEvent {
    Progress { attempt: u32, steps: Vec<Step> },
    /// Le résultat (JSON de `AutoResult`) et son résumé, ou le texte de l'échec.
    Done { attempt: u32, result: Result<(serde_json::Value, Vec<Choice>), String> },
}

/// Le drapeau d'arrêt du test en cours (un seul à la fois).
static CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Arrête le test en cours (« Annuler », ou un nouveau test).
pub fn cancel() {
    if let Some(c) = CANCEL.lock().unwrap().take() {
        c.store(true, Ordering::Relaxed);
    }
}

/// Une seconde au plus par extrait (docs/profil-auto.md).
const SAMPLE_BUDGET: Duration = Duration::from_secs(1);

/// Le décodeur du téléviseur pour le test : NDL, par le chemin de la session ; PyroWave n'existe pas ici.
fn decode(sample: &Sample) -> Option<f64> {
    let kind = match sample.codec {
        Codec::Hevc => ndl::VIDEO_H265,
        Codec::Av1 => ndl::VIDEO_AV1,
        Codec::H264 => ndl::VIDEO_H264,
        Codec::PyroWave => return None,
    };
    let packets: Vec<(bool, &[u8])> = sample.packets.iter().map(|p| (p.is_key, p.data.as_slice())).collect();
    ndl::bench(kind, sample.width, sample.height, &packets, SAMPLE_BUDGET)
}

fn steps(p: &Progress) -> Vec<Step> {
    p.steps
        .iter()
        .map(|s| Step {
            title: s.title.clone(),
            state: match &s.state {
                autotest::StepState::Waiting => StepState::Waiting,
                autotest::StepState::Running => StepState::Running,
                autotest::StepState::Done(t) => StepState::Done(t.clone()),
                autotest::StepState::Skipped(t) => StepState::Skipped(t.clone()),
            },
        })
        .collect()
}

/// Lance le test pour cet ordinateur, sous ce numéro (un test d'avant est arrêté).
pub fn start(app: &App, host: &str, attempt: u32, tx: Sender<Event>) {
    cancel();
    let send = move |tx: &Sender<Event>, e: AutoEvent| {
        let _ = tx.send(Event::Auto(e));
    };
    let Some(h) = app.store.host(host).cloned() else { return };
    let lang = kybernow_core::i18n::current();
    if !app.tv.decoder || app.session.is_some() {
        send(&tx, AutoEvent::Done { attempt, result: Err(autotest::AutoError::Cancelled.text(lang, &h.name)) });
        return;
    }
    // Hors de chez soi, Admin par le relais (le test le dit « Par le relais »).
    let relay = app.live.get(&h.id).is_some_and(|l| l.online == Online::Away);
    let console = Arc::new(super::accounts::admin(&h, relay));
    let (pw, ph) = app.tv.panel;
    let device = Device {
        screen: Mode { width: pw, height: ph, refresh: app.tv.refresh.max(60) },
        // Ce que NDL décode : HEVC, AV1 s'il l'a ; jamais de 4:4:4 ni de PyroWave.
        caps: Capabilities { hevc: true, hevc444: false, av1: app.tv.av1, hdr: app.tv.hdr_decoder, pyrowave: false },
        hdr: app.hdr_on(&h),
        network: None,
        wired: app.net.kind == NetKind::Ethernet,
    };
    let kind = app.net.kind;
    let flag = Arc::new(AtomicBool::new(false));
    *CANCEL.lock().unwrap() = Some(flag.clone());
    let spawned = std::thread::Builder::new().name("kybernow-auto".into()).spawn(move || {
        // Le réseau du moment, gardé avec le résultat : « Ethernet », le nom du Wi-Fi.
        let device = Device { network: system::network_name(kind), ..device };
        log::info!("profil auto : test de {} ({:?}, {:?}{})", h.name, device.screen, device.network, if relay { ", par le relais" } else { "" });
        let mut decoder = decode;
        let tx2 = tx.clone();
        let mut progress = |p: &Progress| send(&tx2, AutoEvent::Progress { attempt, steps: steps(p) });
        let result = autotest::run(console, &device, &mut decoder, &mut progress, &flag, lang);
        // Le décodeur est rendu après chaque extrait ; une dernière fois, au cas où.
        ndl::unload();
        let result = match result {
            Ok(r) => {
                log::info!("profil auto : {} {}×{} à {} Hz, {} Mbit/s, {}", r.codec.label(), r.width, r.height, r.refresh, r.mbps, if r.reliable { "fiable" } else { "rapide" });
                let choices = r.choices(lang).into_iter().map(|c| Choice { title: c.title, value: c.value, reason: c.reason }).collect();
                serde_json::to_value(&r).map(|v| (v, choices)).map_err(|e| e.to_string())
            }
            Err(autotest::AutoError::Cancelled) => {
                log::info!("profil auto : test arrêté");
                return;
            }
            Err(e) => {
                log::warn!("profil auto : {e:?}");
                Err(e.text(lang, &h.name))
            }
        };
        let mut g = CANCEL.lock().unwrap();
        if g.as_ref().is_some_and(|c| Arc::ptr_eq(c, &flag)) {
            *g = None;
        }
        drop(g);
        send(&tx, AutoEvent::Done { attempt, result });
    });
    if spawned.is_err() {
        log::warn!("profil auto : fil du test non lancé");
    }
}
