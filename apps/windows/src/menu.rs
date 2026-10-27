//! Menu de l'icône près de l'horloge (docs/link-ui.md, « Icône près de
//! l'horloge ») : ce qu'il montre selon l'état de l'app, de haut en bas. Sans
//! rien de Windows, pour se tester sur le Mac : `ui.rs` donne l'état
//! ([`State`]), `tray.rs` en fait un menu de Windows et rend à `ui.rs` la
//! commande choisie ([`Cmd`]).

use kybernow_core::share::ShareRole;

/// Textes : clé du catalogue de l'app (`src/i18n/`) et paramètres.
pub type Tr<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// Ce que fait une ligne du menu.
/// Les commandes d'une session portent l'ordinateur de la session : la
/// session active peut changer entre l'ouverture du menu et le choix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Fullscreen(String),
    /// Mode de la souris de la session (`auto`, `bureau`, `jeu`), gardé pour l'ordinateur.
    Mouse(String, &'static str),
    Statistics(String),
    Disconnect(String),
    /// Sa fenêtre passe devant (plusieurs ordinateurs).
    Show(String),
    /// « Garder le son » allumé ou éteint pour cette session.
    KeepSound(String),
    Connect(String),
    Wake(String),
    Retry(String),
    /// Ordinateur et profil (`jeu`, `bureau`, `max`).
    Profile(String, &'static str),
    Settings(String),
    Admin(String),
    Restart(String),
    Update(String),
    Open,
    Add,
    /// « Rejoindre avec un code » (KyberNow Share).
    Join,
    AppSettings,
    CheckUpdates,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    Check,
    /// Coche ronde : un choix parmi plusieurs (profil, souris).
    Radio,
}

/// Point de couleur devant un ordinateur (couleurs d'état de
/// brand/v2/tokens/kybernow.css) : la couleur double le texte de l'état,
/// elle ne le remplace pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dot {
    /// En ligne : `--kn-ok`.
    Ok,
    /// Hors ligne : `--kn-bad`.
    Bad,
    /// Réveil en cours : `--kn-warn`.
    Warn,
    /// Recherche : `--kn-faint`.
    Faint,
}

impl Dot {
    /// La couleur, sur un menu sombre ou clair.
    pub fn rgb(self, dark: bool) -> [u8; 3] {
        match (self, dark) {
            (Dot::Ok, true) => [0x4c, 0xd6, 0x8a],
            (Dot::Ok, false) => [0x13, 0x8a, 0x4e],
            (Dot::Bad, true) => [0xff, 0x6b, 0x6b],
            (Dot::Bad, false) => [0xc9, 0x3a, 0x3a],
            (Dot::Warn, true) => [0xf5, 0xb8, 0x41],
            (Dot::Warn, false) => [0xa8, 0x6a, 0x00],
            (Dot::Faint, true) => [0x65, 0x73, 0x7e],
            (Dot::Faint, false) => [0x74, 0x82, 0x8d],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Une ligne ; sans `cmd`, elle ne fait rien (en-tête, mesures, état en cours).
    Line { text: String, cmd: Option<Cmd>, enabled: bool, mark: Mark, default: bool },
    /// Un sous-menu ; `key` : l'ordinateur qu'il concerne (refait à son
    /// ouverture) ; `dot` : son état en couleur.
    Sub { text: String, key: Option<String>, checked: bool, dot: Option<Dot>, items: Vec<Item> },
    Separator,
}

impl Item {
    fn line(text: String, cmd: Option<Cmd>) -> Item {
        let enabled = cmd.is_some();
        Item::Line { text, cmd, enabled, mark: Mark::None, default: false }
    }

    /// Une ligne inactive.
    fn inactive(text: String) -> Item {
        Item::line(text, None)
    }

    pub fn text(&self) -> &str {
        match self {
            Item::Line { text, .. } | Item::Sub { text, .. } => text,
            Item::Separator => "",
        }
    }
}

/// Une session en cours.
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub id: String,
    pub name: String,
    /// Mesures de la barre de session, à la seconde.
    pub latency_ms: Option<f64>,
    pub fps: Option<u64>,
    /// Débit demandé à l'ordinateur.
    pub mbps: Option<u32>,
    pub fullscreen: bool,
    /// Plusieurs écrans : les fenêtres restent en plein écran, « Plein écran »
    /// n'est pas proposé.
    pub multi: bool,
    /// Mode de la souris de l'ordinateur : `auto`, `bureau` ou `jeu`.
    pub mouse: String,
    /// « Garder le son » (plusieurs ordinateurs).
    pub keep_sound: bool,
    /// Session d'invité (KyberNow Share) : son rôle. `name` est alors
    /// l'ordinateur qui partage, pas un ordinateur de l'app ; ni profil ni
    /// réglages, la souris seulement avec le clavier et la souris, et
    /// « Quitter le partage » à la place de « Se déconnecter ».
    pub guest: Option<ShareRole>,
}

/// Redémarrage ou mise à jour de la Station en cours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Restarting,
    Updating,
}

/// La Station d'un ordinateur (0.5.0 et suivantes : redémarrage et mise à
/// jour depuis l'app).
#[derive(Debug, Clone, Default)]
pub struct Station {
    pub busy: Option<Busy>,
    /// Version qui s'installe depuis un appareil.
    pub update: Option<String>,
}

/// Un ordinateur, dans l'ordre de l'accueil.
#[derive(Debug, Clone, Default)]
pub struct Computer {
    pub id: String,
    pub name: String,
    /// `None` : pas encore de réponse (« Recherche… »).
    pub online: Option<bool>,
    /// `jeu`, `bureau`, `max`.
    pub profile: String,
    /// Les réglages s'écartent du profil : « Personnalisé ».
    pub custom: bool,
    /// L'app connaît ses cartes réseau : « Réveiller l'ordinateur ».
    pub can_wake: bool,
    pub waking: bool,
    /// `None` : Station trop ancienne, ou pas encore vue.
    pub station: Option<Station>,
}

#[derive(Debug, Clone, Default)]
pub struct State {
    /// Les sessions en cours, dans l'ordre d'ouverture (plusieurs ordinateurs).
    pub sessions: Vec<Session>,
    /// L'ordinateur de la session active (celle de la fenêtre au premier plan).
    pub active: Option<String>,
    /// Quatre ordinateurs sont déjà en session (ou s'y connectent) : « Se
    /// connecter » est inactif.
    pub full: bool,
    pub computers: Vec<Computer>,
    /// Version de l'app qui attend d'être installée.
    pub app_update: Option<String>,
}

impl State {
    /// La session de la fenêtre active (la première à défaut).
    pub fn active_session(&self) -> Option<&Session> {
        self.active.as_ref().and_then(|id| self.session(id)).or_else(|| self.sessions.first())
    }

    /// La session de cet ordinateur.
    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }
}

const PROFILES: [&str; 3] = ["jeu", "bureau", "max"];
/// Dans l'ordre des réglages de l'ordinateur.
const MOUSE_MODES: [&str; 3] = ["auto", "bureau", "jeu"];

/// Le menu, de haut en bas.
pub fn build(s: &State, tr: Tr) -> Vec<Item> {
    let mut out = Vec::new();
    // Avec plusieurs ordinateurs : pour la session de la fenêtre active.
    if let Some(session) = s.active_session() {
        let id = &session.id;
        match session.guest {
            // Invité d'un partage : son titre et son rôle (« Spectateur », « Joueur »).
            Some(role) => {
                out.push(Item::inactive(tr("share.guest_on", &[("name", &session.name)])));
                out.push(Item::inactive(tr(crate::sharing::role_label(role), &[])));
            }
            None => out.push(Item::inactive(tr("tray.in_session", &[("name", &session.name)]))),
        }
        out.push(Item::inactive(measures(session, tr)));
        if !session.multi {
            out.push(Item::Line {
                text: tr("tray.fullscreen", &[]),
                cmd: Some(Cmd::Fullscreen(id.clone())),
                enabled: true,
                mark: if session.fullscreen { Mark::Check } else { Mark::None },
                default: false,
            });
        }
        // Un invité n'a la souris que s'il a le clavier et la souris.
        if session.guest.is_none_or(crate::sharing::has_keyboard_mouse) {
            let modes = MOUSE_MODES
                .iter()
                .map(|m| Item::Line {
                    text: tr(&format!("tray.mouse_modes.{m}"), &[]),
                    cmd: Some(Cmd::Mouse(id.clone(), m)),
                    enabled: true,
                    mark: if session.mouse == *m { Mark::Radio } else { Mark::None },
                    default: false,
                })
                .collect();
            out.push(Item::Sub { text: tr("tray.mouse", &[]), key: None, checked: false, dot: None, items: modes });
        }
        out.push(Item::line(tr("tray.statistics", &[]), Some(Cmd::Statistics(id.clone()))));
        let leave = if session.guest.is_some() { "share.leave" } else { "tray.disconnect" };
        out.push(Item::line(tr(leave, &[]), Some(Cmd::Disconnect(id.clone()))));
        out.push(Item::Separator);
    }
    for c in &s.computers {
        let (state, dot) = status(c);
        let state = tr(&format!("tray.{}", state.code()), &[]);
        out.push(Item::Sub {
            text: tr("tray.computer", &[("name", &c.name), ("state", &state)]),
            key: Some(c.id.clone()),
            checked: s.session(&c.id).is_some(),
            dot: Some(dot),
            items: computer(s, c, tr),
        });
    }
    if !s.computers.is_empty() {
        out.push(Item::Separator);
    }
    out.push(Item::Line { text: tr("tray.open", &[]), cmd: Some(Cmd::Open), enabled: true, mark: Mark::None, default: true });
    out.push(Item::line(tr("tray.add", &[]), Some(Cmd::Add)));
    out.push(Item::line(tr("tray.join", &[]), Some(Cmd::Join)));
    out.push(Item::line(tr("tray.app_settings", &[]), Some(Cmd::AppSettings)));
    out.push(match &s.app_update {
        Some(v) => Item::line(tr("tray.app_update", &[("version", v)]), Some(Cmd::AppSettings)),
        None => Item::line(tr("tray.check_updates", &[]), Some(Cmd::CheckUpdates)),
    });
    out.push(Item::Separator);
    out.push(Item::line(tr("tray.quit", &[]), Some(Cmd::Quit)));
    out
}

/// L'état d'un ordinateur, tel que le menu de l'icône et le menu
/// « Ordinateurs » de la barre l'écrivent (« {nom} — En ligne »…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Online,
    Offline,
    Searching,
}

impl Seen {
    /// `online`, `offline`, `searching` : la clé du texte.
    pub fn code(self) -> &'static str {
        match self {
            Seen::Online => "online",
            Seen::Offline => "offline",
            Seen::Searching => "searching",
        }
    }
}

/// L'état d'un ordinateur et son point de couleur : ambre hors ligne pendant
/// un réveil, un redémarrage ou une mise à jour de la Station.
pub fn status(c: &Computer) -> (Seen, Dot) {
    match c.online {
        Some(true) => (Seen::Online, Dot::Ok),
        Some(false) if c.waking || c.station.as_ref().is_some_and(|st| st.busy.is_some()) => (Seen::Offline, Dot::Warn),
        Some(false) => (Seen::Offline, Dot::Bad),
        None => (Seen::Searching, Dot::Faint),
    }
}

/// Ce que fait un clic sur un ordinateur dans le menu « Ordinateurs » de la
/// barre (plusieurs ordinateurs) : comme le premier élément de son sous-menu
/// dans le menu de l'icône, et le bouton principal de l'accueil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// En session : sa fenêtre passe devant (« Afficher »).
    Show,
    /// En ligne (ou en recherche) : sa session s'ouvre ; `false` : quatre
    /// ordinateurs sont déjà en session.
    Connect(bool),
    /// Hors ligne, adresse MAC connue : « Réveiller l'ordinateur ».
    Wake,
    /// Réveil en cours : inactif.
    Waking,
    /// Hors ligne sans adresse MAC : « Réessayer ».
    Retry,
    /// La Station redémarre ou s'installe : inactif, il revient de lui-même.
    Busy,
}

impl Pick {
    pub fn code(self) -> &'static str {
        match self {
            Pick::Show => "show",
            Pick::Connect(true) => "connect",
            Pick::Connect(false) => "full",
            Pick::Wake => "wake",
            Pick::Waking => "waking",
            Pick::Retry => "retry",
            Pick::Busy => "busy",
        }
    }
}

pub fn pick(s: &State, c: &Computer) -> Pick {
    let busy = c.station.as_ref().is_some_and(|st| st.busy.is_some());
    if s.session(&c.id).is_some() {
        Pick::Show
    } else if c.online == Some(false) && busy {
        Pick::Busy
    } else if c.online == Some(false) && c.waking {
        Pick::Waking
    } else if c.online == Some(false) && c.can_wake {
        Pick::Wake
    } else if c.online == Some(false) {
        Pick::Retry
    } else {
        Pick::Connect(!s.full)
    }
}

/// « 11,6 ms · 60 i/s · 80 Mbit/s » (la latence comme dans la barre de
/// session, au dixième) ; « — » pour ce qui n'est pas encore mesuré. Un
/// invité n'a pas de débit demandé (l'image est celle de l'hôte).
fn measures(s: &Session, tr: Tr) -> String {
    let dash = || "—".to_string();
    let decimal = tr("tray.decimal", &[]);
    let latency = s.latency_ms.map(|v| format!("{v:.1}").replace('.', &decimal)).unwrap_or_else(dash);
    let fps = s.fps.map(|v| v.to_string()).unwrap_or_else(dash);
    if s.guest.is_some() {
        return tr("tray.measures_guest", &[("latency", &latency), ("fps", &fps)]);
    }
    let mbps = s.mbps.map(|v| v.to_string()).unwrap_or_else(dash);
    tr("tray.measures", &[("latency", &latency), ("fps", &fps), ("mbps", &mbps)])
}

/// Le sous-menu d'un ordinateur.
pub fn computer(s: &State, c: &Computer, tr: Tr) -> Vec<Item> {
    let mut out = Vec::new();
    let busy = c.station.as_ref().and_then(|st| st.busy);
    let profile_name = |p: &str| tr(&format!("tray.profiles.{p}"), &[]);
    let profile = if c.custom { profile_name("custom") } else { profile_name(&c.profile) };
    let connect = |enabled: bool| Item::Line {
        text: tr("tray.connect", &[("profile", &profile)]),
        cmd: Some(Cmd::Connect(c.id.clone())),
        enabled,
        mark: Mark::None,
        default: false,
    };
    match pick(s, c) {
        Pick::Show => {
            // En session : « Afficher », « Garder le son » (à partir de deux
            // sessions, comme dans la barre), « Se déconnecter », puis le reste.
            let keep = s.session(&c.id).is_some_and(|x| x.keep_sound);
            out.push(Item::line(tr("tray.show", &[]), Some(Cmd::Show(c.id.clone()))));
            if s.sessions.len() >= 2 {
                out.push(Item::Line {
                    text: tr("tray.keep_sound", &[]),
                    cmd: Some(Cmd::KeepSound(c.id.clone())),
                    enabled: true,
                    mark: if keep { Mark::Check } else { Mark::None },
                    default: false,
                });
            }
            out.push(Item::line(tr("tray.disconnect", &[]), Some(Cmd::Disconnect(c.id.clone()))));
            out.push(Item::Separator);
        }
        // La Station redémarre ou s'installe : l'ordinateur revient de lui-même.
        Pick::Busy => out.push(connect(false)),
        Pick::Waking => out.push(Item::inactive(tr("tray.waking", &[]))),
        Pick::Wake => out.push(Item::line(tr("tray.wake", &[]), Some(Cmd::Wake(c.id.clone())))),
        Pick::Retry => out.push(Item::line(tr("tray.retry", &[]), Some(Cmd::Retry(c.id.clone())))),
        // Quatre ordinateurs au plus en même temps.
        Pick::Connect(enabled) => out.push(connect(enabled)),
    }

    let mut profiles: Vec<Item> = PROFILES
        .iter()
        .map(|p| Item::Line {
            text: profile_name(p),
            cmd: Some(Cmd::Profile(c.id.clone(), p)),
            enabled: true,
            mark: if !c.custom && c.profile == *p { Mark::Radio } else { Mark::None },
            default: false,
        })
        .collect();
    if c.custom {
        profiles.push(Item::Line { text: profile_name("custom"), cmd: None, enabled: false, mark: Mark::Radio, default: false });
    }
    out.push(Item::Sub { text: tr("tray.profile", &[]), key: None, checked: false, dot: None, items: profiles });
    out.push(Item::line(tr("tray.settings", &[]), Some(Cmd::Settings(c.id.clone()))));

    let online = c.online == Some(true);
    let station = c.station.as_ref().filter(|_| online || busy.is_some());
    if online || station.is_some() {
        out.push(Item::Separator);
    }
    if online {
        out.push(Item::line(tr("tray.open_admin", &[]), Some(Cmd::Admin(c.id.clone()))));
    }
    if let Some(st) = station {
        out.push(match busy {
            Some(Busy::Restarting) => Item::inactive(tr("tray.restarting", &[])),
            _ => Item::Line {
                text: tr("tray.restart", &[]),
                cmd: Some(Cmd::Restart(c.id.clone())),
                enabled: busy.is_none() && online,
                mark: Mark::None,
                default: false,
            },
        });
        if busy == Some(Busy::Updating) {
            out.push(Item::inactive(tr("tray.updating", &[])));
        } else if let Some(v) = &st.update {
            out.push(Item::Line {
                text: tr("tray.update", &[("version", v)]),
                cmd: Some(Cmd::Update(c.id.clone())),
                enabled: busy.is_none() && online,
                mark: Mark::None,
                default: false,
            });
        }
    }
    out
}

/// Infobulle de l'icône.
pub fn tooltip(s: &State, tr: Tr) -> String {
    match s.sessions.as_slice() {
        [] => tr("tray.tooltip", &[]),
        [session] if session.guest.is_some() => tr("tray.tooltip_guest", &[("name", &session.name)]),
        [session] => tr("tray.tooltip_session", &[("name", &session.name)]),
        many => tr("tray.tooltip_sessions", &[("count", &many.len().to_string())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::i18n::{Catalog, Lang};

    static APP: Catalog = Catalog::new(include_str!("i18n/fr.json"), include_str!("i18n/en.json"));

    /// Les textes de l'app, comme `i18n::tr` (élision comprise).
    fn tr(lang: Lang) -> impl Fn(&str, &[(&str, &str)]) -> String {
        move |key, params| crate::text::fill(&APP.tr(lang, key, &[]), params, lang == Lang::Fr)
    }

    /// Le menu en lignes de texte : `[x]` coché, `(o)` choix rond, `~` inactif,
    /// `*` par défaut, `>` sous-menu (ses lignes en retrait), `{vert}` point de
    /// couleur, `---` séparateur.
    fn render(items: &[Item], depth: usize, out: &mut Vec<String>) {
        let pad = "  ".repeat(depth);
        for item in items {
            match item {
                Item::Line { text, enabled, mark, default, .. } => {
                    let mark = match mark {
                        Mark::None => "",
                        Mark::Check => "[x] ",
                        Mark::Radio => "(o) ",
                    };
                    out.push(format!("{pad}{mark}{}{text}{}", if *enabled { "" } else { "~" }, if *default { " *" } else { "" }));
                }
                Item::Sub { text, checked, dot, items, .. } => {
                    let dot = match dot {
                        Some(Dot::Ok) => "{vert} ",
                        Some(Dot::Bad) => "{rouge} ",
                        Some(Dot::Warn) => "{ambre} ",
                        Some(Dot::Faint) => "{gris} ",
                        None => "",
                    };
                    out.push(format!("{pad}{}{dot}{text} >", if *checked { "[x] " } else { "" }));
                    render(items, depth + 1, out);
                }
                Item::Separator => out.push(format!("{pad}---")),
            }
        }
    }

    fn lines(s: &State, lang: Lang) -> Vec<String> {
        let mut out = Vec::new();
        render(&build(s, &tr(lang)), 0, &mut out);
        out
    }

    fn cmds(items: &[Item], out: &mut Vec<Cmd>) {
        for item in items {
            match item {
                Item::Line { cmd: Some(c), enabled: true, .. } => out.push(c.clone()),
                Item::Sub { items, .. } => cmds(items, out),
                _ => {}
            }
        }
    }

    fn atelier() -> Computer {
        Computer {
            id: "a".into(),
            name: "Atelier".into(),
            online: Some(true),
            profile: "jeu".into(),
            station: Some(Station { busy: None, update: Some("0.5.3".into()) }),
            ..Default::default()
        }
    }

    fn salon() -> Computer {
        Computer { id: "b".into(), name: "Bureau-Salon".into(), online: Some(false), profile: "bureau".into(), can_wake: true, ..Default::default() }
    }

    const BOTTOM_FR: [&str; 7] =
        ["Ouvrir KyberNow Link *", "Ajouter un ordinateur", "Rejoindre avec un code", "Réglages de l'app", "Rechercher des mises à jour", "---", "Quitter KyberNow Link"];

    #[test]
    fn sans_ordinateur() {
        assert_eq!(lines(&State::default(), Lang::Fr), BOTTOM_FR);
        let en = lines(&State { app_update: Some("0.5.3".into()), ..Default::default() }, Lang::En);
        assert_eq!(en, ["Open KyberNow Link *", "Add a computer", "Join with a code", "App settings", "Version 0.5.3 available", "---", "Quit KyberNow Link"]);
    }

    #[test]
    fn en_ligne_et_hors_ligne() {
        let s = State { computers: vec![atelier(), salon()], ..Default::default() };
        let mut expected = vec![
            "{vert} Atelier — En ligne >",
            "  Se connecter (Jeu)",
            "  Profil >",
            "    (o) Jeu",
            "    Bureau",
            "    Max",
            "  Réglages",
            "  ---",
            "  Ouvrir Admin",
            "  Redémarrer la Station",
            "  Mettre à jour la Station (0.5.3)",
            "{rouge} Bureau-Salon — Hors ligne >",
            "  Réveiller l'ordinateur",
            "  Profil >",
            "    Jeu",
            "    (o) Bureau",
            "    Max",
            "  Réglages",
            "---",
        ];
        expected.extend(BOTTOM_FR);
        assert_eq!(lines(&s, Lang::Fr), expected);
    }

    #[test]
    fn hors_ligne_sans_adresse_et_reveil() {
        let mut c = salon();
        c.can_wake = false;
        let s = State { computers: vec![c.clone()], ..Default::default() };
        assert_eq!(lines(&s, Lang::Fr)[1], "  Réessayer");
        assert_eq!(lines(&s, Lang::En)[..2], ["{rouge} Bureau-Salon — Offline >", "  Try again"]);
        c.can_wake = true;
        c.waking = true;
        let s = State { computers: vec![c], ..Default::default() };
        assert_eq!(lines(&s, Lang::Fr)[..2], ["{ambre} Bureau-Salon — Hors ligne >", "  ~Réveil de l'ordinateur…"]);
        assert_eq!(lines(&s, Lang::En)[1], "  ~Waking the computer…");
        // Pas encore de réponse : « Recherche… », et Se connecter comme sur l'accueil.
        let s = State { computers: vec![Computer { online: None, ..atelier() }], ..Default::default() };
        assert_eq!(lines(&s, Lang::Fr)[..2], ["{gris} Atelier — Recherche… >", "  Se connecter (Jeu)"]);
    }

    fn session_atelier() -> Session {
        Session { id: "a".into(), name: "Atelier".into(), latency_ms: Some(11.6), fps: Some(60), mbps: Some(80), fullscreen: true, multi: false, mouse: "jeu".into(), keep_sound: false, guest: None }
    }

    #[test]
    fn en_session() {
        let s = State {
            sessions: vec![session_atelier()],
            active: Some("a".into()),
            computers: vec![atelier(), Computer { online: Some(true), ..salon() }],
            ..Default::default()
        };
        let l = lines(&s, Lang::Fr);
        assert_eq!(
            l[..10],
            [
                "~En session sur Atelier",
                "~11,6 ms · 60 i/s · 80 Mbit/s",
                "[x] Plein écran",
                "Souris >",
                "  Auto",
                "  Souris du bureau",
                "  (o) Souris des jeux",
                "Statistiques",
                "Se déconnecter",
                "---"
            ]
        );
        // L'ordinateur de la session est coché, son sous-menu commence par
        // Afficher et Se déconnecter (« Garder le son » à partir de deux
        // sessions) ; l'autre se connecte (plusieurs ordinateurs à la fois).
        assert_eq!(l[10..14], ["[x] {vert} Atelier — En ligne >", "  Afficher", "  Se déconnecter", "  ---"]);
        assert!(!l.iter().any(|x| x.contains("Garder le son")));
        let other = l.iter().position(|x| x == "{vert} Bureau-Salon — En ligne >").unwrap();
        assert_eq!(l[other + 1], "  Se connecter (Bureau)");
        let en = lines(&s, Lang::En);
        assert_eq!(
            en[..9],
            ["~In session on Atelier", "~11.6 ms · 60 fps · 80 Mbit/s", "[x] Full screen", "Mouse >", "  Auto", "  Desktop mouse", "  (o) Game mouse", "Statistics", "Disconnect"]
        );
        // Choisir un mode : la commande porte l'ordinateur et le mode.
        let menu = build(&s, &tr(Lang::Fr));
        let Item::Sub { items, .. } = &menu[3] else { panic!("sous-menu Souris") };
        assert!(matches!(&items[0], Item::Line { cmd: Some(Cmd::Mouse(id, "auto")), .. } if id == "a"));
        // Mesures pas encore arrivées.
        let mut early = s.clone();
        early.sessions[0].latency_ms = None;
        early.sessions[0].fps = None;
        early.sessions[0].fullscreen = false;
        let l = lines(&early, Lang::Fr);
        assert_eq!(l[1..3], ["~— ms · — i/s · 80 Mbit/s", "Plein écran"]);
        assert_eq!(tooltip(&s, &tr(Lang::Fr)), "KyberNow Link · en session sur Atelier");
        assert_eq!(tooltip(&s, &tr(Lang::En)), "KyberNow Link · in session on Atelier");
        assert_eq!(tooltip(&State::default(), &tr(Lang::Fr)), "KyberNow Link");
    }

    #[test]
    fn plusieurs_ecrans_sans_plein_ecran() {
        // « Plein écran » n'est pas proposé : les fenêtres restent en plein écran.
        let s = State {
            sessions: vec![Session { mbps: Some(120), multi: true, mouse: "auto".into(), ..session_atelier() }],
            computers: vec![atelier()],
            ..Default::default()
        };
        let fr = lines(&s, Lang::Fr);
        assert_eq!(fr[..4], ["~En session sur Atelier", "~11,6 ms · 60 i/s · 120 Mbit/s", "Souris >", "  (o) Auto"]);
        assert!(!fr.iter().any(|l| l.contains("Plein écran")));
        let en = lines(&s, Lang::En);
        assert!(!en.iter().any(|l| l.contains("Full screen")));
        // Un seul écran : comme avant.
        let mut one = s.clone();
        one.sessions[0].multi = false;
        assert!(lines(&one, Lang::Fr).iter().any(|l| l == "[x] Plein écran"));
    }

    #[test]
    fn plusieurs_ordinateurs() {
        let salon_session = Session {
            id: "b".into(),
            name: "Bureau-Salon".into(),
            latency_ms: Some(4.2),
            fps: Some(120),
            mbps: Some(60),
            fullscreen: false,
            multi: false,
            mouse: "auto".into(),
            keep_sound: true,
            guest: None,
        };
        let s = State {
            sessions: vec![session_atelier(), salon_session],
            // La fenêtre de Bureau-Salon est au premier plan.
            active: Some("b".into()),
            computers: vec![atelier(), Computer { online: Some(true), ..salon() }, Computer { id: "c".into(), name: "Studio".into(), ..atelier() }],
            ..Default::default()
        };
        let fr = lines(&s, Lang::Fr);
        // Le haut du menu : la session de la fenêtre active.
        assert_eq!(fr[..3], ["~En session sur Bureau-Salon", "~4,2 ms · 120 i/s · 60 Mbit/s", "Plein écran"]);
        // Les deux ordinateurs en session sont cochés ; Garder le son coché
        // pour Bureau-Salon.
        let a = fr.iter().position(|x| x == "[x] {vert} Atelier — En ligne >").unwrap();
        assert_eq!(fr[a + 1..a + 4], ["  Afficher", "  Garder le son", "  Se déconnecter"]);
        let b = fr.iter().position(|x| x == "[x] {vert} Bureau-Salon — En ligne >").unwrap();
        assert_eq!(fr[b + 1..b + 4], ["  Afficher", "  [x] Garder le son", "  Se déconnecter"]);
        let c = fr.iter().position(|x| x == "{vert} Studio — En ligne >").unwrap();
        assert_eq!(fr[c + 1], "  Se connecter (Jeu)");
        let en = lines(&s, Lang::En);
        let b = en.iter().position(|x| x == "[x] {vert} Bureau-Salon — Online >").unwrap();
        assert_eq!(en[b + 1..b + 4], ["  Show", "  [x] Keep sound", "  Disconnect"]);
        // Les commandes portent l'ordinateur.
        let mut c = Vec::new();
        cmds(&build(&s, &tr(Lang::Fr)), &mut c);
        assert!(c.contains(&Cmd::Statistics("b".into())));
        assert!(c.contains(&Cmd::Show("a".into())));
        assert!(c.contains(&Cmd::KeepSound("a".into())));
        assert!(c.contains(&Cmd::Disconnect("a".into())));
        assert!(c.contains(&Cmd::Connect("c".into())));
        // Infobulle : le nombre d'ordinateurs.
        assert_eq!(tooltip(&s, &tr(Lang::Fr)), "KyberNow Link · en session sur 2 ordinateurs");
        assert_eq!(tooltip(&s, &tr(Lang::En)), "KyberNow Link · in session on 2 computers");
        // Le menu « Ordinateurs » de la barre : le même choix.
        assert_eq!(pick(&s, &s.computers[0]), Pick::Show);
        assert_eq!(pick(&s, &s.computers[2]).code(), "connect");
        assert_eq!(status(&s.computers[2]), (Seen::Online, Dot::Ok));
        let offline = salon();
        assert_eq!(pick(&State::default(), &offline).code(), "wake");
        assert_eq!(pick(&State::default(), &Computer { can_wake: false, ..offline.clone() }).code(), "retry");
        assert_eq!(pick(&State::default(), &Computer { waking: true, ..offline.clone() }).code(), "waking");
        assert_eq!(status(&Computer { waking: true, ..offline }), (Seen::Offline, Dot::Warn));
        // Quatre en session : Se connecter inactif.
        let full = State { full: true, ..s };
        assert_eq!(pick(&full, &full.computers[2]).code(), "full");
        let l = lines(&full, Lang::Fr);
        let c = l.iter().position(|x| x == "{vert} Studio — En ligne >").unwrap();
        assert_eq!(l[c + 1], "  ~Se connecter (Jeu)");
    }

    fn guest_session(role: ShareRole) -> Session {
        Session {
            id: "guest-7".into(),
            name: "Atelier".into(),
            latency_ms: Some(9.0),
            fps: Some(60),
            mbps: None,
            fullscreen: false,
            multi: false,
            mouse: "auto".into(),
            keep_sound: false,
            guest: Some(role),
        }
    }

    #[test]
    fn invite_d_un_partage() {
        let s = State { sessions: vec![guest_session(ShareRole::Spectator)], active: Some("guest-7".into()), computers: vec![atelier()], ..Default::default() };
        let fr = lines(&s, Lang::Fr);
        // Titre, rôle, mesures sans débit ; ni souris (rien ne part de ce PC), ni « Se déconnecter ».
        assert_eq!(fr[..7], ["~Invité sur Atelier", "~Spectateur", "~9,0 ms · 60 i/s", "Plein écran", "Statistiques", "Quitter le partage", "---"]);
        assert!(!fr.iter().any(|l| l.starts_with("Souris") || l.contains("Se déconnecter")));
        // L'ordinateur de l'app du même nom n'est pas en session : son sous-menu se connecte.
        assert_eq!(fr[7..9], ["{vert} Atelier — En ligne >", "  Se connecter (Jeu)"]);
        let en = lines(&s, Lang::En);
        assert_eq!(en[..6], ["~Guest on Atelier", "~Spectator", "~9.0 ms · 60 fps", "Full screen", "Statistics", "Leave"]);
        // Joueur : « Joueur » ; la souris seulement avec le clavier et la souris.
        let player = State { sessions: vec![guest_session(ShareRole::Player)], ..s.clone() };
        assert_eq!(lines(&player, Lang::Fr)[1], "~Joueur");
        assert!(!lines(&player, Lang::Fr).iter().any(|l| l.starts_with("Souris")));
        let kbm = State { sessions: vec![guest_session(ShareRole::PlayerKbm)], ..s.clone() };
        let l = lines(&kbm, Lang::Fr);
        assert_eq!(l[1], "~Joueur");
        assert_eq!(l[4..8], ["Souris >", "  (o) Auto", "  Souris du bureau", "  Souris des jeux"]);
        // La commande de sortie porte la session de l'invité.
        let mut c = Vec::new();
        cmds(&build(&s, &tr(Lang::Fr)), &mut c);
        assert!(c.contains(&Cmd::Disconnect("guest-7".into())));
        assert!(c.contains(&Cmd::Statistics("guest-7".into())));
        assert!(!c.iter().any(|c| matches!(c, Cmd::Mouse(..))));
        // « Rejoindre avec un code » est toujours là, à côté de « Ajouter un ordinateur ».
        assert!(c.contains(&Cmd::Join));
        // Infobulle : invité sur l'ordinateur ; avec une autre session, le nombre.
        assert_eq!(tooltip(&s, &tr(Lang::Fr)), "KyberNow Link · invité sur Atelier");
        assert_eq!(tooltip(&s, &tr(Lang::En)), "KyberNow Link · guest on Atelier");
        let two = State { sessions: vec![session_atelier(), guest_session(ShareRole::Spectator)], ..s };
        assert_eq!(tooltip(&two, &tr(Lang::Fr)), "KyberNow Link · en session sur 2 ordinateurs");
    }

    #[test]
    fn station_en_cours() {
        let mut c = atelier();
        c.station.as_mut().unwrap().busy = Some(Busy::Restarting);
        let s = State { computers: vec![c.clone()], ..Default::default() };
        let l = lines(&s, Lang::Fr);
        assert_eq!(l[9..11], ["  ~La Station redémarre…", "  ~Mettre à jour la Station (0.5.3)"]);
        // Partie pendant le redémarrage : elle revient d'elle-même, pas de réveil.
        c.online = Some(false);
        c.can_wake = true;
        let s = State { computers: vec![c.clone()], ..Default::default() };
        let l = lines(&s, Lang::Fr);
        assert_eq!(l[..2], ["{ambre} Atelier — Hors ligne >", "  ~Se connecter (Jeu)"]);
        assert!(l.contains(&"  ~La Station redémarre…".to_string()));
        assert!(!l.contains(&"  Ouvrir Admin".to_string()));
        c.station.as_mut().unwrap().busy = Some(Busy::Updating);
        let l = lines(&State { computers: vec![c], ..Default::default() }, Lang::En);
        assert!(l.contains(&"  ~Restart the Station".to_string()));
        assert!(l.contains(&"  ~Updating the Station…".to_string()));
    }

    #[test]
    fn station_trop_ancienne_et_personnalise() {
        let c = Computer { station: None, custom: true, profile: "max".into(), ..atelier() };
        let s = State { computers: vec![c], ..Default::default() };
        let l = lines(&s, Lang::Fr);
        assert_eq!(
            l[..10],
            [
                "{vert} Atelier — En ligne >",
                "  Se connecter (Personnalisé)",
                "  Profil >",
                "    Jeu",
                "    Bureau",
                "    Max",
                "    (o) ~Personnalisé",
                "  Réglages",
                "  ---",
                "  Ouvrir Admin",
            ]
        );
        assert_eq!(l[10], "---");
    }

    #[test]
    fn commandes() {
        let s = State { computers: vec![atelier(), salon()], ..Default::default() };
        let mut c = Vec::new();
        cmds(&build(&s, &tr(Lang::Fr)), &mut c);
        assert!(c.contains(&Cmd::Connect("a".into())));
        assert!(c.contains(&Cmd::Profile("b".into(), "max")));
        assert!(c.contains(&Cmd::Update("a".into())));
        assert!(c.contains(&Cmd::Wake("b".into())));
        assert!(!c.contains(&Cmd::Admin("b".into())));
        assert_eq!(c.last(), Some(&Cmd::Quit));
    }

    #[test]
    fn notification_elidee() {
        let t = tr(Lang::Fr);
        assert_eq!(t("notify.station_update_title", &[("name", "Atelier")]), "Mise à jour de la Station d'Atelier");
        assert_eq!(t("notify.station_update_title", &[("name", "Bureau-Salon")]), "Mise à jour de la Station de Bureau-Salon");
        assert_eq!(tr(Lang::En)("notify.station_update_title", &[("name", "Atelier")]), "Station update for Atelier");
    }

    #[test]
    fn catalogues() {
        assert_eq!(APP.mismatched_keys(), Vec::<String>::new());
    }
}
