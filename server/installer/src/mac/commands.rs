//! Messages des fenêtres Mac (`ui/src/mac/bridge.ts`) : mêmes canaux que sous
//! Windows (`events`), contenus propres au Mac.

use serde::{Deserialize, Serialize};

pub use crate::events::{CheckStatus, Existing, TaskStatus};

/// Ce que l'installateur Mac envoie à la fenêtre.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Check { id: String, status: CheckStatus, title: String, detail: String },
    ChecksDone { stream_port: u16, console_port: u16, lan_ip: Option<String>, existing: Option<Existing>, audio_driver: bool },
    PortsValidated { stream_error: Option<String>, console_error: Option<String> },
    Task {
        id: String,
        status: TaskStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Progress { percent: f32 },
    Log { line: String },
    Done { lan_ip: Option<String>, stream_port: u16, console_port: u16 },
    Failed { task: String, message: String },
    Permissions { screen: bool, accessibility: bool, microphone: Mic },
    UpdateInfo { current: String, version: Option<String>, notes: Vec<String>, page: Option<String>, session: Option<String>, refusal: Option<String> },
    UpdateProgress { phase: UpdatePhase, percent: f32 },
    UpdateDone { version: String },
    UpdateFailed { message: String },
    UninstallDone,
    UninstallFailed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mic {
    Granted,
    Denied,
    Undetermined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatePhase {
    Download,
    Install,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Which {
    Screen,
    Accessibility,
    Microphone,
}

impl Which {
    pub fn arg(self) -> &'static str {
        match self {
            Which::Screen => "screen",
            Which::Accessibility => "accessibility",
            Which::Microphone => "microphone",
        }
    }
}

/// Ce que la fenêtre demande.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    RunChecks,
    ValidatePorts { stream: u16, console: u16 },
    Install(InstallRequest),
    WatchPermissions { on: bool },
    RequestPermission { which: Which },
    PermissionsDone,
    UpdateCheck,
    UpdateStart,
    Uninstall { keep_settings: bool },
    OpenUrl { url: String },
    Copy { text: String },
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
    pub virtual_audio: bool,
    pub start_with_mac: bool,
    pub keep_reachable: bool,
    /// Réinstallation qui garde comptes, ports, certificat et réglages.
    #[serde(default)]
    pub keep_settings: bool,
}

pub type Emit = std::sync::Arc<dyn Fn(Event) + Send + Sync>;
