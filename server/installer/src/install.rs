//! Installation de KyberNow, tâche par tâche.
//!
//! Les identifiants de tâche correspondent aux lignes de l'écran « Installation » :
//! files, cert, service, firewall, vdd, gamepad, audio, admin, power (puis finish, sans ligne).

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::checks::SystemInfo;
use crate::events::{Emit, Event, InstallRequest, TaskStatus};
use crate::sys::{self, run, run_ok};

pub const SUDOVDA_HWID: &str = r"root\sudomaker\sudovda";
pub const DISPLAY_CLASS_GUID: &str = "4D36E968-E325-11CE-BFC1-08002BE10318";
pub const UNINSTALL_KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\KyberNow";
/// Icône de la zone de notification, relative au dossier d'installation.
pub const TRAY_EXE: &str = r"tray\kybernow-tray.exe";
/// Lancement de l'icône à l'ouverture de session (valeur `KyberNow`).
pub const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
/// Droits des services `kyber` et `kyber-admin` : ceux de Windows par défaut,
/// plus démarrer, arrêter et suspendre (RP, WP, DT) pour les utilisateurs
/// interactifs (IU), afin que l'icône pilote le serveur sans élévation.
/// Changer la configuration (DC) reste réservé aux administrateurs.
pub const SERVICE_SDDL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)\
    (A;;CCLCSWRPWPDTLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)";
pub const FIREWALL_RULES: &[&str] = &[
    "KyberNow-stream-TCP",
    "KyberNow-stream-UDP",
    "KyberNow-console-TCP",
    "KyberNow-kycontroller",
    "KyberNow-kyavserver",
    "KyberNow-kynputserver",
];

/// Ce qui a été installé, relu par le désinstalleur et par la console.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    pub version: String,
    pub kyber_version: String,
    pub stream_port: u16,
    pub console_port: u16,
    pub virtual_display: bool,
    pub gamepads: bool,
    pub sudovda_thumbprint: Option<String>,
}

struct Ctx<'a> {
    emit: &'a Emit,
    dir: PathBuf,
    log: std::sync::Mutex<Vec<String>>,
}

impl Ctx<'_> {
    fn log(&self, line: impl Into<String>) {
        let line = line.into();
        (self.emit)(Event::Log { line: line.clone() });
        self.log.lock().unwrap().push(line);
    }
    fn progress(&self, percent: f32) {
        (self.emit)(Event::Progress { percent });
    }
    fn task(&self, id: &str, status: TaskStatus) {
        (self.emit)(Event::Task { id: id.into(), status });
    }
    fn kyber(&self) -> PathBuf {
        self.dir.join("kyber")
    }
    fn path(&self, rel: &str) -> String {
        self.dir.join(rel).display().to_string()
    }
}

/// Lance l'installation complète. Renvoie `true` si un redémarrage est nécessaire.
pub fn run_install(req: &InstallRequest, info: &SystemInfo, payload: &Path, emit: &Emit) -> Result<bool> {
    let ctx = Ctx { emit, dir: sys::install_dir(), log: Default::default() };
    let result = if cfg!(windows) { install(&ctx, req, info, payload) } else { simulate(&ctx, req) };
    let log = ctx.log.lock().unwrap().join("\n");
    let _ = std::fs::write(std::env::temp_dir().join("KyberNow-install.log"), &log);
    if ctx.dir.exists() {
        let _ = std::fs::write(ctx.dir.join("install.log"), &log);
    }
    result
}

fn step<T>(ctx: &Ctx, id: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    ctx.task(id, TaskStatus::Running);
    match f() {
        Ok(v) => {
            ctx.task(id, TaskStatus::Done);
            Ok(v)
        }
        Err(e) => {
            ctx.task(id, TaskStatus::Failed);
            ctx.log(format!("ÉCHEC : {e:#}"));
            (ctx.emit)(Event::Failed { task: id.into(), message: format!("{e:#}") });
            Err(e)
        }
    }
}

fn install(ctx: &Ctx, req: &InstallRequest, info: &SystemInfo, payload: &Path) -> Result<bool> {
    let mut need_reboot = false;

    step(ctx, "files", || {
        prepare(ctx, info)?;
        ctx.progress(4.0);
        ctx.log(format!("Extraction vers {}", ctx.dir.display()));
        let count = crate::payload::extract(payload, &ctx.dir, |p, _| ctx.progress(4.0 + p * 36.0))?;
        ctx.log(format!("{count} fichiers installés"));
        Ok(())
    })?;

    step(ctx, "cert", || {
        let kyber = ctx.kyber();
        let out = run_ok(&kyber.join("cert-gen.exe").display().to_string(), &["--output", &kyber.display().to_string()], &[0])?;
        ctx.log(format!("Certificat TLS généré. {}", sys::text(&out).trim()));
        ctx.progress(44.0);
        Ok(())
    })?;

    step(ctx, "service", || {
        let admin = ctx.path(r"admin\kyber-admin.exe");
        let mut cmd = std::process::Command::new(&admin);
        cmd.args([
            "init",
            "--user", &req.user,
            "--kyber-port", &req.stream_port.to_string(),
            "--encoder", &info.encoder,
            "--kyber-dir", &ctx.kyber().display().to_string(),
            "--data-dir", &sys::data_dir().display().to_string(),
            "--listen", &format!("0.0.0.0:{}", req.console_port),
        ]);
        cmd.env("KYBERNOW_PASSWORD", &req.password);
        hide_window(&mut cmd);
        let out = cmd.output().context("configuration initiale")?;
        if !out.status.success() {
            anyhow::bail!("configuration initiale : {}", sys::text(&out).trim());
        }
        ctx.log(format!("Configuration : port {}, encodeur {}, compte « {} »", req.stream_port, info.encoder, req.user));

        let bin = format!("\"{}\"", ctx.kyber().join("kyservice.exe").display());
        let start = if req.start_with_windows { "auto" } else { "demand" };
        run_ok("sc", &["create", "kyber", "binPath=", &bin, "start=", start, "DisplayName=", "KyberNow - serveur de streaming"], &[0])?;
        let _ = run("sc", &["description", "kyber", "Serveur de streaming Kyber, installé et géré par KyberNow."]);
        let _ = run("sc", &["failure", "kyber", "reset=", "86400", "actions=", "restart/5000/restart/5000/restart/30000"]);
        run_ok("sc", &["sdset", "kyber", SERVICE_SDDL], &[0])?;
        ctx.log(format!("Service Windows « kyber » créé (démarrage {start})"));
        ctx.progress(52.0);
        Ok(())
    })?;

    step(ctx, "firewall", || {
        if let Some(index) = info.public_interface {
            let r = sys::powershell(&format!(
                "Set-NetConnectionProfile -InterfaceIndex {index} -NetworkCategory Private"
            ));
            ctx.log(format!("Réseau passé en « Privé » : {}", if r.is_ok() { "réussi" } else { "échec" }));
        }
        remove_firewall_rules();
        let profile = "profile=private,domain";
        let port = |name: &str, proto: &str, port: u16| -> Result<()> {
            run_ok("netsh", &["advfirewall", "firewall", "add", "rule", &format!("name={name}"), "dir=in", "action=allow",
                &format!("protocol={proto}"), &format!("localport={port}"), profile], &[0])?;
            Ok(())
        };
        port("KyberNow-stream-TCP", "TCP", req.stream_port)?;
        port("KyberNow-stream-UDP", "UDP", req.stream_port)?;
        port("KyberNow-console-TCP", "TCP", req.console_port)?;
        for exe in ["kycontroller", "kyavserver", "kynputserver"] {
            let program = format!("program={}", ctx.kyber().join(format!("{exe}.exe")).display());
            run_ok("netsh", &["advfirewall", "firewall", "add", "rule", &format!("name=KyberNow-{exe}"), "dir=in",
                "action=allow", &program, "enable=yes", profile], &[0])?;
        }
        ctx.log(format!("Pare-feu : ports {} (TCP/UDP) et {} (TCP) ouverts sur le réseau privé", req.stream_port, req.console_port));
        ctx.progress(60.0);
        Ok(())
    })?;

    let mut thumbprint = None;
    if req.virtual_display {
        step(ctx, "vdd", || {
            let dir = ctx.dir.join(r"drivers\sudovda");
            let cer = dir.join("sudovda.cer");
            thumbprint = cert_thumbprint(&cer).ok();
            let cer_s = cer.display().to_string();
            run_ok("certutil", &["-addstore", "-f", "Root", &cer_s], &[0])?;
            run_ok("certutil", &["-addstore", "-f", "TrustedPublisher", &cer_s], &[0])?;
            let nefconc = dir.join("nefconc.exe").display().to_string();
            let _ = run(&nefconc, &["--remove-device-node", "--hardware-id", SUDOVDA_HWID, "--class-guid", DISPLAY_CLASS_GUID]);
            run_ok(&nefconc, &["--create-device-node", "--class-name", "Display", "--class-guid", DISPLAY_CLASS_GUID, "--hardware-id", SUDOVDA_HWID], &[0])?;
            let inf = dir.join("SudoVDA.inf").display().to_string();
            let out = run_ok(&nefconc, &["--install-driver", "--inf-path", &inf], &[0, 3010])?;
            if out.status.code() == Some(3010) {
                need_reboot = true;
            }
            ctx.log("Pilote d'écran virtuel SudoVDA installé");
            ctx.progress(70.0);
            Ok(())
        })?;
    } else {
        ctx.task("vdd", TaskStatus::Skipped);
    }

    if req.gamepads {
        step(ctx, "gamepad", || {
            if sys::system_root().join(r"System32\drivers\ViGEmBus.sys").exists() {
                ctx.log("ViGEmBus déjà présent : rien à faire");
            } else {
                let installer = std::fs::read_dir(ctx.dir.join(r"drivers\vigembus"))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")))
                    .context("installeur ViGEmBus absent du contenu")?;
                let out = run_ok(&installer.display().to_string(), &["/quiet", "/norestart"], &[0, 1641, 3010])?;
                need_reboot = true;
                ctx.log(format!("Pilote de manettes ViGEmBus installé (code {})", out.status.code().unwrap_or(0)));
            }
            ctx.progress(82.0);
            Ok(())
        })?;
    } else {
        ctx.task("gamepad", TaskStatus::Skipped);
    }

    match (&info.steam_speakers_inf, req.virtual_audio) {
        (Some(inf), true) => step(ctx, "audio", || {
            let inf = inf.display().to_string();
            let out = run_ok("pnputil", &["/add-driver", &inf, "/install"], &[0, 259, 3010])?;
            if out.status.code() == Some(3010) {
                need_reboot = true;
            }
            ctx.log("Haut-parleurs virtuels de Steam installés : le son du PC sera coupé pendant le stream");
            ctx.progress(86.0);
            Ok(())
        })?,
        _ => ctx.task("audio", TaskStatus::Skipped),
    }

    step(ctx, "admin", || {
        let admin = ctx.path(r"admin\kyber-admin.exe");
        run_ok(&admin, &["service", "install"], &[0])?;
        run_ok("sc", &["sdset", "kyber-admin", SERVICE_SDDL], &[0])?;
        if !req.start_with_windows {
            run_ok("sc", &["config", "kyber-admin", "start=", "demand"], &[0])?;
        }
        ctx.log(format!("Console d'administration démarrée sur le port {}", req.console_port));
        ctx.progress(92.0);
        Ok(())
    })?;

    if req.max_performance {
        step(ctx, "power", || {
            run_ok("powercfg", &["/setactive", "SCHEME_MIN"], &[0])?;
            ctx.log("Mode d'alimentation : Performances élevées");
            ctx.progress(96.0);
            Ok(())
        })?;
    } else {
        ctx.task("power", TaskStatus::Skipped);
    }

    step(ctx, "finish", || {
        let record = InstallRecord {
            version: crate::VERSION.into(),
            kyber_version: crate::KYBER_VERSION.into(),
            stream_port: req.stream_port,
            console_port: req.console_port,
            virtual_display: req.virtual_display,
            gamepads: req.gamepads,
            sudovda_thumbprint: thumbprint.clone(),
        };
        std::fs::write(ctx.dir.join("kybernow.json"), serde_json::to_vec_pretty(&record)?)?;
        let uninstaller = ctx.dir.join("uninstall.exe");
        crate::payload::copy_stub(payload, &uninstaller)?;
        register_uninstaller(ctx, &uninstaller)?;
        write_shortcut(req.console_port);
        run_ok("sc", &["start", "kyber"], &[0, 1056])?; // 1056 : déjà démarré
        ctx.log("Serveur de streaming démarré");
        setup_tray(ctx, req.start_with_windows);
        ctx.progress(100.0);
        Ok(())
    })?;

    Ok(need_reboot)
}

/// Arrête et retire ce qui occupe la place : ancienne version, Kyber officiel.
fn prepare(ctx: &Ctx, info: &SystemInfo) -> Result<()> {
    for service in ["kyber-admin", "kyber"] {
        if run("sc", &["query", service]).is_ok_and(|o| o.status.success()) {
            let _ = run("sc", &["stop", service]);
            wait_stopped(service);
            ctx.log(format!("Service « {service} » arrêté"));
        }
    }
    if let Some(uninstaller) = &info.kyber_uninstaller {
        ctx.log("Retrait de l'installation Kyber officielle");
        remove_official_kyber(uninstaller);
    }
    for service in ["kyber-admin", "kyber"] {
        if run("sc", &["query", service]).is_ok_and(|o| o.status.success()) {
            let _ = run("sc", &["delete", service]);
        }
    }
    for exe in ["kyclient.exe", "kycontroller.exe", "kyavserver.exe", "kynputserver.exe", "kyservice.exe", "kyber-admin.exe", "kybernow-tray.exe"] {
        let _ = run("taskkill", &["/F", "/IM", exe]);
    }
    Ok(())
}

/// Désinstalle Kyber officiel (désinstalleur NSIS) et attend qu'il ait fini.
///
/// Sans `_?=`, un désinstalleur NSIS se recopie ailleurs et rend la main tout
/// de suite : il risquerait de supprimer le service que nous recréons ensuite.
/// `_?=` doit être le dernier paramètre, sans guillemets.
fn remove_official_kyber(uninstaller: &str) {
    let dir = Path::new(uninstaller).parent().map(|p| p.display().to_string()).unwrap_or_default();
    let mut cmd = std::process::Command::new(uninstaller);
    cmd.arg("/S");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.raw_arg(format!("_?={dir}"));
    }
    hide_window(&mut cmd);
    let _ = cmd.status();
    // Le désinstalleur reste dans son dossier avec _?= : on le retire ensuite.
    let _ = std::fs::remove_file(uninstaller);
    let _ = std::fs::remove_dir(&dir);
}

/// Attend qu'un service soit arrêté (20 s au plus).
pub fn wait_stopped(service: &str) {
    for _ in 0..40 {
        let Ok(out) = run("sc", &["query", service]) else { return };
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        // « STATE : 1 STOPPED » : le chiffre ne dépend pas de la langue de Windows.
        let stopped = text.lines().any(|l| {
            l.contains(':') && l.split(':').nth(1).is_some_and(|v| v.split_whitespace().next() == Some("1"))
                && l.to_uppercase().contains("STATE")
        });
        if stopped || !out.status.success() {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

pub fn remove_firewall_rules() {
    for name in FIREWALL_RULES {
        let _ = run("netsh", &["advfirewall", "firewall", "delete", "rule", &format!("name={name}")]);
    }
}

fn cert_thumbprint(cer: &Path) -> Result<String> {
    use sha1::Digest;
    let der = std::fs::read(cer)?;
    Ok(hex::encode(sha1::Sha1::digest(&der)))
}

fn register_uninstaller(ctx: &Ctx, uninstaller: &Path) -> Result<()> {
    let exe = uninstaller.display().to_string();
    let add = |name: &str, kind: &str, value: &str| -> Result<()> {
        run_ok("reg", &["add", UNINSTALL_KEY, "/v", name, "/t", kind, "/d", value, "/f"], &[0])?;
        Ok(())
    };
    add("DisplayName", "REG_SZ", crate::PRODUCT)?;
    add("DisplayVersion", "REG_SZ", crate::VERSION)?;
    add("Publisher", "REG_SZ", crate::PUBLISHER)?;
    add("URLInfoAbout", "REG_SZ", crate::PUBLISHER_URL)?;
    add("InstallLocation", "REG_SZ", &ctx.dir.display().to_string())?;
    add("DisplayIcon", "REG_SZ", &exe)?;
    add("UninstallString", "REG_SZ", &format!("\"{exe}\" --uninstall"))?;
    add("NoModify", "REG_DWORD", "1")?;
    add("NoRepair", "REG_DWORD", "1")?;
    let kb = dir_size(&ctx.dir) / 1024;
    add("EstimatedSize", "REG_DWORD", &kb.to_string())?;
    Ok(())
}

/// Icône de la zone de notification : raccourci « KyberNow » dans le menu
/// Démarrer, lancement à l'ouverture de session, puis lancement tout de suite.
///
/// L'installateur tourne en administrateur ; l'icône doit tourner sans. On la
/// lance donc par l'Explorateur, qui la démarre dans la session de
/// l'utilisateur avec ses droits ordinaires.
fn setup_tray(ctx: &Ctx, start_with_windows: bool) {
    let exe = ctx.path(TRAY_EXE);
    if !Path::new(&exe).exists() {
        ctx.log("Icône de la zone de notification absente du contenu");
        return;
    }
    let lnk = PathBuf::from(std::env::var_os("ProgramData").unwrap_or_default())
        .join(r"Microsoft\Windows\Start Menu\Programs\KyberNow\KyberNow.lnk");
    let _ = sys::powershell(&format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{}'); $s.TargetPath = '{}'; \
         $s.Description = 'KyberNow dans la zone de notification'; $s.Save()",
        lnk.display(),
        exe
    ));
    let _ = if start_with_windows {
        run("reg", &["add", RUN_KEY, "/v", "KyberNow", "/t", "REG_SZ", "/d", &format!("\"{exe}\""), "/f"])
    } else {
        run("reg", &["delete", RUN_KEY, "/v", "KyberNow", "/f"])
    };
    let _ = std::process::Command::new(sys::system_root().join("explorer.exe")).arg(&exe).spawn();
    ctx.log(if start_with_windows {
        "Icône KyberNow lancée (et à chaque ouverture de session)"
    } else {
        "Icône KyberNow lancée"
    });
}

/// Raccourci « Console KyberNow » dans le menu Démarrer.
fn write_shortcut(console_port: u16) {
    let dir = PathBuf::from(std::env::var_os("ProgramData").unwrap_or_default())
        .join(r"Microsoft\Windows\Start Menu\Programs\KyberNow");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("Console KyberNow.url"),
        format!("[InternetShortcut]\r\nURL=https://localhost:{console_port}/\r\n"),
    );
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| match e.metadata() {
                    Ok(m) if m.is_dir() => dir_size(&e.path()),
                    Ok(m) => m.len(),
                    Err(_) => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

fn hide_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let _ = cmd;
}

/// Hors Windows : déroulé simulé, pour tester l'interface sur Mac.
fn simulate(ctx: &Ctx, req: &InstallRequest) -> Result<bool> {
    let tasks = [
        ("files", 40.0, "Extraction des fichiers (simulation)"),
        ("cert", 44.0, "Certificat TLS généré (simulation)"),
        ("service", 52.0, "Service Windows créé (simulation)"),
        ("firewall", 60.0, "Pare-feu configuré (simulation)"),
        ("vdd", 70.0, "Pilote SudoVDA installé (simulation)"),
        ("gamepad", 82.0, "Pilote ViGEmBus installé (simulation)"),
        ("audio", 86.0, "Haut-parleurs virtuels de Steam installés (simulation)"),
        ("admin", 92.0, "Console démarrée (simulation)"),
        ("power", 96.0, "Alimentation réglée (simulation)"),
        ("finish", 100.0, "Terminé (simulation)"),
    ];
    let mut last = 0.0;
    for (id, target, line) in tasks {
        let skip = (id == "vdd" && !req.virtual_display) || (id == "gamepad" && !req.gamepads) || (id == "audio" && !req.virtual_audio) || (id == "power" && !req.max_performance);
        if skip {
            ctx.task(id, TaskStatus::Skipped);
            continue;
        }
        ctx.task(id, TaskStatus::Running);
        let steps = 12;
        for i in 1..=steps {
            std::thread::sleep(Duration::from_millis(90));
            ctx.progress(last + (target - last) * i as f32 / steps as f32);
        }
        last = target;
        ctx.log(line);
        ctx.task(id, TaskStatus::Done);
    }
    Ok(req.gamepads)
}
