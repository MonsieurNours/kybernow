// kybernow-display : écrans de KyberNow Station pour Linux (bureau GNOME,
// docs/station-linux.md), pilotés par Admin (`display/linux.rs`).
//
// Même protocole que l'agent du Mac (server/macos/display-agent) : une
// requête JSON par ligne sur l'entrée standard, une réponse JSON par ligne
// sur la sortie, {"ok": true, ...} ou {"ok": false, "error": "..."}.
//
//   create     {"slot", "width", "height", "refresh", "hidpi"}  → {"id"}
//   remove     {"slot"}
//   isolate    {"screens": [{"id", "x", "y", "width", "height", "refresh", "hidpi"}], "keep_others"}
//              → {"screens": [{"id", "width", "height", "refresh", "x", "y"} | null], "disabled_others"}
//   restore    rend la disposition des écrans de l'ordinateur
//   state      {"ids"} → {"modes": [{"width", "height", "refresh"}], "displays", "others"}
//   set_mode   {"id", "width", "height", "refresh", "hidpi"} → {"width", "height", "refresh"}
//   displays   → {"displays": [{"id", "x", "y", "width", "height", "refresh", "primary"}]}
//   set_hdr    {"id", "on"} → {"supported", "before", "active"}
//   hdr        {"id"} → {"supported", "active"}
//   ping
//
// Tout passe par Mutter, le compositeur de GNOME, sur le bus de la session :
//
// - Écrans virtuels : `org.gnome.Mutter.ScreenCast`, `RecordVirtual` (une
//   session de capture par écran : Mutter ne démarre pas un flux ajouté
//   après `Start`). Mutter crée l'écran quand un lecteur se branche au flux
//   PipeWire, à la taille que ce lecteur négocie : c'est la capture de Kyber
//   (pipewiresrc), qui demande la définition de l'appareil.
// - Écrans de l'ordinateur : la même API, `RecordMonitor`, sans le dialogue
//   du portail de partage d'écran (la Station n'est pas dans un bac à
//   sable) ; enregistrés seulement quand il n'y a pas d'écran virtuel.
// - Kyber lit la liste des écrans dans un fichier TOML que l'agent tient à
//   jour (`[[video_sources]]`, correctif Kyber Linux 0001) : identifiant,
//   nœud PipeWire, taille, place sur le bureau.
// - Disposition : `org.gnome.Mutter.DisplayConfig`, `ApplyMonitorsConfig`
//   en mode temporaire (jamais enregistré). Pendant la session, seuls les
//   écrans virtuels restent sur le bureau, le principal en premier, comme
//   sous Windows ; les écrans de l'ordinateur reviennent à la fin, avec la
//   disposition d'avant. Les écrans virtuels n'apparaissent qu'avec le
//   premier flux : la disposition attend qu'ils soient là (un fil à part,
//   tant qu'elle n'est pas appliquée).
//
// Filet de sécurité : les écrans virtuels vivent avec la connexion au bus de
// ce processus ; s'il s'arrête, Mutter les retire et revient à la
// disposition enregistrée de l'utilisateur.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value as ZValue};

const SC_DEST: &str = "org.gnome.Mutter.ScreenCast";
const SC_PATH: &str = "/org/gnome/Mutter/ScreenCast";
const SC_SESSION: &str = "org.gnome.Mutter.ScreenCast.Session";
const SC_STREAM: &str = "org.gnome.Mutter.ScreenCast.Stream";
const DC_DEST: &str = "org.gnome.Mutter.DisplayConfig";
const DC_PATH: &str = "/org/gnome/Mutter/DisplayConfig";

/// Identifiant Kyber d'un écran virtuel : 101 pour le principal, 102…
const VIRTUAL_ID_BASE: u32 = 101;
/// Écrans virtuels au plus (comme sous Windows et sur Mac).
const MAX_SCREENS: usize = 4;
/// Dès cette largeur, l'écran virtuel est à l'échelle 2 (interface lisible
/// sur un téléphone, une tablette ou un téléviseur 4K), comme sur Mac.
const HIDPI_FROM_WIDTH: u32 = 2048;

type Result<T> = std::result::Result<T, String>;
type Props = HashMap<String, OwnedValue>;
type Spec = (String, String, String, String);
type ModeT = (String, i32, i32, f64, f64, Vec<f64>, Props);
type MonitorT = (Spec, Vec<ModeT>, Props);
type LogicalT = (i32, i32, f64, u32, bool, Vec<Spec>, Props);
type StateT = (u32, Vec<MonitorT>, Vec<LogicalT>, Props);

fn err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{what} : {e}")
}

fn log(msg: &str) {
    eprintln!("kybernow-display : {msg}");
}

/// Une session de capture de Mutter et le nœud PipeWire de son flux.
struct Cast {
    session: OwnedObjectPath,
    node: u32,
    /// Numéro de série PipeWire du nœud, pour `pipewiresrc target-object`.
    serial: String,
}

struct Virtual {
    cast: Cast,
    width: u32,
    height: u32,
    refresh: u32,
    hidpi: bool,
    /// Place sur le bureau (unités de GNOME), une fois la disposition faite.
    place: (i32, i32),
}

struct Physical {
    id: u32,
    name: String,
    cast: Cast,
    width: u32,
    height: u32,
    place: (i32, i32),
}

/// Disposition voulue pendant la session (écrans virtuels, dans l'ordre).
#[derive(Clone)]
struct Layout {
    screens: Vec<LayoutScreen>,
    keep_others: bool,
}

#[derive(Clone)]
struct LayoutScreen {
    slot: usize,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    hidpi: bool,
}

#[derive(Default)]
struct Shared {
    /// Disposition à appliquer dès que les écrans virtuels sont là.
    pending: Option<Layout>,
    /// Disposition appliquée (la même, une fois en place).
    applied: Option<Layout>,
    /// Disposition des écrans de l'ordinateur d'avant la session.
    saved: Option<Vec<LogicalT>>,
    /// Connecteur de chaque écran virtuel, une fois vu par GNOME (« Meta-0 »).
    connectors: [Option<String>; MAX_SCREENS],
    /// Place de chaque écran virtuel sur le bureau.
    places: [Option<(i32, i32)>; MAX_SCREENS],
    /// Le fichier des sources est à réécrire (places changées).
    dirty: bool,
}

struct Agent {
    conn: Connection,
    virtuals: Vec<Option<Virtual>>,
    physicals: Vec<Physical>,
    shared: Arc<Mutex<Shared>>,
    file: PathBuf,
}

fn screens_file() -> PathBuf {
    if let Some(p) = std::env::var_os("KYBERNOW_SCREENS_FILE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    data.join("kybernow/station/kyber-screens.toml")
}

fn dc(conn: &Connection) -> Result<Proxy<'static>> {
    Proxy::new(conn, DC_DEST, DC_PATH, DC_DEST).map_err(err("DisplayConfig"))
}

fn current_state(conn: &Connection) -> Result<StateT> {
    dc(conn)?.call("GetCurrentState", &()).map_err(err("GetCurrentState"))
}

fn prop_bool(props: &Props, key: &str) -> bool {
    props.get(key).and_then(|v| bool::try_from(v).ok()).unwrap_or(false)
}

fn current_mode(monitor: &MonitorT) -> Option<&ModeT> {
    monitor.1.iter().find(|m| prop_bool(&m.6, "is-current"))
}

fn is_virtual(connector: &str) -> bool {
    connector.starts_with("Meta-")
}

/// Échelle la plus proche de `want` parmi celles que GNOME propose pour ce mode.
fn nearest_scale(mode: &ModeT, want: f64) -> f64 {
    mode.5.iter().copied().min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs())).unwrap_or(1.0)
}

fn record(conn: &Connection, connector: Option<&str>) -> Result<Cast> {
    let sc = Proxy::new(conn, SC_DEST, SC_PATH, SC_DEST).map_err(err("ScreenCast"))?;
    let none: HashMap<&str, ZValue> = HashMap::new();
    let session: OwnedObjectPath = sc.call("CreateSession", &(none,)).map_err(err("CreateSession"))?;
    let sp = Proxy::new(conn, SC_DEST, session.clone(), SC_SESSION).map_err(err("session"))?;
    // Le pointeur reste hors de l'image : chaque appareil dessine le sien.
    let mut opts: HashMap<&str, ZValue> = HashMap::new();
    opts.insert("cursor-mode", ZValue::U32(0));
    let stream: OwnedObjectPath = match connector {
        Some(c) => sp.call("RecordMonitor", &(c, opts)).map_err(err("RecordMonitor"))?,
        None => {
            // Écran « de plateforme » : GNOME y met sa barre et ses fenêtres
            // comme sur un vrai écran (Mutter 44 et après ; sinon sans).
            let mut platform = opts.clone();
            platform.insert("is-platform", ZValue::Bool(true));
            match sp.call("RecordVirtual", &(platform,)) {
                Ok(s) => s,
                Err(_) => sp.call("RecordVirtual", &(opts,)).map_err(err("RecordVirtual"))?,
            }
        }
    };
    let st = Proxy::new(conn, SC_DEST, stream, SC_STREAM).map_err(err("flux"))?;
    let mut added = st.receive_signal("PipeWireStreamAdded").map_err(err("PipeWireStreamAdded"))?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let node = added.next().and_then(|m| m.body().deserialize::<u32>().ok());
        let _ = tx.send(node);
    });
    sp.call::<_, _, ()>("Start", &()).map_err(err("Start"))?;
    let node = rx
        .recv_timeout(Duration::from_secs(5))
        .ok()
        .flatten()
        .ok_or_else(|| "Mutter n'a pas donné de flux PipeWire en 5 s".to_string())?;
    Ok(Cast { session, node, serial: serial_of(node) })
}

/// Numéro de série PipeWire (`object.serial`) du nœud `node` : c'est lui
/// que `pipewiresrc` attend (`target-object`) ; l'identifiant que donne
/// Mutter peut désigner un autre nœud une fois les identifiants réutilisés.
/// Lu par `pw-dump` ; sans lui, l'identifiant.
fn serial_of(node: u32) -> String {
    let out = std::process::Command::new("pw-dump").arg(node.to_string()).output();
    let serial = out.ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok()).and_then(|v| {
        v.as_array()?.iter().find(|o| o["id"].as_u64() == Some(u64::from(node))).and_then(|o| {
            let s = &o["info"]["props"]["object.serial"];
            s.as_u64().map(|n| n.to_string()).or_else(|| s.as_str().map(str::to_string))
        })
    });
    serial.unwrap_or_else(|| {
        log(&format!("numéro de série du nœud PipeWire {node} introuvable (pw-dump) : identifiant gardé"));
        node.to_string()
    })
}

fn stop(conn: &Connection, cast: &Cast) {
    if let Ok(sp) = Proxy::new(conn, SC_DEST, cast.session.clone(), SC_SESSION) {
        let _ = sp.call::<_, _, ()>("Stop", &());
    }
}

/// Applique `layout` si tous ses écrans virtuels sont visibles pour GNOME.
/// Rend vrai une fois appliquée.
fn apply_layout(conn: &Connection, shared: &Mutex<Shared>, layout: &Layout) -> Result<bool> {
    let (serial, monitors, logical, props) = current_state(conn)?;
    // Disposition « physique » (`layout-mode` 2, celle des écrans à
    // échelles fractionnaires) : places en pixels ; « logique » (1) : en
    // pixels divisés par l'échelle de l'écran voisin.
    let physical = props.get("layout-mode").and_then(|v| u32::try_from(v).ok()) == Some(2);
    let mut sh = shared.lock().unwrap();
    // Connecteurs des écrans virtuels : ceux déjà attribués, puis les
    // nouveaux « Meta-* » par taille, dans l'ordre des rangs.
    let virtuals: Vec<&MonitorT> = monitors.iter().filter(|m| is_virtual(&m.0.0)).collect();
    for ls in &layout.screens {
        let taken: Vec<String> = sh.connectors.iter().flatten().cloned().collect();
        if sh.connectors[ls.slot].as_ref().is_some_and(|c| virtuals.iter().any(|m| &m.0.0 == c)) {
            continue;
        }
        let found = virtuals.iter().find(|m| {
            !taken.contains(&m.0.0)
                && current_mode(m).or(m.1.first()).is_some_and(|md| md.1 as u32 == ls.width && md.2 as u32 == ls.height)
        });
        let found = found.or_else(|| virtuals.iter().find(|m| !taken.contains(&m.0.0)));
        sh.connectors[ls.slot] = found.map(|m| m.0.0.clone());
    }
    if layout.screens.iter().any(|ls| sh.connectors[ls.slot].is_none()) {
        return Ok(false);
    }
    if sh.saved.is_none() {
        sh.saved = Some(logical.iter().filter(|l| l.5.iter().all(|s| !is_virtual(&s.0))).cloned().collect());
    }
    // Écrans virtuels : à leur place (pixels de l'appareil ramenés à
    // l'échelle de GNOME), le principal en premier et principal.
    let mut out: Vec<(i32, i32, f64, u32, bool, Vec<(String, String, HashMap<String, ZValue>)>)> = Vec::new();
    let mut chosen = Vec::new();
    for ls in &layout.screens {
        let connector = sh.connectors[ls.slot].clone().unwrap();
        let monitor = virtuals.iter().find(|m| m.0.0 == connector).unwrap();
        let mode = current_mode(monitor).or(monitor.1.first()).ok_or("écran virtuel sans mode")?;
        let scale = nearest_scale(mode, if ls.hidpi { 2.0 } else { 1.0 });
        chosen.push((ls, connector, mode.0.clone(), scale));
    }
    // En disposition logique, une place en pixels se ramène à l'échelle de
    // l'écran qu'elle touche (à gauche ou au-dessus), sinon à la sienne.
    let to_units = |v: i32, own: f64, edge: &dyn Fn(&LayoutScreen) -> i32| -> i32 {
        if physical || v == 0 {
            return v;
        }
        let touching = chosen.iter().find(|(o, ..)| edge(o) == v).map_or(own, |c| c.3);
        (v as f64 / touching).round() as i32
    };
    let mut right = 0;
    for (i, (ls, connector, mode_id, scale)) in chosen.iter().enumerate() {
        let x = to_units(ls.x, *scale, &|o: &LayoutScreen| o.x + o.width as i32);
        let y = to_units(ls.y, *scale, &|o: &LayoutScreen| o.y + o.height as i32);
        let width = if physical { ls.width as i32 } else { (ls.width as f64 / scale).round() as i32 };
        right = right.max(x + width);
        sh.places[ls.slot] = Some((x, y));
        out.push((x, y, *scale, 0, i == 0, vec![(connector.clone(), mode_id.clone(), HashMap::new())]));
    }
    // Prêt de l'ordinateur : les écrans de l'ordinateur restent, à droite.
    let mut kept = 0;
    if layout.keep_others {
        let left = logical.iter().filter(|l| l.5.iter().all(|s| !is_virtual(&s.0))).map(|l| l.0).min().unwrap_or(0);
        for l in logical.iter().filter(|l| l.5.iter().all(|s| !is_virtual(&s.0))) {
            let specs = l
                .5
                .iter()
                .filter_map(|s| {
                    let m = monitors.iter().find(|m| m.0 == *s)?;
                    Some((s.0.clone(), current_mode(m)?.0.clone(), HashMap::new()))
                })
                .collect::<Vec<_>>();
            if !specs.is_empty() {
                kept += 1;
                out.push((right + l.0 - left, l.1, l.2, l.3, false, specs));
            }
        }
    }
    let none: HashMap<&str, ZValue> = HashMap::new();
    dc(conn)?
        .call::<_, _, ()>("ApplyMonitorsConfig", &(serial, 1u32, out, none))
        .map_err(err("ApplyMonitorsConfig"))?;
    sh.dirty = true;
    log(&format!("disposition de la session appliquée ({} écran(s) virtuel(s), {kept} de l'ordinateur gardé(s))", layout.screens.len()));
    Ok(true)
}

/// Remet la disposition d'avant la session (mode temporaire, comme
/// `isolate` ; GNOME garde la sienne, enregistrée, pour la suite).
fn apply_saved(conn: &Connection, saved: &[LogicalT]) -> Result<()> {
    let (serial, monitors, _, _) = current_state(conn)?;
    let out: Vec<(i32, i32, f64, u32, bool, Vec<(String, String, HashMap<String, ZValue>)>)> = saved
        .iter()
        .filter_map(|l| {
            let specs: Vec<_> = l
                .5
                .iter()
                .filter_map(|s| {
                    let m = monitors.iter().find(|m| m.0 == *s)?;
                    let mode = current_mode(m).or_else(|| m.1.iter().find(|md| prop_bool(&md.6, "is-preferred")))?;
                    Some((s.0.clone(), mode.0.clone(), HashMap::new()))
                })
                .collect();
            (!specs.is_empty()).then(|| (l.0, l.1, l.2, l.3, l.4, specs))
        })
        .collect();
    if out.is_empty() {
        return Ok(());
    }
    let none: HashMap<&str, ZValue> = HashMap::new();
    dc(conn)?
        .call::<_, _, ()>("ApplyMonitorsConfig", &(serial, 1u32, out, none))
        .map_err(err("ApplyMonitorsConfig"))
}

impl Agent {
    fn new() -> Result<Self> {
        let conn = Connection::session().map_err(err("bus de la session"))?;
        let agent = Self {
            conn,
            virtuals: (0..MAX_SCREENS).map(|_| None).collect(),
            physicals: Vec::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
            file: screens_file(),
        };
        Ok(agent)
    }

    /// Fil de la disposition : applique celle qui attend ses écrans, et
    /// réécrit le fichier des sources quand les places changent.
    fn spawn_layout_thread(&self) -> Result<()> {
        let conn = Connection::session().map_err(err("bus de la session"))?;
        let shared = Arc::clone(&self.shared);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let pending = shared.lock().unwrap().pending.clone();
                if let Some(layout) = pending {
                    match apply_layout(&conn, &shared, &layout) {
                        Ok(true) => {
                            let mut sh = shared.lock().unwrap();
                            sh.pending = None;
                            sh.applied = Some(layout);
                        }
                        Ok(false) => {}
                        Err(e) => log(&format!("disposition : {e}")),
                    }
                }
            }
        });
        Ok(())
    }

    fn handle(&mut self, req: &Value) -> Result<Value> {
        match req["cmd"].as_str() {
            Some("create") => self.create(req),
            Some("remove") => {
                let slot = uint(req, "slot")? as usize;
                self.remove(slot);
                Ok(json!({}))
            }
            Some("isolate") => self.isolate(req),
            Some("restore") => Ok(self.restore()),
            Some("state") => self.state(req),
            Some("set_mode") => self.set_mode(req),
            Some("displays") => Ok(json!({ "displays": self.physical_displays()? })),
            Some("set_hdr") => Ok(json!({ "supported": false, "before": false, "active": false })),
            Some("hdr") => Ok(json!({ "supported": false, "active": false })),
            Some("ping") => {
                self.ensure_sources()?;
                Ok(json!({}))
            }
            _ => Err("requête inconnue".into()),
        }
    }

    fn create(&mut self, req: &Value) -> Result<Value> {
        let slot = uint(req, "slot")? as usize;
        if slot >= MAX_SCREENS {
            return Err(format!("rang d'écran hors limites : {slot}"));
        }
        let (width, height, refresh) = (uint(req, "width")?, uint(req, "height")?, uint(req, "refresh")?);
        let hidpi = req["hidpi"].as_bool().unwrap_or(width >= HIDPI_FROM_WIDTH);
        self.remove_virtual(slot);
        // Les écrans de l'ordinateur ne sont plus capturés pendant la session.
        self.stop_physicals();
        let cast = record(&self.conn, None)?;
        log(&format!("écran virtuel {} : {width} × {height} à {refresh} Hz, flux PipeWire {} (série {})", slot + 1, cast.node, cast.serial));
        self.virtuals[slot] = Some(Virtual { cast, width, height, refresh, hidpi, place: (0, 0) });
        self.write_sources();
        Ok(json!({ "id": VIRTUAL_ID_BASE + slot as u32 }))
    }

    fn remove_virtual(&mut self, slot: usize) {
        if let Some(v) = self.virtuals.get_mut(slot).and_then(Option::take) {
            stop(&self.conn, &v.cast);
            let mut sh = self.shared.lock().unwrap();
            sh.connectors[slot] = None;
            sh.places[slot] = None;
        }
    }

    fn remove(&mut self, slot: usize) {
        self.remove_virtual(slot);
        if self.virtuals.iter().all(Option::is_none) {
            let _ = self.restore();
        } else {
            self.write_sources();
        }
    }

    fn isolate(&mut self, req: &Value) -> Result<Value> {
        let keep_others = req["keep_others"].as_bool().unwrap_or(false);
        let mut screens = Vec::new();
        let mut placed = Vec::new();
        for s in req["screens"].as_array().cloned().unwrap_or_default() {
            let id = uint(&s, "id")?;
            let Some(slot) = id.checked_sub(VIRTUAL_ID_BASE).map(|s| s as usize).filter(|s| *s < MAX_SCREENS) else {
                placed.push(Value::Null);
                continue;
            };
            let Some(v) = self.virtuals[slot].as_ref() else {
                placed.push(Value::Null);
                continue;
            };
            let (x, y) = (s["x"].as_i64().unwrap_or(0) as i32, s["y"].as_i64().unwrap_or(0) as i32);
            screens.push(LayoutScreen { slot, width: v.width, height: v.height, x, y, hidpi: v.hidpi });
            placed.push(json!({ "id": id, "width": v.width, "height": v.height, "refresh": v.refresh, "x": x, "y": y }));
        }
        let layout = Layout { screens, keep_others };
        // Compté avant la disposition, qui les retire du bureau.
        let disabled = if keep_others { 0 } else { self.physical_count() };
        // Déjà visibles (changement de disposition en cours de session) :
        // tout de suite ; sinon, dès que le premier flux les fait apparaître.
        match apply_layout(&self.conn, &self.shared, &layout) {
            Ok(true) => {
                let mut sh = self.shared.lock().unwrap();
                sh.pending = None;
                sh.applied = Some(layout);
            }
            Ok(false) => self.shared.lock().unwrap().pending = Some(layout),
            Err(e) => {
                log(&format!("disposition : {e}"));
                self.shared.lock().unwrap().pending = Some(layout);
            }
        }
        self.sync_places();
        Ok(json!({ "screens": placed, "disabled_others": disabled }))
    }

    fn restore(&mut self) -> Value {
        let saved = {
            let mut sh = self.shared.lock().unwrap();
            sh.pending = None;
            sh.applied = None;
            sh.saved.take()
        };
        if let Some(saved) = saved {
            // Un écran virtuel qui vient de partir change encore la
            // configuration de GNOME un instant : « stale information ».
            // On réessaie avec l'état suivant (une seconde au plus).
            let mut tries = 0;
            loop {
                match apply_saved(&self.conn, &saved) {
                    Ok(()) => break,
                    Err(e) if e.contains("stale") && tries < 5 => {
                        tries += 1;
                        std::thread::sleep(Duration::from_millis(200));
                    }
                    Err(e) => {
                        log(&format!("disposition d'avant la session : {e}"));
                        break;
                    }
                }
            }
        }
        if self.virtuals.iter().all(Option::is_none) {
            // Sans session, Kyber capture les écrans de l'ordinateur.
            if let Err(e) = self.ensure_sources() {
                log(&format!("écrans de l'ordinateur : {e}"));
            }
        }
        json!({ "exit": false })
    }

    fn state(&mut self, req: &Value) -> Result<Value> {
        self.sync_places();
        let (_, monitors, logical, _) = current_state(&self.conn)?;
        let sh = self.shared.lock().unwrap();
        let modes: Vec<Value> = req["ids"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|id| {
                let id = id.as_u64().unwrap_or(0) as u32;
                let slot = id.wrapping_sub(VIRTUAL_ID_BASE) as usize;
                let Some(v) = self.virtuals.get(slot).and_then(Option::as_ref) else {
                    return json!({ "width": 0, "height": 0, "refresh": 0 });
                };
                // Pas encore vu par GNOME (pas de flux) : le mode demandé.
                let shown = sh.connectors[slot]
                    .as_ref()
                    .and_then(|c| monitors.iter().find(|m| &m.0.0 == c))
                    .and_then(current_mode);
                match shown {
                    Some(m) => json!({ "width": m.1, "height": m.2, "refresh": m.3.round() as u32 }),
                    None => json!({ "width": v.width, "height": v.height, "refresh": v.refresh }),
                }
            })
            .collect();
        let others = logical.iter().filter(|l| l.5.iter().all(|s| !is_virtual(&s.0))).count();
        // Disposition en attente : les écrans de l'ordinateur s'éteindront
        // avec elle, Admin n'a rien à réparer.
        let others = if sh.pending.is_some() { 0 } else { others };
        Ok(json!({ "modes": modes, "displays": logical.len(), "others": others }))
    }

    fn set_mode(&mut self, req: &Value) -> Result<Value> {
        let id = uint(req, "id")?;
        let slot = id.wrapping_sub(VIRTUAL_ID_BASE) as usize;
        let (width, height, refresh) = (uint(req, "width")?, uint(req, "height")?, uint(req, "refresh")?);
        let v = self.virtuals.get_mut(slot).and_then(Option::as_mut).ok_or("écran virtuel inconnu")?;
        // La taille d'un écran virtuel de GNOME est celle que la capture
        // négocie : la nouvelle taille va au fichier des sources, que Kyber
        // relit (son flux suivant la demande, et GNOME suit).
        v.width = width;
        v.height = height;
        v.refresh = refresh;
        if let Some(h) = req["hidpi"].as_bool() {
            v.hidpi = h;
        }
        self.write_sources();
        Ok(json!({ "width": width, "height": height, "refresh": refresh }))
    }

    fn physical_count(&self) -> usize {
        current_state(&self.conn)
            .map(|(_, _, logical, _)| logical.iter().filter(|l| l.5.iter().all(|s| !is_virtual(&s.0))).count())
            .unwrap_or(0)
    }

    /// Écrans de l'ordinateur allumés, en pixels de leur mode, places en
    /// unités du bureau de GNOME.
    fn physical_displays(&self) -> Result<Vec<Value>> {
        let (_, monitors, logical, _) = current_state(&self.conn)?;
        let mut list = Vec::new();
        let mut connectors: Vec<&str> = monitors.iter().map(|m| m.0.0.as_str()).filter(|c| !is_virtual(c)).collect();
        connectors.sort();
        for l in &logical {
            for spec in &l.5 {
                if is_virtual(&spec.0) {
                    continue;
                }
                let Some(m) = monitors.iter().find(|m| m.0 == *spec) else { continue };
                let Some(mode) = current_mode(m) else { continue };
                let id = 1 + connectors.iter().position(|c| *c == spec.0).unwrap_or(0) as u32;
                list.push(json!({ "id": id, "name": spec.0, "x": l.0, "y": l.1, "width": mode.1, "height": mode.2,
                                  "refresh": mode.3.round() as u32, "primary": l.4,
                                  "listed_width": (mode.1 as f64 / l.2).round() as i32,
                                  "listed_height": (mode.2 as f64 / l.2).round() as i32 }));
            }
        }
        Ok(list)
    }

    /// Sans écran virtuel : les écrans de l'ordinateur, capturés pour Kyber
    /// (une session de capture par écran), le fichier des sources à jour.
    fn ensure_sources(&mut self) -> Result<()> {
        if self.virtuals.iter().any(Option::is_some) {
            self.write_sources();
            return Ok(());
        }
        let displays = self.physical_displays()?;
        let same = displays.len() == self.physicals.len()
            && displays.iter().zip(&self.physicals).all(|(d, p)| {
                d["name"].as_str() == Some(p.name.as_str())
                    && d["width"].as_u64() == Some(p.width as u64)
                    && d["height"].as_u64() == Some(p.height as u64)
                    && (d["x"].as_i64().unwrap_or(0) as i32, d["y"].as_i64().unwrap_or(0) as i32) == p.place
            });
        if !same {
            self.stop_physicals();
            for d in &displays {
                let name = d["name"].as_str().unwrap_or_default().to_string();
                match record(&self.conn, Some(&name)) {
                    Ok(cast) => self.physicals.push(Physical {
                        id: d["id"].as_u64().unwrap_or(1) as u32,
                        name,
                        cast,
                        width: d["width"].as_u64().unwrap_or(0) as u32,
                        height: d["height"].as_u64().unwrap_or(0) as u32,
                        place: (d["x"].as_i64().unwrap_or(0) as i32, d["y"].as_i64().unwrap_or(0) as i32),
                    }),
                    Err(e) => log(&format!("capture de l'écran {name} : {e}")),
                }
            }
        }
        self.write_sources();
        Ok(())
    }

    fn stop_physicals(&mut self) {
        for p in self.physicals.drain(..) {
            stop(&self.conn, &p.cast);
        }
    }

    /// Places des écrans virtuels vues par le fil de la disposition.
    fn sync_places(&mut self) {
        let (places, dirty) = {
            let mut sh = self.shared.lock().unwrap();
            let dirty = std::mem::take(&mut sh.dirty);
            (sh.places, dirty)
        };
        for (slot, place) in places.iter().enumerate() {
            if let (Some(v), Some(p)) = (self.virtuals[slot].as_mut(), place) {
                v.place = *p;
            }
        }
        if dirty {
            self.write_sources();
        }
    }

    /// Le fichier des sources de Kyber (correctif Kyber Linux 0001), écrit
    /// en entier puis renommé : Kyber ne lit jamais un fichier à moitié écrit.
    fn write_sources(&self) {
        let framerate = std::env::var("KYBERNOW_SCREEN_FRAMERATE").unwrap_or_else(|_| "0/1".into());
        let mut out = String::from("# Écrans de KyberNow Station (kybernow-display) : ne pas modifier.\n");
        let mut entry = |id: u32, name: &str, serial: &str, w: u32, h: u32, place: (i32, i32)| {
            out.push_str(&format!(
                "\n[[video_sources]]\nid = {id}\nname = \"{name}\"\nsource = \"pipewiresrc target-object={serial} do-timestamp=true keepalive-time=1000 resend-last=true\"\nwidth = {w}\nheight = {h}\nframerate = \"{framerate}\"\nformat = \"BGRx\"\nx = {}\ny = {}\n",
                place.0, place.1
            ));
        };
        if self.virtuals.iter().any(Option::is_some) {
            for (slot, v) in self.virtuals.iter().enumerate() {
                if let Some(v) = v {
                    let name = if slot == 0 { "KyberNow".to_string() } else { format!("KyberNow {}", slot + 1) };
                    entry(VIRTUAL_ID_BASE + slot as u32, &name, &v.cast.serial, v.width, v.height, v.place);
                }
            }
        } else {
            for p in &self.physicals {
                entry(p.id, &p.name, &p.cast.serial, p.width, p.height, p.place);
            }
        }
        let tmp = self.file.with_extension("toml.tmp");
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = std::fs::write(&tmp, out).and_then(|()| std::fs::rename(&tmp, &self.file));
        if let Err(e) = written {
            log(&format!("fichier des écrans {} : {e}", self.file.display()));
        }
    }

    /// Admin s'est arrêté (fin de l'entrée standard) : tout est rendu.
    fn shutdown(&mut self) {
        for slot in 0..MAX_SCREENS {
            self.remove_virtual(slot);
        }
        let _ = self.restore();
        self.stop_physicals();
        let _ = std::fs::remove_file(&self.file);
    }
}

fn uint(v: &Value, key: &str) -> Result<u32> {
    v[key].as_u64().map(|n| n as u32).ok_or_else(|| format!("« {key} » manquant"))
}

fn main() {
    let mut agent = match Agent::new() {
        Ok(a) => a,
        Err(e) => {
            println!("{}", json!({ "ok": false, "error": e }));
            std::process::exit(1);
        }
    };
    if let Err(e) = agent.spawn_layout_thread() {
        log(&e);
    }
    // Kyber a ses écrans dès le lancement (ceux de l'ordinateur).
    let started = Instant::now();
    if let Err(e) = agent.ensure_sources() {
        log(&format!("écrans de l'ordinateur : {e}"));
    }
    log(&format!("prêt en {} ms, fichier des écrans : {}", started.elapsed().as_millis(), agent.file.display()));
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => match agent.handle(&req) {
                Ok(mut body) => {
                    if !body.is_object() {
                        body = json!({});
                    }
                    body["ok"] = json!(true);
                    body
                }
                Err(e) => json!({ "ok": false, "error": e }),
            },
            Err(_) => json!({ "ok": false, "error": "requête illisible" }),
        };
        let _ = writeln!(stdout, "{reply}");
        let _ = stdout.flush();
    }
    agent.shutdown();
}
