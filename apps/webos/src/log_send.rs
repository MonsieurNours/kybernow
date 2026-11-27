//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos ») : un téléviseur n'a pas de feuille de
//! partage, le journal passe par un QR code que le téléphone scanne (docs/relais.md, « Dépôt d'un journal »).
//!
//! Le journal est compressé (gzip) puis chiffré sur le téléviseur (AES-256-GCM, clé tirée au hasard pour ce dépôt),
//! déposé sur le relais du compte KyberNow du téléviseur, sinon le relais public, qui ne peut pas le lire : la clé ne
//! part que dans le QR code, après `#k=`, que le navigateur du téléphone n'envoie jamais. Dépôt : `KNJ1` (4 octets),
//! le nonce (12), puis le texte chiffré et son étiquette (16) ; 2 Mio au plus une fois compressé (au-delà : l'en-tête
//! et la fin du journal, la plus récente).
//!
//! Ici : l'écran (`Screen::LogSend`) et ses étapes, sans le cœur ni le réseau (l'interface se compile aussi pour les
//! captures). L'en-tête, la compression, le chiffrement et les appels au relais sont ceux du cœur
//! (`kybernow_core::journal`), appelés par `device::log_send`.

use std::time::{Duration, Instant};

use crate::app::{Action, App, Screen, Target};
use crate::i18n::tr;

/// L'écran du QR code relit l'état du dépôt à ce rythme (`journal::CHECK_EVERY` du cœur).
pub const CHECK_EVERY: Duration = Duration::from_secs(2);

/// Une étape de l'écran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogStep {
    /// « Préparation du journal… » : lu, compressé, chiffré.
    Preparing,
    /// « Envoi du journal… » : déposé sur le relais.
    Uploading,
    /// Le QR code, jusqu'à ce que le téléphone l'ouvre ou qu'il expire.
    Qr(LogDeposit),
    /// « Journal ouvert sur le téléphone. »
    Opened,
    /// « Ce QR code a expiré. » (une heure).
    Expired,
    Failed(LogFail),
}

/// Un dépôt sur le relais : de quoi le suivre et le retirer (`owner` ne part jamais ailleurs qu'au relais).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogDeposit {
    /// Le relais, tel qu'il a servi (empreinte épinglée comprise).
    pub relay: String,
    pub id: String,
    pub owner: String,
    /// L'adresse du QR code, clé comprise.
    pub url: String,
    pub until: Instant,
}

/// Ce qui empêche le dépôt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFail {
    /// Le relais ne répond pas (réseau, nom, certificat).
    Unreachable,
    /// Relais d'avant le dépôt des journaux (`404`).
    OldRelay,
    /// Trop de dépôts depuis cette adresse (`429`), ou relais plein (`503`).
    TooMany,
}

/// Ce que le relais dit d'un dépôt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogRemote {
    Waiting,
    Opened,
    /// Plus là : ouvert puis effacé, expiré, retiré.
    Gone,
    /// Pas de réponse cette fois : on relira.
    Unknown,
}

/// L'écran ouvert, et le numéro de l'envoi en cours (une réponse d'un envoi d'avant est jetée).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSendUi {
    pub step: LogStep,
    pub attempt: u32,
    pub checked: Option<Instant>,
}

// MARK: - Textes (docs/link-ui.md)

pub fn title() -> &'static str {
    tr("Envoyer le journal", "Send the log")
}

/// L'aide de la ligne des Réglages, celle des téléviseurs.
pub fn help() -> &'static str {
    tr(
        "Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Il passe par votre téléphone : vous choisissez à qui l'envoyer.",
        "If the app closes or a session cuts out, this text file tells what happened. It goes through your phone: you choose who to send it to.",
    )
}

pub fn preparing() -> &'static str {
    tr("Préparation du journal…", "Preparing the log…")
}

pub fn uploading() -> &'static str {
    tr("Envoi du journal…", "Uploading the log…")
}

pub fn scan() -> &'static str {
    tr("Scannez ce QR code avec votre téléphone, puis enregistrez ou partagez le journal.", "Scan this QR code with your phone, then save or share the log.")
}

pub fn privacy() -> &'static str {
    tr("Chiffré sur cet appareil : seul ce QR code l'ouvre. Il sert une fois, pendant une heure.", "Encrypted on this device: only this QR code opens it. It works once, for one hour.")
}

pub fn opened() -> &'static str {
    tr("Journal ouvert sur le téléphone.", "Log opened on the phone.")
}

pub fn expired() -> &'static str {
    tr("Ce QR code a expiré.", "This QR code has expired.")
}

pub fn done() -> &'static str {
    tr("Terminé", "Done")
}

impl LogFail {
    pub fn text(self) -> &'static str {
        match self {
            LogFail::Unreachable => tr("Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez.", "The relay isn't answering: check the internet connection, then try again."),
            LogFail::OldRelay => tr("Ce relais ne prend pas encore les journaux : il doit être mis à jour.", "This relay doesn't take logs yet: it needs updating."),
            LogFail::TooMany => tr("Trop d'envois : réessayez dans une heure.", "Too many uploads: try again in an hour."),
        }
    }

    /// Le relais injoignable se réessaie tout de suite ; les autres refus, non.
    pub fn retry(self) -> bool {
        self == LogFail::Unreachable
    }
}

impl LogStep {
    /// Le second bouton, à côté de « Terminé » : « Nouveau QR code », « Réessayer ».
    pub fn again(&self) -> Option<&'static str> {
        match self {
            LogStep::Expired => Some(tr("Nouveau QR code", "New QR code")),
            LogStep::Failed(f) if f.retry() => Some(tr("Réessayer", "Try again")),
            _ => None,
        }
    }
}

// MARK: - L'appareil

/// L'appareil, pour la ligne « Appareil : » de l'en-tête du journal (fait par le cœur, `journal::header`).
pub fn device(model: &str, webos: &str) -> String {
    if webos.is_empty() { format!("LG {model} · webOS") } else { format!("LG {model} · webOS {webos}") }
}

// MARK: - L'écran

impl App {
    /// « Envoyer le journal » (Réglages de l'app › À propos), ou « Nouveau QR code », « Réessayer » : un nouvel envoi.
    pub fn log_send(&mut self) -> Vec<Action> {
        self.log_attempt += 1;
        self.log = Some(LogSendUi { step: LogStep::Preparing, attempt: self.log_attempt, checked: None });
        if self.screen != Screen::LogSend {
            self.back_to = self.screen;
        }
        self.go(Screen::LogSend);
        // Le relais du compte KyberNow du téléviseur, sinon le relais public (vide).
        let relay = self.store.accounts.first().map(|a| a.relay.clone()).unwrap_or_default();
        vec![Action::LogSend { attempt: self.log_attempt, relay, device: device(&self.tv.model, &self.tv.webos) }]
    }

    /// Le journal est prêt et part au relais.
    pub fn log_uploading(&mut self, attempt: u32) {
        if let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt && l.step == LogStep::Preparing) {
            l.step = LogStep::Uploading;
        }
    }

    /// Le dépôt est fait, ou refusé.
    pub fn log_sent(&mut self, attempt: u32, r: Result<LogDeposit, LogFail>) -> Vec<Action> {
        let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt) else {
            // L'écran est fermé, ou un autre envoi a commencé : le dépôt ne sert plus.
            return r.map(|d| vec![Action::LogDiscard { relay: d.relay, id: d.id, owner: d.owner }]).unwrap_or_default();
        };
        l.step = match r {
            Ok(d) => LogStep::Qr(d),
            Err(f) => LogStep::Failed(f),
        };
        l.checked = Some(Instant::now());
        self.focus = self.log_focus();
        Vec::new()
    }

    /// L'état du dépôt, relu toutes les 2 s tant que le QR code est montré.
    pub fn log_state(&mut self, attempt: u32, remote: LogRemote) {
        let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt) else { return };
        if !matches!(l.step, LogStep::Qr(_)) {
            return;
        }
        match remote {
            LogRemote::Opened => l.step = LogStep::Opened,
            LogRemote::Gone => l.step = LogStep::Expired,
            LogRemote::Waiting | LogRemote::Unknown => {}
        }
        self.focus = self.log_focus();
    }

    /// À appeler à chaque tour de la boucle : le dépôt à relire (toutes les 2 s), et l'expiration vue sans le relais.
    pub fn log_tick(&mut self, now: Instant) -> Vec<Action> {
        if self.screen != Screen::LogSend {
            return Vec::new();
        }
        let Some(l) = self.log.as_mut() else { return Vec::new() };
        let LogStep::Qr(d) = &l.step else { return Vec::new() };
        if now >= d.until {
            l.step = LogStep::Expired;
            self.focus = self.log_focus();
            return Vec::new();
        }
        if l.checked.is_some_and(|t| now.duration_since(t) < CHECK_EVERY) {
            return Vec::new();
        }
        l.checked = Some(now);
        vec![Action::LogCheck { attempt: l.attempt, relay: d.relay.clone(), id: d.id.clone(), owner: d.owner.clone() }]
    }

    /// « Terminé », ou Retour : le dépôt pas encore ouvert est retiré, l'écran d'avant revient.
    pub fn log_done(&mut self) -> Vec<Action> {
        let acts = match self.log.take().map(|l| l.step) {
            Some(LogStep::Qr(d)) => vec![Action::LogDiscard { relay: d.relay, id: d.id, owner: d.owner }],
            _ => Vec::new(),
        };
        let back = if self.back_to == Screen::LogSend { Screen::Settings } else { self.back_to };
        self.go(back);
        acts
    }

    /// Le focus de l'écran : le second bouton quand il y en a un, sinon « Terminé ».
    pub fn log_focus(&self) -> Target {
        if self.log.as_ref().is_some_and(|l| l.step.again().is_some()) { Target::LogAgain } else { Target::LogDone }
    }

    pub fn log_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        match t {
            Target::LogDone => Some(self.log_done()),
            Target::LogAgain => Some(self.log_send()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Store;

    #[test]
    fn device_line() {
        assert_eq!(device("OLED55G26LA", "23.20"), "LG OLED55G26LA · webOS 23.20");
        assert_eq!(device("OLED55G26LA", ""), "LG OLED55G26LA · webOS");
    }

    #[test]
    fn screen_steps() {
        let mut app = App::new(Store::default());
        app.screen = Screen::Settings;
        let acts = app.log_send();
        assert!(matches!(acts.as_slice(), [Action::LogSend { attempt: 1, relay, .. }] if relay.is_empty()));
        assert_eq!(app.screen, Screen::LogSend);
        app.log_uploading(1);
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Uploading);
        // Relais d'avant : le texte, « Terminé » seul.
        app.log_sent(1, Err(LogFail::OldRelay));
        assert_eq!(app.log_focus(), Target::LogDone);
        // Injoignable : « Réessayer » relance un envoi.
        app.log_sent(1, Err(LogFail::Unreachable));
        assert_eq!(app.log_focus(), Target::LogAgain);
        let acts = app.activate(Target::LogAgain);
        assert!(matches!(acts.as_slice(), [Action::LogSend { attempt: 2, .. }]));
        // Une réponse de l'envoi d'avant est jetée (et son dépôt retiré).
        let d = |id: &str| LogDeposit { relay: String::new(), id: id.into(), owner: "o".into(), url: "u".into(), until: Instant::now() + Duration::from_secs(3600) };
        assert!(matches!(app.log_sent(1, Ok(d("vieux"))).as_slice(), [Action::LogDiscard { id, .. }] if id == "vieux"));
        app.log_sent(2, Ok(d("x")));
        assert!(matches!(app.log.as_ref().unwrap().step, LogStep::Qr(_)));
        // Relu toutes les 2 s.
        let now = Instant::now() + Duration::from_secs(3);
        assert!(matches!(app.log_tick(now).as_slice(), [Action::LogCheck { attempt: 2, .. }]));
        assert!(app.log_tick(now).is_empty());
        app.log_state(2, LogRemote::Opened);
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Opened);
        // Ouvert : « Terminé » ne retire rien et rend les Réglages.
        assert!(app.activate(Target::LogDone).is_empty());
        assert_eq!(app.screen, Screen::Settings);
        // QR code montré puis « Terminé » : le dépôt est retiré.
        app.log_send();
        app.log_sent(3, Ok(d("y")));
        assert!(matches!(app.log_done().as_slice(), [Action::LogDiscard { id, .. }] if id == "y"));
        // Expiré sans le relais, au bout de l'heure.
        app.log_send();
        app.log_sent(4, Ok(LogDeposit { until: Instant::now(), ..d("z") }));
        app.log_tick(Instant::now() + Duration::from_secs(1));
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Expired);
        assert_eq!(app.log_focus(), Target::LogAgain);
    }
}
