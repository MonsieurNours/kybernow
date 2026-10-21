//! Appairage par code, sans compte (app Apple TV).
//!
//! L'appareil tire un code à 6 chiffres, l'affiche, et l'envoie à la console
//! (`POST /api/pair/request`) avec sa présentation. La console montre la
//! demande et son code (Accès › Appareils, panneau de l'icône) ; l'administrateur
//! vérifie que c'est le code affiché sur l'écran de la télévision, puis accepte
//! ou refuse. L'appareil interroge l'état de sa demande
//! (`GET /api/pair/request/{id}`) et reçoit, une seule fois, l'accès qui lui a
//! été créé : un compte Kyber à son nom, propre à lui et révocable.
//!
//! Les demandes ne vivent qu'en mémoire : 5 minutes pour être acceptées, puis
//! 2 minutes pour que l'appareil vienne chercher son accès. Les demandes sans
//! compte sont limitées par adresse (débit et demandes en attente) et au total.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::activity::DeviceInfo;
use crate::display::now_secs;

/// Durée de vie d'une demande en attente (celle du code sur l'appareil).
pub const REQUEST_TTL: Duration = Duration::from_secs(5 * 60);
/// Délai laissé à l'appareil pour venir chercher son accès une fois accepté.
const COLLECT_TTL: Duration = Duration::from_secs(2 * 60);
/// Demandes par adresse et par minute.
const RATE_PER_MINUTE: usize = 10;
/// Demandes en attente par adresse, et au total.
const PENDING_PER_PEER: usize = 2;
const PENDING_TOTAL: usize = 6;
const MAX_NAME: usize = 60;

#[derive(Debug, PartialEq)]
pub enum RequestError {
    /// Code ou présentation invalides (clé du message, voir `i18n`).
    Invalid(&'static str),
    /// Trop de demandes de cette adresse, ou au total.
    TooMany,
}

/// État d'une demande, tel que l'appareil le lit.
#[derive(Debug, PartialEq)]
pub enum Status {
    Pending,
    Refused,
    Expired,
    /// Livré une seule fois : la demande disparaît aussitôt.
    Accepted { username: String, password: String },
}

/// Une demande en attente, telle que la console l'affiche.
#[derive(Debug, Clone, Serialize)]
pub struct Pending {
    pub id: String,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub os: Option<String>,
    pub app: Option<String>,
    pub address: String,
    /// Fin de validité (secondes depuis l'époque Unix).
    pub expires_at: u64,
}

enum Stage {
    Pending,
    Refused,
    Accepted { username: String, password: String, at: Instant },
}

struct Request {
    code: String,
    device: DeviceInfo,
    peer: IpAddr,
    created: Instant,
    created_secs: u64,
    stage: Stage,
}

impl Request {
    fn expired(&self, now: Instant) -> bool {
        match &self.stage {
            Stage::Accepted { at, .. } => now.duration_since(*at) > COLLECT_TTL,
            _ => now.duration_since(self.created) > REQUEST_TTL,
        }
    }
}

#[derive(Default)]
struct State {
    requests: HashMap<String, Request>,
    /// Heures des dernières demandes, par adresse.
    recent: HashMap<IpAddr, Vec<Instant>>,
    /// Comptes créés pour des demandes acceptées dont l'accès n'a jamais été
    /// livré : à supprimer.
    orphans: Vec<String>,
}

#[derive(Default)]
pub struct Pairing {
    state: Mutex<State>,
}

impl Pairing {
    pub fn new() -> Self {
        Self::default()
    }

    fn sweep(s: &mut State, now: Instant) {
        let mut orphans = Vec::new();
        s.requests.retain(|_, r| {
            if !r.expired(now) {
                return true;
            }
            if let Stage::Accepted { username, .. } = &r.stage {
                orphans.push(username.clone());
            }
            false
        });
        s.orphans.extend(orphans);
        s.recent.retain(|_, times| {
            times.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
            !times.is_empty()
        });
    }

    /// Nouvelle demande. Rend son identifiant (secret : c'est lui qui donne l'accès).
    pub fn request(&self, code: &str, mut device: DeviceInfo, peer: IpAddr) -> Result<String, RequestError> {
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(RequestError::Invalid("pairing.codeDigits"));
        }
        device.id = device.id.trim().chars().filter(|c| !c.is_control()).take(80).collect();
        device.name = device.name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if device.id.is_empty() || device.name.is_empty() {
            return Err(RequestError::Invalid("pairing.deviceIdentity"));
        }
        let now = Instant::now();
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, now);
        let times = s.recent.entry(peer).or_default();
        if times.len() >= RATE_PER_MINUTE {
            return Err(RequestError::TooMany);
        }
        times.push(now);
        // Un nouveau code du même appareil remplace sa demande précédente.
        s.requests.retain(|_, r| !(matches!(r.stage, Stage::Pending) && r.device.id == device.id));
        let pending = |r: &&Request| matches!(r.stage, Stage::Pending);
        if s.requests.values().filter(pending).filter(|r| r.peer == peer).count() >= PENDING_PER_PEER
            || s.requests.values().filter(pending).count() >= PENDING_TOTAL
        {
            return Err(RequestError::TooMany);
        }
        let id = hex::encode(rand::random::<[u8; 16]>());
        s.requests.insert(
            id.clone(),
            Request { code: code.to_string(), device, peer, created: now, created_secs: now_secs(), stage: Stage::Pending },
        );
        Ok(id)
    }

    /// État d'une demande ; l'accès accepté n'est livré qu'une fois.
    pub fn status(&self, id: &str) -> Status {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        match s.requests.get(id).map(|r| &r.stage) {
            None => Status::Expired,
            Some(Stage::Pending) => Status::Pending,
            Some(Stage::Refused) => Status::Refused,
            Some(Stage::Accepted { .. }) => match s.requests.remove(id).map(|r| r.stage) {
                Some(Stage::Accepted { username, password, .. }) => Status::Accepted { username, password },
                _ => Status::Expired,
            },
        }
    }

    /// Demandes en attente, de la plus récente à la plus ancienne.
    pub fn pending(&self) -> Vec<Pending> {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        let mut list: Vec<Pending> = s
            .requests
            .iter()
            .filter(|(_, r)| matches!(r.stage, Stage::Pending))
            .map(|(id, r)| Pending {
                id: id.clone(),
                code: r.code.clone(),
                name: r.device.name.clone(),
                kind: r.device.kind.clone().unwrap_or_else(|| "other".into()),
                os: r.device.os.clone(),
                app: r.device.app.clone(),
                address: r.peer.to_string(),
                expires_at: r.created_secs + REQUEST_TTL.as_secs(),
            })
            .collect();
        list.sort_by(|a, b| b.expires_at.cmp(&a.expires_at));
        list
    }

    /// Présentation de l'appareil d'une demande en attente.
    pub fn device(&self, id: &str) -> Option<DeviceInfo> {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        s.requests.get(id).filter(|r| matches!(r.stage, Stage::Pending)).map(|r| r.device.clone())
    }

    /// Accepte une demande en attente avec l'accès créé pour elle. Faux si
    /// elle a expiré entre-temps (le compte est alors à supprimer).
    pub fn accept(&self, id: &str, username: String, password: String) -> bool {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        match s.requests.get_mut(id).filter(|r| matches!(r.stage, Stage::Pending)) {
            Some(r) => {
                r.stage = Stage::Accepted { username, password, at: Instant::now() };
                true
            }
            None => false,
        }
    }

    /// Refuse une demande en attente ; rend le nom de l'appareil.
    pub fn refuse(&self, id: &str) -> Option<String> {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        let r = s.requests.get_mut(id).filter(|r| matches!(r.stage, Stage::Pending))?;
        r.stage = Stage::Refused;
        Some(r.device.name.clone())
    }

    /// Comptes d'accès jamais livrés (demande expirée après acceptation).
    pub fn take_orphans(&self) -> Vec<String> {
        let mut s = self.state.lock().unwrap();
        Self::sweep(&mut s, Instant::now());
        std::mem::take(&mut s.orphans)
    }
}

/// Identifiant du compte Kyber d'un appareil appairé : son nom en ASCII
/// (lettres, chiffres, tirets), suivi de quatre caractères tirés au hasard.
pub fn account_name(device_name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for c in device_name.chars() {
        let c = fold(c);
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= 24 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    let base = if slug.is_empty() { "appareil" } else { slug };
    loop {
        let name = format!("{base}-{}", &hex::encode(rand::random::<[u8; 2]>()));
        if !taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
            return name;
        }
    }
}

/// Lettres accentuées courantes vers leur lettre de base.
fn fold(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
        'À' | 'Â' | 'Ä' | 'Á' | 'Ã' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'î' | 'ï' | 'í' | 'ì' => 'i',
        'Î' | 'Ï' | 'Í' | 'Ì' => 'I',
        'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
        'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => 'O',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        '\u{2019}' | '\'' => ' ',
        c => c,
    }
}

/// Mot de passe de l'accès d'un appareil : 128 bits tirés au hasard.
pub fn secret() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tv(id: &str) -> DeviceInfo {
        DeviceInfo { id: id.into(), name: "Apple TV".into(), kind: Some("tv".into()), os: Some("tvOS 26.0".into()), app: None }
    }

    const PEER: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 20));

    #[test]
    fn accepte_et_livre_une_seule_fois() {
        let p = Pairing::new();
        let id = p.request("482913", tv("a"), PEER).unwrap();
        assert_eq!(p.status(&id), Status::Pending);
        let pending = p.pending();
        assert_eq!((pending.len(), pending[0].code.as_str(), pending[0].kind.as_str()), (1, "482913", "tv"));
        assert_eq!(p.device(&id).unwrap().name, "Apple TV");
        assert!(p.accept(&id, "Apple-TV-1234".into(), "secret".into()));
        assert!(p.pending().is_empty());
        assert_eq!(p.status(&id), Status::Accepted { username: "Apple-TV-1234".into(), password: "secret".into() });
        assert_eq!(p.status(&id), Status::Expired, "l'accès n'est livré qu'une fois");
        assert!(!p.accept(&id, "x".into(), "y".into()));
        assert!(p.take_orphans().is_empty());
    }

    #[test]
    fn refus_et_demandes_inconnues() {
        let p = Pairing::new();
        let id = p.request("000001", tv("a"), PEER).unwrap();
        assert_eq!(p.refuse(&id).as_deref(), Some("Apple TV"));
        assert_eq!(p.status(&id), Status::Refused);
        assert_eq!(p.refuse(&id), None);
        assert_eq!(p.status("inconnue"), Status::Expired);
    }

    #[test]
    fn codes_et_presentations_invalides() {
        let p = Pairing::new();
        assert!(matches!(p.request("12345", tv("a"), PEER), Err(RequestError::Invalid(_))));
        assert!(matches!(p.request("12345a", tv("a"), PEER), Err(RequestError::Invalid(_))));
        let mut nameless = tv("a");
        nameless.name = "  ".into();
        assert!(matches!(p.request("123456", nameless, PEER), Err(RequestError::Invalid(_))));
    }

    #[test]
    fn limites_anti_abus() {
        let p = Pairing::new();
        // Le même appareil remplace sa demande.
        p.request("111111", tv("a"), PEER).unwrap();
        p.request("222222", tv("a"), PEER).unwrap();
        assert_eq!(p.pending().len(), 1);
        assert_eq!(p.pending()[0].code, "222222");
        // Deux demandes en attente au plus par adresse.
        p.request("333333", tv("b"), PEER).unwrap();
        assert_eq!(p.request("444444", tv("c"), PEER), Err(RequestError::TooMany));
        // Dix demandes par minute au plus par adresse.
        let other = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 21));
        for i in 0..10 {
            p.request("555555", tv(&format!("x{i}")), other).ok();
        }
        assert_eq!(p.request("555555", tv("y"), other), Err(RequestError::TooMany));
        // Six en attente au plus en tout.
        for i in 0..8u8 {
            p.request("666666", tv(&format!("z{i}")), IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, i))).ok();
        }
        assert_eq!(p.pending().len(), PENDING_TOTAL);
    }

    #[test]
    fn expiration() {
        let p = Pairing::new();
        let id = p.request("482913", tv("a"), PEER).unwrap();
        let accepted = p.request("482914", tv("b"), PEER).unwrap();
        assert!(p.accept(&accepted, "compte-b".into(), "secret".into()));
        {
            let mut s = p.state.lock().unwrap();
            s.requests.get_mut(&id).unwrap().created -= REQUEST_TTL + Duration::from_secs(1);
            if let Stage::Accepted { at, .. } = &mut s.requests.get_mut(&accepted).unwrap().stage {
                *at -= COLLECT_TTL + Duration::from_secs(1);
            }
        }
        assert_eq!(p.status(&id), Status::Expired);
        assert_eq!(p.status(&accepted), Status::Expired);
        assert_eq!(p.take_orphans(), vec!["compte-b".to_string()], "l'accès jamais livré est à supprimer");
    }

    #[test]
    fn noms_de_compte() {
        let name = account_name("Apple TV du salon", &[]);
        assert!(name.starts_with("Apple-TV-du-salon-"), "{name}");
        assert_eq!(name.len(), "Apple-TV-du-salon-".len() + 4);
        assert!(account_name("Télé de l’étage", &[]).starts_with("Tele-de-l-etage-"));
        assert!(account_name("📺", &[]).starts_with("appareil-"));
        assert!(account_name("Une télévision au nom vraiment très long", &[]).len() <= 24 + 5);
    }
}
