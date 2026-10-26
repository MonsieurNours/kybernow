//! Ce que l'app suit après une demande, quelle que soit la fenêtre qui l'a
//! faite (accueil, réglages, menu de l'icône) : un ordinateur qu'on réveille,
//! une Station qui redémarre ou se met à jour. L'app interroge l'ordinateur
//! toutes les 2 s le temps du suivi (`ui.rs`) ; ces règles disent quand il
//! s'arrête et ce qu'on en dit (docs/link-ui.md, rubrique Station et
//! notifications). Sans rien de Windows : testé sur le Mac.

use std::time::Duration;

/// Intervalle des relevés pendant un suivi.
pub const POLL: Duration = Duration::from_secs(2);
/// Un réveil attend l'ordinateur 2 min au plus.
pub const WAKE_LIMIT: Duration = Duration::from_secs(120);
/// Sans l'avoir vue partir, on attend un peu avant de conclure : l'état
/// d'avant l'appel peut encore arriver juste après lui.
const SETTLE: Duration = Duration::from_secs(6);
/// La Station tarde à revenir : 2 min pour un redémarrage, 5 min pour une
/// mise à jour (téléchargement et installation).
const RESTART_LIMIT: Duration = Duration::from_secs(120);
const UPDATE_LIMIT: Duration = Duration::from_secs(300);

/// Un réveil en cours : où il en est.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    /// Toujours hors ligne : on continue.
    Waiting,
    /// L'ordinateur répond : « {nom} est en ligne ».
    Online,
    /// Rien au bout de 2 min : « {nom} ne répond toujours pas ».
    NoAnswer,
}

/// `online` : le dernier relevé ; `waited` : depuis l'envoi du paquet.
pub fn wake(online: Option<bool>, waited: Duration) -> Wake {
    if online == Some(true) {
        Wake::Online
    } else if waited >= WAKE_LIMIT {
        Wake::NoAnswer
    } else {
        Wake::Waiting
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Restart,
    Update,
}

impl Kind {
    /// Nom donné à l'interface (`station_pending`).
    pub fn code(self) -> &'static str {
        match self {
            Kind::Restart => "restart",
            Kind::Update => "update",
        }
    }
}

/// Un redémarrage ou une mise à jour accepté par Admin, en attente du retour
/// de la Station.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub kind: Kind,
    /// Version attendue après une mise à jour.
    pub version: Option<String>,
    /// La Station a été vue hors ligne depuis l'appel.
    pub down: bool,
    /// Lancé depuis le menu de l'icône : une notification à la fin.
    pub from_menu: bool,
}

/// La Station telle que le dernier relevé la montre (`info.station`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen<'a> {
    pub version: &'a str,
    pub restarting: bool,
    pub installing: bool,
}

/// La fin d'un suivi, avec les textes de la rubrique Station.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// « La Station a redémarré. »
    Restarted,
    /// « KyberNow Station {v} est en place. »
    Updated(String),
    /// « La mise à jour n'a pas abouti… » : revenue sur l'ancienne version.
    UpdateFailed,
    /// « La Station tarde à revenir… »
    SlowBack,
    /// Revenue sans être partie : le redémarrage n'a pas eu lieu, on ne dit rien.
    Silent,
}

impl Outcome {
    /// Nom donné à l'interface (événement `station-outcome`).
    pub fn code(&self) -> &'static str {
        match self {
            Outcome::Restarted => "restarted",
            Outcome::Updated(_) => "updated",
            Outcome::UpdateFailed => "update_failed",
            Outcome::SlowBack => "slow_back",
            Outcome::Silent => "silent",
        }
    }
}

/// Un relevé de plus pendant le suivi : `Some` quand il se termine. Les
/// règles de la rubrique Station, celles qu'elle suivait elle-même jusqu'à
/// la 0.5.2.
pub fn station(p: &mut Pending, online: Option<bool>, station: Option<Seen>, waited: Duration) -> Option<Outcome> {
    if online == Some(false) && !p.down {
        p.down = true;
        return None;
    }
    let settled = p.down || waited > SETTLE;
    if let (Some(true), Some(s)) = (online, station) {
        match p.kind {
            Kind::Restart if !s.restarting && settled => {
                return Some(if p.down { Outcome::Restarted } else { Outcome::Silent });
            }
            Kind::Update if !s.installing => {
                if let Some(v) = p.version.as_deref().filter(|v| *v == s.version) {
                    return Some(Outcome::Updated(v.to_string()));
                }
                if settled {
                    return Some(Outcome::UpdateFailed);
                }
            }
            _ => {}
        }
    }
    let limit = if p.kind == Kind::Restart { RESTART_LIMIT } else { UPDATE_LIMIT };
    (waited > limit).then_some(Outcome::SlowBack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn reveil() {
        assert_eq!(wake(Some(false), secs(10)), Wake::Waiting);
        assert_eq!(wake(None, secs(10)), Wake::Waiting);
        assert_eq!(wake(Some(true), secs(10)), Wake::Online);
        assert_eq!(wake(Some(false), secs(120)), Wake::NoAnswer);
    }

    fn pending(kind: Kind, version: Option<&str>) -> Pending {
        Pending { kind, version: version.map(Into::into), down: false, from_menu: true }
    }

    fn seen(version: &str, restarting: bool, installing: bool) -> Option<Seen<'_>> {
        Some(Seen { version, restarting, installing })
    }

    #[test]
    fn redemarrage() {
        let mut p = pending(Kind::Restart, None);
        // Juste après l'appel : la Station dit encore « redémarrage ».
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", true, false), secs(1)), None);
        // Partie…
        assert_eq!(station(&mut p, Some(false), None, secs(4)), None);
        assert!(p.down);
        // … revenue.
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, false), secs(30)), Some(Outcome::Restarted));
    }

    #[test]
    fn redemarrage_sans_depart() {
        let mut p = pending(Kind::Restart, None);
        // Pas encore réglé : on attend.
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, false), secs(2)), None);
        // Revenue sans être partie : on ne prétend rien.
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, false), secs(8)), Some(Outcome::Silent));
    }

    #[test]
    fn redemarrage_qui_tarde() {
        let mut p = pending(Kind::Restart, None);
        assert_eq!(station(&mut p, Some(false), None, secs(2)), None);
        assert_eq!(station(&mut p, Some(false), None, secs(100)), None);
        assert_eq!(station(&mut p, Some(false), None, secs(121)), Some(Outcome::SlowBack));
    }

    #[test]
    fn mise_a_jour() {
        let mut p = pending(Kind::Update, Some("0.5.3"));
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, true), secs(2)), None);
        assert_eq!(station(&mut p, Some(false), None, secs(40)), None);
        assert_eq!(station(&mut p, Some(true), seen("0.5.3", false, false), secs(70)), Some(Outcome::Updated("0.5.3".into())));
    }

    #[test]
    fn mise_a_jour_qui_echoue() {
        let mut p = pending(Kind::Update, Some("0.5.3"));
        assert_eq!(station(&mut p, Some(false), None, secs(40)), None);
        // Revenue sur l'ancienne version, plus d'installation en cours.
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, false), secs(90)), Some(Outcome::UpdateFailed));
        // Une mise à jour peut durer plus longtemps qu'un redémarrage.
        let mut p = pending(Kind::Update, Some("0.5.3"));
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, true), secs(200)), None);
        assert_eq!(station(&mut p, Some(true), seen("0.5.2", false, true), secs(301)), Some(Outcome::SlowBack));
    }
}
