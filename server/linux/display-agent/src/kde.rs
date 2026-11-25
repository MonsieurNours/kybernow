//! Bureau KDE Plasma (KWin) : écrans et capture par le protocole Wayland de
//! KWin `zkde_screencast_unstable_v1`, disposition par `kscreen-doctor`.
//!
//! - Écrans virtuels : `stream_virtual_output` (version 2 du protocole,
//!   Plasma 5.25 et après), à la taille donnée (en points, avec l'échelle) ;
//!   le flux PipeWire arrive avec l'événement `created`. Une autre taille
//!   demande un nouvel écran.
//! - Écrans de l'ordinateur : `stream_output` sur leur `wl_output`, sans le
//!   dialogue du portail.
//! - KWin ne donne ce protocole qu'aux programmes qui y ont droit : un
//!   fichier `.desktop` installé dont `Exec` est cet agent et qui nomme
//!   l'interface dans `X-KDE-Wayland-Interfaces` (le paquet le pose :
//!   `fr.leslignesclaires.kybernow.station.display.desktop`).
//! - Disposition : `kscreen-doctor` (activer, placer, échelle, priorité ;
//!   les écrans absents de la disposition sont désactivés). Places en points.
//!
//! Les écrans virtuels vivent avec la connexion Wayland de l'agent : s'il
//! s'arrête, KWin les retire.

use std::collections::HashMap;
use std::process::Command;
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_output, wl_registry};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};

use crate::{Cast, CastKey, Desk, Mon, Place, Recorder, Result, err, log, serial_of};

#[allow(dead_code, non_camel_case_types, non_upper_case_globals, clippy::all)]
pub mod proto {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_backend;
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/zkde-screencast-unstable-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/zkde-screencast-unstable-v1.xml");
}

use proto::zkde_screencast_stream_unstable_v1::{self as zstream, ZkdeScreencastStreamUnstableV1};
use proto::zkde_screencast_unstable_v1::ZkdeScreencastUnstableV1;

/// Pointeur hors de l'image (`pointer.hidden`) : chaque appareil dessine le sien.
const POINTER_HIDDEN: u32 = 1;
/// Nom des écrans virtuels de la Station : « KyberNow-1 », « KyberNow-2 »…
pub const VIRTUAL_PREFIX: &str = "KyberNow-";

enum StreamEvent {
    Created(u32),
    Failed(String),
    Closed,
}

#[derive(Default)]
struct State {
    screencast: Option<ZkdeScreencastUnstableV1>,
    outputs: Vec<(wl_output::WlOutput, Option<String>)>,
    events: HashMap<u32, StreamEvent>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(state: &mut Self, registry: &wl_registry::WlRegistry, event: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "zkde_screencast_unstable_v1" => state.screencast = Some(registry.bind(name, version.min(3), qh, ())),
                // Version 4 : l'événement `name` donne le connecteur (« HDMI-A-1 »).
                "wl_output" => state.outputs.push((registry.bind(name, version.min(4), qh, ()), None)),
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(state: &mut Self, output: &wl_output::WlOutput, event: wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name } = event
            && let Some(slot) = state.outputs.iter_mut().find(|(o, _)| o == output)
        {
            slot.1 = Some(name);
        }
    }
}

impl Dispatch<ZkdeScreencastUnstableV1, ()> for State {
    fn event(_: &mut Self, _: &ZkdeScreencastUnstableV1, _: <ZkdeScreencastUnstableV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZkdeScreencastStreamUnstableV1, ()> for State {
    fn event(state: &mut Self, stream: &ZkdeScreencastStreamUnstableV1, event: zstream::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let id = stream.id().protocol_id();
        let ev = match event {
            zstream::Event::Created { node } => StreamEvent::Created(node),
            zstream::Event::Failed { error } => StreamEvent::Failed(error),
            zstream::Event::Closed => StreamEvent::Closed,
        };
        state.events.insert(id, ev);
    }
}

pub struct Kde {
    queue: EventQueue<State>,
    state: State,
    streams: HashMap<u32, ZkdeScreencastStreamUnstableV1>,
    _conn: Connection,
}

impl Kde {
    /// Connexion au compositeur ; échoue si KWin ne donne pas le protocole.
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env().map_err(err("connexion Wayland"))?;
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());
        let mut state = State::default();
        // Deux allers-retours : les objets, puis leurs premiers événements (noms).
        queue.roundtrip(&mut state).map_err(err("Wayland"))?;
        queue.roundtrip(&mut state).map_err(err("Wayland"))?;
        if state.screencast.is_none() {
            return Err("KWin ne donne pas zkde_screencast_unstable_v1 à cet agent (fichier .desktop de la Station absent ?)".into());
        }
        Ok(Self { queue, state, streams: HashMap::new(), _conn: conn })
    }

    /// Attend le flux PipeWire du `stream` (5 s au plus).
    fn wait(&mut self, stream: ZkdeScreencastStreamUnstableV1) -> Result<Cast> {
        let id = stream.id().protocol_id();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.queue.roundtrip(&mut self.state).map_err(err("Wayland"))?;
            match self.state.events.remove(&id) {
                Some(StreamEvent::Created(node)) => {
                    self.streams.insert(id, stream);
                    return Ok(Cast { key: CastKey::Kde(id), node, serial: serial_of(node) });
                }
                Some(StreamEvent::Failed(e)) => {
                    stream.close();
                    return Err(format!("KWin refuse le flux : {e}"));
                }
                Some(StreamEvent::Closed) => {
                    stream.close();
                    return Err("KWin a fermé le flux".into());
                }
                None if Instant::now() > deadline => {
                    stream.close();
                    return Err("KWin n'a pas donné de flux PipeWire en 5 s".into());
                }
                None => std::thread::sleep(Duration::from_millis(30)),
            }
        }
    }

    fn screencast(&self) -> Result<&ZkdeScreencastUnstableV1> {
        self.state.screencast.as_ref().ok_or_else(|| "zkde_screencast absent".to_string())
    }
}

impl Recorder for Kde {
    fn virtual_output(&mut self, slot: usize, width: u32, height: u32, hidpi: bool) -> Result<(Cast, Option<String>)> {
        let scale = if hidpi { 2.0 } else { 1.0 };
        let name = format!("{VIRTUAL_PREFIX}{}", slot + 1);
        let qh = self.queue.handle();
        let stream = self.screencast()?.stream_virtual_output(
            name.clone(),
            (f64::from(width) / scale).round() as i32,
            (f64::from(height) / scale).round() as i32,
            scale,
            POINTER_HIDDEN,
            &qh,
            (),
        );
        Ok((self.wait(stream)?, Some(name)))
    }

    fn monitor(&mut self, connector: &str) -> Result<Cast> {
        let _ = self.queue.roundtrip(&mut self.state);
        let output = self
            .state
            .outputs
            .iter()
            .find(|(_, n)| n.as_deref() == Some(connector))
            .map(|(o, _)| o.clone())
            .ok_or_else(|| format!("écran {connector} inconnu de KWin"))?;
        let qh = self.queue.handle();
        let stream = self.screencast()?.stream_output(&output, POINTER_HIDDEN, &qh, ());
        self.wait(stream)
    }

    fn stop(&mut self, cast: &Cast) {
        if let CastKey::Kde(id) = cast.key
            && let Some(stream) = self.streams.remove(&id)
        {
            stream.close();
            let _ = self.queue.roundtrip(&mut self.state);
        }
    }

    fn pump(&mut self) {
        let _ = self.queue.roundtrip(&mut self.state);
    }
}

/// Disposition par `kscreen-doctor` (Plasma 5.27 et 6).
pub struct KdeDesk;

/// Une ligne `Output:` de `kscreen-doctor -o`, sans ses couleurs.
pub fn parse_outputs(text: &str) -> Vec<Mon> {
    let plain: String = {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    };
    plain
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.first() != Some(&"Output:") {
                return None;
            }
            let name = words.get(2)?.to_string();
            let after = |key: &str| words.iter().position(|w| *w == key).and_then(|i| words.get(i + 1)).copied();
            let enabled = words.contains(&"enabled");
            let priority: u32 = after("priority").and_then(|p| p.parse().ok()).unwrap_or(0);
            // Mode courant : « 0:1920x1080@60*! » (l'astérisque).
            let (width, height, refresh) = words
                .iter()
                .find(|w| w.contains('*') && w.contains('x') && w.contains('@'))
                .and_then(|w| {
                    let mode = w.split(':').nth(1)?.trim_end_matches(['*', '!']);
                    let (size, hz) = mode.split_once('@')?;
                    let (w, h) = size.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?, hz.parse::<f64>().ok()?.round() as u32))
                })
                .unwrap_or((0, 0, 60));
            let (x, y) = after("Geometry:")
                .and_then(|p| p.split_once(','))
                .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
                .unwrap_or((0, 0));
            let scale = after("Scale:").and_then(|s| s.parse().ok()).unwrap_or(1.0);
            Some(Mon {
                virt: name.starts_with(VIRTUAL_PREFIX),
                connector: name,
                enabled,
                x,
                y,
                width,
                height,
                refresh,
                scale,
                primary: enabled && priority == 1,
                scales: vec![1.0, 1.25, 1.5, 1.75, 2.0],
            })
        })
        .collect()
}

impl KdeDesk {
    fn doctor(args: &[String]) -> Result<String> {
        let out = Command::new("kscreen-doctor").args(args).output().map_err(err("kscreen-doctor"))?;
        if !out.status.success() {
            return Err(format!("kscreen-doctor : {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl Desk for KdeDesk {
    fn name(&self) -> &'static str {
        "KDE"
    }

    fn monitors(&self) -> Result<Vec<Mon>> {
        Ok(parse_outputs(&Self::doctor(&["-o".into()])?))
    }

    fn physical_units(&self) -> Result<bool> {
        Ok(false)
    }

    fn apply(&self, places: &[Place]) -> Result<()> {
        let mons = self.monitors()?;
        let mut args = Vec::new();
        // Priorité : 1 pour le principal, puis dans l'ordre de la disposition.
        let mut rank = 1;
        for p in places.iter().filter(|p| p.primary).chain(places.iter().filter(|p| !p.primary)) {
            let o = format!("output.{}", p.connector);
            args.push(format!("{o}.enable"));
            args.push(format!("{o}.position.{},{}", p.x, p.y));
            args.push(format!("{o}.scale.{}", p.scale));
            args.push(format!("{o}.priority.{rank}"));
            rank += 1;
        }
        for m in mons.iter().filter(|m| m.enabled && !places.iter().any(|p| p.connector == m.connector)) {
            args.push(format!("output.{}.disable", m.connector));
        }
        if args.is_empty() {
            return Ok(());
        }
        Self::doctor(&args).map(|_| ()).inspect_err(|e| log(&format!("disposition KDE : {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_outputs;

    #[test]
    fn reads_kscreen_doctor() {
        let text = "\u{1b}[01;32mOutput: \u{1b}[0;0m1 Virtual-0 \u{1b}[01;32menabled\u{1b}[0;0m \u{1b}[01;32mconnected\u{1b}[0;0m \u{1b}[01;32mpriority 1\u{1b}[0;0m \u{1b}[01;33mUnknown\u{1b}[01;34m Modes: \u{1b}[0;0m0:\u{1b}[01;32m1920x1080@60*\u{1b}[0;0m! \u{1b}[01;33mGeometry: \u{1b}[0;0m0,0 1920x1080 \u{1b}[01;33mScale: \u{1b}[0;0m1 \u{1b}[01;33mRotation: \u{1b}[0;0m1\n\
Output: 2 KyberNow-1 enabled connected priority 2 Modes: 0:2560x1440@60*! Geometry: 1920,0 1280x720 Scale: 2 Rotation: 1\n\
Output: 3 HDMI-A-1 disabled connected Modes: 0:3840x2160@120*! Geometry: 0,0 3840x2160 Scale: 1.5\n";
        let m = parse_outputs(text);
        assert_eq!(m.len(), 3);
        assert_eq!((m[0].connector.as_str(), m[0].enabled, m[0].primary, m[0].width, m[0].height), ("Virtual-0", true, true, 1920, 1080));
        assert!(m[1].virt && !m[1].primary);
        assert_eq!((m[1].x, m[1].y, m[1].scale), (1920, 0, 2.0));
        assert_eq!((m[2].enabled, m[2].refresh, m[2].scale), (false, 120, 1.5));
    }
}
