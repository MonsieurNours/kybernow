//! Les objets rendus à Kotlin, par numéro (AND-10, AND-31) : une session,
//! une partie PlayStation, une console jointe par une Station, l'attente
//! d'un envoi. Kotlin garde un numéro, plus un pointeur : un appel qui arrive
//! après la libération (fil de la caméra ou du micro, rappel du réseau, appel
//! bloquant encore en cours) ne trouve plus rien au lieu de lire une mémoire
//! libérée ; un appel en cours garde l'objet vivant jusqu'à son retour. Les
//! numéros ne reviennent jamais (0 : aucun).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Registry<T> {
    map: OnceLock<Mutex<HashMap<i64, Arc<T>>>>,
    next: AtomicI64,
}

impl<T> Registry<T> {
    pub const fn new() -> Self {
        Self { map: OnceLock::new(), next: AtomicI64::new(1) }
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<T>>> {
        self.map.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Range l'objet ; son numéro (jamais 0).
    pub fn insert(&self, v: T) -> i64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.map().insert(id, Arc::new(v));
        id
    }

    pub fn get(&self, id: i64) -> Option<Arc<T>> {
        if id == 0 {
            return None;
        }
        self.map().get(&id).cloned()
    }

    /// Retire l'objet : il part quand le dernier appel en cours le rend.
    pub fn remove(&self, id: i64) -> Option<Arc<T>> {
        if id == 0 {
            return None;
        }
        self.map().remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static R: Registry<String> = Registry::new();

    #[test]
    fn freed_ids_find_nothing() {
        let a = R.insert("a".into());
        let b = R.insert("b".into());
        assert_ne!(a, 0);
        assert_ne!(a, b);
        assert_eq!(R.get(a).as_deref().map(String::as_str), Some("a"));
        let held = R.get(b).unwrap();
        assert!(R.remove(b).is_some());
        assert!(R.get(b).is_none());
        assert!(R.remove(b).is_none());
        // L'appel en cours garde l'objet.
        assert_eq!(held.as_str(), "b");
        assert!(R.get(0).is_none() && R.remove(0).is_none());
        // Un numéro ne revient pas.
        assert!(R.insert("c".into()) > b);
    }
}
