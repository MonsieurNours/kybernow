//! L'aide root qui prend l'entrée physique d'une Station Linux pendant un
//! prêt (`kyber-admin input serve`, lancée par systemd pour chaque connexion
//! sur `kybernow-station-input.socket`). Protocole et appareils :
//! [`crate::linux_input`].
//!
//! Elle ne croit rien de ce que dit l'agent : le compte de l'autre bout du
//! socket (`SO_PEERCRED`) doit être celui de la Station (le propriétaire
//! noté par le service des mises à jour, s'il l'est ; le socket lui-même
//! n'est ouvert qu'à ce compte et au groupe `kybernow-station`). Elle ne
//! renvoie qu'Entrée et Échap, jamais le reste de ce qui est tapé, et rend
//! tout dès que la connexion se ferme ou se tait.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::time::Instant;

use crate::linux_input::{AgentLine, KEEPALIVE};

/// Le compte de la Station, d'après le propriétaire noté par le service
/// root des mises à jour (`/var/lib/kybernow-station/update/proprietaire`) :
/// `owner` lu dans ce fichier, `None` s'il n'existe pas encore (le socket,
/// réservé au propriétaire et au groupe `kybernow-station`, suffit alors).
pub fn allowed(uid: u32, owner: Option<&str>) -> Result<(), String> {
    if uid == 0 {
        return Ok(());
    }
    match owner.map(|o| o.trim().parse::<u32>()) {
        None => Ok(()),
        Some(Ok(o)) if o == uid => Ok(()),
        Some(Ok(o)) => Err(format!("seul le compte de KyberNow Station sur cet ordinateur (uid {o}) peut prendre l'entrée")),
        Some(Err(_)) => Err("propriétaire de la Station illisible".into()),
    }
}

/// Ce que l'aide fait d'une ligne de l'agent : `true` pour continuer.
pub fn keep_going(line: &str, last: &mut Instant) -> bool {
    match AgentLine::parse(line) {
        Some(AgentLine::Ping) => {
            *last = Instant::now();
            true
        }
        Some(AgentLine::Release) => false,
        // Une ligne inconnue (agent plus récent) compte comme signe de vie.
        None => {
            *last = Instant::now();
            true
        }
    }
}

/// Les lignes complètes reçues de l'agent : `false` s'il rend l'entrée.
pub fn lines(pending: &mut String, last: &mut Instant) -> bool {
    let mut going = true;
    while let Some(at) = pending.find('\n') {
        let line: String = pending.drain(..=at).collect();
        going &= keep_going(&line, last);
    }
    going
}

/// La prise a-t-elle trop attendu un signe de vie ?
pub fn silent(last: Instant, now: Instant) -> bool {
    now.duration_since(last) > KEEPALIVE
}

/// `kyber-admin input serve` : la connexion est l'entrée standard (`Accept=yes`).
#[cfg(target_os = "linux")]
pub fn serve() -> anyhow::Result<()> {
    use std::io::{BufRead, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::net::UnixStream;

    use std::time::Duration;

    use anyhow::{Context, bail};

    use crate::linux_input::{Event, Grabber, Request};

    let stream = unsafe { UnixStream::from_raw_fd(0) };
    let uid = peer_uid(&stream)?;
    let mut out = stream.try_clone()?;
    out.set_write_timeout(Some(Duration::from_secs(5)))?;
    let send = |out: &mut UnixStream, e: Event| -> std::io::Result<()> {
        writeln!(out, "{}", e.line())?;
        out.flush()
    };
    let owner_file = std::path::Path::new(crate::update::linux::STATE_DIR).join(crate::update::linux::OWNER_FILE);
    let owner = std::fs::read_to_string(&owner_file).ok();
    if let Err(why) = allowed(uid, owner.as_deref()) {
        eprintln!("entrée : demande refusée (uid {uid}) : {why}");
        let _ = send(&mut out, Event::Error(why.clone()));
        bail!(why);
    }
    // Une connexion muette ne garde pas l'aide.
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = std::io::BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    Read::take(&mut reader, 256).read_line(&mut first).context("demande")?;
    let Some(request) = Request::parse(&first) else {
        let _ = send(&mut out, Event::Error("demande inconnue".into()));
        bail!("demande inconnue : {}", first.trim());
    };
    if request == Request::Probe {
        send(&mut out, Event::Ok(crate::linux_input::openable()))?;
        return Ok(());
    }

    let mut log = |line: String| eprintln!("{line}");
    let mut grabber = Grabber::new();
    grabber.rescan(&mut log);
    eprintln!("entrée de l'ordinateur prise pour l'uid {uid} : {} appareil(s)", grabber.count());
    send(&mut out, Event::Ok(grabber.count()))?;
    let mut last = Instant::now();
    let mut devices = grabber.count();
    // Ce que l'agent a déjà envoyé après sa demande.
    let mut pending = String::from_utf8_lossy(reader.buffer()).to_string();
    let end = loop {
        let (keys, readable) = grabber.poll(Some(stream.as_raw_fd()), 500);
        if keys.into_iter().any(|key| send(&mut out, Event::Key(key)).is_err()) {
            break "connexion rompue";
        }
        if readable {
            // `poll` l'a dit lisible : la lecture rend tout de suite (données ou fin).
            let mut buf = [0u8; 256];
            match (&stream).read(&mut buf) {
                Ok(0) => break "connexion fermée par l'agent",
                Ok(n) => pending.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted) => {}
                Err(_) => break "connexion rompue",
            }
        }
        if pending.len() > 4096 {
            break "agent trop bavard";
        }
        if !lines(&mut pending, &mut last) {
            break "rendue à la demande de l'agent";
        }
        if silent(last, Instant::now()) {
            break "agent muet";
        }
        grabber.rescan(&mut log);
        if grabber.count() != devices {
            devices = grabber.count();
            if send(&mut out, Event::Devices(devices)).is_err() {
                break "connexion rompue";
            }
        }
    };
    drop(grabber);
    eprintln!("entrée de l'ordinateur rendue ({end})");
    Ok(())
}

/// Compte de l'autre bout du socket.
#[cfg(target_os = "linux")]
fn peer_uid(stream: &std::os::unix::net::UnixStream) -> anyhow::Result<u32> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe { libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, (&raw mut cred).cast(), &mut len) };
    if rc != 0 {
        anyhow::bail!("SO_PEERCRED : {}", std::io::Error::last_os_error());
    }
    Ok(cred.uid)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn seul_le_compte_de_la_station() {
        assert!(allowed(1000, Some("1000\n")).is_ok());
        assert!(allowed(1001, Some("1000")).is_err());
        assert!(allowed(0, Some("1000")).is_ok(), "root");
        assert!(allowed(1001, None).is_ok(), "pas de propriétaire noté : le socket décide");
        assert!(allowed(1000, Some("x")).is_err());
    }

    #[test]
    fn signe_de_vie_et_fin() {
        let start = Instant::now() - Duration::from_secs(60);
        let mut last = start;
        assert!(keep_going("ping\n", &mut last));
        assert!(last > start);
        assert!(!keep_going("release\n", &mut last));
        let mut pending = String::from("ping\npi");
        assert!(lines(&mut pending, &mut last));
        assert_eq!(pending, "pi", "ligne incomplète gardée");
        pending.push_str("ng\nrelease\n");
        assert!(!lines(&mut pending, &mut last));
        assert!(pending.is_empty());
        assert!(!silent(last, last + KEEPALIVE));
        assert!(silent(last, last + KEEPALIVE + Duration::from_millis(1)));
    }
}
