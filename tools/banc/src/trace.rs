//! Traces réseau : l'état d'un lien au fil du temps, dans chaque sens.
//!
//! Une trace est un fichier JSON Lines : une ligne d'en-tête facultative
//! (`{"banc":"trace", "nom":…, "duree_ms":…}`), puis une ligne par palier
//! (`{"t_ms":…, "descente":{…}, "montee":{…}}`). Un palier tient jusqu'au
//! suivant. La descente va de la Station vers l'appareil (image, son), la
//! montée de l'appareil vers la Station (entrées, accusés). Un sens absent
//! d'un palier n'est pas bridé.
//!
//! Les profils intégrés ([`profil`]) donnent des conditions typiques sans
//! fichier ; `banc enregistre` produit une trace d'un vrai lien.

use std::fmt::Write as _;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// L'état d'un sens du lien.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Etat {
    /// Débit du lien en Mbit/s ; 0 : sans limite.
    pub debit_mbps: f64,
    /// Retard fixe ajouté à chaque paquet, en ms.
    pub retard_ms: f64,
    /// Retard supplémentaire tiré au hasard entre 0 et cette valeur, pour
    /// chaque paquet, en ms.
    pub gigue_ms: f64,
    /// Paquets perdus au hasard, en %.
    pub pertes_pct: f64,
    /// Attente maximale dans la file du lien (débit limité), en ms : au-delà,
    /// le paquet est jeté, comme dans un routeur.
    pub file_ms: f64,
}

impl Default for Etat {
    fn default() -> Self {
        Self { debit_mbps: 0.0, retard_ms: 0.0, gigue_ms: 0.0, pertes_pct: 0.0, file_ms: 100.0 }
    }
}

impl Etat {
    pub fn libre() -> Self {
        Self::default()
    }

    fn debit(mbps: f64) -> Self {
        Self { debit_mbps: mbps, ..Self::default() }
    }

    fn texte(&self) -> String {
        let mut s = String::new();
        if self.debit_mbps > 0.0 {
            let _ = write!(s, "{:.0} Mbit/s", self.debit_mbps);
        } else {
            s.push_str("sans limite");
        }
        if self.retard_ms > 0.0 {
            let _ = write!(s, ", +{:.0} ms", self.retard_ms);
        }
        if self.gigue_ms > 0.0 {
            let _ = write!(s, ", gigue {:.0} ms", self.gigue_ms);
        }
        if self.pertes_pct > 0.0 {
            let _ = write!(s, ", {:.1} % perdus", self.pertes_pct);
        }
        s
    }
}

/// Un palier de la trace.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Palier {
    pub t_ms: u64,
    #[serde(default)]
    pub descente: Etat,
    #[serde(default)]
    pub montee: Etat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    Descente,
    Montee,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnTete {
    #[serde(default)]
    pub banc: String,
    #[serde(default)]
    pub nom: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub duree_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Trace {
    pub nom: String,
    pub source: String,
    pub paliers: Vec<Palier>,
    pub duree: Duration,
}

impl Trace {
    pub fn lire(texte: &str) -> Result<Self, String> {
        let mut entete = EnTete::default();
        let mut paliers = Vec::new();
        for (i, ligne) in texte.lines().enumerate() {
            let ligne = ligne.trim();
            if ligne.is_empty() || ligne.starts_with('#') {
                continue;
            }
            let v: serde_json::Value =
                serde_json::from_str(ligne).map_err(|e| format!("ligne {} : {e}", i + 1))?;
            if v.get("banc").is_some() {
                entete = serde_json::from_value(v).map_err(|e| format!("en-tête : {e}"))?;
            } else {
                paliers.push(
                    serde_json::from_value::<Palier>(v).map_err(|e| format!("ligne {} : {e}", i + 1))?,
                );
            }
        }
        if paliers.is_empty() {
            return Err("trace sans palier".into());
        }
        paliers.sort_by_key(|p| p.t_ms);
        let fin = entete.duree_ms.unwrap_or(paliers.last().unwrap().t_ms + 1000);
        Ok(Self { nom: entete.nom, source: entete.source, paliers, duree: Duration::from_millis(fin) })
    }

    pub fn ecrire(&self) -> String {
        let mut s = serde_json::to_string(&EnTete {
            banc: "trace".into(),
            nom: self.nom.clone(),
            source: self.source.clone(),
            duree_ms: Some(self.duree.as_millis() as u64),
        })
        .unwrap();
        s.push('\n');
        for p in &self.paliers {
            s.push_str(&serde_json::to_string(p).unwrap());
            s.push('\n');
        }
        s
    }

    /// L'état d'un sens à l'instant `t` depuis le début ; la trace reprend
    /// au début une fois finie si `boucle`, sinon son dernier palier reste.
    pub fn etat(&self, t: Duration, sens: Sens, boucle: bool) -> Etat {
        let mut ms = t.as_millis() as u64;
        let fin = self.duree.as_millis().max(1) as u64;
        if boucle {
            ms %= fin;
        }
        let i = self.paliers.partition_point(|p| p.t_ms <= ms).saturating_sub(1);
        let p = &self.paliers[i];
        match sens {
            Sens::Descente => p.descente,
            Sens::Montee => p.montee,
        }
    }

    /// Ajoute un aller-retour de base, moitié dans chaque sens.
    pub fn ajouter_rtt(&mut self, rtt_ms: f64) {
        for p in &mut self.paliers {
            p.descente.retard_ms += rtt_ms / 2.0;
            p.montee.retard_ms += rtt_ms / 2.0;
        }
        if !self.nom.is_empty() {
            self.nom = format!("{} (+{rtt_ms:.0} ms)", self.nom);
        }
    }

    /// Les paliers, en clair, pour l'écran.
    pub fn resume(&self) -> String {
        let mut s = format!("{} — {:.0} s, {} paliers\n", self.nom, self.duree.as_secs_f64(), self.paliers.len());
        for p in self.paliers.iter().take(12) {
            let _ = writeln!(
                s,
                "  {:>7.1} s  descente : {} ; montée : {}",
                p.t_ms as f64 / 1000.0,
                p.descente.texte(),
                p.montee.texte()
            );
        }
        if self.paliers.len() > 12 {
            let _ = writeln!(s, "  … {} paliers de plus", self.paliers.len() - 12);
        }
        s
    }
}

/// Les profils intégrés : (nom, ce qu'il joue).
pub const PROFILS: &[(&str, &str)] = &[
    ("propre", "aucune contrainte (le retard de --rtt seul)"),
    ("capacite-20", "descente à 20 Mbit/s"),
    ("chute-20", "descente à 100 Mbit/s, 20 Mbit/s de 30 à 60 s, puis 100 Mbit/s"),
    ("capacite-3-1", "descente à 3 Mbit/s, 1 Mbit/s à partir de 60 s"),
    ("pertes-2", "2 % de pertes en descente, 5 % à partir de 60 s"),
    ("rafales", "toute la descente perdue 50 ms toutes les 2 s"),
    ("gigue-15", "descente retardée de 0 à 15 ms au hasard, paquet par paquet"),
    ("wifi", "Wi-Fi chargé : gigue de 4 ms, 0,2 % de pertes, pic de 60 ms de 300 ms toutes les 7 s"),
    ("4g", "réseau mobile : 15 à 40 Mbit/s changeant chaque seconde, +25 ms, gigue 8 ms, 0,5 % de pertes"),
    ("retour-coupe", "toute la montée perdue pendant 1 s à 30 s : les accusés s'arrêtent"),
];

/// Un profil intégré, ou `None` s'il n'existe pas.
pub fn profil(nom: &str) -> Option<Trace> {
    let s = |t: f64, d: Etat, m: Etat| Palier { t_ms: (t * 1000.0) as u64, descente: d, montee: m };
    let l = Etat::libre;
    let (paliers, duree): (Vec<Palier>, f64) = match nom {
        "propre" => (vec![s(0.0, l(), l())], 120.0),
        "capacite-20" => (vec![s(0.0, Etat::debit(20.0), l())], 120.0),
        "chute-20" => (
            vec![
                s(0.0, Etat::debit(100.0), l()),
                s(30.0, Etat::debit(20.0), l()),
                s(60.0, Etat::debit(100.0), l()),
            ],
            120.0,
        ),
        "capacite-3-1" => (vec![s(0.0, Etat::debit(3.0), l()), s(60.0, Etat::debit(1.0), l())], 120.0),
        "pertes-2" => (
            vec![
                s(0.0, Etat { pertes_pct: 2.0, ..l() }, l()),
                s(60.0, Etat { pertes_pct: 5.0, ..l() }, l()),
            ],
            120.0,
        ),
        "rafales" => {
            let mut v = Vec::new();
            for k in 0..60 {
                let t = k as f64 * 2.0;
                v.push(s(t, l(), l()));
                v.push(s(t + 1.95, Etat { pertes_pct: 100.0, ..l() }, l()));
            }
            (v, 120.0)
        }
        "gigue-15" => (vec![s(0.0, Etat { gigue_ms: 15.0, ..l() }, l())], 120.0),
        "wifi" => {
            let base = Etat { retard_ms: 2.0, gigue_ms: 4.0, pertes_pct: 0.2, ..l() };
            let pic = Etat { retard_ms: 60.0, ..base };
            let mut v = Vec::new();
            for k in 0..18 {
                let t = k as f64 * 7.0;
                v.push(s(t, base, Etat { retard_ms: 2.0, gigue_ms: 4.0, ..l() }));
                v.push(s(t + 3.0, pic, Etat { retard_ms: 30.0, ..l() }));
                v.push(s(t + 3.3, base, Etat { retard_ms: 2.0, gigue_ms: 4.0, ..l() }));
            }
            (v, 126.0)
        }
        "4g" => {
            // Marche au hasard, toujours la même (graine fixe) : deux versions
            // jouent exactement la même trace.
            let mut r = Hasard::new(0x4b59_4245);
            let mut debit = 30.0f64;
            let v = (0..120)
                .map(|k| {
                    debit = (debit + (r.unif() - 0.5) * 12.0).clamp(15.0, 40.0);
                    s(
                        k as f64,
                        Etat { debit_mbps: debit.round(), retard_ms: 12.5, gigue_ms: 8.0, pertes_pct: 0.5, file_ms: 200.0 },
                        Etat { retard_ms: 12.5, gigue_ms: 4.0, ..l() },
                    )
                })
                .collect();
            (v, 120.0)
        }
        "retour-coupe" => (
            vec![s(0.0, l(), l()), s(30.0, l(), Etat { pertes_pct: 100.0, ..l() }), s(31.0, l(), l())],
            120.0,
        ),
        _ => return None,
    };
    Some(Trace {
        nom: nom.into(),
        source: "profil intégré".into(),
        paliers,
        duree: Duration::from_secs_f64(duree),
    })
}

/// Un profil intégré ou un fichier de trace.
pub fn charger(nom: &str) -> Result<Trace, String> {
    if let Some(t) = profil(nom) {
        return Ok(t);
    }
    let texte = std::fs::read_to_string(nom)
        .map_err(|e| format!("« {nom} » : ni un profil (banc profils), ni un fichier lisible ({e})"))?;
    let mut t = Trace::lire(&texte)?;
    if t.nom.is_empty() {
        t.nom = std::path::Path::new(nom).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    }
    Ok(t)
}

/// Petit générateur pseudo-aléatoire (xorshift), sans dépendance.
#[derive(Debug, Clone)]
pub struct Hasard(u64);

impl Hasard {
    pub fn new(graine: u64) -> Self {
        Self(graine.max(1))
    }

    pub fn suivant(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniforme dans [0, 1).
    pub fn unif(&mut self) -> f64 {
        (self.suivant() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tous_les_profils_existent() {
        for (nom, _) in PROFILS {
            let t = profil(nom).unwrap();
            assert!(!t.paliers.is_empty(), "{nom}");
        }
    }

    #[test]
    fn aller_retour_fichier() {
        let t = profil("chute-20").unwrap();
        let u = Trace::lire(&t.ecrire()).unwrap();
        assert_eq!(u.paliers, t.paliers);
        assert_eq!(u.duree, t.duree);
    }

    #[test]
    fn palier_en_cours_et_boucle() {
        let t = profil("chute-20").unwrap();
        let d = |s: u64, b| t.etat(Duration::from_secs(s), Sens::Descente, b).debit_mbps;
        assert_eq!(d(10, false), 100.0);
        assert_eq!(d(45, false), 20.0);
        assert_eq!(d(500, false), 100.0);
        assert_eq!(d(165, true), 20.0);
    }

    #[test]
    fn rafales_de_50_ms() {
        let t = profil("rafales").unwrap();
        let p = |ms: u64| t.etat(Duration::from_millis(ms), Sens::Descente, false).pertes_pct;
        assert_eq!(p(1940), 0.0);
        assert_eq!(p(1960), 100.0);
        assert_eq!(p(2010), 0.0);
    }
}
