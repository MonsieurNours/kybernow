//! Journal de plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) :
//! le nom de l'ordinateur au début des lignes de chaque session,
//! « [PC-ATELIER] image : écran 1… ».
//!
//! Le nom vit dans une étiquette ([`Label`]) partagée par les fils de la
//! session : son fil, ceux du signal de vie et des appels à Admin, le fil des
//! entrées. Chaque fil dit pour quelle session il écrit ([`enter`]) ; les
//! macros `jinfo!`, `jwarn!` et `jerror!` du cœur ajoutent le nom. Une
//! session qui n'a pas de nom (une app à un ordinateur, qui ne l'inscrit pas
//! dans [`crate::focus`]) écrit ses lignes comme avant, sans rien devant.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

/// Le nom d'une session pour le journal (vide : rien devant les lignes).
#[derive(Default)]
pub struct Label(Mutex<String>);

impl Label {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Le nom de l'ordinateur ; vide pour ne plus rien mettre devant.
    pub fn set(&self, name: &str) {
        *self.0.lock().unwrap() = name.trim().to_string();
    }

    pub fn name(&self) -> String {
        self.0.lock().unwrap().clone()
    }

    /// « [nom] », ou rien sans nom.
    pub fn prefix(&self) -> String {
        let name = self.0.lock().unwrap();
        if name.is_empty() { String::new() } else { format!("[{name}] ") }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Label>>> = const { RefCell::new(None) };
}

/// Ce fil écrit désormais au nom de cette session.
pub fn enter(label: Arc<Label>) {
    CURRENT.with(|c| *c.borrow_mut() = Some(label));
}

/// L'étiquette de la session pour laquelle ce fil écrit.
pub fn current() -> Option<Arc<Label>> {
    CURRENT.with(|c| c.borrow().clone())
}

/// Un fil lancé par une session reprend son étiquette (`current()` pris
/// avant de le lancer).
pub fn resume(label: Option<Arc<Label>>) {
    if let Some(l) = label {
        enter(l);
    }
}

/// Ce qu'il faut mettre devant une ligne écrite depuis ce fil.
pub fn prefix() -> String {
    CURRENT.with(|c| c.borrow().as_ref().map(|l| l.prefix()).unwrap_or_default())
}

/// `log::info!` avec le nom de l'ordinateur de la session de ce fil.
macro_rules! jinfo {
    ($($arg:tt)+) => {
        ::log::info!("{}{}", $crate::journal::prefix(), format_args!($($arg)+))
    };
}

/// `log::warn!` avec le nom de l'ordinateur de la session de ce fil.
macro_rules! jwarn {
    ($($arg:tt)+) => {
        ::log::warn!("{}{}", $crate::journal::prefix(), format_args!($($arg)+))
    };
}

/// `log::error!` avec le nom de l'ordinateur de la session de ce fil.
#[allow(unused_macros)]
macro_rules! jerror {
    ($($arg:tt)+) => {
        ::log::error!("{}{}", $crate::journal::prefix(), format_args!($($arg)+))
    };
}

#[allow(unused_imports)]
pub(crate) use {jerror, jinfo, jwarn};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nom_devant_les_lignes_du_fil() {
        assert_eq!(prefix(), "", "fil sans session : rien");
        let label = Label::new();
        enter(label.clone());
        assert_eq!(prefix(), "", "session sans nom : rien, comme avant");
        label.set(" PC-ATELIER ");
        assert_eq!(prefix(), "[PC-ATELIER] ");
        let other = std::thread::spawn(prefix).join().unwrap();
        assert_eq!(other, "", "un autre fil n'écrit pas pour elle");
        let shared = label.clone();
        let inherited = std::thread::spawn(move || {
            enter(shared);
            prefix()
        })
        .join()
        .unwrap();
        assert_eq!(inherited, "[PC-ATELIER] ", "un fil de la session, si");
    }
}
