//! Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations ») : le bandeau de
//! l'accueil pour l'ordinateur choisi, la puce « · Mise à jour » des cartes et l'avis au premier lancement après une
//! mise à jour de l'app. Le calcul est celui du cœur (`kybernow_core::station_offer`), fait par la couche appareil à
//! chaque tour de boucle (`device/offer.rs`) : l'interface lit son résultat (`HostLive::offer`), garde « Plus tard »
//! (`Store::station_offers`) et met à jour par le flux de « Mettre à jour la Station » des réglages
//! (`App::station_op` : confirmation si un appareil est en session, suivi jusqu'au retour, textes d'échec).

use crate::app::{Action, App, Screen, StationKind, StationRefusal, StationStep, Target};
use crate::i18n::tr;
pub use kybernow_ui::station_offer::{OfferBanner, StationOffer, Origin, HomeBanner, RowState, NoticeRow, StationNotice, version_line, row_state_text, update_anyway, chip_suffix};

impl App {
    /// Cet appareil peut mettre la Station à jour : un ordinateur appairé (compte Kyber sur cet appareil), ou
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
        self.station_op.as_ref().is_some_and(|o| o.host == id && o.kind == StationKind::Update && o.busy())
    }

    /// Le bandeau de l'accueil pour cet ordinateur (jamais en session : l'accueil ne se voit pas).
    pub fn home_banner(&self, id: &str) -> Option<HomeBanner> {
        if self.updating(id) {
            return Some(HomeBanner::Busy);
        }
        if self.station_origin == Origin::Banner
            && let Some(op) = self.station_op.as_ref().filter(|o| o.host == id && o.kind == StationKind::Update)
            && let StationStep::Failed(m) = &op.step
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
        if self.station_origin == Origin::Banner && self.station_op.as_ref().is_some_and(|o| o.host == id && !o.busy()) {
            self.station_op = None;
        }
        if !self.station_ready(&id) || self.station(&id).and_then(|s| s.installable_version()).is_none() {
            return Vec::new();
        }
        self.station_origin = Origin::Banner;
        self.focus = Target::Connect;
        self.start_station(id, StationKind::Update, false)
    }

    /// « Plus tard » du bandeau : caché pour cette version et cet ordinateur (7 jours, ou une version plus récente).
    pub fn banner_later(&mut self) -> Vec<Action> {
        let Some(id) = self.selected().map(|h| h.id.clone()) else { return Vec::new() };
        let version = match self.home_banner(&id) {
            Some(HomeBanner::Update(v) | HomeBanner::Setup(v) | HomeBanner::Away(v)) => Some(v),
            Some(HomeBanner::Failed(_)) => self.station(&id).and_then(|s| s.update_version.clone()),
            _ => None,
        };
        if self.station_origin == Origin::Banner && self.station_op.as_ref().is_some_and(|o| o.host == id && !o.busy()) {
            self.station_op = None;
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
        self.screen == Screen::Home && self.sessions.is_empty() && self.confirm.is_none() && self.autotest.is_none() && !self.setup.active && self.station_notice.is_none()
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
        if self.station_op.as_ref().is_some_and(|o| o.busy()) {
            return Vec::new();
        }
        loop {
            let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
            if n.queue.is_empty() {
                return Vec::new();
            }
            let (host, force) = n.queue.remove(0);
            let ready = self.station_ready(&host);
            let Some(n) = self.station_notice.as_mut() else { return Vec::new() };
            let Some(row) = n.rows.iter_mut().find(|r| r.host == host) else { continue };
            if !ready {
                // Hors ligne, ou passée par le relais depuis : l'échec des réglages, et la suivante.
                row.state = RowState::Failed(tr("La Station tarde à revenir. Ouvrez Admin pour en savoir plus.", "The Station is taking a while to come back. Open Admin to find out more.").into());
                continue;
            }
            row.state = RowState::Busy;
            self.station_origin = Origin::Notice;
            return self.start_station(host, StationKind::Update, force);
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
        self.station_op = None;
        true
    }

    /// À chaque tour de boucle : les lignes de l'avis suivent la mise à jour en cours, la suivante part ; une mise à
    /// jour du bandeau (ou de l'avis fermé) finie donne l'avis bref « KyberNow Station {v} est en place. ».
    pub fn offer_tick(&mut self) -> Vec<Action> {
        let Some(op) = self.station_op.clone().filter(|o| o.kind == StationKind::Update) else {
            return self.notice_next();
        };
        match self.station_origin {
            Origin::Settings => return Vec::new(),
            Origin::Notice if self.station_notice.is_some() => {
                let state = match &op.step {
                    StationStep::Asking | StationStep::Waiting { .. } => RowState::Busy,
                    StationStep::Done(_) => RowState::Done,
                    StationStep::Failed(m) => RowState::Failed(m.clone()),
                };
                let finished = !op.busy();
                if let Some(row) = self.station_notice.as_mut().and_then(|n| n.rows.iter_mut().find(|r| r.host == op.host)) {
                    row.state = state;
                }
                if finished {
                    self.station_op = None;
                    if let Some(l) = self.live.get_mut(&op.host) {
                        l.offer = Default::default();
                    }
                    return self.notice_next();
                }
            }
            _ => {
                if let StationStep::Done(m) = &op.step {
                    self.station_op = None;
                    if let Some(l) = self.live.get_mut(&op.host) {
                        l.offer = Default::default();
                    }
                    self.toast(m.clone());
                }
            }
        }
        Vec::new()
    }
}
