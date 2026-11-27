//! Paniques et frontière C des ponts (COEUR-07 à 09, audit du 27/11/2026).
//!
//! Depuis Rust 1.81, une panique qui sort d'une fonction `extern "C"`
//! arrête le processus. Deux outils :
//!
//! - [`install_panic_hook`] : la panique (message, endroit, fil, pile) est
//!   écrite par `log::error!`, donc dans le journal de la plateforme (celui
//!   que le pont a donné à `log` : `os_log` sur Apple, logcat sur Android,
//!   fichier sur Linux et webOS), avant le crochet précédent. Le cœur le pose
//!   lui-même depuis ses points d'entrée ([`crate::session::Session::start`],
//!   la chaîne d'entrées, le moteur de son, [`guard`]) : un pont n'a rien à
//!   faire pour qu'une panique laisse une trace, même si elle arrête l'app.
//! - [`guard`] : à mettre autour du corps de chaque fonction `extern "C"`
//!   d'un pont (et des rappels que le cœur donne à du C) ; une panique y est
//!   rattrapée, écrite au journal, et la fonction rend la valeur par défaut
//!   au lieu d'arrêter l'app.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Once;

static HOOK: Once = Once::new();

/// Pose (une fois) le crochet de panique qui écrit dans le journal.
pub fn install_panic_hook() {
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "?".into());
            let place = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
            let thread = std::thread::current();
            let name = thread.name().unwrap_or("sans nom");
            let trace = std::backtrace::Backtrace::force_capture();
            log::error!("panique dans le fil « {name} » : {message} ({place})\n{trace}");
            previous(info);
        }));
    });
}

/// Le corps d'une fonction `extern "C"` : une panique est rattrapée et
/// écrite au journal (`name` : la fonction), et `fallback` est rendu.
pub fn guard<R>(name: &str, fallback: R, f: impl FnOnce() -> R) -> R {
    install_panic_hook();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => {
            log::error!("{name} : panique rattrapée à la frontière C, valeur par défaut rendue");
            fallback
        }
    }
}

/// [`guard`] avec la valeur par défaut du type (`()`, `0`, `false`,
/// pointeur nul via `Option`…).
pub fn guard_default<R: Default>(name: &str, f: impl FnOnce() -> R) -> R {
    guard(name, R::default(), f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panique_rattrapee() {
        let r = guard("kn_essai", -1, || -> i32 { panic!("é") });
        assert_eq!(r, -1);
        assert_eq!(guard("kn_essai", -1, || 7), 7);
        assert!(!guard_default("kn_essai", || -> bool { panic!("x") }));
    }

    /// Le rappel C du cœur ne laisse pas sortir une panique.
    #[test]
    fn rappel_c_protege() {
        extern "C" fn rappel(v: i32) -> i32 {
            guard("rappel", 0, || if v < 0 { panic!("négatif") } else { v * 2 })
        }
        assert_eq!(rappel(3), 6);
        assert_eq!(rappel(-1), 0);
    }
}
