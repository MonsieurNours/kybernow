//! Ce que l'interface montre du prêt de l'ordinateur (docs/link-ui.md, « Prêt
//! de l'ordinateur ») : l'état de l'ordinateur d'un ami, l'emprunt et la
//! session d'emprunt, la vue de l'hôte et ses demandes, les invitations, et
//! leurs textes.

use std::time::Instant;

use crate::i18n::tr;
use crate::share::{app_label, relay_wait, ShareRelay};

/// Ce que « Prêter l'ordinateur… » peut faire avec un ordinateur (`Info::lend` du cœur).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LendAvail {
    /// Une Station antérieure à la 0.9.0 (`info.lend` absent).
    Unsupported,
    /// Le prêt est coupé dans Admin (page « Prêt ») : `info.lend.enabled` faux.
    Off,
    /// La Station ne sait pas prêter cet ordinateur : `info.lend.available` faux.
    Impossible,
    Available,
}

impl LendAvail {
    /// Pourquoi « Prêter l'ordinateur… » ne peut rien ; rien quand le prêt est possible.
    pub fn text(self) -> Option<&'static str> {
        match self {
            LendAvail::Unsupported => Some(tr("Le prêt demande KyberNow Station 0.9.0 ou plus récente.", "Lending needs KyberNow Station 0.9.0 or later.")),
            LendAvail::Off => Some(tr("Le prêt est coupé dans Admin (page « Prêt »).", "Lending is off in Admin (“Lending” page).")),
            LendAvail::Impossible => Some(tr("Le prêt n'est pas possible sur cet ordinateur.", "Lending isn't possible on this computer.")),
            LendAvail::Available => None,
        }
    }
}

/// L'état de l'ordinateur d'un ami (`LendAvailability` du cœur), ou « Hors ligne » quand sa Station ne répond pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LendState {
    Free,
    Busy,
    Lent,
    Locked,
    Off,
    Offline,
    /// Le prêt en cours est le sien (`LendStatus::mine`) : fenêtre fermée ou réseau perdu, il peut revenir.
    Mine,
}

impl LendState {
    /// L'état de la carte « Ordinateur de {hôte} ».
    pub fn label(self) -> &'static str {
        match self {
            LendState::Free => tr("Disponible", "Available"),
            LendState::Busy => tr("Occupé", "In use"),
            LendState::Lent => tr("Prêté à quelqu'un d'autre", "Lent to someone else"),
            LendState::Locked => tr("Verrouillé", "Locked"),
            LendState::Off => tr("Prêt coupé", "Lending off"),
            LendState::Offline => tr("Hors ligne", "Offline"),
            LendState::Mine => tr("Vous pouvez revenir pendant 15 min.", "You can come back for 15 min."),
        }
    }

    /// « Emprunter » (libre), « Demander à {hôte} » (occupé) ou « Revenir » (son prêt) se peut ; ailleurs, le bouton est
    /// inactif.
    pub fn borrowable(self) -> bool {
        matches!(self, LendState::Free | LendState::Busy | LendState::Mine)
    }
}

/// Le créneau d'un compte d'ami (`LendSlot` du cœur) : en cours (jusqu'à `end`), ou le prochain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LendSlotUi {
    pub now: bool,
    pub start: u64,
    pub end: u64,
}

/// Ce que l'app sait de l'ordinateur d'un ami, d'après `lend::status` (sondé avec l'accueil).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LendLive {
    /// `None` : pas encore su (ou pas de mot de passe à présenter).
    pub state: Option<LendState>,
    pub slot: Option<LendSlotUi>,
}

/// D'où vient un emprunt : « Rejoindre avec un code » (un code de prêt, la recherche `attempt` de l'écran), ou la carte
/// d'un ordinateur emprunté (`Store::borrowed`, l'essai `attempt` de l'accueil). « Revenir » refait le même.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BorrowSource {
    Code { attempt: u32 },
    Account { id: String, attempt: u32 },
}

/// L'emprunt lancé depuis l'accueil (la carte « Ordinateur de {hôte} ») : l'ordinateur, l'appel à la Station
/// (« Préparation de l'ordinateur… », 30 s au plus, le temps du mode console), l'attente de l'accord de l'hôte
/// (« En attente de {hôte}… », « Annuler »), ou ce que la Station a répondu.
#[derive(Clone, Debug)]
pub struct BorrowUi {
    pub id: String,
    pub busy: bool,
    pub waiting: bool,
    pub message: Option<String>,
    pub attempt: u32,
    pub started: Instant,
}

/// « En attente de {hôte}… », “Waiting for {host}…”.
pub fn waiting_for(host: &str) -> String {
    format!("{} {host}…", tr("En attente de", "Waiting for"))
}

/// « Demander à {hôte} », “Ask {host}”.
pub fn ask_host(host: &str) -> String {
    format!("{} {host}", tr("Demander à", "Ask"))
}

/// Le temps qui reste d'un prêt, comme la Station (docs/link-ui.md, « Textes de la Station ») : « Encore 1 h 20 »,
/// « Encore 2 h », « Encore 42 min », « Encore moins d'une minute » ; les minutes sont arrondies au-dessus.
pub fn time_left_label(seconds: u64) -> String {
    let en = crate::i18n::english();
    if seconds < 60 {
        return if en { "Less than a minute left" } else { "Encore moins d'une minute" }.into();
    }
    let m = seconds.div_ceil(60);
    let (h, mm) = (m / 60, m % 60);
    match (en, h, mm) {
        (true, 0, _) => format!("{m} min left"),
        (true, _, 0) => format!("{h} h left"),
        (true, _, _) => format!("{h} h {mm:02} left"),
        (false, 0, _) => format!("Encore {m} min"),
        (false, _, 0) => format!("Encore {h} h"),
        (false, _, _) => format!("Encore {h} h {mm:02}"),
    }
}

/// « Son compte Steam », « Votre compte Steam » : le compte Steam du prêt (`LendSteam` du cœur).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LendSteam {
    /// Proposé.
    #[default]
    Friend,
    Host,
}

impl LendSteam {
    pub const ALL: [LendSteam; 2] = [LendSteam::Friend, LendSteam::Host];

    pub fn label(self) -> &'static str {
        match self {
            LendSteam::Friend => tr("Son compte Steam", "Their Steam account"),
            LendSteam::Host => tr("Votre compte Steam", "Your Steam account"),
        }
    }
}

/// « 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne ».
pub fn lend_duration_label(d: Option<u64>) -> String {
    match d {
        None => tr("Jusqu'à ce que je reprenne", "Until I take it back").into(),
        Some(s) if s <= 3600 => tr("1 heure", "1 hour").into(),
        Some(s) => format!("{} {}", s / 3600, tr("heures", "hours")),
    }
}

/// L'état du prêt chez l'hôte (`LendHostState` du cœur).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LendHostState {
    #[default]
    Free,
    Preparing,
    Lent,
    Ending,
}

/// Pourquoi le préavis de 2 minutes (`LendNoticeReason` du cœur).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LendNoticeKind {
    TakenBack,
    TimeUp,
    SlotEnd,
    SlotStart,
}

/// Une demande d'emprunt, ordinateur utilisé (`LendRequest` du cœur).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LendRequestUi {
    pub id: String,
    pub name: String,
    pub app: String,
    pub via_relay: bool,
    /// Le compte Steam proposé (celui du compte d'ami, ou du code) : « Accepter » le garde.
    pub steam: LendSteam,
    /// La demande vient d'un créneau de l'ami (l'ordinateur lui est prêté dans 2 min sans réponse).
    pub slot: bool,
}

/// La vue du prêt chez l'hôte (`LendView` du cœur) : ce qui est prêté, à qui, le préavis, les demandes, le code.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LendHostView {
    pub enabled: bool,
    pub state: LendHostState,
    /// Le prénom de l'ami qui a l'ordinateur, et la fin prévue.
    pub borrower: Option<(String, Option<u64>)>,
    pub notice: Option<(LendNoticeKind, u64)>,
    pub requests: Vec<LendRequestUi>,
    pub code: Option<LendCodeUi>,
}

/// Ce que fait l'hôte avec le prêt (une demande à Admin, avec le compte de l'appareil).
#[derive(Clone, Debug, PartialEq)]
pub enum LendOp {
    /// « Créer un code » : durée (`None` : jusqu'à ce que je reprenne) et compte Steam.
    Code(Option<u64>, LendSteam),
    /// « Annuler le code ».
    CancelCode,
    /// « Accepter » (avec le compte Steam choisi) ou « Refuser » une demande.
    Answer(String, bool, LendSteam),
    /// « Reprendre l'ordinateur » (préavis de 2 min), ou « Reprendre tout de suite ».
    TakeBack(bool),
}

/// Un lien d'invitation (`kybernow://borrow?…`, `lend::Invite` du cœur) : la Station de l'ami (adresse, port,
/// empreinte), son relais (`relay`, `sid`) pour emprunter aussi hors de chez soi, le compte d'ami (« Identifiant ») et
/// le prénom de l'hôte quand le lien le dit (`name`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InviteUi {
    pub address: String,
    pub port: u16,
    pub fingerprint: Option<String>,
    pub login: String,
    pub relay: Option<String>,
    pub sid: Option<String>,
    pub name: Option<String>,
}

impl InviteUi {
    /// Le titre de l'ajout : « Matthieu vous prête son ordinateur. » ; sans prénom (lien d'avant la 0.9.0), « Avec un
    /// compte d'ami » (docs/link-ui.md, « Textes des apps »).
    pub fn title(&self) -> String {
        match self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) if crate::i18n::english() => format!("{n} is lending you their computer."),
            Some(n) => format!("{n} vous prête son ordinateur."),
            None => tr("Avec un compte d'ami", "With a friend account").into(),
        }
    }
}

/// L'avis de la réussite : « {ordinateur} est ajouté. » (docs/link-ui.md, « Invitations »).
pub fn invitation_added_notice(computer: &str) -> String {
    if crate::i18n::english() { format!("{computer} has been added.") } else { format!("{computer} est ajouté.") }
}

/// Une session d'emprunt : l'ordinateur d'un ami (« Prêté par {hôte} »), la fin prévue, le préavis en cours, et ce qui
/// l'a finie. Ni profil ni réglages de la Station, ni presse-papiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BorrowSessionUi {
    /// Le prénom de l'hôte et le nom de son ordinateur.
    pub host: String,
    pub computer: String,
    /// La fin prévue (secondes Unix) ; `None` : « Jusqu'à ce que {hôte} reprenne ».
    pub until: Option<u64>,
    /// Le préavis de 2 minutes : l'heure de la fin.
    pub notice: Option<u64>,
    /// Le prêt a fini la session (`Platform::on_lend_end`) : sans cela, une fin sur erreur laisse revenir 15 min.
    pub ended_by_lend: bool,
    /// Le texte de la fin dite par le prêt ; « Vous avez rendu l'ordinateur. » (fin voulue, sans carte) se dit en avis
    /// sur l'accueil qui revient.
    pub end_text: Option<String>,
    /// L'emprunt d'où vient la session, que « Revenir » refait.
    pub source: BorrowSource,
}

/// Le code de séance en cours (`LendCodeView` du cœur).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LendCodeUi {
    pub code: String,
    /// Le lien des apps (le QR code) et l'adresse du client web : ceux du relais quand il tient le code
    /// (`ShareRelay::Joined`), sinon ceux du réseau.
    pub link: String,
    pub web: String,
    pub duration: Option<u64>,
    pub steam: LendSteam,
    /// Le relais du code, comme celui du partage (`None` : pas de relais réglé, ou une Station qui ne le dit pas).
    pub relay: Option<ShareRelay>,
    /// Quand l'app a vu ce code « en préparation » chez le relais pour la première fois (`LendHostView::keep_relay_since`).
    pub relay_since: Option<Instant>,
}

impl Default for BorrowUi {
    fn default() -> Self {
        Self { id: String::new(), busy: false, waiting: false, message: None, attempt: 0, started: Instant::now() }
    }
}

impl BorrowSessionUi {
    /// Un emprunt qui commence : rien de fini, pas de préavis.
    pub fn new(host: &str, computer: &str, until: Option<u64>, source: BorrowSource) -> Self {
        Self { host: host.into(), computer: computer.into(), until, notice: None, ended_by_lend: false, end_text: None, source }
    }
}

impl BorrowSessionUi {
    /// « Prêté par {hôte} », “Lent by {host}”.
    pub fn lent_by(&self) -> String {
        format!("{} {}", tr("Prêté par", "Lent by"), self.host)
    }

    /// « Encore 1 h 20 » (la fin prévue, ou celle du préavis), sinon « Jusqu'à ce que {hôte} reprenne ».
    pub fn time_left(&self, now: u64) -> String {
        match self.notice.or(self.until) {
            Some(end) => time_left_label(end.saturating_sub(now)),
            None => format!("{} {} {}", tr("Jusqu'à ce que", "Until"), self.host, tr("reprenne", "takes it back")),
        }
    }
}

impl LendSlotUi {
    /// « Votre créneau, jusqu'à 18 h » (en cours) ; « Votre créneau : mercredi, 14 h – 18 h » (le prochain).
    pub fn text(&self) -> String {
        let end = crate::time::hour_label(self.end);
        if self.now {
            format!("{} {end}", tr("Votre créneau, jusqu'à", "Your time slot, until"))
        } else {
            format!("{} {}, {} – {end}", tr("Votre créneau :", "Your time slot:"), crate::time::weekday_label(self.start), crate::time::hour_label(self.start))
        }
    }
}

impl LendRequestUi {
    /// « Léa veut emprunter l'ordinateur. »
    /// La demande d'un créneau (`slot`) dit le créneau, sans « Refuser » (docs/link-ui.md, « Textes des apps ») :
    /// « Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »
    pub fn line(&self) -> String {
        if self.slot {
            return if crate::i18n::english() { format!("{}'s time slot: the computer is lent to them in 2 min.", self.name) } else { format!("Créneau {} : l'ordinateur lui est prêté dans 2 min.", crate::i18n::de(&self.name)) };
        }
        format!("{} {}", self.name, tr("veut emprunter l'ordinateur.", "wants to borrow the computer."))
    }

    /// Sous la demande : « Android · hors de chez vous » par le relais, sinon son app.
    pub fn meta(&self) -> String {
        if self.via_relay { format!("{} · {}", app_label(&self.app), tr("hors de chez vous", "away from home")) } else { app_label(&self.app) }
    }
}

impl LendCodeUi {
    /// Le lien du relais se prépare : le lien et le QR code attendent (`RELAY_LINK_WAIT` au plus).
    pub fn relay_preparing(&self) -> bool {
        relay_wait::preparing(self.relay, self.relay_since)
    }

    pub fn relay_wait_redraw(&self) -> bool {
        relay_wait::redraw(self.relay, self.relay_since)
    }

    /// « Préparation du lien hors de chez vous… », sous l'aide.
    pub fn relay_wait_text(&self) -> Option<&'static str> {
        relay_wait::wait_text(self.relay, self.relay_since)
    }

    /// « Pas encore joignable hors de chez vous. », sous l'aide.
    pub fn relay_down_text(&self) -> Option<&'static str> {
        relay_wait::down_text(self.relay, self.relay_since)
    }
}

impl LendHostView {
    /// Garde le moment où l'app a vu le code en préparation chez le relais (`before` : la vue d'avant, même code).
    pub fn keep_relay_since(&mut self, before: Option<&LendHostView>) {
        let Some(c) = self.code.as_mut() else { return };
        let kept = before.and_then(|b| b.code.as_ref()).filter(|b| b.code == c.code).and_then(|b| b.relay_since);
        c.relay_since = relay_wait::keep_since(c.relay, c.relay_since, kept);
    }

    /// L'attente du lien du relais du code court encore : l'interface se redessine.
    pub fn relay_wait_redraw(&self) -> bool {
        self.code.as_ref().is_some_and(LendCodeUi::relay_wait_redraw)
    }

    /// L'ordinateur est prêté (ou le prêt se prépare, ou finit) : l'accueil dit « Prêté à {prénom} ».
    pub fn lent(&self) -> bool {
        self.state != LendHostState::Free && self.borrower.is_some()
    }

    /// « Prêté à Léa », « Prêté à Léa jusqu'à 18 h ».
    pub fn lent_line(&self) -> Option<String> {
        let (name, until) = self.borrower.as_ref().filter(|_| self.lent())?;
        let base = format!("{} {name}", tr("Prêté à", "Lent to"));
        Some(match until {
            Some(t) => format!("{base} {} {}", tr("jusqu'à", "until"), crate::time::hour_label(*t)),
            None => base,
        })
    }

    /// Pendant la reprise (préavis de l'hôte) : « Reprise dans 1:42 ».
    pub fn taking_back(&self, now: u64) -> Option<String> {
        let (kind, at) = self.notice.filter(|_| self.lent())?;
        (kind == LendNoticeKind::TakenBack).then(|| {
            let left = at.saturating_sub(now);
            format!("{} {}:{:02}", tr("Reprise dans", "Taking back in"), left / 60, left % 60)
        })
    }

    /// L'avis d'un créneau qui commence : « Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »
    pub fn slot_notice(&self) -> Option<String> {
        let (kind, _) = self.notice?;
        (kind == LendNoticeKind::SlotStart).then(|| {
            let slot = self.requests.iter().find(|r| r.slot).or(self.requests.first());
            let name = slot.map(|r| r.name.as_str()).or(self.borrower.as_ref().map(|(n, _)| n.as_str())).unwrap_or_default();
            if crate::i18n::english() { format!("{name}'s time slot: the computer is lent to them in 2 min.") } else { format!("Créneau {} : l'ordinateur lui est prêté dans 2 min.", crate::i18n::de(name)) }
        })
    }
}
