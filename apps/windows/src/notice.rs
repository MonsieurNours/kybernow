//! La notification de fin d'une session (docs/link-ui.md, « Notifications »
//! et « Plusieurs ordinateurs ») : quand elle vient, sans rien de Windows
//! (testé sur le Mac).

/// Ce qu'on sait d'une session qui vient de finir.
#[derive(Debug, Clone, Copy, Default)]
pub struct End {
    /// Fin demandée (« Se déconnecter », fenêtre fermée, Ctrl+Alt+Maj+D).
    pub requested: bool,
    /// L'app se ferme (« Quitter »).
    pub quitting: bool,
    /// Une fenêtre de l'app qu'on voit est au premier plan.
    pub app_in_front: bool,
    /// D'autres sessions continuent (plusieurs ordinateurs).
    pub others_continue: bool,
    /// La session avait montré son image (sinon c'est la connexion qui a
    /// échoué) ; la première session compte toujours (son échec se lit sur
    /// l'accueil, qui peut être caché).
    pub started: bool,
    /// Le message de l'échec reste dans sa fenêtre de connexion, jusqu'à
    /// « Fermer » (session suivante, ou connexion reprise en session).
    pub kept_in_window: bool,
}

/// Une notification pour une fin non demandée dont la fenêtre disparaît : si
/// personne ne regardait (autre app devant), et toujours quand d'autres
/// sessions continuent. Pas quand le message reste affiché dans sa fenêtre.
pub fn end_notification(e: &End) -> bool {
    if e.requested || e.quitting || e.kept_in_window || !e.started {
        return false;
    }
    !e.app_in_front || e.others_continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn end() -> End {
        End { started: true, ..Default::default() }
    }

    #[test]
    fn fin_non_demandee() {
        // Une seule session : seulement si personne ne regardait.
        assert!(end_notification(&end()));
        assert!(!end_notification(&End { app_in_front: true, ..end() }));
        // D'autres continuent : toujours, même app au premier plan.
        assert!(end_notification(&End { app_in_front: true, others_continue: true, ..end() }));
    }

    #[test]
    fn pas_de_notification() {
        // « Se déconnecter », « Quitter ».
        assert!(!end_notification(&End { requested: true, others_continue: true, ..end() }));
        assert!(!end_notification(&End { quitting: true, ..end() }));
        // Le message de l'échec reste dans sa fenêtre avec « Fermer » (connexion
        // d'une session suivante, ou connexion reprise en session).
        assert!(!end_notification(&End { kept_in_window: true, others_continue: true, ..end() }));
        assert!(!end_notification(&End { kept_in_window: true, ..end() }));
        // Une session suivante qui n'a jamais montré d'image : sa connexion a échoué.
        assert!(!end_notification(&End { started: false, others_continue: true, ..end() }));
    }
}
