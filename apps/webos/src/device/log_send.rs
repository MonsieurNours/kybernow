//! « Envoyer le journal » sur le téléviseur (docs/relais.md, « Dépôt d'un journal ») : le journal de l'app lu
//! (`kybernow.log.1` puis `kybernow.log`), puis l'en-tête, la compression, le chiffrement `KNJ1` et le dépôt sur le
//! relais par le cœur (`kybernow_core::journal` : agent HTTPS du relais, autorités publiques ou empreinte épinglée
//! d'un relais chez soi) ; son état relu et le dépôt retiré à « Terminé ». Hors du fil de l'interface.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use kybernow_core::journal::{self, DepositError, Remote};

use super::Event;
use crate::log_send::{LogDeposit, LogFail, LogRemote};

/// Le journal de l'app, du plus ancien au plus récent : le lancement d'avant (gardé en `.1` au lancement, celui qui
/// a pu se fermer mal), puis celui-ci.
fn read_log() -> String {
    let dir = crate::model::Store::dir();
    let mut body = String::new();
    for name in ["kybernow.log.1", "kybernow.log"] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            body.push_str(&format!("===== {name} =====\n"));
            body.push_str(&String::from_utf8_lossy(&bytes));
            if !body.ends_with('\n') {
                body.push('\n');
            }
        }
    }
    body
}

/// Le refus du cœur dans les termes de l'écran.
fn fail_of(e: &DepositError) -> LogFail {
    match e {
        DepositError::OldRelay => LogFail::OldRelay,
        DepositError::TooMany => LogFail::TooMany,
        DepositError::Unreachable(_) | DepositError::Failed(_) => LogFail::Unreachable,
    }
}

fn remote_of(r: Remote) -> LogRemote {
    match r {
        Remote::Waiting => LogRemote::Waiting,
        Remote::Opened => LogRemote::Opened,
        Remote::Gone => LogRemote::Gone,
        Remote::Unknown => LogRemote::Unknown,
    }
}

/// Prépare, chiffre et dépose le journal ; dit à la boucle quand il part, puis le dépôt ou le refus.
pub fn send(attempt: u32, relay: String, device: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let lang = kybernow_core::i18n::current();
        let header = journal::header_now(lang, env!("CARGO_PKG_VERSION"), &device, &[]);
        let body = read_log();
        let _ = tx.send(Event::LogUploading { attempt });
        super::sdl::wake_main();
        let result = journal::deposit(&relay, &header, &body)
            .map(|d| LogDeposit { relay: d.relay, id: d.id, owner: d.owner, url: d.url, until: Instant::now() + Duration::from_secs(d.expires_in) })
            .map_err(|e| {
                log::warn!("journal : dépôt impossible ({})", e.detail());
                fail_of(&e)
            });
        let _ = tx.send(Event::LogSent { attempt, result });
        super::sdl::wake_main();
    });
}

/// L'état d'un dépôt, relu pour l'écran du QR code.
pub fn check(attempt: u32, relay: String, id: String, owner: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let remote = remote_of(journal::state(&relay, &id, &owner));
        let _ = tx.send(Event::LogState { attempt, remote });
        super::sdl::wake_main();
    });
}

/// « Terminé » : retire un dépôt pas encore ouvert (sans effet sinon).
pub fn discard(relay: String, id: String, owner: String) {
    std::thread::spawn(move || {
        journal::discard(&relay, &id, &owner);
    });
}
