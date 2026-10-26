//! Menu de l'icône près de l'horloge (docs/link-ui.md, « Icône près de
//! l'horloge ») : ce qu'il montre selon l'état de l'app, de haut en bas. Sans
//! rien de Windows, pour se tester sur le Mac : `ui.rs` donne l'état
//! ([`State`]), `tray.rs` en fait un menu de Windows et rend à `ui.rs` la
//! commande choisie ([`Cmd`]).

/// Textes : clé du catalogue de l'app (`src/i18n/`) et paramètres.
pub type Tr<'a> = &'a dyn Fn(&str, &[(&str, &str)]) -> String;

/// Ce que fait une ligne du menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    Fullscreen,
    /// Mode de la souris de la session (`auto`, `bureau`, `jeu`), gardé pour l'ordinateur.
    Mouse(&'static str),
    Statistics,
    Disconnect,
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

/// La session en cours.
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
    /// Mode de la souris de l'ordinateur : `auto`, `bureau` ou `jeu`.
    pub mouse: String,
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
    pub session: Option<Session>,
    pub computers: Vec<Computer>,
    /// Version de l'app qui attend d'être installée.
    pub app_update: Option<String>,
}

const PROFILES: [&str; 3] = ["jeu", "bureau", "max"];
/// Dans l'ordre des réglages de l'ordinateur.
const MOUSE_MODES: [&str; 3] = ["auto", "bureau", "jeu"];

/// Le menu, de haut en bas.
pub fn build(s: &State, tr: Tr) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(session) = &s.session {
        out.push(Item::inactive(tr("tray.in_session", &[("name", &session.name)])));
        out.push(Item::inactive(measures(session, tr)));
        out.push(Item::Line {
            text: tr("tray.fullscreen", &[]),
            cmd: Some(Cmd::Fullscreen),
            enabled: true,
            mark: if session.fullscreen { Mark::Check } else { Mark::None },
            default: false,
        });
        let modes = MOUSE_MODES
            .iter()
            .map(|m| Item::Line {
                text: tr(&format!("tray.mouse_modes.{m}"), &[]),
                cmd: Some(Cmd::Mouse(m)),
                enabled: true,
                mark: if session.mouse == *m { Mark::Radio } else { Mark::None },
                default: false,
            })
            .collect();
        out.push(Item::Sub { text: tr("tray.mouse", &[]), key: None, checked: false, dot: None, items: modes });
        out.push(Item::line(tr("tray.statistics", &[]), Some(Cmd::Statistics)));
        out.push(Item::line(tr("tray.disconnect", &[]), Some(Cmd::Disconnect)));
        out.push(Item::Separator);
    }
    for c in &s.computers {
        let (state, dot) = match c.online {
            Some(true) => (tr("tray.online", &[]), Dot::Ok),
            Some(false) if c.waking || c.station.as_ref().is_some_and(|st| st.busy.is_some()) => (tr("tray.offline", &[]), Dot::Warn),
            Some(false) => (tr("tray.offline", &[]), Dot::Bad),
            None => (tr("tray.searching", &[]), Dot::Faint),
        };
        out.push(Item::Sub {
            text: tr("tray.computer", &[("name", &c.name), ("state", &state)]),
            key: Some(c.id.clone()),
            checked: s.session.as_ref().is_some_and(|x| x.id == c.id),
            dot: Some(dot),
            items: computer(s, c, tr),
        });
    }
    if !s.computers.is_empty() {
        out.push(Item::Separator);
    }
    out.push(Item::Line { text: tr("tray.open", &[]), cmd: Some(Cmd::Open), enabled: true, mark: Mark::None, default: true });
    out.push(Item::line(tr("tray.add", &[]), Some(Cmd::Add)));
    out.push(Item::line(tr("tray.app_settings", &[]), Some(Cmd::AppSettings)));
    out.push(match &s.app_update {
        Some(v) => Item::line(tr("tray.app_update", &[("version", v)]), Some(Cmd::AppSettings)),
        None => Item::line(tr("tray.check_updates", &[]), Some(Cmd::CheckUpdates)),
    });
    out.push(Item::Separator);
    out.push(Item::line(tr("tray.quit", &[]), Some(Cmd::Quit)));
    out
}

/// « 11,6 ms · 60 i/s · 80 Mbit/s » (la latence comme dans la barre de
/// session, au dixième) ; « — » pour ce qui n'est pas encore mesuré.
fn measures(s: &Session, tr: Tr) -> String {
    let dash = || "—".to_string();
    let decimal = tr("tray.decimal", &[]);
    let latency = s.latency_ms.map(|v| format!("{v:.1}").replace('.', &decimal)).unwrap_or_else(dash);
    let fps = s.fps.map(|v| v.to_string()).unwrap_or_else(dash);
    let mbps = s.mbps.map(|v| v.to_string()).unwrap_or_else(dash);
    tr("tray.measures", &[("latency", &latency), ("fps", &fps), ("mbps", &mbps)])
}

/// Le sous-menu d'un ordinateur.
pub fn computer(s: &State, c: &Computer, tr: Tr) -> Vec<Item> {
    let mut out = Vec::new();
    let this_session = s.session.as_ref().is_some_and(|x| x.id == c.id);
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
    out.push(if this_session {
        Item::line(tr("tray.disconnect", &[]), Some(Cmd::Disconnect))
    } else if c.online == Some(false) && busy.is_some() {
        // La Station redémarre ou s'installe : l'ordinateur revient de lui-même.
        connect(false)
    } else if c.online == Some(false) && c.waking {
        Item::inactive(tr("tray.waking", &[]))
    } else if c.online == Some(false) && c.can_wake {
        Item::line(tr("tray.wake", &[]), Some(Cmd::Wake(c.id.clone())))
    } else if c.online == Some(false) {
        Item::line(tr("tray.retry", &[]), Some(Cmd::Retry(c.id.clone())))
    } else {
        // Une session à la fois.
        connect(s.session.is_none())
    });

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
    match &s.session {
        Some(session) => tr("tray.tooltip_session", &[("name", &session.name)]),
        None => tr("tray.tooltip", &[]),
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

    const BOTTOM_FR: [&str; 6] = ["Ouvrir KyberNow Link *", "Ajouter un ordinateur", "Réglages de l'app", "Rechercher des mises à jour", "---", "Quitter KyberNow Link"];

    #[test]
    fn sans_ordinateur() {
        assert_eq!(lines(&State::default(), Lang::Fr), BOTTOM_FR);
        let en = lines(&State { app_update: Some("0.5.3".into()), ..Default::default() }, Lang::En);
        assert_eq!(en, ["Open KyberNow Link *", "Add a computer", "App settings", "Version 0.5.3 available", "---", "Quit KyberNow Link"]);
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

    #[test]
    fn en_session() {
        let s = State {
            session: Some(Session { id: "a".into(), name: "Atelier".into(), latency_ms: Some(11.6), fps: Some(60), mbps: Some(80), fullscreen: true, mouse: "jeu".into() }),
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
        // L'ordinateur de la session est coché, son sous-menu se déconnecte ;
        // l'autre ne se connecte pas (une session à la fois).
        assert_eq!(l[10..12], ["[x] {vert} Atelier — En ligne >", "  Se déconnecter"]);
        let other = l.iter().position(|x| x == "{vert} Bureau-Salon — En ligne >").unwrap();
        assert_eq!(l[other + 1], "  ~Se connecter (Bureau)");
        let en = lines(&s, Lang::En);
        assert_eq!(
            en[..9],
            ["~In session on Atelier", "~11.6 ms · 60 fps · 80 Mbit/s", "[x] Full screen", "Mouse >", "  Auto", "  Desktop mouse", "  (o) Game mouse", "Statistics", "Disconnect"]
        );
        // Choisir un mode : la commande porte le mode.
        let menu = build(&s, &tr(Lang::Fr));
        let Item::Sub { items, .. } = &menu[3] else { panic!("sous-menu Souris") };
        assert!(matches!(&items[0], Item::Line { cmd: Some(Cmd::Mouse("auto")), .. }));
        // Mesures pas encore arrivées.
        let mut early = s.clone();
        early.session.as_mut().unwrap().latency_ms = None;
        early.session.as_mut().unwrap().fps = None;
        early.session.as_mut().unwrap().fullscreen = false;
        let l = lines(&early, Lang::Fr);
        assert_eq!(l[1..3], ["~— ms · — i/s · 80 Mbit/s", "Plein écran"]);
        assert_eq!(tooltip(&s, &tr(Lang::Fr)), "KyberNow Link · en session sur Atelier");
        assert_eq!(tooltip(&s, &tr(Lang::En)), "KyberNow Link · in session on Atelier");
        assert_eq!(tooltip(&State::default(), &tr(Lang::Fr)), "KyberNow Link");
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
