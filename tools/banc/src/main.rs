//! `banc` : la partie réseau du banc de mesure (docs/banc-de-mesure.md).
//!
//!   banc profils
//!   banc trace <profil|fichier> [--rtt MS]
//!   banc bride --vers STATION --udp PORTS [--tcp PORTS] [--profil P] [--rtt MS] …
//!   banc echo [--ecoute ADRESSE:PORT]
//!   banc enregistre --echo HÔTE:PORT [--debit MBPS] [--duree S] [--sortie F]
//!   banc rejeu-debit <profil|fichier>… [--besoin MBPS] [--rtt MS] [--json F]

use std::process::exit;
use std::time::Duration;

use banc::simu::{self, Controle, Mesure};
use banc::trace::{self, Trace};

// Le calcul du débit automatique des apps, repris tel quel : le rejeu juge
// toujours le code de la branche en cours. Il ne lit de la seconde que
// l'aller-retour, les pertes, les octets, les trous et le canal de retour
// (absent ici) ; les petits modules
// ci-dessous tiennent la place de ceux du cœur.
#[path = "../../../apps/core/src/adaptive.rs"]
#[allow(dead_code)]
mod adaptive;

#[allow(dead_code)]
mod latency {
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct Second {
        pub rtt_ms: Option<f64>,
        pub lost_packets: u32,
        pub bytes: u64,
        pub holes: u32,
        /// Canal de retour de la Station : le banc n'en simule pas (pertes
        /// estimées, comme face à une Station sans lui).
        pub feedback: Option<Feedback>,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq)]
    pub struct Feedback {
        pub datagrams: u64,
        pub lost: u64,
        pub lost_frames: u64,
        pub repair_ratio: Option<f64>,
        pub wait_ms: f64,
    }
}

#[allow(dead_code)]
mod i18n {
    #[derive(Debug, Clone, Copy)]
    pub enum Lang {
        Fr,
        En,
    }
    pub fn tr(_: Lang, cle: &str, _: &[(&str, &str)]) -> String {
        cle.to_string()
    }
}

struct DebitAutomatique(adaptive::AdaptiveBitrate);

impl Controle for DebitAutomatique {
    fn seconde(&mut self, m: &Mesure, besoin: u32) -> u32 {
        let s = latency::Second { rtt_ms: m.rtt_ms, lost_packets: m.lost_packets, bytes: m.bytes, holes: m.holes, feedback: None };
        self.0.second(&s, besoin);
        self.0.bitrate(besoin)
    }
}

struct Args(Vec<String>);

impl Args {
    fn option(&mut self, nom: &str) -> Option<String> {
        let i = self.0.iter().position(|a| a == nom)?;
        if i + 1 >= self.0.len() {
            erreur(&format!("{nom} attend une valeur"));
        }
        let v = self.0.remove(i + 1);
        self.0.remove(i);
        Some(v)
    }
    fn nombre(&mut self, nom: &str, defaut: f64) -> f64 {
        self.option(nom).map_or(defaut, |v| v.parse().unwrap_or_else(|_| erreur(&format!("{nom} : nombre attendu"))))
    }
    fn drapeau(&mut self, nom: &str) -> bool {
        let i = self.0.iter().position(|a| a == nom);
        if let Some(i) = i {
            self.0.remove(i);
        }
        i.is_some()
    }
    /// « 30241,37462 » ou « 40241:30241 » (port d'écoute : port de la Station).
    fn ports(&mut self, nom: &str) -> Vec<(u16, u16)> {
        let port = |p: &str| -> u16 {
            p.trim().parse().unwrap_or_else(|_| erreur(&format!("{nom} : port invalide « {p} »")))
        };
        self.option(nom)
            .map(|v| {
                v.split(',')
                    .filter(|p| !p.is_empty())
                    .map(|p| match p.split_once(':') {
                        Some((a, b)) => (port(a), port(b)),
                        None => (port(p), port(p)),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn reste(self) -> Vec<String> {
        if let Some(a) = self.0.iter().find(|a| a.starts_with("--")) {
            erreur(&format!("option inconnue : {a}"));
        }
        self.0
    }
}

fn erreur(m: &str) -> ! {
    eprintln!("banc : {m}");
    exit(2)
}

fn charger(nom: &str, rtt: f64) -> Trace {
    let mut t = trace::charger(nom).unwrap_or_else(|e| erreur(&e));
    if rtt > 0.0 {
        t.ajouter_rtt(rtt);
    }
    t
}

fn main() {
    let mut v: Vec<String> = std::env::args().skip(1).collect();
    if v.is_empty() || v[0] == "--help" || v[0] == "-h" {
        println!("{}", include_str!("aide.txt"));
        return;
    }
    let commande = v.remove(0);
    let mut a = Args(v);
    match commande.as_str() {
        "profils" => {
            for (nom, texte) in trace::PROFILS {
                println!("{nom:<14} {texte}");
            }
        }
        "trace" => {
            let rtt = a.nombre("--rtt", 0.0);
            let json = a.drapeau("--json");
            for nom in a.reste() {
                let t = charger(&nom, rtt);
                if json { print!("{}", t.ecrire()) } else { print!("{}", t.resume()) }
            }
        }
        "bride" => {
            let station = a.option("--vers").unwrap_or_else(|| erreur("--vers <adresse de la Station> manque"));
            let r = banc::bride::Reglages {
                ecoute: a.option("--ecoute").unwrap_or_else(|| "0.0.0.0".into()),
                udp: a.ports("--udp"),
                tcp: a.ports("--tcp"),
                trace: {
                    let rtt = a.nombre("--rtt", 0.0);
                    charger(&a.option("--profil").unwrap_or_else(|| "propre".into()), rtt)
                },
                boucle: a.drapeau("--boucle"),
                desordre: a.drapeau("--desordre"),
                journal: a.option("--journal"),
                duree: a.option("--duree").map(|s| Duration::from_secs_f64(s.parse().unwrap_or(0.0))),
                silencieux: a.drapeau("--silencieux"),
                station,
            };
            a.reste();
            if r.udp.is_empty() {
                erreur("--udp <ports> manque (le flux de la Station)");
            }
            eprint!("{}", r.trace.resume());
            eprintln!(
                "Bride : {} (UDP {:?}, TCP {:?}) → {} ; Ctrl+C pour arrêter.",
                r.ecoute, r.udp, r.tcp, r.station
            );
            if let Err(e) = banc::bride::lancer(r) {
                erreur(&e.to_string());
            }
        }
        "echo" => {
            let ecoute = a.option("--ecoute").unwrap_or_else(|| "0.0.0.0:7979".into());
            a.reste();
            if let Err(e) = banc::releve::echo(&ecoute, false) {
                erreur(&e.to_string());
            }
        }
        "enregistre" => {
            let echo = a.option("--echo").unwrap_or_else(|| erreur("--echo <hôte:port> manque"));
            let sortie = a.option("--sortie");
            let d = banc::releve::Demande {
                debit_kbps: (a.nombre("--debit", 30.0) * 1000.0) as u32,
                taille: a.nombre("--taille", 1200.0) as u16,
                duree: Duration::from_secs_f64(a.nombre("--duree", 60.0)),
                pas: Duration::from_millis(a.nombre("--pas", 200.0) as u64),
                nom: a.option("--nom").unwrap_or_else(|| "relevé".into()),
                echo,
            };
            a.reste();
            eprintln!("Relevé de {} s vers {} à {} kbit/s…", d.duree.as_secs(), d.echo, d.debit_kbps);
            match banc::releve::enregistrer(&d) {
                Ok((t, resume)) => {
                    eprintln!("{resume}");
                    match sortie {
                        Some(f) => {
                            std::fs::write(&f, t.ecrire()).unwrap_or_else(|e| erreur(&e.to_string()));
                            eprintln!("Trace écrite : {f} ({} paliers)", t.paliers.len());
                        }
                        None => print!("{}", t.ecrire()),
                    }
                }
                Err(e) => erreur(&e.to_string()),
            }
        }
        "rejeu-debit" => {
            let besoin = (a.nombre("--besoin", 60.0) * 1e6) as u32;
            let rtt = a.nombre("--rtt", 0.0);
            let json = a.option("--json");
            let etiquette = a.option("--etiquette").unwrap_or_default();
            let mut noms = a.reste();
            if noms.is_empty() {
                noms = trace::PROFILS.iter().map(|p| p.0.to_string()).collect();
            }
            println!(
                "{:<26}{:>9}{:>8}{:>8}{:>8}{:>10}{:>7}{:>9}{:>9}",
                "trace", "reçu", "util.", "file>15", "trous", "perdus", "chgts", "rtt p50", "rtt p95"
            );
            let mut bilans = Vec::new();
            for nom in &noms {
                let t = charger(nom, rtt);
                let b = simu::rejouer(&t, besoin, &mut DebitAutomatique(adaptive::AdaptiveBitrate::new()));
                println!(
                    "{:<26}{:>7.1} M{:>7.1}%{:>7} s{:>6} s{:>10}{:>7}{:>6.1} ms{:>6.1} ms",
                    b.trace,
                    b.recu_moyen_mbps,
                    b.utilisation_pct,
                    b.secondes_file,
                    b.secondes_trous,
                    b.paquets_perdus,
                    b.changements,
                    b.rtt_p50_ms,
                    b.rtt_p95_ms
                );
                bilans.push(b);
            }
            if let Some(f) = json {
                let doc = serde_json::json!({
                    "banc": "rejeu-debit",
                    "etiquette": etiquette,
                    "besoin_mbps": f64::from(besoin) / 1e6,
                    "bilans": bilans,
                });
                std::fs::write(&f, serde_json::to_string_pretty(&doc).unwrap()).unwrap_or_else(|e| erreur(&e.to_string()));
                eprintln!("Écrit : {f}");
            }
        }
        _ => erreur(&format!("commande inconnue : {commande} (banc --help)")),
    }
}
