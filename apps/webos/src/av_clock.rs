//! Horodatage du son envoyé au téléviseur (plan PCM de NDL, chaîne Dolby) : d'après la durée des trames, recalé sur
//! l'horloge du chargement (WEBOS-04).
//!
//! Chaque trame suit la précédente de sa durée. Arrivée en retard (trou dans le flux), elle repart de l'horloge :
//! jamais en arrière, ce que NDL ne supporte pas. Arrivée en avance (rafale après une gigue du réseau, un chargement
//! lent), elle est jetée dès que l'avance dépasse `max_lead` : sans cela, le retard du son croîtrait sans jamais
//! revenir, une rafale après l'autre.

/// L'horodatage de la trame qui suit `last` (`None` : aucune trame encore), de durée `step`, à l'instant `now`
/// (même unité, depuis le chargement) ; `None` si elle est jetée (avance au-delà de `max_lead`).
pub fn next_pts(last: Option<i64>, now: i64, step: i64, max_lead: i64) -> Option<i64> {
    let Some(last) = last else { return Some(now) };
    let next = last + step;
    if next < now {
        return Some(now);
    }
    (next - now <= max_lead).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::next_pts;

    const STEP: i64 = 32;
    const LEAD: i64 = 100;

    #[test]
    fn regulier() {
        assert_eq!(next_pts(None, 5, STEP, LEAD), Some(5));
        assert_eq!(next_pts(Some(5), 37, STEP, LEAD), Some(37));
    }

    #[test]
    fn trou_recale_sur_l_horloge() {
        assert_eq!(next_pts(Some(0), 500, STEP, LEAD), Some(500));
    }

    #[test]
    fn rafale_bornee() {
        // Dix trames arrivées d'un coup : l'avance plafonne à `LEAD`, le reste est jeté.
        let mut last = Some(0);
        let mut kept = 0;
        for _ in 0..10 {
            if let Some(p) = next_pts(last, 0, STEP, LEAD) {
                last = Some(p);
                kept += 1;
            }
        }
        assert_eq!(kept, 3);
        assert!(last.unwrap() <= LEAD);
        // Le flux reprend son rythme : l'avance ne grandit plus.
        let mut now = 0;
        for _ in 0..100 {
            now += STEP;
            if let Some(p) = next_pts(last, now, STEP, LEAD) {
                last = Some(p);
            }
        }
        assert!(last.unwrap() - now <= LEAD);
    }
}
