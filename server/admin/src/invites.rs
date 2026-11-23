//! Invitations : ajouter un ordinateur de loin (docs/invitations.md).
//!
//! L'administrateur crée une invitation dans Admin (nom, durée) ; la Station
//! lui donne un code à six chiffres, tenu chez le relais comme celui du
//! partage, et un mot de passe tiré au hasard, montré une seule fois. Un
//! appareil, où qu'il soit, tape le code dans « Rejoindre avec un code »,
//! puis le mot de passe : il reçoit un compte Kyber à lui, comme après un
//! appairage accepté, sans que personne ait à l'accepter.
//!
//! Le code seul ne donne rien. Les mots de passe faux sont comptés par
//! adresse et au total ; l'invitation et les appareils qu'elle a ajoutés
//! tombent à son échéance.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::display::now_secs;
use crate::i18n::Msg;

pub const FILE: &str = "invites.json";
/// Invitations ouvertes en même temps : le relais garde quatre codes par
/// Station, dont ceux du partage et du prêt.
pub const MAX_INVITES: usize = 2;
/// Appareils ajoutés au plus par une invitation.
pub const MAX_DEVICES: usize = 5;
/// Durées proposées, en jours.
pub const DAYS: [u64; 3] = [1, 7, 30];
const MAX_NAME: usize = 40;

/// Mots de passe faux depuis une même adresse (un /64 en IPv6) avant de la
/// bloquer, et au total avant de bloquer toutes les invitations.
const FAILS_PER_ADDRESS: usize = 5;
const FAILS_ALL: usize = 50;
const FAIL_WINDOW: Duration = Duration::from_secs(10 * 60);
const LOCK_FOR: Duration = Duration::from_secs(10 * 60);

/// Lettres et chiffres sans ambiguïté à l'œil (ni i, l, o, 0, 1).
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub id: String,
    pub name: String,
    pub code: String,
    /// Empreinte Argon2 du mot de passe, jamais rendue.
    hash: String,
    pub created: u64,
    pub expires: u64,
    /// Appareils ajoutés par cette invitation (identifiants des appareils).
    #[serde(default)]
    pub devices: Vec<String>,
    #[serde(default)]
    pub last_used: Option<u64>,
}

impl Invite {
    pub fn active(&self, now: u64) -> bool {
        now < self.expires
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Ce que voit Admin (jamais le mot de passe ni son empreinte).
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "code": self.code,
            "created": self.created,
            "expires": self.expires,
            "devices": self.devices,
            "max_devices": MAX_DEVICES,
            "last_used": self.last_used,
        })
    }
}

/// Un mot de passe d'invitation : douze signes de [`ALPHABET`] (59 bits),
/// groupés par quatre (`k7mq-p2xa-9fz3`).
pub fn generate_password() -> String {
    let mut out = String::with_capacity(14);
    for i in 0..12 {
        if i > 0 && i % 4 == 0 {
            out.push('-');
        }
        let n = rand::random::<u32>() as usize % ALPHABET.len();
        out.push(ALPHABET[n] as char);
    }
    out
}

/// Le mot de passe tel qu'il est vérifié : sans espaces ni tirets, en minuscules
/// (tapé à la télécommande ou recopié, il passe pareil).
pub fn normalize_password(password: &str) -> String {
    password.chars().filter(|c| !c.is_whitespace() && *c != '-').flat_map(char::to_lowercase).collect()
}

pub fn hash_password(password: &str) -> Result<String> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    let salt_bytes: [u8; 16] = rand::random();
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    let normalized = normalize_password(password);
    Ok(argon2::Argon2::default().hash_password(normalized.as_bytes(), &salt).map_err(|e| anyhow::anyhow!("{e}"))?.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    let normalized = normalize_password(password);
    !normalized.is_empty() && PasswordHash::new(hash).is_ok_and(|parsed| argon2::Argon2::default().verify_password(normalized.as_bytes(), &parsed).is_ok())
}

/// Le code d'une invitation : six chiffres (les espaces tapés sont retirés).
pub fn normalize_code(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

fn random_code() -> String {
    format!("{:06}", rand::random::<u32>() % 1_000_000)
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddError {
    /// L'invitation n'existe plus, ou a expiré.
    Gone,
    /// Ses appareils sont au complet.
    Full,
}

#[derive(Default)]
struct Strikes {
    by_address: HashMap<IpAddr, Vec<Instant>>,
    all: Vec<Instant>,
    address_locked: HashMap<IpAddr, Instant>,
    all_locked: Option<Instant>,
}

/// Une adresse : un /64 en IPv6, l'adresse entière en IPv4.
fn address_key(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let s = v6.segments();
            IpAddr::V6(std::net::Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0))
        }
        v4 => v4,
    }
}

/// Les invitations, gardées dans `invites.json` du dossier de données
/// (réservé à SYSTEM et aux Administrateurs).
pub struct Invites {
    path: PathBuf,
    list: Mutex<Vec<Invite>>,
    strikes: Mutex<Strikes>,
}

impl Invites {
    pub fn load(data_dir: &Path) -> Invites {
        let path = data_dir.join(FILE);
        let list = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Vec<Invite>>(&b).ok()).unwrap_or_default();
        Invites { path, list: Mutex::new(list), strikes: Mutex::new(Strikes::default()) }
    }

    fn save(&self, list: &[Invite]) -> Result<()> {
        crate::durable::write(&self.path, serde_json::to_vec_pretty(list)?).with_context(|| format!("remplacement de {}", self.path.display()))
    }

    pub fn all(&self) -> Vec<Invite> {
        self.list.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<Invite> {
        self.list.lock().unwrap().iter().find(|i| i.id == id).cloned()
    }

    /// Une invitation ouverte (pas échue) ?
    pub fn any_active(&self) -> bool {
        let now = now_secs();
        self.list.lock().unwrap().iter().any(|i| i.active(now))
    }

    /// Les codes des invitations ouvertes, à tenir chez le relais.
    pub fn codes(&self) -> Vec<String> {
        let now = now_secs();
        self.list.lock().unwrap().iter().filter(|i| i.active(now)).map(|i| i.code.clone()).collect()
    }

    /// L'invitation ouverte de ce code.
    pub fn by_code(&self, code: &str) -> Option<Invite> {
        let code = normalize_code(code);
        let now = now_secs();
        self.list.lock().unwrap().iter().find(|i| i.code == code && i.active(now)).cloned()
    }

    pub fn is_invite_code(&self, code: &str) -> bool {
        self.by_code(code).is_some()
    }

    /// L'invitation qui a ajouté cet appareil.
    pub fn of_device(&self, device_id: &str) -> Option<Invite> {
        self.list.lock().unwrap().iter().find(|i| i.devices.iter().any(|d| d == device_id)).cloned()
    }

    /// Nouvelle invitation : rend l'invitation et son mot de passe en clair
    /// (montré une seule fois). `hash` : l'empreinte de ce mot de passe,
    /// calculée hors du fil asynchrone ; `taken` : codes déjà pris (partage, prêt).
    pub fn create(&self, name: &str, days: u64, hash: String, taken: impl Fn(&str) -> bool) -> Result<Invite> {
        let name: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if name.is_empty() {
            bail!(Msg::new("invites.nameRequired"));
        }
        if !DAYS.contains(&days) {
            bail!(Msg::new("invites.badDuration"));
        }
        let now = now_secs();
        let mut list = self.list.lock().unwrap();
        if list.iter().filter(|i| i.active(now)).count() >= MAX_INVITES {
            bail!(Msg::new("invites.tooMany").with("max", MAX_INVITES));
        }
        let code = loop {
            let c = random_code();
            if !taken(&c) && !list.iter().any(|i| i.code == c) {
                break c;
            }
        };
        let invite = Invite {
            id: hex::encode(rand::random::<[u8; 8]>()),
            name,
            code,
            hash,
            created: now,
            expires: now + days * 24 * 3600,
            devices: Vec::new(),
            last_used: None,
        };
        list.push(invite.clone());
        self.save(&list)?;
        Ok(invite)
    }

    /// Nouveau mot de passe (empreinte déjà calculée) ; `None` : invitation inconnue.
    pub fn set_hash(&self, id: &str, hash: String) -> Result<Option<Invite>> {
        let mut list = self.list.lock().unwrap();
        let Some(invite) = list.iter_mut().find(|i| i.id == id) else { return Ok(None) };
        invite.hash = hash;
        let invite = invite.clone();
        self.save(&list)?;
        Ok(Some(invite))
    }

    pub fn delete(&self, id: &str) -> Result<Option<Invite>> {
        let mut list = self.list.lock().unwrap();
        let Some(index) = list.iter().position(|i| i.id == id) else { return Ok(None) };
        let invite = list.remove(index);
        self.save(&list)?;
        Ok(Some(invite))
    }

    /// L'appareil `device_id` entre par l'invitation `id` ; le même appareil
    /// qui revient ne prend pas de place de plus.
    pub fn add_device(&self, id: &str, device_id: &str) -> std::result::Result<(), AddError> {
        let now = now_secs();
        let mut list = self.list.lock().unwrap();
        let invite = list.iter_mut().find(|i| i.id == id && i.active(now)).ok_or(AddError::Gone)?;
        if !invite.devices.iter().any(|d| d == device_id) {
            if invite.devices.len() >= MAX_DEVICES {
                return Err(AddError::Full);
            }
            invite.devices.push(device_id.to_string());
        }
        invite.last_used = Some(now);
        let _ = self.save(&list);
        Ok(())
    }

    /// L'appareil est retiré (Admin › Appareils Link) : sa place se libère.
    pub fn remove_device(&self, device_id: &str) {
        let mut list = self.list.lock().unwrap();
        let mut changed = false;
        for invite in list.iter_mut() {
            let before = invite.devices.len();
            invite.devices.retain(|d| d != device_id);
            changed |= invite.devices.len() != before;
        }
        if changed {
            let _ = self.save(&list);
        }
    }

    /// Le relais a donné un autre code (le nôtre était pris ailleurs).
    pub fn replace_code(&self, old: &str, new: &str) -> bool {
        let mut list = self.list.lock().unwrap();
        let Some(invite) = list.iter_mut().find(|i| i.code == old) else { return false };
        invite.code = new.to_string();
        let _ = self.save(&list);
        true
    }

    /// Retire et rend les invitations échues.
    pub fn take_expired(&self) -> Vec<Invite> {
        let now = now_secs();
        let mut list = self.list.lock().unwrap();
        let (gone, kept): (Vec<Invite>, Vec<Invite>) = list.drain(..).partition(|i| !i.active(now));
        *list = kept;
        if !gone.is_empty() {
            let _ = self.save(&list);
        }
        gone
    }

    // --- Essais ---

    /// Avant de vérifier un mot de passe : `Err(attente en secondes)` si
    /// cette adresse, ou toutes, sont bloquées.
    pub fn begin_attempt(&self, ip: IpAddr) -> std::result::Result<(), u64> {
        self.begin_attempt_at(ip, Instant::now())
    }

    fn begin_attempt_at(&self, ip: IpAddr, now: Instant) -> std::result::Result<(), u64> {
        let mut s = self.strikes.lock().unwrap();
        let left = |until: Instant| (until + LOCK_FOR).saturating_duration_since(now);
        if let Some(t) = s.all_locked {
            if left(t) > Duration::ZERO {
                return Err(left(t).as_secs().max(1));
            }
            s.all_locked = None;
        }
        let key = address_key(ip);
        if let Some(t) = s.address_locked.get(&key).copied() {
            if left(t) > Duration::ZERO {
                return Err(left(t).as_secs().max(1));
            }
            s.address_locked.remove(&key);
        }
        Ok(())
    }

    /// Après la vérification : un échec compte ; rend `Some(true)` quand
    /// toutes les invitations viennent d'être bloquées, `Some(false)` quand
    /// cette adresse vient de l'être.
    pub fn end_attempt(&self, ip: IpAddr, ok: bool) -> Option<bool> {
        self.end_attempt_at(ip, ok, Instant::now())
    }

    fn end_attempt_at(&self, ip: IpAddr, ok: bool, now: Instant) -> Option<bool> {
        if ok {
            return None;
        }
        let mut s = self.strikes.lock().unwrap();
        let key = address_key(ip);
        let recent = |times: &mut Vec<Instant>| times.retain(|t| now.saturating_duration_since(*t) < FAIL_WINDOW);
        recent(&mut s.all);
        s.all.push(now);
        s.by_address.retain(|_, times| {
            recent(times);
            !times.is_empty()
        });
        let times = s.by_address.entry(key).or_default();
        times.push(now);
        let address_full = times.len() >= FAILS_PER_ADDRESS;
        if s.all.len() >= FAILS_ALL {
            s.all.clear();
            s.all_locked = Some(now);
            return Some(true);
        }
        if address_full {
            s.by_address.remove(&key);
            s.address_locked.insert(key, now);
            return Some(false);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Invites, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kn-invites-{}", hex::encode(rand::random::<[u8; 6]>())));
        std::fs::create_dir_all(&dir).unwrap();
        (Invites::load(&dir), dir)
    }

    #[test]
    fn mot_de_passe_tire_et_verifie() {
        let p = generate_password();
        assert_eq!(p.len(), 14);
        assert_eq!(p.matches('-').count(), 2);
        assert!(p.chars().all(|c| c == '-' || ALPHABET.contains(&(c as u8))));
        let hash = hash_password(&p).unwrap();
        assert!(verify_password(&hash, &p));
        assert!(verify_password(&hash, &p.to_uppercase().replace('-', " ")), "majuscules, espaces : le même");
        assert!(!verify_password(&hash, "autre-chose-1234"));
        assert!(!verify_password(&hash, ""));
    }

    #[test]
    fn creation_codes_et_limites() {
        let (inv, dir) = store();
        let a = inv.create("App Review", 7, "h".into(), |_| false).unwrap();
        assert_eq!(a.code.len(), 6);
        assert!(a.json().get("hash").is_none());
        assert_eq!(a.expires - a.created, 7 * 24 * 3600);
        assert!(inv.create("  ", 7, "h".into(), |_| false).is_err());
        assert!(inv.create("x", 3, "h".into(), |_| false).is_err(), "durées proposées seulement");
        let b = inv.create("Moi", 1, "h".into(), |c| c == a.code).unwrap();
        assert_ne!(a.code, b.code);
        assert!(inv.create("Trop", 30, "h".into(), |_| false).is_err(), "deux invitations au plus");
        assert_eq!(inv.codes().len(), 2);
        assert!(inv.is_invite_code(&format!("{} {}", &a.code[..3], &a.code[3..])));
        // Relue depuis le disque.
        let again = Invites::load(&dir);
        assert_eq!(again.all().len(), 2);
        assert!(inv.replace_code(&a.code, "999999"));
        assert!(inv.by_code("999999").is_some());
        inv.delete(&b.id).unwrap();
        assert_eq!(inv.all().len(), 1);
    }

    #[test]
    fn appareils_et_echeance() {
        let (inv, _dir) = store();
        let a = inv.create("App Review", 1, "h".into(), |_| false).unwrap();
        for i in 0..MAX_DEVICES {
            inv.add_device(&a.id, &format!("d{i}")).unwrap();
        }
        inv.add_device(&a.id, "d0").unwrap();
        assert_eq!(inv.add_device(&a.id, "d9"), Err(AddError::Full));
        assert_eq!(inv.of_device("d2").unwrap().id, a.id);
        inv.remove_device("d2");
        inv.add_device(&a.id, "d9").unwrap();
        assert_eq!(inv.add_device("inconnue", "x"), Err(AddError::Gone));
        assert!(inv.take_expired().is_empty());
        inv.list.lock().unwrap()[0].expires = now_secs() - 1;
        assert!(!inv.any_active());
        assert!(inv.by_code(&a.code).is_none());
        assert_eq!(inv.add_device(&a.id, "d1"), Err(AddError::Gone));
        let gone = inv.take_expired();
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].devices.len(), MAX_DEVICES);
        assert!(inv.all().is_empty());
    }

    #[test]
    fn mots_de_passe_faux_bloquent() {
        let (inv, _dir) = store();
        let t0 = Instant::now();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let other: IpAddr = "203.0.113.8".parse().unwrap();
        for _ in 0..FAILS_PER_ADDRESS - 1 {
            assert!(inv.begin_attempt_at(ip, t0).is_ok());
            assert_eq!(inv.end_attempt_at(ip, false, t0), None);
        }
        assert_eq!(inv.end_attempt_at(ip, false, t0), Some(false));
        assert!(inv.begin_attempt_at(ip, t0).is_err());
        assert!(inv.begin_attempt_at(other, t0).is_ok(), "une autre adresse passe");
        assert!(inv.begin_attempt_at(ip, t0 + LOCK_FOR + Duration::from_secs(1)).is_ok());
        // Un /64 compte pour une adresse.
        let (v6a, v6b): (IpAddr, IpAddr) = ("2001:db8:1:2::1".parse().unwrap(), "2001:db8:1:2::ffff".parse().unwrap());
        assert_eq!(address_key(v6a), address_key(v6b));
    }

    #[test]
    fn garde_fou_pour_toutes_les_invitations() {
        let (inv, _dir) = store();
        let t0 = Instant::now();
        let mut blocked_all = false;
        for i in 0..FAILS_ALL {
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(198, 51, (i / 200) as u8, (i % 200) as u8));
            if inv.end_attempt_at(ip, false, t0) == Some(true) {
                blocked_all = true;
            }
        }
        assert!(blocked_all);
        assert!(inv.begin_attempt_at("192.0.2.1".parse().unwrap(), t0).is_err());
    }
}
