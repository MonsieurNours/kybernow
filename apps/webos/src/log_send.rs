//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos ») : un téléviseur n'a pas de feuille de
//! partage, le journal passe par un QR code que le téléphone scanne (docs/relais.md, « Dépôt d'un journal »).
//!
//! Le journal est compressé (gzip) puis chiffré sur le téléviseur (AES-256-GCM, clé tirée au hasard pour ce dépôt),
//! déposé sur le relais du compte KyberNow du téléviseur, sinon le relais public, qui ne peut pas le lire : la clé ne
//! part que dans le QR code, après `#k=`, que le navigateur du téléphone n'envoie jamais. Dépôt : `KNJ1` (4 octets),
//! le nonce (12), puis le texte chiffré et son étiquette (16) ; 2 Mio au plus une fois compressé (au-delà : l'en-tête
//! et la fin du journal, la plus récente).
//!
//! Ici : l'écran (`Screen::LogSend`) et ses étapes, l'en-tête du journal, le dépôt chiffré ; sans le cœur ni le
//! réseau (l'interface se compile aussi pour les captures). Le relais est appelé par `device::log_send`.

use std::io::Write;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;

use crate::app::{Action, App, Screen, Target};
use crate::i18n::{english, tr};

/// Le plus grand journal compressé qu'un dépôt porte.
pub const MAX_COMPRESSED: usize = 2 * 1024 * 1024;
const MAGIC: &[u8; 4] = b"KNJ1";
/// L'écran du QR code relit l'état du dépôt à ce rythme.
pub const CHECK_EVERY: Duration = Duration::from_secs(2);

/// Une étape de l'écran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogStep {
    /// « Préparation du journal… » : lu, compressé, chiffré.
    Preparing,
    /// « Envoi du journal… » : déposé sur le relais.
    Uploading,
    /// Le QR code, jusqu'à ce que le téléphone l'ouvre ou qu'il expire.
    Qr(LogDeposit),
    /// « Journal ouvert sur le téléphone. »
    Opened,
    /// « Ce QR code a expiré. » (une heure).
    Expired,
    Failed(LogFail),
}

/// Un dépôt sur le relais : de quoi le suivre et le retirer (`owner` ne part jamais ailleurs qu'au relais).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogDeposit {
    /// Le relais, tel qu'il a servi (empreinte épinglée comprise).
    pub relay: String,
    pub id: String,
    pub owner: String,
    /// L'adresse du QR code, clé comprise.
    pub url: String,
    pub until: Instant,
}

/// Ce qui empêche le dépôt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFail {
    /// Le relais ne répond pas (réseau, nom, certificat).
    Unreachable,
    /// Relais d'avant le dépôt des journaux (`404`).
    OldRelay,
    /// Trop de dépôts depuis cette adresse (`429`), ou relais plein (`503`).
    TooMany,
}

/// Ce que le relais dit d'un dépôt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogRemote {
    Waiting,
    Opened,
    /// Plus là : ouvert puis effacé, expiré, retiré.
    Gone,
    /// Pas de réponse cette fois : on relira.
    Unknown,
}

/// L'écran ouvert, et le numéro de l'envoi en cours (une réponse d'un envoi d'avant est jetée).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSendUi {
    pub step: LogStep,
    pub attempt: u32,
    pub checked: Option<Instant>,
}

// MARK: - Textes (docs/link-ui.md)

pub fn title() -> &'static str {
    tr("Envoyer le journal", "Send the log")
}

/// L'aide de la ligne des Réglages, celle des téléviseurs.
pub fn help() -> &'static str {
    tr(
        "Si l'app se ferme ou qu'une session se coupe, ce fichier texte raconte ce qui s'est passé. Il passe par votre téléphone : vous choisissez à qui l'envoyer.",
        "If the app closes or a session cuts out, this text file tells what happened. It goes through your phone: you choose who to send it to.",
    )
}

pub fn preparing() -> &'static str {
    tr("Préparation du journal…", "Preparing the log…")
}

pub fn uploading() -> &'static str {
    tr("Envoi du journal…", "Uploading the log…")
}

pub fn scan() -> &'static str {
    tr("Scannez ce QR code avec votre téléphone, puis enregistrez ou partagez le journal.", "Scan this QR code with your phone, then save or share the log.")
}

pub fn privacy() -> &'static str {
    tr("Chiffré sur cet appareil : seul ce QR code l'ouvre. Il sert une fois, pendant une heure.", "Encrypted on this device: only this QR code opens it. It works once, for one hour.")
}

pub fn opened() -> &'static str {
    tr("Journal ouvert sur le téléphone.", "Log opened on the phone.")
}

pub fn expired() -> &'static str {
    tr("Ce QR code a expiré.", "This QR code has expired.")
}

pub fn done() -> &'static str {
    tr("Terminé", "Done")
}

impl LogFail {
    pub fn text(self) -> &'static str {
        match self {
            LogFail::Unreachable => tr("Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez.", "The relay isn't answering: check the internet connection, then try again."),
            LogFail::OldRelay => tr("Ce relais ne prend pas encore les journaux : il doit être mis à jour.", "This relay doesn't take logs yet: it needs updating."),
            LogFail::TooMany => tr("Trop d'envois : réessayez dans une heure.", "Too many uploads: try again in an hour."),
        }
    }

    /// Le relais injoignable se réessaie tout de suite ; les autres refus, non.
    pub fn retry(self) -> bool {
        self == LogFail::Unreachable
    }
}

impl LogStep {
    /// Le second bouton, à côté de « Terminé » : « Nouveau QR code », « Réessayer ».
    pub fn again(&self) -> Option<&'static str> {
        match self {
            LogStep::Expired => Some(tr("Nouveau QR code", "New QR code")),
            LogStep::Failed(f) if f.retry() => Some(tr("Réessayer", "Try again")),
            _ => None,
        }
    }
}

// MARK: - En-tête du journal

/// L'en-tête du journal : la version de l'app, l'appareil et son système, le moment, dans les termes que la page du
/// relais lit (« Appareil : … », « Journal écrit le … » ; “Device: …”, “Log written on …”, dans les 4 premiers Kio).
/// webOS ne dit pas la raison des fins précédentes de l'app : pas de ligne pour elles.
pub fn header(version: &str, model: &str, webos: &str, now_secs: u64) -> String {
    let device = if webos.is_empty() { format!("LG {model} · webOS") } else { format!("LG {model} · webOS {webos}") };
    let at = utc(now_secs);
    if english() {
        format!("KyberNow Link {version}\nDevice: {device}\nLog written on {at}\n\n")
    } else {
        format!("KyberNow Link {version}\nAppareil : {device}\nJournal écrit le {at}\n\n")
    }
}

/// « 2026-11-27 12:00:00.000 UTC ».
fn utc(secs: u64) -> String {
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Jours depuis 1970 → date (calendrier grégorien), d'après l'algorithme « civil from days ».
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.000 UTC", rest / 3600, (rest / 60) % 60, rest % 60)
}

// MARK: - Dépôt chiffré

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    // Écrire dans un Vec ne peut pas échouer.
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}

/// Le journal compressé, tenu dans `limit` : entier s'il tient, sinon l'en-tête, une ligne qui dit la coupure, puis
/// la fin du journal (coupée au début d'une ligne).
fn compress(header: &str, body: &str, limit: usize) -> Vec<u8> {
    let whole = gzip(format!("{header}{body}").as_bytes());
    if whole.len() <= limit {
        return whole;
    }
    let (mut keep, mut compressed) = (body.len(), whole.len());
    loop {
        // Ce que la place permet, d'après le taux de compression du dernier essai.
        keep = ((keep as f64) * (limit as f64 / compressed as f64) * 0.9) as usize;
        let mut start = body.len().saturating_sub(keep);
        while !body.is_char_boundary(start) {
            start += 1;
        }
        if start > 0
            && let Some(nl) = body[start..].find('\n')
        {
            start += nl + 1;
        }
        let tail = &body[start..];
        let cut = if english() { format!("[… start of the log removed: {start} bytes]\n") } else { format!("[… début du journal retiré : {start} octets]\n") };
        let out = gzip(format!("{header}{cut}{tail}").as_bytes());
        if out.len() <= limit || tail.is_empty() {
            return out;
        }
        compressed = out.len();
        keep = tail.len();
    }
}

/// Chiffre un journal déjà compressé avec cette clé et ce nonce : `KNJ1`, le nonce, le texte chiffré et son étiquette.
fn seal_with(plain: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let sealed = cipher.encrypt(Nonce::from_slice(nonce), plain).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Le dépôt chiffré du journal (`header` puis `body`, du plus ancien au plus récent), et sa clé (clé et nonce tirés
/// au hasard).
pub fn seal(header: &str, body: &str) -> Result<(Vec<u8>, [u8; 32]), String> {
    let plain = compress(header, body, MAX_COMPRESSED);
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    Ok((seal_with(&plain, &key, &nonce)?, key))
}

/// L'adresse du QR code : la page du journal chez ce relais, la clé après `#k=`.
pub fn page_url(relay_authority: &str, id: &str, key: &[u8; 32]) -> String {
    format!("https://{relay_authority}/journal/{id}#k={}", URL_SAFE_NO_PAD.encode(key))
}

/// Un identifiant de dépôt lisible : base64url, rien à échapper dans l'adresse.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

// MARK: - L'écran

impl App {
    /// « Envoyer le journal » (Réglages de l'app › À propos), ou « Nouveau QR code », « Réessayer » : un nouvel envoi.
    pub fn log_send(&mut self) -> Vec<Action> {
        self.log_attempt += 1;
        self.log = Some(LogSendUi { step: LogStep::Preparing, attempt: self.log_attempt, checked: None });
        if self.screen != Screen::LogSend {
            self.back_to = self.screen;
        }
        self.go(Screen::LogSend);
        // Le relais du compte KyberNow du téléviseur, sinon le relais public (vide).
        let relay = self.store.accounts.first().map(|a| a.relay.clone()).unwrap_or_default();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let header = header(self.version, &self.tv.model, &self.tv.webos, now);
        vec![Action::LogSend { attempt: self.log_attempt, relay, header }]
    }

    /// Le journal est prêt et part au relais.
    pub fn log_uploading(&mut self, attempt: u32) {
        if let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt && l.step == LogStep::Preparing) {
            l.step = LogStep::Uploading;
        }
    }

    /// Le dépôt est fait, ou refusé.
    pub fn log_sent(&mut self, attempt: u32, r: Result<LogDeposit, LogFail>) -> Vec<Action> {
        let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt) else {
            // L'écran est fermé, ou un autre envoi a commencé : le dépôt ne sert plus.
            return r.map(|d| vec![Action::LogDiscard { relay: d.relay, id: d.id, owner: d.owner }]).unwrap_or_default();
        };
        l.step = match r {
            Ok(d) => LogStep::Qr(d),
            Err(f) => LogStep::Failed(f),
        };
        l.checked = Some(Instant::now());
        self.focus = self.log_focus();
        Vec::new()
    }

    /// L'état du dépôt, relu toutes les 2 s tant que le QR code est montré.
    pub fn log_state(&mut self, attempt: u32, remote: LogRemote) {
        let Some(l) = self.log.as_mut().filter(|l| l.attempt == attempt) else { return };
        if !matches!(l.step, LogStep::Qr(_)) {
            return;
        }
        match remote {
            LogRemote::Opened => l.step = LogStep::Opened,
            LogRemote::Gone => l.step = LogStep::Expired,
            LogRemote::Waiting | LogRemote::Unknown => {}
        }
        self.focus = self.log_focus();
    }

    /// À appeler à chaque tour de la boucle : le dépôt à relire (toutes les 2 s), et l'expiration vue sans le relais.
    pub fn log_tick(&mut self, now: Instant) -> Vec<Action> {
        if self.screen != Screen::LogSend {
            return Vec::new();
        }
        let Some(l) = self.log.as_mut() else { return Vec::new() };
        let LogStep::Qr(d) = &l.step else { return Vec::new() };
        if now >= d.until {
            l.step = LogStep::Expired;
            self.focus = self.log_focus();
            return Vec::new();
        }
        if l.checked.is_some_and(|t| now.duration_since(t) < CHECK_EVERY) {
            return Vec::new();
        }
        l.checked = Some(now);
        vec![Action::LogCheck { attempt: l.attempt, relay: d.relay.clone(), id: d.id.clone(), owner: d.owner.clone() }]
    }

    /// « Terminé », ou Retour : le dépôt pas encore ouvert est retiré, l'écran d'avant revient.
    pub fn log_done(&mut self) -> Vec<Action> {
        let acts = match self.log.take().map(|l| l.step) {
            Some(LogStep::Qr(d)) => vec![Action::LogDiscard { relay: d.relay, id: d.id, owner: d.owner }],
            _ => Vec::new(),
        };
        let back = if self.back_to == Screen::LogSend { Screen::Settings } else { self.back_to };
        self.go(back);
        acts
    }

    /// Le focus de l'écran : le second bouton quand il y en a un, sinon « Terminé ».
    pub fn log_focus(&self) -> Target {
        if self.log.as_ref().is_some_and(|l| l.step.again().is_some()) { Target::LogAgain } else { Target::LogDone }
    }

    pub fn log_activate(&mut self, t: Target) -> Option<Vec<Action>> {
        match t {
            Target::LogDone => Some(self.log_done()),
            Target::LogAgain => Some(self.log_send()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::model::Store;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// Ce que fait la page du téléphone : déchiffre et décompresse.
    fn open(deposit: &[u8], key: &[u8; 32]) -> Option<String> {
        if deposit.len() < 4 + 12 + 16 || &deposit[..4] != MAGIC {
            return None;
        }
        let plain = Aes256Gcm::new_from_slice(key).ok()?.decrypt(Nonce::from_slice(&deposit[4..16]), &deposit[16..]).ok()?;
        let mut s = String::new();
        flate2::read::GzDecoder::new(plain.as_slice()).read_to_string(&mut s).ok()?;
        Some(s)
    }

    /// Le vecteur d'essai de docs/relais.md (« Vecteurs d'essai KNJ1 ») : le chiffrement de l'entrée donnée.
    #[test]
    fn knj1_test_vector() {
        let key: [u8; 32] = hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f").try_into().unwrap();
        let nonce: [u8; 12] = hex("a0a1a2a3a4a5a6a7a8a9aaab").try_into().unwrap();
        let input = hex("1f8b08000000000002fff3ae4c4a2df2cb2f57f0c9cccb5630d03334d4b3e0722c28482c4acdcc51b052702d2e4ecc54f0f6f332e4f2ca2f2dca4bcc5138bc32b928b344212755c1c8c0c84cd7d050d7c85cc1d0c8cac00088f40c0c0c1442439cb99cf2f3b2801af4b800b1b88fb162000000");
        let out = hex("4b4e4a31a0a1a2a3a4a5a6a7a8a9aaabf993742d45cb02bf609a747d4b30ed2cbb830ee05b7b893aacde15b2ab189573fe5e0fd3e5ef9f6cefce74e527344fadb7edb57a8622d0516c944092f54839820d94362b1182b1213c223754581e9c6d05576a740d0b597a66813296bd8e0fd7133db759f01c0f816a6306e13d075c4a2450614699bf3b2059e75ad4459d934d121ce0");
        assert_eq!(seal_with(&input, &key, &nonce).unwrap(), out);
        assert_eq!(URL_SAFE_NO_PAD.encode(key), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8");
        assert_eq!(open(&out, &key).unwrap(), "KyberNow Link 0.11.8\nAppareil : Essai KNJ1\nJournal écrit le 2026-11-27 12:00:00.000 UTC\nBonjour.\n");
    }

    #[test]
    fn deposit_round_trip_and_url() {
        let h = header("0.11.8", "OLED55G26LA", "23.20", 1_795_780_800);
        assert!(h.contains("Appareil : LG OLED55G26LA · webOS 23.20\nJournal écrit le 2026-11-27 12:00:00.000 UTC\n"), "{h}");
        let (deposit, key) = seal(&h, "ligne 1\nligne 2\n").unwrap();
        assert_eq!(open(&deposit, &key).unwrap(), format!("{h}ligne 1\nligne 2\n"));
        let url = page_url("share.kybernow.app", "AbC_d-9", &key);
        assert!(url.starts_with("https://share.kybernow.app/journal/AbC_d-9#k=") && url.len() == "https://share.kybernow.app/journal/AbC_d-9#k=".len() + 43);
        assert!(valid_id("AbC_d-9") && !valid_id("a/b") && !valid_id(""));
    }

    /// Trop gros : l'en-tête, la coupure dite, puis la fin du journal (les lignes les plus récentes) ; dans la limite.
    #[test]
    fn too_big_keeps_header_and_tail() {
        let mut body = String::new();
        let mut n: u64 = 1;
        for i in 0..40_000 {
            // Des lignes qui se compressent mal : un nombre pseudo-aléatoire chacune.
            n = n.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            body.push_str(&format!("{i} {n:x}{:x}\n", n.rotate_left(17)));
        }
        let limit = 200_000;
        let out = compress("EN-TÊTE\n", &body, limit);
        assert!(out.len() <= limit);
        let mut s = String::new();
        flate2::read::GzDecoder::new(out.as_slice()).read_to_string(&mut s).unwrap();
        assert!(s.starts_with("EN-TÊTE\n[… début du journal retiré : "));
        assert!(s.ends_with(&format!("{}\n", body.lines().last().unwrap())));
    }

    #[test]
    fn screen_steps() {
        let mut app = App::new(Store::default());
        app.screen = Screen::Settings;
        let acts = app.log_send();
        assert!(matches!(acts.as_slice(), [Action::LogSend { attempt: 1, relay, .. }] if relay.is_empty()));
        assert_eq!(app.screen, Screen::LogSend);
        app.log_uploading(1);
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Uploading);
        // Relais d'avant : le texte, « Terminé » seul.
        app.log_sent(1, Err(LogFail::OldRelay));
        assert_eq!(app.log_focus(), Target::LogDone);
        // Injoignable : « Réessayer » relance un envoi.
        app.log_sent(1, Err(LogFail::Unreachable));
        assert_eq!(app.log_focus(), Target::LogAgain);
        let acts = app.activate(Target::LogAgain);
        assert!(matches!(acts.as_slice(), [Action::LogSend { attempt: 2, .. }]));
        // Une réponse de l'envoi d'avant est jetée (et son dépôt retiré).
        let d = |id: &str| LogDeposit { relay: String::new(), id: id.into(), owner: "o".into(), url: "u".into(), until: Instant::now() + Duration::from_secs(3600) };
        assert!(matches!(app.log_sent(1, Ok(d("vieux"))).as_slice(), [Action::LogDiscard { id, .. }] if id == "vieux"));
        app.log_sent(2, Ok(d("x")));
        assert!(matches!(app.log.as_ref().unwrap().step, LogStep::Qr(_)));
        // Relu toutes les 2 s.
        let now = Instant::now() + Duration::from_secs(3);
        assert!(matches!(app.log_tick(now).as_slice(), [Action::LogCheck { attempt: 2, .. }]));
        assert!(app.log_tick(now).is_empty());
        app.log_state(2, LogRemote::Opened);
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Opened);
        // Ouvert : « Terminé » ne retire rien et rend les Réglages.
        assert!(app.activate(Target::LogDone).is_empty());
        assert_eq!(app.screen, Screen::Settings);
        // QR code montré puis « Terminé » : le dépôt est retiré.
        app.log_send();
        app.log_sent(3, Ok(d("y")));
        assert!(matches!(app.log_done().as_slice(), [Action::LogDiscard { id, .. }] if id == "y"));
        // Expiré sans le relais, au bout de l'heure.
        app.log_send();
        app.log_sent(4, Ok(LogDeposit { until: Instant::now(), ..d("z") }));
        app.log_tick(Instant::now() + Duration::from_secs(1));
        assert_eq!(app.log.as_ref().unwrap().step, LogStep::Expired);
        assert_eq!(app.log_focus(), Target::LogAgain);
    }
}
