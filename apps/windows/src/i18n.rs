//! Langue de l'app (docs/i18n.md) : langue d'affichage de Windows (français
//! si elle commence par `fr`, sinon anglais), ou le choix fait dans les
//! réglages de l'app (`language` de `app.json` : `auto`, `fr`, `en`).
//!
//! Textes montrés par le côté Rust (erreurs rendues à l'interface,
//! notifications, menu de l'icône près de l'horloge, liste de la barre des
//! tâches, titre de la fenêtre de session) : `src/i18n/fr.json` et `en.json`.
//! Les journaux restent en français.

use kybernow_core::i18n::{self as core_i18n, Catalog, Lang};
use windows::Win32::Globalization::{GetUserDefaultUILanguage, GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
use windows::core::PWSTR;

static APP: Catalog = Catalog::new(include_str!("i18n/fr.json"), include_str!("i18n/en.json"));

/// Texte de l'app dans `lang` ; en français, « de {nom} » s'élide devant une
/// voyelle ou un h, comme dans l'interface (`text.rs`).
pub fn tr(lang: Lang, key: &str, params: &[(&str, &str)]) -> String {
    crate::text::fill(&APP.tr(lang, key, &[]), params, lang == Lang::Fr)
}

/// Texte de l'app dans sa langue en cours.
pub fn t(key: &str, params: &[(&str, &str)]) -> String {
    tr(lang(), key, params)
}

pub fn lang() -> Lang {
    core_i18n::current()
}

/// Première langue d'affichage de l'utilisateur (`fr-FR`, `en-US`…).
pub fn system_tag() -> Option<String> {
    unsafe {
        let (mut count, mut len) = (0u32, 0u32);
        if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len).is_ok() && len > 0 {
            let mut buf = vec![0u16; len as usize];
            if GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, Some(PWSTR(buf.as_mut_ptr())), &mut len).is_ok() {
                // Liste de noms séparés par des zéros : le premier compte.
                let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                if end > 0 {
                    return Some(String::from_utf16_lossy(&buf[..end]));
                }
            }
        }
        // Repli : langue principale de l'identifiant (LANG_FRENCH = 0x0C).
        let id = GetUserDefaultUILanguage();
        (id != 0).then(|| if id & 0x3ff == 0x0c { "fr".to_string() } else { "en".to_string() })
    }
}

/// Pose la langue de l'app d'après son choix (`auto`, `fr`, `en`).
pub fn apply(choice: &str) -> Lang {
    let lang = Lang::resolve(choice, system_tag);
    core_i18n::set_current(lang);
    lang
}
