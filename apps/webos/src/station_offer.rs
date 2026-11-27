//! Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations ») : le bandeau de
//! l'accueil pour l'ordinateur choisi (la flèche bas depuis « Se connecter » y mène), la puce « · Mise à jour » des
//! cartes et l'avis en plein écran au premier lancement d'une nouvelle version de l'app (le focus sur son bouton
//! principal). Le calcul est celui du cœur (`kybernow_core::station_offer`), fait par la couche appareil à chaque tour
//! de boucle (`device/offer.rs`) : l'interface lit son résultat (`HostLive::offer`), garde « Plus tard »
//! (`Store::station_offers`) et met à jour par le flux de « Mettre à jour la Station » des réglages (`App::station` :
//! confirmation si un appareil est en session, suivi jusqu'au retour, textes d'échec).

use crate::app::{Action, App, Dialog, Online, Screen, StationKnown, StationOp, StationPhase, StationRefusal, Target};
use crate::i18n::tr;
pub use kybernow_ui::station_offer::{OfferBanner, StationOffer, Origin, HomeBanner, RowState, NoticeRow, StationNotice, version_line, row_state_text, update_anyway, chip_suffix};

impl App {
    /// Cet appareil peut mettre la Station à jour : un ordinateur appairé (compte Kyber sur ce téléviseur), ou
    /// l'ordinateur d'un compte KyberNow avec le droit « Administrer » (`admin`). Jamais un invité ni un prêt (ils ne
    /// sont pas dans `Store::hosts`), ni « Regarder » seul.
    pub fn may_update_station(&self, id: &str) -> bool {
        if self.store.account_host(id).is_some() {
            return self.store.account_computer(id).is_some_and(|c| c.rights.iter().any(|r| r == "admin"));
        }
        self.store.host(id).is_some()
    }

    /// Une mise à jour lancée d'ici pour cet ordinateur, en cours.
    fn updating(&self, id: &str) -> bool {
        self.station.as_ref().is_some_and(|s| s.host == id && matches!(s.phase, StationPhase::Asking(StationOp::Update) | StationPhase::Waiting { op: StationOp::Update, .. }))
    }

    /// La mise à jour de cet ordinateur se lance d'ici : en ligne sur le réseau (jamais par le relais), une version
    /// installable, rien d'autre en cours.
    fn update_ready(&self, id: &str) -> bool {
        let busy = self.station.as_ref().is_some_and(|s| s.phase != StationPhase::Idle);
        let live = self.live.get(id);
        let installable = matches!(live.map(|l| &l.station), Some(StationKnown::Info(st)) if st.update_available && st.installable && !st.requires_setup && !st.installing && !st.restarting);
        !busy && installable && live.is_some_and(|l| l.online == Online::Online) && !self.station_via_relay(id)
    }

    /// La version annoncée par la Station de cet ordinateur.
    fn announced(&self, id: &str) -> Option<String> {
        match self.live.get(id).map(|l| &l.station) {
            Some(StationKnown::Info(st)) => st.update_version.clone(),
            _ => None,
        }
    }

    /// Le bandeau de l'accueil pour cet ordinateur (jamais en session : l'accueil ne se voit pas).
    pub fn home_banner(&self, id: &str) -> Option<HomeBanner> {
        if self.updating(id) {
            return Some(HomeBanner::Busy);
        }
        if self.station_origin == Origin::Banner
            && let Some(s) = self.station.as_ref().filter(|s| s.host == id && s.phase == StationPhase::Idle && !s.done)
            && let Some(m) = &s.message
        {
            return Some(HomeBanner::Failed(m.clone()));
        }
        match &self.live.get(id)?.offer.banner {
            OfferBanner::None => None,
            OfferBanner::Update(v) => Some(HomeBanner::Update(v.clone())),
            OfferBanner::Installing => Some(HomeBanner::Busy),
            OfferBanner::Setup(v) => Some(HomeBanner::Setup(v.clone())),
            OfferBanner::Away(v) => Some(HomeBanner::Away(v.clone())),
        }
    }

    /// Le bandeau de l'ordinateur choisi, s'il a un bouton (la flèche bas depuis « Se connecter » y va).
    pub fn banner_target(&self) -> Option<Target> {
        let b = self.home_banner(&self.selected()?.id)?;
        if b.main_label().is_some() {
            Some(Target::OfferUpdate)
        } else if b.later() {
            Some(Target::OfferLater)
        } else {
            None
        }
    }

    /// « Mettre à jour » (ou « Réessayer ») du bandeau : le flux des réglages ; un appareil en session ouvre leur
    /// confirmation.
    pub fn banner_update(&mut self) -> Vec<Action> {
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
        if self.station.as_ref().is_some_and(|s| s.host == id && s.phase == StationPhase::Idle) {
            self.station = None;
        }
        if !self.update_ready(&id) {
            return Vec::new();
        }
        self.station_origin = Origin::Banner;
        self.focus = Target::Connect;
        self.station_ask(&id, StationOp::Update, false)
    }

    /// « Plus tard » du bandeau : caché pour cette version et cet ordinateur (7 jours, ou une version plus récente).
    pub fn banner_later(&mut self) -> Vec<Action> {
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
        let version = match self.home_banner(&id) {
            Some(HomeBanner::Update(v) | HomeBanner::Setup(v) | HomeBanner::Away(v)) => Some(v),
            Some(HomeBanner::Failed(_)) => self.announced(&id),
            _ => None,
        };
        if self.station_origin == Origin::Banner && self.station.as_ref().is_some_and(|s| s.host == id && s.phase == StationPhase::Idle) {
            self.station = None;
        }
        self.focus = Target::Connect;
        match version.filter(|v| !v.is_empty()) {
            Some(v) => {
                self.snooze(&id, &v);
                vec![Action::Save]
            }
            None => Vec::new(),
        }
    }

    /// « Plus tard » pour cette version de cet ordinateur ; le bandeau part tout de suite (le cœur le confirme au
    /// tour suivant).
    fn snooze(&mut self, id: &str, version: &str) {
        let m = self.store.station_offers.entry(id.to_string()).or_default();
        m.snoozed_version = Some(version.to_string());
        m.snoozed_at = crate::model::now_secs();
        if let Some(l) = self.live.get_mut(id) {
            l.offer.banner = OfferBanner::None;
        }
    }

    /// L'avis peut s'ouvrir : l'accueil, sans session, sans autre fenêtre par-dessus.
    pub fn can_show_station_notice(&self) -> bool {
        self.screen == Screen::Home && self.session.is_none() && self.dialog.is_none() && self.station_notice.is_none()
    }

    /// Ouvre l'avis (les ordinateurs : identifiant, nom, version) ; le focus sur le bouton principal.
    pub fn open_station_notice(&mut self, rows: Vec<(String, String, String)>, app_version: &str) {
        if rows.is_empty() {
            return;
        }
        let rows = rows.into_iter().map(|(host, name, version)| NoticeRow { host, name, version, state: RowState::Ready }).collect();
        self.station_notice = Some(StationNotice { rows, queue: Vec::new(), app_version: app_version.into() });
        self.focus = Target::NoticeMain;
    }

    /// Le bouton principal de l'avis.
    pub fn notice_main(&mut self) -> Vec<Action> {
        let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
        let picks: Vec<(String, bool)> = n
            .rows
            .iter()
            .filter_map(|r| match r.state {
                RowState::Ready | RowState::Failed(_) => Some((r.host.clone(), false)),
                RowState::Refused(_) if n.rows.len() == 1 => Some((r.host.clone(), true)),
                _ => None,
            })
            .collect();
        for p in picks {
            if !n.queue.iter().any(|(h, _)| *h == p.0) {
                n.queue.push(p);
            }
        }
        self.notice_next()
    }

    /// « Mettre à jour » d'une ligne (« Mettre à jour quand même » après un refus).
    pub fn notice_row(&mut self, i: usize) -> Vec<Action> {
        let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
        let Some(r) = n.rows.get(i) else { return Vec::new() };
        let force = matches!(r.state, RowState::Refused(_));
        if matches!(r.state, RowState::Busy | RowState::Done) || n.queue.iter().any(|(h, _)| *h == r.host) {
            return Vec::new();
        }
        n.queue.push((r.host.clone(), force));
        self.notice_next()
    }

    /// « Plus tard » (pour chaque Station de la liste pas encore à jour) ou « Fermer ». Une mise à jour en cours
    /// continue ; le bandeau la suit.
    pub fn notice_later(&mut self) -> Vec<Action> {
        let Some(n) = self.station_notice.take() else { return Vec::new() };
        self.focus = Target::Connect;
        if self.station_origin == Origin::Notice {
            self.station_origin = Origin::Banner;
        }
        let mut acts = Vec::new();
        for r in n.rows.iter().filter(|r| !matches!(r.state, RowState::Done | RowState::Busy)) {
            self.snooze(&r.host, &r.version);
            acts = vec![Action::Save];
        }
        acts
    }

    /// La suivante de la file, quand rien n'est en cours.
    fn notice_next(&mut self) -> Vec<Action> {
        if self.station.as_ref().is_some_and(|s| s.phase != StationPhase::Idle) {
            return Vec::new();
        }
        loop {
            let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
            if n.queue.is_empty() {
                return Vec::new();
            }
            let (host, force) = n.queue.remove(0);
            if self.station.as_ref().is_some_and(|s| s.host == host) {
                self.station = None;
            }
            let ready = self.update_ready(&host);
            let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
            let Some(row) = n.rows.iter_mut().find(|r| r.host == host) else { continue };
            if !ready {
                // Hors ligne, ou passée par le relais depuis : l'échec des réglages, et la suivante.
                row.state = RowState::Failed(tr("La Station tarde à revenir. Ouvrez Admin pour en savoir plus.", "The Station is taking a while to come back. Open Admin to find out more.").into());
                continue;
            }
            row.state = RowState::Busy;
            self.station_origin = Origin::Notice;
            return self.station_ask(&host, StationOp::Update, force);
        }
    }

    /// La réponse d'Admin à une mise à jour lancée par l'avis : un appareil en session reste sur sa ligne (« Mettre à
    /// jour quand même »), sans la confirmation des réglages. Rend vrai quand elle est prise ici.
    pub(crate) fn notice_refused(&mut self, host: &str, r: &Result<Option<String>, StationRefusal>) -> bool {
        if self.station_origin != Origin::Notice {
            return false;
        }
        let Err(StationRefusal::InSession(device)) = r else { return false };
        let Some(row) = self.station_notice.as_mut().and_then(|n| n.rows.iter_mut().find(|r| r.host == host)) else { return false };
        row.state = RowState::Refused(device.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| tr("Un appareil", "A device").into()));
        self.station = None;
        true
    }

    /// À chaque tour de boucle : les lignes de l'avis suivent la mise à jour en cours, la suivante part ; une mise à
    /// jour du bandeau finie s'efface (l'avis bref « KyberNow Station {v} est en place. » est déjà dit).
    pub fn offer_tick(&mut self) -> Vec<Action> {
        let Some(ui) = self.station.clone() else { return self.notice_next() };
        let update = matches!(ui.phase, StationPhase::Asking(StationOp::Update) | StationPhase::Waiting { op: StationOp::Update, .. }) || ui.phase == StationPhase::Idle;
        if !update {
            return Vec::new();
        }
        match self.station_origin {
            Origin::Settings => return Vec::new(),
            Origin::Notice if self.station_notice.is_some() => {
                let state = match (&ui.phase, &ui.message) {
                    (StationPhase::Idle, _) if ui.done => RowState::Done,
                    (StationPhase::Idle, Some(m)) => RowState::Failed(m.clone()),
                    (StationPhase::Idle, None) => return self.notice_next(),
                    _ => RowState::Busy,
                };
                let finished = ui.phase == StationPhase::Idle;
                if let Some(row) = self.station_notice.as_mut().and_then(|n| n.rows.iter_mut().find(|r| r.host == ui.host)) {
                    row.state = state;
                }
                if finished {
                    self.station = None;
                    if let Some(l) = self.live.get_mut(&ui.host) {
                        l.offer = Default::default();
                    }
                    return self.notice_next();
                }
            }
            _ => {
                if ui.phase == StationPhase::Idle && ui.done {
                    self.station = None;
                    if let Some(l) = self.live.get_mut(&ui.host) {
                        l.offer = Default::default();
                    }
                }
            }
        }
        Vec::new()
    }

    /// La confirmation « {appareil} est connecté » d'une mise à jour lancée par le bandeau.
    pub(crate) fn banner_dialog(&self) -> bool {
        self.station_origin == Origin::Banner && matches!(self.dialog, Some(Dialog::Force { op: StationOp::Update, .. }))
    }
}
