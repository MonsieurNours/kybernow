//! La bride : un relais UDP (et TCP) entre l'appareil et la Station, qui
//! joue une trace sur chaque sens. Il tourne en espace utilisateur, sans
//! droits administrateur, sur n'importe quel système : l'app se connecte au
//! relais comme à la Station.
//!
//! Chaque sens est un lien ([`Lien`]) : pertes au hasard, débit limité avec
//! une file d'attente bornée (le paquet qui attendrait trop est jeté), puis
//! retard fixe et gigue. Les paquets gardent leur ordre, sauf `desordre`.
//! Le TCP (Admin, repli WSS) passe sans bride.

use std::collections::{BinaryHeap, HashMap};
use std::io::{self, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::trace::{Etat, Hasard, Sens, Trace};

/// Compteurs d'un sens.
#[derive(Default)]
pub struct Compteurs {
    pub recus: AtomicU64,
    pub octets_recus: AtomicU64,
    pub perdus_hasard: AtomicU64,
    pub perdus_file: AtomicU64,
    pub envoyes: AtomicU64,
    pub octets_envoyes: AtomicU64,
}

impl Compteurs {
    fn lire(&self) -> [u64; 6] {
        [
            self.recus.load(Relaxed),
            self.octets_recus.load(Relaxed),
            self.perdus_hasard.load(Relaxed),
            self.perdus_file.load(Relaxed),
            self.envoyes.load(Relaxed),
            self.octets_envoyes.load(Relaxed),
        ]
    }
}

/// Un sens du lien : décide si un paquet passe, et quand il sort.
pub struct Lien {
    trace: Arc<Trace>,
    sens: Sens,
    debut: Instant,
    boucle: bool,
    desordre: bool,
    /// Instant où le lien aura fini d'émettre ce qui attend (débit limité).
    libre_a: Instant,
    derniere_sortie: Instant,
    hasard: Hasard,
}

/// Ce qui arrive à un paquet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    Sortie(Instant),
    PerduHasard,
    PerduFile,
}

impl Lien {
    pub fn new(trace: Arc<Trace>, sens: Sens, debut: Instant, boucle: bool, desordre: bool, graine: u64) -> Self {
        Self { trace, sens, debut, boucle, desordre, libre_a: debut, derniere_sortie: debut, hasard: Hasard::new(graine) }
    }

    pub fn etat(&self, maintenant: Instant) -> Etat {
        self.trace.etat(maintenant.saturating_duration_since(self.debut), self.sens, self.boucle)
    }

    /// Un paquet de `taille` octets arrive à `maintenant`.
    pub fn admettre(&mut self, maintenant: Instant, taille: usize) -> Sort {
        let e = self.etat(maintenant);
        if e.pertes_pct > 0.0 && self.hasard.unif() * 100.0 < e.pertes_pct {
            return Sort::PerduHasard;
        }
        let mut sortie = maintenant;
        if e.debit_mbps > 0.0 {
            let debut = self.libre_a.max(maintenant);
            if debut.duration_since(maintenant).as_secs_f64() * 1000.0 > e.file_ms {
                return Sort::PerduFile;
            }
            // Octets d'en-tête IP et UDP comptés, comme sur le fil.
            let bits = (taille + 28) as f64 * 8.0;
            self.libre_a = debut + Duration::from_secs_f64(bits / (e.debit_mbps * 1e6));
            sortie = self.libre_a;
        }
        let mut retard = e.retard_ms;
        if e.gigue_ms > 0.0 {
            retard += self.hasard.unif() * e.gigue_ms;
        }
        sortie += Duration::from_secs_f64(retard.max(0.0) / 1000.0);
        if !self.desordre {
            sortie = sortie.max(self.derniere_sortie);
        }
        self.derniere_sortie = self.derniere_sortie.max(sortie);
        Sort::Sortie(sortie)
    }
}

/// Où envoyer un paquet à sa sortie.
#[derive(Clone)]
enum Vers {
    /// Socket connectée vers la Station.
    Station(Arc<UdpSocket>),
    /// Vers l'appareil, par la socket d'écoute.
    Appareil(Arc<UdpSocket>, SocketAddr),
}

struct EnAttente {
    sortie: Instant,
    ordre: u64,
    donnees: Vec<u8>,
    vers: Vers,
}

impl PartialEq for EnAttente {
    fn eq(&self, o: &Self) -> bool {
        (self.sortie, self.ordre) == (o.sortie, o.ordre)
    }
}
impl Eq for EnAttente {}
impl PartialOrd for EnAttente {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for EnAttente {
    // Tas « min » : la sortie la plus proche en tête.
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (o.sortie, o.ordre).cmp(&(self.sortie, self.ordre))
    }
}

/// Un sens complet : le lien, son fil d'émission et ses compteurs.
#[derive(Clone)]
struct Voie {
    lien: Arc<Mutex<Lien>>,
    file: Sender<EnAttente>,
    compteurs: Arc<Compteurs>,
    ordre: Arc<AtomicU64>,
}

impl Voie {
    fn new(lien: Lien) -> Self {
        let (tx, rx) = mpsc::channel();
        let compteurs = Arc::new(Compteurs::default());
        let c = compteurs.clone();
        thread::Builder::new().name("banc-emission".into()).spawn(move || emettre(rx, c)).unwrap();
        Self { lien: Arc::new(Mutex::new(lien)), file: tx, compteurs, ordre: Arc::new(AtomicU64::new(0)) }
    }

    fn passer(&self, donnees: &[u8], vers: Vers) {
        let maintenant = Instant::now();
        self.compteurs.recus.fetch_add(1, Relaxed);
        self.compteurs.octets_recus.fetch_add(donnees.len() as u64, Relaxed);
        let sort = self.lien.lock().unwrap().admettre(maintenant, donnees.len());
        match sort {
            Sort::PerduHasard => {
                self.compteurs.perdus_hasard.fetch_add(1, Relaxed);
            }
            Sort::PerduFile => {
                self.compteurs.perdus_file.fetch_add(1, Relaxed);
            }
            Sort::Sortie(sortie) => {
                let ordre = self.ordre.fetch_add(1, Relaxed);
                let _ = self.file.send(EnAttente { sortie, ordre, donnees: donnees.to_vec(), vers });
            }
        }
    }
}

/// Le fil d'émission d'un sens : garde les paquets jusqu'à leur sortie.
fn emettre(rx: Receiver<EnAttente>, compteurs: Arc<Compteurs>) {
    let mut tas: BinaryHeap<EnAttente> = BinaryHeap::new();
    loop {
        let maintenant = Instant::now();
        while tas.peek().is_some_and(|p| p.sortie <= maintenant) {
            let p = tas.pop().unwrap();
            let r = match &p.vers {
                Vers::Station(s) => s.send(&p.donnees),
                Vers::Appareil(s, a) => s.send_to(&p.donnees, a),
            };
            if r.is_ok() {
                compteurs.envoyes.fetch_add(1, Relaxed);
                compteurs.octets_envoyes.fetch_add(p.donnees.len() as u64, Relaxed);
            }
        }
        let attente = tas.peek().map(|p| p.sortie.saturating_duration_since(Instant::now()));
        let recu = match attente {
            // macOS réveille tard une longue attente (jusqu'à 10 ms pour
            // 20 ms, le regroupement des minuteries) mais tient une attente
            // d'une milliseconde à 0,5 ms près : on attend par tranches
            // d'une milliseconde, et en tournant sur la dernière.
            Some(d) if d < Duration::from_micros(700) => {
                match rx.try_recv() {
                    Ok(p) => Ok(p),
                    Err(mpsc::TryRecvError::Empty) => {
                        thread::yield_now();
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            Some(d) => rx.recv_timeout((d - Duration::from_micros(600)).min(Duration::from_millis(1))),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match recu {
            Ok(p) => {
                tas.push(p);
                while let Ok(p) = rx.try_recv() {
                    tas.push(p);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

pub struct Reglages {
    pub ecoute: String,
    pub station: String,
    /// Ports relayés : (port d'écoute, port de la Station), le même d'habitude.
    pub udp: Vec<(u16, u16)>,
    pub tcp: Vec<(u16, u16)>,
    pub trace: Trace,
    pub boucle: bool,
    pub desordre: bool,
    pub journal: Option<String>,
    pub duree: Option<Duration>,
    pub silencieux: bool,
}

/// Lance la bride et rend la main à la fin de `duree` (jamais sans durée).
pub fn lancer(r: Reglages) -> io::Result<()> {
    let debut = Instant::now();
    let trace = Arc::new(r.trace);
    let descente = Voie::new(Lien::new(trace.clone(), Sens::Descente, debut, r.boucle, r.desordre, 0x6b79_6265));
    let montee = Voie::new(Lien::new(trace.clone(), Sens::Montee, debut, r.boucle, r.desordre, 0x6e6f_7721));

    for &(port, vers) in &r.udp {
        let ecoute = Arc::new(UdpSocket::bind((r.ecoute.as_str(), port))?);
        let station = (r.station.as_str(), vers)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::other(format!("adresse introuvable : {}", r.station)))?;
        let (d, m) = (descente.clone(), montee.clone());
        thread::Builder::new()
            .name(format!("banc-udp-{port}"))
            .spawn(move || relayer_udp(ecoute, station, d, m))?;
    }
    for &(port, vers) in &r.tcp {
        let ecoute = TcpListener::bind((r.ecoute.as_str(), port))?;
        let station = format!("{}:{vers}", r.station);
        thread::Builder::new().name(format!("banc-tcp-{port}")).spawn(move || relayer_tcp(ecoute, station))?;
    }

    let mut journal = match &r.journal {
        Some(p) => Some(std::fs::File::create(p)?),
        None => None,
    };
    let mut avant = [[0u64; 6]; 2];
    let mut seconde = 0u64;
    loop {
        thread::sleep((debut + Duration::from_secs(seconde + 1)).saturating_duration_since(Instant::now()));
        seconde += 1;
        let maintenant = Instant::now();
        let mut lignes = Vec::new();
        for (i, (nom, voie)) in [("descente", &descente), ("montee", &montee)].into_iter().enumerate() {
            let c = voie.compteurs.lire();
            let d: Vec<u64> = c.iter().zip(avant[i]).map(|(a, b)| a - b).collect();
            avant[i] = c;
            let e = voie.lien.lock().unwrap().etat(maintenant);
            lignes.push(serde_json::json!({
                "sens": nom,
                "recus": d[0], "perdus_hasard": d[2], "perdus_file": d[3], "envoyes": d[4],
                "mbps_recus": d[1] as f64 * 8.0 / 1e6, "mbps_envoyes": d[5] as f64 * 8.0 / 1e6,
                "etat": e,
            }));
        }
        let ligne = serde_json::json!({ "t_s": seconde, "voies": lignes });
        if let Some(j) = &mut journal {
            writeln!(j, "{ligne}")?;
        }
        if !r.silencieux {
            let v = |i: usize, k: &str| lignes[i][k].as_f64().unwrap_or(0.0);
            eprintln!(
                "{seconde:>5} s  descente {:>7.2} → {:>7.2} Mbit/s, {:>4} perdus ({:>4} file) | montée {:>6.2} Mbit/s, {:>3} perdus",
                v(0, "mbps_recus"),
                v(0, "mbps_envoyes"),
                v(0, "perdus_hasard") + v(0, "perdus_file"),
                v(0, "perdus_file"),
                v(1, "mbps_envoyes"),
                v(1, "perdus_hasard") + v(1, "perdus_file"),
            );
        }
        if r.duree.is_some_and(|d| maintenant.duration_since(debut) >= d) {
            return Ok(());
        }
    }
}

/// Un port UDP : une socket vers la Station par appareil vu.
fn relayer_udp(ecoute: Arc<UdpSocket>, station: SocketAddr, descente: Voie, montee: Voie) {
    let mut vers_station: HashMap<SocketAddr, Arc<UdpSocket>> = HashMap::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let Ok((n, appareil)) = ecoute.recv_from(&mut buf) else { continue };
        let s = match vers_station.get(&appareil) {
            Some(s) => s.clone(),
            None => {
                let lier = if station.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
                let Ok(s) = UdpSocket::bind(lier).and_then(|s| s.connect(station).map(|_| s)) else { continue };
                let s = Arc::new(s);
                vers_station.insert(appareil, s.clone());
                let (retour, ecoute, d) = (s.clone(), ecoute.clone(), descente.clone());
                thread::spawn(move || {
                    let mut buf = vec![0u8; 65536];
                    loop {
                        match retour.recv(&mut buf) {
                            Ok(n) => d.passer(&buf[..n], Vers::Appareil(ecoute.clone(), appareil)),
                            // ICMP « port fermé » remonté par une socket connectée : on continue.
                            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {}
                            Err(_) => thread::sleep(Duration::from_millis(5)),
                        }
                    }
                });
                s
            }
        };
        montee.passer(&buf[..n], Vers::Station(s));
    }
}

fn relayer_tcp(ecoute: TcpListener, station: String) {
    for entrant in ecoute.incoming().flatten() {
        let station = station.clone();
        thread::spawn(move || {
            let Ok(sortant) = TcpStream::connect(&station) else { return };
            let _ = entrant.set_nodelay(true);
            let _ = sortant.set_nodelay(true);
            let (mut a, mut b) = (entrant.try_clone().unwrap(), sortant.try_clone().unwrap());
            let (mut c, mut d) = (entrant, sortant);
            thread::spawn(move || {
                let _ = io::copy(&mut a, &mut b);
                let _ = b.shutdown(std::net::Shutdown::Write);
            });
            let _ = io::copy(&mut d, &mut c);
            let _ = c.shutdown(std::net::Shutdown::Write);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{Palier, profil};

    fn trace(d: Etat) -> Arc<Trace> {
        Arc::new(Trace {
            nom: "essai".into(),
            source: String::new(),
            paliers: vec![Palier { t_ms: 0, descente: d, montee: Etat::libre() }],
            duree: Duration::from_secs(60),
        })
    }

    #[test]
    fn debit_limite_espace_les_paquets() {
        let t0 = Instant::now();
        let mut l = Lien::new(trace(Etat { debit_mbps: 10.0, ..Etat::libre() }), Sens::Descente, t0, false, false, 1);
        // 1222 + 28 octets à 10 Mbit/s : 1 ms par paquet.
        let s: Vec<_> = (0..5).map(|_| l.admettre(t0, 1222)).collect();
        let Sort::Sortie(dernier) = s[4] else { panic!() };
        let ms = dernier.duration_since(t0).as_secs_f64() * 1000.0;
        assert!((ms - 5.0).abs() < 0.01, "{ms}");
    }

    #[test]
    fn file_pleine_jette() {
        let t0 = Instant::now();
        let e = Etat { debit_mbps: 1.0, file_ms: 20.0, ..Etat::libre() };
        let mut l = Lien::new(trace(e), Sens::Descente, t0, false, false, 1);
        // 10 ms par paquet de 1222 octets à 1 Mbit/s : le 4e attendrait 30 ms.
        let s: Vec<_> = (0..4).map(|_| l.admettre(t0, 1222)).collect();
        assert!(matches!(s[2], Sort::Sortie(_)));
        assert_eq!(s[3], Sort::PerduFile);
    }

    #[test]
    fn pertes_au_hasard_proches_de_la_consigne() {
        let t0 = Instant::now();
        let mut l = Lien::new(trace(Etat { pertes_pct: 5.0, ..Etat::libre() }), Sens::Descente, t0, false, false, 7);
        let perdus = (0..100_000).filter(|_| l.admettre(t0, 100) == Sort::PerduHasard).count();
        assert!((4500..5500).contains(&perdus), "{perdus}");
    }

    #[test]
    fn gigue_sans_desordre() {
        let t0 = Instant::now();
        let mut l = Lien::new(trace(Etat { gigue_ms: 15.0, ..Etat::libre() }), Sens::Descente, t0, false, false, 3);
        let mut avant = t0;
        for k in 0..1000 {
            let Sort::Sortie(s) = l.admettre(t0 + Duration::from_micros(k * 100), 100) else { panic!() };
            assert!(s >= avant);
            avant = s;
        }
    }

    /// Bout en bout sur la boucle locale : une « Station » qui renvoie, la
    /// bride entre les deux, 20 ms de retard dans chaque sens.
    #[test]
    fn relais_udp_avec_retard() {
        let station = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port_station = station.local_addr().unwrap().port();
        thread::spawn(move || {
            let mut b = [0u8; 2048];
            while let Ok((n, a)) = station.recv_from(&mut b) {
                let _ = station.send_to(&b[..n], a);
            }
        });
        // La bride écoute sur un autre port que la Station : on relaie à la main.
        let mut t = profil("propre").unwrap();
        t.ajouter_rtt(40.0);
        let t = Arc::new(t);
        let debut = Instant::now();
        let d = Voie::new(Lien::new(t.clone(), Sens::Descente, debut, false, false, 1));
        let m = Voie::new(Lien::new(t, Sens::Montee, debut, false, false, 2));
        let ecoute = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
        let adresse = ecoute.local_addr().unwrap();
        let cible: SocketAddr = format!("127.0.0.1:{port_station}").parse().unwrap();
        thread::spawn(move || relayer_udp(ecoute, cible, d, m));

        let appareil = UdpSocket::bind("127.0.0.1:0").unwrap();
        appareil.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let t0 = Instant::now();
        appareil.send_to(b"bonjour", adresse).unwrap();
        let mut b = [0u8; 64];
        let n = appareil.recv(&mut b).unwrap();
        let rtt = t0.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(&b[..n], b"bonjour");
        assert!((40.0..48.0).contains(&rtt), "{rtt}");
    }
}
