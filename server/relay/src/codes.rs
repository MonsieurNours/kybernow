//! Les codes de partage réservés par les Stations (docs/relais.md) : uniques
//! entre toutes les Stations du relais, quatre au plus par Station, gardés
//! 120 s quand leur Station part (elle les repropose en revenant).

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use ring::rand::{SecureRandom, SystemRandom};

/// Codes d'une Station partie, gardés ce temps-là.
pub const CODE_KEEP: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
struct Entry {
    station: String,
    /// La Station est partie : le code tombe à cette heure.
    expires: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct Codes {
    by_code: HashMap<String, Entry>,
    by_station: HashMap<String, BTreeSet<String>>,
}

fn random_code() -> String {
    let mut b = [0u8; 4];
    loop {
        SystemRandom::new().fill(&mut b).expect("aléa du système");
        let n = u32::from_be_bytes(b);
        // Rejet des valeurs qui fausseraient la répartition.
        if n < u32::MAX - u32::MAX % 1_000_000 {
            return format!("{:06}", n % 1_000_000);
        }
    }
}

impl Codes {
    fn live(&self, code: &str, now: Instant) -> Option<&Entry> {
        self.by_code.get(code).filter(|e| e.expires.is_none_or(|t| now < t))
    }

    /// Réserve un code pour `station` : `preferred` s'il est libre ou déjà à
    /// elle, sinon un autre ; `None` au-delà de `max` codes.
    pub fn reserve(&mut self, station: &str, preferred: Option<&str>, max: usize, now: Instant) -> Option<String> {
        self.sweep(now);
        if let Some(p) = preferred.filter(|p| kybernow_tunnel::lookup::is_code(p)) {
            match self.live(p, now) {
                Some(e) if e.station == station => return Some(p.to_string()),
                None => {
                    if self.count(station) < max {
                        self.insert(station, p);
                        return Some(p.to_string());
                    }
                    return None;
                }
                Some(_) => {}
            }
        }
        if self.count(station) >= max {
            return None;
        }
        loop {
            let c = random_code();
            if self.live(&c, now).is_none() {
                self.insert(station, &c);
                return Some(c);
            }
        }
    }

    fn count(&self, station: &str) -> usize {
        self.by_station.get(station).map_or(0, BTreeSet::len)
    }

    fn insert(&mut self, station: &str, code: &str) {
        if let Some(old) = self.by_code.insert(code.to_string(), Entry { station: station.to_string(), expires: None })
            && old.station != station
            && let Some(set) = self.by_station.get_mut(&old.station)
        {
            set.remove(code);
        }
        self.by_station.entry(station.to_string()).or_default().insert(code.to_string());
    }

    pub fn release(&mut self, station: &str, code: &str) {
        if self.by_code.get(code).is_some_and(|e| e.station == station) {
            self.by_code.remove(code);
        }
        if let Some(set) = self.by_station.get_mut(station) {
            set.remove(code);
            if set.is_empty() {
                self.by_station.remove(station);
            }
        }
    }

    /// La Station de ce code, s'il vaut encore.
    pub fn lookup(&self, code: &str, now: Instant) -> Option<String> {
        self.live(code, now).map(|e| e.station.clone())
    }

    /// La Station est partie : ses codes tombent dans 120 s.
    pub fn station_left(&mut self, station: &str, now: Instant) {
        for code in self.by_station.get(station).into_iter().flatten() {
            if let Some(e) = self.by_code.get_mut(code) {
                e.expires = Some(now + CODE_KEEP);
            }
        }
    }

    /// La Station revient et repropose ses codes : ceux qu'elle tient encore,
    /// ou qui sont libres, lui restent (rendus) ; ses autres codes tombent.
    pub fn station_back(&mut self, station: &str, proposed: &[String], max: usize, now: Instant) -> Vec<String> {
        self.sweep(now);
        let old: Vec<String> = self.by_station.get(station).into_iter().flatten().cloned().collect();
        for code in old.iter().filter(|c| !proposed.contains(c)) {
            self.release(station, code);
        }
        let mut kept = Vec::new();
        for code in proposed.iter().filter(|c| kybernow_tunnel::lookup::is_code(c)) {
            let free_or_mine = self.live(code, now).is_none_or(|e| e.station == station);
            if free_or_mine && (kept.len() < max) {
                self.insert(station, code);
                kept.push(code.clone());
            }
        }
        kept
    }

    /// Oublie les codes tombés.
    pub fn sweep(&mut self, now: Instant) {
        let dead: Vec<(String, String)> = self
            .by_code
            .iter()
            .filter(|(_, e)| e.expires.is_some_and(|t| now >= t))
            .map(|(c, e)| (c.clone(), e.station.clone()))
            .collect();
        for (code, station) in dead {
            self.release(&station, &code);
        }
    }

    /// Codes en cours (métriques).
    pub fn active(&self, now: Instant) -> usize {
        self.by_code.values().filter(|e| e.expires.is_none_or(|t| now < t)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_uniques_entre_stations() {
        let t = Instant::now();
        let mut c = Codes::default();
        assert_eq!(c.reserve("a", Some("123456"), 4, t).as_deref(), Some("123456"));
        assert_eq!(c.reserve("a", Some("123456"), 4, t).as_deref(), Some("123456"), "déjà à elle");
        let other = c.reserve("b", Some("123456"), 4, t).unwrap();
        assert_ne!(other, "123456");
        assert_eq!(c.lookup("123456", t).as_deref(), Some("a"));
        assert_eq!(c.lookup(&other, t).as_deref(), Some("b"));
        c.release("b", "123456");
        assert_eq!(c.lookup("123456", t).as_deref(), Some("a"), "b ne libère pas le code de a");
        c.release("a", "123456");
        assert_eq!(c.lookup("123456", t), None);
        assert!(c.reserve("a", Some("12345x"), 4, t).is_some(), "code proposé invalide : un autre");
    }

    #[test]
    fn quatre_codes_au_plus() {
        let t = Instant::now();
        let mut c = Codes::default();
        for _ in 0..4 {
            assert!(c.reserve("a", None, 4, t).is_some());
        }
        assert!(c.reserve("a", None, 4, t).is_none());
        assert!(c.reserve("a", Some("000001"), 4, t).is_none());
        assert_eq!(c.active(t), 4);
    }

    #[test]
    fn gardes_120_s_apres_le_depart() {
        let t = Instant::now();
        let mut c = Codes::default();
        c.reserve("a", Some("111111"), 4, t);
        c.reserve("a", Some("222222"), 4, t);
        c.station_left("a", t);
        assert_eq!(c.lookup("111111", t + Duration::from_secs(60)).as_deref(), Some("a"));
        // Pendant ce temps, une autre Station ne prend pas ces codes.
        assert_ne!(c.reserve("b", Some("111111"), 4, t + Duration::from_secs(60)).as_deref(), Some("111111"));
        // La Station revient avec un seul de ses codes.
        let kept = c.station_back("a", &["111111".into()], 4, t + Duration::from_secs(90));
        assert_eq!(kept, vec!["111111".to_string()]);
        assert_eq!(c.lookup("111111", t + Duration::from_secs(500)).as_deref(), Some("a"), "repris, il ne tombe plus");
        assert_eq!(c.lookup("222222", t + Duration::from_secs(90)), None);
        // Partie trop longtemps : tombés.
        c.station_left("a", t + Duration::from_secs(100));
        assert_eq!(c.lookup("111111", t + Duration::from_secs(221)), None);
        assert_eq!(c.station_back("a", &["111111".into()], 4, t + Duration::from_secs(300)), vec!["111111".to_string()]);
    }
}
