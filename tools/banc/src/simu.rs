//! Rejeu du débit automatique sans réseau : une trace, un modèle simple du
//! lien (une file d'attente devant un débit, des pertes, un retard) et le
//! vrai calcul des apps (`apps/core/src/adaptive.rs`), seconde par seconde.
//! Deux minutes de trace se rejouent en quelques millisecondes : de quoi
//! régler les seuils avant d'essayer sur la bride.
//!
//! Le modèle avance par pas de 10 ms : le flux demandé entre dans la file,
//! le lien en sort à son débit, la file au-delà de sa limite est jetée, puis
//! les pertes au hasard. L'aller-retour d'une seconde est pris à un instant
//! de la seconde qui change d'une seconde à l'autre (Kyber en donne un
//! toutes les quelques secondes, sans caler sur la trace) ; un trou d'image est
//! compté quand un pas perd plus que la correction d'erreurs ne rattrape.

use serde::Serialize;
use std::time::Duration;

use crate::trace::{Hasard, Sens, Trace};

/// Ce que la seconde simulée donne au calcul (les champs de `Second` du
/// cœur qu'il lit).
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Mesure {
    pub rtt_ms: Option<f64>,
    pub lost_packets: u32,
    pub bytes: u64,
    pub holes: u32,
}

/// Le calcul rejoué : rend le débit à demander pour la seconde suivante.
pub trait Controle {
    fn seconde(&mut self, m: &Mesure, besoin: u32) -> u32;
}

/// Une seconde du rejeu, pour le rapport.
#[derive(Debug, Clone, Serialize)]
pub struct Ligne {
    pub t_s: u32,
    /// Débit du lien (0 : sans limite), Mbit/s.
    pub capacite_mbps: f64,
    /// Débit demandé à la Station pendant cette seconde.
    pub demande_mbps: f64,
    /// Débit arrivé à l'appareil.
    pub recu_mbps: f64,
    pub rtt_ms: f64,
    pub file_ms: f64,
    pub perdus: u32,
    pub trous: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bilan {
    pub trace: String,
    pub besoin_mbps: f64,
    pub secondes: u32,
    /// Moyenne du débit reçu, Mbit/s.
    pub recu_moyen_mbps: f64,
    /// Part du débit utile disponible (capacité, ou besoin si sans limite)
    /// réellement utilisée, en %.
    pub utilisation_pct: f64,
    /// Secondes où la file a dépassé 15 ms (image en retard).
    pub secondes_file: u32,
    /// Secondes avec au moins un trou d'image.
    pub secondes_trous: u32,
    pub paquets_perdus: u64,
    pub changements: u32,
    pub rtt_p50_ms: f64,
    pub rtt_p95_ms: f64,
    pub lignes: Vec<Ligne>,
}

const PAS: f64 = 0.010;
const PAQUET: f64 = 1200.0 * 8.0;
/// Part des paquets d'un pas au-delà de laquelle la correction d'erreurs
/// (30 % du débit dans les profils) ne rattrape plus l'image.
const TROU: f64 = 0.10;

pub fn rejouer(trace: &Trace, besoin: u32, c: &mut dyn Controle) -> Bilan {
    let secondes = trace.duree.as_secs().max(1) as u32;
    let mut demande = besoin;
    let mut file_bits = 0.0f64;
    let mut perte_reste = 0.0f64;
    let mut lignes = Vec::new();
    let mut changements = 0;
    // Graine fixe : deux versions voient la même gigue.
    let mut hasard = Hasard::new(0x7274_74);
    for s in 0..secondes {
        let (mut recu, mut perdus_bits, mut trous) = (0.0f64, 0.0f64, 0u32);
        let (mut capacite, mut rtt, mut file_ms_max) = (0.0, 0.0, 0.0f64);
        for k in 0..100 {
            let t = Duration::from_secs_f64(f64::from(s) + f64::from(k) * PAS);
            let d = trace.etat(t, Sens::Descente, false);
            let m = trace.etat(t, Sens::Montee, false);
            let entre = f64::from(demande) * PAS;
            let (sort, jete) = if d.debit_mbps > 0.0 {
                let cap = d.debit_mbps * 1e6;
                file_bits += entre;
                let sort = file_bits.min(cap * PAS);
                file_bits -= sort;
                let limite = cap * d.file_ms / 1000.0;
                let jete = (file_bits - limite).max(0.0);
                file_bits -= jete;
                (sort, jete)
            } else {
                file_bits = 0.0;
                (entre, 0.0)
            };
            let au_hasard = sort * d.pertes_pct / 100.0;
            let arrive = sort - au_hasard;
            recu += arrive;
            let perdu_pas = jete + au_hasard;
            perdus_bits += perdu_pas;
            if entre > 0.0 && perdu_pas / entre > TROU {
                trous += 1;
            }
            let file_ms = if d.debit_mbps > 0.0 { file_bits / (d.debit_mbps * 1e6) * 1000.0 } else { 0.0 };
            file_ms_max = file_ms_max.max(file_ms);
            capacite = d.debit_mbps;
            if k != (s * 37 + 11) % 100 {
                continue;
            }
            rtt = d.retard_ms + hasard.unif() * d.gigue_ms + m.retard_ms + hasard.unif() * m.gigue_ms + file_ms;
        }
        perte_reste += perdus_bits / PAQUET;
        let perdus = perte_reste.floor();
        perte_reste -= perdus;
        let mesure = Mesure {
            rtt_ms: Some(rtt.max(0.1)),
            lost_packets: perdus as u32,
            bytes: (recu / 8.0) as u64,
            holes: trous,
        };
        lignes.push(Ligne {
            t_s: s,
            capacite_mbps: capacite,
            demande_mbps: f64::from(demande) / 1e6,
            recu_mbps: recu / 1e6,
            rtt_ms: (rtt * 10.0).round() / 10.0,
            file_ms: (file_ms_max * 10.0).round() / 10.0,
            perdus: perdus as u32,
            trous,
        });
        let nouveau = c.seconde(&mesure, besoin);
        if nouveau != demande {
            changements += 1;
        }
        demande = nouveau;
    }
    let n = lignes.len() as f64;
    let recu_moyen = lignes.iter().map(|l| l.recu_mbps).sum::<f64>() / n;
    let utile = lignes
        .iter()
        .map(|l| {
            let b = f64::from(besoin) / 1e6;
            if l.capacite_mbps > 0.0 { l.capacite_mbps.min(b) } else { b }
        })
        .sum::<f64>()
        / n;
    let mut rtts: Vec<f64> = lignes.iter().map(|l| l.rtt_ms).collect();
    rtts.sort_by(|a, b| a.total_cmp(b));
    let c = |p: f64| rtts[((rtts.len() - 1) as f64 * p).round() as usize];
    Bilan {
        trace: trace.nom.clone(),
        besoin_mbps: f64::from(besoin) / 1e6,
        secondes,
        recu_moyen_mbps: (recu_moyen * 100.0).round() / 100.0,
        utilisation_pct: (recu_moyen / utile * 1000.0).round() / 10.0,
        secondes_file: lignes.iter().filter(|l| l.file_ms > 15.0).count() as u32,
        secondes_trous: lignes.iter().filter(|l| l.trous > 0).count() as u32,
        paquets_perdus: lignes.iter().map(|l| u64::from(l.perdus)).sum(),
        changements,
        rtt_p50_ms: c(0.5),
        rtt_p95_ms: c(0.95),
        lignes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::profil;

    /// Sans calcul (débit fixe) : sur une chute à 20 Mbit/s, la file sature
    /// et jette ; sur un lien propre, rien ne se perd.
    struct Fixe;
    impl Controle for Fixe {
        fn seconde(&mut self, _: &Mesure, besoin: u32) -> u32 {
            besoin
        }
    }

    #[test]
    fn debit_fixe_sur_chute() {
        let b = rejouer(&profil("chute-20").unwrap(), 60_000_000, &mut Fixe);
        assert_eq!(b.secondes, 120);
        assert!(b.secondes_file >= 29, "{}", b.secondes_file);
        assert!(b.paquets_perdus > 100_000);
        let l = &b.lignes[45];
        assert!((l.recu_mbps - 20.0).abs() < 0.5, "{}", l.recu_mbps);
    }

    #[test]
    fn lien_propre_sans_perte() {
        let b = rejouer(&profil("propre").unwrap(), 60_000_000, &mut Fixe);
        assert_eq!(b.paquets_perdus, 0);
        assert_eq!(b.utilisation_pct, 100.0);
    }
}
