//! « Stylet : Pression / Souris » dans l'interface (docs/link-ui.md, « Stylet » ; docs/stylet.md) : la ligne
//! « Stylet » du menu « Souris » de la barre, la tuile du menu, le réglage de la rubrique Commandes et le raccourci
//! Ctrl+Alt+Maj+P. Le choix est gardé par ordinateur sous la clé du cœur (`PenMode::key` : `pressure`, `mouse`) ;
//! la règle de la ligne (`PenChoice::shown`) est tenue par le pilote, qui donne aussi le choix à la session
//! (`Session::set_pen_mode`). Les textes viennent du catalogue du cœur (`pen.*`), les mêmes dans toutes les apps.
//! Sans SDL ni cœur : les captures s'en servent.

use crate::i18n::core_text;

/// Le vrai stylet (par défaut).
pub const PRESSURE: &str = "pressure";
/// Le stylet comme une souris en position absolue.
pub const MOUSE: &str = "mouse";
/// Les choix, dans l'ordre de la ligne et du réglage (`PenMode::KEYS` du cœur).
pub const MODES: [&str; 2] = [PRESSURE, MOUSE];

/// Une clé gardée : `mouse`, sinon `pressure` (vide, ou d'une version plus récente : le choix par défaut).
pub fn parse(key: &str) -> &'static str {
    if key == MOUSE { MOUSE } else { PRESSURE }
}

/// L'autre choix (le raccourci et la tuile basculent).
pub fn toggled(key: &str) -> &'static str {
    if parse(key) == MOUSE { PRESSURE } else { MOUSE }
}

/// La place du choix dans `MODES`.
pub fn index(key: &str) -> usize {
    (parse(key) == MOUSE) as usize
}

/// « Stylet ».
pub fn title() -> String {
    core_text("pen.title", &[])
}

/// « Pression » / « Souris ».
pub fn label(key: &str) -> String {
    core_text(if parse(key) == MOUSE { "pen.mouse" } else { "pen.pressure" }, &[])
}

/// Les deux choix, dans l'ordre de `MODES`.
pub fn labels() -> Vec<String> {
    MODES.iter().map(|m| label(m)).collect()
}

/// L'aide du réglage de la rubrique Commandes.
pub fn help() -> String {
    core_text("pen.help", &[])
}

/// L'avis bref du raccourci : « Stylet : souris » / « Stylet : pression ».
pub fn notice(key: &str) -> String {
    core_text(if parse(key) == MOUSE { "pen.notice_mouse" } else { "pen.notice_pressure" }, &[])
}

/// Le raccourci (Ctrl+Alt+Maj + P).
pub fn shortcut() -> String {
    crate::app::shortcut_text('P')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les clés du cœur, la bascule, et les textes de son catalogue (docs/link-ui.md, « Stylet »).
    #[test]
    fn pression_ou_souris() {
        assert_eq!(parse(""), PRESSURE, "absent d'un fichier d'avant : pression");
        assert_eq!(parse("bientot"), PRESSURE);
        assert_eq!(parse("mouse"), MOUSE);
        assert_eq!((toggled(PRESSURE), toggled(MOUSE)), (MOUSE, PRESSURE));
        assert_eq!((index(PRESSURE), index(MOUSE)), (0, 1));
        crate::i18n::force_english(false);
        assert_eq!(title(), "Stylet");
        assert_eq!(labels(), vec!["Pression".to_string(), "Souris".to_string()]);
        assert_eq!(notice(MOUSE), "Stylet\u{202f}: souris", "espace fine du catalogue du cœur");
        assert_eq!(notice(PRESSURE), "Stylet\u{202f}: pression");
        assert!(help().starts_with("Pression\u{202f}: le stylet arrive comme un vrai stylet"));
        assert_eq!(shortcut(), "Ctrl+Alt+Maj+P");
        crate::i18n::force_english(true);
        assert_eq!(title(), "Pen");
        assert_eq!(labels(), vec!["Pressure".to_string(), "Mouse".to_string()]);
        assert_eq!(notice(MOUSE), "Pen: mouse");
        assert_eq!(shortcut(), "Ctrl+Alt+Shift+P");
        crate::i18n::force_english(false);
    }
}
