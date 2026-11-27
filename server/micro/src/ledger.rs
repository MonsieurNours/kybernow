//! Le micro par défaut, partagé entre les aides (Windows) : plusieurs
//! appareils peuvent envoyer leur micro en même temps, chacun par sa propre
//! aide, et une aide peut être tuée (fin de session brutale). Le registre,
//! écrit sur le disque, garde le micro par défaut d'avant et les aides en
//! cours : c'est la dernière qui s'arrête qui le rend, et `--check` (lancé
//! par Admin) le rend aussi quand toutes les aides sont mortes.

/// Micro d'avant, et identifiants (pid) des aides en cours.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ledger {
    pub previous: Option<String>,
    pub holders: Vec<u32>,
}

#[allow(dead_code)]
impl Ledger {
    /// Format du fichier : le micro d'avant sur la première ligne (vide s'il
    /// n'y en a pas), puis un pid par ligne.
    pub fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        let previous = lines.next().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string);
        let holders = lines.filter_map(|l| l.trim().parse().ok()).collect();
        Self { previous, holders }
    }

    pub fn to_text(&self) -> String {
        let mut text = self.previous.clone().unwrap_or_default();
        for pid in &self.holders {
            text.push('\n');
            text.push_str(&pid.to_string());
        }
        text
    }

    /// Rien à garder : le fichier peut partir.
    pub fn is_empty(&self) -> bool {
        self.previous.is_none() && self.holders.is_empty()
    }

    /// L'aide `pid` commence à envoyer le micro. `current` : micro par défaut
    /// actuel. Rend vrai s'il faut poser CABLE par défaut. Le micro d'avant
    /// n'est noté que par la première aide (et jamais CABLE lui-même).
    pub fn enter(&mut self, pid: u32, current: Option<&str>, cable: &str, alive: impl Fn(u32) -> bool) -> bool {
        self.holders.retain(|p| *p != pid && alive(*p));
        if self.holders.is_empty() && current != Some(cable) {
            self.previous = current.map(str::to_string);
        }
        self.holders.push(pid);
        current != Some(cable)
    }

    /// L'aide `pid` s'arrête (`None` : seulement le ménage des aides mortes).
    /// Rend le micro à remettre quand plus aucune aide ne tourne.
    pub fn leave(&mut self, pid: Option<u32>, alive: impl Fn(u32) -> bool) -> Option<String> {
        self.holders.retain(|p| Some(*p) != pid && alive(*p));
        if self.holders.is_empty() { self.previous.take() } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_derniere_aide_rend_le_micro() {
        let mut l = Ledger::default();
        let alive = |_| true;
        assert!(l.enter(10, Some("casque"), "cable", alive));
        // La deuxième voit CABLE déjà par défaut : rien à poser, le micro d'avant reste celui du casque.
        assert!(!l.enter(11, Some("cable"), "cable", alive));
        assert_eq!(l.previous.as_deref(), Some("casque"));
        assert_eq!(l.leave(Some(10), alive), None, "l'autre parle encore");
        assert_eq!(l.leave(Some(11), alive).as_deref(), Some("casque"));
        assert!(l.is_empty());
    }

    #[test]
    fn une_aide_tuee_n_empeche_pas_de_rendre_le_micro() {
        let mut l = Ledger::parse(&Ledger { previous: Some("casque".into()), holders: vec![42] }.to_text());
        assert_eq!(l.holders, [42]);
        // `--check` d'Admin : l'aide 42 n'existe plus.
        assert_eq!(l.leave(None, |_| false).as_deref(), Some("casque"));
        // Rien à rendre si personne n'avait changé le micro.
        assert_eq!(Ledger::parse("").leave(None, |_| false), None);
    }
}
