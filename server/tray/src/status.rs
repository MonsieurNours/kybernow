//! Ce que l'icône montre de la Station (Windows et Linux) : son état, la
//! ligne de la session, l'apparence de l'icône.

use crate::console::LocalState;
use crate::i18n::{lang, t, tn};
use crate::lend::host as lend;

/// État d'un service (Windows) ou d'une unité systemd de l'utilisateur (Linux).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Running,
    Stopped,
    Starting,
    Stopping,
    Missing,
    Unknown,
}

impl State {
    pub fn is_up(self) -> bool {
        matches!(self, State::Running | State::Starting)
    }
}

/// Après un redémarrage de la Station confié à Admin (STWIN-45) : son
/// assistant arrête Admin puis Kyber et les relance. Attend qu'Admin se soit
/// arrêté (20 s au plus : un assistant qui n'a rien fait ne bloque pas), puis
/// que les deux tournent de nouveau ; un service arrêté en route n'est pas
/// une erreur, seulement le délai dépassé.
pub fn after_restart(state: &dyn Fn(&str) -> State, admin: &str, kyber: &str) -> anyhow::Result<()> {
    use std::time::{Duration, Instant};
    let until = |name: &str, done: &dyn Fn(State) -> bool, timeout: Duration| -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if done(state(name)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        false
    };
    until(admin, &|s| s != State::Running, Duration::from_secs(20));
    for name in [kyber, admin] {
        if !until(name, &|s| s == State::Running, Duration::from_secs(90)) {
            anyhow::bail!(t("tray.errors.serviceTimeout", &[("service", &name), ("seconds", &90)]));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Station en marche, aucune session.
    Running,
    /// Un appareil est connecté.
    Session,
    /// Démarrage, arrêt ou redémarrage en cours.
    Busy,
    /// Station arrêtée : icône grisée.
    Stopped,
}

/// Ce que l'icône sait de l'ordinateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub kyber: State,
    pub admin: State,
    /// `None` : la console ne répond pas.
    pub console: Option<LocalState>,
    pub start_with_computer: Option<bool>,
}

/// Textes et apparence pour un état donné.
pub fn describe(s: &Snapshot, busy: Option<&str>) -> (String, String, Look) {
    let status = match (busy, s.kyber) {
        (Some(key), _) => t(key, &[]),
        (_, State::Running) => t("tray.status.running", &[]),
        (_, State::Starting) => t("tray.status.starting", &[]),
        (_, State::Stopping) => t("tray.status.stopping", &[]),
        (_, State::Stopped) => t("tray.status.stopped", &[]),
        (_, State::Missing) => t("tray.status.missing", &[]),
        (_, State::Unknown) => t("tray.status.unknown", &[]),
    };
    let live = s.console.as_ref().and_then(|c| c.live.as_ref()).and_then(|l| l["name"].as_str()).map(str::to_owned);
    let has_live = live.is_some();
    let session = match (s.kyber, s.admin, &s.console) {
        (State::Running, State::Running, Some(_)) if live.is_some() => t("tray.session.connected", &[("name", &live.unwrap_or_default())]),
        (State::Running, State::Running, Some(LocalState { session: true, user: Some(user), .. })) => t("tray.session.userSession", &[("user", user)]),
        (State::Running, State::Running, Some(LocalState { session: true, user: None, .. })) => t("tray.session.session", &[]),
        (State::Running, State::Running, Some(_)) => t("tray.session.none", &[]),
        (State::Running, State::Running, None) => t("tray.session.consoleSilent", &[]),
        (State::Running, _, _) => t("tray.session.consoleStopped", &[]),
        (State::Stopped, _, _) => t("tray.session.cannotConnect", &[]),
        (State::Missing, _, _) => t("tray.session.reinstall", &[]),
        _ => t("tray.session.wait", &[]),
    };
    // Partage ouvert : l'écran partagé depuis le PC, ou les invités de l'appareil
    // hôte ; ses demandes à côté (« 2 invités · 1 demande »), seulement s'il y en a.
    let (guests, requests) = s.console.as_ref().map_or((0, 0), |c| (c.guests(), c.requests()));
    let counts: Vec<String> =
        [(guests > 0).then(|| tn("tray.guests", guests)), (requests > 0).then(|| tn("tray.requests", requests))].into_iter().flatten().collect();
    let session = match s.console.as_ref().and_then(|c| c.share.as_ref()) {
        Some(share) if s.kyber == State::Running && !has_live && share["origin"] == "local" => {
            let base = t("tray.session.sharing", &[("n", &share["display"]["index"])]);
            std::iter::once(base).chain(counts).collect::<Vec<_>>().join(" · ")
        }
        Some(_) if s.kyber == State::Running && !counts.is_empty() => format!("{session} · {}", counts.join(" · ")),
        _ => session,
    };
    // Ordinateur prêté : « Prêté à Léa jusqu'à 18 h », « Prêté à Léa · Reprise dans 1:42 ».
    let lent = s.console.as_ref().and_then(|c| c.lend.as_ref()).filter(|l| lend::active(l));
    let session = match lent {
        Some(l) if s.kyber == State::Running => {
            let until = l["borrower"]["until"].as_i64().and_then(|u| crate::lend::clock_at(lang(), u));
            lend::status_line(lang(), l, until.as_deref(), crate::lend::unix_now()).unwrap_or(session)
        }
        _ => session,
    };
    let in_session = s.console.as_ref().is_some_and(|c| c.device_session() || c.guests() > 0) || lent.is_some();
    let look = match s.kyber {
        _ if busy.is_some() => Look::Busy,
        State::Starting | State::Stopping => Look::Busy,
        State::Running if in_session => Look::Session,
        State::Running => Look::Running,
        _ => Look::Stopped,
    };
    (status, session, look)
}

