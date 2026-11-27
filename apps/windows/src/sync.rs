//! Verrous de l'app : `locked()` à la place de `lock().unwrap()`. Une panique
//! sous un verrou (un fil refusé, un bogue) l'empoisonne ; avec `unwrap`,
//! chaque appel suivant paniquait à son tour et toute l'app cessait de
//! répondre (relevé, accueil, menu de l'icône, « Quitter »). Les états gardés
//! ici sont des données : on reprend la valeur telle qu'elle est.
//! Rien de Windows : se teste sur le Mac.

use std::sync::{Mutex, MutexGuard, PoisonError};

pub trait Locked<T> {
    /// Le verrou, même empoisonné.
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verrou_empoisonne_repris() {
        let m = std::sync::Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("panique sous le verrou (attendue par l'essai)");
        })
        .join();
        assert!(m.is_poisoned());
        *m.locked() += 1;
        assert_eq!(*m.locked(), 2);
    }
}
