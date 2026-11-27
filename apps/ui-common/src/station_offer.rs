//! La mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à
//! jour des Stations ») : ce que le cœur calcule (`station_offer`, par la
//! couche appareil) tel que l'interface le garde, le bandeau de l'accueil,
//! l'avis après une mise à jour de l'app et leurs textes. Le reste (focus,
//! actions, « Plus tard ») est dans chaque app.

use crate::i18n::{english, tr};

/// D'où vient la mise à jour en cours : les réglages, le bandeau de l'accueil ou l'avis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Origin {
    #[default]
    Settings,
    Banner,
    Notice,
}

/// Le bandeau que le cœur donne pour un ordinateur (`station_offer::Banner`).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum OfferBanner {
    #[default]
    None,
    Update(String),
    Installing,
    Setup(String),
    Away(String),
}

/// Ce que le cœur dit d'un ordinateur (`station_offer::banner`, `chip`, `notice_version`), recalculé par la couche
/// appareil à chaque tour de boucle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StationOffer {
    pub banner: OfferBanner,
    pub chip: bool,
    pub notice_version: Option<String>,
}

/// Le bandeau tel que l'accueil le dessine.
#[derive(Clone, Debug, PartialEq)]
pub enum HomeBanner {
    /// « Mettre à jour », « Plus tard ».
    Update(String),
    /// « Mise à jour de la Station… », sans bouton.
    Busy,
    /// L'installateur, sur l'ordinateur : « Plus tard » seul.
    Setup(String),
    /// Hors de chez vous : « Plus tard » seul.
    Away(String),
    /// Le texte d'échec des réglages, « Réessayer » et « Plus tard ».
    Failed(String),
}

impl HomeBanner {
    /// Le texte du bandeau.
    pub fn text(&self) -> String {
        match self {
            HomeBanner::Update(v) => {
                if english() {
                    format!("KyberNow Station {v} is available for this computer. About a minute.")
                } else {
                    format!("KyberNow Station {v} est disponible pour cet ordinateur. Environ une minute.")
                }
            }
            HomeBanner::Busy => tr("Mise à jour de la Station…", "Updating the Station…").into(),
            HomeBanner::Setup(v) => {
                if english() {
                    format!("KyberNow Station {v} is available. It installs on the computer, with the installer.")
                } else {
                    format!("KyberNow Station {v} est disponible. Elle s'installe sur l'ordinateur, avec l'installateur.")
                }
            }
            HomeBanner::Away(v) => {
                if english() {
                    format!("KyberNow Station {v} is available. Away from home, the app can't update the Station: do it once you're back.")
                } else {
                    format!("KyberNow Station {v} est disponible. Hors de chez vous, l'app ne peut pas mettre à jour la Station : faites-le une fois rentré.")
                }
            }
            HomeBanner::Failed(m) => m.clone(),
        }
    }

    /// Le bouton principal du bandeau : « Mettre à jour », « Réessayer » ; aucun pour l'installateur, hors de chez
    /// vous et pendant la mise à jour.
    pub fn main_label(&self) -> Option<&'static str> {
        match self {
            HomeBanner::Update(_) => Some(tr("Mettre à jour", "Update")),
            HomeBanner::Failed(_) => Some(tr("Réessayer", "Try again")),
            _ => None,
        }
    }

    /// « Plus tard », sauf pendant la mise à jour.
    pub fn later(&self) -> bool {
        !matches!(self, HomeBanner::Busy)
    }
}

/// Une ligne de l'avis : où en est la Station de cet ordinateur.
#[derive(Clone, Debug, PartialEq)]
pub enum RowState {
    Ready,
    /// « Mise à jour… »
    Busy,
    /// « À jour »
    Done,
    /// Un appareil est en session (son nom) : « Mettre à jour quand même ».
    Refused(String),
    /// Le texte d'échec des réglages.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoticeRow {
    pub host: String,
    pub name: String,
    pub version: String,
    pub state: RowState,
}

/// L'avis après une mise à jour de l'app : une ligne par Station à mettre à jour, celles à mettre à jour l'une après
/// l'autre (« Tout mettre à jour »).
#[derive(Clone, Debug, PartialEq)]
pub struct StationNotice {
    pub rows: Vec<NoticeRow>,
    /// Les ordinateurs à mettre à jour, dans l'ordre, et s'il faut couper une session (« Mettre à jour quand même »).
    pub queue: Vec<(String, bool)>,
    /// La version de l'app, dans le texte.
    pub app_version: String,
}

impl StationNotice {
    pub fn single(&self) -> bool {
        self.rows.len() == 1
    }

    /// Toutes à jour : « Plus tard » devient « Fermer ».
    pub fn all_done(&self) -> bool {
        self.rows.iter().all(|r| r.state == RowState::Done)
    }

    pub fn title(&self) -> String {
        match self.rows.as_slice() {
            [r] if english() => format!("{} has an update too", r.name),
            [r] => format!("{} a aussi une mise à jour", r.name),
            _ => tr("Vos Stations aussi ont une mise à jour", "Your Stations have an update too").into(),
        }
    }

    pub fn text(&self) -> String {
        let v = &self.app_version;
        match (self.single(), english()) {
            (true, true) => format!("KyberNow Link is now {v}. Update the Station too: about a minute, a session in progress will be cut off."),
            (true, false) => format!("KyberNow Link est passée en {v}. Mettez aussi la Station à jour : environ une minute, une session en cours sera coupée."),
            (false, true) => format!("KyberNow Link is now {v}. Update your computers too: about a minute each, a session in progress will be cut off."),
            (false, false) => format!("KyberNow Link est passée en {v}. Mettez aussi vos ordinateurs à jour : environ une minute chacun, une session en cours sera coupée."),
        }
    }

    /// Le bouton principal : « Mettre à jour la Station » (une Station ; « Mettre à jour quand même » après un refus,
    /// « Réessayer » après un échec), « Tout mettre à jour » (plusieurs) ; aucun quand il n'y a plus rien à lancer.
    pub fn main_label(&self) -> Option<&'static str> {
        if self.single() {
            return match &self.rows[0].state {
                RowState::Ready => Some(tr("Mettre à jour la Station", "Update the Station")),
                RowState::Refused(_) => Some(update_anyway()),
                RowState::Failed(_) => Some(tr("Réessayer", "Try again")),
                RowState::Busy | RowState::Done => None,
            };
        }
        self.rows.iter().any(|r| matches!(r.state, RowState::Ready | RowState::Failed(_))).then(|| tr("Tout mettre à jour", "Update all"))
    }

    /// « Plus tard », puis « Fermer » quand toutes sont à jour.
    pub fn later_label(&self) -> &'static str {
        if self.all_done() { tr("Fermer", "Close") } else { tr("Plus tard", "Later") }
    }
}

/// « Version {v} » d'une ligne de l'avis.
pub fn version_line(v: &str) -> String {
    format!("{} {v}", tr("Version", "Version"))
}

/// L'état d'une ligne : « Mise à jour… », « À jour », « {appareil} est connecté », l'échec.
pub fn row_state_text(s: &RowState) -> Option<String> {
    match s {
        RowState::Ready => None,
        RowState::Busy => Some(tr("Mise à jour…", "Updating…").into()),
        RowState::Done => Some(tr("À jour", "Up to date").into()),
        RowState::Refused(device) => Some(connected_text(device)),
        RowState::Failed(m) => Some(m.clone()),
    }
}

/// « {appareil} est connecté » (le titre de la confirmation des réglages).
pub fn connected_text(device: &str) -> String {
    if english() { format!("{device} is connected") } else { format!("{device} est connecté") }
}

pub fn update_anyway() -> &'static str {
    tr("Mettre à jour quand même", "Update anyway")
}

/// La puce des cartes : « En ligne · Mise à jour ».
pub fn chip_suffix() -> &'static str {
    tr(" · Mise à jour", " · Update")
}
