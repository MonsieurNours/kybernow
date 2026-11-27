//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos ») côté appareil.
//!
//! - Le fichier, `kybernow-journal.txt` : l'appareil et son système, la version de l'app, la fin du lancement
//!   précédent (`kybernow.log.1` : sa fin, et s'il s'est arrêté normalement), puis le journal de l'app. Ni mot de
//!   passe, ni jeton, ni code : le journal de l'app n'en écrit pas.
//! - **Au bureau** : la fenêtre d'un e-mail, le fichier joint (`xdg-email --attach` ; dans le Flatpak, celui du runtime
//!   passe par le portail e-mail) ; sans app d'e-mail, la fenêtre d'enregistrement (zenity ou kdialog), Téléchargements
//!   proposé ; sans elle, enregistré dans Téléchargements.
//! - **Par QR code** (Steam Deck en mode Jeu, salon, KyberNow Link OS ; docs/relais.md, « Dépôt d'un journal ») :
//!   compressé (gzip, 2 Mio au plus : l'en-tête et la fin du journal, la plus récente), chiffré en AES-256-GCM avec une
//!   clé tirée pour ce dépôt (`KNJ1`, le nonce, le texte chiffré et son étiquette), déposé sur le relais du compte de
//!   l'appareil (sinon le relais public) ; le QR code porte `https://<relais>/journal/<id>#k=<clé>`, la clé que le
//!   téléphone ne renvoie jamais. L'état du dépôt est relu toutes les 2 s (ouvert, expiré) ; « Terminé » retire un
//!   dépôt pas encore ouvert.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;

use super::Event;
use crate::app::App;
use crate::journal::{JournalDone, JournalStep, FILE_NAME};

/// Ce que le fil du journal dit à la boucle.
pub enum JournalEvent {
    /// Par QR code : l'étape suivante.
    Step(JournalStep),
    /// Au bureau : fait (e-mail, enregistré, échec).
    Done(JournalDone),
}

/// Le journal compressé, au plus (docs/link-ui.md).
pub const MAX_COMPRESSED: usize = 2 * 1024 * 1024;
/// L'en-tête du dépôt chiffré (docs/relais.md).
pub const MAGIC: &[u8; 4] = b"KNJ1";
/// L'état du dépôt est relu à ce rythme.
const POLL: Duration = Duration::from_secs(2);

/// L'envoi en cours : arrêté (et le dépôt pas encore ouvert retiré) par « Terminé » ou par un nouvel envoi.
static CURRENT: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// « Envoyer le journal » : l'en-tête est fait ici (il lit l'app), le reste sur un fil.
pub fn start(app: &App, tx: &Sender<Event>) {
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(old) = CURRENT.lock().unwrap().replace(stop.clone()) {
        old.store(true, Ordering::Relaxed);
    }
    let header = header(app);
    let qr = app.journal_by_qr();
    let relay = relay_of(app);
    let version = app.version;
    let tx = tx.clone();
    let send = move |e: JournalEvent| {
        let _ = tx.send(Event::Journal(e));
    };
    std::thread::Builder::new()
        .name("kybernow-journal".into())
        .spawn(move || {
            let text = format!("{header}{}", logs());
            if qr {
                by_qr(&text, &relay, &stop, &send);
            } else {
                send(JournalEvent::Done(by_email(&text, &version)));
            }
        })
        .ok();
}

/// « Terminé » : l'envoi s'arrête ; le dépôt pas encore ouvert est retiré par son fil.
pub fn close() {
    if let Some(s) = CURRENT.lock().unwrap().take() {
        s.store(true, Ordering::Relaxed);
    }
}

/// L'en-tête du journal : l'appareil, son système, la version de l'app.
fn header(app: &App) -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "Linux".into());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| s.trim().to_string()).unwrap_or_default();
    let mut h = format!("KyberNow Link {}\nAppareil : {}\nSystème : {os} · noyau {kernel}\nInterface : {:?}{}\n", app.version, app.device_name, app.layout, if crate::app::flatpak() { " · Flatpak" } else { "" });
    if let Some(o) = &app.os {
        h.push_str(&format!("KyberNow Link OS {} · emplacement {}\n", o.version, o.slot));
    }
    h.push_str(&format!("Date : {}\n\n", crate::model::now_secs()));
    h
}

/// La fin du lancement précédent et son arrêt, puis le journal de ce lancement.
fn logs() -> String {
    let dir = crate::model::Store::dir();
    let mut out = String::new();
    if let Ok(prev) = std::fs::read_to_string(dir.join("kybernow.log.1")) {
        let lines: Vec<&str> = prev.lines().collect();
        let clean = lines.iter().rev().take(50).any(|l| l.contains(super::log_file::STOPPED));
        out.push_str(&format!("== Lancement précédent : {} ==\n", if clean { "arrêt normal" } else { "fin inattendue (pas d'arrêt écrit au journal)" }));
        for l in &lines[lines.len().saturating_sub(300)..] {
            out.push_str(l);
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str("== Ce lancement ==\n");
    out.push_str(&std::fs::read_to_string(dir.join("kybernow.log")).unwrap_or_default());
    out
}

/// Le relais du dépôt : celui du compte KyberNow de l'appareil, sinon le relais public (`hôte[:port]`, sans
/// l'empreinte épinglée).
fn relay_of(app: &App) -> String {
    app.store.accounts.first().and_then(|a| kybernow_core::rules::relay::parse(&a.relay)).map(|r| r.web_authority()).unwrap_or_else(|| kybernow_core::rules::relay::DEFAULT_RELAY.to_string())
}

/// Compressé en gzip.
fn gzip(text: &str) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = e.write_all(text.as_bytes());
    e.finish().unwrap_or_default()
}

/// Le journal compressé, `max` octets au plus : tout s'il tient ; sinon l'en-tête et la fin la plus récente du journal
/// (la plus longue qui tient).
pub fn compressed(header: &str, body: &str, max: usize) -> Vec<u8> {
    let all = gzip(&format!("{header}{body}"));
    if all.len() <= max {
        return all;
    }
    let cut = "[… début du journal retiré : 2 Mio compressés au plus …]\n";
    let fits = |start: usize| {
        let gz = gzip(&format!("{header}{cut}{}", &body[start..]));
        (gz.len() <= max).then_some(gz)
    };
    // Le plus petit début (le plus de journal) qui tient, aux limites des caractères.
    let (mut lo, mut hi) = (0usize, body.len());
    let mut best = fits(body.len()).unwrap_or_default();
    while lo < hi {
        let mut mid = lo + (hi - lo) / 2;
        while !body.is_char_boundary(mid) {
            mid += 1;
        }
        match fits(mid) {
            Some(gz) => {
                best = gz;
                hi = mid;
            }
            None => lo = mid + 1,
        }
    }
    best
}

/// Le dépôt chiffré : `KNJ1`, le nonce (12 octets), le texte chiffré et son étiquette (16 octets), AES-256-GCM.
pub fn seal(data: &[u8], key: &[u8; 32], nonce: &[u8; 12]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(key.into());
    let sealed = cipher.encrypt(Nonce::from_slice(nonce), data).unwrap_or_default();
    let mut out = Vec::with_capacity(4 + 12 + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(nonce);
    out.extend_from_slice(&sealed);
    out
}

/// L'adresse du QR code : la clé après `#k=`, en base64url (43 signes), que le navigateur n'envoie jamais.
pub fn qr_url(relay: &str, id: &str, key: &[u8; 32]) -> String {
    format!("https://{relay}/journal/{id}#k={}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key))
}

/// Un dépôt fait (`201`) : son identifiant, le jeton de son propriétaire, sa durée.
#[derive(Debug, PartialEq)]
pub struct Deposit {
    pub id: String,
    pub owner: String,
    pub expires_in: u64,
}

/// La réponse du relais au dépôt (docs/relais.md) : `201` et le dépôt, ou l'étape à montrer.
pub fn deposit_of(status: u16, body: &str) -> Result<Deposit, JournalStep> {
    match status {
        201 => {
            let v: serde_json::Value = serde_json::from_str(body).map_err(|_| JournalStep::RelayDown)?;
            let s = |k: &str| v[k].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            match (s("id"), s("owner")) {
                (Some(id), Some(owner)) => Ok(Deposit { id, owner, expires_in: v["expires_in"].as_u64().unwrap_or(3600) }),
                _ => Err(JournalStep::RelayDown),
            }
        }
        // Un relais d'avant le dépôt.
        404 => Err(JournalStep::OldRelay),
        429 => Err(JournalStep::TooMany),
        _ => Err(JournalStep::RelayDown),
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(8)).timeout(Duration::from_secs(60)).build()
}

/// Le statut et le corps d'une réponse, d'erreur comprise ; `None` : le relais ne répond pas.
fn answer(r: Result<ureq::Response, ureq::Error>) -> Option<(u16, String)> {
    match r {
        Ok(r) => Some((r.status(), r.into_string().unwrap_or_default())),
        Err(ureq::Error::Status(code, r)) => Some((code, r.into_string().unwrap_or_default())),
        Err(_) => None,
    }
}

/// Par QR code : compressé, chiffré, déposé ; puis l'état relu jusqu'à l'ouverture, l'expiration ou « Terminé ».
fn by_qr(text: &str, relay: &str, stop: &AtomicBool, send: &dyn Fn(JournalEvent)) {
    let (header, body) = text.split_at(text.find("\n\n").map_or(0, |i| i + 2));
    let gz = compressed(header, body, MAX_COMPRESSED);
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    if getrandom::getrandom(&mut key).is_err() || getrandom::getrandom(&mut nonce).is_err() {
        send(JournalEvent::Step(JournalStep::RelayDown));
        return;
    }
    let sealed = seal(&gz, &key, &nonce);
    if stop.load(Ordering::Relaxed) {
        return;
    }
    send(JournalEvent::Step(JournalStep::Uploading));
    let agent = agent();
    let base = format!("https://{relay}");
    let r = agent.post(&format!("{base}/v1/journal")).set("content-type", "application/octet-stream").send_bytes(&sealed);
    let deposit = match answer(r).map(|(status, body)| deposit_of(status, &body)) {
        Some(Ok(d)) => d,
        Some(Err(step)) => {
            log::info!("journal : dépôt refusé par {relay} ({step:?})");
            send(JournalEvent::Step(step));
            return;
        }
        None => {
            log::info!("journal : {relay} ne répond pas");
            send(JournalEvent::Step(JournalStep::RelayDown));
            return;
        }
    };
    log::info!("journal : déposé sur {relay} ({} octets chiffrés)", sealed.len());
    send(JournalEvent::Step(JournalStep::Qr { url: qr_url(relay, &deposit.id, &key) }));
    let state_url = format!("{base}/v1/journal/{}/state", deposit.id);
    let end = Instant::now() + Duration::from_secs(deposit.expires_in);
    loop {
        // « Terminé » : retiré s'il n'a pas été ouvert (sans effet sinon : le relais l'a déjà effacé).
        if stop.load(Ordering::Relaxed) {
            let _ = agent.delete(&format!("{base}/v1/journal/{}", deposit.id)).set("x-kn-owner", &deposit.owner).call();
            return;
        }
        if Instant::now() >= end {
            send(JournalEvent::Step(JournalStep::Expired));
            return;
        }
        match answer(agent.get(&state_url).set("x-kn-owner", &deposit.owner).call()) {
            Some((200, body)) if body.contains("\"opened\"") => {
                send(JournalEvent::Step(JournalStep::Opened));
                return;
            }
            Some((404, _)) => {
                send(JournalEvent::Step(JournalStep::Expired));
                return;
            }
            _ => {}
        }
        // Attendre 2 s, en voyant « Terminé ».
        let wait = Instant::now() + POLL;
        while Instant::now() < wait && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Au bureau : la fenêtre d'un e-mail, le fichier joint ; sinon la fenêtre d'enregistrement, Téléchargements proposé ;
/// sinon enregistré dans Téléchargements.
fn by_email(text: &str, version: &str) -> JournalDone {
    let dir = std::env::temp_dir().join(format!("kybernow-journal-{}", std::process::id()));
    let path = dir.join(FILE_NAME);
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, text)) {
        return JournalDone::Failed(e.to_string());
    }
    let subject = crate::journal::subject(version);
    let mailed = std::process::Command::new("xdg-email").arg("--subject").arg(&subject).arg("--attach").arg(&path).status().is_ok_and(|s| s.success());
    if mailed {
        log::info!("journal : fenêtre d'e-mail ouverte");
        return JournalDone::Email;
    }
    log::info!("journal : pas d'app d'e-mail, enregistrement");
    let downloads = downloads_dir();
    let proposed = downloads.join(FILE_NAME);
    // La fenêtre d'enregistrement du bureau (GNOME : zenity ; KDE : kdialog) ; annulée : rien n'est enregistré.
    let chosen = save_dialog("zenity", &["--file-selection", "--save", "--confirm-overwrite", &format!("--filename={}", proposed.display())])
        .or_else(|| save_dialog("kdialog", &["--getsavefilename", &proposed.display().to_string()]));
    let target = match chosen {
        Some(Ok(p)) => p,
        Some(Err(())) => return JournalDone::Failed(crate::i18n::tr("enregistrement annulé", "save cancelled").into()),
        None => proposed,
    };
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::copy(&path, &target) {
        Ok(_) => JournalDone::Saved(target.display().to_string()),
        Err(e) => JournalDone::Failed(e.to_string()),
    }
}

/// Une fenêtre d'enregistrement : `None` si l'outil manque, `Err` si elle est annulée.
fn save_dialog(tool: &str, args: &[&str]) -> Option<Result<std::path::PathBuf, ()>> {
    let out = std::process::Command::new(tool).args(args).output().ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(if out.status.success() && !p.is_empty() { Ok(p.into()) } else { Err(()) })
}

/// Le dossier Téléchargements (`xdg-user-dir`, sinon `~/Téléchargements` ou `~/Downloads`).
fn downloads_dir() -> std::path::PathBuf {
    if let Ok(out) = std::process::Command::new("xdg-user-dir").arg("DOWNLOAD").output() {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !p.is_empty() {
            return p.into();
        }
    }
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    [home.join("Téléchargements"), home.join("Downloads")].into_iter().find(|d| d.is_dir()).unwrap_or_else(|| home.join("Downloads"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn gunzip(b: &[u8]) -> String {
        let mut s = String::new();
        flate2::read::GzDecoder::new(b).read_to_string(&mut s).unwrap();
        s
    }

    /// Le dépôt du contrat (docs/relais.md) : `KNJ1`, le nonce, le texte chiffré et son étiquette ; il se déchiffre avec
    /// la clé du QR code, qui est dans l'adresse après `#k=` (43 signes en base64url).
    #[test]
    fn depot_knj1_dechiffrable() {
        let (key, nonce) = ([7u8; 32], [3u8; 12]);
        let gz = gzip("KyberNow Link 0.11.8\nAppareil : Deck\n\nune ligne\n");
        let sealed = seal(&gz, &key, &nonce);
        assert_eq!(&sealed[..4], b"KNJ1");
        assert_eq!(&sealed[4..16], &nonce);
        assert_eq!(sealed.len(), 4 + 12 + gz.len() + 16);
        let plain = Aes256Gcm::new((&key).into()).decrypt(Nonce::from_slice(&nonce), &sealed[16..]).unwrap();
        assert_eq!(gunzip(&plain), "KyberNow Link 0.11.8\nAppareil : Deck\n\nune ligne\n");
        let url = qr_url("share.kybernow.app", "abcdefghijklmnopqrstuv", &key);
        let k = url.split("#k=").nth(1).unwrap();
        assert_eq!(k.len(), 43);
        assert_eq!(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(k).unwrap(), key);
        assert!(url.starts_with("https://share.kybernow.app/journal/abcdefghijklmnopqrstuv#k="));
    }

    /// 2 Mio compressés au plus : l'en-tête et la fin du journal, la plus récente.
    #[test]
    fn deux_mio_au_plus_l_en_tete_et_la_fin() {
        let header = "KyberNow Link 0.11.8\n\n";
        // Du texte qui se compresse mal (pseudo-aléatoire), plus gros que la borne.
        let mut x = 0x12345678u32;
        let body: String = (0..40_000)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                format!("{i} {x:08x}{:08x}\n", x.rotate_left(7))
            })
            .collect();
        let max = 64 * 1024;
        let gz = compressed(header, &body, max);
        assert!(gz.len() <= max && !gz.is_empty());
        let text = gunzip(&gz);
        assert!(text.starts_with(header), "l'en-tête reste");
        assert!(text.ends_with(&format!("{}\n", body.lines().last().unwrap())), "la fin la plus récente reste");
        assert!(text.contains("début du journal retiré"));
        // Petit : tout.
        assert_eq!(gunzip(&compressed(header, "court\n", max)), format!("{header}court\n"));
    }

    /// Les réponses du relais au dépôt (docs/relais.md).
    #[test]
    fn reponses_du_relais() {
        let ok = r#"{"id":"AAAAAAAAAAAAAAAAAAAAAA","owner":"BBBBBBBBBBBBBBBBBBBBBB","expires_in":3600}"#;
        assert_eq!(deposit_of(201, ok), Ok(Deposit { id: "AAAAAAAAAAAAAAAAAAAAAA".into(), owner: "BBBBBBBBBBBBBBBBBBBBBB".into(), expires_in: 3600 }));
        assert_eq!(deposit_of(404, ""), Err(JournalStep::OldRelay));
        assert_eq!(deposit_of(429, r#"{"error":"too_many","retry_after":120}"#), Err(JournalStep::TooMany));
        assert_eq!(deposit_of(503, r#"{"error":"full"}"#), Err(JournalStep::RelayDown));
        assert_eq!(deposit_of(201, "{}"), Err(JournalStep::RelayDown));
    }
}
