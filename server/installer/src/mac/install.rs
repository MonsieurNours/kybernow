//! Installation de KyberNow Station sur un Mac, dans l'ordre de l'écran
//! « Installation » : moteur Kyber (l'app dans Applications), certificat,
//! sortie son, démarrage avec le Mac, Admin, pare-feu. Le mot de passe du Mac
//! est demandé une fois, au début (`root`) : copie de l'app, sortie son,
//! pare-feu et réveil par le réseau.

use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::checks::MacInfo;
use super::commands::{Emit, Event, InstallRequest, TaskStatus};
use super::root::{self, quote, quote_path};
use super::{Record, app_exe, app_resource};
use crate::i18n::t;

/// Journal (`~/Library/Logs/KyberNow/install.log`) et messages à la fenêtre.
struct Ctx<'a> {
    emit: &'a Emit,
    log: Mutex<Option<fs::File>>,
    /// Tâche en cours, nommée dans l'échec.
    current: Mutex<String>,
}

impl Ctx<'_> {
    fn log(&self, line: impl AsRef<str>) {
        let line = line.as_ref();
        if let Some(f) = self.log.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{line}");
        }
        (self.emit)(Event::Log { line: line.to_string() });
    }

    fn task(&self, id: &str, status: TaskStatus, detail: Option<String>) {
        (self.emit)(Event::Task { id: id.into(), status, detail });
    }

    fn progress(&self, percent: f32) {
        (self.emit)(Event::Progress { percent });
    }
}

/// Tâches et progression à leur fin.
const PLAN: [(&str, f32); 6] = [("kyber", 35.0), ("cert", 42.0), ("audio", 55.0), ("startup", 68.0), ("admin", 92.0), ("firewall", 100.0)];

pub fn run(req: &InstallRequest, info: &MacInfo, payload: &Path, emit: &Emit) -> Result<()> {
    let _ = fs::create_dir_all(super::logs_dir());
    let log = fs::OpenOptions::new().create(true).append(true).open(super::logs_dir().join("install.log")).ok();
    let ctx = Ctx { emit, log: Mutex::new(log), current: Mutex::default() };
    ctx.log(format!("KyberNow Station {} pour Mac : installation ({})", crate::VERSION, if req.keep_settings { "réglages gardés" } else { "première installation" }));

    let result = steps(&ctx, req, info, payload);
    if let Err(e) = &result {
        let task = ctx.current.lock().unwrap().clone();
        let message = if root::is_cancelled(e) { t("mac.install.cancelled", &[]) } else { format!("{e:#}") };
        ctx.log(format!("ÉCHEC ({task}) : {e:#}"));
        ctx.task(&task, TaskStatus::Failed, None);
        emit(Event::Failed { task, message });
    }
    result
}

fn start(ctx: &Ctx, id: &str) {
    *ctx.current.lock().unwrap() = id.to_string();
    ctx.task(id, TaskStatus::Running, None);
}

fn finish(ctx: &Ctx, id: &str, status: TaskStatus, detail: Option<String>) {
    ctx.task(id, status, detail);
    let percent = PLAN.iter().find(|(t, _)| *t == id).map_or(0.0, |(_, p)| *p);
    ctx.progress(percent);
}

fn steps(ctx: &Ctx, req: &InstallRequest, info: &MacInfo, payload: &Path) -> Result<()> {
    let app = super::app_path();
    if !app_exe(payload, "kycontroller").is_file() {
        bail!(t("mac.install.payloadMissing", &[("path", &payload.display())]));
    }

    // Station déjà en marche (réinstallation, Station d'essai) : arrêtée avant
    // de remplacer l'app. L'agent des écrans rallume les écrans du Mac s'il
    // les avait éteints.
    for label in super::LABELS.iter().rev() {
        if super::loaded(label) {
            ctx.log(format!("arrêt de {label}"));
            super::bootout(label);
        }
    }

    // 1. L'app dans Applications, la sortie son, le pare-feu, le réveil : en administrateur.
    start(ctx, "kyber");
    ctx.progress(4.0);
    ctx.log(format!("copie de {} vers {} (mot de passe du Mac)", payload.display(), app.display()));
    let out = root::run(&root_script(req, info, payload, &app), &t("mac.install.passwordPrompt", &[]))?;
    // AppleScript rend les fins de ligne du script en retours chariot.
    for line in out.split(['\n', '\r']).filter(|l| !l.trim().is_empty()) {
        ctx.log(format!("  {line}"));
    }
    if !app_exe(&app, "kycontroller").is_file() {
        bail!(t("mac.install.copyFailed", &[("path", &app.display())]));
    }
    finish(ctx, "kyber", TaskStatus::Done, None);

    // 2. Réglages et certificat de Kyber.
    start(ctx, "cert");
    let station = super::station_dir();
    fs::create_dir_all(super::kyber_dir())?;
    fs::create_dir_all(super::admin_dir())?;
    set_mode(&station, 0o700);
    let config = super::kyber_dir().join("kyber_config.toml");
    let kept = req.keep_settings && config.is_file();
    if !kept {
        fs::copy(app_resource(&app, "kyber/kyber_config.toml"), &config).context("kyber_config.toml")?;
        certificate(ctx)?;
    }
    configure_kyber(&config, &app)?;
    finish(ctx, "cert", TaskStatus::Done, kept.then(|| t("mac.install.certKept", &[])));

    // 3. Sortie son : posée par l'étape administrateur, on attend que Core Audio la montre.
    start(ctx, "audio");
    if req.virtual_audio {
        if wait_for_speakers(ctx, &app) {
            finish(ctx, "audio", TaskStatus::Done, None);
        } else {
            ctx.log("ATTENTION : la sortie son KyberNow n'apparaît pas encore (Core Audio)");
            finish(ctx, "audio", TaskStatus::Warn, Some(t("mac.install.audioLate", &[])));
        }
    } else {
        finish(ctx, "audio", TaskStatus::Skipped, None);
    }

    // 4. Services de la session : Kyber, Admin, l'icône.
    start(ctx, "startup");
    let record = Record {
        version: crate::VERSION.into(),
        kyber_version: crate::KYBER_VERSION.into(),
        stream_port: req.stream_port,
        console_port: req.console_port,
        virtual_display: req.virtual_display,
        virtual_audio: req.virtual_audio,
        start_with_mac: req.start_with_mac,
        keep_reachable: req.keep_reachable,
        // Admin la réécrit à chaque démarrage.
        console_fingerprint: super::read_record().and_then(|r| r.console_fingerprint),
    };
    super::write_record(&record)?;
    fs::create_dir_all(super::agents_dir())?;
    for label in super::LABELS {
        let plist = plist(label, &app, req.start_with_mac);
        fs::write(super::plist_path(label), plist).with_context(|| super::plist_path(label).display().to_string())?;
    }
    load(ctx, super::LABEL_KYBER)?;
    if !wait_port(req.stream_port, Duration::from_secs(20)) {
        let last = fs::read_to_string(super::logs_dir().join("kyber.log"))
            .ok()
            .and_then(|l| l.lines().rev().find(|l| l.contains("ERROR")).map(str::to_string))
            .unwrap_or_default();
        ctx.log(format!("Kyber ne répond pas : {last}"));
        bail!(t("mac.install.kyberSilent", &[("port", &req.stream_port), ("error", &last)]));
    }
    ctx.log(format!("Kyber répond sur le port {}", req.stream_port));
    finish(ctx, "startup", TaskStatus::Done, (!req.start_with_mac).then(|| t("mac.install.startupOff", &[])));

    // 5. Admin : compte, certificat et port à la première installation, puis le service.
    start(ctx, "admin");
    if !kept {
        init_admin(ctx, req, &app)?;
    }
    load(ctx, super::LABEL_ADMIN)?;
    if !wait_port(req.console_port, Duration::from_secs(40)) {
        bail!(t("mac.install.adminSilent", &[("port", &req.console_port)]));
    }
    ctx.log(format!("Admin répond sur le port {}", req.console_port));
    finish(ctx, "admin", TaskStatus::Done, None);

    // 6. Pare-feu : réglé par l'étape administrateur s'il était allumé.
    start(ctx, "firewall");
    let detail = if info.firewall_on { t("mac.install.firewallOn", &[]) } else { t("mac.install.firewallOff", &[]) };
    ctx.log(&detail);
    finish(ctx, "firewall", TaskStatus::Done, Some(detail));

    // L'icône de la barre des menus, en dernier : la Station est prête.
    load(ctx, super::LABEL_MENUBAR)?;
    ctx.log("installation terminée");
    (ctx.emit)(Event::Done { lan_ip: info.lan_ip.clone().or_else(crate::checks::lan_ip), stream_port: req.stream_port, console_port: req.console_port });
    Ok(())
}

/// Script de l'étape administrateur.
fn root_script(req: &InstallRequest, info: &MacInfo, payload: &Path, app: &Path) -> String {
    let user = quote(super::user_name());
    let (src, dst) = (quote_path(payload), quote_path(app));
    let new = quote(format!("{}.new", app.display()));
    let mut s = format!(
        "rm -rf {new}\n/usr/bin/ditto {src} {new}\nrm -rf {dst}\nmv {new} {dst}\nchown -R {user}:staff {dst}\n/usr/bin/xattr -dr com.apple.quarantine {dst} 2>/dev/null || true\necho \"app : {}\"\n",
        app.display()
    );
    if req.virtual_audio {
        let driver = quote_path(&app_resource(app, super::DRIVER_NAME));
        let target = quote(format!("{}/{}", super::HAL_DIR, super::DRIVER_NAME));
        let staged = quote(format!("{}/{}.new", super::HAL_DIR, super::DRIVER_NAME));
        // Même pilote déjà en place : Core Audio n'est pas relancé (le son ne coupe pas).
        s.push_str(&format!(
            "if ! /usr/bin/diff -rq {driver} {target} >/dev/null 2>&1; then\n  mkdir -p {hal}\n  rm -rf {staged}\n  /usr/bin/ditto {driver} {staged}\n  rm -rf {target}\n  mv {staged} {target}\n  chown -R root:wheel {target}\n  /usr/bin/killall coreaudiod 2>/dev/null || true\n  echo 'sortie son : posée'\nelse\n  echo 'sortie son : déjà à jour'\nfi\n",
            hal = quote(super::HAL_DIR)
        ));
    }
    if info.firewall_on {
        let fw = "/usr/libexec/ApplicationFirewall/socketfilterfw";
        for exe in ["kycontroller", "kyber-admin"] {
            let path = quote_path(&app_exe(app, exe));
            s.push_str(&format!("{fw} --add {path} >/dev/null 2>&1 || true\n{fw} --unblockapp {path} >/dev/null 2>&1 || true\n"));
        }
        s.push_str("echo 'pare-feu : Kyber et Admin autorisés'\n");
    }
    if req.keep_reachable {
        s.push_str("/usr/bin/pmset -a womp 1 && echo 'réveil par le réseau : activé'\n");
    }
    s
}

fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
}

/// Réglages d'`openssl req` : certificat X.509 version 3 (rustls, dans
/// Kyber, refuse les versions 1 que LibreSSL fait sans extensions).
const CERT_CONFIG: &str = "[req]\ndistinguished_name = dn\nx509_extensions = v3\nprompt = no\n[dn]\nCN = KyberNow Station\n[v3]\nbasicConstraints = critical,CA:FALSE\nkeyUsage = critical,digitalSignature\nextendedKeyUsage = serverAuth\nsubjectAltName = DNS:localhost,DNS:kybernow.station\nsubjectKeyIdentifier = hash\n";

/// Certificat propre à cette Station (pas celui d'essai, public, de Kyber),
/// par l'`openssl` de macOS (LibreSSL). Deux pièges, vus à l'essai du
/// 03/11/2026 : `req -newkey ec` y écrit la courbe en entier au lieu de son
/// nom (rustls : « failed to parse private key »), et sans extensions le
/// certificat est en version 1 (« UnsupportedCertVersion »). D'où une clé
/// faite par `ecparam` (courbe nommée), mise en PKCS#8, et des extensions v3.
fn certificate(ctx: &Ctx) -> Result<()> {
    let dir = super::kyber_dir();
    let (key, cert) = (dir.join("kyber_key.pem"), dir.join("kyber_cert.pem"));
    let (sec1, config) = (dir.join("kyber_key.sec1.tmp"), dir.join("openssl.cnf.tmp"));
    fs::write(&config, CERT_CONFIG)?;
    let openssl = |args: &[&std::ffi::OsStr]| -> Result<()> {
        let out = std::process::Command::new("/usr/bin/openssl").args(args).output().context("openssl")?;
        if !out.status.success() {
            bail!("openssl : {}", super::output_text(&out));
        }
        Ok(())
    };
    let os = |s: &'static str| std::ffi::OsStr::new(s);
    let result = openssl(&[os("ecparam"), os("-name"), os("prime256v1"), os("-genkey"), os("-noout"), os("-out"), sec1.as_os_str()])
        .and_then(|_| openssl(&[os("pkcs8"), os("-topk8"), os("-nocrypt"), os("-in"), sec1.as_os_str(), os("-out"), key.as_os_str()]))
        .and_then(|_| {
            openssl(&[os("req"), os("-new"), os("-x509"), os("-key"), key.as_os_str(), os("-days"), os("3650"), os("-config"), config.as_os_str(), os("-out"), cert.as_os_str()])
        });
    let _ = fs::remove_file(&sec1);
    let _ = fs::remove_file(&config);
    result?;
    set_mode(&key, 0o600);
    ctx.log(format!("certificat de Kyber : {}", cert.display()));
    Ok(())
}

/// Chemins de `kyber_config.toml` : certificat de la Station et moteur du
/// client web (dans l'app). Refait à chaque installation : l'app peut avoir
/// changé de place.
fn configure_kyber(config: &Path, app: &Path) -> Result<()> {
    let dir = super::kyber_dir();
    let text = fs::read_to_string(config).context("kyber_config.toml")?;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start_matches('#').trim_start();
        let replaced = if trimmed.starts_with("tls_cert =") {
            format!("tls_cert = {:?}", dir.join("kyber_cert.pem").display().to_string())
        } else if trimmed.starts_with("tls_key =") {
            format!("tls_key = {:?}", dir.join("kyber_key.pem").display().to_string())
        } else if trimmed.starts_with("webclient =") && app_resource(app, "webclient").is_dir() {
            format!("webclient = {:?}", app_resource(app, "webclient").display().to_string())
        } else {
            line.to_string()
        };
        out.push(replaced);
    }
    fs::write(config, out.join("\n") + "\n")?;
    set_mode(config, 0o600);
    Ok(())
}

fn wait_for_speakers(ctx: &Ctx, app: &Path) -> bool {
    let agent = app_exe(app, "kybernow-display");
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if let Some(out) = super::read(&agent.to_string_lossy(), &["audio", "status"])
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&out)
            && v["speakers"].is_object()
        {
            ctx.log(format!("sortie son : {}", v["speakers"]["name"].as_str().unwrap_or("KyberNow")));
            return true;
        }
        std::thread::sleep(Duration::from_millis(700));
    }
    false
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Plist d'un LaunchAgent de la session ; `run_at_load` suit « Démarrer avec le Mac ».
fn plist(label: &str, app: &Path, run_at_load: bool) -> String {
    let logs = super::logs_dir();
    let (program, extra, log): (Vec<String>, String, PathBuf) = match label {
        super::LABEL_KYBER => (
            vec![app_exe(app, "kycontroller").display().to_string()],
            format!(
                "    <key>EnvironmentVariables</key><dict><key>KYBER_CONFIG</key><string>{}</string></dict>\n    <key>WorkingDirectory</key><string>{}</string>\n    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n    <key>ThrottleInterval</key><integer>5</integer>\n",
                xml(&super::kyber_dir().join("kyber_config.toml").display().to_string()),
                xml(&super::kyber_dir().display().to_string())
            ),
            logs.join("kyber.log"),
        ),
        super::LABEL_ADMIN => (
            vec![
                app_exe(app, "kyber-admin").display().to_string(),
                "--data-dir".into(),
                super::admin_dir().display().to_string(),
                "--kyber-dir".into(),
                super::kyber_dir().display().to_string(),
            ],
            format!(
                "    <key>EnvironmentVariables</key><dict><key>KYBERNOW_KYBER_PLIST</key><string>{}</string></dict>\n    <key>WorkingDirectory</key><string>{}</string>\n    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n    <key>ThrottleInterval</key><integer>5</integer>\n",
                xml(&super::plist_path(super::LABEL_KYBER).display().to_string()),
                xml(&super::admin_dir().display().to_string())
            ),
            logs.join("admin.log"),
        ),
        _ => (vec![app_exe(app, super::MENUBAR_EXE).display().to_string()], String::new(), logs.join("menubar.log")),
    };
    let args: String = program.iter().map(|a| format!("<string>{}</string>", xml(a))).collect();
    let log = xml(&log.display().to_string());
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n    <key>Label</key><string>{label}</string>\n    <key>ProgramArguments</key><array>{args}</array>\n{extra}    <key>StandardOutPath</key><string>{log}</string>\n    <key>StandardErrorPath</key><string>{log}</string>\n    <key>RunAtLoad</key><{run}/>\n    <key>ProcessType</key><string>Interactive</string>\n    <key>LimitLoadToSessionType</key><string>Aqua</string>\n    <key>AssociatedBundleIdentifiers</key><array><string>{bundle}</string></array>\n</dict>\n</plist>\n",
        run = if run_at_load { "true" } else { "false" },
        bundle = super::BUNDLE_ID,
    )
}

/// Charge un service dans la session et le lance (même sans « Démarrer avec le Mac »).
fn load(ctx: &Ctx, label: &str) -> Result<()> {
    let plist = super::plist_path(label);
    if !super::loaded(label) {
        let out = super::launchctl(&["bootstrap", &super::domain(), &plist.to_string_lossy()])?;
        if !out.status.success() {
            bail!("launchctl bootstrap {label} : {}", super::output_text(&out));
        }
    }
    let _ = super::launchctl(&["kickstart", &format!("{}/{label}", super::domain())]);
    ctx.log(format!("launchctl bootstrap {} …{}", super::domain(), label.trim_start_matches("fr.leslignesclaires.kybernow")));
    Ok(())
}

fn init_admin(ctx: &Ctx, req: &InstallRequest, app: &Path) -> Result<()> {
    let admin = app_exe(app, "kyber-admin");
    let mut cmd = std::process::Command::new(&admin);
    cmd.arg("--data-dir")
        .arg(super::admin_dir())
        .arg("--kyber-dir")
        .arg(super::kyber_dir())
        .arg("--listen")
        .arg(format!("0.0.0.0:{}", req.console_port))
        .args(["init", "--user", req.user.trim(), "--kyber-port", &req.stream_port.to_string(), "--encoder", super::ENCODER])
        .env("KYBERNOW_PASSWORD", &req.password);
    if !req.virtual_display {
        cmd.args(["--virtual-display", "off"]);
    }
    if !req.virtual_audio {
        cmd.args(["--virtual-audio", "off"]);
    }
    let out = cmd.output().context("kyber-admin init")?;
    if !out.status.success() {
        bail!("kyber-admin init : {}", super::output_text(&out));
    }
    ctx.log(format!("kyber-admin init : compte {}, flux {}, Admin {}", req.user.trim(), req.stream_port, req.console_port));
    Ok(())
}

fn wait_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_is_well_formed() {
        let app = Path::new("/Applications/KyberNow Station.app");
        for label in super::super::LABELS {
            let p = plist(label, app, true);
            assert!(p.contains(&format!("<string>{label}</string>")));
            assert!(p.contains("<key>RunAtLoad</key><true/>"));
            assert!(p.contains(super::super::BUNDLE_ID));
        }
        assert!(plist(super::super::LABEL_ADMIN, app, false).contains("<key>RunAtLoad</key><false/>"));
    }

    #[test]
    fn kyber_config_points_at_the_station() {
        let dir = std::env::temp_dir().join(format!("kn-cfg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("kyber_config.toml");
        fs::write(&cfg, "[kycontroller]\n#tls_cert = \"/path/to/server_cert.pem\"\n#tls_key = \"/path/to/server_key.pem\"\nport = 8080\n").unwrap();
        configure_kyber(&cfg, Path::new("/nulle/part.app")).unwrap();
        let text = fs::read_to_string(&cfg).unwrap();
        assert!(text.contains("tls_cert = \""), "{text}");
        assert!(!text.contains("#tls_key"), "{text}");
        assert!(text.contains("port = 8080"));
        let _ = fs::remove_dir_all(&dir);
    }
}
