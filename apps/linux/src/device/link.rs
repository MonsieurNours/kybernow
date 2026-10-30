//! Les liens `kybernow://` (docs/pret.md, docs/kybernow-share.md) : le fichier .desktop déclare l'app pour
//! `x-scheme-handler/kybernow` (`Exec=kybernow %u`), le navigateur ou le gestionnaire de fichiers la lance avec le
//! lien. Un lien d'invitation (`kybernow://borrow?…`) ouvre l'ajout « Avec un compte d'ami », un lien de partage
//! (`kybernow://join?…`) « Rejoindre un partage ».
//!
//! Une seule app reçoit les liens : la première ouverte écoute sur une prise Unix (`$XDG_RUNTIME_DIR/kybernow-link.sock`
//! ; sous Flatpak, dans `$XDG_RUNTIME_DIR/app/$FLATPAK_ID`, commun à ses instances), et une app lancée avec un lien
//! alors qu'une autre est déjà ouverte le lui transmet, puis se ferme aussitôt. Lancée sans lien, une app s'ouvre
//! comme avant, même s'il y en a déjà une.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use kybernow_core::{lend, share};

use super::Event;
use crate::app::{InviteUi, JoinLinkUi, LinkUi};

/// Le lien passé à l'app (`kybernow://…`, n'importe où dans les arguments) ; `None` sans lien.
pub fn from_args(args: &[String]) -> Option<String> {
    args.iter().skip(1).find(|a| a.trim().to_ascii_lowercase().starts_with("kybernow://")).map(|a| a.trim().to_string())
}

/// Ce que l'app fait d'un lien : une invitation, un partage, ou une invitation illisible ; `None` pour le reste
/// (lien inconnu, partage illisible : rien ne s'ouvre).
pub fn parse(url: &str) -> Option<LinkUi> {
    if let Some(i) = lend::parse_invite(url) {
        return Some(LinkUi::Invite(InviteUi { address: i.host, port: i.port, fingerprint: i.fingerprint, login: i.login, relay: i.relay, sid: i.sid, name: i.name }));
    }
    if url.trim().to_ascii_lowercase().starts_with("kybernow://borrow") {
        return Some(LinkUi::BadInvite);
    }
    let l = share::parse_link(url)?;
    Some(LinkUi::Join(JoinLinkUi { address: l.host, port: l.port, fingerprint: l.fingerprint, code: l.code, relay: l.relay, sid: l.sid }))
}

/// Cette app a ouvert la prise (et la retire en se fermant).
static OWNED: AtomicBool = AtomicBool::new(false);

/// La prise où l'app ouverte attend les liens. Sous Flatpak, chaque app a son propre `$XDG_RUNTIME_DIR` : le dossier
/// commun à toutes ses instances est `$XDG_RUNTIME_DIR/app/$FLATPAK_ID`.
fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()).map(PathBuf::from);
    let flatpak = std::env::var_os("FLATPAK_ID").filter(|v| !v.is_empty());
    match (runtime, flatpak) {
        (Some(dir), Some(id)) => dir.join("app").join(id).join("kybernow-link.sock"),
        (Some(dir), None) => dir.join("kybernow-link.sock"),
        (None, _) => crate::model::Store::dir().join("link.sock"),
    }
}

/// Transmet le lien à l'app déjà ouverte : vrai si elle l'a reçu (cette app-ci n'a plus qu'à se fermer).
pub fn forward(url: &str) -> bool {
    let Ok(mut s) = UnixStream::connect(socket_path()) else { return false };
    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
    s.write_all(format!("{}\n", url.replace('\n', "")).as_bytes()).and_then(|_| s.flush()).is_ok()
}

/// L'app ouverte écoute : chaque lien reçu arrive en `Event::Link`. Une prise laissée par une app qui s'est mal
/// fermée (personne n'y répond) est remplacée.
pub fn listen(tx: Sender<Event>) {
    let path = socket_path();
    if UnixStream::connect(&path).is_err() {
        let _ = std::fs::remove_file(&path);
    }
    let listener = match UnixListener::bind(&path) {
        Ok(l) => {
            OWNED.store(true, Ordering::Relaxed);
            l
        }
        Err(e) => {
            log::warn!("liens : la prise {} ne s'ouvre pas ({e}) ; les liens ouvrent une autre app", path.display());
            return;
        }
    };
    std::thread::Builder::new()
        .name("links".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new(stream).take(8192).read_line(&mut line).is_err() {
                    continue;
                }
                let url = line.trim();
                match parse(url) {
                    Some(link) => {
                        log::info!("liens : lien reçu de l'app lancée à nouveau");
                        if tx.send(Event::Link(link)).is_err() {
                            return;
                        }
                    }
                    None if !url.is_empty() => log::info!("liens : lien inconnu, ignoré"),
                    None => {}
                }
            }
        })
        .ok();
}

/// La prise est retirée quand l'app qui l'a ouverte se ferme.
pub fn close() {
    if OWNED.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(socket_path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_from_the_command_line() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(from_args(&args(&["kybernow", "--ui", "tv"])), None);
        assert_eq!(from_args(&args(&["kybernow", "KyberNow://borrow?host=h&port=1&login=lea"])).as_deref(), Some("KyberNow://borrow?host=h&port=1&login=lea"));
        match parse("kybernow://borrow?host=192.168.1.20&port=24811&fp=sha256%3Aaa&login=lea&name=Ma%C3%ABl&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz") {
            Some(LinkUi::Invite(i)) => {
                assert_eq!((i.address.as_str(), i.port, i.login.as_str(), i.name.as_deref()), ("192.168.1.20", 24811, "lea", Some("Maël")));
                assert!(i.relay.is_some() && i.sid.is_some());
            }
            other => panic!("invitation attendue : {other:?}"),
        }
        assert_eq!(parse("kybernow://borrow?host=h&port=1"), Some(LinkUi::BadInvite), "sans identifiant : illisible");
        match parse("kybernow://join?host=192.168.1.20&port=24811&fp=sha256%3Aaa&code=482917") {
            Some(LinkUi::Join(j)) => assert_eq!((j.address.as_str(), j.code.as_str()), ("192.168.1.20", "482917")),
            other => panic!("partage attendu : {other:?}"),
        }
        assert_eq!(parse("kybernow://ailleurs"), None);
    }
}
