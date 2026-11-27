//! Mise à jour des Stations proposée par l'app, sans passer par les réglages
//! (docs/link-ui.md « Mise à jour des Stations ») : le bandeau de l'accueil et
//! la puce « · Mise à jour » (`offer`, dans chaque ordinateur de l'accueil),
//! la vérification demandée à la Station (`check`, `POST
//! /api/stream/station/check`) et l'avis au premier lancement après une mise à
//! jour de l'app (`notice_after_update`, puis `take_station_notice`). Le calcul
//! est celui du cœur (`kybernow_core::station_offer`) ; « Mettre à jour » passe
//! par `station_update`, le même flux que la rubrique Station des réglages.
//!
//! Gardé par ordinateur dans `computers.json` (`Computer::station_offer` :
//! « Plus tard », dernière vérification) ; la dernière version lancée de l'app
//! dans `app.json` (`Prefs::last_launched`).

use crate::sync::Locked;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kybernow_core::station_offer::{self, Banner, Context, NOTICE_WAIT_SECS};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{Live, UiState, account, console_for, emit_computers};
use crate::store::Computer;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Secondes Unix.
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Cet appareil peut mettre la Station à jour : un ordinateur appairé (ou
/// ajouté par code, par une invitation : un compte Kyber sur ce PC), ou un
/// ordinateur d'un compte KyberNow avec le droit `admin`.
fn may_update(c: &Computer) -> bool {
    c.account.as_ref().is_none_or(|a| a.can("admin"))
}

/// Ce que l'app sait de l'ordinateur pour le calcul du cœur. `in_session` :
/// une session de l'app est ouverte (avec cet ordinateur ou un autre).
fn context(c: &Computer, l: &Live, in_session: bool, now: u64) -> Context {
    Context {
        station: l.info.as_ref().and_then(|i| i.station.clone()),
        may_update: may_update(c),
        online: l.online == Some(true),
        away: l.away,
        in_session,
        now,
    }
}

/// Le bandeau de l'accueil (ordinateur choisi) et la puce « · Mise à jour »
/// (rangée des ordinateurs), tels que le cœur les calcule.
#[derive(Debug, Clone, Serialize)]
pub struct Offer {
    /// `{ kind: none | update | installing | setup | away, version? }`.
    banner: Banner,
    chip: bool,
}

/// [`Offer`] de l'ordinateur `c` (vue de l'accueil).
pub fn offer(c: &Computer, l: &Live, in_session: bool) -> Offer {
    let ctx = context(c, l, in_session, now());
    Offer { banner: station_offer::banner(&ctx, &c.station_offer), chip: station_offer::chip(&ctx) }
}

/// Demande à la Station de `c` (sur le réseau : `c` déjà situé) de vérifier
/// ses mises à jour, sur un fil à part, quand le cœur le veut
/// (`station_offer::should_check` : l'app a changé de version, ou la Station
/// est plus ancienne qu'elle ; une fois par jour au plus ; jamais en
/// session). La réponse devient son annonce, sur l'accueil. Rend vrai si une
/// vérification part.
pub fn check(app: &AppHandle, c: &Computer) -> bool {
    let state = app.state::<UiState>();
    let in_session = !state.session_ids().is_empty();
    let now = now();
    let ctx = {
        let live = state.live.locked();
        let Some(l) = live.get(&c.id) else { return false };
        // Seulement sur le réseau : par le relais, l'annonce de la Station suffit.
        if l.away || l.info.is_none() {
            return false;
        }
        context(c, l, in_session, now)
    };
    // La vérification est notée avant de partir : une seule à la fois, même
    // si le relevé suivant arrive avant sa réponse.
    let go = state.with_store(|s| {
        let Some(m) = s.get_mut(&c.id) else { return false };
        if !station_offer::should_check(&ctx, &m.station_offer, APP_VERSION) {
            return false;
        }
        m.station_offer.checked(APP_VERSION, now);
        s.save();
        true
    });
    if !go {
        return false;
    }
    state.checking.locked().insert(c.id.clone());
    let app = app.clone();
    let c = c.clone();
    std::thread::spawn(move || {
        let state = app.state::<UiState>();
        match console_for(&c).station_check() {
            Ok(Some(station)) => {
                log::info!(
                    "Station de {} vérifiée : {}",
                    c.name,
                    station.update.version.as_deref().filter(|_| station.update.available).unwrap_or("à jour")
                );
                let changed = {
                    let mut live = state.live.locked();
                    match live.get_mut(&c.id).and_then(|l| l.info.as_mut()) {
                        Some(info) if info.station.as_ref() != Some(&station) => {
                            info.station = Some(station);
                            true
                        }
                        _ => false,
                    }
                };
                if changed {
                    emit_computers(&app);
                }
            }
            // Station d'avant la 0.11.3 : on se fie à son annonce.
            Ok(None) => log::info!("Station de {} : pas de vérification à la demande", c.name),
            Err(e) => log::warn!("Station de {} : vérification refusée ({e})", c.name),
        }
        state.checking.locked().remove(&c.id);
    });
    true
}

/// Une Station de l'avis après une mise à jour de l'app.
#[derive(Debug, Clone, Serialize)]
pub struct NoticeStation {
    id: String,
    name: String,
    version: String,
}

/// L'avis après une mise à jour de l'app : sa version, et les Stations que
/// l'app peut mettre à jour maintenant.
#[derive(Debug, Clone, Serialize)]
pub struct StationNotice {
    app: String,
    stations: Vec<NoticeStation>,
}

/// Au lancement : la version lancée est gardée, et rend vrai au premier
/// lancement après une mise à jour de l'app (jamais au tout premier).
/// Sans version gardée mais avec des ordinateurs connus, l'app vient d'une
/// version d'avant l'avis (`station_offer::BEFORE_NOTICE`) : c'est une mise à jour.
pub fn launched(prefs: &mut super::Prefs) -> bool {
    let last = prefs.last_launched.clone().or_else(|| {
        let store = crate::store::Store::load();
        (!store.computers.is_empty() || !store.account_computers.is_empty()).then(|| station_offer::BEFORE_NOTICE.to_string())
    });
    let updated = station_offer::app_updated(last.as_deref(), APP_VERSION);
    if prefs.last_launched.as_deref() != Some(APP_VERSION) {
        prefs.last_launched = Some(APP_VERSION.to_string());
        super::write_prefs(prefs);
    }
    updated
}

/// Premier lancement après une mise à jour de l'app (sur un fil) : le premier
/// relevé des ordinateurs (15 s au plus), puis les ordinateurs en ligne sur le
/// réseau vérifient (`NOTICE_WAIT_SECS` au plus) ; s'il reste au moins une
/// Station que l'app peut mettre à jour (`station_offer::notice_version`),
/// l'avis attend l'interface (`station-notice`, puis `take_station_notice`),
/// qui le montre une fois revenue à l'accueil (jamais en session).
pub fn notice_after_update(app: AppHandle) {
    let state = app.state::<UiState>();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        let ids: Vec<String> = state.with_store(|s| s.computers.iter().map(|c| c.id.clone()).collect());
        let known = {
            let live = state.live.locked();
            ids.iter().all(|id| live.get(id).is_some_and(|l| l.online.is_some()))
        };
        if known {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let computers = state.with_store(|s| s.computers.clone());
    for c in &computers {
        let located = account::locate(&app, c);
        check(&app, located.as_ref().unwrap_or(c));
    }
    let waiting = Instant::now();
    while waiting.elapsed() < Duration::from_secs(NOTICE_WAIT_SECS) && !state.checking.locked().is_empty() {
        std::thread::sleep(Duration::from_millis(250));
    }
    let now = now();
    let stations: Vec<NoticeStation> = {
        let computers = state.with_store(|s| s.computers.clone());
        let live = state.live.locked();
        computers
            .iter()
            .filter_map(|c| {
                // Hors session : l'avis attend de toute façon le retour à l'accueil.
                let version = station_offer::notice_version(&context(c, live.get(&c.id)?, false, now))?;
                Some(NoticeStation { id: c.id.clone(), name: c.name.clone(), version })
            })
            .collect()
    };
    if stations.is_empty() {
        log::info!("app mise à jour en {APP_VERSION} : aucune Station à mettre à jour");
        return;
    }
    log::info!("app mise à jour en {APP_VERSION} : avis pour {} Station(s)", stations.len());
    *state.station_notice.locked() = Some(StationNotice { app: APP_VERSION.to_string(), stations });
    let _ = app.emit_to("main", "station-notice", ());
}

/// L'avis après une mise à jour de l'app, s'il attend (une seule fois).
#[tauri::command]
pub fn take_station_notice(state: tauri::State<'_, UiState>) -> Option<StationNotice> {
    state.station_notice.locked().take()
}

/// « Plus tard » (bandeau, ou avis pour chacune de ses Stations) : le bandeau
/// de cet ordinateur se cache pour cette version, 7 jours au plus ; la puce
/// reste.
#[tauri::command(async)]
pub fn station_later(app: AppHandle, id: String, version: String) {
    let state = app.state::<UiState>();
    let now = now();
    state.with_store(|s| {
        if let Some(m) = s.get_mut(&id) {
            m.station_offer.snooze(&version, now);
            s.save();
        }
    });
    emit_computers(&app);
}
