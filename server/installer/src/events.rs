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
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct BusyPort {
    pub port: u16,
    /// Nom du programme qui l'occupe, quand Windows le donne.
    pub process: Option<String>,
}

/// Ce que l'installateur envoie à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Check { id: String, status: CheckStatus, title: String, detail: String },
    ChecksDone { stream_port: u16, console_port: u16, busy: Option<BusyPort>, lan_ip: Option<String> },
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
    pub max_performance: bool,
    pub start_with_windows: bool,
}

pub type Emit = std::sync::Arc<dyn Fn(Event) + Send + Sync>;
