//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos ») côté appareil.
//!
//! - Le fichier, `kybernow-journal.txt` : l'appareil et son système, la version de l'app, la fin du lancement
//!   précédent (`kybernow.log.1` : sa fin, et s'il s'est arrêté normalement), puis le journal de l'app. Ni mot de
//!   passe, ni jeton, ni code : le journal de l'app n'en écrit pas.
//! - **Au bureau** : la fenêtre d'un e-mail, le fichier joint (`xdg-email --attach` ; dans le Flatpak, celui du runtime
//!   passe par le portail e-mail) ; sans app d'e-mail, la fenêtre d'enregistrement (zenity ou kdialog), Téléchargements
//!   proposé ; sans elle, enregistré dans Téléchargements.
//! - **Par QR code** (Steam Deck en mode Jeu, salon, KyberNow Link OS ; docs/relais.md, « Dépôt d'un journal ») : par
//!   le cœur (`kybernow_core::journal`) : compressé (gzip, 2 Mio au plus : l'en-tête et la fin du journal, la plus
//!   récente), chiffré en AES-256-GCM avec une clé tirée pour ce dépôt (`KNJ1`), déposé sur le relais du compte de
//!   l'appareil (sinon le relais public), par l'agent HTTPS du relais (empreinte épinglée d'un relais chez soi
//!   comprise) ; le QR code porte `https://<relais>/journal/<id>#k=<clé>`, la clé que le téléphone ne renvoie jamais.
//!   L'état du dépôt est relu toutes les 2 s (ouvert, expiré) ; « Terminé » retire un dépôt pas encore ouvert.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kybernow_core::journal::{self, DepositError, Remote};

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
            let body = logs();
            if qr {
                by_qr(&header, &body, &relay, &stop, &send);
            } else {
                send(JournalEvent::Done(by_email(&format!("{header}{body}"), &version)));
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

/// L'en-tête du journal (`journal::header` du cœur, les lignes que lit la page du relais) : l'appareil, son système,
/// l'interface, Link OS, le moment.
fn header(app: &App) -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "Linux".into());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").map(|s| s.trim().to_string()).unwrap_or_default();
    let mut extra = vec![format!("Système : {os} · noyau {kernel}"), format!("Interface : {:?}{}", app.layout, if crate::app::flatpak() { " · Flatpak" } else { "" })];
    if let Some(o) = &app.os {
        extra.push(format!("KyberNow Link OS {} · emplacement {}", o.version, o.slot));
    }
    journal::header_now(kybernow_core::i18n::current(), &app.version, &app.device_name, &extra)
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

/// Le relais du dépôt : celui du compte KyberNow de l'appareil (empreinte épinglée comprise), sinon le relais public
/// (vide).
fn relay_of(app: &App) -> String {
    app.store.accounts.first().map(|a| a.relay.clone()).unwrap_or_default()
}

/// Le refus du cœur, dans les étapes de l'écran.
fn step_of(e: &DepositError) -> JournalStep {
    match e {
        DepositError::OldRelay => JournalStep::OldRelay,
        DepositError::TooMany => JournalStep::TooMany,
        DepositError::Unreachable(_) | DepositError::Failed(_) => JournalStep::RelayDown,
    }
}

/// Par QR code (`journal::deposit` du cœur : compressé, chiffré, déposé) ; puis l'état relu jusqu'à l'ouverture,
/// l'expiration ou « Terminé ».
fn by_qr(header: &str, body: &str, relay: &str, stop: &AtomicBool, send: &dyn Fn(JournalEvent)) {
    send(JournalEvent::Step(JournalStep::Uploading));
    let deposit = match journal::deposit(relay, header, body) {
        Ok(d) => d,
        Err(e) => {
            log::info!("journal : dépôt impossible ({})", e.detail());
            send(JournalEvent::Step(step_of(&e)));
            return;
        }
    };
    send(JournalEvent::Step(JournalStep::Qr { url: deposit.url.clone() }));
    let end = Instant::now() + Duration::from_secs(deposit.expires_in);
    loop {
        // « Terminé » : retiré s'il n'a pas été ouvert (sans effet sinon : le relais l'a déjà effacé).
        if stop.load(Ordering::Relaxed) {
            journal::discard(&deposit.relay, &deposit.id, &deposit.owner);
            return;
        }
        if Instant::now() >= end {
            send(JournalEvent::Step(JournalStep::Expired));
            return;
        }
        match journal::state(&deposit.relay, &deposit.id, &deposit.owner) {
            Remote::Opened => {
                send(JournalEvent::Step(JournalStep::Opened));
                return;
            }
            Remote::Gone => {
                send(JournalEvent::Step(JournalStep::Expired));
                return;
            }
            Remote::Waiting | Remote::Unknown => {}
        }
        // Attendre, en voyant « Terminé ».
        let wait = Instant::now() + journal::CHECK_EVERY;
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

    /// Les refus du relais (`journal::deposit` du cœur) dans les étapes de l'écran.
    #[test]
    fn refus_du_relais() {
        assert_eq!(step_of(&DepositError::OldRelay), JournalStep::OldRelay);
        assert_eq!(step_of(&DepositError::TooMany), JournalStep::TooMany);
        assert_eq!(step_of(&DepositError::Unreachable("HTTP 500".into())), JournalStep::RelayDown);
        assert_eq!(step_of(&DepositError::Failed("adresse".into())), JournalStep::RelayDown);
    }
}
