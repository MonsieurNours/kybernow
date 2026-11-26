//! Relevé d'un vrai lien : `banc echo` tourne à l'autre bout (la Station,
//! le relais, n'importe quelle machine), `banc enregistre` lui demande un
//! flux régulier vers l'appareil, comme l'image d'une session, et en tire
//! une trace rejouable (débit reçu, retard, gigue, pertes, par pas de temps).
//!
//! Messages UDP, premier octet :
//! - 1 demande (appareil → écho) : nonce u32, débit kbit/s u32, taille u16,
//!   durée ms u32 ;
//! - 2 donnée (écho → appareil) : nonce u32, numéro u32, envoi µs u64 (horloge
//!   de l'écho) ;
//! - 3 sonde (appareil → écho) : numéro u32, envoi µs u64 (horloge de
//!   l'appareil), renvoyée telle quelle en 4 réponse.
//!
//! Le retard d'un sens se lit sans horloges communes : la différence
//! « arrivée − envoi » vaut un écart d'horloge constant plus le retard ; son
//! minimum sur le relevé est pris pour le retard de base, estimé à la moitié
//! de l'aller-retour le plus court des sondes.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::trace::{Etat, Palier, Trace};

const DEMANDE: u8 = 1;
const DONNEE: u8 = 2;
const SONDE: u8 = 3;
const REPONSE: u8 = 4;

fn micros(depuis: Instant) -> u64 {
    depuis.elapsed().as_micros() as u64
}

/// L'écho : répond aux sondes et envoie les flux demandés.
pub fn echo(ecoute: &str, silencieux: bool) -> io::Result<()> {
    let s = Arc::new(UdpSocket::bind(ecoute)?);
    let debut = Instant::now();
    if !silencieux {
        eprintln!("Écho prêt sur {}", s.local_addr()?);
    }
    let vus: Arc<Mutex<HashMap<u32, Instant>>> = Arc::default();
    let mut b = [0u8; 2048];
    loop {
        let (n, de) = s.recv_from(&mut b)?;
        match b[0] {
            SONDE if n >= 13 => {
                b[0] = REPONSE;
                let _ = s.send_to(&b[..n], de);
            }
            DEMANDE if n >= 15 => {
                let nonce = u32::from_be_bytes(b[1..5].try_into().unwrap());
                let kbps = u32::from_be_bytes(b[5..9].try_into().unwrap());
                let taille = u16::from_be_bytes(b[9..11].try_into().unwrap()).clamp(32, 1400) as usize;
                let duree = u32::from_be_bytes(b[11..15].try_into().unwrap()).min(30 * 60_000);
                {
                    let mut v = vus.lock().unwrap();
                    v.retain(|_, t| t.elapsed() < Duration::from_secs(3600));
                    if v.insert(nonce, Instant::now()).is_some() {
                        continue;
                    }
                }
                if !silencieux {
                    eprintln!("Flux vers {de} : {} kbit/s, {taille} octets, {} s", kbps, duree / 1000);
                }
                let s = s.clone();
                thread::spawn(move || envoyer(&s, de, nonce, kbps, taille, duree, debut));
            }
            _ => {}
        }
    }
}

fn envoyer(s: &UdpSocket, vers: SocketAddr, nonce: u32, kbps: u32, taille: usize, duree_ms: u32, horloge: Instant) {
    let ecart = (taille + 28) as f64 * 8.0 / (f64::from(kbps.max(1)) * 1000.0);
    let debut = Instant::now();
    let fin = debut + Duration::from_millis(u64::from(duree_ms));
    let mut paquet = vec![0u8; taille];
    paquet[0] = DONNEE;
    paquet[1..5].copy_from_slice(&nonce.to_be_bytes());
    let mut numero: u32 = 0;
    loop {
        let maintenant = Instant::now();
        if maintenant >= fin {
            return;
        }
        // Tout ce qui est dû à cet instant part, puis on dort un peu : un
        // débit régulier sans réveil à chaque paquet.
        let du = (maintenant.duration_since(debut).as_secs_f64() / ecart) as u32;
        while numero <= du {
            paquet[5..9].copy_from_slice(&numero.to_be_bytes());
            paquet[9..17].copy_from_slice(&micros(horloge).to_be_bytes());
            let _ = s.send_to(&paquet, vers);
            numero += 1;
        }
        thread::sleep(Duration::from_micros(500));
    }
}

pub struct Demande {
    pub echo: String,
    pub debit_kbps: u32,
    pub taille: u16,
    pub duree: Duration,
    pub pas: Duration,
    pub nom: String,
}

/// Une donnée reçue : son numéro, « arrivée − envoi » brut (µs), arrivée (µs
/// depuis le début du relevé).
struct Recu {
    numero: u32,
    ecart: i64,
    arrivee: u64,
}

pub fn enregistrer(d: &Demande) -> io::Result<(Trace, String)> {
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.connect(&d.echo)?;
    s.set_read_timeout(Some(Duration::from_millis(50)))?;
    let debut = Instant::now();
    let nonce = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u32) | 1;
    let mut demande = vec![DEMANDE];
    demande.extend(nonce.to_be_bytes());
    demande.extend(d.debit_kbps.to_be_bytes());
    demande.extend(d.taille.to_be_bytes());
    demande.extend((d.duree.as_millis() as u32).to_be_bytes());
    // Trois fois : la demande peut se perdre ; l'écho ignore les doublons.
    for _ in 0..3 {
        s.send(&demande)?;
    }

    let mut recus = Vec::new();
    let mut rtts: Vec<(u64, f64)> = Vec::new();
    let mut sondes_envoyees = 0u32;
    let mut prochaine_sonde = Instant::now();
    let fin = debut + d.duree + Duration::from_millis(1500);
    let mut b = [0u8; 2048];
    let mut vu_debut: Option<Instant> = None;
    while Instant::now() < fin {
        if Instant::now() >= prochaine_sonde {
            let mut p = vec![SONDE];
            p.extend(sondes_envoyees.to_be_bytes());
            p.extend(micros(debut).to_be_bytes());
            let _ = s.send(&p);
            sondes_envoyees += 1;
            prochaine_sonde += Duration::from_millis(20);
            if vu_debut.is_none() && sondes_envoyees == 50 {
                return Err(io::Error::other(format!("pas de réponse de l'écho {} : est-il lancé ?", d.echo)));
            }
        }
        match s.recv(&mut b) {
            Ok(n) if n >= 17 && b[0] == DONNEE && u32::from_be_bytes(b[1..5].try_into().unwrap()) == nonce => {
                let arrivee = micros(debut);
                vu_debut.get_or_insert_with(Instant::now);
                let envoi = u64::from_be_bytes(b[9..17].try_into().unwrap());
                recus.push(Recu {
                    numero: u32::from_be_bytes(b[5..9].try_into().unwrap()),
                    ecart: arrivee as i64 - envoi as i64,
                    arrivee,
                });
            }
            Ok(n) if n >= 13 && b[0] == REPONSE => {
                vu_debut.get_or_insert_with(Instant::now);
                let envoi = u64::from_be_bytes(b[5..13].try_into().unwrap());
                rtts.push((envoi, (micros(debut) - envoi) as f64 / 1000.0));
            }
            _ => {}
        }
    }
    if recus.is_empty() {
        return Err(io::Error::other("aucune donnée reçue de l'écho"));
    }
    Ok(analyser(d, &recus, &rtts, sondes_envoyees))
}

fn centile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

fn analyser(d: &Demande, recus: &[Recu], rtts: &[(u64, f64)], sondes: u32) -> (Trace, String) {
    let ecart_paquet_us = (f64::from(d.taille) + 28.0) * 8.0 / (f64::from(d.debit_kbps) * 1000.0) * 1e6;
    let base_ms = rtts.iter().map(|r| r.1).fold(f64::INFINITY, f64::min) / 2.0;
    let base_ms = if base_ms.is_finite() { base_ms } else { 0.0 };
    let min_ecart = recus.iter().map(|r| r.ecart).min().unwrap();
    let pas_us = d.pas.as_micros() as f64;
    // Le premier paquet reçu donne l'instant zéro du flux sur notre horloge.
    let premier = recus.iter().min_by_key(|r| r.numero).unwrap();
    let zero = premier.arrivee as f64 - (premier.ecart - min_ecart) as f64 - premier.numero as f64 * ecart_paquet_us;
    let pas_n = (d.duree.as_micros() as f64 / pas_us).ceil() as usize;
    let attendus = (pas_us / ecart_paquet_us).max(1.0);

    let mut par_pas: Vec<(Vec<f64>, u32)> = vec![(Vec::new(), 0); pas_n];
    let mut recus_par_arrivee = vec![0u64; pas_n + 20];
    for r in recus {
        // Pas de l'envoi : d'après le numéro (le flux est régulier).
        let i = (r.numero as f64 * ecart_paquet_us / pas_us) as usize;
        if i < pas_n {
            par_pas[i].0.push((r.ecart - min_ecart) as f64 / 1000.0);
            par_pas[i].1 += 1;
        }
        let j = ((r.arrivee as f64 - zero).max(0.0) / pas_us) as usize;
        if j < recus_par_arrivee.len() {
            recus_par_arrivee[j] += 1;
        }
    }
    let mut paliers = Vec::new();
    let mut total_attendus = 0.0;
    let mut total_recus = 0u32;
    let mut retards = Vec::new();
    for (i, (files, n)) in par_pas.iter_mut().enumerate() {
        total_attendus += attendus;
        total_recus += *n;
        let pertes = ((1.0 - f64::from(*n) / attendus) * 100.0).clamp(0.0, 100.0);
        let p10 = centile(files, 0.1);
        let p90 = centile(files, 0.9);
        let p50 = centile(files, 0.5);
        retards.extend(files.iter().copied());
        // Débit vu à l'arrivée : la capacité, quand le lien sature (file qui
        // monte ou pertes) ; sinon le lien n'est pas limité à ce débit.
        let mbps = recus_par_arrivee[i] as f64 * (f64::from(d.taille) + 28.0) * 8.0 / pas_us;
        let sature = p50 > 10.0 || pertes > 1.0;
        let montee_pertes = {
            let t0 = (i as f64 * pas_us) as u64;
            let t1 = t0 + pas_us as u64;
            let envoyees = (pas_us / 20_000.0).max(1.0);
            let revues = rtts.iter().filter(|r| r.0 >= t0 && r.0 < t1).count() as f64;
            ((1.0 - revues / envoyees) * 100.0).clamp(0.0, 100.0)
        };
        paliers.push(Palier {
            t_ms: (i as f64 * pas_us / 1000.0) as u64,
            descente: Etat {
                debit_mbps: if sature && *n > 0 { (mbps * 10.0).round() / 10.0 } else { 0.0 },
                // Lien saturé : la file vient du débit, que la bride rejoue ;
                // la compter aussi en retard la doublerait.
                retard_ms: ((base_ms + if sature { 0.0 } else { p10 }) * 10.0).round() / 10.0,
                gigue_ms: if sature { 0.0 } else { ((p90 - p10) * 10.0).round() / 10.0 },
                pertes_pct: if sature { 0.0 } else { (pertes * 10.0).round() / 10.0 },
                file_ms: 100.0,
            },
            montee: Etat {
                retard_ms: (base_ms * 10.0).round() / 10.0,
                // Sondes perdues : aller ou retour, on ne sait pas. Descente
                // saturée : la file de descente les a jetées au retour. Un
                // seuil évite de bruiter la montée d'une sonde en retard.
                pertes_pct: if !sature && montee_pertes >= 20.0 { (montee_pertes * 10.0).round() / 10.0 } else { 0.0 },
                ..Etat::libre()
            },
        });
    }
    // Paliers voisins identiques fusionnés.
    paliers.dedup_by(|b, a| a.descente == b.descente && a.montee == b.montee);
    let pertes = (1.0 - f64::from(total_recus) / total_attendus) * 100.0;
    let mut rtt: Vec<f64> = rtts.iter().map(|r| r.1).collect();
    let resume = format!(
        "{} données reçues sur ~{:.0} ({:.2} % perdues), retard de file p50 {:.1} ms, p95 {:.1} ms ; \
         aller-retour des sondes p50 {:.1} ms, p95 {:.1} ms ({} sur {} revenues)",
        total_recus,
        total_attendus,
        pertes.max(0.0),
        centile(&mut retards.clone(), 0.5),
        centile(&mut retards, 0.95),
        centile(&mut rtt, 0.5),
        centile(&mut rtt, 0.95),
        rtts.len(),
        sondes
    );
    (
        Trace {
            nom: d.nom.clone(),
            source: format!(
                "banc enregistre {} à {} kbit/s, pas de {} ms",
                d.echo,
                d.debit_kbps,
                d.pas.as_millis()
            ),
            paliers,
            duree: d.duree,
        },
        resume,
    )
}
