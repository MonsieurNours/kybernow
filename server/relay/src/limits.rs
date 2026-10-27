//! Les protections du relais : seaux à jetons (horloge passée en argument,
//! pour les essais), compteurs de connexions par adresse, liste de
//! bannissement relue à chaud.

use std::collections::HashMap;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use crate::config::Limits;

/// Un seau à jetons : `capacity` jetons, rendus en `period`.
#[derive(Debug, Clone)]
pub struct Bucket {
    capacity: f64,
    tokens: f64,
    per_sec: f64,
    last: Instant,
}

impl Bucket {
    pub fn new(capacity: u32, period: Duration, now: Instant) -> Bucket {
        let capacity = f64::from(capacity.max(1));
        Bucket { capacity, tokens: capacity, per_sec: capacity / period.as_secs_f64(), last: now }
    }

    fn refill(&mut self, now: Instant) {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + dt * self.per_sec).min(self.capacity);
        self.last = now;
    }

    /// Prend `n` jetons, ou dit dans combien de temps ils seront là.
    pub fn take_n(&mut self, n: f64, now: Instant) -> Result<(), Duration> {
        self.refill(now);
        if self.tokens >= n {
            self.tokens -= n;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((n - self.tokens) / self.per_sec))
        }
    }

    pub fn take(&mut self, now: Instant) -> Result<(), Duration> {
        self.take_n(1.0, now)
    }

    fn is_full(&mut self, now: Instant) -> bool {
        self.refill(now);
        self.tokens >= self.capacity
    }
}

/// Un seau par clé (adresse, réseau).
#[derive(Debug)]
pub struct Buckets<K> {
    map: HashMap<K, Bucket>,
    capacity: u32,
    period: Duration,
}

impl<K: Eq + Hash + Clone> Buckets<K> {
    pub fn new(capacity: u32, period: Duration) -> Self {
        Buckets { map: HashMap::new(), capacity, period }
    }

    pub fn take(&mut self, key: &K, now: Instant) -> Result<(), Duration> {
        let (capacity, period) = (self.capacity, self.period);
        self.map.entry(key.clone()).or_insert_with(|| Bucket::new(capacity, period, now)).take(now)
    }

    /// Oublie les seaux pleins (adresses revenues au calme).
    pub fn sweep(&mut self, now: Instant) {
        self.map.retain(|_, b| !b.is_full(now));
    }

}

/// Le réseau d'une adresse pour les limites : /24 en IPv4, /48 en IPv6.
pub fn subnet(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            IpAddr::V4(Ipv4Addr::new(o[0], o[1], o[2], 0))
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(Ipv6Addr::new(s[0], s[1], s[2], 0, 0, 0, 0, 0))
        }
    }
}

/// Recherches de codes : par adresse (minute, heure), par réseau, en tout.
#[derive(Debug)]
pub struct LookupLimiter {
    per_minute: Buckets<IpAddr>,
    per_hour: Buckets<IpAddr>,
    per_subnet: Buckets<IpAddr>,
    total: Bucket,
}

impl LookupLimiter {
    pub fn new(l: &Limits, now: Instant) -> Self {
        LookupLimiter {
            per_minute: Buckets::new(l.lookups_per_minute, Duration::from_secs(60)),
            per_hour: Buckets::new(l.lookups_per_hour, Duration::from_secs(3600)),
            per_subnet: Buckets::new(l.lookups_per_minute_subnet, Duration::from_secs(60)),
            total: Bucket::new(l.lookups_per_minute_total, Duration::from_secs(60), now),
        }
    }

    /// Une recherche de plus ; `Err` : réessayer dans ce temps, avec le nom de la limite.
    pub fn check(&mut self, ip: IpAddr, now: Instant) -> Result<(), (Duration, &'static str)> {
        let ip = ip.to_canonical();
        self.per_minute.take(&ip, now).map_err(|d| (d, "lookup_ip_minute"))?;
        self.per_hour.take(&ip, now).map_err(|d| (d, "lookup_ip_hour"))?;
        self.per_subnet.take(&subnet(ip), now).map_err(|d| (d, "lookup_subnet"))?;
        self.total.take(now).map_err(|d| (d, "lookup_total"))
    }

    pub fn sweep(&mut self, now: Instant) {
        self.per_minute.sweep(now);
        self.per_hour.sweep(now);
        self.per_subnet.sweep(now);
    }
}

/// Connexions en cours par adresse, rendues quand le jeton est lâché.
#[derive(Debug, Clone, Default)]
pub struct ConnCounter(Arc<Mutex<HashMap<IpAddr, usize>>>);

#[derive(Debug)]
pub struct ConnGuard {
    counter: ConnCounter,
    ip: IpAddr,
}

impl ConnCounter {
    pub fn try_acquire(&self, ip: IpAddr, max: usize) -> Option<ConnGuard> {
        let ip = ip.to_canonical();
        let mut map = self.0.lock().unwrap();
        let n = map.entry(ip).or_insert(0);
        if *n >= max {
            return None;
        }
        *n += 1;
        Some(ConnGuard { counter: self.clone(), ip })
    }

    pub fn total(&self) -> usize {
        self.0.lock().unwrap().values().sum()
    }
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        let mut map = self.counter.0.lock().unwrap();
        if let Some(n) = map.get_mut(&self.ip) {
            *n -= 1;
            if *n == 0 {
                map.remove(&self.ip);
            }
        }
    }
}

/// Liste de bannissement : une adresse ou un réseau (`203.0.113.0/24`) par
/// ligne, `#` pour un commentaire ; relue quand le fichier change.
#[derive(Debug, Default)]
pub struct Banlist {
    path: Option<PathBuf>,
    nets: RwLock<Vec<(IpAddr, u8)>>,
    stamp: Mutex<Option<SystemTime>>,
}

pub fn parse_net(line: &str) -> Option<(IpAddr, u8)> {
    let line = line.split('#').next()?.trim();
    if line.is_empty() {
        return None;
    }
    let (addr, bits) = match line.split_once('/') {
        Some((a, b)) => (a.trim().parse::<IpAddr>().ok()?, b.trim().parse::<u8>().ok()?),
        None => {
            let a = line.parse::<IpAddr>().ok()?;
            (a, if a.is_ipv4() { 32 } else { 128 })
        }
    };
    let max = if addr.is_ipv4() { 32 } else { 128 };
    (bits <= max).then_some((addr.to_canonical(), bits))
}

fn in_net(ip: IpAddr, (net, bits): (IpAddr, u8)) -> bool {
    match (ip.to_canonical(), net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

impl Banlist {
    pub fn new(path: Option<PathBuf>) -> Arc<Banlist> {
        let b = Arc::new(Banlist { path, ..Default::default() });
        b.reload_if_changed();
        b
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        self.nets.read().unwrap().iter().any(|n| in_net(ip, *n))
    }

    pub fn reload_if_changed(&self) {
        let Some(path) = &self.path else { return };
        let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if *self.stamp.lock().unwrap() == stamp {
            return;
        }
        let nets: Vec<(IpAddr, u8)> = std::fs::read_to_string(path).unwrap_or_default().lines().filter_map(parse_net).collect();
        tracing::info!("liste de bannissement : {} entrées", nets.len());
        *self.nets.write().unwrap() = nets;
        *self.stamp.lock().unwrap() = stamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seau_a_jetons() {
        let t0 = Instant::now();
        let mut b = Bucket::new(10, Duration::from_secs(60), t0);
        for _ in 0..10 {
            assert!(b.take(t0).is_ok());
        }
        let wait = b.take(t0).unwrap_err();
        assert!(wait <= Duration::from_secs(6) && wait > Duration::from_secs(5));
        assert!(b.take(t0 + Duration::from_secs(6)).is_ok());
    }

    #[test]
    fn recherches_limitees() {
        let t0 = Instant::now();
        let mut l = LookupLimiter::new(&Limits::default(), t0);
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        for _ in 0..10 {
            assert!(l.check(ip, t0).is_ok());
        }
        assert_eq!(l.check(ip, t0).unwrap_err().1, "lookup_ip_minute");
        // Le même /24 : 30 par minute en tout.
        for i in 0..20 {
            assert!(l.check(format!("203.0.113.{}", 10 + i).parse().unwrap(), t0).is_ok());
        }
        assert_eq!(l.check("203.0.113.200".parse().unwrap(), t0).unwrap_err().1, "lookup_subnet");
        assert!(l.check("198.51.100.1".parse().unwrap(), t0).is_ok());
        // Une heure : 60 au plus par adresse.
        let mut t = t0;
        let other: IpAddr = "192.0.2.1".parse().unwrap();
        let mut ok = 0;
        for _ in 0..80 {
            t += Duration::from_secs(7);
            if l.check(other, t).is_ok() {
                ok += 1;
            }
        }
        assert!(ok < 80);
        assert_eq!(subnet("2001:db8:1:2:3::9".parse().unwrap()), "2001:db8:1::".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn connexions_par_adresse() {
        let c = ConnCounter::default();
        let ip: IpAddr = "192.0.2.7".parse().unwrap();
        let a = c.try_acquire(ip, 2).unwrap();
        let _b = c.try_acquire(ip, 2).unwrap();
        assert!(c.try_acquire(ip, 2).is_none());
        drop(a);
        assert!(c.try_acquire(ip, 2).is_some());
    }

    #[test]
    fn bannissement() {
        let nets: Vec<_> = "203.0.113.0/24 # abus\n2001:db8::/32\n\n192.0.2.9\nimpossible".lines().filter_map(parse_net).collect();
        assert_eq!(nets.len(), 3);
        let b = Banlist { nets: RwLock::new(nets), ..Default::default() };
        assert!(b.contains("203.0.113.77".parse().unwrap()));
        assert!(b.contains("::ffff:203.0.113.1".parse().unwrap()));
        assert!(b.contains("2001:db8:ffff::1".parse().unwrap()));
        assert!(b.contains("192.0.2.9".parse().unwrap()));
        assert!(!b.contains("192.0.2.10".parse().unwrap()));
    }
}
