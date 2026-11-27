//! Bureau GNOME : tout passe par Mutter, sur le bus de la session.
//!
//! - Écrans virtuels : `org.gnome.Mutter.ScreenCast`, `RecordVirtual`, une
//!   session de capture par écran (Mutter ne démarre pas un flux ajouté
//!   après `Start`). Mutter crée l'écran quand un lecteur se branche au flux
//!   PipeWire, à la taille que ce lecteur négocie (la capture de Kyber).
//! - Écrans de l'ordinateur : la même API, `RecordMonitor`, sans le dialogue
//!   du portail de partage d'écran (la Station n'est pas dans un bac à sable).
//! - Disposition : `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig` en
//!   mode temporaire (jamais enregistré).

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use zbus::MatchRule;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::message::Type as MessageType;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value as ZValue};

use crate::{Cast, CastKey, Desk, Mon, Place, Recorder, Result, err, serial_of};

const SC_DEST: &str = "org.gnome.Mutter.ScreenCast";
const SC_PATH: &str = "/org/gnome/Mutter/ScreenCast";
const SC_SESSION: &str = "org.gnome.Mutter.ScreenCast.Session";
const SC_STREAM: &str = "org.gnome.Mutter.ScreenCast.Stream";
const DC_DEST: &str = "org.gnome.Mutter.DisplayConfig";
const DC_PATH: &str = "/org/gnome/Mutter/DisplayConfig";

type Props = HashMap<String, OwnedValue>;
type Spec = (String, String, String, String);
type ModeT = (String, i32, i32, f64, f64, Vec<f64>, Props);
type MonitorT = (Spec, Vec<ModeT>, Props);
type LogicalT = (i32, i32, f64, u32, bool, Vec<Spec>, Props);
type StateT = (u32, Vec<MonitorT>, Vec<LogicalT>, Props);
type LogicalOut = (i32, i32, f64, u32, bool, Vec<(String, String, HashMap<String, ZValue<'static>>)>);

/// Mutter est là : `org.gnome.Mutter.ScreenCast` a un propriétaire sur le bus.
pub fn present() -> bool {
    let Ok(conn) = Connection::session() else { return false };
    let Ok(dbus) = Proxy::new(&conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus") else {
        return false;
    };
    dbus.call::<_, _, bool>("NameHasOwner", &(SC_DEST,)).unwrap_or(false)
}

fn prop_bool(props: &Props, key: &str) -> bool {
    props.get(key).and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
}

fn current_mode(monitor: &MonitorT) -> Option<&ModeT> {
    monitor.1.iter().find(|m| prop_bool(&m.6, "is-current"))
}

/// Les écrans virtuels de Mutter s'appellent « Meta-0 », « Meta-1 »…
fn is_virtual(connector: &str) -> bool {
    connector.starts_with("Meta-")
}

pub struct Gnome {
    conn: Connection,
}

impl Gnome {
    pub fn new() -> Result<Self> {
        Ok(Self { conn: Connection::session().map_err(err("bus de la session"))? })
    }

    fn record(&self, connector: Option<&str>) -> Result<Cast> {
        let conn = &self.conn;
        let sc = Proxy::new(conn, SC_DEST, SC_PATH, SC_DEST).map_err(err("ScreenCast"))?;
        let none: HashMap<&str, ZValue> = HashMap::new();
        let session: OwnedObjectPath = sc.call("CreateSession", &(none,)).map_err(err("CreateSession"))?;
        let sp = Proxy::new(conn, SC_DEST, session.clone(), SC_SESSION).map_err(err("session"))?;
        // Sur toute erreur après CreateSession, la session est arrêtée : sinon
        // elle restait chez Mutter, avec un fil qui l'attendait, à chacun des
        // 7 essais par écran (STLIN-10).
        match self.start(&sp, &session, connector) {
            Ok(node) => Ok(Cast { key: CastKey::Mutter(session), node, serial: serial_of(node) }),
            Err(e) => {
                let _ = sp.call::<_, _, ()>("Stop", &());
                Err(e)
            }
        }
    }

    /// Le flux de la session `session`, démarrée ; le numéro de son nœud PipeWire.
    fn start(&self, sp: &Proxy<'_>, session: &OwnedObjectPath, connector: Option<&str>) -> Result<u32> {
        let conn = &self.conn;
        // Le pointeur reste hors de l'image : chaque appareil dessine le sien.
        let mut opts: HashMap<&str, ZValue> = HashMap::new();
        opts.insert("cursor-mode", ZValue::U32(0));
        // Signaux de ScreenCast écoutés avant d'en demander : le flux annoncé,
        // ou la session fermée (arrêtée sur erreur), qui libère le fil d'attente.
        let rule = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender(SC_DEST)
            .map_err(err("règle des signaux"))?
            .build();
        let signals = MessageIterator::for_match_rule(rule, conn, Some(16)).map_err(err("signaux de ScreenCast"))?;
        let stream: OwnedObjectPath = match connector {
            Some(c) => sp.call("RecordMonitor", &(c, opts)).map_err(err("RecordMonitor"))?,
            None => {
                // Écran « de plateforme » : GNOME y met sa barre et ses
                // fenêtres comme sur un vrai écran (Mutter 44 et après).
                let mut platform = opts.clone();
                platform.insert("is-platform", ZValue::Bool(true));
                match sp.call("RecordVirtual", &(platform,)) {
                    Ok(s) => s,
                    Err(_) => sp.call("RecordVirtual", &(opts,)).map_err(err("RecordVirtual"))?,
                }
            }
        };
        let (tx, rx) = mpsc::channel();
        let (stream_path, session_path) = (stream.to_string(), session.to_string());
        std::thread::spawn(move || {
            for msg in signals {
                let Ok(msg) = msg else { break };
                let header = msg.header();
                let path = header.path().map(|p| p.to_string()).unwrap_or_default();
                let member = header.member().map(|m| m.to_string()).unwrap_or_default();
                let interface = header.interface().map(|i| i.to_string()).unwrap_or_default();
                if member == "PipeWireStreamAdded" && interface == SC_STREAM && path == stream_path {
                    let _ = tx.send(msg.body().deserialize::<u32>().ok());
                    return;
                }
                if member == "Closed" && path == session_path {
                    let _ = tx.send(None);
                    return;
                }
            }
        });
        sp.call::<_, _, ()>("Start", &()).map_err(err("Start"))?;
        rx.recv_timeout(Duration::from_secs(5))
            .ok()
            .flatten()
            .ok_or_else(|| "Mutter n'a pas donné de flux PipeWire en 5 s".to_string())
    }

    fn dc(&self) -> Result<Proxy<'static>> {
        Proxy::new(&self.conn, DC_DEST, DC_PATH, DC_DEST).map_err(err("DisplayConfig"))
    }

    fn state(&self) -> Result<StateT> {
        self.dc()?.call("GetCurrentState", &()).map_err(err("GetCurrentState"))
    }

    fn apply_once(&self, places: &[Place]) -> Result<()> {
        let (serial, monitors, _, _) = self.state()?;
        let out: Vec<LogicalOut> = places
            .iter()
            .filter_map(|p| {
                let m = monitors.iter().find(|m| m.0.0 == p.connector)?;
                let mode = current_mode(m).or_else(|| m.1.iter().find(|md| prop_bool(&md.6, "is-preferred"))).or(m.1.first())?;
                Some((p.x, p.y, p.scale, 0, p.primary, vec![(p.connector.clone(), mode.0.clone(), HashMap::new())]))
            })
            .collect();
        if out.is_empty() {
            return Ok(());
        }
        let none: HashMap<&str, ZValue> = HashMap::new();
        self.dc()?
            .call::<_, _, ()>("ApplyMonitorsConfig", &(serial, 1u32, out, none))
            .map_err(err("ApplyMonitorsConfig"))
    }
}

impl Recorder for Gnome {
    fn virtual_output(&mut self, _slot: usize, _width: u32, _height: u32, _hidpi: bool) -> Result<(Cast, Option<String>)> {
        // Le connecteur (« Meta-N ») n'est connu qu'une fois l'écran apparu.
        Ok((self.record(None)?, None))
    }

    fn monitor(&mut self, connector: &str) -> Result<Cast> {
        self.record(Some(connector))
    }

    fn stop(&mut self, cast: &Cast) {
        if let CastKey::Mutter(session) = &cast.key
            && let Ok(sp) = Proxy::new(&self.conn, SC_DEST, session.clone(), SC_SESSION)
        {
            let _ = sp.call::<_, _, ()>("Stop", &());
        }
    }
}

impl Desk for Gnome {
    fn name(&self) -> &'static str {
        "GNOME"
    }

    fn monitors(&self) -> Result<Vec<Mon>> {
        let (_, monitors, logical, _) = self.state()?;
        Ok(monitors
            .iter()
            .map(|m| {
                let l = logical.iter().find(|l| l.5.contains(&m.0));
                let mode = current_mode(m).or(m.1.first());
                Mon {
                    connector: m.0.0.clone(),
                    virt: is_virtual(&m.0.0),
                    enabled: l.is_some(),
                    x: l.map_or(0, |l| l.0),
                    y: l.map_or(0, |l| l.1),
                    width: mode.map_or(0, |md| md.1 as u32),
                    height: mode.map_or(0, |md| md.2 as u32),
                    refresh: mode.map_or(60, |md| md.3.round() as u32),
                    scale: l.map_or(1.0, |l| l.2),
                    primary: l.is_some_and(|l| l.4),
                    scales: mode.map(|md| md.5.clone()).unwrap_or_else(|| vec![1.0]),
                }
            })
            .collect())
    }

    /// Disposition « physique » (`layout-mode` 2, celle des échelles
    /// fractionnaires) : places en pixels ; « logique » (1) : en points.
    fn physical_units(&self) -> Result<bool> {
        let (_, _, _, props) = self.state()?;
        Ok(props.get("layout-mode").and_then(|v| u32::try_from(v).ok()) == Some(2))
    }

    fn apply(&self, places: &[Place]) -> Result<()> {
        // Un écran virtuel qui vient d'arriver ou de partir change encore la
        // configuration un instant : « stale information ». On réessaie avec
        // l'état suivant (une seconde au plus).
        let mut tries = 0;
        loop {
            match self.apply_once(places) {
                Err(e) if e.contains("stale") && tries < 5 => {
                    tries += 1;
                    std::thread::sleep(Duration::from_millis(200));
                }
                other => return other,
            }
        }
    }
}
