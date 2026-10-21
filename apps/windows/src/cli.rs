//! Ligne de commande : une session vers un ordinateur, sans l'accueil
//! (essais, dépannage). `kybernow --probe` dit ce que la carte décode.

use std::sync::{Arc, mpsc};

use kybernow_core::console::{Console, Mode};
use kybernow_core::profile::{Codec, Profile};
use kybernow_core::session::SessionParams;

use crate::host::{HostEvents, HostOptions, SessionHost, Summary};
use crate::plan::{Choices, plan};
use crate::{audio, decoder, render, system};

const USAGE: &str = "\
KyberNow pour Windows : session directe en ligne de commande (sans argument,
l'app s'ouvre sur son accueil)

Utilisation :
  kybernow <adresse>[:port] [options]

  <adresse>         adresse de l'ordinateur (sa console KyberNow)
  :port             port de la console (affiché dans la console, Accès)

Options :
  --user <compte>           compte KyberNow de l'ordinateur
  --password <mot de passe> (ou la variable KYBERNOW_PASSWORD, sinon demandé)
  --fingerprint <empreinte> empreinte de la console (sha256:…), sinon retenue
                            au premier contact
  --profile jeu|bureau|max  profil (Jeu par défaut)
  --codec h264|hevc|av1     format vidéo (selon la carte graphique par défaut)
  --444 / --420             couleurs complètes ou non
  --mbps <débit>            débit fixe en Mbit/s (sinon celui du profil)
  --size <L>x<H>@<Hz>       mode de l'écran de l'ordinateur (sinon cet écran)
  --window                  en fenêtre plutôt qu'en plein écran
  --no-vsync                présente chaque image sans attendre l'écran
  --no-hw                   décodage par le processeur
  --no-audio                son laissé sur l'ordinateur
  --physical                écrans de l'ordinateur tels quels, sans écran virtuel
  --probe                   affiche ce que la carte graphique décode, puis quitte

Raccourcis : Ctrl+Alt+Maj+D se déconnecter, +F plein écran, +M souris des jeux.
";

struct Args {
    host: String,
    port: u16,
    user: Option<String>,
    password: Option<String>,
    fingerprint: Option<String>,
    profile: Profile,
    codec: Option<Codec>,
    yuv444: Option<bool>,
    mbps: Option<u32>,
    size: Option<Mode>,
    window: bool,
    vsync: bool,
    hardware: bool,
    audio: bool,
    physical: bool,
    probe: bool,
    probe_json: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        host: String::new(),
        port: 0,
        user: None,
        password: std::env::var("KYBERNOW_PASSWORD").ok(),
        fingerprint: None,
        profile: Profile::Jeu,
        codec: None,
        yuv444: None,
        mbps: None,
        size: None,
        window: false,
        vsync: true,
        hardware: true,
        audio: true,
        physical: false,
        probe: false,
        probe_json: false,
    };
    let mut it = std::env::args().skip(1);
    let value = |name: &str, it: &mut dyn Iterator<Item = String>| {
        it.next().ok_or_else(|| format!("{name} : valeur manquante"))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--user" => a.user = Some(value("--user", &mut it)?),
            "--password" => a.password = Some(value("--password", &mut it)?),
            "--fingerprint" => a.fingerprint = Some(value("--fingerprint", &mut it)?),
            "--profile" => {
                let v = value("--profile", &mut it)?;
                a.profile = Profile::parse(&v).ok_or(format!("profil inconnu : {v}"))?;
            }
            "--codec" => {
                let v = value("--codec", &mut it)?;
                a.codec = Some(Codec::parse(&v).ok_or(format!("format inconnu : {v}"))?);
            }
            "--444" => a.yuv444 = Some(true),
            "--420" => a.yuv444 = Some(false),
            "--mbps" => {
                let v = value("--mbps", &mut it)?;
                a.mbps = Some(v.parse().map_err(|_| format!("débit invalide : {v}"))?);
            }
            "--size" => {
                let v = value("--size", &mut it)?;
                a.size = Some(parse_mode(&v).ok_or(format!("mode invalide : {v} (exemple : 1920x1080@60)"))?);
            }
            "--window" => a.window = true,
            "--no-vsync" => a.vsync = false,
            "--no-hw" => a.hardware = false,
            "--no-audio" => a.audio = false,
            "--physical" => a.physical = true,
            "--probe" => a.probe = true,
            // Processus de sonde lancé par l'app (pc.rs).
            "--probe-json" => a.probe_json = true,
            s if s.starts_with('-') => return Err(format!("option inconnue : {s}")),
            s => {
                let (h, p) = split_host(s);
                a.host = h;
                a.port = p.unwrap_or(0);
            }
        }
    }
    if a.host.is_empty() && !a.probe && !a.probe_json {
        return Err(String::new());
    }
    Ok(a)
}

fn parse_mode(s: &str) -> Option<Mode> {
    let (size, hz) = s.split_once('@').unwrap_or((s, "60"));
    let (w, h) = size.split_once(['x', '×'])?;
    Some(Mode { width: w.trim().parse().ok()?, height: h.trim().parse().ok()?, refresh: hz.trim().parse().ok()? })
}

/// `hôte`, `hôte:port`, `[ipv6]:port`.
fn split_host(s: &str) -> (String, Option<u16>) {
    if let Some(rest) = s.strip_prefix('[') {
        if let Some((h, p)) = rest.split_once(']') {
            return (h.to_string(), p.strip_prefix(':').and_then(|p| p.parse().ok()));
        }
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => (h.to_string(), p.parse().ok()),
        _ => (s.to_string(), None),
    }
}

fn prompt(label: &str) -> String {
    use std::io::Write;
    eprint!("{label} : ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    s.trim().to_string()
}

struct CliEvents(std::sync::Mutex<mpsc::Sender<Summary>>);

impl HostEvents for CliEvents {
    fn ended(&self, s: &Summary) {
        let _ = self.0.lock().unwrap().send(s.clone());
    }
}

pub fn main() -> i32 {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}\n");
            }
            eprint!("{USAGE}");
            return 2;
        }
    };

    if args.probe_json {
        let gpu = crate::pc::probe_gpu();
        println!("{}", serde_json::to_string(&gpu).unwrap_or_default());
        log::info!("sonde : {gpu:?}");
        return 0;
    }

    if args.probe {
        return match system::probe_only() {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        };
    }

    let port = if args.port == 0 { 9443 } else { args.port };
    let user = args.user.clone().unwrap_or_else(|| prompt("Compte KyberNow de l'ordinateur"));
    let password = args.password.clone().unwrap_or_else(|| prompt("Mot de passe"));

    // Certificat de la console : celui qu'on a retenu, sinon le premier vu.
    let key = format!("{}:{port}", args.host);
    let known = args.fingerprint.clone().or_else(|| system::known_console(&key));
    let console = Console::new(&args.host, port, &user, &password, known.clone());
    let info = match console.info() {
        Ok(i) => i,
        Err(e) => {
            system::error_box(&format!("Impossible de joindre l'ordinateur : {e}"));
            return 1;
        }
    };
    let fingerprint = known.or_else(|| console.fingerprint_seen());
    if args.fingerprint.is_none() && system::known_console(&key).is_none() {
        if let Some(fp) = &fingerprint {
            eprintln!("Empreinte de la console : {fp}\n(comparez avec celle affichée dans la console, Accès)");
            system::remember_console(&key, fp);
        }
    }
    let host_name = if info.hostname.is_empty() { args.host.clone() } else { info.hostname.clone() };
    log::info!("ordinateur : {host_name} ({}, {:?})", info.os, info.gpu);

    let caps = if args.hardware {
        match render::create_device() {
            Ok((device, _)) => decoder::probe(&device).caps,
            Err(_) => Default::default(),
        }
    } else {
        Default::default()
    };
    let choices = Choices {
        profile: Some(args.profile),
        codec: args.codec,
        yuv444: args.yuv444,
        mbps: args.mbps,
        size: args.size,
        ..Default::default()
    };
    let (settings, mode) = plan(&choices, caps, system::screen_mode());
    log::info!(
        "réglages : {} · {} {} · {} Mbit/s · {}×{} à {} Hz",
        settings.label(),
        settings.codec.label(),
        if settings.yuv444 { "4:4:4" } else { "4:2:0" },
        settings.mbps,
        mode.width,
        mode.height,
        mode.refresh
    );
    let audio_channels = if args.audio { audio::default_output_channels() } else { 0 };
    let params = SessionParams {
        console_host: args.host.clone(),
        console_port: port,
        user,
        password,
        console_fingerprint: fingerprint,
        device: system::device_info(),
        mode,
        audio_channels,
        virtual_display: !args.physical,
        settings,
        auto_reconnect: true,
        gamepad_xbox360: true,
        clipboard: true,
    };
    let (tx, rx) = mpsc::channel();
    let _host = SessionHost::start(
        host_name,
        params,
        HostOptions {
            vsync: args.vsync,
            hardware: args.hardware,
            fullscreen: !args.window,
            mouse: None,
            window_size: system::initial_window_size(mode),
        },
        Arc::new(CliEvents(std::sync::Mutex::new(tx))),
    );
    let summary = rx.recv().unwrap_or_default();
    match summary.error {
        Some(e) => {
            system::error_box(&format!("La session s'est arrêtée : {e}"));
            1
        }
        None => 0,
    }
}
