//! Prêt de l'ordinateur (KyberNow Share, `docs/pret.md`).
//!
//! Deux côtés dans l'icône :
//! - l'hôte (`host`) : « Prêter l'ordinateur… », le code, les demandes et
//!   « Reprendre l'ordinateur », dans le menu et le panneau de l'icône ;
//! - le **mode console** (`agent`, `kybernow-tray.exe --console`) : Admin lance
//!   cet exécutable dans la session Windows de l'utilisateur, avec son compte,
//!   quand un prêt commence. Il porte le lanceur (sur l'écran virtuel de l'ami),
//!   le cache des écrans du PC, les crochets clavier et souris, les fenêtres de
//!   l'utilisateur, la barre des tâches, le curseur, Steam et les fenêtres
//!   « Ouvrir ». Admin surveille les processus lui-même.
//!
//! Ce fichier garde la logique sans Windows (essayée sur le Mac) : la vue
//! d'Admin, l'enchaînement des phases, les textes, le cache et le tri des
//! touches des crochets.

pub mod host;
pub mod store;

#[cfg(windows)]
pub mod agent;
#[cfg(windows)]
mod steam;
#[cfg(windows)]
mod system;

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::i18n::{Lang, tr};

/// Phase du prêt vue par l'agent (`phase` de `GET /api/local/lend/console`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Preparing,
    Lent,
    Ending,
}

impl Phase {
    /// Une phase inconnue (Admin plus récent) vaut `idle` : l'agent se retire.
    pub fn parse(text: &str) -> Phase {
        match text {
            "preparing" => Phase::Preparing,
            "lent" => Phase::Lent,
            "ending" => Phase::Ending,
            _ => Phase::Idle,
        }
    }
}

/// L'écran virtuel de l'ami (coordonnées du bureau Windows, en pixels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    /// « \\.\DISPLAY5 » : le nom GDI de l'écran, pour le retrouver.
    pub gdi_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Display {
    /// Le point est-il sur cet écran ?
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && (x as i64) < self.x as i64 + self.width as i64 && (y as i64) < self.y as i64 + self.height as i64
    }
}

/// Une app permise, telle qu'Admin la décrit au lanceur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub id: String,
    pub name: String,
    /// Chemin de son icône chez Admin (`/api/local/lend/icons/<fichier>.png`).
    pub icon: Option<String>,
}

/// Préavis de 2 minutes : sa raison et l'heure de la fin (secondes Unix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub reason: String,
    pub at: i64,
}

/// Ce qu'Admin dit à l'agent (`GET /api/local/lend/console`).
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub phase: Phase,
    pub lend: String,
    pub borrower: String,
    pub host: String,
    pub computer: String,
    pub until: Option<i64>,
    pub notice: Option<Notice>,
    /// « Son compte Steam » : Steam passe sur le compte de l'ami.
    pub steam_friend: bool,
    /// `None` tant que l'écran virtuel n'est pas là.
    pub display: Option<Display>,
    pub apps: Vec<App>,
    pub steam_exe: Option<String>,
    pub lang: Lang,
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

impl View {
    pub fn parse(v: &Value, fallback: Lang) -> Option<View> {
        let phase = Phase::parse(v["phase"].as_str()?);
        let display = v["display"].as_object().and_then(|d| {
            Some(Display {
                gdi_name: d.get("gdi_name").map(text).unwrap_or_default(),
                x: i32::try_from(d.get("x")?.as_i64()?).ok()?,
                y: i32::try_from(d.get("y")?.as_i64()?).ok()?,
                width: u32::try_from(d.get("width")?.as_u64()?).ok().filter(|w| *w > 0)?,
                height: u32::try_from(d.get("height")?.as_u64()?).ok().filter(|h| *h > 0)?,
            })
        });
        let apps = v["apps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| {
                Some(App {
                    id: a["id"].as_str().filter(|s| !s.is_empty())?.to_string(),
                    name: text(&a["name"]),
                    icon: a["icon"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
                })
            })
            .collect();
        let notice = v["notice"].as_object().and_then(|n| {
            Some(Notice { reason: n.get("reason").map(text).unwrap_or_default(), at: n.get("at")?.as_i64()? })
        });
        Some(View {
            phase,
            lend: text(&v["lend"]),
            borrower: text(&v["borrower"]),
            host: text(&v["host"]),
            computer: text(&v["computer"]),
            until: v["until"].as_i64(),
            notice,
            steam_friend: v["steam"] == "friend",
            display,
            apps,
            steam_exe: v["steam_exe"].as_str().filter(|s| !s.is_empty()).map(str::to_owned),
            lang: v["lang"].as_str().map_or(fallback, Lang::from_tag),
        })
    }
}

/// Réponse d'Admin à un relevé.
#[derive(Debug, Clone, PartialEq)]
pub enum Reply {
    View(Box<View>),
    /// 404 : pas de prêt (comme `idle`).
    Gone,
    /// Pas de réponse (Admin arrêté, délai dépassé, jeton refusé…).
    Silent,
}

/// Ce que l'agent doit faire, dans l'ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Mise en place sans l'écran virtuel : cache, crochets, fenêtres réduites,
    /// barre des tâches cachée, Steam basculé.
    Begin,
    /// Le lanceur sur l'écran virtuel, le curseur gardé dessus.
    Show,
    /// `POST …/ready`.
    Ready,
    /// `POST …/beat`.
    Beat,
    /// Remise en état (aussi celle d'un agent tombé en plein prêt).
    TearDown,
    /// `POST …/ended`.
    Ended,
    /// `LockWorkStation` (Admin ne répond plus).
    Lock,
    Exit,
}

/// Admin muet depuis ce temps : remise en état, verrouillage, sortie.
pub const SILENCE: Duration = Duration::from_secs(60);
/// Signe de vie de l'agent pendant le prêt.
pub const BEAT: Duration = Duration::from_secs(2);

/// L'enchaînement des phases (`docs/pret.md`, agent de console) : ce qui
/// est fait, ce qui reste. L'agent appelle [`Machine::next`] à chaque relevé
/// et exécute les étapes ; un envoi raté se refait au relevé suivant
/// ([`Machine::retry`]).
#[derive(Debug, Default)]
pub struct Machine {
    /// Le prêt pour lequel l'agent est en place.
    lend: Option<String>,
    begun: bool,
    shown: bool,
    ready_sent: bool,
    torn_down: bool,
    ended_sent: bool,
    /// La mise en place a échoué (Steam n'a pas pu changer de compte) :
    /// pas de `ready`, Admin abandonne le prêt et demande la remise en état.
    broken: bool,
    /// Quelque chose est à remettre en état : mise en place faite, ou état
    /// laissé sur le disque par un agent tombé.
    dirty: bool,
    silent_since: Option<Instant>,
    last_beat: Option<Instant>,
}

impl Machine {
    /// `leftover` : un état du prêt traîne sur le disque (agent tombé).
    pub fn new(leftover: bool) -> Self {
        Machine { dirty: leftover, ..Default::default() }
    }

    pub fn begun(&self) -> bool {
        self.begun && !self.torn_down
    }

    /// Un envoi ou une étape a échoué : à refaire au prochain relevé.
    pub fn retry(&mut self, step: Step) {
        match step {
            Step::Ready => self.ready_sent = false,
            Step::Ended => self.ended_sent = false,
            Step::Beat => self.last_beat = None,
            Step::Show => self.shown = false,
            // Steam n'a pas changé de compte : jamais de `ready` pour ce prêt.
            Step::Begin => self.broken = true,
            _ => {}
        }
    }

    pub fn next(&mut self, reply: &Reply, now: Instant) -> Vec<Step> {
        let mut steps = Vec::new();
        let view = match reply {
            Reply::Silent => {
                let since = *self.silent_since.get_or_insert(now);
                if now.duration_since(since) >= SILENCE {
                    if self.dirty && !self.torn_down {
                        steps.push(Step::TearDown);
                        self.torn_down = true;
                    }
                    // Verrouiller seulement si un prêt a pu commencer.
                    if self.dirty {
                        steps.push(Step::Lock);
                    }
                    steps.push(Step::Exit);
                }
                return steps;
            }
            Reply::Gone => None,
            Reply::View(v) if v.phase == Phase::Idle => None,
            Reply::View(v) => Some(&**v),
        };
        self.silent_since = None;
        let Some(view) = view else {
            if self.dirty && !self.torn_down {
                steps.push(Step::TearDown);
                self.torn_down = true;
            }
            steps.push(Step::Exit);
            return steps;
        };
        // Un autre prêt que celui pour lequel l'agent est en place : remise en
        // état d'abord, puis le nouveau prêt de zéro.
        if self.begun && self.lend.as_deref() != Some(view.lend.as_str()) {
            if !self.torn_down {
                steps.push(Step::TearDown);
            }
            *self = Machine::new(false);
        }
        match view.phase {
            Phase::Preparing | Phase::Lent => {
                if self.torn_down {
                    // Remis en état (Admin muet un moment, puis revenu) : rien
                    // à refaire, Admin conclura.
                    return steps;
                }
                if !self.begun {
                    steps.push(Step::Begin);
                    self.begun = true;
                    self.dirty = true;
                    self.lend = Some(view.lend.clone());
                }
                if view.display.is_some() && !self.shown {
                    steps.push(Step::Show);
                    self.shown = true;
                }
                if view.phase == Phase::Preparing && self.shown && !self.ready_sent && !self.broken {
                    steps.push(Step::Ready);
                    self.ready_sent = true;
                }
                if view.phase == Phase::Lent && self.last_beat.is_none_or(|t| now.duration_since(t) >= BEAT) {
                    steps.push(Step::Beat);
                    self.last_beat = Some(now);
                }
            }
            Phase::Ending => {
                if !self.torn_down {
                    steps.push(Step::TearDown);
                    self.torn_down = true;
                }
                if !self.ended_sent {
                    steps.push(Step::Ended);
                    self.ended_sent = true;
                }
            }
            Phase::Idle => unreachable!(),
        }
        steps
    }
}

/// « Encore 1 h 20 », « Encore 45 min » (le temps qui reste au prêt).
pub fn left(lang: Lang, seconds: i64) -> String {
    let minutes = (seconds.max(0) + 59) / 60;
    if seconds < 60 {
        return tr(lang, "lend.left.lessThanMinute", &[]);
    }
    if minutes < 60 {
        return tr(lang, "lend.left.minutes", &[("m", &minutes)]);
    }
    let (h, m) = (minutes / 60, minutes % 60);
    if m == 0 {
        tr(lang, "lend.left.wholeHours", &[("h", &h)])
    } else {
        tr(lang, "lend.left.hours", &[("h", &h), ("mm", &format!("{m:02}"))])
    }
}

/// L'heure de fin : « 18 h », « 18 h 30 » ; « 6 PM », « 6:30 PM ».
pub fn clock(lang: Lang, hour: u32, minute: u32) -> String {
    match lang {
        Lang::Fr if minute == 0 => format!("{hour} h"),
        Lang::Fr => format!("{hour} h {minute:02}"),
        Lang::En => {
            let (h, half) = match hour % 24 {
                0 => (12, "AM"),
                h @ 1..=11 => (h, "AM"),
                12 => (12, "PM"),
                h => (h - 12, "PM"),
            };
            if minute == 0 { format!("{h} {half}") } else { format!("{h}:{minute:02} {half}") }
        }
    }
}

/// L'heure de fin écrite (« 18 h ») d'un instant (secondes Unix), à l'heure du PC.
#[cfg(windows)]
pub fn clock_at(lang: Lang, unix: i64) -> Option<String> {
    system::local_clock(unix).map(|(h, m)| clock(lang, h, m))
}

#[cfg(windows)]
pub fn unix_now() -> i64 {
    system::unix_now()
}

/// Le décompte du préavis : « 1:42 ».
pub fn countdown(seconds: i64) -> String {
    let s = seconds.max(0);
    format!("{}:{:02}", s / 60, s % 60)
}

/// Le préavis tel que l'ami le lit dans le lanceur.
pub fn notice_text(lang: Lang, reason: &str, host: &str) -> Option<String> {
    let key = match reason {
        "taken_back" => "lend.notice.takenBack",
        "time_up" => "lend.notice.timeUp",
        "slot_end" => "lend.notice.slotEnd",
        _ => return None,
    };
    Some(tr(lang, key, &[("host", &host)]))
}

/// Touches physiques gardées pour le cache de l'écran du PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverKey {
    Enter,
    Escape,
}

/// Le cache : au repos, ou la question « Reprendre l'ordinateur ? ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cover {
    #[default]
    Idle,
    Asking,
}

/// Une touche sur le cache : le nouvel état, et la reprise à demander
/// (`Some(now)` : `POST …/take-back {"now":now}`). Pendant un préavis, Entrée
/// reprend tout de suite.
pub fn cover_key(cover: Cover, key: CoverKey, notice: bool) -> (Cover, Option<bool>) {
    match (notice, cover, key) {
        (true, _, CoverKey::Enter) => (Cover::Idle, Some(true)),
        (true, _, CoverKey::Escape) => (Cover::Idle, None),
        (false, Cover::Idle, CoverKey::Enter) => (Cover::Asking, None),
        (false, Cover::Idle, CoverKey::Escape) => (Cover::Idle, None),
        (false, Cover::Asking, CoverKey::Enter) => (Cover::Idle, Some(false)),
        (false, Cover::Asking, CoverKey::Escape) => (Cover::Idle, None),
    }
}

/// Les lignes du cache (`docs/link-ui.md`, « Écran du PC »).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverText {
    pub title: String,
    pub until: Option<String>,
    pub line: String,
    pub keys: Option<String>,
}

pub fn cover_text(lang: Lang, borrower: &str, until: Option<&str>, notice_left: Option<i64>, cover: Cover) -> CoverText {
    let title = tr(lang, "lend.cover.title", &[("name", &borrower)]);
    let until = until.map(|time| tr(lang, "lend.cover.until", &[("time", &time)]));
    let (line, keys) = match (notice_left, cover) {
        (Some(left), _) => (tr(lang, "lend.cover.notice", &[("left", &countdown(left))]), None),
        (None, Cover::Asking) => (tr(lang, "lend.cover.ask", &[("name", &borrower)]), Some(tr(lang, "lend.cover.askKeys", &[]))),
        (None, Cover::Idle) => (tr(lang, "lend.cover.hint", &[]), None),
    };
    CoverText { title, until, line, keys }
}

/// Touches de modification au moment d'une touche (crochet clavier).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

/// Le crochet clavier laisse-t-il passer cette touche ? L'entrée physique
/// (clavier du PC) jamais ; l'entrée injectée (Kyber, l'ami) oui, sauf les
/// touches Windows, Alt+Tab, Alt+Échap, Ctrl+Échap et Ctrl+Maj+Échap.
pub fn keep(injected: bool, vk: u32, mods: Mods) -> bool {
    if !injected {
        return false;
    }
    match vk {
        VK_LWIN | VK_RWIN => false,
        VK_TAB => !mods.alt,
        VK_ESCAPE => !(mods.alt || mods.ctrl),
        _ => true,
    }
}

/// Entrée et Échap du clavier du PC vont au cache (et pas plus loin).
pub fn cover_key_of(injected: bool, vk: u32) -> Option<CoverKey> {
    match (injected, vk) {
        (false, VK_RETURN) => Some(CoverKey::Enter),
        (false, VK_ESCAPE) => Some(CoverKey::Escape),
        _ => None,
    }
}

/// Boutons de manette (`wButtons` d'XInput) que le lanceur comprend.
pub mod buttons {
    pub const UP: u16 = 0x0001;
    pub const DOWN: u16 = 0x0002;
    pub const LEFT: u16 = 0x0004;
    pub const RIGHT: u16 = 0x0008;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
}

const DIRECTIONS: [(u16, &str); 4] = [(buttons::UP, "up"), (buttons::DOWN, "down"), (buttons::LEFT, "left"), (buttons::RIGHT, "right")];
/// Direction tenue : répétée après ce délai, puis à ce rythme.
const PAD_DELAY: Duration = Duration::from_millis(400);
const PAD_REPEAT: Duration = Duration::from_millis(140);

/// Le stick gauche, comme la croix (au-delà de la moitié de sa course).
pub fn stick(x: i16, y: i16) -> u16 {
    const DEAD: i16 = 16_000;
    let mut bits = 0;
    if x < -DEAD {
        bits |= buttons::LEFT;
    } else if x > DEAD {
        bits |= buttons::RIGHT;
    }
    if y > DEAD {
        bits |= buttons::UP;
    } else if y < -DEAD {
        bits |= buttons::DOWN;
    }
    bits
}

/// Les manettes dans le lanceur : croix (ou stick) pour se déplacer, A pour
/// ouvrir, B pour revenir ; une direction tenue se répète.
#[derive(Debug, Default)]
pub struct Pad {
    armed: bool,
    prev: u16,
    held: Option<(u16, Instant)>,
}

impl Pad {
    /// Le lanceur n'est plus devant : au retour, les boutons déjà enfoncés
    /// (dans le jeu) ne comptent pas.
    pub fn reset(&mut self) {
        *self = Pad::default();
    }

    /// `now` : boutons de toutes les manettes réunis ; ce que le lanceur doit faire.
    pub fn next(&mut self, now_buttons: u16, now: Instant) -> Vec<&'static str> {
        if !self.armed {
            *self = Pad { armed: true, prev: now_buttons, held: None };
            return Vec::new();
        }
        let pressed = now_buttons & !self.prev;
        self.prev = now_buttons;
        let mut out = Vec::new();
        for (bit, name) in DIRECTIONS {
            if pressed & bit != 0 {
                out.push(name);
                self.held = Some((bit, now + PAD_DELAY));
            }
        }
        if pressed & buttons::A != 0 {
            out.push("a");
        }
        if pressed & buttons::B != 0 {
            out.push("b");
        }
        if out.is_empty()
            && let Some((bit, at)) = self.held
        {
            if now_buttons & bit == 0 {
                self.held = None;
            } else if now >= at {
                out.extend(DIRECTIONS.iter().filter(|(b, _)| *b == bit).map(|(_, n)| *n));
                self.held = Some((bit, now + PAD_REPEAT));
            }
        }
        out
    }
}

/// Base64 standard, pour les icônes des apps posées en `data:` dans le lanceur.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn view(phase: &str, display: bool) -> Reply {
        let v = json!({
            "phase": phase, "lend": "l1", "borrower": "Léa", "host": "Matthieu", "computer": "PC-SALON",
            "until": 1793180000, "notice": null, "steam": "friend",
            "display": if display { json!({"gdi_name": "\\\\.\\DISPLAY5", "x": 3840, "y": 0, "width": 2560, "height": 1440}) } else { Value::Null },
            "apps": [{"id": "steam", "name": "Steam", "icon": null, "kind": "uri", "target": "steam://open/bigpicture"},
                     {"id": "ab12", "name": "Jeu", "icon": "/api/local/lend/icons/ab12.png"}, {"name": "sans id"}],
            "steam_exe": "C:\\Steam\\steam.exe", "lang": "en",
        });
        Reply::View(Box::new(View::parse(&v, Lang::Fr).unwrap()))
    }

    #[test]
    fn vue_lue() {
        let Reply::View(v) = view("lent", true) else { panic!() };
        assert_eq!(v.phase, Phase::Lent);
        assert_eq!(v.lang, Lang::En);
        assert!(v.steam_friend);
        let d = v.display.unwrap();
        assert_eq!((d.gdi_name.as_str(), d.x, d.width), ("\\\\.\\DISPLAY5", 3840, 2560));
        assert!(d.contains(3840, 0) && d.contains(6399, 1439) && !d.contains(6400, 0) && !d.contains(0, 0));
        assert_eq!(v.apps.len(), 2);
        assert_eq!(v.apps[1].icon.as_deref(), Some("/api/local/lend/icons/ab12.png"));
        assert_eq!(Phase::parse("plus tard"), Phase::Idle);
        let Reply::View(v) = view("preparing", false) else { panic!() };
        assert!(v.display.is_none());
    }

    #[test]
    fn phases() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut m = Machine::new(false);
        // Préparation sans écran : la mise en place, pas le lanceur.
        assert_eq!(m.next(&view("preparing", false), at(0)), [Step::Begin]);
        assert_eq!(m.next(&view("preparing", false), at(1)), []);
        // L'écran arrive : lanceur, puis « prêt ».
        assert_eq!(m.next(&view("preparing", true), at(2)), [Step::Show, Step::Ready]);
        assert_eq!(m.next(&view("preparing", true), at(3)), []);
        // Prêté : un signe de vie toutes les 2 s.
        assert_eq!(m.next(&view("lent", true), at(4)), [Step::Beat]);
        assert_eq!(m.next(&view("lent", true), at(5)), []);
        assert_eq!(m.next(&view("lent", true), at(6)), [Step::Beat]);
        m.retry(Step::Beat);
        assert_eq!(m.next(&view("lent", true), at(7)), [Step::Beat]);
        // Fin : remise en état, « fini » (refait s'il a échoué), puis sortie au retour à idle.
        assert_eq!(m.next(&view("ending", true), at(8)), [Step::TearDown, Step::Ended]);
        m.retry(Step::Ended);
        assert_eq!(m.next(&view("ending", true), at(9)), [Step::Ended]);
        assert_eq!(m.next(&view("idle", false), at(10)), [Step::Exit]);
    }

    #[test]
    fn admin_muet_ou_absent() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut m = Machine::new(false);
        m.next(&view("lent", true), at(0));
        assert_eq!(m.next(&Reply::Silent, at(1)), []);
        assert_eq!(m.next(&Reply::Silent, at(60)), []);
        assert_eq!(m.next(&Reply::Silent, at(61)), [Step::TearDown, Step::Lock, Step::Exit]);
        // Muet dès le départ, sans rien de mis en place : on sort sans verrouiller.
        let mut m = Machine::new(false);
        m.next(&Reply::Silent, at(0));
        assert_eq!(m.next(&Reply::Silent, at(60)), [Step::Exit]);
        // Un état laissé par un agent tombé, et plus de prêt : remise en état, sortie.
        let mut m = Machine::new(true);
        assert_eq!(m.next(&Reply::Gone, at(0)), [Step::TearDown, Step::Exit]);
        // Agent relancé en plein prêt : il se remet en place, sans « prêt ».
        let mut m = Machine::new(true);
        assert_eq!(m.next(&view("lent", true), at(0)), [Step::Begin, Step::Show, Step::Beat]);
        // Fin sans mise en place : la remise en état (de l'état laissé), puis « fini ».
        let mut m = Machine::new(true);
        assert_eq!(m.next(&view("ending", false), at(0)), [Step::TearDown, Step::Ended]);
    }

    #[test]
    fn steam_rate_pas_de_pret() {
        let t0 = Instant::now();
        let mut m = Machine::new(false);
        assert_eq!(m.next(&view("preparing", false), t0), [Step::Begin]);
        m.retry(Step::Begin);
        assert_eq!(m.next(&view("preparing", true), t0), [Step::Show]);
        assert_eq!(m.next(&view("ending", true), t0), [Step::TearDown, Step::Ended]);
    }

    #[test]
    fn autre_pret() {
        let t0 = Instant::now();
        let mut m = Machine::new(false);
        m.next(&view("lent", true), t0);
        let Reply::View(mut other) = view("preparing", true) else { panic!() };
        other.lend = "l2".into();
        assert_eq!(m.next(&Reply::View(other), t0), [Step::TearDown, Step::Begin, Step::Show, Step::Ready]);
    }

    #[test]
    fn textes() {
        assert_eq!(left(Lang::Fr, 80 * 60), "Encore 1 h 20");
        assert_eq!(left(Lang::En, 80 * 60), "1 h 20 left");
        assert_eq!(left(Lang::Fr, 2 * 3600), "Encore 2 h");
        assert_eq!(left(Lang::Fr, 65 * 60 - 30), "Encore 1 h 05");
        assert_eq!(left(Lang::Fr, 45 * 60), "Encore 45 min");
        assert_eq!(left(Lang::En, 59 * 60 + 30), "1 h left");
        assert_eq!(left(Lang::Fr, 20), "Encore moins d'une minute");
        assert_eq!(clock(Lang::Fr, 18, 0), "18 h");
        assert_eq!(clock(Lang::Fr, 9, 5), "9 h 05");
        assert_eq!(clock(Lang::En, 18, 0), "6 PM");
        assert_eq!(clock(Lang::En, 18, 30), "6:30 PM");
        assert_eq!(clock(Lang::En, 0, 15), "12:15 AM");
        assert_eq!(clock(Lang::En, 12, 0), "12 PM");
        assert_eq!(countdown(102), "1:42");
        assert_eq!(countdown(-3), "0:00");
        assert_eq!(notice_text(Lang::Fr, "taken_back", "Matthieu").unwrap(), "Matthieu reprend l'ordinateur dans 2 min : sauvegardez votre partie.");
        assert_eq!(notice_text(Lang::En, "time_up", "Matthieu").unwrap(), "The loan ends in 2 min: save your game.");
        assert_eq!(notice_text(Lang::Fr, "slot_start", "Matthieu"), None);
    }

    #[test]
    fn cache() {
        // Entrée, puis Entrée : reprise avec préavis ; Échap annule.
        assert_eq!(cover_key(Cover::Idle, CoverKey::Enter, false), (Cover::Asking, None));
        assert_eq!(cover_key(Cover::Asking, CoverKey::Escape, false), (Cover::Idle, None));
        assert_eq!(cover_key(Cover::Asking, CoverKey::Enter, false), (Cover::Idle, Some(false)));
        // Pendant le préavis : Entrée, tout de suite.
        assert_eq!(cover_key(Cover::Idle, CoverKey::Enter, true), (Cover::Idle, Some(true)));
        assert_eq!(cover_key(Cover::Asking, CoverKey::Escape, true), (Cover::Idle, None));
        let c = cover_text(Lang::Fr, "Léa", Some("18 h"), None, Cover::Idle);
        assert_eq!(c.title, "Ordinateur prêté à Léa");
        assert_eq!(c.until.as_deref(), Some("Jusqu'à 18 h"));
        assert_eq!(c.line, "Pour reprendre l'ordinateur, appuyez sur Entrée.");
        let c = cover_text(Lang::Fr, "Léa", None, None, Cover::Asking);
        assert_eq!(c.line, "Reprendre l'ordinateur ? Léa a 2 minutes pour sauvegarder.");
        assert_eq!(c.keys.as_deref(), Some("Entrée : reprendre · Échap : annuler"));
        let c = cover_text(Lang::En, "Léa", Some("6 PM"), Some(102), Cover::Asking);
        assert_eq!(c.line, "Taking back in 1:42 · Enter: right now");
        assert_eq!(c.until.as_deref(), Some("Until 6 PM"));
        assert_eq!(c.keys, None);
    }

    #[test]
    fn crochets() {
        let none = Mods::default();
        let alt = Mods { alt: true, ..none };
        let ctrl = Mods { ctrl: true, ..none };
        let ctrl_shift = Mods { ctrl: true, shift: true, ..none };
        // Le clavier du PC : rien ne passe.
        assert!(!keep(false, 0x41, none));
        assert!(!keep(false, VK_RETURN, none));
        // Celui de l'ami (injecté) : tout, sauf Windows et les bascules de fenêtres.
        assert!(keep(true, 0x41, none));
        assert!(keep(true, VK_RETURN, none));
        assert!(keep(true, VK_ESCAPE, none));
        assert!(keep(true, VK_TAB, none));
        assert!(keep(true, 0x73 /* F4 */, alt));
        assert!(!keep(true, VK_LWIN, none));
        assert!(!keep(true, VK_RWIN, ctrl));
        assert!(!keep(true, VK_TAB, alt));
        assert!(!keep(true, VK_ESCAPE, alt));
        assert!(!keep(true, VK_ESCAPE, ctrl));
        assert!(!keep(true, VK_ESCAPE, ctrl_shift));
        assert_eq!(cover_key_of(false, VK_RETURN), Some(CoverKey::Enter));
        assert_eq!(cover_key_of(false, VK_ESCAPE), Some(CoverKey::Escape));
        assert_eq!(cover_key_of(true, VK_RETURN), None);
        assert_eq!(cover_key_of(false, 0x41), None);
    }

    #[test]
    fn manette() {
        use buttons::*;
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut pad = Pad::default();
        // A déjà enfoncé au retour dans le lanceur : rien.
        assert!(pad.next(A, ms(0)).is_empty());
        assert!(pad.next(A, ms(50)).is_empty());
        assert!(pad.next(0, ms(100)).is_empty());
        assert_eq!(pad.next(A, ms(150)), ["a"]);
        assert_eq!(pad.next(B | A, ms(200)), ["b"]);
        // Droite tenue : une fois, puis répétée après 400 ms, toutes les 140 ms.
        assert_eq!(pad.next(RIGHT, ms(300)), ["right"]);
        assert!(pad.next(RIGHT, ms(500)).is_empty());
        assert_eq!(pad.next(RIGHT, ms(700)), ["right"]);
        assert!(pad.next(RIGHT, ms(800)).is_empty());
        assert_eq!(pad.next(RIGHT, ms(840)), ["right"]);
        assert!(pad.next(0, ms(900)).is_empty());
        assert!(pad.next(0, ms(2000)).is_empty());
        pad.reset();
        assert!(pad.next(DOWN, ms(2100)).is_empty());
        assert_eq!(stick(-20_000, 0), LEFT);
        assert_eq!(stick(0, 20_000), UP);
        assert_eq!(stick(20_000, -20_000), RIGHT | DOWN);
        assert_eq!(stick(1_000, -1_000), 0);
    }

    #[test]
    fn base64_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
    }
}
