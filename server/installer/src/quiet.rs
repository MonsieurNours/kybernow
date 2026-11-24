//! Installation silencieuse de la Station (déploiement d'une flotte,
//! docs/comptes.md, « Enrôlement ») :
//!
//! ```text
//! KyberNow-Station-Setup.exe /quiet [/enroll=<jeton>] [/relay=<adresse>]
//! ```
//!
//! Sans écran : l'analyse de l'ordinateur, l'installation avec les options
//! d'office (une installation existante garde ses réglages), puis, avec un
//! jeton, l'enrôlement par Admin (`POST /api/local/account/enroll`, que
//! Admin passe au relais par sa connexion de contrôle). Le journal est celui
//! de l'installation (`install.log`), complété de l'enrôlement.
//!
//! Codes de sortie : `0` installée (et enrôlée), `3010` installée, à
//! redémarrer (manettes, écran virtuel), `1` installation échouée, `2`
//! installée mais pas enrôlée (jeton refusé, relais injoignable).

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Les arguments de l'installation silencieuse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuietArgs {
    pub enroll: Option<String>,
    pub relay: Option<String>,
}

pub const EXIT_OK: i32 = 0;
pub const EXIT_INSTALL_FAILED: i32 = 1;
pub const EXIT_ENROLL_FAILED: i32 = 2;
pub const EXIT_REBOOT: i32 = 3010;

/// `/quiet` (ou `--quiet`, `/q`) présent : les arguments ; `/enroll=…`,
/// `--enroll=…` ou `--enroll …`, de même pour `relay`.
pub fn parse_args(args: &[String]) -> Option<QuietArgs> {
    let quiet = args.iter().any(|a| matches!(a.to_ascii_lowercase().as_str(), "/quiet" | "--quiet" | "/q"));
    if !quiet {
        return None;
    }
    let value = |name: &str| -> Option<String> {
        args.iter().enumerate().find_map(|(i, a)| {
            let lower = a.to_ascii_lowercase();
            for prefix in [format!("/{name}="), format!("--{name}="), format!("/{name}:")] {
                if lower.starts_with(&prefix) {
                    return Some(a[prefix.len()..].to_string());
                }
            }
            (lower == format!("--{name}") || lower == format!("/{name}")).then(|| args.get(i + 1).cloned()).flatten()
        })
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
    };
    Some(QuietArgs { enroll: value("enroll"), relay: value("relay") })
}

/// Mot de passe tiré au hasard pour le compte d'installation (Kyber et
/// Admin) : personne ne le connaît ; on prend la main sur Admin par l'icône
/// (« Réinitialiser le mot de passe d'Admin… ») ou, plus tard, avec un compte
/// KyberNow qui a le droit Administrer.
pub fn random_password() -> String {
    use rand::Rng;
    const SET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..24).map(|_| SET[rng.random_range(0..SET.len())] as char).collect()
}

/// Le compte d'installation d'une installation silencieuse.
pub const QUIET_USER: &str = "kybernow";

/// Après l'installation : l'enrôlement (avec un jeton), ou seulement le
/// relais (`/relay=` sans jeton). Rend les lignes du journal et le code.
pub fn after_install(args: &QuietArgs) -> (Vec<String>, i32) {
    let mut lines = Vec::new();
    match (&args.enroll, &args.relay) {
        (Some(token), relay) => match crate::account::enroll(token, relay.as_deref(), Duration::from_secs(180)) {
            Ok(state) => {
                let owner = state["owner"]["name"].as_str().unwrap_or_default();
                lines.push(format!("Enrôlée : rattachée à {owner} (relais {})", state["relay"].as_str().unwrap_or_default()));
                (lines, EXIT_OK)
            }
            Err(e) => {
                lines.push(format!("ÉCHEC de l'enrôlement : {e:#}"));
                (lines, EXIT_ENROLL_FAILED)
            }
        },
        (None, Some(relay)) => {
            let result = crate::account::RelayAddr::parse(relay)
                .ok_or_else(|| anyhow::anyhow!("adresse de relais illisible : {relay}"))
                .and_then(|addr| {
                    let deadline = std::time::Instant::now() + Duration::from_secs(120);
                    loop {
                        match crate::account::Admin::locate().and_then(|a| a.set_relay(&addr.to_string())) {
                            Ok(_) => return Ok(addr),
                            Err(e) if std::time::Instant::now() >= deadline => return Err(e),
                            Err(_) => std::thread::sleep(Duration::from_secs(1)),
                        }
                    }
                });
            match result {
                Ok(addr) => lines.push(format!("Relais réglé : {addr}")),
                Err(e) => lines.push(format!("ATTENTION : relais non réglé ({e:#})")),
            }
            (lines, EXIT_OK)
        }
        (None, None) => (lines, EXIT_OK),
    }
}

/// Installation silencieuse de la Station Windows (le lanceur porte le contenu).
pub fn run_windows(args: &QuietArgs, payload: &Path) -> i32 {
    use crate::events::{Emit, Event, InstallRequest};
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = lines.clone();
    let emit: Emit = Arc::new(move |event: Event| {
        if let Event::Log { line } = event {
            seen.lock().unwrap().push(line);
        }
    });
    let info = crate::checks::run(&emit);
    let keep = info.existing.as_ref().is_some_and(|e| e.settings);
    let (stream_port, console_port) = match (&info.existing, keep) {
        (Some(e), true) => (e.stream_port, e.console_port),
        _ => (info.stream_port, info.console_port),
    };
    let request = InstallRequest {
        user: if keep { String::new() } else { QUIET_USER.into() },
        password: if keep { String::new() } else { random_password() },
        stream_port,
        console_port,
        virtual_display: true,
        gamepads: true,
        virtual_audio: info.steam_speakers_inf.is_some(),
        max_performance: info.power_needs_tuning,
        start_with_windows: true,
        keep_settings: keep,
    };
    let mut info = info;
    info.stream_port = stream_port;
    info.console_port = console_port;
    let need_reboot = match crate::install::run_install(&request, &info, payload, &emit) {
        Ok(r) => r,
        Err(_) => return EXIT_INSTALL_FAILED,
    };
    let mut journal = vec![String::new(), "Installation silencieuse".to_string()];
    if !keep {
        journal.push(format!("Compte d'installation « {QUIET_USER} », mot de passe tiré au hasard : réinitialisez-le depuis l'icône (« Réinitialiser le mot de passe d'Admin… »)"));
    }
    let (more, code) = after_install(args);
    journal.extend(more);
    append_log(&crate::sys::install_dir().join("install.log"), &journal);
    append_log(&std::env::temp_dir().join("KyberNow-install.log"), &journal);
    match code {
        EXIT_OK if need_reboot => EXIT_REBOOT,
        c => c,
    }
}

/// Ajoute des lignes à la fin d'un journal.
pub fn append_log(path: &Path, lines: &[String]) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(path) {
        for line in lines {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn arguments_de_l_installation_silencieuse() {
        assert_eq!(parse_args(&args(&["setup.exe"])), None);
        assert_eq!(parse_args(&args(&["setup.exe", "/enroll=kne_x"])), None, "sans /quiet, l'interface s'ouvre");
        assert_eq!(
            parse_args(&args(&["setup.exe", "/quiet", "/enroll=kne_7Qm2xR9p", "/relay=share.kybernow.app"])),
            Some(QuietArgs { enroll: Some("kne_7Qm2xR9p".into()), relay: Some("share.kybernow.app".into()) })
        );
        assert_eq!(
            parse_args(&args(&["kybernow-ui", "--quiet", "--enroll", "kne_7Qm2xR9p", "--relay=\"relais.exemple.org:9443\""])),
            Some(QuietArgs { enroll: Some("kne_7Qm2xR9p".into()), relay: Some("relais.exemple.org:9443".into()) })
        );
        assert_eq!(parse_args(&args(&["setup.exe", "/QUIET", "/ENROLL=Kne_Casse"])), Some(QuietArgs { enroll: Some("Kne_Casse".into()), relay: None }));
        assert_eq!(parse_args(&args(&["setup.exe", "/quiet", "/enroll="])), Some(QuietArgs::default()));
    }

    #[test]
    fn mot_de_passe_au_hasard() {
        let (a, b) = (random_password(), random_password());
        assert_eq!(a.len(), 24);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
