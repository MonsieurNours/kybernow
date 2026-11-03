//! Essai des consoles PlayStation depuis le terminal (docs/playstation.md).
//!
//! ```text
//! playstation search [adresse…]
//! playstation login                     (affiche la page de Sony, lit l'adresse de retour collée)
//! playstation login <adresse de retour> (même chose sans attendre la saisie)
//! playstation account <base64>          (identifiant saisi à la main, en secours)
//! playstation regist <adresse> <code>   (associe la console : écran « Associer un appareil » ouvert)
//! playstation wake <adresse>            (réveille une console associée)
//! ```
//!
//! L'identifiant et les consoles associées sont gardés dans le fichier
//! `PS_STATE` (par défaut `playstation-essai.json` dans le dossier courant).

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use kybernow_core::playstation::discovery::{self, Model};
use kybernow_core::playstation::psn;
use kybernow_core::playstation::regist::{self, PsnId, Registered};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct State {
    account_id: Option<String>,
    /// Par `host-id` (adresse matérielle).
    consoles: BTreeMap<String, Registered>,
}

fn state_path() -> String {
    std::env::var("PS_STATE").unwrap_or_else(|_| "playstation-essai.json".into())
}

fn load() -> State {
    std::fs::read_to_string(state_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(state: &State) {
    std::fs::write(state_path(), serde_json::to_string_pretty(state).unwrap()).expect("écriture de l'état");
}

fn find(addr: Ipv4Addr) -> discovery::Console {
    discovery::search(Duration::from_secs(2), &[addr])
        .expect("recherche")
        .into_iter()
        .find(|c| c.addr == addr)
        .expect("aucune console à cette adresse")
}

fn main() {
    let _ = simple_log();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("search") => {
            let direct: Vec<Ipv4Addr> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
            let consoles = discovery::search(Duration::from_secs(3), &direct).expect("recherche");
            if consoles.is_empty() {
                println!("Aucune console trouvée.");
            }
            let state = load();
            for c in consoles {
                println!(
                    "{} « {} » {} {:?} id={} système={} cible={:?} jeu={} associée={}",
                    c.model().name(),
                    c.name,
                    c.addr,
                    c.state,
                    c.host_id,
                    c.system_version,
                    c.target(),
                    c.running_app.as_deref().unwrap_or("-"),
                    if state.consoles.contains_key(&c.host_id.to_ascii_uppercase()) { "oui" } else { "non" },
                );
            }
        }
        Some("login") => {
            let url = match args.get(1) {
                Some(u) => u.clone(),
                None => {
                    println!("Ouvrez cette page, connectez-vous, puis collez ici l'adresse de la page « redirect » :\n\n{}\n", psn::LOGIN_URL);
                    let mut line = String::new();
                    std::io::stdin().read_line(&mut line).expect("lecture");
                    line
                }
            };
            let code = psn::code_from_redirect(url.trim()).expect("pas de code dans l'adresse");
            let id = psn::account_id(&code).unwrap_or_else(|e| panic!("{e}"));
            let mut state = load();
            state.account_id = id.to_base64();
            save(&state);
            println!("Identifiant du compte lu et gardé dans {}.", state_path());
        }
        Some("account") if args.len() == 2 => {
            let id = PsnId::from_base64(&args[1]).expect("identifiant base64 de 8 octets");
            let mut state = load();
            state.account_id = id.to_base64();
            save(&state);
            println!("Identifiant gardé dans {}.", state_path());
        }
        Some("regist") if args.len() == 3 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let pin: u32 = args[2].replace(' ', "").parse().expect("code à 8 chiffres");
            let mut state = load();
            let id = state
                .account_id
                .as_deref()
                .and_then(PsnId::from_base64)
                .expect("pas d'identifiant : lancez d'abord « login »");
            let console = find(addr);
            println!("Association de « {} » ({:?})…", console.name, console.target());
            match regist::regist(addr, console.target(), &id, pin) {
                Ok(r) => {
                    println!("Associée : « {} », {}, clé de réveil {:?}", r.nickname, r.mac, r.wake_credential().is_some());
                    state.consoles.insert(r.mac.clone(), r);
                    save(&state);
                }
                Err(e) => {
                    eprintln!("Échec : {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("wake") if args.len() == 2 => {
            let addr: Ipv4Addr = args[1].parse().expect("adresse IPv4");
            let console = find(addr);
            let state = load();
            let r = state
                .consoles
                .get(&console.host_id.to_ascii_uppercase())
                .expect("console pas encore associée : lancez « regist »");
            let model = if console.model() == Model::Ps5 { Model::Ps5 } else { Model::Ps4 };
            discovery::wake(IpAddr::V4(addr), model, r.wake_credential().expect("clé de réveil")).expect("réveil");
            println!("Réveil envoyé à « {} ».", console.name);
        }
        _ => eprintln!("usage : search [adresse…] | login [adresse de retour] | account <base64> | regist <adresse> <code> | wake <adresse>"),
    }
}

fn simple_log() -> Result<(), log::SetLoggerError> {
    struct L;
    impl log::Log for L {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, r: &log::Record) {
            eprintln!("[{}] {}", r.level(), r.args());
        }
        fn flush(&self) {}
    }
    log::set_logger(&L).map(|()| log::set_max_level(log::LevelFilter::Info))
}
