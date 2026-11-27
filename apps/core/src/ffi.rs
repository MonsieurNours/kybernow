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
/// `Option`, `JObject` nul…).
pub fn guard_default<R: Default>(name: &str, f: impl FnOnce() -> R) -> R {
    guard(name, R::default(), f)
}

/// [`guard`] pour une fonction qui rend un pointeur (chaîne C, poignée,
/// `jstring`) : nul après une panique.
pub fn guard_null<T>(name: &str, f: impl FnOnce() -> *mut T) -> *mut T {
    guard(name, std::ptr::null_mut(), f)
}

/// Le nom court d'une fonction, d'après le chemin que donne
/// `type_name_of_val` d'une fonction locale (`…::kn_session_stop::here`) :
/// pour [`ffi_guard!`](crate::ffi_guard).
#[doc(hidden)]
pub fn function_name(path: &'static str) -> &'static str {
    let p = path.strip_suffix("::here").unwrap_or(path);
    let p = p.trim_end_matches("::{{closure}}");
    p.rsplit("::").next().unwrap_or(p)
}

/// Le corps d'une fonction de pont (`extern "C"`, `extern "system"` du JNI,
/// rappel donné à du C), protégé par [`guard`] avec le nom de la fonction
/// pris tout seul :
///
/// ```ignore
/// #[unsafe(no_mangle)]
/// pub unsafe extern "C" fn kn_session_stop(s: *const KnSession) {
///     kybernow_core::ffi_guard!({ with(s, |s| s.stop()) })
/// }
///
/// #[unsafe(no_mangle)]
/// pub extern "C" fn kn_count() -> i32 {
///     kybernow_core::ffi_guard!(-1 => { compter() })
/// }
///
/// #[unsafe(no_mangle)]
/// pub extern "C" fn kn_name() -> *mut c_char {
///     kybernow_core::ffi_guard!(null => { give(nom()) })
/// }
/// ```
///
/// `{ … }` seul : la valeur par défaut du type ; `défaut => { … }` : celle-ci ;
/// `null => { … }` : un pointeur nul. Le corps devient une fermeture : un
/// `return` y rend la valeur de la fonction, comme avant.
#[macro_export]
macro_rules! ffi_guard {
    (null => $body:block) => {
        $crate::ffi::guard_null($crate::ffi_fn_name!(), || $body)
    };
    ($fallback:expr => $body:block) => {
        $crate::ffi::guard($crate::ffi_fn_name!(), $fallback, || $body)
    };
    ($body:block) => {
        $crate::ffi::guard_default($crate::ffi_fn_name!(), || $body)
    };
}

/// Le nom de la fonction où il est écrit (pour [`ffi_guard!`](crate::ffi_guard)).
#[doc(hidden)]
#[macro_export]
macro_rules! ffi_fn_name {
    () => {{
        fn here() {}
        $crate::ffi::function_name(::std::any::type_name_of_val(&here))
    }};
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

    /// La macro des ponts : le nom de la fonction, la valeur par défaut, un
    /// pointeur nul.
    #[test]
    fn macro_des_ponts() {
        extern "C" fn kn_essai_compte(v: i32) -> i32 {
            crate::ffi_guard!(-1 => {
                if v < 0 {
                    panic!("négatif");
                }
                v + 1
            })
        }
        extern "C" fn kn_essai_drapeau(v: i32) -> bool {
            crate::ffi_guard!({ if v < 0 { panic!("x") } else { true } })
        }
        extern "C" fn kn_essai_texte(v: i32) -> *mut u8 {
            crate::ffi_guard!(null => {
                if v < 0 {
                    panic!("x");
                }
                Box::into_raw(Box::new(7u8))
            })
        }
        assert_eq!((kn_essai_compte(1), kn_essai_compte(-1)), (2, -1));
        assert!(kn_essai_drapeau(1) && !kn_essai_drapeau(-1));
        assert!(kn_essai_texte(-1).is_null());
        let p = kn_essai_texte(1);
        assert_eq!(unsafe { *Box::from_raw(p) }, 7);
        fn kn_nom() -> &'static str {
            crate::ffi_fn_name!()
        }
        assert_eq!(kn_nom(), "kn_nom");
        assert_eq!(function_name("kybernow::lib::kn_session_stop::here"), "kn_session_stop");
        assert_eq!(function_name("kybernow::exports::Java_x_sessionStop::{{closure}}::here"), "Java_x_sessionStop");
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
