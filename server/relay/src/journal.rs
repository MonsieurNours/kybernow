//! Dépôt d'un journal (docs/relais.md, « Dépôt d'un journal ») : un téléviseur
//! ou Link OS dépose son journal **déjà chiffré** (format `KNJ1`), le téléphone
//! qui scanne le QR code le reprend une fois.
//!
//! Le relais ne voit jamais le journal en clair : la clé n'existe que dans le
//! fragment de l'adresse du QR code (`#k=…`), que le navigateur n'envoie pas.
//! Il garde les dépôts en mémoire seulement, une heure au plus, et ne met dans
//! ses journaux ni leur identifiant ni leur jeton.
//!
//! Format `KNJ1` : `KNJ1` (4 octets), le nonce (12 octets), puis le journal
//! compressé en gzip et chiffré en AES-256-GCM (sans données associées), avec
//! son étiquette de 16 octets. Le relais vérifie seulement la marque et la taille.

use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bytes::Bytes;
use hyper::body::{Body, Frame, SizeHint};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};

use crate::limits::Buckets;

/// La marque d'un dépôt.
pub const MAGIC: &[u8; 4] = b"KNJ1";
/// Le nonce d'AES-GCM.
pub const NONCE_LEN: usize = 12;
/// L'étiquette d'AES-GCM.
pub const TAG_LEN: usize = 16;
/// Le plus petit dépôt : la marque, le nonce et l'étiquette.
pub const MIN_BYTES: usize = MAGIC.len() + NONCE_LEN + TAG_LEN;
/// Le journal compressé, au plus (link-ui : 2 Mio une fois compressé).
pub const MAX_JOURNAL: usize = 2 * 1024 * 1024;
/// Un dépôt, au plus : le journal et 64 octets (marque, nonce, étiquette).
pub const MAX_BYTES: usize = MAX_JOURNAL + 64;
/// Un dépôt vit une heure.
pub const TTL: Duration = Duration::from_secs(3600);
/// Dépôts par heure et par adresse (un /64 en IPv6).
pub const PER_ADDRESS: u32 = 6;
/// Tous les dépôts gardés, au plus (au-delà, `503 full`).
pub const MAX_TOTAL: usize = 256 * 1024 * 1024;
/// Dépôts gardés en même temps, au plus (les ouverts comptent jusqu'à leur échéance).
pub const MAX_DEPOSITS: usize = 4096;
/// Le corps d'un dépôt arrive dans ce temps (2 Mio sur un lien montant lent).
pub const UPLOAD_TIMEOUT: Duration = Duration::from_secs(60);
/// L'envoi au téléphone, par morceaux.
const CHUNK: usize = 64 * 1024;

/// Un identifiant ou un jeton : 16 octets en base64url (22 signes).
pub fn is_token(s: &str) -> bool {
    s.len() == 22 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Un refus de dépôt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Plus de [`MAX_BYTES`] (413).
    TooLarge,
    /// Pas un dépôt `KNJ1` (400).
    BadRequest,
    /// Le relais garde déjà tout ce qu'il peut (503).
    Full,
}

/// Ce que voit l'appareil qui a déposé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Waiting,
    Opened,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Waiting => "waiting",
            State::Opened => "opened",
        }
    }
}

struct Deposit {
    /// Le dépôt, jusqu'à son ouverture.
    data: Option<Bytes>,
    /// SHA-256 du jeton de l'appareil.
    owner: [u8; 32],
    until: Instant,
}

struct Inner {
    deposits: HashMap<String, Deposit>,
    /// Octets gardés (dépôts pas encore ouverts).
    bytes: usize,
    per_address: Buckets<IpAddr>,
}

/// Les dépôts du relais, en mémoire.
pub struct Journals {
    inner: Mutex<Inner>,
    rng: SystemRandom,
}

fn digest32(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(digest::digest(&digest::SHA256, s.as_bytes()).as_ref());
    out
}

/// Égalité en temps constant (deux condensés de même taille).
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Le dépôt a-t-il la forme `KNJ1` ?
pub fn check(body: &[u8]) -> Result<(), Refused> {
    if body.len() > MAX_BYTES {
        return Err(Refused::TooLarge);
    }
    if body.len() < MIN_BYTES || !body.starts_with(MAGIC) {
        return Err(Refused::BadRequest);
    }
    Ok(())
}

impl Default for Journals {
    fn default() -> Self {
        Journals::new()
    }
}

impl Journals {
    pub fn new() -> Journals {
        Journals {
            inner: Mutex::new(Inner { deposits: HashMap::new(), bytes: 0, per_address: Buckets::new(PER_ADDRESS, TTL) }),
            rng: SystemRandom::new(),
        }
    }

    fn token(&self) -> String {
        let mut raw = [0u8; 16];
        self.rng.fill(&mut raw).expect("hasard du système");
        URL_SAFE_NO_PAD.encode(raw)
    }

    /// Un dépôt de plus depuis cette adresse (déjà passée par `hub::ip_key`) ? `Err` : réessayer dans ce temps.
    pub fn admit(&self, ip: IpAddr, now: Instant) -> Result<(), Duration> {
        self.inner.lock().unwrap().per_address.take(&ip, now)
    }

    /// Rend le dépôt compté à cette adresse (le relais était plein : ce n'est pas sa faute).
    pub fn refund(&self, ip: IpAddr, now: Instant) {
        self.inner.lock().unwrap().per_address.refund(&ip, now);
    }

    /// Garde un dépôt ; rend son identifiant et le jeton de l'appareil.
    pub fn put(&self, body: Bytes, now: Instant) -> Result<(String, String), Refused> {
        check(&body)?;
        let (id, owner) = (self.token(), self.token());
        let mut inner = self.inner.lock().unwrap();
        Self::expire(&mut inner, now);
        if inner.bytes + body.len() > MAX_TOTAL || inner.deposits.len() >= MAX_DEPOSITS {
            return Err(Refused::Full);
        }
        inner.bytes += body.len();
        inner.deposits.insert(id.clone(), Deposit { data: Some(body), owner: digest32(&owner), until: now + TTL });
        Ok((id, owner))
    }

    /// Le dépôt à envoyer au téléphone, s'il est encore là. Il est effacé quand le dernier octet
    /// est parti ; un envoi interrompu le laisse à qui le redemande (dans l'heure).
    pub fn open(self: &Arc<Self>, id: &str, now: Instant) -> Option<JournalBody> {
        let inner = self.inner.lock().unwrap();
        let d = inner.deposits.get(id).filter(|d| d.until > now)?;
        let data = d.data.clone()?;
        Some(JournalBody { data, sent: 0, id: id.to_string(), store: Some(self.clone()) })
    }

    /// Le dernier octet est parti : le dépôt est oublié, son état devient « ouvert ».
    fn opened(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let freed = inner.deposits.get_mut(id).and_then(|d| d.data.take()).map_or(0, |b| b.len());
        inner.bytes -= freed;
        if freed > 0 {
            tracing::info!(taille = freed, "journal ouvert par le téléphone");
        }
    }

    /// L'état vu par l'appareil (`x-kn-owner`) ; `None` : inconnu, expiré, ou pas son jeton.
    pub fn state(&self, id: &str, owner: &str, now: Instant) -> Option<State> {
        let inner = self.inner.lock().unwrap();
        let d = inner.deposits.get(id).filter(|d| d.until > now && same(&d.owner, &digest32(owner)))?;
        Some(if d.data.is_some() { State::Waiting } else { State::Opened })
    }

    /// Retire un dépôt (« Terminé » sur l'appareil) ; faux : inconnu, expiré, ou pas son jeton.
    pub fn remove(&self, id: &str, owner: &str, now: Instant) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if !inner.deposits.get(id).is_some_and(|d| d.until > now && same(&d.owner, &digest32(owner))) {
            return false;
        }
        let freed = inner.deposits.remove(id).and_then(|d| d.data).map_or(0, |b| b.len());
        inner.bytes -= freed;
        true
    }

    fn expire(inner: &mut Inner, now: Instant) {
        let mut freed = 0;
        inner.deposits.retain(|_, d| {
            let keep = d.until > now;
            if !keep {
                freed += d.data.as_ref().map_or(0, Bytes::len);
            }
            keep
        });
        inner.bytes -= freed;
    }

    /// Oublie les dépôts expirés et les adresses revenues au calme.
    pub fn sweep(&self, now: Instant) {
        let mut inner = self.inner.lock().unwrap();
        Self::expire(&mut inner, now);
        inner.per_address.sweep(now);
    }

    /// Dépôts gardés et leurs octets (métriques).
    pub fn usage(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap();
        (inner.deposits.values().filter(|d| d.data.is_some()).count(), inner.bytes)
    }
}

/// Le corps de la réponse au téléphone : le dépôt par morceaux de 64 Kio ; le dernier
/// morceau parti, le dépôt est oublié.
pub struct JournalBody {
    data: Bytes,
    sent: usize,
    id: String,
    store: Option<Arc<Journals>>,
}

impl JournalBody {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Body for JournalBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        if self.sent >= self.data.len() {
            if let Some(store) = self.store.take() {
                store.opened(&self.id);
            }
            return Poll::Ready(None);
        }
        let end = (self.sent + CHUNK).min(self.data.len());
        let chunk = self.data.slice(self.sent..end);
        self.sent = end;
        if end == self.data.len()
            && let Some(store) = self.store.take()
        {
            store.opened(&self.id);
        }
        Poll::Ready(Some(Ok(Frame::data(chunk))))
    }

    fn is_end_stream(&self) -> bool {
        self.sent >= self.data.len() && self.store.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact((self.data.len() - self.sent) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    fn deposit(len: usize) -> Bytes {
        let mut v = MAGIC.to_vec();
        v.resize(len, 7);
        Bytes::from(v)
    }

    #[test]
    fn forme_et_taille() {
        assert_eq!(check(&deposit(MIN_BYTES)), Ok(()));
        assert_eq!(check(&deposit(MAX_BYTES)), Ok(()));
        assert_eq!(check(&deposit(MAX_BYTES + 1)), Err(Refused::TooLarge));
        assert_eq!(check(&deposit(MIN_BYTES - 1)), Err(Refused::BadRequest));
        assert_eq!(check(b"KNJ2xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"), Err(Refused::BadRequest));
        assert!(is_token("AAAAAAAAAAAAAAAAAAAAAA") && is_token("a-_9a-_9a-_9a-_9a-_9a-") && !is_token("AAAA") && !is_token("AAAAAAAAAAAAAAAAAAAAA="));
    }

    #[tokio::test]
    async fn un_seul_envoi_puis_ouvert() {
        let store = Arc::new(Journals::new());
        let now = Instant::now();
        let body = deposit(200_000);
        let (id, owner) = store.put(body.clone(), now).unwrap();
        assert!(is_token(&id) && is_token(&owner) && id != owner);
        assert_eq!(store.state(&id, &owner, now), Some(State::Waiting));
        assert_eq!(store.state(&id, "AAAAAAAAAAAAAAAAAAAAAA", now), None, "pas son jeton");
        assert_eq!(store.usage(), (1, 200_000));

        // Un envoi interrompu (le téléphone a perdu le réseau) laisse le dépôt.
        let mut partial = store.open(&id, now).unwrap();
        let first = partial.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(first.len(), CHUNK);
        drop(partial);
        assert_eq!(store.state(&id, &owner, now), Some(State::Waiting));

        // Un envoi entier : le même dépôt, puis plus rien.
        let got = store.open(&id, now).unwrap().collect().await.unwrap().to_bytes();
        assert_eq!(got, body);
        assert_eq!(store.state(&id, &owner, now), Some(State::Opened));
        assert!(store.open(&id, now).is_none(), "usage unique");
        assert_eq!(store.usage(), (0, 0));
        // « Terminé » après l'ouverture : l'état s'efface.
        assert!(!store.remove(&id, "AAAAAAAAAAAAAAAAAAAAAA", now));
        assert!(store.remove(&id, &owner, now));
        assert_eq!(store.state(&id, &owner, now), None);
    }

    #[test]
    fn une_heure_puis_rien() {
        let store = Arc::new(Journals::new());
        let now = Instant::now();
        let (id, owner) = store.put(deposit(1000), now).unwrap();
        let later = now + TTL + Duration::from_secs(1);
        assert!(store.open(&id, later).is_none());
        assert_eq!(store.state(&id, &owner, later), None);
        store.sweep(later);
        assert_eq!(store.usage(), (0, 0));
        // Retiré avant l'ouverture : la place est rendue.
        let (id, owner) = store.put(deposit(1000), now).unwrap();
        assert!(store.remove(&id, &owner, now));
        assert!(store.open(&id, now).is_none());
        assert_eq!(store.usage(), (0, 0));
    }

    #[test]
    fn six_par_heure_et_par_adresse() {
        let store = Journals::new();
        let now = Instant::now();
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        for _ in 0..PER_ADDRESS {
            store.admit(ip, now).unwrap();
        }
        let wait = store.admit(ip, now).unwrap_err();
        assert!(wait > Duration::from_secs(500) && wait <= Duration::from_secs(600), "{wait:?}");
        assert!(store.admit("203.0.113.6".parse().unwrap(), now).is_ok(), "une autre adresse");
        store.refund(ip, now);
        assert!(store.admit(ip, now).is_ok());
    }

    #[test]
    fn plafond_de_tout_le_relais() {
        let store = Journals::new();
        let now = Instant::now();
        // Un même tampon (cloner `Bytes` ne copie rien) : le compte se fait sur la taille.
        let big = deposit(MAX_BYTES);
        for _ in 0..MAX_TOTAL / MAX_BYTES {
            store.put(big.clone(), now).unwrap();
        }
        assert_eq!(store.put(big.clone(), now), Err(Refused::Full));
        store.sweep(now + TTL);
        assert!(store.put(big, now + TTL).is_ok(), "place rendue à l'échéance");
    }

    /// Le vecteur d'essai de docs/relais.md (« Vecteurs d'essai KNJ1 ») : AES-256-GCM de ring sur
    /// l'entrée gzip donnée, avec la clé et le nonce donnés, rend exactement la sortie donnée.
    #[test]
    fn vecteur_d_essai_knj1() {
        use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
        let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
        let key: Vec<u8> = (0u8..32).collect();
        let nonce: Vec<u8> = (0xa0u8..0xac).collect();
        let gzip = hex(KNJ1_GZIP);
        let sealed = {
            let k = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key).unwrap());
            let mut buf = gzip.clone();
            k.seal_in_place_append_tag(Nonce::try_assume_unique_for_key(&nonce).unwrap(), Aad::empty(), &mut buf).unwrap();
            buf
        };
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        assert_eq!(out, hex(KNJ1_OUT));
        assert_eq!(check(&out), Ok(()));
        assert_eq!(URL_SAFE_NO_PAD.encode(&key), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8");
        // Et dans l'autre sens, comme la page du téléphone.
        let k = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key).unwrap());
        let mut inner = out[MAGIC.len() + NONCE_LEN..].to_vec();
        let plain = k.open_in_place(Nonce::try_assume_unique_for_key(&out[4..16]).unwrap(), Aad::empty(), &mut inner).unwrap();
        assert_eq!(plain, &gzip[..]);
    }

    /// Entrée du vecteur : le journal de docs/relais.md compressé en gzip (octets fixes).
    pub const KNJ1_GZIP: &str = "1f8b08000000000002fff3ae4c4a2df2cb2f57f0c9cccb5630d03334d4b3e0722c28482c4acdcc51b052702d2e4ecc54f0f6f332e4f2ca2f2dca4bcc5138bc32b928b344212755c1c8c0c84cd7d050d7c85cc1d0c8cac00088f40c0c0c1442439cb99cf2f3b2801af4b800b1b88fb162000000";
    /// Sortie du vecteur : le dépôt `KNJ1` entier (147 octets).
    pub const KNJ1_OUT: &str = "4b4e4a31a0a1a2a3a4a5a6a7a8a9aaabf993742d45cb02bf609a747d4b30ed2cbb830ee05b7b893aacde15b2ab189573fe5e0fd3e5ef9f6cefce74e527344fadb7edb57a8622d0516c944092f54839820d94362b1182b1213c223754581e9c6d05576a740d0b597a66813296bd8e0fd7133db759f01c0f816a6306e13d075c4a2450614699bf3b2059e75ad4459d934d121ce0";
}
