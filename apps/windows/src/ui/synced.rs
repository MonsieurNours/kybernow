//! Réglages synchronisés par le compte, côté Windows (docs/comptes.md,
//! « Réglages synchronisés et déplacer un ordinateur » ; apps/core/README.md,
//! « Réglages synchronisés ») : le profil, le format vidéo, le 4:4:4, le débit,
//! le HDR et le mode immersif de chaque ordinateur d'un compte suivent la
//! personne d'un appareil à l'autre.
//!
//! - **L'interrupteur** « Synchroniser les réglages des ordinateurs » (réglages
//!   de l'app, « Comptes », textes du cœur) : `Prefs::sync_settings`, allumé
//!   tant qu'il n'a pas été changé, gardé sur ce PC. Coupé : ni lecture ni
//!   envoi.
//! - **Lire** ([`fetch_all`]) : au lancement et à chaque relevé des comptes
//!   (`account::sync`), et juste avant une session ([`before_session`], qui
//!   n'attend pas plus de 1,5 s). Ce qui attendait le relais part d'abord ;
//!   ensuite, chaque ordinateur changé depuis le dernier `updated_at` appliqué
//!   prend ses valeurs (`synced::apply`), sans les renvoyer.
//! - **Envoyer** ([`changed`]) : quand la personne change un de ces réglages
//!   (réglages de l'ordinateur, profil de l'accueil, de la barre ou de l'icône,
//!   puce « HDR »), seulement les clés changées ; tout la première fois.
//! - Refus : `UnknownComputer` oublie les réglages gardés de l'ordinateur ;
//!   `NoAccounts` (relais d'avant) : rien, sans rien dire ; relais muet : le
//!   changement attend le relevé suivant.
//!
//! Gardé dans `%APPDATA%\KyberNow\synced.json` (`synced::SyncFile`, rien de
//! secret). Les règles se testent sur le Mac (`src/synced.rs`).

use std::sync::mpsc;

use kybernow_core::account::{AccountError, Paired, SyncedValues};

use super::*;
use crate::synced::{self as rules, Local, SyncFile};

/// `synced.json`, lu une fois (`None` : pas encore lu).
static FILE: Mutex<Option<SyncFile>> = Mutex::new(None);
/// Un échange avec le relais à la fois (lecture ou envoi), dans l'ordre.
static BUSY: Mutex<()> = Mutex::new(());
/// Avant une session : au plus ce temps pour lire les réglages du compte.
const BEFORE_SESSION: Duration = Duration::from_millis(1500);

fn path() -> std::path::PathBuf {
    std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow").join("synced.json")
}

fn with_file<T>(f: impl FnOnce(&mut SyncFile) -> T) -> T {
    let mut guard = FILE.lock().unwrap();
    let file = guard.get_or_insert_with(|| std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default());
    let out = f(file);
    if let Ok(text) = serde_json::to_string_pretty(&*file) {
        let p = path();
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
    out
}

/// L'interrupteur des réglages de l'app (allumé tant qu'il n'a pas été changé).
pub fn enabled(app: &AppHandle) -> bool {
    app.state::<UiState>().prefs.lock().unwrap().sync_settings.unwrap_or(true)
}

/// Un compte oublié : ses réglages gardés aussi.
pub fn forget_account(key: &str) {
    with_file(|f| f.accounts.remove(key));
}

pub fn local_of(s: &Settings) -> Local {
    Local {
        profile: s.profile.clone(),
        codec: s.codec.clone(),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        hdr: s.hdr,
        immersive: s.immersive.clone(),
    }
}

fn put_local(s: &mut Settings, l: Local) {
    s.profile = l.profile;
    s.codec = l.codec;
    s.yuv444 = l.yuv444;
    s.mbps = l.mbps;
    s.reliable = l.reliable;
    s.hdr = l.hdr;
    s.immersive = l.immersive;
}

/// Ce que ce PC demanderait pour ces réglages (sans « Sur batterie »).
fn effective(c: &Computer, l: &Local, pc: &ThisPc) -> kybernow_core::profile::StreamSettings {
    let mut c = c.clone();
    put_local(&mut c.settings, l.clone());
    session_plan(&c, pc, false, None).0
}

fn this_pc(app: &AppHandle) -> ThisPc {
    app.state::<UiState>().this_pc.lock().unwrap().clone().unwrap_or_default()
}

/// Le compte et la Station d'un ordinateur de l'accueil : celui par lequel il
/// s'ouvre ; pour un ordinateur appairé, le premier compte qui a la même
/// Station (`account::merge`).
fn station_of(store: &Store, c: &Computer) -> Option<(String, String)> {
    if let Some(ac) = &c.account {
        return Some((ac.account.clone(), ac.station.clone()));
    }
    let paired: Paired = account::paired_of(c);
    let merged = kybernow_core::account::merge(&[paired], &store.account_computers);
    merged.linked.first().map(|(_, ac)| (ac.account.clone(), ac.station.clone()))
}

// MARK: - Lire

/// Lit les réglages de chaque compte et applique ce qui a changé (bloquant :
/// un appel au relais par compte, plus les renvois). Coupé : rien.
pub fn fetch_all(app: &AppHandle) {
    if !enabled(app) {
        return;
    }
    let _busy = BUSY.lock().unwrap();
    for a in account::list() {
        fetch_one(app, &a);
    }
}

fn fetch_one(app: &AppHandle, a: &kybernow_core::account::Account) {
    let key = a.key();
    let mut list = match kybernow_core::account::fetch_settings(a) {
        Ok(list) => list,
        Err(AccountError::NoAccounts) => return,
        Err(e) => {
            log::info!("réglages synchronisés : {key} pas lus ({})", e.kind());
            return;
        }
    };
    // Ce qui attendait le relais part d'abord.
    let pending: Vec<(String, SyncedValues)> = with_file(|f| f.accounts.get(&key).map(|s| s.pending.clone().into_iter().collect()).unwrap_or_default());
    for (station, values) in pending {
        let last = kybernow_core::account::settings::find(&list, &station).cloned();
        match kybernow_core::account::put_settings(a, &station, last.as_ref(), &values) {
            Ok(s) => {
                list.retain(|x| x.station != station);
                list.push(s.clone());
                with_file(|f| f.accounts.entry(key.clone()).or_default().sent(s));
            }
            Err(AccountError::UnknownComputer) => with_file(|f| f.accounts.entry(key.clone()).or_default().forget(&station)),
            Err(e) => log::info!("réglages synchronisés : {station} toujours pas envoyés ({})", e.kind()),
        }
    }
    let incoming = with_file(|f| f.accounts.entry(key.clone()).or_default().fetched(list));
    if incoming.is_empty() {
        return;
    }
    let pc = this_pc(app);
    let caps = capabilities(&pc);
    let state = app.state::<UiState>();
    let applied = state.with_store(|s| {
        let targets: Vec<(usize, String)> = s
            .computers
            .iter()
            .enumerate()
            .filter_map(|(i, c)| station_of(s, c).filter(|(k, _)| k == &key).map(|(_, st)| (i, st)))
            .collect();
        let mut n = 0;
        for (i, station) in targets {
            let Some((_, values)) = incoming.iter().find(|(st, _)| st == &station) else { continue };
            let c = s.computers[i].clone();
            let before = local_of(&c.settings);
            let after = rules::apply(&before, values, caps, |l| effective(&c, l, &pc));
            if after != before {
                put_local(&mut s.computers[i].settings, after);
                n += 1;
            }
        }
        if n > 0 {
            s.save();
        }
        n
    });
    if applied > 0 {
        log::info!("réglages synchronisés : {applied} ordinateur(s) mis à jour d'après {key}");
        emit_computers(app);
    }
}

/// Juste avant une session sur un ordinateur d'un compte : les réglages du
/// compte, 1,5 s au plus (sinon la session part avec ceux de ce PC, la
/// lecture finit pour la fois suivante).
pub fn before_session(app: &AppHandle, id: &str) {
    if !enabled(app) {
        return;
    }
    let state = app.state::<UiState>();
    let Some(key) = state.with_store(|s| s.get(id).cloned().and_then(|c| station_of(s, &c))).map(|(k, _)| k) else { return };
    let Some(a) = account::list().into_iter().find(|a| a.key() == key) else { return };
    let (tx, rx) = mpsc::channel();
    let app = app.clone();
    std::thread::spawn(move || {
        {
            let _busy = BUSY.lock().unwrap();
            fetch_one(&app, &a);
        }
        let _ = tx.send(());
    });
    if rx.recv_timeout(BEFORE_SESSION).is_err() {
        log::info!("réglages synchronisés : pas lus à temps, la session part avec ceux de ce PC");
    }
}

// MARK: - Envoyer

/// La personne a changé les réglages de l'ordinateur `id` (`before` : ceux
/// d'avant) : les clés changées partent au relais, sur un fil. Rien pour un
/// ordinateur sans compte, interrupteur coupé, ou rien de synchronisé changé.
pub fn changed(app: &AppHandle, id: &str, before: &Settings) {
    if !enabled(app) {
        return;
    }
    let state = app.state::<UiState>();
    let Some((c, (key, station))) = state.with_store(|s| s.get(id).cloned().and_then(|c| station_of(s, &c).map(|k| (c, k)))) else { return };
    let pc = this_pc(app);
    let (old, new) = (local_of(before), local_of(&c.settings));
    let (old_eff, new_eff) = (effective(&c, &old, &pc), effective(&c, &new, &pc));
    let change = rules::changed(&old, &new, &old_eff, &new_eff);
    if change.is_empty() {
        return;
    }
    let Some(a) = account::list().into_iter().find(|a| a.key() == key) else { return };
    std::thread::spawn(move || {
        let _busy = BUSY.lock().unwrap();
        let (values, last) = with_file(|f| {
            let s = f.accounts.entry(key.clone()).or_default();
            (s.to_send(&station, &change, || rules::everything(&new, &new_eff)), s.last(&station).cloned())
        });
        match kybernow_core::account::put_settings(&a, &station, last.as_ref(), &values) {
            Ok(s) => with_file(|f| f.accounts.entry(key.clone()).or_default().sent(s)),
            Err(AccountError::UnknownComputer) => {
                log::info!("réglages synchronisés : {station} n'est plus au compte");
                with_file(|f| f.accounts.entry(key.clone()).or_default().forget(&station));
            }
            Err(AccountError::NoAccounts) => {}
            Err(e) => {
                log::info!("réglages synchronisés : {station} renvoyé au prochain relevé ({})", e.kind());
                with_file(|f| f.accounts.entry(key.clone()).or_default().unsent(&station, &values));
            }
        }
    });
}

// MARK: - Interrupteur

/// « Synchroniser les réglages des ordinateurs » (réglages de l'app) ; rallumé,
/// les réglages des comptes sont lus tout de suite.
#[tauri::command(async)]
pub fn set_sync_settings(app: AppHandle, on: bool) -> bool {
    let state = app.state::<UiState>();
    let prefs = {
        let mut p = state.prefs.lock().unwrap();
        p.sync_settings = Some(on);
        p.clone()
    };
    save_prefs(&prefs);
    log::info!("réglages synchronisés : {}", if on { "allumés" } else { "coupés" });
    if on {
        std::thread::spawn(move || fetch_all(&app));
    }
    on
}
