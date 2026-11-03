//! Essai des consoles PlayStation depuis le terminal (docs/playstation.md).
//!
//! cargo run -p kybernow-core --example playstation -- search [adresse…]
//! cargo run -p kybernow-core --example playstation -- wake <adresse> <ps5|ps4> <clé RP-RegistKey>

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use kybernow_core::playstation::discovery::{self, Model};

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
            for c in consoles {
                println!(
                    "{} « {} » {} {:?} id={} système={} protocole={} port={} cible={:?} jeu={}",
                    c.model().name(),
                    c.name,
                    c.addr,
                    c.state,
                    c.host_id,
                    c.system_version,
                    c.protocol_version,
                    c.request_port,
                    c.target(),
                    c.running_app.as_deref().unwrap_or("-"),
                );
            }
        }
        Some("wake") if args.len() == 4 => {
            let addr: IpAddr = args[1].parse().expect("adresse");
            let model = if args[2] == "ps4" { Model::Ps4 } else { Model::Ps5 };
            let credential = discovery::credential_from_regist_key(args[3].as_bytes()).expect("clé hexadécimale");
            discovery::wake(addr, model, credential).expect("réveil");
        }
        _ => eprintln!("usage : search [adresse…] | wake <adresse> <ps5|ps4> <clé>"),
    }
}

fn simple_log() -> Result<(), log::SetLoggerError> {
    struct L;
    impl log::Log for L {
        fn enabled(&self, _: &log::Metadata) -> bool { true }
        fn log(&self, r: &log::Record) { eprintln!("[{}] {}", r.level(), r.args()); }
        fn flush(&self) {}
    }
    log::set_logger(&L).map(|()| log::set_max_level(log::LevelFilter::Debug))
}
