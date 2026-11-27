//! L'entrée physique d'une Station Linux pendant un prêt (docs/pret.md,
//! « Station Linux » ; docs/station-linux.md, « Prêt de l'ordinateur »).
//!
//! Le paquet n'ouvre pas `/dev/input/event*` à l'utilisateur : tout programme
//! de la session pourrait alors lire ce qui est tapé. Les claviers, souris,
//! pavés tactiles et manettes branchés sur l'ordinateur sont donc pris
//! (`EVIOCGRAB`) par une petite aide root, `kyber-admin input serve`, que
//! systemd lance à chaque connexion sur [`SOCKET`]
//! (`kybernow-station-input.socket`, comme les mises à jour), et que seul le
//! compte de la Station peut joindre.
//!
//! L'aide garde elle-même les descripteurs : rien de ce qui est tapé ne sort
//! d'elle, sauf Entrée et Échap (le cache de l'agent de console). La prise
//! dure autant que la connexion : quand l'agent la ferme (fin du prêt), la
//! rompt (agent arrêté, session fermée, panne) ou se tait plus de
//! [`KEEPALIVE`], l'aide s'arrête et le noyau rend l'entrée au compositeur.
//! Admin n'y est pour rien : l'entrée est rendue même s'il ne répond plus.
//!
//! Protocole, une ligne de texte par message :
//!
//! - l'agent commence par `grab` (prendre l'entrée) ou `probe` (l'aide
//!   est-elle là ? rien n'est pris) ;
//! - l'aide répond `ok <n>` (appareils pris, ou qu'elle saurait prendre),
//!   ou `error <texte>` puis ferme ;
//! - pendant une prise : l'aide envoie `key enter`, `key escape`, et
//!   `devices <n>` quand un appareil est branché ou retiré ; l'agent envoie
//!   `ping` toutes les [`PING_EVERY`], et `release` pour rendre l'entrée.
//!
//! Ce fichier est aussi celui de l'agent de console (`server/tray`, inclus
//! par `#[path]`) : la prise directe, quand l'aide n'est pas là (installation
//! dans le dossier de l'utilisateur, sans root), passe par le même code.

// Chacun des deux programmes n'en prend qu'une partie.
#![allow(dead_code, unused_imports)]

use std::time::Duration;

/// Socket de l'aide root (`kybernow-station-input.socket`).
pub const SOCKET: &str = "/run/kybernow-station/input.sock";
/// Sans message de l'agent pendant ce temps, l'aide rend l'entrée.
pub const KEEPALIVE: Duration = Duration::from_secs(20);
/// Signe de vie de l'agent.
pub const PING_EVERY: Duration = Duration::from_secs(5);
/// Un appareil branché en cours de prêt est pris à la relecture suivante.
pub const RESCAN_EVERY: Duration = Duration::from_secs(2);

/// Une touche du cache de l'agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Escape,
}

/// Ce que l'agent demande, en première ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Grab,
    Probe,
}

impl Request {
    pub fn parse(line: &str) -> Option<Request> {
        match line.trim() {
            "grab" => Some(Request::Grab),
            "probe" => Some(Request::Probe),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Request::Grab => "grab",
            Request::Probe => "probe",
        }
    }
}

/// Ce que l'aide envoie à l'agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Réponse à la demande : appareils pris (ou qu'elle saurait prendre).
    Ok(usize),
    /// Demande refusée ; l'aide ferme ensuite.
    Error(String),
    Key(Key),
    /// Nombre d'appareils tenus, après un branchement ou un retrait.
    Devices(usize),
}

impl Event {
    pub fn line(&self) -> String {
        match self {
            Event::Ok(n) => format!("ok {n}"),
            Event::Error(text) => format!("error {}", text.replace(['\n', '\r'], " ")),
            Event::Key(Key::Enter) => "key enter".into(),
            Event::Key(Key::Escape) => "key escape".into(),
            Event::Devices(n) => format!("devices {n}"),
        }
    }

    pub fn parse(line: &str) -> Option<Event> {
        let line = line.trim();
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "ok" => rest.parse().ok().map(Event::Ok),
            "error" => Some(Event::Error(rest.to_string())),
            "key" => match rest {
                "enter" => Some(Event::Key(Key::Enter)),
                "escape" => Some(Event::Key(Key::Escape)),
                _ => None,
            },
            "devices" => rest.parse().ok().map(Event::Devices),
            _ => None,
        }
    }
}

/// Ce que l'agent envoie pendant une prise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentLine {
    Ping,
    Release,
}

impl AgentLine {
    pub fn parse(line: &str) -> Option<AgentLine> {
        match line.trim() {
            "ping" => Some(AgentLine::Ping),
            "release" => Some(AgentLine::Release),
            _ => None,
        }
    }
}

/// Appareils à prendre parmi ceux de `/sys/class/input` (`(nom de l'entrée,
/// chemin résolu, nom de l'appareil)`) : pas ceux de uinput et de uhid
/// (`/devices/virtual/` : Kyber, l'entrée de l'ami, ses manettes), pas les
/// boutons et interrupteurs que logind garde (alimentation, veille, rabat de
/// l'écran, touches de luminosité de la carte graphique).
pub fn physical(entries: &[(String, String, String)]) -> Vec<String> {
    let mut out: Vec<String> = entries
        .iter()
        .filter(|(name, _, _)| name.starts_with("event") && name[5..].chars().all(|c| c.is_ascii_digit()) && name.len() > 5)
        .filter(|(_, resolved, _)| !resolved.contains("/devices/virtual/"))
        .filter(|(_, _, device)| !["Power Button", "Sleep Button", "Lid Switch", "Video Bus"].iter().any(|n| device.contains(n)))
        .map(|(name, _, _)| format!("/dev/input/{name}"))
        .collect();
    out.sort();
    out
}

#[cfg(target_os = "linux")]
pub use device::{Grabber, physical_devices};

#[cfg(target_os = "linux")]
mod device {
    use std::collections::HashMap;
    use std::os::fd::{AsRawFd, OwnedFd, RawFd};
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use super::{Key, RESCAN_EVERY};

    /// `EVIOCGRAB` : `_IOW('E', 0x90, int)`.
    const EVIOCGRAB: libc::c_ulong = 0x4004_4590;
    const EV_KEY: u16 = 1;
    const KEY_ESC: u16 = 1;
    const KEY_ENTER: u16 = 28;
    const KEY_KPENTER: u16 = 96;

    /// `EVIOCGNAME(len)` : `_IOC(_IOC_READ, 'E', 0x06, len)`.
    pub(super) fn eviocgname(len: usize) -> libc::c_ulong {
        (2 << 30) | ((len as libc::c_ulong) << 16) | (0x45 << 8) | 0x06
    }

    /// Les appareils d'entrée branchés sur l'ordinateur (voir [`super::physical`]).
    pub fn physical_devices() -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir("/sys/class/input") else { return Vec::new() };
        let entries: Vec<(String, String, String)> = entries
            .flatten()
            .map(|e| {
                let resolved = std::fs::canonicalize(e.path()).map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let device = std::fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
                (e.file_name().to_string_lossy().to_string(), resolved, device)
            })
            .filter(|(_, resolved, _)| !resolved.is_empty())
            .collect();
        super::physical(&entries).into_iter().map(PathBuf::from).collect()
    }

    fn device_name(fd: &OwnedFd) -> String {
        let mut buf = [0u8; 128];
        let n = unsafe { libc::ioctl(fd.as_raw_fd(), eviocgname(buf.len()), buf.as_mut_ptr()) };
        if n <= 0 {
            return String::new();
        }
        String::from_utf8_lossy(&buf[..(n as usize).min(buf.len())]).trim_end_matches('\0').to_string()
    }

    fn open(path: &Path) -> std::io::Result<OwnedFd> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW).open(path)?;
        Ok(file.into())
    }

    /// Ouvre et prend un appareil en exclusivité.
    fn grab(path: &Path) -> std::io::Result<OwnedFd> {
        let fd = open(path)?;
        if unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, 1 as libc::c_int) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(fd)
    }

    /// Lit les évènements en attente : les touches du cache appuyées, et si
    /// l'appareil a été retiré.
    fn read_keys(fd: &OwnedFd) -> (Vec<Key>, bool) {
        const SIZE: usize = std::mem::size_of::<libc::input_event>();
        let mut keys = Vec::new();
        let mut buf = [0u8; SIZE * 64];
        loop {
            let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                let gone = std::io::Error::last_os_error().raw_os_error() == Some(libc::ENODEV);
                return (keys, gone);
            }
            if n == 0 {
                return (keys, true);
            }
            for chunk in buf[..n as usize].chunks_exact(SIZE) {
                let ev: libc::input_event = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                if ev.type_ == EV_KEY && ev.value == 1 {
                    match ev.code {
                        KEY_ENTER | KEY_KPENTER => keys.push(Key::Enter),
                        KEY_ESC => keys.push(Key::Escape),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Combien d'appareils physiques ce processus saurait prendre (ouvrir
    /// en lecture), sans rien prendre.
    pub fn openable() -> usize {
        physical_devices().iter().filter(|p| open(p).is_ok()).count()
    }

    /// Les appareils pris, relus toutes les [`RESCAN_EVERY`] ; rendus quand
    /// il est lâché (le noyau rend aussi l'entrée quand le processus meurt).
    pub struct Grabber {
        held: HashMap<PathBuf, OwnedFd>,
        refused: HashMap<PathBuf, Instant>,
        /// Refus déjà notés au journal (une fois par appareil).
        told: std::collections::HashSet<PathBuf>,
        scanned: Option<Instant>,
    }

    impl Default for Grabber {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Grabber {
        pub fn new() -> Grabber {
            Grabber { held: HashMap::new(), refused: HashMap::new(), told: Default::default(), scanned: None }
        }

        pub fn count(&self) -> usize {
            self.held.len()
        }

        /// Prend les appareils nouveaux (tout de suite la première fois, puis
        /// toutes les [`RESCAN_EVERY`]). Un refus est retenté au bout de 30 s.
        pub fn rescan(&mut self, log: &mut dyn FnMut(String)) {
            if self.scanned.is_some_and(|at| at.elapsed() < RESCAN_EVERY) {
                return;
            }
            self.scanned = Some(Instant::now());
            self.refused.retain(|_, at| at.elapsed() < std::time::Duration::from_secs(30));
            for path in physical_devices() {
                if self.held.contains_key(&path) || self.refused.contains_key(&path) {
                    continue;
                }
                match grab(&path) {
                    Ok(fd) => {
                        log(format!("entrée de l'ordinateur prise : {} « {} »", path.display(), device_name(&fd)));
                        self.held.insert(path, fd);
                    }
                    Err(e) => {
                        if self.told.insert(path.clone()) {
                            log(format!("entrée de l'ordinateur non prise : {} ({e})", path.display()));
                        }
                        self.refused.insert(path, Instant::now());
                    }
                }
            }
        }

        /// Attend un évènement des appareils pris (ou de `extra`, la
        /// connexion de l'aide) pendant `timeout_ms` au plus : rend les
        /// touches du cache, et si `extra` est lisible (ou fermé).
        pub fn poll(&mut self, extra: Option<RawFd>, timeout_ms: i32) -> (Vec<Key>, bool) {
            let paths: Vec<PathBuf> = self.held.keys().cloned().collect();
            let mut fds: Vec<libc::pollfd> =
                paths.iter().map(|p| libc::pollfd { fd: self.held[p].as_raw_fd(), events: libc::POLLIN, revents: 0 }).collect();
            if let Some(fd) = extra {
                fds.push(libc::pollfd { fd, events: libc::POLLIN, revents: 0 });
            }
            if fds.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(timeout_ms.max(0) as u64));
                return (Vec::new(), false);
            }
            let n = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout_ms) };
            if n <= 0 {
                return (Vec::new(), false);
            }
            let extra_ready = extra.is_some() && fds.last().is_some_and(|p| p.revents != 0);
            let mut keys = Vec::new();
            for (path, pfd) in paths.iter().zip(&fds) {
                if pfd.revents == 0 {
                    continue;
                }
                let (k, gone) = read_keys(&self.held[path]);
                keys.extend(k);
                if gone || pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                    self.held.remove(path);
                }
            }
            (keys, extra_ready)
        }
    }

    impl Drop for Grabber {
        /// Rend chaque appareil au compositeur (fermer le descripteur suffit ;
        /// l'appel explicite le dit dans l'ordre).
        fn drop(&mut self) {
            for (_, fd) in self.held.drain() {
                unsafe { libc::ioctl(fd.as_raw_fd(), EVIOCGRAB, 0 as libc::c_int) };
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use device::openable;

/// Joindre l'aide root (agent de console et Admin) : `None` si elle n'est pas
/// là (socket absent, droits refusés, aide qui ne répond pas).
#[cfg(unix)]
pub mod client {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use super::{Event, Request, SOCKET};

    /// Une connexion à l'aide, après sa réponse.
    pub struct Connection {
        pub stream: UnixStream,
        pub reader: BufReader<UnixStream>,
        /// Appareils pris (ou qu'elle saurait prendre).
        pub devices: usize,
    }

    /// Envoie la demande et lit la réponse (`wait` au plus).
    pub fn open(request: Request, wait: Duration) -> std::io::Result<Connection> {
        open_at(std::path::Path::new(SOCKET), request, wait)
    }

    pub fn open_at(socket: &std::path::Path, request: Request, wait: Duration) -> std::io::Result<Connection> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(wait))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        writeln!(stream, "{}", request.as_str())?;
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        match Event::parse(&line) {
            Some(Event::Ok(devices)) => Ok(Connection { stream, reader, devices }),
            Some(Event::Error(text)) => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, text)),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("réponse inattendue de l'aide : {}", line.trim()))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocole_aller_retour() {
        for e in [Event::Ok(3), Event::Error("refusé".into()), Event::Key(Key::Enter), Event::Key(Key::Escape), Event::Devices(0)] {
            assert_eq!(Event::parse(&e.line()), Some(e.clone()), "{e:?}");
        }
        assert_eq!(Event::Error("deux\nlignes".into()).line(), "error deux lignes");
        assert_eq!(Event::parse("ok"), None);
        assert_eq!(Event::parse("key tab"), None);
        assert_eq!(Request::parse("grab\n"), Some(Request::Grab));
        assert_eq!(Request::parse(Request::Probe.as_str()), Some(Request::Probe));
        assert_eq!(Request::parse("tout"), None);
        assert_eq!(AgentLine::parse(" ping\n"), Some(AgentLine::Ping));
        assert_eq!(AgentLine::parse("release"), Some(AgentLine::Release));
        assert_eq!(AgentLine::parse("grab"), None);
    }

    #[test]
    fn appareils_physiques_seulement() {
        let e = |name: &str, resolved: &str, device: &str| (name.to_string(), resolved.to_string(), device.to_string());
        let entries = [
            e("event3", "/sys/devices/pci0000:00/0000:00:14.0/usb1/1-2/input/input7/event3", "Logitech USB Keyboard"),
            e("event1", "/sys/devices/platform/i8042/serio0/input/input1/event1", "AT Translated Set 2 keyboard"),
            // Kyber (uinput) et les manettes de l'ami (uhid) : jamais pris.
            e("event12", "/sys/devices/virtual/input/input30/event12", "Kyber Virtual Keyboard"),
            e("event13", "/sys/devices/virtual/misc/uhid/0003:054C:0CE6.0009/input/input31/event13", "DualSense Wireless Controller"),
            e("event0", "/sys/devices/LNXSYSTM:00/LNXPWRBN:00/input/input0/event0", "Power Button"),
            e("event2", "/sys/devices/platform/PNP0C0D:00/input/input2/event2", "Lid Switch"),
            e("mouse0", "/sys/devices/pci0000:00/input/input7/mouse0", "Logitech USB Mouse"),
            e("event", "/sys/devices/pci0000:00/input/event", ""),
        ];
        assert_eq!(physical(&entries), ["/dev/input/event1", "/dev/input/event3"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn le_numero_de_eviocgname() {
        // EVIOCGNAME(256) de linux/input.h : 0x81004506.
        assert_eq!(device::eviocgname(256), 0x8100_4506);
    }

    /// Une fausse aide sur un socket : réponse, refus, réponse illisible.
    #[cfg(unix)]
    #[test]
    fn client_lit_la_reponse_de_l_aide() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;

        let dir = std::env::temp_dir().join(format!("kn-input-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("input.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            for answer in ["ok 2", "error ce compte n'est pas celui de la Station", "bonjour"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
                assert_eq!(line.trim(), "probe");
                writeln!(stream, "{answer}").unwrap();
            }
        });
        assert_eq!(client::open_at(&socket, Request::Probe, Duration::from_secs(2)).unwrap().devices, 2);
        let refused = client::open_at(&socket, Request::Probe, Duration::from_secs(2)).err().unwrap();
        assert_eq!((refused.kind(), refused.to_string()), (std::io::ErrorKind::PermissionDenied, "ce compte n'est pas celui de la Station".to_string()));
        assert_eq!(client::open_at(&socket, Request::Probe, Duration::from_secs(2)).err().unwrap().kind(), std::io::ErrorKind::InvalidData);
        server.join().unwrap();
        assert!(client::open_at(&dir.join("absent.sock"), Request::Probe, Duration::from_secs(2)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
