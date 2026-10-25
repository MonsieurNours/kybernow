//! Redémarrage de KyberNow Station entière : le serveur Kyber (service
//! `kyber`) et Admin (service `kyber-admin`), comme « Redémarrer la Station »
//! de l'icône. Demandé depuis Admin (`POST /api/station/restart`) ou depuis
//! une app KyberNow Link (`POST /api/stream/station/restart`).
//!
//! Admin ne peut pas s'arrêter lui-même au milieu de la requête : il lance un
//! assistant détaché (`kyber-admin restart-station`, commande cachée, en
//! SYSTEM comme le service), sur le modèle de l'applicateur des mises à jour.
//! L'assistant laisse partir la réponse, arrête Admin puis Kyber, et les
//! redémarre dans l'ordre inverse : Kyber, puis Admin. Le service
//! `kyber-admin` n'a pas de relance automatique : l'assistant le relance même
//! si un arrêt a échoué. Son journal (celui du dernier redémarrage) est
//! `station\redemarrage.log` dans le dossier de données, sa sortie
//! `station\lancement.log`.
//!
//! Au plus un redémarrage par minute : l'heure du dernier est gardée dans
//! `station\dernier-redemarrage`, qui survit au redémarrage d'Admin.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::update::apply::Journal;

pub const KYBER: &str = "kyber";
pub const ADMIN: &str = "kyber-admin";
/// Au plus un redémarrage par minute.
pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
/// Temps laissé à la réponse pour partir avant l'arrêt d'Admin.
const GRACE: Duration = Duration::from_secs(2);
/// Au-delà, un redémarrage accepté qui n'a pas arrêté Admin n'est plus « en cours ».
const RESTARTING_FOR: Duration = Duration::from_secs(120);
const DIR: &str = "station";
const LAST: &str = "dernier-redemarrage";
const LOG: &str = "redemarrage.log";
#[cfg(windows)]
const OUTPUT: &str = "lancement.log";

/// Lance l'assistant, détaché, avec le dossier de données ; remplacé dans les
/// tests. `None` : rien à suivre.
pub type Launcher = Box<dyn Fn(&Path) -> Result<Option<Child>> + Send + Sync>;

/// Pourquoi un redémarrage n'est pas lancé.
#[derive(Debug)]
pub enum Refused {
    /// Le précédent date de moins d'une minute : reste à attendre.
    TooSoon(Duration),
    /// L'assistant n'a pas pu être lancé (hors Windows : jamais).
    Failed(anyhow::Error),
}

#[derive(Default)]
struct State {
    /// Demande acceptée par cet Admin, qui va s'arrêter.
    requested: Option<Instant>,
    /// L'assistant lancé : s'il s'arrête alors qu'Admin tourne encore, le
    /// redémarrage n'a pas eu lieu.
    child: Option<Child>,
    /// Heure du dernier redémarrage accepté, si le fichier n'a pas pu l'écrire.
    last: Option<SystemTime>,
}

pub struct Restarter {
    data_dir: PathBuf,
    launcher: Launcher,
    state: Mutex<State>,
}

impl Restarter {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_launcher(data_dir, Box::new(launch_assistant))
    }

    pub fn with_launcher(data_dir: &Path, launcher: Launcher) -> Self {
        Self { data_dir: data_dir.to_path_buf(), launcher, state: Mutex::new(State::default()) }
    }

    fn dir(&self) -> PathBuf {
        self.data_dir.join(DIR)
    }

    /// Un redémarrage accepté ici est en cours : Admin va s'arrêter.
    pub fn restarting(&self) -> bool {
        let mut s = self.state.lock().unwrap();
        Self::reap(&mut s, &self.dir());
        s.requested.is_some_and(|t| t.elapsed() < RESTARTING_FOR)
    }

    /// L'assistant s'est arrêté alors qu'Admin tourne encore : le redémarrage
    /// a échoué en route (son journal le dit).
    fn reap(s: &mut State, dir: &Path) {
        let Some(child) = s.child.as_mut() else { return };
        let Ok(Some(exit)) = child.try_wait() else { return };
        s.child = None;
        s.requested = None;
        tracing::error!("redémarrage de la Station interrompu ({exit}) : voir {}", dir.join(LOG).display());
    }

    /// Heure du dernier redémarrage accepté (fichier, sinon mémoire). Une
    /// heure dans le futur (horloge remise en arrière) ne compte pas.
    fn last(&self, s: &State) -> Option<SystemTime> {
        let from_file = fs::read_to_string(self.dir().join(LAST))
            .ok()
            .and_then(|t| t.trim().parse::<u64>().ok())
            .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));
        [from_file, s.last].into_iter().flatten().filter(|t| *t <= SystemTime::now()).max()
    }

    /// Lance le redémarrage, au plus un par minute.
    pub fn restart(&self) -> std::result::Result<(), Refused> {
        let mut s = self.state.lock().unwrap();
        if let Some(elapsed) = self.last(&s).and_then(|t| t.elapsed().ok())
            && elapsed < MIN_INTERVAL
        {
            return Err(Refused::TooSoon(MIN_INTERVAL - elapsed));
        }
        let child = (self.launcher)(&self.data_dir).map_err(Refused::Failed)?;
        let now = SystemTime::now();
        let secs = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default();
        if let Err(e) = fs::create_dir_all(self.dir()).and_then(|_| fs::write(self.dir().join(LAST), secs.to_string())) {
            tracing::warn!("heure du redémarrage non écrite : {e}");
        }
        s.last = Some(now);
        s.requested = Some(Instant::now());
        s.child = child;
        Ok(())
    }
}

/// Ce que le redémarrage demande au système : arrêter et démarrer un service.
pub trait Services {
    /// Arrête le service et attend qu'il soit arrêté.
    fn stop(&mut self, name: &str) -> Result<()>;
    /// Démarre le service et attend qu'il tourne.
    fn start(&mut self, name: &str) -> Result<()>;
}

/// Arrête Admin (il ne relance plus Kyber pendant ce temps : comptes en
/// attente, bouton Démarrer) puis Kyber, et les redémarre : Kyber, puis
/// Admin. Chaque étape est tentée quoi qu'il arrive avant : les deux services
/// repartent même si un arrêt a échoué. Rend la première erreur.
pub fn restart(sys: &mut dyn Services, log: &mut Journal) -> Result<()> {
    let mut first: Option<anyhow::Error> = None;
    for (name, start) in [(ADMIN, false), (KYBER, false), (KYBER, true), (ADMIN, true)] {
        log.line(format!("==> {} du service {name}", if start { "Démarrage" } else { "Arrêt" }));
        let result = if start { sys.start(name) } else { sys.stop(name) };
        if let Err(e) = result {
            log.line(format!("    ERREUR : {e:#}"));
            first.get_or_insert(e);
        }
    }
    match first {
        None => {
            log.line("Terminé : KyberNow Station redémarrée.");
            Ok(())
        }
        Some(e) => Err(e),
    }
}

/// Point d'entrée de `kyber-admin restart-station` (l'assistant).
pub fn run(data_dir: &Path) -> Result<()> {
    let dir = data_dir.join(DIR);
    // Le journal ne garde que le dernier redémarrage.
    let _ = fs::remove_file(dir.join(LOG));
    let mut log = Journal::open(dir.join(LOG));
    log.line("==> Redémarrage de KyberNow Station demandé depuis Admin");
    std::thread::sleep(GRACE);
    let mut sys = platform()?;
    let result = restart(sys.as_mut(), &mut log);
    if let Err(e) = &result {
        log.line(format!("ERREUR : {e:#}"));
    }
    result
}

/// L'assistant : cet exécutable (`kyber-admin.exe restart-station`), détaché
/// d'Admin. Il tourne depuis `admin\` : une mise à jour (qui renomme ce
/// dossier) est refusée pendant un redémarrage, et l'inverse.
#[cfg(windows)]
fn launch_assistant(data_dir: &Path) -> Result<Option<Child>> {
    use anyhow::Context;
    let dir = data_dir.join(DIR);
    fs::create_dir_all(&dir).with_context(|| format!("création de {}", dir.display()))?;
    let exe = std::env::current_exe().context("chemin d'Admin")?;
    let args: [std::ffi::OsString; 3] = ["restart-station".into(), "--data-dir".into(), data_dir.as_os_str().to_owned()];
    let child = crate::update::spawn_detached(&exe, &args, &dir, &dir.join(OUTPUT))?;
    tracing::info!("redémarrage de la Station : assistant lancé (processus {})", child.id());
    Ok(Some(child))
}

#[cfg(not(windows))]
fn launch_assistant(_: &Path) -> Result<Option<Child>> {
    Err(crate::i18n::Msg::new("station.windowsOnly").into())
}

#[cfg(windows)]
fn platform() -> Result<Box<dyn Services>> {
    Ok(Box::new(windows::WindowsServices))
}

#[cfg(not(windows))]
fn platform() -> Result<Box<dyn Services>> {
    anyhow::bail!("redémarrage de la Station réservé à Windows")
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::time::{Duration, Instant};

    use anyhow::{Context, Result, bail};
    use windows_service::service::{Service, ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    use super::Services;

    /// Attente d'un arrêt ou d'un démarrage, comme l'icône.
    const WAIT: Duration = Duration::from_secs(30);
    /// Après l'arrêt forcé d'un processus resté bloqué.
    const AFTER_KILL: Duration = Duration::from_secs(10);

    pub struct WindowsServices;

    fn open(name: &str, access: ServiceAccess) -> Result<Service> {
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("connexion au gestionnaire de services")?
            .open_service(name, access)
            .with_context(|| format!("service {name} introuvable"))
    }

    fn wait_for(service: &Service, state: ServiceState, timeout: Duration) -> Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            if service.query_status()?.current_state == state {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Arrêt forcé d'un processus (jamais celui de l'assistant).
    fn kill(pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle.is_null() {
                return false;
            }
            let ok = TerminateProcess(handle, 1) != 0;
            CloseHandle(handle);
            ok
        }
    }

    impl Services for WindowsServices {
        fn stop(&mut self, name: &str) -> Result<()> {
            let service = open(name, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP)?;
            let deadline = Instant::now() + WAIT;
            let mut pid = None;
            loop {
                let status = service.query_status()?;
                pid = status.process_id.or(pid);
                match status.current_state {
                    ServiceState::Stopped => return Ok(()),
                    // En marche ou en train de démarrer : l'arrêt est (re)demandé.
                    ServiceState::Running | ServiceState::StartPending | ServiceState::Paused => {
                        let _ = service.stop();
                    }
                    _ => {}
                }
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            // Bloqué en cours d'arrêt : son processus est arrêté, pour qu'il puisse redémarrer.
            if pid.is_some_and(kill) && wait_for(&service, ServiceState::Stopped, AFTER_KILL)? {
                return Ok(());
            }
            bail!("le service {name} ne s'est pas arrêté en {} s", WAIT.as_secs())
        }

        fn start(&mut self, name: &str) -> Result<()> {
            let service = open(name, ServiceAccess::QUERY_STATUS | ServiceAccess::START)?;
            if !matches!(service.query_status()?.current_state, ServiceState::Running | ServiceState::StartPending) {
                service.start::<&OsStr>(&[]).with_context(|| format!("démarrage du service {name}"))?;
            }
            if !wait_for(&service, ServiceState::Running, WAIT)? {
                bail!("le service {name} n'a pas démarré en {} s", WAIT.as_secs());
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::bail;

    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kn-station-{}", rand::random::<u64>()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Faux services : notent les appels, un arrêt peut échouer.
    #[derive(Default)]
    struct Fake {
        calls: Vec<String>,
        stuck: Option<&'static str>,
    }

    impl Services for Fake {
        fn stop(&mut self, name: &str) -> Result<()> {
            self.calls.push(format!("stop {name}"));
            if self.stuck == Some(name) {
                bail!("{name} ne s'arrête pas");
            }
            Ok(())
        }
        fn start(&mut self, name: &str) -> Result<()> {
            self.calls.push(format!("start {name}"));
            Ok(())
        }
    }

    #[test]
    fn admin_stops_first_and_starts_last() {
        let dir = temp();
        let mut log = Journal::open(dir.join(LOG));
        let mut sys = Fake::default();
        restart(&mut sys, &mut log).unwrap();
        assert_eq!(sys.calls, ["stop kyber-admin", "stop kyber", "start kyber", "start kyber-admin"]);
        let journal = fs::read_to_string(dir.join(LOG)).unwrap();
        assert!(journal.contains("Terminé : KyberNow Station redémarrée."), "{journal}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn both_services_come_back_even_if_one_does_not_stop() {
        let dir = temp();
        let mut log = Journal::open(dir.join(LOG));
        let mut sys = Fake { stuck: Some(KYBER), ..Default::default() };
        let err = restart(&mut sys, &mut log).unwrap_err();
        assert!(err.to_string().contains("kyber ne s'arrête pas"), "{err}");
        assert_eq!(sys.calls, ["stop kyber-admin", "stop kyber", "start kyber", "start kyber-admin"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn one_restart_per_minute_even_across_admin_restarts() {
        let dir = temp();
        let launched = Arc::new(AtomicUsize::new(0));
        let counter = launched.clone();
        let restarter = Restarter::with_launcher(
            &dir,
            Box::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );
        assert!(!restarter.restarting());
        restarter.restart().unwrap();
        assert!(restarter.restarting());
        match restarter.restart() {
            Err(Refused::TooSoon(wait)) => assert!(wait > Duration::from_secs(55) && wait <= MIN_INTERVAL, "{wait:?}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(launched.load(Ordering::SeqCst), 1);
        // Un nouvel Admin (redémarré) relit l'heure du dernier redémarrage.
        let again = Restarter::with_launcher(&dir, Box::new(|_| Ok(None)));
        assert!(!again.restarting());
        assert!(matches!(again.restart(), Err(Refused::TooSoon(_))));
        // Plus d'une minute après : accepté.
        let old = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 61;
        fs::write(dir.join(DIR).join(LAST), old.to_string()).unwrap();
        let later = Restarter::with_launcher(&dir, Box::new(|_| Ok(None)));
        later.restart().unwrap();
        // Une heure dans le futur (horloge remise en arrière) ne bloque pas.
        let future = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600;
        fs::write(dir.join(DIR).join(LAST), future.to_string()).unwrap();
        Restarter::with_launcher(&dir, Box::new(|_| Ok(None))).restart().unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_launch_is_not_counted() {
        let dir = temp();
        let restarter = Restarter::with_launcher(&dir, Box::new(|_| bail!("pas d'assistant")));
        assert!(matches!(restarter.restart(), Err(Refused::Failed(_))));
        assert!(!restarter.restarting());
        assert!(!dir.join(DIR).join(LAST).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(not(windows))]
    #[test]
    fn outside_windows_the_launch_fails_cleanly() {
        let dir = temp();
        let restarter = Restarter::new(&dir);
        match restarter.restart() {
            Err(Refused::Failed(e)) => {
                assert_eq!(crate::i18n::error_text(crate::i18n::Lang::Fr, &e), "redémarrer la Station n'est possible que sous Windows")
            }
            other => panic!("{other:?}"),
        }
        assert!(!restarter.restarting());
        let _ = fs::remove_dir_all(dir);
    }
}
