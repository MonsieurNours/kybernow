//! Fenêtre de mise à jour : ce que la Station installée sait d'elle-même.
//!
//! La recherche, le téléchargement, la vérification de la signature et
//! l'installation passent par Admin, comme sous Windows. En attendant que
//! l'applicateur d'Admin sache remplacer l'app du Mac, la fenêtre dit la
//! version installée et renvoie vers la page Système d'Admin.

use super::commands::Event;

pub fn info() -> Event {
    let current = super::read_record().map(|r| r.version).unwrap_or_else(|| crate::VERSION.to_string());
    Event::UpdateInfo { current, version: None, notes: Vec::new(), page: None, session: None, refusal: None }
}
