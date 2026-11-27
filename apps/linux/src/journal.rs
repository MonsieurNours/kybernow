//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos ») : les étapes à l'écran et leurs textes.
//! Au bureau, le journal part dans un e-mail (sinon il s'enregistre) ; sur le Steam Deck en mode Jeu, au salon et sur
//! KyberNow Link OS, il passe par un QR code : chiffré sur l'appareil, déposé sur le relais (docs/relais.md, « Dépôt
//! d'un journal »), ouvert par le téléphone. Sans SDL ni cœur : le pilote (`device::journal`) prépare, chiffre et
//! dépose ; les captures et les essais se servent d'ici.

use crate::i18n::tr;

/// Le nom du fichier, partout.
pub const FILE_NAME: &str = "kybernow-journal.txt";

/// Où en est l'envoi par QR code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalStep {
    /// « Préparation du journal… »
    Preparing,
    /// « Envoi du journal… »
    Uploading,
    /// Le QR code (l'adresse du dépôt, la clé après `#k=`).
    Qr { url: String },
    /// « Journal ouvert sur le téléphone. »
    Opened,
    /// Une heure passée : « Ce QR code a expiré. »
    Expired,
    /// Relais injoignable, relais d'avant (sans dépôt), trop d'envois.
    RelayDown,
    OldRelay,
    TooMany,
}

impl JournalStep {
    /// Le texte de l'étape (sous le titre « Envoyer le journal »).
    pub fn text(&self) -> &'static str {
        match self {
            JournalStep::Preparing => preparing(),
            JournalStep::Uploading => tr("Envoi du journal…", "Uploading the log…"),
            JournalStep::Qr { .. } => tr("Scannez ce QR code avec votre téléphone, puis enregistrez ou partagez le journal.", "Scan this QR code with your phone, then save or share the log."),
            JournalStep::Opened => tr("Journal ouvert sur le téléphone.", "Log opened on the phone."),
            JournalStep::Expired => tr("Ce QR code a expiré.", "This QR code has expired."),
            JournalStep::RelayDown => tr("Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez.", "The relay isn't answering: check the internet connection, then try again."),
            JournalStep::OldRelay => tr("Ce relais ne prend pas encore les journaux : il doit être mis à jour.", "This relay doesn't take logs yet: it needs updating."),
            JournalStep::TooMany => tr("Trop d'envois : réessayez dans une heure.", "Too many uploads: try again in an hour."),
        }
    }

    /// Le bouton de l'étape : « Nouveau QR code » une fois expiré, « Réessayer » si le relais ne répond pas, sinon
    /// « Terminé » (qui retire un dépôt pas encore ouvert).
    pub fn button(&self) -> &'static str {
        match self {
            JournalStep::Expired => tr("Nouveau QR code", "New QR code"),
            JournalStep::RelayDown => tr("Réessayer", "Try again"),
            _ => done(),
        }
    }

    /// Le bouton relance l'envoi (sinon il ferme).
    pub fn retries(&self) -> bool {
        matches!(self, JournalStep::Expired | JournalStep::RelayDown)
    }

    /// Une attente (le cercle tourne).
    pub fn busy(&self) -> bool {
        matches!(self, JournalStep::Preparing | JournalStep::Uploading)
    }
}

/// Ce que l'envoi au bureau a fait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JournalDone {
    /// La fenêtre d'un e-mail s'est ouverte, le fichier joint.
    Email,
    /// Enregistré à cet endroit (pas d'app d'e-mail).
    Saved(String),
    /// Ni l'un ni l'autre : la raison.
    Failed(String),
}

impl JournalDone {
    /// L'avis bref après l'envoi au bureau.
    pub fn notice(&self) -> Option<String> {
        match self {
            JournalDone::Email => None,
            JournalDone::Saved(path) => Some(format!("{} {path}", tr("Journal enregistré :", "Log saved:"))),
            JournalDone::Failed(e) => Some(format!("{} ({e})", tr("Le journal n'a pas pu être préparé", "The log couldn't be prepared"))),
        }
    }
}

/// Le titre, le bouton des Réglages de l'app.
pub fn title() -> &'static str {
    tr("Envoyer le journal", "Send the log")
}

/// « Préparation du journal… », à la place du bouton pendant la préparation.
pub fn preparing() -> &'static str {
    tr("Préparation du journal…", "Preparing the log…")
}

/// L'aide du bouton : au bureau (e-mail), ou par le téléphone (QR code).
pub fn help(qr: bool) -> &'static str {
    if qr {
        tr(
            "Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Il passe par votre téléphone : vous choisissez à qui l'envoyer.",
            "If the app closes or a session cuts out, this text file tells what happened. It goes through your phone: you choose who to send it to.",
        )
    } else {
        tr(
            "Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Joignez-le à votre message : vous choisissez à qui l'envoyer.",
            "If the app closes or a session cuts out, this text file tells what happened. Attach it to your message: you choose who to send it to.",
        )
    }
}

/// Sous le QR code, en petit.
pub fn encrypted_note() -> &'static str {
    tr("Chiffré sur cet appareil : seul ce QR code l'ouvre. Il sert une fois, pendant une heure.", "Encrypted on this device: only this QR code opens it. It works once, for one hour.")
}

pub fn done() -> &'static str {
    tr("Terminé", "Done")
}

/// Le sujet de l'e-mail.
pub fn subject(version: &str) -> String {
    format!("{} {version}", tr("Journal de KyberNow Link", "KyberNow Link log"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textes_de_link_ui() {
        crate::i18n::force_english(false);
        assert_eq!(JournalStep::Qr { url: String::new() }.button(), "Terminé");
        assert_eq!((JournalStep::Expired.button(), JournalStep::Expired.retries()), ("Nouveau QR code", true));
        assert_eq!(JournalStep::RelayDown.button(), "Réessayer");
        assert!(JournalStep::Uploading.busy() && !JournalStep::Opened.busy());
        crate::i18n::force_english(true);
        assert_eq!(JournalStep::OldRelay.text(), "This relay doesn't take logs yet: it needs updating.");
        assert_eq!(title(), "Send the log");
        crate::i18n::force_english(false);
    }
}
