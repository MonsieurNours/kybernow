//! Messages échangés entre l'interface (JavaScript) et l'installateur (Rust).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Running,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Running,
    Done,
    Skipped,
    /// Faite en partie ou pas faite, sans empêcher la suite (détail dans le journal).
    Warn,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct BusyPort {
    pub port: u16,
    /// Nom du programme qui l'occupe, quand Windows le donne.
    pub process: Option<String>,
}

/// KyberNow déjà installé sur cet ordinateur (`kybernow.json`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Existing {
    pub version: String,
    /// Ports en service (réglages de Kyber et de la console, sinon `kybernow.json`).
    pub stream_port: u16,
    pub console_port: u16,
    /// `kyber_config.toml` et `kyber-admin.toml` présents : « Conserver » possible.
    pub settings: bool,
}

/// Ce que l'installateur envoie à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Check { id: String, status: CheckStatus, title: String, detail: String },
    ChecksDone {
        stream_port: u16,
        console_port: u16,
        busy: Option<BusyPort>,
        lan_ip: Option<String>,
        steam_speakers: bool,
        /// `false` : déjà en Performances élevées (ou optimales), l'option est décochée.
        power_needs_tuning: bool,
        /// KyberNow déjà installé : l'interface propose de conserver les réglages.
        existing: Option<Existing>,
    },
    PortsValidated { stream_error: Option<String>, console_error: Option<String> },
    Task { id: String, status: TaskStatus },
    Progress { percent: f32 },
    Log { line: String },
    Done { lan_ip: Option<String>, stream_port: u16, console_port: u16, need_reboot: bool },
    Failed { task: String, message: String },
}

/// Ce que l'interface demande à l'installateur.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    RunChecks,
    ValidatePorts { stream: u16, console: u16 },
    Install(InstallRequest),
    OpenUrl { url: String },
    Reboot,
    Minimize,
    Drag,
    Close,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub user: String,
    pub password: String,
    pub stream_port: u16,
    pub console_port: u16,
    pub virtual_display: bool,
    pub gamepads: bool,
    /// Haut-parleurs virtuels de Steam pour le son pendant le stream.
    #[serde(default = "yes")]
    pub virtual_audio: bool,
    pub max_performance: bool,
    pub start_with_windows: bool,
    /// Réinstallation par-dessus KyberNow : garder `kyber_config.toml`, le
    /// certificat de Kyber, les comptes, les ports et les réglages de la
    /// console ; ne remplacer que les programmes, les pilotes et la surcouche.
    /// `user` et `password` sont alors ignorés.
    #[serde(default)]
    pub keep_settings: bool,
}

pub type Emit = std::sync::Arc<dyn Fn(Event) + Send + Sync>;

fn yes() -> bool {
    true
}
