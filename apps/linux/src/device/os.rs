//! KyberNow Link OS : ce que l'app demande au système quand elle est l'OS
//! (image flashée sur un Raspberry Pi) : qui elle est (`/etc/kybernow-os`),
//! la mise à jour A/B (`kybernow-os-update`, par sudo), le reste par
//! `kybernow-os-ctl` (SSH, veille, CEC, redémarrage, mode HDMI), le réseau
//! par NetworkManager (`nmcli`), les manettes Bluetooth par `bluetoothctl`
//! (agent sans code, à l'écoute le temps de l'écran Manette, au premier
//! démarrage ou depuis Réglages de l'appareil › Manettes ; seules les manettes
//! sont associées). Tout passe par
//! des fils : la boucle principale ne bloque jamais.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::Event;
use crate::app::{BtDevice, OsInfo, WifiNet};

const RELEASE: &str = "/etc/kybernow-os/release";

/// L'app tourne sur KyberNow Link OS (variable du service, ou le dossier de l'OS).
pub fn is_os() -> bool {
    std::env::var("KYBERNOW_OS").is_ok_and(|v| v == "1") || Path::new(RELEASE).exists()
}

/// Le dossier des données de l'app sur l'OS (partition de données) : les
/// images téléchargées y vont.
pub fn update_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/var/lib/kybernow/config"));
    base.parent().map(Path::to_path_buf).unwrap_or(base).join("update")
}

fn release_field(key: &str) -> String {
    std::fs::read_to_string(RELEASE)
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_default()
}

/// Ce que l'OS dit de lui-même, sans le script (vite, au lancement).
pub fn info() -> OsInfo {
    OsInfo { version: release_field("VERSION"), built: release_field("BUILT").chars().take(10).collect(), ssh: ssh_on(), ..OsInfo::default() }
}

fn ssh_on() -> bool {
    Command::new("systemctl").args(["is-active", "--quiet", "ssh.service"]).status().is_ok_and(|s| s.success())
}

fn sudo(args: &[&str]) -> Result<String, String> {
    let out = Command::new("sudo").arg("-n").args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// `kybernow-os-ctl …` en root (sudoers), sans attendre le résultat.
pub fn ctl(args: &[&str]) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(e) = sudo(&[&["/usr/local/sbin/kybernow-os-ctl"], a.as_slice()].concat()) {
            log::warn!("kybernow-os-ctl {} : {e}", a.join(" "));
        }
    });
}

/// L'état A/B (`kybernow-os-update status`), envoyé à la boucle.
pub fn status(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let mut info = info();
        match sudo(&["/usr/local/sbin/kybernow-os-update", "status"]) {
            Ok(json) => {
                let get = |k: &str| json.split(&format!("\"{k}\":")).nth(1).map(|r| r.trim_start().trim_start_matches('"').split(['"', ',', '}']).next().unwrap_or("").to_string()).unwrap_or_default();
                info.slot = get("current");
                info.other = get("other");
                info.other_version = get("other_version");
                info.ab = get("ab") == "1";
                info.pending = get("pending");
                info.tryboot = get("tryboot") == "1";
                if !get("version").is_empty() {
                    info.version = get("version");
                }
            }
            Err(e) => log::warn!("kybernow-os-update status : {e}"),
        }
        let _ = tx.send(Event::OsStatus(info));
    });
}

/// L'image téléchargée s'écrit dans l'autre emplacement, puis redémarrage à l'essai.
pub fn apply(image: PathBuf, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let r = sudo(&["/usr/local/sbin/kybernow-os-update", "apply", &image.to_string_lossy()]).map(|_| ());
        let _ = tx.send(Event::OsApplied(r));
    });
}

/// L'essai a réussi (l'accueil s'est affiché) : l'emplacement devient le défaut.
pub fn commit() {
    std::thread::spawn(|| match sudo(&["/usr/local/sbin/kybernow-os-update", "commit"]) {
        Ok(s) => log::info!("A/B : {}", s.trim()),
        Err(e) => log::warn!("A/B commit : {e}"),
    });
}

pub fn rollback(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let r = sudo(&["/usr/local/sbin/kybernow-os-update", "rollback"]).map(|_| ());
        let _ = tx.send(Event::OsApplied(r));
    });
}

// MARK: - Réseau (NetworkManager)

pub fn wifi_scan(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let _ = Command::new("nmcli").args(["radio", "wifi", "on"]).status();
        let out = Command::new("nmcli").args(["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list", "--rescan", "yes"]).output();
        let r = match out {
            Ok(o) if o.status.success() => {
                let mut nets: Vec<WifiNet> = Vec::new();
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    let mut parts = line.splitn(4, ':');
                    let used = parts.next().unwrap_or("").trim() == "*";
                    let ssid = parts.next().unwrap_or("").replace("\\:", ":");
                    let signal: u8 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
                    let sec = parts.next().unwrap_or("").trim();
                    if ssid.is_empty() || nets.iter().any(|n| n.ssid == ssid) {
                        continue;
                    }
                    nets.push(WifiNet { ssid, signal, secured: !sec.is_empty() && sec != "--", active: used });
                }
                nets.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
                Ok(nets)
            }
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(Event::Wifi(r));
    });
}

pub fn wifi_connect(ssid: String, password: String, tx: Sender<Event>) {
    std::thread::spawn(move || {
        let mut cmd = Command::new("nmcli");
        cmd.args(["dev", "wifi", "connect", &ssid]);
        if !password.is_empty() {
            cmd.args(["password", &password]);
        }
        let r = match cmd.output() {
            Ok(o) if o.status.success() => Ok(ssid),
            Ok(o) => {
                let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
                Err(if e.contains("Secrets were required") || e.contains("802-11-wireless-security") { crate::i18n::tr("Mot de passe refusé", "Password refused").to_string() } else { e })
            }
            Err(e) => Err(e.to_string()),
        };
        let _ = tx.send(Event::WifiDone(r));
    });
}

// MARK: - Manettes Bluetooth

/// Une écoute Bluetooth : `bluetoothctl` tenu ouvert avec un agent sans code,
/// en recherche. Seules les manettes sont associées, approuvées, connectées :
/// celles que BlueZ classe en manette (icône `input-gaming`, classe ou
/// apparence de manette, lues par `bluetoothctl info`), ou dont le nom le dit
/// (`gamepad_name`). Un casque ou un téléphone qui passe reste à l'écart. Les
/// appareils vus partent à la boucle (`Event::Bt`).
pub struct BtListener {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stop: Arc<Mutex<bool>>,
}

/// Associe, approuve, connecte une manette (une seule fois par écoute).
fn pair_gamepad(stdin: &Mutex<ChildStdin>, wanted: &Mutex<Vec<String>>, mac: &str) {
    let mut w = wanted.lock().unwrap();
    if w.iter().any(|m| m == mac) {
        return;
    }
    w.push(mac.to_string());
    let mut stdin = stdin.lock().unwrap();
    let _ = writeln!(stdin, "pair {mac}\ntrust {mac}\nconnect {mac}");
    let _ = stdin.flush();
}

impl BtListener {
    pub fn start(tx: Sender<Event>) -> Option<BtListener> {
        // Sans adaptateur (carte sans Bluetooth, émulation), bluetoothd ne
        // tourne pas et bluetoothctl attendrait le bus : pas d'écoute.
        if std::fs::read_dir("/sys/class/bluetooth").map(|d| d.count() == 0).unwrap_or(true) {
            return None;
        }
        let mut child = Command::new("bluetoothctl").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let mut stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let stop = Arc::new(Mutex::new(false));
        let stop2 = stop.clone();
        let _ = writeln!(stdin, "power on\nagent NoInputNoOutput\ndefault-agent\npairable on\ndiscoverable on\nscan on");
        let _ = stdin.flush();
        let stdin = Arc::new(Mutex::new(stdin));
        let stdin2 = stdin.clone();
        // Les manettes à qui l'association a été demandée : seules elles sont
        // approuvées et connectées quand l'association aboutit.
        let wanted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        std::thread::spawn(move || {
            let mut devices: Vec<BtDevice> = Vec::new();
            let mut last_sent = Instant::now();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if *stop2.lock().unwrap() {
                    break;
                }
                // Les lignes utiles : [NEW] Device XX:… Nom, [CHG] Device XX:… Connected: yes / Paired: yes / Name: / Icon:.
                let clean: String = line.chars().filter(|c| !c.is_control()).collect();
                let clean = strip_ansi(&clean);
                let Some(rest) = clean.find("Device ").map(|i| &clean[i + 7..]) else { continue };
                let mut it = rest.splitn(2, ' ');
                let mac = it.next().unwrap_or("").to_string();
                let tail = it.next().unwrap_or("").trim().to_string();
                if mac.len() != 17 {
                    continue;
                }
                let entry = match devices.iter_mut().find(|d| d.mac == mac) {
                    Some(d) => d,
                    None => {
                        devices.push(BtDevice { mac: mac.clone(), ..Default::default() });
                        devices.last_mut().unwrap()
                    }
                };
                if clean.contains("[NEW]") {
                    if !tail.is_empty() && tail != mac.replace(':', "-") {
                        entry.name = tail.clone();
                    }
                    // Un appareil qui se présente : une manette au nom connu tout
                    // de suite, sinon sa classe, lue à part (la lecture ne bloque
                    // pas celle des lignes).
                    if gamepad_name(&entry.name) {
                        pair_gamepad(&stdin2, &wanted, &mac);
                    } else {
                        let (stdin3, wanted3, mac3) = (stdin2.clone(), wanted.clone(), mac.clone());
                        std::thread::spawn(move || {
                            if btctl(&["info", &mac3]).is_some_and(|o| gamepad_info(&String::from_utf8_lossy(&o.stdout))) {
                                pair_gamepad(&stdin3, &wanted3, &mac3);
                            }
                        });
                    }
                } else if let Some(v) = tail.strip_prefix("Connected: ") {
                    entry.connected = v.trim() == "yes";
                } else if let Some(v) = tail.strip_prefix("Paired: ") {
                    entry.paired = v.trim() == "yes";
                    if entry.paired && wanted.lock().unwrap().iter().any(|m| *m == mac) {
                        let mut stdin = stdin2.lock().unwrap();
                        let _ = writeln!(stdin, "trust {mac}\nconnect {mac}");
                        let _ = stdin.flush();
                    }
                } else if let Some(v) = tail.strip_prefix("Name: ") {
                    entry.name = v.trim().to_string();
                    // Un nom arrivé après coup (manettes Bluetooth basse consommation).
                    if gamepad_name(&entry.name) {
                        pair_gamepad(&stdin2, &wanted, &mac);
                    }
                } else if let Some(v) = tail.strip_prefix("Alias: ") {
                    if entry.name.is_empty() {
                        entry.name = v.trim().to_string();
                    }
                } else if tail.strip_prefix("Icon: ").is_some_and(|v| v.trim() == "input-gaming") {
                    pair_gamepad(&stdin2, &wanted, &mac);
                }
                if last_sent.elapsed() > Duration::from_millis(200) {
                    last_sent = Instant::now();
                    let shown: Vec<BtDevice> = devices.iter().filter(|d| !d.name.is_empty()).cloned().collect();
                    let _ = tx.send(Event::Bt(shown));
                }
            }
        });
        Some(BtListener { child, stdin, stop })
    }
}

impl Drop for BtListener {
    fn drop(&mut self) {
        *self.stop.lock().unwrap() = true;
        if let Ok(mut stdin) = self.stdin.lock() {
            let _ = writeln!(stdin, "scan off\ndiscoverable off\nquit");
            let _ = stdin.flush();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Un nom de manette courant (Xbox, DualSense, DualShock, Switch Pro, Joy-Con,
/// 8BitDo, Stadia, Luna, Steam) : pas « Xbox » ni « Razer » seuls, qui
/// nomment aussi des casques.
fn gamepad_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["controller", "gamepad", "dualsense", "dualshock", "joy-con", "8bitdo", "manette", "stadia"].iter().any(|k| n.contains(k))
}

/// `bluetoothctl info` : BlueZ classe l'appareil en manette ou en joystick
/// (icône `input-gaming`, déduite de la classe Bluetooth classique ou de
/// l'apparence en basse consommation).
fn gamepad_info(info: &str) -> bool {
    info.lines().map(str::trim).any(|l| {
        if let Some(v) = l.strip_prefix("Icon: ") {
            return v.trim() == "input-gaming";
        }
        if let Some(v) = l.strip_prefix("Class: ") {
            // Classe de l'appareil : périphérique (classe majeure 0x05), joystick (1) ou manette (2).
            // (« 0x00002508 », suivi du décimal entre parenthèses selon la version de BlueZ.)
            let c = u32::from_str_radix(v.split_whitespace().next().unwrap_or("").trim_start_matches("0x"), 16).unwrap_or(0);
            return (c >> 8) & 0x1f == 0x05 && matches!((c >> 2) & 0x0f, 1 | 2);
        }
        if let Some(v) = l.strip_prefix("Appearance: ") {
            // Apparence GATT : joystick (0x03c3), manette (0x03c4).
            return matches!(v.split_whitespace().next(), Some("0x03c3" | "0x03c4"));
        }
        false
    })
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            while let Some(n) = chars.next() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

static LISTENER: Mutex<Option<BtListener>> = Mutex::new(None);

/// Le Bluetooth à l'écoute (écran Manette, Réglages › Manettes), ou plus ;
/// rend vrai si l'écoute est en place.
pub fn listen(on: bool, tx: Sender<Event>) -> bool {
    let mut l = LISTENER.lock().unwrap();
    if on {
        if l.is_none() {
            *l = BtListener::start(tx);
            if l.is_none() {
                log::warn!("Bluetooth indisponible : pas d'association");
            }
        }
    } else {
        *l = None;
    }
    l.is_some()
}

/// `bluetoothctl` en une commande, bornée à 3 s : sans bluetoothd (carte sans
/// Bluetooth, service arrêté, émulation) il attend le bus indéfiniment, et le
/// lancement resterait sur « bluetooth » pour toujours.
fn btctl(args: &[&str]) -> Option<std::process::Output> {
    Command::new("timeout").arg("3").arg("bluetoothctl").args(args).output().ok().filter(|o| o.status.success())
}

/// Les manettes déjà associées (`bluetoothctl devices Paired`), pour les réglages.
pub fn paired(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let Some(o) = btctl(&["devices", "Paired"]) else { return };
        let list: Vec<BtDevice> = String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| {
                let rest = l.strip_prefix("Device ")?;
                let (mac, name) = rest.split_once(' ')?;
                Some(BtDevice { mac: mac.into(), name: name.trim().into(), paired: true, connected: false })
            })
            .collect();
        let _ = tx.send(Event::Bt(list));
    });
}

/// Ce qui se prépare au lancement : réseau, Bluetooth. La boucle montre
/// la couture qui se remplit tant que ce n'est pas prêt (8 s au plus).
pub fn readiness(tx: Sender<Event>) {
    std::thread::spawn(move || {
        let start = Instant::now();
        loop {
            let net = super::system::net().kind != crate::app::NetKind::Unknown;
            let bt = btctl(&["show"]).is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains("Powered: yes"));
            let done = [net, bt].iter().filter(|b| **b).count();
            let _ = tx.send(Event::OsLoading(done as f32 / 2.0, net, bt));
            if done == 2 || start.elapsed() > Duration::from_secs(8) {
                let _ = tx.send(Event::OsReady);
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manettes_seules() {
        // Manettes Bluetooth classique (classe 0x002508), basse consommation (apparence), icône.
        assert!(gamepad_info("Device AC:8E:BD:12:34:56 (public)\n\tName: Xbox Wireless Controller\n\tClass: 0x00002508 (9480)\n"));
        assert!(gamepad_info("\tAppearance: 0x03c4 (964)\n"));
        assert!(gamepad_info("\tIcon: input-gaming\n"));
        // Casque (audio, 0x240404), téléphone (0x5a020c) : non.
        assert!(!gamepad_info("\tName: WH-1000XM4\n\tClass: 0x00240404\n\tIcon: audio-headset\n"));
        assert!(!gamepad_info("\tName: Pixel 8\n\tClass: 0x005a020c\n\tIcon: phone\n"));
        for n in ["Xbox Wireless Controller", "DualSense Wireless Controller", "Wireless Controller", "Pro Controller", "Joy-Con (L)", "8BitDo Pro 2"] {
            assert!(gamepad_name(n), "{n}");
        }
        for n in ["Xbox Wireless Headset", "Razer Barracuda", "Pixel 8", "", "AC-8E-BD-12-34-56"] {
            assert!(!gamepad_name(n), "{n}");
        }
    }
}
