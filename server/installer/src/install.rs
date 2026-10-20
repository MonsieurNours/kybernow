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
/// plus démarrer et arrêter (RP, WP) pour les utilisateurs interactifs (IU),
/// afin que l'icône pilote le serveur sans élévation. Suspendre (DT) ne sert
/// pas : il n'est pas donné. Changer la configuration (DC) reste réservé aux
/// administrateurs. Même chaîne dans `update.ps1`.
pub const SERVICE_SDDL: &str = "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)\
    (A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)S:(AU;FA;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;WD)";
/// Réglages de Kyber, relatif au dossier d'installation.
pub const KYBER_CONFIG: &str = r"kyber\kyber_config.toml";
/// Certificat TLS de Kyber (`cert-gen.exe`), relatif à `kyber\`.
pub const KYBER_CERT: [&str; 2] = ["kybertest_cert.pem", "kybertest_key.pem"];
pub const FIREWALL_RULES: &[&str] = &[
    "KyberNow-stream-TCP",
    "KyberNow-stream-UDP",
    "KyberNow-console-TCP",
    "KyberNow-bonjour-UDP",
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
    /// Ce que la désinstallation remet comme avant KyberNow.
    #[serde(default)]
    pub restore: Restore,
}

/// Réglages de Windows changés par l'installation, notés à la première
/// installation et gardés par les suivantes (et par `update.ps1`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Restore {
    /// Mode d'alimentation actif avant KyberNow (GUID).
    pub power_scheme: Option<String>,
    /// Mode Performances élevées recréé par KyberNow, à supprimer.
    pub created_power_scheme: Option<String>,
    /// Réseau passé de « Public » à « Privé ».
    pub public_network: Option<PublicNetwork>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicNetwork {
    pub interface_index: u32,
    /// Nom du réseau (profil Windows) : remis en « Public » seulement s'il n'a pas changé.
    pub name: String,
}

struct Ctx<'a> {
    emit: &'a Emit,
    dir: PathBuf,
    log: std::sync::Mutex<Vec<String>>,
    /// Repris de l'installation précédente, complété au fil des étapes.
    restore: std::sync::Mutex<Restore>,
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
    let dir = sys::install_dir();
    let restore = read_record(&dir).map(|r| r.restore).unwrap_or_default();
    let ctx = Ctx { emit, dir, log: Default::default(), restore: std::sync::Mutex::new(restore) };
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

    let keep = req.keep_settings;
    step(ctx, "files", || {
        if keep {
            check_kept_settings()?;
        }
        prepare(ctx, info)?;
        secure_data_dir(ctx);
        if keep {
            backup_kyber_config(ctx)?;
        }
        ctx.progress(4.0);
        ctx.log(format!("Extraction vers {}", ctx.dir.display()));
        // Réglages de Kyber gardés : la copie d'origine du contenu n'est pas écrite.
        let skip = |p: &Path| keep && p.components().map(|c| c.as_os_str()).eq(Path::new(KYBER_CONFIG).components().map(|c| c.as_os_str()));
        let count = crate::payload::extract(payload, &ctx.dir, skip, |p, _| ctx.progress(4.0 + p * 36.0))?;
        ctx.log(format!("{count} fichiers installés"));
        // Désinstallable dès maintenant : une installation interrompue plus
        // loin se retire depuis « Applications installées ».
        write_record(ctx, req, None)?;
        let uninstaller = ctx.dir.join("uninstall.exe");
        crate::payload::copy_stub(payload, &uninstaller)?;
        register_uninstaller(ctx, &uninstaller)?;
        ctx.log("Inscrit dans « Applications installées » (désinstallation possible)");
        Ok(())
    })?;

    step(ctx, "cert", || {
        let kyber = ctx.kyber();
        if keep && KYBER_CERT.iter().all(|f| kyber.join(f).is_file()) {
            ctx.log("Certificat TLS de Kyber conservé : les appareils le reconnaissent déjà");
            ctx.progress(44.0);
            return Ok(());
        }
        let out = run_ok(&kyber.join("cert-gen.exe").display().to_string(), &["--output", &kyber.display().to_string()], &[0])?;
        ctx.log(format!("Certificat TLS généré. {}", sys::text(&out).trim()));
        ctx.progress(44.0);
        Ok(())
    })?;

    step(ctx, "service", || {
        if keep {
            ctx.log(format!(
                "Réglages conservés : comptes, encodeur, ports (flux {}, console {}), mot de passe de la console",
                req.stream_port, req.console_port
            ));
        } else {
            init_settings(ctx, req, info)?;
        }
        create_kyber_service(ctx, req)?;
        ctx.progress(52.0);
        Ok(())
    })?;

    step(ctx, "firewall", || {
        if let Some(index) = info.public_interface {
            let name = sys::powershell(&format!("(Get-NetConnectionProfile -InterfaceIndex {index}).Name")).unwrap_or_default();
            let r = sys::powershell(&format!(
                "Set-NetConnectionProfile -InterfaceIndex {index} -NetworkCategory Private"
            ));
            ctx.log(format!("Réseau passé en « Privé » : {}", if r.is_ok() { "réussi" } else { "échec" }));
            let mut restore = ctx.restore.lock().unwrap();
            if r.is_ok() && !name.is_empty() && restore.public_network.is_none() {
                restore.public_network = Some(PublicNetwork { interface_index: index, name });
            }
            drop(restore);
            write_record(ctx, req, None)?;
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
        // Annonce Bonjour de la console : l'app la trouve seule sur le réseau local.
        let admin = format!("program={}", ctx.path(r"admin\kyber-admin.exe"));
        run_ok("netsh", &["advfirewall", "firewall", "add", "rule", "name=KyberNow-bonjour-UDP", "dir=in", "action=allow",
            "protocol=UDP", "localport=5353", &admin, profile], &[0])?;
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
            // Empreinte du certificat notée tout de suite : le désinstalleur le retire.
            write_record(ctx, req, thumbprint.clone())?;
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
            ctx.log("Haut-parleurs virtuels de Steam installés : le son de l'ordinateur sera coupé pendant le stream");
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
        // Jamais bloquant : beaucoup de portables n'ont pas ce mode, ou le refusent.
        ctx.task("power", TaskStatus::Running);
        let before = run("powercfg", &["/getactivescheme"]).ok().and_then(|o| find_guid(&sys::text(&o), ""));
        match set_high_performance() {
            Ok((how, created)) => {
                let mut restore = ctx.restore.lock().unwrap();
                if restore.power_scheme.is_none() {
                    restore.power_scheme = before.filter(|g| Some(g) != created.as_ref() && g != HIGH_PERFORMANCE_SCHEME);
                }
                if created.is_some() {
                    restore.created_power_scheme = created;
                }
                drop(restore);
                let _ = write_record(ctx, req, None);
                ctx.log(format!("Mode d'alimentation : Performances élevées{how}"));
                ctx.task("power", TaskStatus::Done);
            }
            Err(e) => {
                ctx.log(format!(
                    "ATTENTION : mode d'alimentation inchangé ({e:#}). Choisissez « Performances élevées » dans les options d'alimentation de Windows si cet ordinateur le propose."
                ));
                ctx.task("power", TaskStatus::Warn);
            }
        }
        ctx.progress(96.0);
    } else {
        ctx.task("power", TaskStatus::Skipped);
    }

    step(ctx, "finish", || {
        write_record(ctx, req, thumbprint.clone())?;
        // Taille finale, pilotes compris.
        let _ = run("reg", &["add", UNINSTALL_KEY, "/v", "EstimatedSize", "/t", "REG_DWORD", "/d", &(dir_size(&ctx.dir) / 1024).to_string(), "/f"]);
        write_shortcut(req.console_port);
        run_ok("sc", &["start", "kyber"], &[0, 1056])?; // 1056 : déjà démarré
        // Démarré pour de bon : service en marche, puis serveur et console qui répondent.
        if !wait_running("kyber", Duration::from_secs(30)) {
            ctx.log("ATTENTION : le service « kyber » n'est pas passé en marche en 30 s (voir C:\\Program Files\\KyberNow\\kyber\\log)");
        } else if !port_answers(req.stream_port, Duration::from_secs(30)) {
            ctx.log(format!("ATTENTION : le serveur ne répond pas encore sur le port {}", req.stream_port));
        } else {
            ctx.log(format!("Serveur de streaming démarré (port {})", req.stream_port));
        }
        if port_answers(req.console_port, Duration::from_secs(15)) {
            ctx.log(format!("Console en ligne (port {})", req.console_port));
        } else {
            ctx.log(format!("ATTENTION : la console ne répond pas encore sur le port {}", req.console_port));
        }
        setup_tray(ctx, req.start_with_windows);
        ctx.progress(100.0);
        Ok(())
    })?;

    Ok(need_reboot)
}

/// Mode « Performances élevées » de Windows.
pub const HIGH_PERFORMANCE_SCHEME: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

/// Active Performances élevées. Absent de la liste (portables, Windows
/// récents), il est recréé à partir du modèle de Windows
/// (`powercfg /duplicatescheme`), puis activé. Renvoie un complément pour le
/// journal et le GUID du mode recréé.
fn set_high_performance() -> Result<(&'static str, Option<String>)> {
    if run_ok("powercfg", &["/setactive", HIGH_PERFORMANCE_SCHEME], &[0]).is_ok() {
        return Ok(("", None));
    }
    let out = run_ok("powercfg", &["/duplicatescheme", HIGH_PERFORMANCE_SCHEME], &[0])?;
    let guid = find_guid(&sys::text(&out), HIGH_PERFORMANCE_SCHEME).context("identifiant du mode recréé introuvable")?;
    if let Err(e) = run_ok("powercfg", &["/setactive", &guid], &[0]) {
        let _ = run("powercfg", &["/delete", &guid]);
        return Err(e);
    }
    Ok((" (recréé à partir du modèle de Windows)", Some(guid)))
}

/// Premier GUID du texte (8-4-4-4-12 chiffres hexadécimaux) autre que `except`.
pub fn find_guid(text: &str, except: &str) -> Option<String> {
    let bytes = text.as_bytes();
    (0..bytes.len().saturating_sub(35)).find_map(|i| {
        let candidate = text.get(i..i + 36)?;
        let shape = candidate.char_indices().all(|(j, c)| if [8, 13, 18, 23].contains(&j) { c == '-' } else { c.is_ascii_hexdigit() });
        (shape && !candidate.eq_ignore_ascii_case(except)).then(|| candidate.to_ascii_lowercase())
    })
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

/// `C:\ProgramData\KyberNow` réservé à SYSTEM et aux Administrateurs, avant
/// que `kyber-admin init` y écrive l'empreinte du mot de passe (voir `acl`).
/// La console reprend la pose à chaque démarrage : un échec ici est signalé
/// dans le journal sans arrêter l'installation.
fn secure_data_dir(ctx: &Ctx) {
    let dir = sys::data_dir();
    match crate::acl::secure_dir(&dir) {
        Ok(0) => ctx.log(format!("Droits de {} : SYSTEM et Administrateurs seulement", dir.display())),
        Ok(n) => ctx.log(format!("Droits de {} posés, {n} lien(s) retiré(s)", dir.display())),
        Err(e) => ctx.log(format!("ATTENTION : droits de {} non posés ({e:#}) ; la console réessaiera à son démarrage", dir.display())),
    }
}

/// `kyber-admin init` : port, encodeur, compte unique, secret JWT, mot de
/// passe de la console (installation neuve, ou « Repartir de zéro »).
fn init_settings(ctx: &Ctx, req: &InstallRequest, info: &SystemInfo) -> Result<()> {
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
    Ok(())
}

/// Service `kyber` : chemin entre guillemets, redémarrage en cas d'échec,
/// démarrer et arrêter ouverts aux utilisateurs interactifs.
fn create_kyber_service(ctx: &Ctx, req: &InstallRequest) -> Result<()> {
    let bin = format!("\"{}\"", ctx.kyber().join("kyservice.exe").display());
    let start = if req.start_with_windows { "auto" } else { "demand" };
    run_ok("sc", &["create", "kyber", "binPath=", &bin, "start=", start, "DisplayName=", "KyberNow - serveur de streaming"], &[0])?;
    let _ = run("sc", &["description", "kyber", "Serveur de streaming Kyber, installé et géré par KyberNow."]);
    let _ = run("sc", &["failure", "kyber", "reset=", "86400", "actions=", "restart/5000/restart/5000/restart/30000"]);
    run_ok("sc", &["sdset", "kyber", SERVICE_SDDL], &[0])?;
    ctx.log(format!("Service Windows « kyber » créé (démarrage {start})"));
    Ok(())
}

/// « Conserver mes réglages » : ceux de Kyber et de la console doivent exister.
fn check_kept_settings() -> Result<()> {
    let kyber = sys::install_dir().join(KYBER_CONFIG);
    let admin = sys::data_dir().join("kyber-admin.toml");
    for file in [&kyber, &admin] {
        if !file.is_file() {
            anyhow::bail!("réglages introuvables ({}) : choisissez « Repartir de zéro »", file.display());
        }
    }
    Ok(())
}

/// Copie de `kyber_config.toml` dans `C:\ProgramData\KyberNow\backups`
/// avant l'extraction (qui ne l'écrase pas), par précaution.
fn backup_kyber_config(ctx: &Ctx) -> Result<()> {
    let from = ctx.dir.join(KYBER_CONFIG);
    let dir = sys::data_dir().join("backups");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let to = dir.join(format!("kyber_config.toml.reinstallation-{stamp}"));
    std::fs::copy(&from, &to).with_context(|| format!("sauvegarde de {}", from.display()))?;
    ctx.log(format!("Réglages de Kyber gardés (copie : {})", to.display()));
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

/// Attend qu'un service soit arrêté (20 s au plus), ou qu'il n'existe plus.
pub fn wait_stopped(service: &str) {
    for _ in 0..40 {
        match service_state(service) {
            None | Some(1) => return, // SERVICE_STOPPED
            Some(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// État d'un service d'après `sc query` : « STATE : 4 RUNNING » (« ÉTAT » en
/// français) donne 4. Le chiffre ne dépend pas de la langue de Windows.
fn service_state(service: &str) -> Option<u32> {
    let out = run("sc", &["query", service]).ok().filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    parse_state(&text)
}

pub fn parse_state(sc_query: &str) -> Option<u32> {
    sc_query
        .lines()
        .filter(|l| l.to_uppercase().contains("STATE") || l.to_uppercase().contains("TAT"))
        .find_map(|l| l.split_once(':')?.1.split_whitespace().next()?.parse().ok())
}

/// Attend qu'un service soit en marche (`SERVICE_RUNNING`, 4).
fn wait_running(service: &str, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if service_state(service) == Some(4) {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Un programme accepte une connexion TCP sur ce port de l'ordinateur.
fn port_answers(port: u16, timeout: Duration) -> bool {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok() {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
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

/// `kybernow.json` : ce qui est installé, relu par le désinstalleur, l'icône
/// (port de la console) et une réinstallation.
fn write_record(ctx: &Ctx, req: &InstallRequest, sudovda_thumbprint: Option<String>) -> Result<()> {
    // Empreinte déjà notée (étape vdd) : gardée quand on réécrit sans elle.
    let sudovda_thumbprint = sudovda_thumbprint.or_else(|| read_record(&ctx.dir).and_then(|r| r.sudovda_thumbprint).filter(|_| req.virtual_display));
    let record = InstallRecord {
        version: crate::VERSION.into(),
        kyber_version: crate::KYBER_VERSION.into(),
        stream_port: req.stream_port,
        console_port: req.console_port,
        virtual_display: req.virtual_display,
        gamepads: req.gamepads,
        sudovda_thumbprint,
        restore: ctx.restore.lock().unwrap().clone(),
    };
    std::fs::write(ctx.dir.join("kybernow.json"), serde_json::to_vec_pretty(&record)?)?;
    Ok(())
}

/// `kybernow.json` d'une installation, s'il est lisible.
pub fn read_record(dir: &Path) -> Option<InstallRecord> {
    serde_json::from_slice(&std::fs::read(dir.join("kybernow.json")).ok()?).ok()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_of_the_duplicated_power_scheme() {
        let out = "GUID du mode de gestion de l'alimentation : 3F2A9C1E-7B4D-4E8A-9C6F-1D2E3F4A5B6C  (Performances élevées)";
        assert_eq!(find_guid(out, HIGH_PERFORMANCE_SCHEME).as_deref(), Some("3f2a9c1e-7b4d-4e8a-9c6f-1d2e3f4a5b6c"));
        let echo = format!("{HIGH_PERFORMANCE_SCHEME} -> 11111111-2222-3333-4444-555555555555");
        assert_eq!(find_guid(&echo, HIGH_PERFORMANCE_SCHEME).as_deref(), Some("11111111-2222-3333-4444-555555555555"));
        assert_eq!(find_guid("Paramètre non valide.", HIGH_PERFORMANCE_SCHEME), None);
    }

    #[test]
    fn service_state_whatever_the_language() {
        let en = "SERVICE_NAME: kyber\r\n        TYPE               : 10  WIN32_OWN_PROCESS\r\n        STATE              : 4  RUNNING\r\n";
        assert_eq!(parse_state(en), Some(4));
        let fr = "NOM_SERVICE : kyber\r\n        TYPE               : 10  WIN32_OWN_PROCESS\r\n        ÉTAT               : 2  START_PENDING\r\n";
        assert_eq!(parse_state(fr), Some(2));
    }
}
