//! L'accueil des apps Link et l'incitation au compte KyberNow pour les apps
//! Apple (docs/link-ui.md, « Accueil et compte KyberNow » ; règles :
//! `kybernow_core::welcome`).
//!
//! L'app garde l'état (`WelcomeState`) en JSON dans ses préférences et le
//! passe à chaque appel ; un JSON vide ou illisible vaut l'état d'une app
//! neuve (accueil à montrer). Les appels qui changent l'état le rendent dans
//! `out` (à libérer par `kn_string_free`), à garder à la place de l'ancien.
//! `now` : secondes depuis 1970.

use std::ffi::c_char;

use kybernow_core::i18n;
use kybernow_core::welcome::{self, WelcomeState};

use crate::{give, give_to, text};

fn state(json: *const c_char) -> WelcomeState {
    serde_json::from_str(&text(json)).unwrap_or_default()
}

fn keep(s: &WelcomeState) -> *mut c_char {
    give(serde_json::to_string(s).unwrap_or_default())
}

/// Montrer l'accueil : jamais vu dans cette version, et sans compte.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_should_show(state_json: *const c_char, has_account: bool) -> bool {
    state(state_json).should_show(has_account)
}

/// L'accueil est vu (fini, « Passer », « Plus tard ») : le nouvel état.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_done(state_json: *const c_char) -> *mut c_char {
    let mut s = state(state_json);
    s.welcome_done();
    keep(&s)
}

/// La carte du compte à l'accueil (sans compte ; revenue 30 jours après une
/// première fermeture ; jamais après « Ne plus afficher »).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_card_visible(state_json: *const c_char, has_account: bool, now: u64) -> bool {
    state(state_json).card_visible(has_account, now)
}

/// La croix de la carte dit « Ne plus afficher » (sinon « Fermer »).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_card_close_is_final(state_json: *const c_char) -> bool {
    state(state_json).card_close_is_final()
}

/// La croix de la carte : le nouvel état.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_close_card(state_json: *const c_char, now: u64) -> *mut c_char {
    let mut s = state(state_json);
    s.close_card(now);
    keep(&s)
}

/// Le bandeau après l'appairage de `computer` (un identifiant stable de
/// l'ordinateur) : vrai une fois par ordinateur, sans compte. `out` : le
/// nouvel état (toujours rendu).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_take_paired_hint(state_json: *const c_char, has_account: bool, computer: *const c_char, out: *mut *mut c_char) -> bool {
    let mut s = state(state_json);
    let take = s.take_paired_hint(has_account, &text(computer));
    give_to(out, serde_json::to_string(&s).unwrap_or_default());
    take
}

/// La fenêtre « Hors de chez vous ? » pour `computer` : vrai une fois par
/// jour au plus, sans compte. `out` : le nouvel état (toujours rendu).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_take_away_hint(state_json: *const c_char, has_account: bool, computer: *const c_char, now: u64, out: *mut *mut c_char) -> bool {
    let mut s = state(state_json);
    let take = s.take_away_hint(has_account, &text(computer), now);
    give_to(out, serde_json::to_string(&s).unwrap_or_default());
    take
}

/// La page d'inscription du relais (vide : le relais public), dans la langue
/// de l'app, à ouvrir dans le navigateur de l'appareil.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_welcome_sign_up_url(relay: *const c_char) -> *mut c_char {
    give(welcome::sign_up_url(&text(relay), i18n::current()))
}

/// La page de téléchargement de KyberNow Station (écran 3 de l'accueil).
#[unsafe(no_mangle)]
pub extern "C" fn kn_welcome_station_download_url() -> *mut c_char {
    give(welcome::STATION_DOWNLOAD_URL.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kn_string_free;
    use std::ffi::{CStr, CString};

    fn take(p: *mut c_char) -> String {
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { kn_string_free(p) };
        s
    }

    #[test]
    fn accueil_carte_et_moments_par_le_pont() {
        let empty = CString::new("").unwrap();
        assert!(unsafe { kn_welcome_should_show(empty.as_ptr(), false) }, "app neuve");
        assert!(!unsafe { kn_welcome_should_show(empty.as_ptr(), true) }, "avec un compte");
        let done = CString::new(take(unsafe { kn_welcome_done(empty.as_ptr()) })).unwrap();
        assert!(!unsafe { kn_welcome_should_show(done.as_ptr(), false) }, "déjà vu");

        let t = 1_800_000_000;
        assert!(unsafe { kn_welcome_card_visible(done.as_ptr(), false, t) });
        assert!(!unsafe { kn_welcome_card_close_is_final(done.as_ptr()) });
        let closed = CString::new(take(unsafe { kn_welcome_close_card(done.as_ptr(), t) })).unwrap();
        assert!(!unsafe { kn_welcome_card_visible(closed.as_ptr(), false, t + 3600) });
        assert!(unsafe { kn_welcome_card_close_is_final(closed.as_ptr()) });

        let pc = CString::new("pc1").unwrap();
        let mut out: *mut c_char = std::ptr::null_mut();
        assert!(unsafe { kn_welcome_take_paired_hint(closed.as_ptr(), false, pc.as_ptr(), &mut out) });
        let after = CString::new(take(out)).unwrap();
        assert!(!unsafe { kn_welcome_take_paired_hint(after.as_ptr(), false, pc.as_ptr(), &mut out) }, "une fois");
        let after = CString::new(take(out)).unwrap();
        assert!(unsafe { kn_welcome_take_away_hint(after.as_ptr(), false, pc.as_ptr(), t, &mut out) });
        let after = CString::new(take(out)).unwrap();
        assert!(!unsafe { kn_welcome_take_away_hint(after.as_ptr(), false, pc.as_ptr(), t + 60, &mut out) }, "une fois par jour");
        take(out);

        let url = take(unsafe { kn_welcome_sign_up_url(empty.as_ptr()) });
        assert!(url.starts_with("https://share.kybernow.app/compte/inscription?lang="), "{url}");
        assert_eq!(take(kn_welcome_station_download_url()), "https://www.kybernow.app/#telecharger");
    }
}
