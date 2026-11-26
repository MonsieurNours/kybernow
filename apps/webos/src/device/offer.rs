//! Mise à jour des Stations (docs/link-ui.md, « Mise à jour des Stations ») : à chaque tour de boucle, le calcul du
//! cœur (`kybernow_core::station_offer`) pour chaque ordinateur, que l'interface lit (`HostLive::offer`) ; les
//! vérifications demandées aux Stations (`Console::station_check`), sur un fil à part ; l'avis au premier lancement
//! d'une nouvelle version de l'app, après les vérifications des ordinateurs en ligne (10 s au plus), à l'accueil et
//! hors session.

use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use kybernow_core::console::{StationInfo, StationUpdate};
use kybernow_core::station_offer::{self, Banner, Context, Memory, NOTICE_WAIT_SECS};

use super::{jobs, Event};
use crate::app::{Action, App, Online, StationKnown, StationLive};
use crate::model::StationMemory;
use crate::station_offer::{OfferBanner, StationOffer};

/// La version de l'app.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Les ordinateurs dont la vérification est en cours (une à la fois par ordinateur).
static CHECKING: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub struct Offers {
    /// L'avis attendu depuis ce lancement (l'app vient d'être mise à jour) ; `None` : rien à montrer, ou déjà fait.
    notice: Option<Instant>,
}

impl Offers {
    /// Au lancement : la version lancée gardée ; l'avis attendu si c'est le premier lancement d'une nouvelle version
    /// (jamais au tout premier lancement). Un fichier d'avant la 0.11.3 ne garde pas la version lancée : des
    /// ordinateurs déjà là disent que ce n'est pas le tout premier lancement (`station_offer::BEFORE_NOTICE`).
    pub fn start(app: &mut App) -> Self {
        let last = app.store.app.last_launched.clone();
        let last = if last.is_empty() && !app.store.hosts.is_empty() { station_offer::BEFORE_NOTICE.to_string() } else { last };
        let updated = station_offer::app_updated(Some(last.as_str()).filter(|v| !v.is_empty()), APP_VERSION);
        if app.store.app.last_launched != APP_VERSION {
            app.store.app.last_launched = APP_VERSION.into();
            app.store.save();
        }
        if updated {
            log::info!("mise à jour des Stations : l'app est passée de {last} à {APP_VERSION}, avis à proposer");
        }
        Self { notice: updated.then(Instant::now) }
    }

    /// Un tour de boucle : rend les actions (mises à jour de l'avis) et si l'interface a changé.
    pub fn tick(&mut self, app: &mut App, tx: &Sender<Event>) -> (Vec<Action>, bool) {
        let now = crate::model::now_secs();
        let in_session = app.session.is_some();
        let mut changed = false;
        let ids: Vec<String> = app.store.hosts.iter().map(|h| h.id.clone()).collect();
        for id in &ids {
            let ctx = context(app, id, now, in_session);
            let memory = memory(app, id);
            let offer = StationOffer {
                banner: match station_offer::banner(&ctx, &memory) {
                    Banner::None => OfferBanner::None,
                    Banner::Update { version } => OfferBanner::Update(version),
                    Banner::Installing => OfferBanner::Installing,
                    Banner::Setup { version } => OfferBanner::Setup(version),
                    Banner::Away { version } => OfferBanner::Away(version),
                },
                chip: station_offer::chip(&ctx),
                notice_version: station_offer::notice_version(&ctx),
            };
            let live = app.live.entry(id.clone()).or_default();
            if live.offer != offer {
                live.offer = offer;
                changed = true;
            }
            if station_offer::should_check(&ctx, &memory, APP_VERSION) && start_check(id)
                && let Some(h) = app.store.host(id)
            {
                log::info!("{} : vérification des mises à jour de la Station demandée", h.name);
                jobs::station_check(h.clone(), tx.clone());
            }
        }
        // L'avis : les vérifications des ordinateurs en ligne faites (ou 10 s passées), puis l'accueil hors session.
        if let Some(since) = self.notice {
            let checking = !CHECKING.lock().unwrap().is_empty();
            let known = app.store.hosts.iter().all(|h| app.live.get(&h.id).is_some_and(|l| l.online != Online::Unknown));
            let waited = since.elapsed() >= Duration::from_secs(NOTICE_WAIT_SECS);
            if (waited || (known && !checking && since.elapsed() >= Duration::from_secs(1))) && app.can_show_station_notice() {
                self.notice = None;
                let rows: Vec<(String, String, String)> = app
                    .store
                    .hosts
                    .iter()
                    .filter_map(|h| Some((h.id.clone(), h.name.clone(), app.live.get(&h.id)?.offer.notice_version.clone()?)))
                    .collect();
                if !rows.is_empty() {
                    log::info!("mise à jour des Stations : avis pour {} ordinateur(s)", rows.len());
                    app.open_station_notice(rows, APP_VERSION);
                    changed = true;
                }
            }
        }
        let acts = app.offer_tick();
        changed |= !acts.is_empty();
        (acts, changed)
    }
}

/// La vérification de cet ordinateur commence (faux : déjà en cours).
fn start_check(id: &str) -> bool {
    let mut c = CHECKING.lock().unwrap();
    if c.iter().any(|i| i == id) {
        return false;
    }
    c.push(id.to_string());
    true
}

/// La vérification est faite : gardée (`Memory::checked`), et ce que la Station annonce devient la nouvelle annonce.
pub fn checked(app: &mut App, id: &str, station: Option<StationLive>) {
    CHECKING.lock().unwrap().retain(|i| i != id);
    let mut m = memory(app, id);
    m.checked(APP_VERSION, crate::model::now_secs());
    app.store.station_offers.insert(id.to_string(), to_store(&m));
    app.store.save();
    if let Some(s) = station
        && let Some(l) = app.live.get_mut(id)
    {
        l.station = StationKnown::Info(s);
    }
}

/// Ce que l'app sait de l'ordinateur, pour le cœur.
fn context(app: &App, id: &str, now: u64, in_session: bool) -> Context {
    let live = app.live.get(id);
    let online = live.map(|l| l.online).unwrap_or_default();
    Context {
        station: match live.map(|l| &l.station) {
            Some(StationKnown::Info(s)) => Some(station_info(s)),
            _ => None,
        },
        may_update: app.may_update_station(id),
        online: matches!(online, Online::Online | Online::Away),
        away: online == Online::Away,
        in_session,
        now,
    }
}

/// La Station vue par l'interface, rendue au cœur.
fn station_info(s: &StationLive) -> StationInfo {
    StationInfo {
        version: s.version.clone(),
        update: StationUpdate { available: s.update_available, version: s.update_version.clone(), installable: s.installable, requires_setup: s.requires_setup, installing: s.installing },
        restarting: s.restarting,
    }
}

/// La mémoire gardée de cet ordinateur (même JSON que celle du cœur).
fn memory(app: &App, id: &str) -> Memory {
    let m = app.store.station_offers.get(id).cloned().unwrap_or_default();
    Memory { snoozed_version: m.snoozed_version, snoozed_at: m.snoozed_at, checked_app_version: m.checked_app_version, checked_at: m.checked_at }
}

fn to_store(m: &Memory) -> StationMemory {
    StationMemory { snoozed_version: m.snoozed_version.clone(), snoozed_at: m.snoozed_at, checked_app_version: m.checked_app_version.clone(), checked_at: m.checked_at }
}
