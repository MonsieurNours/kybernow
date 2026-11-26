//! PyroWave (docs/pyrowave.md) : la Station le propose-t-elle ? Les apps ne
//! le demandent que si `GET /api/stream/info` le dit (`pyrowave`), avec un
//! format de repli ; kycontroller et kyavserver vérifient encore
//! `[kyavserver] pyrowave`.
//!
//! La carte graphique doit avoir un Vulkan 1.3 qui convient à PyroWave et
//! savoir importer les textures et les fences de D3D11 : l'outil de PyroWave
//! `pyrowave-device-validation.exe` (dans le dossier de Kyber, surcouche de
//! KyberNow) le vérifie sur la carte qui encode (`--luid`), dans un processus
//! à part (un pilote qui plante n'emporte pas Admin), avec un aller-retour
//! d'encodage et de décodage. Si aucune carte Vulkan ne porte ce LUID (le
//! service d'Admin ne voit pas toujours la carte sous le même), elle est
//! cherchée par son fabricant et son modèle PCI (`--vid`/`--pid`). Une réussite est gardée pour chaque carte, le
//! temps qu'Admin tourne ; un échec, 2 minutes seulement (au démarrage du
//! PC, le pilote peut ne pas être prêt quand Admin vérifie).
//!
//! Station Mac : la puce Apple doit être de la famille Apple7 ou plus (toutes
//! les puces M) et l'encodeur Metal de la Station doit démarrer :
//! `kybernow-pyrowave-check`, à côté d'Admin dans l'app (server/macos/pyrowave),
//! le vérifie une fois par lancement d'Admin, en fond, dans un processus à
//! part, avec un aller-retour d'une mire.
//!
//! Station Linux : le même outil de PyroWave, compilé pour Linux
//! (`runtime/bin/pyrowave-device-validation`, server/linux/scripts/pyrowave-linux.sh),
//! sur la carte du nœud de rendu de l'encodeur (`/dev/dri/renderD128`,
//! `--vid`/`--pid` lus dans `/sys`) : Vulkan 1.3 qui convient, import des
//! DMA-BUF de la capture PipeWire et des descripteurs de synchronisation,
//! aller-retour. Une seule vérification par lancement d'Admin.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Pourquoi la Station ne propose pas PyroWave (dans l'ordre) : `off` (réglé
/// sur non), `windows` (ni une Station Windows, ni Mac, ni Linux), `vulkan`
/// (Windows et Linux : pas de Vulkan 1.3 qui convient sur la carte, ou outil
/// de vérification absent), `metal` (Mac : puce qui ne convient pas, encodeur
/// qui ne démarre pas, ou outil absent) ; `None` : proposé.
pub fn unavailable(enabled: bool, kyber_dir: &Path) -> Option<&'static str> {
    if !enabled {
        return Some("off");
    }
    if cfg!(target_os = "macos") {
        return (!metal_ok()).then_some("metal");
    }
    if cfg!(target_os = "linux") {
        return (!vulkan_ok(kyber_dir, None)).then_some("vulkan");
    }
    if !cfg!(windows) {
        return Some("windows");
    }
    (!vulkan_ok(kyber_dir, crate::gpu::encoding_luid())).then_some("vulkan")
}

/// Le `pyrowave` de `info` : `{ available }`, `reason` s'il ne l'est pas,
/// et `max_mbps` ([`max_mbps`]) quand il est connu.
pub fn info(enabled: bool, max_mbps: Option<u32>, kyber_dir: &Path) -> serde_json::Value {
    let mut v = match unavailable(enabled, kyber_dir) {
        None => serde_json::json!({ "available": true }),
        Some(reason) => serde_json::json!({ "available": false, "reason": reason }),
    };
    if let Some(m) = max_mbps {
        v["max_mbps"] = m.into();
    }
    v
}

/// Part de la vitesse de la prise que PyroWave peut prendre en Auto :
/// Ethernet, le lien moins les en-têtes et le reste du trafic ; Wi-Fi, la
/// vitesse annoncée par la borne est un débit radio, le débit réel en est
/// à peu près la moitié.
const AUTO_SHARE_ETHERNET: f64 = 0.8;
const AUTO_SHARE_WIFI: f64 = 0.5;

/// Le « Débit maximal » annoncé aux apps (`pyrowave.max_mbps`, Mbit/s) :
/// `setting` (réglage `video.pyrowave_max_mbps` : `auto`, `300`, `500`,
/// `800`) ; en Auto (ou valeur inconnue), d'après `link`, la prise de la
/// Station ([`crate::wake::link`]) : 80 % de sa vitesse en Ethernet, 50 % en
/// Wi-Fi, arrondi aux 50 Mbit/s du dessous (1 Gbit/s → 800, 2,5 Gbit/s →
/// 2000, Wi-Fi à 1201 Mbit/s → 600). `None` : prise inconnue, pas de plafond
/// de l'ordinateur (les apps gardent le leur).
pub fn max_mbps(setting: &str, link: Option<(crate::wake::Kind, u32)>) -> Option<u32> {
    if let Ok(v) = setting.parse::<u32>()
        && v > 0
    {
        return Some(v);
    }
    let (kind, mbps) = link?;
    let share = match kind {
        crate::wake::Kind::Ethernet => AUTO_SHARE_ETHERNET,
        crate::wake::Kind::Wifi => AUTO_SHARE_WIFI,
    };
    let v = (f64::from(mbps) * share / 50.0).floor() as u32 * 50;
    (v >= 50).then_some(v)
}

/// Vérifie la carte une fois au démarrage d'Admin, en fond : la première
/// demande d'`info` n'attend pas l'outil.
pub fn warm_up(kyber_dir: &Path) {
    if cfg!(target_os = "macos") {
        std::thread::spawn(metal_ok);
        return;
    }
    let dir = kyber_dir.to_path_buf();
    if cfg!(target_os = "linux") {
        std::thread::spawn(move || vulkan_ok(&dir, None));
        return;
    }
    if !cfg!(windows) {
        return;
    }
    std::thread::spawn(move || {
        // La liste des cartes (et leur LUID) arrive juste après le démarrage.
        std::thread::sleep(std::time::Duration::from_secs(5));
        vulkan_ok(&dir, crate::gpu::encoding_luid());
    });
}

/// Durée pendant laquelle un échec est gardé avant une nouvelle vérification.
const FAILURE_KEPT: std::time::Duration = std::time::Duration::from_secs(120);

fn results() -> &'static Mutex<HashMap<Option<u64>, (bool, std::time::Instant)>> {
    static RESULTS: OnceLock<Mutex<HashMap<Option<u64>, (bool, std::time::Instant)>>> = OnceLock::new();
    RESULTS.get_or_init(Default::default)
}

/// Un résultat gardé vaut-il encore ? Une réussite toujours, un échec
/// [`FAILURE_KEPT`].
fn still_valid(ok: bool, at: std::time::Instant) -> bool {
    ok || at.elapsed() < FAILURE_KEPT
}

/// Appel bloquant (jusqu'à 20 s la première fois pour une carte).
fn vulkan_ok(kyber_dir: &Path, luid: Option<u64>) -> bool {
    // Verrou tenu pendant l'outil : deux demandes simultanées n'en lancent
    // qu'un.
    let mut results = results().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(ok, at)) = results.get(&luid)
        && still_valid(ok, at)
    {
        return ok;
    }
    let ok = run_validation(kyber_dir, luid);
    results.insert(luid, (ok, std::time::Instant::now()));
    ok
}

/// Arguments de l'outil : l'import D3D11 et un aller-retour, sur la carte
/// `luid` (la première carte Vulkan sans LUID connu).
fn validation_args(luid: Option<u64>) -> Vec<String> {
    let mut args = vec!["--external".to_string(), "--roundtrip".to_string(), "--timeout".to_string(), "15".to_string()];
    if let Some(luid) = luid {
        args.push("--luid".to_string());
        args.push(format!("{luid:x}"));
    }
    args
}

#[cfg(windows)]
fn run_validation(kyber_dir: &Path, luid: Option<u64>) -> bool {
    let exe = kyber_dir.join("pyrowave-device-validation.exe");
    if !exe.is_file() {
        tracing::warn!("PyroWave : {} introuvable, pas proposé", exe.display());
        return false;
    }
    let mut output = run_tool(&exe, kyber_dir, validation_args(luid));
    // Code 2 avec un LUID : aucune carte Vulkan ne porte ce LUID. Le service
    // d'Admin (session 0) ne voit pas toujours la carte sous le même LUID que
    // Vulkan (essai du 26/11/2026, RTX 3080 Ti : code 2 avec le LUID, 0 sans) ;
    // la carte est alors cherchée par son fabricant et son modèle PCI.
    if luid.is_some()
        && matches!(&output, Ok(o) if o.status.code() == Some(2))
        && let Some(ids) = crate::gpu::encoding_pci_ids()
    {
        tracing::info!("PyroWave : aucune carte Vulkan au LUID {luid:x?}, essai par fabricant et modèle {:04x}:{:04x}", ids.0, ids.1);
        output = run_tool(&exe, kyber_dir, pci_validation_args(ids));
    }
    match output {
        Ok(o) if o.status.success() => {
            tracing::info!("PyroWave : Vulkan convient (carte {luid:x?})");
            true
        }
        // 2 : pas de carte Vulkan 1.3 qui convient ; 3 : délai dépassé ;
        // 4 : pas d'import D3D11 ; 5 : aller-retour faux.
        Ok(o) => {
            tracing::warn!(
                "PyroWave : Vulkan ne convient pas (carte {luid:x?}, code {:?}), pas proposé ; l'outil dit : {}",
                o.status.code(),
                tool_said(&o.stdout, &o.stderr)
            );
            false
        }
        Err(e) => {
            tracing::warn!("PyroWave : {} ne se lance pas ({e}), pas proposé", exe.display());
            false
        }
    }
}

#[cfg(windows)]
fn run_tool(exe: &Path, dir: &Path, args: Vec<String>) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new(exe)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

/// Arguments de l'outil pour la carte de fabricant et de modèle PCI `ids`
/// (repli quand le LUID ne désigne aucune carte Vulkan).
#[cfg_attr(not(windows), allow(dead_code))]
fn pci_validation_args(ids: (u16, u16)) -> Vec<String> {
    let mut args = validation_args(None);
    args.extend(["--vid".to_string(), format!("{:x}", ids.0), "--pid".to_string(), format!("{:x}", ids.1)]);
    args
}

/// Les dernières lignes que l'outil a écrites (sortie d'erreur d'abord), sur
/// une ligne, pour le journal : pourquoi Vulkan ne convient pas.
#[cfg_attr(not(windows), allow(dead_code))]
fn tool_said(stdout: &[u8], stderr: &[u8]) -> String {
    let text = format!("{}\n{}", String::from_utf8_lossy(stderr), String::from_utf8_lossy(stdout));
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return "rien".into();
    }
    let start = lines.len().saturating_sub(12);
    lines[start..].join(" | ")
}

/// Station Linux : l'outil de la copie de Kyber, à côté d'Admin
/// (`<dossier>/bin/kyber-admin` et `<dossier>/runtime/bin/…`, paquet comme
/// Station d'essai), ou `KYBERNOW_PYROWAVE_VALIDATION`.
#[cfg(target_os = "linux")]
fn validation_tool() -> Option<std::path::PathBuf> {
    std::env::var_os("KYBERNOW_PYROWAVE_VALIDATION")
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| Some(std::env::current_exe().ok()?.parent()?.parent()?.join("runtime/bin/pyrowave-device-validation")))
        .filter(|p| p.is_file())
}

/// Le fournisseur et le modèle PCI d'un nœud de rendu (`/sys/class/drm/renderD128/device/vendor`, « 0x1002 »).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn render_node_ids(sys: &Path) -> Option<(u32, u32)> {
    let read = |name: &str| -> Option<u32> {
        let text = std::fs::read_to_string(sys.join(name)).ok()?;
        u32::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
    };
    Some((read("vendor")?, read("device")?))
}

/// Arguments de l'outil sous Linux : l'import des DMA-BUF et un aller-retour,
/// sur la carte `ids` (la première carte Vulkan sinon).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_validation_args(ids: Option<(u32, u32)>) -> Vec<String> {
    let mut args = validation_args(None);
    if let Some((vid, pid)) = ids {
        args.extend(["--vid".to_string(), format!("{vid:x}"), "--pid".to_string(), format!("{pid:x}")]);
    }
    args
}

#[cfg(target_os = "linux")]
fn run_validation(_kyber_dir: &Path, _luid: Option<u64>) -> bool {
    let Some(tool) = validation_tool() else {
        tracing::warn!("PyroWave : pyrowave-device-validation introuvable, pas proposé");
        return false;
    };
    let node = std::env::var("KYBERNOW_RENDER_NODE").unwrap_or_else(|_| "renderD128".into());
    let node = node.trim_start_matches("/dev/dri/").to_string();
    let ids = render_node_ids(&Path::new("/sys/class/drm").join(&node).join("device"));
    // L'outil peut rester bloqué dans un pilote : 20 s au plus, puis arrêté.
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = match Command::new(&tool).args(linux_validation_args(ids)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("PyroWave : {} ne se lance pas ({e}), pas proposé", tool.display());
            return false;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    let code = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.code(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let card = ids.map(|(v, d)| format!("{v:04x}:{d:04x}")).unwrap_or_else(|| "première carte Vulkan".into());
    match code {
        Some(0) => {
            tracing::info!("PyroWave : Vulkan convient ({node}, {card})");
            true
        }
        // 2 : pas de carte Vulkan 1.3 qui convient ; 3 : délai dépassé ;
        // 4 : pas d'import DMA-BUF ; 5 : aller-retour faux.
        code => {
            tracing::warn!("PyroWave : Vulkan ne convient pas ({node}, {card}, code {code:?}), pas proposé");
            false
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
fn run_validation(_kyber_dir: &Path, _luid: Option<u64>) -> bool {
    false
}

/// Station Mac : `kybernow-pyrowave-check` dit-il que la puce convient et
/// que l'encodeur démarre ? Une fois par lancement d'Admin (la puce ne
/// change pas) ; appel bloquant la première fois (1 s environ, 20 s au plus).
fn metal_ok() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| match check_tool() {
        Some(tool) => run_check(&tool),
        None => {
            tracing::warn!("PyroWave : kybernow-pyrowave-check introuvable, pas proposé");
            false
        }
    })
}

/// `KYBERNOW_PYROWAVE_CHECK`, sinon l'outil à côté d'Admin (Contents/MacOS de
/// l'app KyberNow Station).
fn check_tool() -> Option<std::path::PathBuf> {
    std::env::var_os("KYBERNOW_PYROWAVE_CHECK")
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| Some(std::env::current_exe().ok()?.parent()?.join("kybernow-pyrowave-check")))
        .filter(|p| p.is_file())
}

fn run_check(tool: &Path) -> bool {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = match Command::new(tool).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("PyroWave : {} ne se lance pas ({e}), pas proposé", tool.display());
            return false;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let mut said = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = std::io::Read::read_to_string(&mut out, &mut said);
    }
    let said = said.trim();
    match status.and_then(|s| s.code()) {
        Some(0) => {
            tracing::info!("PyroWave : la puce convient ({said})");
            true
        }
        // 2 : puce qui ne convient pas ; 4 : encodeur qui ne démarre pas ;
        // 5 : aller-retour faux ; None : délai dépassé ou plantage.
        code => {
            tracing::warn!("PyroWave : pas proposé (code {code:?} : {said})");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_comes_first() {
        assert_eq!(unavailable(false, Path::new("nulle-part")), Some("off"));
        assert_eq!(info(false, None, Path::new("nulle-part")), serde_json::json!({ "available": false, "reason": "off" }));
        assert_eq!(info(false, Some(500), Path::new("nulle-part")), serde_json::json!({ "available": false, "reason": "off", "max_mbps": 500 }));
    }

    #[test]
    fn maximum_bitrate_set_or_from_the_port() {
        use crate::wake::Kind::{Ethernet, Wifi};
        assert_eq!(max_mbps("500", Some((Ethernet, 10_000))), Some(500));
        assert_eq!(max_mbps("auto", Some((Ethernet, 1000))), Some(800));
        assert_eq!(max_mbps("auto", Some((Ethernet, 2500))), Some(2000));
        assert_eq!(max_mbps("auto", Some((Ethernet, 100))), Some(50));
        assert_eq!(max_mbps("auto", Some((Wifi, 1201))), Some(600));
        assert_eq!(max_mbps("auto", Some((Ethernet, 10))), None);
        assert_eq!(max_mbps("auto", None), None);
        assert_eq!(max_mbps("n'importe", Some((Ethernet, 1000))), Some(800));
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    #[test]
    fn windows_mac_or_linux_station_only() {
        assert_eq!(unavailable(true, Path::new("nulle-part")), Some("windows"));
    }

    #[test]
    fn linux_checks_the_card_of_the_render_node() {
        let dir = std::env::temp_dir().join(format!("kn-pyrowave-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("vendor"), "0x1002\n").unwrap();
        std::fs::write(dir.join("device"), "0x744c\n").unwrap();
        assert_eq!(render_node_ids(&dir), Some((0x1002, 0x744c)));
        assert_eq!(&linux_validation_args(Some((0x1002, 0x744c)))[4..], ["--vid", "1002", "--pid", "744c"]);
        assert_eq!(linux_validation_args(None), ["--external", "--roundtrip", "--timeout", "15"]);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(render_node_ids(&dir), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn without_a_matching_luid_the_card_is_found_by_its_pci_ids() {
        assert_eq!(
            pci_validation_args((0x10de, 0x2208)),
            ["--external", "--roundtrip", "--timeout", "15", "--vid", "10de", "--pid", "2208"]
        );
    }

    #[test]
    fn the_tool_output_goes_to_the_log_on_one_line() {
        assert_eq!(tool_said(b"", b""), "rien");
        assert_eq!(tool_said(b"ok\n", b"[ERROR]: vkCreateInstance failed\n\n"), "[ERROR]: vkCreateInstance failed | ok");
        let many: String = (0..20).map(|i| format!("l{i}\n")).collect();
        assert!(tool_said(b"", many.as_bytes()).starts_with("l8 | "));
    }

    #[test]
    fn a_failure_is_checked_again_after_two_minutes() {
        let now = std::time::Instant::now();
        assert!(still_valid(true, now));
        assert!(still_valid(false, now), "échec récent : gardé");
        if let Some(old) = now.checked_sub(FAILURE_KEPT + std::time::Duration::from_secs(1)) {
            assert!(!still_valid(false, old), "échec ancien : nouvelle vérification");
            assert!(still_valid(true, old), "réussite : gardée");
        }
    }

    #[test]
    fn a_mac_without_the_check_tool_does_not_offer_it() {
        // Les tests tournent sans l'outil à côté d'eux : « metal ».
        if check_tool().is_none() {
            assert_eq!(unavailable(true, Path::new("nulle-part")), Some("metal"));
        }
    }

    #[test]
    fn the_tool_checks_the_encoding_card() {
        assert_eq!(validation_args(None), ["--external", "--roundtrip", "--timeout", "15"]);
        let args = validation_args(Some(0x1_0000_C5E4));
        assert_eq!(&args[4..], ["--luid", "10000c5e4"]);
    }
}
