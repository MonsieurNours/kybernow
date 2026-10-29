//! Prêt de l'ordinateur (KyberNow Share, docs/pret.md, textes de docs/link-ui.md
//! « Prêt de l'ordinateur »), ce que l'app en tire sans rien de Windows, pour se
//! tester sur le Mac : les ordinateurs empruntés (« Ordinateur de {hôte} », gardés
//! à part des ordinateurs appairés, dans `friends.json`) et leur état sur
//! l'accueil, le panneau « Prêter l'ordinateur… » de l'hôte tel que la barre le
//! montre (code, lien, QR code, demandes, prêt en cours), l'état « Prêté à
//! {prénom} » de l'accueil de l'hôte, la barre de l'ami (« Prêté par {hôte} »,
//! temps restant, préavis) et la carte de fin. Les appels à Admin, la session
//! d'emprunt et les fenêtres sont dans `ui/lend.rs`.
//!
//! Les textes sont ceux de l'interface (`ui/src/lib/i18n`, section `lend`) : l'app
//! n'envoie que des états (`free`, `busy`…) et des heures en secondes Unix, que
//! l'interface met en mots dans la langue et à l'heure de ce PC (« jusqu'à 18 h »,
//! « Encore 1 h 20 », « Reprise dans 1:42 »). Les refus, préavis et fins viennent
//! du cœur, déjà dans la langue de l'app.

use std::path::Path;

use kybernow_core::lend::{Invite, LendBorrower, LendCodeView, LendEnd, LendHostState, LendInfo, LendKind, LendNotice, LendNoticeReason, LendRequest, LendSlot, LendStatus, LendSteam, LendView};
use kybernow_core::relay::RelayRoute;
use kybernow_core::share::{ShareTarget, ShareVia};
use serde::{Deserialize, Serialize};

use crate::sharing::{Qr, qr};

// MARK: - Ordinateurs empruntés

/// Un ordinateur emprunté par un compte d'ami : ajouté par le lien d'invitation
/// (`kybernow://borrow?…`) ou par « Ajouter un ordinateur » › « Avec un compte
/// d'ami ». Le mot de passe est au gestionnaire d'identifiants de Windows
/// (`KyberNow:lend-<id>`) seulement avec « Retenir le mot de passe » ; sinon l'app
/// le garde le temps de son lancement, et le redemande ensuite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    /// L'adresse de la Station sur son réseau, et le port d'Admin.
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// L'identifiant du compte d'ami.
    pub login: String,
    /// Le relais de la Station et son identifiant chez lui (lien d'invitation),
    /// pour l'emprunter hors de chez soi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>,
    /// Le prénom de l'hôte (« Ordinateur de {hôte} »), dit par `status`.
    #[serde(default)]
    pub owner: String,
    /// Le nom de l'ordinateur, dit par `status`.
    #[serde(default)]
    pub computer: String,
    /// « Retenir le mot de passe ».
    #[serde(default)]
    pub remember: bool,
}

impl Friend {
    /// Un ordinateur emprunté d'après son lien d'invitation (prénom et nom de
    /// l'ordinateur inconnus jusqu'au premier `status`).
    pub fn from_invite(id: String, invite: &Invite) -> Self {
        Friend {
            id,
            host: invite.host.clone(),
            port: invite.port,
            fingerprint: invite.fingerprint.clone(),
            login: invite.login.clone(),
            relay: invite.relay.clone(),
            sid: invite.sid.clone(),
            owner: String::new(),
            computer: String::new(),
            remember: false,
        }
    }

    fn invite(&self) -> Invite {
        Invite {
            host: self.host.clone(),
            port: self.port,
            fingerprint: self.fingerprint.clone(),
            login: self.login.clone(),
            relay: self.relay.clone(),
            sid: self.sid.clone(),
        }
    }

    /// La Station sur son réseau.
    pub fn target(&self) -> ShareTarget {
        self.invite().target()
    }

    /// Hors de chez soi : le relais du lien d'invitation (avec l'empreinte d'Admin).
    pub fn route(&self) -> Option<RelayRoute> {
        self.invite().relay_route()
    }

    /// La clé de son mot de passe dans le gestionnaire d'identifiants (`KyberNow:lend-<id>`).
    pub fn credential(&self) -> String {
        format!("lend-{}", self.id)
    }

    /// {hôte} des textes : le prénom de l'hôte, sinon le nom de l'ordinateur, sinon son adresse.
    pub fn owner_name(&self) -> &str {
        [self.owner.trim(), self.computer.trim(), self.host.trim()].into_iter().find(|s| !s.is_empty()).unwrap_or_default()
    }

    /// Ce que `status` apprend : le prénom de l'hôte et le nom de l'ordinateur
    /// (vrai s'ils ont changé, à réécrire dans `friends.json`).
    pub fn learn(&mut self, s: &LendStatus) -> bool {
        let mut changed = false;
        if !s.host.trim().is_empty() && s.host != self.owner {
            self.owner = s.host.clone();
            changed = true;
        }
        if !s.computer.trim().is_empty() && s.computer != self.computer {
            self.computer = s.computer.clone();
            changed = true;
        }
        changed
    }
}

/// `friends.json` : la liste, vide si le fichier manque ou se lit mal.
pub fn load_friends(path: &Path) -> Vec<Friend> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Écrit `friends.json` (posé sur le disque avant de remplacer l'ancien).
pub fn save_friends(path: &Path, friends: &[Friend]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(friends).map_err(std::io::Error::other)?;
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// Ajoute ou remplace (même Station, même compte : même adresse, même port, même
/// identifiant) ; rend l'identifiant gardé. Le prénom et le nom déjà appris restent
/// tant que le nouvel ajout ne les dit pas.
pub fn upsert(friends: &mut Vec<Friend>, mut f: Friend) -> String {
    match friends.iter_mut().find(|o| o.host.eq_ignore_ascii_case(&f.host) && o.port == f.port && o.login == f.login) {
        Some(old) => {
            f.id = old.id.clone();
            if f.owner.is_empty() {
                f.owner = std::mem::take(&mut old.owner);
            }
            if f.computer.is_empty() {
                f.computer = std::mem::take(&mut old.computer);
            }
            if f.fingerprint.is_none() {
                f.fingerprint = old.fingerprint.take();
            }
            if f.relay.is_none() || f.sid.is_none() {
                f.relay = old.relay.take();
                f.sid = old.sid.take();
            }
            *old = f;
            old.id.clone()
        }
        None => {
            let id = f.id.clone();
            friends.push(f);
            id
        }
    }
}

/// Ce que l'app sait de l'ordinateur d'un ami (la carte de l'accueil).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Seen {
    /// Pas encore demandé.
    #[default]
    Unknown,
    /// La réponse de `status`.
    Status(LendStatus),
    /// Admin ne répond pas (ni sur le réseau, ni par le relais) : « Hors ligne ».
    Offline,
    /// « Identifiant ou mot de passe refusé. »
    Refused,
    /// Station sans prêt : « Le prêt demande KyberNow Station 0.9.0… »
    Unsupported,
    /// Mot de passe ni retenu ni donné depuis le lancement de l'app : rien à demander.
    NoPassword,
}

impl Seen {
    /// Le code de l'état pour l'interface : `free`, `busy`, `lent`, `locked`,
    /// `off` (ceux du cœur), `offline`, `refused`, `unsupported`, `password`,
    /// `unknown`.
    pub fn code(&self) -> &'static str {
        match self {
            Seen::Unknown => "unknown",
            Seen::Status(s) => s.state.as_str(),
            Seen::Offline => "offline",
            Seen::Refused => "refused",
            Seen::Unsupported => "unsupported",
            Seen::NoPassword => "password",
        }
    }
}

/// La carte « Ordinateur de {hôte} » de l'accueil (`lend_friends`, événement
/// `lend-friends`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FriendView {
    pub id: String,
    /// {hôte} : son prénom, sinon le nom de l'ordinateur, sinon son adresse.
    pub owner: String,
    pub computer: String,
    pub login: String,
    pub remember: bool,
    /// Voir [`Seen::code`].
    pub state: &'static str,
    /// Le créneau de ce compte (`None` : pas de créneau), en secondes Unix :
    /// l'interface dit « Votre créneau : mercredi, 14 h – 18 h » ou « Votre
    /// créneau, jusqu'à 18 h ».
    pub slot: Option<LendSlot>,
    /// Joignable hors de chez soi (le lien d'invitation donnait le relais).
    pub relay: bool,
}

impl FriendView {
    pub fn new(f: &Friend, seen: &Seen) -> Self {
        FriendView {
            id: f.id.clone(),
            owner: f.owner_name().to_string(),
            computer: f.computer.clone(),
            login: f.login.clone(),
            remember: f.remember,
            state: seen.code(),
            slot: match seen {
                Seen::Status(s) => s.slot,
                _ => None,
            },
            relay: f.route().is_some(),
        }
    }
}

// MARK: - L'ami : emprunter

/// Ce que `lend_borrow` (et « Revenir ») rend : le prêt démarre (`ready`, la
/// session à ouvrir par `lend_connect`), ou l'hôte doit accepter (`pending` :
/// « En attente de {hôte}… », l'interface rappelle `lend_wait`).
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BorrowOutcome {
    Ready(LoanReady),
    Pending { wait: u64, host: String },
}

/// Réponse de `lend_wait` (un appel tenu 30 s au plus par le cœur) : toujours
/// en attente (`waiting`), accepté (`ready`), ou « Annuler » est passé
/// (`cancelled`). Le refus (« {hôte} a refusé. », « {hôte} n'a pas répondu. »)
/// est l'erreur de la commande.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LendWaitOutcome {
    Waiting,
    Ready(LoanReady),
    Cancelled,
}

/// La session d'emprunt à ouvrir : de quoi montrer le passage (« Ordinateur de
/// {hôte} », la teinte de son Admin).
#[derive(Debug, Clone, Serialize)]
pub struct LoanReady {
    /// `lend-N` : la session d'emprunt dans tous les événements.
    pub id: String,
    /// « Ordinateur de {hôte} ».
    pub title: String,
    pub host: String,
    pub fingerprint: Option<String>,
}

/// La barre de l'ami (barre réduite, dans `bar-shown` et l'événement
/// `lend-borrow`) : « Prêté par {hôte} », la fin du prêt (`until`, `None` :
/// « Jusqu'à ce que {hôte} reprenne ») et le préavis en cours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BorrowBar {
    pub host: String,
    pub until: Option<u64>,
    pub notice: Option<NoticeView>,
}

/// Un préavis de 2 minutes : sa raison (`taken_back`, `time_up`, `slot_end`,
/// `slot_start`), l'heure de la fin, et son texte (celui du cœur).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoticeView {
    pub reason: &'static str,
    pub at: u64,
    pub text: String,
}

pub fn notice_reason(r: LendNoticeReason) -> &'static str {
    match r {
        LendNoticeReason::TakenBack => "taken_back",
        LendNoticeReason::TimeUp => "time_up",
        LendNoticeReason::SlotEnd => "slot_end",
        LendNoticeReason::SlotStart => "slot_start",
    }
}

impl NoticeView {
    pub fn new(n: &LendNotice, text: String) -> Self {
        NoticeView { reason: notice_reason(n.reason), at: n.at, text }
    }
}

/// La fin d'une session d'emprunt, pour la carte de fin de l'accueil (dans
/// `session-ended`) : le texte du prêt (« {hôte} a repris l'ordinateur. », « Le
/// prêt est terminé. », « Votre créneau est terminé. ») avec « Fermer » ; ou,
/// fenêtre fermée ou réseau perdu, `come_back` : « Vous pouvez revenir pendant
/// 15 min. » avec « Revenir ». Rien quand l'ami a rendu l'ordinateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EndView {
    pub host: String,
    pub text: Option<String>,
    pub come_back: bool,
}

/// La carte de fin d'une session d'emprunt : `end` (le prêt l'a finie, avec
/// son texte), sinon le retour possible. `None` : l'ordinateur est rendu.
pub fn end_view(host: &str, end: Option<(LendEnd, String)>) -> Option<EndView> {
    match end {
        Some((LendEnd::Left, _)) => None,
        Some((_, text)) => Some(EndView { host: host.to_string(), text: Some(text).filter(|t| !t.is_empty()), come_back: false }),
        None => Some(EndView { host: host.to_string(), text: None, come_back: true }),
    }
}

/// Les formats que la carte graphique de ce PC décode (`codecs` de `borrow`),
/// le plus efficace d'abord ; H.264 toujours.
pub fn codecs(hevc: bool, av1: bool) -> Vec<String> {
    let mut out = Vec::new();
    if av1 {
        out.push("av1".to_string());
    }
    if hevc {
        out.push("hevc".to_string());
    }
    out.push("h264".to_string());
    out
}

// MARK: - L'hôte

/// Ce que « Prêter l'ordinateur… » peut faire, d'après `info.lend` :
/// `available`, `unsupported` (« Le prêt demande KyberNow Station 0.9.0 ou plus
/// récente. »), `disabled` (« Le prêt est coupé dans Admin (page « Prêt »). ») ;
/// `unknown` : l'ordinateur n'a encore rien dit (`info` pas encore lu).
pub fn availability(info_known: bool, lend: Option<LendInfo>) -> &'static str {
    match (info_known, lend) {
        (false, _) => "unknown",
        (true, None) => "unsupported",
        (true, Some(l)) if !l.available => "unsupported",
        (true, Some(l)) if !l.enabled => "disabled",
        (true, Some(_)) => "available",
    }
}

/// Les durées de « Durée », en secondes, dans l'ordre : « 1 heure »,
/// « 2 heures » (proposée), « 4 heures », « Jusqu'à ce que je reprenne » : celles
/// de l'interface (`lendDurations` de `api.ts`, vérifié par les tests).
#[cfg_attr(not(test), allow(dead_code))]
pub const DURATIONS: [Option<u64>; 4] = [Some(3600), Some(7200), Some(14400), None];

pub fn steam_code(s: LendSteam) -> &'static str {
    match s {
        LendSteam::Friend => "friend",
        LendSteam::Host => "host",
    }
}

/// `friend` (« Son compte Steam », proposé) ou `host` (« Votre compte Steam ») ;
/// tout autre nom : `friend`.
pub fn parse_steam(s: &str) -> LendSteam {
    if s == "host" { LendSteam::Host } else { LendSteam::Friend }
}

fn kind_code(k: LendKind) -> &'static str {
    match k {
        LendKind::Code => "code",
        LendKind::Friend => "friend",
        LendKind::Slot => "slot",
    }
}

fn state_code(s: LendHostState) -> &'static str {
    match s {
        LendHostState::Free => "free",
        LendHostState::Preparing => "preparing",
        LendHostState::Lent => "lent",
        LendHostState::Ending => "ending",
    }
}

/// L'ami qui a l'ordinateur, tel que le panneau et l'accueil le montrent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BorrowerView {
    pub name: String,
    /// `code`, `friend`, `slot`.
    pub kind: &'static str,
    pub app: String,
    /// Venu par le relais : « hors de chez vous ».
    pub away: bool,
    pub since: u64,
    /// La fin prévue (`None` : jusqu'à ce que l'hôte reprenne).
    pub until: Option<u64>,
    pub steam: &'static str,
    /// Sa session est là (sinon il peut revenir pendant 15 min).
    pub connected: bool,
}

impl From<&LendBorrower> for BorrowerView {
    fn from(b: &LendBorrower) -> Self {
        BorrowerView {
            name: b.name.clone(),
            kind: kind_code(b.kind),
            app: b.app.clone(),
            away: b.via == ShareVia::Relay,
            since: b.since,
            until: b.until,
            steam: steam_code(b.steam),
            connected: b.connected,
        }
    }
}

/// Une demande d'emprunt : « {prénom} veut emprunter l'ordinateur. », avec
/// « Accepter » et « Refuser » ; « {app} · hors de chez vous » si l'ami passe
/// par le relais.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestView {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub app: String,
    pub away: bool,
    pub since: u64,
}

impl From<&LendRequest> for RequestView {
    fn from(r: &LendRequest) -> Self {
        RequestView { id: r.id.clone(), name: r.name.clone(), kind: kind_code(r.kind), app: r.app.clone(), away: r.via == ShareVia::Relay, since: r.since }
    }
}

/// Le code de séance, comme le partage : « 123 456 », le lien des apps (celui
/// du QR code), l'adresse du client web (« Copier le lien »), la durée et le
/// compte Steam choisis, la fin de sa validité (24 h).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeView {
    pub code: String,
    pub link: String,
    pub web: String,
    pub duration: Option<u64>,
    pub steam: &'static str,
    pub expires: u64,
    pub qr: Option<Qr>,
}

impl From<&LendCodeView> for CodeView {
    fn from(c: &LendCodeView) -> Self {
        CodeView {
            code: c.display_code(),
            link: c.link.clone(),
            web: c.web.clone(),
            duration: c.duration,
            steam: steam_code(c.steam),
            expires: c.expires,
            qr: qr(if c.link.is_empty() { &c.web } else { &c.link }),
        }
    }
}

/// Le panneau « Prêter l'ordinateur… » de la barre d'une session d'hôte (dans
/// `bar-shown`, l'événement `lend-state` et les réponses des actions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LendPanel {
    /// Voir [`availability`].
    pub availability: &'static str,
    /// `free`, `preparing`, `lent`, `ending`.
    pub state: &'static str,
    pub borrower: Option<BorrowerView>,
    /// Le préavis en cours chez l'hôte (raison et heure de la fin ; son texte
    /// pour `slot_start` : « Créneau de {prénom} : l'ordinateur lui est prêté
    /// dans 2 min. »).
    pub notice: Option<NoticeView>,
    pub requests: Vec<RequestView>,
    pub code: Option<CodeView>,
}

impl LendPanel {
    /// `view` : le prêt tel qu'Admin le dit (`None` : rien à dire) ;
    /// `availability` : voir [`availability`] ; `slot_text` : le texte d'un
    /// préavis de créneau chez l'hôte, d'après le prénom de l'ami.
    pub fn new(view: Option<&LendView>, availability: &'static str, slot_text: impl Fn(&LendNotice) -> String) -> Self {
        let availability = match (availability, view) {
            // Admin a répondu avec un prêt coupé.
            ("available" | "unknown", Some(v)) if !v.enabled => "disabled",
            ("unknown", Some(_)) => "available",
            (a, _) => a,
        };
        match view {
            Some(v) => LendPanel {
                availability,
                state: state_code(v.state),
                borrower: v.borrower.as_ref().map(BorrowerView::from),
                notice: v.notice.as_ref().map(|n| NoticeView::new(n, if n.reason == LendNoticeReason::SlotStart { slot_text(n) } else { String::new() })),
                requests: v.requests.iter().map(RequestView::from).collect(),
                code: v.code.as_ref().filter(|c| !c.code.is_empty()).map(CodeView::from),
            },
            None => LendPanel { availability, state: "free", borrower: None, notice: None, requests: Vec::new(), code: None },
        }
    }
}

/// L'état du prêt sur l'accueil de l'hôte (`ComputerView::lend`) : « Prêté à
/// {prénom} » (avec la fin : « … jusqu'à 18 h »), « Reprendre l'ordinateur »,
/// et pendant le préavis « Reprise dans 1:42 » et « Reprendre tout de suite ».
/// Seulement pendant un prêt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HomeLend {
    /// `preparing`, `lent`, `ending`.
    pub state: &'static str,
    pub name: String,
    pub until: Option<u64>,
    /// La fin du préavis en cours (« Reprise dans 1:42 »).
    pub notice_at: Option<u64>,
    pub notice: Option<&'static str>,
}

impl HomeLend {
    pub fn of(view: &LendView) -> Option<Self> {
        let b = view.borrower.as_ref()?;
        if view.state == LendHostState::Free {
            return None;
        }
        let notice = view.notice.filter(|n| n.reason != LendNoticeReason::SlotStart);
        Some(HomeLend {
            state: state_code(view.state),
            name: b.name.clone(),
            until: b.until,
            notice_at: notice.map(|n| n.at),
            notice: notice.map(|n| notice_reason(n.reason)),
        })
    }
}

/// Les demandes d'emprunt arrivées depuis la vue d'avant : le prénom de chacune,
/// dans l'ordre d'arrivée ; l'avis « {prénom} veut emprunter l'ordinateur. » est
/// dit pour chacune, une fois. Rien tant que `before` est inconnu (le panneau
/// montre les demandes déjà là).
pub fn new_requests(before: Option<&LendView>, after: Option<&LendView>, known: bool) -> Vec<String> {
    let Some(after) = after else { return Vec::new() };
    if !known {
        return Vec::new();
    }
    after.requests.iter().filter(|r| before.is_none_or(|b| !b.requests.iter().any(|o| o.id == r.id))).map(|r| r.name.clone()).collect()
}

/// Le préavis de créneau qui vient d'arriver chez l'hôte (« Créneau de
/// {prénom} : l'ordinateur lui est prêté dans 2 min. ») : une fois.
pub fn new_slot_notice(before: Option<&LendView>, after: Option<&LendView>) -> Option<LendNotice> {
    let n = after?.notice.filter(|n| n.reason == LendNoticeReason::SlotStart)?;
    (before.and_then(|b| b.notice) != Some(n)).then_some(n)
}

/// Le prénom de l'ami à qui le préavis de créneau prête l'ordinateur : celui de
/// l'emprunteur s'il est déjà là, sinon celui de la première demande de créneau.
pub fn slot_name(view: &LendView) -> String {
    view.borrower
        .as_ref()
        .map(|b| b.name.clone())
        .or_else(|| view.requests.iter().find(|r| r.kind == LendKind::Slot).map(|r| r.name.clone()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::i18n::Lang;
    use kybernow_core::lend::{LendAvailability, LendPeriod, parse_invite};

    fn invite() -> Invite {
        parse_invite("kybernow://borrow?host=192.168.1.100&port=9443&fp=sha256:aa&login=lea&relay=share.kybernow.app&sid=abcdefghijklmnopqrstuvwxyz").unwrap()
    }

    #[test]
    fn ordinateur_emprunte() {
        let f = Friend::from_invite("f1".into(), &invite());
        assert_eq!((f.host.as_str(), f.port, f.login.as_str()), ("192.168.1.100", 9443, "lea"));
        assert_eq!(f.target(), ShareTarget { host: "192.168.1.100".into(), port: 9443, fingerprint: Some("sha256:aa".into()) });
        assert_eq!(f.credential(), "lend-f1");
        // Hors de chez soi : le relais du lien, avec l'empreinte d'Admin.
        let r = f.route().expect("route du relais");
        assert_eq!((r.station.as_str(), r.admin_fp.as_str()), ("abcdefghijklmnopqrstuvwxyz", "sha256:aa"));
        // Sans relais (lien du client web, ou ajout par l'adresse) : pas de route.
        let local = Friend { relay: None, sid: None, ..f.clone() };
        assert!(local.route().is_none());
        // {hôte} : le prénom, sinon le nom de l'ordinateur, sinon l'adresse.
        assert_eq!(f.owner_name(), "192.168.1.100");
        let mut g = f.clone();
        let s = LendStatus { state: LendAvailability::Free, host: "Matthieu".into(), computer: "PC-SALON".into(), slot: None };
        assert!(g.learn(&s));
        assert!(!g.learn(&s), "rien de nouveau");
        assert_eq!(g.owner_name(), "Matthieu");
        assert_eq!(Friend { owner: String::new(), ..g }.owner_name(), "PC-SALON");
    }

    #[test]
    fn liste_des_ordinateurs_empruntes() {
        let mut list = Vec::new();
        let a = Friend::from_invite("a".into(), &invite());
        assert_eq!(upsert(&mut list, a.clone()), "a");
        // Le même compte sur la même Station : remplacé, l'identifiant et le prénom appris restent.
        list[0].owner = "Matthieu".into();
        let again = Friend { id: "b".into(), remember: true, relay: None, sid: None, ..a.clone() };
        assert_eq!(upsert(&mut list, again), "a");
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].owner.as_str(), list[0].remember), ("Matthieu", true));
        assert!(list[0].relay.is_some(), "le relais connu est gardé");
        // Un autre compte : un autre ordinateur emprunté.
        upsert(&mut list, Friend { id: "c".into(), login: "noe".into(), ..a });
        assert_eq!(list.len(), 2);
        // Aller-retour par le fichier.
        let dir = std::env::temp_dir().join(format!("kybernow-friends-{}", std::process::id()));
        let path = dir.join("friends.json");
        save_friends(&path, &list).unwrap();
        assert_eq!(load_friends(&path), list);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_friends(&dir.join("absent.json")).is_empty());
    }

    #[test]
    fn carte_de_l_accueil() {
        let mut f = Friend::from_invite("f1".into(), &invite());
        f.owner = "Matthieu".into();
        let slot = LendSlot { now: false, next: Some(LendPeriod { start: 100, end: 200 }) };
        let s = LendStatus { state: LendAvailability::Busy, host: "Matthieu".into(), computer: "PC-SALON".into(), slot: Some(slot) };
        let v = FriendView::new(&f, &Seen::Status(s));
        assert_eq!((v.owner.as_str(), v.state, v.relay), ("Matthieu", "busy", true));
        assert_eq!(v.slot, Some(slot));
        for (seen, code) in [
            (Seen::Unknown, "unknown"),
            (Seen::Offline, "offline"),
            (Seen::Refused, "refused"),
            (Seen::Unsupported, "unsupported"),
            (Seen::NoPassword, "password"),
        ] {
            let v = FriendView::new(&f, &seen);
            assert_eq!((v.state, v.slot), (code, None));
        }
    }

    /// Les états de la carte ont leur texte dans l'interface, le même que celui du cœur (link-ui).
    #[test]
    fn etats_de_la_carte_dans_l_interface() {
        for (lang, ui) in [(Lang::Fr, include_str!("../ui/src/lib/i18n/fr.json")), (Lang::En, include_str!("../ui/src/lib/i18n/en.json"))] {
            let ui: serde_json::Value = serde_json::from_str(ui).unwrap();
            for a in [LendAvailability::Free, LendAvailability::Busy, LendAvailability::Lent, LendAvailability::Locked, LendAvailability::Off] {
                assert_eq!(ui["lend"][format!("state_{}", a.as_str())].as_str(), Some(a.label(lang).as_str()), "{lang:?} {}", a.as_str());
            }
        }
    }

    #[test]
    fn disponibilite_du_pret() {
        assert_eq!(availability(false, None), "unknown");
        assert_eq!(availability(true, None), "unsupported");
        assert_eq!(availability(true, Some(LendInfo { available: false, enabled: true })), "unsupported");
        assert_eq!(availability(true, Some(LendInfo { available: true, enabled: false })), "disabled");
        assert_eq!(availability(true, Some(LendInfo { available: true, enabled: true })), "available");
    }

    fn view() -> LendView {
        serde_json::from_str(
            r#"{"enabled":true,"state":"lent",
                "borrower":{"name":"Léa","kind":"friend","app":"ios","via":"relay","since":10,"until":7210,"steam":"friend","connected":true},
                "notice":{"reason":"taken_back","at":500},
                "requests":[{"id":"r1","name":"Noé","kind":"code","app":"web","via":"lan","since":20}],
                "code":{"code":"123456","link":"kybernow://join?host=192.168.1.100&port=9443&code=123456","web":"https://192.168.1.100:9443/link/?join=123456","duration":7200,"steam":"host","expires":86400}}"#,
        )
        .unwrap()
    }

    #[test]
    fn panneau_de_l_hote() {
        let v = view();
        let p = LendPanel::new(Some(&v), "available", |_| String::new());
        assert_eq!((p.availability, p.state), ("available", "lent"));
        let b = p.borrower.as_ref().unwrap();
        assert_eq!((b.name.as_str(), b.kind, b.away, b.until, b.steam, b.connected), ("Léa", "friend", true, Some(7210), "friend", true));
        assert_eq!(p.notice.as_ref().map(|n| (n.reason, n.at)), Some(("taken_back", 500)));
        assert_eq!(p.requests.iter().map(|r| (r.name.as_str(), r.away)).collect::<Vec<_>>(), [("Noé", false)]);
        let c = p.code.as_ref().unwrap();
        // Comme le partage : « 123 456 », le QR code porte le lien des apps.
        assert_eq!((c.code.as_str(), c.duration, c.steam), ("123 456", Some(7200), "host"));
        assert!(c.qr.as_ref().is_some_and(|q| q.size >= 21));
        // Admin dit le prêt coupé : le panneau aussi, même si `info` disait autre chose.
        let off = LendView { enabled: false, ..LendView::default() };
        assert_eq!(LendPanel::new(Some(&off), "available", |_| String::new()).availability, "disabled");
        // Pas de vue : rien de prêté, ce que dit `info`.
        let none = LendPanel::new(None, "unsupported", |_| String::new());
        assert_eq!((none.availability, none.state, none.code.is_none()), ("unsupported", "free", true));
        // Une vue d'Admin sans `info` connu : le prêt existe.
        assert_eq!(LendPanel::new(Some(&LendView { enabled: true, ..LendView::default() }), "unknown", |_| String::new()).availability, "available");
    }

    #[test]
    fn accueil_de_l_hote() {
        let v = view();
        let h = HomeLend::of(&v).unwrap();
        assert_eq!((h.state, h.name.as_str(), h.until, h.notice_at, h.notice), ("lent", "Léa", Some(7210), Some(500), Some("taken_back")));
        // Rien de prêté : rien sur l'accueil.
        assert!(HomeLend::of(&LendView::default()).is_none());
        assert!(HomeLend::of(&LendView { borrower: None, ..v.clone() }).is_none());
        // Le préavis d'un créneau n'est pas une reprise.
        let slot = LendView { notice: Some(LendNotice { reason: LendNoticeReason::SlotStart, at: 9 }), ..v };
        assert_eq!(HomeLend::of(&slot).unwrap().notice_at, None);
    }

    #[test]
    fn demandes_qui_arrivent() {
        let before = view();
        let mut after = before.clone();
        after.requests.push(LendRequest { id: "r2".into(), name: "Inès".into(), ..LendRequest::default() });
        assert_eq!(new_requests(Some(&before), Some(&after), true), ["Inès"]);
        assert!(new_requests(Some(&after), Some(&after), true).is_empty(), "une fois");
        // La première vue de la session ne dit pas ses demandes déjà là.
        assert!(new_requests(None, Some(&after), false).is_empty());
        // Vue d'avant connue mais vide (rien à dire) : chaque demande est nouvelle.
        assert_eq!(new_requests(None, Some(&after), true), ["Noé", "Inès"]);
        // Le préavis d'un créneau chez l'hôte : une fois.
        let slot = LendView { notice: Some(LendNotice { reason: LendNoticeReason::SlotStart, at: 9 }), ..before.clone() };
        assert!(new_slot_notice(Some(&before), Some(&slot)).is_some());
        assert!(new_slot_notice(Some(&slot), Some(&slot)).is_none());
        assert!(new_slot_notice(Some(&slot), Some(&before)).is_none(), "une reprise n'est pas un créneau");
        assert_eq!(slot_name(&slot), "Léa");
    }

    #[test]
    fn fin_de_l_emprunt() {
        // Rendu : pas de carte.
        assert_eq!(end_view("Matthieu", Some((LendEnd::Left, String::new()))), None);
        // Repris, fini : le texte du cœur, sans retour.
        let e = end_view("Matthieu", Some((LendEnd::TakenBack, LendEnd::TakenBack.text(Lang::Fr, "Matthieu")))).unwrap();
        assert_eq!((e.text.as_deref(), e.come_back), (Some("Matthieu a repris l'ordinateur."), false));
        // Fenêtre fermée, réseau perdu : « Vous pouvez revenir pendant 15 min. »
        let e = end_view("Matthieu", None).unwrap();
        assert_eq!((e.text, e.come_back), (None, true));
    }

    #[test]
    fn reponses_d_emprunt() {
        let b = || LoanReady { id: "lend-3".into(), title: "Ordinateur de Matthieu".into(), host: "Matthieu".into(), fingerprint: None };
        assert_eq!(serde_json::to_value(BorrowOutcome::Ready(b())).unwrap()["kind"], "ready");
        assert_eq!(serde_json::to_value(BorrowOutcome::Pending { wait: 2, host: "Matthieu".into() }).unwrap(), serde_json::json!({ "kind": "pending", "wait": 2, "host": "Matthieu" }));
        assert_eq!(serde_json::to_value(LendWaitOutcome::Waiting).unwrap(), serde_json::json!({ "kind": "waiting" }));
        assert_eq!(serde_json::to_value(LendWaitOutcome::Cancelled).unwrap(), serde_json::json!({ "kind": "cancelled" }));
        let api = include_str!("../ui/src/lib/api.ts");
        for name in ["kind: 'ready'", "kind: 'pending'", "kind: 'waiting'", "kind: 'cancelled'", "kind: 'lend'"] {
            assert!(api.contains(name), "{name}");
        }
    }

    #[test]
    fn formats_et_choix() {
        assert_eq!(codecs(true, true), ["av1", "hevc", "h264"]);
        assert_eq!(codecs(false, false), ["h264"]);
        assert_eq!(DURATIONS[1], Some(7200), "2 heures proposées");
        // L'interface propose les mêmes, dans le même ordre, chacune avec son libellé.
        let api = include_str!("../ui/src/lib/api.ts");
        let line = api.lines().find(|l| l.starts_with("export const lendDurations")).expect("les durées de l'interface");
        let listed = line.split_once("= [").unwrap().1.split(']').next().unwrap().split(',').map(|v| v.trim().parse::<u64>().ok()).collect::<Vec<_>>();
        assert_eq!(listed, DURATIONS);
        for ui in [include_str!("../ui/src/lib/i18n/fr.json"), include_str!("../ui/src/lib/i18n/en.json")] {
            let ui: serde_json::Value = serde_json::from_str(ui).unwrap();
            for d in DURATIONS {
                let key = d.map_or("duration_none".to_string(), |d| format!("duration_{d}"));
                assert!(ui["lend"][&key].is_string(), "{key}");
            }
        }
        assert_eq!((parse_steam("host"), parse_steam("friend"), parse_steam("?")), (LendSteam::Host, LendSteam::Friend, LendSteam::Friend));
        assert_eq!(steam_code(LendSteam::Host), "host");
    }
}
