//! Écran virtuel adapté à l'appareil qui se connecte.
//!
//! Au début d'une session : un écran virtuel SudoVDA est créé au mode du
//! client, puis il devient le seul écran actif (les écrans physiques
//! s'éteignent). À la fin : l'écran virtuel est retiré et la disposition
//! habituelle des écrans physiques est réappliquée.
//!
//! Multi-écrans (`screens` de `prepare`, docs/multi-ecrans.md) : un écran
//! virtuel par écran de l'appareil, quatre au plus, placés comme demandé,
//! le principal en (0, 0) ; seuls ces écrans restent actifs. Le principal
//! garde l'identifiant de toujours (`KyberNow-Display`), les autres ont le
//! leur (`slot_guid`). Veille, politique de mode et filets de sécurité
//! valent pour chacun.
//!
//! Filets de sécurité :
//! - le pilote retire ses écrans s'il ne reçoit plus de signal de vie (3 s) ;
//! - l'état d'une session est écrit sur disque et nettoyé au démarrage suivant ;
//! - une session client sans nouvelles depuis 20 s, et sans flux encodé (NVIDIA, AMD ou Intel), s'arrête.
//!
//! Pendant la session, la console relit chaque seconde le mode de l'écran
//! virtuel et les écrans actifs (agent de veille, `watch.rs`) : le mode
//! courant est donné aux apps avec le signal de vie, et un écran physique
//! qui se rallume (câble rebranché, disposition réappliquée par Windows) est
//! éteint de nouveau après 2 s. Si le mode change (un jeu qui passe en
//! 1080p), la politique décide : `follow` laisse faire et l'annonce,
//! `restore` remet le mode de la session (au plus une fois toutes les 5 s).

#[cfg(windows)]
pub mod agent;
#[cfg(windows)]
pub mod pointer;
#[cfg(windows)]
mod sudovda;
#[cfg(windows)]
pub mod topology;
#[cfg(windows)]
pub mod watch;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::i18n::Msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// Jamais d'écran virtuel.
    Off,
    /// Seulement quand une app KyberNow annonce sa résolution.
    #[default]
    Apps,
    /// Pour toute connexion (navigateur, client Kyber…), au mode par défaut.
    All,
}

/// Que faire quand le mode de l'écran virtuel change pendant la session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModePolicy {
    /// Laisser faire (un jeu choisit sa définition) et l'annoncer.
    #[default]
    Follow,
    /// Remettre le mode demandé par l'appareil.
    Restore,
}

/// Ce qu'un appareil demande (`mode_policy` de `prepare` et du signal de
/// vie) : une politique, ou `default` pour revenir au réglage de la console
/// (profil Max ou personnalisé choisi en session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyChoice {
    Follow,
    Restore,
    Default,
}

impl PolicyChoice {
    fn policy(self) -> Option<ModePolicy> {
        match self {
            PolicyChoice::Follow => Some(ModePolicy::Follow),
            PolicyChoice::Restore => Some(ModePolicy::Restore),
            PolicyChoice::Default => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VdSettings {
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_refresh")]
    pub refresh: u32,
    /// Politique par défaut, quand l'app n'en donne pas (`mode_policy`).
    #[serde(default)]
    pub on_mode_change: ModePolicy,
}

fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_refresh() -> u32 {
    60
}

impl Default for VdSettings {
    fn default() -> Self {
        Self {
            trigger: Trigger::Apps,
            width: default_width(),
            height: default_height(),
            refresh: default_refresh(),
            on_mode_change: ModePolicy::default(),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

impl Mode {
    pub fn validate(&self) -> Result<()> {
        if !(640..=7680).contains(&self.width) || !(480..=4320).contains(&self.height) {
            bail!(Msg::new("display.resolutionRange"));
        }
        if !(24..=500).contains(&self.refresh) {
            bail!(Msg::new("display.refreshRange"));
        }
        Ok(())
    }
}

/// Au plus quatre écrans par session (`screens` de `prepare`).
pub const MAX_SCREENS: usize = 4;

/// Un écran demandé par l'appareil (`screens` de `prepare`) : son mode et sa
/// place sur le bureau de Windows, en pixels, l'écran principal en (0, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub primary: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Screen {
    /// Session à un écran : le principal, en (0, 0).
    pub fn single(mode: Mode) -> Self {
        Screen { width: mode.width, height: mode.height, refresh: mode.refresh, x: 0, y: 0, primary: true }
    }

    pub fn mode(&self) -> Mode {
        Mode { width: self.width, height: self.height, refresh: self.refresh }
    }

    /// Le même écran, à un autre mode et à une autre place.
    fn at(self, mode: Mode, x: i32, y: i32) -> Self {
        Screen { width: mode.width, height: mode.height, refresh: mode.refresh, x, y, primary: self.primary }
    }

    fn left(&self) -> i64 {
        i64::from(self.x)
    }
    fn top(&self) -> i64 {
        i64::from(self.y)
    }
    fn right(&self) -> i64 {
        self.left() + i64::from(self.width)
    }
    fn bottom(&self) -> i64 {
        self.top() + i64::from(self.height)
    }
}

/// Les intervalles [a0, a1[ et [b0, b1[ ont-ils une longueur en commun ?
fn spans_overlap(a0: i64, a1: i64, b0: i64, b1: i64) -> bool {
    a0.max(b0) < a1.min(b1)
}

fn overlapping(a: &Screen, b: &Screen) -> bool {
    spans_overlap(a.left(), a.right(), b.left(), b.right()) && spans_overlap(a.top(), a.bottom(), b.top(), b.bottom())
}

/// Deux écrans partagent-ils un segment de bord (un coin ne suffit pas :
/// Windows exige un bord commun) ?
fn touching(a: &Screen, b: &Screen) -> bool {
    ((a.right() == b.left() || b.right() == a.left()) && spans_overlap(a.top(), a.bottom(), b.top(), b.bottom()))
        || ((a.bottom() == b.top() || b.bottom() == a.top()) && spans_overlap(a.left(), a.right(), b.left(), b.right()))
}

/// Disposition demandée par un appareil : de 1 à 4 écrans, exactement un
/// principal, en (0, 0), chaque mode valide, aucun chevauchement, tous
/// reliés au principal par des bords partagés.
pub fn validate_screens(screens: &[Screen]) -> Result<()> {
    if screens.is_empty() || screens.len() > MAX_SCREENS {
        bail!(Msg::new("display.screensCount").with("max", MAX_SCREENS));
    }
    let primaries: Vec<&Screen> = screens.iter().filter(|s| s.primary).collect();
    if primaries.len() != 1 {
        bail!(Msg::new("display.screensPrimary"));
    }
    if (primaries[0].x, primaries[0].y) != (0, 0) {
        bail!(Msg::new("display.screensOrigin"));
    }
    for s in screens {
        s.mode().validate()?;
    }
    for (i, a) in screens.iter().enumerate() {
        if screens[i + 1..].iter().any(|b| overlapping(a, b)) {
            bail!(Msg::new("display.screensOverlap"));
        }
    }
    let start = screens.iter().position(|s| s.primary).unwrap_or(0);
    let mut reached = vec![false; screens.len()];
    reached[start] = true;
    let mut todo = vec![start];
    while let Some(i) = todo.pop() {
        for j in 0..screens.len() {
            if !reached[j] && touching(&screens[i], &screens[j]) {
                reached[j] = true;
                todo.push(j);
            }
        }
    }
    if reached.contains(&false) {
        bail!(Msg::new("display.screensApart"));
    }
    Ok(())
}

/// Ordre de création : le principal d'abord, puis les autres dans l'ordre de
/// la demande. Le rang dans cet ordre donne l'identifiant de l'écran.
fn slot_order(screens: &[Screen]) -> Vec<usize> {
    let primary = screens.iter().position(|s| s.primary).unwrap_or(0);
    std::iter::once(primary).chain((0..screens.len()).filter(|i| *i != primary)).collect()
}

/// « KyberNow écran virtuel », identifiant stable du moniteur SudoVDA de
/// l'écran principal (celui de toujours, pour une session à un écran).
const VIRTUAL_DISPLAY_GUID: [u8; 16] = *b"KyberNow-Display";

/// Identifiant stable du moniteur de rang `slot` (0 : le principal) :
/// `KyberNow-Display`, puis `02-KyberNow-Disp` à `04-…`. Windows retrouve
/// ainsi les réglages de chaque écran (échelle) d'une session à l'autre.
/// SudoVDA fait des 4 premiers octets le numéro de série de l'EDID : ils
/// diffèrent d'un écran à l'autre, pour que Windows ne les confonde pas.
pub fn slot_guid(slot: usize) -> [u8; 16] {
    if slot == 0 {
        return VIRTUAL_DISPLAY_GUID;
    }
    let mut guid = *b"00-KyberNow-Disp";
    let n = (slot + 1) as u8;
    guid[0] = b'0' + n / 10;
    guid[1] = b'0' + n % 10;
    guid
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    App,
    Auto,
    Test,
}

/// Où retrouver l'écran créé dans la configuration d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub luid_low: u32,
    pub luid_high: i32,
    pub target: u32,
}

/// Nombres séparés par des virgules (arguments de l'agent d'affichage).
fn numbers<const N: usize>(arg: &str) -> Result<[i64; N]> {
    let parts: Vec<i64> = arg.split(',').map(|p| p.trim().parse::<i64>()).collect::<std::result::Result<_, _>>()
        .map_err(|_| anyhow::anyhow!("argument d'écran illisible : {arg}"))?;
    <[i64; N]>::try_from(parts).map_err(|_| anyhow::anyhow!("argument d'écran : {N} nombres attendus dans {arg}"))
}

fn narrow<T: TryFrom<i64>>(n: i64, arg: &str) -> Result<T> {
    T::try_from(n).map_err(|_| anyhow::anyhow!("argument d'écran hors limites : {arg}"))
}

impl Created {
    /// `luid_low,luid_high,target` (veille des écrans d'une session).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn to_arg(&self) -> String {
        format!("{},{},{}", self.luid_low, self.luid_high, self.target)
    }

    pub fn parse(arg: &str) -> Result<Self> {
        let [low, high, target] = numbers::<3>(arg)?;
        Ok(Created { luid_low: narrow(low, arg)?, luid_high: narrow(high, arg)?, target: narrow(target, arg)? })
    }
}

/// Un écran virtuel à placer, tel que l'agent d'affichage le reçoit
/// (`display-agent isolate-many --screen …`, le principal en premier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Placement {
    pub created: Created,
    pub mode: Mode,
    pub x: i32,
    pub y: i32,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Placement {
    /// `luid_low,luid_high,target,width,height,refresh,x,y`
    pub fn to_arg(&self) -> String {
        format!("{},{},{},{},{},{}", self.created.to_arg(), self.mode.width, self.mode.height, self.mode.refresh, self.x, self.y)
    }

    /// Place d'un écran que Windows a mis à `width` × `height` au lieu du mode
    /// demandé : collé du même côté que demandé (un écran à gauche du
    /// principal garde son bord droit, un écran au-dessus son bord bas).
    pub fn anchored(&self, width: u32, height: u32) -> (i32, i32) {
        let right = i64::from(self.x) + i64::from(self.mode.width);
        let bottom = i64::from(self.y) + i64::from(self.mode.height);
        let x = if right <= 0 { right - i64::from(width) } else { i64::from(self.x) };
        let y = if bottom <= 0 { bottom - i64::from(height) } else { i64::from(self.y) };
        (x as i32, y as i32)
    }

    pub fn parse(arg: &str) -> Result<Self> {
        let [low, high, target, width, height, refresh, x, y] = numbers::<8>(arg)?;
        Ok(Placement {
            created: Created { luid_low: narrow(low, arg)?, luid_high: narrow(high, arg)?, target: narrow(target, arg)? },
            mode: Mode { width: narrow(width, arg)?, height: narrow(height, arg)?, refresh: narrow(refresh, arg)? },
            x: narrow(x, arg)?,
            y: narrow(y, arg)?,
        })
    }
}

/// Ce que Windows affiche pendant la session (agent de veille).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observed {
    /// Mode courant de l'écran virtuel principal (0 × 0 s'il n'est plus actif).
    pub mode: Mode,
    #[serde(skip)]
    pub gdi_name: String,
    /// Écrans actifs, écrans virtuels compris.
    pub displays: u32,
    /// Écrans actifs autres que les écrans virtuels (écrans physiques rallumés).
    pub others: u32,
    /// Mode courant et nom GDI des autres écrans virtuels, dans l'ordre de
    /// la session (vide pour une session à un écran).
    #[serde(skip)]
    pub extra: Vec<(Mode, String)>,
}

impl Observed {
    /// Mode et nom GDI de l'écran de rang `i` de la session (0 : le principal).
    fn slot(&self, i: usize) -> Option<(Mode, &str)> {
        if i == 0 {
            Some((self.mode, self.gdi_name.as_str()))
        } else {
            self.extra.get(i - 1).map(|(m, g)| (*m, g.as_str()))
        }
    }
}

/// Ce qu'une session a mis en place (réponse de `prepare`).
#[derive(Debug, Clone)]
pub struct Started {
    /// Résultat de l'isolement pour l'écran principal (`applied`).
    pub applied: Value,
    /// Écrans en place, dans l'ordre de la demande ; `None` : pas créé.
    pub screens: Vec<Option<Screen>>,
}

/// Un écran physique rallumé n'est éteint qu'après ce délai de stabilité.
const PHYSICAL_DEBOUNCE: Duration = Duration::from_secs(2);
/// Au plus un nouvel isolement dans cet intervalle (Windows qui insiste).
const REISOLATE_COOLDOWN: Duration = Duration::from_secs(10);
/// Politique `restore` : au plus une remise du mode dans cet intervalle.
const MODE_FIX_INTERVAL: Duration = Duration::from_secs(5);
/// Un jeu qui remet sa définition après chaque remise : au bout de trois
/// remises en une minute, la console laisse faire pour le reste de la session.
const MAX_MODE_FIXES: usize = 3;
const MODE_FIX_WINDOW: Duration = Duration::from_secs(60);
/// Windows qui rallume sans cesse un écran : au bout de cinq isolements en
/// deux minutes, ou de trois échecs de suite, la console renonce.
const MAX_REISOLATIONS: usize = 5;
const REISOLATE_WINDOW: Duration = Duration::from_secs(120);
const MAX_REISOLATE_FAILURES: u32 = 3;

/// Même mode, à 1 Hz près (Windows dit 59 pour 59,94).
fn same_mode(a: Mode, b: Mode) -> bool {
    a.width == b.width && a.height == b.height && a.refresh.abs_diff(b.refresh) <= 1
}

/// Mode réellement obtenu à la création (`applied`), sinon celui demandé.
fn applied_mode(applied: &Value, requested: Mode) -> Mode {
    let n = |k: &str| applied[k].as_u64().filter(|v| *v > 0).map(|v| v as u32);
    match (n("width"), n("height")) {
        (Some(width), Some(height)) => Mode { width, height, refresh: n("refresh").unwrap_or(requested.refresh) },
        _ => requested,
    }
}

/// Mode et place obtenus pour chaque écran donné à l'isolement (même ordre) ;
/// `None` : Windows ne l'a pas activé. Une session à un écran n'a pas de
/// liste `screens` : son écran est en (0, 0).
fn placed_screens(applied: &Value, asked: &[Screen]) -> Vec<Option<Screen>> {
    let list = applied["screens"].as_array();
    asked
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let entry = match list {
                Some(list) => list.get(i).filter(|v| !v.is_null()),
                None => (i == 0).then_some(applied),
            };
            match entry {
                Some(v) => {
                    let pos = |k: &str| v[k].as_i64().map(|n| n as i32);
                    Some(s.at(applied_mode(v, s.mode()), pos("x").unwrap_or(s.x), pos("y").unwrap_or(s.y)))
                }
                // Le principal est toujours là quand l'isolement réussit.
                None if i == 0 => Some(*s),
                None => None,
            }
        })
        .collect()
}

/// Ce que l'agent d'affichage a noté en plaçant les écrans (mode refusé,
/// proposé ou non par Windows…) : il n'a pas de journal, Admin l'écrit dans
/// le sien et le retire du résultat.
fn log_notes(applied: &mut Value) {
    let Some(o) = applied.as_object_mut() else { return };
    // Ce que Windows a mis en place pour chaque écran (mode, place, échelle).
    if let Some(facts) = o.remove("facts") {
        for fact in facts.as_array().into_iter().flatten().filter_map(Value::as_str) {
            tracing::info!("écrans virtuels : {fact}");
        }
    }
    if let Some(notes) = o.remove("notes") {
        for note in notes.as_array().into_iter().flatten().filter_map(Value::as_str) {
            tracing::warn!("écrans virtuels : {note}");
        }
    }
}

/// « 1920 × 1080 à 60 Hz » (journaux).
fn label(m: Mode) -> String {
    format!("{} × {} à {} Hz", m.width, m.height, m.refresh)
}

/// Le même, en paramètre d'un événement : écrit dans la langue de qui lit.
fn mode_param(m: Mode) -> Value {
    Msg::new("fmt.mode").with("width", m.width).with("height", m.height).with("refresh", m.refresh).to_value()
}

/// Fabricant (identifiant PCI) de la carte qui fait tourner l'encodeur de
/// Kyber (`video.encoder`) ; aucun pour l'encodage par le processeur.
pub fn encoder_vendor(encoder: &str) -> Option<u16> {
    match encoder {
        "nvenc" => Some(0x10DE),
        "amf" => Some(0x1002),
        "qsv" => Some(0x8086),
        _ => None,
    }
}

/// Fabricant d'une carte d'après le chemin de son périphérique
/// (`\\?\PCI#VEN_10DE&DEV_2208…`).
fn path_vendor(path: &str) -> Option<u16> {
    let upper = path.to_ascii_uppercase();
    let at = upper.find("VEN_")? + 4;
    u16::from_str_radix(upper.get(at..at + 4)?, 16).ok()
}

/// Carte à donner au pilote pour rendre les écrans virtuels : la première
/// du fabricant de l'encodeur parmi celles que liste l'agent (`adapters`).
fn pick_adapter(adapters: &Value, vendor: u16) -> Option<(u32, i32, String)> {
    adapters["adapters"].as_array()?.iter().find_map(|a| {
        let path = a["path"].as_str()?;
        (path_vendor(path) == Some(vendor)).then(|| {
            (a["luid_low"].as_u64().unwrap_or(0) as u32, a["luid_high"].as_i64().unwrap_or(0) as i32, path.to_string())
        })
    })
}

/// Une remise du mode d'un écran (politique `restore`).
struct Fix {
    /// Rang de l'écran dans la session.
    slot: usize,
    gdi_name: String,
    /// Mode vu sur l'ordinateur, et celui à remettre.
    from: Mode,
    target: Mode,
}

/// Un écran virtuel de la session.
struct Slot {
    guid: [u8; 16],
    created: Created,
    /// Rang de l'écran dans la demande de l'appareil.
    index: usize,
    /// Mode et place obtenus à la création : la référence pour dire que le
    /// mode a changé.
    placed: Screen,
    /// Dernier mode inattendu annoncé dans le fil (une fois par mode).
    announced: Option<Mode>,
}

impl Slot {
    fn target(&self) -> Mode {
        self.placed.mode()
    }
}

struct Session {
    origin: Origin,
    /// Écrans demandés, dans l'ordre de la demande (un seul : session à un écran).
    layout: Vec<Screen>,
    /// Écrans créés, le principal en premier.
    slots: Vec<Slot>,
    user: Option<String>,
    /// Appareil pour lequel l'écran a été créé (apps à partir de 0.2.5).
    device: Option<String>,
    started_at: u64,
    last_seen: Instant,
    until: Option<Instant>,
    applied: Value,
    /// Dernier état lu par la veille.
    observed: Option<Observed>,
    /// Écrans physiques vus allumés depuis cet instant.
    others_since: Option<Instant>,
    last_reisolate: Option<Instant>,
    /// Politique donnée par l'app (sinon, celle des réglages).
    policy: Option<ModePolicy>,
    last_mode_fix: Option<Instant>,
    /// Remises du mode de la dernière minute ; au-delà, la console renonce.
    mode_fixes: Vec<Instant>,
    restore_gave_up: bool,
    /// Isolements refaits des deux dernières minutes, échecs de suite.
    reisolations: Vec<Instant>,
    reisolate_failures: u32,
    reisolate_gave_up: bool,
}

impl Session {
    /// Politique en vigueur : celle de l'app, sinon le réglage ; `follow` si
    /// la console a renoncé à remettre le mode.
    fn effective_policy(&self, default: ModePolicy) -> ModePolicy {
        if self.restore_gave_up { ModePolicy::Follow } else { self.policy.unwrap_or(default) }
    }

    /// Mode demandé pour l'écran principal.
    fn mode(&self) -> Mode {
        self.layout[self.slots[0].index].mode()
    }

    /// Ce qui est en place, pour la réponse de `prepare`.
    fn started(&self) -> Started {
        let mut screens = vec![None; self.layout.len()];
        for slot in &self.slots {
            screens[slot.index] = Some(slot.placed);
        }
        Started { applied: self.applied.clone(), screens }
    }
}

/// Fil des événements de la console (vue d'ensemble).
pub type EventSink = Arc<dyn Fn(&str, Msg) + Send + Sync>;

/// Fichier écrit pendant une session, pour nettoyer après un plantage.
#[derive(Serialize, Deserialize)]
struct Record {
    /// Écran principal (seul champ des versions à un écran).
    guid: String,
    /// Tous les écrans créés, le principal compris.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    guids: Vec<String>,
}

impl Record {
    fn guids(&self) -> Vec<[u8; 16]> {
        std::iter::once(&self.guid)
            .chain(self.guids.iter())
            .filter_map(|g| hex::decode(g).ok())
            .filter_map(|b| <[u8; 16]>::try_from(b.as_slice()).ok())
            .fold(Vec::new(), |mut all, g| {
                if !all.contains(&g) {
                    all.push(g);
                }
                all
            })
    }
}

trait Backend: Send + Sync {
    fn driver(&self) -> Result<Value>;
    /// Fait rendre les écrans du pilote par la carte du fabricant `vendor`
    /// (celle de l'encodeur) ; rend le chemin de la carte choisie, `None` si
    /// aucune ne correspond.
    fn pin_render_adapter(&self, vendor: u16) -> Result<Option<String>>;
    /// Crée l'écran de rang `slot` (0 : le principal ; donne son numéro de série).
    fn create(&self, guid: [u8; 16], slot: usize, mode: Mode) -> Result<Created>;
    fn remove(&self, guid: [u8; 16]) -> Result<()>;
    fn ping(&self) -> Result<()>;
    /// Ne garde actifs que ces écrans virtuels (le principal en premier), à
    /// leur mode et à leur place. Avec plusieurs écrans, le résultat donne
    /// dans `screens` ce qui est en place pour chacun (null : pas activé).
    fn isolate(&self, screens: &[(Created, Screen)]) -> Result<Value>;
    fn restore(&self) -> Result<()>;
    /// Début et fin de la veille des écrans créés (le principal en premier).
    fn watch(&self, created: &[Created]);
    fn unwatch(&self);
    /// Dernier état vu par la veille (None : inconnu pour l'instant).
    fn observe(&self) -> Option<Observed>;
    /// Remet `mode` sur l'écran `gdi_name` ; rend le mode obtenu.
    fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode>;
}

pub struct Manager {
    backend: Box<dyn Backend>,
    data_dir: PathBuf,
    session: Mutex<Option<Session>>,
    /// Sérialise les opérations longues (création, isolement, restauration).
    ops: Mutex<()>,
    last_error: Mutex<Option<String>>,
    /// Un écran virtuel existe : le fil des signaux de vie doit pinguer le pilote.
    alive: AtomicBool,
    pub settings: Mutex<VdSettings>,
    events: Mutex<Option<EventSink>>,
    /// Fabricant de la carte de l'encodeur de Kyber (`set_render_vendor`).
    render_vendor: Mutex<Option<u16>>,
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Manager {
    pub fn new(data_dir: PathBuf, settings: VdSettings) -> Arc<Self> {
        #[cfg(windows)]
        let backend: Box<dyn Backend> = Box::new(windows::WinBackend::new(data_dir.clone()));
        #[cfg(not(windows))]
        let backend: Box<dyn Backend> = Box::new(Simulated::default());
        Self::with_backend(data_dir, settings, backend)
    }

    fn with_backend(data_dir: PathBuf, settings: VdSettings, backend: Box<dyn Backend>) -> Arc<Self> {
        Arc::new(Self {
            backend,
            data_dir,
            session: Mutex::new(None),
            ops: Mutex::new(()),
            last_error: Mutex::new(None),
            alive: AtomicBool::new(false),
            settings: Mutex::new(settings),
            events: Mutex::new(None),
            render_vendor: Mutex::new(None),
        })
    }

    /// Carte de l'encodeur de Kyber (`encoder_vendor`), qui rendra les écrans
    /// virtuels d'une session à plusieurs écrans.
    pub fn set_render_vendor(&self, vendor: Option<u16>) {
        *self.render_vendor.lock().unwrap() = vendor;
    }

    /// Branche le fil des événements (vue d'ensemble de la console).
    pub fn on_event(&self, sink: EventSink) {
        *self.events.lock().unwrap() = Some(sink);
    }

    fn event(&self, kind: &str, msg: Msg) {
        let sink = self.events.lock().unwrap().clone();
        match sink {
            Some(sink) => sink(kind, msg),
            None => tracing::info!("écran virtuel : {msg}"),
        }
    }

    fn record_path(&self) -> PathBuf {
        self.data_dir.join("display-session.json")
    }

    /// Note sur disque les écrans créés (le principal en premier).
    fn write_record(&self, guids: &[[u8; 16]]) {
        let Some(first) = guids.first() else { return };
        let record = Record {
            guid: hex::encode(first),
            guids: if guids.len() > 1 { guids.iter().map(hex::encode).collect() } else { Vec::new() },
        };
        if let Ok(bytes) = serde_json::to_vec(&record) {
            let _ = std::fs::write(self.record_path(), bytes);
        }
    }

    fn fail(&self, e: &anyhow::Error) {
        tracing::error!("écran virtuel : {e:#}");
        *self.last_error.lock().unwrap() = Some(format!("{e:#}"));
    }

    /// Après un plantage ou une coupure : retire les écrans et restaure les écrans physiques.
    pub fn recover(&self) {
        let Ok(text) = std::fs::read_to_string(self.record_path()) else { return };
        tracing::warn!("session d'écran virtuel interrompue trouvée : restauration des écrans");
        if let Ok(record) = serde_json::from_str::<Record>(&text) {
            for guid in record.guids() {
                let _ = self.backend.remove(guid);
            }
        }
        self.alive.store(false, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(500));
        match self.backend.restore() {
            Ok(()) => {
                let _ = std::fs::remove_file(self.record_path());
            }
            // Les écrans ne sont pas rendus (pas de session console, par
            // exemple) : on garde le fichier pour réessayer au prochain démarrage.
            Err(e) => self.fail(&e),
        }
    }

    /// Démarre (ou reprend) une session à un écran, à `mode`. Un autre mode
    /// remplace la session en cours, sauf qu'une session d'app n'est jamais
    /// remplacée par le déclenchement automatique ni par un essai depuis la
    /// console. `policy` : ce que l'app veut quand le mode change (sinon, le réglage).
    pub fn start(
        &self,
        origin: Origin,
        mode: Mode,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
        policy: Option<PolicyChoice>,
    ) -> Result<Value> {
        mode.validate()?;
        let _op = self.ops.lock().unwrap();
        self.start_locked(origin, vec![Screen::single(mode)], user, device, duration, policy).map(|s| s.applied)
    }

    /// Comme `start`, avec un écran virtuel par entrée de `screens`
    /// (`prepare` d'une app multi-écrans), placés comme demandé. L'écran
    /// principal est toujours créé, sinon la session échoue ; un autre écran
    /// que Windows ou le pilote refuse est laissé de côté (`None`).
    pub fn start_screens(
        &self,
        origin: Origin,
        screens: Vec<Screen>,
        user: Option<String>,
        device: Option<String>,
        policy: Option<PolicyChoice>,
    ) -> Result<Started> {
        validate_screens(&screens)?;
        let _op = self.ops.lock().unwrap();
        self.start_locked(origin, screens, user, device, None, policy)
    }

    fn start_locked(
        &self,
        origin: Origin,
        layout: Vec<Screen>,
        user: Option<String>,
        device: Option<String>,
        duration: Option<Duration>,
        policy: Option<PolicyChoice>,
    ) -> Result<Started> {
        let default_policy = self.settings.lock().unwrap().on_mode_change;
        let mut fixes: Vec<Fix> = Vec::new();
        let kept = {
            let mut session = self.session.lock().unwrap();
            match session.as_mut() {
                Some(s) if s.layout == layout => {
                    s.last_seen = Instant::now();
                    // Une app garde son écran : « Auto » ne la rétrograde pas.
                    if !(s.origin == Origin::App && origin == Origin::Auto) {
                        s.origin = origin;
                        s.until = duration.map(|d| Instant::now() + d);
                    }
                    s.user = user.clone().or(s.user.take());
                    s.device = device.clone().or(s.device.take());
                    if let Some(choice) = policy {
                        s.policy = choice.policy();
                    }
                    // « Remettre » dans l'app : la console réessaie, même si
                    // elle avait renoncé face à un jeu qui insistait.
                    if policy == Some(PolicyChoice::Restore) {
                        s.restore_gave_up = false;
                        s.mode_fixes.clear();
                    }
                    // Mêmes écrans redemandés alors qu'un mode a changé : en
                    // politique `restore` (« Remettre » dans l'app), on le remet.
                    if s.effective_policy(default_policy) == ModePolicy::Restore {
                        if let Some(o) = s.observed.as_ref() {
                            for (i, slot) in s.slots.iter().enumerate() {
                                if let Some((mode, gdi)) = o.slot(i) {
                                    if mode.width > 0 && !same_mode(mode, slot.target()) {
                                        fixes.push(Fix { slot: i, gdi_name: gdi.to_string(), from: mode, target: slot.target() });
                                    }
                                }
                            }
                        }
                    }
                    Some(s.started())
                }
                Some(s) if s.origin == Origin::App => match origin {
                    // Le fil de surveillance pendant une session d'app : rien à faire.
                    Origin::Auto => return Ok(s.started()),
                    Origin::Test => bail!("un appareil utilise l'écran virtuel : essai impossible pendant sa session"),
                    Origin::App => None,
                },
                _ => None,
            }
        };
        if let Some(started) = kept {
            let now = Instant::now();
            for fix in fixes {
                self.fix_mode(fix, now);
            }
            return Ok(started);
        }
        // Autres écrans (un autre appareil prend la main) : on repart de zéro.
        self.stop_locked("remplacée par une nouvelle session");

        // Identifiants fixes : Windows retrouve la disposition de session
        // enregistrée pour ces écrans (voir topology.rs).
        let begun = Instant::now();
        // Plusieurs écrans : tous rendus par la carte qui capture et encode,
        // fixée avant de les créer (sinon Windows choisit pour chacun).
        let vendor = *self.render_vendor.lock().unwrap();
        if let Some(vendor) = vendor.filter(|_| layout.len() > 1) {
            match self.backend.pin_render_adapter(vendor) {
                Ok(Some(path)) => tracing::info!("écrans virtuels rendus par la carte {path}"),
                Ok(None) => tracing::warn!("aucune carte du fabricant {vendor:04X} pour rendre les écrans virtuels : Windows choisit"),
                Err(e) => tracing::warn!("carte de rendu des écrans virtuels : {e:#}"),
            }
        }
        let mut pending: Vec<(usize, [u8; 16], Created)> = Vec::new();
        for (slot, &index) in slot_order(&layout).iter().enumerate() {
            let guid = slot_guid(slot);
            match self.backend.create(guid, slot, layout[index].mode()) {
                Ok(created) => {
                    pending.push((index, guid, created));
                    // Pinguer dès le premier écran : le chien de garde du
                    // pilote est armé à sa création, et la suite (autres
                    // écrans, isolement) peut durer plus que lui.
                    self.alive.store(true, Ordering::SeqCst);
                    self.write_record(&pending.iter().map(|p| p.1).collect::<Vec<_>>());
                }
                Err(e) if slot == 0 => {
                    self.fail(&e);
                    return Err(e);
                }
                Err(e) => tracing::warn!("écran virtuel {} sur {} non créé : {e:#}", slot + 1, layout.len()),
            }
        }
        let asked: Vec<(Created, Screen)> = pending.iter().map(|(index, _, created)| (*created, layout[*index])).collect();
        let applied = match self.backend.isolate(&asked) {
            Ok(v) => v,
            Err(e) => {
                self.fail(&e);
                for (_, guid, _) in &pending {
                    let _ = self.backend.remove(*guid);
                }
                self.alive.store(false, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(500));
                let _ = self.backend.restore();
                let _ = std::fs::remove_file(self.record_path());
                return Err(e);
            }
        };
        let placed = placed_screens(&applied, &asked.iter().map(|(_, s)| *s).collect::<Vec<_>>());
        // `applied` reste le mode du principal ; les écrans vont dans `screens` de la réponse.
        let mut applied = applied;
        log_notes(&mut applied);
        if let Some(o) = applied.as_object_mut() {
            o.remove("screens");
        }
        let mut slots = Vec::new();
        for ((index, guid, created), placed) in pending.into_iter().zip(placed) {
            match placed {
                Some(placed) => slots.push(Slot { guid, created, index, placed, announced: None }),
                None => {
                    tracing::warn!("un écran virtuel n'a pas été activé par Windows : retiré");
                    let _ = self.backend.remove(guid);
                }
            }
        }
        self.write_record(&slots.iter().map(|s| s.guid).collect::<Vec<_>>());
        tracing::info!(
            "{} écran(s) virtuel(s) actif(s) ({origin:?}) : {}, prêt(s) en {} ms",
            slots.len(),
            slots.iter().map(|s| format!("{} en ({}, {})", label(s.target()), s.placed.x, s.placed.y)).collect::<Vec<_>>().join(", "),
            begun.elapsed().as_millis()
        );
        let missing = layout.len() - slots.len();
        if missing > 0 {
            self.event("warn", Msg::new("events.screensMissing").with("count", missing).with("requested", layout.len()));
        }
        *self.last_error.lock().unwrap() = None;
        self.backend.watch(&slots.iter().map(|s| s.created).collect::<Vec<_>>());
        let session = Session {
            origin,
            layout,
            slots,
            user,
            device,
            started_at: now_secs(),
            last_seen: Instant::now(),
            until: duration.map(|d| Instant::now() + d),
            applied,
            observed: None,
            others_since: None,
            last_reisolate: None,
            policy: policy.and_then(PolicyChoice::policy),
            last_mode_fix: None,
            mode_fixes: Vec::new(),
            restore_gave_up: false,
            reisolations: Vec::new(),
            reisolate_failures: 0,
            reisolate_gave_up: false,
        };
        let started = session.started();
        *self.session.lock().unwrap() = Some(session);
        Ok(started)
    }

    /// Signal de vie d'un appareil. Vrai si l'écran virtuel en place est le
    /// sien : un appareil dont l'écran a été remplacé par celui d'un autre
    /// l'apprend ainsi, au lieu de le redemander sans fin.
    /// `policy` : celle du profil choisi en ce moment dans l'app.
    pub fn heartbeat(&self, device: Option<&str>, policy: Option<PolicyChoice>) -> bool {
        match self.session.lock().unwrap().as_mut() {
            Some(s) => {
                let owner = match (device, s.device.as_deref()) {
                    (Some(d), Some(owner)) => d == owner,
                    _ => true,
                };
                if owner {
                    s.last_seen = Instant::now();
                    if let Some(choice) = policy {
                        s.policy = choice.policy();
                    }
                }
                owner
            }
            None => false,
        }
    }

    /// Politique en vigueur pour la session en cours.
    pub fn policy(&self) -> Option<ModePolicy> {
        let default = self.settings.lock().unwrap().on_mode_change;
        self.session.lock().unwrap().as_ref().map(|s| s.effective_policy(default))
    }

    /// Remet le mode de la session sur un écran virtuel (politique `restore`).
    /// Si Windows garde un autre mode, celui-ci devient la référence de
    /// l'écran pour le reste de la session : une remise au plus, jamais de
    /// boucle (chaque changement de mode coupe la capture de l'écran).
    fn fix_mode(&self, fix: Fix, now: Instant) {
        if let Some(s) = self.session.lock().unwrap().as_mut() {
            s.last_mode_fix = Some(now);
        }
        tracing::info!("écran virtuel {} : remise du mode, {} → {}", fix.gdi_name, label(fix.from), label(fix.target));
        match self.backend.set_mode(&fix.gdi_name, fix.target) {
            Ok(got) if same_mode(got, fix.target) => tracing::info!("écran virtuel {} : remis en {}", fix.gdi_name, label(fix.target)),
            Ok(got) => {
                tracing::warn!(
                    "écran virtuel {} : Windows garde {} au lieu de {}, accepté pour le reste de la session",
                    fix.gdi_name,
                    label(got),
                    label(fix.target)
                );
                if got.width > 0 {
                    if let Some(slot) = self.session.lock().unwrap().as_mut().and_then(|s| s.slots.get_mut(fix.slot)) {
                        slot.placed = slot.placed.at(got, slot.placed.x, slot.placed.y);
                        slot.announced = None;
                    }
                }
            }
            Err(e) => self.fail(&e),
        }
    }

    /// L'écran virtuel en place a-t-il été créé pour cet appareil ?
    pub fn owned_by(&self, device: &str) -> bool {
        self.session.lock().unwrap().as_ref().is_some_and(|s| s.device.as_deref() == Some(device))
    }

    pub fn stop(&self, reason: &str) {
        let _op = self.ops.lock().unwrap();
        self.stop_locked(reason);
    }

    fn stop_locked(&self, reason: &str) {
        let Some(session) = self.session.lock().unwrap().take() else { return };
        tracing::info!("fin de l'écran virtuel : {reason}");
        self.backend.unwatch();
        for slot in &session.slots {
            if let Err(e) = self.backend.remove(slot.guid) {
                self.fail(&e);
            }
        }
        self.alive.store(false, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(500));
        match self.backend.restore() {
            // Le fichier de session reste tant que les écrans ne sont pas
            // rendus : le prochain démarrage réessaiera.
            Ok(()) => {
                let _ = std::fs::remove_file(self.record_path());
            }
            Err(e) => self.fail(&e),
        }
    }

    pub fn status(&self) -> Value {
        let observed = self.observed();
        let session = self.session.lock().unwrap();
        let session = session.as_ref().map(|s| {
            // Chaque écran à son mode courant (un jeu a pu le changer), à sa place.
            let screens: Vec<Screen> = s
                .slots
                .iter()
                .enumerate()
                .map(|(i, slot)| {
                    let now = observed.as_ref().and_then(|o| o.slot(i)).map(|(m, _)| m).filter(|m| m.width > 0);
                    slot.placed.at(now.unwrap_or(slot.target()), slot.placed.x, slot.placed.y)
                })
                .collect();
            json!({
                "origin": s.origin,
                "mode": s.mode(),
                "screens": screens,
                "screens_requested": s.layout.len(),
                "user": s.user,
                "started_at": s.started_at,
                "seconds_left": s.until.map(|u| u.saturating_duration_since(Instant::now()).as_secs()),
                "applied": s.applied,
                "mode_policy": s.effective_policy(self.settings.lock().unwrap().on_mode_change),
            })
        });
        let driver = match self.backend.driver() {
            Ok(v) => json!({ "available": true, "info": v }),
            Err(e) => json!({ "available": false, "error": format!("{e:#}") }),
        };
        json!({
            "supported": cfg!(windows),
            "settings": *self.settings.lock().unwrap(),
            "driver": driver,
            "session": session,
            // Mode courant de l'écran virtuel principal et nombre d'écrans actifs, en session.
            "mode": observed.as_ref().map(|o| o.mode).filter(|m| m.width > 0),
            "displays": observed.as_ref().map(|o| o.displays),
            "last_error": *self.last_error.lock().unwrap(),
        })
    }

    /// Ce que la veille a vu en dernier, pendant une session.
    pub fn observed(&self) -> Option<Observed> {
        self.session.lock().unwrap().as_ref().and_then(|s| s.observed.clone())
    }

    /// Un tour de veille (chaque seconde pendant une session) : relit le mode
    /// des écrans virtuels et les écrans actifs, annonce un changement de
    /// mode, et éteint de nouveau un écran physique rallumé depuis 2 s.
    pub fn watch_tick(&self, now: Instant) {
        // Une création ou une fin en cours a la priorité : on repassera.
        let Ok(_op) = self.ops.try_lock() else { return };
        if self.session.lock().unwrap().is_none() {
            return;
        }
        let observed = self.backend.observe();
        let mut notes: Vec<(&str, Msg)> = Vec::new();
        let mut reisolate: Option<(Vec<(Created, Screen)>, u32)> = None;
        let mut fixes: Vec<Fix> = Vec::new();
        let default_policy = self.settings.lock().unwrap().on_mode_change;
        {
            let mut guard = self.session.lock().unwrap();
            let Some(s) = guard.as_mut() else { return };
            let policy = s.effective_policy(default_policy);
            s.observed = observed.clone();
            let Some(o) = observed else {
                // Veille muette : la stabilité d'un écran rallumé se recompte à son retour.
                s.others_since = None;
                return;
            };

            // Écran physique rallumé pendant la session.
            if o.others > 0 && !s.reisolate_gave_up {
                let since = *s.others_since.get_or_insert(now);
                // 10 s entre deux isolements, doublés après chaque échec (20, puis 40 s).
                let cooldown = REISOLATE_COOLDOWN * (1 << s.reisolate_failures.min(4));
                let cooled = s.last_reisolate.is_none_or(|t| now.duration_since(t) >= cooldown);
                if now.duration_since(since) >= PHYSICAL_DEBOUNCE && cooled {
                    s.reisolations.retain(|t| now.duration_since(*t) < REISOLATE_WINDOW);
                    if s.reisolations.len() >= MAX_REISOLATIONS {
                        s.reisolate_gave_up = true;
                        notes.push(("warn", Msg::new("events.displayKeepsTurningOn")));
                    } else if let [slot] = s.slots.as_slice() {
                        // En `follow`, le mode en cours est gardé (un jeu a pu le choisir).
                        let keep = if policy == ModePolicy::Follow && o.mode.width > 0 { o.mode } else { slot.target() };
                        reisolate = Some((vec![(slot.created, slot.placed.at(keep, 0, 0))], o.others));
                    } else {
                        // Plusieurs écrans : chacun reprend son mode et sa place
                        // du début de session (un mode gardé pourrait faire
                        // chevaucher un voisin).
                        reisolate = Some((s.slots.iter().map(|sl| (sl.created, sl.placed)).collect(), o.others));
                    }
                }
            } else {
                s.others_since = None;
            }

            // Mode changé sur l'ordinateur (jeu, réglage de Windows), écran par écran.
            let mut changed: Vec<Fix> = Vec::new();
            for (i, slot) in s.slots.iter_mut().enumerate() {
                let Some((mode, gdi_name)) = o.slot(i) else { continue };
                let target = slot.target();
                if mode.width > 0 && !same_mode(mode, target) {
                    if reisolate.is_some() {
                        continue;
                    }
                    if slot.announced != Some(mode) {
                        slot.announced = Some(mode);
                        let key = match policy {
                            ModePolicy::Follow => "events.displayModeFollowed",
                            ModePolicy::Restore => "events.displayModeRestored",
                        };
                        notes.push(("info", Msg::new(key).with("mode", mode_param(mode)).with("target", mode_param(target))));
                    }
                    changed.push(Fix { slot: i, gdi_name: gdi_name.to_string(), from: mode, target });
                } else if same_mode(mode, target) {
                    slot.announced = None;
                }
            }
            let due = s.last_mode_fix.is_none_or(|t| now.duration_since(t) >= MODE_FIX_INTERVAL);
            if policy == ModePolicy::Restore && due && !changed.is_empty() {
                s.mode_fixes.retain(|t| now.duration_since(*t) < MODE_FIX_WINDOW);
                if s.mode_fixes.len() >= MAX_MODE_FIXES {
                    // Le jeu remet sa définition après chaque remise : on
                    // arrête la ronde (et les relances d'image qu'elle coûte).
                    s.restore_gave_up = true;
                    notes.push(("warn", Msg::new("events.displayModeGaveUp").with("mode", mode_param(changed[0].from))));
                } else {
                    s.mode_fixes.push(now);
                    fixes = changed;
                }
            }
        }
        for (kind, msg) in notes {
            self.event(kind, msg);
        }
        for fix in fixes {
            self.fix_mode(fix, now);
        }
        if let Some((asked, others)) = reisolate {
            tracing::info!(
                "{others} écran(s) physique(s) rallumé(s) : nouvel isolement des écrans virtuels ({})",
                asked.iter().map(|(_, s)| format!("{} en ({}, {})", label(s.mode()), s.x, s.y)).collect::<Vec<_>>().join(", ")
            );
            match self.backend.isolate(&asked) {
                Ok(mut applied) => {
                    log_notes(&mut applied);
                    if let Some(s) = self.session.lock().unwrap().as_mut() {
                        s.applied = applied;
                        s.last_reisolate = Some(now);
                        s.others_since = None;
                        s.reisolations.push(now);
                        s.reisolate_failures = 0;
                    }
                    self.event("info", if others > 1 {
                        Msg::new("events.displaysOffAgain").with("count", others)
                    } else {
                        Msg::new("events.displayOffAgain")
                    });
                }
                Err(e) => {
                    self.fail(&e);
                    let (first, gave_up) = match self.session.lock().unwrap().as_mut() {
                        Some(s) => {
                            s.last_reisolate = Some(now);
                            s.reisolate_failures += 1;
                            s.reisolate_gave_up = s.reisolate_failures >= MAX_REISOLATE_FAILURES;
                            (s.reisolate_failures == 1, s.reisolate_gave_up)
                        }
                        None => (false, false),
                    };
                    // Annoncé au premier échec et quand la console renonce, pas à chaque essai.
                    if first {
                        self.event("warn", Msg::new("events.displayOffFailed").with("error", crate::i18n::error_param(&e)));
                    } else if gave_up {
                        self.event("warn", Msg::new("events.displayOffGaveUp"));
                    }
                }
            }
        }
    }


    /// Fils dédiés : signaux de vie du pilote, puis surveillance des sessions.
    pub fn supervise(self: Arc<Self>) {
        // Signaux de vie : un fil à part, que rien d'autre ne retarde (NVML,
        // arrêt d'une session…), pour que le chien de garde (3 s) ne se déclenche
        // pas en pleine session.
        let pinger = self.clone();
        std::thread::spawn(move || {
            let mut ping_failed = false;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                if !pinger.alive.load(Ordering::SeqCst) {
                    ping_failed = false;
                    continue;
                }
                match pinger.backend.ping() {
                    Ok(()) => ping_failed = false,
                    Err(e) if !ping_failed => {
                        ping_failed = true;
                        pinger.fail(&e);
                    }
                    Err(_) => {}
                }
            }
        });

        std::thread::spawn(move || {
            let mut idle_since: Option<Instant> = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let encoding = crate::gpu::encoding();
                let (active, expired_test, client_gone, origin) = {
                    let s = self.session.lock().unwrap();
                    match s.as_ref() {
                        Some(s) => (
                            true,
                            s.until.is_some_and(|u| Instant::now() > u),
                            s.last_seen.elapsed() > crate::activity::HEARTBEAT_GRACE,
                            Some(s.origin),
                        ),
                        None => (false, false, false, None),
                    }
                };

                if active {
                    self.watch_tick(Instant::now());
                    idle_since = if encoding { None } else { idle_since.or(Some(Instant::now())) };
                    let idle_long = idle_since.is_some_and(|t| t.elapsed() > Duration::from_secs(10));
                    if expired_test {
                        self.stop("fin du test");
                    } else if origin == Some(Origin::App) && client_gone && !encoding {
                        self.stop("l'appareil ne donne plus de nouvelles");
                    } else if origin == Some(Origin::Auto) && idle_long {
                        self.stop("plus aucun flux en cours");
                    }
                    continue;
                }

                idle_since = None;
                let settings = self.settings.lock().unwrap().clone();
                if settings.trigger == Trigger::All && encoding {
                    // Une opération en cours (l'écran d'une app en train d'être
                    // créé) a la priorité : on ne lui passe pas devant.
                    let Ok(_op) = self.ops.try_lock() else { continue };
                    if self.session.lock().unwrap().is_some() {
                        continue;
                    }
                    let mode = Mode { width: settings.width, height: settings.height, refresh: settings.refresh };
                    let _ = self.start_locked(Origin::Auto, vec![Screen::single(mode)], None, None, None, None);
                }
            }
        });
    }

    /// Arrêt du service : l'ordinateur retrouve ses écrans tout de suite,
    /// sans attendre le prochain démarrage.
    pub fn shutdown(&self) {
        self.stop("arrêt de la console");
    }
}


/// Hors Windows : pilote simulé, pour développer l'interface sur Mac. La
/// veille y voit le mode demandé ; les tests changent les écrans à la main.
#[cfg(not(windows))]
#[derive(Clone, Default)]
struct Simulated {
    screen: Arc<Mutex<SimScreen>>,
}

#[cfg(not(windows))]
#[derive(Default)]
struct SimScreen {
    /// Mode de l'écran virtuel principal, s'il existe.
    mode: Option<Mode>,
    /// Mode des autres écrans virtuels (rangs 1 à 3), s'ils existent.
    extra: [Option<Mode>; MAX_SCREENS - 1],
    /// Écrans physiques allumés.
    others: u32,
    watching: bool,
    /// Écrans surveillés par la veille (rangs, le principal en premier).
    watched: Vec<u32>,
    isolations: u32,
    mode_fixes: u32,
    /// Les isolements échouent (écran virtuel introuvable, par exemple).
    isolate_fails: bool,
    isolate_attempts: u32,
    /// Rangs dont la création échoue, et ceux que Windows n'active pas.
    create_fails: Vec<usize>,
    inactive: Vec<u32>,
    /// Écrans (rang, place) donnés au dernier isolement.
    last_isolate: Vec<(u32, i32, i32)>,
    /// Windows refuse les remises de mode (il garde le mode en place).
    refuse_modes: bool,
    /// Fabricant de la carte fixée pour le rendu.
    render_vendor: Option<u16>,
    /// Pings reçus, et écrans existants à chaque ping.
    pings: u32,
}

#[cfg(not(windows))]
impl SimScreen {
    fn slot_mut(&mut self, slot: usize) -> &mut Option<Mode> {
        if slot == 0 { &mut self.mode } else { &mut self.extra[slot - 1] }
    }
    fn exists(&self) -> usize {
        usize::from(self.mode.is_some()) + self.extra.iter().flatten().count()
    }
}

#[cfg(not(windows))]
fn sim_gdi_name(slot: u32) -> String {
    format!("\\\\.\\DISPLAY{}", 9 + slot)
}

#[cfg(not(windows))]
impl Backend for Simulated {
    fn driver(&self) -> Result<Value> {
        Ok(json!({ "version": "simulation", "watchdog_seconds": 3 }))
    }
    fn pin_render_adapter(&self, vendor: u16) -> Result<Option<String>> {
        self.screen.lock().unwrap().render_vendor = Some(vendor);
        Ok(Some(format!("(simulée) VEN_{vendor:04X}")))
    }
    fn create(&self, _: [u8; 16], slot: usize, mode: Mode) -> Result<Created> {
        let mut screen = self.screen.lock().unwrap();
        if screen.create_fails.contains(&slot) {
            bail!("(simulé) le pilote refuse l'écran {}", slot + 1);
        }
        tracing::info!("(simulé) création d'un écran {}×{} à {} Hz", mode.width, mode.height, mode.refresh);
        *screen.slot_mut(slot) = Some(mode);
        Ok(Created { luid_low: 0, luid_high: 0, target: slot as u32 })
    }
    fn remove(&self, guid: [u8; 16]) -> Result<()> {
        let slot = (0..MAX_SCREENS).find(|i| slot_guid(*i) == guid).unwrap_or(0);
        *self.screen.lock().unwrap().slot_mut(slot) = None;
        Ok(())
    }
    fn ping(&self) -> Result<()> {
        self.screen.lock().unwrap().pings += 1;
        Ok(())
    }
    fn isolate(&self, screens: &[(Created, Screen)]) -> Result<Value> {
        let mut screen = self.screen.lock().unwrap();
        screen.isolate_attempts += 1;
        if screen.isolate_fails {
            bail!("(simulé) l'écran virtuel n'apparaît pas dans la configuration d'affichage");
        }
        let disabled = screen.others;
        screen.others = 0;
        screen.isolations += 1;
        screen.last_isolate = screens.iter().map(|(c, s)| (c.target, s.x, s.y)).collect();
        let mut list = Vec::new();
        for (c, s) in screens {
            if screen.inactive.contains(&c.target) {
                list.push(Value::Null);
                continue;
            }
            *screen.slot_mut(c.target as usize) = Some(s.mode());
            list.push(json!({ "gdi_name": sim_gdi_name(c.target), "width": s.width, "height": s.height, "refresh": s.refresh, "x": s.x, "y": s.y }));
        }
        let (_, primary) = screens[0];
        let mut applied = json!({ "gdi_name": sim_gdi_name(0), "width": primary.width, "height": primary.height, "refresh": primary.refresh, "disabled_others": disabled });
        if screens.len() > 1 {
            applied["screens"] = Value::Array(list);
        }
        Ok(applied)
    }
    fn restore(&self) -> Result<()> {
        tracing::info!("(simulé) restauration des écrans");
        Ok(())
    }
    fn watch(&self, created: &[Created]) {
        let mut screen = self.screen.lock().unwrap();
        screen.watching = true;
        screen.watched = created.iter().map(|c| c.target).collect();
    }
    fn unwatch(&self) {
        let mut screen = self.screen.lock().unwrap();
        screen.watching = false;
        screen.watched.clear();
    }
    fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode> {
        let mut screen = self.screen.lock().unwrap();
        let slot = (0..MAX_SCREENS as u32).find(|i| sim_gdi_name(*i) == gdi_name).unwrap_or(0);
        screen.mode_fixes += 1;
        if screen.refuse_modes {
            return Ok(screen.slot_mut(slot as usize).unwrap_or(Mode { width: 0, height: 0, refresh: 0 }));
        }
        *screen.slot_mut(slot as usize) = Some(mode);
        Ok(mode)
    }
    fn observe(&self) -> Option<Observed> {
        let mut screen = self.screen.lock().unwrap();
        let mode = screen.mode.filter(|_| screen.watching)?;
        let watched = screen.watched.clone();
        let extra = watched
            .iter()
            .skip(1)
            .map(|slot| (screen.slot_mut(*slot as usize).unwrap_or(Mode { width: 0, height: 0, refresh: 0 }), sim_gdi_name(*slot)))
            .collect();
        let virtuals = screen.exists() as u32;
        Some(Observed { mode, gdi_name: sim_gdi_name(0), displays: virtuals + screen.others, others: screen.others, extra })
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    pub struct WinBackend {
        data_dir: PathBuf,
        device: Mutex<Option<sudovda::Device>>,
        watch: watch::Watch,
    }

    impl WinBackend {
        pub fn new(data_dir: PathBuf) -> Self {
            Self { watch: watch::Watch::new(&data_dir), data_dir, device: Mutex::new(None) }
        }

        fn with_device<T>(&self, f: impl FnOnce(&sudovda::Device) -> Result<T>) -> Result<T> {
            let mut guard = self.device.lock().unwrap();
            if guard.is_none() {
                *guard = Some(sudovda::Device::open()?);
            }
            let result = f(guard.as_ref().unwrap());
            if result.is_err() {
                // Le pilote a pu être rechargé : on rouvrira au prochain appel.
                *guard = None;
            }
            result
        }
    }

    impl Backend for WinBackend {
        fn driver(&self) -> Result<Value> {
            self.with_device(|d| {
                let (timeout, _) = d.watchdog().unwrap_or((0, 0));
                Ok(json!({ "version": d.protocol_version()?, "watchdog_seconds": timeout }))
            })
        }
        fn pin_render_adapter(&self, vendor: u16) -> Result<Option<String>> {
            let adapters = agent::run_with_timeout(&self.data_dir, &["adapters".to_string()], 10_000)?;
            let Some((low, high, path)) = pick_adapter(&adapters, vendor) else { return Ok(None) };
            self.with_device(|d| d.set_render_adapter(low, high))?;
            Ok(Some(path))
        }
        fn create(&self, guid: [u8; 16], slot: usize, mode: Mode) -> Result<Created> {
            let c = self.with_device(|d| d.add(guid, slot, mode.width, mode.height, mode.refresh))?;
            Ok(Created { luid_low: c.adapter_luid_low, luid_high: c.adapter_luid_high, target: c.target_id })
        }
        fn remove(&self, guid: [u8; 16]) -> Result<()> {
            self.with_device(|d| d.remove(guid))
        }
        fn ping(&self) -> Result<()> {
            self.with_device(|d| d.ping())
        }
        fn isolate(&self, screens: &[(Created, Screen)]) -> Result<Value> {
            // Un écran : l'isolement de toujours.
            if let [(c, s)] = screens {
                let args = [
                    "isolate".to_string(),
                    "--luid-low".into(), c.luid_low.to_string(),
                    "--luid-high".into(), c.luid_high.to_string(),
                    "--target".into(), c.target.to_string(),
                    "--width".into(), s.width.to_string(),
                    "--height".into(), s.height.to_string(),
                    "--refresh".into(), s.refresh.to_string(),
                ];
                return agent::run(&self.data_dir, &args);
            }
            let mut args = vec!["isolate-many".to_string()];
            for (c, s) in screens {
                args.push("--screen".into());
                args.push(Placement { created: *c, mode: s.mode(), x: s.x, y: s.y }.to_arg());
            }
            agent::run(&self.data_dir, &args)
        }
        fn restore(&self) -> Result<()> {
            agent::run(&self.data_dir, &["restore".to_string()]).map(|_| ())
        }
        fn watch(&self, created: &[Created]) {
            self.watch.start(&self.data_dir, created);
        }
        fn unwatch(&self) {
            self.watch.stop();
        }
        fn observe(&self) -> Option<Observed> {
            self.watch.observe(&self.data_dir)
        }
        fn set_mode(&self, gdi_name: &str, mode: Mode) -> Result<Mode> {
            if gdi_name.is_empty() {
                bail!("écran virtuel introuvable pour remettre son mode");
            }
            let args = [
                "set-mode".to_string(),
                "--gdi-name".into(), gdi_name.to_string(),
                "--width".into(), mode.width.to_string(),
                "--height".into(), mode.height.to_string(),
                "--refresh".into(), mode.refresh.to_string(),
            ];
            let v = agent::run_with_timeout(&self.data_dir, &args, 10_000)?;
            let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
            Ok(Mode { width: n("width"), height: n("height"), refresh: n("refresh") })
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn manager() -> Arc<Manager> {
        let dir = std::env::temp_dir().join(format!("kn-display-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        Manager::new(dir, VdSettings::default())
    }

    const MAC: Mode = Mode { width: 3456, height: 2234, refresh: 120 };
    const TV: Mode = Mode { width: 3840, height: 2160, refresh: 60 };
    const GAME: Mode = Mode { width: 1920, height: 1080, refresh: 60 };

    /// Gestionnaire sur un écran simulé que le test peut changer, et le fil
    /// des événements qu'il écrit.
    fn simulated(settings: VdSettings) -> (Arc<Manager>, Simulated, Arc<Mutex<Vec<String>>>) {
        let dir = std::env::temp_dir().join(format!("kn-display-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let sim = Simulated::default();
        let m = Manager::with_backend(dir, settings, Box::new(sim.clone()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let feed = events.clone();
        m.on_event(Arc::new(move |_: &str, msg: Msg| feed.lock().unwrap().push(msg.to_string())));
        (m, sim, events)
    }

    fn screen_mode(sim: &Simulated) -> Mode {
        sim.screen.lock().unwrap().mode.unwrap()
    }

    #[test]
    fn mode_change_en_session_politique_follow() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Follow)).unwrap();
        let t0 = Instant::now();
        m.watch_tick(t0);
        assert_eq!(m.observed().unwrap().mode, TV);
        assert_eq!(m.status()["mode"]["width"], 3840);
        assert_eq!(m.status()["displays"], 1);

        // Un jeu passe l'écran virtuel en 1080p : la console laisse faire et l'annonce une fois.
        sim.screen.lock().unwrap().mode = Some(GAME);
        for i in 0..8 {
            m.watch_tick(t0 + Duration::from_secs(1 + i));
        }
        assert_eq!(screen_mode(&sim), GAME);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 0);
        assert_eq!(m.observed().unwrap().mode, GAME);
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.len(), 1, "{feed:?}");
        assert!(feed[0].contains("1920 × 1080") && feed[0].contains("laissé tel quel"), "{feed:?}");

        // Un prepare au même mode (Kyber redémarré) ne touche pas au mode en follow.
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert_eq!(screen_mode(&sim), GAME);

        // Le jeu rend la main : retour au mode demandé, plus rien à annoncer ;
        // un nouveau changement est annoncé de nouveau.
        sim.screen.lock().unwrap().mode = Some(TV);
        m.watch_tick(t0 + Duration::from_secs(10));
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(11));
        assert_eq!(events.lock().unwrap().len(), 2);
        // 59 Hz pour 60 : le même mode.
        sim.screen.lock().unwrap().mode = Some(Mode { refresh: 59, ..TV });
        m.watch_tick(t0 + Duration::from_secs(12));
        assert_eq!(events.lock().unwrap().len(), 2);
    }

    #[test]
    fn mode_change_en_session_politique_restore() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        m.watch_tick(t0);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 0);

        // Le mode change : remis aussitôt.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(1));
        assert_eq!(screen_mode(&sim), TV);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
        assert!(events.lock().unwrap()[0].contains("remis en 3840 × 2160"));

        // Le jeu insiste : au plus une remise toutes les 5 s.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(2));
        m.watch_tick(t0 + Duration::from_secs(5));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
        assert_eq!(screen_mode(&sim), GAME);
        m.watch_tick(t0 + Duration::from_secs(6));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 2);
        assert_eq!(screen_mode(&sim), TV);
        // Annoncé une seule fois tant que le mode inattendu est le même.
        assert_eq!(events.lock().unwrap().len(), 1);

        // « Remettre » dans l'app : un prepare au même mode remet tout de suite.
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(7));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 2);
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
        assert_eq!(screen_mode(&sim), TV);

        // Profil Jeu choisi en session (signal de vie) : la console laisse faire.
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Follow)));
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(20));
        assert_eq!(screen_mode(&sim), GAME);
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
    }

    #[test]
    fn politique_restore_face_a_un_jeu_qui_insiste() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        // Le jeu repasse en 1080p après chaque remise ; la veille voit les deux.
        for i in 0..12u64 {
            let at = t0 + Duration::from_secs(i * 3);
            sim.screen.lock().unwrap().mode = Some(GAME);
            m.watch_tick(at);
            m.watch_tick(at + Duration::from_secs(1));
        }
        // Trois remises, puis la console laisse faire pour le reste de la session.
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        assert_eq!(m.status()["session"]["mode_policy"], "follow");
        let feed = events.lock().unwrap().clone();
        assert!(feed.last().unwrap().contains("laissée telle quelle"), "{feed:?}");
        assert!(feed.len() <= 5, "{feed:?}");
        // Le signal de vie (profil Bureau) ne relance pas la ronde…
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Restore)));
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
        // … « Remettre » dans l'app, si.
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        assert_eq!(screen_mode(&sim), TV);
    }

    #[test]
    fn profil_sans_politique_revient_au_reglage() {
        let (m, _, _) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        // Profil Max choisi en session : le réglage de la console (follow).
        assert!(m.heartbeat(Some("tv"), Some(PolicyChoice::Default)));
        assert_eq!(m.policy(), Some(ModePolicy::Follow));
    }

    #[test]
    fn politique_par_defaut_du_reglage() {
        let settings = VdSettings { on_mode_change: ModePolicy::Restore, ..VdSettings::default() };
        let (m, sim, _) = simulated(settings);
        // Client sans politique (navigateur, app ancienne) : celle du réglage.
        m.start(Origin::App, TV, None, Some("web".into()), None, None).unwrap();
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
        assert_eq!(m.status()["session"]["mode_policy"], "restore");
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(Instant::now());
        assert_eq!(screen_mode(&sim), TV);
        // Un signal de vie sans politique ne la change pas.
        assert!(m.heartbeat(Some("web"), None));
        assert_eq!(m.policy(), Some(ModePolicy::Restore));
    }

    #[test]
    fn ecran_physique_revenu_en_session() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);
        let t0 = Instant::now();

        // Un jeu a choisi 1080p, puis Windows rallume l'écran physique.
        sim.screen.lock().unwrap().mode = Some(GAME);
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0);
        assert_eq!(m.observed().unwrap().displays, 2);
        assert_eq!(m.status()["displays"], 2);
        // Pas avant 2 s de stabilité.
        m.watch_tick(t0 + Duration::from_millis(1500));
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);
        m.watch_tick(t0 + Duration::from_secs(2));
        {
            let screen = sim.screen.lock().unwrap();
            assert_eq!(screen.isolations, 2);
            assert_eq!(screen.others, 0);
            // Politique follow : le mode choisi par le jeu est gardé.
            assert_eq!(screen.mode, Some(GAME));
        }
        assert!(events.lock().unwrap().iter().any(|e| e.contains("rallumé")), "{:?}", events.lock().unwrap());

        // Un écran qui clignote (rallumé moins de 2 s) est laissé tranquille.
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0 + Duration::from_secs(3));
        sim.screen.lock().unwrap().others = 0;
        m.watch_tick(t0 + Duration::from_secs(4));
        sim.screen.lock().unwrap().others = 1;
        m.watch_tick(t0 + Duration::from_secs(5));
        // Stable depuis 2 s, mais au plus un isolement toutes les 10 s.
        m.watch_tick(t0 + Duration::from_secs(7));
        assert_eq!(sim.screen.lock().unwrap().isolations, 2);
        m.watch_tick(t0 + Duration::from_secs(12));
        assert_eq!(sim.screen.lock().unwrap().isolations, 3);

        // Fin de session : plus de veille.
        m.stop("fin du test");
        assert!(m.observed().is_none());
        assert!(!sim.screen.lock().unwrap().watching);
        assert!(m.status()["mode"].is_null());
    }

    #[test]
    fn ecran_physique_qui_revient_sans_cesse() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        let t0 = Instant::now();
        // Windows le rallume juste après chaque isolement.
        for i in 0..40u64 {
            sim.screen.lock().unwrap().others = 1;
            m.watch_tick(t0 + Duration::from_secs(i * 3));
        }
        // Cinq isolements en deux minutes, puis la console renonce, une seule annonce.
        assert_eq!(sim.screen.lock().unwrap().isolations, 1 + 5);
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.iter().filter(|e| e.contains("sans cesse")).count(), 1, "{feed:?}");
    }

    #[test]
    fn ecran_physique_qui_ne_s_eteint_pas() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        sim.screen.lock().unwrap().isolate_fails = true;
        sim.screen.lock().unwrap().others = 1;
        let t0 = Instant::now();
        let attempts = |sim: &Simulated| sim.screen.lock().unwrap().isolate_attempts - 1;
        // Premier essai après 2 s de stabilité, puis 20 s et 40 s plus tard.
        m.watch_tick(t0);
        m.watch_tick(t0 + Duration::from_secs(2));
        assert_eq!(attempts(&sim), 1);
        m.watch_tick(t0 + Duration::from_secs(21));
        assert_eq!(attempts(&sim), 1);
        m.watch_tick(t0 + Duration::from_secs(22));
        assert_eq!(attempts(&sim), 2);
        m.watch_tick(t0 + Duration::from_secs(61));
        assert_eq!(attempts(&sim), 2);
        m.watch_tick(t0 + Duration::from_secs(62));
        assert_eq!(attempts(&sim), 3);
        // Puis plus rien : la console a renoncé, deux annonces en tout.
        for i in 63..400u64 {
            m.watch_tick(t0 + Duration::from_secs(i));
        }
        assert_eq!(attempts(&sim), 3);
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.len(), 2, "{feed:?}");
        assert!(feed[1].contains("trois essais"), "{feed:?}");
    }

    #[test]
    fn ecran_physique_revenu_politique_restore() {
        let (m, sim, _) = simulated(VdSettings::default());
        m.start(Origin::App, TV, None, Some("tv".into()), None, Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        sim.screen.lock().unwrap().others = 2;
        sim.screen.lock().unwrap().mode = Some(GAME);
        m.watch_tick(t0);
        m.watch_tick(t0 + Duration::from_secs(2));
        // L'isolement remet aussi le mode de la session.
        let screen = sim.screen.lock().unwrap();
        assert_eq!((screen.isolations, screen.others, screen.mode), (2, 0, Some(TV)));
    }

    #[test]
    fn une_app_garde_son_ecran_face_au_declenchement_automatique() {
        let m = manager();
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None, None).unwrap();
        // Le fil de surveillance (mode par défaut) ne remplace pas l'écran de l'app.
        m.start(Origin::Auto, TV, None, None, None, None).unwrap();
        let status = m.status();
        assert_eq!(status["session"]["mode"]["width"], 3456);
        assert_eq!(status["session"]["origin"], "app");
        // Même mode en « Auto » : l'origine reste celle de l'app.
        m.start(Origin::Auto, MAC, None, None, None, None).unwrap();
        assert_eq!(m.status()["session"]["origin"], "app");
        // Un essai depuis la console est refusé pendant la session de l'app.
        assert!(m.start(Origin::Test, TV, None, None, Some(Duration::from_secs(5)), None).is_err());
    }

    #[test]
    fn signal_de_vie_du_proprietaire_seulement() {
        let m = manager();
        assert!(!m.heartbeat(Some("mac"), None));
        m.start(Origin::App, MAC, Some("matthieu".into()), Some("mac".into()), None, None).unwrap();
        assert!(m.heartbeat(Some("mac"), None));
        assert!(m.owned_by("mac"));
        // Un autre appareil prend la main : le premier n'a plus « son » écran.
        m.start(Origin::App, TV, Some("matthieu".into()), Some("tv".into()), None, None).unwrap();
        assert!(!m.heartbeat(Some("mac"), None));
        assert!(m.heartbeat(Some("tv"), None));
        // Une app d'avant 0.2.5 (sans identifiant) est crue sur parole.
        assert!(m.heartbeat(None, None));
        m.stop("fin du test");
        assert!(!m.heartbeat(Some("tv"), None));
    }

    // --- Multi-écrans ---

    fn scr(width: u32, height: u32, refresh: u32, x: i32, y: i32, primary: bool) -> Screen {
        Screen { width, height, refresh, x, y, primary }
    }

    /// L'ordinateur d'essai de l'auteur : XG32UQ au centre, Studio Display à
    /// droite, MacBook Pro à gauche, plus bas.
    fn trois_ecrans() -> Vec<Screen> {
        vec![
            scr(3840, 2160, 160, 0, 0, true),
            scr(5120, 2880, 60, 3840, 0, false),
            scr(3456, 2234, 60, -3456, 400, false),
        ]
    }

    fn refus(screens: &[Screen]) -> String {
        validate_screens(screens).unwrap_err().to_string()
    }

    #[test]
    fn dispositions_valides() {
        validate_screens(&trois_ecrans()).unwrap();
        validate_screens(&[scr(1920, 1080, 60, 0, 0, true)]).unwrap();
        // Au-dessus, décalé ; quatre écrans en carré.
        validate_screens(&[scr(1920, 1080, 60, 0, 0, true), scr(2560, 1440, 60, -300, -1440, false)]).unwrap();
        validate_screens(&[
            scr(1920, 1080, 60, 0, 0, true),
            scr(1920, 1080, 60, 1920, 0, false),
            scr(1920, 1080, 60, 0, 1080, false),
            scr(1920, 1080, 60, 1920, 1080, false),
        ])
        .unwrap();
        // Relié par un voisin, pas directement au principal.
        validate_screens(&[
            scr(1920, 1080, 60, 0, 0, true),
            scr(1920, 1080, 60, 1920, 0, false),
            scr(1920, 1080, 60, 3840, 500, false),
        ])
        .unwrap();
    }

    #[test]
    fn dispositions_refusees() {
        let p = scr(1920, 1080, 60, 0, 0, true);
        assert!(refus(&[]).contains("1 à 4"));
        let cinq: Vec<Screen> =
            std::iter::once(p).chain((1..5).map(|i| scr(1920, 1080, 60, 1920 * i, 0, false))).collect();
        assert!(refus(&cinq).contains("1 à 4"));
        // Aucun ou deux principaux.
        assert!(refus(&[scr(1920, 1080, 60, 0, 0, false)]).contains("principal"));
        assert!(refus(&[p, scr(1920, 1080, 60, 1920, 0, true)]).contains("principal"));
        // Principal ailleurs qu'en (0, 0).
        assert!(refus(&[scr(1920, 1080, 60, 10, 0, true)]).contains("(0, 0)"));
        // Mode hors limites.
        assert!(refus(&[p, scr(8000, 1080, 60, 1920, 0, false)]).contains("résolution"));
        assert!(refus(&[p, scr(1920, 1080, 10, 1920, 0, false)]).contains("fréquence"));
        // Chevauchement.
        assert!(refus(&[p, scr(1920, 1080, 60, 1000, 0, false)]).contains("chevauchent"));
        // Coin contre coin, écart, chaîne coupée.
        assert!(refus(&[p, scr(1920, 1080, 60, 1920, 1080, false)]).contains("bord"));
        assert!(refus(&[p, scr(1920, 1080, 60, 1921, 0, false)]).contains("bord"));
        assert!(refus(&[p, scr(1920, 1080, 60, 1920, 1080, false), scr(1920, 1080, 60, -1920, 1080, false)]).contains("bord"));
    }

    #[test]
    fn arguments_de_l_agent() {
        let p = Placement {
            created: Created { luid_low: 4_000_000_000, luid_high: -2, target: 257 },
            mode: Mode { width: 3456, height: 2234, refresh: 60 },
            x: -3456,
            y: 400,
        };
        assert_eq!(p.to_arg(), "4000000000,-2,257,3456,2234,60,-3456,400");
        assert_eq!(Placement::parse(&p.to_arg()).unwrap(), p);
        assert_eq!(Created::parse(&p.created.to_arg()).unwrap(), p.created);
        assert!(Placement::parse("1,2,3").is_err());
        assert!(Created::parse("1,x,3").is_err());
        assert!(Created::parse("-1,0,0").is_err());
    }

    #[test]
    fn ecran_impose_reste_colle() {
        let created = Created { luid_low: 0, luid_high: 0, target: 0 };
        let at = |w, h, x, y| Placement { created, mode: Mode { width: w, height: h, refresh: 60 }, x, y };
        // MacBook à gauche, mis en 3840 × 2160 par Windows : bord droit gardé.
        assert_eq!(at(3456, 2234, -3456, 322).anchored(3840, 2160), (-3840, 322));
        // À droite : bord gauche gardé, rien ne bouge.
        assert_eq!(at(5120, 2880, 3840, 0).anchored(3840, 2160), (3840, 0));
        // Au-dessus : bord bas gardé.
        assert_eq!(at(2560, 1440, 0, -1440).anchored(1920, 1080), (0, -1080));
        // En dessous, à gauche du bord : seul x suit.
        assert_eq!(at(1920, 1080, -1920, 2160).anchored(1280, 720), (-1280, 2160));
    }

    #[test]
    fn carte_de_rendu_des_ecrans() {
        assert_eq!(encoder_vendor("nvenc"), Some(0x10DE));
        assert_eq!(encoder_vendor("x264"), None);
        assert_eq!(path_vendor(r"\\?\PCI#VEN_10DE&DEV_2208&SUBSYS_38971462&REV_A1#4&1a2b#{5b45201d}"), Some(0x10DE));
        assert_eq!(path_vendor(r"\\?\root#display#0000#{5b45201d}"), None);
        let adapters = json!({ "adapters": [
            { "luid_low": 70000, "luid_high": 0, "path": r"\\?\ROOT#DISPLAY#0000#{x}" },
            { "luid_low": 1234, "luid_high": 0, "path": r"\\?\PCI#VEN_8086&DEV_4680#{x}" },
            { "luid_low": 5678, "luid_high": 1, "path": r"\\?\PCI#VEN_10DE&DEV_2208#{x}" },
        ] });
        assert_eq!(pick_adapter(&adapters, 0x10DE).map(|(l, h, _)| (l, h)), Some((5678, 1)));
        assert_eq!(pick_adapter(&adapters, 0x1002), None);

        // Fixée seulement pour une session à plusieurs écrans.
        let (m, sim, _) = simulated(VdSettings::default());
        m.set_render_vendor(Some(0x10DE));
        m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        assert_eq!(sim.screen.lock().unwrap().render_vendor, None);
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        assert_eq!(sim.screen.lock().unwrap().render_vendor, Some(0x10DE));
    }

    #[test]
    fn identifiants_des_ecrans() {
        assert_eq!(&slot_guid(0), b"KyberNow-Display");
        assert_eq!(&slot_guid(1), b"02-KyberNow-Disp");
        assert_eq!(&slot_guid(3), b"04-KyberNow-Disp");
        let all: Vec<_> = (0..MAX_SCREENS).map(slot_guid).collect();
        for (i, g) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(g));
            // Numéro de série de l'EDID (4 premiers octets) propre à chaque écran.
            assert!(!all[i + 1..].iter().any(|o| o[..4] == g[..4]));
        }
    }

    #[test]
    fn session_a_trois_ecrans() {
        let (m, sim, events) = simulated(VdSettings::default());
        let started = m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        // Dans l'ordre de la demande, à leur mode et à leur place.
        assert_eq!(started.screens, trois_ecrans().into_iter().map(Some).collect::<Vec<_>>());
        assert_eq!(started.applied["width"], 3840);
        assert!(started.applied.get("screens").is_none());
        assert!(m.alive.load(Ordering::SeqCst));
        {
            let screen = sim.screen.lock().unwrap();
            assert_eq!(screen.exists(), 3);
            assert_eq!(screen.isolations, 1);
            assert_eq!(screen.last_isolate, vec![(0, 0, 0), (1, 3840, 0), (2, -3456, 400)]);
            assert_eq!(screen.watched, vec![0, 1, 2]);
        }
        let record: Record = serde_json::from_slice(&std::fs::read(m.record_path()).unwrap()).unwrap();
        assert_eq!(record.guids(), vec![slot_guid(0), slot_guid(1), slot_guid(2)]);

        m.watch_tick(Instant::now());
        let status = m.status();
        assert_eq!(status["displays"], 3);
        assert_eq!(status["session"]["mode"]["refresh"], 160);
        assert_eq!(status["session"]["screens"].as_array().unwrap().len(), 3);
        assert_eq!(status["session"]["screens"][1]["width"], 5120);
        assert_eq!(status["session"]["screens"][2]["x"], -3456);
        assert_eq!(status["session"]["screens"][0]["primary"], true);
        assert!(status["session"]["screens"][1].get("primary").is_none());
        assert!(events.lock().unwrap().is_empty());

        // Kyber redémarré : mêmes écrans redemandés, la session continue.
        let again = m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        assert_eq!(again.screens, started.screens);
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);

        // Deux écrans seulement (Studio Display débranché) : la session se refait.
        let deux = trois_ecrans()[..2].to_vec();
        let started = m.start_screens(Origin::App, deux, None, Some("mac".into()), None).unwrap();
        assert_eq!(started.screens.len(), 2);
        assert_eq!(sim.screen.lock().unwrap().exists(), 2);
        assert_eq!(sim.screen.lock().unwrap().isolations, 2);

        // Fin de session : tous les écrans retirés.
        m.stop("fin du test");
        assert_eq!(sim.screen.lock().unwrap().exists(), 0);
        assert!(!m.alive.load(Ordering::SeqCst));
        assert!(!m.record_path().exists());
    }

    #[test]
    fn principal_donne_en_dernier() {
        let (m, sim, _) = simulated(VdSettings::default());
        let mut screens = trois_ecrans();
        screens.rotate_left(1);
        let started = m.start_screens(Origin::App, screens.clone(), None, Some("mac".into()), None).unwrap();
        assert_eq!(started.screens, screens.iter().copied().map(Some).collect::<Vec<_>>());
        // Le principal est créé le premier, avec l'identifiant de toujours.
        assert_eq!(sim.screen.lock().unwrap().mode.unwrap().refresh, 160);
        assert_eq!(sim.screen.lock().unwrap().last_isolate[0], (0, 0, 0));
        assert_eq!(m.status()["session"]["mode"]["refresh"], 160);
    }

    #[test]
    fn un_ecran_refuse_par_le_pilote() {
        let (m, sim, events) = simulated(VdSettings::default());
        sim.screen.lock().unwrap().create_fails = vec![1];
        let started = m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        assert_eq!(started.screens[0], Some(trois_ecrans()[0]));
        assert_eq!(started.screens[1], None);
        assert_eq!(started.screens[2], Some(trois_ecrans()[2]));
        assert_eq!(sim.screen.lock().unwrap().watched, vec![0, 2]);
        let feed = events.lock().unwrap().clone();
        assert_eq!(feed.len(), 1, "{feed:?}");
        assert!(feed[0].contains("1 des 3"), "{feed:?}");

        // La veille suit l'écran 2 au deuxième rang de la session.
        sim.screen.lock().unwrap().extra[1] = Some(GAME);
        m.watch_tick(Instant::now());
        assert_eq!(m.status()["session"]["screens"][1]["width"], 1920);
    }

    #[test]
    fn ecran_principal_refuse_par_le_pilote() {
        let (m, sim, _) = simulated(VdSettings::default());
        sim.screen.lock().unwrap().create_fails = vec![0];
        assert!(m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).is_err());
        assert_eq!(sim.screen.lock().unwrap().exists(), 0);
        assert!(m.status()["session"].is_null());
        assert!(!m.alive.load(Ordering::SeqCst));
    }

    #[test]
    fn un_ecran_que_windows_n_active_pas() {
        let (m, sim, events) = simulated(VdSettings::default());
        sim.screen.lock().unwrap().inactive = vec![1, 2];
        let started = m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        assert_eq!(started.screens, vec![Some(trois_ecrans()[0]), None, None]);
        // Retirés du pilote et du fichier de session.
        assert_eq!(sim.screen.lock().unwrap().exists(), 1);
        let record: Record = serde_json::from_slice(&std::fs::read(m.record_path()).unwrap()).unwrap();
        assert_eq!(record.guids(), vec![slot_guid(0)]);
        assert!(events.lock().unwrap()[0].contains("2 des 3"));
    }

    #[test]
    fn isolement_rate_retire_tous_les_ecrans() {
        let (m, sim, _) = simulated(VdSettings::default());
        sim.screen.lock().unwrap().isolate_fails = true;
        assert!(m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).is_err());
        assert_eq!(sim.screen.lock().unwrap().exists(), 0);
        assert!(!m.record_path().exists());
    }

    #[test]
    fn politique_restore_sur_chaque_ecran() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), Some(PolicyChoice::Restore)).unwrap();
        let t0 = Instant::now();
        m.watch_tick(t0);
        // Le Studio Display passe en 1080p : remis, sans toucher aux autres.
        sim.screen.lock().unwrap().extra[0] = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(1));
        {
            let screen = sim.screen.lock().unwrap();
            assert_eq!(screen.mode_fixes, 1);
            assert_eq!(screen.extra[0], Some(trois_ecrans()[1].mode()));
            assert_eq!(screen.mode.unwrap().refresh, 160);
        }
        assert!(events.lock().unwrap()[0].contains("remis en 5120 × 2880"));
        // Deux écrans changés au même tour : une seule remise comptée.
        sim.screen.lock().unwrap().mode = Some(GAME);
        sim.screen.lock().unwrap().extra[1] = Some(GAME);
        m.watch_tick(t0 + Duration::from_secs(7));
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 3);
        assert_eq!(m.session.lock().unwrap().as_ref().unwrap().mode_fixes.len(), 2);
    }

    #[test]
    fn politique_restore_une_remise_au_plus_si_windows_garde_son_mode() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), Some(PolicyChoice::Restore)).unwrap();
        // Windows passe le 5K en 3840 × 2160 à 59 Hz et refuse d'y revenir.
        let kept = Mode { width: 3840, height: 2160, refresh: 59 };
        sim.screen.lock().unwrap().extra[0] = Some(kept);
        sim.screen.lock().unwrap().refuse_modes = true;
        let t0 = Instant::now();
        for i in 0..120u64 {
            m.watch_tick(t0 + Duration::from_secs(i));
        }
        // Une seule remise ; le mode gardé par Windows devient la référence.
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
        assert_eq!(m.status()["session"]["screens"][1]["width"], 3840);
        assert_eq!(events.lock().unwrap().len(), 1, "{:?}", events.lock().unwrap());
        // Un prepare au même mode (« Remettre ») ne relance rien : la référence est le mode gardé.
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), Some(PolicyChoice::Restore)).unwrap();
        for i in 120..180u64 {
            m.watch_tick(t0 + Duration::from_secs(i));
        }
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 1);
    }

    #[test]
    fn politique_follow_sur_un_ecran_secondaire() {
        let (m, sim, events) = simulated(VdSettings::default());
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), Some(PolicyChoice::Follow)).unwrap();
        sim.screen.lock().unwrap().extra[1] = Some(GAME);
        for i in 0..5 {
            m.watch_tick(Instant::now() + Duration::from_secs(i));
        }
        assert_eq!(sim.screen.lock().unwrap().mode_fixes, 0);
        assert_eq!(events.lock().unwrap().len(), 1);
        assert_eq!(m.status()["session"]["screens"][2]["width"], 1920);
    }

    #[test]
    fn ecran_physique_revenu_en_session_a_plusieurs_ecrans() {
        let (m, sim, _) = simulated(VdSettings::default());
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), Some(PolicyChoice::Follow)).unwrap();
        let t0 = Instant::now();
        sim.screen.lock().unwrap().others = 1;
        sim.screen.lock().unwrap().extra[0] = Some(GAME);
        m.watch_tick(t0);
        assert_eq!(m.status()["displays"], 4);
        m.watch_tick(t0 + Duration::from_secs(2));
        let screen = sim.screen.lock().unwrap();
        assert_eq!(screen.isolations, 2);
        assert_eq!(screen.others, 0);
        // Tous les écrans reprennent leur mode et leur place.
        assert_eq!(screen.last_isolate, vec![(0, 0, 0), (1, 3840, 0), (2, -3456, 400)]);
        assert_eq!(screen.extra[0], Some(trois_ecrans()[1].mode()));
    }

    #[test]
    fn reprise_apres_plantage_retire_tous_les_ecrans() {
        let (m, sim, _) = simulated(VdSettings::default());
        m.start_screens(Origin::App, trois_ecrans(), None, Some("mac".into()), None).unwrap();
        // Le service redémarre sans avoir fini la session.
        let again = Manager::with_backend(m.data_dir.clone(), VdSettings::default(), Box::new(sim.clone()));
        again.recover();
        assert_eq!(sim.screen.lock().unwrap().exists(), 0);
        assert!(!again.record_path().exists());
        // Fichier d'une version à un écran.
        let old: Record = serde_json::from_str(&format!("{{\"guid\":\"{}\"}}", hex::encode(slot_guid(0)))).unwrap();
        assert_eq!(old.guids(), vec![slot_guid(0)]);
    }

    #[test]
    fn session_a_un_ecran_sans_changement() {
        let (m, sim, _) = simulated(VdSettings::default());
        let applied = m.start(Origin::App, TV, None, Some("tv".into()), None, None).unwrap();
        assert!(applied.get("screens").is_none());
        assert_eq!(sim.screen.lock().unwrap().exists(), 1);
        // Une liste d'un seul écran : la même session.
        let started = m.start_screens(Origin::App, vec![Screen::single(TV)], None, Some("tv".into()), None).unwrap();
        assert_eq!(started.screens, vec![Some(Screen::single(TV))]);
        assert_eq!(sim.screen.lock().unwrap().isolations, 1);
        let record: Record = serde_json::from_slice(&std::fs::read(m.record_path()).unwrap()).unwrap();
        assert!(record.guids.is_empty());
    }
}
