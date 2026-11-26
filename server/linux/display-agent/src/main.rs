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
// Trois familles de bureaux, choisies au démarrage
// (`KYBERNOW_DESKTOP=gnome|kde|sway|hyprland` pour forcer) : GNOME, par Mutter
// (`gnome.rs`), KDE Plasma, par KWin (`kde.rs`), et les bureaux wlroots,
// Sway et Hyprland, par leurs écrans sans tête (`wlroots.rs` ; Kyber les
// capture lui-même, `source = "wayland:NOM"`). Le reste est commun :
//
// - Écrans virtuels créés par le compositeur, capturés par PipeWire ; les
//   écrans de l'ordinateur capturés sans dialogue, seulement quand il n'y a
//   pas d'écran virtuel (partage depuis le PC, session sans écran virtuel).
// - Kyber lit la liste des écrans dans un fichier TOML que l'agent tient à
//   jour (`[[video_sources]]`, correctif Kyber Linux 0001) : identifiant,
//   flux PipeWire (son numéro de série), taille, place sur le bureau.
// - Disposition temporaire pendant la session : seuls les écrans virtuels
//   restent sur le bureau, le principal en premier, comme sous Windows ; les
//   écrans de l'ordinateur reviennent à la fin, avec la disposition d'avant.
//   Un écran virtuel n'apparaît parfois qu'avec le premier flux (GNOME) : la
//   disposition attend qu'il soit là (un fil à part).
//
// Filet de sécurité : les écrans virtuels vivent avec la connexion de cet
// agent au compositeur ; s'il s'arrête, le compositeur les retire et revient
// à la disposition enregistrée de l'utilisateur.


mod gnome;
mod kde;
mod wlroots;

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use zbus::zvariant::OwnedObjectPath;

/// Identifiant Kyber d'un écran virtuel : 101 pour le principal, 102…
const VIRTUAL_ID_BASE: u32 = 101;
/// Écrans virtuels au plus (comme sous Windows et sur Mac).
const MAX_SCREENS: usize = 4;
/// Dès cette largeur, l'écran virtuel est à l'échelle 2 (interface lisible
/// sur un téléphone, une tablette ou un téléviseur 4K), comme sur Mac.
const HIDPI_FROM_WIDTH: u32 = 2048;

type Result<T> = std::result::Result<T, String>;

/// Un écran tel que le bureau le montre.
#[derive(Clone, Debug)]
pub struct Mon {
    /// Connecteur (« HDMI-A-1 », « Meta-0 », « KyberNow-1 »).
    pub connector: String,
    /// Écran virtuel de la Station.
    pub virt: bool,
    /// Sur le bureau en ce moment.
    pub enabled: bool,
    /// Place, dans les unités du bureau.
    pub x: i32,
    pub y: i32,
    /// Mode courant, en pixels.
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub scale: f64,
    pub primary: bool,
    /// Échelles que le bureau accepte pour ce mode.
    pub scales: Vec<f64>,
}

/// Un écran dans une disposition à appliquer.
#[derive(Clone, Debug)]
pub struct Place {
    pub connector: String,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub primary: bool,
}

/// Comment rendre une capture : session de Mutter, flux de KWin, ou sortie
/// d'un bureau wlroots (que Kyber capture lui-même, par son nom).
pub enum CastKey {
    Mutter(OwnedObjectPath),
    Kde(u32),
    Wlr { name: String, headless: bool },
}

/// Une capture du compositeur et son flux PipeWire.
pub struct Cast {
    pub key: CastKey,
    pub node: u32,
    /// Numéro de série PipeWire du nœud, pour `pipewiresrc target-object`.
    pub serial: String,
}

/// Création des écrans et des captures (fil principal).
pub trait Recorder {
    /// Un écran virtuel ; son connecteur s'il est connu tout de suite.
    fn virtual_output(&mut self, slot: usize, width: u32, height: u32, hidpi: bool) -> Result<(Cast, Option<String>)>;
    fn monitor(&mut self, connector: &str) -> Result<Cast>;
    fn stop(&mut self, cast: &Cast);
    /// Lit les événements en attente du compositeur.
    fn pump(&mut self) {}
    /// Change le mode d'un écran virtuel quand le compositeur le fait lui-même
    /// (wlroots) ; faux : c'est à la capture de le négocier (GNOME) ou à un
    /// nouvel écran (KDE).
    fn resize(&mut self, _cast: &Cast, _width: u32, _height: u32, _refresh: u32) -> Result<bool> {
        Ok(false)
    }
}

/// Lecture et disposition des écrans (fil principal et fil de la disposition).
pub trait Desk: Send + Sync {
    fn name(&self) -> &'static str;
    fn monitors(&self) -> Result<Vec<Mon>>;
    /// Places en pixels (sinon en points : pixels divisés par l'échelle).
    fn physical_units(&self) -> Result<bool>;
    /// Ces écrans, à ces places ; les autres quittent le bureau.
    fn apply(&self, places: &[Place]) -> Result<()>;
}

fn err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> String + '_ {
    move |e| format!("{what} : {e}")
}

fn log(msg: &str) {
    eprintln!("kybernow-display : {msg}");
}

/// Numéro de série PipeWire (`object.serial`) du nœud `node` : c'est lui
/// que `pipewiresrc` attend (`target-object`) ; l'identifiant que donne le
/// compositeur peut désigner un autre nœud une fois les identifiants
/// réutilisés. Lu par `pw-dump` ; sans lui, l'identifiant.
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

struct Virtual {
    cast: Cast,
    width: u32,
    height: u32,
    refresh: u32,
    hidpi: bool,
    /// Place sur le bureau, une fois la disposition faite.
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
    /// Disposition des écrans de l'ordinateur d'avant la session.
    saved: Option<Vec<Place>>,
    /// Connecteur de chaque écran virtuel (« Meta-0 », « KyberNow-1 »).
    connectors: [Option<String>; MAX_SCREENS],
    /// Place de chaque écran virtuel sur le bureau.
    places: [Option<(i32, i32)>; MAX_SCREENS],
    /// Le fichier des sources est à réécrire (places changées).
    dirty: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Gnome,
    Kde,
    Wlr(wlroots::Flavor),
}

fn new_desk(kind: Kind) -> Result<Arc<dyn Desk>> {
    Ok(match kind {
        Kind::Gnome => Arc::new(gnome::Gnome::new()?),
        Kind::Kde => Arc::new(kde::KdeDesk),
        Kind::Wlr(flavor) => Arc::new(wlroots::WlrDesk(flavor)),
    })
}

/// Le bureau : `KYBERNOW_DESKTOP` s'il le dit, sinon Sway ou Hyprland si la
/// session l'annonce, sinon GNOME si Mutter est là, sinon KDE si KWin donne
/// son protocole de capture.
fn detect() -> Result<(Kind, Box<dyn Recorder>)> {
    let wanted = std::env::var("KYBERNOW_DESKTOP").unwrap_or_default().to_lowercase();
    let wlr = match wanted.as_str() {
        "sway" => Some(wlroots::Flavor::Sway),
        "hyprland" => Some(wlroots::Flavor::Hyprland),
        "" => wlroots::present(),
        _ => None,
    };
    if let Some(flavor) = wlr {
        return Ok((Kind::Wlr(flavor), Box::new(wlroots::Wlr::new(flavor)?)));
    }
    if wanted != "kde" && (wanted == "gnome" || gnome::present()) {
        return Ok((Kind::Gnome, Box::new(gnome::Gnome::new()?)));
    }
    match kde::Kde::new() {
        Ok(k) => Ok((Kind::Kde, Box::new(k))),
        Err(e) if wanted == "kde" => Err(e),
        Err(e) => Err(format!("ni GNOME, ni KDE, ni Sway, ni Hyprland : pas d'écran virtuel ici ({e})")),
    }
}

/// `shown` (le nom que donne le bureau) désigne l'écran créé sous le nom `made`.
fn same_output(shown: &str, made: &str) -> bool {
    shown == made || shown.strip_suffix(made).is_some_and(|p| p.ends_with('-'))
}

/// Échelle la plus proche de `want` parmi celles que le bureau propose.
fn nearest_scale(scales: &[f64], want: f64) -> f64 {
    scales.iter().copied().min_by(|a, b| (a - want).abs().total_cmp(&(b - want).abs())).unwrap_or(1.0)
}

/// Ce qu'il faut ajouter aux places pour que la plus à gauche et la plus haute
/// soient en 0.
fn shift_to_origin(places: impl Iterator<Item = (i32, i32)>) -> (i32, i32) {
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    for (x, y) in places {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
    }
    if min_x == i32::MAX { (0, 0) } else { (-min_x, -min_y) }
}

/// Applique `layout` si tous ses écrans virtuels sont sur le bureau. Rend
/// vrai une fois appliquée.
fn apply_layout(desk: &dyn Desk, shared: &Mutex<Shared>, layout: &Layout) -> Result<bool> {
    // Aucun écran virtuel en place : rien à disposer (sous KDE, une
    // disposition vide désactiverait tous les écrans).
    if layout.screens.is_empty() {
        return Ok(true);
    }
    let mons = desk.monitors()?;
    let physical = desk.physical_units()?;
    let mut sh = shared.lock().unwrap();
    // Connecteurs des écrans virtuels : ceux déjà connus, puis les nouveaux
    // par taille, dans l'ordre des rangs (GNOME les nomme à leur arrivée).
    let virtuals: Vec<&Mon> = mons.iter().filter(|m| m.virt).collect();
    for ls in &layout.screens {
        let taken: Vec<String> = sh.connectors.iter().flatten().cloned().collect();
        // Nom connu à la création : celui du bureau peut porter un préfixe
        // (KWin de Plasma 6 : « Virtual-KyberNow-1 » pour « KyberNow-1 »).
        if let Some(known) = sh.connectors[ls.slot].clone()
            && let Some(m) = virtuals.iter().find(|m| same_output(&m.connector, &known))
        {
            sh.connectors[ls.slot] = Some(m.connector.clone());
            continue;
        }
        let found = virtuals.iter().find(|m| !taken.contains(&m.connector) && m.width == ls.width && m.height == ls.height);
        let found = found.or_else(|| virtuals.iter().find(|m| !taken.contains(&m.connector)));
        sh.connectors[ls.slot] = found.map(|m| m.connector.clone());
    }
    if layout.screens.iter().any(|ls| sh.connectors[ls.slot].is_none()) {
        return Ok(false);
    }
    let others: Vec<&Mon> = mons.iter().filter(|m| !m.virt && m.enabled).collect();
    if sh.saved.is_none() {
        sh.saved = Some(
            others
                .iter()
                .map(|m| Place { connector: m.connector.clone(), x: m.x, y: m.y, scale: m.scale, primary: m.primary })
                .collect(),
        );
    }
    // Écrans virtuels : à leur place (pixels de l'appareil, ramenés en
    // points si le bureau compte en points), le principal en premier.
    let mut chosen = Vec::new();
    for ls in &layout.screens {
        let connector = sh.connectors[ls.slot].clone().unwrap();
        let monitor = virtuals.iter().find(|m| m.connector == connector).unwrap();
        let scale = nearest_scale(&monitor.scales, if ls.hidpi { 2.0 } else { 1.0 });
        chosen.push((ls, connector, scale));
    }
    // Une place en pixels se ramène à l'échelle de l'écran qu'elle touche
    // (à gauche ou au-dessus), sinon à la sienne.
    let to_units = |v: i32, own: f64, edge: &dyn Fn(&LayoutScreen) -> i32| -> i32 {
        if physical || v == 0 {
            return v;
        }
        let touching = chosen.iter().find(|(o, ..)| edge(o) == v).map_or(own, |c| c.2);
        (v as f64 / touching).round() as i32
    };
    let mut placed = Vec::new();
    for (ls, connector, scale) in &chosen {
        let x = to_units(ls.x, *scale, &|o: &LayoutScreen| o.x + o.width as i32);
        let y = to_units(ls.y, *scale, &|o: &LayoutScreen| o.y + o.height as i32);
        let width = if physical { ls.width as i32 } else { (ls.width as f64 / scale).round() as i32 };
        placed.push((ls.slot, connector.clone(), x, y, width, *scale));
    }
    // Le bureau commence en (0, 0) : Mutter refuse une place négative
    // (« Invalid logical monitor position »), que donne un écran de l'appareil
    // à gauche ou au-dessus du principal ; toute la disposition glisse d'autant.
    let (dx, dy) = shift_to_origin(placed.iter().map(|p| (p.2, p.3)));
    let mut places = Vec::new();
    let mut right = 0;
    for (i, (slot, connector, x, y, width, scale)) in placed.into_iter().enumerate() {
        let (x, y) = (x + dx, y + dy);
        right = right.max(x + width);
        sh.places[slot] = Some((x, y));
        places.push(Place { connector, x, y, scale, primary: i == 0 });
    }
    // Prêt de l'ordinateur : les écrans de l'ordinateur restent, à droite.
    if layout.keep_others {
        let left = others.iter().map(|m| m.x).min().unwrap_or(0);
        for m in &others {
            places.push(Place { connector: m.connector.clone(), x: right + m.x - left, y: m.y, scale: m.scale, primary: false });
        }
    }
    desk.apply(&places)?;
    sh.dirty = true;
    let kept = if layout.keep_others { others.len() } else { 0 };
    log(&format!("disposition de la session appliquée ({} écran(s) virtuel(s), {kept} de l'ordinateur gardé(s))", layout.screens.len()));
    Ok(true)
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

struct Agent {
    recorder: Box<dyn Recorder>,
    desk: Arc<dyn Desk>,
    kind: Kind,
    virtuals: Vec<Option<Virtual>>,
    physicals: Vec<Physical>,
    shared: Arc<Mutex<Shared>>,
    file: PathBuf,
}

impl Agent {
    fn new() -> Result<Self> {
        let (kind, recorder) = detect()?;
        Ok(Self {
            recorder,
            desk: new_desk(kind)?,
            kind,
            virtuals: (0..MAX_SCREENS).map(|_| None).collect(),
            physicals: Vec::new(),
            shared: Arc::new(Mutex::new(Shared::default())),
            file: screens_file(),
        })
    }

    /// Fil de la disposition : applique celle qui attend ses écrans.
    fn spawn_layout_thread(&self) -> Result<()> {
        let desk = new_desk(self.kind)?;
        let shared = Arc::clone(&self.shared);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let pending = shared.lock().unwrap().pending.clone();
                if let Some(layout) = pending {
                    match apply_layout(desk.as_ref(), &shared, &layout) {
                        Ok(true) => shared.lock().unwrap().pending = None,
                        Ok(false) => {}
                        Err(e) => log(&format!("disposition : {e}")),
                    }
                }
            }
        });
        Ok(())
    }

    fn handle(&mut self, req: &Value) -> Result<Value> {
        self.recorder.pump();
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
                Ok(json!({ "desktop": self.desk.name() }))
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
        let (cast, connector) = match self.recorder.virtual_output(slot, width, height, hidpi) {
            Ok(made) => made,
            Err(e) => {
                // Kyber garde les écrans de l'ordinateur s'il n'y a pas
                // d'autre écran virtuel.
                if self.virtuals.iter().all(Option::is_none) {
                    let _ = self.ensure_sources();
                }
                return Err(e);
            }
        };
        let capture = match &cast.key {
            CastKey::Wlr { name, .. } => format!("sortie {name}, capturée par Kyber"),
            _ => format!("flux PipeWire {} (série {})", cast.node, cast.serial),
        };
        log(&format!("écran virtuel {} ({}) : {width} × {height} à {refresh} Hz, {capture}", slot + 1, self.desk.name()));
        self.shared.lock().unwrap().connectors[slot] = connector;
        self.virtuals[slot] = Some(Virtual { cast, width, height, refresh, hidpi, place: (0, 0) });
        self.write_sources();
        Ok(json!({ "id": VIRTUAL_ID_BASE + slot as u32 }))
    }

    fn remove_virtual(&mut self, slot: usize) {
        if let Some(v) = self.virtuals.get_mut(slot).and_then(Option::take) {
            self.recorder.stop(&v.cast);
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
        // Déjà visibles : tout de suite ; sinon, dès qu'ils apparaissent.
        match apply_layout(self.desk.as_ref(), &self.shared, &layout) {
            Ok(true) => self.shared.lock().unwrap().pending = None,
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
            sh.saved.take()
        };
        if let Some(saved) = saved
            && let Err(e) = self.desk.apply(&saved)
        {
            log(&format!("disposition d'avant la session : {e}"));
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
        let mons = self.desk.monitors()?;
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
                // Pas encore sur le bureau (pas de flux) : le mode demandé.
                let shown = sh.connectors[slot].as_ref().and_then(|c| mons.iter().find(|m| same_output(&m.connector, c) && m.enabled));
                match shown {
                    Some(m) => json!({ "width": m.width, "height": m.height, "refresh": m.refresh }),
                    None => json!({ "width": v.width, "height": v.height, "refresh": v.refresh }),
                }
            })
            .collect();
        let displays = mons.iter().filter(|m| m.enabled).count();
        let others = mons.iter().filter(|m| m.enabled && !m.virt).count();
        // Disposition en attente : les écrans de l'ordinateur s'éteindront
        // avec elle, Admin n'a rien à réparer.
        let others = if sh.pending.is_some() { 0 } else { others };
        Ok(json!({ "modes": modes, "displays": displays, "others": others }))
    }

    fn set_mode(&mut self, req: &Value) -> Result<Value> {
        let id = uint(req, "id")?;
        let slot = id.wrapping_sub(VIRTUAL_ID_BASE) as usize;
        let (width, height, refresh) = (uint(req, "width")?, uint(req, "height")?, uint(req, "refresh")?);
        let hidpi = req["hidpi"].as_bool();
        if self.kind == Kind::Kde {
            // KWin fixe la taille d'un écran virtuel à sa création : un
            // nouvel écran, au même rang, à la nouvelle taille.
            let hidpi = hidpi.or(self.virtuals.get(slot).and_then(Option::as_ref).map(|v| v.hidpi)).unwrap_or(width >= HIDPI_FROM_WIDTH);
            self.create(&json!({ "slot": slot, "width": width, "height": height, "refresh": refresh, "hidpi": hidpi }))?;
            return Ok(json!({ "width": width, "height": height, "refresh": refresh }));
        }
        let v = self.virtuals.get_mut(slot).and_then(Option::as_mut).ok_or("écran virtuel inconnu")?;
        // Sway, Hyprland : le compositeur change le mode lui-même.
        self.recorder.resize(&v.cast, width, height, refresh)?;
        // GNOME : la taille d'un écran virtuel est celle que la capture
        // négocie : la nouvelle taille va au fichier des sources, que Kyber
        // relit (son flux suivant la demande, et GNOME suit).
        v.width = width;
        v.height = height;
        v.refresh = refresh;
        if let Some(h) = hidpi {
            v.hidpi = h;
        }
        self.write_sources();
        Ok(json!({ "width": width, "height": height, "refresh": refresh }))
    }

    fn physical_count(&self) -> usize {
        self.desk.monitors().map(|m| m.iter().filter(|m| m.enabled && !m.virt).count()).unwrap_or(0)
    }

    /// Écrans de l'ordinateur allumés, en pixels de leur mode, places dans
    /// les unités du bureau.
    fn physical_displays(&self) -> Result<Vec<Value>> {
        let mons = self.desk.monitors()?;
        let mut connectors: Vec<&str> = mons.iter().filter(|m| !m.virt).map(|m| m.connector.as_str()).collect();
        connectors.sort();
        Ok(mons
            .iter()
            .filter(|m| m.enabled && !m.virt)
            .map(|m| {
                let id = 1 + connectors.iter().position(|c| *c == m.connector).unwrap_or(0) as u32;
                json!({ "id": id, "name": m.connector, "x": m.x, "y": m.y, "width": m.width, "height": m.height,
                        "refresh": m.refresh, "primary": m.primary,
                        "listed_width": (m.width as f64 / m.scale).round() as i32,
                        "listed_height": (m.height as f64 / m.scale).round() as i32 })
            })
            .collect())
    }

    /// Sans écran virtuel : les écrans de l'ordinateur, capturés pour Kyber
    /// (une capture par écran), le fichier des sources à jour.
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
                // Un écran qui vient d'être rallumé (fin de session) peut ne
                // pas être encore revenu chez le compositeur (KWin) : on
                // réessaie un instant.
                let mut cast = self.recorder.monitor(&name);
                for _ in 0..6 {
                    if cast.is_ok() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                    self.recorder.pump();
                    cast = self.recorder.monitor(&name);
                }
                match cast {
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
        for p in std::mem::take(&mut self.physicals) {
            self.recorder.stop(&p.cast);
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
        let mut entry = |id: u32, name: &str, cast: &Cast, w: u32, h: u32, place: (i32, i32)| {
            // Sway, Hyprland : la sortie, capturée par Kyber (correctif 0013) ;
            // sinon le flux PipeWire du compositeur.
            let source = match &cast.key {
                CastKey::Wlr { name, .. } => format!("wayland:{name}"),
                _ => format!("pipewiresrc target-object={} do-timestamp=true keepalive-time=1000 resend-last=true", cast.serial),
            };
            out.push_str(&format!(
                "\n[[video_sources]]\nid = {id}\nname = \"{name}\"\nsource = \"{source}\"\nwidth = {w}\nheight = {h}\nframerate = \"{framerate}\"\nformat = \"BGRx\"\nx = {}\ny = {}\n",
                place.0, place.1
            ));
        };
        if self.virtuals.iter().any(Option::is_some) {
            for (slot, v) in self.virtuals.iter().enumerate() {
                if let Some(v) = v {
                    let name = if slot == 0 { "KyberNow".to_string() } else { format!("KyberNow {}", slot + 1) };
                    entry(VIRTUAL_ID_BASE + slot as u32, &name, &v.cast, v.width, v.height, v.place);
                }
            }
        } else {
            for p in &self.physicals {
                entry(p.id, &p.name, &p.cast, p.width, p.height, p.place);
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
            log(&e);
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
    log(&format!(
        "prêt en {} ms ({}), fichier des écrans : {}",
        started.elapsed().as_millis(),
        agent.desk.name(),
        agent.file.display()
    ));
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

#[cfg(test)]
mod tests {
    use super::shift_to_origin;

    #[test]
    fn un_ecran_a_gauche_fait_glisser_le_bureau() {
        // L'écran de gauche d'un Mac : (-1920, 0) ; Mutter veut tout en positif.
        assert_eq!(shift_to_origin([(0, 0), (1920, 0), (-1920, 0)].into_iter()), (1920, 0));
        assert_eq!(shift_to_origin([(0, 0), (0, -1080)].into_iter()), (0, 1080));
        assert_eq!(shift_to_origin([(0, 0), (2560, 200)].into_iter()), (0, 0));
        assert_eq!(shift_to_origin(std::iter::empty()), (0, 0));
    }
}
